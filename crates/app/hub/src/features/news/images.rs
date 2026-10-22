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

/// Whether `url` is on dayz.com, whose certificate fails validation: only its
/// pictures go through the client that does not check.
fn on_dayz(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    let host = host.split(':').next().unwrap_or("").to_ascii_lowercase();
    host == "dayz.com" || host.ends_with(".dayz.com")
}

impl Hub {
    /// Fetch an image URL, cache it to disk, and return the cached file's path.
    pub async fn fetch_image(&self, url: String) -> Result<PathBuf, String> {
        mocked!(self, w => w.news_image(&url).await);
        let cache_dir = image_cache_dir().await?;
        let filename = url_to_cache_filename(&url);
        let path = cache_dir.join(&filename);

        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(path);
        }

        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(format!("Not a web image: {url}"));
        }
        let client = if on_dayz(&url) {
            crate::net::dayz_cdn()
        } else {
            crate::net::pictures()
        };
        let resp = send_ok(client.get(&url)).await.http_err("Image download")?;
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

#[cfg(test)]
mod tests {
    use super::on_dayz;

    #[test]
    fn only_dayz_skips_certificates() {
        assert!(on_dayz("https://dayz.com/app-static/a.png"));
        assert!(on_dayz("https://cdn.DayZ.com:443/a.png"));
        assert!(!on_dayz("https://files.bistudio.com/public/x"));
        assert!(!on_dayz("https://evil.com/dayz.com"));
        assert!(!on_dayz("https://dayz.com@evil.com/a.png"));
        assert!(!on_dayz("https://notdayz.com/a.png"));
    }
}
