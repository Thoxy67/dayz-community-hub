//! How the app is asked to connect somewhere: command-line arguments,
//! `.dzch` files and `dzch://` links.

use crate::Hub;
use clap::Parser;
use dz_game::DzchConfig;
use serde::Serialize;

use crate::error::{ResultExt, spawn_blocking_mapped};

/// DayZ Community Hub launcher.
#[derive(Parser, Debug, Clone, Serialize, serde::Deserialize)]
#[command(name = "dayz-community-hub", about = "DayZ Community Hub")]
pub struct CliArgs {
    /// Open the Direct Connect tab and pre-fill this IP address.
    /// Accepts "ip" or "ip:port" (port defaults to 2302).
    #[arg(long, value_name = "IP[:PORT]")]
    pub connect: Option<String>,

    /// Immediately reconnect to the last server from history.
    #[arg(long)]
    pub reconnect: bool,

    /// A `.dzch` file path or `dzch://` URL to open.
    /// When provided, the app switches to Direct Connect and auto-connects
    /// to the server described in the file / URL (downloading missing mods
    /// if the user agrees).
    #[arg(value_name = "FILE_OR_URL")]
    pub open: Option<String>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self::none()
    }
}

impl CliArgs {
    /// Arguments that ask for nothing.
    pub fn none() -> Self {
        Self {
            connect: None,
            reconnect: false,
            open: None,
        }
    }

    /// What a second launch asked for, from its `argv` and the directory it
    /// ran in (what the single-instance callback did): arguments that do not
    /// parse ask for nothing, and a file named relative to `cwd` is made
    /// absolute. A `dzch://` link is kept: nothing else forwards it now.
    pub fn from_second_launch<I, T>(argv: I, cwd: &std::path::Path) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let mut args = CliArgs::try_parse_from(argv).unwrap_or_else(|_| CliArgs::none());
        if let Some(open) = &args.open
            && !open.starts_with("dzch://")
            && std::path::Path::new(open).is_relative()
        {
            args.open = Some(cwd.join(open).to_string_lossy().into());
        }
        args
    }
}

/// The scheme of the links that open the launcher (`dzch://…`), the
/// `plugins.deep-link.desktop.schemes` of `tauri.conf.json`.
pub const URL_SCHEME: &str = "dzch";

/// `productName` in `tauri.conf.json`: the handler's name on Linux.
#[cfg(target_os = "linux")]
const PRODUCT_NAME: &str = "DayZ_Community_Hub";

/// Make `dzch://` links open this executable, for the current user, as
/// tauri-plugin-deep-link's `register_all` did at every start (the AppImage
/// and the portable zip have no installer, and a moved executable needs it
/// pointed at its new place).
///
/// - Linux: `<data dir>/applications/<exe name>-handler.desktop` (Exec is
///   `$APPIMAGE` when set, else this executable), then
///   `update-desktop-database` and `xdg-mime default … x-scheme-handler/dzch`.
/// - Windows: `HKCU\Software\Classes\dzch`, written only when it differs.
pub fn register_url_scheme() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        register_url_scheme_linux()
    }
    #[cfg(windows)]
    {
        register_url_scheme_windows()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Err("Registering dzch:// is not supported on this system".into())
    }
}

/// The handler's `.desktop` file, as the plugin's template wrote it.
#[cfg(any(target_os = "linux", test))]
fn handler_desktop_entry(name: &str, exec: &str, mime_types: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nTerminal=false\nMimeType={mime_types}\nNoDisplay=true\n"
    )
}

/// The `MimeType=` value a handler file should have: `mime` first, then the
/// other types the file already listed (the plugin kept them).
#[cfg(any(target_os = "linux", test))]
fn merged_mime_types(existing: Option<&str>, mime: &str) -> (String, bool) {
    match existing {
        Some(old) if old.split(';').any(|m| m == mime) => (old.to_owned(), false),
        Some(old) => (format!("{mime};{old}"), true),
        None => (format!("{mime};"), true),
    }
}

