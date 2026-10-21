//! The server list, as the window sees it: never the whole list (=
//! `stores/servers.svelte.ts`).
//!
//! The hub holds the nine-thousand-odd servers, their pings and live counts, and
//! filters, sorts and counts them (`servers_query`, which the views call for the rows on
//! screen). This store keeps the totals, the maps, the refresh and scan state, and the
//! few servers other parts of the window mention (favourites, history, the rejoin card),
//! fetched by address in batches ([`Servers::find`]). When the hub says something
//! changed (`ServersChanged`, at most twice a second) those are asked for again, and
//! [`Servers::generation`] tells the views to re-query.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dz_hub::features::browser::{MapCount, ServerRow};
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

/// Rows kept by address; past this the least recently asked-for are let go.
const KNOWN_MAX: usize = 6000;
/// Addresses re-asked for when the hub's data changes: the ones shown lately.
const WATCHED_MAX: usize = 400;
/// Lookups asked for in the same moment go as one batch.
const LOOKUP_DELAY: Duration = Duration::from_millis(30);
/// The maps and figures that follow the list's total, at most this often.
const COUNTS_DELAY: Duration = Duration::from_secs(2);

type Lookup = (Vec<String>, Result<Vec<Option<ServerRow>>, String>);
/// An explicit ping list in flight: its targets (pending until they answer).
type PingList = (Vec<String>, Task<Result<(), String>>);

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
    /// How many servers the hub holds.
    pub total: u32,
    /// Why the hub has no list (API down and no cache), until a refresh works.
    pub list_error: Option<String>,
    pub maps: Vec<MapCount>,
    pub loading: bool,
    pub refreshing: bool,
    /// When the list on hand was fetched, Unix ms (0 before).
    pub last_refreshed: i64,
    /// The title bar's totals.
    pub stats: Option<AppStatsDto>,
    pub steam_players: Option<u32>,
    /// Bumped when the hub's data changed: views re-query on it.
    pub generation: u64,
    /// The hub's last word on the list's refresh, for the status bar.
    pub last_refresh: Option<RefreshState>,
    /// The whole-list ping scan: (done, total) while it runs.
    pub scan: Option<(u32, u32)>,
    pub scan_paused: bool,

    /// Rows seen, by "ip:query_port" and "ip:game_port", with when they were stored.
    known: FxHashMap<String, (Arc<ServerRow>, u64)>,
    stamp: u64,
    /// Round trip per server (ms; ≥ 5000 is a timeout).
    pub ping: FxHashMap<String, u32>,
    pub pending: FxHashSet<String>,
    pub timeouts: FxHashMap<String, u32>,
    pub a2s_failures: FxHashSet<String>,

    wanted: FxHashSet<String>,
    /// Addresses asked for with `find`, and when last.
    watched: FxHashMap<String, u64>,
    /// Addresses the hub said it does not list, until the next change.
    missing: FxHashSet<String>,
    lookup_due: Option<Instant>,
    lookup: Option<Task<Lookup>>,

    load_task: Option<Task<Result<Vec<MapCount>, String>>>,
    /// The load was started by a refresh's end: (quiet, count) for its toast.
    load_after_refresh: Option<(bool, u32)>,
    first_load_done: bool,
    resume_task: Option<Task<(Option<ScanState>, RefreshState)>>,
    counts_due: Option<Instant>,
    maps_task: Option<Task<Result<Vec<MapCount>, String>>>,
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
            known: FxHashMap::default(),
            stamp: 0,
            ping: FxHashMap::default(),
            pending: FxHashSet::default(),
            timeouts: FxHashMap::default(),
            a2s_failures: FxHashSet::default(),
            wanted: FxHashSet::default(),
            watched: FxHashMap::default(),
            missing: FxHashSet::default(),
            lookup_due: None,
            lookup: None,
            load_task: None,
            load_after_refresh: None,
            first_load_done: false,
            resume_task: None,
            counts_due: None,
            maps_task: None,
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

    // ── rows ────────────────────────────────────────────────────────────────

    /// Put rows from a query or a lookup where `find` and the pings see them.
    pub fn remember(&mut self, rows: impl IntoIterator<Item = ServerRow>) {
        if self.known.len() > KNOWN_MAX {
            self.prune();
        }
        for r in rows {
            let k = key_of(&r);
            if let Some(ms) = r.ping_ms {
                self.ping
                    .insert(k.clone(), if r.ping_failed { TIMEOUT_MS } else { ms });
            }
            if r.unverified_full {
                self.a2s_failures.insert(k.clone());
            } else {
                self.a2s_failures.remove(&k);
            }
            let game = format!("{}:{}", r.ip, r.game_port);
            self.stamp += 1;
            let row = Arc::new(r);
            self.known.insert(game, (Arc::clone(&row), self.stamp));
            self.known.insert(k, (row, self.stamp));
        }
    }

    /// Let go of the oldest rows nobody has asked for lately, with their pings.
    fn prune(&mut self) {
        let target = KNOWN_MAX - 1000;
        let mut by_age: Vec<(u64, String)> = self
            .known
            .iter()
            .filter(|(k, _)| !self.watched.contains_key(*k))
            .map(|(k, (_, s))| (*s, k.clone()))
            .collect();
        by_age.sort_unstable();
        let excess = self.known.len().saturating_sub(target);
        for (_, k) in by_age.into_iter().take(excess) {
            self.known.remove(&k);
            self.ping.remove(&k);
        }
    }

    fn watch(&mut self, key: &str) {
        self.stamp += 1;
        if let Some(s) = self.watched.get_mut(key) {
            *s = self.stamp;
            return;
        }
        self.watched.insert(key.to_string(), self.stamp);
        if self.watched.len() > WATCHED_MAX
            && let Some(oldest) = self
                .watched
                .iter()
                .min_by_key(|(_, s)| **s)
                .map(|(k, _)| k.clone())
        {
            self.watched.remove(&oldest);
        }
    }

    /// A server by address, whether `port` is its query or its game port. What is not
    /// known yet is fetched (batched with others asked for in the same moment) and
    /// appears when it arrives; `None` meanwhile, and for servers the list lacks.
    pub fn find(&mut self, ip: &str, port: i64) -> Option<Arc<ServerRow>> {
        let key = format!("{ip}:{port}");
        self.watch(&key);
        let row = self.known.get(&key).map(|(r, _)| Arc::clone(r));
        if row.is_none() && !self.missing.contains(&key) {
            self.want(key);
        }
        row
    }

    /// A known row, without asking for it.
    pub fn peek(&self, ip: &str, port: i64) -> Option<&ServerRow> {
        self.known.get(&format!("{ip}:{port}")).map(|(r, _)| &**r)
    }

    fn want(&mut self, key: String) {
        self.wanted.insert(key);
        if self.lookup_due.is_none() {
            self.lookup_due = Some(Instant::now() + LOOKUP_DELAY);
            self.bus.ctx().request_repaint_after(LOOKUP_DELAY);
        }
    }

    fn start_lookup(&mut self) {
        self.lookup_due = None;
        // Before the list is loaded nothing can be found: keep the wish, `load` asks again.
        if self.total == 0 || self.lookup.is_some() {
            return;
        }
        if self.wanted.is_empty() {
            return;
        }
        let keys: Vec<String> = self.wanted.drain().collect();
        let hub = self.hub.clone();
        self.lookup = Some(rt::spawn(async move {
            let r = hub.servers_lookup(keys.clone()).await;
            (keys, r)
        }));
    }

    /// The freshest head-count for a row.
    pub fn count(r: &ServerRow) -> Live {
        Live {
            players: r.players,
            max: r.max_players,
            bots: r.bots,
        }
    }

    /// A query answered by the server itself: shown at once, before the next change.
    pub fn apply_live_count(&mut self, key: &str, players: i64, max: i64, bots: u32) {
        if let Some((r, _)) = self.known.get(key) {
            let mut row = (**r).clone();
            row.players = players;
            row.max_players = max;
            row.bots = bots;
            self.remember([row]);
        }
    }

    // ── loading ─────────────────────────────────────────────────────────────

    /// After start-up: the maps and the total.
    pub fn load(&mut self) {
        self.loading = true;
        let hub = self.hub.clone();
        self.load_task = Some(rt::spawn(async move { hub.server_maps().await }));
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

    /// The list holds `total` servers now: show it, and fetch the maps and figures that
    /// depend on it again, at most every two seconds.
    fn follow_total(&mut self, total: u32) {
        if total == self.total || self.refreshing {
            return;
        }
        self.total = total;
        if self.counts_due.is_none() {
            self.counts_due = Some(Instant::now() + COUNTS_DELAY);
            self.bus.ctx().request_repaint_after(COUNTS_DELAY);
        }
    }

    /// Something the rows show changed here (the mods on disk): ask again.
    pub fn touch(&mut self) {
        self.changed(self.generation + 1);
    }

    fn changed(&mut self, generation: u64) {
        self.generation = (self.generation + 1).max(generation);
        self.reask();
    }

    /// Whatever other views show is asked for again, pings and counts included.
    fn reask(&mut self) {
        self.missing.clear();
        let keys: Vec<String> = self.watched.keys().cloned().collect();
        self.wanted.extend(keys);
        if !self.wanted.is_empty() && self.lookup_due.is_none() {
            self.lookup_due = Some(Instant::now() + LOOKUP_DELAY);
            self.bus.ctx().request_repaint_after(LOOKUP_DELAY);
        }
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
            self.known.clear();
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
            .filter(|k| !self.pending.contains(*k) && !self.ping.contains_key(*k))
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
            HubEvent::ServersChanged(c) => self.changed(c.generation),
            HubEvent::PingScanProgress(p) => self.show_scan(p.0),
            HubEvent::RefreshChanged(r) => {
                if let Some(st) = mirror(&r.0) {
                    self.follow(st);
                }
            }
            HubEvent::SourcesChanged(s) => {
                if self.first_load_done {
                    self.follow_total(s.total);
                }
            }
            HubEvent::PingResults(r) => self.apply_pings(r),
            _ => {}
        }
    }

    pub fn poll(&mut self, signals: &mut Vec<Signal>) -> bool {
        let mut changed = false;
        let now = Instant::now();

        if let Some(due) = self.lookup_due {
            if now >= due {
                self.start_lookup();
            } else {
                self.bus.ctx().request_repaint_after(due - now);
            }
        }
        if let Some(t) = &mut self.lookup
            && let Some((keys, r)) = t.poll()
        {
            self.lookup = None;
            if let Ok(rows) = r {
                let mut found = Vec::new();
                for (k, row) in keys.into_iter().zip(rows) {
                    match row {
                        Some(r) => found.push(r),
                        None => {
                            self.missing.insert(k);
                        }
                    }
                }
                self.remember(found);
                changed = true;
            }
            // Wishes made while it was in flight.
            if !self.wanted.is_empty() && self.lookup_due.is_none() {
                self.lookup_due = Some(Instant::now());
                self.bus.ctx().request_repaint();
            }
        }

        if let Some(t) = &mut self.load_task
            && let Some(r) = t.poll()
        {
            self.load_task = None;
            changed = true;
            match r {
                Ok(maps) => {
                    self.total = maps.iter().map(|m| m.count).sum();
                    self.maps = maps;
                    self.last_refreshed = now_ms();
                    self.generation += 1;
                    self.reask();
                    if !self.first_load_done {
                        self.first_load_done = true;
                        signals.push(Signal::ServersLoaded);
                    }
                    if let Some((false, count)) = self.load_after_refresh {
                        self.bus.ok(tf("servers", "loaded", &[("count", &count)]));
                    }
                }
                Err(e) => self.bus.err(tf("servers", "loadFailed", &[("error", &e)])),
            }
            if self.load_after_refresh.take().is_some() {
                self.loading = false;
                self.refreshing = false;
            } else {
                // A refresh in flight owns the flag until it is done.
                self.loading = self.refreshing && !self.quiet;
            }
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
                let hub = self.hub.clone();
                self.maps_task = Some(rt::spawn(async move { hub.server_maps().await }));
                self.load_stats();
            } else {
                self.bus.ctx().request_repaint_after(due - now);
            }
        }
        if let Some(t) = &mut self.maps_task
            && let Some(r) = t.poll()
        {
            self.maps_task = None;
            if let Ok(maps) = r {
                self.maps = maps;
                changed = true;
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
