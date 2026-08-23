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

    if inner.height >= 1 {
        let title = " ⚡ CTXCUT v2.0 AST CONTEXT STUDIO & TELEMETRY ";
        buf.set_string(
            inner.x + 1,
            inner.y,
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        );

        let ws_str = format!("[WORKSPACE: {}]", app.workspace_root.display());
        let ws_x = inner.x + inner.width.saturating_sub(ws_str.len() as u16 + 2);
        if ws_x > inner.x + title.len() as u16 {
            buf.set_string(
                ws_x,
                inner.y,
                &ws_str,
                Style::default().fg(Color::Gray),
            );
        }
    }
}
