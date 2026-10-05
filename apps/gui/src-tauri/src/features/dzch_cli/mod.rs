//! How the app is asked to connect somewhere: command-line arguments,
//! `.dzch` files and `dzch://` links.

use clap::Parser;
use dz_game::DzchConfig;
use serde::Serialize;
use std::sync::OnceLock;

use crate::error::{ResultExt, spawn_blocking_mapped};

/// DayZ Community Hub launcher.
#[derive(
    Parser, Debug, Clone, Serialize, serde::Deserialize, specta::Type, tauri_specta::Event,
)]
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

/// Parsed CLI args stored at startup; re-used by the `get_cli_args` command.
static CLI_ARGS: OnceLock<CliArgs> = OnceLock::new();

impl CliArgs {
    /// Keep the arguments this instance started with, for `get_cli_args`.
    pub(crate) fn remember(self) {
        let _ = CLI_ARGS.set(self);
    }

    /// Arguments that ask for nothing.
    pub(crate) fn none() -> Self {
        Self {
            connect: None,
            reconnect: false,
            open: None,
        }
    }
}

/// Return the CLI args that were passed when this instance started.
#[tauri::command]
#[specta::specta]
pub(crate) fn get_cli_args() -> CliArgs {
    CLI_ARGS.get().cloned().unwrap_or_else(CliArgs::none)
}

/// Read a `.dzch` server-connection file and return its contents.
#[tauri::command]
#[specta::specta]
pub(crate) async fn read_dzch_file(path: String) -> Result<DzchConfig, String> {
    spawn_blocking_mapped(move || DzchConfig::read_file(std::path::Path::new(&path))).await
}

/// Write a `.dzch` server-connection file to disk.
#[tauri::command]
#[specta::specta]
pub(crate) async fn write_dzch_file(path: String, config: DzchConfig) -> Result<(), String> {
    spawn_blocking_mapped(move || config.write_file(std::path::Path::new(&path))).await
}

/// Parse a `dzch://` URL into a DzchConfig.
#[tauri::command]
#[specta::specta]
pub(crate) fn parse_dzch_url(url: String) -> Result<DzchConfig, String> {
    DzchConfig::from_url(&url).cmd_err()
}

/// Windows: make `.dzch` files open in this executable, for the current user.
/// Installers do this; the portable zip has none, and a moved folder needs it
/// again, so it is checked at every start and written only when it differs.
#[cfg(windows)]
pub(crate) fn register_file_type() {
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
