use dayz_community_hub_core::config;
use tauri::State;

use crate::convert::profile_to_dto;
use crate::dto::ProfileDto;
use crate::state::SharedState;
use crate::utils::error::ResultExt;

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
pub(crate) async fn export_profile(path: String, include_mods: bool) -> Result<(), String> {
    let data_dir = config::default_data_dir();

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

/// Import a profile bundle, overwriting all settings files.
#[tauri::command]
pub(crate) async fn import_profile(
    path: String,
    state: State<'_, SharedState>,
) -> Result<ProfileDto, String> {
    let compressed = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Cannot read import file: {e}"))?;

    let json_bytes =
        tokio::task::spawn_blocking(move || zstd::decode_all(std::io::Cursor::new(&compressed)))
            .await
            .map_err(|e| format!("decompression task failed: {e}"))?
            .map_err(|e| format!("zstd decompression failed: {e}"))?;

    let raw: serde_json::Value =
        serde_json::from_slice(&json_bytes).map_err(|e| format!("Bundle parse error: {e}"))?;

    let version = raw["version"].as_u64().unwrap_or(1) as u8;

    let data_dir = config::default_data_dir();
    tokio::fs::create_dir_all(&data_dir).await.cmd_err()?;

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

    match version {
        1 => {
            let profile_str = serde_json::to_string_pretty(&raw["profile"]).cmd_err()?;
            tokio::fs::write(data_dir.join("profile.json"), &profile_str)
                .await
                .map_err(|e| format!("Cannot write profile.json: {e}"))?;
            if !raw["mods"].is_null() {
                let mods_str = serde_json::to_string_pretty(&raw["mods"]).cmd_err()?;
                tokio::fs::write(data_dir.join("mods.json"), &mods_str)
                    .await
                    .map_err(|e| format!("Cannot write mods.json: {e}"))?;
            }
        }
        2 => {
            let files = raw["files"].as_object().ok_or("Bundle files map missing")?;
            for (filename, value) in files {
                let content = serde_json::to_string_pretty(value).cmd_err()?;
                tokio::fs::write(data_dir.join(filename), &content)
                    .await
                    .map_err(|e| format!("Cannot write {filename}: {e}"))?;
            }
        }
        v => return Err(format!("Unsupported bundle version {v}")),
    }

    let profile_path = config::default_profile_path();
    let mut state = state.write().await;
    state.ctl.reload_profile(&profile_path).cmd_err()?;
    state.ctl.rebuild_steamcmd();
    state.cached_avatar = None;
    Ok(profile_to_dto(state.ctl.profile()))
}

/// Wipe the entire data directory so the app looks brand-new on next boot.
#[tauri::command]
pub(crate) async fn reset_profile(_state: State<'_, SharedState>) -> Result<(), String> {
    let data_dir = config::default_data_dir();
    if data_dir.exists() {
        tokio::fs::remove_dir_all(&data_dir)
            .await
            .map_err(|e| format!("Cannot remove data directory: {e}"))?;
    }
    Ok(())
}

/// Restart the application immediately.
#[tauri::command]
pub(crate) fn restart_app(app: tauri::AppHandle) {
    app.restart();
}
