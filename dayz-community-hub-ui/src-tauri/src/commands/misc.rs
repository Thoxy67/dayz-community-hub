use tauri::State;

use crate::convert::server_to_slim_dto;
use crate::dto::ServerSlimDto;
use crate::helpers::{find_server_flexible_in, find_server_in, spawn_blocking_mapped};
use crate::state::SharedState;

/// Check if a server is in favorites.
#[tauri::command]
pub(crate) async fn is_favorite(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<bool, String> {
    let state = state.read().await;
    Ok(state.ctl.profile().is_favorite(&ip, port))
}

/// Setup mod symlinks for a server.
#[tauri::command]
pub(crate) async fn setup_mod_symlinks(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (server, ctl_clone) = {
        let state = state.read().await;
        let server = find_server_in(&state.servers, &ip, port)
            .cloned()
            .ok_or_else(|| "Server not found".to_string())?;
        (server, state.ctl.clone_for_launch())
    };
    spawn_blocking_mapped(move || ctl_clone.setup_mod_symlinks(&server).map(|_| ())).await
}

/// Find a server in the list by ip. Returns the full DTO if found.
#[tauri::command]
pub(crate) async fn find_server(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<Option<ServerSlimDto>, String> {
    let state = state.read().await;
    Ok(find_server_flexible_in(&state.servers, &ip, port as i64).map(server_to_slim_dto))
}
