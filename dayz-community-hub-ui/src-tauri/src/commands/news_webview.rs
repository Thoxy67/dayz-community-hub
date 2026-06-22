//! Cloudflare-challenge fallback for the DayZ news feed.
//!
//! dayz.com fronts its `/api/article` endpoint with a Cloudflare managed
//! challenge ("Just a moment…") that a plain HTTP client can't pass. A real
//! browser engine *can* — the challenge auto-solves in a few seconds and sets a
//! `cf_clearance` cookie bound to that browser's TLS/JS fingerprint (so the
//! cookie can't be replayed via reqwest). We therefore fetch the JSON from
//! inside a Tauri WebView and hand it back over IPC.
//!
//! Flow:
//!   1. Open a hidden dayz.com window with an injected fetch-retry script.
//!   2. The challenge auto-solves; the script's `fetch('/api/article')` then
//!      succeeds (same-origin, with cf_clearance) and invokes
//!      `news_webview_result` with the JSON body.
//!   3. If nothing comes back within a few seconds (an *interactive* challenge),
//!      the window is revealed so the user can solve it themselves.
//!   4. On success the window is closed and the feed renders normally. If it had
//!      to be revealed and still timed out, the window is left open so the user
//!      can keep reading the real site.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dayz_community_hub_core::news::{self, Article};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const NEWS_WINDOW_LABEL: &str = "news-fetch";
/// Reveal the (otherwise hidden) window after this long with no result — at that
/// point the challenge almost certainly needs human interaction.
const REVEAL_AFTER_SECS: u64 = 8;
/// Overall ceiling, generous enough to allow manual solving.
const OVERALL_TIMEOUT_SECS: u64 = 150;

/// Injected into the news window on every navigation. Polls the JSON API until
/// it returns real JSON (i.e. once the Cloudflare challenge has been passed),
/// then ships the body back to Rust via IPC exactly once.
const INIT_SCRIPT: &str = r#"
(function () {
  if (window.__dzchNewsPolling) return;
  window.__dzchNewsPolling = true;
  var done = false;
  async function attempt() {
    if (done) return;
    try {
      var r = await fetch('/api/article?rowsPerPage=50', { credentials: 'include' });
      var ct = (r.headers.get('content-type') || '').toLowerCase();
      if (r.ok && ct.indexOf('json') !== -1) {
        var txt = await r.text();
        if (txt && txt.charAt(0) === '{') {
          done = true;
          clearInterval(iv);
          try {
            await window.__TAURI_INTERNALS__.invoke('news_webview_result', { payload: txt });
          } catch (e) {}
        }
      }
    } catch (e) {}
  }
  var iv = setInterval(attempt, 1500);
  attempt();
})();
"#;

/// Holds the one-shot sender that the in-page script's IPC result is routed to.
/// Managed once at startup; the active fetch fills the slot for the duration of
/// one attempt.
pub(crate) struct NewsWebviewState(pub Mutex<Option<tokio::sync::oneshot::Sender<String>>>);

impl NewsWebviewState {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(None))
    }
}

/// IPC target invoked by the in-page script with the fetched JSON body.
#[tauri::command]
pub(crate) fn news_webview_result(payload: String, state: tauri::State<'_, NewsWebviewState>) {
    if let Ok(mut guard) = state.0.lock()
        && let Some(tx) = guard.take()
    {
        let _ = tx.send(payload);
    }
}

/// Fetch the news JSON via a WebView that can clear the Cloudflare challenge.
pub(crate) async fn fetch_news_via_webview(app: &AppHandle) -> Result<Vec<Article>, String> {
    // Drop any lingering window from a previous attempt.
    if let Some(existing) = app.get_webview_window(NEWS_WINDOW_LABEL) {
        let _ = existing.close();
    }

    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    {
        let state = app.state::<NewsWebviewState>();
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "news state poisoned".to_string())?;
        *guard = Some(tx);
    }

    // Window creation must happen on the main thread (required on Windows/macOS).
    let (build_tx, build_rx) = std::sync::mpsc::channel();
    let app_for_build = app.clone();
    app.run_on_main_thread(move || {
        let url = WebviewUrl::External("https://dayz.com/news".parse().unwrap());
        let result = WebviewWindowBuilder::new(&app_for_build, NEWS_WINDOW_LABEL, url)
            .title("DayZ News")
            .inner_size(1000.0, 760.0)
            .center()
            .visible(false)
            .initialization_script(INIT_SCRIPT)
            .build();
        let _ = build_tx.send(result.map_err(|e| e.to_string()));
    })
    .map_err(|e| format!("could not schedule news window: {e}"))?;

    let window = build_rx
        .recv()
        .map_err(|_| "news window build channel dropped".to_string())??;

    // Reveal the window only if the challenge hasn't resolved on its own — i.e.
    // it likely needs the user to interact.
    let revealed = Arc::new(AtomicBool::new(false));
    let reveal_window = window.clone();
    let reveal_flag = Arc::clone(&revealed);
    let reveal_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(REVEAL_AFTER_SECS)).await;
        reveal_flag.store(true, Ordering::SeqCst);
        let _ = reveal_window.show();
        let _ = reveal_window.set_focus();
    });

    let outcome = tokio::time::timeout(Duration::from_secs(OVERALL_TIMEOUT_SECS), rx).await;
    reveal_task.abort();

    // Clear the sender slot regardless of outcome.
    if let Ok(mut guard) = app.state::<NewsWebviewState>().0.lock() {
        *guard = None;
    }

    match outcome {
        Ok(Ok(body)) => {
            // Got the JSON — tear the window down and render the integrated feed.
            let _ = window.close();
            news::parse_news_json(&body).map_err(|e| e.to_string())
        }
        _ => {
            // Timed out / channel closed. If we'd revealed the window, leave it
            // open so the user can keep reading the real site; otherwise close it.
            if !revealed.load(Ordering::SeqCst) {
                let _ = window.close();
            }
            Err("Could not load DayZ news (Cloudflare challenge not completed)".to_string())
        }
    }
}
