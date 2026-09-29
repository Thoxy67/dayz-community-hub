//! Installed workshop mods and the `@<id>` links DayZ loads them through.
//!
//! A mod can be in two places: the launcher's SteamCMD directory, which the
//! launcher owns, and the Steam client's workshop folders (one per library,
//! subscriptions and mods an earlier version downloaded there), which it
//! only reads. The list is the union; when both hold a mod, the copy updated
//! last is the one linked and shown.

use dz_common::Error;
use dz_common::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Whose folder a mod copy is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModSource {
    /// The launcher's SteamCMD directory: it may update and delete it.
    Launcher,
    /// A Steam library's workshop folder: read only.
    Steam,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledMod {
    pub name: String,
    pub id: u64,
    /// Local install/update time (filesystem mtime of meta.cpp, rewritten by steamcmd on each download)
    pub local_updated: i64,
    pub size: u64,
    /// An `@<id>` link in the DayZ directory loads it.
    pub managed: bool,
    pub source: ModSource,
    /// The mod's directory.
    pub path: PathBuf,
    /// Another, older copy exists in the other kind of folder.
    pub other_copy: bool,
}

/// Where installed mods are looked for.
#[derive(Debug, Clone)]
pub struct ModDirs {
    /// The launcher's workshop folder
    /// (`<data>/steamcmd-content/steamapps/workshop/content/221100`).
    pub launcher: PathBuf,
    /// The Steam client's workshop folders, one per library. Read only.
    pub steam: Vec<PathBuf>,
}

impl ModDirs {
    /// Every folder with its source, the launcher's first; a Steam folder
    /// that is the launcher's own (a link) is left out.
    fn all(&self) -> impl Iterator<Item = (&Path, ModSource)> {
        let own = self.launcher.canonicalize().ok();
        std::iter::once((self.launcher.as_path(), ModSource::Launcher)).chain(
            self.steam
                .iter()
                .filter(move |d| own.is_none() || d.canonicalize().ok() != own)
                .map(|d| (d.as_path(), ModSource::Steam)),
        )
    }

    /// Every installed mod, one entry per id (the copy updated last), sorted
    /// by name. `dayz` tells which are linked.
    pub fn scan(&self, dayz: Option<&Path>) -> Vec<InstalledMod> {
        let copies = self
            .all()
            .flat_map(|(dir, source)| scan_workshop_dir(dir, source).unwrap_or_default())
            .collect();
        let mut mods = merge_copies(copies);
        if let Some(dayz) = dayz {
            for m in &mut mods {
                m.managed = is_linked(dayz, m.id);
            }
        }
        mods
    }

    /// The directory of the copy of `id` to use: the one updated last, the
    /// launcher's on a tie. Reads only each copy's `meta.cpp`.
    pub fn copy_of(&self, id: u64) -> Option<(PathBuf, ModSource)> {
        let mut best: Option<(PathBuf, ModSource, i64)> = None;
        for (dir, source) in self.all() {
            let path = dir.join(id.to_string());
            let Some(updated) = meta_mtime(&path.join("meta.cpp")) else {
                continue;
            };
            if best.as_ref().is_none_or(|(_, _, b)| updated > *b) {
                best = Some((path, source, updated));
            }
        }
        best.map(|(path, source, _)| (path, source))
    }

    /// The launcher's copy of `id`, whether or not it exists.
    pub fn launcher_copy(&self, id: u64) -> PathBuf {
        self.launcher.join(id.to_string())
    }

    /// Look in `dir` too, a Steam library's workshop folder, unless it is
    /// already listed.
    pub fn add_steam(&mut self, dir: &Path) {
        let same = |d: &PathBuf| d == dir || (d.canonicalize().ok() == dir.canonicalize().ok());
        if !self.steam.iter().any(same) {
            self.steam.push(dir.to_path_buf());
        }
    }

    /// A Steam copy of `id` exists.
    pub fn in_steam(&self, id: u64) -> bool {
        self.all()
            .filter(|(_, s)| *s == ModSource::Steam)
            .any(|(dir, _)| dir.join(id.to_string()).join("meta.cpp").is_file())
    }
}

/// One entry per mod id from copies in several folders: the copy updated
/// last wins, the launcher's on a tie (it is the one the launcher can
/// update). Sorted by name.
pub fn merge_copies(copies: Vec<InstalledMod>) -> Vec<InstalledMod> {
    let mut by_id: HashMap<u64, InstalledMod> = HashMap::with_capacity(copies.len());
    for c in copies {
        match by_id.get_mut(&c.id) {
            None => {
                by_id.insert(c.id, c);
            }
            Some(kept) => {
                let newer = c.local_updated > kept.local_updated
                    || (c.local_updated == kept.local_updated
                        && c.source == ModSource::Launcher
                        && kept.source != ModSource::Launcher);
                let other_copy = kept.source != c.source || kept.other_copy;
                if newer {
                    *kept = c;
                }
                kept.other_copy = other_copy;
            }
        }
    }
    let mut mods: Vec<InstalledMod> = by_id.into_values().collect();
    mods.sort_by_key(|a| a.name.to_lowercase());
    mods
}

fn meta_mtime(meta: &Path) -> Option<i64> {
    fs::metadata(meta)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// Regexes for parsing meta.cpp — compiled once at first use.
static NAME_RE: OnceLock<Regex> = OnceLock::new();
static ID_RE: OnceLock<Regex> = OnceLock::new();

/// Process-lifetime cache: mod directory → (stamp, total size in bytes). The
/// stamp is the directory's mtime together with `meta.cpp`'s: the directory's
/// own mtime only moves when an entry is added or removed at its top level,
/// while steamcmd rewrites `meta.cpp` on every download, so an update that
/// only changed files deeper down still invalidates the size.
type SizeCache = Mutex<HashMap<PathBuf, ((u64, u64), u64)>>;
static SIZE_CACHE: OnceLock<SizeCache> = OnceLock::new();

fn size_cache() -> &'static SizeCache {
    SIZE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Scan one workshop folder and return the mods in it, as `source`, sorted
/// by name. Reads `meta.cpp` from each mod directory for its name and id.
pub fn scan_workshop_dir(workshop_path: &Path, source: ModSource) -> Result<Vec<InstalledMod>> {
    if !workshop_path.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    let entries = fs::read_dir(workshop_path)?;
    let name_re = NAME_RE.get_or_init(|| Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap());
    let id_re = ID_RE.get_or_init(|| Regex::new(r#"publishedid\s*=\s*(\d+)"#).unwrap());

    // An entry that cannot be read is skipped, not a reason to list nothing.
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("meta.cpp");
        let Ok(meta_content) = fs::read_to_string(&meta_path) else {
            continue;
        };
        let name = name_re
            .captures(&meta_content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        // Without an id in meta.cpp, the directory's name is the id.
        let id = id_re
            .captures(&meta_content)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.parse::<u64>().ok())
            });
        let Some(id) = id else {
            continue;
        };

        let local_updated = meta_mtime(&meta_path).unwrap_or(0);
        let size = du_dir_cached(&path, local_updated.max(0) as u64).unwrap_or(0);

        mods.push(InstalledMod {
            name,
            id,
            local_updated,
            size,
            managed: false,
            source,
            path,
            other_copy: false,
        });
    }

    mods.sort_by_key(|a| a.name.to_lowercase());
    Ok(mods)
}

/// Cached directory size: skips the recursive walk when neither the
/// directory's mtime nor `meta.cpp`'s (`meta_mtime`) has changed.
fn du_dir_cached(path: &Path, meta_mtime: u64) -> Result<u64> {
    // Read the directory's own mtime — changes when files are added/removed.
    let dir_mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Check the cache first.
    let stamp = (dir_mtime, meta_mtime);
    if let Ok(cache) = size_cache().lock()
        && let Some(&(cached_stamp, cached_size)) = cache.get(path)
        && cached_stamp == stamp
        && dir_mtime != 0
    {
        return Ok(cached_size);
    }

    // Cache miss or mtime changed — do the full walk.
    let size = du_dir(path)?;

    // Update the cache.
    if let Ok(mut cache) = size_cache().lock() {
        cache.insert(path.to_path_buf(), (stamp, size));
    }

    Ok(size)
}

/// Total size of the files under `path`. Links are not followed (a link
/// cycle would never end), and unreadable entries count as nothing.
fn du_dir(path: &Path) -> Result<u64> {
    let mut total = 0;
    if path.is_dir() {
        for entry in fs::read_dir(path)?.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                total += du_dir(&entry.path()).unwrap_or(0);
            } else if let Ok(meta) = entry.metadata() {
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
        .copied()
        .collect()
}

/// `dayz_path/@<mod_id>`.
fn link_path(dayz_path: &Path, mod_id: u64) -> PathBuf {
    dayz_path.join(format!("@{mod_id}"))
}

/// An `@<id>` link (a symlink, or on Windows a junction) is in the DayZ
/// directory.
pub fn is_linked(dayz_path: &Path, mod_id: u64) -> bool {
    let link = link_path(dayz_path, mod_id);
    #[cfg(windows)]
    if is_junction(&link) {
        return true;
    }
    link.is_symlink()
}

/// Link `dayz_path/@<mod_id>` to `source`, the mod's directory. This is how
/// DayZ discovers mods at launch time. A link already pointing there is
/// left as it is.
pub fn create_mod_symlink(source: &Path, dayz_path: &Path, mod_id: u64) -> Result<()> {
    let target = link_path(dayz_path, mod_id);

    if !source.exists() {
        return Err(Error::Mod(format!(
            "Mod directory does not exist: {source:?}"
        )));
    }
    if fs::read_link(&target).is_ok_and(|to| to == source) {
        return Ok(());
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
    symlink(source, &target)?;

    #[cfg(windows)]
    {
        // Symlinks require admin on Windows; NTFS junctions do not.
        // mklink is a cmd.exe built-in, not a standalone executable, so it
        // must be invoked via `cmd /c "mklink /J ..."`.
        //
        // We must build the command line with `raw_arg` rather than `arg`/`args`:
        // Rust's normal argument escaping follows the MSVCRT convention and turns
        // internal quotes into `\"`, but cmd.exe does NOT understand `\"` as an
        // escaped quote. With the default Steam path (`C:\Program Files (x86)\Steam`,
        // which always contains spaces) that mangling splits the paths and makes
        // `mklink` fail, so no mods ever get linked.
        //
        // cmd.exe's `/c` dequoting strips only the outermost quote pair when the
        // remainder both starts and ends with a quote and contains more quotes, so
        // we wrap the whole `mklink ...` invocation in an extra quote pair. After
        // cmd strips the outer pair, `mklink` sees each path correctly quoted.
        use std::os::windows::process::CommandExt;
        let target_str = target.to_string_lossy();
        let source_str = source.to_string_lossy();
        let inner = format!("\"mklink /J \"{target_str}\" \"{source_str}\"\"");
        let out = std::process::Command::new("cmd")
            .raw_arg("/c")
            .raw_arg(&inner)
            .creation_flags(dz_common::CREATE_NO_WINDOW)
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

/// Link each of `mod_ids` to its chosen copy. Returns the ids linked.
pub fn create_mod_symlinks(dirs: &ModDirs, dayz_path: &Path, mod_ids: &[u64]) -> Result<Vec<u64>> {
    let mut created = Vec::new();
    for &mod_id in mod_ids {
        let Some((source, _)) = dirs.copy_of(mod_id) else {
            continue;
        };
        match create_mod_symlink(&source, dayz_path, mod_id) {
            Ok(()) => created.push(mod_id),
            Err(e) => {
                // Log error but continue with other mods
                eprintln!("Warning: failed to create symlink for mod {mod_id}: {e}");
            }
        }
    }
    Ok(created)
}

/// Point the links of those `mod_ids` that are linked at their chosen copy:
/// after an update the launcher's fresh copy wins over an older Steam one.
pub fn relink_linked(dirs: &ModDirs, dayz_path: &Path, mod_ids: &[u64]) {
    let linked: Vec<u64> = mod_ids
        .iter()
        .copied()
        .filter(|&id| is_linked(dayz_path, id))
        .collect();
    let _ = create_mod_symlinks(dirs, dayz_path, &linked);
}

/// Remove a single `@<mod_id>` symlink/junction from the DayZ game directory.
pub fn remove_mod_symlink(dayz_path: &Path, mod_id: u64) -> Result<()> {
    let target = link_path(dayz_path, mod_id);

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

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        #[cfg(unix)]
        if file_name.starts_with('@') && path.is_symlink() && fs::remove_file(&path).is_ok() {
            count += 1;
        }
        // On Windows, mod links are NTFS junctions which appear as directories.
        // We verify the path is a reparse point (junction) before removing so
        // we never accidentally delete a real directory.
        // remove_dir on a junction removes only the junction point, not its target.
        #[cfg(windows)]
        if file_name.starts_with('@') && is_junction(&path) && fs::remove_dir(&path).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

/// Delete every mod in the launcher's folder. Returns (count, bytes).
pub fn remove_launcher_mods(launcher: &Path) -> Result<(usize, u64)> {
    let mut count = 0;
    let mut total_size = 0;

    if !launcher.exists() {
        return Ok((0, 0));
    }

    // Best effort: a mod that cannot be removed (in use, permissions) does
    // not stop the others from being.
    for entry in fs::read_dir(launcher)?.flatten() {
        let path = entry.path();
        if !path.is_dir() || path.is_symlink() {
            continue;
        }
        let size = du_dir(&path).unwrap_or(0);
        if fs::remove_dir_all(&path).is_ok() {
            count += 1;
            total_size += size;
        }
    }

    Ok((count, total_size))
}

/// What deleting a mod did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// The launcher's copy and the link are gone.
    Deleted,
    /// A copy in a Steam library remains: the launcher never deletes there.
    /// Its link is gone; unsubscribing in Steam removes the files.
    KeptInSteam,
}

/// Delete a mod: the launcher's copy, and its `@<id>` link. A Steam copy is
/// left where it is (see [`DeleteOutcome::KeptInSteam`]).
pub fn delete_mod(dirs: &ModDirs, dayz_path: Option<&Path>, mod_id: u64) -> Result<DeleteOutcome> {
    let own = dirs.launcher_copy(mod_id);
    let in_steam = dirs.in_steam(mod_id);
    if !own.exists() && !in_steam {
        return Err(Error::Mod(format!("Mod {mod_id} does not exist")));
    }
    if own.exists() {
        fs::remove_dir_all(&own)?;
    }
    if let Some(dayz) = dayz_path {
        let _ = remove_mod_symlink(dayz, mod_id);
    }
    Ok(if in_steam {
        DeleteOutcome::KeptInSteam
    } else {
        DeleteOutcome::Deleted
    })
}

/// Delete a mod's copies in the Steam libraries. Only for a mod Steam said
/// the account is not subscribed to: Steam does not manage those copies and
/// never removes them.
pub fn delete_steam_copies(dirs: &ModDirs, mod_id: u64) -> Result<()> {
    for (dir, source) in dirs.all() {
        if source != ModSource::Steam {
            continue;
        }
        let copy = dir.join(mod_id.to_string());
        if copy.join("meta.cpp").is_file() {
            fs::remove_dir_all(&copy)?;
        }
    }
    Ok(())
}

/// Link or unlink a mod: a linked mod loses its `@<id>` link, another gets
/// one to its chosen copy. Nothing is written into the mod's folder.
///
/// Returns the new state (linked or not).
pub fn toggle_mod_managed(dirs: &ModDirs, dayz_path: &Path, mod_id: u64) -> Result<bool> {
    if is_linked(dayz_path, mod_id) {
        remove_mod_symlink(dayz_path, mod_id)?;
        return Ok(false);
    }
    let (source, _) = dirs
        .copy_of(mod_id)
        .ok_or_else(|| Error::Mod(format!("Mod {mod_id} does not exist")))?;
    create_mod_symlink(&source, dayz_path, mod_id)?;
    Ok(true)
}

/// Delete every mod the launcher downloaded and every `@` link. Mods in
/// Steam libraries stay.
pub fn cleanup_mods(dirs: &ModDirs, dayz_path: &Path) -> Result<ModManagementStats> {
    let (removed_count, removed_size) = remove_launcher_mods(&dirs.launcher)?;
    let symlinks_removed = remove_all_mod_symlinks(dayz_path)?;

    Ok(ModManagementStats {
        removed_count,
        removed_size,
        symlinks_removed,
    })
}

/// Format a file size in bytes to a human-readable string.
pub fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1024.0 / 1024.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else if mb >= 1.0 {
        format!("{mb:.1} MB")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dz-mods-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn fake_mod(workshop: &Path, id: u64, name: &str, bytes: usize) {
        let dir = workshop.join(id.to_string());
        fs::create_dir_all(dir.join("addons")).unwrap();
        fs::write(
            dir.join("meta.cpp"),
            format!("protocol = 1;\npublishedid = {id};\nname = \"{name}\";\n"),
        )
        .unwrap();
        fs::write(dir.join("addons").join("data.pbo"), vec![0u8; bytes]).unwrap();
    }

    /// Set a copy's `meta.cpp` mtime: which copy is newer is decided by it.
    fn touch(workshop: &Path, id: u64, secs: u64) {
        let f = fs::File::options()
            .write(true)
            .open(workshop.join(id.to_string()).join("meta.cpp"))
            .unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
            .unwrap();
    }

    fn copy(id: u64, source: ModSource, updated: i64) -> InstalledMod {
        InstalledMod {
            name: format!("m{id}"),
            id,
            local_updated: updated,
            size: 0,
            managed: false,
            source,
            path: PathBuf::from(format!("/{source:?}/{id}")),
            other_copy: false,
        }
    }

    #[test]
    fn scans_names_ids_and_sizes() {
        let ws = tmp("scan");
        fake_mod(&ws, 1559212036, "CF", 2048);
        fake_mod(&ws, 42, "alpha", 10);
        fs::create_dir_all(ws.join("not-a-mod")).unwrap();
        let mods = scan_workshop_dir(&ws, ModSource::Launcher).unwrap();
        let names: Vec<_> = mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["alpha", "CF"], "sorted by name, case-insensitively");
        let cf = mods.iter().find(|m| m.id == 1559212036).unwrap();
        assert!(cf.size >= 2048);
        assert!(!cf.managed);
        assert_eq!(cf.path, ws.join("1559212036"));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn the_newer_copy_wins_and_the_launcher_on_a_tie() {
        let merged = merge_copies(vec![
            copy(1, ModSource::Steam, 200),
            copy(1, ModSource::Launcher, 100),
            copy(2, ModSource::Steam, 100),
            copy(2, ModSource::Launcher, 300),
            copy(3, ModSource::Steam, 100),
            copy(3, ModSource::Launcher, 100),
            copy(4, ModSource::Steam, 50),
        ]);
        let got: Vec<_> = merged
            .iter()
            .map(|m| (m.id, m.source, m.other_copy))
            .collect();
        assert_eq!(
            got,
            [
                (1, ModSource::Steam, true),
                (2, ModSource::Launcher, true),
                (3, ModSource::Launcher, true),
                (4, ModSource::Steam, false),
            ]
        );
    }

    #[test]
    fn the_list_is_the_union_of_both_folders() {
        let root = tmp("union");
        let dirs = ModDirs {
            launcher: root.join("launcher"),
            steam: vec![root.join("lib1"), root.join("lib2"), root.join("gone")],
        };
        fake_mod(&dirs.launcher, 1, "one", 1);
        fake_mod(&dirs.steam[0], 1, "one", 1);
        fake_mod(&dirs.steam[0], 2, "two", 1);
        fake_mod(&dirs.steam[1], 3, "three", 1);
        touch(&dirs.launcher, 1, 1_000);
        touch(&dirs.steam[0], 1, 2_000);
        let mods = dirs.scan(None);
        let got: Vec<_> = mods.iter().map(|m| (m.id, m.source)).collect();
        assert_eq!(
            got,
            [
                (1, ModSource::Steam),
                (3, ModSource::Steam),
                (2, ModSource::Steam)
            ]
        );
        assert_eq!(mods[0].path, dirs.steam[0].join("1"));
        assert_eq!(
            dirs.copy_of(1),
            Some((dirs.steam[0].join("1"), ModSource::Steam))
        );
        // The launcher downloads an update: its copy is now the newer.
        touch(&dirs.launcher, 1, 3_000);
        assert_eq!(
            dirs.copy_of(1),
            Some((dirs.launcher.join("1"), ModSource::Launcher))
        );
        assert_eq!(dirs.copy_of(9), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_mods_are_the_uninstalled_ones() {
        let installed = vec![copy(1, ModSource::Steam, 0)];
        assert_eq!(get_missing_mods(&[1, 2, 3], &installed), [2, 3]);
    }

    #[cfg(unix)]
    #[test]
    fn link_cycles_do_not_hang_the_size_walk() {
        let d = tmp("cycle");
        fs::create_dir_all(d.join("a")).unwrap();
        symlink(&d, d.join("a").join("loop")).unwrap();
        fs::write(d.join("a").join("f"), [0u8; 5]).unwrap();
        assert_eq!(du_dir(&d).unwrap(), 5);
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn toggling_links_and_unlinks_without_writing_into_the_mod() {
        let root = tmp("toggle");
        let dirs = ModDirs {
            launcher: root.join("launcher"),
            steam: vec![root.join("steam")],
        };
        let dayz = root.join("DayZ");
        fs::create_dir_all(&dayz).unwrap();
        fake_mod(&dirs.steam[0], 7, "seven", 1);
        let before: Vec<_> = fs::read_dir(dirs.steam[0].join("7")).unwrap().collect();
        assert!(toggle_mod_managed(&dirs, &dayz, 7).unwrap());
        assert_eq!(
            fs::read_link(dayz.join("@7")).unwrap(),
            dirs.steam[0].join("7")
        );
        assert!(dirs.scan(Some(&dayz))[0].managed);
        assert!(!toggle_mod_managed(&dirs, &dayz, 7).unwrap());
        assert!(dayz.join("@7").symlink_metadata().is_err());
        let after: Vec<_> = fs::read_dir(dirs.steam[0].join("7")).unwrap().collect();
        assert_eq!(before.len(), after.len());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn an_update_moves_the_link_to_the_launchers_copy() {
        let root = tmp("relink");
        let dirs = ModDirs {
            launcher: root.join("launcher"),
            steam: vec![root.join("steam")],
        };
        let dayz = root.join("DayZ");
        fs::create_dir_all(&dayz).unwrap();
        fake_mod(&dirs.steam[0], 5, "five", 1);
        touch(&dirs.steam[0], 5, 1_000);
        create_mod_symlinks(&dirs, &dayz, &[5]).unwrap();
        fake_mod(&dirs.launcher, 5, "five", 1);
        touch(&dirs.launcher, 5, 2_000);
        relink_linked(&dirs, &dayz, &[5, 6]);
        assert_eq!(
            fs::read_link(dayz.join("@5")).unwrap(),
            dirs.launcher.join("5")
        );
        assert!(dayz.join("@6").symlink_metadata().is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unsubscribed_steam_copy_is_deleted_in_every_library_only() {
        let root = tmp("steam-copies");
        let dirs = ModDirs {
            launcher: root.join("launcher"),
            steam: vec![root.join("lib1"), root.join("lib2")],
        };
        fake_mod(&dirs.launcher, 4, "four", 1);
        fake_mod(&dirs.steam[0], 4, "four", 1);
        fake_mod(&dirs.steam[1], 4, "four", 1);
        fake_mod(&dirs.steam[0], 5, "five", 1);
        delete_steam_copies(&dirs, 4).unwrap();
        assert!(!dirs.steam[0].join("4").exists());
        assert!(!dirs.steam[1].join("4").exists());
        assert!(dirs.launcher.join("4").join("meta.cpp").is_file());
        assert!(dirs.steam[0].join("5").join("meta.cpp").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn deleting_never_touches_a_steam_library() {
        let root = tmp("delete");
        let dirs = ModDirs {
            launcher: root.join("launcher"),
            steam: vec![root.join("steam")],
        };
        let dayz = root.join("DayZ");
        fs::create_dir_all(&dayz).unwrap();
        fake_mod(&dirs.launcher, 1, "own", 1);
        fake_mod(&dirs.steam[0], 2, "subscribed", 1);
        fake_mod(&dirs.launcher, 3, "both", 1);
        fake_mod(&dirs.steam[0], 3, "both", 1);
        create_mod_symlinks(&dirs, &dayz, &[1, 2, 3]).unwrap();

        assert_eq!(
            delete_mod(&dirs, Some(&dayz), 1).unwrap(),
            DeleteOutcome::Deleted
        );
        assert!(!dirs.launcher.join("1").exists());
        assert!(!is_linked(&dayz, 1));

        assert_eq!(
            delete_mod(&dirs, Some(&dayz), 2).unwrap(),
            DeleteOutcome::KeptInSteam
        );
        assert!(dirs.steam[0].join("2").join("meta.cpp").is_file());
        assert!(!is_linked(&dayz, 2));

        assert_eq!(
            delete_mod(&dirs, Some(&dayz), 3).unwrap(),
            DeleteOutcome::KeptInSteam
        );
        assert!(!dirs.launcher.join("3").exists());
        assert!(dirs.steam[0].join("3").join("meta.cpp").is_file());

        assert!(delete_mod(&dirs, Some(&dayz), 4).is_err());

        fake_mod(&dirs.launcher, 8, "eight", 1);
        let stats = cleanup_mods(&dirs, &dayz).unwrap();
        assert_eq!(stats.removed_count, 1);
        assert!(dirs.steam[0].join("2").exists() && dirs.steam[0].join("3").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sizes_read_as_people_say_them() {
        assert_eq!(format_size(512 * 1024), "512 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
