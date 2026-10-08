//! The servers Steam's master lists that the launcher list lacks, found
//! through the Steam client and added to the list as they answer.
//!
//! Steam's master gives addresses in about a second a request, then allows
//! about one request a minute: each request runs in its own short Steam
//! session (Steam shows DayZ running for those seconds only), the most
//! useful first (servers with players, then one per address, then map by
//! map), and a refused one is asked again after a pause. Every new address
//! is queried here (A2S_INFO, and A2S_RULES for its mods), many at once,
//! and lands in the list as soon as it answers.
//!
//! What Steam added is kept apart from the launcher list it is merged into
//! ([`compose`]), so a refresh of that list keeps it, and so farms found
//! late still leave the list (`dz_api::FarmCounter`).
//!
//! A server the list had and a refresh no longer lists is kept too: it may
//! be delisted and still running. It leaves only once it does not answer
//! ([`check_kept`]). The cache holds the whole list, so this holds across
//! restarts as well.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use dz_api::{FarmCounter, Mod, Server, ServerList, SteamListed};
use dz_steamworks::{Answer, Filters, ListEvent, MasterEntry};
use futures_util::{StreamExt, stream};
use rustc_hash::{FxHashMap, FxHashSet};
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use crate::features::browser::live::{self, Live};
use crate::state::SharedState;

/// A2S queries in flight at once.
const CONCURRENCY: usize = 256;
/// How long a server has to answer A2S_INFO.
const INFO_TIMEOUT: Duration = Duration::from_millis(1500);
/// How long a modded server has to answer A2S_RULES.
const RULES_TIMEOUT: Duration = Duration::from_secs(2);
/// How often what answered is put in the list.
const PUBLISH_EVERY: Duration = Duration::from_millis(700);
/// Wait after the master refused a request (it allows about one a minute).
const THROTTLE_WAIT: Duration = Duration::from_secs(60);
/// Times a refused request is asked again.
const RETRIES: usize = 2;
/// Maps asked for one by one, the busiest of the list first.
const MAX_MAPS: usize = 8;
/// A listing is not started again sooner than this.
const MIN_INTERVAL: Duration = Duration::from_secs(10 * 60);
/// Times a kept server is asked before it is taken for gone.
const KEPT_TRIES: usize = 2;

#[derive(Default)]
struct Found {
    /// The launcher list, before Steam's servers are merged in.
    base: Arc<ServerList>,
    /// Every server Steam listed that answered, once each.
    rows: Vec<SteamListed>,
    keys: FxHashSet<(String, i64)>,
    farms: FarmCounter,
    /// Servers an earlier list had that the current one lacks, kept while
    /// they answer.
    kept: Vec<Server>,
    /// When the last listing started.
    last: Option<Instant>,
}

static FOUND: LazyLock<Mutex<Found>> = LazyLock::new(Mutex::default);
static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);

fn found() -> std::sync::MutexGuard<'static, Found> {
    FOUND
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn key(s: &Server) -> (String, i64) {
    (s.endpoint.ip.clone(), s.endpoint.port)
}

/// `base`, the launcher list, with what Steam found merged in. Keeps
/// `base` for the merges to come.
pub(crate) fn with_steam(base: Arc<ServerList>) -> Arc<ServerList> {
    found().base = base;
    compose()
}

/// [`with_steam`] for a refresh: the servers of `previous` that the new
/// list lacks are kept, until [`check_kept`] finds them gone.
pub(crate) fn refreshed(previous: &ServerList, base: Arc<ServerList>) -> Arc<ServerList> {
    {
        let mut f = found();
        let mut known: FxHashSet<(String, i64)> = base.result.iter().map(key).collect();
        known.extend(f.keys.iter().cloned());
        f.kept.retain(|s| known.insert(key(s)));
        let more: Vec<Server> = previous
            .result
            .iter()
            .filter(|s| known.insert(key(s)))
            .cloned()
            .collect();
        f.kept.extend(more);
        f.base = base;
    }
    compose()
}

fn compose() -> Arc<ServerList> {
    let f = found();
    if f.rows.is_empty() && f.kept.is_empty() {
        return Arc::clone(&f.base);
    }
    let mut list = ServerList {
        status: f.base.status,
        result: f.base.result.clone(),
    };
    dz_api::merge_steam(&mut list.result, &f.rows, &f.farms);
    if !f.kept.is_empty() {
        let mut have: FxHashSet<(String, i64)> = list.result.iter().map(key).collect();
        list.result
            .extend(f.kept.iter().filter(|s| have.insert(key(s))).cloned());
    }
    Arc::new(list)
}

