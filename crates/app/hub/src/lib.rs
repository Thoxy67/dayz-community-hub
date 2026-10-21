//! The backend the launcher's window talks to (L2 in `plan/architecture.md`):
//! what `apps/gui/src-tauri/src` did, without Tauri. The egui window keeps a
//! [`Hub`], calls its methods from the tokio runtime, and drains [`HubEvent`]s
//! from the sink it handed [`Hub::new`].
//!
//! # Start-up
//!
//! ```text
//! let hub = Hub::new(HubConfig { mode, data_dir, args }, sink)?;   // args: CliArgs::parse()
//! hub.start();               // inside the runtime: notifiers, gamepad, play tracker
//! hub.initialize().await?;   // was the window's first `initialize` invoke
//! ```
//!
//! # From the old commands to `Hub` methods
//!
//! Every `#[tauri::command]` of `ipc.rs` is a method of the same name with the
//! same arguments (`state`, `app` and `ping` were Tauri's and are gone), so
//! `bindings.ts`' `commands.x(...)` is `hub.x(...).await`. Grouped by the
//! module that holds them (`features::<module>`, where their DTOs live too):
//!
//! - `a2s`:
//!   - `query_a2s(&self, ip: String, query_port: i64, game_port: Option<i64>) -> Result<Arc<A2sDetailsDto>, String>`
//! - `browser`:
//!   - `servers_query(&self, query: ServerQuery) -> Result<ServerPage, String>`
//!   - `servers_lookup(&self, keys: Vec<String>) -> Result<Vec<Option<ServerRow>>, String>`
//!   - `server_maps(&self) -> Result<Vec<MapCount>, String>`
//!   - `start_scan(&self) -> Result<ScanProgress, String>`
//! - `dzch_cli`:
//!   - `get_cli_args(&self) -> CliArgs`
//!   - `read_dzch_file(&self, path: String) -> Result<DzchConfig, String>`
//!   - `write_dzch_file(&self, path: String, config: DzchConfig) -> Result<(), String>`
//!   - `parse_dzch_url(&self, url: String) -> Result<DzchConfig, String>`
//! - `friends`:
//!   - `steam_friends(&self, client: bool) -> Result<FriendsDto, String>`
//! - `gamepad`:
//!   - `gamepad_status(&self) -> GamepadStatus`
//! - `launch`:
//!   - `toggle_launch_option(&self, key: String) -> Result<bool, String>`
//!   - `set_launch_option_value(&self, key: String, value: Option<String>) -> Result<(), String>`
//!   - `launch_server(&self, ip: String, port: i64, password: Option<String>) -> Result<(), String>`
//!   - `launch_direct(&self, ip: String, game_port: u16, password: Option<String>, extra_args: Option<Vec<String>>) -> Result<(), String>`
//!   - `steam_launch_info(&self) -> Result<SteamLaunchInfoDto, String>`
//!   - `game_running(&self) -> Result<bool, String>`
//!   - `kill_game(&self) -> Result<u32, String>`
//!   - `battleye_status(&self) -> Result<dz_game::battleye::BattlEyeStatus, String>`
//!   - `repair_battleye(&self, clear: bool) -> Result<dz_game::battleye::BattlEyeStatus, String>`
//!   - `game_data_dirs(&self) -> Result<Vec<String>, String>`
//! - `metrics`:
//!   - `fetch_server_metrics(&self, ip: String, game_port: u32, query_port: u32, name: String) -> Result<ServerMetricsDto, String>`
//!   - `fetch_metrics_history(&self, id: u64, range: String) -> Result<Vec<(i64, f64)>, String>`
//!   - `fetch_metrics_rank_history(&self, id: u64, range: String) -> Result<Vec<(String, f64)>, String>`
//!   - `fetch_metrics_heatmap(&self, id: u64) -> Result<Vec<HeatCell>, String>`
//! - `mods`:
//!   - `mods_usage(&self) -> Result<Vec<ModUsageDto>, String>`
//!   - `get_installed_mods(&self) -> Result<Vec<InstalledModDto>, String>`
//!   - `check_mod_updates(&self) -> Result<Vec<InstalledModDto>, String>`
//!   - `delete_mod(&self, mod_id: u64) -> Result<bool, String>`
//!   - `delete_mods_bulk(&self, mod_ids: Vec<u64>) -> Result<Vec<u64>, String>`
//!   - `toggle_mod_managed(&self, mod_id: u64) -> Result<bool, String>`
//!   - `cleanup_mods(&self) -> Result<String, String>`
//!   - `open_workshop_dir(&self) -> Result<std::path::PathBuf, String>`
//!   - `open_mod_dir(&self, mod_id: u64) -> Result<std::path::PathBuf, String>`
//!   - `setup_mod_symlinks(&self, ip: String, port: i64) -> Result<(), String>`
//! - `native`:
//!   - `geolocate_ip(&self) -> Result<GeoLocation, String>`
//! - `news::images`:
//!   - `fetch_image(&self, url: String) -> Result<PathBuf, String>`
//!   - `resolve_cached_images(&self, urls: Vec<String>) -> Result<Vec<(String, PathBuf)>, String>`
//! - `news`:
//!   - `fetch_news(&self) -> Result<Vec<ArticleDto>, String>`
//! - `offline`:
//!   - `get_offline_missions(&self) -> Result<Vec<String>, String>`
//!   - `update_offline_mode(&self) -> Result<(), String>`
//!   - `remove_offline_mode(&self) -> Result<usize, String>`
//!   - `clear_offline_saves(&self) -> Result<usize, String>`
//!   - `offline_saves(&self) -> Result<Vec<MissionSaveDto>, String>`
//!   - `clear_mission_save(&self, mission: String) -> Result<bool, String>`
//!   - `remove_mission(&self, mission: String) -> Result<(), String>`
//!   - `open_missions_dir(&self) -> Result<std::path::PathBuf, String>`
//!   - `launch_offline_mission(&self, mission: String) -> Result<(), String>`
//!   - `open_mission_dir(&self, mission: String) -> Result<std::path::PathBuf, String>`
//! - `ping`:
//!   - `ping_servers(&self, targets: Vec<String>, concurrency: Option<usize>, timeout_ms: Option<u64>) -> Result<(), String>`
//!   - `ping_scan_state(&self) -> Result<ScanState, String>`
//!   - `ping_single(&self, ip: String, port: i64, timeout_ms: Option<u64>) -> Result<u32, String>`
//!   - `cancel_ping(&self) -> Result<(), String>`
//!   - `toggle_ping_pause(&self) -> Result<bool, String>`
//! - `profile::io`:
//!   - `export_profile(&self, path: String, include_mods: bool) -> Result<(), String>`
//!   - `import_profile(&self, path: String) -> Result<ProfileDto, String>`
//!   - `reset_profile(&self) -> Result<(), String>`
//!   - `restart_app(&self) -> Result<(), String>`
//! - `profile`:
//!   - `get_profile(&self) -> Result<ProfileDto, String>`
//!   - `save_profile_settings(&self, settings: ProfileSettingsInput) -> Result<(), String>`
//!   - `add_favorite(&self, name: String, ip: String, port: u16, password: Option<String>) -> Result<(), String>`
//!   - `remove_favorite(&self, ip: String, port: u16) -> Result<(), String>`
//!   - `remove_history_entry(&self, ip: String, port: u16) -> Result<(), String>`
//!   - `clear_history(&self) -> Result<(), String>`
//!   - `add_excluded_ip(&self, ip: String) -> Result<(), String>`
//!   - `remove_excluded_ip(&self, ip: String) -> Result<(), String>`
//! - `servers`:
//!   - `check_first_launch(&self) -> bool`
//!   - `initialize(&self) -> Result<InitResult, String>`
//!   - `get_server_details(&self, ip: String, port: i64) -> Result<ServerDto, String>`
//!   - `set_steam_server_list(&self, enabled: bool) -> Result<(), String>`
//!   - `get_app_stats(&self) -> Result<AppStatsDto, String>`
//! - `servers::refresh`:
//!   - `servers_refresh(&self) -> Result<RefreshState, String>`
//!   - `refresh_state(&self) -> RefreshState`
//! - `servers::sources`:
//!   - `sources_status(&self) -> SourcesChanged`
//! - `stats`:
//!   - `play_stats(&self, range: StatsRange, utc_offset_min: i32) -> Result<PlayStatsDto, String>`
//!   - `play_sessions(&self, search: String, from: Option<i64>, to: Option<i64>, offset: u32, limit: u32) -> Result<SessionsPage, String>`
//!   - `delete_session(&self, start: i64, name: String) -> Result<bool, String>`
//! - `steam`:
//!   - `steam_me(&self) -> Result<SteamMeDto, String>`
//!   - `fetch_steam_avatar(&self) -> Result<Option<String>, String>`
//!   - `fetch_steam_player_count(&self) -> Result<u32, String>`
//! - `steamcmd::detect`:
//!   - `detect_steamcmd(&self) -> Result<SteamcmdStatusDto, String>`
//!   - `watch_steamcmd(&self) -> Result<(), String>`
//!   - `download_steamcmd_windows(&self) -> Result<String, String>`
//!   - `detect_dayz(&self, path: Option<String>) -> Result<DayzDetectDto, String>`
//! - `steamcmd`:
//!   - `start_mod_operation(&self, op_type: ModOpType, ip: Option<String>, port: Option<i64>, mod_id: Option<u64>, mod_name: Option<String>, mod_ids: Option<Vec<u64>>, mod_names: Option<Vec<String>>) -> Result<(), String>`
//!   - `send_steamcmd_input(&self, input: String) -> Result<(), String>`
//!   - `cancel_mod_operation(&self) -> Result<(), String>`
//!   - `mod_op_state(&self) -> Option<op::ModOpSnapshot>`
//!   - `mod_op_attach(&self) -> Option<op::ModOpSnapshot>`
//!   - `mod_op_dismiss(&self)`
//!   - `steamcmd_dirs(&self) -> Result<SteamcmdDirsDto, String>`
//!   - `open_steamcmd_dir(&self) -> Result<std::path::PathBuf, String>`
//! - `steamworks`:
//!   - `set_mod_downloader(&self, downloader: ModDownloaderDto) -> Result<(), String>`
//!   - `steamworks_status(&self) -> Result<SteamworksStatusDto, String>`
//!   - `steamworks_check(&self) -> Result<(), String>`
//!   - `steam_subscriptions(&self, ids: Vec<u64>) -> Result<SteamSubscriptionsDto, String>`
//! - `system`:
//!   - `get_system_specs(&self) -> Result<SystemSpecsDto, String>`
//! - `updater`:
//!   - `update_support(&self) -> UpdateSupport`
//!   - `check_for_update(&self) -> Result<Option<UpdateInfo>, String>`
//!   - `install_update(&self) -> Result<(), String>`
//!   - `update_download_state(&self) -> DownloadState`
//!
//! # What changed on the way
//!
//! - **The window's own commands left**: `open_url`, `copy_text`,
//!   `pick_file`, `save_file` (the window opens links with `open`, after
//!   [`features::native::is_openable`]; the clipboard is egui's; dialogs are
//!   `rfd`'s) and `news_webview_result` (the WebView fallback is dropped).
//! - **Opening a folder** (`open_workshop_dir`, `open_mod_dir`,
//!   `open_steamcmd_dir`, `open_missions_dir`, `open_mission_dir`) returns the
//!   `PathBuf` to open (created first where the command created it).
//! - **Channels became events**: `start_mod_operation` takes no
//!   `on_progress`, its steps arrive as `HubEvent::ModProgress`;
//!   `mod_op_attach()` returns the snapshot (steps at or below its `seq` are
//!   in it). `install_update` takes no `on_event`: `HubEvent::UpdateDownload`.
//!   `ping_servers` takes no `on_progress`: `HubEvent::PingResults`.
//! - **News**: `fetch_news` never opens a WebView; when Cloudflare blocks,
//!   its error starts with `"blocked:"` ([`features::news::BLOCKED_PREFIX`]).
//!   `fetch_image` returns the cached file's `PathBuf` and
//!   `resolve_cached_images` `(url, PathBuf)` pairs (no asset-protocol URLs);
//!   the cache is the shell's (`paths::images_cache_dir`).
//! - **`restart_app`** starts this executable (or `$APPIMAGE`) again with the
//!   same arguments and exits; it returns only an error.
//! - **The updater** is plain Rust (`features::updater`): the same
//!   `latest.json`, the same minisign public key, the AppImage written over
//!   `$APPIMAGE` on Linux, the zip swap on Windows.
//! - Commands that answered without a `Result` still do (`check_first_launch`,
//!   `refresh_state`, `sources_status`, `get_cli_args`, `mod_op_state`,
//!   `mod_op_attach`, `mod_op_dismiss`, `update_support`,
//!   `update_download_state`, `gamepad_status`). `get_cli_args` hands the
//!   start-up arguments once, then `CliArgs::none()`.
//!
//! # Events
//!
//! [`HubEvent`] has one variant per `tauri_specta::Event` (`LaunchDone`,
//! `LaunchError`, `OfflineModeUpdated`, `OfflineModeError`,
//! `SteamcmdDetected`, `CliArgs`, `ServersChanged` (at most 2/s),
//! `SourcesChanged` (at most 4/s), `RefreshChanged`, `PingScanProgress`
//! (at most 2/s), `PlaySessionsChanged`, `GamepadInput`, `GamepadPads`), with
//! the same payloads, plus `ModProgress`, `UpdateDownload` and `PingResults`.
//!
//! # Beside the commands
//!
//! - [`Hub::hand_off`]: a second launch's arguments (see
//!   [`CliArgs::from_second_launch`]) become a `HubEvent::CliArgs`.
//! - [`Hub::set_focused`]: the window's focus, for the pads.
//! - [`features::dzch_cli::register_url_scheme`] (`dzch://`: a Linux
//!   `.desktop` file and `xdg-mime`, or HKCU on Windows) and, on Windows,
//!   `features::dzch_cli::register_file_type` (`.dzch`): call at start-up.
//! - **Mock mode** (`HubMode::Mock`, the `mock` feature): the invented world
//!   of `apps/gui/src/lib/ipc/mock.ts` (9 500 servers, 24 mods, a profile, a
//!   year of play), seeded into the real state in a temporary data directory;
//!   what would reach the network, Steam or the game answers from fixtures.
//!   A build without the feature refuses Mock mode.
//! - [`paths`]: the data directory, and Tauri's `app_*_dir()`s resolved as
//!   Tauri did (`com.thoxy.dayz-community-hub`).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

/// In Mock mode, answer from the mock world instead of going on:
/// `mocked!(self => expr)`, or `mocked!(self, w => expr)` with the world as
/// `w`. Without the `mock` feature it is nothing.
macro_rules! mocked {
    ($hub:expr => $body:expr) => {
        #[cfg(feature = "mock")]
        if $hub.is_mock() {
            return $body;
        }
    };
    ($hub:expr, $w:ident => $body:expr) => {
        #[cfg(feature = "mock")]
        if let Some($w) = $hub.mock() {
            return $body;
        }
    };
}

mod error;
pub mod events;
pub mod features;
mod hub;
#[cfg(feature = "mock")]
mod mock;
mod net;
pub mod paths;
mod state;

pub use events::HubEvent;
pub use features::dzch_cli::CliArgs;
pub use hub::{EventSink, Hub, HubConfig, HubMode};
