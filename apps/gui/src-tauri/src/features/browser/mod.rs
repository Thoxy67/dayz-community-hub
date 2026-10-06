//! The server browser, answered in Rust: the window asks for a filtered,
//! sorted window of rows and never holds the list itself.
//!
//! - `servers_query` pages through the filtered list;
//! - `start_scan` pings the whole list in the background, reporting progress;
//! - full servers are re-queried after every load, since the list's player
//!   counts are often stale for them;
//! - `servers-changed` tells the window, at most twice a second, that what
//!   it shows may have changed.

pub(crate) mod live;
mod query;

pub use query::{ServerPage, ServerQuery, ServerRow};

use dz_api::ServerList;
use futures_util::{StreamExt, stream};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::error::ResultExt;
use crate::features::ping::{self, PingState, ScanProgress};
use crate::state::{AppState, SharedState};

/// The list, or live data, changed: re-query what is on screen.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct ServersChanged {
    pub generation: u64,
}

/// How often `servers-changed` may fire.
const CHANGED_INTERVAL: Duration = Duration::from_millis(500);
/// Concurrent A2S queries of the full-server check.
const VERIFY_CONCURRENT: usize = 8;
const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);

/// A map and how many servers run it.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct MapCount {
    pub map: String,
    pub count: u32,
}

fn profile_view(s: &AppState) -> query::ProfileView {
    let p = s.ctl.profile();
    query::ProfileView {
        excluded: p.excluded_ips.iter().cloned().collect(),
        favorites: p
            .favorites
            .iter()
            .map(|f| format!("{}:{}", f.ip, f.port))
            .collect(),
        mods: s.mods_on_disk.as_ref().map(|disk| {
            disk.iter()
                .map(|(&id, &local)| {
                    let stale = s.mod_update_cache.get(&id).is_some_and(|&r| r > local);
                    (id, stale)
                })
                .collect()
        }),
    }
}

/// A filtered, sorted window of the server list, with totals.
#[tauri::command]
#[specta::specta]
pub(crate) async fn servers_query(
    query: ServerQuery,
    state: State<'_, SharedState>,
) -> Result<ServerPage, String> {
    let (list, profile) = {
        let s = state.read().await;
        (Arc::clone(&s.servers), profile_view(&s))
    };
    let generation = live::store().generation();
    tokio::task::spawn_blocking(move || query::run(list, &profile, &query, generation))
        .await
        .cmd_err()
}

/// Full rows for "ip:port" keys (query or game port), in the same order;
/// null for a server that is not in the list.
#[tauri::command]
#[specta::specta]
pub(crate) async fn servers_lookup(
    keys: Vec<String>,
    state: State<'_, SharedState>,
) -> Result<Vec<Option<ServerRow>>, String> {
    let s = state.read().await;
    let profile = profile_view(&s);
    let live = live::store().read();
    Ok(keys
        .iter()
        .map(|k| {
            let (ip, port) = ping::parse_target(k)?;
            s.find_flexible(ip, port)
                .map(|srv| query::row(srv, &live, &profile))
        })
        .collect())
}

/// Every map in the list, busiest first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn server_maps(state: State<'_, SharedState>) -> Result<Vec<MapCount>, String> {
    let list = Arc::clone(&state.read().await.servers);
    tokio::task::spawn_blocking(move || {
        let mut counts: rustc_hash::FxHashMap<&str, u32> = Default::default();
        for s in &list.result {
            *counts.entry(s.map.as_str()).or_default() += 1;
        }
        let mut maps: Vec<MapCount> = counts
            .into_iter()
            .map(|(map, count)| MapCount {
                map: map.to_owned(),
                count,
            })
            .collect();
        maps.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.map.cmp(&b.map)));
        maps
    })
    .await
    .cmd_err()
}

