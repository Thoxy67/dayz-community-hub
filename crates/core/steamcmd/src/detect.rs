//! Finding Steam, its libraries, and steamcmd on this machine.

use std::path::{Path, PathBuf};

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
    // A library Steam does not list (a reinstalled Steam, a drive plugged
    // back in, a Steam installed off the default path): look where libraries
    // usually sit on every other drive.
    if let Some(found) = other_drive_libraries()
        .iter()
        .find_map(|sa| library_with_dayz(sa))
    {
        return Some(found);
    }
    // Otherwise fall back to the first existing default steamapps directory.
    candidates.into_iter().find(|c| c.is_dir())
}

/// Folders a Steam library is commonly made in, relative to a drive's root.
const LIBRARY_NAMES: &[&str] = &[
    "SteamLibrary",
    "Steam",
    "steam",
    "Games/SteamLibrary",
    "Games/Steam",
    "games/SteamLibrary",
    "games/steam",
    "Program Files (x86)/Steam",
    "Program Files/Steam",
];

/// `steamapps` folders at the usual places on every drive but the system's
/// defaults: drive letters on Windows, mount points on Linux. Only existing
/// folders are returned; a missing drive costs one failed stat.
fn other_drive_libraries() -> Vec<PathBuf> {
    drive_roots()
        .into_iter()
        .flat_map(|root| {
            let mut v = vec![root.join("steamapps")];
            v.extend(LIBRARY_NAMES.iter().map(|n| root.join(n).join("steamapps")));
            v
        })
        .filter(|sa| sa.is_dir())
        .collect()
}

