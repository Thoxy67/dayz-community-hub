//! Files the launcher writes and must never find half-written: the profile, the
//! play log, the interface's preferences, the caches, the updater's new executable.

use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Write `data` to `path` atomically: into a new file in the same directory (so the
/// rename stays on one file system), flushed to the disk, then renamed over `path`,
/// which replaces it on every platform. A reader sees the old file or the new one,
/// never half of it, and a power cut just after leaves one of the two, not an empty
/// file (a rename is journalled, the data before it is not unless flushed). The
/// temporary file is removed if anything fails.
///
/// On Windows a scanner or the search indexer that holds the file open for a moment
/// makes the rename fail with a sharing violation: it is tried again for a second or
/// so instead of losing the write. On Unix the directory is flushed too, so the
/// rename itself survives a crash.
///
/// Blocking: async callers run it on the blocking pool.
pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_owned());
    // Unique per process and per write: two writes in flight never share one.
    let tmp = dir.join(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let written = std::fs::File::create(&tmp)
        .and_then(|mut f| {
            f.write_all(data)?;
            f.sync_all()
        })
        .and_then(|()| replace(&tmp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return written;
    }
    #[cfg(unix)]
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Rename `from` over `to`, through a passing lock on Windows.
pub fn replace(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        crate::win::retry_locked(|| std::fs::rename(from, to))
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(from, to)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn writes_replace_and_leave_nothing_beside() {
        let dir = std::env::temp_dir().join(format!("dz-common-fs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sub").join("a.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("a.json")]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
