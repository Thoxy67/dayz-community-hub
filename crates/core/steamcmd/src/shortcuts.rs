//! Steam's non-Steam games: `userdata/<account>/config/shortcuts.vdf`, a
//! binary VDF file, read and written here in full so that every entry the
//! player already has survives untouched.
//!
//! The format: a node is a type byte, a key ending in a NUL, then its value.
//! `0x00` opens a map (closed by `0x08`), `0x01` is a string ending in a NUL,
//! `0x02` a little-endian 32-bit integer; `0x03` (float), `0x04` (pointer),
//! `0x06` (colour), `0x07` (unsigned 64-bit) and `0x0a` (signed 64-bit) are
//! kept as their bytes. A file holds one map, `shortcuts`, whose children are
//! named `0`, `1`, `2`… A file with anything else (a wide string, a cut file)
//! is refused rather than rewritten wrong.
//!
//! The shortcut's id is the one Steam derives itself: the CRC-32 of the `Exe`
//! value (quotes included) followed by the name, with the top bit set. Art in
//! `config/grid/` and `steam://rungameid/` links are keyed by it.
//!
//! Steam reads the file when it starts and writes its own copy back whenever
//! its list changes and when it exits: an edit made while it runs is lost, or
//! comes back over the player's own changes. [`add`] is therefore only called
//! with Steam closed (the caller checks).

use std::path::{Path, PathBuf};

const MAP: u8 = 0x00;
const STR: u8 = 0x01;
const INT: u8 = 0x02;
const END: u8 = 0x08;

/// A value in a binary VDF file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Map(Vec<(Vec<u8>, Value)>),
    Str(Vec<u8>),
    Int(i32),
    /// A type kept as its bytes: (type, bytes).
    Raw(u8, Vec<u8>),
}

impl Value {
    fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(kids) => kids
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key.as_bytes()))
                .map(|(_, v)| v),
            _ => None,
        }
    }

    fn str(&self, key: &str) -> Option<&[u8]> {
        match self.get(key) {
            Some(Value::Str(s)) => Some(s),
            _ => None,
        }
    }
}

/// Read a binary VDF document: its top-level entries (a map's children).
pub fn parse(bytes: &[u8]) -> Result<Vec<(Vec<u8>, Value)>, String> {
    let mut at = 0usize;
    let kids = map_body(bytes, &mut at, 0, true)?;
    Ok(kids)
}

/// How deep maps may nest: Steam's files go three levels down.
const MAX_DEPTH: usize = 32;

fn map_body(
    b: &[u8],
    at: &mut usize,
    depth: usize,
    top: bool,
) -> Result<Vec<(Vec<u8>, Value)>, String> {
    if depth > MAX_DEPTH {
        return Err("shortcuts.vdf nests too deep".into());
    }
    let mut kids = Vec::new();
    loop {
        let Some(&ty) = b.get(*at) else {
            // The top level may end with the file; a map inside may not.
            return if top {
                Ok(kids)
            } else {
                Err("shortcuts.vdf ends inside a map".into())
            };
        };
        *at += 1;
        if ty == END {
            return Ok(kids);
        }
        let key = cstr(b, at)?;
        let v = match ty {
            MAP => Value::Map(map_body(b, at, depth + 1, false)?),
            STR => Value::Str(cstr(b, at)?),
            INT => {
                let raw = take(b, at, 4)?;
                let mut four = [0u8; 4];
                four.copy_from_slice(raw);
                Value::Int(i32::from_le_bytes(four))
            }
            0x03 | 0x04 | 0x06 => Value::Raw(ty, take(b, at, 4)?.to_vec()),
            0x07 | 0x0a => Value::Raw(ty, take(b, at, 8)?.to_vec()),
            other => {
                return Err(format!(
                    "shortcuts.vdf has an unknown value type {other:#04x}"
                ));
            }
        };
        kids.push((key, v));
    }
}

fn cstr(b: &[u8], at: &mut usize) -> Result<Vec<u8>, String> {
    let rest = b.get(*at..).ok_or("shortcuts.vdf is cut")?;
    let n = rest
        .iter()
        .position(|&c| c == 0)
        .ok_or("shortcuts.vdf is cut inside a string")?;
    let s = rest.get(..n).unwrap_or_default().to_vec();
    *at += n + 1;
    Ok(s)
}

fn take<'a>(b: &'a [u8], at: &mut usize, n: usize) -> Result<&'a [u8], String> {
    let end = at.checked_add(n).ok_or("shortcuts.vdf is cut")?;
    let s = b.get(*at..end).ok_or("shortcuts.vdf is cut")?;
    *at = end;
    Ok(s)
}

