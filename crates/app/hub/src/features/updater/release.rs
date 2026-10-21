//! The release manifest and the downloads it names, as tauri-plugin-updater
//! read them: `latest.json` (`version`, `notes`, `pub_date`, `platforms`
//! keyed `{os}-{arch}[-{installer}]`, each with a `url` and a base64
//! minisign `signature`), checked against the public key of
//! `plugins.updater.pubkey` in `tauri.conf.json`.

use std::cmp::Ordering;
use std::collections::HashMap;

use base64::Engine;
use serde::Deserialize;

use super::{DownloadEvent, Reporter};

/// `plugins.updater.endpoints` in `tauri.conf.json`.
pub(crate) const ENDPOINT: &str =
    "https://git.thoxy.xyz/thoxy/dayz-community-hub/releases/download/latest/latest.json";

/// `plugins.updater.pubkey` in `tauri.conf.json`: a minisign public key,
/// base64 of its two-line text form.
pub(crate) const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDhGNEMzMUQyRDA3NzkwMkIKUldRcmtIZlEwakZNaitxdFFGOU54YkxrTXNkNGR5TWhtRG90Yy9iTEd3TXZOME1ubHc0REVoMFcK";

/// The largest download accepted: a release is tens of MB.
const MAX_DOWNLOAD: usize = 1024 * 1024 * 1024;

/// A newer release than the one running, as the last check found it.
#[derive(Clone, Debug)]
pub(crate) struct Update {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    /// `pub_date` as the manifest wrote it (RFC 3339).
    pub date: Option<String>,
    pub download_url: String,
    /// Base64 of the minisign signature file.
    pub signature: String,
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
    #[serde(default)]
    platforms: HashMap<String, Platform>,
}

#[derive(Deserialize)]
struct Platform {
    signature: String,
    url: String,
}

fn os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "i686"
    } else {
        "x86_64"
    }
}

/// Ask the endpoint for the manifest; an update when it names a newer
/// version than `current`. `installer` is the bundle kind tried before the
/// bare `{os}-{arch}` key (`appimage`), as the plugin did.
pub(crate) async fn check(
    current: &str,
    installer: Option<&str>,
) -> Result<Option<Update>, String> {
    let resp = crate::net::api()
        .get(ENDPOINT)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?;
    if resp.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    let resp = resp
        .error_for_status()
        .map_err(|e| e.without_url().to_string())?;
    let manifest: Manifest = resp.json().await.map_err(|e| e.without_url().to_string())?;
    from_manifest(manifest, current, installer)
}

fn from_manifest(
    manifest: Manifest,
    current: &str,
    installer: Option<&str>,
) -> Result<Option<Update>, String> {
    let newer = compare_versions(&manifest.version, current)
        .ok_or_else(|| format!("unreadable version {:?}", manifest.version))?
        == Ordering::Greater;
    if !newer {
        return Ok(None);
    }
    let (os, arch) = (os(), arch());
    let mut targets = Vec::new();
    if let Some(i) = installer {
        targets.push(format!("{os}-{arch}-{i}"));
    }
    targets.push(format!("{os}-{arch}"));
    let platform = targets
        .iter()
        .find_map(|t| manifest.platforms.get(t))
        .ok_or_else(|| format!("the release has none of {}", targets.join(", ")))?;
    Ok(Some(Update {
        version: manifest.version.trim_start_matches('v').to_owned(),
        current_version: current.to_owned(),
        body: manifest.notes,
        date: manifest.pub_date,
        download_url: platform.url.clone(),
        signature: platform.signature.clone(),
    }))
}

/// A version as semver orders it: the numbers, then a pre-release (which
/// comes before the release it precedes). Build metadata is ignored.
fn parse_version(v: &str) -> Option<([u64; 3], Option<Vec<String>>)> {
    let v = v.trim().trim_start_matches('v');
    let v = v.split('+').next().unwrap_or(v);
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, Some(p.split('.').map(str::to_owned).collect())),
        None => (v, None),
    };
    let mut nums = [0u64; 3];
    let mut parts = core.split('.');
    for n in &mut nums {
        *n = parts.next()?.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some((nums, pre))
}

/// `a` against `b`, as semver orders them; `None` when either is not a version.
pub(crate) fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    let (an, ap) = parse_version(a)?;
    let (bn, bp) = parse_version(b)?;
    Some(an.cmp(&bn).then_with(|| match (ap, bp) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => {
            for (x, y) in a.iter().zip(b.iter()) {
                let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
            a.len().cmp(&b.len())
        }
    }))
}

/// Download `url` into memory, reporting `Started` and then `Progress` at
/// most every 100 ms.
pub(crate) async fn download(url: &str, on_event: &Reporter) -> Result<Vec<u8>, String> {
    let mut response = crate::net::download()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("download failed: {}", e.without_url()))?;
    let content_length = response.content_length();
    on_event.send(DownloadEvent::Started { content_length });
    let capacity = content_length
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0)
        .min(MAX_DOWNLOAD);
    let mut bytes: Vec<u8> = Vec::with_capacity(capacity);
    let mut last_emit = std::time::Instant::now();
    let mut pending: usize = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("download error: {}", e.without_url()))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_DOWNLOAD {
            return Err("download error: the file is far too large".into());
        }
        pending += chunk.len();
        bytes.extend_from_slice(&chunk);
        if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
            on_event.send(DownloadEvent::Progress {
                chunk_length: std::mem::take(&mut pending),
            });
            last_emit = std::time::Instant::now();
        }
    }
    if pending > 0 {
        on_event.send(DownloadEvent::Progress {
            chunk_length: pending,
        });
    }
    Ok(bytes)
}

