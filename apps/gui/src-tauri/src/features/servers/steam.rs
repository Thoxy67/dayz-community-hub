//! The servers Steam's master lists that the launcher list lacks, found and
//! added to the list as they answer.
//!
//! The master is reached through the Steam **Web API** only (a key in the
//! profile): about two seconds a request, a few at once, no Steam session.
//! A request that hits the 10 000 cap is split by how the names it answered
//! start, whatever the script, plus the rest (`dz_api::WebPartition`), which
//! brings back the honest servers farms crowd out. Its silent rationing (an
//! empty answer) is waited out. Without a key the listing does not run and
//! says so (`SteamStatus::needs_key`): the Steam client is never asked, as it
//! showed DayZ running for each request and answered about once a minute.
//!
//! Both give addresses only: each new one is queried here (A2S_INFO, and
//! A2S_RULES for its mods), many at once, and lands in the list as soon as
//! it answers, joining the ping scan's queue (the burst of queries would
//! skew a round trip measured here). Farms are left out: many empty servers
//! on one address before anything is asked of them, and any server
//! reporting bots (DayZ has none: farm emulators pad their count with them)
//! once it answers. Kept servers reporting bots leave too.
//!
//! What Steam added is kept apart from the launcher list it is merged into
//! ([`compose`]), so a refresh of that list keeps it, and so farms found
//! late still leave the list (`dz_api::FarmCounter`).
//!
//! A server the list had and a refresh no longer lists is kept too: it may
//! be delisted and still running. It leaves only once it does not answer
//! ([`check_kept`]). The cache holds the whole list, so this holds across
//! restarts as well.
//!
//! Every list put together is combed for duplicates ([`comb`]): entries
//! that share an address to join (ip, game port) under different query
//! ports are asked, and only the one that answers for that game port stays.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use dz_api::{FarmCounter, Mod, Server, ServerList, SteamListed, WebPartition};
use futures_util::stream::FuturesUnordered;
use futures_util::{StreamExt, stream};
use rustc_hash::{FxHashMap, FxHashSet};
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use super::sources::{self, Phase};
use crate::features::browser::live::{self, Live};
use crate::state::SharedState;

/// A2S queries in flight at once.
const CONCURRENCY: usize = 128;
/// The second pass, for what did not answer the first: slower, more patient.
/// UDP drops replies under a burst, and a server missed is otherwise
/// "found" again by the next listing.
const SLOW_CONCURRENCY: usize = 64;
const SLOW_TIMEOUT: Duration = Duration::from_secs(3);
/// How often the list is written to the cache while a listing adds to it,
/// so closing the launcher midway loses little.
const SAVE_EVERY: Duration = Duration::from_secs(30);
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
    /// Entries combed out as duplicates, by (ip, query port), and the join
    /// addresses (ip, game port) already combed.
    dropped: FxHashSet<(String, i64)>,
    combed: FxHashSet<(String, i64)>,
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

/// An (ip, port) pair.
type Addr = (String, i64);

fn key(s: &Server) -> Addr {
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
    if f.rows.is_empty() && f.kept.is_empty() && f.dropped.is_empty() {
        return Arc::clone(&f.base);
    }
    // One allocation of the final size: cloning the base and then merging
    // into it grew the list a second time.
    let mut list = ServerList {
        status: f.base.status,
        result: dz_api::merged_steam(&f.base.result, &f.rows, &f.farms),
    };
    if !f.kept.is_empty() {
        let mut have: FxHashSet<(String, i64)> = list.result.iter().map(key).collect();
        list.result
            .extend(f.kept.iter().filter(|s| have.insert(key(s))).cloned());
    }
    if !f.dropped.is_empty() {
        list.result.retain(|s| !f.dropped.contains(&key(s)));
    }
    Arc::new(list)
}

/// Put the composed list in the app, and comb it.
async fn show(app: &AppHandle, state: &SharedState) -> Arc<ServerList> {
    let list = compose();
    state.write().await.set_servers(Arc::clone(&list));
    live::store().touch();
    comb(app.clone());
    list
}

static COMBING: AtomicBool = AtomicBool::new(false);
/// The list changed while a comb ran: comb again after it.
static RECOMB: AtomicBool = AtomicBool::new(false);

/// Comb the list for duplicates in the background: entries that claim the
/// same address to join under different query ports (`dz_api::join_conflicts`).
/// Each is asked (A2S_INFO, twice); the first that answers stating that game
/// port stays, else the first that answers, else all stay (the kept check
/// deals with servers that do not answer). Each address is combed once.
pub(crate) fn comb(app: AppHandle) {
    RECOMB.store(true, Ordering::SeqCst);
    if COMBING.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        while RECOMB.swap(false, Ordering::SeqCst) {
            comb_once(&app).await;
        }
        COMBING.store(false, Ordering::SeqCst);
    });
}

