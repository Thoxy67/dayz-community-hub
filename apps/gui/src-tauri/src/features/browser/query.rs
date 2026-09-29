//! Filtering, sorting and paging the server list for the browser.
//!
//! The filtered and sorted order is cached by (query without its window,
//! generation), so scrolling only builds `limit` rows.

use dz_api::{Server, ServerList};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};

use super::live::{Live, LiveMap};

/// A three-way filter: everything, only those with the flag, only those without.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Tri {
    #[default]
    All,
    Only,
    None,
}

impl Tri {
    fn keeps(self, flag: bool) -> bool {
        match self {
            Tri::All => true,
            Tri::Only => flag,
            Tri::None => !flag,
        }
    }
}

/// The column the browser sorts by.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SortCol {
    #[default]
    None,
    Ping,
    Players,
    Name,
    Map,
    Mods,
    /// In-game time, "HH:MM" as minutes.
    Time,
}

/// What the browser shows: filters, sort, and the window of rows wanted.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerQuery {
    /// Case-insensitive, over name, IP and map.
    pub search: String,
    pub map: Option<String>,
    pub first_person: Tri,
    pub password: Tri,
    pub battleye: Tri,
    pub modded: Tri,
    /// Bohemia's own servers (only listed with a Steam API key).
    pub official: Tri,
    pub hide_empty: bool,
    pub hide_full: bool,
    /// 0 = any. Otherwise timeouts and slower servers go; unpinged ones stay.
    pub max_ping: u32,
    /// Show servers on excluded IPs too.
    pub show_excluded: bool,
    pub sort: SortCol,
    pub asc: bool,
    pub offset: u32,
    pub limit: u32,
}

/// One row of the browser: the server with its freshest known state.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct ServerRow {
    pub game_port: i64,
    pub ip: String,
    pub query_port: i64,
    pub name: String,
    pub map: String,
    /// Freshest known: the last successful query, else the list.
    pub players: i64,
    pub max_players: i64,
    pub environment: String,
    pub password: bool,
    pub version: String,
    pub first_person_only: bool,
    pub time: String,
    pub mods_count: usize,
    pub vac: bool,
    pub battl_eye: Option<bool>,
    pub bots: u32,
    /// Null when never pinged. >= 5000 when it timed out.
    pub ping_ms: Option<u32>,
    pub ping_failed: bool,
    pub favorite: bool,
    pub excluded: bool,
    /// Full according to the list, but the last query failed.
    pub unverified_full: bool,
    /// Bohemia's own server.
    pub official: bool,
    /// A community server named like an official one ("1234 | EUROPE - DE").
    pub mimics_official: bool,
}

/// Totals over the filtered servers.
#[derive(Serialize, Clone, Debug, Default, specta::Type)]
pub struct ServerStats {
    pub shown: u32,
    pub players: u64,
    pub full: u32,
    pub empty: u32,
    pub modded: u32,
    pub official: u32,
    pub pinged: u32,
    pub best_ping: Option<u32>,
}

/// A window of the filtered, sorted list.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct ServerPage {
    /// Servers matching the filters.
    pub total: u32,
    /// Rows `offset..offset + limit`.
    pub rows: Vec<ServerRow>,
    pub stats: ServerStats,
    /// The data generation these rows reflect.
    pub generation: u64,
}

/// What a query reads besides the list and the live data.
pub(crate) struct ProfileView {
    pub excluded: FxHashSet<String>,
    /// "ip:port" of each favorite.
    pub favorites: FxHashSet<String>,
}

impl ProfileView {
    fn is_favorite(&self, s: &Server) -> bool {
        !self.favorites.is_empty()
            && (self
                .favorites
                .contains(&format!("{}:{}", s.endpoint.ip, s.endpoint.port))
                || self
                    .favorites
                    .contains(&format!("{}:{}", s.endpoint.ip, s.game_port)))
    }
}

/// A server with its live entry folded in, as the filters and sort see it.
struct View<'a> {
    s: &'a Server,
    live: Option<&'a Live>,
}

impl View<'_> {
    fn ok_live(&self) -> Option<&Live> {
        self.live.filter(|l| !l.failed)
    }
    fn players(&self) -> i64 {
        self.ok_live()
            .and_then(|l| l.players)
            .map_or(self.s.players, i64::from)
    }
    fn max_players(&self) -> i64 {
        self.ok_live()
            .and_then(|l| l.max_players)
            .map_or(self.s.max_players, i64::from)
    }
    /// `None` never pinged, `Some(None)` failed, `Some(Some(ms))` answered.
    fn ping(&self) -> Option<Option<u32>> {
        let l = self.live?;
        if l.ping_failed() {
            return Some(None);
        }
        l.ms.map(Some)
    }
    fn is_full(&self) -> bool {
        let max = self.max_players();
        max > 0 && self.players() >= max
    }
}

