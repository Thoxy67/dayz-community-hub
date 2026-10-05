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
                // A port outside 1..=65535 cannot be joined again: not recorded.
                if let Ok(port) = u16::try_from(server.game_port)
                    && port != 0
                {
                    let state = app.state::<SharedState>();
                    let _ = mutate_profile(&state, |s| {
                        s.ctl.profile_mut().add_history(
                            server.name.clone(),
                            server.endpoint.ip.clone(),
                            port,
                        );
                        Ok(())
                    })
                    .await;
                }
                let _ = LaunchDone(server.name).emit(&app);
            }
            Err(e) => {
                let _ = LaunchError(e.to_string()).emit(&app);
            }
        }
    });
}

/// How DayZ gets started, for the launch options page: what the launcher
/// runs, and what the Steam client adds of its own.
#[derive(serde::Serialize, Clone, Debug, specta::Type)]
pub struct SteamLaunchInfoDto {
    /// Built for Linux: DayZ (a Windows game) runs under Proton there.
    pub linux: bool,
    /// The program the launcher runs, with its own first arguments
    /// (`["/usr/bin/steam"]`, `["flatpak", "run", "com.valvesoftware.Steam"]`);
    /// empty when Steam is not found.
    pub launcher: Vec<String>,
    /// The arguments every launch starts with (`-applaunch 221100 …`), before
    /// the server and the options.
    pub applaunch: Vec<String>,
    /// DayZ's launch options in Steam's own properties, with `%command%`.
    pub launch_options: Option<String>,
    /// The Proton (or other) tool DayZ runs under on Linux.
    pub compat_tool: Option<String>,
    /// That tool is Steam's default rather than one chosen for DayZ.
    pub compat_tool_default: bool,
    /// DayZ's Proton prefix (`steamapps/compatdata/221100`), when it exists:
    /// its Windows drive, documents and saves.
    pub prefix: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn steam_launch_info(
    state: State<'_, SharedState>,
) -> Result<SteamLaunchInfoDto, String> {
    let (player, dayz) = {
        let s = state.read().await;
        (s.ctl.profile().player.clone(), s.ctl.dayz_path().ok())
    };
    tokio::task::spawn_blocking(move || {
        // <steamapps>/common/DayZ → <steamapps>/compatdata/221100
        let prefix = dayz
            .as_deref()
            .and_then(|d| {
                d.parent()?
                    .parent()
                    .map(|sa| sa.join("compatdata").join("221100"))
            })
            .filter(|p| p.is_dir())
            .map(|p| p.to_string_lossy().into_owned());
        let launcher = dz_steamcmd::SteamClient::launcher()
            .map(|(program, pre)| {
                std::iter::once(program.to_string_lossy().into_owned())
                    .chain(pre)
                    .collect()
            })
            .unwrap_or_default();
        let applaunch = dz_game::launch::build_steam_applaunch_args(
            dz_steamcmd::DAYZ_GAME_ID,
            &[],
            player.as_deref(),
        );
        let steam = dz_steamcmd::dayz_steam_config();
        SteamLaunchInfoDto {
            linux: cfg!(target_os = "linux"),
            launcher,
            applaunch,
            launch_options: steam.launch_options,
            compat_tool: steam.compat_tool,
            compat_tool_default: steam.compat_tool_default,
            prefix,
        }
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))
}
