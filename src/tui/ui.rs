use ratatui::prelude::*;
use ratatui::widgets::*;
use std::sync::Arc;

use super::app::{Panel, TuiApp};
use crate::state::{AppState, ClientInfo, DownloadInfo};

pub fn draw_ui(
    frame: &mut Frame,
    state: &Arc<AppState>,
    tui_app: &TuiApp,
    clients: &[ClientInfo],
    downloads: &[DownloadInfo],
) {
    let area = frame.area();

    let url_lines = 3 + state.interfaces.len() as u16 + 1;
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(url_lines),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(frame, state, main_layout[0]);
    draw_panels(frame, tui_app, clients, downloads, main_layout[1]);
    draw_footer(frame, state, main_layout[2]);
}

fn draw_header(frame: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "serve ",
                Style::default().fg(Color::Rgb(240, 136, 62)).bold(),
            ),
            Span::styled(
                "file server",
                Style::default().fg(Color::Rgb(201, 209, 217)),
            ),
        ]),
        Line::from(vec![
            Span::styled("root: ", Style::default().fg(Color::Rgb(139, 148, 158))),
            Span::styled(
                state.root.display().to_string(),
                Style::default().fg(Color::Rgb(126, 231, 135)),
            ),
        ]),
        Line::from(""),
    ];

    for iface in &state.interfaces {
        let http_url = format!("http://{}:{}", iface.ip, state.http_port);
        let mut spans = vec![
            Span::styled(
                format!("  {:10} ", iface.name),
                Style::default().fg(Color::Rgb(139, 148, 158)),
            ),
            Span::styled(
                http_url,
                Style::default()
                    .fg(Color::Rgb(88, 166, 255))
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ];

        if let Some(ssl_port) = state.https_port {
            spans.push(Span::styled(
                format!("  https://{}:{}", iface.ip, ssl_port),
                Style::default()
                    .fg(Color::Rgb(126, 231, 135))
                    .add_modifier(Modifier::UNDERLINED),
            ));
        }

        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(48, 54, 61)))
        .title(Span::styled(
            " URLs ",
            Style::default().fg(Color::Rgb(240, 136, 62)),
        ))
        .style(Style::default().bg(Color::Rgb(13, 17, 23)));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_panels(
    frame: &mut Frame,
    tui_app: &TuiApp,
    clients: &[ClientInfo],
    downloads: &[DownloadInfo],
    area: Rect,
) {
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_clients_panel(frame, tui_app, clients, panels[0]);
    draw_downloads_panel(frame, tui_app, downloads, panels[1]);
}

fn draw_clients_panel(frame: &mut Frame, tui_app: &TuiApp, clients: &[ClientInfo], area: Rect) {
    let is_active = tui_app.active_panel == Panel::Clients;
    let border_color = if is_active {
        Color::Rgb(88, 166, 255)
    } else {
        Color::Rgb(48, 54, 61)
    };

    let title = format!(" Clients ({}) ", clients.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(Color::Rgb(240, 136, 62)),
        ))
        .style(Style::default().bg(Color::Rgb(13, 17, 23)));

    let rows: Vec<Row> = clients
        .iter()
        .map(|c| {
            let elapsed = c.last_seen.elapsed().as_secs();
            Row::new(vec![
                Cell::from(c.ip.to_string()).style(Style::default().fg(Color::Rgb(88, 166, 255))),
                Cell::from(c.user_agent.chars().take(30).collect::<String>())
                    .style(Style::default().fg(Color::Rgb(139, 148, 158))),
                Cell::from(format!("{elapsed}s ago"))
                    .style(Style::default().fg(Color::Rgb(110, 118, 129))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Min(20),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["IP", "User-Agent", "Last Seen"])
            .style(Style::default().fg(Color::Rgb(240, 136, 62)))
            .bottom_margin(1),
    )
    .block(block)
    .row_highlight_style(Style::default().bg(Color::Rgb(22, 27, 34)));

    let mut table_state = TableState::default();
    if !clients.is_empty() {
        table_state.select(Some(tui_app.client_scroll.min(clients.len() - 1)));
    }

    frame.render_stateful_widget(table, area, &mut table_state);
}

fn draw_downloads_panel(
    frame: &mut Frame,
    tui_app: &TuiApp,
    downloads: &[DownloadInfo],
    area: Rect,
) {
    let is_active = tui_app.active_panel == Panel::Downloads;
    let border_color = if is_active {
        Color::Rgb(88, 166, 255)
    } else {
        Color::Rgb(48, 54, 61)
    };

    let title = format!(" Downloads ({}) ", downloads.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(Color::Rgb(240, 136, 62)),
        ))
        .style(Style::default().bg(Color::Rgb(13, 17, 23)));

    let rows: Vec<Row> = downloads
        .iter()
        .map(|d| {
            let progress = if d.total_bytes > 0 {
                format!("{}%", d.bytes_sent * 100 / d.total_bytes)
            } else {
                "?".into()
            };
            let filename = d.path.rsplit('/').next().unwrap_or(&d.path);
            Row::new(vec![
                Cell::from(filename.to_string())
                    .style(Style::default().fg(Color::Rgb(126, 231, 135))),
                Cell::from(d.client_ip.to_string())
                    .style(Style::default().fg(Color::Rgb(88, 166, 255))),
                Cell::from(progress).style(Style::default().fg(Color::Rgb(240, 136, 62))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(16),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["File", "Client", "Progress"])
            .style(Style::default().fg(Color::Rgb(240, 136, 62)))
            .bottom_margin(1),
    )
    .block(block)
    .row_highlight_style(Style::default().bg(Color::Rgb(22, 27, 34)));

    let mut table_state = TableState::default();
    if !downloads.is_empty() {
        table_state.select(Some(tui_app.download_scroll.min(downloads.len() - 1)));
    }

    frame.render_stateful_widget(table, area, &mut table_state);
}

fn draw_footer(frame: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let uptime = state.format_uptime();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(48, 54, 61)))
        .style(Style::default().bg(Color::Rgb(13, 17, 23)));

    let line = Line::from(vec![
        Span::styled(" uptime: ", Style::default().fg(Color::Rgb(110, 118, 129))),
        Span::styled(uptime, Style::default().fg(Color::Rgb(126, 231, 135))),
        Span::styled(
            "   q: quit  tab: switch panel  \u{2191}\u{2193}: scroll",
            Style::default().fg(Color::Rgb(110, 118, 129)),
        ),
    ]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}
