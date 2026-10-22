//! The server browser, answered in Rust.
//!
//! - The window reads the list in process: [`Hub::servers_snapshot`] hands
//!   the current [`ServerSnapshot`] (an `Arc`: the servers, their live and
//!   profile state in atomics), and [`order`] / [`Hub::servers_order`] filter
//!   and sort it into an `Arc<[u32]>` of indices with the header's counts.
//!   Rows are read by index straight from the snapshot; pings land in it as
//!   they arrive.
//! - `servers_query` pages through the same order as owned rows, for the
//!   control channel and the tests;
//! - `start_scan` pings the whole list in the background, reporting progress
//!   as `ping-scan-progress` events (see `ping`);
//! - full servers are re-queried after every load, since the list's player
//!   counts are often stale for them;
//! - `servers-changed` wakes the window when the list, the profile's flags
//!   or live data moved, at most ~30 times a second.

pub(crate) mod live;
mod query;
mod snapshot;

pub use query::{ServerPage, ServerQuery, ServerRow, ServerStats, SortCol, Tri};
#[cfg(all(test, feature = "mock"))]
pub(crate) use snapshot::ProfileView;
pub use snapshot::{ServerOrder, ServerRef, ServerSnapshot, order};

use crate::{Hub, HubEvent};
use dz_api::ServerList;
use futures_util::{StreamExt, stream};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::ResultExt;
use crate::features::ping::{self, PingState, ScanProgress};
use crate::state::{AppState, SharedState};

/// The list, the profile's flags or live data changed: look at the snapshot again.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServersChanged {
    pub generation: u64,
}

/// How often `servers-changed` may fire: the window picks the change up on
/// its next frame, so a scan repaints it at most ~30 times a second.
const CHANGED_INTERVAL: Duration = Duration::from_millis(33);
/// Concurrent A2S queries of the full-server check.
const VERIFY_CONCURRENT: usize = 8;
const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);

/// A map and how many servers run it.
#[derive(Serialize, Clone, Debug)]
pub struct MapCount {
    pub map: String,
    pub count: u32,
}

