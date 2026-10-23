//! A shortcut on the desktop that joins one server: it starts the launcher with
//! `--connect ip:port` (a running launcher is handed the address by
//! [`super::instance`] and joins; a new one joins once ready), after the window's
//! own `--data-dir` and `--instance`, so it opens the same launcher.
//!
//! - Linux: `<desktop>/<server name>.desktop`, `Exec` the AppImage (`$APPIMAGE`)
//!   or this executable, marked executable (some desktops, GNOME's, still ask
//!   once before they trust a launcher put there).
//! - Windows: `<Desktop>\<server name>.lnk`, written in Rust (no COM), with the
//!   executable's icon.
//!
//! The desktop folder is the user's (`dirs::desktop_dir`); when there is none the
//! shortcut is not written elsewhere: [`Failure::NoDesktop`]. Blocking: call it off
//! the UI thread.

use std::path::{Path, PathBuf};

/// The longest file name kept, in bytes (before the extension): file systems take
/// 255, and ` (99).desktop` must fit after it.
const MAX_STEM_BYTES: usize = 200;
/// The last resort when neither the name nor the address leaves a usable file name.
const LAST_STEM: &str = "DayZ";

/// Why no shortcut was written; the window says it in its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The user has no desktop folder.
    NoDesktop,
    /// (Windows) the launcher is not on a lettered drive (a network share…).
    NotOnDrive,
    /// Shortcuts of this name, to other servers, fill every free name.
    Taken,
    /// The system's own error, with the path.
    Io(String),
}

/// What the shortcut says and does.
pub struct Shortcut<'a> {
    /// The server's name, the file's; may be empty.
    pub name: &'a str,
    /// The file name used when `name` leaves none (the address).
    pub fallback: &'a str,
    pub comment: &'a str,
    /// `ip:game port`.
    pub address: &'a str,
    /// The window's own flags, passed before `--connect` (`--data-dir <dir>`…).
    pub args: &'a [String],
}

/// A name made safe as a file name on every system: no path separators, no
/// characters Windows refuses, no control characters, no leading or trailing
/// dots and spaces, not one of Windows' reserved device names, at most
/// [`MAX_STEM_BYTES`] bytes; `None` when nothing is left.
pub fn file_stem(name: &str) -> Option<String> {
    let mut out = String::with_capacity(name.len().min(MAX_STEM_BYTES));
    let mut space = false;
    for c in name.chars() {
        let c = match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if c.is_control() => ' ',
            c => c,
        };
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        let room = out.len() + usize::from(space) + c.len_utf8();
        if room > MAX_STEM_BYTES {
            break;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    let out = out.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if out.is_empty() {
        return None;
    }
    let base = out
        .split('.')
        .next()
        .unwrap_or(out)
        .trim()
        .to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes().get(3).is_some_and(u8::is_ascii_digit));
    Some(if reserved {
        format!("{out}_")
    } else {
        out.to_string()
    })
}

/// The shortcut's file name: the server's name, else the fallback, else [`LAST_STEM`].
fn stem_of(s: &Shortcut) -> String {
    file_stem(s.name)
        .or_else(|| file_stem(s.fallback))
        .unwrap_or_else(|| LAST_STEM.to_string())
}

/// A Desktop Entry string value: `\` `\n` `\t` `\r` escaped (the spec's escapes),
/// on one line.
fn entry_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// The `Exec=` arguments after the executable: `args`, then `--connect <address>`.
fn exec_tail(args: &[String], address: &str) -> String {
    use super::register::exec_arg;
    let mut out = String::new();
    for a in args {
        out.push(' ');
        out.push_str(&exec_arg(a));
    }
    out.push_str(" --connect ");
    out.push_str(&exec_arg(address));
    out
}

/// The `.desktop` file that starts `exec` with `args` and `--connect <address>`.
pub fn desktop_entry(exec: &str, s: &Shortcut) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={}\n\
         Comment={}\n\
         Exec={}{}\n\
         Icon=dayz-community-hub\n\
         Terminal=false\n\
         Categories=Game;\n\
         StartupWMClass=dayz-community-hub\n",
        entry_value(s.name),
        entry_value(s.comment),
        super::register::exec_arg(exec),
        exec_tail(s.args, s.address),
    )
}