#[cfg(target_os = "linux")]
fn register_url_scheme_linux() -> Result<(), String> {
    let bin = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let bin_name = bin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dayz-community-hub".into());
    let file_name = format!("{bin_name}-handler.desktop");
    let exec = std::env::var_os("APPIMAGE")
        .unwrap_or_else(|| bin.into_os_string())
        .to_string_lossy()
        .into_owned();
    let exec = format!("\"{exec}\" %u");
    let target = dirs::data_dir()
        .ok_or("No data directory")?
        .join("applications");
    std::fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
    let target_file = target.join(&file_name);
    let mime = format!("x-scheme-handler/{URL_SCHEME}");

    let existing = std::fs::read_to_string(&target_file).ok();
    let field = |key: &str| {
        existing.as_deref().and_then(|text| {
            text.lines()
                .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
                .map(str::trim)
        })
    };
    let (mimes, mime_changed) = merged_mime_types(field("MimeType"), &mime);
    let exec_changed = field("Exec") != Some(exec.as_str());
    if existing.is_none() || mime_changed || exec_changed {
        let name = field("Name").unwrap_or(PRODUCT_NAME).to_owned();
        std::fs::write(&target_file, handler_desktop_entry(&name, &exec, &mimes))
            .map_err(|e| format!("{}: {e}", target_file.display()))?;
    }

    let run = |program: &str, args: &[&std::ffi::OsStr]| {
        std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("{program}: {e}"))
    };
    run("update-desktop-database", &[target.as_os_str()])?;
    run(
        "xdg-mime",
        &[
            "default".as_ref(),
            file_name.as_ref(),
            std::ffi::OsStr::new(&mime),
        ],
    )?;
    Ok(())
}

