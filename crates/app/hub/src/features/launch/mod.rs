//! Joining a server through Steam, and the DayZ launch options.

use crate::{Hub, HubEvent};
use dz_api::{Ipv4Addr, Server, SocketAddrV4};
use dz_game::DayzCtl;

use crate::state::mutate_profile;

/// Link the server's mods, hand Steam the launch, then record the server in
/// the history and tell the window how it went.
fn spawn_launch(
    app: Hub,
    ctl: DayzCtl,
    server: Server,
    password: Option<String>,
    extra: Vec<String>,
) {
    tokio::spawn(async move {
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
                // An unknown game port cannot be joined again: not recorded.
                if let Some(join) = server.join_addr()
                    && let Ok(state) = app.state()
                {
                    let _ = mutate_profile(&state, |s| {
                        s.ctl.profile_mut().add_history(
                            server.name.clone(),
                            join.ip().to_string(),
                            join.port(),
                        );
                        Ok(())
                    })
                    .await;
                }
                // The game that appears next is this server, for the play stats.
                crate::features::stats::tracker::expect(
                    crate::features::stats::tracker::Expected {
                        kind: dz_profile::SessionKind::Server,
                        name: server.name.clone(),
                        ip: Some(server.ip().to_string()),
                        port: (server.game_port != 0).then_some(server.game_port),
                        map: (!server.map.is_empty()).then(|| server.map.clone()),
                    },
                );
                app.emit(HubEvent::LaunchDone(server.name));
            }
            Err(e) => {
                app.emit(HubEvent::LaunchError(e.to_string()));
            }
        }
    });
}

/// The IPv4 `host` names: itself when it is one, else the first IPv4 its
/// name resolves to.
async fn resolve_v4(host: &str) -> Result<Ipv4Addr, String> {
    let host = host.trim();
    if let Ok(ip) = host.parse() {
        return Ok(ip);
    }
    let found = tokio::net::lookup_host((host, 0))
        .await
        .map_err(|e| format!("{host}: {e}"))?;
    found
        .filter_map(|a| match a {
            std::net::SocketAddr::V4(a) => Some(*a.ip()),
            std::net::SocketAddr::V6(_) => None,
        })
        .next()
        .ok_or_else(|| format!("{host} has no IPv4 address to join"))
}

/// How DayZ gets started, for the launch options page: what the launcher
/// runs, and what the Steam client adds of its own.
#[derive(serde::Serialize, Clone, Debug)]
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

