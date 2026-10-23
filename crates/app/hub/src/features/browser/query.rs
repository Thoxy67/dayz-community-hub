//! The browser's query and its answer as the control channel and the tests
//! see it: filters and sort ([`ServerQuery`]), owned rows ([`ServerRow`]),
//! a page of them ([`ServerPage`]). The work itself is the snapshot's
//! ([`super::snapshot::order`]); a page is that order's window, made owned.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::snapshot::{ServerSnapshot, order};

/// A three-way filter: everything, only those with the flag, only those without.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum Tri {
    #[default]
    All,
    Only,
    None,
}

impl Tri {
    pub(crate) fn keeps(self, flag: bool) -> bool {
        match self {
            Tri::All => true,
            Tri::Only => flag,
            Tri::None => !flag,
        }
    }
}

/// The column the browser sorts by.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
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

/// The in-game time of day a server is at now (see [`super::clock`] for the
/// bounds and how the clock is estimated).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum DayTime {
    #[default]
    Any,
    /// 06:00 to 20:00.
    Day,
    /// 20:00 to 06:00.
    Night,
}

/// What the browser shows: filters, sort, and the window of rows wanted.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerQuery {
    /// Case-insensitive, over name, IP and map.
    pub search: String,
    /// The maps shown, matched case-insensitively; empty for every map. Reads the
    /// old single `map` too (a name or null), and a name alone as a set of one.
    #[serde(default, alias = "map", deserialize_with = "one_or_many")]
    pub maps: Vec<String>,
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
    /// Servers whose in-game clock is now at this time of day; servers with
    /// no clock go unless it is `Any`.
    #[serde(default)]
    pub time_of_day: DayTime,
    /// Servers running a mod: its Workshop id, or part of its name
    /// (case-insensitive); blank for any.
    #[serde(default)]
    pub with_mod: String,
    /// Only servers a join starts at once: every mod on disk and up to date
    /// (known once the disk is read), and the server's version the game's
    /// when both are known.
    #[serde(default)]
    pub ready: bool,
    /// The installed game's version (`1.28.162391`), for `ready`; `None`
    /// when not known, and the version is not compared.
    #[serde(default)]
    pub game_version: Option<String>,
    pub sort: SortCol,
    pub asc: bool,
    pub offset: u32,
    pub limit: u32,
}

/// A map filter as written: null, one name, or a list of names.
fn one_or_many<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(Option<String>),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::One(m) => m.into_iter().collect(),
        OneOrMany::Many(v) => v,
    })
}

