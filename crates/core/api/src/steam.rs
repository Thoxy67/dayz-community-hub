//! Servers from Steam's master list (through the Steam client), merged into
//! the launcher list.
//!
//! Steam lists every server that heartbeats to it, which is more than
//! dayzsalauncher.com carries but also tens of thousands of fakes: farms
//! that register hundreds of servers on one address, all claiming players.
//! Two signals screen them out without querying anything:
//!
//! - Steam's master counts players itself (`hasplayers` / `noplayers`), and
//!   a server cannot inflate that count the way it inflates its A2S answer.
//!   A server the master lists as empty is shown with 0 players.
//! - An address carrying many *empty* listed servers is a farm ([`FarmCounter`]): on
//!   2026-09-25, no verified server sat on an address with 50 entries or
//!   more, and 22 of thousands on addresses with 20 or more (DayZ-Launcher,
//!   docs/11).
//!
//! Only servers the launcher list does not have are added; those it has
//! keep its data.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{Endpoint, Mod, Server, tags_say_official};

/// Empty servers listed on one address from which the address is a farm.
pub const FARM_ADDRESS: usize = 20;

/// One server Steam listed, as it answered A2S.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SteamListed {
    pub ip: String,
    pub query_port: i64,
    pub game_port: i64,
    pub name: String,
    pub map: String,
    /// DayZ's gametags.
    pub tags: String,
    pub players: i64,
    pub max_players: i64,
    pub password: bool,
    pub secure: bool,
    pub version: String,
    /// "w" for Windows, "l" for Linux.
    pub environment: String,
    /// From its A2S rules.
    pub mods: Vec<Mod>,
    /// What Steam's master counts: `Some(true)` no player, `Some(false)`
    /// players, `None` not asked.
    pub empty: Option<bool>,
}

impl SteamListed {
    /// The app's model of it.
    pub fn to_server(&self) -> Server {
        let tags: Vec<&str> = self.tags.split(',').map(str::trim).collect();
        let has = |t: &str| tags.iter().any(|x| x.eq_ignore_ascii_case(t));
        Server {
            game_port: if self.game_port > 0 {
                self.game_port
            } else {
                self.query_port
            },
            endpoint: Endpoint {
                ip: self.ip.clone(),
                port: self.query_port,
            },
            name: self.name.clone(),
            map: self.map.clone(),
            players: if self.empty == Some(true) {
                0
            } else {
                self.players
            },
            max_players: self.max_players,
            environment: self.environment.clone(),
            password: self.password,
            version: self.version.clone(),
            vac: self.secure,
            battl_eye: Some(has("battleye")),
            first_person_only: has("no3rd"),
            time: game_time(&tags),
            mods: self.mods.clone(),
            official: tags_say_official(&self.tags),
        }
    }
}

/// The in-game time, DayZ's last tag ("15:12").
pub(crate) fn game_time(tags: &[&str]) -> String {
    tags.iter()
        .rev()
        .find(|t| t.len() == 5 && t.as_bytes()[2] == b':')
        .map(|t| (*t).to_string())
        .unwrap_or_default()
}

/// Counts, as the master lists them, the servers on each address that
/// Steam does not count players on, to tell farms.
#[derive(Debug, Default, Clone)]
pub struct FarmCounter {
    seen: FxHashSet<(String, i64)>,
    per_ip: FxHashMap<String, usize>,
}

impl FarmCounter {
    /// One listed server; servers with players counted by Steam are real
    /// wherever they sit, and do not count.
    pub fn add(&mut self, ip: &str, query_port: i64, empty: Option<bool>) {
        if empty != Some(false) && self.seen.insert((ip.to_owned(), query_port)) {
            // An address is counted many times (that is what a farm is): its
            // string is made once, not per server.
            match self.per_ip.get_mut(ip) {
                Some(n) => *n += 1,
                None => {
                    self.per_ip.insert(ip.to_owned(), 1);
                }
            }
        }
    }

    /// Whether `ip` carries [`FARM_ADDRESS`] or more such servers.
    pub fn is_farm(&self, ip: &str) -> bool {
        self.per_ip.get(ip).is_some_and(|&n| n >= FARM_ADDRESS)
    }
}

/// Add to `list` the servers Steam listed that it lacks, those on farm
/// addresses left out (unless Steam counts players on them), and mark
/// official the listed ones Steam's tags say are. Returns how many were
/// added.
pub fn merge_steam(list: &mut Vec<Server>, listed: &[SteamListed], farms: &FarmCounter) -> usize {
    let (mark, add) = plan(list, listed, farms);
    for i in mark {
        list[i].official = true;
    }
    let added = add.len();
    list.reserve(added);
    list.extend(add.into_iter().map(steam_server));
    added
}

/// [`merge_steam`] into a new list, `base` left as it is: one allocation of
/// the final size and one copy of each server, where cloning `base` and then
/// merging grew (and moved) the whole list a second time.
pub fn merged_steam(base: &[Server], listed: &[SteamListed], farms: &FarmCounter) -> Vec<Server> {
    let (mark, add) = plan(base, listed, farms);
    let mut out = Vec::with_capacity(base.len() + add.len());
    out.extend_from_slice(base);
    for i in mark {
        out[i].official = true;
    }
    out.extend(add.into_iter().map(steam_server));
    out
}

/// One server to add, and whether a repeat of it says it is official.
type Addition<'a> = (&'a SteamListed, bool);

