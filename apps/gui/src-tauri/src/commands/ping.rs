use dayz_community_hub_core::{Endpoint, a2s_query, api};
use futures_util::{StreamExt, stream};
use std::sync::Arc;
use std::time::Duration;
use tauri::State;
use tauri::ipc::Channel;

use crate::dto::PingResultDto;
use crate::state::{CachedPingResult, PingCache, SharedState};

/// Timeout for background bulk pinging (fast, allows filtering)
const BACKGROUND_TIMEOUT_MS: u64 = 2_000;
/// Timeout for visible server pings (longer for accuracy when scrolling)
const VISIBLE_TIMEOUT_MS: u64 = 5_000;
/// Timeout for manual single-server ping
const MANUAL_TIMEOUT_MS: u64 = 10_000;
/// Max concurrent pings for background bulk operation.
/// A2S queries are network-RTT bound and share one multiplexed UDP socket, so
/// higher in-flight counts mostly hide latency rather than burn CPU. 64 roughly
/// halves the timeout-dominated tail on large lists vs. the old 25.
const BACKGROUND_CONCURRENT: usize = 64;
/// Max concurrent pings for visible server operation
const VISIBLE_CONCURRENT: usize = 10;
/// Batch size for streaming results via Channel
const BATCH_SIZE: usize = 50;
/// Flush interval for partial batches (ms)
const FLUSH_INTERVAL_MS: u64 = 200;

/// Parse "ip:port" string into (ip, port) tuple.
fn parse_target(target: &str) -> Option<(String, i64)> {
    target.rsplit_once(':').and_then(|(ip, port_str)| {
        port_str
            .parse::<i64>()
            .ok()
            .map(|port| (ip.to_string(), port))
    })
}

/// Background ping all servers - streams results via Channel.
/// Results are sent in batches of ~50 or every 200ms for progressive UI updates.
#[tauri::command]
pub(crate) async fn ping_all_background(
    targets: Vec<String>,
    concurrency: Option<usize>,
    timeout_ms: Option<u64>,
    on_progress: Channel<Vec<PingResultDto>>,
    ping_cache: State<'_, PingCache>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    if targets.is_empty() {
        return Ok(());
    }

    // Abort previous ping task if any
    {
        let mut s = state.write().await;
        if let Some(prev_abort) = s.ping_abort.take() {
            prev_abort.abort();
        }
    }

    let ping_cache = ping_cache.inner().clone();
    let state_clone = state.inner().clone();
    let concurrency = concurrency.unwrap_or(BACKGROUND_CONCURRENT).clamp(5, 200);
    let timeout = Duration::from_millis(
        timeout_ms
            .unwrap_or(BACKGROUND_TIMEOUT_MS)
            .clamp(1000, 5000),
    );

    let handle = tokio::spawn(async move {
        // Parse all targets upfront with pre-computed keys (avoids format! in hot loop)
        let servers: Vec<(String, i64, String)> = targets
            .iter()
            .filter_map(|t| {
                parse_target(t).map(|(ip, port)| {
                    let key = t.clone(); // Key is already "ip:port"
                    (ip, port, key)
                })
            })
            .collect();

        let mut batch: Vec<PingResultDto> = Vec::with_capacity(BATCH_SIZE);
        let mut flush_timer = tokio::time::interval(Duration::from_millis(FLUSH_INTERVAL_MS));
        flush_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // One A2S client per scan, shared across all concurrent queries.
        // async-a2s multiplexes responses on its single UDP socket, so we
        // skip ~18k UDP-bind syscalls compared to one client per query.
        let client = match a2s_query::new_client().await {
            Ok(c) => Arc::new(c),
            Err(_) => {
                state_clone.write().await.ping_abort = None;
                return;
            }
        };

        let mut stream = stream::iter(servers)
            .map(|(ip, port, key)| {
                let client = client.clone();
                async move {
                    let server = api::Server {
                        endpoint: Endpoint {
                            ip: ip.clone(),
                            port,
                        },
                        ..Default::default()
                    };

                    let result = match tokio::time::timeout(
                        timeout,
                        a2s_query::ping_via_a2s_with_info_using(&client, &server),
                    )
                    .await
                    {
                        Ok(Ok((rtt, p, mp, b))) => CachedPingResult {
                            ms: rtt,
                            players: Some(p),
                            max_players: Some(mp),
                            bots: Some(b),
                            failed: false,
                        },
                        _ => CachedPingResult {
                            ms: a2s_query::PING_TIMEOUT_SENTINEL,
                            players: None,
                            max_players: None,
                            bots: None,
                            failed: true,
                        },
                    };

                    (ip, port, key, result)
                }
            })
            .buffer_unordered(concurrency)
            .fuse();

        loop {
            // Wait while paused
            while state_clone.read().await.ping_paused {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            tokio::select! {
                biased;

                maybe_result = stream.next() => {
                    match maybe_result {
                        Some((ip, port, key, result)) => {
                            // Write to cache immediately (key already computed)
                            ping_cache.write().await.insert(key, result.clone());

                            batch.push(PingResultDto {
                                ip,
                                port,
                                ms: result.ms,
                                players: result.players,
                                max_players: result.max_players,
                                bots: result.bots,
                                failed: result.failed,
                            });

                            if batch.len() >= BATCH_SIZE {
                                // drain() keeps Vec capacity, avoiding reallocation
                                let _ = on_progress.send(std::mem::take(&mut batch));
                                tokio::task::yield_now().await;
                                flush_timer.reset();
                            }
                        }
                        None => break, // Stream exhausted
                    }
                }

                _ = flush_timer.tick() => {
                    // Time-based flush for stragglers
                    if !batch.is_empty() {
                        let _ = on_progress.send(std::mem::take(&mut batch));
                        tokio::task::yield_now().await;
                    }
                }
            }
        }

        // Final flush
        if !batch.is_empty() {
            let _ = on_progress.send(batch);
            tokio::task::yield_now().await;
        }

        // Clear abort handle when done
        state_clone.write().await.ping_abort = None;
    });

    // Store abort handle for cancellation
    state.write().await.ping_abort = Some(handle.abort_handle());

    Ok(())
}

