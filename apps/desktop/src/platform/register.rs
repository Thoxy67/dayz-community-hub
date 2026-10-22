//! `dzch://` links and `.dzch` files open this executable, for the current user: what
//! tauri-plugin-deep-link's `register_all`, `dzch_cli::register_file_type` and the
//! bundle's `fileAssociations` (`application/x-dzch`, `*.dzch`) did. The AppImage and
//! the Windows zip have no installer, and a moved executable needs it pointed at its
//! new place, so it is checked at every start, on a thread of its own, and written only
//! when it differs. Never for `--mock`, never from a debug build (a `cargo run` must
//! not take the links away from the installed launcher).
//!
//! - Linux: `<data>/applications/<exe name>-handler.desktop` (Exec is `$APPIMAGE` when
//!   set, else this executable), `<data>/mime/packages/dayz-community-hub.xml`; when
//!   either changed, `update-mime-database`, `update-desktop-database` and
//!   `xdg-mime default <file> x-scheme-handler/dzch application/x-dzch`.
//! - Windows: `HKCU\Software\Classes\dzch` (URL protocol), `.dzch` →
//!   `DayZCommunityHub.dzch` with its open command and icon; then the shell is told.

/// The scheme of the links (`plugins.deep-link.desktop.schemes` in `tauri.conf.json`).
pub const SCHEME: &str = dz_hub::features::dzch_cli::URL_SCHEME;
/// The `.dzch` files' type (`bundle.fileAssociations`).
pub const FILE_MIME: &str = "application/x-dzch";
/// What the system calls a `.dzch` file.
pub const FILE_DESCRIPTION: &str = "DayZ Community Hub server config";

/// Register in the background when this launch should (see the module docs).
pub fn spawn(mock: bool) {
    if mock || cfg!(debug_assertions) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("register-links".into())
        .spawn(|| {
            if let Err(e) = register() {
                eprintln!("registering dzch:// and .dzch: {e}");
            }
        });
}

/// Point `dzch://` and `.dzch` at this executable now (idempotent).
pub fn register() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        linux::register()
    }
    #[cfg(windows)]
    {
        windows::register()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Err("not supported on this system".into())
    }
}

/// One argument of a `.desktop` `Exec=` line: quoted, with the quoting rule's escapes
/// (`"` `` ` `` `$` `\`), then the string rule's (every `\` doubled) and `%` as `%%`.
pub fn exec_arg(s: &str) -> String {
    let mut quoted = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(c);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            '%' => quoted.push_str("%%"),
            '\n' | '\r' => {}
            c => quoted.push(c),
        }
    }
    format!("\"{quoted}\"")
}

/// The handler's `.desktop` file for an executable at `exec`.
pub fn desktop_entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=DayZ Community Hub\n\
         Comment=DayZ SA Multi Launcher and Server Browser\n\
         Exec={} %u\n\
         Icon=dayz-community-hub\n\
         Terminal=false\n\
         NoDisplay=true\n\
         StartupWMClass=dayz-community-hub\n\
         MimeType=x-scheme-handler/{SCHEME};{FILE_MIME};\n",
        exec_arg(exec)
    )
}

/// The shared-mime-info package that names `*.dzch` files.
pub fn mime_package() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n  \
         <mime-type type=\"{FILE_MIME}\">\n    \
         <comment>{FILE_DESCRIPTION}</comment>\n    \
         <glob pattern=\"*.dzch\"/>\n  \
         </mime-type>\n\
         </mime-info>\n"
    )
}

/// The handler file's name for an executable called `bin_name` (the deep-link
/// plugin's, so the Tauri launcher's file is replaced in place under the same name).
pub fn handler_file_name(bin_name: &str) -> String {
    format!("{bin_name}-handler.desktop")
}

