//! The servers Steam's master lists that the launcher list lacks, found and
//! added to the list as they answer.
//!
//! The master is reached the best way at hand, the other as its fallback:
//!
//! - **Web API** (a key in the profile): about two seconds a request, a few
//!   at once, no Steam session. A request that hits the 10 000 cap is split
//!   by how the names it answered start, whatever the script, plus the rest
//!   (`dz_api::WebPartition`), which brings back the honest servers farms
//!   crowd out. Its silent rationing (an empty answer) is waited out.
//! - **Steam client** (Steamworks): addresses in about a second, but about
//!   one request a minute once past a short burst, so only the most useful
//!   (servers with players, then the busiest maps); each runs in its own
//!   short session (Steam shows DayZ running for those seconds), a refused
//!   one asked again after a pause.
//!
//! Both give addresses only: each new one is queried here (A2S_INFO, and
//! A2S_RULES for its mods), many at once, and lands in the list as soon as
//! it answers, joining the ping scan's queue (the burst of queries would
//! skew a round trip measured here). Farms (many empty servers on one address) are left out
//! before anything is asked of them.
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

use dz_api::{FarmCounter, Mod, Server, ServerList, SteamListed, WebPartition};
use dz_steamworks::{Answer, Filters, ListEvent};
use futures_util::stream::FuturesUnordered;
use futures_util::{StreamExt, stream};
use rustc_hash::{FxHashMap, FxHashSet};
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use super::sources::{self, Phase, SteamVia};
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
/// Web API requests in flight at once, and the pause between two starts.
const WEB_CONCURRENCY: usize = 4;
const WEB_SPACING: Duration = Duration::from_millis(250);
/// The Web API withholds answers past a burst (an empty `200`): the first
/// pause, doubled each time it is still withholding, up to the last.
const WEB_THROTTLE_FIRST: Duration = Duration::from_secs(15);
const WEB_THROTTLE_MAX: Duration = Duration::from_secs(120);
/// Pauses in a row after which the listing gives up.
const WEB_THROTTLE_GIVE_UP: u32 = 8;
/// Maps asked for one by one through the Web API: every map the list has
/// at least this many servers on, at most [`WEB_MAX_MAPS`].
const WEB_MIN_SERVERS: usize = 2;
const WEB_MAX_MAPS: usize = 60;
/// Wait after the client's master refused a request (about one a minute).
const THROTTLE_WAIT: Duration = Duration::from_secs(60);
/// Times a refused request is asked again.
const RETRIES: usize = 2;
/// Maps asked for one by one through the client, the busiest first.
const CLIENT_MAX_MAPS: usize = 8;
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

/// An address the master listed, with what it counts for its players.
#[derive(Debug, Clone)]
struct Candidate {
    ip: String,
    query_port: u16,
    game_port: u16,
    empty: Option<bool>,
}

// ── the list ────────────────────────────────────────────────────────────

/// `base`, the launcher list, with what Steam found merged in. Keeps
/// `base` for the merges to come.
pub(crate) fn with_steam(base: Arc<ServerList>) -> Arc<ServerList> {
    found().base = base;
    compose()
}

