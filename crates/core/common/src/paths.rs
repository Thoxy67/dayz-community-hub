//! Where the app keeps its files.

use std::path::PathBuf;

const APP_NAME: &str = "dayz-community-hub";

/// The platform-appropriate data directory:
/// - Linux/macOS: `~/.local/share/dayz-community-hub/`
/// - Windows:     `%APPDATA%\dayz-community-hub\`
pub fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| {
            std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Public".to_string())
        });
        PathBuf::from(appdata).join(APP_NAME)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join(APP_NAME)
    }
}

/// `<data dir>/profile.json`.
pub fn default_profile_path() -> PathBuf {
    default_data_dir().join("profile.json")
}

/// `<data dir>/server_list_cache.json`.
pub fn server_list_cache_path() -> PathBuf {
    default_data_dir().join("server_list_cache.json")
}

/// `<data dir>/steamcmd-content`: SteamCMD's own install directory, where it
/// downloads workshop mods. Never a Steam library.
pub fn steamcmd_content_dir() -> PathBuf {
    default_data_dir().join("steamcmd-content")
}

/// `<data dir>/steamcmd-home`: the `HOME` SteamCMD runs with on Linux, so
/// it keeps its config and login away from the Steam client's.
pub fn steamcmd_home_dir() -> PathBuf {
    default_data_dir().join("steamcmd-home")
}