#[cfg(target_os = "windows")]
fn drive_roots() -> Vec<PathBuf> {
    // A to B are floppies by convention: asking them can stall. Network
    // drives are left out for the same reason (a disconnected share).
    ('C'..='Z')
        .map(|l| format!("{l}:\\"))
        .filter(|root| dz_common::win::is_local_drive(root))
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn drive_roots() -> Vec<PathBuf> {
    let user = std::env::var("USER").unwrap_or_default();
    let mut bases = vec![PathBuf::from("/mnt"), PathBuf::from("/media")];
    if !user.is_empty() {
        bases.push(PathBuf::from("/media").join(&user));
        bases.push(PathBuf::from("/run/media").join(&user));
    }
    // Listing a folder does not touch what is mounted in it, but a stat
    // does: a network mount (a stale NFS or sshfs one above all) can hang
    // it, so those are left out before anything asks them.
    let remote = remote_mounts();
    bases
        .iter()
        .filter_map(|b| std::fs::read_dir(b).ok())
        .flat_map(|entries| entries.flatten())
        .map(|e| e.path())
        .filter(|p| !remote.iter().any(|m| p.starts_with(m)))
        .filter(|p| p.is_dir())
        .collect()
}

/// Mount points of network and FUSE-over-network file systems, from
/// `/proc/self/mounts`.
#[cfg(not(target_os = "windows"))]
fn remote_mounts() -> Vec<PathBuf> {
    const REMOTE: &[&str] = &[
        "nfs",
        "nfs4",
        "cifs",
        "smb3",
        "smbfs",
        "sshfs",
        "fuse.sshfs",
        "fuse.rclone",
        "davfs",
        "fuse.davfs2",
        "afs",
        "ceph",
        "glusterfs",
        "fuse.glusterfs",
        "9p",
    ];
    let Ok(mounts) = std::fs::read_to_string("/proc/self/mounts") else {
        return Vec::new();
    };
    mounts
        .lines()
        .filter_map(|line| {
            let mut f = line.split_whitespace();
            let (_, at, kind) = (f.next()?, f.next()?, f.next()?);
            // Spaces in mount points are written as \040.
            REMOTE
                .contains(&kind)
                .then(|| PathBuf::from(at.replace("\\040", " ")))
        })
        .collect()
}

/// Where DayZ is on this machine, as the setup shows it.
#[derive(Debug, Clone, Default)]
pub struct DayzInstall {
    /// The `steamapps` directory of the library that holds DayZ, or of the
    /// default Steam install when no library does.
    pub steamapps: Option<PathBuf>,
    /// `<steamapps>/common/DayZ`, when it is there.
    pub dayz: Option<PathBuf>,
    /// Every Steam library's DayZ workshop folder that exists.
    pub workshop_dirs: Vec<PathBuf>,
}

/// Find DayZ: in `explicit` (a library or its `steamapps`, as a player would
/// pick it) and the libraries it lists, else wherever Steam is installed.
/// Touches the filesystem (and on Windows the registry): blocking.
pub fn detect_dayz(explicit: Option<&Path>) -> DayzInstall {
    let steamapps = match explicit {
        Some(p) => {
            let sa = if p.file_name().is_some_and(|n| n == "steamapps")
                || !p.join("steamapps").is_dir()
            {
                p.to_path_buf()
            } else {
                p.join("steamapps")
            };
            Some(library_with_dayz(&sa).unwrap_or(sa))
        }
        None => find_steam_root(),
    };
    let dayz = steamapps
        .as_ref()
        .map(|sa| sa.join("common").join("DayZ"))
        .filter(|d| d.is_dir());
    let workshop_dirs = steamapps
        .as_ref()
        .filter(|sa| sa.is_dir())
        .map(|sa| steam_workshop_dirs(sa))
        .unwrap_or_default();
    DayzInstall {
        steamapps,
        dayz,
        workshop_dirs,
    }
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

/// Every Steam library's DayZ workshop folder
/// (`<library>/steamapps/workshop/content/221100`) that exists: the library
/// at `steam_root`, the default install's, and every library their
/// `libraryfolders.vdf` lists. These belong to the Steam client: read only.
pub fn steam_workshop_dirs(steam_root: &Path) -> Vec<PathBuf> {
    let mut steamapps = vec![steam_root.to_path_buf()];
    steamapps.extend(default_steamapps_candidates());
    let mut libraries: Vec<PathBuf> = Vec::new();
    for sa in &steamapps {
        libraries.push(sa.clone());
        libraries.extend(
            steam_libraries_from_vdf(sa)
                .into_iter()
                .map(|lib| lib.join("steamapps")),
        );
    }
    let mut seen = rustc_hash::FxHashSet::default();
    libraries
        .into_iter()
        .map(|sa| crate::cmd::workshop_content(&sa, DAYZ_GAME_ID))
        .filter(|dir| dir.is_dir())
        .filter(|dir| seen.insert(dir.canonicalize().unwrap_or_else(|_| dir.clone())))
        .collect()
}

/// Read the Steam install path from the Windows registry.
///
/// Queries `HKCU\Software\Valve\Steam` → `SteamPath` (REG_SZ).
/// This is the most reliable way to find Steam when it was installed
/// to a non-default drive or directory (e.g. `D:\Games\Steam`).
///
/// Read through the registry API, not `reg query`: it runs on every mod
/// scan (through [`steam_workshop_dirs`]), where a `reg.exe` start cost tens
/// of milliseconds each time, and `reg` prints in the console's code page,
/// which mangled a path with accents.
#[cfg(target_os = "windows")]
pub(crate) fn query_steam_registry_path() -> Option<String> {
    let value = dz_common::win::reg_string(
        dz_common::win::HKEY_CURRENT_USER,
        "Software\\Valve\\Steam",
        "SteamPath",
    )
    .ok()??;
    let value = value.trim();
    // Steam writes forward slashes in the registry; normalise to backslashes.
    (!value.is_empty()).then(|| value.replace('/', "\\"))
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
            // Never a steamcmd.exe inside the Steam client's own directory:
            // it would update and write over the client's files.
            let mut v = Vec::new();
            if let Ok(appdata) = std::env::var("APPDATA") {
                v.push(
                    PathBuf::from(appdata)
                        .join("dayz-community-hub")
                        .join("steamcmd")
                        .join("steamcmd.exe"),
                );
            }
            for base in &[
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temp dir, removed by the caller.
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dz-detect-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_picked_library_finds_dayz_and_its_mods() {
        let lib = scratch("lib");
        std::fs::create_dir_all(lib.join("steamapps/common/DayZ")).unwrap();
        std::fs::create_dir_all(lib.join("steamapps/workshop/content/221100/1559212036")).unwrap();
        // Picked as the library folder, not its steamapps: both are accepted.
        let found = detect_dayz(Some(&lib));
        assert_eq!(
            found.steamapps.as_deref(),
            Some(lib.join("steamapps").as_path())
        );
        assert_eq!(
            found.dayz.as_deref(),
            Some(lib.join("steamapps/common/DayZ").as_path())
        );
        assert!(
            found
                .workshop_dirs
                .contains(&lib.join("steamapps/workshop/content/221100"))
        );
        std::fs::remove_dir_all(&lib).unwrap();
    }

    #[test]
    fn a_library_without_dayz_says_so() {
        let lib = scratch("empty");
        std::fs::create_dir_all(lib.join("steamapps")).unwrap();
        let found = detect_dayz(Some(&lib.join("steamapps")));
        assert_eq!(
            found.steamapps.as_deref(),
            Some(lib.join("steamapps").as_path())
        );
        assert!(found.dayz.is_none());
        std::fs::remove_dir_all(&lib).unwrap();
    }
}
