//! Favorites tab renderer.

use crate::{
    app::App,
    widgets::{make_server_list_item, render_server_details, split_list_details},
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render_favorites_tab(f: &mut Frame, app: &App, area: Rect) {
    let favorites = &app.ctl.profile().favorites;
    let selected = app.selected_index();

    if favorites.is_empty() {
        let text = Paragraph::new("No favorites yet.\nPress 'f' on a server to add it.")
            .block(Block::default().title("Favorites").borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(text, area);
        return;
    }

    let (list_area, details_area) = split_list_details(area, app.show_server_details);

    let items: Vec<ListItem> = favorites
        .iter()
        .enumerate()
        .map(|(i, fav)| {
            let is_sel = i == selected;
            let server = app.servers.iter().find(|s| {
                s.endpoint.ip == fav.ip
                    && (s.endpoint.port as u16 == fav.port || s.game_port as u16 == fav.port)
            });

            match server {
                Some(s) => {
                    let prefix = Span::styled(" ON  ", Style::default().fg(Color::Green));
                    let ping = app.get_ping(s);
                    make_server_list_item(i, s, is_sel, true, Some(prefix), None, ping)
                }
                None => {
                    let bg = if is_sel { Color::Cyan } else { Color::Reset };
                    let content = vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{:>5} ", i + 1),
                                Style::default().fg(Color::DarkGray).bg(bg),
                            ),
                            Span::styled("★ ", Style::default().fg(Color::Yellow).bg(bg)),
                            Span::styled(
                                "OFFLINE",
                                Style::default()
                                    .fg(Color::Red)
                                    .bg(bg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled("  ", Style::default().bg(bg)),
                            Span::styled(
                                fav.name.as_str(),
                                Style::default().fg(Color::DarkGray).bg(bg),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("       ", Style::default().bg(bg)),
                            Span::styled(
                                format!("{}:{}", fav.ip, fav.port),
                                Style::default().fg(Color::DarkGray).bg(bg),
                            ),
                        ]),
                    ];
                    ListItem::new(content)
                }
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" Favorites  {} ", favorites.len()))
                .borders(Borders::ALL),
        )
        .highlight_symbol("  ");
    f.render_widget(list, list_area);

    if let Some(details_area) = details_area {
        if let Some(server) = app.get_selected_server() {
            render_server_details(f, app, &server, details_area);
        }
    }
}