async fn comb_once(app: &AppHandle) {
    let Some(state) = app.try_state::<SharedState>().map(|s| Arc::clone(&s)) else {
        return;
    };
    let list = Arc::clone(&state.read().await.servers);
    let groups: Vec<(i64, Vec<(String, i64)>)> = {
        let conflicts = dz_api::join_conflicts(&list.result);
        if conflicts.is_empty() {
            return;
        }
        // Borrowed keys: this runs on every change to the list while a
        // listing adds to it, and an owned key would copy every IP.
        let game_port: FxHashMap<(&str, i64), i64> = list
            .result
            .iter()
            .map(|s| ((s.endpoint.ip.as_str(), s.endpoint.port), s.game_port))
            .collect();
        let f = found();
        conflicts
            .into_iter()
            .filter_map(|g| {
                let gp = *game_port.get(&(g[0].0.as_str(), g[0].1))?;
                (!f.combed.contains(&(g[0].0.clone(), gp))).then_some((gp, g))
            })
            .collect()
    };
    if groups.is_empty() {
        return;
    }
    let Ok(client) = dz_a2s::client_with_timeout(INFO_TIMEOUT).await else {
        return;
    };
    let client = Arc::new(client);
    let verdicts: Vec<(Addr, Vec<Addr>)> = stream::iter(groups)
        .map(|(gp, group)| {
            let client = Arc::clone(&client);
            async move {
                let ip = group[0].0.clone();
                let mut stating = None;
                let mut answering = None;
                for (ip, qp) in &group {
                    let addr = format!("{ip}:{qp}");
                    for _ in 0..KEPT_TRIES {
                        if let Ok((info, _)) = client.info(addr.as_str(), None).await {
                            let port = info.extended_server_info.port.map(i64::from);
                            if stating.is_none() && port.is_none_or(|p| p == gp) {
                                stating = Some((ip.clone(), *qp));
                            }
                            answering.get_or_insert((ip.clone(), *qp));
                            break;
                        }
                    }
                }
                let losers = match stating.or(answering) {
                    Some(keep) => group.into_iter().filter(|k| *k != keep).collect(),
                    None => Vec::new(),
                };
                ((ip, gp), losers)
            }
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;
    let mut removed = 0;
    {
        let mut f = found();
        for ((ip, gp), losers) in verdicts {
            f.combed.insert((ip, gp));
            removed += losers.len();
            f.dropped.extend(losers);
        }
    }
    sources::update(|s| s.duplicates += removed as u32);
    if removed > 0 {
        let list = compose();
        state.write().await.set_servers(Arc::clone(&list));
        live::store().touch();
        super::save_cache_in_background(Arc::clone(&list));
        crate::features::browser::list_replaced(list);
    }
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
        let (Ok(fast), Ok(slow)) = (
            dz_a2s::client_with_timeout(INFO_TIMEOUT).await,
            dz_a2s::client_with_timeout(SLOW_TIMEOUT).await,
        ) else {
            return;
        };
        // Silent ones are asked again, slowly, before they are taken for gone.
        let silent = kept_pass(&fast, kept, CONCURRENCY, false).await;
        let gone = kept_pass(&slow, silent, SLOW_CONCURRENCY, true).await;
        if !gone.is_empty() {
            let gone: FxHashSet<(String, i64)> = gone.into_iter().collect();
            found().kept.retain(|s| !gone.contains(&key(s)));
            let list = show(&app, &state).await;
            super::save_cache_in_background(Arc::clone(&list));
            crate::features::browser::list_replaced(list);
        }
        sources::update(|s| s.kept.phase = Phase::Done);
    });
}

