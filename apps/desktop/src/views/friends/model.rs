//! What the Friends list shows, worked out once per answer and search (= the `shown`,
//! `dayz` and `rest` deriveds of `View.svelte`): friends in DayZ clustered by the server
//! they are on, then the other groups in a fixed order, flattened into the rows the list
//! draws (heads, friends, servers) with their heights, so only the ones on screen are
//! drawn.

use std::cmp::Ordering;

use dz_hub::features::friends::{FriendDto, FriendStatus};

/// A group of the list, in its fixed order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Dayz,
    InGame,
    Online,
    Away,
    Offline,
}

impl Group {
    /// Its heading's words key (`friends` dictionary).
    pub fn label(self) -> &'static str {
        match self {
            Group::Dayz => "groupDayz",
            Group::InGame => "groupInGame",
            Group::Online => "groupOnline",
            Group::Away => "groupAway",
            Group::Offline => "groupOffline",
        }
    }
}

/// Friends on one server: the friend rows, then the server's row under them.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub ip: String,
    /// The port friends are on: the listed server's query port, else the game port.
    pub port: u16,
    /// The list knows the server (Steam's answer named it).
    pub listed: bool,
    /// Indexes into the friend list, by name.
    pub who: Vec<usize>,
    /// The first friend's view of the server's name and map, for an unlisted one.
    pub name: Option<String>,
    pub map: Option<String>,
    /// Every friend on it (search or not), for the row's "friends here" badge.
    pub here: Vec<String>,
}

impl Block {
    pub fn key(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

/// One row of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// A group's heading and how many it holds.
    Head(Group, usize),
    /// A friend, by index into the friend list.
    Friend(usize),
    /// The server of a block, by index into [`Model::blocks`].
    Server(usize),
}

/// `h-row`, the sticky heading (`py-1.5` around the section label, its border), the
/// server's row (`LIST_ROW_PX`).
pub const FRIEND_H: f32 = 44.0;
pub const HEAD_H: f32 = 28.0;
pub const SERVER_H: f32 = 48.0;

impl Item {
    pub fn height(self) -> f32 {
        match self {
            Item::Head(..) => HEAD_H,
            Item::Friend(_) => FRIEND_H,
            Item::Server(_) => SERVER_H,
        }
    }
}

/// The list, ready to draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub items: Vec<Item>,
    pub blocks: Vec<Block>,
    /// How many friends the search keeps.
    pub shown: usize,
}

fn by_name(a: &FriendDto, b: &FriendDto) -> Ordering {
    // `localeCompare(…, { sensitivity: "base" })`: case does not count.
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then_with(|| a.name.cmp(&b.name))
}

fn is_away(s: FriendStatus) -> bool {
    matches!(
        s,
        FriendStatus::Away | FriendStatus::Busy | FriendStatus::Snooze
    )
}

/// What a friend's server is called and on which map, from the listed row when the list
/// has it (`server_of` gives it), else from Steam's answer.
pub type ServerOf<'a> = dyn Fn(&str, u16) -> Option<(String, String)> + 'a;

/// Whether `f` matches `q` (trimmed, lower case; empty matches all): its name, its game,
/// its server's name or map.
pub fn matches(f: &FriendDto, q: &str, server_of: &ServerOf) -> bool {
    if q.is_empty() {
        return true;
    }
    let has = |s: &str| s.to_lowercase().contains(q);
    if has(&f.name) || f.game.as_deref().is_some_and(has) {
        return true;
    }
    let Some(s) = &f.server else {
        return false;
    };
    let listed = server_of(&s.ip, s.port);
    let name = listed.as_ref().map(|l| l.0.as_str()).or(s.name.as_deref());
    let map = listed.as_ref().map(|l| l.1.as_str()).or(s.map.as_deref());
    name.is_some_and(has) || map.is_some_and(has)
}

