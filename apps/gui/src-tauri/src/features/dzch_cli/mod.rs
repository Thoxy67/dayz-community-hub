//! How the app is asked to connect somewhere: command-line arguments,
//! `.dzch` files and `dzch://` links.

use clap::Parser;
use dz_game::DzchConfig;
use serde::Serialize;
use std::sync::OnceLock;

use crate::error::{ResultExt, spawn_blocking_mapped};

/// DayZ Community Hub launcher.
#[derive(Parser, Debug, Clone, Serialize)]
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
pub(crate) fn get_cli_args() -> CliArgs {
    CLI_ARGS.get().cloned().unwrap_or_else(CliArgs::none)
}

/// Read a `.dzch` server-connection file and return its contents.
#[tauri::command]
pub(crate) async fn read_dzch_file(path: String) -> Result<DzchConfig, String> {
    spawn_blocking_mapped(move || DzchConfig::read_file(std::path::Path::new(&path))).await
}

/// Write a `.dzch` server-connection file to disk.
#[tauri::command]
pub(crate) async fn write_dzch_file(path: String, config: DzchConfig) -> Result<(), String> {
    spawn_blocking_mapped(move || config.write_file(std::path::Path::new(&path))).await
}

/// Parse a `dzch://` URL into a DzchConfig.
#[tauri::command]
pub(crate) fn parse_dzch_url(url: String) -> Result<DzchConfig, String> {
    DzchConfig::from_url(&url).cmd_err()
}
