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
use std::sync::{LazyLock, Mutex};

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
    /// The installed copy's version, as a time: the Workshop's `time_updated`
    /// of it from Steam's manifest, or `meta.cpp`'s mtime when that is later
    /// (or the manifest does not name it).
    pub local_updated: i64,
    pub size: u64,
    /// An `@<id>` link in the DayZ directory loads it.
    pub managed: bool,
    pub source: ModSource,
    /// The mod's directory.
    pub path: PathBuf,
    /// Another, older copy exists in the other kind of folder.
    pub other_copy: bool,
    /// A Steam copy the Steam account is subscribed to, as Steam's manifest
    /// says: Steam keeps it updated itself, so an update all leaves it to
    /// Steam. Always false for the launcher's copies.
    #[serde(default)]
    pub subscribed: bool,
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

    /// [`Self::all`], resolved once: each call canonicalises every folder,
    /// which linking a modded server's thirty mods would otherwise repeat
    /// per mod.
    fn resolved(&self) -> Vec<(&Path, ModSource)> {
        self.all().collect()
    }

    /// Every installed mod, one entry per id (the copy updated last), sorted
    /// by name. `dayz` tells which are linked.
    pub fn scan(&self, dayz: Option<&Path>) -> Vec<InstalledMod> {
        // Each folder unsorted: `merge_copies` sorts the union once.
        let copies = self
            .all()
            .flat_map(|(dir, source)| scan_dir(dir, source).unwrap_or_default())
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
        copy_in(&self.resolved(), id)
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
        in_steam_in(&self.resolved(), id)
    }

    /// The Steam libraries' workshop folders (`steamapps/workshop`), each
    /// holding the manifest of its content folder; the launcher's own left out.
    pub fn steam_workshops(&self) -> Vec<PathBuf> {
        self.all()
            .filter(|(_, s)| *s == ModSource::Steam)
            .filter_map(|(dir, _)| workshop_of(dir).map(Path::to_path_buf))
            .collect()
    }
}

/// [`ModDirs::in_steam`] over folders already resolved.
fn in_steam_in(dirs: &[(&Path, ModSource)], id: u64) -> bool {
    let name = id.to_string();
    dirs.iter()
        .filter(|(_, s)| *s == ModSource::Steam)
        .any(|(dir, _)| dir.join(&name).join("meta.cpp").is_file())
}

/// [`ModDirs::copy_of`] over folders already resolved.
fn copy_in(dirs: &[(&Path, ModSource)], id: u64) -> Option<(PathBuf, ModSource)> {
    let name = id.to_string();
    let mut best: Option<(PathBuf, ModSource, i64)> = None;
    for &(dir, source) in dirs {
        let path = dir.join(&name);
        let Some(updated) = meta_mtime(&path.join("meta.cpp")) else {
            continue;
        };
        if best.as_ref().is_none_or(|(_, _, b)| updated > *b) {
            best = Some((path, source, updated));
        }
    }
    best.map(|(path, source, _)| (path, source))
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
    sort_by_name(&mut mods);
    mods
}

/// Sorted by name without regard to case. Each lowercase key is made once,
/// not once per comparison.
fn sort_by_name(mods: &mut [InstalledMod]) {
    mods.sort_by_cached_key(|a| a.name.to_lowercase());
}

fn meta_mtime(meta: &Path) -> Option<i64> {
    mtime_secs(&fs::metadata(meta).ok()?)
}

fn mtime_secs(meta: &fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// Regexes for parsing meta.cpp — compiled once at first use.
static NAME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap());
static ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"publishedid\s*=\s*(\d+)"#).unwrap());

/// When a mod's size was last walked: its directory's mtime, `meta.cpp`'s,
/// and the version Steam's manifest gives it (0 when unlisted).
type SizeStamp = (u64, u64, i64);

/// Process-lifetime cache: mod directory → (stamp, total size in bytes). The
/// directory's own mtime only moves when an entry is added or removed at its
/// top level, while steamcmd rewrites `meta.cpp` on every download and the
/// manifest records each version, so an update that only changed files
/// deeper down still invalidates the size.
type SizeCache = Mutex<HashMap<PathBuf, (SizeStamp, u64)>>;
static SIZE_CACHE: LazyLock<SizeCache> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn size_cache() -> &'static SizeCache {
    &SIZE_CACHE
}