/// Ping visible servers with longer timeout - streams results via Channel.
/// Re-tests servers that timed out in background scan.
#[tauri::command]
pub(crate) async fn ping_servers(
    targets: Vec<String>,
    concurrency: Option<usize>,
    timeout_ms: Option<u64>,
    on_progress: Channel<Vec<PingResultDto>>,
    ping_cache: State<'_, PingCache>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    if targets.is_empty() {
        return Ok(());
    }

    // Note: ping_servers doesn't abort previous - it's for visible servers only
    // and runs concurrently with background ping

    let ping_cache = ping_cache.inner().clone();
    let state_clone = state.inner().clone();
    let concurrency = concurrency.unwrap_or(VISIBLE_CONCURRENT).clamp(5, 100);
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(VISIBLE_TIMEOUT_MS).clamp(1000, 5000));

    tokio::spawn(async move {
        // Check if paused at start
        if state_clone.read().await.ping_paused {
            return;
        }
        // Parse all targets upfront with pre-computed keys (avoids format! in hot loop)
        let servers: Vec<(String, i64, String)> = targets
            .iter()
            .filter_map(|t| {
                parse_target(t).map(|(ip, port)| {
                    let key = t.clone(); // Key is already "ip:port"
                    (ip, port, key)
                })
            })
            .collect();

        let mut batch: Vec<PingResultDto> = Vec::with_capacity(BATCH_SIZE);
        let mut flush_timer = tokio::time::interval(Duration::from_millis(FLUSH_INTERVAL_MS));
        flush_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Shared A2S client for the entire visible-set scan (one UDP socket).
        let client = match a2s_query::new_client().await {
            Ok(c) => Arc::new(c),
            Err(_) => return,
        };

        let mut stream = stream::iter(servers)
            .map(|(ip, port, key)| {
                let client = client.clone();
                async move {
                    let server = api::Server {
                        endpoint: Endpoint {
                            ip: ip.clone(),
                            port,
                        },
                        ..Default::default()
                    };

                    let result = match tokio::time::timeout(
                        timeout,
                        a2s_query::ping_via_a2s_with_info_using(&client, &server),
                    )
                    .await
                    {
                        Ok(Ok((rtt, p, mp, b))) => CachedPingResult {
                            ms: rtt,
                            players: Some(p),
                            max_players: Some(mp),
                            bots: Some(b),
                            failed: false,
                        },
                        _ => CachedPingResult {
                            ms: a2s_query::PING_TIMEOUT_SENTINEL,
                            players: None,
                            max_players: None,
                            bots: None,
                            failed: true,
                        },
                    };

                    (ip, port, key, result)
                }
            })
            .buffer_unordered(concurrency)
            .fuse();

        loop {
            // Wait while paused
            while state_clone.read().await.ping_paused {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            tokio::select! {
                biased;

                maybe_result = stream.next() => {
                    match maybe_result {
                        Some((ip, port, key, result)) => {
                            // Write to cache immediately (key already computed)
                            ping_cache.write().await.insert(key, result.clone());

                            batch.push(PingResultDto {
                                ip,
                                port,
                                ms: result.ms,
                                players: result.players,
                                max_players: result.max_players,
                                bots: result.bots,
                                failed: result.failed,
                            });

                            if batch.len() >= BATCH_SIZE {
                                // drain() keeps Vec capacity, avoiding reallocation
                                let _ = on_progress.send(std::mem::take(&mut batch));
                                tokio::task::yield_now().await;
                                flush_timer.reset();
                            }
                        }
                        None => break, // Stream exhausted
                    }
                }

                _ = flush_timer.tick() => {
                    // Time-based flush for stragglers
                    if !batch.is_empty() {
                        let _ = on_progress.send(std::mem::take(&mut batch));
                        tokio::task::yield_now().await;
                    }
                }
            }
        }

        // Final flush
        if !batch.is_empty() {
            let _ = on_progress.send(batch);
            tokio::task::yield_now().await;
        }
    });

    Ok(())
}

