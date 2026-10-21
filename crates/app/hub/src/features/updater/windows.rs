//! The Windows self-updater: download the release zip, check its minisign
//! signature, swap the executable in place and relaunch. The swap is ours
//! rather than a crate's, so it can put the old executable back when the new
//! one cannot be moved in (an antivirus holding it).

use std::path::Path;

use super::release::{self, Update};
use super::{DownloadEvent, Reporter};

/// Download the zip `update` names (the manifest's `windows-x86_64` entry),
/// verify it against the public key, and put its executable in place of this
/// one. The window restarts through `restart_app`.
pub(super) async fn install(update: &Update, on_event: Reporter) -> Result<(), String> {
    let zip_bytes = release::download(&update.download_url, &on_event).await?;
    // Verify, unpack, swap: CPU and disk work, off the async runtime.
    let signature = update.signature.clone();
    tokio::task::spawn_blocking(move || verify_and_replace(&zip_bytes, &signature))
        .await
        .map_err(|e| format!("update task: {e}"))??;
    on_event.send(DownloadEvent::Finished);
    Ok(())
}

/// Check `zip_bytes` against the minisign `signature` and the public key,
/// take the executable out of it and put it in place of the running one.
fn verify_and_replace(zip_bytes: &[u8], signature: &str) -> Result<(), String> {
    release::verify_signature(zip_bytes, signature, release::PUBKEY, false, None)?;

    // ── Extract exe from zip ───────────────────────────────────────────
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("zip open: {e}"))?;

    // The executable, found by name rather than assumed to be the first entry.
    let exe_index = (0..archive.len())
        .find(|&i| {
            archive
                .name_for_index(i)
                .is_some_and(|n| n.to_ascii_lowercase().ends_with(".exe"))
        })
        .ok_or_else(|| "the update archive holds no .exe".to_string())?;
    let mut exe_bytes: Vec<u8> = Vec::new();
    {
        let mut entry = archive
            .by_index(exe_index)
            .map_err(|e| format!("zip entry: {e}"))?;
        exe_bytes.reserve(entry.size() as usize);
        std::io::Read::read_to_end(&mut entry, &mut exe_bytes)
            .map_err(|e| format!("zip read: {e}"))?;
    }

    // ── Write new exe to a temp path beside the current exe ───────────
    let current_exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "no parent dir".to_string())?;

    let new_exe = exe_dir.join(NEW_EXE);
    // A leftover from an update that stopped halfway is replaced.
    let _ = std::fs::remove_file(&new_exe);
    std::fs::write(&new_exe, &exe_bytes).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "cannot write in {}: move the launcher to a folder you can write to \
                 (not Program Files), or download the new version by hand",
                exe_dir.display()
            )
        } else {
            format!("write new exe: {e}")
        }
    })?;

    // ── Swap: running exe aside, new exe in, old one back on failure ───
    let swapped = swap(&current_exe, &new_exe, &exe_dir.join(OLD_EXE));
    let _ = std::fs::remove_file(&new_exe);
    swapped.map_err(|e| format!("could not replace the executable: {e}"))
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
