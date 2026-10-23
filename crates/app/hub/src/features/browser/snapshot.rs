//! The server list as the window reads it, in process: an immutable snapshot
//! behind an `Arc`, and orders of indices into it.
//!
//! - [`ServerSnapshot`] is built once per list (`AppState::set_servers`): the
//!   servers, their index by address, and what the filters and sorts need
//!   worked out once (lowercased names, each map's one spelling, name and map
//!   ranks, clock minutes). It carries a table of per-server live state
//!   (ping, live counts) and profile state (favourite, excluded, mods missing
//!   or behind), each server one atomic word, written by the ping tasks and
//!   the profile's changes and read by the rows directly: a ping appears in
//!   its row on the next frame, with no query.
//! - [`order`] filters and sorts a snapshot into an [`ServerOrder`]: an
//!   `Arc<[u32]>` of indices and the header's counts. The window's virtual
//!   list reads the visible rows by index ([`ServerSnapshot::get`]); nothing is
//!   paged or serialized.
//!
//! Generations: [`ServerSnapshot::generation`] is the list's (a new snapshot,
//! a new number); [`ServerSnapshot::profile_generation`] moves when the
//! profile's flags were written again; [`ServerSnapshot::live_stamp`] moves
//! with every live write. An order stays right until one it depends on moves.

use dz_api::{Ipv4Addr, Server, ServerIndex, ServerList, SocketAddrV4};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::clock::{self, Clock};
use super::live::{Live, PING_FAILED_MS};
use super::query::{DayTime, ServerQuery, ServerRow, ServerStats, SortCol};
use super::{MapCount, count_maps};

/// What the profile says of servers, as the flags need it: its addresses
/// read into numbers once, so each server's flags are two or three integer
/// lookups. An entry that is no IPv4 address names no listed server.
#[derive(Default)]
pub(crate) struct ProfileView {
    pub excluded: FxHashSet<Ipv4Addr>,
    /// Each favourite's address (by query or game port).
    pub favorites: FxHashSet<SocketAddrV4>,
    /// Workshop id of each mod on disk → an update is waiting for it; `None`
    /// while the disk has not been read.
    pub mods: Option<FxHashMap<u64, bool>>,
}

impl ProfileView {
    fn is_favorite(&self, s: &Server) -> bool {
        self.favorites.contains(&s.endpoint)
            || s.join_addr().is_some_and(|j| self.favorites.contains(&j))
    }

    /// The packed profile word for one server.
    fn word(&self, s: &Server) -> u64 {
        let mut w = 0u64;
        if self.is_favorite(s) {
            w |= P_FAVORITE;
        }
        if self.excluded.contains(s.endpoint.ip()) {
            w |= P_EXCLUDED;
        }
        if let Some(have) = &self.mods {
            let mut missing = 0u64;
            let mut stale = 0u64;
            for m in &s.mods {
                match have.get(&(m.steam_workshop_id as u64)) {
                    None => missing += 1,
                    Some(true) => stale += 1,
                    Some(false) => {}
                }
            }
            w |= P_MODS_KNOWN | (missing.min(0xFFFF) << 16) | (stale.min(0xFFFF) << 32);
        }
        w
    }
}

// The live word: 0 is "nothing known".
const L_MS_MASK: u64 = 0xFFFF_FFFF;
const L_NO_MS: u64 = 0xFFFF_FFFF;
const L_PLAYERS_SHIFT: u32 = 32;
const L_MAX_SHIFT: u32 = 40;
const L_BOTS_SHIFT: u32 = 48;
const L_HAS_PLAYERS: u64 = 1 << 56;
const L_FAILED: u64 = 1 << 57;
const L_PRESENT: u64 = 1 << 58;
const L_HAS_MAX: u64 = 1 << 59;
const L_HAS_BOTS: u64 = 1 << 60;

// The profile word.
const P_FAVORITE: u64 = 1;
const P_EXCLUDED: u64 = 1 << 1;
const P_MODS_KNOWN: u64 = 1 << 2;

pub(crate) fn pack_live(l: &Live) -> u64 {
    let mut w = L_PRESENT | l.ms.map_or(L_NO_MS, |ms| u64::from(ms.min(u32::MAX - 1)));
    if let Some(p) = l.players {
        w |= L_HAS_PLAYERS | (u64::from(p) << L_PLAYERS_SHIFT);
    }
    if let Some(m) = l.max_players {
        w |= L_HAS_MAX | (u64::from(m) << L_MAX_SHIFT);
    }
    if let Some(b) = l.bots {
        w |= L_HAS_BOTS | (u64::from(b) << L_BOTS_SHIFT);
    }
    if l.failed {
        w |= L_FAILED;
    }
    w
}

