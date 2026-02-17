//! A Rust library for interacting with DayZ servers and mods.
//!
//! This library provides functionality to:
//! - Fetch server lists from dayzsalauncher.com API
//! - Download and update DayZ mods via SteamCMD
//! - Detect installed mods in the Steam Workshop directory
//! - Generate launch arguments for connecting to servers
//! - Manage user profiles with favorites, history, and launch options
//!
//! # Example
//! ```no_run
//! use a2sdayz::{ctl::DayzCtl, Result, config};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let profile_path = config::default_profile_path();
//!     let mut ctl = DayzCtl::new(&profile_path).await?;
//!     ctl.fetch_servers().await?;
//!     if let Some(server) = ctl.find_server("127.0.0.1", 2302) {
//!         ctl.install_missing_mods(server).await?;
//!         let args = ctl.build_steam_launch_args(server, None);
//!         println!("Launch with: steam {}", args.join(" "));
//!     }
//!     Ok(())
//! }
//! ```

pub mod a2s_query;
pub mod api;
pub mod config;
pub mod ctl;
pub mod errors;
pub mod launch;
pub mod mods;
pub mod news;
pub mod offline;
pub mod steamcmd;
pub mod system;
pub mod utils;

pub use a2s_query::ServerDetails;
pub use api::{Endpoint, Mod, Server, ServerList};
pub use config::Profile;
pub use errors::Error;

pub type Result<T> = std::result::Result<T, Error>;
