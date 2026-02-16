use crate::Result;
use crate::errors::Error;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledMod {
    pub name: String,
    pub id: u64,
    pub timestamp: i64,
    pub size: u64,
    pub managed: bool,
}

/// Scan the workshop directory and return all installed mods with metadata.
/// Reads `meta.cpp` from each mod directory to extract name, ID, and timestamp.
pub fn scan_workshop_dir(workshop_path: &Path) -> Result<Vec<InstalledMod>> {
    if !workshop_path.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    let entries = fs::read_dir(workshop_path)?;
    let name_re = Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap();
    let id_re = Regex::new(r#"publishedid\s*=\s*(\d+)"#).unwrap();
    let ts_re = Regex::new(r#"timestamp\s*=\s*(-?\d+)"#).unwrap();

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

        let timestamp = ts_re
            .captures(&meta_content)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or(0);
        let size = du_dir(&path).unwrap_or(0);
        let managed = path.join(".dayz-ctl").exists();

        mods.push(InstalledMod {
            name,
            id,
            timestamp,
            size,
            managed,
        });
    }

    // Sort by name for consistent display
    mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(mods)
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
pub fn get_missing_mods(server_mods: &[u64], installed_mods: &[InstalledMod]) -> Vec<u64> {
    let installed_ids: Vec<u64> = installed_mods.iter().map(|m| m.id).collect();
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

    // Remove existing symlink/file if it exists
    if target.symlink_metadata().is_ok() {
        if target.is_symlink() || target.is_file() {
            fs::remove_file(&target)?;
        } else if target.is_dir() {
            fs::remove_dir_all(&target)?;
        }
    }

    #[cfg(unix)]
    symlink(&source, &target)?;

    #[cfg(windows)]
    symlink_dir(&source, &target)?;

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

/// Mark a mod as managed by writing the mod ID to a `.dayz-ctl` marker file.
/// This matches the bash script's behavior: `echo "$id" > "$dayz_workshop_path/$id/.dayz-ctl"`
pub fn mark_mod_as_managed(workshop_path: &Path, mod_id: u64) -> Result<()> {
    let managed_file = workshop_path.join(mod_id.to_string()).join(".dayz-ctl");
    fs::write(managed_file, mod_id.to_string())?;
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
        if file_name.starts_with('@') && path.is_symlink() {
            fs::remove_file(&path)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Remove all mods that have the `.dayz-ctl` marker file (managed mods).
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

        let managed_file = path.join(".dayz-ctl");
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
        let managed_file = mod_path.join(".dayz-ctl");
        if !managed_file.exists() {
            return Err(Error::Mod(format!(
                "Mod {} is not managed (no .dayz-ctl file)",
                mod_id
            )));
        }
    }

    fs::remove_dir_all(&mod_path)?;
    Ok(())
}

/// Toggle the managed status of a mod. Returns the new managed state.
pub fn toggle_mod_managed(workshop_path: &Path, mod_id: u64) -> Result<bool> {
    let mod_path = workshop_path.join(mod_id.to_string());
    if !mod_path.exists() {
        return Err(Error::Mod(format!("Mod {} does not exist", mod_id)));
    }

    let managed_file = mod_path.join(".dayz-ctl");
    let currently_managed = managed_file.exists();

    if currently_managed {
        fs::remove_file(&managed_file)?;
        Ok(false)
    } else {
        fs::write(&managed_file, mod_id.to_string())?;
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

pub struct ModManagementStats {
    pub removed_count: usize,
    pub removed_size: u64,
    pub symlinks_removed: usize,
}