fn unpack_live(w: u64) -> Option<Live> {
    if w & L_PRESENT == 0 {
        return None;
    }
    let byte = |shift: u32| ((w >> shift) & 0xFF) as u8;
    Some(Live {
        ms: (w & L_MS_MASK != L_NO_MS).then_some((w & L_MS_MASK) as u32),
        players: (w & L_HAS_PLAYERS != 0).then(|| byte(L_PLAYERS_SHIFT)),
        max_players: (w & L_HAS_MAX != 0).then(|| byte(L_MAX_SHIFT)),
        bots: (w & L_HAS_BOTS != 0).then(|| byte(L_BOTS_SHIFT)),
        failed: w & L_FAILED != 0,
    })
}

/// No map, no clock: the sentinel in the per-server tables.
const NONE_U32: u32 = u32::MAX;
const NONE_U16: u16 = u16::MAX;

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// One server list, with everything the browser needs worked out once.
pub struct ServerSnapshot {
    generation: u64,
    list: Arc<ServerList>,
    index: ServerIndex,
    /// Each map's spelling (its most common), busiest first.
    maps: Vec<MapCount>,
    /// Per map in `maps`: its name lowercased, for the map filter.
    map_lower: Box<[Box<str>]>,
    /// Per server: its map in `maps`, or `NONE_U32`.
    map_of: Box<[u32]>,
    /// Per map in `maps`: its rank by lowercased name.
    map_rank: Box<[u32]>,
    /// Per server: its name lowercased, for the search.
    name_lower: Box<[Box<str>]>,
    /// Per server: its rank by lowercased name (equal names, equal ranks).
    name_rank: Box<[u32]>,
    /// Per server: in-game time as minutes, or `NONE_U16`.
    time_min: Box<[u16]>,
    /// Per server: a community server named like an official one.
    mimics: Box<[bool]>,
    /// Every distinct mod of the list (by Workshop id): its id and its name.
    mod_ids: Box<[i64]>,
    mod_names: Box<[Arc<str>]>,
    /// Per server, its mods as places in `mod_ids`: server `i`'s are
    /// `server_mods[mods_at[i]..mods_at[i + 1]]`.
    server_mods: Box<[u32]>,
    mods_at: Box<[u32]>,
    /// When the list's clocks were read (Unix seconds).
    list_seen: AtomicU64,
    /// Per server: its clock as an A2S answer last gave it (packed
    /// [`Clock`], 0 for none), fresher than the list's.
    clock: Box<[AtomicU64]>,
    live: Box<[AtomicU64]>,
    profile: Box<[AtomicU64]>,
    profile_gen: AtomicU64,
    live_stamp: AtomicU64,
}

impl std::fmt::Debug for ServerSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerSnapshot")
            .field("generation", &self.generation)
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

/// When the list on hand had its clocks read: when the launcher list was
/// fetched, else now (tests, the mock world, a skewed stamp).
fn list_seen_at() -> u64 {
    let now = dz_common::time::now_secs();
    #[cfg(not(test))]
    {
        let at = crate::features::servers::launcher().fetched_at;
        if at > 0 && at <= now {
            return at;
        }
    }
    now
}

/// Every distinct mod of `servers` once, by Workshop id (the first name met
/// names it), and each server's mods as places in that table.
type ModTable = (Box<[i64]>, Box<[Arc<str>]>, Box<[u32]>, Box<[u32]>);

fn mod_table(servers: &[Arc<Server>]) -> ModTable {
    let mut by_id: FxHashMap<i64, u32> = FxHashMap::default();
    let (mut ids, mut names) = (Vec::new(), Vec::new());
    let mut flat = Vec::with_capacity(servers.iter().map(|s| s.mods.len()).sum());
    let mut at = Vec::with_capacity(servers.len() + 1);
    at.push(0u32);
    for s in servers {
        for m in &s.mods {
            let k = *by_id.entry(m.steam_workshop_id).or_insert_with(|| {
                ids.push(m.steam_workshop_id);
                names.push(Arc::clone(&m.name));
                (ids.len() - 1) as u32
            });
            flat.push(k);
        }
        at.push(u32::try_from(flat.len()).unwrap_or(u32::MAX));
    }
    (
        ids.into_boxed_slice(),
        names.into_boxed_slice(),
        flat.into_boxed_slice(),
        at.into_boxed_slice(),
    )
}

