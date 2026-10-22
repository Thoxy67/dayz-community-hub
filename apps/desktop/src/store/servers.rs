//! The server list, as the window sees it (= `stores/servers.svelte.ts`), read in
//! process.
//!
//! The hub publishes the list as an immutable [`ServerSnapshot`] (an `Arc`) whose rows
//! carry their live state (pings, live counts) and the profile's flags in atomics the
//! ping tasks write. This store holds the current snapshot and the browser's query; it
//! asks for the filtered, sorted order ([`ServerOrder`]: an `Arc<[u32]>` of indices and
//! the header's counts) only when the snapshot, the profile's flags or the query change
//! (typing debounced, toggles at once, the newest request winning), and again when live
//! data moved under a query that depends on it. The virtual list reads its visible rows
//! straight from the snapshot by index: scrolling costs nothing, a ping appears in its
//! row on the next frame, nothing is paged.
//!
//! The hub wakes the window (`ServersChanged`, at most ~30 times a second) when anything
//! moved; [`Servers::poll`] picks the new state up on that frame.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dz_hub::features::browser::{
    MapCount, ServerOrder, ServerQuery, ServerRef, ServerRow, ServerSnapshot, SortCol, Tri, order,
};
use dz_hub::features::ping::{PingResultDto, ScanProgress, ScanState};
use dz_hub::features::servers::AppStatsDto;
use dz_hub::{Hub, HubEvent};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{Bus, Signal, mirror, now_ms};
use crate::i18n::{t, tf};
use crate::rt::{self, Task};

/// A server's key everywhere in the app: "ip:query_port".
pub fn key_of(r: &ServerRow) -> String {
    format!("{}:{}", r.ip, r.query_port)
}

/// The freshest head-count of a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Live {
    pub players: i64,
    pub max: i64,
    pub bots: u32,
}

/// Data older than this earns a warning and a refresh button (ms).
pub const STALE_MS: i64 = 10 * 60 * 1000;
/// A ping at or above this is a timeout.
pub const TIMEOUT_MS: u32 = 9999;

/// Typing in the search waits this long after the last key (`rt::Debounce`'s 80 ms).
pub const QUERY_DEBOUNCE: Duration = Duration::from_millis(80);
/// Under a query that depends on live data (ping sort, hide full…), a scan landing
/// results re-orders the list at most this often: rows show new pings at once, but
/// do not jump under the pointer on every result.
const LIVE_REORDER: Duration = Duration::from_millis(500);
/// The title bar's figures that follow a new list, at most this often.
const COUNTS_DELAY: Duration = Duration::from_secs(2);
/// Owned rows kept for `find` (favourites, history, the rejoin card).
const ROWS_MAX: usize = 512;

/// An explicit ping list in flight: its targets (pending until they answer).
type PingList = (Vec<String>, Task<Result<(), String>>);

/// Everything shown, no sort: the browser's query before the view sets one.
pub fn default_query() -> ServerQuery {
    ServerQuery {
        search: String::new(),
        maps: Vec::new(),
        first_person: Tri::All,
        password: Tri::All,
        battleye: Tri::All,
        modded: Tri::All,
        official: Tri::All,
        hide_empty: false,
        hide_full: false,
        max_ping: 0,
        show_excluded: false,
        sort: SortCol::None,
        asc: true,
        offset: 0,
        limit: 0,
    }
}

/// "ip:port" into its parts.
fn split_key(k: &str) -> Option<(&str, i64)> {
    let (ip, port) = k.rsplit_once(':')?;
    Some((ip, port.parse().ok()?))
}

/// What an order was computed from: when one of these moves, it is asked again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Seen {
    list: u64,
    profile: u64,
    live: u64,
}

impl Seen {
    fn of(s: &ServerSnapshot) -> Self {
        Self {
            list: s.generation(),
            profile: s.profile_generation(),
            live: s.live_stamp(),
        }
    }
}

