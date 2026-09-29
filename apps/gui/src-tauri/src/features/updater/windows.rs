//! The Windows self-updater: download the release zip, check its minisign
//! signature, swap the executable in place and relaunch. The swap is ours
//! rather than a crate's, so it can put the old executable back when the new
//! one cannot be moved in (an antivirus holding it).

use std::path::Path;

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, ipc::Channel};
use tauri_plugin_updater::Update;

use super::DownloadEvent;

// ── Error type ────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error(transparent)]
    Updater(#[from] tauri_plugin_updater::Error),
    #[error("update error: {0}")]
    Other(String),
}

impl Serialize for UpdateError {
    fn serialize<S>(&self, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        s.serialize_str(&self.to_string())
    }
}

impl From<String> for UpdateError {
    fn from(s: String) -> Self {
        UpdateError::Other(s)
    }
}
impl From<&str> for UpdateError {
    fn from(s: &str) -> Self {
        UpdateError::Other(s.to_string())
    }
}

type Result<T> = std::result::Result<T, UpdateError>;

// ── Install ───────────────────────────────────────────────────────────────

/// Download the zip `update` names (the manifest's `windows-x86_64` entry),
/// verify it against the configured public key, and put its executable in
/// place of this one. The window restarts through `restart_app`.
pub(super) async fn install(
    app: &AppHandle,
    update: &Update,
    on_event: Channel<DownloadEvent>,
) -> Result<()> {
    let url = update.download_url.to_string();
    let signature = &update.signature;

    // ── 1. Download ───────────────────────────────────────────────────────
    let response = crate::net::download()
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| UpdateError::Other(format!("download failed: {}", e.without_url())))?;

    let content_length = response.content_length();
    let _ = on_event.send(DownloadEvent::Started { content_length });

    let mut zip_bytes: Vec<u8> = Vec::with_capacity(content_length.unwrap_or(0) as usize);
    let mut stream = response.bytes_stream();
    let mut last_emit = std::time::Instant::now();
    let mut pending_bytes: usize = 0;

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| UpdateError::Other(format!("download error: {e}")))?;
        pending_bytes += chunk.len();
        zip_bytes.extend_from_slice(&chunk);
        if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
            let _ = on_event.send(DownloadEvent::Progress {
                chunk_length: pending_bytes,
            });
            pending_bytes = 0;
            last_emit = std::time::Instant::now();
        }
    }
    if pending_bytes > 0 {
        let _ = on_event.send(DownloadEvent::Progress {
            chunk_length: pending_bytes,
        });
    }

    // ── 2-5. Verify, unpack, swap: CPU and disk work, off the async runtime
    let pubkey_b64 = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|v| v.get("pubkey"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| UpdateError::Other("updater pubkey not found in config".into()))?
        .to_owned();
    let signature = signature.clone();
    tokio::task::spawn_blocking(move || verify_and_replace(&zip_bytes, &pubkey_b64, &signature))
        .await
        .map_err(|e| UpdateError::Other(format!("update task: {e}")))??;

    let _ = on_event.send(DownloadEvent::Finished);
    Ok(())
}

/// Check `zip_bytes` against the minisign `signature` and the public key,
/// take the executable out of it and put it in place of the running one.
fn verify_and_replace(zip_bytes: &[u8], pubkey_b64: &str, signature: &str) -> Result<()> {
    // ── 2. Verify minisign signature ──────────────────────────────────────
    let pubkey_text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(pubkey_b64)
            .map_err(|e| UpdateError::Other(format!("pubkey base64 decode: {e}")))?,
    )
    .map_err(|e| UpdateError::Other(format!("pubkey utf8: {e}")))?;

    let sig_text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(signature)
            .map_err(|e| UpdateError::Other(format!("signature base64 decode: {e}")))?,
    )
    .map_err(|e| UpdateError::Other(format!("signature utf8: {e}")))?;

    let public_key = minisign_verify::PublicKey::decode(&pubkey_text)
        .map_err(|e| UpdateError::Other(format!("pubkey decode: {e}")))?;
    let signature = minisign_verify::Signature::decode(&sig_text)
        .map_err(|e| UpdateError::Other(format!("signature decode: {e}")))?;
    public_key
        .verify(zip_bytes, &signature, false)
        .map_err(|e| UpdateError::Other(format!("signature verification failed: {e}")))?;

    // ── 3. Extract exe from zip ───────────────────────────────────────────
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| UpdateError::Other(format!("zip open: {e}")))?;

    // The executable, found by name rather than assumed to be the first entry.
    let exe_index = (0..archive.len())
        .find(|&i| {
            archive
                .name_for_index(i)
                .is_some_and(|n| n.to_ascii_lowercase().ends_with(".exe"))
        })
        .ok_or_else(|| UpdateError::Other("the update archive holds no .exe".into()))?;
    let mut exe_bytes: Vec<u8> = Vec::new();
    {
        let mut entry = archive
            .by_index(exe_index)
            .map_err(|e| UpdateError::Other(format!("zip entry: {e}")))?;
        exe_bytes.reserve(entry.size() as usize);
        std::io::Read::read_to_end(&mut entry, &mut exe_bytes)
            .map_err(|e| UpdateError::Other(format!("zip read: {e}")))?;
    }

    // ── 4. Write new exe to a temp path beside the current exe ───────────
    let current_exe =
        std::env::current_exe().map_err(|e| UpdateError::Other(format!("current_exe: {e}")))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| UpdateError::Other("no parent dir".into()))?;

    let new_exe = exe_dir.join(NEW_EXE);
    // A leftover from an update that stopped halfway is replaced.
    let _ = std::fs::remove_file(&new_exe);
    std::fs::write(&new_exe, &exe_bytes).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            UpdateError::Other(format!(
                "cannot write in {}: move the launcher to a folder you can write to \
                 (not Program Files), or download the new version by hand",
                exe_dir.display()
            ))
        } else {
            UpdateError::Other(format!("write new exe: {e}"))
        }
    })?;

    // ── 5. Swap: running exe aside, new exe in, old one back on failure ───
    let swapped = swap(&current_exe, &new_exe, &exe_dir.join(OLD_EXE));
    let _ = std::fs::remove_file(&new_exe);
    swapped.map_err(|e| UpdateError::Other(format!("could not replace the executable: {e}")))
}

/// The new executable, written beside the running one before the swap.
const NEW_EXE: &str = "dayz-community-hub-update.exe";
/// The previous executable, renamed aside by the swap. Windows lets a running
/// program's file be renamed but not deleted: it goes at the next start.
const OLD_EXE: &str = "dayz-community-hub.old.exe";

/// Put `new` at `exe`, the running executable's path, keeping the running
/// file as `old`. Each rename is retried while an antivirus scanner holds the
/// file it has just seen written; if the new file cannot be moved in, the
/// old one is moved back, so a failed update never leaves the folder without
/// a launcher.
fn swap(exe: &Path, new: &Path, old: &Path) -> std::io::Result<()> {
    use dz_common::win::retry_locked;
    let _ = std::fs::remove_file(old);
    retry_locked(|| std::fs::rename(exe, old))?;
    if let Err(e) = retry_locked(|| std::fs::rename(new, exe)) {
        let _ = retry_locked(|| std::fs::rename(old, exe));
        return Err(e);
    }
    Ok(())
}

/// Remove what the last update left beside the executable: the previous
/// version, no longer running.
pub(super) fn remove_leftovers() {
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        let _ = std::fs::remove_file(dir.join(OLD_EXE));
    }
}