/// One row of the browser: the server with its freshest known state.
#[derive(Serialize, Clone, Debug)]
pub struct ServerRow {
    /// 0 when unknown.
    pub game_port: u16,
    pub ip: dz_api::Ipv4Addr,
    pub query_port: u16,
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
    /// Of the server's mods, how many are not on disk; null while the mods
    /// on disk are not known yet.
    pub mods_missing: Option<u32>,
    /// Of the server's mods on disk, how many have an update waiting.
    pub mods_stale: u32,
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
#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
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
#[derive(Serialize, Clone, Debug)]
pub struct ServerPage {
    /// Servers matching the filters.
    pub total: u32,
    /// Rows `offset..offset + limit`.
    pub rows: Vec<ServerRow>,
    pub stats: ServerStats,
    /// The data generation these rows reflect.
    pub generation: u64,
}

/// Answer a query from a snapshot: the order, then the rows of the window
/// asked for. Blocking work: call it from `spawn_blocking`.
pub(crate) fn run(snap: &Arc<ServerSnapshot>, q: &ServerQuery, generation: u64) -> ServerPage {
    let o = order(snap, q);
    let start = (q.offset as usize).min(o.order.len());
    let end = start.saturating_add(q.limit as usize).min(o.order.len());
    let rows = o
        .order
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .filter_map(|&i| snap.get(i).map(|r| r.to_row()))
        .collect();
    ServerPage {
        total: o.order.len() as u32,
        rows,
        stats: o.stats,
        generation,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::super::live::Live;
    use super::super::snapshot::{ProfileView, order_at};
    use super::*;
    use dz_api::{Server, ServerList, SocketAddrV4};
    use rustc_hash::FxHashSet;

    fn at(ip: &str, port: u16) -> SocketAddrV4 {
        SocketAddrV4::new(ip.parse().unwrap(), port)
    }

    fn query() -> ServerQuery {
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
            limit: 100,
            ..ServerQuery::default()
        }
    }

    fn ping(ms: u32) -> Live {
        Live {
            ms: Some(ms),
            players: None,
            max_players: None,
            bots: None,
            failed: false,
        }
    }

    /// A snapshot of `list` with the profile and these pings.
    fn snap(
        list: ServerList,
        profile: &ProfileView,
        pings: &[(&str, u16, u32)],
    ) -> Arc<ServerSnapshot> {
        let s = Arc::new(ServerSnapshot::build(Arc::new(list), profile));
        for &(ip, port, ms) in pings {
            s.put_live(at(ip, port), &ping(ms));
        }
        s
    }

    fn compute(s: &Arc<ServerSnapshot>, q: &ServerQuery) -> (Vec<u32>, ServerStats) {
        let o = order(s, q);
        (o.order.to_vec(), o.stats)
    }

    fn timed(name: &str, qport: u16, players: i64, max: i64, time: &str) -> Server {
        Server {
            endpoint: at("1.1.1.1", qport),
            game_port: qport - 1,
            name: name.into(),
            map: "chernarusplus".into(),
            players,
            max_players: max,
            time: time.into(),
            ..Default::default()
        }
    }

    fn server(ip: &str, qport: u16, players: i64, mods: usize) -> Server {
        Server {
            endpoint: at(ip, qport),
            game_port: qport - 1,
            name: format!("{ip}:{qport}"),
            map: "enoch".into(),
            players,
            max_players: 60,
            mods: (0..mods)
                .map(|i| dz_api::Mod {
                    name: "".into(),
                    steam_workshop_id: i as i64,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn list() -> ServerList {
        ServerList {
            status: 0,
            result: vec![
                server("1.1.1.1", 2, 10, 0).into(),
                server("2.2.2.2", 2, 20, 3).into(),
                server("3.3.3.3", 2, 30, 0).into(),
            ],
        }
    }

    #[test]
    fn filters_sorts_and_counts() {
        let list = ServerList {
            status: 0,
            result: vec![
                timed("Bravo", 1, 0, 60, "12:00").into(),
                timed("alpha", 2, 60, 60, "06:30").into(),
                timed("Charlie", 3, 10, 60, "").into(),
            ],
        };
        let s = snap(list, &ProfileView::default(), &[("1.1.1.1", 3, 40)]);

        let mut q = query();
        q.sort = SortCol::Name;
        let (order, stats) = compute(&s, &q);
        assert_eq!(order, [1, 0, 2]);
        assert_eq!(stats.shown, 3);
        assert_eq!(stats.full, 1);
        assert_eq!(stats.empty, 1);
        assert_eq!(stats.best_ping, Some(40));

        q.asc = false;
        assert_eq!(compute(&s, &q).0, [2, 0, 1]);
        q.asc = true;

        q.sort = SortCol::Ping;
        let (order, _) = compute(&s, &q);
        assert_eq!(order[0], 2, "the only pinged server comes first");

        q.sort = SortCol::Time;
        let (order, _) = compute(&s, &q);
        assert_eq!(order, [1, 0, 2], "no time goes last");
        q.asc = false;
        assert_eq!(compute(&s, &q).0, [0, 1, 2], "no time goes last either way");
        q.asc = true;

        q.hide_empty = true;
        q.hide_full = true;
        q.search = "CHAR".into();
        let (order, _) = compute(&s, &q);
        assert_eq!(order, [2]);
    }

    #[test]
    fn a_search_finds_servers_by_their_address() {
        // Named apart from their addresses: only the address can match.
        let mut l = list();
        for s in &mut l.result {
            Arc::make_mut(s).name = "server".into();
        }
        let s = snap(l, &ProfileView::default(), &[]);
        let mut q = query();
        q.search = "2.2.2".into();
        assert_eq!(compute(&s, &q).0, [1]);
        q.search = " 3.3.3.3 ".into();
        assert_eq!(compute(&s, &q).0, [2]);
        q.search = "4.4".into();
        assert!(compute(&s, &q).0.is_empty());
    }

    #[test]
    fn a_ping_lands_in_the_row_without_a_query() {
        let s = snap(list(), &ProfileView::default(), &[]);
        assert_eq!(s.get(1).unwrap().ping_ms(), None);
        let stamp = s.live_stamp();
        s.put_live(at("2.2.2.2", 2), &ping(77));
        assert_eq!(s.get(1).unwrap().ping_ms(), Some(77));
        assert!(s.live_stamp() > stamp);
        // A failed one: a timeout, and the list's count stays.
        s.put_live(
            at("2.2.2.2", 2),
            &Live {
                ms: Some(9999),
                players: Some(1),
                max_players: Some(2),
                bots: Some(0),
                failed: true,
            },
        );
        let r = s.get(1).unwrap();
        assert!(r.ping_failed());
        assert_eq!(r.ping(), Some(None));
        assert_eq!(r.players(), 20);
    }

    #[test]
    fn maps_spelled_in_several_cases_are_one_map() {
        let mut l = list();
        std::sync::Arc::make_mut(&mut l.result[0]).map = "ChernarusPlus".into();
        std::sync::Arc::make_mut(&mut l.result[1]).map = "chernarusplus".into();
        std::sync::Arc::make_mut(&mut l.result[2]).map = " chernarusplus".into();
        let s = snap(l, &ProfileView::default(), &[]);
        assert_eq!(s.maps().len(), 1);
        assert_eq!(s.maps()[0].map, "chernarusplus");
        assert_eq!(s.maps()[0].count, 3);
        for i in 0..3 {
            assert_eq!(s.get(i).unwrap().map, "chernarusplus");
        }
        let mut q = query();
        q.maps = vec!["CHERNARUSPLUS".into()];
        assert_eq!(compute(&s, &q).0.len(), 3);
        q.maps = vec!["enoch".into()];
        assert!(compute(&s, &q).0.is_empty());
    }

    #[test]
    fn several_maps_at_once_and_none_for_all() {
        let mut l = list();
        std::sync::Arc::make_mut(&mut l.result[0]).map = "ChernarusPlus".into();
        std::sync::Arc::make_mut(&mut l.result[1]).map = "sakhal".into();
        let s = snap(l, &ProfileView::default(), &[]);
        let mut q = query();
        q.maps = vec!["chernarusplus".into(), "Enoch".into()];
        let (order, stats) = compute(&s, &q);
        assert_eq!(order, [0, 2]);
        assert_eq!(stats.shown, 2);
        q.maps = vec!["sakhal".into(), "nowhere".into()];
        assert_eq!(compute(&s, &q).0, [1], "an unknown map adds nothing");
        q.maps = vec!["nowhere".into()];
        assert!(compute(&s, &q).0.is_empty());
        q.maps = vec![" ".into()];
        assert_eq!(compute(&s, &q).0.len(), 3, "a blank name is no filter");
        q.maps.clear();
        assert_eq!(compute(&s, &q).0.len(), 3);
    }

    #[test]
    fn the_map_filter_reads_the_old_single_map_and_a_list() {
        let base = || {
            serde_json::json!({
                "search": "", "firstPerson": "all", "password": "all",
                "battleye": "all", "modded": "all", "official": "all",
                "hideEmpty": false, "hideFull": false, "maxPing": 0,
                "showExcluded": false, "sort": "none", "asc": true,
                "offset": 0, "limit": 10,
            })
        };
        let with = |k: &str, v: serde_json::Value| {
            let mut q = base();
            q[k] = v;
            serde_json::from_value::<ServerQuery>(q).unwrap().maps
        };
        assert!(
            serde_json::from_value::<ServerQuery>(base())
                .unwrap()
                .maps
                .is_empty()
        );
        assert!(with("map", serde_json::Value::Null).is_empty());
        assert_eq!(with("map", "enoch".into()), ["enoch"]);
        assert_eq!(
            with("map", serde_json::json!(["enoch", "sakhal"])),
            ["enoch", "sakhal"]
        );
        assert_eq!(with("maps", serde_json::json!(["sakhal"])), ["sakhal"]);
        assert_eq!(with("maps", "sakhal".into()), ["sakhal"]);
    }

    #[test]
    fn max_ping_drops_slow_and_failed_but_keeps_unpinged() {
        let s = snap(
            list(),
            &ProfileView::default(),
            &[("1.1.1.1", 2, 40), ("2.2.2.2", 2, 400)],
        );
        let mut q = query();
        q.max_ping = 100;
        let (order, stats) = compute(&s, &q);
        assert_eq!(order, [0, 2], "the 400 ms one goes, the unpinged one stays");
        assert_eq!(stats.pinged, 1);
        assert!(q.depends_on_live());
    }

    #[test]
    fn excluded_ips_are_hidden_unless_asked_and_favorites_are_marked() {
        let profile = ProfileView {
            excluded: [at("2.2.2.2", 1).ip().to_owned()].into_iter().collect(),
            favorites: [at("3.3.3.3", 2)].into_iter().collect(),
            mods: None,
        };
        let s = snap(list(), &profile, &[]);
        let mut q = query();
        assert_eq!(compute(&s, &q).0, [0, 2]);
        q.show_excluded = true;
        assert_eq!(compute(&s, &q).0.len(), 3);
        assert!(s.get(2).unwrap().to_row().favorite);
        assert!(s.get(1).unwrap().to_row().excluded);

        // The profile changes: the flags follow, in place.
        let before = s.profile_generation();
        s.apply_profile(&ProfileView::default());
        assert!(s.profile_generation() > before);
        assert!(!s.get(2).unwrap().favorite());
        q.show_excluded = false;
        assert_eq!(compute(&s, &q).0.len(), 3);
    }

    #[test]
    fn the_same_flags_written_again_do_not_move_the_profile_generation() {
        let profile = ProfileView {
            excluded: FxHashSet::default(),
            // The favourite is saved under the game port.
            favorites: [at("2.2.2.2", 1)].into_iter().collect(),
            mods: None,
        };
        let s = snap(list(), &profile, &[]);
        assert!(s.get(1).unwrap().favorite());
        let before = s.profile_generation();
        assert!(!s.apply_profile(&profile));
        assert_eq!(s.profile_generation(), before);
        assert!(s.apply_profile(&ProfileView::default()));
        assert!(s.profile_generation() > before);
    }

    #[test]
    fn the_figures_follow_pings_over_an_order_kept_as_it_is() {
        let s = snap(list(), &ProfileView::default(), &[]);
        let o = order(&s, &query());
        assert_eq!((o.stats.pinged, o.stats.best_ping), (0, None));
        s.put_live(at("2.2.2.2", 2), &ping(80));
        s.put_live(at("3.3.3.3", 2), &ping(30));
        let stats = super::super::stats_of(&s, &o.order);
        assert_eq!((stats.pinged, stats.best_ping), (2, Some(30)));
        assert_eq!(stats, order(&s, &query()).stats, "the same as a new order");
    }

    #[test]
    fn modded_and_players_sort() {
        let s = snap(list(), &ProfileView::default(), &[]);
        let mut q = query();
        q.modded = Tri::Only;
        assert_eq!(compute(&s, &q).0, [1]);
        q.modded = Tri::All;
        q.sort = SortCol::Players;
        q.asc = false;
        assert_eq!(compute(&s, &q).0, [2, 1, 0]);
    }

    #[test]
    fn official_filter_and_lookalikes() {
        let mut l = list();
        std::sync::Arc::make_mut(&mut l.result[0]).official = true;
        std::sync::Arc::make_mut(&mut l.result[0]).name = "4193 | EUROPE - DE".into();
        std::sync::Arc::make_mut(&mut l.result[1]).name = "4193 | EUROPE - DE | 100x loot".into();
        let s = snap(l, &ProfileView::default(), &[]);
        let mut q = query();
        q.official = Tri::Only;
        let (order, stats) = compute(&s, &q);
        assert_eq!(order, [0]);
        assert_eq!(stats.official, 1);
        q.official = Tri::None;
        assert_eq!(compute(&s, &q).0, [1, 2]);
        let official = s.get(0).unwrap().to_row();
        assert!(official.official && !official.mimics_official);
        let copy = s.get(1).unwrap().to_row();
        assert!(!copy.official && copy.mimics_official);
        assert!(!s.get(2).unwrap().to_row().mimics_official);
    }

    #[test]
    fn a_page_is_the_window_asked_for() {
        let s = snap(list(), &ProfileView::default(), &[]);
        let mut q = query();
        q.offset = 1;
        q.limit = 5;
        let page = run(&s, &q, 1);
        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0].ip, *at("2.2.2.2", 1).ip());
        q.offset = 99;
        assert!(run(&s, &q, 1).rows.is_empty());
    }

    #[test]
    fn rows_say_which_of_a_servers_mods_are_missing_or_behind() {
        // The server runs mods 0, 1, 2: 0 is current, 1 is behind, 2 is not on disk.
        let one = || ServerList {
            status: 0,
            result: vec![server("1.1.1.1", 2, 5, 3).into()],
        };
        let s = snap(one(), &ProfileView::default(), &[]);
        let r = s.get(0).unwrap().to_row();
        assert_eq!(
            (r.mods_missing, r.mods_stale),
            (None, 0),
            "disk not read yet"
        );
        let profile = ProfileView {
            excluded: FxHashSet::default(),
            favorites: FxHashSet::default(),
            mods: Some([(0, false), (1, true)].into_iter().collect()),
        };
        let s = snap(one(), &profile, &[]);
        let r = s.get(0).unwrap().to_row();
        assert_eq!((r.mods_missing, r.mods_stale), (Some(1), 1));
    }

    #[test]
    fn lookups_by_query_or_game_port() {
        let s = snap(list(), &ProfileView::default(), &[]);
        assert_eq!(s.find(at("2.2.2.2", 2)), Some(1));
        assert_eq!(s.find(at("2.2.2.2", 1)), Some(1), "the game port");
        assert_eq!(s.find(at("9.9.9.9", 2)), None);
    }

    fn modded(qport: u16, mods: &[(i64, &str)], version: &str) -> Arc<Server> {
        Server {
            endpoint: at("1.1.1.1", qport),
            game_port: qport - 1,
            name: format!("s{qport}"),
            map: "chernarusplus".into(),
            version: version.into(),
            mods: mods
                .iter()
                .map(|&(id, name)| dz_api::Mod {
                    name: name.into(),
                    steam_workshop_id: id,
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }

    #[test]
    fn the_time_of_day_filter_reads_the_clock_now() {
        let list = ServerList {
            status: 0,
            result: vec![
                timed("noon", 2, 1, 60, "12:00").into(),
                timed("late", 3, 1, 60, "19:30").into(),
                timed("night", 4, 1, 60, "02:00").into(),
                timed("none", 5, 1, 60, "").into(),
            ],
        };
        let s = snap(list, &ProfileView::default(), &[]);
        let seen = 1_790_000_000;
        s.set_list_seen(seen);
        let mut q = query();
        assert_eq!(order_at(&s, &q, seen).order.len(), 4, "any: every server");
        q.time_of_day = DayTime::Day;
        assert_eq!(order_at(&s, &q, seen).order.to_vec(), [0, 1]);
        q.time_of_day = DayTime::Night;
        assert_eq!(
            order_at(&s, &q, seen).order.to_vec(),
            [2],
            "no clock, no answer"
        );
        assert!(q.depends_on_live());

        // An hour later at real time: 19:30 has turned 20:30.
        let later = seen + 3600;
        assert_eq!(order_at(&s, &q, later).order.to_vec(), [1, 2]);

        // A ping reads the clock and its rates: 03:00 at 6× by day, 6×1.5 = 9×
        // at night, seen now. Twenty minutes of real time later it is 06:00.
        s.put_clock(
            at("1.1.1.1", 4),
            &super::super::clock::Clock::from_keywords("etm6.000000,entm1.500000,03:00", seen)
                .unwrap(),
        );
        let half = seen + 20 * 60;
        q.time_of_day = DayTime::Day;
        assert_eq!(order_at(&s, &q, half).order.to_vec(), [0, 1, 2]);
        assert_eq!(s.clock_minute(2, half), Some(6 * 60));

        // Sorted by the clock now: an hour on, 13:00, 20:30 and (06:00 then
        // forty minutes at 6×) 10:00; no clock last.
        q.time_of_day = DayTime::Any;
        q.sort = SortCol::Time;
        assert_eq!(s.clock_minute(2, later), Some(10 * 60));
        assert_eq!(order_at(&s, &q, later).order.to_vec(), [2, 0, 1, 3]);
    }

    #[test]
    fn the_mod_filter_matches_a_name_or_a_workshop_id() {
        let list = ServerList {
            status: 0,
            result: vec![
                modded(
                    2,
                    &[(1559212036, "CF"), (1564026768, "Community-Online-Tools")],
                    "",
                ),
                modded(3, &[(1559212036, "CF")], ""),
                modded(4, &[(2545327648, "Dabs Framework")], ""),
                modded(5, &[], ""),
            ],
        };
        let s = snap(list, &ProfileView::default(), &[]);
        assert_eq!(s.mods().count(), 3, "each mod once");
        let mut q = query();
        q.with_mod = "  cf ".into();
        assert_eq!(compute(&s, &q).0, [0, 1]);
        q.with_mod = "online-tools".into();
        assert_eq!(compute(&s, &q).0, [0]);
        q.with_mod = "2545327648".into();
        assert_eq!(compute(&s, &q).0, [2]);
        q.with_mod = "nothing like it".into();
        assert!(compute(&s, &q).0.is_empty());
        q.with_mod = " ".into();
        assert_eq!(compute(&s, &q).0.len(), 4, "a blank name is no filter");
    }

    #[test]
    fn ready_to_join_needs_every_mod_current_and_the_games_version() {
        let list = || ServerList {
            status: 0,
            result: vec![
                modded(2, &[(1, "a"), (2, "b")], "1.28.162391"),
                modded(3, &[(1, "a"), (3, "c")], "1.28.162391"),
                modded(4, &[(2, "b")], "1.28.162391"),
                modded(5, &[], "1.27.160000"),
                modded(6, &[], ""),
            ],
        };
        // Mod 1 is current, mod 2 is behind, mod 3 is not on disk.
        let profile = ProfileView {
            mods: Some([(1, false), (2, true)].into_iter().collect()),
            ..ProfileView::default()
        };
        let s = snap(list(), &profile, &[]);
        let mut q = query();
        q.ready = true;
        assert_eq!(
            compute(&s, &q).0,
            [3, 4],
            "only the vanilla ones: no game version to compare"
        );
        q.game_version = Some("1.28.0.162391".into());
        assert_eq!(compute(&s, &q).0, [4], "the other version goes");

        // Mod 2 updated and mod 3 downloaded.
        s.apply_profile(&ProfileView {
            mods: Some([(1, false), (2, false), (3, false)].into_iter().collect()),
            ..ProfileView::default()
        });
        assert_eq!(compute(&s, &q).0, [0, 1, 2, 4]);

        // The disk not read yet: no modded server can be promised.
        let s = snap(list(), &ProfileView::default(), &[]);
        assert_eq!(compute(&s, &q).0, [4]);
    }

    #[test]
    fn versions_compare_by_their_numbers() {
        use super::super::snapshot::same_version;
        assert!(same_version("1.28.162391", " 1.28.0.162391 "));
        assert!(!same_version("1.28.162391", "1.28.162392"));
        assert!(!same_version("1.28", "1.28.162391"));
        assert!(same_version("exp", "exp"));
        assert!(!same_version("1.2.3.4.5", "1.2.3"));
    }

    #[test]
    fn the_new_filters_read_from_json_and_default_off() {
        let q: ServerQuery = serde_json::from_value(serde_json::json!({
            "search": "", "firstPerson": "all", "password": "all",
            "battleye": "all", "modded": "all", "official": "all",
            "hideEmpty": false, "hideFull": false, "maxPing": 0,
            "showExcluded": false, "sort": "none", "asc": true,
            "offset": 0, "limit": 10,
        }))
        .unwrap();
        assert_eq!(q.time_of_day, DayTime::Any);
        assert!(q.with_mod.is_empty() && !q.ready && q.game_version.is_none());
        let q: ServerQuery = serde_json::from_value(serde_json::json!({
            "search": "", "firstPerson": "all", "password": "all",
            "battleye": "all", "modded": "all", "official": "all",
            "hideEmpty": false, "hideFull": false, "maxPing": 0,
            "showExcluded": false, "sort": "none", "asc": true,
            "offset": 0, "limit": 10, "timeOfDay": "night", "withMod": "CF",
            "ready": true, "gameVersion": "1.28.162391",
        }))
        .unwrap();
        assert_eq!(q.time_of_day, DayTime::Night);
        assert_eq!(q.with_mod, "CF");
        assert!(q.ready);
    }

    #[test]
    fn case_insensitive_contains() {
        use super::super::snapshot::contains_ci;
        assert!(contains_ci("DayZ Deer Isle", "deer"));
        assert!(!contains_ci("DayZ", "deer"));
        assert!(contains_ci("Él Server", "él"));
    }
}