/// Where a refresh has got (mirror of the hub's `RefreshPhase`, which it does not export).
#[derive(serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefreshPhase {
    #[default]
    Idle,
    Fetching,
    Done,
    Failed,
}

/// The list's refresh as it stands, times in Unix ms (mirror of the hub's
/// `RefreshState`, read through serde: the hub does not export the type).
#[derive(serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct RefreshState {
    pub id: u32,
    pub running: bool,
    pub phase: RefreshPhase,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    /// When the list on hand was fetched from the network; `None` while it is the
    /// on-disk cache's.
    pub loaded_at: Option<i64>,
    pub count: u32,
    pub error: Option<String>,
}

pub struct Servers {
    hub: Hub,
    bus: Bus,
    /// How many servers the list holds (the snapshot's, deduplicated by the hub).
    pub total: u32,
    /// Why the hub has no list (API down and no cache), until a refresh works.
    pub list_error: Option<String>,
    /// Every map, busiest first, each named by its most common spelling.
    pub maps: Vec<MapCount>,
    pub loading: bool,
    pub refreshing: bool,
    /// When the list on hand was fetched, Unix ms (0 before).
    pub last_refreshed: i64,
    /// The title bar's totals.
    pub stats: Option<AppStatsDto>,
    pub steam_players: Option<u32>,
    /// Moves whenever anything the rows show changed (the list, a flag, a ping).
    pub generation: u64,
    /// The hub's last word on the list's refresh, for the status bar.
    pub last_refresh: Option<RefreshState>,
    /// The whole-list ping scan: (done, total) while it runs.
    pub scan: Option<(u32, u32)>,
    pub scan_paused: bool,

    /// The list as the hub published it, and what of it was last seen.
    snapshot: Arc<ServerSnapshot>,
    seen: Seen,

    /// The browser's query, its answer, and the answer on its way.
    query: ServerQuery,
    order: Option<ServerOrder>,
    order_task: Option<(u64, Task<Option<ServerOrder>>)>,
    order_req: u64,
    /// When the query's debounce ends.
    query_due: Option<Instant>,
    /// The last time the order was asked for because of live data.
    live_ordered: Option<Instant>,

    /// Owned rows handed out by `find`, by "ip:port"; let go when anything changes.
    rows: FxHashMap<String, Arc<ServerRow>>,
    /// Round trips of servers pinged on request (ms; ≥ 5000 is a timeout), for those the
    /// list lacks: a listed server's ping is in the snapshot.
    pub ping: FxHashMap<String, u32>,
    pub pending: FxHashSet<String>,
    pub timeouts: FxHashMap<String, u32>,
    pub a2s_failures: FxHashSet<String>,

    /// `load` was asked: take the snapshot in on the next poll.
    load_due: bool,
    /// The load was started by a refresh's end: (quiet, count) for its toast.
    load_after_refresh: Option<(bool, u32)>,
    first_load_done: bool,
    resume_task: Option<Task<(Option<ScanState>, RefreshState)>>,
    counts_due: Option<Instant>,
    refresh_task: Option<Task<Result<RefreshState, String>>>,
    refresh_id: u32,
    ended_id: u32,
    quiet: bool,
    stats_task: Option<Task<Result<AppStatsDto, String>>>,
    players_task: Option<Task<Result<u32, String>>>,
    scan_id: u32,
    scan_task: Option<Task<Result<ScanProgress, String>>>,
    pause_task: Option<Task<Result<bool, String>>>,
    ping_lists: Vec<PingList>,
    ping_one: Vec<(String, Task<Result<u32, String>>)>,
}

