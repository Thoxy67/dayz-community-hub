//! Article images, fetched through Rust (DayZ's CDN certificate fails
//! validation) and cached on disk, where the window loads them from. The
//! cache is the one the Tauri shell kept (`paths::images_cache_dir`), with
//! the same file names.

use std::path::PathBuf;

use crate::Hub;
use crate::error::{HttpResultExt, ResultExt, send_ok};

/// The on-disk image cache directory, created.
async fn image_cache_dir() -> Result<PathBuf, String> {
    let dir = crate::paths::images_cache_dir()?;
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

impl Hub {
    /// Fetch an image URL through Rust (bypasses WebKit TLS cert validation),
    /// cache it to disk, and return the cached file's path.
    pub async fn fetch_image(&self, url: String) -> Result<PathBuf, String> {
        mocked!(self => Err(format!("The mock world has no images: {url}")));
        let cache_dir = image_cache_dir().await?;
        let filename = url_to_cache_filename(&url);
        let path = cache_dir.join(&filename);

        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(path);
        }

        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(format!("Not a web image: {url}"));
        }
        let resp = send_ok(crate::net::dayz_cdn().get(&url))
            .await
            .http_err("Image download")?;
        // An image, not the error page some CDNs answer with a 200.
        if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE)
            && !ct.to_str().unwrap_or("").starts_with("image/")
        {
            return Err(format!("Not an image: {url}"));
        }
        let bytes = resp.bytes().await.http_err("Image download")?;

        let tmp = path.with_extension("tmp");
        tokio::fs::write(&tmp, &bytes)
            .await
            .map_err(|e| format!("Failed to write cache file: {e}"))?;
        tokio::fs::rename(&tmp, &path)
            .await
            .map_err(|e| format!("Failed to rename cache file: {e}"))?;

        Ok(path)
    }

    /// Resolve multiple image URLs at once. Returns a Vec of (url, local_path)
    /// for every URL that is already cached on disk — no network requests.
    pub async fn resolve_cached_images(
        &self,
        urls: Vec<String>,
    ) -> Result<Vec<(String, PathBuf)>, String> {
        mocked!(self => Ok(Vec::new()));
        let cache_dir = image_cache_dir().await?;
        tokio::task::spawn_blocking(move || {
            urls.into_iter()
                .filter_map(|url| {
                    let path = cache_dir.join(url_to_cache_filename(&url));
                    path.exists().then_some((url, path))
                })
                .collect()
        })
        .await
        .cmd_err()
    }
}
