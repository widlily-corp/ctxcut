//! Terminal keyboard and input event handler.

use crate::tui::app::AppState;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

/// Polls and handles terminal input events.
pub fn handle_events(app: &mut AppState, tick_rate: Duration) -> std::io::Result<()> {
    if event::poll(tick_rate)? {
        if let Event::Key(key) = event::read()? {
            handle_key_event(app, key);
        }
    }
    Ok(())
}

fn handle_key_event(app: &mut AppState, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    if app.is_searching {
        match key.code {
            KeyCode::Enter => {
                app.is_searching = false;
                app.apply_filter();
                if !app.filtered_symbols.is_empty() {
                    app.trigger_slice();
                }
            }
            KeyCode::Esc => {
                app.is_searching = false;
                app.search_query.clear();
                app.apply_filter();
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.apply_filter();
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.apply_filter();
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Tab => {
            app.active_pane = app.active_pane.next();
        }
        KeyCode::BackTab => {
            app.active_pane = app.active_pane.prev();
        }
        KeyCode::Char('/') => {
            app.is_searching = true;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.select_next();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.select_prev();
        }
        KeyCode::Enter => {
            app.trigger_slice();
        }
        KeyCode::Char('c') => {
            app.copy_slice_clipboard();
        }
        KeyCode::Char('i') => {
            app.trigger_impact();
        }
        KeyCode::Char('t') => {
            app.trigger_trace();
        }
        KeyCode::Char('r') => {
            app.refresh();
        }
        _ => {}
    }
}
