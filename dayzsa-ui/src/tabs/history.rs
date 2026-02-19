//! History tab renderer.

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

pub fn render_history_tab(f: &mut Frame, app: &App, area: Rect) {
    let history = &app.ctl.profile().history;
    let selected = app.selected_index();

    if history.is_empty() {
        let text = Paragraph::new("No history yet.\nConnect to a server to see it here.")
            .block(Block::default().title("History").borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(text, area);
        return;
    }

    let (list_area, details_area) = split_list_details(area, app.show_server_details);

    let items: Vec<ListItem> = history
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_sel = i == selected;

            let server = app.servers.iter().find(|s| {
                s.endpoint.ip == entry.ip
                    && (s.endpoint.port as u16 == entry.port || s.game_port as u16 == entry.port)
            });

            let is_fav = app.ctl.profile().is_favorite(&entry.ip, entry.port);
            let time_suffix =
                Span::styled(entry.relative_time(), Style::default().fg(Color::DarkGray));

            match server {
                Some(s) => {
                    let prefix = Span::styled(" ON  ", Style::default().fg(Color::Green));
                    let ping = app.get_ping(s);
                    make_server_list_item(
                        i,
                        s,
                        is_sel,
                        is_fav,
                        Some(prefix),
                        Some(time_suffix),
                        ping,
                    )
                }
                None => {
                    let style = if is_sel {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let fav_marker = if is_fav {
                        Span::styled("* ", Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("  ")
                    };
                    let content = vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{:>5}. ", i + 1),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(" OFF ", Style::default().fg(Color::Red)),
                            fav_marker,
                            Span::styled(entry.name.as_str(), Style::default().fg(Color::DarkGray)),
                        ]),
                        Line::from(vec![
                            Span::styled("        ", Style::default()),
                            Span::styled(
                                format!("{}:{}", entry.ip, entry.port),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                entry.relative_time(),
                                Style::default().fg(Color::Magenta),
                            ),
                        ]),
                    ];
                    ListItem::new(content).style(style)
                }
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!("Recently Played [{}]", history.len()))
                .borders(Borders::ALL),
        )
        .highlight_symbol(">> ");
    f.render_widget(list, list_area);

    if let Some(details_area) = details_area {
        if let Some(server) = app.get_selected_server() {
            render_server_details(f, app, &server, details_area);
        }
    }
}
