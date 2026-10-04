use dz_profile::Profile;
use serde::Serialize;

use crate::features::steamworks::ModDownloaderDto;

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct FavoriteDto {
    pub name: String,
    pub ip: String,
    pub port: u16,
    /// Saved join password, filled into Direct Connect.
    pub password: Option<String>,
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct HistoryDto {
    pub name: String,
    pub ip: String,
    pub port: u16,
    /// Unix seconds of the last join.
    pub ts: i64,
    pub relative_time: String,
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct LaunchOptionDto {
    pub key: String,
    pub enabled: bool,
    pub value: Option<String>,
    pub description: String,
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct ProfileDto {
    pub steam_login: Option<String>,
    /// A password an earlier version saved is still to be typed once at
    /// SteamCMD's prompt (never sent back to the window).
    pub has_saved_password: bool,
    /// The account SteamCMD last logged in as; its login is cached.
    pub steamcmd_logged_in: Option<String>,
    pub steam_root: Option<String>,
    pub steamcmd_enabled: bool,
    /// What downloads mods: SteamCMD or the Steam client.
    pub mod_downloader: ModDownloaderDto,
    /// Explicit path to the steamcmd binary (overrides auto-detection).
    pub steamcmd_path: Option<String>,
    pub player: Option<String>,
    pub steam_api_key: Option<String>,
    pub steam_id: Option<String>,
    /// The user's location for distances, as (longitude, latitude).
    pub user_location: Option<(f64, f64)>,
    pub favorites: Vec<FavoriteDto>,
    pub history: Vec<HistoryDto>,
    pub options: Vec<LaunchOptionDto>,
    /// IPs hidden from the server browser.
    pub excluded_ips: Vec<String>,
    /// Ping concurrency (5-100).
    pub ping_concurrency: u32,
    /// Background ping timeout in ms (1000-5000).
    pub ping_timeout_auto: u32,
    /// Manual ping timeout in ms (1000-30000).
    pub ping_timeout_manual: u32,
    /// Consecutive timeouts before auto-retry stops (0-5).
    pub ping_max_retries: u32,
    /// Include favorites in the background scan.
    pub ping_scan_favorites: bool,
    /// Include history in the background scan.
    pub ping_scan_history: bool,
    /// Include every other server in the background scan.
    pub ping_scan_servers: bool,
}

pub(crate) fn profile_to_dto(profile: &Profile) -> ProfileDto {
    ProfileDto {
        steam_login: profile.steam_login.clone(),
        has_saved_password: profile.steam_password.is_some(),
        steamcmd_logged_in: profile.steamcmd_logged_in.clone(),
        steam_root: profile.steam_root.clone(),
        steamcmd_enabled: profile.steamcmd_enabled,
        mod_downloader: profile.mod_downloader.into(),
        steamcmd_path: profile.steamcmd_path.clone(),
        player: profile.player.clone(),
        steam_api_key: profile.steam_api_key.clone(),
        steam_id: profile.steam_id.clone(),
        user_location: profile.user_location,
        favorites: profile
            .favorites
            .iter()
            .map(|f| FavoriteDto {
                name: f.name.clone(),
                ip: f.ip.clone(),
                port: f.port,
                password: f.password.clone(),
            })
            .collect(),
        history: profile
            .history
            .iter()
            .map(|h| HistoryDto {
                name: h.name.clone(),
                ip: h.ip.clone(),
                port: h.port,
                ts: h.ts,
                relative_time: h.relative_time(),
            })
            .collect(),
        options: profile
            .options
            .all_options()
            .into_iter()
            .map(|(key, opt)| LaunchOptionDto {
                key: key.to_string(),
                enabled: opt.enabled,
                value: opt.value.clone(),
                description: opt.description.clone(),
            })
            .collect(),
        excluded_ips: profile.excluded_ips.clone(),
        ping_concurrency: profile.ping_concurrency,
        ping_timeout_auto: profile.ping_timeout_auto,
        ping_timeout_manual: profile.ping_timeout_manual,
        ping_max_retries: profile.ping_max_retries,
        ping_scan_favorites: profile.ping_scan_favorites,
        ping_scan_history: profile.ping_scan_history,
        ping_scan_servers: profile.ping_scan_servers,
    }
}
