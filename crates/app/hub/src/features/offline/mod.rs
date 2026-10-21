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

impl Hub {
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
        let (om, player) = {
            let state = state.read().await;
            (
                OfflineMode::new(
                    state.ctl.dayz_path().cmd_err()?,
                    state.ctl.http_client().clone(),
                ),
                state.ctl.profile().player.clone(),
            )
        };
        let steam_args = dz_game::launch::build_steam_applaunch_args(
            dz_steamcmd::DAYZ_GAME_ID,
            &om.build_launch_args(&mission, &[], false),
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