impl Servers {
    pub fn new(hub: Hub, bus: Bus) -> Self {
        let snapshot = hub.servers_snapshot();
        Self {
            hub,
            bus,
            total: 0,
            list_error: None,
            maps: Vec::new(),
            loading: false,
            refreshing: false,
            last_refreshed: 0,
            stats: None,
            steam_players: None,
            generation: 0,
            last_refresh: None,
            scan: None,
            scan_paused: false,
            seen: Seen::of(&snapshot),
            snapshot,
            query: default_query(),
            order: None,
            order_task: None,
            order_req: 0,
            query_due: None,
            live_ordered: None,
            rows: FxHashMap::default(),
            ping: FxHashMap::default(),
            pending: FxHashSet::default(),
            timeouts: FxHashMap::default(),
            a2s_failures: FxHashSet::default(),
            load_due: false,
            load_after_refresh: None,
            first_load_done: false,
            resume_task: None,
            counts_due: None,
            refresh_task: None,
            refresh_id: 0,
            ended_id: 0,
            quiet: true,
            stats_task: None,
            players_task: None,
            scan_id: 0,
            scan_task: None,
            pause_task: None,
            ping_lists: Vec::new(),
            ping_one: Vec::new(),
        }
    }

    // ── the list, in process ────────────────────────────────────────────────

    /// The list as it stands (rows by index, lookups by address).
    pub fn snapshot(&self) -> &Arc<ServerSnapshot> {
        &self.snapshot
    }

