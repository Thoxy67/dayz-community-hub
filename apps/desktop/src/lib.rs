//! The DayZ Community Hub window, in egui. See `plan/architecture.md` (L3) and `AGENTS.md`.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod app;
pub mod catalog;
pub mod commands;
pub mod control;
pub mod i18n;
pub mod icons;
pub mod kit;
pub mod pad;
pub mod parts;
pub mod platform;
pub mod prefs;
pub mod rt;
pub mod shell;
pub mod store;
pub mod theme;
pub mod views;

pub use app::HubApp;