/// Ask every kept server whether it still runs, and drop those that do not
/// answer (twice). The ones that answer stay, their pings recorded.
pub(crate) fn check_kept(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let kept: Vec<(String, i64)> = found().kept.iter().map(key).collect();
        if kept.is_empty() {
            return;
        }
        let Some(state) = app.try_state::<SharedState>().map(|s| Arc::clone(&s)) else {
            return;
        };
        let Ok(client) = dz_a2s::client_with_timeout(INFO_TIMEOUT).await else {
            return;
        };
        let client = Arc::new(client);
        let total = kept.len();
        let gone: FxHashSet<(String, i64)> = stream::iter(kept)
            .map(|(ip, port)| {
                let client = Arc::clone(&client);
                async move {
                    let addr = format!("{ip}:{port}");
                    for _ in 0..KEPT_TRIES {
                        if let Ok(p) = dz_a2s::ping_using(&client, &addr).await {
                            live::store().record(&ip, port, Live::from_result(&Ok(p)));
                            return None;
                        }
                    }
                    Some((ip, port))
                }
            })
            .buffer_unordered(CONCURRENCY)
            .filter_map(std::future::ready)
            .collect()
            .await;
        if !gone.is_empty() {
            found().kept.retain(|s| !gone.contains(&key(s)));
            let list = compose();
            state.write().await.set_servers(Arc::clone(&list));
            super::save_cache_in_background(Arc::clone(&list));
            crate::features::browser::list_replaced(list);
        }
        eprintln!(
            "kept servers: {} of {total} still answer",
            total - gone.len()
        );
    });
}

/// Stop the listing running, if any.
pub(crate) fn stop() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// List Steam's servers in the background, unless a listing runs, ran in
/// the last minutes, or the profile turned it off.
pub(crate) fn start(app: AppHandle) {
    if found().last.is_some_and(|t| t.elapsed() < MIN_INTERVAL)
        || RUNNING.swap(true, Ordering::SeqCst)
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        run(&app).await;
        RUNNING.store(false, Ordering::SeqCst);
    });
}

async fn run(app: &AppHandle) {
    let Some(state) = app.try_state::<SharedState>().map(|s| Arc::clone(&s)) else {
        return;
    };
    let (enabled, maps, known) = {
        let s = state.read().await;
        let known: FxHashSet<(String, i64)> = s
            .servers
            .result
            .iter()
            .map(|s| (s.endpoint.ip.clone(), s.endpoint.port))
            .collect();
        (
            s.ctl.profile().steam_server_list,
            busiest_maps(&s.servers),
            known,
        )
    };
    if !enabled {
        return;
    }
    let started = Instant::now();
    found().last = Some(started);
    CANCEL.store(false, Ordering::SeqCst);
    let Ok(client) = dz_a2s::client_with_timeout(INFO_TIMEOUT).await else {
        return;
    };
    let client = Arc::new(client);

    let (tx, rx) = mpsc::unbounded_channel::<(MasterEntry, Option<bool>)>();
    let parts = dz_steamworks::partitions(&maps);
    let lister = tauri::async_runtime::spawn_blocking(move || list_all(&parts, &tx));

    let mut seen = known;
    let entries = stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|e| (e, rx)) });
    let mut answers = Box::pin(
        entries
            .filter_map(move |(e, empty)| {
                let fresh = seen.insert((e.ip.to_string(), i64::from(e.query_port)));
                std::future::ready(fresh.then_some((e, empty)))
            })
            .map(|(e, empty)| query(Arc::clone(&client), e, empty))
            .buffer_unordered(CONCURRENCY)
            .filter_map(std::future::ready)
            .fuse(),
    );

    let mut fresh: Vec<SteamListed> = Vec::new();
    let mut added = 0;
    let mut tick = tokio::time::interval(PUBLISH_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            next = answers.next() => match next {
                Some(row) => fresh.push(row),
                None => break,
            },
            _ = tick.tick() => {
                added += fresh.len();
                publish(&state, std::mem::take(&mut fresh)).await;
            }
        }
    }
    added += fresh.len();
    publish(&state, fresh).await;
    let _ = lister.await;

    let list = Arc::clone(&state.read().await.servers);
    super::save_cache_in_background(Arc::clone(&list));
    crate::features::browser::list_replaced(list);
    eprintln!(
        "steam servers: {added} added in {:.1} s",
        started.elapsed().as_secs_f32()
    );
}

/// Put `rows` in the list the app shows.
async fn publish(state: &SharedState, rows: Vec<SteamListed>) {
    if rows.is_empty() {
        return;
    }
    {
        let mut f = found();
        for r in rows {
            if f.keys.insert((r.ip.clone(), r.query_port)) {
                f.rows.push(r);
            }
        }
    }
    let list = compose();
    state.write().await.set_servers(list);
    live::store().touch();
}