    /// A listed server with its freshest state, by address (query or game port),
    /// borrowed from the snapshot.
    pub fn row(&self, ip: &str, port: i64) -> Option<ServerRef<'_>> {
        self.snapshot.lookup(ip, port)
    }

    /// The browser's query as it stands (the last one set, maybe still debouncing).
    pub fn query(&self) -> &ServerQuery {
        &self.query
    }

    /// Set the browser's query. `debounce` for typing (the order follows
    /// [`QUERY_DEBOUNCE`] after the last change); toggles and sorts go at once. The
    /// last order stays shown until the new one is in.
    pub fn set_query(&mut self, q: ServerQuery, debounce: bool) {
        let q = ServerQuery {
            offset: 0,
            limit: 0,
            ..q
        };
        if q == self.query {
            return;
        }
        self.query = q;
        if debounce {
            self.query_due = Some(Instant::now() + QUERY_DEBOUNCE);
            self.bus.ctx().request_repaint_after(QUERY_DEBOUNCE);
        } else {
            self.query_due = None;
            self.ask_order();
        }
    }

    /// The browser's filtered, sorted list: indices into its snapshot, and the header's
    /// counts. `None` before the first one is in.
    pub fn order(&self) -> Option<&ServerOrder> {
        self.order.as_ref()
    }

    /// An order is on its way (a query or a list change).
    pub fn ordering(&self) -> bool {
        self.order_task.is_some() || self.query_due.is_some()
    }

    /// Ask for the order of the current query on the current snapshot, superseding one
    /// in flight. The very first is worked out here, so the list is there on the first
    /// frame; later ones off the frame, the old order shown meanwhile.
    fn ask_order(&mut self) {
        self.order_req += 1;
        let snap = Arc::clone(&self.snapshot);
        let q = self.query.clone();
        if !snap.is_empty() && self.order.as_ref().is_none_or(|o| o.snapshot.is_empty()) {
            self.order = Some(order(&snap, &q));
            self.order_task = None;
            self.generation += 1;
            return;
        }
        let req = self.order_req;
        let task = rt::spawn(async move {
            tokio::task::spawn_blocking(move || order(&snap, &q))
                .await
                .ok()
        });
        self.order_task = Some((req, task));
    }

    /// The round trip of a server, listed or pinged on request; `None` never pinged.
    pub fn ping_ms(&self, ip: &str, port: i64) -> Option<u32> {
        self.snapshot
            .lookup(ip, port)
            .and_then(|r| {
                if r.ping_failed() {
                    Some(TIMEOUT_MS)
                } else {
                    r.ping_ms()
                }
            })
            .or_else(|| self.ping.get(&format!("{ip}:{port}")).copied())
    }

    /// How many servers have answered a ping or timed out.
    pub fn pinged(&self) -> usize {
        let listed = (0..self.snapshot.len() as u32)
            .filter(|&i| self.snapshot.get(i).is_some_and(|r| r.ping_ms().is_some()))
            .count();
        listed
            + self
                .ping
                .keys()
                .filter(|k| {
                    split_key(k).is_none_or(|(ip, port)| self.snapshot.find(ip, port).is_none())
                })
                .count()
    }

    /// A server by address, whether `port` is its query or its game port, as an owned
    /// row (for the parts that keep one); `None` for servers the list lacks.
    pub fn find(&mut self, ip: &str, port: i64) -> Option<Arc<ServerRow>> {
        let key = format!("{ip}:{port}");
        if let Some(r) = self.rows.get(&key) {
            return Some(Arc::clone(r));
        }
        let row = Arc::new(self.snapshot.lookup(ip, port)?.to_row());
        if self.rows.len() >= ROWS_MAX {
            self.rows.clear();
        }
        self.rows.insert(key, Arc::clone(&row));
        Some(row)
    }

    /// A listed row, without keeping it.
    pub fn peek(&self, ip: &str, port: i64) -> Option<Arc<ServerRow>> {
        self.rows
            .get(&format!("{ip}:{port}"))
            .cloned()
            .or_else(|| self.snapshot.lookup(ip, port).map(|r| Arc::new(r.to_row())))
    }

    /// Kept for callers that hold rows from a lookup: a listed row is in the snapshot
    /// already (with its live state), so there is nothing to keep; owned rows handed
    /// out by `find` are made again.
    pub fn remember(&mut self, rows: impl IntoIterator<Item = ServerRow>) {
        for r in rows {
            self.rows.remove(&key_of(&r));
        }
    }

    /// The freshest head-count for a row.
    pub fn count(r: &ServerRow) -> Live {
        Live {
            players: r.players,
            max: r.max_players,
            bots: r.bots,
        }
    }

    /// A query answered by the server itself: the hub has written it into the
    /// snapshot; the owned row is made again on the next `find`.
    pub fn apply_live_count(&mut self, key: &str, _players: i64, _max: i64, _bots: u32) {
        self.rows.remove(key);
        self.generation += 1;
    }

    /// Take a new snapshot or new state of it in, and ask for what follows.
    fn follow_snapshot(&mut self) -> bool {
        let snap = self.hub.servers_snapshot();
        let seen = Seen::of(&snap);
        if seen == self.seen && Arc::ptr_eq(&snap, &self.snapshot) {
            return false;
        }
        let list_moved = seen.list != self.seen.list;
        let flags_moved = list_moved || seen.profile != self.seen.profile;
        self.snapshot = snap;
        self.seen = seen;
        self.rows.clear();
        self.generation += 1;
        if list_moved && self.first_load_done {
            self.take_list_in();
            if self.counts_due.is_none() {
                self.counts_due = Some(Instant::now() + COUNTS_DELAY);
                self.bus.ctx().request_repaint_after(COUNTS_DELAY);
            }
        }
        if flags_moved {
            self.ask_order();
        } else if self.query.depends_on_live() {
            self.order_for_live();
        }
        true
    }

    /// Live data moved under a query that depends on it: order again, at most every
    /// [`LIVE_REORDER`].
    fn order_for_live(&mut self) {
        let now = Instant::now();
        match self.live_ordered {
            Some(at) if now < at + LIVE_REORDER => {
                self.bus
                    .ctx()
                    .request_repaint_after(at + LIVE_REORDER - now);
            }
            _ => {
                self.live_ordered = Some(now);
                if self.order_task.is_none() {
                    self.ask_order();
                }
            }
        }
    }

    /// The total and the maps of the snapshot on hand.
    fn take_list_in(&mut self) {
        self.total = u32::try_from(self.snapshot.len()).unwrap_or(u32::MAX);
        self.maps = self.snapshot.maps().to_vec();
    }

    // ── loading ─────────────────────────────────────────────────────────────

    /// After start-up, or a refresh: the list's total and maps (taken on the next poll).
    pub fn load(&mut self) {
        self.loading = true;
        self.load_due = true;
        self.bus.ctx().request_repaint();
    }

    /// After the first `load`: show what the hub is doing, and start only what a fresh
    /// launch starts.
    pub fn resume(&mut self) {
        let hub = self.hub.clone();
        self.resume_task = Some(rt::spawn(async move {
            let (scan, refresh) = tokio::join!(hub.ping_scan_state(), hub.refresh_state());
            (scan.ok(), mirror(&refresh).unwrap_or_default())
        }));
    }

    /// Something the rows show changed here (the mods on disk): look again.
    pub fn touch(&mut self) {
        self.rows.clear();
        self.generation += 1;
        self.follow_snapshot();
    }

    /// Have the hub fetch a fresh list; `quiet` for the background refresh after a
    /// cached start.
    pub fn refresh(&mut self, quiet: bool) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        self.quiet = quiet;
        if !quiet {
            self.loading = true;
            self.bus.info(t("servers", "refreshing"));
        }
        // The hub stops the scan first.
        self.scan = None;
        self.scan_paused = false;
        let hub = self.hub.clone();
        self.refresh_task = Some(rt::spawn(async move {
            hub.servers_refresh()
                .await
                .map(|s| mirror(&s).unwrap_or_default())
        }));
    }

    /// Where the hub's refresh stands, from its answer or its event.
    fn follow(&mut self, st: RefreshState) {
        if st.id < self.refresh_id {
            return;
        }
        let ended = st.id <= self.ended_id;
        self.last_refresh = Some(st.clone());
        if ended {
            return;
        }
        self.refresh_id = st.id;
        if st.running {
            // Started elsewhere: shown, quietly.
            if !self.refreshing {
                self.refreshing = true;
                self.quiet = true;
                self.scan = None;
            }
            return;
        }
        self.ended_id = st.id;
        if self.refreshing {
            self.ended(st);
        }
    }

    fn ended(&mut self, st: RefreshState) {
        let quiet = self.quiet;
        if st.phase == RefreshPhase::Done {
            self.list_error = None;
            self.rows.clear();
            self.ping.clear();
            self.load();
            self.load_after_refresh = Some((quiet, st.count));
            self.load_stats();
            self.load_steam_players();
        } else {
            if !quiet {
                let e = st.error.unwrap_or_default();
                self.bus
                    .err(tf("servers", "refreshFailed", &[("error", &e)]));
            }
            self.loading = false;
            self.refreshing = false;
        }
    }

    pub fn load_stats(&mut self) {
        let hub = self.hub.clone();
        self.stats_task = Some(rt::spawn(async move { hub.get_app_stats().await }));
    }

    pub fn load_steam_players(&mut self) {
        let hub = self.hub.clone();
        self.players_task = Some(rt::spawn(
            async move { hub.fetch_steam_player_count().await },
        ));
    }

    /// Data older than [`STALE_MS`].
    pub fn stale(&self) -> bool {
        self.last_refreshed > 0 && now_ms() - self.last_refreshed > STALE_MS
    }

    // ── pings ───────────────────────────────────────────────────────────────

    /// The hub pings everything in its own order; only progress comes back.
    pub fn start_scan(&mut self) {
        let hub = self.hub.clone();
        self.scan_task = Some(rt::spawn(async move { hub.start_scan().await }));
    }

    fn show_scan(&mut self, p: ScanProgress) {
        if p.id < self.scan_id {
            return;
        }
        self.scan_id = p.id;
        self.scan_paused = p.paused;
        self.scan = p.running.then_some((p.done, p.total));
    }

    fn apply_pings(&mut self, results: &[PingResultDto]) {
        for r in results {
            let key = format!("{}:{}", r.ip, r.port);
            self.pending.remove(&key);
            self.ping.insert(key.clone(), r.ms);
            if r.failed {
                self.a2s_failures.insert(key.clone());
                *self.timeouts.entry(key).or_insert(0) += 1;
            } else {
                self.a2s_failures.remove(&key);
                self.timeouts.remove(&key);
                if let Some(p) = r.players {
                    self.apply_live_count(
                        &key,
                        i64::from(p),
                        i64::from(r.max_players.unwrap_or(0)),
                        u32::from(r.bots.unwrap_or(0)),
                    );
                }
            }
        }
    }

    /// Ping a few servers that have never answered (favourites, history).
    pub fn ping_visible(&mut self, keys: &[String], concurrency: u32, timeout_ms: u32) {
        let todo: Vec<String> = keys
            .iter()
            .filter(|k| {
                !self.pending.contains(*k)
                    && split_key(k).is_some_and(|(ip, port)| self.ping_ms(ip, port).is_none())
            })
            .cloned()
            .collect();
        if todo.is_empty() {
            return;
        }
        self.pending.extend(todo.iter().cloned());
        let hub = self.hub.clone();
        let targets = todo.clone();
        let task = rt::spawn(async move {
            hub.ping_servers(
                targets,
                Some(usize::try_from(concurrency).unwrap_or(64)),
                Some(u64::from(timeout_ms)),
            )
            .await
        });
        self.ping_lists.push((todo, task));
    }

    /// One server, asked by the player, with the longer manual timeout.
    pub fn ping_one(&mut self, ip: &str, port: i64, timeout_ms: u32) {
        let key = format!("{ip}:{port}");
        self.timeouts.remove(&key);
        self.pending.insert(key.clone());
        let hub = self.hub.clone();
        let ip = ip.to_string();
        let task =
            rt::spawn(async move { hub.ping_single(ip, port, Some(u64::from(timeout_ms))).await });
        self.ping_one.push((key, task));
    }

    pub fn cancel_scan(&mut self) {
        // Its last words, still in flight, must not bring it back.
        self.scan_id += 1;
        let hub = self.hub.clone();
        rt::fire(async move {
            let _ = hub.cancel_ping().await;
        });
        self.scan = None;
        self.scan_paused = false;
    }

    pub fn toggle_scan_pause(&mut self) {
        // Shown at once; the hub's answer settles it.
        self.scan_paused = !self.scan_paused;
        let hub = self.hub.clone();
        self.pause_task = Some(rt::spawn(async move { hub.toggle_ping_pause().await }));
    }

    // ── the frame ───────────────────────────────────────────────────────────

    pub fn on_event(&mut self, ev: &HubEvent) {
        match ev {
            // Picked up by `poll` on the frame this wakes.
            HubEvent::ServersChanged(_) => {}
            HubEvent::PingScanProgress(p) => self.show_scan(p.0),
            HubEvent::RefreshChanged(r) => {
                if let Some(st) = mirror(&r.0) {
                    self.follow(st);
                }
            }
            HubEvent::PingResults(r) => self.apply_pings(r),
            _ => {}
        }
    }

    pub fn poll(&mut self, signals: &mut Vec<Signal>) -> bool {
        let mut changed = false;
        let now = Instant::now();

        changed |= self.follow_snapshot();

        if self.load_due {
            self.load_due = false;
            changed = true;
            self.take_list_in();
            self.last_refreshed = now_ms();
            self.generation += 1;
            if !self.first_load_done {
                self.first_load_done = true;
                signals.push(Signal::ServersLoaded);
            }
            if self.order.is_none() {
                self.ask_order();
            }
            if let Some((false, count)) = self.load_after_refresh {
                self.bus.ok(tf("servers", "loaded", &[("count", &count)]));
            }
            if self.load_after_refresh.take().is_some() {
                self.loading = false;
                self.refreshing = false;
            } else {
                // A refresh in flight owns the flag until it is done.
                self.loading = self.refreshing && !self.quiet;
            }
        }

        if let Some(due) = self.query_due {
            if now >= due {
                self.query_due = None;
                self.ask_order();
            } else {
                self.bus.ctx().request_repaint_after(due - now);
            }
        }
        if let Some((req, t)) = &mut self.order_task
            && let Some(o) = t.poll()
        {
            let req = *req;
            self.order_task = None;
            // An older answer than the newest request is dropped.
            if req == self.order_req
                && let Some(o) = o
            {
                self.order = Some(o);
                self.generation += 1;
                changed = true;
            }
        }
        // Live data moved during a reorder wait: once it is over, order again.
        if self.query.depends_on_live()
            && self.order_task.is_none()
            && self
                .order
                .as_ref()
                .is_some_and(|o| o.live_stamp != self.snapshot.live_stamp())
        {
            self.order_for_live();
        }

        if let Some(t) = &mut self.resume_task
            && let Some((scan, refresh)) = t.poll()
        {
            self.resume_task = None;
            changed = true;
            if let Some(s) = &scan {
                self.show_scan(s.progress);
            }
            if let Some(at) = refresh.loaded_at {
                self.last_refreshed = at;
            }
            self.last_refresh = Some(refresh.clone());
            if refresh.running {
                self.follow(refresh.clone());
            }
            let scanned = scan.as_ref().is_some_and(|s| s.progress.id != 0);
            if !scanned && !refresh.running {
                self.start_scan();
            }
            // A list from the disk cache that no refresh has been asked for yet.
            if refresh.loaded_at.is_none() && refresh.started_at.is_none() {
                self.refresh(true);
            }
        }

        if let Some(due) = self.counts_due {
            if now >= due {
                self.counts_due = None;
                self.load_stats();
            } else {
                self.bus.ctx().request_repaint_after(due - now);
            }
        }

        if let Some(t) = &mut self.refresh_task
            && let Some(r) = t.poll()
        {
            self.refresh_task = None;
            changed = true;
            match r {
                Ok(st) => self.follow(st),
                Err(e) => {
                    if !self.quiet {
                        self.bus
                            .err(tf("servers", "refreshFailed", &[("error", &e)]));
                    }
                    self.loading = false;
                    self.refreshing = false;
                }
            }
        }

        if let Some(t) = &mut self.stats_task
            && let Some(r) = t.poll()
        {
            self.stats_task = None;
            if let Ok(s) = r {
                self.stats = Some(s);
                changed = true;
            }
        }
        if let Some(t) = &mut self.players_task
            && let Some(r) = t.poll()
        {
            self.players_task = None;
            if let Ok(n) = r {
                self.steam_players = Some(n);
                changed = true;
            }
        }
        if let Some(t) = &mut self.scan_task
            && let Some(r) = t.poll()
        {
            self.scan_task = None;
            changed = true;
            match r {
                Ok(p) => self.show_scan(p),
                Err(_) => self.scan = None,
            }
        }
        if let Some(t) = &mut self.pause_task
            && let Some(r) = t.poll()
        {
            self.pause_task = None;
            if let Ok(p) = r {
                self.scan_paused = p;
                changed = true;
            }
        }

        let mut failed_lists = Vec::new();
        self.ping_lists
            .retain_mut(|(keys, task)| match task.poll() {
                Some(Ok(())) => false,
                Some(Err(_)) => {
                    failed_lists.push(std::mem::take(keys));
                    false
                }
                None => task.is_pending(),
            });
        for keys in failed_lists {
            for k in keys {
                self.pending.remove(&k);
            }
            changed = true;
        }

        let mut answered = Vec::new();
        self.ping_one.retain_mut(|(key, task)| match task.poll() {
            Some(r) => {
                answered.push((std::mem::take(key), r));
                false
            }
            None => task.is_pending(),
        });
        for (key, r) in answered {
            changed = true;
            self.pending.remove(&key);
            match r {
                Ok(ms) => {
                    self.ping.insert(key.clone(), ms);
                    self.a2s_failures.remove(&key);
                }
                Err(_) => {
                    self.ping.insert(key.clone(), TIMEOUT_MS);
                    self.a2s_failures.insert(key);
                }
            }
        }
        changed
    }
}

/// One server by address, asked of the hub now (for the async connect flows).
pub async fn lookup_one(hub: &Hub, ip: &str, port: i64) -> Option<ServerRow> {
    hub.servers_lookup(vec![format!("{ip}:{port}")])
        .await
        .ok()
        .and_then(|mut v| v.pop().flatten())
}
