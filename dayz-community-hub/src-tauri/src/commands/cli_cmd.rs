use dayz_community_hub_core::dzch::DzchConfig;

use crate::cli::{CLI_ARGS, CliArgs};

/// Return the CLI args that were passed when this instance started.
#[tauri::command]
pub(crate) fn get_cli_args() -> CliArgs {
    CLI_ARGS.get().cloned().unwrap_or_else(|| CliArgs {
        connect: None,
        reconnect: false,
        open: None,
    })
}

/// Read a `.dzch` server-connection file and return its contents.
#[tauri::command]
pub(crate) fn read_dzch_file(path: String) -> Result<DzchConfig, String> {
    DzchConfig::read_file(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

/// Write a `.dzch` server-connection file to disk.
#[tauri::command]
pub(crate) fn write_dzch_file(path: String, config: DzchConfig) -> Result<(), String> {
    config
        .write_file(std::path::Path::new(&path))
        .map_err(|e| e.to_string())
}

/// Parse a `dzch://` URL into a DzchConfig.
#[tauri::command]
pub(crate) fn parse_dzch_url(url: String) -> Result<DzchConfig, String> {
    DzchConfig::from_url(&url).map_err(|e| e.to_string())
}