/// One Windows command-line argument, as `CommandLineToArgvW` reads it back:
/// quoted, without `"`, its trailing backslashes doubled (so the closing quote
/// stays one).
fn windows_arg(s: &str) -> String {
    let s: String = s.chars().filter(|c| *c != '"' && !c.is_control()).collect();
    let trailing = s.len() - s.trim_end_matches('\\').len();
    format!("\"{s}{}\"", "\\".repeat(trailing))
}

/// The Windows arguments: `args`, then `--connect "<address>"`.
pub fn windows_arguments(args: &[String], address: &str) -> String {
    let mut out = String::new();
    for a in args {
        out.push_str(&windows_arg(a));
        out.push(' ');
    }
    out.push_str("--connect ");
    out.push_str(&windows_arg(address));
    out
}

/// Where the shortcut goes in `dir`: `<stem>.<ext>`, or `<stem> (2).<ext>`… when a
/// file of that name is a shortcut to somewhere else (one to the same place is
/// written over); `None` when every name up to ` (99)` is another server's.
pub fn free_path(
    dir: &Path,
    stem: &str,
    ext: &str,
    same: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() || same(&first) {
        return Some(first);
    }
    (2..100)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|p| !p.exists() || same(p))
}

/// Write the shortcut into `dir` (Linux), for the executable `exec`.
#[cfg(not(windows))]
pub fn write_desktop_file(dir: &Path, exec: &str, s: &Shortcut) -> Result<PathBuf, Failure> {
    let io = |p: &Path, e: std::io::Error| Failure::Io(format!("{}: {e}", p.display()));
    if !dir.is_dir() {
        return Err(Failure::NoDesktop);
    }
    let entry = desktop_entry(exec, s);
    let connect = format!("--connect {}", super::register::exec_arg(s.address));
    let path = free_path(dir, &stem_of(s), "desktop", |p| {
        std::fs::read_to_string(p).is_ok_and(|t| t.contains(&connect))
    })
    .ok_or(Failure::Taken)?;
    std::fs::write(&path, entry).map_err(|e| io(&path, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| io(&path, e))?;
    }
    Ok(path)
}

/// Write a shortcut on the user's desktop that joins `s.address`, named after the
/// server. Answers the file written. Blocking.
pub fn create(s: &Shortcut) -> Result<PathBuf, Failure> {
    let desktop = dirs::desktop_dir()
        .filter(|d| d.is_dir())
        .ok_or(Failure::NoDesktop)?;
    let exe = std::env::current_exe().map_err(|e| Failure::Io(format!("current_exe: {e}")))?;
    #[cfg(not(windows))]
    {
        let exec = std::env::var_os("APPIMAGE")
            .filter(|a| !a.is_empty())
            .map_or(exe, PathBuf::from);
        write_desktop_file(&desktop, &exec.to_string_lossy(), s)
    }
    #[cfg(windows)]
    {
        windows::write_lnk(&desktop, &exe, s)
    }
}

