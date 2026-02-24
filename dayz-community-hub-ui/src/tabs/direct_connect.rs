//! Direct connect tab renderer.

use crate::app::{App, DirectConnectField};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render_direct_connect_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let field_style = |active: bool| {
        if active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    // Address
    let cursor = if app.direct_cursor == DirectConnectField::Address {
        "|"
    } else {
        ""
    };
    let addr = Paragraph::new(format!("{}{}", app.direct_address, cursor))
        .style(field_style(
            app.direct_cursor == DirectConnectField::Address,
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Server Address (IP or IP:PORT)"),
        );
    f.render_widget(addr, chunks[0]);

    // Port
    let cursor = if app.direct_cursor == DirectConnectField::Port {
        "|"
    } else {
        ""
    };
    let port = Paragraph::new(format!("{}{}", app.direct_port, cursor))
        .style(field_style(app.direct_cursor == DirectConnectField::Port))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Port (default: 2302)"),
        );
    f.render_widget(port, chunks[1]);

    // Password
    let cursor = if app.direct_cursor == DirectConnectField::Password {
        "|"
    } else {
        ""
    };
    let pw_display = if app.direct_password.is_empty() {
        format!("(optional){}", cursor)
    } else {
        format!("{}{}", "*".repeat(app.direct_password.len()), cursor)
    };
    let pw = Paragraph::new(pw_display)
        .style(field_style(
            app.direct_cursor == DirectConnectField::Password,
        ))
        .block(Block::default().borders(Borders::ALL).title("Password"));
    f.render_widget(pw, chunks[2]);

    // Button row
    let btn_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[3]);

    let info_btn_style = if app.direct_cursor == DirectConnectField::ServerInfo {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let info_btn = Paragraph::new("[ SERVER INFO ]")
        .style(info_btn_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(info_btn, btn_chunks[0]);

    let connect_btn_style = if app.direct_cursor == DirectConnectField::Connect {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let connect_btn = Paragraph::new("[ CONNECT ]")
        .style(connect_btn_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(connect_btn, btn_chunks[1]);

    // Server info panel
    let info = if let Some(ref server) = app.direct_server_found {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Found: ", Style::default().fg(Color::Green)),
                Span::styled(&server.name, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(format!(
                "Players: {}/{}",
                server.players, server.max_players
            )),
        ];
        if !server.mods.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("Mods ({}):", server.mods.len()),
                Style::default().fg(Color::Magenta),
            )));
            for m in server.mods.iter().take(15) {
                lines.push(Line::from(format!(
                    "  - {} ({})",
                    m.name, m.steam_workshop_id
                )));
            }
            if server.mods.len() > 15 {
                lines.push(Line::from(format!(
                    "  ... and {} more",
                    server.mods.len() - 15
                )));
            }
        }
        Text::from(lines)
    } else {
        Text::from("Enter server address above. Use Up/Down to navigate fields.")
    };

    let info_widget = Paragraph::new(info)
        .block(Block::default().borders(Borders::ALL).title("Server Info"))
        .wrap(Wrap { trim: true });
    f.render_widget(info_widget, chunks[4]);
}
