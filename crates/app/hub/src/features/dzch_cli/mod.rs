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
