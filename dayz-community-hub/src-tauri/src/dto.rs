use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerDto {
    pub game_port: i64,
    pub ip: String,
    pub query_port: i64,
    pub name: String,
    pub map: String,
    pub players: i64,
    pub max_players: i64,
    pub environment: String,
    pub password: bool,
    pub version: String,
    pub first_person_only: bool,
    pub time: String,
    pub mods_count: usize,
    pub mods: Vec<ModDto>,
    pub vac: bool,
    pub battl_eye: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerSlimDto {
    pub game_port: i64,
    pub ip: String,
    pub query_port: i64,
    pub name: String,
    pub map: String,
    pub players: i64,
    pub max_players: i64,
    pub environment: String,
    pub password: bool,
    pub version: String,
    pub first_person_only: bool,
    pub time: String,
    pub mods_count: usize,
    pub vac: bool,
    pub battl_eye: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModDto {
    pub name: String,
    pub steam_workshop_id: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstalledModDto {
    pub name: String,
    pub id: u64,
    pub local_updated: i64,
    pub size: u64,
    pub size_human: String,
    pub managed: bool,
    /// Remote `time_updated` from Steam Workshop API. None if not yet checked.
    pub remote_updated: Option<i64>,
    /// True when `remote_updated > local_updated`.
    pub update_available: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FavoriteDto {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryDto {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub ts: i64,
    pub relative_time: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileDto {
    pub steam_login: Option<String>,
    pub steam_password: Option<String>,
    pub steam_root: Option<String>,
    pub steamcmd_enabled: bool,
    /// Explicit path to steamcmd binary (overrides auto-detection).
    pub steamcmd_path: Option<String>,
    pub player: Option<String>,
    pub steam_api_key: Option<String>,
    pub steam_id: Option<String>,
    pub battlemetrics_api_key: Option<String>,
    /// User location for distance calculation (longitude, latitude).
    pub user_location: Option<(f64, f64)>,
    pub favorites: Vec<FavoriteDto>,
    pub history: Vec<HistoryDto>,
    pub options: Vec<LaunchOptionDto>,
    /// IPs excluded from the server browser.
    pub excluded_ips: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LaunchOptionDto {
    pub key: String,
    pub enabled: bool,
    pub value: Option<String>,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArticleDto {
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content_text: String,
    pub content_html: String,
    pub date: String,
    pub url: String,
    pub image_url: Option<String>,
    pub category: Option<String>,
    pub author: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct A2sPlayerDto {
    pub name: String,
    pub score: i32,
    pub duration: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct A2sDetailsDto {
    pub server_name: String,
    pub game: String,
    pub players: u8,
    pub max_players: u8,
    pub map: String,
    pub version: String,
    pub players_list: Vec<A2sPlayerDto>,
    /// Mods from the server list (if server was found); empty if querying unknown server
    pub mods: Vec<ModDto>,
    /// The query port actually used (so frontend can display/cache correctly)
    pub query_port: i64,
    /// The server's actual game port from the A2S extended info (edf 0x80).
    /// `null` when the server does not include this optional field.
    pub game_port: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppStatsDto {
    pub server_count: usize,
    pub total_players: i64,
    pub player_name: Option<String>,
    pub steam_login: Option<String>,
    pub has_steamcmd: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModProgressEvent {
    pub kind: String,
    pub current: usize,
    pub total: usize,
    pub mod_id: u64,
    pub name: String,
    pub ok: usize,
    pub failed: usize,
    pub hint: Option<String>,
    pub log_line: Option<String>,
}

/// Response from the initialize command.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InitResult {
    pub server_count: usize,
    pub from_cache: bool,
    /// True when the profile was freshly created (no existing profile.json found).
    pub is_first_launch: bool,
}

/// BattleMetrics server info fetched on demand for the detail panel.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BattleMetricsDto {
    /// BattleMetrics server ID (used to build the BM page URL).
    pub id: String,
    /// Global rank (1 = most popular). None if not ranked.
    pub rank: Option<i64>,
    /// Server status: "online" | "offline" | "dead"
    pub status: String,
    /// ISO 3166-1 alpha-2 country code, e.g. "DE", "US".
    pub country: Option<String>,
    /// Server coordinates (longitude, latitude). None if unavailable.
    pub location: Option<(f64, f64)>,
    /// Uptime percentage over the last 30 days (0–100).
    pub uptime: Option<f64>,
    /// Whether the server is private (password protected).
    pub private: Option<bool>,
    /// Whether this is an official server.
    pub official: Option<bool>,
    /// Whether third-person view is allowed.
    pub third_person: Option<bool>,
    /// Whether the server is modded.
    pub modded: Option<bool>,
    /// Query status: "valid", "invalid", etc.
    pub query_status: Option<String>,
    /// Server's Steam ID.
    pub server_steam_id: Option<String>,
    /// When the server was first seen on BattleMetrics (ISO 8601).
    pub created_at: Option<String>,
    /// Player count data points for the last 24 h: (unix_secs, player_count) pairs.
    pub player_history: Vec<(i64, i64)>,
}

/// Ping result sent via Channel.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PingResultDto {
    pub ip: String,
    pub port: i64,
    pub ms: u32,
}
