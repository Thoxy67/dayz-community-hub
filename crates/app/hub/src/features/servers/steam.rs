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
//! skew a round trip measured here, so it is shown at once and measured
//! again). Farms are left out: many empty servers on one address, and any
//! server whose master row counts bots (DayZ has none: farm emulators pad
//! their count with them), before anything is asked of them; any server
//! reporting bots once it answers, for emulators that tell the master
//! none. Kept servers reporting bots leave too.
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
    FarmCounter, MasterAddr, Mod, PlannedPart, Server, ServerList, SocketAddrV4, SteamListed,
    SteamRow, WebError, WebPartition,
};
use futures_util::stream::FuturesUnordered;
use futures_util::{StreamExt, stream};
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::{Notify, mpsc};

use super::memory::{self, ListingMemory, Memo, Verdict};
use super::sources::{self, Phase};
use crate::features::browser::live::{self, Live};
use crate::state::SharedState;

/// How often the list is written to the cache while a listing adds to it,
/// so closing the launcher midway loses little.
const SAVE_EVERY: Duration = Duration::from_secs(30);
/// How long a server has to answer A2S_INFO. The queries go out at the
/// shared engine's pace (`crate::net::a2s`), which loses about as few
/// answers as the official launcher (0.4%), so most answer the first time.
/// One that does not is asked once more, with [`INFO_RETRY_TIMEOUT`], before
/// it is taken for silent: that verdict keeps it out of every listing for
/// an hour, too long for one lost datagram or a slow reply.
const INFO_TIMEOUT: Duration = Duration::from_millis(1500);
const INFO_RETRY_TIMEOUT: Duration = Duration::from_secs(3);
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
/// The wait before a request that failed on the way (no answer, a cut or
/// garbled one, a server error) is asked once more.
const WEB_RETRY_AFTER: Duration = Duration::from_secs(1);
/// Maps asked for one by one through the Web API: every map the list has
/// at least this many servers on, at most [`WEB_MAX_MAPS`].
const WEB_MIN_SERVERS: usize = 2;
const WEB_MAX_MAPS: usize = 60;
/// A listing is not started again sooner than this after the last started
/// in this process (whether it finished or not).
const MIN_INTERVAL: Duration = Duration::from_secs(memory::MIN_INTERVAL as u64);
/// Times a kept server is asked, and how long each time, before it is taken
/// for gone: patiently, since one wrongly taken leaves the list.
const KEPT_TRIES: usize = 2;
const KEPT_TIMEOUT: Duration = Duration::from_secs(3);
/// A kept server that answered a query (the ping scan's, an earlier check's)
/// this recently is not asked again: it still runs.
const KEPT_FRESH: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
struct Found {
    /// The launcher list, before Steam's servers are merged in.
    base: Arc<ServerList>,
    /// Every server Steam listed that answered, once each, with its
    /// [`Server`] made once.
    rows: Vec<SteamRow>,
    keys: FxHashSet<Addr>,
    farms: FarmCounter,
    /// Servers an earlier list had that the current one lacks, kept while
    /// they answer.
    kept: Vec<Arc<Server>>,
    /// Entries combed out as duplicates, by query address, and the
    /// addresses to join (ip, game port) already combed.
    dropped: FxHashSet<Addr>,
    combed: FxHashSet<SocketAddrV4>,
    /// When the last listing started.
    last: Option<Instant>,
    /// What listings learned of each address they queried.
    memo: FxHashMap<Addr, Memo>,
    /// The last complete listing (its addresses are in `memo`).
    saved: ListingMemory,
}

static FOUND: LazyLock<Mutex<Found>> = LazyLock::new(Mutex::default);
/// Whether a listing runs, and whether another is to run once it ends
/// (turned back on while a cancelled one winds down). One lock for both, so
/// a start never falls between a listing's end and its check for another.
static LISTING: Mutex<Listing> = Mutex::new(Listing {
    running: false,
    again: false,
});
static CANCEL: AtomicBool = AtomicBool::new(false);
/// Wakes a listing waiting out the master's pause when it is cancelled.
static CANCELLED: Notify = Notify::const_new();

