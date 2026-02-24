//! Options tab renderer.

use crate::app::App;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub fn render_options_tab(f: &mut Frame, app: &App, area: Rect) {
    let options = app.ctl.profile().options.all_options();
    let selected = app.selected_index();

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, (name, opt))| {
            let is_sel = i == selected;
            let is_editing = is_sel && app.option_edit_active;
            let style = if is_sel {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let check = if opt.enabled {
                Span::styled("[x] ", Style::default().fg(Color::Green))
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            };

            let value_str = if is_editing {
                format!(" = {}|", app.option_edit_value)
            } else {
                opt.value
                    .as_ref()
                    .map(|v| format!(" = {}", v))
                    .unwrap_or_default()
            };

            let value_color = if is_editing {
                Color::Green
            } else if is_sel {
                Color::Yellow
            } else {
                Color::White
            };

            let content = vec![
                Line::from(vec![
                    Span::styled(
                        format!("{:>3}. ", i + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    check,
                    Span::styled(format!("-{}", name), Style::default().fg(value_color)),
                    Span::styled(value_str, Style::default().fg(value_color)),
                ]),
                Line::from(vec![
                    Span::styled("         ", Style::default()),
                    Span::styled(&opt.description, Style::default().fg(Color::DarkGray)),
                ]),
            ];

            ListItem::new(content).style(style)
        })
        .collect();

    let args_preview = app.ctl.profile().options.to_args().join(" ");
    let title = format!(
        "Launch Options [{}]",
        if args_preview.is_empty() {
            "none"
        } else {
            &args_preview
        }
    );

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_symbol(">> ");
    f.render_widget(list, area);
}
