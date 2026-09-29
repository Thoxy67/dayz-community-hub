//! The Steam client itself: find it, start it, shut it down.

use dz_common::Result;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

#[cfg(target_os = "windows")]
use crate::detect::query_steam_registry_path;

/// Steam client management (detect, start, shutdown).
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
        use sysinfo::System;
        let mut system = System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
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

    /// Graceful shutdown via `steam -shutdown`.
    pub async fn shutdown() -> Result<()> {
        let mut cmd = Command::new(Self::steam_exe_path());
        cmd.arg("-shutdown")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(dz_common::CREATE_NO_WINDOW);
        let _ = cmd.spawn()?.wait().await;
        // Give it time to shut down
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        Ok(())
    }

    /// Force kill all Steam processes.
    pub fn shutdown_force() -> Result<()> {
        use sysinfo::System;
        let mut system = System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        for process in system.processes().values() {
            if is_steam_client_process(&process.name().to_string_lossy()) {
                process.kill();
            }
        }
        Ok(())
    }

    /// [`SteamClient::is_running`] off the async runtime: it scans the whole
    /// process table.
    async fn is_running_async() -> bool {
        tokio::task::spawn_blocking(Self::is_running)
            .await
            .unwrap_or(false)
    }

    /// Shut down Steam gracefully before running steamcmd.
    ///
    /// steamcmd and the Steam client share the same auth session — running both
    /// simultaneously causes Steam to kick you offline. This function:
    ///   1. Does nothing if Steam is not running.
    ///   2. Sends `steam -shutdown` and waits up to 15 s for all processes to exit.
    ///   3. Force-kills any remaining Steam processes if they didn't exit in time.
    pub async fn shutdown_for_steamcmd() {
        if !Self::is_running_async().await {
            return;
        }

        // Ask Steam to shut down gracefully
        let mut cmd = Command::new(Self::steam_exe_path());
        cmd.arg("-shutdown")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(dz_common::CREATE_NO_WINDOW);
        let _ = cmd.spawn();

        // Poll every 500 ms for up to 15 s
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if !Self::is_running_async().await {
                return;
            }
        }

        // Still running — force kill
        let _ = tokio::task::spawn_blocking(Self::shutdown_force).await;

        // Brief pause to let OS release file locks before steamcmd starts
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

/// The Steam client's own processes, and nothing else whose name merely
/// starts with "steam": steamcmd, SteamOS services on a Steam Deck
/// (`steamos-*`), third-party tools such as steamtinkerlaunch.
fn is_steam_client_process(name: &str) -> bool {
    let name = name.strip_suffix(".exe").unwrap_or(name);
    [
        "steam",
        "steamwebhelper",
        "steamservice",
        "steamerrorreporter",
        "steamerrorreporter64",
    ]
    .iter()
    .any(|n| name.eq_ignore_ascii_case(n))
}

#[cfg(test)]
mod tests {
    use super::is_steam_client_process;

    #[test]
    fn only_the_client_is_force_closed() {
        for name in [
            "steam",
            "Steam.exe",
            "steamwebhelper",
            "steamwebhelper.exe",
            "steamservice.exe",
        ] {
            assert!(is_steam_client_process(name), "{name}");
        }
        for name in [
            "steamcmd",
            "steamcmd.exe",
            "steamos-manager",
            "steamtinkerlaunch",
            "steamapps",
        ] {
            assert!(!is_steam_client_process(name), "{name}");
        }
    }
}
