//! The installed game's version, as servers give theirs (`1.30.164200`).
//!
//! It is in the executable's version resource (`VS_FIXEDFILEINFO`): the file
//! version `1.30.164.200` is the major, the minor, then the build written in
//! two parts, thousands and the rest (164 200). The same file is run under
//! Proton, so this works on every system.

use std::path::Path;

/// `VS_FIXEDFILEINFO`'s signature, little-endian.
const SIGNATURE: [u8; 4] = [0xbd, 0x04, 0xef, 0xfe];
/// The largest executable read (DayZ's is about 26 MB).
const MAX_BYTES: u64 = 256 * 1024 * 1024;

/// The game's version from `DayZ_x64.exe` under `dayz`, if it can be read.
/// Blocking: reads the executable's resource section (a hundred kilobytes
/// of DayZ's 26 MB), or the whole file when its headers do not say where
/// that is.
pub fn installed(dayz: &Path) -> Option<String> {
    let exe = dayz.join("DayZ_x64.exe");
    if let Some(v) = from_resources(&exe) {
        return Some(v);
    }
    if std::fs::metadata(&exe).ok()?.len() > MAX_BYTES {
        return None;
    }
    from_exe(&std::fs::read(exe).ok()?)
}

/// The version in the executable's `.rsrc` section, read alone.
fn from_resources(exe: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(exe).ok()?;
    let mut read_at = |at: u64, len: usize| -> Option<Vec<u8>> {
        let mut buf = vec![0u8; len];
        file.seek(SeekFrom::Start(at)).ok()?;
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    };
    let dos = read_at(0, 64)?;
    let pe = u64::from(le32(&dos, 0x3c)?);
    let coff = read_at(pe, 24)?;
    let sections = usize::from(le16(&coff, 6)?);
    let optional = u64::from(le16(&coff, 20)?);
    if &coff[..4] != b"PE\0\0" || sections > 96 {
        return None;
    }
    let table = read_at(pe + 24 + optional, sections * SECTION)?;
    let (at, len) = rsrc_section(&table)?;
    if u64::from(len) > MAX_BYTES {
        return None;
    }
    from_exe(&read_at(u64::from(at), len as usize)?)
}

/// The size of a section header.
const SECTION: usize = 40;

/// Where the `.rsrc` section is in the file (its offset and size), from the
/// section table.
fn rsrc_section(table: &[u8]) -> Option<(u32, u32)> {
    let (headers, _) = table.as_chunks::<SECTION>();
    let h = headers.iter().find(|h| h[..8] == *b".rsrc\0\0\0")?;
    Some((le32(h, 20)?, le32(h, 16)?))
}

fn le16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The version in an executable's bytes. The resource sits near the end: the
/// last signature is the file's own.
pub fn from_exe(bytes: &[u8]) -> Option<String> {
    let at = bytes.windows(4).rposition(|w| w == SIGNATURE)?;
    let word = |i: usize| -> Option<u32> {
        let b = bytes.get(at + 4 * i..at + 4 * i + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    // After the signature: the structure's version, then the file version.
    let (ms, ls) = (word(2)?, word(3)?);
    let (major, minor, hi, lo) = (ms >> 16, ms & 0xffff, ls >> 16, ls & 0xffff);
    if major == 0 && minor == 0 {
        return None;
    }
    Some(format!("{major}.{minor}.{}", hi * 1000 + lo))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(ms: u32, ls: u32) -> Vec<u8> {
        let mut b = vec![0u8; 64];
        b.extend_from_slice(&SIGNATURE);
        for w in [0x0001_0000, ms, ls, 0, 0] {
            b.extend_from_slice(&u32::to_le_bytes(w));
        }
        b
    }

    #[test]
    fn reads_the_file_version() {
        assert_eq!(
            from_exe(&exe((1 << 16) | 30, (164 << 16) | 200)),
            Some("1.30.164200".into())
        );
        assert_eq!(
            from_exe(&exe((1 << 16) | 30, (164 << 16) | 78)),
            Some("1.30.164078".into())
        );
    }

    /// A minimal PE file: the DOS header, the PE and COFF headers, an
    /// optional header of 16 bytes, two sections, then `.rsrc`'s content.
    fn pe(rsrc: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 0x80];
        b[..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b.extend_from_slice(b"PE\0\0");
        let mut coff = [0u8; 20];
        coff[2..4].copy_from_slice(&2u16.to_le_bytes());
        coff[16..18].copy_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(&coff);
        b.extend_from_slice(&[0u8; 16]);
        let data = (b.len() + 2 * SECTION) as u32;
        for (name, at, len) in [
            (*b".text\0\0\0", 0u32, 0u32),
            (*b".rsrc\0\0\0", data, rsrc.len() as u32),
        ] {
            let mut h = [0u8; SECTION];
            h[..8].copy_from_slice(&name);
            h[16..20].copy_from_slice(&len.to_le_bytes());
            h[20..24].copy_from_slice(&at.to_le_bytes());
            b.extend_from_slice(&h);
        }
        b.extend_from_slice(rsrc);
        // A signature after the section, which only a whole-file scan sees.
        b.extend_from_slice(&exe(2 << 16, 0));
        b
    }

    #[test]
    fn reads_only_the_resource_section() {
        let dir = std::env::temp_dir().join(format!("dzch-version-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("DayZ_x64.exe");
        std::fs::write(&path, pe(&exe((1 << 16) | 30, (164 << 16) | 200))).unwrap();
        let got = installed(&dir);
        // Not a PE file: the whole file is searched.
        std::fs::write(&path, exe((1 << 16) | 29, 7)).unwrap();
        let fallback = installed(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got.as_deref(), Some("1.30.164200"));
        assert_eq!(fallback.as_deref(), Some("1.29.7"));
    }

    /// By hand: `DAYZ=<…/common/DayZ> cargo test -p dz-game -- --ignored --nocapture version_here`.
    #[test]
    #[ignore = "reads this machine's game"]
    fn version_here() {
        let dir = std::path::PathBuf::from(std::env::var_os("DAYZ").unwrap_or_default());
        let exe = dir.join("DayZ_x64.exe");
        println!("resources: {:?}", from_resources(&exe));
        println!(
            "whole file: {:?}",
            std::fs::read(&exe).ok().and_then(|b| from_exe(&b))
        );
    }

    #[test]
    fn junk_is_none() {
        assert_eq!(from_exe(b""), None);
        assert_eq!(from_exe(&SIGNATURE), None);
        assert_eq!(from_exe(&exe(0, 5)), None);
    }
}
