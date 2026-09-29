//! Joining a server through Steam, and the DayZ launch options.

use dz_api::Server;
use dz_game::DayzCtl;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::events::{LaunchDone, LaunchError};

use crate::state::{SharedState, mutate_profile};

/// Flip a launch option. Returns whether it is now enabled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn toggle_launch_option(
    key: String,
    state: State<'_, SharedState>,
) -> Result<bool, String> {
    mutate_profile(&state, |s| {
        let opt = s
            .ctl
            .profile_mut()
            .options
            .get_mut(&key)
            .ok_or_else(|| format!("Unknown option: {key}"))?;
        opt.enabled = !opt.enabled;
        Ok(opt.enabled)
    })
    .await
}

/// Set a launch option's value; a value also enables it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_launch_option_value(
    key: String,
    value: Option<String>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    mutate_profile(&state, |s| {
        let opt = s
            .ctl
            .profile_mut()
            .options
            .get_mut(&key)
            .ok_or_else(|| format!("Unknown option: {key}"))?;
        if value.is_some() {
            opt.enabled = true;
        }
        opt.value = value;
        Ok(())
    })
    .await
}

/// Join a listed server by its query port. Returns at once; the outcome
/// arrives as a `launch-done` or `launch-error` event.
#[tauri::command]
#[specta::specta]
pub(crate) async fn launch_server(
    ip: String,
    port: i64,
    password: Option<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (server, ctl) = {
        let state = state.read().await;
        let server = state
            .find_by_query_port(&ip, port)
            .cloned()
            .ok_or_else(|| "Server not found".to_string())?;
        (server, state.ctl.clone_for_task())
    };
    spawn_launch(app, ctl, server, password, vec![]);
    Ok(())
}

/// Join by address, whether or not the server is listed. Returns at once;
/// the outcome arrives as a `launch-done` or `launch-error` event.
#[tauri::command]
#[specta::specta]
pub(crate) async fn launch_direct(
    ip: String,
    game_port: u16,
    password: Option<String>,
    extra_args: Option<Vec<String>>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (server, ctl) = {
        let st = state.read().await;
        let server = st
            .find_flexible(&ip, i64::from(game_port))
            .cloned()
            .unwrap_or_else(|| Server::unlisted(&ip, i64::from(game_port)));
        (server, st.ctl.clone_for_task())
    };
    spawn_launch(app, ctl, server, password, extra_args.unwrap_or_default());
    Ok(())
}

/// Link the server's mods, hand Steam the launch, then record the server in
/// the history and tell the window how it went.
fn spawn_launch(
    app: AppHandle,
    ctl: DayzCtl,
    server: Server,
    password: Option<String>,
    extra: Vec<String>,
) {
    tauri::async_runtime::spawn(async move {
        // The `@<id>` links DayZ loads mods through: without them `-mod=@<id>`
        // points at nothing (notably on Windows, where they are NTFS
        // junctions) and the game starts without its mods. Filesystem work,
        // so off the runtime; a failure still lets the launch go ahead.
        if !server.mods.is_empty() {
            let (c, s) = (ctl.clone_for_task(), server.clone());
            let _ = tokio::task::spawn_blocking(move || c.setup_mod_symlinks(&s)).await;
        }
        match ctl.launch_game(&server, password.as_deref(), &extra).await {
            Ok(()) => {
                // Recorded on the profile the app keeps, not the task's copy,
                // so the window sees the entry and the next save keeps it.
                let state = app.state::<SharedState>();
                let _ = mutate_profile(&state, |s| {
                    s.ctl.profile_mut().add_history(
                        server.name.clone(),
                        server.endpoint.ip.clone(),
                        server.game_port as u16,
                    );
                    Ok(())
                })
                .await;
                let _ = LaunchDone(server.name).emit(&app);
            }
            Err(e) => {
                let _ = LaunchError(e.to_string()).emit(&app);
            }
        }
    });
}