fn steam_server((s, official): Addition<'_>) -> Server {
    let mut server = s.to_server();
    server.official |= official;
    server
}

/// What merging `listed` into `list` does: the positions to mark official
/// and the servers to add, decided with keys borrowed from both lists (an
/// index keyed by a copy of every address made ten thousand strings or more
/// for each merge).
fn plan<'a>(
    list: &[Server],
    listed: &'a [SteamListed],
    farms: &FarmCounter,
) -> (Vec<usize>, Vec<Addition<'a>>) {
    let at: FxHashMap<(&str, i64), usize> = list
        .iter()
        .enumerate()
        .map(|(i, s)| ((s.endpoint.ip.as_str(), s.endpoint.port), i))
        .collect();
    let mut mark: Vec<usize> = Vec::new();
    let mut add: Vec<Addition<'a>> = Vec::new();
    let mut added: FxHashMap<(&str, i64), usize> = FxHashMap::default();
    for s in listed {
        let key = (s.ip.as_str(), s.query_port);
        if let Some(&i) = at.get(&key) {
            if tags_say_official(&s.tags) {
                mark.push(i);
            }
            continue;
        }
        if let Some(&j) = added.get(&key) {
            if tags_say_official(&s.tags) {
                add[j].1 = true;
            }
            continue;
        }
        if s.empty != Some(false) && farms.is_farm(&s.ip) {
            continue;
        }
        added.insert(key, add.len());
        add.push((s, false));
    }
    (mark, add)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(ip: &str, port: i64, empty: Option<bool>) -> SteamListed {
        SteamListed {
            ip: ip.into(),
            query_port: port,
            game_port: port - 1,
            name: format!("{ip}:{port}"),
            map: "chernarusplus".into(),
            tags: "battleye,no3rd,external,privHive,shardABC123,lqs0,mod,15:12".into(),
            players: 60,
            max_players: 60,
            password: false,
            secure: true,
            empty,
            ..Default::default()
        }
    }

    fn counted(rows: &[SteamListed]) -> FarmCounter {
        let mut f = FarmCounter::default();
        for r in rows {
            f.add(&r.ip, r.query_port, r.empty);
        }
        f
    }

    fn merge(list: &mut Vec<Server>, rows: &[SteamListed]) -> usize {
        merge_steam(list, rows, &counted(rows))
    }

    fn known(ip: &str, port: i64) -> Server {
        Server {
            endpoint: Endpoint {
                ip: ip.into(),
                port,
            },
            name: "known".into(),
            ..Default::default()
        }
    }

    #[test]
    fn steam_fills_in_what_the_list_lacks_and_keeps_what_it_has() {
        let mut list = vec![known("1.1.1.1", 27016)];
        let added = merge(
            &mut list,
            &[
                listed("1.1.1.1", 27016, Some(false)),
                listed("2.2.2.2", 27016, Some(false)),
                listed("2.2.2.2", 27016, Some(false)),
            ],
        );
        assert_eq!(added, 1);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "known", "the launcher list's data stays");
        assert_eq!(list[1].game_port, 27015);
        assert!(list[1].first_person_only && list[1].battl_eye == Some(true));
        assert_eq!(list[1].time, "15:12");
    }

    #[test]
    fn a_server_the_master_counts_empty_shows_no_players() {
        let mut list = Vec::new();
        merge(&mut list, &[listed("3.3.3.3", 27016, Some(true))]);
        assert_eq!(list[0].players, 0, "its A2S claim of 60 is not believed");
    }

    #[test]
    fn a_farm_address_is_left_out_but_its_counted_players_are_not() {
        let mut rows: Vec<SteamListed> = (0..FARM_ADDRESS as i64)
            .map(|i| listed("6.6.6.6", 30000 + i, Some(true)))
            .collect();
        rows.push(listed("6.6.6.6", 2303, Some(false)));
        // A host with a few empty servers is not a farm.
        rows.extend((0..5).map(|i| listed("7.7.7.7", 27016 + i, Some(true))));
        let mut list = Vec::new();
        merge(&mut list, &rows);
        let farm: Vec<_> = list.iter().filter(|s| s.endpoint.ip == "6.6.6.6").collect();
        assert_eq!(farm.len(), 1);
        assert_eq!(farm[0].endpoint.port, 2303);
        assert_eq!(
            list.iter().filter(|s| s.endpoint.ip == "7.7.7.7").count(),
            5
        );
    }

    #[test]
    fn a_new_merged_list_is_the_same_as_one_merged_in_place() {
        let base = vec![known("1.1.1.1", 27016)];
        let rows = [
            listed("1.1.1.1", 27016, Some(false)),
            listed("2.2.2.2", 27016, Some(true)),
        ];
        let mut in_place = base.clone();
        merge(&mut in_place, &rows);
        assert_eq!(merged_steam(&base, &rows, &counted(&rows)), in_place);
        assert_eq!(base.len(), 1);
    }

    #[test]
    fn official_servers_are_recognised_from_their_tags() {
        let mut official = listed("8.8.8.8", 27016, Some(false));
        official.tags = "battleye,shard000,lqs0,etm5.400000,entm2.170000,20:16".into();
        let mut list = vec![known("9.9.9.9", 27016)];
        let mut also = official.clone();
        also.ip = "9.9.9.9".into();
        merge(&mut list, &[official, also]);
        assert!(list.iter().all(|s| s.official));
    }
}
