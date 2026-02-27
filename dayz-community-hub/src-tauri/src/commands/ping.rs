use dayz_community_hub_core::{a2s_query, api};
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use surge_ping::PingIdentifier;
use tauri::{AppHandle, Emitter, State};

use crate::dto::PingResultDto;
use crate::state::{PingCache, SharedState};

/// Start background pinging for all servers.
#[tauri::command]
pub(crate) async fn start_pinging(
    targets: Vec<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
    ping_cache: State<'_, PingCache>,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();

    let mut priority: Vec<(String, i64)> = Vec::new();
    for target in &targets {
        if seen.insert(target.clone()) {
            if let Some((ip, port_str)) = target.rsplit_once(':') {
                if let Ok(port) = port_str.parse::<i64>() {
                    priority.push((ip.to_string(), port));
                }
            }
        }
    }

    let mut bulk: Vec<(String, i64)> = Vec::new();
    {
        let state = state.lock().await;
        for s in state.servers.iter() {
            let key = format!("{}:{}", s.endpoint.ip, s.endpoint.port);
            if seen.insert(key) {
                bulk.push((s.endpoint.ip.clone(), s.endpoint.port));
            }
        }
    }

    let ping_cache_arc = ping_cache.inner().clone();

    let clients = Arc::new(a2s_query::IcmpClients::new().map_err(|e| format!("ICMP init: {e}"))?);

    let ident_counter = Arc::new(AtomicU16::new(1));

    const CONCURRENCY: usize = 100;
    const BATCH_SIZE: usize = 100;
    const TIMEOUT_MS: u64 = 1_500;
    let timeout = std::time::Duration::from_millis(TIMEOUT_MS);

    let semaphore = Arc::new(tokio::sync::Semaphore::new(CONCURRENCY));

    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<PingResultDto>(1024);

        /// Ping one server, update the cache, and send the result.
        async fn ping_one(
            ip: String,
            port: i64,
            clients: &a2s_query::IcmpClients,
            counter: &AtomicU16,
            timeout: std::time::Duration,
            cache: &PingCache,
            tx: &tokio::sync::mpsc::Sender<PingResultDto>,
        ) {
            let parsed_ip: std::net::IpAddr = match ip.parse() {
                Ok(a) => a,
                Err(_) => return,
            };
            let server = api::Server {
                endpoint: dayz_community_hub_core::Endpoint {
                    ip: ip.clone(),
                    port,
                },
                ..Default::default()
            };
            let ident = PingIdentifier(counter.fetch_add(1, Ordering::Relaxed));
            let client = clients.for_ip(parsed_ip);
            let ms = a2s_query::ping_once(client, parsed_ip, ident, timeout, 2, &server).await;
            cache.write().await.insert(format!("{}:{}", ip, port), ms);
            let _ = tx.send(PingResultDto { ip, port, ms }).await;
        }

        for (ip, port) in priority {
            let cache_clone = ping_cache_arc.clone();
            let tx_clone = tx.clone();
            let clients_clone = clients.clone();
            let counter_clone = ident_counter.clone();

            tokio::spawn(async move {
                ping_one(
                    ip,
                    port,
                    &clients_clone,
                    &counter_clone,
                    timeout,
                    &cache_clone,
                    &tx_clone,
                )
                .await;
            });
        }

        for (ip, port) in bulk {
            let cache_clone = ping_cache_arc.clone();
            let sem = semaphore.clone();
            let tx_clone = tx.clone();
            let clients_clone = clients.clone();
            let counter_clone = ident_counter.clone();

            tokio::spawn(async move {
                let _permit = sem.acquire().await;
                ping_one(
                    ip,
                    port,
                    &clients_clone,
                    &counter_clone,
                    timeout,
                    &cache_clone,
                    &tx_clone,
                )
                .await;
            });
        }

        drop(tx);

        let mut batch: Vec<PingResultDto> = Vec::with_capacity(BATCH_SIZE);
        while let Some(result) = rx.recv().await {
            batch.push(result);
            if batch.len() >= BATCH_SIZE {
                let _ = app.emit("ping-batch", &batch);
                batch.clear();
            }
        }
        if !batch.is_empty() {
            let _ = app.emit("ping-batch", &batch);
        }
    });

    Ok(())
}

/// Ping a single server and update the ping cache.
#[tauri::command]
pub(crate) async fn ping_single(
    ip: String,
    port: i64,
    timeout_ms: Option<u64>,
    ping_cache: State<'_, PingCache>,
) -> Result<u32, String> {
    let server = api::Server {
        endpoint: dayz_community_hub_core::Endpoint {
            ip: ip.clone(),
            port,
        },
        ..Default::default()
    };
    let effective_timeout = Some(timeout_ms.unwrap_or(10_000));
    let ms = a2s_query::ping_server(&server, effective_timeout)
        .await
        .map_err(|e| format!("Ping failed: {e}"))?;
    let key = format!("{ip}:{port}");
    ping_cache.write().await.insert(key, ms);
    Ok(ms)
}
