use dayz_community_hub_core::{api::Server, ctl::DayzCtl, offline::OfflineMode};
use tauri::{AppHandle, Emitter};

use crate::state::SharedState;

/// Run a blocking closure on the tokio blocking pool and flatten the two
/// error layers (`JoinError` + inner `E`) into a single `String`.
/// Eliminates the repeated `.await.map_err(…)?.map_err(…)` pattern.
pub(crate) async fn spawn_blocking_mapped<T, E>(
    f: impl FnOnce() -> std::result::Result<T, E> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
    E: ToString + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())
}

/// Build an `OfflineMode` from the shared state. Extracts the DayZ path and
/// HTTP client in a single lock, then releases the lock before any I/O.
pub(crate) async fn offline_mode_from_state(state: &SharedState) -> Result<OfflineMode, String> {
    let (dayz_path, client) = {
        let s = state.read().await;
        let path = s.ctl.dayz_path().map_err(|e| e.to_string())?;
        let client = s.ctl.http_client().clone();
        (path, client)
    };
    Ok(OfflineMode::new(dayz_path, client))
}

/// Find a server in the list by IP and port (exact match on query port).
pub(crate) fn find_server_in<'a>(servers: &'a [Server], ip: &str, port: i64) -> Option<&'a Server> {
    servers
        .iter()
        .find(|s| s.endpoint.ip == ip && s.endpoint.port == port)
}

/// Find a server by IP, matching either query port or game port (flexible).
pub(crate) fn find_server_flexible_in<'a>(
    servers: &'a [Server],
    ip: &str,
    port: i64,
) -> Option<&'a Server> {
    servers
        .iter()
        .find(|s| s.endpoint.ip == ip && (s.endpoint.port == port || s.game_port == port))
}

/// Spawn a background game-launch task that emits "launch-done" / "launch-error".
/// Shared by `launch_server` and `launch_direct`.
pub(crate) fn spawn_launch(
    app: AppHandle,
    mut ctl: DayzCtl,
    server: Server,
    password: Option<String>,
    extra: Vec<String>,
) {
    tokio::spawn(async move {
        match ctl
            .launch_game_with_extra(&server, password.as_deref(), &extra)
            .await
        {
            Ok(_) => {
                let _ = app.emit("launch-done", server.name.clone());
            }
            Err(e) => {
                let _ = app.emit("launch-error", e.to_string());
            }
        }
    });
}

/// Deduplicate a server list by (ip, query_port), keeping the first occurrence.
pub(crate) fn dedup_servers(
    mut servers: Vec<dayz_community_hub_core::Server>,
) -> Vec<dayz_community_hub_core::Server> {
    let mut seen = std::collections::HashSet::new();
    servers.retain(|s| seen.insert((s.endpoint.ip.clone(), s.endpoint.port)));
    servers
}
