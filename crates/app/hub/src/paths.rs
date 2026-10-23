//! Where the hub keeps its files: the same places the Tauri shell used, so
//! the egui window finds the profile, the caches and the play log as they
//! were.
//!
//! - Most files live in `dz_common::paths::default_data_dir()`
//!   (`~/.local/share/dayz-community-hub`, `%APPDATA%\dayz-community-hub`).
//! - The news image cache lived under Tauri's `app_data_dir()`, which is the
//!   platform data directory joined with the bundle identifier
//!   (`com.thoxy.dayz-community-hub`): `$XDG_DATA_HOME` or
//!   `~/.local/share` on Linux, the roaming AppData known folder on Windows.
//!   [`tauri_app_data_dir`] resolves it as Tauri 2 did (through the `dirs`
//!   crate, as `tauri::path::PathResolver` does).
//!
//! [`HubConfig::data_dir`](crate::HubConfig) moves both to one directory.

use std::path::PathBuf;
use std::sync::RwLock;

/// The `identifier` the Tauri build had (`tauri.conf.json`), kept for its data folders.
pub const TAURI_IDENTIFIER: &str = "com.thoxy.dayz-community-hub";

static OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Set by `Hub::new` from its config; the latest hub's wins.
pub(crate) fn set_override(dir: Option<PathBuf>) {
    if let Ok(mut o) = OVERRIDE.write() {
        *o = dir;
    }
}

fn overridden() -> Option<PathBuf> {
    OVERRIDE.read().ok().and_then(|o| o.clone())
}

/// The data directory: the override, else the one every version used.
pub fn data_dir() -> PathBuf {
    overridden().unwrap_or_else(dz_common::paths::default_data_dir)
}

/// `<data dir>/profile.json`.
pub fn profile_path() -> PathBuf {
    data_dir().join("profile.json")
}

/// `<data dir>/server_list_cache.json`.
pub fn server_list_cache_path() -> PathBuf {
    data_dir().join("server_list_cache.json")
}

/// `<data dir>/dayzmetrics_ids.json`: the DayZ Metrics ids already found.
pub fn dayzmetrics_ids_path() -> PathBuf {
    data_dir().join("dayzmetrics_ids.json")
}

/// Tauri's `app_data_dir()`: `dirs::data_dir()/<identifier>`.
pub fn tauri_app_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(TAURI_IDENTIFIER))
}

/// The news image cache: `app_data_dir()/cache/images`, as the shell kept it.
pub fn images_cache_dir() -> Result<PathBuf, String> {
    overridden()
        .or_else(tauri_app_data_dir)
        .map(|d| d.join("cache").join("images"))
        .ok_or_else(|| "No app data directory".to_string())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    #[test]
    fn tauri_dirs_end_in_the_identifier() {
        if let Some(d) = super::tauri_app_data_dir() {
            assert!(d.ends_with(super::TAURI_IDENTIFIER));
        }
    }
}
