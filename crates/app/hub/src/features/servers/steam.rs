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
//!
//! What a listing learned outlives the process (`super::memory`): a
//! listing finished less than [`memory::MIN_INTERVAL`] ago is not run again
//! after a restart (the sources bar says when it ran), the next one asks
//! the requests the last one found under the cap rather than splitting
//! again, addresses that answered as fakes, without mods, as duplicates or
//! not at all are not queried again for a while, and the servers it added
//! are its own again ([`remember`]), not kept servers to check.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::Hub;
use dz_api::{
    FarmCounter, MasterAddr, Mod, PlannedPart, Server, ServerList, SteamListed, SteamRow,
    WebPartition,
};
use futures_util::stream::FuturesUnordered;
use futures_util::{StreamExt, stream};
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::mpsc;

use super::memory::{self, ListingMemory, Memo, Verdict};
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
/// A listing is not started again sooner than this after the last started
/// in this process (whether it finished or not).
const MIN_INTERVAL: Duration = Duration::from_secs(memory::MIN_INTERVAL as u64);
/// Times a kept server is asked before it is taken for gone.
const KEPT_TRIES: usize = 2;

#[derive(Default)]
struct Found {
    /// The launcher list, before Steam's servers are merged in.
    base: Arc<ServerList>,
    /// Every server Steam listed that answered, once each, with its
    /// [`Server`] made once.
    rows: Vec<SteamRow>,
    keys: FxHashSet<(String, i64)>,
    farms: FarmCounter,
    /// Servers an earlier list had that the current one lacks, kept while
    /// they answer.
    kept: Vec<Arc<Server>>,
    /// Entries combed out as duplicates, by (ip, query port), and the join
    /// addresses (ip, game port) already combed.
    dropped: FxHashSet<(String, i64)>,
    combed: FxHashSet<(String, i64)>,
    /// When the last listing started.
    last: Option<Instant>,
    /// What listings learned of each address they queried.
    memo: FxHashMap<Addr, Memo>,
    /// The last complete listing (its addresses are in `memo`).
    saved: ListingMemory,
}

static FOUND: LazyLock<Mutex<Found>> = LazyLock::new(Mutex::default);
static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);

fn found() -> std::sync::MutexGuard<'static, Found> {
    FOUND
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

use memory::Addr;

