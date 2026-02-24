//! First-run setup wizard rendered with ratatui/termion.

use dayz_community_hub_core::{Result, config, steamcmd, system};
use ratatui::{
    Terminal,
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io;
use termion::{event::Key, input::TermRead, raw::IntoRawMode};

/// Run the first-run setup wizard if any required fields are missing.
/// Returns immediately (no TUI rendered) when everything is already configured.
pub fn run_setup_if_needed(profile_path: &std::path::Path) -> Result<()> {
    let mut profile = if profile_path.exists() {
        config::Profile::load(profile_path)?
    } else {
        let mut prof = config::Profile::default_with_version(env!("CARGO_PKG_VERSION"));
        prof.path = profile_path.to_path_buf();
        prof
    };

    // Auto-detect steam root silently
    if profile.steam_root.is_none() {
        if let Some(root) = steamcmd::find_steam_root() {
            profile.steam_root = Some(root.to_string_lossy().to_string());
        }
    }

    let need_login = profile.steam_login.is_none();
    let need_player = profile.player.is_none();
    let need_steam_root = profile.steam_root.is_none();

    let has_warnings = (profile.steamcmd_enabled && steamcmd::find_steamcmd().is_none())
        || matches!(
            system::check_max_map_count(),
            Ok(ref c) if !c.ok
        );

    if !need_login && !need_player && !need_steam_root && !has_warnings {
        return Ok(());
    }

    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    #[derive(Clone, Copy, PartialEq)]
    enum WizardField {
        Login,
        Player,
        SteamRoot,
        Warnings,
        Done,
    }

    let fields_order: Vec<WizardField> = {
        let mut v = vec![];
        if need_login {
            v.push(WizardField::Login);
        }
        if need_player {
            v.push(WizardField::Player);
        }
        if need_steam_root {
            v.push(WizardField::SteamRoot);
        }
        if has_warnings {
            v.push(WizardField::Warnings);
        }
        v.push(WizardField::Done);
        v
    };

    let mut field_idx = 0usize;
    let mut input_login = String::new();
    let mut input_player = String::new();
    let mut input_steam_root = String::new();

    let async_stdin = termion::async_stdin();
    let mut keys = async_stdin.keys();

    let warnings_text: String = {
        let mut w = vec![];
        if profile.steamcmd_enabled && steamcmd::find_steamcmd().is_none() {
            w.push(
                "steamcmd not found — mod downloads will be disabled.\n\
                 Install: sudo apt install steamcmd"
                    .to_string(),
            );
        }
        if let Ok(ref check) = system::check_max_map_count() {
            if !check.ok {
                w.push(check.recommendation());
            }
        }
        w.join("\n\n")
    };

    loop {
        let current_field = fields_order[field_idx];

        terminal.draw(|f| {
            let size = f.area();

            let block = Block::default()
                .title(" DayZ-SA Multi Launcher — First Run Setup ")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(block, size);

            let inner = Rect {
                x: size.x + 2,
                y: size.y + 1,
                width: size.width.saturating_sub(4),
                height: size.height.saturating_sub(2),
            };

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Min(0),
                    Constraint::Length(2),
                ])
                .split(inner);

            let step = field_idx + 1;
            let total = fields_order.len();
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "Welcome! ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("Step {}/{} — ", step, total),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        match current_field {
                            WizardField::Login => "Steam login",
                            WizardField::Player => "Player name",
                            WizardField::SteamRoot => "Steam root path",
                            WizardField::Warnings => "System warnings",
                            WizardField::Done => "All done",
                        },
                        Style::default().fg(Color::Cyan),
                    ),
                ])),
                chunks[0],
            );

            let content_area = chunks[1];
            match current_field {
                WizardField::Login => {
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(Span::styled(
                                "Steam username used for workshop (mod) downloads.",
                                Style::default().fg(Color::White),
                            )),
                            Line::from(Span::styled(
                                "Type 'anonymous' to skip mod downloads.",
                                Style::default().fg(Color::DarkGray),
                            )),
                            Line::from(""),
                            Line::from(vec![
                                Span::styled("Username: ", Style::default().fg(Color::Gray)),
                                Span::styled(
                                    format!("{}|", input_login),
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ]),
                        ])
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Steam Login "),
                        ),
                        content_area,
                    );
                }
                WizardField::Player => {
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(Span::styled(
                                "Your in-game display name.",
                                Style::default().fg(Color::White),
                            )),
                            Line::from(""),
                            Line::from(vec![
                                Span::styled("Player name: ", Style::default().fg(Color::Gray)),
                                Span::styled(
                                    format!("{}|", input_player),
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ]),
                        ])
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Player Name "),
                        ),
                        content_area,
                    );
                }
                WizardField::SteamRoot => {
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(Span::styled(
                                "Path to your steamapps directory.",
                                Style::default().fg(Color::White),
                            )),
                            Line::from(Span::styled(
                                "e.g. /home/user/.steam/steam/steamapps",
                                Style::default().fg(Color::DarkGray),
                            )),
                            Line::from(""),
                            Line::from(vec![
                                Span::styled("Path: ", Style::default().fg(Color::Gray)),
                                Span::styled(
                                    format!("{}|", input_steam_root),
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ]),
                        ])
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Steam Root Path "),
                        ),
                        content_area,
                    );
                }
                WizardField::Warnings => {
                    let mut lines: Vec<Line> = vec![
                        Line::from(Span::styled(
                            "Please review the following warnings:",
                            Style::default().fg(Color::Yellow),
                        )),
                        Line::from(""),
                    ];
                    for warn_line in warnings_text.lines() {
                        lines.push(Line::from(Span::styled(
                            warn_line.to_string(),
                            Style::default().fg(Color::Red),
                        )));
                    }
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "Press Enter to continue.",
                        Style::default().fg(Color::DarkGray),
                    )));
                    f.render_widget(
                        Paragraph::new(lines)
                            .block(Block::default().borders(Borders::ALL).title(" Warnings "))
                            .wrap(ratatui::widgets::Wrap { trim: true }),
                        content_area,
                    );
                }
                WizardField::Done => {
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(Span::styled(
                                "Setup complete! Configuration saved.",
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            )),
                            Line::from(""),
                            Line::from(Span::styled(
                                "Press Enter to launch the launcher.",
                                Style::default().fg(Color::White),
                            )),
                        ])
                        .block(Block::default().borders(Borders::ALL).title(" Ready ")),
                        content_area,
                    );
                }
            }

            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Enter", Style::default().fg(Color::Green)),
                    Span::styled(":Next  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Backspace", Style::default().fg(Color::Yellow)),
                    Span::styled(":Delete  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Ctrl+C", Style::default().fg(Color::Red)),
                    Span::styled(":Quit", Style::default().fg(Color::DarkGray)),
                ])),
                chunks[2],
            );
        })?;

        std::thread::sleep(std::time::Duration::from_millis(16));

        while let Some(Ok(key)) = keys.next() {
            match key {
                Key::Ctrl('c') => {
                    terminal.clear()?;
                    terminal.show_cursor()?;
                    return Err(dayz_community_hub_core::Error::Io(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "Setup cancelled",
                    )));
                }
                Key::Char('\n') => {
                    match current_field {
                        WizardField::Login => {
                            let login = if input_login.trim().is_empty() {
                                "anonymous".to_string()
                            } else {
                                input_login.trim().to_string()
                            };
                            profile.steam_login = Some(login);
                        }
                        WizardField::Player => {
                            if !input_player.trim().is_empty() {
                                profile.player = Some(input_player.trim().to_string());
                            }
                        }
                        WizardField::SteamRoot => {
                            if !input_steam_root.trim().is_empty() {
                                profile.steam_root = Some(input_steam_root.trim().to_string());
                            }
                        }
                        WizardField::Warnings => {}
                        WizardField::Done => {
                            profile.save()?;
                            terminal.clear()?;
                            terminal.show_cursor()?;
                            return Ok(());
                        }
                    }
                    if field_idx + 1 < fields_order.len() {
                        field_idx += 1;
                    }
                }
                Key::Backspace => match current_field {
                    WizardField::Login => {
                        input_login.pop();
                    }
                    WizardField::Player => {
                        input_player.pop();
                    }
                    WizardField::SteamRoot => {
                        input_steam_root.pop();
                    }
                    _ => {}
                },
                Key::Char(c) => match current_field {
                    WizardField::Login => input_login.push(c),
                    WizardField::Player => input_player.push(c),
                    WizardField::SteamRoot => input_steam_root.push(c),
                    _ => {}
                },
                _ => {}
            }
            break;
        }
    }
}