impl Hub {
    /// Flip a launch option. Returns whether it is now enabled.
    pub async fn toggle_launch_option(&self, key: String) -> Result<bool, String> {
        let state = self.state()?;
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
    pub async fn set_launch_option_value(
        &self,
        key: String,
        value: Option<String>,
    ) -> Result<(), String> {
        let state = self.state()?;
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

    /// Join a listed server by its query address. Returns at once; the
    /// outcome arrives as a `launch-done` or `launch-error` event.
    pub async fn launch_server(
        &self,
        addr: SocketAddrV4,
        password: Option<String>,
    ) -> Result<(), String> {
        mocked!(self, w => w.launch(self, addr).await);
        let state = self.state()?;
        let app = self.clone();
        let (server, ctl) = {
            let state = state.read().await;
            let server = state
                .find_by_query_port(addr)
                .cloned()
                .ok_or_else(|| "Server not found".to_string())?;
            (server, state.ctl.clone_for_task())
        };
        spawn_launch(app, ctl, server, password, vec![]);
        Ok(())
    }

    /// Join by address, whether or not the server is listed. `host` is what
    /// was typed: an IPv4, or a name looked up here (DayZ joins an IPv4).
    /// Returns at once; the outcome arrives as a `launch-done` or
    /// `launch-error` event.
    pub async fn launch_direct(
        &self,
        host: String,
        game_port: u16,
        password: Option<String>,
        extra_args: Option<Vec<String>>,
    ) -> Result<(), String> {
        // The mock looks no name up: nothing of it goes over the network.
        mocked!(self, w => {
            let ip = host.trim().parse().map_err(|_| format!("{host}: no IPv4 address"))?;
            w.launch(self, SocketAddrV4::new(ip, game_port)).await
        });
        let join = SocketAddrV4::new(resolve_v4(&host).await?, game_port);
        let state = self.state()?;
        let app = self.clone();
        let (server, ctl) = {
            let st = state.read().await;
            let server = st
                .find_flexible(join)
                .cloned()
                .unwrap_or_else(|| Server::unlisted(join));
            (server, st.ctl.clone_for_task())
        };
        spawn_launch(app, ctl, server, password, extra_args.unwrap_or_default());
        Ok(())
    }

    pub async fn steam_launch_info(&self) -> Result<SteamLaunchInfoDto, String> {
        mocked!(self => Ok(crate::mock::fixtures::steam_launch_info()));
        let state = self.state()?;
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

    /// DayZ is running (started from here or not).
    pub async fn game_running(&self) -> Result<bool, String> {
        mocked!(self => Ok(false));
        tokio::task::spawn_blocking(dz_steamcmd::DayzGame::is_running)
            .await
            .map_err(|e| format!("Task join error: {e}"))
    }

    /// Close DayZ. Returns how many of its processes were stopped.
    pub async fn kill_game(&self) -> Result<u32, String> {
        mocked!(self => Ok(2));
        tokio::task::spawn_blocking(|| dz_steamcmd::DayzGame::kill() as u32)
            .await
            .map_err(|e| format!("Task join error: {e}"))
    }

    /// Where BattlEye stands: whether its service (Windows) or Proton runtime
    /// is there, and whether it runs.
    pub async fn battleye_status(&self) -> Result<dz_game::battleye::BattlEyeStatus, String> {
        mocked!(self => Ok(crate::mock::fixtures::battleye()));
        let state = self.state()?;
        let ctl = state.read().await.ctl.clone_for_task();
        tokio::task::spawn_blocking(move || ctl.battleye_status())
            .await
            .map_err(|e| format!("Task join error: {e}"))
    }

    /// The installed game's version, as servers give theirs (`1.30.164200`), or
    /// `None` when DayZ is not found. Read from the executable once per change
    /// of it (its size and time), so asking often costs a file's metadata.
    pub async fn game_version(&self) -> Result<Option<String>, String> {
        mocked!(self => Ok(Some(crate::mock::fixtures::GAME_VERSION.to_owned())));
        type Seen = Option<((u64, std::time::SystemTime), Option<String>)>;
        static LAST: std::sync::Mutex<Seen> = std::sync::Mutex::new(None);
        let state = self.state()?;
        let dayz = state
            .read()
            .await
            .ctl
            .dayz_path()
            .map_err(|e| e.to_string())?;
        tokio::task::spawn_blocking(move || {
            let exe = dayz.join("DayZ_x64.exe");
            let Ok(meta) = std::fs::metadata(&exe) else {
                return None;
            };
            let key = (meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH));
            if let Ok(last) = LAST.lock()
                && let Some((k, v)) = last.as_ref()
                && *k == key
            {
                return v.clone();
            }
            let v = dz_game::version::installed(&dayz);
            if let Ok(mut last) = LAST.lock() {
                *last = Some((key, v.clone()));
            }
            v
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))
    }

    /// Repair BattlEye: reinstall its service as administrator (a UAC prompt) on
    /// Windows, have Steam install the Proton runtime on Linux. With `clear`,
    /// first delete the game's data outside its folder (DayZ must be closed).
    /// Returns the state after.
    pub async fn repair_battleye(
        &self,
        clear: bool,
    ) -> Result<dz_game::battleye::BattlEyeStatus, String> {
        mocked!(self => Ok(crate::mock::fixtures::battleye()));
        let state = self.state()?;
        let ctl = state.read().await.ctl.clone_for_task();
        tokio::task::spawn_blocking(move || {
            ctl.repair_battleye(clear)?;
            Ok::<_, dz_common::Error>(ctl.battleye_status())
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))?
        .map_err(|e| e.to_string())
    }

    /// The game's data outside its folder (`Documents\DayZ`,
    /// `AppData\Local\DayZ`) that a thorough BattlEye repair deletes.
    pub async fn game_data_dirs(&self) -> Result<Vec<String>, String> {
        mocked!(self => Ok(crate::mock::fixtures::game_data_dirs()));
        let state = self.state()?;
        let ctl = state.read().await.ctl.clone_for_task();
        tokio::task::spawn_blocking(move || {
            ctl.game_data_dirs()
                .into_iter()
                .map(|d| d.to_string_lossy().into_owned())
                .collect()
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))
    }
}
