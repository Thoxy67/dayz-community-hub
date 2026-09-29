use base64::Engine;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, State, ipc::Channel};
use tauri_plugin_updater::UpdaterExt;

// ── Error type ────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error(transparent)]
    Updater(#[from] tauri_plugin_updater::Error),
    #[error("no pending update — call check_for_update first")]
    NoPendingUpdate,
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

// ── Download progress events sent over a Channel ──────────────────────────

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum DownloadEvent {
    Started { content_length: Option<u64> },
    Progress { chunk_length: usize },
    Finished,
}

// ── Pending update state ──────────────────────────────────────────────────

/// Holds the download URL + minisign signature from the last check_for_update.
pub struct PendingUpdate(pub Mutex<Option<PendingUpdateData>>);

pub struct PendingUpdateData {
    pub url: String,
    pub signature: String,
}

// ── DTO returned to the frontend ──────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

// ── Commands ──────────────────────────────────────────────────────────────

/// Check for a new version.
#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>> {
    fetch_update_info(&app, &pending).await
}

/// Download and install the update that was fetched by check_for_update.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
    on_event: Channel<DownloadEvent>,
) -> Result<()> {
    do_install(&app, &pending, on_event).await
}

// ── Internals ─────────────────────────────────────────────────────────────

async fn fetch_update_info(
    app: &AppHandle,
    pending: &State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>> {
    let update = match app.updater()?.check().await? {
        Some(u) => u,
        None => {
            *pending.0.lock().unwrap() = None;
            return Ok(None);
        }
    };

    let info = UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        body: update.body.clone(),
        date: update.date.map(|d| {
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                d.year(),
                d.month() as u8,
                d.day(),
                d.hour(),
                d.minute(),
                d.second(),
            )
        }),
    };

    let url = update.download_url.to_string();
    let signature = update.signature.clone();

    *pending.0.lock().unwrap() = Some(PendingUpdateData { url, signature });

    Ok(Some(info))
}

async fn do_install(
    app: &AppHandle,
    pending: &State<'_, PendingUpdate>,
    on_event: Channel<DownloadEvent>,
) -> Result<()> {
    let data = pending
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or(UpdateError::NoPendingUpdate)?;

    // ── 1. Download ───────────────────────────────────────────────────────
    let client = reqwest::Client::new();
    let response = client
        .get(&data.url)
        .send()
        .await
        .map_err(|e| UpdateError::Other(format!("download failed: {e}")))?;

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
            .decode(&data.signature)
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

    let mut exe_bytes: Vec<u8> = Vec::new();
    {
        let mut entry = archive
            .by_index(0)
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

    let _ = on_event.send(DownloadEvent::Finished);

    // ── 5. Replace this exe with the new one and relaunch ─────────────────
    let new_exe_canon = new_exe
        .canonicalize()
        .map_err(|e| UpdateError::Other(format!("canonicalize new exe: {e}")))?;

    self_replace::self_replace(&new_exe_canon)
        .map_err(|e| UpdateError::Other(format!("self_replace failed: {e}")))?;

    let _ = std::fs::remove_file(&new_exe_canon);

    use std::os::windows::process::CommandExt;
    std::process::Command::new(&current_exe)
        .creation_flags(0x00000008) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| UpdateError::Other(format!("relaunch failed: {e}")))?;

    std::process::exit(0);
}
