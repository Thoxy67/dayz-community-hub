use crate::Result;
use crate::errors::Error;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledMod {
    pub name: String,
    pub id: u64,
    /// Local install/update time (filesystem mtime of meta.cpp, rewritten by steamcmd on each download)
    pub local_updated: i64,
    pub size: u64,
    pub managed: bool,
}

/// Regexes for parsing meta.cpp — compiled once at first use.
static NAME_RE: OnceLock<Regex> = OnceLock::new();
static ID_RE: OnceLock<Regex> = OnceLock::new();

/// Process-lifetime cache: mod directory path → (dir_mtime_secs, total_size_bytes).
/// A directory's mtime is updated by the OS when its contents change, so this
/// avoids the expensive recursive walk on every scan when nothing has changed.
static SIZE_CACHE: OnceLock<Mutex<HashMap<PathBuf, (u64, u64)>>> = OnceLock::new();

fn size_cache() -> &'static Mutex<HashMap<PathBuf, (u64, u64)>> {
    SIZE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Scan the workshop directory and return all installed mods with metadata.
/// Reads `meta.cpp` from each mod directory to extract name and ID.
pub fn scan_workshop_dir(workshop_path: &Path) -> Result<Vec<InstalledMod>> {
    if !workshop_path.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    let entries = fs::read_dir(workshop_path)?;
    let name_re = NAME_RE.get_or_init(|| Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap());
    let id_re = ID_RE.get_or_init(|| Regex::new(r#"publishedid\s*=\s*(\d+)"#).unwrap());

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("meta.cpp");
        if !meta_path.exists() {
            continue;
        }
        let meta_content = match fs::read_to_string(&meta_path) {
            Ok(c) => c,
            Err(_) => continue, // Skip unreadable mods
        };
        let name = name_re
            .captures(&meta_content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let id = id_re
            .captures(&meta_content)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok());

        // If no ID in meta.cpp, try to use the directory name as the ID
        let id = match id {
            Some(id) => id,
            None => {
                match path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.parse::<u64>().ok())
                {
                    Some(id) => id,
                    None => continue, // Skip mods without any identifiable ID
                }
            }
        };

        // Local install/update time: use meta.cpp mtime (rewritten by steamcmd on each download)
        let local_updated = fs::metadata(&meta_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let size = du_dir_cached(&path).unwrap_or(0);
        let managed = path.join(".dayz-community-hub").exists();

        mods.push(InstalledMod {
            name,
            id,
            local_updated,
            size,
            managed,
        });
    }

    // Sort by name for consistent display
    mods.sort_by_key(|a| a.name.to_lowercase());
    Ok(mods)
}

/// Cached directory size: skips the recursive walk when the directory's mtime
/// has not changed since the last scan.
fn du_dir_cached(path: &Path) -> Result<u64> {
    // Read the directory's own mtime — changes when files are added/removed.
    let dir_mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Check the cache first.
    if let Ok(cache) = size_cache().lock()
        && let Some(&(cached_mtime, cached_size)) = cache.get(path)
        && cached_mtime == dir_mtime
        && dir_mtime != 0
    {
        return Ok(cached_size);
    }

    // Cache miss or mtime changed — do the full walk.
    let size = du_dir(path)?;

    // Update the cache.
    if let Ok(mut cache) = size_cache().lock() {
        cache.insert(path.to_path_buf(), (dir_mtime, size));
    }

    Ok(size)
}

/// Calculate total directory size recursively.
fn du_dir(path: &Path) -> Result<u64> {
    let mut total = 0;
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                total += du_dir(&entry.path())?;
            } else {
                total += meta.len();
            }
        }
    }
    Ok(total)
}

/// Determine which mod IDs from the server are not installed.
/// Uses a HashSet for O(1) lookups instead of O(n) Vec::contains.
pub fn get_missing_mods(server_mods: &[u64], installed_mods: &[InstalledMod]) -> Vec<u64> {
    let installed_ids: HashSet<u64> = installed_mods.iter().map(|m| m.id).collect();
    server_mods
        .iter()
        .filter(|id| !installed_ids.contains(id))
        .cloned()
        .collect()
}