/// Scan one workshop folder and return the mods in it, as `source`, sorted
/// by name. Reads `meta.cpp` from each mod directory for its name and id.
pub fn scan_workshop_dir(workshop_path: &Path, source: ModSource) -> Result<Vec<InstalledMod>> {
    let mut mods = scan_dir(workshop_path, source)?;
    sort_by_name(&mut mods);
    Ok(mods)
}

/// [`scan_workshop_dir`] in directory order.
fn scan_dir(workshop_path: &Path, source: ModSource) -> Result<Vec<InstalledMod>> {
    let entries = match fs::read_dir(workshop_path) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut mods = Vec::new();
    let manifest = manifest_beside(workshop_path);

    // An entry that cannot be read is skipped, not a reason to list nothing.
    // No separate "is it a directory" check: a file has no `meta.cpp` under
    // it, so opening that fails the same way, one stat sooner.
    for entry in entries.flatten() {
        let path = entry.path();
        let meta_path = path.join("meta.cpp");
        let Ok(mut meta_file) = fs::File::open(&meta_path) else {
            continue;
        };
        // The open file's own metadata: no second lookup of the path.
        let local_updated = meta_file
            .metadata()
            .ok()
            .and_then(|m| mtime_secs(&m))
            .unwrap_or(0);
        let mut meta_content = String::new();
        if std::io::Read::read_to_string(&mut meta_file, &mut meta_content).is_err() {
            continue;
        }
        let name = NAME_RE
            .captures(&meta_content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        // Without an id in meta.cpp, the directory's name is the id.
        let id = ID_RE
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

        let listed = manifest.installed.get(&id);
        // The size Steam recorded when it installed this version: no walk of
        // every file. A copy the manifest does not list is walked (cached).
        let size = match listed {
            Some(i) if i.size > 0 => i.size,
            _ => du_dir_cached(
                &path,
                local_updated.max(0) as u64,
                listed.map_or(0, |i| i.updated),
            )
            .unwrap_or(0),
        };
        // The version Steam installed, when it says: an update that left
        // `meta.cpp` as it was still moves it.
        let local_updated = listed
            .filter(|i| i.updated > 0)
            .map_or(local_updated, |i| i.updated.max(local_updated));

        mods.push(InstalledMod {
            name,
            id,
            local_updated,
            size,
            managed: false,
            source,
            path,
            other_copy: false,
            subscribed: source == ModSource::Steam && manifest.subscribed.contains(&id),
        });
    }
    Ok(mods)
}

/// The manifest's file name in a library's `steamapps/workshop` folder.
pub const MANIFEST: &str = "appworkshop_221100.acf";

/// An item Steam (or SteamCMD) installed, as its manifest records it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ManifestItem {
    /// The installed version: the Workshop's `time_updated` of it.
    pub updated: i64,
    /// Its content's size in bytes.
    pub size: u64,
}

/// What the workshop manifest beside a content folder says
/// (`steamapps/workshop/appworkshop_221100.acf`). Steam and SteamCMD both
/// write the installed items; only the Steam client writes subscriptions
/// and the versions it knows are out.
#[derive(Debug, Clone, Default)]
pub struct WorkshopManifest {
    /// The items installed, by id.
    pub installed: HashMap<u64, ManifestItem>,
    /// The items a Steam account is subscribed to (`subscribedby`).
    pub subscribed: HashSet<u64>,
    /// The items Steam knows a newer version of (`latest_timeupdated` past
    /// the installed `timeupdated`). Steam updates this in its own time.
    pub needs_update: HashSet<u64>,
    /// Steam has items of the game to download (`NeedsDownload`).
    pub needs_download: bool,
}

