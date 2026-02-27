use dayz_community_hub_core::offline::OfflineMode;
use tauri::{AppHandle, Emitter, State};

use crate::helpers::{offline_mode_from_state, spawn_blocking_mapped};
use crate::state::SharedState;

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

/// Launch an offline mission.
#[tauri::command]
pub(crate) async fn launch_offline_mission(
    mission: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (dayz_path, client, player) = {
        let state = state.lock().await;
        let path = state.ctl.dayz_path().map_err(|e| e.to_string())?;
        let client = state.ctl.http_client().clone();
        let player = state.ctl.profile().player.clone();
        (path, client, player)
    };

    let om = OfflineMode::new(dayz_path, client);
    let dayz_args = om.build_launch_args(&mission, &[], false);
    let steam_args = dayz_community_hub_core::launch::build_steam_applaunch_args(
        dayz_community_hub_core::steamcmd::DAYZ_GAME_ID,
        &dayz_args,
        player.as_deref(),
    );

    if let Err(e) = dayz_community_hub_core::steamcmd::SteamClient::start() {
        return Err(format!("Could not start Steam: {}", e));
    }

    let steam_bin = dayz_community_hub_core::steamcmd::SteamClient::steam_exe();
    #[cfg(not(target_os = "windows"))]
    std::process::Command::new(steam_bin)
        .args(&steam_args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(steam_bin)
            .args(&steam_args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
