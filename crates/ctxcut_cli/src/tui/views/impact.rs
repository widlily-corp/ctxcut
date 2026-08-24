//! Caller and impact graph visualization panel.

use crate::tui::app::{ActivePane, AppState};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

/// Renders the Caller & Impact Graph panel.
pub fn render_impact(app: &AppState, area: Rect, buf: &mut Buffer) {
    let is_active = app.active_pane == ActivePane::Impact;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" 🌐 CALLER & IMPACT GRAPH [i] ")
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

    let mut lines = Vec::new();

    if let Some(ref callers) = app.current_impact {
        lines.push(format!("▲ Upstream Callers ({}):", callers.len()));
        if callers.is_empty() {
            lines.push("  • (No external callers detected in workspace)".to_string());
        } else {
            for c in callers {
                let caller_path_str = &c.file_path;
                let ws_root_str = app.workspace_root.to_string_lossy();
                let clean_caller = caller_path_str
                    .strip_prefix(r"\\?\")
                    .unwrap_or(caller_path_str);
                let clean_ws_root = ws_root_str.strip_prefix(r"\\?\").unwrap_or(&ws_root_str);
                let caller_norm = clean_caller.replace('\\', "/");
                let ws_norm = clean_ws_root.replace('\\', "/");
                let ws_norm_trimmed = ws_norm.trim_end_matches('/');

                let rel = if caller_norm
                    .to_lowercase()
                    .starts_with(&ws_norm_trimmed.to_lowercase())
                {
                    caller_norm[ws_norm_trimmed.len()..]
                        .trim_start_matches('/')
                        .to_string()
                } else {
                    caller_norm
                };
                lines.push(format!(
                    "  • {}:{} in `{}`",
                    rel, c.line_number, c.caller_symbol
                ));
                if !c.call_snippet.is_empty() {
                    lines.push(format!("    └─ {}", c.call_snippet.trim()));
                }
            }
        }
    } else if let Some(ref trace) = app.current_trace {
        lines.push(format!(
            "▼ Execution Flow Trace ({} hops):",
            trace.steps.len()
        ));
        for (i, step) in trace.steps.iter().enumerate() {
            let next_info = step
                .next_target
                .as_deref()
                .map(|tgt| format!(" -> {tgt}"))
                .unwrap_or_default();
            lines.push(format!(
                "  {}. [{}] {}{}",
                i + 1,
                step.kind,
                step.symbol_name,
                next_info
            ));
        }
    } else {
        lines.push("Press [i] to trace upstream callers of selected symbol.".to_string());
        lines.push("Press [t] to run execution flow trace.".to_string());
    }

    let visible_rows = inner.height as usize;
    let max_scroll = lines.len().saturating_sub(visible_rows);
    let scroll = (app.impact_scroll as usize).min(max_scroll);

    for (row, line) in lines.iter().skip(scroll).take(visible_rows).enumerate() {
        let y = inner.y + row as u16;
        if y >= inner.bottom() || y >= buf.area().bottom() {
            break;
        }

        let max_w = inner.width.saturating_sub(2) as usize;
        let truncated = super::truncate_chars(line, max_w);

        let style = if line.starts_with('▲') || line.starts_with('▼') {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if line.contains("└─") || line.starts_with("Press") {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::LightCyan)
        };

        buf.set_string(inner.x + 1, y, truncated, style);
    }
}