/// [`with_steam`] for a refresh: the servers of `previous` that the new
/// list lacks are kept, until [`check_kept`] finds them gone.
pub(crate) fn refreshed(previous: &ServerList, base: Arc<ServerList>) -> Arc<ServerList> {
    let kept = {
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
        f.kept.len() as u32
    };
    sources::update(|s| {
        s.kept = sources::KeptStatus {
            phase: if kept > 0 {
                Phase::Running
            } else {
                Phase::Done
            },
            total: kept,
            ..Default::default()
        };
    });
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

/// Put the composed list in the app.
async fn show(state: &SharedState) -> Arc<ServerList> {
    let list = compose();
    state.write().await.set_servers(Arc::clone(&list));
    live::store().touch();
    list
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
        let gone: FxHashSet<(String, i64)> = stream::iter(kept)
            .map(|(ip, port)| {
                let client = Arc::clone(&client);
                async move {
                    let addr = format!("{ip}:{port}");
                    let mut alive = false;
                    for _ in 0..KEPT_TRIES {
                        if let Ok(p) = dz_a2s::ping_using(&client, &addr).await {
                            live::store().record(&ip, port, Live::from_result(&Ok(p)));
                            alive = true;
                            break;
                        }
                    }
                    sources::update(|s| {
                        s.kept.checked += 1;
                        if alive {
                            s.kept.alive += 1;
                        } else {
                            s.kept.removed += 1;
                        }
                    });
                    (!alive).then_some((ip, port))
                }
            })
            .buffer_unordered(CONCURRENCY)
            .filter_map(std::future::ready)
            .collect()
            .await;
        if !gone.is_empty() {
            found().kept.retain(|s| !gone.contains(&key(s)));
            let list = show(&state).await;
            super::save_cache_in_background(Arc::clone(&list));
            crate::features::browser::list_replaced(list);
        }
        sources::update(|s| s.kept.phase = Phase::Done);
    });
}

// ── the listing ─────────────────────────────────────────────────────────

/// Stop the listing running, if any.
pub(crate) fn stop() {
    CANCEL.store(true, Ordering::SeqCst);
    sources::update(|s| s.steam.phase = Phase::Off);
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
    let (enabled, maps, known, ping_settings) = {
        let s = state.read().await;
        let known: FxHashSet<(String, i64)> = s.servers.result.iter().map(key).collect();
        let p = s.ctl.profile();
        (
            p.steam_server_list,
            maps_by_size(&s.servers),
            known,
            // As the window's scan asks (browser::start_scan); `None` when
            // the profile does not scan the list.
            p.ping_scan_servers.then(|| PingSettings {
                concurrency: (p.ping_concurrency as usize).clamp(5, 200),
                timeout: Duration::from_millis(u64::from(p.ping_timeout_auto).clamp(1000, 5000)),
                retries: p.ping_max_retries.min(5),
            }),
        )
    };
    if !enabled {
        sources::update(|s| s.steam.phase = Phase::Off);
        return;
    }
    let started = Instant::now();
    found().last = Some(started);
    CANCEL.store(false, Ordering::SeqCst);
    let web_key = crate::features::steam::access::web_key(&state).await;
    sources::update(|s| {
        s.steam = sources::SteamStatus {
            phase: Phase::Running,
            via: Some(if web_key.is_some() {
                SteamVia::WebApi
            } else {
                SteamVia::SteamClient
            }),
            started: Some(sources::now()),
            ..Default::default()
        };
    });
    let Ok(client) = dz_a2s::client_with_timeout(INFO_TIMEOUT).await else {
        sources::update(|s| s.steam.phase = Phase::Failed);
        return;
    };
    let client = Arc::new(client);

    let (tx, rx) = mpsc::unbounded_channel::<Candidate>();
    let lister = {
        let maps = maps.clone();
        tauri::async_runtime::spawn(async move { list(web_key, &maps, &tx).await })
    };

    let mut seen = known;
    let candidates = stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|c| (c, rx)) });
    let mut answers = Box::pin(
        candidates
            .filter_map(move |c| {
                let fresh = seen.insert((c.ip.clone(), i64::from(c.query_port)));
                if !fresh {
                    sources::update(|s| s.steam.known += 1);
                }
                std::future::ready(fresh.then_some(c))
            })
            .map(|c| query(Arc::clone(&client), c))
            .buffer_unordered(CONCURRENCY)
            .filter_map(std::future::ready)
            .fuse(),
    );

    let mut fresh: Vec<SteamListed> = Vec::new();
    let mut tick = tokio::time::interval(PUBLISH_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            next = answers.next() => match next {
                Some(row) => fresh.push(row),
                None => break,
            },
            _ = tick.tick() => publish(app, &state, std::mem::take(&mut fresh), ping_settings).await,
        }
    }
    publish(app, &state, fresh, ping_settings).await;
    let outcome = lister.await.unwrap_or_else(|e| Err(e.to_string()));

    let list = Arc::clone(&state.read().await.servers);
    super::save_cache_in_background(Arc::clone(&list));
    crate::features::browser::list_replaced(list);
    sources::update(|s| {
        s.steam.finished = Some(sources::now());
        s.steam.current = None;
        s.steam.waiting_until = None;
        match outcome {
            Ok(()) => s.steam.phase = Phase::Done,
            Err(e) => {
                s.steam.phase = Phase::Failed;
                s.steam.note = Some(e);
            }
        }
        if CANCEL.load(Ordering::SeqCst) {
            s.steam.phase = Phase::Off;
        }
    });
}

