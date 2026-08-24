//! Symbol navigator list view.

use crate::tui::app::{ActivePane, AppState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

/// Renders the Symbol Navigator panel.
pub fn render_navigator(app: &AppState, area: Rect, buf: &mut Buffer) {
    let is_active = app.active_pane == ActivePane::Navigator;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if app.is_loading {
        " 📂 SYMBOLS [Scanning...] ".to_string()
    } else if app.search_query.is_empty() {
        format!(" 📂 SYMBOLS ({}) [/ Search] ", app.filtered_symbols.len())
    } else {
        format!(
            " 📂 SYMBOLS ({}/{}) [Filter: '{}'] ",
            app.filtered_symbols.len(),
            app.symbols.len(),
            app.search_query
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title)
        .title_style(
            Style::default()
                .fg(if is_active { Color::White } else { Color::Gray })
                .add_modifier(if is_active {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        );

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if app.is_loading {
        if inner.y < buf.area().bottom() {
            let max_w = inner.width.saturating_sub(2) as usize;
            let msg = if max_w > 30 {
                "⟳ Scanning workspace symbols..."
            } else {
                "⟳ Scanning..."
            };
            let truncated = super::truncate_chars(msg, max_w);
            buf.set_string(
                inner.x + 1,
                inner.y,
                truncated,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        }
        return;
    }

    if app.filtered_symbols.is_empty() {
        if inner.y < buf.area().bottom() {
            let max_w = inner.width.saturating_sub(2) as usize;
            let msg = if max_w > 30 {
                "No symbols found in workspace."
            } else {
                "No symbols"
            };
            let truncated = super::truncate_chars(msg, max_w);
            buf.set_string(
                inner.x + 1,
                inner.y,
                truncated,
                Style::default().fg(Color::DarkGray),
            );
        }
        return;
    }

    let visible_rows = inner.height as usize;
    let selected_pos = app.selected_symbol_idx;

    // Scroll window calculation with end-of-list clamping
    let raw_start = if selected_pos >= visible_rows {
        selected_pos.saturating_sub(visible_rows / 2)
    } else {
        0
    };
    let max_start = app.filtered_symbols.len().saturating_sub(visible_rows);
    let start_idx = raw_start.min(max_start);

    for (row, &sym_idx) in app
        .filtered_symbols
        .iter()
        .skip(start_idx)
        .take(visible_rows)
        .enumerate()
    {
        let y = inner.y + row as u16;
        if y >= inner.bottom() || y >= buf.area().bottom() {
            break;
        }

        let sym = &app.symbols[sym_idx];
        let is_selected = sym_idx
            == app
                .filtered_symbols
                .get(selected_pos)
                .copied()
                .unwrap_or(usize::MAX);

        let prefix = if is_selected { " > " } else { "   " };
        let sym_path_str = sym.file_path.to_string_lossy();
        let ws_root_str = app.workspace_root.to_string_lossy();
        let clean_sym_path = sym_path_str.strip_prefix(r"\\?\").unwrap_or(&sym_path_str);
        let clean_ws_root = ws_root_str.strip_prefix(r"\\?\").unwrap_or(&ws_root_str);
        let sym_norm = clean_sym_path.replace('\\', "/");
        let ws_norm = clean_ws_root.replace('\\', "/");
        let ws_norm_trimmed = ws_norm.trim_end_matches('/');

        let rel_path = if sym_norm
            .to_lowercase()
            .starts_with(&ws_norm_trimmed.to_lowercase())
        {
            sym_norm[ws_norm_trimmed.len()..]
                .trim_start_matches('/')
                .to_string()
        } else {
            sym_norm
        };

        let line_text = format!(
            "{prefix}{}:{}:{} ({})",
            rel_path, sym.line, sym.symbol_name, sym.kind
        );
        let max_w = inner.width as usize;
        let truncated = if line_text.chars().count() > max_w {
            format!(
                "{}…",
                super::truncate_chars(&line_text, max_w.saturating_sub(1))
            )
        } else {
            line_text
        };

        let item_style = if is_selected {
            Style::default()
                .bg(Color::Rgb(30, 50, 80))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::LightCyan)
        };

        buf.set_string(inner.x, y, &truncated, item_style);
    }
}
