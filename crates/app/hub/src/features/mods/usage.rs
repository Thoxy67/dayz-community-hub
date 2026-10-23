//! Which installed mods the saved servers still need, for the unused-mods
//! filter and "remove this server's mods".
//!
//! **Unused**, precisely: an installed mod is unused when
//! 1. no favourite and no server of the history runs it, by the server list's
//!    mod list for that server, else (history) the mods it ran when it was
//!    last joined from here;
//! 2. it is not one of the mods chosen for the offline mode;
//! 3. no mod kept by 1 or 2 requires it on the Workshop (transitively);
//! 4. and every favourite and history server's mods are known: while one of
//!    them is in neither the list nor the history's record, nothing is called
//!    unused (that server might need any of them), and the servers to blame
//!    are named instead.
//!
//! "Remove this server's mods" removes the server's installed mods that the
//! same rules keep for no *other* saved server; the servers whose mods are
//! unknown are named in the question, since they could need what goes.

use dz_api::SocketAddrV4;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;

use crate::Hub;

/// A favourite or a server played recently, with the mods it runs when
/// they are known.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct SavedServerModsDto {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub favorite: bool,
    pub history: bool,
    /// The query address of the listed server it is.
    pub listed: Option<SocketAddrV4>,
    /// Workshop ids it runs; `None` when unknown (not listed, never joined
    /// from here since mods were remembered).
    pub mods: Option<Vec<u64>>,
}

/// The saved servers' mods and the offline mode's.
#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedModsDto {
    pub servers: Vec<SavedServerModsDto>,
    /// The mods chosen for offline missions.
    pub offline: Vec<u64>,
}

/// Why a mod stays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeepReason {
    /// This saved server runs it (its name).
    Server(String),
    /// Chosen for the offline mode.
    Offline,
    /// A kept mod requires it (that mod's id).
    Required(u64),
}

/// The installed mods nothing needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unused {
    /// Sorted by id; empty while `unknown` is not.
    pub ids: Vec<u64>,
    /// Saved servers whose mods are not known: until they are, nothing is unused.
    pub unknown: Vec<String>,
}

/// What removing one server's mods does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerRemoval {
    /// Its installed mods no other saved server, nor the offline mode, needs.
    pub remove: Vec<u64>,
    /// Its installed mods that stay, and why.
    pub keep: Vec<(u64, KeepReason)>,
    /// Other saved servers whose mods are not known.
    pub unknown: Vec<String>,
}

/// The server whose mods are being removed, as the window knows it: every
/// saved entry that is this server (by the list, by its address, or a
/// history entry kept under a host name) does not keep its mods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerRef {
    /// Its query address in the server list.
    pub listed: SocketAddrV4,
    /// Its address as text and its game port, as a favourite saves them.
    pub ip: String,
    pub port: u16,
    pub name: String,
}

impl ServerRef {
    /// Whether the saved entry `s` is this server.
    fn is(&self, s: &SavedServerModsDto) -> bool {
        s.listed == Some(self.listed)
            || (s.port == self.port && s.ip == self.ip)
            // Joined by a host name: not matched to the list, but the same
            // game port and the name it had when joined.
            || (s.listed.is_none() && s.port == self.port && s.name == self.name)
    }
}

/// The mods kept, with the first reason found, and the servers whose mods
/// are unknown; the server `except` does not count.
fn kept(
    saved: &SavedModsDto,
    except: Option<&ServerRef>,
    requires: &dyn Fn(u64) -> Vec<u64>,
) -> (FxHashMap<u64, KeepReason>, Vec<String>) {
    let mut keep: FxHashMap<u64, KeepReason> = FxHashMap::default();
    let mut unknown = Vec::new();
    for s in &saved.servers {
        if except.is_some_and(|e| e.is(s)) {
            continue;
        }
        match &s.mods {
            Some(ids) => {
                for id in ids {
                    keep.entry(*id)
                        .or_insert_with(|| KeepReason::Server(s.name.clone()));
                }
            }
            None => {
                if !unknown.contains(&s.name) {
                    unknown.push(s.name.clone());
                }
            }
        }
    }
    for id in &saved.offline {
        keep.entry(*id).or_insert(KeepReason::Offline);
    }
    // What the kept mods require stays with them, however deep.
    let mut todo: Vec<u64> = keep.keys().copied().collect();
    let mut seen: FxHashSet<u64> = todo.iter().copied().collect();
    while let Some(id) = todo.pop() {
        for dep in requires(id) {
            if seen.insert(dep) {
                keep.entry(dep).or_insert(KeepReason::Required(id));
                todo.push(dep);
            }
        }
    }
    (keep, unknown)
}

