//! The Steam client itself: find it and start it. The launcher never closes
//! it: SteamCMD runs beside it with its own install directory.

use dz_common::Result;
use std::path::PathBuf;
use std::process::Stdio;

#[cfg(target_os = "windows")]
use crate::detect::query_steam_registry_path;

/// Steam client management (detect, start).
pub struct SteamClient;

impl SteamClient {
    /// Returns the Steam executable name for the current platform.
    pub fn steam_exe() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "steam.exe"
        }
        #[cfg(not(target_os = "windows"))]
        {
            "steam"
        }
    }

    /// How to run the Steam client: the program, and the arguments that come
    /// before Steam's own. `None` when Steam is nowhere to be found.
    ///
    /// On Windows `steam.exe` lives at `<SteamPath>\steam.exe` (from the
    /// registry) and is *not* on `PATH`. On Linux `steam` is on `PATH` for
    /// most installs, but not for Debian's `/usr/games` in some desktop
    /// sessions, and not at all for Flatpak Steam, which runs through
    /// `flatpak run com.valvesoftware.Steam`.
    pub fn launcher() -> Option<(PathBuf, Vec<String>)> {
        #[cfg(target_os = "windows")]
        {
            // PATH first (rare, but honour it if a user added Steam there).
            if let Ok(p) = which::which("steam.exe") {
                return Some((p, Vec::new()));
            }
            // Registry is the reliable source for non-default install drives.
            if let Some(steam_path) = query_steam_registry_path() {
                let candidate = PathBuf::from(&steam_path).join("steam.exe");
                if candidate.exists() {
                    return Some((candidate, Vec::new()));
                }
            }
            [
                "C:\\Program Files (x86)\\Steam\\steam.exe",
                "C:\\Program Files\\Steam\\steam.exe",
            ]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.exists())
            .map(|p| (p, Vec::new()))
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(p) = which::which("steam") {
                return Some((p, Vec::new()));
            }
            for p in [
                "/usr/games/steam",
                "/usr/bin/steam",
                "/usr/local/bin/steam",
                "/snap/bin/steam",
                "/var/lib/snapd/snap/bin/steam",
            ] {
                let p = PathBuf::from(p);
                if p.is_file() {
                    return Some((p, Vec::new()));
                }
            }
            let flatpak_data = std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".var/app/com.valvesoftware.Steam"));
            if flatpak_data.is_some_and(|d| d.is_dir())
                && let Ok(flatpak) = which::which("flatpak")
            {
                return Some((
                    flatpak,
                    vec!["run".into(), "com.valvesoftware.Steam".into()],
                ));
            }
            None
        }
    }

    /// The error for a machine without a Steam client the launcher can find.
    pub fn not_found() -> dz_common::Error {
        dz_common::Error::Other(
            "Steam was not found. Install the Steam client (or start it once), then try again."
                .into(),
        )
    }

    /// Check if the Steam client is currently running.
    pub fn is_running() -> bool {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
        // Names only. The default refresh also reads each process's memory,
        // CPU, disk use and executable path: on Windows that opens every
        // process in the table, a hundred milliseconds and more per poll,
        // and launching polls this every second while Steam starts.
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        system.processes().values().any(|process| {
            let name = process.name().to_string_lossy();
            // "steam" — native Linux / macOS / Flatpak entry-point.
            // "steam.exe" — Windows and Wine/Proton on Linux.
            // Exclude "steamwebhelper" — it can outlive a crashed client.
            // Case-insensitive compare avoids allocating a lowercased String
            // for every process in the table on every poll.
            name.eq_ignore_ascii_case("steam") || name.eq_ignore_ascii_case("steam.exe")
        })
    }

    /// Start Steam in silent mode (no friends UI).
    ///
    /// Returns `true` if Steam was already running, `false` if it was just
    /// started (so the caller knows whether to wait for it to become ready).
    pub fn start() -> Result<bool> {
        if Self::is_running() {
            return Ok(true);
        }

        let (program, pre) = Self::launcher().ok_or_else(Self::not_found)?;
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // On Windows spawn steam.exe directly; no nohup equivalent needed
            // because detached processes persist after the parent exits.
            std::process::Command::new(program)
                .args(&pre)
                .arg("-nofriendsui")
                .arg("-silent")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(dz_common::CREATE_NO_WINDOW)
                .spawn()?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            // On Linux/macOS use nohup so Steam keeps running if the spawner exits.
            let mut cmd = std::process::Command::new("nohup");
            cmd.arg(program)
                .args(&pre)
                .arg("-nofriendsui")
                .arg("-silent")
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            for v in appimage_env() {
                cmd.env_remove(v);
            }
            cmd.spawn()?;
        }

        Ok(false)
    }
}

