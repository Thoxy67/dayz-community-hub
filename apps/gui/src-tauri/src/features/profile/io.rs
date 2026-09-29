//! Moving a profile between machines: export and import every settings
//! file as one zstd-compressed bundle, reset to a blank profile, restart.

use dz_common::paths;
use tauri::State;

use super::{ProfileDto, profile_to_dto};
use crate::error::ResultExt;
use crate::state::SharedState;

/// Bundle format: a map of filename → raw JSON value for every settings file.
#[derive(serde::Serialize, serde::Deserialize)]
struct ProfileBundle {
    version: u8,
    files: std::collections::HashMap<String, serde_json::Value>,
}

/// Files that should never be included in an export (transient caches etc.)
const EXPORT_EXCLUDE: &[&str] = &["server_list_cache.json"];

/// Export all settings files from the data directory as a zstd-compressed bundle.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_profile(path: String, include_mods: bool) -> Result<(), String> {
    let data_dir = paths::default_data_dir();

    let mut files: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();

    let mut read_dir = tokio::fs::read_dir(&data_dir)
        .await
        .map_err(|e| format!("Cannot read data dir: {e}"))?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| format!("Cannot read dir entry: {e}"))?
    {
        let fname = entry.file_name();
        let name = fname.to_string_lossy().to_string();
        if !name.ends_with(".json") {
            continue;
        }
        if EXPORT_EXCLUDE.contains(&name.as_str()) {
            continue;
        }
        if !include_mods && name == "mods.json" {
            continue;
        }
        let raw = tokio::fs::read_to_string(entry.path())
            .await
            .map_err(|e| format!("Cannot read {name}: {e}"))?;
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("{name} parse error: {e}"))?;
        files.insert(name, value);
    }

    if !files.contains_key("profile.json") {
        return Err("profile.json not found in data directory".into());
    }

    let bundle = ProfileBundle { version: 2, files };
    let json_bytes = serde_json::to_vec(&bundle).cmd_err()?;

    // zstd is CPU-bound — run off the async runtime. Level 3 is plenty for a
    // small settings JSON (the size delta vs. level 9 is negligible) and far
    // faster.
    let compressed =
        tokio::task::spawn_blocking(move || zstd::encode_all(std::io::Cursor::new(&json_bytes), 3))
            .await
            .map_err(|e| format!("compression task failed: {e}"))?
            .map_err(|e| format!("zstd compression failed: {e}"))?;

    tokio::fs::write(&path, &compressed)
        .await
        .map_err(|e| format!("Cannot write export file: {e}"))?;

    Ok(())
}

/// Largest decompressed bundle accepted: settings are kilobytes, so anything
/// near this is not a profile (or is a decompression bomb).
const MAX_BUNDLE_BYTES: u64 = 32 * 1024 * 1024;

/// A settings file name a bundle may write: a plain `name.json` in the data
/// directory, never a path.
fn is_settings_file_name(name: &str) -> bool {
    name.ends_with(".json")
        && name.len() > ".json".len()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !name.contains("..")
        && !EXPORT_EXCLUDE.contains(&name)
}

/// The files a bundle holds, checked before anything on disk is touched.
fn bundle_files(raw: &serde_json::Value) -> Result<Vec<(String, String)>, String> {
    let version = raw["version"].as_u64().unwrap_or(1);
    let files: Vec<(String, &serde_json::Value)> = match version {
        1 => {
            if !raw["profile"].is_object() {
                return Err("This bundle holds no profile".into());
            }
            let mut v = vec![("profile.json".to_string(), &raw["profile"])];
            if !raw["mods"].is_null() {
                v.push(("mods.json".to_string(), &raw["mods"]));
            }
            v
        }
        2 => raw["files"]
            .as_object()
            .ok_or("This bundle's file list is missing")?
            .iter()
            .map(|(k, v)| (k.clone(), v))
            .collect(),
        v => return Err(format!("Unsupported bundle version {v}")),
    };
    if !files.iter().any(|(name, _)| name == "profile.json") {
        return Err("This bundle holds no profile.json".into());
    }
    files
        .into_iter()
        .map(|(name, value)| {
            if !is_settings_file_name(&name) {
                return Err(format!("This bundle names a file it may not write: {name}"));
            }
            Ok((name, serde_json::to_string_pretty(value).cmd_err()?))
        })
        .collect()
}