pub(crate) fn profile_view(s: &AppState) -> snapshot::ProfileView {
    let p = s.ctl.profile();
    snapshot::ProfileView {
        excluded: p.excluded_ips.iter().cloned().collect(),
        favorites: p
            .favorites
            .iter()
            .map(|f| (f.ip.clone(), i64::from(f.port)))
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

/// The profile's flags (favourites, exclusions, mods on disk) may have
/// changed in `s`: write them into its snapshot again (`snap`, the one the
/// state holds, which is the one published).
pub(crate) fn profile_changed(snap: &ServerSnapshot, s: &AppState) {
    snap.apply_profile(&profile_view(s));
    live::store().touch();
}

/// A filtered, sorted window of the server list, with totals.
async fn servers_query(hub: &Hub, query: ServerQuery) -> Result<ServerPage, String> {
    hub.state()?;
    let snap = live::store().snapshot();
    let generation = live::store().generation();
    tokio::task::spawn_blocking(move || query::run(&snap, &query, generation))
        .await
        .cmd_err()
}

/// Full rows for "ip:port" keys (query or game port), in the same order;
/// null for a server that is not in the list.
async fn servers_lookup(hub: &Hub, keys: Vec<String>) -> Result<Vec<Option<ServerRow>>, String> {
    hub.state()?;
    let snap = live::store().snapshot();
    Ok(keys
        .iter()
        .map(|k| {
            let (ip, port) = ping::parse_target(k)?;
            snap.lookup(ip, port).map(|r| r.to_row())
        })
        .collect())
}

/// Every map in the list, busiest first. Servers spell the same map in
/// different cases ("chernarusplus", "ChernarusPlus"): they count as one map,
/// named by its most common spelling.
async fn server_maps(hub: &Hub) -> Result<Vec<MapCount>, String> {
    hub.state()?;
    Ok(live::store().snapshot().maps().to_vec())
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
/// Progress goes out as `ping-scan-progress` at most twice a second, to
/// whichever window is loaded; the rows themselves change through
/// `servers-changed`. Returns the new scan's first progress.
async fn start_scan(hub: &Hub) -> Result<ScanProgress, String> {
    let state = hub.state()?;
    Ok(scan_whole(hub, &state, hub.ping()).await)
}

/// Start the whole-list scan with the profile's settings.
pub(crate) async fn scan_whole(
    hub: &Hub,
    state: &SharedState,
    ping: &Arc<PingState>,
) -> ScanProgress {
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
    ping::start_whole_scan(hub, ping, targets, concurrency, timeout, retries)
}

/// The running full-server check, so a newer list can replace it.
static VERIFY: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);

/// Re-query every server the list says is full: its count is often stale.
pub(crate) fn verify_full_servers(list: Arc<ServerList>) {
    let targets: Vec<(String, i64)> = list
        .result
        .iter()
        .filter(|s| s.players > 0 && s.players >= s.max_players)
        .map(|s| (s.endpoint.ip.clone(), s.endpoint.port))
        .collect();
    let handle = tokio::spawn(async move {
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
/// [`CHANGED_INTERVAL`]. Runs for the life of the app, asleep while nothing
/// changes (the live store wakes it).
pub(crate) fn spawn_change_notifier(hub: Hub) {
    tokio::spawn(async move {
        let mut last = 0;
        crate::wake::on_change(live::store().changed(), CHANGED_INTERVAL, || {
            let generation = live::store().tick();
            if generation != last {
                last = generation;
                // Nothing to show before the state exists.
                if hub.try_state().is_some() {
                    hub.emit(HubEvent::ServersChanged(ServersChanged { generation }));
                }
            }
            true
        })
        .await;
    });
}

impl Hub {
    /// The server list as it stands, with its live and profile state (a
    /// refcount bump; never waits). Empty before `initialize`.
    pub fn servers_snapshot(&self) -> Arc<ServerSnapshot> {
        live::store().snapshot()
    }

    /// The current snapshot filtered and sorted for `query` (its window is
    /// ignored), off the async threads.
    pub async fn servers_order(&self, query: ServerQuery) -> Result<ServerOrder, String> {
        let snap = live::store().snapshot();
        tokio::task::spawn_blocking(move || order(&snap, &query))
            .await
            .cmd_err()
    }

    /// A filtered, sorted window of the server list, with totals.
    pub async fn servers_query(&self, query: ServerQuery) -> Result<ServerPage, String> {
        servers_query(self, query).await
    }

    /// Full rows for "ip:port" keys (query or game port), in the same order;
    /// `None` for a server that is not in the list.
    pub async fn servers_lookup(
        &self,
        keys: Vec<String>,
    ) -> Result<Vec<Option<ServerRow>>, String> {
        servers_lookup(self, keys).await
    }

    /// Every map in the list, busiest first.
    pub async fn server_maps(&self) -> Result<Vec<MapCount>, String> {
        server_maps(self).await
    }

    /// Ping the whole list in the background (replacing a running scan).
    /// Progress goes out as `PingScanProgress`; returns the first.
    pub async fn start_scan(&self) -> Result<ScanProgress, String> {
        start_scan(self).await
    }
}

/// A map's total and each spelling's count, keyed by the lowercased name.
type Spellings<'a> = (u32, Vec<(&'a str, u32)>);

/// Map names counted case-insensitively (and trimmed), each named by its most
/// common spelling; ties go to the first spelling met. Blank names are skipped.
fn count_maps<'a>(names: impl Iterator<Item = &'a str>) -> Vec<(String, u32)> {
    let mut by_key: rustc_hash::FxHashMap<String, Spellings<'a>> = Default::default();
    for name in names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let entry = by_key.entry(name.to_lowercase()).or_default();
        entry.0 += 1;
        match entry.1.iter_mut().find(|(s, _)| *s == name) {
            Some((_, n)) => *n += 1,
            None => entry.1.push((name, 1)),
        }
    }
    by_key
        .into_values()
        .map(|(total, spellings)| {
            let mut best = ("", 0);
            for (s, n) in spellings {
                if n > best.1 {
                    best = (s, n);
                }
            }
            (best.0.to_owned(), total)
        })
        .collect()
}

#[cfg(test)]
mod map_tests {
    use super::count_maps;

    #[test]
    fn maps_differing_in_case_are_one_map() {
        let mut maps = count_maps(
            [
                "chernarusplus",
                "ChernarusPlus",
                "chernarusplus",
                " enoch ",
                "Enoch",
                "",
            ]
            .into_iter(),
        );
        maps.sort();
        assert_eq!(
            maps,
            vec![("chernarusplus".to_owned(), 3), ("enoch".to_owned(), 2)]
        );
    }
}
