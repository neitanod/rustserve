pub mod app;
pub mod ui;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui::Terminal;
use std::sync::Arc;

use crate::state::AppState;
use app::TuiApp;
use ui::draw_ui;

pub async fn run_tui(state: Arc<AppState>) -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut tui_app = TuiApp::new();

    loop {
        let clients: Vec<_> = state.clients.read().await.values().cloned().collect();
        let downloads: Vec<_> = state.downloads.read().await.values().cloned().collect();

        terminal.draw(|frame| {
            draw_ui(frame, &state, &tui_app, &clients, &downloads);
        })?;

        if event::poll(std::time::Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up => tui_app.scroll_up(),
                    KeyCode::Down => tui_app.scroll_down(),
                    KeyCode::Tab => tui_app.next_panel(),
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => tui_app.scroll_up(),
                    MouseEventKind::ScrollDown => tui_app.scroll_down(),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