/// Write a document's top-level entries as Steam does: each node, then the
/// closing `0x08` of the top level.
pub fn write(kids: &[(Vec<u8>, Value)]) -> Vec<u8> {
    let mut out = Vec::new();
    write_kids(&mut out, kids);
    out
}

fn write_kids(out: &mut Vec<u8>, kids: &[(Vec<u8>, Value)]) {
    for (k, v) in kids {
        let key = |out: &mut Vec<u8>| {
            out.extend(k.iter().copied().filter(|&c| c != 0));
            out.push(0);
        };
        match v {
            Value::Map(m) => {
                out.push(MAP);
                key(out);
                write_kids(out, m);
            }
            Value::Str(s) => {
                out.push(STR);
                key(out);
                out.extend(s.iter().copied().filter(|&c| c != 0));
                out.push(0);
            }
            Value::Int(i) => {
                out.push(INT);
                key(out);
                out.extend(i.to_le_bytes());
            }
            Value::Raw(t, bytes) => {
                out.push(*t);
                key(out);
                out.extend(bytes);
            }
        }
    }
    out.push(END);
}

/// CRC-32 (IEEE, as zlib computes it).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// The id Steam gives a non-Steam game: from its `Exe` value as stored (with
/// its quotes) and its name.
pub fn shortcut_id(exe: &str, name: &str) -> u32 {
    let mut s = Vec::with_capacity(exe.len() + name.len());
    s.extend_from_slice(exe.as_bytes());
    s.extend_from_slice(name.as_bytes());
    crc32(&s) | 0x8000_0000
}

/// `steam://rungameid/<id>` wants the 64-bit game id of a shortcut.
pub fn rungameid(id: u32) -> u64 {
    (u64::from(id) << 32) | 0x0200_0000
}

/// The launcher as a non-Steam game.
#[derive(Clone, Debug)]
pub struct Shortcut {
    pub name: String,
    /// The program, unquoted.
    pub exe: PathBuf,
    /// Its folder, unquoted.
    pub start_dir: PathBuf,
    /// An image file for the library and the Game Mode tiles.
    pub icon: Option<PathBuf>,
    /// Arguments, as Steam passes them (already quoted where needed).
    pub launch_options: String,
}

/// What [`add`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Added {
    /// A new entry, with its id.
    New(u32),
    /// The launcher was there already (same program): its entry now says the
    /// same as `s`, the player's other settings (tags, hidden, overlay) kept.
    Updated(u32),
}

fn quoted(p: &Path) -> String {
    format!("\"{}\"", p.to_string_lossy())
}

/// The entry, as Steam writes a new one.
fn entry(s: &Shortcut, id: u32) -> Vec<(Vec<u8>, Value)> {
    let st = |k: &str, v: &str| (k.as_bytes().to_vec(), Value::Str(v.as_bytes().to_vec()));
    let int = |k: &str, v: i32| (k.as_bytes().to_vec(), Value::Int(v));
    let icon = s
        .icon
        .as_deref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    vec![
        // The id's bits, as Steam stores them (a signed field).
        int("appid", i32::from_le_bytes(id.to_le_bytes())),
        st("AppName", &s.name),
        st("Exe", &quoted(&s.exe)),
        st("StartDir", &quoted(&s.start_dir)),
        st("icon", &icon),
        st("ShortcutPath", ""),
        st("LaunchOptions", &s.launch_options),
        int("IsHidden", 0),
        int("AllowDesktopConfig", 1),
        int("AllowOverlay", 1),
        int("OpenVR", 0),
        int("Devkit", 0),
        st("DevkitGameID", ""),
        int("DevkitOverrideAppID", 0),
        int("LastPlayTime", 0),
        st("FlatpakAppID", ""),
        (b"tags".to_vec(), Value::Map(Vec::new())),
    ]
}

