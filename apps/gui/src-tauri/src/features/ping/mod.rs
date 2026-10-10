//! Pinging servers over A2S: the scan of the whole list (driven by Rust,
//! reporting only its progress), small explicit lists, and one server.
//!
//! Every result lands in the live store (`browser::live`), which the browser
//! reads; nothing here touches the app state's lock.

use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_specta::Event;
use tokio::sync::mpsc;

use crate::features::browser::live::{self, Live};

/// Default timeout for an explicit list.
const LIST_TIMEOUT_MS: u64 = 5_000;
/// Default timeout of a manual ping.
const MANUAL_TIMEOUT_MS: u64 = 10_000;
/// Default concurrency for an explicit list.
const LIST_CONCURRENT: usize = 10;
/// Results per Channel message for an explicit list.
const BATCH_SIZE: usize = 50;
/// How often partial batches of an explicit list are sent (at most).
const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
/// How often a whole-list scan's progress is sent (at most).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);

/// One ping, as the window receives it.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct PingResultDto {
    pub ip: String,
    pub port: i64,
    pub ms: u32,
    pub players: Option<u8>,
    pub max_players: Option<u8>,
    /// Bots reported by A2S_INFO (DayZ servers often pad this to fake a full server).
    pub bots: Option<u8>,
    /// True when the query failed (timeout or error).
    pub failed: bool,
}

fn to_dto(l: &Live, ip: String, port: i64) -> PingResultDto {
    PingResultDto {
        ip,
        port,
        ms: l.ms.unwrap_or(dz_a2s::PING_TIMEOUT_SENTINEL),
        players: l.players,
        max_players: l.max_players,
        bots: l.bots,
        failed: l.failed,
    }
}

/// How far the scan of the whole list has got.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
pub struct ScanProgress {
    pub done: u32,
    pub total: u32,
    pub paused: bool,
    pub running: bool,
}

/// The progress of a whole-list scan the window did not start (servers
/// added to the list after its own scan ended), for the status bar.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, specta::Type, Event)]
pub struct PingScanProgress(pub ScanProgress);

/// The scan's controls, managed by the app on its own.
#[derive(Default)]
pub struct PingState {
    /// Read by every scan loop on every result: an atomic, not a lock.
    paused: AtomicBool,
    /// The running scan, so a new one (or a refresh) can abort it.
    background: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// The running whole-list scan's queue, which servers added to the list
    /// meanwhile join ([`enqueue`]).
    queue: Mutex<Option<Queue>>,
}

struct Queue {
    /// Targets, with the tries each has had.
    tx: mpsc::UnboundedSender<(String, u32)>,
    /// Targets sent so far: the scan is over when it has done as many.
    total: Arc<AtomicU32>,
}

