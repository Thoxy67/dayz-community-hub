//! Key event handling for the main TUI loop.

use crate::app::{App, ConfirmAction, DirectConnectField, Popup, Tab};
use termion::event::Key;

/// Process a single keypress and return `false` if the application should quit.
pub fn handle_key(app: &mut App, key: Key) -> bool {
    // ── Popup takes priority ──────────────────────────────────────────────
    if app.popup.is_some() {
        if matches!(app.popup, Some(Popup::Info { .. })) {
            app.popup = None;
            return true;
        }
        match key {
            Key::Char('y') | Key::Char('Y') => {
                if let Some(Popup::Confirm { action, .. }) = app.popup.take() {
                    app.execute_confirm_action(action);
                }
            }
            Key::Char('n') | Key::Char('N') | Key::Esc => {
                if let Some(Popup::Confirm {
                    action: ConfirmAction::UpdateThenLaunch(server, pw),
                    ..
                }) = app.popup.take()
                {
                    app.set_info("Skipping update, launching...");
                    if let Err(e) = app.ctl.setup_mod_symlinks(&server) {
                        app.set_warn(format!("Symlink warning: {}", e));
                    }
                    app.launch_server(&server, pw.as_deref());
                } else {
                    app.popup = None;
                    app.set_info("Cancelled");
                }
            }
            _ => {}
        }
        return true;
    }

    // ── Option value editing mode ─────────────────────────────────────────
    if app.option_edit_active {
        match key {
            Key::Esc => app.cancel_option_edit(),
            Key::Char('\n') => app.apply_option_edit(),
            Key::Backspace => {
                app.option_edit_value.pop();
            }
            Key::Char(c) => app.option_edit_value.push(c),
            _ => {}
        }
        return true;
    }

    // ── Search mode ───────────────────────────────────────────────────────
    if app.search_active {
        match key {
            Key::Esc => {
                app.search_active = false;
                app.search_query.clear();
                app.filtered_indices = None;
                app.set_info("Search cleared");
            }
            Key::Char('\n') => {
                app.search_active = false;
                let count = app
                    .filtered_indices
                    .as_ref()
                    .map(|i| i.len())
                    .unwrap_or(app.servers.len());
                app.set_info(format!("Found {} servers", count));
            }
            Key::Backspace => {
                app.search_query.pop();
                app.update_search_filter();
            }
            Key::Char(c) => {
                app.search_query.push(c);
                app.update_search_filter();
            }
            _ => {}
        }
        return true;
    }

    // ── Direct connect tab ────────────────────────────────────────────────
    if app.tab == Tab::DirectConnect {
        match key {
            Key::Esc => {
                app.tab = Tab::Servers;
                app.status_message = None;
            }
            Key::Up => app.move_selection(-1),
            Key::Down => app.move_selection(1),
            Key::Char('\t') | Key::Ctrl('n') => {
                app.tab = app.tab.next();
                app.status_message = None;
            }
            Key::BackTab | Key::Ctrl('p') => {
                app.tab = app.tab.prev();
                app.status_message = None;
            }
            Key::Char('\n') => {
                if app.direct_cursor == DirectConnectField::Connect {
                    app.handle_direct_connect();
                } else if app.direct_cursor == DirectConnectField::ServerInfo {
                    app.fetch_direct_server_info();
                } else {
                    app.move_selection(1);
                }
            }
            Key::Backspace => app.handle_direct_backspace(),
            Key::Char(c) => app.handle_direct_input(c),
            _ => {}
        }
        return true;
    }

    // ── Normal mode ───────────────────────────────────────────────────────
    match key {
        Key::Char('q') | Key::Ctrl('c') => return false,

        Key::Esc => {
            if app.show_server_details {
                app.show_server_details = false;
                app.current_a2s_details = None;
                app.detailed_server_index = None;
                app.details_scroll_offset = 0;
            } else {
                app.status_message = None;
            }
        }

        // Details panel scrolling
        Key::Ctrl('d') if app.show_server_details => {
            app.details_scroll_offset = app.details_scroll_offset.saturating_add(5);
        }
        Key::Ctrl('u') if app.show_server_details => {
            app.details_scroll_offset = app.details_scroll_offset.saturating_sub(5);
        }

        // News detail panel scrolling
        Key::Ctrl('d') if app.tab == Tab::News => {
            app.news_detail_scroll = app.news_detail_scroll.saturating_add(3);
        }
        Key::Ctrl('u') if app.tab == Tab::News => {
            app.news_detail_scroll = app.news_detail_scroll.saturating_sub(3);
        }

        Key::Char('j') | Key::Down => {
            app.news_detail_scroll = 0;
            app.move_selection(1);
        }
        Key::Char('k') | Key::Up => {
            app.news_detail_scroll = 0;
            app.move_selection(-1);
        }
        Key::Char('G') => {
            let len = app.current_list_len();
            if len > 0 {
                *app.selected_index_mut() = len - 1;
                let visible = app.visible_items();
                if len > visible {
                    *app.offset_mut() = len - visible;
                }
            }
        }
        Key::Char('g') => {
            *app.selected_index_mut() = 0;
            *app.offset_mut() = 0;
        }
        Key::PageUp => app.page_up(),
        Key::PageDown => app.page_down(),

        // Tab switching
        Key::Char('\t') | Key::Ctrl('n') => {
            app.tab = app.tab.next();
            on_tab_switch(app);
        }
        Key::BackTab | Key::Ctrl('p') => {
            app.tab = app.tab.prev();
            on_tab_switch(app);
        }

        // Actions
        Key::Char('\n') => {
            if app.tab == Tab::Options {
                app.toggle_option_at_index();
            } else if app.tab == Tab::News {
                app.open_selected_news_url();
            } else if app.tab == Tab::Offline {
                app.launch_offline_mission();
            } else {
                app.connect_to_selected();
            }
        }

        Key::Char('u') if app.tab == Tab::Offline => app.update_offline_mode(),
        Key::Char('r') if app.tab == Tab::Offline => app.refresh_offline_missions(),
        Key::Char('e') if app.tab == Tab::Options => app.start_option_edit(),

        Key::Char('f') if app.tab == Tab::Servers || app.tab == Tab::History => app.add_favorite(),
        Key::Char('x') if app.tab == Tab::Favorites => app.remove_favorite_at_index(),
        Key::Char('x') if app.tab == Tab::History => app.remove_history_entry_at_index(),
        Key::Char('c') if app.tab == Tab::History => app.clear_history(),

        Key::Char('i')
            if app.tab == Tab::Servers || app.tab == Tab::Favorites || app.tab == Tab::History =>
        {
            app.fetch_a2s_info();
        }
        Key::Char('m')
            if app.tab == Tab::Servers || app.tab == Tab::Favorites || app.tab == Tab::History =>
        {
            app.show_server_details = !app.show_server_details;
            app.details_scroll_offset = 0;
            if app.show_server_details {
                app.detailed_server_index = Some(app.selected_index());
            }
        }
        Key::Char('/') if app.tab == Tab::Servers => {
            app.search_active = true;
            app.search_query.clear();
            app.set_info("Type to search, Enter to confirm, Esc to cancel");
        }
        Key::Char('r') if app.tab == Tab::Servers => app.refresh_server_list(),

        // Mod actions
        Key::Char('r') if app.tab == Tab::Mods => app.refresh_installed_mods(),
        Key::Char('u') if app.tab == Tab::Mods => app.update_selected_mod(),
        Key::Char('U') if app.tab == Tab::Mods => app.update_all_installed_mods(),
        Key::Char('d') if app.tab == Tab::Mods => app.delete_selected_mod(),
        Key::Char('m') if app.tab == Tab::Mods => app.toggle_selected_mod_managed(),
        Key::Char('c') if app.tab == Tab::Mods => app.cleanup_mods(),

        _ => {}
    }

    true
}

/// Side-effects when switching to a new tab.
fn on_tab_switch(app: &mut App) {
    if app.tab == Tab::Mods && app.installed_mods.is_none() {
        app.refresh_installed_mods();
    }
    if app.tab == Tab::News {
        app.fetch_news_if_needed();
    }
    if app.tab == Tab::Offline && app.offline_missions.is_none() {
        app.refresh_offline_missions();
    }
    app.status_message = None;
}
