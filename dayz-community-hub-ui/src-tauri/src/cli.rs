use clap::Parser;
use serde::Serialize;
use std::sync::OnceLock;

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
pub(crate) static CLI_ARGS: OnceLock<CliArgs> = OnceLock::new();