/// mslnk reads the target as `X:\…` text (and gives up the hard way on anything
/// else): only such a path is handed to it. `\\?\X:\…` is taken without its prefix.
#[cfg_attr(not(windows), allow(dead_code))]
fn plain_drive_path(exe: &str) -> Option<String> {
    let s = exe.strip_prefix(r"\\?\").unwrap_or(exe);
    let b = s.as_bytes();
    let ok = b.len() > 3
        && b.first().is_some_and(u8::is_ascii_alphabetic)
        && b.get(1) == Some(&b':')
        && b.get(2) == Some(&b'\\')
        && s.get(3..).is_some_and(|rest| !rest.starts_with('\\'))
        && Path::new(s).parent().is_some();
    ok.then(|| s.to_string())
}

#[cfg(windows)]
mod windows {
    use std::path::{Path, PathBuf};

    use super::{Failure, Shortcut};

    pub fn write_lnk(desktop: &Path, exe: &Path, s: &Shortcut) -> Result<PathBuf, Failure> {
        let target = exe
            .to_str()
            .and_then(super::plain_drive_path)
            .ok_or(Failure::NotOnDrive)?;
        let mut link =
            mslnk::ShellLink::new(&target).map_err(|e| Failure::Io(format!("{target}: {e:?}")))?;
        let args = super::windows_arguments(s.args, s.address);
        link.set_arguments(Some(args));
        link.set_icon_location(Some(target));
        let comment: String = s.comment.chars().filter(|c| !c.is_control()).collect();
        link.set_name(Some(comment));
        // An existing shortcut to the same server is written over: its bytes hold
        // the arguments as UTF-16.
        let connect = format!("--connect {}", super::windows_arg(s.address));
        let needle: Vec<u8> = connect.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let path = super::free_path(desktop, &super::stem_of(s), "lnk", |p| {
            std::fs::read(p).is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle))
        })
        .ok_or(Failure::Taken)?;
        link.create_lnk(&path)
            .map_err(|e| Failure::Io(format!("{}: {e:?}", path.display())))?;
        Ok(path)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn sc<'a>(name: &'a str, address: &'a str, args: &'a [String]) -> Shortcut<'a> {
        Shortcut {
            name,
            fallback: address,
            comment: "c",
            address,
            args,
        }
    }

    #[test]
    fn names_become_safe_file_names() {
        let stem = |s: &str| file_stem(s).unwrap();
        assert_eq!(stem("Namalsk PvE | 1PP"), "Namalsk PvE 1PP");
        assert_eq!(stem("a/b\\c:d*e?f\"g<h>i"), "a b c d e f g h i");
        assert_eq!(stem("  ..hidden.  "), "hidden");
        assert_eq!(stem("tab\there\nnew"), "tab here new");
        assert_eq!(file_stem(""), None);
        assert_eq!(file_stem(" ... "), None);
        assert_eq!(file_stem("|||"), None);
        assert_eq!(stem("CON"), "CON_");
        assert_eq!(stem("com1.txt"), "com1.txt_");
        assert_eq!(stem("COM10"), "COM10");
        assert_eq!(stem("Console"), "Console");
        assert_eq!(stem("Сервер ДейЗ"), "Сервер ДейЗ");
        assert_eq!(stem(&"é".repeat(200)), "é".repeat(100));
    }

    #[test]
    fn long_names_fit_in_bytes_and_are_cut_between_characters() {
        // Styled letters are 4 bytes each: 80 of them would be 320 bytes.
        let styled = "𝐒𝐞𝐫𝐯𝐞𝐫".repeat(20);
        let s = file_stem(&styled).unwrap();
        assert!(s.len() <= MAX_STEM_BYTES, "{} bytes", s.len());
        assert_eq!(s.len(), MAX_STEM_BYTES, "50 whole characters");
        assert!(styled.starts_with(&s));
        // Mixed widths never split one.
        let mixed = "a𝐒é".repeat(60);
        let s = file_stem(&mixed).unwrap();
        assert!(s.len() <= MAX_STEM_BYTES && mixed.starts_with(&s));
    }

    #[test]
    fn a_name_without_letters_takes_the_address() {
        assert_eq!(stem_of(&sc("|||", "1.2.3.4:2302", &[])), "1.2.3.4 2302");
        assert_eq!(stem_of(&sc("Farm", "1.2.3.4:2302", &[])), "Farm");
        let none = Shortcut {
            fallback: "",
            ..sc("", "", &[])
        };
        assert_eq!(stem_of(&none), LAST_STEM);
    }

    #[test]
    fn the_desktop_file_joins_the_server() {
        let e = desktop_entry(
            "/home/me/Apps/DayZ Hub.AppImage",
            &Shortcut {
                name: "Namalsk \\ PvE\nnew",
                fallback: "",
                comment: "Join 1.2.3.4:2302",
                address: "1.2.3.4:2302",
                args: &[],
            },
        );
        assert!(e.starts_with("[Desktop Entry]\n"));
        assert!(e.contains("\nType=Application\n"));
        assert!(e.contains("\nName=Namalsk \\\\ PvE\\nnew\n"));
        assert!(e.contains("\nComment=Join 1.2.3.4:2302\n"));
        assert!(
            e.contains("\nExec=\"/home/me/Apps/DayZ Hub.AppImage\" --connect \"1.2.3.4:2302\"\n")
        );
        assert!(e.contains("\nIcon=dayz-community-hub\n"));
        assert!(e.contains("\nTerminal=false\n"));
        assert_eq!(e.lines().count(), 10, "nothing spills on a line of its own");
    }

    #[test]
    fn the_windows_own_flags_come_before_connect() {
        let args = vec![
            "--data-dir".to_string(),
            "/home/me/dz test".to_string(),
            "--instance".to_string(),
            "two".to_string(),
        ];
        let e = desktop_entry("/opt/hub", &sc("A", "1.2.3.4:2302", &args));
        assert!(e.contains(
            "\nExec=\"/opt/hub\" \"--data-dir\" \"/home/me/dz test\" \"--instance\" \"two\" --connect \"1.2.3.4:2302\"\n"
        ));
        assert_eq!(
            windows_arguments(&[], "1.2.3.4:2302"),
            "--connect \"1.2.3.4:2302\""
        );
        assert_eq!(windows_arguments(&[], "a\"b"), "--connect \"ab\"");
        let win = vec!["--data-dir".to_string(), r"C:\".to_string()];
        assert_eq!(
            windows_arguments(&win, "1.2.3.4:2302"),
            r#""--data-dir" "C:\\" --connect "1.2.3.4:2302""#
        );
    }

    #[test]
    fn only_a_lettered_drive_path_goes_to_mslnk() {
        assert_eq!(
            plain_drive_path(r"C:\Games\hub.exe").as_deref(),
            Some(r"C:\Games\hub.exe")
        );
        assert_eq!(
            plain_drive_path(r"\\?\D:\hub.exe").as_deref(),
            Some(r"D:\hub.exe")
        );
        assert_eq!(plain_drive_path(r"\\server\share\hub.exe"), None);
        assert_eq!(plain_drive_path(r"\\?\UNC\server\share\hub.exe"), None);
        assert_eq!(plain_drive_path(r"C:hub.exe"), None);
        assert_eq!(plain_drive_path(r"C:\"), None);
        assert_eq!(plain_drive_path(r"C:\\x"), None);
        assert_eq!(plain_drive_path("/usr/bin/hub"), None);
    }

    #[cfg(not(windows))]
    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "dzch-shortcut-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[cfg(not(windows))]
    #[test]
    fn the_shortcut_is_written_executable_and_never_over_another_servers() {
        let dir = temp_dir("write");
        let a =
            write_desktop_file(&dir, "/opt/hub", &sc("My: Server", "1.2.3.4:2302", &[])).unwrap();
        assert_eq!(a, dir.join("My Server.desktop"));
        let text = std::fs::read_to_string(&a).unwrap();
        assert!(text.contains("--connect \"1.2.3.4:2302\""));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&a).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
        // The same server again: written over.
        let again = write_desktop_file(&dir, "/opt/hub", &sc("My: Server", "1.2.3.4:2302", &[]));
        assert_eq!(again.unwrap(), a);
        // Another server of the same name: beside it.
        let b =
            write_desktop_file(&dir, "/opt/hub", &sc("My Server", "5.6.7.8:2402", &[])).unwrap();
        assert_eq!(b, dir.join("My Server (2).desktop"));
        assert!(
            std::fs::read_to_string(&a)
                .unwrap()
                .contains("1.2.3.4:2302")
        );
        // A 4-byte styled name is written, not refused for its length.
        let styled = "𝐒𝐞𝐫𝐯𝐞𝐫".repeat(20);
        assert!(write_desktop_file(&dir, "/opt/hub", &sc(&styled, "9.9.9.9:1", &[])).is_ok());
        // No folder: an error, nothing written elsewhere.
        let missing = dir.join("missing");
        assert_eq!(
            write_desktop_file(&missing, "/opt/hub", &sc("x", "1.2.3.4:2302", &[])),
            Err(Failure::NoDesktop)
        );
        assert!(!missing.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn every_name_taken_by_other_servers_is_an_error_not_an_overwrite() {
        let dir = temp_dir("taken");
        std::fs::write(dir.join("S.desktop"), "other").unwrap();
        for n in 2..100 {
            std::fs::write(dir.join(format!("S ({n}).desktop")), "other").unwrap();
        }
        assert_eq!(free_path(&dir, "S", "desktop", |_| false), None);
        assert_eq!(
            write_desktop_file(&dir, "/opt/hub", &sc("S", "1.2.3.4:2302", &[])),
            Err(Failure::Taken)
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("S.desktop")).unwrap(),
            "other"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