impl Model {
    /// Group `list` under the search `query` (as typed).
    pub fn build(list: &[FriendDto], query: &str, server_of: &ServerOf) -> Model {
        let q = query.trim().to_lowercase();
        let shown: Vec<usize> = (0..list.len())
            .filter(|&i| list.get(i).is_some_and(|f| matches(f, &q, server_of)))
            .collect();
        let get = |i: &usize| list.get(*i);
        let cmp = |a: &usize, b: &usize| match (get(a), get(b)) {
            (Some(a), Some(b)) => by_name(a, b),
            _ => Ordering::Equal,
        };

        // DayZ: blocks by server, then the friends whose server is unknown.
        let mut blocks: Vec<Block> = Vec::new();
        let mut lost: Vec<usize> = Vec::new();
        let mut dayz = 0;
        for &i in &shown {
            let Some(f) = list.get(i) else { continue };
            if !f.in_dayz {
                continue;
            }
            dayz += 1;
            let Some(s) = &f.server else {
                lost.push(i);
                continue;
            };
            match blocks.iter_mut().find(|b| b.ip == s.ip && b.port == s.port) {
                Some(b) => b.who.push(i),
                None => blocks.push(Block {
                    ip: s.ip.clone(),
                    port: s.port,
                    listed: s.name.is_some(),
                    who: vec![i],
                    name: s.name.clone(),
                    map: s.map.clone(),
                    here: Vec::new(),
                }),
            }
        }
        for b in &mut blocks {
            b.who.sort_by(cmp);
            b.here = list
                .iter()
                .filter(|f| {
                    f.in_dayz
                        && f.server
                            .as_ref()
                            .is_some_and(|s| s.ip == b.ip && s.port == b.port)
                })
                .map(|f| f.name.clone())
                .collect();
            // The first friend's view of the server, as the row shows it.
            if let Some(s) = b.who.first().and_then(|i| list.get(*i)?.server.as_ref()) {
                b.name.clone_from(&s.name);
                b.map.clone_from(&s.map);
            }
        }
        blocks.sort_by(|a, b| {
            b.who
                .len()
                .cmp(&a.who.len())
                .then_with(|| match (a.who.first(), b.who.first()) {
                    (Some(x), Some(y)) => cmp(x, y),
                    _ => Ordering::Equal,
                })
        });
        lost.sort_by(cmp);

        let mut items = Vec::new();
        if dayz > 0 {
            items.push(Item::Head(Group::Dayz, dayz));
            for (n, b) in blocks.iter().enumerate() {
                items.extend(b.who.iter().map(|&i| Item::Friend(i)));
                items.push(Item::Server(n));
            }
            items.extend(lost.iter().map(|&i| Item::Friend(i)));
        }

        // The others, in their fixed order.
        let others: Vec<usize> = shown
            .iter()
            .copied()
            .filter(|&i| list.get(i).is_some_and(|f| !f.in_dayz))
            .collect();
        let pick = |keep: &dyn Fn(&FriendDto) -> bool| -> Vec<usize> {
            others
                .iter()
                .copied()
                .filter(|&i| list.get(i).is_some_and(keep))
                .collect()
        };
        let mut in_game = pick(&|f| f.game.is_some());
        in_game.sort_by(cmp);
        let mut online = pick(&|f| f.game.is_none() && f.status == FriendStatus::Online);
        online.sort_by(cmp);
        let mut away = pick(&|f| f.game.is_none() && is_away(f.status));
        away.sort_by(cmp);
        let mut offline = pick(&|f| f.game.is_none() && f.status == FriendStatus::Offline);
        offline.sort_by(|a, b| match (get(a), get(b)) {
            (Some(x), Some(y)) => y
                .last_online
                .unwrap_or(0)
                .cmp(&x.last_online.unwrap_or(0))
                .then_with(|| by_name(x, y)),
            _ => Ordering::Equal,
        });
        for (g, rows) in [
            (Group::InGame, in_game),
            (Group::Online, online),
            (Group::Away, away),
            (Group::Offline, offline),
        ] {
            if rows.is_empty() {
                continue;
            }
            items.push(Item::Head(g, rows.len()));
            items.extend(rows.into_iter().map(Item::Friend));
        }
        Model {
            items,
            blocks,
            shown: shown.len(),
        }
    }