fn time_minutes(t: &str) -> Option<u16> {
    let (h, m) = t.split_once(':')?;
    let total = h.trim().parse::<u32>().ok()? * 60 + m.trim().parse::<u32>().ok()?;
    u16::try_from(total).ok().filter(|&v| v != NONE_U16)
}

impl ServerSnapshot {
    /// The empty list, before anything is loaded.
    pub(crate) fn empty() -> Self {
        Self::build(Arc::default(), &ProfileView::default())
    }

    /// Work out everything once for `list`, with the profile's flags.
    pub(crate) fn build(list: Arc<ServerList>, profile: &ProfileView) -> Self {
        let servers = &list.result;
        let n = servers.len();
        let index = ServerIndex::build(servers);

        // Maps, one per spelling-insensitive name, busiest first.
        let (counted, first_of) = count_maps(servers.iter().map(|s| s.map.as_str()));
        let mut counted: Vec<(u32, MapCount, Box<str>)> = counted
            .into_iter()
            .enumerate()
            .map(|(k, (map, count))| {
                let lower = map.to_lowercase().into_boxed_str();
                (k as u32, MapCount { map, count }, lower)
            })
            .collect();
        counted.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.2.cmp(&b.2)));
        // Where each map, in the order first met, went in the sorted list.
        let mut moved = vec![NONE_U32; counted.len()];
        for (at, (k, _, _)) in counted.iter().enumerate() {
            if let Some(slot) = moved.get_mut(*k as usize) {
                *slot = at as u32;
            }
        }
        let map_of: Box<[u32]> = first_of
            .iter()
            .map(|&k| moved.get(k as usize).copied().unwrap_or(NONE_U32))
            .collect();
        let (maps, map_lower): (Vec<MapCount>, Vec<Box<str>>) =
            counted.into_iter().map(|(_, m, lower)| (m, lower)).unzip();
        let map_lower = map_lower.into_boxed_slice();
        let mut by_name: Vec<u32> = (0..maps.len() as u32).collect();
        by_name.sort_by_key(|&i| map_lower.get(i as usize));
        let mut map_rank = vec![0u32; maps.len()].into_boxed_slice();
        for (rank, &i) in by_name.iter().enumerate() {
            if let Some(slot) = map_rank.get_mut(i as usize) {
                *slot = rank as u32;
            }
        }

        let name_lower: Box<[Box<str>]> = servers
            .iter()
            .map(|s| s.name.to_lowercase().into_boxed_str())
            .collect();
        let mut by_name: Vec<u32> = (0..n as u32).collect();
        by_name.sort_by(|&a, &b| {
            let (a, b) = (
                name_lower.get(a as usize).map(|s| &**s),
                name_lower.get(b as usize).map(|s| &**s),
            );
            a.cmp(&b)
        });
        // Dense ranks: equal names rank equal, so a stable sort keeps their order.
        let mut name_rank = vec![0u32; n].into_boxed_slice();
        let mut rank = 0u32;
        let mut prev: Option<&str> = None;
        for &i in &by_name {
            let name = name_lower.get(i as usize).map(|s| &**s);
            if prev.is_some() && name != prev {
                rank += 1;
            }
            prev = name;
            if let Some(slot) = name_rank.get_mut(i as usize) {
                *slot = rank;
            }
        }

        let time_min = servers
            .iter()
            .map(|s| time_minutes(&s.time).unwrap_or(NONE_U16))
            .collect();
        let mimics = servers
            .iter()
            .map(|s| !s.official && dz_api::name_looks_official(&s.name))
            .collect();
        let (mod_ids, mod_names, server_mods, mods_at) = mod_table(servers);
        let live = (0..n).map(|_| AtomicU64::new(0)).collect();
        let profile_words = servers
            .iter()
            .map(|s| AtomicU64::new(profile.word(s)))
            .collect();

        Self {
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
            list,
            index,
            maps,
            map_lower,
            map_of,
            map_rank,
            name_lower,
            name_rank,
            time_min,
            mimics,
            mod_ids,
            mod_names,
            server_mods,
            mods_at,
            list_seen: AtomicU64::new(list_seen_at()),
            clock: clock::table(n),
            live,
            profile: profile_words,
            profile_gen: AtomicU64::new(0),
            live_stamp: AtomicU64::new(0),
        }
    }

    /// The list's generation: every snapshot has its own.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Moves when the profile's flags (favourites, exclusions, mods) changed.
    pub fn profile_generation(&self) -> u64 {
        self.profile_gen.load(Ordering::Acquire)
    }

    /// Moves with every live write (a ping, a live count).
    pub fn live_stamp(&self) -> u64 {
        self.live_stamp.load(Ordering::Acquire)
    }

    pub fn len(&self) -> usize {
        self.list.result.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.result.is_empty()
    }

    /// The list itself.
    pub fn list(&self) -> &Arc<ServerList> {
        &self.list
    }

    /// Every map, busiest first, each named by its most common spelling.
    pub fn maps(&self) -> &[MapCount] {
        &self.maps
    }

    /// Every distinct mod of the list: its Workshop id and its name.
    pub fn mods(&self) -> impl Iterator<Item = (i64, &Arc<str>)> {
        self.mod_ids.iter().copied().zip(self.mod_names.iter())
    }

    /// Server `i`'s mods as places in the mod table.
    fn mods_of(&self, i: usize) -> &[u32] {
        let (Some(&a), Some(&b)) = (self.mods_at.get(i), self.mods_at.get(i + 1)) else {
            return &[];
        };
        self.server_mods
            .get(a as usize..b as usize)
            .unwrap_or_default()
    }

    /// Server `i`'s clock as last seen: whichever of an A2S answer's and the
    /// list's was seen later (a server restarted since an old answer starts its
    /// day again), the rates only an A2S answer gives kept either way.
    fn clock_of(&self, i: usize) -> Option<Clock> {
        let a2s = clock::load(&self.clock, i);
        let list = self
            .time_min
            .get(i)
            .copied()
            .filter(|&m| m != NONE_U16)
            .map(|minute| Clock {
                minute: minute.min(1439),
                day_accel: None,
                night_mult: None,
                seen: self.list_seen.load(Ordering::Relaxed),
            });
        match (a2s, list) {
            (Some(a), Some(l)) if l.seen > a.seen => Some(Clock {
                day_accel: a.day_accel,
                night_mult: a.night_mult,
                ..l
            }),
            (Some(a), _) => Some(a),
            (None, l) => l,
        }
    }

    /// Server `i`'s in-game clock at `now` (Unix seconds), as minutes of the
    /// day, estimated from the last one seen (see [`super::clock`]); `None`
    /// when no clock was ever seen.
    pub fn clock_minute(&self, i: u32, now: u64) -> Option<u32> {
        self.clock_of(i as usize).map(|c| c.minute_at(now))
    }

    /// Write a clock an A2S answer gave into the server whose query address
    /// is `addr`, if listed.
    pub(crate) fn put_clock(&self, addr: SocketAddrV4, c: &Clock) {
        if let Some(i) = self.index.by_query_port(addr)
            && let Some(slot) = self.clock.get(i)
        {
            slot.store(c.pack(), Ordering::Relaxed);
            self.live_stamp.fetch_add(1, Ordering::Release);
        }
    }

    /// When the list's clocks were read (tests set it).
    #[cfg(test)]
    pub(crate) fn set_list_seen(&self, secs: u64) {
        self.list_seen.store(secs, Ordering::Relaxed);
    }

    /// Server `i` with its live and profile state.
    pub fn get(&self, i: u32) -> Option<ServerRef<'_>> {
        let i_us = i as usize;
        let server = self.list.result.get(i_us)?;
        let map = self
            .map_of
            .get(i_us)
            .and_then(|&m| self.maps.get(m as usize))
            .map_or_else(|| server.map.trim(), |m| m.map.as_str());
        Some(ServerRef {
            index: i,
            server,
            map,
            live: self.live.get(i_us).map_or(0, |a| a.load(Ordering::Relaxed)),
            profile: self
                .profile
                .get(i_us)
                .map_or(0, |a| a.load(Ordering::Relaxed)),
            mimics_official: self.mimics.get(i_us).copied().unwrap_or(false),
            clock: self.clock_of(i_us),
        })
    }

    /// The index of the server at `addr`, by query port first, then game port.
    pub fn find(&self, addr: SocketAddrV4) -> Option<u32> {
        self.index.flexible(addr).map(|i| i as u32)
    }

    /// The index of the server whose query address is `addr`.
    pub fn find_by_query_port(&self, addr: SocketAddrV4) -> Option<u32> {
        self.index.by_query_port(addr).map(|i| i as u32)
    }

    /// The server at `addr` (query or game port) with its state.
    pub fn lookup(&self, addr: SocketAddrV4) -> Option<ServerRef<'_>> {
        self.get(self.find(addr)?)
    }

    /// Write a live result into the server whose query address is `addr`,
    /// if listed.
    pub(crate) fn put_live(&self, addr: SocketAddrV4, live: &Live) {
        if let Some(i) = self.index.by_query_port(addr)
            && let Some(slot) = self.live.get(i)
        {
            slot.store(pack_live(live), Ordering::Relaxed);
            self.live_stamp.fetch_add(1, Ordering::Release);
        }
    }

    /// Fill the live table from what is known by address.
    pub(crate) fn fill_live(&self, live: &super::live::LiveMap) {
        for ((s, slot), clock) in self
            .list
            .result
            .iter()
            .zip(self.live.iter())
            .zip(self.clock.iter())
        {
            let w = live.get(s.endpoint).map_or(0, pack_live);
            slot.store(w, Ordering::Relaxed);
            clock.store(
                live.clock(s.endpoint).map_or(0, |c| c.pack()),
                Ordering::Relaxed,
            );
        }
        self.live_stamp.fetch_add(1, Ordering::Release);
    }

    /// Write the profile's flags again (favourites, exclusions, mods on disk).
    /// The profile generation moves only when a server's flags did change, so
    /// writing the same flags again does not make the window order the list
    /// again. Returns whether any changed.
    pub(crate) fn apply_profile(&self, profile: &ProfileView) -> bool {
        let mut changed = false;
        for (s, slot) in self.list.result.iter().zip(self.profile.iter()) {
            let w = profile.word(s);
            changed |= slot.swap(w, Ordering::Relaxed) != w;
        }
        if changed {
            self.profile_gen.fetch_add(1, Ordering::Release);
        }
        changed
    }
}

