//! Watching the mod folders, so the window reads them again when they
//! change (Steam installs, updates or removes a mod, the player deletes a
//! folder, an `@` link comes or goes) instead of reading them on a timer.
//!
//! The watches are few and not recursive: each content folder (a mod's
//! folder comes or goes), each `steamapps/workshop` folder (Steam replaces
//! its manifest by a rename, which only the parent folder sees), and the
//! DayZ folder (its `@<id>` links). A download writes many files: changes
//! are debounced, and one burst makes one call.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dz_game::mods::{MANIFEST, ModDirs, workshop_of};
use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

/// How long the folders must be still before a change is told.
const QUIET: Duration = Duration::from_millis(500);

/// Which changes in a watched folder count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Only {
    /// Anything: a content folder, whose entries are mods.
    Any,
    /// The workshop manifest, in a `steamapps/workshop` folder.
    Manifest,
    /// `@` links, in the DayZ folder.
    Links,
}

/// What is told of a change: called from the watcher's thread.
pub(crate) type OnChange = Arc<dyn Fn() + Send + Sync>;

struct Watching {
    targets: Vec<(PathBuf, Only)>,
    /// Swapped when the window asks again with the same folders.
    on_change: Arc<Mutex<OnChange>>,
    _debouncer: Debouncer<notify_debouncer_full::notify::RecommendedWatcher, RecommendedCache>,
}

static WATCHING: Mutex<Option<Watching>> = Mutex::new(None);

/// The folders to watch for `dirs` and `dayz`, those that exist.
pub(crate) fn targets(dirs: &ModDirs, dayz: Option<&Path>) -> Vec<(PathBuf, Only)> {
    let mut out: Vec<(PathBuf, Only)> = Vec::new();
    let mut add = |p: &Path, only: Only| {
        if p.is_dir() && !out.iter().any(|(q, _)| q == p) {
            out.push((p.to_path_buf(), only));
        }
    };
    for content in std::iter::once(&dirs.launcher).chain(&dirs.steam) {
        add(content, Only::Any);
        if let Some(workshop) = workshop_of(content) {
            add(workshop, Only::Manifest);
        }
    }
    if let Some(dayz) = dayz {
        add(dayz, Only::Links);
    }
    out
}

/// Whether a change at `path` counts, for the folders watched.
pub(crate) fn counts(path: &Path, targets: &[(PathBuf, Only)]) -> bool {
    // A watched folder itself (removed, renamed): read again.
    if targets.iter().any(|(dir, _)| dir == path) {
        return true;
    }
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return false;
    };
    let name = name.to_string_lossy();
    targets
        .iter()
        .filter(|(dir, _)| dir == parent)
        .any(|(_, only)| match only {
            Only::Any => true,
            Only::Manifest => name.eq_ignore_ascii_case(MANIFEST),
            Only::Links => name.starts_with('@'),
        })
}

/// Watch the folders of `targets`, calling `on_change` once per burst of
/// changes that count. The same folders as the watch running keep it, with
/// the new `on_change`; other folders (a library added, the Steam folder
/// changed) replace it. Blocking: sets up the OS watches.
pub(crate) fn watch(targets: Vec<(PathBuf, Only)>, on_change: OnChange) -> Result<(), String> {
    let mut slot = WATCHING.lock().map_err(|e| e.to_string())?;
    if let Some(w) = slot.as_ref()
        && w.targets == targets
    {
        if let Ok(mut cb) = w.on_change.lock() {
            *cb = on_change;
        }
        return Ok(());
    }
    // The old watch goes first: on Windows it holds a handle on each folder.
    *slot = None;
    if targets.is_empty() {
        return Ok(());
    }
    let shared = Arc::new(Mutex::new(on_change));
    let call = Arc::clone(&shared);
    let seen = targets.clone();
    let mut debouncer = new_debouncer(QUIET, None, move |res: DebounceEventResult| {
        let changed = match res {
            Ok(events) => events
                .iter()
                .any(|e| e.paths.iter().any(|p| counts(p, &seen))),
            // The watch overflowed or failed: what changed is unknown.
            Err(_) => true,
        };
        let cb = call.lock().ok().map(|cb| Arc::clone(&cb));
        if changed && let Some(cb) = cb {
            cb();
        }
    })
    .map_err(|e| e.to_string())?;
    for (dir, _) in &targets {
        if let Err(e) = debouncer.watch(dir, RecursiveMode::NonRecursive) {
            eprintln!("[mods] {} not watched: {e}", dir.display());
        }
    }
    *slot = Some(Watching {
        targets,
        on_change: shared,
        _debouncer: debouncer,
    });
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn only_mods_the_manifest_and_links_count() {
        let root = std::env::temp_dir().join(format!("dzch-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let content = root.join("lib/steamapps/workshop/content/221100");
        let dayz = root.join("lib/steamapps/common/DayZ");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::create_dir_all(&dayz).unwrap();
        let dirs = ModDirs {
            launcher: root.join("not-made-yet/content/221100"),
            steam: vec![content.clone()],
        };
        let t = targets(&dirs, Some(&dayz));
        let workshop = root.join("lib/steamapps/workshop");
        assert_eq!(
            t,
            [
                (content.clone(), Only::Any),
                (workshop.clone(), Only::Manifest),
                (dayz.clone(), Only::Links),
            ],
            "a folder that does not exist is not watched"
        );
        assert!(counts(&content.join("1559212036"), &t));
        assert!(counts(&workshop.join(MANIFEST), &t));
        assert!(!counts(&workshop.join("appworkshop_105600.acf"), &t));
        assert!(!counts(&workshop.join("downloads"), &t));
        assert!(counts(&dayz.join("@1559212036"), &t));
        assert!(!counts(&dayz.join("DayZ_x64.exe"), &t));
        assert!(counts(&content, &t), "the folder itself went");
        assert!(
            !counts(&content.join("1/addons/a.pbo"), &t),
            "not recursive"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
