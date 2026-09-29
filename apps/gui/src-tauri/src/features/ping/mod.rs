//! Pinging servers over A2S: the background scan of the whole list, the
//! visible rows, and a single server on demand.
//!
//! Results stream to the window over a `Channel` in batches, and are kept in
//! a cache of their own so the ~64 concurrent queries never contend with the
//! commands that need the app state.

use futures_util::{StreamExt, stream};
use rustc_hash::FxHashMap;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::State;
use tauri::ipc::Channel;
use tokio::sync::RwLock;

/// Default timeout of the background scan.
const BACKGROUND_TIMEOUT_MS: u64 = 2_000;
/// Default timeout for the visible rows.
const VISIBLE_TIMEOUT_MS: u64 = 5_000;
/// Default timeout of a manual ping.
const MANUAL_TIMEOUT_MS: u64 = 10_000;
/// Default concurrency of the background scan. A2S queries are RTT bound and
/// share one multiplexed UDP socket, so more in flight mostly hides latency.
const BACKGROUND_CONCURRENT: usize = 64;
/// Default concurrency for the visible rows.
const VISIBLE_CONCURRENT: usize = 10;
/// Results per Channel message.
const BATCH_SIZE: usize = 50;
/// A partial batch is flushed after this long.
const FLUSH_INTERVAL_MS: u64 = 200;

/// One ping, as the window receives it.
#[derive(Serialize, Clone, Debug)]
pub struct PingResultDto {
    pub ip: String,
    pub port: i64,
    pub ms: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub players: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_players: Option<u8>,
    /// Bots reported by A2S_INFO (DayZ servers often pad this to fake a full server).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bots: Option<u8>,
    /// True when the query failed (timeout or error).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
}

/// A cached ping.
#[derive(Clone, Copy, Debug)]
struct Cached {
    ms: u32,
    players: Option<u8>,
    max_players: Option<u8>,
    bots: Option<u8>,
    failed: bool,
}

impl Cached {
    fn from_result(r: &dz_common::Result<dz_a2s::Ping>) -> Self {
        match r {
            Ok(p) => Self {
                ms: p.ms,
                players: Some(p.players),
                max_players: Some(p.max_players),
                bots: Some(p.bots),
                failed: false,
            },
            Err(_) => Self {
                ms: dz_a2s::PING_TIMEOUT_SENTINEL,
                players: None,
                max_players: None,
                bots: None,
                failed: true,
            },
        }
    }

    fn to_dto(self, ip: String, port: i64) -> PingResultDto {
        PingResultDto {
            ip,
            port,
            ms: self.ms,
            players: self.players,
            max_players: self.max_players,
            bots: self.bots,
            failed: self.failed,
        }
    }
}

/// The ping cache and the scan's controls, managed by the app on its own.
#[derive(Default)]
pub struct PingState {
    /// "ip:query_port" → last result.
    cache: RwLock<FxHashMap<String, Cached>>,
    /// Read by every scan loop on every result: an atomic, not a lock.
    paused: AtomicBool,
    /// The background scan, so a new one (or a refresh) can abort it.
    background: Mutex<Option<tokio::task::AbortHandle>>,
}

impl PingState {
    fn abort_background(&self) {
        if let Some(handle) = self.background.lock().ok().and_then(|mut h| h.take()) {
            handle.abort();
        }
    }
}

/// Parse "ip:port".
fn parse_target(target: &str) -> Option<(&str, i64)> {
    let (ip, port) = target.rsplit_once(':')?;
    Some((ip, port.parse().ok()?))
}

/// Ping `targets` with `concurrency` queries in flight and stream the results
/// over `channel` in batches, writing each to the cache as it lands.
async fn scan(
    targets: Vec<String>,
    concurrency: usize,
    timeout: Duration,
    channel: Channel<Vec<PingResultDto>>,
    ping: Arc<PingState>,
) {
    // One client for the whole scan: async-a2s multiplexes replies on its one
    // UDP socket, which saves a socket bind per query.
    let Ok(client) = dz_a2s::new_client().await else {
        return;
    };
    let client = Arc::new(client);

    let mut results = stream::iter(targets)
        .map(|target| {
            let client = Arc::clone(&client);
            async move {
                let (ip, port) = parse_target(&target)?;
                let (ip, port) = (ip.to_owned(), port);
                let r = tokio::time::timeout(timeout, dz_a2s::ping_using(&client, &target))
                    .await
                    .unwrap_or_else(|_| Err(dz_common::Error::A2sQuery("timeout".into())));
                Some((target, ip, port, Cached::from_result(&r)))
            }
        })
        .buffer_unordered(concurrency)
        .filter_map(std::future::ready)
        .fuse();

    let mut batch: Vec<PingResultDto> = Vec::with_capacity(BATCH_SIZE);
    let mut flush = tokio::time::interval(Duration::from_millis(FLUSH_INTERVAL_MS));
    flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        while ping.paused.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        tokio::select! {
            biased;
            next = results.next() => {
                let Some((key, ip, port, cached)) = next else { break };
                ping.cache.write().await.insert(key, cached);
                batch.push(cached.to_dto(ip, port));
                if batch.len() >= BATCH_SIZE {
                    let _ = channel.send(std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE)));
                    tokio::task::yield_now().await;
                    flush.reset();
                }
            }
            _ = flush.tick() => {
                if !batch.is_empty() {
                    let _ = channel.send(std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE)));
                    tokio::task::yield_now().await;
                }
            }
        }
    }
    if !batch.is_empty() {
        let _ = channel.send(batch);
    }
}

