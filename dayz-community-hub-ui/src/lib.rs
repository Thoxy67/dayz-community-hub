//! TUI layer for the DayZ-SA Multi Launcher.
//!
//! Exposes the `App` state struct, the main `draw_ui` function,
//! event handling (`handle_key`), and the first-run setup wizard.

pub mod app;
pub mod events;
pub mod tabs;
pub mod widgets;
pub mod wizard;

pub use app::App;
pub use events::handle_key;
pub use wizard::run_setup_if_needed;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use app::Tab;
use tabs::{
    direct_connect::render_direct_connect_tab,
    favorites::render_favorites_tab,
    history::render_history_tab,
    mods::render_mods_tab,
    news::render_news_tab,
    offline::render_offline_tab,
    options::render_options_tab,
    servers::render_servers_tab,
};
use widgets::{
    draw_popup, draw_progress_overlay, draw_status_bar, draw_tab_bar, draw_title_bar,
};

/// Render the entire TUI for a single frame.
pub fn draw_ui(f: &mut Frame, app: &App) {
    let size = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Length(3), // Tab bar
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    draw_title_bar(f, app, chunks[0]);
    draw_tab_bar(f, app, chunks[1]);

    match app.tab {
        Tab::Servers => render_servers_tab(f, app, chunks[2]),
        Tab::Favorites => render_favorites_tab(f, app, chunks[2]),
        Tab::History => render_history_tab(f, app, chunks[2]),
        Tab::Mods => render_mods_tab(f, app, chunks[2]),
        Tab::News => render_news_tab(f, app, chunks[2]),
        Tab::DirectConnect => render_direct_connect_tab(f, app, chunks[2]),
        Tab::Options => render_options_tab(f, app, chunks[2]),
        Tab::Offline => render_offline_tab(f, app, chunks[2]),
    }

    draw_status_bar(f, app, chunks[3]);

    if let Some(ref progress) = app.progress_state {
        draw_progress_overlay(f, progress, size);
    }

    if let Some(ref popup) = app.popup {
        draw_popup(f, popup, size);
    }
}