/// The file's entries with `s` added, or brought up to date when an entry
/// already starts the same program. Pure: [`add`] reads and writes the file.
pub fn merge(doc: Vec<(Vec<u8>, Value)>, s: &Shortcut) -> (Vec<(Vec<u8>, Value)>, Added) {
    let exe = quoted(&s.exe);
    let id = shortcut_id(&exe, &s.name);
    let mut doc = doc;
    let at = doc
        .iter()
        .position(|(k, v)| k.eq_ignore_ascii_case(b"shortcuts") && matches!(v, Value::Map(_)));
    let at = match at {
        Some(i) => i,
        None => {
            doc.push((b"shortcuts".to_vec(), Value::Map(Vec::new())));
            doc.len() - 1
        }
    };
    let Some((_, Value::Map(list))) = doc.get_mut(at) else {
        return (doc, Added::New(id));
    };
    // The same program (the AppImage's path, or the executable's) is the same
    // shortcut, whatever its name became; else one of the same name is (the
    // AppImage was moved or renamed).
    let same_exe = |v: &Value| {
        v.str("Exe")
            .is_some_and(|e| e == exe.as_bytes() || e == s.exe.to_string_lossy().as_bytes())
    };
    let same_name = |v: &Value| v.str("AppName").is_some_and(|n| n == s.name.as_bytes());
    let found = list
        .iter()
        .position(|(_, v)| same_exe(v))
        .or_else(|| list.iter().position(|(_, v)| same_name(v)));
    if let Some((_, Value::Map(old))) = found.and_then(|i| list.get_mut(i)) {
        let keep = |k: &[u8]| {
            [
                "IsHidden",
                "AllowOverlay",
                "AllowDesktopConfig",
                "OpenVR",
                "LastPlayTime",
                "tags",
            ]
            .iter()
            .any(|n| k.eq_ignore_ascii_case(n.as_bytes()))
        };
        // The id stays (Steam keys the art and the controller layout to it), and so
        // does an icon the player chose.
        let id = match old.iter().find(|(k, _)| k.eq_ignore_ascii_case(b"appid")) {
            Some((_, Value::Int(was))) => u32::from_le_bytes(was.to_le_bytes()),
            _ => id,
        };
        let ours = s
            .icon
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let theirs = old.iter().find_map(|(k, v)| match v {
            Value::Str(i) if k.eq_ignore_ascii_case(b"icon") => Some(i),
            _ => None,
        });
        let theirs = theirs
            .filter(|i| !i.is_empty() && i.as_slice() != ours.as_bytes())
            .cloned();
        let mut fresh = entry(s, id);
        for (k, v) in fresh.iter_mut() {
            if k.eq_ignore_ascii_case(b"icon")
                && let Some(i) = &theirs
            {
                *v = Value::Str(i.clone());
            } else if keep(k)
                && let Some((_, was)) = old.iter().find(|(ok, _)| ok.eq_ignore_ascii_case(k))
            {
                *v = was.clone();
            }
        }
        // Fields this version does not know stay as they were.
        for (k, v) in old.iter() {
            if !fresh.iter().any(|(fk, _)| fk.eq_ignore_ascii_case(k)) {
                fresh.push((k.clone(), v.clone()));
            }
        }
        *old = fresh;
        return (doc, Added::Updated(id));
    }
    // The next free index (Steam numbers them 0, 1, 2…).
    let next = list
        .iter()
        .filter_map(|(k, _)| std::str::from_utf8(k).ok()?.parse::<u32>().ok())
        .max()
        .map_or(0, |n| n.saturating_add(1));
    list.push((next.to_string().into_bytes(), Value::Map(entry(s, id))));
    (doc, Added::New(id))
}

/// `<steam>/userdata/<account>/config/shortcuts.vdf`.
pub fn shortcuts_path(steam: &Path, account: u64) -> PathBuf {
    steam
        .join("userdata")
        .join(account.to_string())
        .join("config")
        .join("shortcuts.vdf")
}

/// Add the launcher to `account`'s non-Steam games in the Steam install at
/// `steam`, keeping every other entry. The file is written beside itself and
/// renamed over the old one; the first time, the old one is kept as
/// `shortcuts.vdf.dzch-backup`. Blocking. Steam must not be running.
pub fn add(steam: &Path, account: u64, s: &Shortcut) -> Result<Added, String> {
    let path = shortcuts_path(steam, account);
    let user = path
        .parent()
        .and_then(Path::parent)
        .ok_or("no userdata folder")?;
    if !user.is_dir() {
        return Err(format!("{} does not exist", user.display()));
    }
    let doc = match std::fs::read(&path) {
        Ok(bytes) => {
            let backup = path.with_extension("vdf.dzch-backup");
            if !backup.exists() {
                std::fs::write(&backup, &bytes)
                    .map_err(|e| format!("{}: {e}", backup.display()))?;
            }
            parse(&bytes)?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let (doc, added) = merge(doc, s);
    let dir = path.parent().ok_or("no config folder")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = path.with_extension("vdf.dzch-tmp");
    std::fs::write(&tmp, write(&doc)).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", path.display())
    })?;
    Ok(added)
}

