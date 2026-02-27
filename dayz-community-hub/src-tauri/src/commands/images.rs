use tauri::{AppHandle, Manager};

use crate::state::insecure_client;

/// Resolve the base app-data directory via Tauri's path resolver.
fn base_data_dir_from_app(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))
}

/// Resolve the on-disk image cache directory (sync — for use in sync contexts).
fn image_cache_dir_sync(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = base_data_dir_from_app(app)?.join("cache").join("images");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create cache dir: {e}"))?;
    Ok(dir)
}

/// Resolve the on-disk image cache directory (async — for use in async commands).
async fn image_cache_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = base_data_dir_from_app(app)?.join("cache").join("images");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create cache dir: {e}"))?;
    Ok(dir)
}

/// Turn a URL into a deterministic, filesystem-safe filename.
fn url_to_cache_filename(url: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    url.hash(&mut h);
    let hash = format!("{:016x}", h.finish());
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|seg| {
            let seg = seg.split('?').next().unwrap_or(seg);
            seg.rsplit('.').next()
        })
        .and_then(|e| {
            let e = e.to_lowercase();
            if matches!(
                e.as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "avif"
            ) {
                Some(e)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "jpg".to_string());
    format!("{hash}.{ext}")
}

/// Convert a path to a string with forward slashes on all platforms.
fn path_to_forward_slashes(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Fetch an image URL through Rust (bypasses WebKit TLS cert validation),
/// cache it to disk, and return the local file path.
#[tauri::command]
pub(crate) async fn fetch_image(app: AppHandle, url: String) -> Result<String, String> {
    let cache_dir = image_cache_dir(&app).await?;
    let filename = url_to_cache_filename(&url);
    let path = cache_dir.join(&filename);

    if path.exists() {
        return Ok(path_to_forward_slashes(&path));
    }

    let client = insecure_client();
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, &bytes)
        .await
        .map_err(|e| format!("Failed to write cache file: {e}"))?;
    tokio::fs::rename(&tmp, &path)
        .await
        .map_err(|e| format!("Failed to rename cache file: {e}"))?;

    Ok(path_to_forward_slashes(&path))
}

/// Resolve multiple image URLs at once. Returns a Vec of (url, local_path)
/// for every URL that is already cached on disk — no network requests.
#[tauri::command]
pub(crate) fn resolve_cached_images(
    app: AppHandle,
    urls: Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let cache_dir = image_cache_dir_sync(&app)?;
    let mut result = Vec::with_capacity(urls.len());
    for url in urls {
        let filename = url_to_cache_filename(&url);
        let path = cache_dir.join(&filename);
        if path.exists() {
            result.push((url, path_to_forward_slashes(&path)));
        }
    }
    Ok(result)
}
