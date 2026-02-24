//! Offline mode tab renderer.

use crate::app::{App, Tab};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render_offline_tab(f: &mut Frame, app: &App, area: Rect) {
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let list_area = h_chunks[0];
    let info_area = h_chunks[1];

    let selected = app.selected_indices[Tab::Offline.index()];
    let offset = app.offsets[Tab::Offline.index()];
    let visible = app.visible_items();

    let missions = app.offline_missions.as_deref().unwrap_or(&[]);

    let items: Vec<ListItem> = missions
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .map(|(i, name)| {
            let is_sel = i == selected;
            let style = if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:>3}. ", i + 1),
                    Style::default()
                        .fg(if is_sel {
                            Color::Black
                        } else {
                            Color::DarkGray
                        })
                        .bg(if is_sel { Color::Cyan } else { Color::Reset }),
                ),
                Span::styled(name.as_str(), style),
            ]))
        })
        .collect();

    let list_title = if missions.is_empty() {
        " Offline Missions  (none installed) ".to_string()
    } else {
        format!(" Offline Missions  [{}/{}] ", selected + 1, missions.len())
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(list_title)
                .borders(Borders::ALL)
                .style(Style::default()),
        )
        .highlight_symbol(">> ");
    f.render_widget(list, list_area);

    let selected_mission = missions.get(selected).cloned();

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "DayZ Community Offline Mode",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Play DayZ offline with a local mission,",
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "no internet connection required.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    if let Some(ref status) = app.offline_status {
        lines.push(Line::from(Span::styled(
            status.as_str(),
            Style::default().fg(app.offline_status_color),
        )));
        lines.push(Line::from(""));
    }

    if let Some(ref name) = selected_mission {
        lines.push(Line::from(Span::styled(
            "Selected Mission",
            Style::default().fg(Color::Gray),
        )));
        lines.push(Line::from(Span::styled(
            name.as_str(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press Enter to launch this mission.",
            Style::default().fg(Color::Green),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "No missions installed.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "Press 'u' to download DayZCommunityOfflineMode",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(Span::styled(
            "from github.com/Arkensor/DayZCommunityOfflineMode",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─── Actions ───",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(vec![
        Span::styled("Enter ", Style::default().fg(Color::Yellow)),
        Span::styled("launch selected mission", Style::default().fg(Color::Gray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("u     ", Style::default().fg(Color::Yellow)),
        Span::styled(
            "install / update offline mode",
            Style::default().fg(Color::Gray),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("r     ", Style::default().fg(Color::Yellow)),
        Span::styled("refresh mission list", Style::default().fg(Color::Gray)),
    ]));

    let info = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Offline Mode Info ")
                .borders(Borders::ALL),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(info, info_area);
}
