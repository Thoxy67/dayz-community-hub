//! Refreshing the server list, as a task of the backend's own.
//!
//! The window used to drive a refresh step by step (stop the scan, fetch,
//! load, scan again), so a reload in the middle lost the last steps, and the
//! reloaded window, seeing a cached list, started a second refresh. Now
//! `servers_refresh` starts the whole sequence and returns at once; the
//! window follows it through `refresh-changed` and, after a reload, asks
//! [`refresh_state`] where it stands.
//!
//! Returning at once matters for another reason: WebView2 and WebKitGTK
//! re-send an invoke still pending when the page reloads (tauri-apps/tauri
//! #14154, "The asynchronous command is re-invoked when the WebView
//! reloads"). A start that is a no-op while one runs is safe either way.

use std::sync::{Arc, LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::features::ping::{PingState, now_ms};
use crate::state::SharedState;

/// Where a refresh has got.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RefreshPhase {
    /// None has run in this process.
    #[default]
    Idle,
    /// Asking the launcher list (and Steam's official servers).
    Fetching,
    Done,
    Failed,
}

/// The list's refresh as it stands. Times are Unix ms.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, specta::Type)]
pub struct RefreshState {
    /// Which refresh: one more for each started. 0 before the first.
    pub id: u32,
    pub running: bool,
    pub phase: RefreshPhase,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    /// When the list on hand was fetched from the network, by `initialize`
    /// or a refresh; null while it is the on-disk cache's.
    pub loaded_at: Option<i64>,
    /// Servers in the list after the last refresh that worked.
    pub count: u32,
    /// Why the last fetch failed.
    pub error: Option<String>,
}

/// The refresh started, or ended; carries where it stands.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct RefreshChanged(pub RefreshState);

static STATE: LazyLock<Mutex<RefreshState>> = LazyLock::new(Mutex::default);

fn lock() -> std::sync::MutexGuard<'static, RefreshState> {
    STATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn snapshot() -> RefreshState {
    lock().clone()
}

/// What `initialize` learnt about the list it loaded: fetched now (`Ok`
/// with the time), from the cache (`Ok(None)`), or not at all.
pub(crate) fn initialized(loaded: Result<Option<i64>, String>) {
    let mut s = lock();
    match loaded {
        Ok(at) => {
            s.loaded_at = at;
            s.error = None;
        }
        Err(e) => s.error = Some(e),
    }
}

/// Mark a refresh started, unless one runs. Returns its id when this call
/// started it.
fn begin() -> (RefreshState, Option<u32>) {
    let mut s = lock();
    if s.running {
        return (s.clone(), None);
    }
    s.id += 1;
    s.running = true;
    s.phase = RefreshPhase::Fetching;
    s.started_at = Some(now_ms());
    s.finished_at = None;
    s.error = None;
    (s.clone(), Some(s.id))
}

/// Mark refresh `id` over with its outcome.
fn end(id: u32, outcome: &Result<u32, String>) -> RefreshState {
    let mut s = lock();
    if s.id == id {
        let now = now_ms();
        s.running = false;
        s.finished_at = Some(now);
        match outcome {
            Ok(count) => {
                s.phase = RefreshPhase::Done;
                s.count = *count;
                s.loaded_at = Some(now);
                s.error = None;
            }
            Err(e) => {
                s.phase = RefreshPhase::Failed;
                s.error = Some(e.clone());
            }
        }
    }
    s.clone()
}

/// Fetch the list again in the background: the ping scan stops, the list is
/// fetched and swapped in, and the scan starts again over the new one. A
/// call while a refresh runs starts nothing and returns that one. Returns at
/// once; `refresh-changed` says when it is over.
#[tauri::command]
#[specta::specta]
pub(crate) async fn servers_refresh(app: AppHandle) -> Result<RefreshState, String> {
    let (now, started) = begin();
    let Some(id) = started else {
        return Ok(now);
    };
    let _ = RefreshChanged(now.clone()).emit(&app);
    tauri::async_runtime::spawn(async move {
        let ping = Arc::clone(&app.state::<Arc<PingState>>());
        // The scan would measure a list about to be replaced.
        ping.stop();
        let state = app.try_state::<SharedState>().map(|s| Arc::clone(&s));
        let outcome = match &state {
            Some(state) => super::refresh_list(app.clone(), state).await,
            None => Err("The launcher has not started yet".to_string()),
        };
        let done = end(id, &outcome);
        let _ = RefreshChanged(done).emit(&app);
        if let (Ok(_), Some(state)) = (&outcome, &state) {
            crate::features::browser::scan_whole(&app, state, &ping).await;
        }
    });
    Ok(now)
}

/// The refresh as it stands, for a window that has just loaded.
#[tauri::command]
#[specta::specta]
pub(crate) async fn refresh_state() -> RefreshState {
    snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test for the one process-wide state: tests run in parallel.
    #[test]
    fn a_refresh_runs_once_and_records_its_outcome() {
        initialized(Ok(None));
        assert_eq!(snapshot().loaded_at, None);

        let (s, id) = begin();
        let id = id.expect("nothing ran: this one starts");
        assert!(s.running);
        assert_eq!(s.phase, RefreshPhase::Fetching);
        assert!(s.started_at.is_some());

        // A second start while it runs is the same refresh.
        let (again, none) = begin();
        assert_eq!(none, None);
        assert_eq!(again.id, id);

        let s = end(id, &Ok(42));
        assert!(!s.running);
        assert_eq!(s.phase, RefreshPhase::Done);
        assert_eq!(s.count, 42);
        assert!(s.loaded_at.is_some());

        let (_, next) = begin();
        let next = next.unwrap();
        let s = end(next, &Err("down".into()));
        assert_eq!(s.phase, RefreshPhase::Failed);
        assert_eq!(s.error.as_deref(), Some("down"));
        // The list on hand is still the one fetched before.
        assert_eq!(s.count, 42);
        assert!(s.loaded_at.is_some());

        // A stale id ends nothing.
        let (_, third) = begin();
        let s = end(next, &Ok(1));
        assert!(s.running);
        end(third.unwrap(), &Ok(1));
    }
}