/// A server of a snapshot with its freshest known state, borrowed.
#[derive(Clone, Copy, Debug)]
pub struct ServerRef<'a> {
    /// Its index in the snapshot.
    pub index: u32,
    pub server: &'a Server,
    /// The map's one spelling (maps differing only in case are one map).
    pub map: &'a str,
    live: u64,
    profile: u64,
    pub mimics_official: bool,
    clock: Option<Clock>,
}

impl<'a> ServerRef<'a> {
    pub fn ip(&self) -> Ipv4Addr {
        self.server.ip()
    }
    /// Its query address.
    pub fn addr(&self) -> SocketAddrV4 {
        self.server.endpoint
    }
    pub fn query_port(&self) -> u16 {
        self.server.query_port()
    }
    /// 0 when unknown.
    pub fn game_port(&self) -> u16 {
        self.server.game_port
    }
    pub fn name(&self) -> &'a str {
        &self.server.name
    }

    /// Its live and profile words as they were read: equal bits, equal live
    /// counts, ping and flags, so a cache keyed on them (one server's panel)
    /// is made again only when this server changed.
    pub fn state_bits(&self) -> (u64, u64) {
        (self.live, self.profile)
    }

    /// The last query of this server, if any.
    pub(crate) fn live(&self) -> Option<Live> {
        unpack_live(self.live)
    }

    fn ok_live(&self) -> Option<Live> {
        self.live().filter(|l| !l.failed)
    }

    /// Freshest known: the last successful query, else the list.
    pub fn players(&self) -> i64 {
        self.ok_live()
            .and_then(|l| l.players)
            .map_or(self.server.players, i64::from)
    }
    pub fn max_players(&self) -> i64 {
        self.ok_live()
            .and_then(|l| l.max_players)
            .map_or(self.server.max_players, i64::from)
    }
    pub fn bots(&self) -> u32 {
        self.ok_live().and_then(|l| l.bots).map_or(0, u32::from)
    }
    /// `None` when never pinged; `>= 5000` when it timed out.
    pub fn ping_ms(&self) -> Option<u32> {
        self.live().and_then(|l| l.ms)
    }
    pub fn ping_failed(&self) -> bool {
        self.live().is_some_and(|l| l.ping_failed())
    }
    /// `None` never pinged, `Some(None)` failed, `Some(Some(ms))` answered.
    pub fn ping(&self) -> Option<Option<u32>> {
        let l = self.live()?;
        if l.failed || l.ms.is_some_and(|ms| ms >= PING_FAILED_MS) {
            return Some(None);
        }
        l.ms.map(Some)
    }
    pub fn is_full(&self) -> bool {
        let max = self.max_players();
        max > 0 && self.players() >= max
    }
    /// Full according to the list, but the last query failed.
    pub fn unverified_full(&self) -> bool {
        let s = self.server;
        s.max_players > 0 && s.players >= s.max_players && self.live().is_some_and(|l| l.failed)
    }
    pub fn favorite(&self) -> bool {
        self.profile & P_FAVORITE != 0
    }
    pub fn excluded(&self) -> bool {
        self.profile & P_EXCLUDED != 0
    }
    /// Of its mods, how many are not on disk; `None` while the disk is not read.
    pub fn mods_missing(&self) -> Option<u32> {
        (self.profile & P_MODS_KNOWN != 0).then_some(((self.profile >> 16) & 0xFFFF) as u32)
    }
    /// Of its mods on disk, how many have an update waiting.
    pub fn mods_stale(&self) -> u32 {
        ((self.profile >> 32) & 0xFFFF) as u32
    }
    pub fn mods_count(&self) -> usize {
        self.server.mods.len()
    }

    /// Its in-game clock at `now` (Unix seconds) as minutes of the day,
    /// estimated from the last one seen (see [`super::clock`]); `None` when no
    /// clock was ever seen.
    pub fn minute_at(&self, now: u64) -> Option<u32> {
        self.clock.map(|c| c.minute_at(now))
    }

    /// Its in-game clock now, "HH:MM" (estimated, as the rows, the filter and
    /// the sort read it); the list's text when no clock was ever seen.
    pub fn time_now(&self) -> String {
        let mut buf = [0u8; 5];
        self.minute_at(dz_common::time::now_secs()).map_or_else(
            || self.server.time.clone(),
            |m| clock::clock_text(m, &mut buf).to_owned(),
        )
    }

    /// The owned row (for the control channel, a connect flow).
    pub fn to_row(&self) -> ServerRow {
        let s = self.server;
        ServerRow {
            game_port: s.game_port,
            ip: s.ip(),
            query_port: s.query_port(),
            name: s.name.clone(),
            map: self.map.to_owned(),
            players: self.players(),
            max_players: self.max_players(),
            environment: s.environment.clone(),
            password: s.password,
            version: s.version.clone(),
            first_person_only: s.first_person_only,
            time: self.time_now(),
            mods_count: s.mods.len(),
            mods_missing: self.mods_missing(),
            mods_stale: self.mods_stale(),
            vac: s.vac,
            battl_eye: s.battl_eye,
            bots: self.bots(),
            ping_ms: self.ping_ms(),
            ping_failed: self.ping_failed(),
            favorite: self.favorite(),
            excluded: self.excluded(),
            unverified_full: self.unverified_full(),
            official: s.official,
            mimics_official: self.mimics_official,
        }
    }
}