/// The maps of `list` with the most servers, busiest first.
fn busiest_maps(list: &ServerList) -> Vec<String> {
    let mut count: FxHashMap<&str, usize> = FxHashMap::default();
    for s in &list.result {
        if !s.map.is_empty() {
            *count.entry(s.map.as_str()).or_default() += 1;
        }
    }
    let mut maps: Vec<(&str, usize)> = count.into_iter().collect();
    maps.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    maps.into_iter()
        .take(MAX_MAPS)
        .map(|(m, _)| m.to_owned())
        .collect()
}

/// Ask Steam for each partition, one short session each, waiting out the
/// master's refusals. Addresses go to `tx`: at once for servers Steam counts
/// players on, the others once their partition is complete and farms can
/// be told. Blocking.
fn list_all(parts: &[Filters], tx: &mpsc::UnboundedSender<(MasterEntry, Option<bool>)>) {
    for part in parts {
        for attempt in 0..=RETRIES {
            if CANCEL.load(Ordering::SeqCst) {
                return;
            }
            let mut pending: Vec<(MasterEntry, Option<bool>)> = Vec::new();
            let mut refused = false;
            let outcome = dz_steamworks::list_servers(
                std::slice::from_ref(part),
                &CANCEL,
                &mut |ev| match ev {
                    ListEvent::Found { empty, servers } if empty == Some(false) => {
                        for s in servers {
                            let _ = tx.send((s, empty));
                        }
                    }
                    ListEvent::Found { empty, servers } => {
                        let mut f = found();
                        for s in servers {
                            f.farms
                                .add(&s.ip.to_string(), i64::from(s.query_port), empty);
                            pending.push((s, empty));
                        }
                    }
                    ListEvent::Done(l) => {
                        refused |= l.answer == Answer::NoServers && l.listed == 0;
                        eprintln!(
                            "steam servers: {:?} {:?}, {} listed{}",
                            l.filters,
                            l.answer,
                            l.listed,
                            if l.capped { " (capped)" } else { "" }
                        );
                        let f = found();
                        for (s, empty) in pending.drain(..) {
                            if !f.farms.is_farm(&s.ip.to_string()) {
                                let _ = tx.send((s, empty));
                            }
                        }
                    }
                },
            );
            match outcome {
                Err(e) => {
                    // Steam not running, busy, or the account not logged in.
                    eprintln!("steam servers: {e}");
                    return;
                }
                Ok(_) if refused && attempt < RETRIES => {
                    if !wait(THROTTLE_WAIT) {
                        return;
                    }
                }
                Ok(_) => break,
            }
        }
    }
}

/// Sleep `d` unless the listing is stopped; false when it is.
fn wait(d: Duration) -> bool {
    let end = Instant::now() + d;
    while Instant::now() < end {
        if CANCEL.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    true
}

/// A2S_INFO, and A2S_RULES for a modded server's mods. `None` when it does
/// not answer, or is modded and its mods cannot be read (it could not be
/// joined with them).
async fn query(
    client: Arc<dz_a2s::Client>,
    e: MasterEntry,
    empty: Option<bool>,
) -> Option<SteamListed> {
    let ip = e.ip.to_string();
    let port = i64::from(e.query_port);
    let addr = format!("{ip}:{port}");
    let (info, latency) = client.info(addr.as_str(), None).await.ok()?;
    let players = dz_a2s::human_player_count(&info);
    live::store().record(
        &ip,
        port,
        Live {
            ms: Some(
                u32::try_from(latency.as_millis())
                    .unwrap_or(u32::MAX)
                    .max(1),
            ),
            players: Some(players),
            max_players: Some(info.max_players),
            bots: Some(info.bots),
            failed: false,
        },
    );
    let ext = &info.extended_server_info;
    let tags = ext.keywords.clone().unwrap_or_default();
    let modded = tags.split(',').any(|t| t.trim() == "mod");
    let mods = if modded {
        let rules = dz_a2s::query_dayz_rules(&addr, RULES_TIMEOUT).await.ok()?;
        let mods: Vec<Mod> = rules
            .mods
            .into_iter()
            .filter_map(|m| {
                Some(Mod {
                    name: m.name,
                    steam_workshop_id: i64::try_from(m.id?).ok()?,
                })
            })
            .collect();
        if mods.is_empty() {
            return None;
        }
        mods
    } else {
        Vec::new()
    };
    Some(SteamListed {
        query_port: port,
        game_port: ext.port.map_or(i64::from(e.game_port), i64::from),
        name: info.name.trim().to_owned(),
        map: info.map.clone(),
        tags,
        players: i64::from(players),
        max_players: i64::from(info.max_players),
        password: info.visibility,
        secure: info.vac,
        version: info.version.clone(),
        environment: String::from(char::from(info.server_os as u8)),
        mods,
        empty,
        ip,
    })
}
