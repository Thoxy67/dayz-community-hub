use dayz_community_hub_core::dzch::DzchConfig;

use crate::cli::{CLI_ARGS, CliArgs};
use crate::utils::error::ResultExt;

/// Return the CLI args that were passed when this instance started.
#[tauri::command]
pub(crate) fn get_cli_args() -> CliArgs {
    CLI_ARGS.get().cloned().unwrap_or(CliArgs {
        connect: None,
        reconnect: false,
        open: None,
    })
}

/// Read a `.dzch` server-connection file and return its contents.
#[tauri::command]
pub(crate) fn read_dzch_file(path: String) -> Result<DzchConfig, String> {
    DzchConfig::read_file(std::path::Path::new(&path)).cmd_err()
}

/// Write a `.dzch` server-connection file to disk.
#[tauri::command]
pub(crate) fn write_dzch_file(path: String, config: DzchConfig) -> Result<(), String> {
    config.write_file(std::path::Path::new(&path)).cmd_err()
}

/// Parse a `dzch://` URL into a DzchConfig.
#[tauri::command]
pub(crate) fn parse_dzch_url(url: String) -> Result<DzchConfig, String> {
    DzchConfig::from_url(&url).cmd_err()
}