/// Ping `addrs`; returns those to drop: on the `last` pass the silent and
/// the fakes, before it the fakes and those to ask again (the silent).
async fn kept_pass(
    client: &dz_a2s::Client,
    addrs: Vec<Addr>,
    concurrency: usize,
    last: bool,
) -> Vec<Addr> {
    stream::iter(addrs)
        .map(|(ip, port)| async move {
            let addr = format!("{ip}:{port}");
            let mut alive = false;
            let mut fake = false;
            for _ in 0..KEPT_TRIES {
                if let Ok(p) = dz_a2s::ping_using(client, &addr).await {
                    live::store().record(&ip, port, Live::from_result(&Ok(p)));
                    alive = true;
                    fake = p.bots > 0;
                    break;
                }
            }
            if !alive && !last {
                return Some((ip, port));
            }
            sources::update(|s| {
                s.kept.checked += 1;
                if fake {
                    s.kept.fakes += 1;
                } else if alive {
                    s.kept.alive += 1;
                } else {
                    s.kept.removed += 1;
                }
            });
            (!alive || fake).then_some((ip, port))
        })
        .buffer_unordered(concurrency)
        .filter_map(std::future::ready)
        .collect()
        .await
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
    CANCEL.store(false, Ordering::SeqCst);
    let Some(web_key) = crate::features::steam::access::web_key(&state).await else {
        sources::update(|s| {
            s.steam = sources::SteamStatus {
                phase: Phase::Off,
                needs_key: true,
                ..Default::default()
            };
        });
        return;
    };
    found().last = Some(started);
    sources::update(|s| {
        s.steam = sources::SteamStatus {
            phase: Phase::Running,
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
            .map(|c| query(Arc::clone(&client), c, false))
            .buffer_unordered(CONCURRENCY)
            .fuse(),
    );
    let silent = take_answers(app, &state, &mut answers, ping_settings).await;
    let outcome = lister.await.unwrap_or_else(|e| Err(e.to_string()));

    // What did not answer, once more, patiently.
    if !silent.is_empty()
        && !CANCEL.load(Ordering::SeqCst)
        && let Ok(slow) = dz_a2s::client_with_timeout(SLOW_TIMEOUT).await
    {
        let slow = Arc::new(slow);
        sources::update(|s| s.steam.retried += silent.len() as u32);
        let mut again = Box::pin(
            stream::iter(silent)
                .map(|c| query(Arc::clone(&slow), c, true))
                .buffer_unordered(SLOW_CONCURRENCY)
                .fuse(),
        );
        take_answers(app, &state, &mut again, ping_settings).await;
    }

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

/// Put the servers `answers` yields in the list as they come; returns the
/// candidates that did not answer.
async fn take_answers(
    app: &AppHandle,
    state: &SharedState,
    answers: &mut (impl Unpin + futures_util::stream::FusedStream<Item = Outcome>),
    ping: Option<PingSettings>,
) -> Vec<Candidate> {
    let mut fresh: Vec<SteamListed> = Vec::new();
    let mut silent = Vec::new();
    let mut tick = tokio::time::interval(PUBLISH_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            next = answers.next() => match next {
                Some(Outcome::Row(row)) => fresh.push(row),
                Some(Outcome::Silent(c)) => silent.push(c),
                Some(Outcome::Rejected) => {}
                None => break,
            },
            _ = tick.tick() => publish(app, state, std::mem::take(&mut fresh), ping).await,
        }
    }
    publish(app, state, fresh, ping).await;
    silent
}

static LAST_SAVE: Mutex<Option<Instant>> = Mutex::new(None);

/// Put `rows` in the list the app shows, and in the ping scan's queue. The
/// cache is written every [`SAVE_EVERY`] meanwhile.
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
    // Only servers another pass already put in: the list is as it was, and
    // composing, indexing and combing 10 000 servers again would change nothing.
    if added.is_empty() {
        return;
    }
    sources::update(|s| s.steam.added += added.len() as u32);
    let list = show(app, state).await;
    let due = LAST_SAVE.lock().is_ok_and(|mut last| {
        let due = last.is_none_or(|t| t.elapsed() >= SAVE_EVERY);
        if due {
            *last = Some(Instant::now());
        }
        due
    });
    if due {
        super::save_cache_in_background(list);
    }
    if let Some(p) = ping
        && let Some(scan) = app.try_state::<Arc<crate::features::ping::PingState>>()
    {
        crate::features::ping::enqueue(app, &scan, added, p.concurrency, p.timeout, p.retries);
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

/// Ask the master through the Web API: every map with servers enough, the
/// busiest first.
async fn list(
    key: String,
    maps: &[(String, usize)],
    tx: &mpsc::UnboundedSender<Candidate>,
) -> Result<(), String> {
    let names: Vec<String> = maps
        .iter()
        .filter(|(_, n)| *n >= WEB_MIN_SERVERS)
        .take(WEB_MAX_MAPS)
        .map(|(m, _)| m.clone())
        .collect();
    list_web(&key, &names, tx).await
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

/// Through the Web API, splitting what hits the cap. `Err` when the very
/// first request fails (a refused key, Steam unreachable) or the master
/// keeps withholding answers.
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

// ── the servers themselves ──────────────────────────────────────────────

/// What asking a candidate gave.
enum Outcome {
    Row(SteamListed),
    /// No answer: asked again in the second pass.
    Silent(Candidate),
    /// A fake, or modded with mods that cannot be read.
    Rejected,
}

/// A2S_INFO, and A2S_RULES for a modded server's mods. A server that does
/// not answer is silent (asked again unless this is the `last` pass); a
/// fake, or a modded one whose mods cannot be read (it could not be joined
/// with them) on the last pass, is rejected.
async fn query(client: Arc<dz_a2s::Client>, c: Candidate, last: bool) -> Outcome {
    if !last {
        sources::update(|s| s.steam.queried += 1);
    }
    let port = i64::from(c.query_port);
    let addr = format!("{}:{port}", c.ip);
    let Ok((info, _)) = client.info(addr.as_str(), None).await else {
        return if last {
            Outcome::Rejected
        } else {
            Outcome::Silent(c)
        };
    };
    sources::update(|s| s.steam.answered += 1);
    // DayZ has no bots: a server reporting some is a farm's emulator
    // padding its count. Measured on 2026-10-08: of 388 dayzsalauncher.com
    // servers that answered, none reported a bot; of 388 only Steam
    // listed, 324 reported as many bots as players.
    if info.bots > 0 {
        sources::update(|s| s.steam.fakes += 1);
        return Outcome::Rejected;
    }
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
            if !last {
                return Outcome::Silent(c);
            }
            sources::update(|s| s.steam.no_mods += 1);
            return Outcome::Rejected;
        }
        mods
    } else {
        Vec::new()
    };
    Outcome::Row(SteamListed {
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
