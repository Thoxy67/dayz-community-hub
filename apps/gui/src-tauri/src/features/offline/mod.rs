//! DayZ Community Offline Mode: download and update it, list and launch its
//! missions, remove missions and their saves.

use dz_game::OfflineMode;
use tauri::{AppHandle, Emitter, State};
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
pub(crate) async fn get_offline_missions(
    state: State<'_, SharedState>,
) -> Result<Vec<String>, String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.get_available_missions()).await
}

/// Download/update DayZCommunityOfflineMode.
#[tauri::command]
pub(crate) async fn update_offline_mode(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let om = offline_mode_from_state(state.inner()).await?;
    tokio::spawn(async move {
        match om.update().await {
            Ok(()) => {
                let _ = app.emit("offline-mode-updated", ());
            }
            Err(e) => {
                let _ = app.emit("offline-mode-error", e.to_string());
            }
        }
    });
    Ok(())
}

/// Remove all DayZCommunityOfflineMode mission folders from DayZ/Missions/.
#[tauri::command]
pub(crate) async fn remove_offline_mode(state: State<'_, SharedState>) -> Result<usize, String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.remove_offline_mode()).await
}

/// Delete storage_-1/ save directories inside each offline mission folder.
#[tauri::command]
pub(crate) async fn clear_offline_saves(state: State<'_, SharedState>) -> Result<usize, String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.clear_offline_saves()).await
}

/// Remove a single mission.
#[tauri::command]
pub(crate) async fn remove_mission(
    mission: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let om = offline_mode_from_state(state.inner()).await?;
    spawn_blocking_mapped(move || om.remove_mission(&mission)).await
}

/// Open the missions directory in the file manager.
#[tauri::command]
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
pub(crate) async fn launch_offline_mission(
    mission: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (dayz_path, client, player) = {
        let state = state.read().await;
        let path = state.ctl.dayz_path().cmd_err()?;
        let client = state.ctl.http_client().clone();
        let player = state.ctl.profile().player.clone();
        (path, client, player)
    };

    let om = OfflineMode::new(dayz_path, client);
    let dayz_args = om.build_launch_args(&mission, &[], false);
    let steam_args = dz_game::launch::build_steam_applaunch_args(
        dz_steamcmd::DAYZ_GAME_ID,
        &dayz_args,
        player.as_deref(),
    );

    // Starting Steam scans the process table: blocking work.
    spawn_blocking_mapped(move || -> Result<(), String> {
        dz_steamcmd::SteamClient::start().map_err(|e| format!("Could not start Steam: {e}"))?;
        let mut cmd = std::process::Command::new(dz_steamcmd::SteamClient::steam_exe_path());
        cmd.args(&steam_args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(dz_common::CREATE_NO_WINDOW);
        }
        cmd.spawn().cmd_err()?;
        Ok(())
    })
    .await
}

/// Open a specific offline mission's folder in the system file manager.
#[tauri::command]
pub(crate) async fn open_mission_dir(
    app: AppHandle,
    mission: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let path = state
        .read()
        .await
        .ctl
        .dayz_path()
        .cmd_err()?
        .join("Missions")
        .join(&mission);
    app.opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .cmd_err()
}

