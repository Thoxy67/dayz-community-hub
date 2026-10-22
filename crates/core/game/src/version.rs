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
/// Blocking: reads the file.
pub fn installed(dayz: &Path) -> Option<String> {
    let exe = dayz.join("DayZ_x64.exe");
    if std::fs::metadata(&exe).ok()?.len() > MAX_BYTES {
        return None;
    }
    from_exe(&std::fs::read(exe).ok()?)
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

    #[test]
    fn junk_is_none() {
        assert_eq!(from_exe(b""), None);
        assert_eq!(from_exe(&SIGNATURE), None);
        assert_eq!(from_exe(&exe(0, 5)), None);
    }
}
