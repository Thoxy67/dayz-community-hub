use dayz_community_hub_core::api::Server;
use tauri::{AppHandle, State};

use crate::helpers::{find_server_flexible_in, find_server_in, spawn_launch};
use crate::state::SharedState;
use crate::utils::error::ResultExt;

/// Toggle a launch option.
#[tauri::command]
pub(crate) async fn toggle_launch_option(
    key: String,
    state: State<'_, SharedState>,
) -> Result<bool, String> {
    let mut state = state.write().await;
    let options = &mut state.ctl.profile_mut().options;
    let mut all = options.all_options_mut();
    if let Some((_, opt)) = all.iter_mut().find(|(k, _)| *k == key) {
        opt.enabled = !opt.enabled;
        let new_state = opt.enabled;
        drop(all);
        state.ctl.save_profile_async().await.cmd_err()?;
        Ok(new_state)
    } else {
        Err(format!("Unknown option: {}", key))
    }
}

/// Set a launch option value.
#[tauri::command]
pub(crate) async fn set_launch_option_value(
    key: String,
    value: Option<String>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.write().await;
    let options = &mut state.ctl.profile_mut().options;
    let mut all = options.all_options_mut();
    if let Some((_, opt)) = all.iter_mut().find(|(k, _)| *k == key) {
        opt.value = value.clone();
        if value.is_some() {
            opt.enabled = true;
        }
        drop(all);
        state.ctl.save_profile_async().await.cmd_err()
    } else {
        Err(format!("Unknown option: {}", key))
    }
}

/// Launch a game server. Emits "launch-done" or "launch-error" event.
#[tauri::command]
pub(crate) async fn launch_server(
    ip: String,
    port: i64,
    password: Option<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (server, ctl_clone) = {
        let state = state.read().await;
        let server = find_server_in(&state.servers, &ip, port)
            .cloned()
            .ok_or_else(|| "Server not found".to_string())?;
        (server, state.ctl.clone_for_launch())
    };

    spawn_launch(app, ctl_clone, server, password, vec![]);

    Ok(())
}

/// Launch with a direct ip:port.
#[tauri::command]
pub(crate) async fn launch_direct(
    ip: String,
    game_port: u16,
    password: Option<String>,
    extra_args: Option<Vec<String>>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let extra = extra_args.unwrap_or_default();
    let (server, ctl_clone) = {
        let st = state.read().await;
        let server = find_server_flexible_in(&st.servers, &ip, game_port as i64)
            .cloned()
            .unwrap_or_else(|| Server {
                endpoint: dayz_community_hub_core::Endpoint {
                    ip: ip.clone(),
                    port: game_port as i64,
                },
                name: format!("{}:{}", ip, game_port),
                game_port: game_port as i64,
                mods: vec![],
                ..Default::default()
            });

        if !server.mods.is_empty() {
            let _ = st.ctl.setup_mod_symlinks(&server);
        }

        (server, st.ctl.clone_for_launch())
    };

    spawn_launch(app, ctl_clone, server, password, extra);

    Ok(())
}
