//! Interactive Terminal UI (TUI) Dashboard for AST slicing and telemetry.
//!
//! Provides a 4-pane layout:
//! 1. Symbol Navigator list
//! 2. Live AST Slice preview
//! 3. Caller / Callee Impact graph
//! 4. Token & Dollar ROI Telemetry dashboard ($3.00 Standard, $15.00 Frontier, $0.50 Economy)

pub mod app;
pub mod events;
pub mod views;
pub mod widgets;

pub use app::{ActivePane, AppState, SymbolEntry};
pub use events::handle_events;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::time::Duration;

/// RAII Terminal mode guard ensuring cleanup on exit or panic.
pub struct TerminalGuard;

impl TerminalGuard {
    /// Initializes terminal raw mode and alternate screen.
    pub fn init() -> io::Result<TerminalGuard> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

/// Runs the interactive TUI application.
pub fn run_tui(workspace_root: Option<PathBuf>) -> Result<()> {
    // Headless / non-TTY fallback
    if !io::stdout().is_terminal() {
        return crate::metrics::run_metrics_command("text");
    }

    let ws_root = workspace_root.unwrap_or_else(|| PathBuf::from("."));
    let ws_root = ws_root.canonicalize().unwrap_or(ws_root);

    let _guard = TerminalGuard::init()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut app = AppState::new(ws_root);
    let tick_rate = Duration::from_millis(50);

    while !app.should_quit {
        terminal.draw(|f| {
            let size = f.area();

            // Main vertical layout: Header, Content Grid, Status Bar
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(8),
                    Constraint::Length(1),
                ])
                .split(size);

            // Render Header
            views::render_header(&app, vertical_chunks[0], f.buffer_mut());

            // Horizontal Grid split: Left Column vs Right Column
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(vertical_chunks[1]);

            // Left Column: Top Navigator (60%), Bottom Impact (40%)
            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(horizontal_chunks[0]);

            views::render_navigator(&app, left_chunks[0], f.buffer_mut());
            views::render_impact(&app, left_chunks[1], f.buffer_mut());

            // Right Column: Top Preview (55%), Bottom Telemetry (45%)
            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(horizontal_chunks[1]);

            views::render_preview(&app, right_chunks[0], f.buffer_mut());
            views::render_telemetry(&app, right_chunks[1], f.buffer_mut());

            // Render Status Bar
            let status_bar = widgets::StatusBar::new(
                &app.status_message,
                app.is_searching,
                &app.search_query,
            );
            f.render_widget(status_bar, vertical_chunks[2]);
        })?;

        handle_events(&mut app, tick_rate)?;
    }

    Ok(())
}
