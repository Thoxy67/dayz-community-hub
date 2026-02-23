//! Shared TUI drawing helpers used across multiple tabs.

use crate::app::{App, Popup, ProgressPhase, ProgressState, Tab};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, ListItem, Paragraph, Wrap},
    Frame,
};

// ─── Layout helpers ───────────────────────────────────────────────────────

/// Split an area into a list area and an optional details panel area.
/// When `show` is true the area is split 55/45 vertically; otherwise the
/// full area is returned as the list area with no details panel.
pub fn split_list_details(area: Rect, show: bool) -> (Rect, Option<Rect>) {
    if show {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    }
}

// ─── Color helpers ────────────────────────────────────────────────────────

/// Color for player count based on server fill.
pub fn player_color(players: i64, max_players: i64) -> Color {
    if players == 0 {
        Color::DarkGray
    } else if players >= max_players {
        Color::Red
    } else if players > max_players / 2 {
        Color::Yellow
    } else {
        Color::Green
    }
}

// ─── Server list item ─────────────────────────────────────────────────────

/// Build a two-line `ListItem` for a server entry.
/// Used by Servers, Favorites, and History tabs for consistent layout.
///
/// Line 1 — glanceable:  idx  ★  ping  players/max  name  [flags]
/// Line 2 — secondary:       ip:port  map  mods  version  time
pub fn make_server_list_item<'a>(
    index: usize,
    server: &'a dayz_community_hub_core::Server,
    is_selected: bool,
    is_favorite: bool,
    prefix: Option<Span<'a>>,
    suffix: Option<Span<'a>>,
    ping_ms: Option<u32>,
) -> ListItem<'a> {
    let bg = if is_selected {
        Color::Cyan
    } else {
        Color::Reset
    };
    let fg = |c: Color| if is_selected { Color::Black } else { c };

    let pc = player_color(server.players, server.max_players);

    // ── Ping ──────────────────────────────────────────────────────────────
    let (ping_str, ping_color) = match ping_ms {
        Some(ms) => {
            let c = if ms < 50 {
                Color::Green
            } else if ms < 100 {
                Color::Yellow
            } else {
                Color::Red
            };
            (format!("{:>3}ms", ms), c)
        }
        None => ("   -  ".to_string(), Color::DarkGray),
    };

    // ── Line 1 ────────────────────────────────────────────────────────────
    let mut line1: Vec<Span> = vec![
        Span::styled(
            format!("{:>5} ", index + 1),
            Style::default().fg(fg(Color::DarkGray)).bg(bg),
        ),
        if is_favorite {
            Span::styled(" ", Style::default().fg(fg(Color::Yellow)).bg(bg))
        } else {
            Span::styled("  ", Style::default().bg(bg))
        },
        Span::styled(
            format!("{:<7}", ping_str),
            Style::default().fg(fg(ping_color)).bg(bg),
        ),
        Span::styled(
            format!("{:>3}/{:<3} ", server.players, server.max_players),
            Style::default()
                .fg(fg(pc))
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    if let Some(pfx) = prefix {
        line1.push(Span::styled(pfx.content, pfx.style.bg(bg)));
        line1.push(Span::styled(" ", Style::default().bg(bg)));
    }

    line1.push(Span::styled(
        server.name.as_str(),
        Style::default()
            .fg(fg(Color::White))
            .bg(bg)
            .add_modifier(if is_selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    ));

    if let Some(sfx) = suffix {
        line1.push(Span::styled("  ", Style::default().bg(bg)));
        line1.push(Span::styled(sfx.content, sfx.style.bg(bg)));
    }

    line1.push(Span::styled("  ", Style::default().bg(bg)));

    // Password lock
    if server.password {
        line1.push(Span::styled(
            " ",
            Style::default().fg(fg(Color::Red)).bg(bg),
        ));
    } else {
        line1.push(Span::styled("  ", Style::default().bg(bg)));
    }

    line1.push(Span::styled(" ", Style::default().bg(bg)));

    // First-person only
    if server.first_person_only {
        line1.push(Span::styled(
            "1P",
            Style::default().fg(fg(Color::Yellow)).bg(bg),
        ));
    } else {
        line1.push(Span::styled("  ", Style::default().bg(bg)));
    }

    line1.push(Span::styled(" ", Style::default().bg(bg)));

    // Platform: Windows / Linux
    if server.environment == "w" {
        line1.push(Span::styled(
            " ",
            Style::default().fg(fg(Color::Blue)).bg(bg),
        ));
    } else {
        line1.push(Span::styled(
            " ",
            Style::default().fg(fg(Color::Green)).bg(bg),
        ));
    }

    // ── Line 2 ────────────────────────────────────────────────────────────
    let mods_str = if server.mods.is_empty() {
        "no mods".to_string()
    } else {
        format!("{} mods", server.mods.len())
    };

    let line2: Vec<Span> = vec![
        Span::styled("       ", Style::default().bg(bg)),
        Span::styled(
            format!(
                "{:<21}",
                format!("{}:{}", server.endpoint.ip, server.game_port)
            ),
            Style::default().fg(fg(Color::Green)).bg(bg),
        ),
        Span::styled(
            format!("{:<14}", server.map),
            Style::default().fg(fg(Color::Cyan)).bg(bg),
        ),
        Span::styled(
            format!("{:<8}", mods_str),
            Style::default().fg(fg(Color::Magenta)).bg(bg),
        ),
        Span::styled(
            format!("{:<10}", server.version),
            Style::default().fg(fg(Color::DarkGray)).bg(bg),
        ),
        Span::styled(
            server.time.as_str(),
            Style::default().fg(fg(Color::DarkGray)).bg(bg),
        ),
    ];

    ListItem::new(vec![Line::from(line1), Line::from(line2)])
}

// ─── Title bar ────────────────────────────────────────────────────────────

pub fn draw_title_bar(f: &mut Frame, app: &App, area: Rect) {
    let server_count = if let Some(ref indices) = app.filtered_indices {
        format!("{}/{}", indices.len(), app.servers.len())
    } else {
        format!("{}", app.servers.len())
    };

    let sa_players: i64 = app.servers.iter().map(|s| s.players).sum();

    let steam_str = app
        .steam_players
        .map(|n| format!("{}", n))
        .unwrap_or_else(|| "…".to_string());

    let login_info = app.ctl.steamcmd_login().unwrap_or("no steamcmd");
    let player = app.ctl.profile().player.as_deref().unwrap_or("unnamed");

    let sep = || Span::styled(" | ", Style::default().fg(Color::White));
    let title = Line::from(vec![
        Span::styled(
            " DayZ-SA Multi Launcher ",
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        sep(),
        Span::styled("Steam: ", Style::default().fg(Color::Gray)),
        Span::styled(steam_str, Style::default().fg(Color::Blue)),
        sep(),
        Span::styled("DzSA: ", Style::default().fg(Color::Gray)),
        Span::styled(format!("{}", sa_players), Style::default().fg(Color::Green)),
        sep(),
        Span::styled("Servers: ", Style::default().fg(Color::Gray)),
        Span::styled(format!("{}", server_count), Style::default().fg(Color::Red)),
        sep(),
        Span::styled(player, Style::default().fg(Color::Cyan)),
        sep(),
        Span::styled(
            format!("{} ", login_info),
            Style::default().fg(Color::Magenta),
        ),
    ]);

    f.render_widget(Paragraph::new(title), area);
}

// ─── Tab bar ──────────────────────────────────────────────────────────────

pub fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let tabs: Vec<Span> = Tab::all()
        .iter()
        .enumerate()
        .flat_map(|(i, tab)| {
            let is_sel = *tab == app.tab;
            let label = tab.label();

            let extra = match tab {
                Tab::Favorites => format!("({})", app.ctl.profile().favorites.len()),
                Tab::History => format!("({})", app.ctl.profile().history.len()),
                Tab::Mods => {
                    let count = app.installed_mods.as_ref().map(|m| m.len()).unwrap_or(0);
                    format!("({})", count)
                }
                _ => String::new(),
            };

            let style = if is_sel {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let sep = if i < Tab::all().len() - 1 {
                Span::styled(" | ", Style::default().fg(Color::DarkGray))
            } else {
                Span::raw(" ")
            };

            vec![Span::styled(format!(" {}{} ", label, extra), style), sep]
        })
        .collect();

    let search_info = if app.search_active {
        vec![
            Span::styled("  Search: ", Style::default().fg(Color::Yellow)),
            Span::styled(&app.search_query, Style::default().fg(Color::White)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]
    } else {
        vec![]
    };

    let mut all_spans = tabs;
    all_spans.extend(search_info);

    let line = Line::from(all_spans);
    let widget = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    f.render_widget(widget, area);
}

// ─── Status bar ───────────────────────────────────────────────────────────

pub fn keybinds_for_tab(tab: Tab) -> &'static str {
    match tab {
        Tab::Servers => {
            "j/k:Nav | Enter:Connect | f:Fav | i:A2S | m:Details | /:Search | r:Refresh | Tab/S-Tab:Tabs | q:Quit"
        }
        Tab::Favorites => {
            "j/k:Nav | Enter:Connect | i:A2S | x:Remove | Tab/S-Tab:Tabs | q:Quit"
        }
        Tab::History => {
            "j/k:Nav | Enter:Connect | i:A2S | f:Fav | x:Remove | c:Clear | Tab/S-Tab:Tabs | q:Quit"
        }
        Tab::Mods => {
            "j/k:Nav | u:Update | U:All | d:Del | m:Managed | c:Cleanup | r:Refresh | q:Quit"
        }
        Tab::News => {
            "j/k:Nav | Enter:Open in browser | Ctrl+d/u:Scroll content | Tab/S-Tab:Tabs | q:Quit"
        }
        Tab::DirectConnect => "Up/Down:Fields | Enter:Activate | Tab/S-Tab:Tabs | Esc:Back",
        Tab::Options => "j/k:Nav | Enter:Toggle | e:Edit Value | Tab/S-Tab:Tabs | q:Quit",
        Tab::Offline => {
            "j/k:Nav | Enter:Launch mission | u:Update/Install | Tab/S-Tab:Tabs | q:Quit"
        }
    }
}

pub fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let keybinds = keybinds_for_tab(app.tab);

    let line = if let Some(ref msg) = app.status_message {
        Line::from(vec![
            Span::styled(msg.as_str(), Style::default().fg(app.status_color)),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(keybinds, Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(Span::styled(keybinds, Style::default().fg(Color::DarkGray)))
    };

    let widget = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    f.render_widget(widget, area);
}

// ─── Progress overlay ─────────────────────────────────────────────────────

pub fn draw_progress_overlay(f: &mut Frame, progress: &ProgressState, area: Rect) {
    let popup_width = (area.width as f32 * 0.7).min(70.0).max(40.0) as u16;

    let hint = match &progress.phase {
        ProgressPhase::Finished { hint, .. } => hint.clone(),
        _ => None,
    };
    let hint_lines = hint
        .as_ref()
        .map(|h| h.lines().count() as u16 + 1)
        .unwrap_or(0);

    let completed_lines = progress.completed.len().min(8) as u16;
    let popup_height = (6 + completed_lines + hint_lines).min(area.height.saturating_sub(4));

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let constraints = if hint.is_some() {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(hint_lines),
            Constraint::Min(0),
        ]
    } else {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ]
    };

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .margin(1)
        .split(popup_area);

    let title = if progress.total > 0 {
        format!(" Mod Operation [{}/{}] ", progress.current, progress.total)
    } else {
        " Mod Operation ".to_string()
    };

    let border_color = match &progress.phase {
        ProgressPhase::Finished { hint, .. } if hint.is_some() => Color::Red,
        _ => Color::Cyan,
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().fg(border_color));
    f.render_widget(block, popup_area);

    let status_text = match &progress.phase {
        ProgressPhase::ShuttingDownSteam => {
            "Closing Steam (required before steamcmd can run)...".to_string()
        }
        ProgressPhase::Downloading => {
            format!(
                "Downloading: {} ({})",
                progress.current_mod_name, progress.current_mod_id
            )
        }
        ProgressPhase::Finished {
            ok, failed, hint, ..
        } => {
            if hint.is_some() {
                "Login failed or expired".to_string()
            } else if *failed == 0 {
                format!("Done! {} mods completed successfully", ok)
            } else {
                format!("Done: {} OK, {} failed", ok, failed)
            }
        }
    };
    let status_color = match &progress.phase {
        ProgressPhase::Downloading => Color::Yellow,
        ProgressPhase::Finished { hint, .. } if hint.is_some() => Color::Red,
        ProgressPhase::Finished { failed, .. } if *failed > 0 => Color::Red,
        _ => Color::Green,
    };
    f.render_widget(
        Paragraph::new(status_text).style(Style::default().fg(status_color)),
        inner[0],
    );

    let ratio = if progress.total > 0 {
        let done = progress.completed.len() as f64;
        done / progress.total as f64
    } else {
        0.0
    };
    let gauge_label = if progress.total > 0 {
        format!("{}/{}", progress.completed.len(), progress.total)
    } else {
        "...".to_string()
    };
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ratio.min(1.0))
        .label(gauge_label);
    f.render_widget(gauge, inner[1]);

    let list_area_idx = if hint.is_some() {
        if let Some(ref hint_text) = hint {
            let hint_lines: Vec<Line> = hint_text
                .lines()
                .map(|l| {
                    Line::from(Span::styled(
                        l.to_string(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ))
                })
                .collect();
            f.render_widget(Paragraph::new(hint_lines), inner[2]);
        }
        3
    } else {
        2
    };

    if !progress.completed.is_empty() && list_area_idx < inner.len() {
        let max_show = inner[list_area_idx].height as usize;
        let skip = progress.completed.len().saturating_sub(max_show);
        let lines: Vec<Line> = progress
            .completed
            .iter()
            .skip(skip)
            .map(|(id, name, success)| {
                let marker = if *success { "+" } else { "x" };
                let color = if *success { Color::Green } else { Color::Red };
                Line::from(vec![
                    Span::styled(format!(" {} ", marker), Style::default().fg(color)),
                    Span::styled(name.as_str(), Style::default().fg(Color::White)),
                    Span::styled(format!(" ({})", id), Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lines), inner[list_area_idx]);
    }
}

// ─── Popup overlay ────────────────────────────────────────────────────────

pub fn draw_popup(f: &mut Frame, popup: &Popup, area: Rect) {
    let popup_width = (area.width as f32 * 0.6).min(60.0) as u16;
    let popup_height = (area.height as f32 * 0.4).min(20.0) as u16;

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    match popup {
        Popup::Confirm { title, message, .. } => {
            let mut lines: Vec<Line> = message.lines().map(|l| Line::from(l.to_string())).collect();
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "[y] Yes  ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "[n] No",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]));

            let widget = Paragraph::new(Text::from(lines))
                .block(
                    Block::default()
                        .title(title.as_str())
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(widget, popup_area);
        }
        Popup::Info { title, message } => {
            let mut lines: Vec<Line> = message.lines().map(|l| Line::from(l.to_string())).collect();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "[Press any key to dismiss]",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
            let widget = Paragraph::new(Text::from(lines))
                .block(
                    Block::default()
                        .title(title.as_str())
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Red)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(widget, popup_area);
        }
    }
}

// ─── Server details panel ─────────────────────────────────────────────────

pub fn render_server_details(f: &mut Frame, app: &App, server: &dayz_community_hub_core::Server, area: Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Gray)),
            Span::styled(&server.name, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("IP: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!(
                    "{}:{} (query:{})",
                    server.endpoint.ip, server.game_port, server.endpoint.port
                ),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Players: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}/{}", server.players, server.max_players),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled("  Map: ", Style::default().fg(Color::Gray)),
            Span::styled(&server.map, Style::default().fg(Color::Cyan)),
            Span::styled("  Version: ", Style::default().fg(Color::Gray)),
            Span::styled(&server.version, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Time: ", Style::default().fg(Color::Gray)),
            Span::styled(&server.time, Style::default().fg(Color::White)),
            Span::styled("  1PP: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if server.first_person_only {
                    "Yes"
                } else {
                    "No"
                },
                Style::default().fg(Color::White),
            ),
            Span::styled("  Password: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if server.password { "Yes" } else { "No" },
                Style::default().fg(Color::White),
            ),
            Span::styled("  Platform: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if server.environment == "w" {
                    "Windows"
                } else {
                    "Linux"
                },
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    if !server.mods.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("Mods ({}):", server.mods.len()),
            Style::default().fg(Color::Magenta),
        )));

        let installed_ids: std::collections::HashSet<u64> = app
            .installed_mods
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|m| m.id)
            .collect();

        for mod_ in &server.mods {
            let is_installed = installed_ids.contains(&(mod_.steam_workshop_id as u64));
            let marker = if is_installed { "+" } else { "-" };
            let color = if is_installed {
                Color::Green
            } else {
                Color::Red
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", marker), Style::default().fg(color)),
                Span::styled(&mod_.name, Style::default().fg(Color::White)),
                Span::styled(
                    format!(" ({})", mod_.steam_workshop_id),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    if let Some(ref details) = app.current_a2s_details {
        if app.detailed_server_index == Some(app.selected_index()) {
            lines.push(Line::from(Span::styled(
                "--- A2S Live Info ---",
                Style::default().fg(Color::Green),
            )));
            lines.push(Line::from(format!(
                "  Players: {}/{}  Game: {}",
                details.info.players, details.info.max_players, details.info.game
            )));
            if let Some(ref players) = details.players {
                for p in players.iter().take(20) {
                    if !p.name.is_empty() {
                        lines.push(Line::from(format!("    {} (score: {})", p.name, p.score)));
                    }
                }
            }
        }
    }

    let total_lines = lines.len();
    let visible_height = area.height.saturating_sub(2) as usize;
    let scroll_indicator = if total_lines > visible_height {
        format!(
            " [{}-{}/{}] [Ctrl+d/Ctrl+u:Scroll]",
            app.details_scroll_offset + 1,
            (app.details_scroll_offset as usize + visible_height).min(total_lines),
            total_lines
        )
    } else {
        String::new()
    };

    let text = Text::from(lines);
    let widget = Paragraph::new(text)
        .block(
            Block::default()
                .title(format!("Server Details [Esc to close]{}", scroll_indicator))
                .borders(Borders::ALL),
        )
        .scroll((app.details_scroll_offset, 0));
    f.render_widget(widget, area);
}

// ─── Word wrap helper ─────────────────────────────────────────────────────

/// Word-wrap `text` to at most `width` chars per line, returning owned strings.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
            } else if current.len() + 1 + word.len() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = word.to_string();
            }
        }
        if !current.is_empty() || !paragraph.is_empty() {
            lines.push(current);
        }
    }
    lines
}