#[cfg(windows)]
fn register_url_scheme_windows() -> Result<(), String> {
    use dz_common::win::{HKEY_CURRENT_USER, reg_set_string, reg_string};
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe = exe.to_string_lossy();
    let exe = exe.strip_prefix(r"\\?\").unwrap_or(&exe);
    let base = format!("Software\\Classes\\{URL_SCHEME}");
    let command = format!("\"{exe}\" \"%1\"");
    let command_key = format!("{base}\\shell\\open\\command");
    if reg_string(HKEY_CURRENT_USER, &command_key, "")
        .ok()
        .flatten()
        .as_deref()
        == Some(command.as_str())
        && reg_string(HKEY_CURRENT_USER, &base, "URL Protocol")
            .ok()
            .flatten()
            .is_some()
    {
        return Ok(());
    }
    let writes: [(String, Option<&str>, String); 4] = [
        (
            base.clone(),
            None,
            format!("URL:{} protocol", crate::paths::TAURI_IDENTIFIER),
        ),
        (base.clone(), Some("URL Protocol"), String::new()),
        (format!("{base}\\DefaultIcon"), None, format!("{exe},0")),
        (command_key, None, command),
    ];
    for (key, value, data) in writes {
        reg_set_string(HKEY_CURRENT_USER, &key, value, &data)
            .map_err(|e| format!("registering dzch://: {e}"))?;
    }
    Ok(())
}

/// Windows: make `.dzch` files open in this executable, for the current user.
/// Installers do this; the portable zip has none, and a moved folder needs it
/// again, so it is checked at every start and written only when it differs.
#[cfg(windows)]
pub fn register_file_type() {
    use dz_common::win::{HKEY_CURRENT_USER, associations_changed, reg_set_string, reg_string};
    const PROG_ID: &str = "DayZCommunityHub.dzch";
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let exe = exe.to_string_lossy();
    let command = format!("\"{exe}\" \"%1\"");
    let classes = "Software\\Classes";
    let open_key = format!("{classes}\\{PROG_ID}\\shell\\open\\command");
    let current = reg_string(HKEY_CURRENT_USER, &open_key, "").ok().flatten();
    let points_here = reg_string(HKEY_CURRENT_USER, &format!("{classes}\\.dzch"), "")
        .ok()
        .flatten()
        .is_some_and(|p| p == PROG_ID);
    if points_here && current.as_deref() == Some(command.as_str()) {
        return;
    }
    let writes = [
        (format!("{classes}\\.dzch"), PROG_ID.to_string()),
        (
            format!("{classes}\\{PROG_ID}"),
            "DayZ Community Hub server".to_string(),
        ),
        (
            format!("{classes}\\{PROG_ID}\\DefaultIcon"),
            format!("\"{exe}\",0"),
        ),
        (open_key, command),
    ];
    for (key, data) in writes {
        if let Err(e) = reg_set_string(HKEY_CURRENT_USER, &key, None, &data) {
            eprintln!("registering .dzch: {e}");
            return;
        }
    }
    associations_changed();
}

impl Hub {
    /// Return the CLI args that were passed when this instance started, once:
    /// a window that reloads gets none, or `--connect` / `--reconnect` would
    /// join the server again on every reload.
    pub async fn get_cli_args(&self) -> CliArgs {
        self.inner
            .args
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .unwrap_or_else(CliArgs::none)
    }

    /// Read a `.dzch` server-connection file and return its contents.
    pub async fn read_dzch_file(&self, path: String) -> Result<DzchConfig, String> {
        spawn_blocking_mapped(move || DzchConfig::read_file(std::path::Path::new(&path))).await
    }

    /// Write a `.dzch` server-connection file to disk.
    pub async fn write_dzch_file(&self, path: String, config: DzchConfig) -> Result<(), String> {
        mocked!(self => Ok(()));
        spawn_blocking_mapped(move || config.write_file(std::path::Path::new(&path))).await
    }

    /// Parse a `dzch://` URL into a DzchConfig.
    pub async fn parse_dzch_url(&self, url: String) -> Result<DzchConfig, String> {
        DzchConfig::from_url(&url).cmd_err()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;

    #[test]
    fn a_second_launch_names_files_from_where_it_ran() {
        let cwd = std::path::Path::new("/home/me");
        let a = CliArgs::from_second_launch(["hub", "server.dzch"], cwd);
        assert_eq!(
            a.open.as_deref().map(std::path::PathBuf::from),
            Some(cwd.join("server.dzch"))
        );
        let a = CliArgs::from_second_launch(["hub", "dzch://1.2.3.4:2302"], cwd);
        assert_eq!(a.open.as_deref(), Some("dzch://1.2.3.4:2302"));
        let a = CliArgs::from_second_launch(["hub", "--reconnect"], cwd);
        assert!(a.reconnect && a.open.is_none());
        let a = CliArgs::from_second_launch(["hub", "--nope"], cwd);
        assert!(!a.reconnect && a.open.is_none() && a.connect.is_none());
    }

    #[test]
    fn the_handler_file_keeps_other_types_and_adds_ours_first() {
        let mime = "x-scheme-handler/dzch";
        assert_eq!(merged_mime_types(None, mime), (format!("{mime};"), true));
        assert_eq!(
            merged_mime_types(Some("text/x-foo;"), mime),
            (format!("{mime};text/x-foo;"), true)
        );
        assert_eq!(
            merged_mime_types(Some("x-scheme-handler/dzch;"), mime),
            ("x-scheme-handler/dzch;".to_owned(), false)
        );
        let entry = handler_desktop_entry("DayZ_Community_Hub", "\"/a b/hub\" %u", "m;");
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("\nExec=\"/a b/hub\" %u\n"));
        assert!(entry.contains("\nNoDisplay=true\n"));
    }

    #[tokio::test]
    async fn the_start_up_arguments_are_handed_once() {
        let args = CliArgs {
            reconnect: true,
            ..CliArgs::none()
        };
        let hub = crate::Hub::new(
            crate::HubConfig {
                args,
                ..Default::default()
            },
            std::sync::Arc::new(|_| {}),
        )
        .unwrap();
        assert!(hub.get_cli_args().await.reconnect);
        assert!(!hub.get_cli_args().await.reconnect);
        // Commands that need the state say so before `initialize`.
        assert!(hub.get_app_stats().await.is_err());
    }
}