fn key<S: std::borrow::Borrow<Server>>(s: &S) -> Addr {
    let s = s.borrow();
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

/// Take in what the last listings learned (`super::memory`), read at start
/// with the cached `list`: the servers the last complete listing added are
/// its rows again (kept by a refresh, not checked as kept servers), while
/// it finished in the last day and Steam listed them then. Called before
/// [`with_steam`].
pub(crate) fn remember(memory: Option<ListingMemory>, list: &ServerList) {
    let Some(mut m) = memory else {
        return;
    };
    let since = m.restorable_since(sources::now());
    let mut f = found();
    f.memo = m.take_addresses().collect();
    if let Some(since) = since {
        let mut rows = Vec::new();
        for s in &list.result {
            let k = key(s);
            let ours = f
                .memo
                .get(&k)
                .is_some_and(|m| m.verdict == Verdict::Answered && m.seen >= since);
            if ours && f.keys.insert(k) {
                rows.push(SteamRow::from_server(Arc::clone(s)));
            }
        }
        f.rows.extend(rows);
    }
    f.saved = m;
}

/// [`with_steam`] for a refresh: the servers of `previous` that the new
/// list lacks are kept, until [`check_kept`] finds them gone.
pub(crate) fn refreshed(previous: &ServerList, base: Arc<ServerList>) -> Arc<ServerList> {
    let kept = {
        let mut f = found();
        let mut known: FxHashSet<(String, i64)> = base.result.iter().map(key).collect();
        known.extend(f.keys.iter().cloned());
        f.kept.retain(|s| known.insert(key(s)));
        let more: Vec<Arc<Server>> = previous
            .result
            .iter()
            .filter(|s| known.insert(key(*s)))
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

/// The base, Steam's rows and the kept servers as one list. The servers are
/// shared with `base` and the rows (refcounts, no copies), and the keys
/// compared are borrowed: this runs every time more servers answer.
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
        let mut have: FxHashSet<(&str, i64)> = list
            .result
            .iter()
            .map(|s| (s.endpoint.ip.as_str(), s.endpoint.port))
            .collect();
        let more: Vec<Arc<Server>> = f
            .kept
            .iter()
            .filter(|s| have.insert((s.endpoint.ip.as_str(), s.endpoint.port)))
            .cloned()
            .collect();
        drop(have);
        list.result.extend(more);
    }
    if !f.dropped.is_empty() {
        let dropped: FxHashSet<(&str, i64)> =
            f.dropped.iter().map(|(ip, p)| (ip.as_str(), *p)).collect();
        list.result
            .retain(|s| !dropped.contains(&(s.endpoint.ip.as_str(), s.endpoint.port)));
    }
    Arc::new(list)
}

/// Held from composing a list to putting it in the app, so lists go in in
/// the order they were composed: one composed earlier (with fewer servers,
/// or before a refresh) never replaces a later one. Taken before the state's
/// lock, never while holding it.
pub(super) static LIST_WRITER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Put the composed list in the app (its snapshot built off the state's
/// lock), and comb it.
async fn show(hub: &Hub, state: &SharedState) -> Arc<ServerList> {
    let list = {
        let _order = LIST_WRITER.lock().await;
        let list = compose();
        crate::state::replace_servers(state, Arc::clone(&list)).await;
        list
    };
    live::store().touch();
    comb(hub.clone());
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
pub(crate) fn comb(hub: Hub) {
    RECOMB.store(true, Ordering::SeqCst);
    if COMBING.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async move {
        while RECOMB.swap(false, Ordering::SeqCst) {
            comb_once(&hub).await;
        }
        COMBING.store(false, Ordering::SeqCst);
    });
}

async fn comb_once(hub: &Hub) {
    let Some(state) = hub.try_state() else {
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
        let now = sources::now();
        for ((ip, gp), losers) in verdicts {
            f.combed.insert((ip, gp));
            removed += losers.len();
            // A listing does not query them again for a while.
            for k in &losers {
                if let Some(m) = f.memo.get_mut(k) {
                    m.verdict = Verdict::Duplicate;
                    m.at = now;
                }
            }
            f.dropped.extend(losers);
        }
    }
    sources::update(|s| s.duplicates += removed as u32);
    if removed > 0 {
        let list = {
            let _order = LIST_WRITER.lock().await;
            let list = compose();
            crate::state::replace_servers(&state, Arc::clone(&list)).await;
            list
        };
        live::store().touch();
        super::save_cache_in_background(Arc::clone(&list));
        crate::features::browser::list_replaced(list);
    }
}

/// Ask every kept server whether it still runs, and drop those that do not
/// answer (twice). The ones that answer stay, their pings recorded.
pub(crate) fn check_kept(hub: Hub) {
    tokio::spawn(async move {
        let kept: Vec<(String, i64)> = found().kept.iter().map(key).collect();
        if kept.is_empty() {
            return;
        }
        let Some(state) = hub.try_state() else {
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
            let list = show(&hub, &state).await;
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
pub(crate) fn start(hub: Hub) {
    if found().last.is_some_and(|t| t.elapsed() < MIN_INTERVAL)
        || RUNNING.swap(true, Ordering::SeqCst)
    {
        return;
    }
    tokio::spawn(async move {
        run(&hub).await;
        RUNNING.store(false, Ordering::SeqCst);
    });
}

async fn run(hub: &Hub) {
    let Some(state) = hub.try_state() else {
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
    let started_at = sources::now();
    // A listing finished minutes ago (before a restart, say): its servers
    // are in the list already, and the master would answer the same.
    let (recent, plan, remembered) = {
        let f = found();
        let recent = f.saved.next_listing(started_at).map(|next| {
            (
                next,
                f.saved.status.clone(),
                f.saved.started_at,
                f.saved.completed_at,
            )
        });
        let remembered: FxHashSet<Addr> = f
            .memo
            .iter()
            .filter(|(_, m)| m.holds(started_at))
            .map(|(a, _)| a.clone())
            .collect();
        (recent, f.saved.plan.clone(), remembered)
    };
    if let Some((next, last, from, to)) = recent {
        sources::update(|s| {
            s.steam = sources::SteamStatus {
                phase: Phase::Done,
                recent: true,
                next_at: Some(next),
                note: None,
                current: None,
                waiting_until: None,
                started: from,
                finished: to,
                ..last
            };
        });
        return;
    }
    found().last = Some(started);
    sources::update(|s| {
        s.steam = sources::SteamStatus {
            phase: Phase::Running,
            started: Some(started_at),
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
        tokio::spawn(async move { list(web_key, &maps, &plan, &tx).await })
    };

    let mut seen = known;
    let candidates = stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|c| (c, rx)) });
    let mut answers = Box::pin(
        candidates
            .filter_map(move |c| {
                let k = (c.ip.clone(), i64::from(c.query_port));
                let ask = if !seen.insert(k.clone()) {
                    sources::update(|s| s.steam.known += 1);
                    false
                } else if remembered.contains(&k) {
                    // Answered as a fake, without mods, as a duplicate or
                    // not at all, a short while ago.
                    sources::update(|s| s.steam.remembered += 1);
                    false
                } else {
                    true
                };
                std::future::ready(ask.then_some(c))
            })
            .map(|c| query(Arc::clone(&client), c, false))
            .buffer_unordered(CONCURRENCY)
            .fuse(),
    );
    let silent = take_answers(hub, &state, &mut answers, ping_settings).await;
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
        take_answers(hub, &state, &mut again, ping_settings).await;
    }

    let list = Arc::clone(&state.read().await.servers);
    super::save_cache_in_background(Arc::clone(&list));
    crate::features::browser::list_replaced(list);
    let cancelled = CANCEL.load(Ordering::SeqCst);
    sources::update(|s| {
        s.steam.finished = Some(sources::now());
        s.steam.current = None;
        s.steam.waiting_until = None;
        match &outcome {
            Ok(_) => s.steam.phase = Phase::Done,
            Err(e) => {
                s.steam.phase = Phase::Failed;
                s.steam.note = Some(e.clone());
            }
        }
        if cancelled {
            s.steam.phase = Phase::Off;
        }
    });
    let complete = match outcome {
        Ok(web) if !web.cancelled && !cancelled => Some(web),
        _ => None,
    };
    let memory = learned(complete, started_at);
    if let Err(e) = memory::save(memory::path(), memory).await {
        eprintln!("steam servers: the listing's memory was not saved: {e}");
    }
}

/// What to keep on disk after a listing: the addresses it judged, and, if
/// it went through (`complete`), when it ran, its figures and its plan. A
/// base with a request that failed keeps the plan it had.
fn learned(complete: Option<WebOutcome>, started_at: i64) -> ListingMemory {
    let now = sources::now();
    let mut f = found();
    if let Some(web) = complete {
        let mut done = web.leaves;
        for base in &web.failed {
            done.retain(|p| !p.part.same_base(base));
            done.extend(
                f.saved
                    .plan
                    .iter()
                    .filter(|p| p.part.same_base(base))
                    .cloned(),
            );
        }
        f.saved.plan = dz_api::compact_plan(done);
        f.saved.started_at = Some(started_at);
        f.saved.completed_at = Some(now);
        f.saved.status = sources::steam();
    }
    let mut m = f.saved.clone();
    m.set_addresses(f.memo.iter(), now);
    m
}

/// How the profile has the list pinged.
#[derive(Clone, Copy)]
struct PingSettings {
    concurrency: usize,
    timeout: Duration,
    retries: u32,
}

/// Put the servers `answers` yields in the list as they come; returns the
/// candidates that did not answer. How each answered is remembered.
async fn take_answers(
    hub: &Hub,
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
                Some(Outcome::Row(row)) => {
                    judged((row.ip.clone(), row.query_port), Verdict::Answered);
                    fresh.push(row);
                }
                Some(Outcome::Silent(c)) => silent.push(c),
                Some(Outcome::Rejected(addr, verdict)) => judged(addr, verdict),
                None => break,
            },
            _ = tick.tick() => publish(hub, state, std::mem::take(&mut fresh), ping).await,
        }
    }
    publish(hub, state, fresh, ping).await;
    silent
}

/// Remember how `addr`, listed now, answered.
fn judged(addr: Addr, verdict: Verdict) {
    let now = sources::now();
    found().memo.insert(
        addr,
        Memo {
            seen: now,
            verdict,
            at: now,
        },
    );
}

static LAST_SAVE: Mutex<Option<Instant>> = Mutex::new(None);

/// Put `rows` in the list the app shows, and in the ping scan's queue. The
/// cache is written every [`SAVE_EVERY`] meanwhile.
async fn publish(
    hub: &Hub,
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
                f.rows.push(SteamRow::from(r));
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
    let list = show(hub, state).await;
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
    if let Some(p) = ping {
        crate::features::ping::enqueue(hub, hub.ping(), added, p.concurrency, p.timeout, p.retries);
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
/// busiest first, as the last listing's `plan` split them.
async fn list(
    key: String,
    maps: &[(String, usize)],
    plan: &[PlannedPart],
    tx: &mpsc::UnboundedSender<Candidate>,
) -> Result<WebOutcome, String> {
    let names: Vec<String> = maps
        .iter()
        .filter(|(_, n)| *n >= WEB_MIN_SERVERS)
        .take(WEB_MAX_MAPS)
        .map(|(m, _)| m.clone())
        .collect();
    let parts = dz_api::planned_partitions(dz_api::web_partitions(&names), plan);
    let web = SteamWeb {
        http: crate::net::api(),
        key,
    };
    list_web(&web, parts, |part, rows| {
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
    })
    .await
}

/// Hand on what a request listed: at once for servers Steam counts players
/// on, the others unless they sit on a farm address. An address remembered
/// is marked as listed now.
fn hand_on(rows: Vec<Candidate>, tx: &mpsc::UnboundedSender<Candidate>) {
    let n = rows.len() as u32;
    let mut farms = 0;
    let now = sources::now();
    {
        let mut f = found();
        for c in &rows {
            f.farms.add(&c.ip, i64::from(c.query_port), c.empty);
        }
        for c in rows {
            if c.empty != Some(false) && f.farms.is_farm(&c.ip) {
                farms += 1;
            } else {
                if !f.memo.is_empty()
                    && let Some(m) = f.memo.get_mut(&(c.ip.clone(), i64::from(c.query_port)))
                {
                    m.seen = now;
                }
                let _ = tx.send(c);
            }
        }
    }
    sources::update(|s| {
        s.steam.listed += n;
        s.steam.farms += farms;
    });
}

/// Where the master's lists come from: Steam's Web API, or a made-up master
/// in the tests.
trait WebSource: Sync {
    /// One request's servers.
    fn fetch(
        &self,
        part: &WebPartition,
    ) -> impl Future<Output = Result<Vec<MasterAddr>, String>> + Send;
    /// Whether an empty answer was the master withholding it.
    fn throttled(&self) -> impl Future<Output = bool> + Send;
}

struct SteamWeb {
    http: &'static reqwest::Client,
    key: String,
}

impl WebSource for SteamWeb {
    fn fetch(
        &self,
        part: &WebPartition,
    ) -> impl Future<Output = Result<Vec<MasterAddr>, String>> + Send {
        dz_api::fetch_web_partition(self.http, &self.key, part)
    }

    fn throttled(&self) -> impl Future<Output = bool> + Send {
        dz_api::web_throttled(self.http, &self.key)
    }
}

/// What the requests of a listing came to.
#[derive(Debug, Default)]
struct WebOutcome {
    /// Every request that did not need splitting, with its servers: the
    /// plan of the next listing.
    leaves: Vec<PlannedPart>,
    /// The bases (requests before any split) one of whose requests failed.
    failed: Vec<WebPartition>,
    /// Stopped before the end.
    cancelled: bool,
    /// Requests answered (splits and retries after a pause included), and
    /// pauses the master imposed.
    requests: u32,
    pauses: u32,
}

/// Ask `parts` of the master, splitting what hits the cap; `on_rows` gets
/// each answer. `Err` when the very first request fails (a refused key,
/// Steam unreachable) or the master keeps withholding answers.
async fn list_web<S: WebSource>(
    src: &S,
    parts: Vec<WebPartition>,
    mut on_rows: impl FnMut(&WebPartition, Vec<MasterAddr>),
) -> Result<WebOutcome, String> {
    let mut queue: std::collections::VecDeque<WebPartition> = parts.into();
    sources::update(|s| s.steam.requests_total = queue.len() as u32);
    let mut out = WebOutcome::default();
    let mut running = FuturesUnordered::new();
    let mut answered_once = false;
    // While the master withholds answers: until when, how long the next
    // pause is, and how many there have been in a row.
    let mut paused_until: Option<tokio::time::Instant> = None;
    let mut backoff = WEB_THROTTLE_FIRST;
    let mut pauses = 0;
    loop {
        if CANCEL.load(Ordering::SeqCst) {
            out.cancelled = true;
            return Ok(out);
        }
        if let Some(until) = paused_until {
            if tokio::time::Instant::now() < until {
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
                let mut r = src.fetch(&part).await;
                // One lost request should not cost a partition.
                if r.as_ref().is_err_and(|e| !e.contains("API key")) {
                    r = src.fetch(&part).await;
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
            Ok(rows) if rows.is_empty() && src.throttled().await => {
                queue.push_front(part);
                // Its fellows in flight withheld too: one pause for all.
                if paused_until.is_some() {
                    continue;
                }
                pauses += 1;
                out.pauses += 1;
                if pauses > WEB_THROTTLE_GIVE_UP {
                    return Err("Steam kept withholding the server list".into());
                }
                let shown = sources::now() + backoff.as_secs() as i64;
                sources::update(|s| s.steam.waiting_until = Some(shown));
                paused_until = Some(tokio::time::Instant::now() + backoff);
                backoff = (backoff * 2).min(WEB_THROTTLE_MAX);
            }
            Ok(rows) => {
                answered_once = true;
                pauses = 0;
                backoff = WEB_THROTTLE_FIRST;
                out.requests += 1;
                sources::update(|s| s.steam.requests_done += 1);
                let more = if rows.len() >= dz_api::WEB_LIST_CAP {
                    part.split(rows.iter().map(|r| r.name.as_str()))
                } else {
                    Vec::new()
                };
                if more.is_empty() {
                    out.leaves.push(PlannedPart {
                        part: part.clone(),
                        rows: u32::try_from(rows.len()).unwrap_or(u32::MAX),
                    });
                } else {
                    sources::update(|s| s.steam.requests_total += more.len() as u32);
                    queue.extend(more);
                }
                on_rows(&part, rows);
            }
            Err(e) if !answered_once => return Err(e),
            Err(e) => {
                out.requests += 1;
                sources::update(|s| s.steam.requests_done += 1);
                eprintln!("steam servers: {}: {e}", part.label());
                let base = part.base();
                if !out.failed.contains(&base) {
                    out.failed.push(base);
                }
            }
        }
    }
    Ok(out)
}

// ── the servers themselves ──────────────────────────────────────────────

/// What asking a candidate gave.
enum Outcome {
    Row(SteamListed),
    /// No answer: asked again in the second pass.
    Silent(Candidate),
    /// A fake, or modded with mods that cannot be read, or silent on the
    /// last pass: why.
    Rejected(Addr, Verdict),
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
            Outcome::Rejected((c.ip, port), Verdict::Silent)
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
        return Outcome::Rejected((c.ip, port), Verdict::Fake);
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
            return Outcome::Rejected((c.ip, port), Verdict::NoMods);
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    /// One server of the made-up master.
    struct Listed {
        addr: MasterAddr,
        map: String,
        empty: bool,
    }

    /// Steam's master as the Web API shows it: filters as it reads them,
    /// the 10 000 cap, and its rationing (a burst, then one request every
    /// few seconds; withheld answers are empty).
    struct FakeMaster {
        servers: Vec<Listed>,
        /// HTTP requests made, the throttle checks included.
        http: AtomicU32,
        tokens: Mutex<(f64, tokio::time::Instant)>,
    }

    const BURST: f64 = 40.0;
    const REFILL_PER_SEC: f64 = 1.0 / 3.0;

    /// A small deterministic generator.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }
        fn pick<'a>(&mut self, of: &[&'a str]) -> &'a str {
            of[(self.next() as usize) % of.len()]
        }
    }

    impl FakeMaster {
        /// About 80 000 servers, as on 2026-10-08: thousands with players,
        /// farms crowding empty Chernarus under a few names.
        fn realistic() -> Self {
            let mut rng = Lcg(7);
            let mut servers = Vec::new();
            let words = [
                "Alpha",
                "Bear",
                "Cold",
                "Echo",
                "Frost",
                "Ghost",
                "Hard",
                "Iron",
                "Jolly",
                "Kill",
                "Lost",
                "Mad",
                "Night",
                "Old",
                "Pvp",
                "Quiet",
                "Raid",
                "Storm",
                "Toxic",
                "Ultra",
                "Vybor",
                "Wolf",
                "Zone",
                "Чернарусь",
                "Выживание",
                "末日",
                "🔥 Hot",
                "[EU]",
                "=FARM=",
                "1PP",
                "#1",
            ];
            let mut add = |n: usize, map: &str, empty: bool, names: &dyn Fn(&mut Lcg) -> String| {
                for _ in 0..n {
                    let i = servers.len() as u32;
                    servers.push(Listed {
                        addr: MasterAddr {
                            ip: format!("10.{}.{}.{}", i >> 16, (i >> 8) & 255, i & 255),
                            name: names(&mut rng),
                            query_port: 27016,
                            game_port: 2302,
                        },
                        map: map.to_owned(),
                        empty,
                    });
                }
            };
            let plain = |r: &mut Lcg| format!("{} {}", r.pick(&words), r.next() % 1000);
            let farm = |r: &mut Lcg| {
                let start = r.pick(&["DayZ", "Dark", "Deer", "Dead", "Dz", "Dawn"]);
                format!("{start} Farm #{}", r.next() % 100_000)
            };
            add(7_000, "chernarusplus", false, &plain);
            add(30_000, "chernarusplus", true, &farm);
            add(15_000, "chernarusplus", true, &plain);
            add(12_000, "enoch", true, &plain);
            add(3_000, "sakhal", true, &plain);
            add(2_000, "deerisle", true, &plain);
            add(1_500, "namalsk", true, &plain);
            add(900, "banov", true, &plain);
            add(600, "pripyat", true, &plain);
            add(500, "takistanplus", true, &plain);
            add(700, "elsewhere", true, &plain);
            // Steam's order is not the names': shuffled once.
            for i in (1..servers.len()).rev() {
                let j = (rng.next() as usize) % (i + 1);
                servers.swap(i, j);
            }
            FakeMaster {
                servers,
                http: AtomicU32::new(0),
                tokens: Mutex::new((BURST, tokio::time::Instant::now())),
            }
        }

        fn maps() -> Vec<String> {
            [
                "chernarusplus",
                "enoch",
                "sakhal",
                "deerisle",
                "namalsk",
                "banov",
                "pripyat",
                "takistanplus",
            ]
            .map(String::from)
            .to_vec()
        }

        /// Whether the master answers now; spends a token if so.
        fn allowed(&self, spend: bool) -> bool {
            let mut t = self.tokens.lock().unwrap();
            let now = tokio::time::Instant::now();
            t.0 = (t.0 + (now - t.1).as_secs_f64() * REFILL_PER_SEC).min(BURST);
            t.1 = now;
            if t.0 < 1.0 {
                return false;
            }
            if spend {
                t.0 -= 1.0;
            }
            true
        }

        fn starts(name: &str, prefix: &str) -> bool {
            let mut chars = name.chars();
            prefix
                .chars()
                .all(|p| chars.next().is_some_and(|c| c.to_ascii_lowercase() == p))
        }

        fn matches(s: &Listed, part: &WebPartition) -> bool {
            if part.empty.is_some_and(|e| e != s.empty) {
                return false;
            }
            let maps: Vec<&str> = part.extra.split("\\map\\").skip(1).collect();
            let on_map = maps.contains(&s.map.as_str());
            let map_ok = if part.extra.starts_with("\\nor\\") {
                !on_map
            } else {
                maps.is_empty() || on_map
            };
            map_ok
                && Self::starts(&s.addr.name, &part.prefix)
                && !part.excluded.iter().any(|e| Self::starts(&s.addr.name, e))
        }
    }

    impl WebSource for FakeMaster {
        fn fetch(
            &self,
            part: &WebPartition,
        ) -> impl Future<Output = Result<Vec<MasterAddr>, String>> + Send {
            self.http.fetch_add(1, Ordering::SeqCst);
            let rows = if self.allowed(true) {
                self.servers
                    .iter()
                    .filter(|s| Self::matches(s, part))
                    .take(dz_api::WEB_LIST_CAP)
                    .map(|s| s.addr.clone())
                    .collect()
            } else {
                Vec::new()
            };
            std::future::ready(Ok(rows))
        }

        fn throttled(&self) -> impl Future<Output = bool> + Send {
            self.http.fetch_add(1, Ordering::SeqCst);
            std::future::ready(!self.allowed(false))
        }
    }

    /// A listing over the made-up master: what it found, and its outcome.
    async fn listing(
        master: &FakeMaster,
        plan: &[PlannedPart],
    ) -> (FxHashSet<String>, WebOutcome, u32) {
        master.http.store(0, Ordering::SeqCst);
        let parts = dz_api::planned_partitions(dz_api::web_partitions(&FakeMaster::maps()), plan);
        let mut found = FxHashSet::default();
        let out = list_web(master, parts, |_, rows| {
            found.extend(rows.into_iter().map(|r| r.ip));
        })
        .await
        .unwrap();
        (found, out, master.http.load(Ordering::SeqCst))
    }

    #[tokio::test(start_paused = true)]
    async fn a_saved_plan_lists_the_same_servers_with_far_fewer_requests() {
        let master = FakeMaster::realistic();
        let t0 = tokio::time::Instant::now();
        let (cold, first, http_cold) = listing(&master, &[]).await;
        let took_cold = t0.elapsed();
        let plan = dz_api::compact_plan(first.leaves);

        // Refilled, as ten minutes later.
        tokio::time::advance(Duration::from_secs(600)).await;
        let t1 = tokio::time::Instant::now();
        let (warm, second, http_warm) = listing(&master, &plan).await;
        let took_warm = t1.elapsed();
        eprintln!(
            "cold: {} requests answered, {} HTTP, {} pauses, {:?}; \
             with the plan ({} parts): {} answered, {} HTTP, {} pauses, {:?}; {} servers",
            first.requests,
            http_cold,
            first.pauses,
            took_cold,
            plan.len(),
            second.requests,
            http_warm,
            second.pauses,
            took_warm,
            warm.len(),
        );
        assert_eq!(warm, cold, "the same servers either way");
        assert!(
            second.requests * 3 / 2 < first.requests,
            "a third fewer at least"
        );
        assert!(http_warm * 2 < http_cold, "half the HTTP requests at most");
        assert!(second.pauses <= first.pauses);
        assert!(second.failed.is_empty() && !second.cancelled);
        // What the master holds, but for the names two characters deep that
        // still pass the cap (the farms' "da"): missed alike either way.
        assert!(cold.len() > master.servers.len() * 9 / 10);
    }

    #[test]
    fn after_a_restart_the_last_listings_servers_are_its_own_again() {
        let now = sources::now();
        let server = |ip: &str| {
            Arc::new(Server {
                endpoint: dz_api::Endpoint {
                    ip: ip.into(),
                    port: 27016,
                },
                name: ip.into(),
                ..Default::default()
            })
        };
        let list = ServerList {
            status: 0,
            result: vec![
                server("192.0.2.1"),
                server("192.0.2.2"),
                server("192.0.2.3"),
            ],
        };
        let memo = |seen: i64, verdict| Memo {
            seen,
            verdict,
            at: seen,
        };
        let mut m = ListingMemory::default();
        m.started_at = Some(now - 600);
        m.completed_at = Some(now - 300);
        let addrs: Vec<(Addr, Memo)> = vec![
            // Added and listed by the last listing: Steam's.
            (
                ("192.0.2.1".into(), 27016),
                memo(now - 500, Verdict::Answered),
            ),
            // Added long before, not listed since: a kept server to check.
            (
                ("192.0.2.2".into(), 27016),
                memo(now - 7200, Verdict::Answered),
            ),
            (("192.0.2.3".into(), 27016), memo(now - 500, Verdict::Fake)),
        ];
        m.set_addresses(addrs.iter().map(|(a, m)| (a, m)), now);
        remember(Some(m), &list);
        let f = found();
        assert!(f.keys.contains(&("192.0.2.1".to_string(), 27016)));
        assert!(!f.keys.contains(&("192.0.2.2".to_string(), 27016)));
        assert!(!f.keys.contains(&("192.0.2.3".to_string(), 27016)));
        assert!(
            f.rows
                .iter()
                .any(|r| Arc::ptr_eq(r.server(), &list.result[0]))
        );
        assert!(f.memo[&("192.0.2.3".to_string(), 27016)].holds(now));
        assert_eq!(
            f.saved.next_listing(now),
            Some(now - 300 + memory::MIN_INTERVAL)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_saved_part_that_overflows_is_split_again() {
        let mut master = FakeMaster::realistic();
        let (_, first, _) = listing(&master, &[]).await;
        let plan = dz_api::compact_plan(first.leaves);
        // Sakhal grows past the cap: one request last time.
        let n = master.servers.len() as u32;
        for i in 0..9_000u32 {
            let k = n + i;
            master.servers.push(Listed {
                addr: MasterAddr {
                    ip: format!("11.{}.{}.{}", k >> 16, (k >> 8) & 255, k & 255),
                    name: format!("{} new", ["Ash", "Birch", "Cedar"][(i % 3) as usize]),
                    query_port: 27016,
                    game_port: 2302,
                },
                map: "sakhal".into(),
                empty: true,
            });
        }
        tokio::time::advance(Duration::from_secs(600)).await;
        let (found, again, _) = listing(&master, &plan).await;
        let sakhal = master
            .servers
            .iter()
            .filter(|s| s.map == "sakhal")
            .all(|s| found.contains(&s.addr.ip));
        assert!(sakhal, "every Sakhal server found once split");
        assert!(again.requests > plan.len() as u32);
    }
}
