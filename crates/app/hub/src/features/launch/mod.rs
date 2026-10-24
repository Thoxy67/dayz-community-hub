//! Joining a server through Steam, and the DayZ launch options.

pub mod args;

use crate::{Hub, HubEvent};
use dz_api::{Ipv4Addr, Server, SocketAddrV4};
use dz_game::DayzCtl;

use crate::state::mutate_profile;

/// Link the server's mods, hand Steam the launch, then record the server in
/// the history and tell the window how it went. `host` is the name typed for
/// a server the list does not know: the history and the play stats keep it in
/// place of the IP it resolved to, so a rejoin follows the name. `typed` is
/// what was typed to join, listed or not: a favourite saved under that name
/// gives the launch its own character name and arguments.
fn spawn_launch(
    app: Hub,
    mut ctl: DayzCtl,
    server: Server,
    host: Option<String>,
    typed: Option<&str>,
    password: Option<String>,
    extra: Vec<String>,
) {
    let extra = per_server(&mut ctl, &server, typed, &extra);
    let joined_mods = joined_mods(&server, &extra, host.is_none());
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
                let host = host.unwrap_or_else(|| server.ip().to_string());
                // Recorded on the profile the app keeps, not the task's copy,
                // so the window sees the entry and the next save keeps it.
                // An unknown game port cannot be joined again: not recorded.
                if let Some(join) = server.join_addr()
                    && let Ok(state) = app.state()
                {
                    let _ = mutate_profile(&state, |s| {
                        let p = s.ctl.profile_mut();
                        p.add_history(server.name.clone(), host.clone(), join.port());
                        // What it needs while the list does not show it (the
                        // unused-mods filter keeps these); left unknown when
                        // nothing says.
                        if let Some(ids) = joined_mods {
                            p.set_history_mods(&host, join.port(), ids);
                        }
                        Ok(())
                    })
                    .await;
                }
                // The game that appears next is this server, for the play stats.
                crate::features::stats::tracker::expect(
                    crate::features::stats::tracker::Expected {
                        kind: dz_profile::SessionKind::Server,
                        name: server.name.clone(),
                        ip: Some(host),
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

/// The mods a join carried, for the history: the server's own and those the
/// extra arguments name (`-mod=@1;@2`, the way Direct Connect passes an
/// unlisted server's mods). `None` when there are none and the server is not
/// `listed`: its mods are unknown, not known to be none, and the unused-mods
/// filter must not take an unlisted server for one that runs no mods.
fn joined_mods(server: &Server, extra: &[String], listed: bool) -> Option<Vec<u64>> {
    let mut ids = server.mod_ids();
    for arg in extra {
        let Some((key, value)) = arg.split_once('=') else {
            continue;
        };
        if !key.eq_ignore_ascii_case("-mod") {
            continue;
        }
        for id in value.split(';') {
            let id = id.trim().trim_start_matches('@');
            if let Ok(id) = id.parse::<u64>()
                && !ids.contains(&id)
            {
                ids.push(id);
            }
        }
    }
    (listed || !ids.is_empty()).then_some(ids)
}

/// A favourite's own character name and launch arguments over the global
/// ones ([`args::merge`]), on the task's copy of the profile: the merged
/// arguments are returned to go as the launch's extra arguments, and the
/// options they already hold are switched off on the copy so none goes twice.
fn per_server(
    ctl: &mut DayzCtl,
    server: &Server,
    typed: Option<&str>,
    extra: &[String],
) -> Vec<String> {
    apply_per_server(ctl.profile_mut(), server, typed, extra)
}

/// [`per_server`] on a profile: the favourite is looked up under the name
/// typed (a host name saved from Direct Connect) and under the server's IP,
/// with its game or query port.
fn apply_per_server(
    p: &mut dz_profile::Profile,
    server: &Server,
    typed: Option<&str>,
    extra: &[String],
) -> Vec<String> {
    let ip = server.ip().to_string();
    let hosts: Vec<&str> = typed
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .into_iter()
        .chain([ip.as_str()])
        .collect();
    let ports = [server.game_port, server.query_port()];
    let fav = args::favorite_for(&p.favorites, &hosts, &ports);
    let merged = args::merge(
        &p.options.to_args(),
        fav.map_or("", |f| f.launch_args.as_str()),
        extra,
        fav.and_then(|f| f.character.as_deref()),
        p.player.as_deref(),
    );
    p.player = merged.name;
    for (_, o) in p.options.all_options_mut() {
        o.enabled = false;
    }
    merged.args
}

/// How long a name may take to resolve before the join gives up: the system
/// resolver has no limit of its own, and the toast says "launching" meanwhile.
const RESOLVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The IPv4 `host` names: itself when it is one, else the first IPv4 its
/// name resolves to.
async fn resolve_v4(host: &str) -> Result<Ipv4Addr, String> {
    if let Ok(ip) = host.parse() {
        return Ok(ip);
    }
    let found = tokio::time::timeout(RESOLVE_TIMEOUT, tokio::net::lookup_host((host, 0)))
        .await
        .map_err(|_| format!("{host}: no answer from the name lookup"))?
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
        spawn_launch(app, ctl, server, None, None, password, vec![]);
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
        let host = host.trim();
        let join = SocketAddrV4::new(resolve_v4(host).await?, game_port);
        let state = self.state()?;
        let app = self.clone();
        let (server, typed, ctl) = {
            let st = state.read().await;
            let (server, typed) = match st.find_flexible(join) {
                Some(s) => (s.clone(), None),
                // Not listed: named as typed, the way the Recent list shows it.
                None => {
                    let mut s = Server::unlisted(join);
                    s.name = format!("{host}:{game_port}");
                    (s, Some(host.to_string()))
                }
            };
            (server, typed, st.ctl.clone_for_task())
        };
        spawn_launch(
            app,
            ctl,
            server,
            typed,
            Some(host),
            password,
            extra_args.unwrap_or_default(),
        );
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use dz_profile::{Favorite, Profile};

    fn listed() -> Server {
        let mut s = Server::unlisted(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 7), 2302));
        s.name = "Listed".into();
        s
    }

    #[test]
    fn the_history_keeps_the_mods_a_join_carried() {
        let s = listed();
        let v = |a: &[&str]| a.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
        // A listed server without mods is known to run none.
        assert_eq!(joined_mods(&s, &[], true), Some(vec![]));
        // An unlisted one without any is unknown.
        assert_eq!(joined_mods(&s, &v(&["-nosplash"]), false), None);
        // Direct Connect's mods arrive as `-mod=` arguments.
        assert_eq!(
            joined_mods(&s, &v(&["-mod=@12", "-MOD=@34;@12", "-mod=junk"]), false),
            Some(vec![12, 34])
        );
    }

    fn fav(ip: &str, port: u16, character: Option<&str>, args: &str) -> Favorite {
        Favorite {
            name: "Fav".into(),
            ip: ip.into(),
            port,
            character: character.map(str::to_string),
            launch_args: args.into(),
            ..Default::default()
        }
    }

    fn profile() -> Profile {
        let mut p = Profile {
            player: Some("Global".into()),
            ..Default::default()
        };
        p.options.window.enabled = true;
        p
    }

    #[test]
    fn a_host_name_favourite_applies_to_its_listed_server() {
        let mut p = profile();
        p.favorites
            .push(fav("play.example.net", 2302, Some("Hostie"), "-hostonly=1"));
        let out = apply_per_server(&mut p, &listed(), Some(" play.example.net "), &[]);
        assert_eq!(p.player.as_deref(), Some("Hostie"));
        assert!(out.iter().any(|a| a == "-hostonly=1"));
        // Without the typed name the IP finds nothing: the global name stays.
        let mut p2 = profile();
        p2.favorites
            .push(fav("play.example.net", 2302, Some("Hostie"), "-hostonly=1"));
        let out = apply_per_server(&mut p2, &listed(), None, &[]);
        assert_eq!(p2.player.as_deref(), Some("Global"));
        assert!(!out.iter().any(|a| a == "-hostonly=1"));
    }

    #[test]
    fn options_go_once_and_only_on_the_copy() {
        let base = {
            let mut p = profile();
            p.favorites
                .push(fav("10.0.0.7", 2302, Some("Local"), "-window -cpuCount=4"));
            p
        };
        let mut copy = base.clone();
        let out = apply_per_server(&mut copy, &listed(), None, &["-extra".into()]);
        assert_eq!(copy.player.as_deref(), Some("Local"));
        assert_eq!(
            out.iter()
                .filter(|a| a.eq_ignore_ascii_case("-window"))
                .count(),
            1
        );
        assert!(out.iter().any(|a| a == "-cpuCount=4"));
        assert_eq!(out.last().map(String::as_str), Some("-extra"));
        // The copy's options are off, so the launch does not add them again...
        assert!(copy.options.to_args().is_empty());
        // ...and the profile it was copied from keeps its own.
        assert!(base.options.window.enabled);
        assert_eq!(base.player.as_deref(), Some("Global"));
    }

    #[test]
    fn no_favourite_keeps_the_global_options_and_name() {
        let mut p = profile();
        let out = apply_per_server(&mut p, &listed(), None, &[]);
        assert_eq!(p.player.as_deref(), Some("Global"));
        assert!(out.iter().any(|a| a.eq_ignore_ascii_case("-window")));
    }
}