/// The installed mods (`installed`) nothing needs (see the module's
/// definition). `requires` gives a mod's Workshop requirements.
pub fn unused_mods(
    installed: &[u64],
    saved: &SavedModsDto,
    requires: &dyn Fn(u64) -> Vec<u64>,
) -> Unused {
    let (keep, unknown) = kept(saved, None, requires);
    if !unknown.is_empty() {
        return Unused {
            ids: Vec::new(),
            unknown,
        };
    }
    let mut ids: Vec<u64> = installed
        .iter()
        .copied()
        .filter(|id| !keep.contains_key(id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    Unused { ids, unknown }
}

/// What removing the mods of the server `this` (running `server_mods`)
/// would remove and keep, of the mods installed.
pub fn server_removal(
    server_mods: &[u64],
    this: &ServerRef,
    installed: &FxHashSet<u64>,
    saved: &SavedModsDto,
    requires: &dyn Fn(u64) -> Vec<u64>,
) -> ServerRemoval {
    let (keep, unknown) = kept(saved, Some(this), requires);
    let mut out = ServerRemoval {
        unknown,
        ..ServerRemoval::default()
    };
    let mut seen: FxHashSet<u64> = FxHashSet::default();
    for id in server_mods {
        if !installed.contains(id) || !seen.insert(*id) {
            continue;
        }
        match keep.get(id) {
            Some(why) => out.keep.push((*id, why.clone())),
            None => out.remove.push(*id),
        }
    }
    out
}

impl Hub {
    /// Every favourite and history server with the mods it runs (from the
    /// server list, else what the history remembers of its last join), and
    /// the mods chosen for the offline mode.
    pub async fn saved_server_mods(&self) -> Result<SavedModsDto, String> {
        mocked!(self, w => w.saved_mods_now(self).await);
        let state = self.state()?;
        let s = state.read().await;
        let p = s.ctl.profile();
        let mut out = SavedModsDto {
            servers: Vec::with_capacity(p.favorites.len() + p.history.len()),
            offline: p.offline_mods.clone(),
        };
        // `remembered`: the mods a history entry recorded at its last join
        // (`Some(vec![])` is a server known to run none).
        let mut add =
            |name: &str, ip: &str, port: u16, favorite: bool, remembered: Option<&Vec<u64>>| {
                let listed = dz_api::addr(ip, i64::from(port)).and_then(|a| s.find_flexible(a));
                if let Some(e) = out
                    .servers
                    .iter_mut()
                    .find(|e| e.ip == ip && e.port == port)
                {
                    e.favorite |= favorite;
                    e.history |= !favorite;
                    if e.mods.is_none() {
                        e.mods = remembered.cloned();
                    }
                    return;
                }
                let mods = match listed {
                    Some(srv) => Some(srv.mod_ids()),
                    None => remembered.cloned(),
                };
                out.servers.push(SavedServerModsDto {
                    name: name.to_owned(),
                    ip: ip.to_owned(),
                    port,
                    favorite,
                    history: !favorite,
                    listed: listed.map(|srv| srv.endpoint),
                    mods,
                });
            };
        for f in &p.favorites {
            add(&f.name, &f.ip, f.port, true, None);
        }
        for h in &p.history {
            add(&h.name, &h.ip, h.port, false, h.mod_ids.as_ref());
        }
        Ok(out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn addr(n: u8) -> SocketAddrV4 {
        SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, n), 27016)
    }

    fn server(n: u8, mods: Option<&[u64]>) -> SavedServerModsDto {
        SavedServerModsDto {
            name: format!("s{n}"),
            ip: format!("10.0.0.{n}"),
            port: 2302,
            favorite: n.is_multiple_of(2),
            history: !n.is_multiple_of(2),
            listed: mods.is_some().then(|| addr(n)),
            mods: mods.map(<[u64]>::to_vec),
        }
    }

    fn target(n: u8) -> ServerRef {
        ServerRef {
            listed: addr(n),
            ip: format!("10.0.0.{n}"),
            port: 2302,
            name: format!("s{n}"),
        }
    }

    fn no_deps(_: u64) -> Vec<u64> {
        Vec::new()
    }

    #[test]
    fn unused_leaves_out_what_saved_servers_and_offline_need() {
        let saved = SavedModsDto {
            servers: vec![server(1, Some(&[1, 2])), server(2, Some(&[2, 3]))],
            offline: vec![4],
        };
        let u = unused_mods(&[6, 1, 2, 3, 4, 5], &saved, &no_deps);
        assert_eq!(u.ids, vec![5, 6]);
        assert!(u.unknown.is_empty());
    }

    #[test]
    fn nothing_is_unused_while_a_saved_servers_mods_are_unknown() {
        let saved = SavedModsDto {
            servers: vec![server(1, Some(&[1])), server(3, None)],
            offline: Vec::new(),
        };
        let u = unused_mods(&[1, 2], &saved, &no_deps);
        assert!(u.ids.is_empty());
        assert_eq!(u.unknown, vec!["s3".to_string()]);
    }

    #[test]
    fn requirements_of_kept_mods_stay_however_deep() {
        // 1 requires 7, which requires 8; 9 is required by nothing kept.
        let deps = |id: u64| match id {
            1 => vec![7],
            7 => vec![8, 1],
            5 => vec![9],
            _ => Vec::new(),
        };
        let saved = SavedModsDto {
            servers: vec![server(1, Some(&[1]))],
            offline: Vec::new(),
        };
        assert_eq!(unused_mods(&[1, 5, 7, 8, 9], &saved, &deps).ids, vec![5, 9]);
    }

    #[test]
    fn no_saved_server_leaves_every_mod_but_the_offline_ones_unused() {
        let saved = SavedModsDto {
            servers: Vec::new(),
            offline: vec![2],
        };
        assert_eq!(unused_mods(&[3, 2, 1], &saved, &no_deps).ids, vec![1, 3]);
    }

    #[test]
    fn a_servers_removal_keeps_what_others_need_and_says_why() {
        let saved = SavedModsDto {
            servers: vec![
                server(1, Some(&[1, 2, 3, 4, 5])),
                server(2, Some(&[2])),
                server(3, None),
            ],
            offline: vec![3],
        };
        let deps = |id: u64| if id == 2 { vec![4] } else { Vec::new() };
        let installed: FxHashSet<u64> = [1, 2, 3, 4].into_iter().collect();
        let r = server_removal(&[1, 2, 3, 4, 5, 1], &target(1), &installed, &saved, &deps);
        assert_eq!(r.remove, vec![1], "5 is not installed, 1 only once");
        assert_eq!(
            r.keep,
            vec![
                (2, KeepReason::Server("s2".into())),
                (3, KeepReason::Offline),
                (4, KeepReason::Required(2)),
            ]
        );
        assert_eq!(r.unknown, vec!["s3".to_string()]);
    }

    #[test]
    fn the_server_itself_does_not_keep_its_mods() {
        let saved = SavedModsDto {
            servers: vec![server(1, Some(&[1, 2]))],
            offline: Vec::new(),
        };
        let installed: FxHashSet<u64> = [1, 2].into_iter().collect();
        let r = server_removal(&[1, 2], &target(1), &installed, &saved, &no_deps);
        assert_eq!(r.remove, vec![1, 2]);
        assert!(r.keep.is_empty() && r.unknown.is_empty());
    }

    #[test]
    fn a_history_entry_kept_under_a_host_name_is_the_server_itself() {
        // Joined as "play.example.com": not matched to the list, but its mods
        // were remembered at the join.
        let mut by_host = server(1, Some(&[1, 2]));
        by_host.ip = "play.example.com".into();
        by_host.listed = None;
        let saved = SavedModsDto {
            servers: vec![by_host, server(2, Some(&[2]))],
            offline: Vec::new(),
        };
        let installed: FxHashSet<u64> = [1, 2].into_iter().collect();
        let r = server_removal(&[1, 2], &target(1), &installed, &saved, &no_deps);
        assert_eq!(r.remove, vec![1]);
        assert_eq!(r.keep, vec![(2, KeepReason::Server("s2".into()))]);
    }

    #[test]
    fn a_server_known_to_run_no_mod_does_not_block_unused() {
        let mut vanilla = server(3, Some(&[]));
        vanilla.listed = None;
        let saved = SavedModsDto {
            servers: vec![server(1, Some(&[1])), vanilla],
            offline: Vec::new(),
        };
        let u = unused_mods(&[1, 2], &saved, &no_deps);
        assert_eq!(u.ids, vec![2]);
        assert!(u.unknown.is_empty());
    }
}