/// Import a profile bundle, replacing the settings files. The bundle is read
/// and checked in full first, so a bad file leaves the current settings alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_profile(
    path: String,
    state: State<'_, SharedState>,
) -> Result<ProfileDto, String> {
    let compressed = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Cannot read import file: {e}"))?;

    let json_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        use std::io::Read;
        let decoder = zstd::Decoder::new(std::io::Cursor::new(compressed))
            .map_err(|e| format!("Not a profile bundle: {e}"))?;
        let mut out = Vec::new();
        decoder
            .take(MAX_BUNDLE_BYTES + 1)
            .read_to_end(&mut out)
            .map_err(|e| format!("Not a profile bundle: {e}"))?;
        if out.len() as u64 > MAX_BUNDLE_BYTES {
            return Err("This file is too large to be a profile bundle".into());
        }
        Ok(out)
    })
    .await
    .map_err(|e| format!("decompression task failed: {e}"))??;

    let raw: serde_json::Value =
        serde_json::from_slice(&json_bytes).map_err(|e| format!("Bundle parse error: {e}"))?;
    let files = bundle_files(&raw)?;

    let data_dir = paths::default_data_dir();
    tokio::fs::create_dir_all(&data_dir).await.cmd_err()?;

    // Only now: remove the old settings files and write the bundle's.
    {
        let mut rd = tokio::fs::read_dir(&data_dir)
            .await
            .map_err(|e| format!("Cannot read data dir: {e}"))?;
        while let Some(entry) = rd
            .next_entry()
            .await
            .map_err(|e| format!("Cannot read dir entry: {e}"))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") && !EXPORT_EXCLUDE.contains(&name.as_str()) {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
    for (name, content) in &files {
        tokio::fs::write(data_dir.join(name), content)
            .await
            .map_err(|e| format!("Cannot write {name}: {e}"))?;
    }

    let profile_path = paths::default_profile_path();
    let mut state = state.write().await;
    state.ctl.reload_profile(&profile_path).cmd_err()?;
    state.ctl.rebuild_steamcmd();
    state.cached_avatar = None;
    Ok(profile_to_dto(state.ctl.profile()))
}

/// Wipe the entire data directory so the app looks brand-new on next boot.
#[tauri::command]
#[specta::specta]
pub(crate) async fn reset_profile(_state: State<'_, SharedState>) -> Result<(), String> {
    let data_dir = paths::default_data_dir();
    if data_dir.exists() {
        tokio::fs::remove_dir_all(&data_dir)
            .await
            .map_err(|e| format!("Cannot remove data directory: {e}"))?;
    }
    Ok(())
}

/// Restart the application immediately (into the new version after an
/// update: an AppImage restarts from `$APPIMAGE`).
#[tauri::command]
#[specta::specta]
pub(crate) fn restart_app(app: tauri::AppHandle) {
    crate::features::updater::restart(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_json_names_may_be_written() {
        assert!(is_settings_file_name("profile.json"));
        assert!(is_settings_file_name("mods.json"));
        assert!(!is_settings_file_name("../profile.json"));
        assert!(!is_settings_file_name("..\\x.json"));
        assert!(!is_settings_file_name("/etc/x.json"));
        assert!(!is_settings_file_name("sub/x.json"));
        assert!(!is_settings_file_name(".json"));
        assert!(!is_settings_file_name("x.txt"));
        assert!(!is_settings_file_name("server_list_cache.json"));
    }

    #[test]
    fn bundles_are_checked_before_use() {
        let ok = serde_json::json!({"version": 2, "files": {"profile.json": {}, "mods.json": []}});
        assert_eq!(bundle_files(&ok).unwrap().len(), 2);
        let evil =
            serde_json::json!({"version": 2, "files": {"profile.json": {}, "../../x.json": {}}});
        assert!(bundle_files(&evil).is_err());
        let empty = serde_json::json!({"version": 2, "files": {"mods.json": []}});
        assert!(bundle_files(&empty).is_err());
        let v1 = serde_json::json!({"version": 1, "profile": {"player": "x"}});
        assert_eq!(bundle_files(&v1).unwrap()[0].0, "profile.json");
        assert!(bundle_files(&serde_json::json!({"version": 9})).is_err());
    }
}
