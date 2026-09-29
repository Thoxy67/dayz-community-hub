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

    /// Resolve the full path to the Steam client executable to spawn.
    ///
    /// On Windows `steam.exe` lives at `<SteamPath>\steam.exe` (from the
    /// registry) and is *not* on `PATH`, so spawning the bare name fails with
    /// "program not found". We therefore resolve the real install location.
    /// On Linux/macOS `steam` is a launcher script that is reliably on `PATH`,
    /// so the bare name is correct.
    pub fn steam_exe_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            // PATH first (rare, but honour it if a user added Steam there).
            if let Ok(p) = which::which("steam.exe") {
                return p;
            }
            // Registry is the reliable source for non-default install drives.
            if let Some(steam_path) = query_steam_registry_path() {
                let candidate = PathBuf::from(&steam_path).join("steam.exe");
                if candidate.exists() {
                    return candidate;
                }
            }
            for base in &[
                "C:\\Program Files (x86)\\Steam\\steam.exe",
                "C:\\Program Files\\Steam\\steam.exe",
            ] {
                let candidate = PathBuf::from(base);
                if candidate.exists() {
                    return candidate;
                }
            }
            // Last resort: bare name so the resulting error is still meaningful.
            PathBuf::from("steam.exe")
        }
        #[cfg(not(target_os = "windows"))]
        {
            PathBuf::from("steam")
        }
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

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // On Windows spawn steam.exe directly; no nohup equivalent needed
            // because detached processes persist after the parent exits.
            std::process::Command::new(Self::steam_exe_path())
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
            std::process::Command::new("nohup")
                .arg(Self::steam_exe_path())
                .arg("-nofriendsui")
                .arg("-silent")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }

        Ok(false)
    }
}