impl PingState {
    pub(crate) fn abort_background(&self) {
        if let Some(handle) = self.background.lock().ok().and_then(|mut h| h.take()) {
            handle.abort();
        }
        if let Ok(mut q) = self.queue.lock() {
            *q = None;
        }
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
}

/// Parse "ip:port".
pub(crate) fn parse_target(target: &str) -> Option<(&str, i64)> {
    let (ip, port) = target.rsplit_once(':')?;
    Some((ip, port.parse().ok()?))
}

/// What asking one target came to.
enum Asked {
    Done(String, i64, Live),
    /// No answer, tries left: to the back of the queue.
    Again(String, u32),
    /// Not "ip:port": counted done, nothing recorded.
    Invalid,
}

/// Where a scan's results go besides the live store.
enum Sink {
    /// Batches of results, for explicit lists.
    Batches(Channel<Vec<PingResultDto>>),
    /// Progress only, for the whole-list scan.
    Progress(Channel<ScanProgress>),
    /// `ping-scan-progress` events: a scan the window did not ask for.
    Event(AppHandle),
}

/// Ping `targets` with `concurrency` queries in flight, recording each result
/// in the live store as it lands. A server that does not answer is asked
/// again up to `retries` times, after the targets queued before it: one lost
/// UDP packet should not mark it dead, nor should it hold a slot meanwhile.
/// A `whole` scan takes more targets while it runs ([`enqueue`]).
async fn scan(
    targets: Vec<String>,
    concurrency: usize,
    timeout: Duration,
    retries: u32,
    sink: Sink,
    ping: Arc<PingState>,
    whole: bool,
) {
    // Each target with the number of tries it has had: one that timed out
    // goes to the back of the queue rather than being asked again at once,
    // so the silent ones are retried in a short pass at the end instead of
    // each holding a slot for `retries + 1` timeouts.
    let (tx, rx) = mpsc::unbounded_channel::<(String, u32)>();
    let total = Arc::new(AtomicU32::new(0));
    for t in targets {
        if tx.send((t, 0)).is_ok() {
            total.fetch_add(1, Ordering::Relaxed);
        }
    }
    let again = tx.clone();
    if whole && let Ok(mut q) = ping.queue.lock() {
        *q = Some(Queue {
            tx,
            total: Arc::clone(&total),
        });
    }
    let done = Arc::new(AtomicU32::new(0));
    let progress = |running: bool| ScanProgress {
        done: done.load(Ordering::Relaxed),
        total: total.load(Ordering::Relaxed),
        paused: ping.is_paused(),
        running,
    };
    // A scan is over when it has done every target it was sent (retries
    // keep the queue open, so it never runs dry by itself). A whole scan's
    // queue then closes, under its lock so nothing is sent into it after.
    let finished = || {
        if done.load(Ordering::Relaxed) < total.load(Ordering::Relaxed) {
            return false;
        }
        if !whole {
            return true;
        }
        let Ok(mut q) = ping.queue.lock() else {
            return true;
        };
        if done.load(Ordering::Relaxed) < total.load(Ordering::Relaxed) {
            return false;
        }
        *q = None;
        true
    };

    // One client for the whole scan. async-a2s binds a socket per query, so
    // the sockets open at once are the queries in flight, bounded by
    // `concurrency`: an unbounded burst runs Winsock out of buffer space
    // (WSAENOBUFS, 10055, learn.microsoft.com "Windows Sockets Error Codes").
    // Its own retries are off: a timeout here is retried by the queue.
    let Ok(mut client) = dz_a2s::client_with_timeout(timeout).await else {
        if let Sink::Progress(ch) = &sink {
            let _ = ch.send(progress(false));
        }
        return;
    };
    client.retries(0);
    let client = Arc::new(client);

    let targets = Box::pin(stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|t| (t, rx))
    }));
    let mut results = targets
        .map(|(target, tries)| {
            let client = Arc::clone(&client);
            async move {
                let Some((ip, port)) = parse_target(&target) else {
                    return Asked::Invalid;
                };
                // The challenge and its answer, both within `timeout`.
                let r = tokio::time::timeout(timeout, dz_a2s::ping_using(&client, &target))
                    .await
                    .unwrap_or_else(|_| Err(dz_common::Error::A2sQuery("timeout".into())));
                if r.is_err() && tries < retries {
                    return Asked::Again(target, tries + 1);
                }
                let l = Live::from_result(&r);
                live::store().record(ip, port, l);
                Asked::Done(ip.to_owned(), port, l)
            }
        })
        .buffer_unordered(concurrency)
        .fuse();

    let mut batch: Vec<PingResultDto> = Vec::new();
    let mut flush = tokio::time::interval(if whole {
        PROGRESS_INTERVAL
    } else {
        FLUSH_INTERVAL
    });
    flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Progress is sent only when it moved: a paused scan, or one waiting on
    // slow servers, would otherwise repeat itself on every tick.
    let mut last_sent: Option<ScanProgress> = None;
    let mut send = |batch: &mut Vec<PingResultDto>| {
        let fresh = |p: ScanProgress, last: &mut Option<ScanProgress>| {
            let moved = *last != Some(p);
            *last = Some(p);
            moved
        };
        match &sink {
            Sink::Batches(ch) if !batch.is_empty() => {
                let _ = ch.send(std::mem::take(batch));
            }
            Sink::Progress(ch) => {
                let p = progress(true);
                if fresh(p, &mut last_sent) {
                    let _ = ch.send(p);
                }
            }
            Sink::Event(app) => {
                let p = progress(true);
                if fresh(p, &mut last_sent) {
                    let _ = PingScanProgress(p).emit(app);
                }
            }
            _ => {}
        }
    };

    loop {
        if finished() {
            break;
        }
        if ping.is_paused() {
            flush.tick().await;
            send(&mut batch);
            continue;
        }
        tokio::select! {
            biased;
            next = results.next() => match next {
                None => break,
                Some(Asked::Again(target, tries)) => {
                    if again.send((target, tries)).is_err() {
                        done.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Some(Asked::Invalid) => {
                    done.fetch_add(1, Ordering::Relaxed);
                }
                Some(Asked::Done(ip, port, l)) => {
                    done.fetch_add(1, Ordering::Relaxed);
                    if let Sink::Batches(_) = &sink {
                        batch.push(to_dto(&l, ip, port));
                        if batch.len() >= BATCH_SIZE {
                            send(&mut batch);
                        }
                    }
                }
            },
            _ = flush.tick() => send(&mut batch),
        }
    }
    match &sink {
        Sink::Batches(ch) if !batch.is_empty() => {
            let _ = ch.send(batch);
        }
        Sink::Progress(ch) => {
            let _ = ch.send(progress(false));
        }
        Sink::Event(app) => {
            let _ = PingScanProgress(progress(false)).emit(app);
        }
        _ => {}
    }
}

/// Start the scan of the whole list, replacing any running one. The order
/// and settings come from the profile; see `browser::scan_targets`.
pub(crate) fn start_whole_scan(
    ping: &Arc<PingState>,
    targets: Vec<String>,
    concurrency: usize,
    timeout: Duration,
    retries: u32,
    on_progress: Channel<ScanProgress>,
) {
    ping.abort_background();
    let state = Arc::clone(ping);
    let handle = tauri::async_runtime::spawn(async move {
        scan(
            targets,
            concurrency,
            timeout,
            retries,
            Sink::Progress(on_progress),
            state,
            true,
        )
        .await;
    });
    if let Ok(mut h) = ping.background.lock() {
        *h = Some(handle);
    }
}

/// Have `targets` pinged with the whole-list scan: into its queue when one
/// runs (its progress grows), else in a scan of their own, whose progress
/// the status bar hears as `ping-scan-progress`.
pub(crate) fn enqueue(
    app: &AppHandle,
    ping: &Arc<PingState>,
    targets: Vec<String>,
    concurrency: usize,
    timeout: Duration,
    retries: u32,
) {
    if targets.is_empty() {
        return;
    }
    if let Ok(mut q) = ping.queue.lock()
        && let Some(queue) = q.as_ref()
    {
        let mut sent = true;
        for t in &targets {
            if queue.tx.send((t.clone(), 0)).is_err() {
                sent = false;
                break;
            }
            queue.total.fetch_add(1, Ordering::Relaxed);
        }
        if sent {
            return;
        }
        // The scan was stopped without closing its queue.
        *q = None;
    }
    let state = Arc::clone(ping);
    let app = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        scan(
            targets,
            concurrency,
            timeout,
            retries,
            Sink::Event(app),
            state,
            true,
        )
        .await;
    });
    if let Ok(mut h) = ping.background.lock() {
        *h = Some(handle);
    }
}