fn time_minutes(t: &str) -> Option<u32> {
    let (h, m) = t.split_once(':')?;
    Some(h.trim().parse::<u32>().ok()? * 60 + m.trim().parse::<u32>().ok()?)
}

/// ASCII case-insensitive `contains` for an already lowercased needle.
fn contains_ci(hay: &str, needle_lower: &str) -> bool {
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

/// The cached order for one query.
struct Cached {
    key: ServerQuery,
    generation: u64,
    list: Arc<ServerList>,
    order: Arc<Vec<u32>>,
    stats: ServerStats,
}

static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

fn key_of(q: &ServerQuery) -> ServerQuery {
    ServerQuery {
        offset: 0,
        limit: 0,
        ..q.clone()
    }
}

/// Filter and sort the whole list into an order of indices, with its stats.
fn compute(
    list: &ServerList,
    live: &LiveMap,
    profile: &ProfileView,
    q: &ServerQuery,
) -> (Vec<u32>, ServerStats) {
    let search = q.search.trim().to_lowercase();
    let mut stats = ServerStats::default();
    let mut order: Vec<u32> = Vec::with_capacity(list.result.len());

    for (i, s) in list.result.iter().enumerate() {
        if !q.show_excluded && profile.excluded.contains(&s.endpoint.ip) {
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
        if let Some(map) = &q.map
            && !map.is_empty()
            && s.map != *map
        {
            continue;
        }
        if !search.is_empty()
            && !(contains_ci(&s.name, &search)
                || s.endpoint.ip.contains(search.as_str())
                || contains_ci(&s.map, &search))
        {
            continue;
        }
        let v = View {
            s,
            live: live.get(&s.endpoint.ip, s.endpoint.port),
        };
        let players = v.players();
        if q.hide_empty && players == 0 {
            continue;
        }
        let full = v.is_full();
        if q.hide_full && full {
            continue;
        }
        let ping = v.ping();
        if q.max_ping > 0 {
            match ping {
                Some(None) => continue,
                Some(Some(ms)) if ms > q.max_ping => continue,
                _ => {}
            }
        }

        stats.shown += 1;
        stats.players += players.max(0) as u64;
        stats.full += u32::from(full);
        stats.empty += u32::from(players == 0);
        stats.modded += u32::from(!s.mods.is_empty());
        stats.official += u32::from(s.official);
        if let Some(Some(ms)) = ping {
            stats.pinged += 1;
            stats.best_ping = Some(stats.best_ping.map_or(ms, |b| b.min(ms)));
        }
        order.push(i as u32);
    }

    sort(&mut order, list, live, q.sort, q.asc);
    (order, stats)
}

fn sort(order: &mut [u32], list: &ServerList, live: &LiveMap, col: SortCol, asc: bool) {
    let dir = |o: Ordering| if asc { o } else { o.reverse() };
    let at = |i: u32| &list.result[i as usize];
    let view = |i: u32| {
        let s = at(i);
        View {
            s,
            live: live.get(&s.endpoint.ip, s.endpoint.port),
        }
    };
    match col {
        SortCol::None => {}
        SortCol::Players => order.sort_by_key(|&i| {
            let p = view(i).players();
            if asc { p } else { -p }
        }),
        SortCol::Mods => order.sort_by(|&a, &b| dir(at(a).mods.len().cmp(&at(b).mods.len()))),
        SortCol::Name => {
            // Lowercase once per server rather than once per comparison.
            let mut keyed: Vec<(String, u32)> = order
                .iter()
                .map(|&i| (at(i).name.to_lowercase(), i))
                .collect();
            keyed.sort_by(|a, b| dir(a.0.cmp(&b.0)));
            for (slot, (_, i)) in order.iter_mut().zip(keyed) {
                *slot = i;
            }
        }
        SortCol::Map => order.sort_by(|&a, &b| dir(at(a).map.cmp(&at(b).map))),
        SortCol::Time => order.sort_by(|&a, &b| {
            // Servers without a time go last either way.
            match (time_minutes(&at(a).time), time_minutes(&at(b).time)) {
                (Some(x), Some(y)) => dir(x.cmp(&y)),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
        }),
        SortCol::Ping => order.sort_by_key(|&i| {
            // Answered first (by RTT, in the chosen direction), then timeouts,
            // then servers never pinged.
            match view(i).ping() {
                Some(Some(ms)) => (0u8, if asc { ms as i64 } else { -(ms as i64) }),
                Some(None) => (1, 0),
                None => (2, 0),
            }
        }),
    }
}

/// Build the row for one server.
pub(crate) fn row(s: &Server, live: &LiveMap, profile: &ProfileView) -> ServerRow {
    let v = View {
        s,
        live: live.get(&s.endpoint.ip, s.endpoint.port),
    };
    let list_full = s.max_players > 0 && s.players >= s.max_players;
    ServerRow {
        game_port: s.game_port,
        ip: s.endpoint.ip.clone(),
        query_port: s.endpoint.port,
        name: s.name.clone(),
        map: s.map.clone(),
        players: v.players(),
        max_players: v.max_players(),
        environment: s.environment.clone(),
        password: s.password,
        version: s.version.clone(),
        first_person_only: s.first_person_only,
        time: s.time.clone(),
        mods_count: s.mods.len(),
        vac: s.vac,
        battl_eye: s.battl_eye,
        bots: v.ok_live().and_then(|l| l.bots).map_or(0, u32::from),
        ping_ms: v.live.and_then(|l| l.ms),
        ping_failed: v.live.is_some_and(Live::ping_failed),
        favorite: profile.is_favorite(s),
        excluded: profile.excluded.contains(&s.endpoint.ip),
        unverified_full: list_full && v.live.is_some_and(|l| l.failed),
        official: s.official,
        mimics_official: !s.official && dz_api::name_looks_official(&s.name),
    }
}

/// Answer a query. Blocking work: call it from `spawn_blocking`.
pub(crate) fn run(
    list: Arc<ServerList>,
    profile: &ProfileView,
    q: &ServerQuery,
    generation: u64,
) -> ServerPage {
    let key = key_of(q);
    let hit = CACHE.lock().ok().and_then(|c| {
        c.as_ref()
            .filter(|c| c.generation == generation && c.key == key && Arc::ptr_eq(&c.list, &list))
            .map(|c| (Arc::clone(&c.order), c.stats.clone()))
    });
    let live = super::live::store().read();
    let (order, stats) = match hit {
        Some(h) => h,
        None => {
            let (order, stats) = compute(&list, &live, profile, q);
            let order = Arc::new(order);
            if let Ok(mut c) = CACHE.lock() {
                *c = Some(Cached {
                    key,
                    generation,
                    list: Arc::clone(&list),
                    order: Arc::clone(&order),
                    stats: stats.clone(),
                });
            }
            (order, stats)
        }
    };
    let start = (q.offset as usize).min(order.len());
    let end = start.saturating_add(q.limit as usize).min(order.len());
    let rows = order[start..end]
        .iter()
        .map(|&i| row(&list.result[i as usize], &live, profile))
        .collect();
    ServerPage {
        total: order.len() as u32,
        rows,
        stats,
        generation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dz_api::Endpoint;

    fn server(name: &str, qport: i64, players: i64, max: i64, time: &str) -> Server {
        Server {
            endpoint: Endpoint {
                ip: "1.1.1.1".into(),
                port: qport,
            },
            game_port: qport - 1,
            name: name.into(),
            map: "chernarusplus".into(),
            players,
            max_players: max,
            time: time.into(),
            ..Default::default()
        }
    }

    fn query() -> ServerQuery {
        ServerQuery {
            search: String::new(),
            map: None,
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
            limit: 10,
        }
    }

    fn profile() -> ProfileView {
        ProfileView {
            excluded: FxHashSet::default(),
            favorites: FxHashSet::default(),
        }
    }

    #[test]
    fn filters_sorts_and_counts() {
        let list = ServerList {
            status: 0,
            result: vec![
                server("Bravo", 1, 0, 60, "12:00"),
                server("alpha", 2, 60, 60, "06:30"),
                server("Charlie", 3, 10, 60, ""),
            ],
        };
        let mut live = LiveMap::default();
        live.put_for_test("1.1.1.1", 3, 40);

        let mut q = query();
        q.sort = SortCol::Name;
        let (order, stats) = compute(&list, &live, &profile(), &q);
        assert_eq!(order, [1, 0, 2]);
        assert_eq!(stats.shown, 3);
        assert_eq!(stats.full, 1);
        assert_eq!(stats.empty, 1);
        assert_eq!(stats.best_ping, Some(40));

        q.sort = SortCol::Ping;
        let (order, _) = compute(&list, &live, &profile(), &q);
        assert_eq!(order[0], 2, "the only pinged server comes first");

        q.sort = SortCol::Time;
        let (order, _) = compute(&list, &live, &profile(), &q);
        assert_eq!(order, [1, 0, 2], "no time goes last");

        q.hide_empty = true;
        q.hide_full = true;
        q.search = "CHAR".into();
        let (order, _) = compute(&list, &live, &profile(), &q);
        assert_eq!(order, [2]);
    }

    #[test]
    fn case_insensitive_contains() {
        assert!(contains_ci("DayZ Deer Isle", "deer"));
        assert!(!contains_ci("DayZ", "deer"));
        assert!(contains_ci("Él Server", "él"));
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;
    use dz_api::Endpoint;

    fn server(ip: &str, qport: i64, players: i64, mods: usize) -> Server {
        Server {
            endpoint: Endpoint {
                ip: ip.into(),
                port: qport,
            },
            game_port: qport - 1,
            name: format!("{ip}:{qport}"),
            map: "enoch".into(),
            players,
            max_players: 60,
            mods: (0..mods)
                .map(|i| dz_api::Mod {
                    name: String::new(),
                    steam_workshop_id: i as i64,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn query() -> ServerQuery {
        ServerQuery {
            search: String::new(),
            map: None,
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
            limit: 100,
        }
    }

    fn list() -> ServerList {
        ServerList {
            status: 0,
            result: vec![
                server("1.1.1.1", 2, 10, 0),
                server("2.2.2.2", 2, 20, 3),
                server("3.3.3.3", 2, 30, 0),
            ],
        }
    }

    #[test]
    fn max_ping_drops_slow_and_failed_but_keeps_unpinged() {
        let mut live = LiveMap::default();
        live.put_for_test("1.1.1.1", 2, 40);
        live.put_for_test("2.2.2.2", 2, 400);
        let mut q = query();
        q.max_ping = 100;
        let profile = ProfileView {
            excluded: FxHashSet::default(),
            favorites: FxHashSet::default(),
        };
        let (order, stats) = compute(&list(), &live, &profile, &q);
        assert_eq!(order, [0, 2], "the 400 ms one goes, the unpinged one stays");
        assert_eq!(stats.pinged, 1);
    }

    #[test]
    fn excluded_ips_are_hidden_unless_asked_and_favorites_are_marked() {
        let live = LiveMap::default();
        let profile = ProfileView {
            excluded: ["2.2.2.2".to_string()].into_iter().collect(),
            favorites: ["3.3.3.3:2".to_string()].into_iter().collect(),
        };
        let mut q = query();
        let (order, _) = compute(&list(), &live, &profile, &q);
        assert_eq!(order, [0, 2]);
        q.show_excluded = true;
        let (order, _) = compute(&list(), &live, &profile, &q);
        assert_eq!(order.len(), 3);
        let l = list();
        assert!(row(&l.result[2], &live, &profile).favorite);
        assert!(row(&l.result[1], &live, &profile).excluded);
    }

    #[test]
    fn modded_and_players_sort() {
        let live = LiveMap::default();
        let profile = ProfileView {
            excluded: FxHashSet::default(),
            favorites: FxHashSet::default(),
        };
        let mut q = query();
        q.modded = Tri::Only;
        assert_eq!(compute(&list(), &live, &profile, &q).0, [1]);
        q.modded = Tri::All;
        q.sort = SortCol::Players;
        q.asc = false;
        assert_eq!(compute(&list(), &live, &profile, &q).0, [2, 1, 0]);
    }

    #[test]
    fn official_filter_and_lookalikes() {
        let live = LiveMap::default();
        let profile = ProfileView {
            excluded: FxHashSet::default(),
            favorites: FxHashSet::default(),
        };
        let mut l = list();
        l.result[0].official = true;
        l.result[0].name = "4193 | EUROPE - DE".into();
        l.result[1].name = "4193 | EUROPE - DE | 100x loot".into();
        let mut q = query();
        q.official = Tri::Only;
        let (order, stats) = compute(&l, &live, &profile, &q);
        assert_eq!(order, [0]);
        assert_eq!(stats.official, 1);
        q.official = Tri::None;
        assert_eq!(compute(&l, &live, &profile, &q).0, [1, 2]);
        let official = row(&l.result[0], &live, &profile);
        assert!(official.official && !official.mimics_official);
        let copy = row(&l.result[1], &live, &profile);
        assert!(!copy.official && copy.mimics_official);
        assert!(!row(&l.result[2], &live, &profile).mimics_official);
    }

    #[test]
    fn a_page_is_the_window_asked_for() {
        let profile = ProfileView {
            excluded: FxHashSet::default(),
            favorites: FxHashSet::default(),
        };
        let list = Arc::new(list());
        let mut q = query();
        q.offset = 1;
        q.limit = 5;
        let page = run(Arc::clone(&list), &profile, &q, 1_000_000);
        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0].ip, "2.2.2.2");
        q.offset = 99;
        assert!(run(list, &profile, &q, 1_000_000).rows.is_empty());
    }
}