/// Create a symlink from `dayz_path/@<mod_id>` -> `workshop_path/<mod_id>`.
/// This is how DayZ discovers mods at launch time.
pub fn create_mod_symlink(workshop_path: &Path, dayz_path: &Path, mod_id: u64) -> Result<()> {
    let source = workshop_path.join(mod_id.to_string());
    let link_name = format!("@{}", mod_id);
    let target = dayz_path.join(link_name);

    if !source.exists() {
        return Err(Error::Mod(format!(
            "Mod directory does not exist: {:?}",
            source
        )));
    }

    // Remove existing link/file if it exists
    if target.symlink_metadata().is_ok() {
        if target.is_symlink() || target.is_file() {
            fs::remove_file(&target)?;
        } else if target.is_dir() {
            // Use remove_dir (not remove_dir_all) so we never delete mod content.
            // On Windows this removes NTFS junctions; on Linux it removes empty dirs.
            // If it fails (non-empty real dir) we just leave it and try to overwrite.
            let _ = fs::remove_dir(&target);
        }
    }

    #[cfg(unix)]
    symlink(&source, &target)?;

    #[cfg(windows)]
    {
        // Symlinks require admin on Windows; NTFS junctions do not.
        // mklink is a cmd.exe built-in, not a standalone executable, so it
        // must be invoked via `cmd /c "mklink /J ..."`.
        // Paths are double-quoted to handle spaces in directory names.
        use std::os::windows::process::CommandExt;
        let target_str = target.to_string_lossy();
        let source_str = source.to_string_lossy();
        let mklink_cmd = format!("mklink /J \"{target_str}\" \"{source_str}\"");
        let out = std::process::Command::new("cmd")
            .args(["/c", &mklink_cmd])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| Error::Mod(format!("Failed to run mklink: {}", e)))?;
        if !out.status.success() {
            return Err(Error::Mod(format!(
                "mklink /J failed for mod {}: {}",
                mod_id,
                String::from_utf8_lossy(&out.stdout).trim()
            )));
        }
    }

    Ok(())
}

/// Create symlinks for all given mod IDs.
pub fn create_mod_symlinks(
    workshop_path: &Path,
    dayz_path: &Path,
    mod_ids: &[u64],
) -> Result<Vec<u64>> {
    let mut created = Vec::new();
    for &mod_id in mod_ids {
        match create_mod_symlink(workshop_path, dayz_path, mod_id) {
            Ok(_) => created.push(mod_id),
            Err(e) => {
                // Log error but continue with other mods
                eprintln!(
                    "Warning: failed to create symlink for mod {}: {}",
                    mod_id, e
                );
            }
        }
    }
    Ok(created)
}

/// Mark a mod as managed by writing the mod ID to a `.dayz-community-hub` marker file.
/// This matches the bash script's behavior: `echo "$id" > "$dayz_workshop_path/$id/.dayz-community-hub"`
pub fn mark_mod_as_managed(workshop_path: &Path, mod_id: u64) -> Result<()> {
    let managed_file = workshop_path
        .join(mod_id.to_string())
        .join(".dayz-community-hub");
    fs::write(managed_file, mod_id.to_string())?;
    Ok(())
}

/// Remove a single `@<mod_id>` symlink/junction from the DayZ game directory.
pub fn remove_mod_symlink(dayz_path: &Path, mod_id: u64) -> Result<()> {
    let link_name = format!("@{}", mod_id);
    let target = dayz_path.join(link_name);

    if target.symlink_metadata().is_err() {
        return Ok(()); // nothing to remove
    }

    #[cfg(unix)]
    if target.is_symlink() {
        fs::remove_file(&target)?;
    }

    #[cfg(windows)]
    if is_junction(&target) {
        fs::remove_dir(&target)?;
    }

    Ok(())
}

