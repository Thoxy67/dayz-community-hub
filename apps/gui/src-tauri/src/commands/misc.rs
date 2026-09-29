use tauri::State;

use crate::convert::server_to_slim_dto;
use crate::dto::{ServerSlimDto, SystemSpecsDto};
use crate::helpers::spawn_blocking_mapped;
use crate::state::SharedState;

/// Detect the machine's CPU/RAM specs so the UI can recommend tuned DayZ launch
/// options (`-cpuCount`, `-exThreads`, `-maxMem`, …).
#[tauri::command]
pub(crate) async fn get_system_specs() -> Result<SystemSpecsDto, String> {
    // Logical cores: cheap and reliable via std, no sysinfo refresh needed.
    let logical_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1) as u32;

    // Physical cores is a static query; memory needs a (fast) memory refresh.
    let physical_cores = sysinfo::System::physical_core_count()
        .map(|c| c as u32)
        .unwrap_or(logical_cores)
        .max(1);

    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let total_memory_mb = sys.total_memory() / (1024 * 1024); // bytes → MB

    Ok(SystemSpecsDto {
        logical_cores: logical_cores.max(1),
        physical_cores,
        total_memory_mb,
    })
}

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
        let server = state
            .find_by_query_port(&ip, port)
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
    Ok(state
        .find_flexible(&ip, port as i64)
        .map(server_to_slim_dto))
}