/// How the profile has the list pinged.
#[derive(Clone, Copy)]
struct PingSettings {
    concurrency: usize,
    timeout: Duration,
    retries: u32,
}

/// Put `rows` in the list the app shows, and in the ping scan's queue.
async fn publish(
    app: &AppHandle,
    state: &SharedState,
    rows: Vec<SteamListed>,
    ping: Option<PingSettings>,
) {
    if rows.is_empty() {
        return;
    }
    let added: Vec<String> = {
        let mut f = found();
        let mut added = Vec::new();
        for r in rows {
            if f.keys.insert((r.ip.clone(), r.query_port)) {
                added.push(format!("{}:{}", r.ip, r.query_port));
                f.rows.push(r);
            }
        }
        added
    };
    sources::update(|s| s.steam.added += added.len() as u32);
    show(state).await;
    if let Some(p) = ping
        && let Some(scan) = app.try_state::<Arc<crate::features::ping::PingState>>()
    {
        crate::features::ping::enqueue(&scan, added, p.concurrency, p.timeout, p.retries);
    }
}

/// The maps of `list`, the one with the most servers first.
fn maps_by_size(list: &ServerList) -> Vec<(String, usize)> {
    let mut count: FxHashMap<&str, usize> = FxHashMap::default();
    for s in &list.result {
        if !s.map.is_empty() {
            *count.entry(s.map.as_str()).or_default() += 1;
        }
    }
    let mut maps: Vec<(&str, usize)> = count.into_iter().collect();
    maps.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    maps.into_iter().map(|(m, n)| (m.to_owned(), n)).collect()
}

/// Ask the master the best way at hand: the Web API with a key, the Steam
/// client otherwise or when the Web API fails outright.
async fn list(
    web_key: Option<String>,
    maps: &[(String, usize)],
    tx: &mpsc::UnboundedSender<Candidate>,
) -> Result<(), String> {
    if let Some(key) = web_key {
        let names: Vec<String> = maps
            .iter()
            .filter(|(_, n)| *n >= WEB_MIN_SERVERS)
            .take(WEB_MAX_MAPS)
            .map(|(m, _)| m.clone())
            .collect();
        match list_web(&key, &names, tx).await {
            Ok(()) => return Ok(()),
            Err(e) => sources::update(|s| {
                s.steam.via = Some(SteamVia::SteamClient);
                s.steam.note = Some(format!("Web API: {e}. Through the Steam client instead."));
            }),
        }
    }
    let names: Vec<String> = maps
        .iter()
        .take(CLIENT_MAX_MAPS)
        .map(|(m, _)| m.clone())
        .collect();
    let tx = tx.clone();
    tauri::async_runtime::spawn_blocking(move || list_client(&names, &tx))
        .await
        .map_err(|e| e.to_string())?
}

/// Hand on what a request listed: at once for servers Steam counts players
/// on, the others unless they sit on a farm address.
fn hand_on(rows: Vec<Candidate>, tx: &mpsc::UnboundedSender<Candidate>) {
    let n = rows.len() as u32;
    let mut farms = 0;
    {
        let mut f = found();
        for c in &rows {
            f.farms.add(&c.ip, i64::from(c.query_port), c.empty);
        }
        for c in rows {
            if c.empty != Some(false) && f.farms.is_farm(&c.ip) {
                farms += 1;
            } else {
                let _ = tx.send(c);
            }
        }
    }
    sources::update(|s| {
        s.steam.listed += n;
        s.steam.farms += farms;
    });
}