/// A filtered, sorted order of a snapshot, with the header's counts.
#[derive(Clone, Debug)]
pub struct ServerOrder {
    pub snapshot: Arc<ServerSnapshot>,
    /// Indices into the snapshot, in display order.
    pub order: Arc<[u32]>,
    pub stats: ServerStats,
    /// The query it answers.
    pub query: ServerQuery,
    /// The snapshot's profile generation and live stamp it was computed at.
    pub profile_generation: u64,
    pub live_stamp: u64,
}

impl ServerOrder {
    /// Servers matching the filters.
    pub fn total(&self) -> usize {
        self.order.len()
    }

    /// The `n`th server shown.
    pub fn get(&self, n: usize) -> Option<ServerRef<'_>> {
        self.snapshot.get(*self.order.get(n)?)
    }
}

impl ServerQuery {
    /// Whether the answer depends on live data (pings, live counts), so a
    /// scan landing results can change it.
    pub fn depends_on_live(&self) -> bool {
        self.hide_empty
            || self.hide_full
            || self.max_ping > 0
            // A clock read by a ping can move a server across dusk, or past another.
            || self.time_of_day != DayTime::Any
            || matches!(self.sort, SortCol::Ping | SortCol::Players | SortCol::Time)
    }
}

/// ASCII case-insensitive `contains` for an already lowercased needle.
pub(crate) fn contains_ci(hay: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    let (h, n) = (hay.as_bytes(), needle_lower.as_bytes());
    if n.len() > h.len() {
        return false;
    }
    if !hay.is_ascii() || !needle_lower.is_ascii() {
        return hay.to_lowercase().contains(needle_lower);
    }
    h.windows(n.len())
        .any(|w| w.iter().zip(n).all(|(a, b)| a.to_ascii_lowercase() == *b))
}

