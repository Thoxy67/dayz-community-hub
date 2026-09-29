//! Finding Steam, its libraries, and steamcmd on this machine.

use std::path::PathBuf;

use crate::DAYZ_GAME_ID;

/// Try to find Steam root directory by checking common locations.
/// Returns the path to the `steamapps` directory.
pub fn find_steam_root() -> Option<PathBuf> {
    let candidates = default_steamapps_candidates();
    // Prefer whichever Steam library actually contains DayZ. This handles
    // secondary drives (e.g. /mnt/ssd2/SteamLibrary) that Steam records in
    // `libraryfolders.vdf` but that aren't one of the default install paths.
    for candidate in &candidates {
        if let Some(dayz_steamapps) = library_with_dayz(candidate) {
            return Some(dayz_steamapps);
        }
    }
    // Otherwise fall back to the first existing default steamapps directory.
    candidates.into_iter().find(|c| c.is_dir())
}

/// Default `steamapps` directory candidates for the current platform, in
/// priority order. These are the *default* install locations; secondary
/// libraries are discovered from `libraryfolders.vdf` (see [`library_with_dayz`]).
#[cfg(target_os = "windows")]
fn default_steamapps_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    // STEAM_PATH env override (power users)
    if let Ok(p) = std::env::var("STEAM_PATH") {
        v.push(PathBuf::from(&p).join("steamapps"));
    }
    // Read the install path from the Windows registry.
    //   HKCU\Software\Valve\Steam  →  SteamPath (REG_SZ)
    // This handles custom install drives/directories reliably.
    if let Some(reg_path) = query_steam_registry_path() {
        v.push(PathBuf::from(&reg_path).join("steamapps"));
    }
    // Default install locations (fallback when registry key is absent)
    for base in &["C:\\Program Files (x86)\\Steam", "C:\\Program Files\\Steam"] {
        v.push(PathBuf::from(base).join("steamapps"));
    }
    // Per-user roaming / local variants
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        v.push(PathBuf::from(&local).join("Steam").join("steamapps"));
    }
    v
}

#[cfg(not(target_os = "windows"))]
fn default_steamapps_candidates() -> Vec<PathBuf> {
    let Ok(home) = std::env::var("HOME") else {
        return Vec::new();
    };
    let home_path = PathBuf::from(home);
    // Cover the major Linux Steam install sources: native, Flatpak, Snap.
    vec![
        home_path.join(".steam/steam/steamapps"),
        home_path.join(".local/share/Steam/steamapps"),
        home_path.join(".steam/root/steamapps"),
        // Flatpak Steam (com.valvesoftware.Steam)
        home_path.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps"),
        home_path.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps"),
        // Snap Steam
        home_path.join("snap/steam/common/.local/share/Steam/steamapps"),
        home_path.join("snap/steam/current/.local/share/Steam/steamapps"),
    ]
}

/// Given a `steamapps` directory, return the `steamapps` directory of whichever
/// Steam library actually has DayZ installed — checking this library first, then
/// every library listed in its `libraryfolders.vdf`. Returns `None` if DayZ
/// isn't found in any of them.
fn library_with_dayz(steamapps: &std::path::Path) -> Option<PathBuf> {
    if steamapps_has_dayz(steamapps) {
        return Some(steamapps.to_path_buf());
    }
    for lib in steam_libraries_from_vdf(steamapps) {
        let sa = lib.join("steamapps");
        if steamapps_has_dayz(&sa) {
            return Some(sa);
        }
    }
    None
}

/// True if this `steamapps` dir holds DayZ (the game dir or its app manifest).
fn steamapps_has_dayz(steamapps: &std::path::Path) -> bool {
    steamapps.join("common").join("DayZ").is_dir()
        || steamapps
            .join(format!("appmanifest_{DAYZ_GAME_ID}.acf"))
            .is_file()
}

/// Parse the `"path"` entries out of `<steamapps>/libraryfolders.vdf`.
/// Crude but sufficient: the file is a flat VDF where each library is recorded
/// as `"path"   "<dir>"`. Windows paths are double-backslash escaped.
fn steam_libraries_from_vdf(steamapps: &std::path::Path) -> Vec<PathBuf> {
    let vdf = steamapps.join("libraryfolders.vdf");
    let Ok(content) = std::fs::read_to_string(&vdf) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("\"path\"")?.trim();
            let path = rest.trim_matches('"').replace("\\\\", "\\");
            (!path.is_empty()).then(|| PathBuf::from(path))
        })
        .collect()
}

