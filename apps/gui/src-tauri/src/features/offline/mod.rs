//! DayZ Community Offline Mode: download and update it, list and launch its
//! missions, remove missions and their saves.

use dz_game::OfflineMode;
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::events::{OfflineModeError, OfflineModeUpdated};
use tauri_plugin_opener::OpenerExt;

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

/// Get available offline missions.
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_offline_missions(
    state: State<'_, SharedState>,
) -> Result<Vec<String>, String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.get_available_missions()).await
}

/// Download/update DayZCommunityOfflineMode.
#[tauri::command]
#[specta::specta]
pub(crate) async fn update_offline_mode(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let om = offline_mode_from_state(state.inner()).await?;
    tokio::spawn(async move {
        match om.update().await {
            Ok(()) => {
                let _ = OfflineModeUpdated.emit(&app);
            }
            Err(e) => {
                let _ = OfflineModeError(e.to_string()).emit(&app);
            }
        }
    });
    Ok(())
}

/// Remove all DayZCommunityOfflineMode mission folders from DayZ/Missions/.
#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_offline_mode(state: State<'_, SharedState>) -> Result<usize, String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.remove_offline_mode()).await
}

/// Delete storage_-1/ save directories inside each offline mission folder.
#[tauri::command]
#[specta::specta]
pub(crate) async fn clear_offline_saves(state: State<'_, SharedState>) -> Result<usize, String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.clear_offline_saves()).await
}

/// One mission's save: its size and when it was last written (Unix
/// seconds), that is when the mission was last played.
#[derive(serde::Serialize, Clone, Debug, specta::Type)]
pub struct MissionSaveDto {
    pub mission: String,
    pub bytes: u64,
    pub written: Option<i64>,
}

/// The save of each mission that has one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn offline_saves(
    state: State<'_, SharedState>,
) -> Result<Vec<MissionSaveDto>, String> {
    let om = offline_mode_from_state(state.inner()).await?;
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
#[tauri::command]
#[specta::specta]
pub(crate) async fn clear_mission_save(
    mission: String,
    state: State<'_, SharedState>,
) -> Result<bool, String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.clear_mission_save(&mission)).await
}

/// Remove a single mission.
#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_mission(
    mission: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.remove_mission(&mission)).await
}

/// Open the missions directory in the file manager.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_missions_dir(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let om = offline_mode_from_state(state.inner()).await?;
    let path = om.missions_path();
    tokio::fs::create_dir_all(&path).await.cmd_err()?;
    app.opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .cmd_err()
}

/// Launch an offline mission.
#[tauri::command]
#[specta::specta]
pub(crate) async fn launch_offline_mission(
    mission: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
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
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_mission_dir(
    app: AppHandle,
    mission: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let path = offline_mode_from_state(state.inner())
        .await?
        .mission_path(&mission)
        .cmd_err()?;
    app.opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .cmd_err()
}
