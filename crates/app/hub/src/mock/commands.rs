//! The mock's answers that read or change the real state: usage from the
//! profile, joins recorded in its history, folders in its data directory.

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
    pub(crate) async fn launch(&self, hub: &Hub, ip: &str, port: i64) -> Result<(), String> {
        let state = hub.state()?;
        let (name, game_port) = {
            let s = state.read().await;
            match s.find_flexible(ip, port) {
                Some(srv) => (srv.name.clone(), srv.game_port),
                None => (format!("{ip}:{port}"), port),
            }
        };
        if let Ok(p) = u16::try_from(game_port)
            && p != 0
        {
            let (n, i) = (name.clone(), ip.to_string());
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