struct Listing {
    running: bool,
    again: bool,
}

fn listing() -> std::sync::MutexGuard<'static, Listing> {
    LISTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn found() -> std::sync::MutexGuard<'static, Found> {
    FOUND
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

use memory::Addr;

fn key<S: std::borrow::Borrow<Server>>(s: &S) -> Addr {
    s.borrow().endpoint
}

/// An address the master listed, with what it counts for its players.
#[derive(Debug, Clone)]
struct Candidate {
    addr: Addr,
    game_port: u16,
    empty: Option<bool>,
    /// Bots the master's row counts.
    bots: u8,
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
        let mut known: FxHashSet<Addr> = base.result.iter().map(key).collect();
        known.extend(f.keys.iter().copied());
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
/// compared are addresses (integers): this runs every time more servers
/// answer.
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
        let mut have: FxHashSet<Addr> = list.result.iter().map(key).collect();
        let more: Vec<Arc<Server>> = f
            .kept
            .iter()
            .filter(|s| have.insert(s.endpoint))
            .cloned()
            .collect();
        list.result.extend(more);
    }
    if !f.dropped.is_empty() {
        list.result.retain(|s| !f.dropped.contains(&s.endpoint));
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
    let groups: Vec<(SocketAddrV4, Vec<Addr>)> = {
        let conflicts = dz_api::join_conflicts(&list.result);
        if conflicts.is_empty() {
            return;
        }
        let f = found();
        conflicts
            .into_iter()
            .filter(|(join, _)| !f.combed.contains(join))
            .collect()
    };
    if groups.is_empty() {
        return;
    }
    let engine = crate::net::a2s();
    let window = engine.window(INFO_TIMEOUT);
    let verdicts: Vec<(SocketAddrV4, Vec<Addr>)> = stream::iter(groups)
        .map(|(join, group)| {
            let engine = engine.clone();
            async move {
                let mut stating = None;
                let mut answering = None;
                for &addr in &group {
                    for _ in 0..KEPT_TRIES {
                        if let Ok(a) = engine.info(addr.into(), INFO_TIMEOUT).await {
                            let port = a.info.extended_server_info.port;
                            if stating.is_none() && port.is_none_or(|p| p == join.port()) {
                                stating = Some(addr);
                            }
                            answering.get_or_insert(addr);
                            break;
                        }
                    }
                }
                let losers = match stating.or(answering) {
                    Some(keep) => group.into_iter().filter(|k| *k != keep).collect(),
                    None => Vec::new(),
                };
                (join, losers)
            }
        })
        .buffer_unordered(window)
        .collect()
        .await;
    let mut removed = 0;
    {
        let mut f = found();
        let now = sources::now();
        for (join, losers) in verdicts {
            f.combed.insert(join);
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
/// answer (twice). The ones that answer stay, their pings recorded. One that
/// answered a query a moment ago (the ping scan pings the whole list, kept
/// servers included) is judged on that answer instead of being asked again.
pub(crate) fn check_kept(hub: Hub) {
    tokio::spawn(async move {
        let kept: Vec<Addr> = found().kept.iter().map(key).collect();
        if kept.is_empty() {
            return;
        }
        let Some(state) = hub.try_state() else {
            return;
        };
        let (answered, ask) = split_kept(kept, &live::store().read(), KEPT_FRESH);
        let mut gone = Vec::new();
        for (addr, fake) in answered {
            sources::update(|s| {
                s.kept.checked += 1;
                if fake {
                    s.kept.fakes += 1;
                } else {
                    s.kept.alive += 1;
                }
            });
            if fake {
                gone.push(addr);
            }
        }
        gone.extend(kept_pass(ask).await);
        if !gone.is_empty() {
            let gone: FxHashSet<Addr> = gone.into_iter().collect();
            found().kept.retain(|s| !gone.contains(&key(s)));
            let list = show(&hub, &state).await;
            super::save_cache_in_background(Arc::clone(&list));
            crate::features::browser::list_replaced(list);
        }
        sources::update(|s| s.kept.phase = Phase::Done);
    });
}

/// The kept servers that answered a query less than `within` ago, each with
/// whether it is a fake (it reports bots), and those to ask. A failed query
/// decides nothing: only the check's own, slower tries take a server for gone.
fn split_kept(
    kept: Vec<Addr>,
    live: &live::LiveMap,
    within: Duration,
) -> (Vec<(Addr, bool)>, Vec<Addr>) {
    let mut answered = Vec::new();
    let mut ask = Vec::new();
    for addr in kept {
        match live.fresh(addr, within) {
            Some(l) if l.ms.is_some() && !l.ping_failed() => {
                let fake = l.bots.is_some_and(|b| b > 0);
                answered.push((addr, fake));
            }
            _ => ask.push(addr),
        }
    }
    (answered, ask)
}

/// Ping `addrs`, each up to [`KEPT_TRIES`] times; returns those to drop: the
/// silent and the fakes.
async fn kept_pass(addrs: Vec<Addr>) -> Vec<Addr> {
    let engine = crate::net::a2s();
    let window = engine.window(KEPT_TIMEOUT);
    stream::iter(addrs)
        .map(|addr| {
            let engine = engine.clone();
            async move {
                let mut alive = false;
                let mut fake = false;
                for _ in 0..KEPT_TRIES {
                    if let Ok(a) = engine.info(addr.into(), KEPT_TIMEOUT).await {
                        let p = a.ping();
                        live::store().record(addr, Live::from_result(&Ok(p)));
                        alive = true;
                        fake = p.bots > 0;
                        break;
                    }
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
                (!alive || fake).then_some(addr)
            }
        })
        .buffer_unordered(window)
        .filter_map(std::future::ready)
        .collect()
        .await
}

// ── the listing ─────────────────────────────────────────────────────────

/// Stop the listing running, if any, and one waiting to run after it.
pub(crate) fn stop() {
    listing().again = false;
    CANCEL.store(true, Ordering::SeqCst);
    CANCELLED.notify_waiters();
    sources::update(|s| s.steam.phase = Phase::Off);
}

/// List Steam's servers in the background, unless a listing runs, started
/// in the last minutes, or the profile turned it off.
pub(crate) fn start(hub: Hub) {
    if found().last.is_some_and(|t| t.elapsed() < MIN_INTERVAL) {
        return;
    }
    launch(hub);
}

/// The listing turned back on: list now, whenever the last one started. A
/// complete listing minutes old (this run or before a restart) still stands
/// and is shown as done ([`run`]); one cancelled and still winding down is
/// followed by a new one.
pub(crate) fn resume(hub: Hub) {
    launch(hub);
}

fn launch(hub: Hub) {
    {
        let mut l = listing();
        if l.running {
            if CANCEL.load(Ordering::SeqCst) {
                l.again = true;
            }
            return;
        }
        l.running = true;
    }
    tokio::spawn(async move {
        loop {
            run(&hub).await;
            let mut l = listing();
            if !std::mem::take(&mut l.again) {
                l.running = false;
                break;
            }
        }
    });
}

async fn run(hub: &Hub) {
    let Some(state) = hub.try_state() else {
        return;
    };
    let (enabled, maps, known, ping_settings) = {
        let s = state.read().await;
        let known: FxHashSet<Addr> = s.servers.result.iter().map(key).collect();
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
            .map(|(a, _)| *a)
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
    let (tx, rx) = mpsc::unbounded_channel::<Candidate>();
    let lister = {
        let maps = maps.clone();
        tokio::spawn(async move { list(web_key, &maps, &plan, &tx).await })
    };

    let mut seen = known;
    let engine = crate::net::a2s();
    // A silent candidate waits out both tries.
    let window = engine.window(INFO_TIMEOUT + INFO_RETRY_TIMEOUT);
    let candidates = stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|c| (c, rx)) });
    let mut answers = Box::pin(
        candidates
            .filter_map(move |c| {
                let k = c.addr;
                let ask = if !seen.insert(k) {
                    sources::update(|s| s.steam.known += 1);
                    false
                } else if remembered.contains(&k) {
                    // Answered as a fake, without mods, as a duplicate or
                    // not at all, a short while ago.
                    sources::update(|s| s.steam.remembered += 1);
                    false
                } else if c.bots > 0 {
                    // Its own heartbeat counts bots: a farm's emulator,
                    // known as such without a query (`query` checks again
                    // for those that tell the master none).
                    sources::update(|s| s.steam.fakes += 1);
                    judged(k, Verdict::Fake);
                    false
                } else {
                    true
                };
                std::future::ready(ask.then_some(c))
            })
            .map(move |c| query(engine.clone(), c))
            .buffer_unordered(window)
            .fuse(),
    );
    take_answers(hub, &state, &mut answers, ping_settings).await;
    let outcome = lister.await.unwrap_or_else(|e| Err(e.to_string()));

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

/// Put the servers `answers` yields in the list as they come. How each
/// answered is remembered.
async fn take_answers(
    hub: &Hub,
    state: &SharedState,
    answers: &mut (impl Unpin + futures_util::stream::FusedStream<Item = Outcome>),
    ping: Option<PingSettings>,
) {
    let mut fresh: Vec<SteamListed> = Vec::new();
    let mut tick = tokio::time::interval(PUBLISH_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            next = answers.next() => match next {
                Some(Outcome::Row(row)) => {
                    judged(row.addr, Verdict::Answered);
                    fresh.push(row);
                }
                Some(Outcome::Rejected(addr, verdict)) => judged(addr, verdict),
                None => break,
            },
            _ = tick.tick() => publish(hub, state, std::mem::take(&mut fresh), ping).await,
        }
    }
    publish(hub, state, fresh, ping).await;
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
    let added: Vec<Addr> = {
        let mut f = found();
        let mut added = Vec::new();
        for r in rows {
            if f.keys.insert(r.addr) {
                added.push(r.addr);
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
                    addr: r.addr,
                    game_port: r.game_port,
                    empty: part.empty,
                    bots: r.bots,
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
            f.farms.add(c.addr, c.empty);
        }
        for c in rows {
            if c.empty != Some(false) && f.farms.is_farm(*c.addr.ip()) {
                farms += 1;
            } else {
                if let Some(m) = f.memo.get_mut(&c.addr) {
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
    ) -> impl Future<Output = Result<Vec<MasterAddr>, WebError>> + Send;
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
    ) -> impl Future<Output = Result<Vec<MasterAddr>, WebError>> + Send {
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

/// The master's pauses: until when the listing waits, how long the next
/// pause is, and how many there have been in a row.
struct Throttle {
    until: Option<tokio::time::Instant>,
    backoff: Duration,
    pauses: u32,
}

impl Throttle {
    fn new() -> Self {
        Self {
            until: None,
            backoff: WEB_THROTTLE_FIRST,
            pauses: 0,
        }
    }

    /// Pause for the next backoff, or for `at_least` if the master asked for
    /// longer (capped at the last). `false` once it has paused too often in
    /// a row. A pause under way covers its fellows in flight: no new one.
    fn hold(&mut self, at_least: Option<Duration>) -> bool {
        if self.until.is_some() {
            return true;
        }
        self.pauses += 1;
        if self.pauses > WEB_THROTTLE_GIVE_UP {
            return false;
        }
        let pause = at_least
            .map_or(self.backoff, |w| w.max(self.backoff))
            .min(WEB_THROTTLE_MAX);
        self.until = Some(tokio::time::Instant::now() + pause);
        self.backoff = (self.backoff * 2).min(WEB_THROTTLE_MAX);
        true
    }

    /// An answer came: the next pause starts over.
    fn answered(&mut self) {
        self.pauses = 0;
        self.backoff = WEB_THROTTLE_FIRST;
    }
}

/// Wait until `until`, or less if the listing is cancelled meanwhile.
async fn wait_unless_cancelled(until: tokio::time::Instant) {
    let cancelled = CANCELLED.notified();
    tokio::pin!(cancelled);
    // Registered before the flag is read, so a cancel in between still wakes it.
    cancelled.as_mut().enable();
    if CANCEL.load(Ordering::SeqCst) {
        return;
    }
    tokio::select! {
        () = tokio::time::sleep_until(until) => {}
        () = cancelled => {}
    }
}

/// Ask `parts` of the master, splitting what hits the cap; `on_rows` gets
/// each answer. `Err` when the very first request fails (a refused key,
/// Steam unreachable) or the master keeps withholding answers. A request
/// lost on the way is asked once more after a second; one the master
/// refuses to answer now (an empty answer while it rations, a 429) goes
/// back to the front of the queue and the listing pauses.
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
    let mut throttle = Throttle::new();
    loop {
        if CANCEL.load(Ordering::SeqCst) {
            out.cancelled = true;
            return Ok(out);
        }
        if let Some(until) = throttle.until {
            if tokio::time::Instant::now() < until {
                if running.is_empty() {
                    wait_unless_cancelled(until).await;
                    continue;
                }
            } else {
                throttle.until = None;
                sources::update(|s| s.steam.waiting_until = None);
            }
        }
        while throttle.until.is_none()
            && running.len() < WEB_CONCURRENCY
            && let Some(part) = queue.pop_front()
        {
            sources::update(|s| s.steam.current = Some(part.label()));
            running.push(async move {
                let mut r = src.fetch(&part).await;
                // One lost request should not cost a partition.
                if r.as_ref().is_err_and(WebError::transient) {
                    tokio::time::sleep(WEB_RETRY_AFTER).await;
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
        // Nothing: matched nothing, or withheld. Asking for one server with
        // players tells which. A 429 says so outright.
        let refused = match &result {
            Ok(rows) if rows.is_empty() && src.throttled().await => Some(None),
            Err(WebError::RateLimited(wait)) => Some(*wait),
            _ => None,
        };
        if let Some(at_least) = refused {
            queue.push_front(part);
            let fresh = throttle.until.is_none();
            if !throttle.hold(at_least) {
                return Err("Steam kept withholding the server list".into());
            }
            if fresh && let Some(until) = throttle.until {
                out.pauses += 1;
                let left = until.saturating_duration_since(tokio::time::Instant::now());
                let shown = sources::now() + left.as_secs() as i64;
                sources::update(|s| s.steam.waiting_until = Some(shown));
            }
            continue;
        }
        match result {
            Ok(rows) => {
                answered_once = true;
                throttle.answered();
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
            Err(e) if !answered_once => return Err(e.to_string()),
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
    /// Silent, a fake, or modded with mods that cannot be read: why.
    Rejected(Addr, Verdict),
}

/// A2S_INFO, and A2S_RULES for a modded server's mods with the challenge
/// A2S_INFO got, both through the shared `engine`. A server that does not
/// answer A2S_INFO twice, a fake, and a modded one whose mods cannot be read
/// (it could not be joined with them) are rejected.
async fn query(engine: dz_a2s::Engine, c: Candidate) -> Outcome {
    sources::update(|s| s.steam.queried += 1);
    let addr = c.addr.into();
    let answer = match engine.info(addr, INFO_TIMEOUT).await {
        Ok(answer) => Ok(answer),
        Err(_) => engine.info(addr, INFO_RETRY_TIMEOUT).await,
    };
    let Ok(answer) = answer else {
        return Outcome::Rejected(c.addr, Verdict::Silent);
    };
    sources::update(|s| s.steam.answered += 1);
    let info = &answer.info;
    // DayZ has no bots: a server reporting some is a farm's emulator
    // padding its count. Measured on 2026-10-08: of 388 dayzsalauncher.com
    // servers that answered, none reported a bot; of 388 only Steam
    // listed, 324 reported as many bots as players.
    if info.bots > 0 {
        sources::update(|s| s.steam.fakes += 1);
        return Outcome::Rejected(c.addr, Verdict::Fake);
    }
    // Its counts and its round trip now, shown at once; the ping scan
    // measures it again once it is in the list, away from this burst.
    let ping = answer.ping();
    live::store().record(c.addr, Live::from_result(&Ok(ping)));
    let ext = &info.extended_server_info;
    let tags = ext.keywords.clone().unwrap_or_default();
    let modded = tags.split(',').any(|t| t.trim() == "mod");
    let mut row = SteamListed {
        addr: c.addr,
        game_port: ext.port.unwrap_or(c.game_port),
        name: info.name.trim().to_owned(),
        map: info.map.clone(),
        tags,
        players: i64::from(ping.players),
        max_players: i64::from(info.max_players),
        password: info.visibility,
        secure: info.vac,
        version: info.version.clone(),
        environment: String::from(char::from(info.server_os as u8)),
        mods: Vec::new(),
        empty: c.empty,
    };
    if modded {
        // A lost datagram (a modded server's rules take up to a dozen) or a
        // challenge gone stale: asked once more, from a fresh challenge.
        let mut rules = engine.rules(addr, answer.challenge(), RULES_TIMEOUT).await;
        if rules.is_err() {
            rules = engine.rules(addr, None, RULES_TIMEOUT).await;
        }
        row.mods = rules
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
        if row.mods.is_empty() {
            sources::update(|s| s.steam.no_mods += 1);
            return Outcome::Rejected(row.addr, Verdict::NoMods);
        }
    }
    Outcome::Row(row)
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

    fn at(s: &str) -> Addr {
        s.parse().unwrap()
    }

    /// The `i`th made-up address of a block.
    fn nth(block: u8, i: u32) -> Addr {
        let [_, b, c, d] = i.to_be_bytes();
        SocketAddrV4::new(dz_api::Ipv4Addr::new(block, b, c, d), 27016)
    }

    #[test]
    fn kept_servers_that_answered_lately_are_not_asked_again() {
        let ping = |bots: u8| dz_a2s::Ping {
            ms: 40,
            players: 3,
            max_players: 60,
            bots,
        };
        let mut map = live::LiveMap::default();
        map.put(at("1.1.1.1:1"), Live::from_result(&Ok(ping(0))));
        map.put(at("2.2.2.2:2"), Live::from_result(&Ok(ping(5))));
        map.put(
            at("3.3.3.3:3"),
            Live::from_result(&Err(dz_common::Error::A2sQuery("timeout".into()))),
        );
        let kept: Vec<Addr> = ["1.1.1.1:1", "2.2.2.2:2", "3.3.3.3:3", "4.4.4.4:4"]
            .into_iter()
            .map(at)
            .collect();
        let (answered, ask) = split_kept(kept.clone(), &map, Duration::from_secs(60));
        assert_eq!(
            answered,
            [(at("1.1.1.1:1"), false), (at("2.2.2.2:2"), true)]
        );
        // A failure, and a server never asked, are asked by the check itself.
        assert_eq!(ask, [at("3.3.3.3:3"), at("4.4.4.4:4")]);
        // Answers older than the window decide nothing.
        let (answered, ask) = split_kept(kept, &map, Duration::ZERO);
        assert!(answered.is_empty());
        assert_eq!(ask.len(), 4);
    }

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
                            addr: nth(10, i),
                            name: names(&mut rng),
                            game_port: 2302,
                            bots: 0,
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
        ) -> impl Future<Output = Result<Vec<MasterAddr>, WebError>> + Send {
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
    ) -> (FxHashSet<Addr>, WebOutcome, u32) {
        master.http.store(0, Ordering::SeqCst);
        let parts = dz_api::planned_partitions(dz_api::web_partitions(&FakeMaster::maps()), plan);
        let mut found = FxHashSet::default();
        let out = list_web(master, parts, |_, rows| {
            found.extend(rows.into_iter().map(|r| r.addr));
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
                endpoint: at(&format!("{ip}:27016")),
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
            (at("192.0.2.1:27016"), memo(now - 500, Verdict::Answered)),
            // Added long before, not listed since: a kept server to check.
            (at("192.0.2.2:27016"), memo(now - 7200, Verdict::Answered)),
            (at("192.0.2.3:27016"), memo(now - 500, Verdict::Fake)),
        ];
        m.set_addresses(addrs.iter().map(|(a, m)| (a, m)), now);
        remember(Some(m), &list);
        let f = found();
        assert!(f.keys.contains(&at("192.0.2.1:27016")));
        assert!(!f.keys.contains(&at("192.0.2.2:27016")));
        assert!(!f.keys.contains(&at("192.0.2.3:27016")));
        assert!(
            f.rows
                .iter()
                .any(|r| Arc::ptr_eq(r.server(), &list.result[0]))
        );
        assert!(f.memo[&at("192.0.2.3:27016")].holds(now));
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
                    addr: nth(11, k),
                    name: format!("{} new", ["Ash", "Birch", "Cedar"][(i % 3) as usize]),
                    game_port: 2302,
                    bots: 0,
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
            .all(|s| found.contains(&s.addr.addr));
        assert!(sakhal, "every Sakhal server found once split");
        assert!(again.requests > plan.len() as u32);
    }

    /// A master that answers its first request with a 429 asking for
    /// `wait`, then lists one server per request; when each request came.
    struct SlowDown {
        wait: Option<Duration>,
        asked: Mutex<Vec<tokio::time::Instant>>,
    }

    impl WebSource for SlowDown {
        fn fetch(
            &self,
            _: &WebPartition,
        ) -> impl Future<Output = Result<Vec<MasterAddr>, WebError>> + Send {
            let mut asked = self.asked.lock().unwrap();
            asked.push(tokio::time::Instant::now());
            let r = if asked.len() == 1 {
                Err(WebError::RateLimited(self.wait))
            } else {
                Ok(vec![MasterAddr {
                    addr: nth(192, asked.len() as u32),
                    name: "x".into(),
                    game_port: 2302,
                    bots: 0,
                }])
            };
            std::future::ready(r)
        }

        fn throttled(&self) -> impl Future<Output = bool> + Send {
            std::future::ready(false)
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_429_pauses_the_listing_and_keeps_the_part() {
        for (wait, pause) in [
            (None, WEB_THROTTLE_FIRST),
            (Some(Duration::from_secs(40)), Duration::from_secs(40)),
        ] {
            let master = SlowDown {
                wait,
                asked: Mutex::new(Vec::new()),
            };
            let part = WebPartition::new(Some(true), "");
            let mut rows = 0;
            let out = list_web(&master, vec![part], |_, r| rows += r.len())
                .await
                .unwrap();
            let asked = master.asked.lock().unwrap();
            // Not asked again at once: after the pause, and then answered.
            assert_eq!(asked.len(), 2);
            assert!(asked[1] - asked[0] >= pause, "{:?}", asked[1] - asked[0]);
            assert_eq!((rows, out.pauses, out.requests), (1, 1, 1));
            assert!(out.failed.is_empty());
        }
    }

    #[test]
    fn pauses_double_up_to_the_last_and_start_over_after_an_answer() {
        let mut t = Throttle::new();
        assert!(t.hold(None));
        assert_eq!(t.backoff, WEB_THROTTLE_FIRST * 2);
        // A pause under way covers the requests in flight beside it.
        assert!(t.hold(None));
        assert_eq!(t.pauses, 1);
        for _ in 1..WEB_THROTTLE_GIVE_UP {
            t.until = None;
            assert!(t.hold(Some(Duration::from_secs(1))));
        }
        assert_eq!(t.backoff, WEB_THROTTLE_MAX);
        t.until = None;
        assert!(!t.hold(None), "gives up past the last");
        t.answered();
        t.until = None;
        assert!(t.hold(None));
        assert_eq!(t.backoff, WEB_THROTTLE_FIRST * 2);
    }
}