/// HKCU writes (key under `HKCU`, value name or default, data) for an executable at
/// `exe`, in order.
pub fn windows_writes(exe: &str) -> Vec<(String, Option<&'static str>, String)> {
    const PROG_ID: &str = "DayZCommunityHub.dzch";
    let exe = exe.strip_prefix(r"\\?\").unwrap_or(exe);
    let open = format!("\"{exe}\" \"%1\"");
    let scheme = format!("Software\\Classes\\{SCHEME}");
    let prog = format!("Software\\Classes\\{PROG_ID}");
    vec![
        (
            scheme.clone(),
            None,
            format!("URL:{} protocol", dz_hub::paths::TAURI_IDENTIFIER),
        ),
        (scheme.clone(), Some("URL Protocol"), String::new()),
        (format!("{scheme}\\DefaultIcon"), None, format!("{exe},0")),
        (
            format!("{scheme}\\shell\\open\\command"),
            None,
            open.clone(),
        ),
        ("Software\\Classes\\.dzch".into(), None, PROG_ID.to_string()),
        (prog.clone(), None, "DayZ Community Hub server".into()),
        (format!("{prog}\\DefaultIcon"), None, format!("\"{exe}\",0")),
        (format!("{prog}\\shell\\open\\command"), None, open),
    ]
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::OsStr;
    use std::path::Path;
    use std::process::{Command, Stdio};

    pub fn register() -> Result<(), String> {
        let bin = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let bin_name = bin
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "dayz-community-hub".into());
        let exec = std::env::var_os("APPIMAGE")
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| bin.into_os_string());
        let data = dirs::data_dir().ok_or("no data directory")?;
        let apps = data.join("applications");
        let mime = data.join("mime");
        let file_name = super::handler_file_name(&bin_name);

        let mime_changed = write_if_changed(
            &mime.join("packages").join("dayz-community-hub.xml"),
            &super::mime_package(),
        )?;
        let entry_changed = write_if_changed(
            &apps.join(&file_name),
            &super::desktop_entry(&exec.to_string_lossy()),
        )?;
        // The tools may be missing (a bare window manager): the files are right anyway.
        if mime_changed {
            run("update-mime-database", &[mime.as_os_str()]);
        }
        if mime_changed || entry_changed {
            run("update-desktop-database", &[apps.as_os_str()]);
            let scheme = format!("x-scheme-handler/{}", super::SCHEME);
            run(
                "xdg-mime",
                &[
                    OsStr::new("default"),
                    OsStr::new(&file_name),
                    OsStr::new(&scheme),
                    OsStr::new(super::FILE_MIME),
                ],
            );
        }
        Ok(())
    }

    fn write_if_changed(path: &Path, content: &str) -> Result<bool, String> {
        if std::fs::read_to_string(path).is_ok_and(|old| old == content) {
            return Ok(false);
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(path, content).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(true)
    }

    fn run(program: &str, args: &[&OsStr]) {
        let status = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if let Err(e) = status {
            eprintln!("{program}: {e}");
        }
    }
}

#[cfg(windows)]
mod windows {
    use dz_common::win::{HKEY_CURRENT_USER, associations_changed, reg_set_string, reg_string};

    pub fn register() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let writes = super::windows_writes(&exe.to_string_lossy());
        let current = writes.iter().all(|(key, value, data)| {
            reg_string(HKEY_CURRENT_USER, key, value.unwrap_or(""))
                .ok()
                .flatten()
                .is_some_and(|d| d == *data)
        });
        if current {
            return Ok(());
        }
        for (key, value, data) in &writes {
            reg_set_string(HKEY_CURRENT_USER, key, *value, data)
                .map_err(|e| format!("{key}: {e}"))?;
        }
        associations_changed();
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn exec_lines_quote_what_the_spec_says() {
        assert_eq!(exec_arg("/opt/hub"), "\"/opt/hub\"");
        assert_eq!(exec_arg("/a b/hub"), "\"/a b/hub\"");
        // `$` → `\$` by the quoting rule, then `\` → `\\` by the string rule.
        assert_eq!(exec_arg("/a$b"), "\"/a\\\\$b\"");
        assert_eq!(exec_arg("/a\"b"), "\"/a\\\\\"b\"");
        // A literal backslash is four.
        assert_eq!(exec_arg("/a\\b"), "\"/a\\\\\\\\b\"");
        assert_eq!(exec_arg("/100%"), "\"/100%%\"");
        assert_eq!(exec_arg("/a\nb"), "\"/ab\"");
    }

    #[test]
    fn the_handler_file_claims_links_and_files() {
        let e = desktop_entry("/home/me/Apps/Hub.AppImage");
        assert!(e.starts_with("[Desktop Entry]\n"));
        assert!(e.contains("\nExec=\"/home/me/Apps/Hub.AppImage\" %u\n"));
        assert!(e.contains("\nMimeType=x-scheme-handler/dzch;application/x-dzch;\n"));
        assert!(e.contains("\nNoDisplay=true\n"));
        assert!(e.contains("\nType=Application\n"));
        assert_eq!(
            handler_file_name("dayz-community-hub"),
            "dayz-community-hub-handler.desktop"
        );
        let m = mime_package();
        assert!(m.contains("<mime-type type=\"application/x-dzch\">"));
        assert!(m.contains("<glob pattern=\"*.dzch\"/>"));
        assert!(m.contains("shared-mime-info"));
    }

    #[test]
    fn windows_classes_point_at_the_executable() {
        let w = windows_writes(r"\\?\C:\Games\Hub\hub.exe");
        let find = |k: &str, v: Option<&str>| {
            w.iter()
                .find(|(key, val, _)| key == k && *val == v)
                .map(|(_, _, d)| d.as_str())
        };
        assert_eq!(
            find(r"Software\Classes\dzch\shell\open\command", None),
            Some(r#""C:\Games\Hub\hub.exe" "%1""#)
        );
        assert_eq!(
            find(r"Software\Classes\dzch", Some("URL Protocol")),
            Some("")
        );
        assert_eq!(
            find(r"Software\Classes\dzch", None),
            Some("URL:com.thoxy.dayz-community-hub protocol")
        );
        assert_eq!(
            find(r"Software\Classes\.dzch", None),
            Some("DayZCommunityHub.dzch")
        );
        assert_eq!(
            find(
                r"Software\Classes\DayZCommunityHub.dzch\shell\open\command",
                None
            ),
            Some(r#""C:\Games\Hub\hub.exe" "%1""#)
        );
        assert_eq!(
            find(r"Software\Classes\DayZCommunityHub.dzch\DefaultIcon", None),
            Some(r#""C:\Games\Hub\hub.exe",0"#)
        );
    }
}
