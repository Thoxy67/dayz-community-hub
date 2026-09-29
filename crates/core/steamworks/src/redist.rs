//! Valve's library, embedded at build time (see build.rs) and written to
//! disk the first time the Steamworks mode is used.
//!
//! It goes to `<steamworks dir>/<hash>/<name>`: a new app version with a new
//! library gets a new folder, so a copy another running instance has loaded
//! (Windows locks it) never needs overwriting. A copy whose bytes differ from
//! the embedded ones (damaged, truncated) is rewritten.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The library for this target: its file name and bytes. None where the
/// build embeds none (anything but x86_64 Linux and Windows).
#[cfg(dz_steamworks_redist)]
pub(crate) const EMBEDDED: Option<(&str, &[u8])> = Some((
    env!("DZ_STEAMWORKS_REDIST_NAME"),
    include_bytes!(env!("DZ_STEAMWORKS_REDIST")),
));
#[cfg(not(dz_steamworks_redist))]
pub(crate) const EMBEDDED: Option<(&str, &[u8])> = None;

/// FNV-1a, 64 bits: enough to name a folder after a file's content.
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Write `bytes` as `<dir>/<hash>/<name>` unless that file already holds
/// exactly them, and remove the folders of other versions. Returns the path.
pub(crate) fn extract(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let folder = dir.join(format!("{:016x}", fnv1a(bytes)));
    let path = folder.join(name);
    if fs::read(&path).is_ok_and(|b| b == bytes) {
        return Ok(path);
    }
    fs::create_dir_all(&folder)?;
    // Written aside and renamed into place, so a crash never leaves half a
    // library where the next start would load it.
    let tmp = folder.join(format!("{name}.{}.tmp", std::process::id()));
    fs::write(&tmp, bytes)?;
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        // Another instance may have put the same file there first.
        if !fs::read(&path).is_ok_and(|b| b == bytes) {
            return Err(e);
        }
    }
    prune(dir, &folder);
    Ok(path)
}

/// Remove every version folder but `keep`. One still loaded by another
/// instance on Windows cannot go: it is left for next time.
fn prune(dir: &Path, keep: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p != keep && p.is_dir() {
            let _ = fs::remove_dir_all(&p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dz-steamworks-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn fnv1a_matches_the_reference_values() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn extracts_once_and_rewrites_a_damaged_copy() {
        let dir = scratch("extract");
        let p = extract(&dir, "lib.so", b"library v1").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"library v1");
        assert!(p.starts_with(&dir));

        // Same bytes: the same file, untouched.
        let modified = fs::metadata(&p).unwrap().modified().unwrap();
        assert_eq!(extract(&dir, "lib.so", b"library v1").unwrap(), p);
        assert_eq!(fs::metadata(&p).unwrap().modified().unwrap(), modified);

        // Damaged: rewritten.
        fs::write(&p, b"libr").unwrap();
        assert_eq!(extract(&dir, "lib.so", b"library v1").unwrap(), p);
        assert_eq!(fs::read(&p).unwrap(), b"library v1");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_version_replaces_the_old_folder() {
        let dir = scratch("versions");
        let old = extract(&dir, "lib.so", b"library v1").unwrap();
        let new = extract(&dir, "lib.so", b"library v2").unwrap();
        assert_ne!(old.parent(), new.parent());
        assert!(!old.exists());
        assert_eq!(fs::read(&new).unwrap(), b"library v2");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn this_target_embeds_a_library() {
        if cfg!(all(
            target_arch = "x86_64",
            any(target_os = "linux", target_os = "windows")
        )) {
            let (name, bytes) = EMBEDDED.expect("embedded");
            assert!(name.contains("steam_api"));
            // An ELF or a PE image, not an empty placeholder.
            assert!(bytes.starts_with(b"\x7fELF") || bytes.starts_with(b"MZ"));
        }
    }
}
