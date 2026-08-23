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
                .add_modifier(if is_active { Modifier::BOLD } else { Modifier::empty() }),
        );

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height == 0 {
        return;
    }

    let mut lines = Vec::new();

    if let Some(ref callers) = app.current_impact {
        lines.push(format!("▲ Upstream Callers ({}):", callers.len()));
        if callers.is_empty() {
            lines.push("  • (No external callers detected in workspace)".to_string());
        } else {
            for c in callers {
                let rel = std::path::Path::new(&c.file_path)
                    .strip_prefix(&app.workspace_root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| c.file_path.clone())
                    .replace('\\', "/");
                lines.push(format!("  • {}:{} in `{}`", rel, c.line_number, c.caller_symbol));
                if !c.call_snippet.is_empty() {
                    lines.push(format!("    └─ {}", c.call_snippet.trim()));
                }
            }
        }
    } else if let Some(ref trace) = app.current_trace {
        lines.push(format!("▼ Execution Flow Trace ({} hops):", trace.steps.len()));
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
    let scroll = app.impact_scroll as usize;

    for (row, line) in lines.iter().skip(scroll).take(visible_rows).enumerate() {
        let y = inner.y + row as u16;
        let max_w = inner.width as usize;
        let truncated = if line.len() > max_w {
            &line[..max_w]
        } else {
            line.as_str()
        };

        let style = if line.starts_with('▲') || line.starts_with('▼') {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if line.contains("└─") || line.starts_with("Press") {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::LightCyan)
        };

        buf.set_string(inner.x + 1, y, truncated, style);
    }
}