/// Variables an AppImage's AppRun sets for the launcher itself. Steam, or a
/// file manager, started from the launcher must not inherit them: they point
/// into the AppImage's mount, which goes away when the launcher closes, and
/// load the AppImage's GTK modules into programs built against the system's.
/// Empty outside an AppImage.
pub fn appimage_env() -> &'static [&'static str] {
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_some() {
        return &[
            "APPDIR",
            "APPIMAGE",
            "ARGV0",
            "OWD",
            "LD_LIBRARY_PATH",
            "GDK_BACKEND",
            "GDK_PIXBUF_MODULE_FILE",
            "GDK_PIXBUF_MODULEDIR",
            "GIO_MODULE_DIR",
            "GIO_EXTRA_MODULES",
            "GSETTINGS_SCHEMA_DIR",
            "GTK_PATH",
            "GTK_EXE_PREFIX",
            "GTK_DATA_PREFIX",
            "GTK_IM_MODULE_FILE",
            "PYTHONHOME",
            "PYTHONPATH",
            "PERLLIB",
            "QT_PLUGIN_PATH",
        ];
    }
    &[]
}

/// The game's own processes, beside the Steam client that starts them: the
/// launcher Steam starts, the BattlEye one and the game itself. On Windows
/// their names say so. Under Proton they do not: the game shows as
/// `enfMain` (the engine's main thread), and only the program at the head of
/// its command line (`S:\common\DayZ\DayZ_x64.exe`) gives it away.
pub struct DayzGame;

impl DayzGame {
    /// Process names that are DayZ, compared without case.
    const NAMES: &'static [&'static str] = &[
        "dayz_x64.exe",
        "dayz_be.exe",
        "dayz.exe",
        "dayzlauncher.exe",
        "dayzdiag_x64.exe",
    ];

    fn is_dayz(name: &std::ffi::OsStr) -> bool {
        let name = name.to_string_lossy();
        Self::NAMES.iter().any(|n| name.eq_ignore_ascii_case(n))
    }

    /// The process is DayZ: by its name, or by the program its command line
    /// starts with (a Windows path under Proton, a Unix one otherwise).
    fn is_game(p: &sysinfo::Process) -> bool {
        Self::is_dayz(p.name())
            || p.cmd().first().is_some_and(|first| {
                let first = first.to_string_lossy();
                let base = first.rsplit(['/', '\\']).next().unwrap_or(&first);
                Self::is_dayz(std::ffi::OsStr::new(base))
            })
    }

    fn processes() -> sysinfo::System {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
        let mut system = System::new();
        // Names only on Windows, where they suffice (and reading command
        // lines opens every process: see `SteamClient::is_running`). On Unix
        // the command lines too, for Proton's games.
        let kind = ProcessRefreshKind::nothing();
        #[cfg(not(windows))]
        let kind = kind.with_cmd(sysinfo::UpdateKind::OnlyIfNotSet);
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        system
    }

    /// DayZ is running. Blocking (a scan of the process table).
    pub fn is_running() -> bool {
        Self::processes().processes().values().any(Self::is_game)
    }

    /// Close DayZ: asked to quit first where the system allows it, then
    /// killed if it is still there a moment later. Returns how many processes
    /// were stopped. Blocking.
    pub fn kill() -> usize {
        let system = Self::processes();
        let targets: Vec<_> = system
            .processes()
            .iter()
            .filter(|(_, p)| Self::is_game(p))
            .map(|(pid, _)| *pid)
            .collect();
        if targets.is_empty() {
            return 0;
        }
        // SIGTERM on Unix lets Wine and the game close cleanly; Windows has
        // no such request, so `kill_with` returns None and it is killed below.
        for pid in &targets {
            if let Some(p) = system.process(*pid) {
                let _ = p.kill_with(sysinfo::Signal::Term);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let system = Self::processes();
        for pid in &targets {
            if let Some(p) = system.process(*pid)
                && Self::is_game(p)
            {
                p.kill();
            }
        }
        targets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::DayzGame;
    use std::ffi::OsStr;

    /// By hand, with DayZ running: `cargo test -p dz-steamcmd -- --ignored running`.
    #[test]
    #[ignore = "reads this machine's processes"]
    fn running_here() {
        println!("running: {}", DayzGame::is_running());
    }

    #[test]
    fn only_the_games_processes_count() {
        assert!(DayzGame::is_dayz(OsStr::new("DayZ_x64.exe")));
        assert!(DayzGame::is_dayz(OsStr::new("DAYZ_BE.EXE")));
        assert!(!DayzGame::is_dayz(OsStr::new("dayz-community-hub")));
        assert!(!DayzGame::is_dayz(OsStr::new("DayZServer_x64.exe")));
    }
}