/// The account whose library gets the shortcut: the one Steam logged in last
/// (`config/loginusers.vdf`), else the only one with a `userdata` folder.
pub fn account(steam: &Path) -> Option<u64> {
    std::fs::read_to_string(steam.join("config").join("loginusers.vdf"))
        .ok()
        .and_then(|t| crate::detect::most_recent_account(&t))
        .filter(|a| steam.join("userdata").join(a.to_string()).is_dir())
        .or_else(|| {
            let ids: Vec<u64> = std::fs::read_dir(steam.join("userdata"))
                .ok()?
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().to_str()?.parse::<u64>().ok())
                // `0` is Steam's anonymous user, `ac` and such are not ids.
                .filter(|&id| id != 0)
                .collect();
            match ids.as_slice() {
                [one] => Some(*one),
                _ => None,
            }
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dzch-shortcuts-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn me(exe: &str) -> Shortcut {
        Shortcut {
            name: "DayZ Community Hub".into(),
            exe: PathBuf::from(exe),
            start_dir: PathBuf::from("/home/deck/Applications"),
            icon: Some(PathBuf::from(
                "/home/deck/.local/share/dayz-community-hub/icon.png",
            )),
            launch_options: String::new(),
        }
    }

    /// A file as Steam writes it, with one game the player added.
    fn steams_file() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(b"\x00shortcuts\x00");
        b.extend(b"\x000\x00");
        b.extend(b"\x02appid\x00");
        b.extend(0x9234_5678u32.to_le_bytes());
        b.extend(b"\x01AppName\x00Heroic\x00");
        b.extend(b"\x01Exe\x00\"/usr/bin/heroic\"\x00");
        b.extend(b"\x02IsHidden\x00\x00\x00\x00\x00");
        b.extend(b"\x07LastPlayTime64\x00\x01\x02\x03\x04\x05\x06\x07\x08");
        b.extend(b"\x00tags\x00\x010\x00favorite\x00\x08");
        b.extend(b"\x08");
        b.extend(b"\x08\x08");
        b
    }

    #[test]
    fn crc_and_ids_are_steams() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let id = shortcut_id("\"/usr/bin/app\"", "App");
        assert_eq!(id, crc32(b"\"/usr/bin/app\"App") | 0x8000_0000);
        assert!(id & 0x8000_0000 != 0);
        assert_eq!(rungameid(0x8000_0001), 0x8000_0001_0200_0000);
    }

    #[test]
    fn steams_file_reads_and_writes_back_byte_for_byte() {
        let bytes = steams_file();
        let doc = parse(&bytes).unwrap();
        assert_eq!(write(&doc), bytes);
        let Value::Map(list) = &doc[0].1 else {
            panic!()
        };
        assert_eq!(list[0].1.str("appname"), Some(&b"Heroic"[..]));
    }

    #[test]
    fn a_cut_or_unknown_file_is_refused() {
        let bytes = steams_file();
        assert!(parse(&bytes[..bytes.len() - 3]).is_err());
        assert!(parse(b"\x05wide\x00").is_err());
        assert!(parse(b"\x00shortcuts\x00\x01AppName\x00no end").is_err());
        assert_eq!(parse(b"").unwrap(), Vec::new());
    }

    #[test]
    fn the_launcher_is_added_after_the_players_games_and_updated_in_place() {
        let doc = parse(&steams_file()).unwrap();
        let (doc, added) = merge(doc, &me("/home/deck/Applications/dzch.AppImage"));
        let id = shortcut_id(
            "\"/home/deck/Applications/dzch.AppImage\"",
            "DayZ Community Hub",
        );
        assert_eq!(added, Added::New(id));
        let Value::Map(list) = &doc[0].1 else {
            panic!()
        };
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].1.str("AppName"), Some(&b"Heroic"[..]));
        assert_eq!(list[1].0, b"1");
        assert_eq!(list[1].1.get("appid"), Some(&Value::Int(id as i32)));
        assert_eq!(
            list[1].1.str("icon"),
            Some(&b"/home/deck/.local/share/dayz-community-hub/icon.png"[..])
        );

        // The player hid it and tagged it; adding again changes the options,
        // keeps theirs and adds no second entry.
        let mut doc = doc;
        if let Value::Map(list) = &mut doc[0].1
            && let Value::Map(e) = &mut list[1].1
        {
            for (k, v) in e.iter_mut() {
                if k == b"IsHidden" {
                    *v = Value::Int(1);
                }
            }
        }
        let mut again = me("/home/deck/Applications/dzch.AppImage");
        again.launch_options = "--instance deck".into();
        let (doc, added) = merge(doc, &again);
        assert_eq!(added, Added::Updated(id));
        let Value::Map(list) = &doc[0].1 else {
            panic!()
        };
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].1.get("IsHidden"), Some(&Value::Int(1)));
        assert_eq!(
            list[1].1.str("LaunchOptions"),
            Some(&b"--instance deck"[..])
        );
        // And it reads back as written.
        assert_eq!(parse(&write(&doc)).unwrap(), doc);

        // The player chose an icon, then the AppImage moved: the same entry (found
        // by its name) keeps its id and the icon, and points at the new path.
        let mut doc = doc;
        if let Value::Map(list) = &mut doc[0].1
            && let Value::Map(e) = &mut list[1].1
        {
            for (k, v) in e.iter_mut() {
                if k == b"icon" {
                    *v = Value::Str(b"/home/deck/art/dayz.png".to_vec());
                }
            }
        }
        let moved = me("/home/deck/Games/dzch-2.AppImage");
        let (doc, added) = merge(doc, &moved);
        assert_eq!(added, Added::Updated(id));
        let Value::Map(list) = &doc[0].1 else {
            panic!()
        };
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].1.get("appid"), Some(&Value::Int(id as i32)));
        assert_eq!(list[1].1.str("icon"), Some(&b"/home/deck/art/dayz.png"[..]));
        assert_eq!(
            list[1].1.str("Exe"),
            Some(&b"\"/home/deck/Games/dzch-2.AppImage\""[..])
        );
    }

    #[test]
    fn the_file_is_written_whole_with_a_backup_and_only_in_a_known_account() {
        let steam = temp("add");
        let s = me("/opt/dzch");
        assert!(
            add(&steam, 42, &s).is_err(),
            "no userdata/42: nothing made up"
        );

        let cfg = steam.join("userdata/42/config");
        std::fs::create_dir_all(&cfg).unwrap();
        // No file yet: one is made.
        assert!(matches!(add(&steam, 42, &s).unwrap(), Added::New(_)));
        let first = std::fs::read(cfg.join("shortcuts.vdf")).unwrap();
        assert!(matches!(add(&steam, 42, &s).unwrap(), Added::Updated(_)));
        assert_eq!(std::fs::read(cfg.join("shortcuts.vdf")).unwrap(), first);

        // The player's file: kept beside, and every game still there.
        std::fs::write(cfg.join("shortcuts.vdf"), steams_file()).unwrap();
        let _ = std::fs::remove_file(cfg.join("shortcuts.vdf.dzch-backup"));
        add(&steam, 42, &s).unwrap();
        assert_eq!(
            std::fs::read(cfg.join("shortcuts.vdf.dzch-backup")).unwrap(),
            steams_file()
        );
        let doc = parse(&std::fs::read(cfg.join("shortcuts.vdf")).unwrap()).unwrap();
        let Value::Map(list) = &doc[0].1 else {
            panic!()
        };
        assert_eq!(list.len(), 2);
        assert!(!cfg.join("shortcuts.vdf.dzch-tmp").exists());

        // A file it cannot read is left alone.
        std::fs::write(cfg.join("shortcuts.vdf"), b"\x00shortcuts\x00\x05").unwrap();
        assert!(add(&steam, 42, &s).is_err());
        assert_eq!(
            std::fs::read(cfg.join("shortcuts.vdf")).unwrap(),
            b"\x00shortcuts\x00\x05"
        );
        let _ = std::fs::remove_dir_all(&steam);
    }

    #[test]
    fn the_account_is_the_last_logged_in_else_the_only_one() {
        let steam = temp("account");
        assert_eq!(account(&steam), None);
        std::fs::create_dir_all(steam.join("userdata/7")).unwrap();
        std::fs::create_dir_all(steam.join("userdata/0")).unwrap();
        assert_eq!(account(&steam), Some(7));
        std::fs::create_dir_all(steam.join("userdata/39734272")).unwrap();
        assert_eq!(
            account(&steam),
            None,
            "two accounts and no record: no guess"
        );
        std::fs::create_dir_all(steam.join("config")).unwrap();
        std::fs::write(
            steam.join("config/loginusers.vdf"),
            r#""users" { "76561198000000000" { "MostRecent" "1" } }"#,
        )
        .unwrap();
        assert_eq!(account(&steam), Some(39_734_272));
        let _ = std::fs::remove_dir_all(&steam);
    }
}
