//! DayZ Community Offline Mode: download and update it, list and launch its
//! missions, remove missions and their saves.

use crate::{Hub, HubEvent};
use dz_game::OfflineMode;

use crate::error::{ResultExt, spawn_blocking_mapped};
use crate::state::SharedState;

/// An `OfflineMode` from the state's DayZ path and HTTP client, taken in one
/// short read lock before any I/O.
async fn offline_mode_from_state(state: &SharedState) -> Result<OfflineMode, String> {
    let (dayz_path, client) = {
        let s = state.read().await;
        (s.ctl.dayz_path().cmd_err()?, s.ctl.http_client().clone())
    };
    Ok(OfflineMode::new(dayz_path, client))
}

/// One mission's save: its size and when it was last written (Unix
/// seconds), that is when the mission was last played.
#[derive(serde::Serialize, Clone, Debug)]
pub struct MissionSaveDto {
    pub mission: String,
    pub bytes: u64,
    pub written: Option<i64>,
}

/// The mods an offline mission loads: those chosen that could be linked, in
/// the order chosen, each once.
fn mods_to_load(chosen: &[u64], linked: &[u64]) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::with_capacity(chosen.len());
    for id in chosen {
        if linked.contains(id) && !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

/// Most mods an offline selection keeps (a hand-edited profile or a
/// control-channel call cannot make the command line grow without end).
const MAX_OFFLINE_MODS: usize = 500;

impl Hub {
    /// The mods chosen for offline missions, in the order chosen.
    pub async fn offline_mods(&self) -> Result<Vec<u64>, String> {
        let state = self.state()?;
        Ok(state.read().await.ctl.profile().offline_mods.clone())
    }

    /// Choose the mods offline missions load (`-mod=`): kept in the profile.
    /// Repeats and zeros are dropped.
    pub async fn set_offline_mods(&self, ids: Vec<u64>) -> Result<(), String> {
        let mut clean: Vec<u64> = Vec::with_capacity(ids.len().min(MAX_OFFLINE_MODS));
        for id in ids {
            if id != 0 && !clean.contains(&id) && clean.len() < MAX_OFFLINE_MODS {
                clean.push(id);
            }
        }
        let state = self.state()?;
        crate::state::mutate_profile(&state, |s| {
            s.ctl.profile_mut().offline_mods = clean;
            Ok(())
        })
        .await
    }

    /// Get available offline missions.
    pub async fn get_offline_missions(&self) -> Result<Vec<String>, String> {
        mocked!(self => Ok(crate::mock::fixtures::offline_missions()));
        let state = self.state()?;
        let om = offline_mode_from_state(&state).await?;
        spawn_blocking_mapped(move || om.get_available_missions()).await
    }

    /// Download/update DayZCommunityOfflineMode.
    pub async fn update_offline_mode(&self) -> Result<(), String> {
        // `DZCH_MOCK=offline-installing`: the download never ends (its screenshots).
        mocked!(self => {
            if !crate::mock::switch("offline-installing") {
                self.emit(crate::HubEvent::OfflineModeUpdated);
            }
            Ok(())
        });
        let state = self.state()?;
        let app = self.clone();
        let om = offline_mode_from_state(&state).await?;
        tokio::spawn(async move {
            match om.update().await {
                Ok(()) => {
                    app.emit(HubEvent::OfflineModeUpdated);
                }
                Err(e) => {
                    app.emit(HubEvent::OfflineModeError(e.to_string()));
                }
            }
        });
        Ok(())
    }

    /// Remove all DayZCommunityOfflineMode mission folders from DayZ/Missions/.
    pub async fn remove_offline_mode(&self) -> Result<usize, String> {
        mocked!(self => Ok(0));
        let state = self.state()?;
        let om = offline_mode_from_state(&state).await?;
        spawn_blocking_mapped(move || om.remove_offline_mode()).await
    }

    /// Delete storage_-1/ save directories inside each offline mission folder.
    pub async fn clear_offline_saves(&self) -> Result<usize, String> {
        mocked!(self => Ok(0));
        let state = self.state()?;
        let om = offline_mode_from_state(&state).await?;
        spawn_blocking_mapped(move || om.clear_offline_saves()).await
    }

    /// The save of each mission that has one.
    pub async fn offline_saves(&self) -> Result<Vec<MissionSaveDto>, String> {
        mocked!(self => Ok(crate::mock::fixtures::offline_saves()));
        let state = self.state()?;
        let om = offline_mode_from_state(&state).await?;
        spawn_blocking_mapped(move || {
            let mut out = Vec::new();
            for mission in om.get_available_missions()? {
                if let Some(save) = om.mission_save(&mission)? {
                    out.push(MissionSaveDto {
                        mission,
                        bytes: save.bytes,
                        written: save.written,
                    });
                }
            }
            Ok::<_, dz_common::Error>(out)
        })
        .await
    }

    /// Delete one mission's save; true when it had one.
    pub async fn clear_mission_save(&self, mission: String) -> Result<bool, String> {
        mocked!(self => Ok(true));
        let state = self.state()?;
        let om = offline_mode_from_state(&state).await?;
        spawn_blocking_mapped(move || om.clear_mission_save(&mission)).await
    }

    /// Remove a single mission.
    pub async fn remove_mission(&self, mission: String) -> Result<(), String> {
        mocked!(self => Ok(()));
        let state = self.state()?;
        let om = offline_mode_from_state(&state).await?;
        spawn_blocking_mapped(move || om.remove_mission(&mission)).await
    }

    /// Open the missions directory in the file manager.
    /// Returns the folder: the window opens it.
    pub async fn open_missions_dir(&self) -> Result<std::path::PathBuf, String> {
        mocked!(self, w => w.folder("missions"));
        let state = self.state()?;
        let om = offline_mode_from_state(&state).await?;
        let path = om.missions_path();
        tokio::fs::create_dir_all(&path).await.cmd_err()?;
        Ok(path)
    }

    /// Launch an offline mission.
    pub async fn launch_offline_mission(&self, mission: String) -> Result<(), String> {
        mocked!(self => {
            self.emit(crate::HubEvent::LaunchDone(mission));
            Ok(())
        });
        let state = self.state()?;
        if !dz_game::offline::is_mission_name(&mission) {
            return Err(format!("Not a mission name: {mission}"));
        }
        let (om, player, chosen, ctl) = {
            let state = state.read().await;
            (
                OfflineMode::new(
                    state.ctl.dayz_path().cmd_err()?,
                    state.ctl.http_client().clone(),
                ),
                state.ctl.profile().player.clone(),
                state.ctl.profile().offline_mods.clone(),
                state.ctl.clone_for_task(),
            )
        };
        // The chosen mods load through their `@<id>` links, as a server's do;
        // one no longer installed (or that cannot be linked) is left out
        // rather than failing the launch.
        let mod_ids = if chosen.is_empty() {
            Vec::new()
        } else {
            let wanted = chosen.clone();
            let linked = tokio::task::spawn_blocking(move || {
                let dayz = ctl.dayz_path()?;
                dz_game::mods::create_mod_symlinks(&ctl.mod_dirs(), &dayz, &wanted)
            })
            .await
            .map_err(|e| format!("Task join error: {e}"))?
            .unwrap_or_default();
            crate::features::mods::mods_changed(&state).await;
            mods_to_load(&chosen, &linked)
        };
        let steam_args = dz_game::launch::build_steam_applaunch_args(
            dz_steamcmd::DAYZ_GAME_ID,
            &om.build_launch_args(&mission, &mod_ids, false),
            player.as_deref(),
        );
        // The same path as joining a server: a cold Steam is waited for.
        dz_game::run_through_steam(steam_args)
            .await
            .map_err(|e| format!("Could not start the mission through Steam: {e}"))?;
        // The game that appears next plays this mission, for the play stats.
        // `DayZCommunityOfflineMode.Enoch`: the map is the part after the dot.
        let map = mission.rsplit('.').next().map(str::to_lowercase);
        crate::features::stats::tracker::expect(crate::features::stats::tracker::Expected {
            kind: dz_profile::SessionKind::Offline,
            name: mission,
            ip: None,
            port: None,
            map,
        });
        Ok(())
    }

    /// Open a specific offline mission's folder in the system file manager.
    /// Returns the folder: the window opens it.
    pub async fn open_mission_dir(&self, mission: String) -> Result<std::path::PathBuf, String> {
        mocked!(self, w => w.folder(&format!("missions/{mission}")));
        let state = self.state()?;
        let path = offline_mode_from_state(&state)
            .await?
            .mission_path(&mission)
            .cmd_err()?;
        Ok(path)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn only_linked_chosen_mods_load_in_the_order_chosen() {
        assert_eq!(mods_to_load(&[3, 1, 2, 3], &[1, 2, 3]), vec![3, 1, 2]);
        assert_eq!(mods_to_load(&[3, 9, 1], &[1, 3]), vec![3, 1]);
        assert!(mods_to_load(&[], &[1]).is_empty());
    }

    #[test]
    fn the_offline_command_line_carries_the_mods_as_the_online_one_does() {
        let om = OfflineMode::new("/games/DayZ", reqwest::Client::new());
        let mission = "DayZCommunityOfflineMode.ChernarusPlus";
        let plain = om.build_launch_args(mission, &[], false);
        assert!(plain.iter().all(|a| !a.starts_with("-mod=")));
        assert_eq!(
            plain.first().map(String::as_str),
            Some("-mission=.\\Missions\\DayZCommunityOfflineMode.ChernarusPlus")
        );
        let ids = mods_to_load(
            &[1_559_212_036, 1_564_026_768],
            &[1_564_026_768, 1_559_212_036],
        );
        let with = om.build_launch_args(mission, &ids, false);
        assert_eq!(with.len(), plain.len() + 1);
        assert_eq!(
            with.last().map(String::as_str),
            Some("-mod=@1559212036;@1564026768")
        );
        // The same `-mod=` an online join writes for these mods.
        let server = dz_api::Server {
            mods: ids
                .iter()
                .map(|id| dz_api::Mod {
                    name: "m".into(),
                    steam_workshop_id: *id as i64,
                })
                .collect(),
            ..Default::default()
        };
        let online = dz_game::launch::build_launch_args(
            &server,
            &server.mod_ids(),
            None,
            &dz_profile::LaunchOptions::defaults(),
            &[],
        );
        assert!(online.iter().any(|a| Some(a) == with.last()));
    }
}
