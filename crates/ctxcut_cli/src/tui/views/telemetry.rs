//! Telemetry and Token ROI Dashboard view.

use crate::tui::app::{ActivePane, AppState};
use crate::tui::widgets::KpiCard;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

/// Renders the Telemetry & ROI Dashboard panel.
pub fn render_telemetry(app: &AppState, area: Rect, buf: &mut Buffer) {
    let is_active = app.active_pane == ActivePane::Telemetry;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" 📊 LIFETIME TELEMETRY & TOKEN ROI DASHBOARD ")
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

    let summary = &app.telemetry_summary;

    // Subdivide inner into top KPI row and bottom details
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(inner);

    // KPI Cards
    let kpi_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(chunks[0]);

    let tokens_str = format_token_count(summary.total_saved_tokens);
    let avg_reduction_str = format!("{:.1}% Avg Reduction", summary.compression_percentage);
    let kpi1 = KpiCard::new(
        "Tokens Saved",
        &tokens_str,
        &avg_reduction_str,
        Color::Green,
    );
    kpi1.render(kpi_chunks[0], buf);

    let standard_dollars = summary.cost_savings_by_tier.standard_sonnet_gpt4o;
    let standard_dollars_str = format!("${:.2}", standard_dollars);
    let kpi2 = KpiCard::new(
        "Standard ROI ($3/1M)",
        &standard_dollars_str,
        "Claude 3.5 Sonnet / GPT-4o",
        Color::Cyan,
    );
    kpi2.render(kpi_chunks[1], buf);

    let frontier_dollars = summary.cost_savings_by_tier.frontier_opus;
    let frontier_dollars_str = format!("${:.2}", frontier_dollars);
    let kpi3 = KpiCard::new(
        "Frontier ROI ($15/1M)",
        &frontier_dollars_str,
        "Claude 3.7 Opus / GPT-4",
        Color::Yellow,
    );
    kpi3.render(kpi_chunks[2], buf);

    // Bottom details: Pricing tiers + language breakdown
    if chunks[1].height > 0 {
        let mut lines = Vec::new();
        lines.push("─ Model Tier Pricing Savings ───────────────────────".to_string());
        lines.push(format!(
            "  • Economy  ($0.50/1M tk): ${:.2} (Haiku / 4o-mini)",
            summary.cost_savings_by_tier.economy_haiku_mini
        ));
        lines.push(format!(
            "  • Standard ($3.00/1M tk): ${:.2} (Sonnet / GPT-4o)",
            summary.cost_savings_by_tier.standard_sonnet_gpt4o
        ));
        lines.push(format!(
            "  • Frontier ($15.0/1M tk): ${:.2} (Opus / GPT-4)",
            summary.cost_savings_by_tier.frontier_opus
        ));

        if !summary.by_language.is_empty() {
            lines.push(String::new());
            lines.push("─ Language Breakdown ───────────────────────────────".to_string());
            for l in summary.by_language.iter().take(4) {
                let saved_fmt = format_token_count(l.saved_tokens);
                lines.push(format!(
                    "  • {:<12} {:>8} saved ({:>3} invocations)",
                    l.language, saved_fmt, l.requests
                ));
            }
        }

        let visible_rows = chunks[1].height as usize;
        let scroll = app.telemetry_scroll as usize;

        for (row, line) in lines.iter().skip(scroll).take(visible_rows).enumerate() {
            let y = chunks[1].y + row as u16;
            let max_w = chunks[1].width as usize;
            let truncated = if line.len() > max_w {
                &line[..max_w]
            } else {
                line.as_str()
            };

            let style = if line.starts_with('─') {
                Style::default().fg(Color::DarkGray)
            } else if line.contains('$') {
                Style::default().fg(Color::LightGreen)
            } else {
                Style::default().fg(Color::LightCyan)
            };

            buf.set_string(chunks[1].x + 1, y, truncated, style);
        }
    }
}

fn format_token_count(tokens: usize) -> String {
    if tokens < 1_000 {
        format!("{tokens}")
    } else if tokens < 1_000_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{:.2}M", tokens as f64 / 1_000_000.0)
    }
}