/// Whether `ip` written out ("1.2.3.4") holds `needle`; written on the
/// stack, so a search through ten thousand servers allocates nothing.
fn ip_contains(ip: Ipv4Addr, needle: &str) -> bool {
    use std::io::Write;
    let mut buf = [0u8; 15];
    let mut at = &mut buf[..];
    if write!(at, "{ip}").is_err() {
        return false;
    }
    let len = 15 - at.len();
    let text = buf.get(..len).unwrap_or_default();
    text.windows(needle.len().max(1))
        .any(|w| w == needle.as_bytes())
}

/// Whether a server's version is the game's: "1.28.162391" against
/// "1.28.162391", or against a file version with a zero between
/// ("1.28.0.162391"). Compares the numbers, else the trimmed text; nothing
/// is allocated.
pub fn same_version(a: &str, b: &str) -> bool {
    fn parts(v: &str) -> Option<([u64; 4], usize)> {
        let mut out = [0u64; 4];
        let mut n = 0;
        for p in v.trim().split('.') {
            *out.get_mut(n)? = p.parse().ok()?;
            n += 1;
        }
        // A file version's third field is a zero the servers' version leaves out.
        if n == 4 && out[2] == 0 {
            out[2] = out[3];
            n = 3;
        }
        Some((out, n))
    }
    match (parts(a), parts(b)) {
        (Some((x, n)), Some((y, m))) => n == m && x.get(..n) == y.get(..m),
        _ => a.trim() == b.trim(),
    }
}

