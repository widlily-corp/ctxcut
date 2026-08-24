//! Terminal keyboard and input event handler.

use crate::tui::app::AppState;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

/// Polls and handles terminal input events.
pub fn handle_events(app: &mut AppState, tick_rate: Duration) -> std::io::Result<()> {
    app.tick();

    if event::poll(tick_rate)? {
        loop {
            let ev = event::read()?;
            match ev {
                Event::Key(key) => {
                    if key.kind != event::KeyEventKind::Release {
                        handle_key_event(app, key);
                    }
                }
                Event::Resize(_w, _h) => {
                    // Windows Terminal resize event: no-op, ratatui auto-adjusts next draw frame
                }
                Event::FocusGained | Event::FocusLost => {
                    // Windows Terminal focus events: ignored safely without freezing
                }
                Event::Paste(text) if app.is_searching => {
                    app.search_query.push_str(&text);
                    app.apply_filter();
                }
                Event::Mouse(_) | Event::Paste(_) => {}
            }

            if app.should_quit || !event::poll(Duration::from_millis(0))? {
                break;
            }
        }
    }
    Ok(())
}

/// Dispatches a key event to the active TUI state machine.
pub fn handle_key_event(app: &mut AppState, key: KeyEvent) {
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
            KeyCode::Down => {
                app.select_next();
            }
            KeyCode::Up => {
                app.select_prev();
            }
            KeyCode::PageDown => {
                app.select_page_down();
            }
            KeyCode::PageUp => {
                app.select_page_up();
            }
            KeyCode::Home => {
                app.select_first();
            }
            KeyCode::End => {
                app.select_last();
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
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::BackTab => {
            app.active_pane = app.active_pane.prev();
        }
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.active_pane = app.active_pane.prev();
        }
        KeyCode::Tab => {
            app.active_pane = app.active_pane.next();
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
        KeyCode::PageDown => {
            app.select_page_down();
        }
        KeyCode::PageUp => {
            app.select_page_up();
        }
        KeyCode::Home => {
            app.select_first();
        }
        KeyCode::End => {
            app.select_last();
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
