//! The mock's answers that read or change the real state: usage from the
//! profile, joins recorded in its history, folders in its data directory.

use dz_api::SocketAddrV4;
use std::path::PathBuf;

use crate::features::mods::ModUsageDto;
use crate::state::mutate_profile;
use crate::{Hub, HubEvent};

use super::World;

impl World {
    /// [`World::mods_usage`] with the profile's favourites and last game.
    pub(crate) async fn mods_usage_now(&self, hub: &Hub) -> Result<Vec<ModUsageDto>, String> {
        let state = hub.state()?;
        let s = state.read().await;
        let p = s.ctl.profile();
        let favorites: Vec<String> = p.favorites.iter().map(|f| f.name.clone()).collect();
        Ok(self.mods_usage(&favorites, p.history.first().map(|h| h.name.as_str())))
    }

    /// The saved servers' mods as [`World::mods_usage`] tells them (so the Unused
    /// filter agrees with the rows' "played on" lines): a favourite runs the mods
    /// naming it, the last game the ones naming it, other history servers none.
    pub(crate) async fn saved_mods_now(
        &self,
        hub: &Hub,
    ) -> Result<crate::features::mods::usage::SavedModsDto, String> {
        use crate::features::mods::usage::{SavedModsDto, SavedServerModsDto};
        let state = hub.state()?;
        let s = state.read().await;
        let p = s.ctl.profile();
        let favorites: Vec<String> = p.favorites.iter().map(|f| f.name.clone()).collect();
        let first = p.history.first().map(|h| h.name.clone());
        let usage = self.mods_usage(&favorites, first.as_deref());
        let runs = |name: &str, favorite: bool| -> Vec<u64> {
            usage
                .iter()
                .filter(|u| {
                    if favorite {
                        u.favorites.iter().any(|n| n == name)
                    } else {
                        u.history.iter().any(|n| n == name)
                    }
                })
                .map(|u| u.id)
                .collect()
        };
        let listed = |ip: &str, port: u16| {
            dz_api::addr(ip, i64::from(port))
                .and_then(|a| s.find_flexible(a))
                .map(|srv| srv.endpoint)
        };
        let mut servers: Vec<SavedServerModsDto> = Vec::new();
        for f in &p.favorites {
            servers.push(SavedServerModsDto {
                name: f.name.clone(),
                ip: f.ip.clone(),
                port: f.port,
                favorite: true,
                history: false,
                listed: listed(&f.ip, f.port),
                mods: Some(runs(&f.name, true)),
            });
        }
        for (i, h) in p.history.iter().enumerate() {
            if servers.iter().any(|e| e.ip == h.ip && e.port == h.port) {
                continue;
            }
            servers.push(SavedServerModsDto {
                name: h.name.clone(),
                ip: h.ip.clone(),
                port: h.port,
                favorite: false,
                history: true,
                listed: listed(&h.ip, h.port),
                mods: Some(if i == 0 {
                    runs(&h.name, false)
                } else {
                    Vec::new()
                }),
            });
        }
        Ok(SavedModsDto {
            servers,
            offline: p.offline_mods.clone(),
        })
    }

    /// Of `ids`, those whose copy is in a Steam library: what deleting them
    /// leaves behind.
    pub(crate) fn kept_in_steam(&self, ids: &[u64]) -> Vec<u64> {
        ids.iter()
            .copied()
            .filter(|id| self.mods.iter().any(|m| m.id == *id && m.steam))
            .collect()
    }

    /// Whether a mod is linked after toggling it.
    pub(crate) fn toggled_managed(&self, id: u64) -> bool {
        !self.mods.iter().any(|m| m.id == id && m.managed)
    }

    /// A folder inside the mock's data directory, made if missing: what the
    /// "open folder" commands hand back.
    pub(crate) fn folder(&self, rel: &str) -> Result<PathBuf, String> {
        let dir = self.data_dir().join(rel);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }

    /// A join: recorded in the history as for real, then reported done.
    /// Nothing is started.
    pub(crate) async fn launch(&self, hub: &Hub, addr: SocketAddrV4) -> Result<(), String> {
        let state = hub.state()?;
        let (name, game_port) = {
            let s = state.read().await;
            match s.find_flexible(addr) {
                Some(srv) => (srv.name.clone(), srv.game_port),
                None => (addr.to_string(), addr.port()),
            }
        };
        if game_port != 0 {
            let (n, i, p) = (name.clone(), addr.ip().to_string(), game_port);
            mutate_profile(&state, |s| {
                s.ctl.profile_mut().add_history(n, i, p);
                Ok(())
            })
            .await?;
        }
        hub.emit(HubEvent::LaunchDone(name));
        Ok(())
    }
}
