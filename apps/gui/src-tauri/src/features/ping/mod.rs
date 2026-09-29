//! Pinging servers over A2S: the scan of the whole list (driven by Rust,
//! reporting only its progress), small explicit lists, and one server.
//!
//! Every result lands in the live store (`browser::live`), which the browser
//! reads; nothing here touches the app state's lock.

use futures_util::{StreamExt, stream};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::State;
use tauri::ipc::Channel;

use crate::features::browser::live::{self, Live};

/// Default timeout for an explicit list.
const LIST_TIMEOUT_MS: u64 = 5_000;
/// Default timeout of a manual ping.
const MANUAL_TIMEOUT_MS: u64 = 10_000;
/// Default concurrency for an explicit list.
const LIST_CONCURRENT: usize = 10;
/// Results per Channel message for an explicit list.
const BATCH_SIZE: usize = 50;
/// How often partial batches and scan progress are sent (at most).
const FLUSH_INTERVAL: Duration = Duration::from_millis(250);

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
        ms: l.ms,
        players: l.players,
        max_players: l.max_players,
        bots: l.bots,
        failed: l.failed,
    }
}

/// How far the scan of the whole list has got.
#[derive(Serialize, Clone, Copy, Debug, specta::Type)]
pub struct ScanProgress {
    pub done: u32,
    pub total: u32,
    pub paused: bool,
    pub running: bool,
}

/// The scan's controls, managed by the app on its own.
#[derive(Default)]
pub struct PingState {
    /// Read by every scan loop on every result: an atomic, not a lock.
    paused: AtomicBool,
    /// The running scan, so a new one (or a refresh) can abort it.
    background: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl PingState {
    pub(crate) fn abort_background(&self) {
        if let Some(handle) = self.background.lock().ok().and_then(|mut h| h.take()) {
            handle.abort();
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

/// Where a scan's results go besides the live store.
enum Sink {
    /// Batches of results, for explicit lists.
    Batches(Channel<Vec<PingResultDto>>),
    /// Progress only, for the whole-list scan.
    Progress(Channel<ScanProgress>),
}

/// Ping `targets` with `concurrency` queries in flight, recording each result
/// in the live store as it lands.
async fn scan(
    targets: Vec<String>,
    concurrency: usize,
    timeout: Duration,
    sink: Sink,
    ping: Arc<PingState>,
) {
    let total = targets.len() as u32;
    let done = Arc::new(AtomicU32::new(0));
    let progress = |running: bool| ScanProgress {
        done: done.load(Ordering::Relaxed),
        total,
        paused: ping.is_paused(),
        running,
    };

    // One client for the whole scan: async-a2s multiplexes replies on its one
    // UDP socket, which saves a socket bind per query.
    let Ok(client) = dz_a2s::new_client().await else {
        if let Sink::Progress(ch) = &sink {
            let _ = ch.send(progress(false));
        }
        return;
    };
    let client = Arc::new(client);

    let mut results = stream::iter(targets)
        .map(|target| {
            let client = Arc::clone(&client);
            async move {
                let (ip, port) = parse_target(&target)?;
                let r = tokio::time::timeout(timeout, dz_a2s::ping_using(&client, &target))
                    .await
                    .unwrap_or_else(|_| Err(dz_common::Error::A2sQuery("timeout".into())));
                let l = Live::from_result(&r);
                live::store().record(ip, port, l);
                Some((ip.to_owned(), port, l))
            }
        })
        .buffer_unordered(concurrency)
        .filter_map(std::future::ready)
        .fuse();

    let mut batch: Vec<PingResultDto> = Vec::new();
    let mut flush = tokio::time::interval(FLUSH_INTERVAL);
    flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let send = |batch: &mut Vec<PingResultDto>| match &sink {
        Sink::Batches(ch) if !batch.is_empty() => {
            let _ = ch.send(std::mem::take(batch));
        }
        Sink::Progress(ch) => {
            let _ = ch.send(progress(true));
        }
        _ => {}
    };

    loop {
        if ping.is_paused() {
            flush.tick().await;
            send(&mut batch);
            continue;
        }
        tokio::select! {
            biased;
            next = results.next() => {
                let Some((ip, port, l)) = next else { break };
                done.fetch_add(1, Ordering::Relaxed);
                if let Sink::Batches(_) = &sink {
                    batch.push(to_dto(&l, ip, port));
                    if batch.len() >= BATCH_SIZE {
                        send(&mut batch);
                    }
                }
            }
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
    on_progress: Channel<ScanProgress>,
) {
    ping.abort_background();
    let state = Arc::clone(ping);
    let handle = tauri::async_runtime::spawn(async move {
        scan(
            targets,
            concurrency,
            timeout,
            Sink::Progress(on_progress),
            state,
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
        Sink::Batches(on_progress),
        state,
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
            Some(to_dto(map.get(ip, port)?, ip.to_owned(), port))
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
