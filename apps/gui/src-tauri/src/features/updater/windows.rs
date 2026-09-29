//! The Windows self-updater: download the release zip, check its minisign
//! signature, swap the executable in place and relaunch.

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

    // ── 2. Verify minisign signature ──────────────────────────────────────
    let pubkey_b64 = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|v| v.get("pubkey"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| UpdateError::Other("updater pubkey not found in config".into()))?;

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
        .verify(&zip_bytes, &signature, false)
        .map_err(|e| UpdateError::Other(format!("signature verification failed: {e}")))?;

    // ── 3. Extract exe from zip ───────────────────────────────────────────
    let cursor = std::io::Cursor::new(&zip_bytes);
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
        std::io::Read::read_to_end(&mut entry, &mut exe_bytes)
            .map_err(|e| UpdateError::Other(format!("zip read: {e}")))?;
    }

    // ── 4. Write new exe to a temp path beside the current exe ───────────
    let current_exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| UpdateError::Other(format!("current_exe: {e}")))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| UpdateError::Other("no parent dir".into()))?;

    let new_exe = exe_dir.join("dayz-community-hub-update.exe");
    std::fs::write(&new_exe, &exe_bytes)
        .map_err(|e| UpdateError::Other(format!("write new exe: {e}")))?;

    // ── 5. Replace this exe with the new one ────────────────────────────────
    let new_exe_canon = new_exe
        .canonicalize()
        .map_err(|e| UpdateError::Other(format!("canonicalize new exe: {e}")))?;
    self_replace::self_replace(&new_exe_canon)
        .map_err(|e| UpdateError::Other(format!("self_replace failed: {e}")))?;
    let _ = std::fs::remove_file(&new_exe_canon);

    let _ = on_event.send(DownloadEvent::Finished);
    Ok(())
}