    /// Where each row starts, and the whole height.
    pub fn offsets(&self) -> (Vec<f32>, f32) {
        let mut y = 0.0;
        let tops = self
            .items
            .iter()
            .map(|it| {
                let top = y;
                y += it.height();
                top
            })
            .collect();
        (tops, y)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use dz_hub::features::friends::FriendServerDto;

    use super::*;

    fn friend(
        name: &str,
        status: FriendStatus,
        game: Option<&str>,
        at: Option<(&str, u16)>,
    ) -> FriendDto {
        FriendDto {
            steam_id: name.to_string(),
            name: name.to_string(),
            status,
            avatar: None,
            game: game.map(str::to_string),
            in_dayz: game == Some("DayZ"),
            server: at.map(|(ip, port)| FriendServerDto {
                ip: ip.to_string(),
                port,
                name: (ip != "9.9.9.9").then(|| format!("srv {ip}")),
                map: Some("chernarusplus".to_string()),
                players: None,
                max_players: None,
            }),
            last_online: None,
        }
    }

    fn none(_: &str, _: u16) -> Option<(String, String)> {
        None
    }

    #[test]
    fn groups_cluster_and_order() {
        use FriendStatus as S;
        let list = vec![
            friend("zed", S::Online, Some("DayZ"), Some(("1.1.1.1", 1))),
            friend("Bob", S::Online, Some("DayZ"), Some(("2.2.2.2", 2))),
            friend("amy", S::Online, Some("DayZ"), Some(("2.2.2.2", 2))),
            friend("lost", S::Away, Some("DayZ"), None),
            friend("cs", S::Online, Some("CS2"), None),
            friend("tobi", S::Busy, None, None),
            friend("gus", S::Offline, None, None),
        ];
        let m = Model::build(&list, "", &none);
        assert_eq!(m.shown, 7);
        // The busier server first; its friends by name, case aside.
        assert_eq!(m.blocks[0].ip, "2.2.2.2");
        assert_eq!(m.blocks[0].who, vec![2, 1]);
        assert_eq!(m.blocks[0].here.len(), 2);
        assert_eq!(
            m.items,
            vec![
                Item::Head(Group::Dayz, 4),
                Item::Friend(2),
                Item::Friend(1),
                Item::Server(0),
                Item::Friend(0),
                Item::Server(1),
                Item::Friend(3),
                Item::Head(Group::InGame, 1),
                Item::Friend(4),
                Item::Head(Group::Away, 1),
                Item::Friend(5),
                Item::Head(Group::Offline, 1),
                Item::Friend(6),
            ]
        );
        let (tops, total) = m.offsets();
        assert_eq!(tops.len(), m.items.len());
        assert!(total > 0.0);
    }

    #[test]
    fn search_reads_names_games_and_servers() {
        use FriendStatus as S;
        let list = vec![
            friend("zed", S::Online, Some("DayZ"), Some(("1.1.1.1", 1))),
            friend("cs", S::Online, Some("Counter-Strike"), None),
        ];
        assert_eq!(Model::build(&list, "  COUNTER ", &none).shown, 1);
        assert_eq!(Model::build(&list, "srv 1.1", &none).shown, 1);
        assert_eq!(Model::build(&list, "chernarus", &none).shown, 1);
        let listed = |_: &str, _: u16| Some(("Frontline".to_string(), "enoch".to_string()));
        assert_eq!(Model::build(&list, "frontline", &listed).shown, 1);
        let m = Model::build(&list, "nobody", &none);
        assert_eq!(m.shown, 0);
        assert!(m.items.is_empty());
    }

    #[test]
    fn unlisted_servers_are_marked() {
        let list = vec![friend(
            "yuri",
            FriendStatus::Snooze,
            Some("DayZ"),
            Some(("9.9.9.9", 2302)),
        )];
        let m = Model::build(&list, "", &none);
        assert!(!m.blocks[0].listed);
        assert_eq!(m.blocks[0].key(), "9.9.9.9:2302");
    }
}
