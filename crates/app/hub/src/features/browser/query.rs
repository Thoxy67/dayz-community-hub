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

/// What the browser shows: filters, sort, and the window of rows wanted.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
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
    use super::super::snapshot::ProfileView;
    use super::*;
    use dz_api::{Endpoint, Server, ServerList};
    use rustc_hash::FxHashSet;

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
        pings: &[(&str, i64, u32)],
    ) -> Arc<ServerSnapshot> {
        let s = Arc::new(ServerSnapshot::build(Arc::new(list), profile));
        for &(ip, port, ms) in pings {
            s.put_live(ip, port, &ping(ms));
        }
        s
    }

    fn compute(s: &Arc<ServerSnapshot>, q: &ServerQuery) -> (Vec<u32>, ServerStats) {
        let o = order(s, q);
        (o.order.to_vec(), o.stats)
    }

    fn timed(name: &str, qport: i64, players: i64, max: i64, time: &str) -> Server {
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
    fn a_ping_lands_in_the_row_without_a_query() {
        let s = snap(list(), &ProfileView::default(), &[]);
        assert_eq!(s.get(1).unwrap().ping_ms(), None);
        let stamp = s.live_stamp();
        s.put_live("2.2.2.2", 2, &ping(77));
        assert_eq!(s.get(1).unwrap().ping_ms(), Some(77));
        assert!(s.live_stamp() > stamp);
        // A failed one: a timeout, and the list's count stays.
        s.put_live(
            "2.2.2.2",
            2,
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
            excluded: ["2.2.2.2".to_string()].into_iter().collect(),
            favorites: [("3.3.3.3".to_string(), vec![2])].into_iter().collect(),
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
            favorites: [("2.2.2.2".to_string(), vec![1])].into_iter().collect(),
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
        s.put_live("2.2.2.2", 2, &ping(80));
        s.put_live("3.3.3.3", 2, &ping(30));
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
        assert_eq!(page.rows[0].ip, "2.2.2.2");
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
            favorites: rustc_hash::FxHashMap::default(),
            mods: Some([(0, false), (1, true)].into_iter().collect()),
        };
        let s = snap(one(), &profile, &[]);
        let r = s.get(0).unwrap().to_row();
        assert_eq!((r.mods_missing, r.mods_stale), (Some(1), 1));
    }

    #[test]
    fn lookups_by_query_or_game_port() {
        let s = snap(list(), &ProfileView::default(), &[]);
        assert_eq!(s.find("2.2.2.2", 2), Some(1));
        assert_eq!(s.find("2.2.2.2", 1), Some(1), "the game port");
        assert_eq!(s.find("9.9.9.9", 2), None);
    }

    #[test]
    fn case_insensitive_contains() {
        use super::super::snapshot::contains_ci;
        assert!(contains_ci("DayZ Deer Isle", "deer"));
        assert!(!contains_ci("DayZ", "deer"));
        assert!(contains_ci("Él Server", "él"));
    }
}