/// Through the Web API, splitting what hits the cap. `Err` only when the
/// very first request fails (a refused key, Steam unreachable): the client
/// then takes over.
async fn list_web(
    key: &str,
    maps: &[String],
    tx: &mpsc::UnboundedSender<Candidate>,
) -> Result<(), String> {
    let http = crate::net::api();
    let mut queue: std::collections::VecDeque<WebPartition> = dz_api::web_partitions(maps).into();
    sources::update(|s| s.steam.requests_total = queue.len() as u32);
    let mut running = FuturesUnordered::new();
    let mut answered_once = false;
    // While the master withholds answers: until when, how long the next
    // pause is, and how many there have been in a row.
    let mut paused_until: Option<Instant> = None;
    let mut backoff = WEB_THROTTLE_FIRST;
    let mut pauses = 0;
    loop {
        if CANCEL.load(Ordering::SeqCst) {
            return Ok(());
        }
        if let Some(until) = paused_until {
            if Instant::now() < until {
                if running.is_empty() {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
            } else {
                paused_until = None;
                sources::update(|s| s.steam.waiting_until = None);
            }
        }
        while paused_until.is_none()
            && running.len() < WEB_CONCURRENCY
            && let Some(part) = queue.pop_front()
        {
            sources::update(|s| s.steam.current = Some(part.label()));
            running.push(async move {
                let mut r = dz_api::fetch_web_partition(http, key, &part).await;
                // One lost request should not cost a partition.
                if r.as_ref().is_err_and(|e| !e.contains("API key")) {
                    r = dz_api::fetch_web_partition(http, key, &part).await;
                }
                (part, r)
            });
            tokio::time::sleep(WEB_SPACING).await;
        }
        let Some((part, result)) = running.next().await else {
            if queue.is_empty() {
                break;
            }
            continue;
        };
        match result {
            // Nothing: matched nothing, or withheld. Asking for one server
            // with players tells which.
            Ok(rows) if rows.is_empty() && dz_api::web_throttled(http, key).await => {
                queue.push_front(part);
                // Its fellows in flight withheld too: one pause for all.
                if paused_until.is_some() {
                    continue;
                }
                pauses += 1;
                if pauses > WEB_THROTTLE_GIVE_UP {
                    return Err("Steam kept withholding the server list".into());
                }
                let until = Instant::now() + backoff;
                let shown = sources::now() + backoff.as_secs() as i64;
                sources::update(|s| s.steam.waiting_until = Some(shown));
                paused_until = Some(until);
                backoff = (backoff * 2).min(WEB_THROTTLE_MAX);
            }
            Ok(rows) => {
                answered_once = true;
                pauses = 0;
                backoff = WEB_THROTTLE_FIRST;
                sources::update(|s| s.steam.requests_done += 1);
                if rows.len() >= dz_api::WEB_LIST_CAP {
                    let more = part.split(rows.iter().map(|r| r.name.as_str()));
                    sources::update(|s| s.steam.requests_total += more.len() as u32);
                    queue.extend(more);
                }
                hand_on(
                    rows.into_iter()
                        .map(|r| Candidate {
                            ip: r.ip,
                            query_port: r.query_port,
                            game_port: r.game_port,
                            empty: part.empty,
                        })
                        .collect(),
                    tx,
                );
            }
            Err(e) if !answered_once => return Err(e),
            Err(e) => {
                sources::update(|s| s.steam.requests_done += 1);
                eprintln!("steam servers: {}: {e}", part.label());
            }
        }
    }
    Ok(())
}

/// Through the Steam client, one short session per partition, waiting out
/// the master's refusals. Blocking.
fn list_client(maps: &[String], tx: &mpsc::UnboundedSender<Candidate>) -> Result<(), String> {
    let parts = dz_steamworks::partitions(maps);
    sources::update(|s| s.steam.requests_total = parts.len() as u32);
    for part in &parts {
        sources::update(|s| s.steam.current = Some(client_label(part)));
        for attempt in 0..=RETRIES {
            if CANCEL.load(Ordering::SeqCst) {
                return Ok(());
            }
            let mut rows: Vec<Candidate> = Vec::new();
            let mut refused = false;
            let outcome = dz_steamworks::list_servers(
                std::slice::from_ref(part),
                &CANCEL,
                &mut |ev| match ev {
                    ListEvent::Found { empty, servers } => {
                        let found = servers.into_iter().map(|s| Candidate {
                            ip: s.ip.to_string(),
                            query_port: s.query_port,
                            game_port: s.game_port,
                            empty,
                        });
                        // With players: on at once; empty: once the whole
                        // partition is in, so farms can be told.
                        if empty == Some(false) {
                            hand_on(found.collect(), tx);
                        } else {
                            rows.extend(found);
                        }
                    }
                    ListEvent::Done(l) => {
                        refused |= l.answer == Answer::NoServers && l.listed == 0;
                        hand_on(std::mem::take(&mut rows), tx);
                    }
                },
            );
            match outcome {
                // Steam not running, busy, or the account not logged in.
                Err(e) => return Err(e),
                Ok(_) if refused && attempt < RETRIES => {
                    let until = sources::now() + THROTTLE_WAIT.as_secs() as i64;
                    sources::update(|s| s.steam.waiting_until = Some(until));
                    let went_on = wait(THROTTLE_WAIT);
                    sources::update(|s| s.steam.waiting_until = None);
                    if !went_on {
                        return Ok(());
                    }
                }
                Ok(_) => break,
            }
        }
        sources::update(|s| s.steam.requests_done += 1);
    }
    Ok(())
}

fn client_label(f: &Filters) -> String {
    let mut parts = Vec::new();
    match dz_steamworks::empty_of(f) {
        Some(true) => parts.push("empty".to_owned()),
        Some(false) => parts.push("with players".to_owned()),
        None => {}
    }
    if let Some((_, map)) = f.iter().find(|(k, _)| k == "map") {
        parts.push(format!("map {map}"));
    }
    parts.join(" · ")
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

// ── the servers themselves ──────────────────────────────────────────────

/// A2S_INFO, and A2S_RULES for a modded server's mods. `None` when it does
/// not answer, or is modded and its mods cannot be read (it could not be
/// joined with them).
async fn query(client: Arc<dz_a2s::Client>, c: Candidate) -> Option<SteamListed> {
    sources::update(|s| s.steam.queried += 1);
    let port = i64::from(c.query_port);
    let addr = format!("{}:{port}", c.ip);
    let (info, _) = client.info(addr.as_str(), None).await.ok()?;
    sources::update(|s| s.steam.answered += 1);
    let players = dz_a2s::human_player_count(&info);
    // Its counts now; its ping from the scan, once it is in the list.
    live::store().record_counts(&c.ip, port, players, info.max_players, info.bots);
    let ext = &info.extended_server_info;
    let tags = ext.keywords.clone().unwrap_or_default();
    let modded = tags.split(',').any(|t| t.trim() == "mod");
    let mods = if modded {
        let mods: Vec<Mod> = dz_a2s::query_dayz_rules(&addr, RULES_TIMEOUT)
            .await
            .map(|r| {
                r.mods
                    .into_iter()
                    .filter_map(|m| {
                        Some(Mod {
                            name: m.name,
                            steam_workshop_id: i64::try_from(m.id?).ok()?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if mods.is_empty() {
            sources::update(|s| s.steam.no_mods += 1);
            return None;
        }
        mods
    } else {
        Vec::new()
    };
    Some(SteamListed {
        query_port: port,
        game_port: ext.port.map_or(i64::from(c.game_port), i64::from),
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
        empty: c.empty,
        ip: c.ip,
    })
}