/// Remove all `@*` symlinks from the DayZ game directory.
pub fn remove_all_mod_symlinks(dayz_path: &Path) -> Result<usize> {
    let mut count = 0;
    if !dayz_path.exists() {
        return Ok(0);
    }
    let entries = fs::read_dir(dayz_path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        #[cfg(unix)]
        if file_name.starts_with('@') && path.is_symlink() {
            fs::remove_file(&path)?;
            count += 1;
        }
        // On Windows, mod links are NTFS junctions which appear as directories.
        // We verify the path is a reparse point (junction) before removing so
        // we never accidentally delete a real directory.
        // remove_dir on a junction removes only the junction point, not its target.
        #[cfg(windows)]
        if file_name.starts_with('@') && is_junction(&path) {
            if fs::remove_dir(&path).is_ok() {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Remove all mods that have the `.dayz-community-hub` marker file (managed mods).
pub fn remove_managed_mods(workshop_path: &Path) -> Result<(usize, u64)> {
    let mut count = 0;
    let mut total_size = 0;

    if !workshop_path.exists() {
        return Ok((0, 0));
    }

    let entries = fs::read_dir(workshop_path)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let managed_file = path.join(".dayz-community-hub");
        if !managed_file.exists() {
            continue;
        }

        let size = du_dir(&path).unwrap_or(0);
        fs::remove_dir_all(&path)?;
        count += 1;
        total_size += size;
    }

    Ok((count, total_size))
}

/// Delete a specific mod by ID.
pub fn delete_mod(workshop_path: &Path, mod_id: u64, only_managed: bool) -> Result<()> {
    let mod_path = workshop_path.join(mod_id.to_string());
    if !mod_path.exists() {
        return Err(Error::Mod(format!("Mod {} does not exist", mod_id)));
    }

    if only_managed {
        let managed_file = mod_path.join(".dayz-community-hub");
        if !managed_file.exists() {
            return Err(Error::Mod(format!(
                "Mod {} is not managed (no .dayz-community-hub file)",
                mod_id
            )));
        }
    }

    fs::remove_dir_all(&mod_path)?;
    Ok(())
}

/// Toggle the managed status of a mod.
///
/// When **linking** (marking as managed): writes the `.dayz-community-hub`
/// marker file and creates a symlink/junction `@<mod_id>` in the DayZ
/// game directory so the mod is loaded at launch time.
///
/// When **unlinking** (marking as unmanaged): removes the marker file and
/// the symlink/junction so the mod is no longer loaded.
///
/// Returns the new managed state.
pub fn toggle_mod_managed(workshop_path: &Path, dayz_path: &Path, mod_id: u64) -> Result<bool> {
    let mod_path = workshop_path.join(mod_id.to_string());
    if !mod_path.exists() {
        return Err(Error::Mod(format!("Mod {} does not exist", mod_id)));
    }

    let managed_file = mod_path.join(".dayz-community-hub");
    let currently_managed = managed_file.exists();

    if currently_managed {
        // Unlink: remove marker + symlink
        fs::remove_file(&managed_file)?;
        let _ = remove_mod_symlink(dayz_path, mod_id);
        Ok(false)
    } else {
        // Link: write marker + create symlink
        fs::write(&managed_file, mod_id.to_string())?;
        let _ = create_mod_symlink(workshop_path, dayz_path, mod_id);
        Ok(true)
    }
}

/// Remove all managed mods and all mod symlinks.
pub fn cleanup_mods(workshop_path: &Path, dayz_path: &Path) -> Result<ModManagementStats> {
    let (removed_count, removed_size) = remove_managed_mods(workshop_path)?;
    let symlinks_removed = remove_all_mod_symlinks(dayz_path)?;

    Ok(ModManagementStats {
        removed_count,
        removed_size,
        symlinks_removed,
    })
}

/// Check if a specific mod directory exists in the workshop path.
pub fn mod_exists(workshop_path: &Path, mod_id: u64) -> bool {
    workshop_path.join(mod_id.to_string()).exists()
}

/// Verify that all required mods exist in the workshop directory.
/// Returns a list of mod IDs that are missing.
pub fn verify_mods(workshop_path: &Path, mod_ids: &[u64]) -> Vec<u64> {
    mod_ids
        .iter()
        .filter(|&&id| !mod_exists(workshop_path, id))
        .cloned()
        .collect()
}

/// Format a file size in bytes to a human-readable string.
pub fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1024.0 / 1024.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else if mb >= 1.0 {
        format!("{:.1} MB", mb)
    } else {
        format!("{} KB", bytes / 1024)
    }
}

/// Returns `true` if `path` is an NTFS junction (reparse point).
///
/// On Windows, NTFS junctions have the `FILE_ATTRIBUTE_REPARSE_POINT` flag
/// set in their metadata. This lets us distinguish junctions created by
/// `mklink /J` from real directories that happen to start with `@`.
#[cfg(windows)]
fn is_junction(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    // FILE_ATTRIBUTE_REPARSE_POINT = 0x400
    fs::symlink_metadata(path)
        .map(|m| m.file_attributes() & 0x400 != 0)
        .unwrap_or(false)
}

pub struct ModManagementStats {
    pub removed_count: usize,
    pub removed_size: u64,
    pub symlinks_removed: usize,
}
