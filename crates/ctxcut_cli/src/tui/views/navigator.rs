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

    let title = if app.search_query.is_empty() {
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
                .add_modifier(if is_active { Modifier::BOLD } else { Modifier::empty() }),
        );

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height == 0 || app.filtered_symbols.is_empty() {
        if inner.height > 0 {
            buf.set_string(
                inner.x + 2,
                inner.y + 1,
                "No symbols found in workspace.",
                Style::default().fg(Color::DarkGray),
            );
        }
        return;
    }

    let visible_rows = inner.height as usize;
    let selected_pos = app.selected_symbol_idx;

    // Scroll window calculation
    let start_idx = if selected_pos >= visible_rows {
        selected_pos.saturating_sub(visible_rows / 2)
    } else {
        0
    };

    for (row, &sym_idx) in app
        .filtered_symbols
        .iter()
        .skip(start_idx)
        .take(visible_rows)
        .enumerate()
    {
        let sym = &app.symbols[sym_idx];
        let is_selected = sym_idx == app.filtered_symbols.get(selected_pos).copied().unwrap_or(usize::MAX);
        let y = inner.y + row as u16;

        let prefix = if is_selected { " > " } else { "   " };
        let rel_path = sym
            .file_path
            .strip_prefix(&app.workspace_root)
            .unwrap_or(&sym.file_path)
            .to_string_lossy()
            .replace('\\', "/");

        let line_text = format!("{prefix}{}:{}:{} ({})", rel_path, sym.line, sym.symbol_name, sym.kind);
        let max_w = inner.width as usize;
        let truncated = if line_text.len() > max_w {
            format!("{}…", &line_text[..max_w.saturating_sub(1)])
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