impl WorkshopManifest {
    /// Read from its text; what is missing or unreadable is left empty.
    pub fn parse(text: &str) -> Self {
        use dz_steamcmd::vdf::Vdf;
        let doc = dz_steamcmd::vdf::parse(text);
        let num = |v: &Vdf, key: &str| v.str(&[key]).and_then(|s| s.trim().parse::<i64>().ok());
        let mut out = Self {
            needs_download: doc
                .str(&["AppWorkshop", "NeedsDownload"])
                .is_some_and(|v| v.trim() != "0" && !v.trim().is_empty()),
            ..Self::default()
        };
        if let Some(Vdf::Obj(items)) = doc.get(&["AppWorkshop", "WorkshopItemsInstalled"]) {
            for (id, item) in items {
                let Ok(id) = id.parse::<u64>() else { continue };
                let updated = num(item, "timeupdated").unwrap_or(0).max(0);
                let size = num(item, "size").and_then(|s| u64::try_from(s).ok());
                out.installed.insert(
                    id,
                    ManifestItem {
                        updated,
                        size: size.unwrap_or(0),
                    },
                );
            }
        }
        if let Some(Vdf::Obj(items)) = doc.get(&["AppWorkshop", "WorkshopItemDetails"]) {
            for (id, item) in items {
                let Ok(id) = id.parse::<u64>() else { continue };
                if num(item, "subscribedby").is_some_and(|by| by != 0) {
                    out.subscribed.insert(id);
                }
                let at = num(item, "timeupdated").unwrap_or(0);
                if num(item, "latest_timeupdated").is_some_and(|latest| latest > at) {
                    out.needs_update.insert(id);
                }
            }
        }
        out
    }

    /// The manifest in `workshop` (a library's `steamapps/workshop`), if
    /// there is one.
    pub fn read(workshop: &Path) -> Option<Self> {
        fs::read_to_string(workshop.join(MANIFEST))
            .ok()
            .map(|t| Self::parse(&t))
    }
}

/// `steamapps/workshop` for its `steamapps/workshop/content/221100`.
pub fn workshop_of(content: &Path) -> Option<&Path> {
    content.parent().and_then(Path::parent)
}

/// The manifest beside `content`; empty when there is none.
fn manifest_beside(content: &Path) -> WorkshopManifest {
    workshop_of(content)
        .and_then(WorkshopManifest::read)
        .unwrap_or_default()
}

