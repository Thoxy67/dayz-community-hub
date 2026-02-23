//! Servers tab renderer.

use crate::{
    app::App,
    widgets::{make_server_list_item, render_server_details, split_list_details},
};
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, List},
    Frame,
};

pub fn render_servers_tab(f: &mut Frame, app: &App, area: Rect) {
    let offset = app.offset();
    let selected = app.selected_index();
    let visible = app.visible_items();

    let (list_area, details_area) = split_list_details(area, app.show_server_details);

    let server_iter: Box<dyn Iterator<Item = (usize, &dayz_community_hub_core::Server)>> =
        if let Some(ref indices) = app.filtered_indices {
            Box::new(
                indices
                    .iter()
                    .enumerate()
                    .skip(offset)
                    .take(visible)
                    .map(|(display_i, &real_i)| (display_i, &app.servers[real_i])),
            )
        } else {
            Box::new(app.servers.iter().enumerate().skip(offset).take(visible))
        };

    let items: Vec<ratatui::widgets::ListItem> = server_iter
        .map(|(display_idx, server)| {
            let is_sel = display_idx == selected;
            let is_fav = app
                .ctl
                .profile()
                .is_favorite(&server.endpoint.ip, server.endpoint.port as u16);
            let ping = app.get_ping(server);
            make_server_list_item(display_idx, server, is_sel, is_fav, None, None, ping)
        })
        .collect();

    let total = if let Some(ref indices) = app.filtered_indices {
        indices.len()
    } else {
        app.servers.len()
    };

    let title = if app.filtered_indices.is_some() {
        format!("Servers [{}/{}] (filtered)", selected + 1, total)
    } else {
        format!("Servers [{}/{}]", selected + 1, total)
    };

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_symbol("  ");
    f.render_widget(list, list_area);

    if let Some(details_area) = details_area {
        let server = if let Some(ref indices) = app.filtered_indices {
            indices.get(selected).and_then(|&i| app.servers.get(i))
        } else {
            app.servers.get(selected)
        };
        if let Some(server) = server {
            render_server_details(f, app, server, details_area);
        }
    }
}
