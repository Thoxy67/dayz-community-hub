//! Mods tab renderer.

use crate::app::App;
use dayzsa_core::{mods, utils};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render_mods_tab(f: &mut Frame, app: &App, area: Rect) {
    let offset = app.offset();
    let selected = app.selected_index();
    let visible = app.visible_items();
    let mods = app.installed_mods.as_deref().unwrap_or(&[]);

    if mods.is_empty() {
        let text = Paragraph::new("No mods installed or failed to load.\nPress 'r' to refresh.")
            .block(
                Block::default()
                    .title("Installed Mods")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(text, area);
        return;
    }

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let list_area = h_chunks[0];
    let detail_area = h_chunks[1];

    let items: Vec<ListItem> = mods
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .map(|(i, m)| {
            let is_sel = i == selected;

            let name_style = if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let managed_span = if m.managed {
                Span::styled("●", Style::default().fg(Color::Green))
            } else {
                Span::styled("○", Style::default().fg(Color::DarkGray))
            };

            let size_str = mods::format_size(m.size);

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:>4} ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                managed_span,
                Span::raw(" "),
                Span::styled(m.name.as_str(), name_style),
                Span::styled(
                    format!("  {}", size_str),
                    Style::default()
                        .fg(if is_sel {
                            Color::Black
                        } else {
                            Color::DarkGray
                        })
                        .bg(if is_sel { Color::Cyan } else { Color::Reset }),
                ),
            ]))
        })
        .collect();

    let total_size: u64 = mods.iter().map(|m| m.size).sum();
    let managed_count = mods.iter().filter(|m| m.managed).count();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(
                    " Mods  {}/{}  total {}  managed {} ",
                    selected + 1,
                    mods.len(),
                    mods::format_size(total_size),
                    managed_count,
                ))
                .borders(Borders::ALL),
        )
        .highlight_symbol("  ");
    f.render_widget(list, list_area);

    let detail_lines = if let Some(m) = mods.get(selected) {
        let installed = if m.local_updated > 0 {
            utils::format_relative_time(m.local_updated)
        } else {
            "unknown".to_string()
        };

        vec![
            Line::from(Span::styled(
                m.name.as_str(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("ID       ", Style::default().fg(Color::DarkGray)),
                Span::styled(m.id.to_string(), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Size     ", Style::default().fg(Color::DarkGray)),
                Span::styled(mods::format_size(m.size), Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::styled("Updated  ", Style::default().fg(Color::DarkGray)),
                Span::styled(installed, Style::default().fg(Color::Magenta)),
            ]),
            Line::from(vec![
                Span::styled("Managed  ", Style::default().fg(Color::DarkGray)),
                if m.managed {
                    Span::styled(
                        "yes  (owned by launcher)",
                        Style::default().fg(Color::Green),
                    )
                } else {
                    Span::styled("no   (external)", Style::default().fg(Color::DarkGray))
                },
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "─── Actions ───",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(vec![
                Span::styled("u ", Style::default().fg(Color::Yellow)),
                Span::styled("update this mod", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("U ", Style::default().fg(Color::Yellow)),
                Span::styled("update all mods", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("d ", Style::default().fg(Color::Yellow)),
                Span::styled("delete this mod", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("m ", Style::default().fg(Color::Yellow)),
                Span::styled("toggle managed", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("c ", Style::default().fg(Color::Yellow)),
                Span::styled("cleanup all managed", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("r ", Style::default().fg(Color::Yellow)),
                Span::styled("refresh list", Style::default().fg(Color::Gray)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
                    m.id
                ),
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "No mod selected",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let detail = Paragraph::new(detail_lines)
        .block(Block::default().title(" Details ").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(detail, detail_area);
}
