//! Studio header banner view.

use crate::tui::app::AppState;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

/// Renders the top title and workspace header bar.
pub fn render_header(app: &AppState, area: Rect, buf: &mut Buffer) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height >= 1 && inner.width > 0 && inner.y < buf.area().bottom() {
        let title = " ⚡ CTXCUT v2.0 AST CONTEXT STUDIO & TELEMETRY ";
        let max_w = inner.width.saturating_sub(1) as usize;
        let truncated_title = super::truncate_chars(title, max_w);
        buf.set_string(
            inner.x,
            inner.y,
            truncated_title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

        let ws_display = app.workspace_root.to_string_lossy();
        let clean_ws = ws_display.strip_prefix(r"\\?\").unwrap_or(&ws_display);
        let status_tag = if app.is_loading { "[SCANNING] " } else { "" };
        let ws_str = format!("{}[WORKSPACE: {}]", status_tag, clean_ws);
        let ws_chars = ws_str.chars().count() as u16;
        let title_chars = truncated_title.chars().count() as u16;
        let ws_x = inner.x + inner.width.saturating_sub(ws_chars + 1);
        if ws_x > inner.x + title_chars + 1 {
            buf.set_string(
                ws_x,
                inner.y,
                &ws_str,
                Style::default().fg(if app.is_loading {
                    Color::Yellow
                } else {
                    Color::Gray
                }),
            );
        }
    }
}
