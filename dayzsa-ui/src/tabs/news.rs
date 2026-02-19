//! News tab renderer.

use crate::{app::App, widgets::wrap_text};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render_news_tab(f: &mut Frame, app: &App, area: Rect) {
    let offset = app.offset();
    let selected = app.selected_index();
    let visible = app.visible_items();

    let articles = match app.news_articles.as_deref() {
        None => {
            let text = Paragraph::new("Fetching DayZ news…")
                .block(Block::default().title("DayZ News").borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(text, area);
            return;
        }
        Some([]) => {
            let text = Paragraph::new("No articles found.")
                .block(Block::default().title("DayZ News").borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(text, area);
            return;
        }
        Some(a) => a,
    };

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let list_area = h_chunks[0];
    let detail_area = h_chunks[1];

    let items: Vec<ListItem> = articles
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .map(|(i, article)| {
            let is_sel = i == selected;

            let cat = article
                .category
                .as_ref()
                .map(|c| c.slug.as_str())
                .unwrap_or("news");

            let date = article.date().to_string();
            let content = vec![
                Line::from(vec![
                    Span::styled(
                        format!("{:>4}. ", i + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        &article.title,
                        if is_sel {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::styled(
                        if date.is_empty() {
                            String::new()
                        } else {
                            format!("  {}", date)
                        },
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("       ", Style::default()),
                    Span::styled(format!("[{}]", cat), Style::default().fg(Color::Cyan)),
                ]),
            ];

            ListItem::new(content)
        })
        .collect();

    let cache_age = app
        .news_fetched_at
        .map(|t| {
            let secs = t.elapsed().as_secs();
            if secs < 60 {
                format!("{}s ago", secs)
            } else {
                format!("{}m ago", secs / 60)
            }
        })
        .unwrap_or_else(|| "?".to_string());

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(
                    "DayZ News [{}/{}] (cached {})",
                    selected + 1,
                    articles.len(),
                    cache_age
                ))
                .borders(Borders::ALL),
        )
        .highlight_symbol(">> ")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(list, list_area);

    let detail_lines = if let Some(article) = articles.get(selected) {
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            &article.title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        let cat_name = article
            .category
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("News");
        let date = article.date();
        lines.push(Line::from(vec![
            Span::styled(format!("[{}]", cat_name), Style::default().fg(Color::Cyan)),
            if !date.is_empty() {
                Span::styled(format!("  {}", date), Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]));

        if let Some(ver) = &article.version {
            if !ver.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Version: ", Style::default().fg(Color::Gray)),
                    Span::styled(ver.as_str(), Style::default().fg(Color::Green)),
                ]));
            }
        }

        if let Some(author) = &article.author {
            lines.push(Line::from(""));
            let mut author_spans = vec![
                Span::styled("By: ", Style::default().fg(Color::Gray)),
                Span::styled(&author.name, Style::default().fg(Color::Magenta)),
            ];
            if let Some(role) = &author.role {
                author_spans.push(Span::styled(
                    format!("  ({})", role),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(author_spans));
        }

        let body = article.html_to_text();
        if !body.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "─── Content ───",
                Style::default().fg(Color::DarkGray),
            )));
            let panel_w = detail_area.width.saturating_sub(4) as usize;
            let panel_w = if panel_w == 0 { 40 } else { panel_w };
            for word_line in wrap_text(&body, panel_w) {
                lines.push(Line::from(Span::styled(
                    word_line,
                    Style::default().fg(Color::White),
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            article.url(),
            Style::default().fg(Color::DarkGray),
        )));

        lines
    } else {
        vec![Line::from(Span::styled(
            "Select an article",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let detail = Paragraph::new(detail_lines)
        .block(
            Block::default()
                .title("Article  (Ctrl+d/u: scroll)")
                .borders(Borders::ALL),
        )
        .wrap(ratatui::widgets::Wrap { trim: true })
        .scroll((app.news_detail_scroll, 0));
    f.render_widget(detail, detail_area);
}