/// Get all cached ping results for given targets.
/// Frontend calls this to fetch results after pinging completes.
#[tauri::command]
pub(crate) async fn get_pings(
    targets: Vec<String>,
    ping_cache: State<'_, PingCache>,
) -> Result<Vec<PingResultDto>, String> {
    let cache = ping_cache.read().await;
    let results = targets
        .into_iter()
        .filter_map(|key| {
            cache.get(&key).map(|r| {
                let (ip, port) = parse_target(&key).unwrap_or_default();
                PingResultDto {
                    ip,
                    port,
                    ms: r.ms,
                    players: r.players,
                    max_players: r.max_players,
                    bots: r.bots,
                    failed: r.failed,
                }
            })
        })
        .collect();
    Ok(results)
}

/// Ping a single server (manual mode - longest timeout for reliability)
#[tauri::command]
pub(crate) async fn ping_single(
    ip: String,
    port: i64,
    timeout_ms: Option<u64>,
    ping_cache: State<'_, PingCache>,
) -> Result<u32, String> {
    let server = api::Server {
        endpoint: Endpoint {
            ip: ip.clone(),
            port,
        },
        ..Default::default()
    };
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(MANUAL_TIMEOUT_MS));

    let (ms, players, max_players, bots, failed) =
        match tokio::time::timeout(timeout, a2s_query::ping_via_a2s_with_info(&server)).await {
            Ok(Ok((rtt, p, mp, b))) => (rtt, Some(p), Some(mp), Some(b), false),
            Ok(Err(e)) => return Err(format!("A2S query failed: {e}")),
            Err(_) => return Err("Timeout".into()),
        };

    let key = format!("{ip}:{port}");
    ping_cache.write().await.insert(
        key,
        CachedPingResult {
            ms,
            players,
            max_players,
            bots,
            failed,
        },
    );
    Ok(ms)
}

/// Cancel any ongoing ping scan.
#[tauri::command]
pub(crate) async fn cancel_ping(state: State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.write().await;
    if let Some(abort) = s.ping_abort.take() {
        abort.abort();
    }
    s.ping_paused = false;
    Ok(())
}

/// Toggle ping pause state. Returns new paused state.
#[tauri::command]
pub(crate) async fn toggle_ping_pause(state: State<'_, SharedState>) -> Result<bool, String> {
    let mut s = state.write().await;
    s.ping_paused = !s.ping_paused;
    Ok(s.ping_paused)
}

/// Get current ping pause state.
#[tauri::command]
pub(crate) async fn get_ping_paused(state: State<'_, SharedState>) -> Result<bool, String> {
    Ok(state.read().await.ping_paused)
}
