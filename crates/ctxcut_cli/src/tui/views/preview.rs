//! AST Slice live preview panel.

use crate::tui::app::{ActivePane, AppState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

/// Renders the AST Slice live preview panel.
pub fn render_preview(app: &AppState, area: Rect, buf: &mut Buffer) {
    let is_active = app.active_pane == ActivePane::Preview;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if let Some(ref slice) = app.current_slice {
        format!(
            " 🔬 AST SLICE LIVE PREVIEW [{}: {} | -{:.1}%] ",
            slice.target_symbol.name, slice.target_symbol.language, slice.stats.savings_percentage
        )
    } else {
        " 🔬 AST SLICE LIVE PREVIEW ".to_string()
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
            let msg = if max_w > 40 {
                "⚡ Discovering AST context and indexing workspace..."
            } else {
                "⚡ Indexing..."
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

    let Some(ref slice) = app.current_slice else {
        if inner.y < buf.area().bottom() {
            let max_w = inner.width.saturating_sub(2) as usize;
            let msg = if max_w > 40 {
                "Press [Enter] on a symbol to generate AST context slice."
            } else {
                "Press [Enter] to slice"
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
    };

    // Construct preview lines
    let mut lines = Vec::new();

    // Target definition
    lines.push(format!("// Target Symbol: {}", slice.target_symbol.name));
    if let Some(ref doc) = slice.target_symbol.doc_comment {
        for l in doc.lines() {
            lines.push(format!("// {l}"));
        }
    }
    for l in slice.target_symbol.body.lines() {
        lines.push(l.to_string());
    }

    // Hoisted Types
    if !slice.hoisted_types.is_empty() {
        lines.push(String::new());
        lines.push("/* ── Hoisted Types ────────────────────────────── */".to_string());
        for t in &slice.hoisted_types {
            for l in t.definition.lines() {
                lines.push(l.to_string());
            }
        }
    }

    // Hoisted Implementors
    if !slice.hoisted_implementors.is_empty() {
        lines.push(String::new());
        lines.push("/* ── Hoisted Implementors ────────────────────── */".to_string());
        for imp in &slice.hoisted_implementors {
            for l in imp.definition.lines() {
                lines.push(l.to_string());
            }
        }
    }

    // Stripped Signatures
    if !slice.stripped_calls.is_empty() {
        lines.push(String::new());
        lines.push("/* ── Stripped Signatures ─────────────────────── */".to_string());
        for c in &slice.stripped_calls {
            lines.push(c.signature.clone());
        }
    }

    let visible_rows = inner.height as usize;
    let max_scroll = lines.len().saturating_sub(visible_rows);
    let scroll = (app.preview_scroll as usize).min(max_scroll);

    for (row, line) in lines.iter().skip(scroll).take(visible_rows).enumerate() {
        let y = inner.y + row as u16;
        if y >= inner.bottom() || y >= buf.area().bottom() {
            break;
        }

        let max_w = inner.width.saturating_sub(2) as usize;
        let truncated = super::truncate_chars(line, max_w);

        let style = if line.starts_with("/*") || line.starts_with("//") {
            Style::default().fg(Color::DarkGray)
        } else if line.starts_with("export")
            || line.starts_with("pub")
            || line.starts_with("def")
            || line.starts_with("func")
        {
            Style::default().fg(Color::LightBlue)
        } else {
            Style::default().fg(Color::White)
        };

        buf.set_string(inner.x + 1, y, truncated, style);
    }
}