/// Filter and sort `snap` for `q` (its `offset`/`limit` are ignored). Pure and
/// quick (well under a millisecond on 9 500 servers in release): call it on
/// any thread.
pub fn order(snap: &Arc<ServerSnapshot>, q: &ServerQuery) -> ServerOrder {
    order_at(snap, q, dz_common::time::now_secs())
}

/// [`order`] with the clocks read at `now` (Unix seconds).
pub(crate) fn order_at(snap: &Arc<ServerSnapshot>, q: &ServerQuery, now: u64) -> ServerOrder {
    let profile_generation = snap.profile_generation();
    let live_stamp = snap.live_stamp();
    let search = q.search.trim().to_lowercase();
    // Only digits and dots can be found in an address's text.
    let search_ip = !search.is_empty() && search.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    // The maps wanted, as a mask over the snapshot's maps (one spelling per map); none
    // for every map. A name no server runs adds nothing.
    let wanted: Vec<String> = q
        .maps
        .iter()
        .map(|m| m.trim().to_lowercase())
        .filter(|m| !m.is_empty())
        .collect();
    let map_filter: Option<Vec<bool>> = (!wanted.is_empty()).then(|| {
        snap.map_lower
            .iter()
            .map(|c| wanted.iter().any(|w| **w == **c))
            .collect()
    });
    // Maps whose name holds the search: one check per map, not per server.
    let map_hits: Vec<bool> = if search.is_empty() {
        Vec::new()
    } else {
        snap.maps
            .iter()
            .map(|m| contains_ci(&m.map, &search))
            .collect()
    };

    // The mods asked for, as a mask over the snapshot's distinct mods: one
    // check per mod, then a lookup per server's mod. A number is a Workshop id.
    let with_mod = q.with_mod.trim().to_lowercase();
    let mod_mask: Option<Vec<bool>> = (!with_mod.is_empty()).then(|| {
        let id = with_mod.parse::<i64>().ok();
        snap.mod_ids
            .iter()
            .zip(snap.mod_names.iter())
            .map(|(&mid, name)| Some(mid) == id || contains_ci(name, &with_mod))
            .collect()
    });
    let game_version = q
        .game_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let mut stats = ServerStats::default();
    let mut order: Vec<u32> = Vec::with_capacity(snap.len());
    for (i, s) in snap.list.result.iter().enumerate() {
        let profile = snap.profile.get(i).map_or(0, |a| a.load(Ordering::Relaxed));
        if !q.show_excluded && profile & P_EXCLUDED != 0 {
            continue;
        }
        if !q.first_person.keeps(s.first_person_only)
            || !q.password.keeps(s.password)
            || !q.battleye.keeps(s.battl_eye.unwrap_or(false))
            || !q.modded.keeps(!s.mods.is_empty())
            || !q.official.keeps(s.official)
        {
            continue;
        }
        if let Some(mask) = &mod_mask
            && !snap
                .mods_of(i)
                .iter()
                .any(|&m| mask.get(m as usize).copied().unwrap_or(false))
        {
            continue;
        }
        if q.ready {
            // Every mod on disk and current (a vanilla server needs none), and
            // the game's version when both sides say theirs.
            let mods_ok = s.mods.is_empty()
                || (profile & P_MODS_KNOWN != 0 && (profile >> 16) & 0xFFFF_FFFF == 0);
            let version_ok = match game_version {
                Some(g) if !s.version.trim().is_empty() => same_version(&s.version, g),
                _ => true,
            };
            if !mods_ok || !version_ok {
                continue;
            }
        }
        if q.time_of_day != DayTime::Any {
            let Some(minute) = snap.clock_of(i).map(|c| c.minute_at(now)) else {
                continue;
            };
            if clock::is_day(minute) != (q.time_of_day == DayTime::Day) {
                continue;
            }
        }
        let map = snap.map_of.get(i).copied().unwrap_or(NONE_U32);
        if let Some(mask) = &map_filter
            && !mask.get(map as usize).copied().unwrap_or(false)
        {
            continue;
        }
        if !search.is_empty() {
            let name_hit = snap
                .name_lower
                .get(i)
                .is_some_and(|n| n.contains(search.as_str()));
            let hit = name_hit
                || (search_ip && ip_contains(s.ip(), &search))
                || map_hits.get(map as usize).copied().unwrap_or(false);
            if !hit {
                continue;
            }
        }
        let Some(r) = snap.get(i as u32) else {
            continue;
        };
        let players = r.players();
        if q.hide_empty && players == 0 {
            continue;
        }
        let full = r.is_full();
        if q.hide_full && full {
            continue;
        }
        let ping = r.ping();
        if q.max_ping > 0 {
            match ping {
                Some(None) => continue,
                Some(Some(ms)) if ms > q.max_ping => continue,
                _ => {}
            }
        }

        stats.add(players, full, ping, s);
        order.push(i as u32);
    }

    sort(&mut order, snap, q.sort, q.asc, now);
    ServerOrder {
        snapshot: Arc::clone(snap),
        order: order.into(),
        stats,
        query: q.clone(),
        profile_generation,
        live_stamp,
    }
}

