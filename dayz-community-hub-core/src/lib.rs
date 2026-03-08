//! Core library for interacting with DayZ servers and mods.
//!
//! This crate provides:
//! - Server list fetching from dayzsalauncher.com API
//! - A2S protocol queries to live servers
//! - SteamCMD mod downloading with progress streaming
//! - Mod installation and symlink management
//! - Game launch argument assembly
//! - User profile persistence (favorites, history, launch options)
//! - DayZ news fetching
//! - Offline mode (DayZCommunityOfflineMode) management
//! - System sanity checks

pub mod a2s_query;
pub mod api;
pub mod config;
pub mod ctl;
pub mod dzch;
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
pub use dzch::DzchConfig;
pub use errors::Error;

pub type Result<T> = std::result::Result<T, Error>;