/// Read the Steam install path from the Windows registry.
///
/// Queries `HKCU\Software\Valve\Steam` → `SteamPath` (REG_SZ).
/// This is the most reliable way to find Steam when it was installed
/// to a non-default drive or directory (e.g. `D:\Games\Steam`).
#[cfg(target_os = "windows")]
pub(crate) fn query_steam_registry_path() -> Option<String> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("reg")
        .args(["query", "HKCU\\Software\\Valve\\Steam", "/v", "SteamPath"])
        .creation_flags(dz_common::CREATE_NO_WINDOW) // CREATE_NO_WINDOW
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // reg query output format:
    //   HKEY_CURRENT_USER\Software\Valve\Steam
    //       SteamPath    REG_SZ    C:/Program Files (x86)/Steam
    for line in stdout.lines() {
        let line = line.trim();
        if line.to_lowercase().starts_with("steampath") {
            // Split on "REG_SZ" and take the value after it
            if let Some(pos) = line.to_uppercase().find("REG_SZ") {
                let value = line[pos + "REG_SZ".len()..].trim();
                if !value.is_empty() {
                    // Steam writes forward slashes in the registry; normalise to backslashes.
                    return Some(value.replace('/', "\\"));
                }
            }
        }
    }
    None
}

/// Try to find steamcmd binary in PATH or common locations.
pub fn find_steamcmd() -> Option<PathBuf> {
    // Honour PATH first (works on all platforms)
    #[cfg(target_os = "windows")]
    let binary = "steamcmd.exe";
    #[cfg(not(target_os = "windows"))]
    let binary = "steamcmd";

    if let Ok(path) = which::which(binary) {
        return Some(path);
    }

    #[cfg(target_os = "windows")]
    {
        let candidates: Vec<PathBuf> = {
            let mut v = Vec::new();
            // If Steam is found via registry, its bundled steamcmd is in the same directory.
            if let Some(steam_path) = query_steam_registry_path() {
                v.push(PathBuf::from(&steam_path).join("steamcmd.exe"));
            }
            for base in &[
                "C:\\Program Files (x86)\\Steam\\steamcmd.exe",
                "C:\\Program Files (x86)\\SteamCMD\\steamcmd.exe",
                "C:\\SteamCMD\\steamcmd.exe",
            ] {
                v.push(PathBuf::from(base));
            }
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                v.push(
                    PathBuf::from(&local)
                        .join("Programs")
                        .join("steamcmd")
                        .join("steamcmd.exe"),
                );
            }
            v
        };
        for candidate in &candidates {
            if candidate.exists() {
                return Some(candidate.clone());
            }
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").ok()?;
        let home_path = PathBuf::from(home);
        // Linux + macOS: covers the most common package sources users hit.
        // Snap and Flatpak ship steamcmd in non-PATH locations, so users
        // with those installs were previously falling through to "not found"
        // even when a working binary was present.
        let candidates = [
            home_path.join(".steam/steamcmd/steamcmd.sh"),
            home_path.join(".local/share/Steam/steamcmd/steamcmd.sh"),
            // Flatpak (steamcmd packaged separately from the Steam flatpak)
            home_path.join(".var/app/com.valvesoftware.SteamCMD/data/steamcmd.sh"),
            home_path.join(".var/app/com.valvesoftware.SteamCMD/.local/share/Steam/steamcmd.sh"),
            // Standard Linux distribution paths
            PathBuf::from("/usr/games/steamcmd"),
            PathBuf::from("/usr/local/games/steamcmd"),
            PathBuf::from("/usr/bin/steamcmd"),
            PathBuf::from("/usr/local/bin/steamcmd"),
            // Snap (snap shadows /usr/bin in PATH on some distros, but the
            // direct path is more reliable for detection)
            PathBuf::from("/snap/bin/steamcmd"),
            PathBuf::from("/var/lib/snapd/snap/bin/steamcmd"),
            // AUR package — installs into /opt
            PathBuf::from("/opt/steamcmd/steamcmd.sh"),
            // macOS Homebrew
            PathBuf::from("/usr/local/opt/steamcmd/bin/steamcmd"),
            PathBuf::from("/opt/homebrew/bin/steamcmd"),
        ];
        for candidate in candidates.iter() {
            if candidate.exists() {
                return Some(candidate.to_path_buf());
            }
        }
        None
    }
}