fn base64_text(what: &str, b64: &str) -> Result<String, String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("{what} base64 decode: {e}"))?;
    String::from_utf8(raw).map_err(|e| format!("{what} utf8: {e}"))
}

/// Check `data` against the base64 minisign `signature` and public key.
/// With `announced`, a version the signature's trusted comment records
/// (`version:1.2.3`, as the Tauri CLI signs) must be that one: the manifest
/// itself is not signed.
pub(crate) fn verify_signature(
    data: &[u8],
    signature: &str,
    pubkey: &str,
    allow_legacy: bool,
    announced: Option<&str>,
) -> Result<(), String> {
    let public_key = minisign_verify::PublicKey::decode(&base64_text("pubkey", pubkey)?)
        .map_err(|e| format!("pubkey decode: {e}"))?;
    let signature = minisign_verify::Signature::decode(&base64_text("signature", signature)?)
        .map_err(|e| format!("signature decode: {e}"))?;
    public_key
        .verify(data, &signature, allow_legacy)
        .map_err(|e| format!("signature verification failed: {e}"))?;
    if let Some(announced) = announced
        && let Some(signed) = signature
            .trusted_comment()
            .split('\t')
            .find_map(|f| f.strip_prefix("version:"))
        && compare_versions(signed, announced) != Some(Ordering::Equal)
        && signed != announced
    {
        return Err(format!(
            "the download is signed for version {signed}, not {announced}"
        ));
    }
    Ok(())
}

/// Write `bytes` over the AppImage at `image`, keeping its permissions. The
/// old image is moved aside first, into a private directory on the same
/// filesystem (a rename cannot cross filesystems), and moved back if the new
/// one cannot be written; it is removed once the new one is in place.
#[cfg(unix)]
pub(crate) fn replace_appimage(image: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let meta = std::fs::metadata(image).map_err(|e| format!("{}: {e}", image.display()))?;
    let permissions = meta.permissions();
    let candidates = [
        Some(std::env::temp_dir()),
        dirs::cache_dir(),
        image.parent().map(std::path::Path::to_path_buf),
    ];
    for base in candidates.into_iter().flatten() {
        let Ok(base_meta) = std::fs::metadata(&base) else {
            continue;
        };
        if base_meta.dev() != meta.dev() {
            continue;
        }
        let dir = base.join(format!(".dzch-current-app-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        if std::fs::create_dir(&dir).is_err() {
            continue;
        }
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        let backup = dir.join("current_app.AppImage");
        std::fs::rename(image, &backup).map_err(|e| format!("moving the old image aside: {e}"))?;
        let written = std::fs::write(image, bytes)
            .and_then(|()| std::fs::set_permissions(image, permissions.clone()));
        return match written {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&dir);
                Ok(())
            }
            Err(e) => {
                let _ = std::fs::rename(&backup, image);
                let _ = std::fs::remove_dir_all(&dir);
                Err(format!("writing the new image: {e}"))
            }
        };
    }
    Err("no temporary directory on the AppImage's filesystem".into())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;

    #[test]
    fn versions_order_as_semver() {
        assert_eq!(compare_versions("0.5.5", "0.5.4"), Some(Ordering::Greater));
        assert_eq!(compare_versions("v0.5.4", "0.5.4"), Some(Ordering::Equal));
        assert_eq!(compare_versions("0.10.0", "0.9.9"), Some(Ordering::Greater));
        assert_eq!(
            compare_versions("1.0.0-rc.1", "1.0.0"),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare_versions("1.0.0-rc.2", "1.0.0-rc.10"),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare_versions("1.0.0+build", "1.0.0"),
            Some(Ordering::Equal)
        );
        assert_eq!(compare_versions("1.0", "1.0.0"), None);
    }

    fn manifest(version: &str, keys: &[&str]) -> Manifest {
        Manifest {
            version: version.into(),
            notes: Some("notes".into()),
            pub_date: Some("2026-10-13T01:13:52Z".into()),
            platforms: keys
                .iter()
                .map(|k| {
                    (
                        (*k).to_owned(),
                        Platform {
                            signature: format!("sig-{k}"),
                            url: format!("https://x/{k}"),
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn a_newer_release_names_this_platforms_download() {
        let key = format!("{}-{}", os(), arch());
        let u = from_manifest(manifest("0.6.0", &[&key]), "0.5.4", None)
            .unwrap()
            .unwrap();
        assert_eq!(u.version, "0.6.0");
        assert_eq!(u.download_url, format!("https://x/{key}"));
        assert_eq!(u.date.as_deref(), Some("2026-10-13T01:13:52Z"));
        // The installer's own key wins over the bare one.
        let special = format!("{key}-appimage");
        let u = from_manifest(
            manifest("0.6.0", &[&key, &special]),
            "0.5.4",
            Some("appimage"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(u.signature, format!("sig-{special}"));
        // Not newer: nothing, even without this platform.
        assert!(
            from_manifest(manifest("0.5.4", &[]), "0.5.4", None)
                .unwrap()
                .is_none()
        );
        assert!(from_manifest(manifest("0.6.0", &[]), "0.5.4", None).is_err());
    }

    #[test]
    fn a_bad_signature_is_refused() {
        let sig = base64::engine::general_purpose::STANDARD.encode("not a signature");
        assert!(verify_signature(b"data", &sig, PUBKEY, true, None).is_err());
        // The key itself decodes.
        assert!(
            minisign_verify::PublicKey::decode(&base64_text("pubkey", PUBKEY).unwrap()).is_ok()
        );
    }
}