/// Ping a small explicit list (favorites, history) and stream the results.
/// Runs beside the whole-list scan, not instead of it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ping_servers(
    targets: Vec<String>,
    concurrency: Option<usize>,
    timeout_ms: Option<u64>,
    on_progress: Channel<Vec<PingResultDto>>,
    ping: State<'_, Arc<PingState>>,
) -> Result<(), String> {
    if targets.is_empty() || ping.is_paused() {
        return Ok(());
    }
    let concurrency = concurrency.unwrap_or(LIST_CONCURRENT).clamp(5, 100);
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(LIST_TIMEOUT_MS).clamp(1000, 5000));
    let state = Arc::clone(&ping);
    tauri::async_runtime::spawn(scan(
        targets,
        concurrency,
        timeout,
        0,
        Sink::Batches(on_progress),
        state,
        false,
    ));
    Ok(())
}

/// Known results for `targets` ("ip:port"); targets never pinged are left out.
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_pings(targets: Vec<String>) -> Result<Vec<PingResultDto>, String> {
    let map = live::store().read();
    Ok(targets
        .iter()
        .filter_map(|key| {
            let (ip, port) = parse_target(key)?;
            let l = map.get(ip, port).filter(|l| l.ms.is_some() || l.failed)?;
            Some(to_dto(l, ip.to_owned(), port))
        })
        .collect())
}

/// Ping one server, with the long manual timeout. Returns the RTT in ms.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ping_single(
    ip: String,
    port: i64,
    timeout_ms: Option<u64>,
) -> Result<u32, String> {
    let addr = format!("{ip}:{port}");
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(MANUAL_TIMEOUT_MS));
    let r = match tokio::time::timeout(timeout, dz_a2s::ping(&addr)).await {
        Ok(r) => r,
        Err(_) => Err(dz_common::Error::A2sQuery("Timeout".into())),
    };
    live::store().record(&ip, port, Live::from_result(&r));
    r.map(|p| p.ms).map_err(|e| e.to_string())
}

/// Stop the whole-list scan and clear the pause.
#[tauri::command]
#[specta::specta]
pub(crate) async fn cancel_ping(ping: State<'_, Arc<PingState>>) -> Result<(), String> {
    ping.abort_background();
    ping.paused.store(false, Ordering::Relaxed);
    Ok(())
}

/// Pause or resume scanning. Returns whether it is now paused.
#[tauri::command]
#[specta::specta]
pub(crate) async fn toggle_ping_pause(ping: State<'_, Arc<PingState>>) -> Result<bool, String> {
    Ok(!ping.paused.fetch_xor(true, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_parse() {
        assert_eq!(parse_target("1.2.3.4:27016"), Some(("1.2.3.4", 27016)));
        assert_eq!(parse_target("::1:27016"), Some(("::1", 27016)));
        assert_eq!(parse_target("nope"), None);
        assert_eq!(parse_target("1.2.3.4:x"), None);
    }
}
