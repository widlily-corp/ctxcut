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
pub use events::{handle_events, handle_key_event};

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

/// RAII Terminal mode guard ensuring cleanup on exit or panic.
pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    /// Initializes terminal raw mode, alternate screen, hides cursor, and installs emergency panic hook.
    pub fn init() -> io::Result<TerminalGuard> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(e) = execute!(
            stdout,
            EnterAlternateScreen,
            crossterm::cursor::Hide,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        ) {
            let _ = disable_raw_mode();
            return Err(e);
        }
        let _ = stdout.flush();

        // Install emergency panic hook to cleanly restore terminal on any unhandled panics
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, crossterm::cursor::Show, LeaveAlternateScreen);
            let _ = stdout.flush();
            let _ = disable_raw_mode();
            default_hook(panic_info);
        }));

        Ok(TerminalGuard { active: true })
    }

    /// Explicitly restores standard terminal state and cursor visibility.
    pub fn restore(&mut self) {
        if self.active {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, crossterm::cursor::Show, LeaveAlternateScreen);
            let _ = stdout.flush();
            let _ = disable_raw_mode();
            self.active = false;
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Renders the complete TUI dashboard layout into the given frame.
pub fn render_dashboard(f: &mut Frame<'_>, app: &AppState) {
    let size = f.area();
    if size.width < 20 || size.height < 5 {
        let msg = if size.width >= 12 && size.height >= 1 {
            "Size < 20x5"
        } else {
            "..."
        };
        if size.width > 0 && size.height > 0 {
            let truncated_msg = views::truncate_chars(msg, size.width as usize);
            f.buffer_mut().set_string(
                size.x,
                size.y,
                truncated_msg,
                ratatui::style::Style::default().fg(ratatui::style::Color::Yellow),
            );
        }
        return;
    }

    // Main vertical layout: Header, Content Grid, Status Bar
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(size);

    // Render Header
    views::render_header(app, vertical_chunks[0], f.buffer_mut());

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

    views::render_navigator(app, left_chunks[0], f.buffer_mut());
    views::render_impact(app, left_chunks[1], f.buffer_mut());

    // Right Column: Top Preview (55%), Bottom Telemetry (45%)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(horizontal_chunks[1]);

    views::render_preview(app, right_chunks[0], f.buffer_mut());
    views::render_telemetry(app, right_chunks[1], f.buffer_mut());

    // Render Status Bar
    let status_bar =
        widgets::StatusBar::new(&app.status_message, app.is_searching, &app.search_query);
    f.render_widget(status_bar, vertical_chunks[2]);
}

/// Runs the interactive TUI application.
pub fn run_tui(workspace_root: Option<PathBuf>) -> Result<()> {
    let ws_root = workspace_root.unwrap_or_else(|| PathBuf::from("."));
    let ws_root = ws_root.canonicalize().unwrap_or(ws_root);

    // Check if terminal dimensions meet minimum required interactive size (20x5)
    if let Ok((width, height)) = crossterm::terminal::size() {
        if width < 20 || height < 5 {
            eprintln!(
                "Warning: Terminal dimensions ({}x{}) are too small for interactive TUI (minimum required: 20x5).",
                width, height
            );
            eprintln!("Displaying dashboard metrics summary:\n");
            return crate::metrics::run_metrics_command("text");
        }
    }

    let mut guard = match TerminalGuard::init() {
        Ok(g) => g,
        Err(e) => {
            eprintln!(
                "Interactive TUI requires a standard terminal console with raw mode support (error: {}). Displaying dashboard metrics summary:\n",
                e
            );
            return crate::metrics::run_metrics_command("text");
        }
    };

    let mut terminal = match Terminal::new(CrosstermBackend::new(io::stdout())) {
        Ok(t) => t,
        Err(e) => {
            guard.restore();
            eprintln!(
                "Failed to initialize Ratatui terminal backend (error: {}). Displaying dashboard metrics summary:\n",
                e
            );
            return crate::metrics::run_metrics_command("text");
        }
    };

    let _ = terminal.clear();

    let mut app = AppState::new(ws_root);
    let tick_rate = Duration::from_millis(50);

    let run_res = (|| -> Result<()> {
        while !app.should_quit {
            terminal.draw(|f| render_dashboard(f, &app))?;
            handle_events(&mut app, tick_rate)?;
        }
        Ok(())
    })();

    guard.restore();
    run_res
}