/// Cached directory size: skips the recursive walk when neither the
/// directory's mtime, `meta.cpp`'s (`meta_mtime`) nor the version the
/// manifest records (`version`, 0 when unlisted) has changed.
fn du_dir_cached(path: &Path, meta_mtime: u64, version: i64) -> Result<u64> {
    // Read the directory's own mtime — changes when files are added/removed.
    let dir_mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Check the cache first.
    let stamp = (dir_mtime, meta_mtime, version);
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
    if fs::read_link(&target).is_ok_and(|to| same_path(&to, source)) {
        return Ok(());
    }

    // Remove existing link/file if it exists
    if target.symlink_metadata().is_ok() {
        // A junction (or a directory symlink) is a directory to Windows:
        // `remove_file` (DeleteFileW) refuses it, so re-linking a mod to its
        // other copy failed. `remove_dir` removes the link, never the mod.
        #[cfg(windows)]
        let reparse = is_junction(&target);
        #[cfg(not(windows))]
        let reparse = false;
        if reparse {
            fs::remove_dir(&target)?;
        } else if target.is_symlink() || target.is_file() {
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

    // Symlinks require admin (or Developer Mode) on Windows; NTFS junctions
    // do not. Made through the API: a `cmd /c mklink /J` per mod cost a
    // process start (and a Defender scan of it) each, seconds for a modded
    // server. The command stays as the fallback.
    #[cfg(windows)]
    if let Err(e) = dz_common::win::create_junction(source, &target) {
        eprintln!("junction for mod {mod_id} through the API failed ({e}); trying mklink");
        mklink_junction(source, &target, mod_id)?;
    }

    Ok(())
}

/// `cmd /c mklink /J`: the fallback for [`dz_common::win::create_junction`].
#[cfg(windows)]
fn mklink_junction(source: &Path, target: &Path, mod_id: u64) -> Result<()> {
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
    Ok(())
}

/// Whether a link's target is `source`. Windows paths are compared without
/// regard to case (NTFS is case-insensitive), so a link that already points
/// at the right folder is left alone rather than deleted and made again on
/// every launch.
fn same_path(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

/// Link each of `mod_ids` to its chosen copy. Returns the ids linked.
pub fn create_mod_symlinks(dirs: &ModDirs, dayz_path: &Path, mod_ids: &[u64]) -> Result<Vec<u64>> {
    let resolved = dirs.resolved();
    let mut created = Vec::with_capacity(mod_ids.len());
    for &mod_id in mod_ids {
        let Some((source, _)) = copy_in(&resolved, mod_id) else {
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

    // The sizes already known (the manifest's, a scan's), so the bytes freed
    // cost no walk of every file.
    let manifest = manifest_beside(launcher);
    let known = |path: &Path| -> Option<u64> {
        let by_manifest = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.parse::<u64>().ok())
            .and_then(|id| manifest.installed.get(&id))
            .map(|i| i.size)
            .filter(|&s| s > 0);
        by_manifest.or_else(|| {
            size_cache()
                .lock()
                .ok()
                .and_then(|c| c.get(path).map(|&(_, size)| size))
        })
    };

    // Best effort: a mod that cannot be removed (in use, permissions) does
    // not stop the others from being.
    for entry in fs::read_dir(launcher)?.flatten() {
        let path = entry.path();
        if !path.is_dir() || path.is_symlink() {
            continue;
        }
        let size = known(&path).unwrap_or_else(|| du_dir(&path).unwrap_or(0));
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
    delete_in(dirs, &dirs.resolved(), dayz_path, mod_id)
}

/// [`delete_mod`] for each of `mod_ids`, with the folders resolved once.
/// Stops at the first failure: the results so far, then that one.
pub fn delete_mods(
    dirs: &ModDirs,
    dayz_path: Option<&Path>,
    mod_ids: &[u64],
) -> Vec<(u64, Result<DeleteOutcome>)> {
    let resolved = dirs.resolved();
    let mut out = Vec::with_capacity(mod_ids.len());
    for &id in mod_ids {
        let r = delete_in(dirs, &resolved, dayz_path, id);
        let failed = r.is_err();
        out.push((id, r));
        if failed {
            break;
        }
    }
    out
}

/// [`delete_mod`] over folders already resolved.
fn delete_in(
    dirs: &ModDirs,
    resolved: &[(&Path, ModSource)],
    dayz_path: Option<&Path>,
    mod_id: u64,
) -> Result<DeleteOutcome> {
    let own = dirs.launcher_copy(mod_id);
    let in_steam = in_steam_in(resolved, mod_id);
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
    steam_copies_gone(&dirs.resolved(), mod_id)
}

/// [`delete_steam_copies`] for each of `mod_ids`, with the folders resolved
/// once. Each id's result, in order.
pub fn delete_steam_copies_of(dirs: &ModDirs, mod_ids: &[u64]) -> Vec<(u64, Result<()>)> {
    let resolved = dirs.resolved();
    mod_ids
        .iter()
        .map(|&id| (id, steam_copies_gone(&resolved, id)))
        .collect()
}

fn steam_copies_gone(dirs: &[(&Path, ModSource)], mod_id: u64) -> Result<()> {
    let name = mod_id.to_string();
    for &(dir, source) in dirs {
        if source != ModSource::Steam {
            continue;
        }
        let copy = dir.join(&name);
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
            subscribed: false,
        }
    }

    const STEAM_MANIFEST: &str = r#"
"AppWorkshop"
{
	"appid"		"221100"
	"NeedsUpdate"		"1"
	"NeedsDownload"		"1"
	"WorkshopItemsInstalled"
	{
		"7"
		{
			"size"		"123456789"
			"timeupdated"		"5000"
			"manifest"		"1"
		}
		"8"
		{
			"timeupdated"		"4000"
		}
	}
	"WorkshopItemDetails"
	{
		"7"
		{
			"timeupdated"		"5000"
			"subscribedby"		"92612506"
			"latest_timeupdated"		"6000"
		}
		"8"
		{
			"timeupdated"		"4000"
			"latest_timeupdated"		"4000"
		}
		"9"
		{
			"timeupdated"		"0"
			"subscribedby"		"0"
		}
	}
}
"#;

    #[test]
    fn the_manifest_says_what_steam_installed_and_follows() {
        let m = WorkshopManifest::parse(STEAM_MANIFEST);
        assert_eq!(
            m.installed.get(&7),
            Some(&ManifestItem {
                updated: 5000,
                size: 123_456_789
            })
        );
        assert_eq!(
            m.installed.get(&8).map(|i| i.size),
            Some(0),
            "no size given"
        );
        assert_eq!(m.subscribed, HashSet::from([7]), "subscribedby 0 is nobody");
        assert_eq!(m.needs_update, HashSet::from([7]));
        assert!(m.needs_download);
        let none = WorkshopManifest::parse("\"AppWorkshop\" { \"NeedsDownload\" \"0\" }");
        assert!(none.installed.is_empty() && !none.needs_download);
    }

    #[test]
    fn a_listed_copy_takes_its_size_and_subscription_from_the_manifest() {
        let root = tmp("acf-size");
        let ws = root.join("content").join("221100");
        fake_mod(&ws, 7, "listed", 10);
        fake_mod(&ws, 8, "unsized", 10);
        fs::write(root.join(MANIFEST), STEAM_MANIFEST).unwrap();
        let steam = scan_workshop_dir(&ws, ModSource::Steam).unwrap();
        let get = |mods: &[InstalledMod], id| mods.iter().find(|m| m.id == id).cloned().unwrap();
        assert_eq!(get(&steam, 7).size, 123_456_789, "no walk");
        assert!(
            get(&steam, 8).size >= 10,
            "walked: the manifest has no size"
        );
        assert!(get(&steam, 7).subscribed);
        assert!(!get(&steam, 8).subscribed);
        // The launcher's copies are its own to update, whatever a manifest says.
        let own = scan_workshop_dir(&ws, ModSource::Launcher).unwrap();
        assert!(!get(&own, 7).subscribed);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_new_version_in_the_manifest_walks_the_size_again() {
        let d = tmp("stamp");
        fs::write(d.join("a"), [0u8; 3]).unwrap();
        assert_eq!(du_dir_cached(&d, 1, 100).unwrap(), 3);
        // Same directory mtime and meta.cpp: only the version moved.
        let f = fs::File::options().append(true).open(d.join("a")).unwrap();
        std::io::Write::write_all(&mut &f, &[0u8; 2]).unwrap();
        assert_eq!(du_dir_cached(&d, 1, 100).unwrap(), 3, "cached");
        assert_eq!(du_dir_cached(&d, 1, 200).unwrap(), 5);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_manifest_gives_the_installed_version() {
        let root = tmp("manifest");
        let ws = root.join("content").join("221100");
        fake_mod(&ws, 7, "patched", 1);
        fake_mod(&ws, 8, "unlisted", 1);
        touch(&ws, 7, 1_000);
        touch(&ws, 8, 1_000);
        fs::write(
            root.join("appworkshop_221100.acf"),
            "\"AppWorkshop\"\n{\n\t\"appid\"\t\t\"221100\"\n\t\"WorkshopItemsInstalled\"\n\t{\n\t\t\"7\"\n\t\t{\n\t\t\t\"size\"\t\t\"1\"\n\t\t\t\"timeupdated\"\t\t\"5000\"\n\t\t}\n\t}\n\t\"WorkshopItemDetails\"\n\t{\n\t\t\"8\"\n\t\t{\n\t\t\t\"timeupdated\"\t\t\"9000\"\n\t\t}\n\t}\n}\n",
        )
        .unwrap();
        let mods = scan_workshop_dir(&ws, ModSource::Steam).unwrap();
        let at = |id| mods.iter().find(|m| m.id == id).unwrap().local_updated;
        assert_eq!(
            at(7),
            5_000,
            "an update that left meta.cpp alone still counts"
        );
        assert_eq!(at(8), 1_000, "only installed items, not the details");
        touch(&ws, 7, 6_000);
        let mods = scan_workshop_dir(&ws, ModSource::Steam).unwrap();
        assert_eq!(
            mods.iter().find(|m| m.id == 7).unwrap().local_updated,
            6_000
        );
        let _ = fs::remove_dir_all(&root);
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
        let gone = delete_steam_copies_of(&dirs, &[4]);
        assert!(gone.iter().all(|(id, r)| *id == 4 && r.is_ok()));
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

        // In bulk: each in order, up to the first failure.
        fake_mod(&dirs.launcher, 5, "five", 1);
        fake_mod(&dirs.launcher, 6, "six", 1);
        let bulk = delete_mods(&dirs, Some(&dayz), &[5, 4, 6]);
        let got: Vec<_> = bulk.iter().map(|(id, r)| (*id, r.is_ok())).collect();
        assert_eq!(got, [(5, true), (4, false)]);
        assert!(!dirs.launcher.join("5").exists() && dirs.launcher.join("6").exists());
        let _ = fs::remove_dir_all(dirs.launcher.join("6"));

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