/// Every query address to scan, in order: favorites, then history, then the
/// rest, as far as the profile's scopes allow, leaving excluded IPs out.
fn scan_targets(s: &AppState) -> Vec<String> {
    let p = s.ctl.profile();
    let excluded: FxHashSet<&str> = p.excluded_ips.iter().map(String::as_str).collect();
    let favorites: FxHashSet<String> = p
        .favorites
        .iter()
        .map(|f| format!("{}:{}", f.ip, f.port))
        .collect();
    let history: FxHashSet<String> = p
        .history
        .iter()
        .map(|h| format!("{}:{}", h.ip, h.port))
        .filter(|k| !favorites.contains(k))
        .collect();

    let (mut fav, mut hist, mut rest) = (Vec::new(), Vec::new(), Vec::new());
    for srv in &s.servers.result {
        if excluded.contains(srv.endpoint.ip.as_str()) {
            continue;
        }
        let key = format!("{}:{}", srv.endpoint.ip, srv.endpoint.port);
        if favorites.contains(&key) {
            if p.ping_scan_favorites {
                fav.push(key);
            }
        } else if history.contains(&key) {
            if p.ping_scan_history {
                hist.push(key);
            }
        } else if p.ping_scan_servers {
            rest.push(key);
        }
    }
    fav.extend(hist);
    fav.extend(rest);
    fav
}

/// Ping the whole list in the background (replacing a running scan).
/// Progress arrives on `on_progress` at most four times a second; the rows
/// themselves change through `servers-changed`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn start_scan(
    on_progress: Channel<ScanProgress>,
    state: State<'_, SharedState>,
    ping: State<'_, Arc<PingState>>,
) -> Result<(), String> {
    let (targets, concurrency, timeout, retries) = {
        let s = state.read().await;
        let p = s.ctl.profile();
        (
            scan_targets(&s),
            (p.ping_concurrency as usize).clamp(5, 200),
            Duration::from_millis(u64::from(p.ping_timeout_auto).clamp(1000, 5000)),
            p.ping_max_retries.min(5),
        )
    };
    ping::start_whole_scan(&ping, targets, concurrency, timeout, retries, on_progress);
    Ok(())
}

/// The running full-server check, so a newer list can replace it.
static VERIFY: Mutex<Option<tauri::async_runtime::JoinHandle<()>>> = Mutex::new(None);

/// Re-query every server the list says is full: its count is often stale.
pub(crate) fn verify_full_servers(list: Arc<ServerList>) {
    let targets: Vec<(String, i64)> = list
        .result
        .iter()
        .filter(|s| s.players > 0 && s.players >= s.max_players)
        .map(|s| (s.endpoint.ip.clone(), s.endpoint.port))
        .collect();
    let handle = tauri::async_runtime::spawn(async move {
        let Ok(client) = dz_a2s::new_client().await else {
            return;
        };
        let client = Arc::new(client);
        stream::iter(targets)
            .for_each_concurrent(VERIFY_CONCURRENT, |(ip, port)| {
                let client = Arc::clone(&client);
                async move {
                    let addr = format!("{ip}:{port}");
                    let r =
                        tokio::time::timeout(VERIFY_TIMEOUT, dz_a2s::ping_using(&client, &addr))
                            .await
                            .unwrap_or_else(|_| Err(dz_common::Error::A2sQuery("timeout".into())));
                    live::store().record(&ip, port, live::Live::from_result(&r));
                }
            })
            .await;
    });
    if let Ok(mut v) = VERIFY.lock()
        && let Some(prev) = v.replace(handle)
    {
        prev.abort();
    }
}

/// The list was replaced: move the generation and start the full-server check.
pub(crate) fn list_replaced(list: Arc<ServerList>) {
    live::store().touch();
    verify_full_servers(list);
}

/// Emit `servers-changed` whenever the generation moved, at most every
/// [`CHANGED_INTERVAL`]. Runs for the life of the app.
pub(crate) fn spawn_change_notifier(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last = 0;
        let mut tick = tokio::time::interval(CHANGED_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let generation = live::store().tick();
            if generation != last {
                last = generation;
                // Nothing to show before the state exists.
                if app.try_state::<SharedState>().is_some() {
                    let _ = ServersChanged { generation }.emit(&app);
                }
            }
        }
    });
}