impl ServerStats {
    /// Count one shown server in the header's figures.
    fn add(&mut self, players: i64, full: bool, ping: Option<Option<u32>>, s: &Server) {
        self.shown += 1;
        self.players += players.max(0) as u64;
        self.full += u32::from(full);
        self.empty += u32::from(players == 0);
        self.modded += u32::from(!s.mods.is_empty());
        self.official += u32::from(s.official);
        if let Some(Some(ms)) = ping {
            self.pinged += 1;
            self.best_ping = Some(self.best_ping.map_or(ms, |b| b.min(ms)));
        }
    }
}

/// The header's figures over an order already worked out, with the live data
/// as it stands now: no filtering, no sorting. For a query that does not
/// depend on live data the rows stay where they are while pings land, but
/// the figures (pinged, best ping, players, full, empty) follow them.
pub fn stats_of(snap: &ServerSnapshot, order: &[u32]) -> ServerStats {
    let mut stats = ServerStats::default();
    for r in order.iter().filter_map(|&i| snap.get(i)) {
        stats.add(r.players(), r.is_full(), r.ping(), r.server);
    }
    stats
}

fn sort(order: &mut [u32], snap: &ServerSnapshot, col: SortCol, asc: bool, now: u64) {
    // Every key is worked out once per server; the sorts are stable.
    let flip = |k: i64| if asc { k } else { -k };
    match col {
        SortCol::None => {}
        SortCol::Players => {
            order.sort_by_cached_key(|&i| flip(snap.get(i).map_or(0, |r| r.players())));
        }
        SortCol::Mods => order.sort_by_cached_key(|&i| {
            flip(
                snap.list
                    .result
                    .get(i as usize)
                    .map_or(0, |s| s.mods.len() as i64),
            )
        }),
        SortCol::Name => order.sort_by_cached_key(|&i| {
            flip(i64::from(
                snap.name_rank.get(i as usize).copied().unwrap_or(0),
            ))
        }),
        // One map spelled in several cases sorts as one; no map sorts first, as "".
        SortCol::Map => order.sort_by_cached_key(|&i| {
            let rank = snap
                .map_of
                .get(i as usize)
                .and_then(|&m| snap.map_rank.get(m as usize))
                .map_or(-1, |&r| i64::from(r));
            flip(rank)
        }),
        // By the clock now, as the rows show it; servers without one go last
        // either way.
        SortCol::Time => {
            order.sort_by_cached_key(|&i| match snap.clock_minute(i, now) {
                None => (1u8, 0),
                Some(m) => (0, flip(i64::from(m))),
            });
        }
        // Answered first (by RTT, in the chosen direction), then timeouts,
        // then servers never pinged.
        SortCol::Ping => order.sort_by_cached_key(|&i| match snap.get(i).and_then(|r| r.ping()) {
            Some(Some(ms)) => (0u8, flip(i64::from(ms))),
            Some(None) => (1, 0),
            None => (2, 0),
        }),
    }
}