/// Ping every target in list order (the window puts favorites and history
/// first) and stream the results. Aborts the previous background scan.
#[tauri::command]
pub(crate) async fn ping_all_background(
    targets: Vec<String>,
    concurrency: Option<usize>,
    timeout_ms: Option<u64>,
    on_progress: Channel<Vec<PingResultDto>>,
    ping: State<'_, Arc<PingState>>,
) -> Result<(), String> {
    ping.abort_background();
    if targets.is_empty() {
        return Ok(());
    }
    let concurrency = concurrency.unwrap_or(BACKGROUND_CONCURRENT).clamp(5, 200);
    let timeout =
        Duration::from_millis(timeout_ms.unwrap_or(BACKGROUND_TIMEOUT_MS).clamp(1000, 5000));
    let state = Arc::clone(&ping);
    let handle = tauri::async_runtime::spawn(async move {
        scan(targets, concurrency, timeout, on_progress, Arc::clone(&state)).await;
        if let Ok(mut h) = state.background.lock() {
            *h = None;
        }
    });
    if let Ok(mut h) = ping.background.lock() {
        *h = Some(handle.inner().abort_handle());
    }
    Ok(())
}

/// Ping the rows on screen. Runs beside the background scan, not instead of it.
#[tauri::command]
pub(crate) async fn ping_servers(
    targets: Vec<String>,
    concurrency: Option<usize>,
    timeout_ms: Option<u64>,
    on_progress: Channel<Vec<PingResultDto>>,
    ping: State<'_, Arc<PingState>>,
) -> Result<(), String> {
    if targets.is_empty() || ping.paused.load(Ordering::Relaxed) {
        return Ok(());
    }
    let concurrency = concurrency.unwrap_or(VISIBLE_CONCURRENT).clamp(5, 100);
    let timeout =
        Duration::from_millis(timeout_ms.unwrap_or(VISIBLE_TIMEOUT_MS).clamp(1000, 5000));
    let state = Arc::clone(&ping);
    tauri::async_runtime::spawn(scan(targets, concurrency, timeout, on_progress, state));
    Ok(())
}

/// Cached results for `targets` ("ip:port"); targets never pinged are left out.
#[tauri::command]
pub(crate) async fn get_pings(
    targets: Vec<String>,
    ping: State<'_, Arc<PingState>>,
) -> Result<Vec<PingResultDto>, String> {
    let cache = ping.cache.read().await;
    Ok(targets
        .iter()
        .filter_map(|key| {
            let cached = cache.get(key)?;
            let (ip, port) = parse_target(key)?;
            Some(cached.to_dto(ip.to_owned(), port))
        })
        .collect())
}

/// Ping one server, with the long manual timeout. Returns the RTT in ms.
#[tauri::command]
pub(crate) async fn ping_single(
    ip: String,
    port: i64,
    timeout_ms: Option<u64>,
    ping: State<'_, Arc<PingState>>,
) -> Result<u32, String> {
    let addr = format!("{ip}:{port}");
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(MANUAL_TIMEOUT_MS));
    let p = match tokio::time::timeout(timeout, dz_a2s::ping(&addr)).await {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => return Err(format!("A2S query failed: {e}")),
        Err(_) => return Err("Timeout".into()),
    };
    ping.cache
        .write()
        .await
        .insert(addr, Cached::from_result(&Ok(p)));
    Ok(p.ms)
}

/// Stop the background scan and clear the pause.
#[tauri::command]
pub(crate) async fn cancel_ping(ping: State<'_, Arc<PingState>>) -> Result<(), String> {
    ping.abort_background();
    ping.paused.store(false, Ordering::Relaxed);
    Ok(())
}

/// Pause or resume scanning. Returns whether it is now paused.
#[tauri::command]
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
