//! Article images, fetched through Rust (DayZ's CDN certificate fails
//! validation) and cached on disk, where the window loads them from. The
//! cache is the one the Tauri shell kept (`paths::images_cache_dir`).
//!
//! File names are a stable hash of the URL (FNV-1a), so a new Rust release does
//! not move them; a picture cached under the Tauri-era name (std's unspecified
//! `DefaultHasher`) is renamed on its next use. The folder is kept under
//! [`CACHE_MAX_BYTES`] and [`CACHE_MAX_AGE`], once per run, and a download
//! larger than [`MAX_IMAGE_BYTES`] is dropped before it reaches the disk.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

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

/// A picture larger than this is not one the news shows (the window drops what
/// passes 8 MB too).
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
/// The cache folder is pruned, oldest first, down to this.
const CACHE_MAX_BYTES: u64 = 100 * 1024 * 1024;
/// A cached picture untouched this long goes.
const CACHE_MAX_AGE: Duration = Duration::from_secs(60 * 24 * 3600);

/// Turn a URL into a deterministic, filesystem-safe filename that stays the
/// same across Rust releases.
fn url_to_cache_filename(url: &str) -> String {
    let hash = url.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}.{}", extension(url))
}

/// The name the Tauri shell and earlier versions used (std's `DefaultHasher`,
/// whose algorithm may change with the compiler): looked up once so their
/// pictures are not fetched again.
fn legacy_cache_filename(url: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    url.hash(&mut h);
    format!("{:016x}.{}", h.finish(), extension(url))
}

/// The URL's picture extension, `jpg` when it names none.
fn extension(url: &str) -> String {
    url.rsplit('/')
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
        .unwrap_or_else(|| "jpg".to_string())
}

/// The cached file for `url` in `dir`, when there is one (a legacy name is
/// renamed to the stable one on the way).
fn cached(dir: &Path, url: &str) -> Option<PathBuf> {
    let path = dir.join(url_to_cache_filename(url));
    if path.exists() {
        return Some(path);
    }
    let legacy = dir.join(legacy_cache_filename(url));
    if legacy.exists() {
        return Some(match std::fs::rename(&legacy, &path) {
            Ok(()) => path,
            Err(_) => legacy,
        });
    }
    None
}

/// Keep the cache folder bounded: files older than [`CACHE_MAX_AGE`] go, then
/// the oldest until the rest fits in [`CACHE_MAX_BYTES`]. Blocking.
fn prune(dir: &Path, now: SystemTime) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = read
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok().filter(std::fs::Metadata::is_file)?;
            let at = m.modified().unwrap_or(now);
            Some((at, m.len(), e.path()))
        })
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    let mut kept = 0u64;
    for (at, len, path) in files {
        let old = now.duration_since(at).unwrap_or_default() > CACHE_MAX_AGE;
        if old || kept + len > CACHE_MAX_BYTES {
            let _ = std::fs::remove_file(path);
        } else {
            kept += len;
        }
    }
}

/// Prune the cache once per run, on the blocking pool.
fn prune_once(dir: &Path) {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) {
        return;
    }
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || prune(&dir, SystemTime::now()));
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
        prune_once(&cache_dir);
        let (dir, u) = (cache_dir.clone(), url.clone());
        if let Ok(Some(path)) = tokio::task::spawn_blocking(move || cached(&dir, &u)).await {
            return Ok(path);
        }
        let path = cache_dir.join(url_to_cache_filename(&url));

        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(format!("Not a web image: {url}"));
        }
        let client = if on_dayz(&url) {
            crate::net::dayz_cdn()
        } else {
            crate::net::pictures()
        };
        let mut resp = send_ok(client.get(&url)).await.http_err("Image download")?;
        // An image, not the error page some CDNs answer with a 200.
        if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE)
            && !ct.to_str().unwrap_or("").starts_with("image/")
        {
            return Err(format!("Not an image: {url}"));
        }
        let too_big = || format!("Image too large: {url}");
        if resp
            .content_length()
            .is_some_and(|n| n > MAX_IMAGE_BYTES as u64)
        {
            return Err(too_big());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = resp.chunk().await.http_err("Image download")? {
            if bytes.len() + chunk.len() > MAX_IMAGE_BYTES {
                return Err(too_big());
            }
            bytes.extend_from_slice(&chunk);
        }

        let target = path.clone();
        tokio::task::spawn_blocking(move || dz_common::fs::write_atomic(&target, &bytes))
            .await
            .cmd_err()?
            .map_err(|e| format!("Failed to write cache file: {e}"))?;

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
                    let path = cached(&cache_dir, &url)?;
                    Some((url, path))
                })
                .collect()
        })
        .await
        .cmd_err()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stable_and_keep_the_extension() {
        // FNV-1a of the URL: the same on every Rust release.
        assert_eq!(
            url_to_cache_filename("https://dayz.com/a.PNG?x=1"),
            format!(
                "{:016x}.png",
                "https://dayz.com/a.PNG?x=1"
                    .bytes()
                    .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b))
                        .wrapping_mul(0x0100_0000_01b3))
            )
        );
        assert_eq!(url_to_cache_filename("").len(), 16 + 4);
        assert!(url_to_cache_filename("https://x/y").ends_with(".jpg"));
    }

    #[test]
    fn a_legacy_name_is_found_and_renamed() {
        let dir = std::env::temp_dir().join(format!("dzch-images-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let url = "https://files.example/p.webp";
        std::fs::write(dir.join(legacy_cache_filename(url)), b"img").unwrap();
        let got = cached(&dir, url).unwrap();
        assert_eq!(got, dir.join(url_to_cache_filename(url)));
        assert_eq!(std::fs::read(&got).unwrap(), b"img");
        assert!(cached(&dir, "https://files.example/other.png").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_cache_is_pruned_oldest_first() {
        let dir = std::env::temp_dir().join(format!("dzch-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jpg"), b"x").unwrap();
        // Far in the future, everything is past its age.
        prune(&dir, SystemTime::now() + CACHE_MAX_AGE * 2);
        assert!(!dir.join("a.jpg").exists());
        std::fs::write(dir.join("b.jpg"), b"x").unwrap();
        prune(&dir, SystemTime::now());
        assert!(dir.join("b.jpg").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

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
