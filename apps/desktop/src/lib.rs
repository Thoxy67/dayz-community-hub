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
pub mod rt;
pub mod theme;

pub use app::HubApp;
