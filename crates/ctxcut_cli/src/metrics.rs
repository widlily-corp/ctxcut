//! Interactive terminal dashboard and ROI metrics visualization.
//!
//! Provides a high-density, beautifully styled terminal dashboard per Titan Core standards,
//! displaying lifetime token savings, economic ROI, language breakdowns, and recent slicing activity.

use anyhow::Result;
use colored::Colorize;
use ctxcut_core::{TelemetryLogger, TelemetrySummary};
use std::path::Path;

/// Formats an integer with thousands separator commas (e.g. 4821390 -> "4,821,390").
pub fn format_number(num: usize) -> String {
    let s = num.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();

    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Formats a float as USD currency (e.g. 14.46 -> "$14.46").
pub fn format_currency(val: f64) -> String {
    format!("${:.2}", val)
}

/// Formats a float percentage (e.g. 84.25 -> "84.3%").
pub fn format_percentage(val: f64) -> String {
    format!("{:.1}%", val)
}

/// Renders the complete terminal dashboard for telemetry summary.
pub fn render_dashboard(summary: &TelemetrySummary, metrics_path: &Path) -> String {
    let mut out = String::new();
    let path_display = metrics_path.display().to_string();

    out.push_str(
        "┌─────────────────────────────────────────────────────────────────────────────┐\n",
    );
    out.push_str(
        "│  ⚡ CTXCUT TELEMETRY & TOKEN SAVINGS DASHBOARD                              │\n",
    );
    out.push_str(
        "│  Lifetime ROI & Context Optimization Analytics                              │\n",
    );
    out.push_str(
        "├─────────────────────────────────────────────────────────────────────────────┤\n",
    );

    if summary.total_requests == 0 {
        out.push_str(&format!(
            "│  No telemetry data recorded yet in {:<40} │\n",
            path_display
        ));
        out.push_str(
            "│                                                                             │\n",
        );
        out.push_str(
            "│  Start saving tokens by running:                                            │\n",
        );
        out.push_str(
            "│    • ctxcut slice <file:symbol>                                             │\n",
        );
        out.push_str(
            "│    • ctxcut diff                                                            │\n",
        );
        out.push_str(
            "│    • or connect AI agents via MCP (ctxcut mcp)                              │\n",
        );
        out.push_str(
            "└─────────────────────────────────────────────────────────────────────────────┘\n",
        );
        return out;
    }

    // Top Summary KPI Cards
    out.push_str(
        "│                                                                             │\n",
    );
    out.push_str(
        "│   TOTAL REQUESTS      TOKENS SAVED         AVG REDUCTION     EST. SAVINGS   │\n",
    );
    out.push_str(
        "│   ──────────────      ────────────         ─────────────     ────────────   │\n",
    );

    let req_str = format_number(summary.total_requests);
    let saved_str = format_number(summary.total_saved_tokens);
    let pct_str = format_percentage(summary.compression_percentage);
    let cost_str = format_currency(summary.estimated_cost_savings_usd);

    out.push_str(&format!(
        "│   {:<19} {:<20} {:<17} {:<13} │\n",
        req_str, saved_str, pct_str, cost_str
    ));
    out.push_str(
        "│                                                                             │\n",
    );

    // Language Breakdown Table
    if !summary.by_language.is_empty() {
        out.push_str(
            "├─────────────────────────────────────────────────────────────────────────────┤\n",
        );
        out.push_str(
            "│  📊 LANGUAGE BREAKDOWN                                                      │\n",
        );
        out.push_str(
            "├─────────────────────────────────────────────────────────────────────────────┤\n",
        );
        out.push_str(
            "│  LANGUAGE      REQUESTS     RAW TOKENS     SLICED TOKENS   SAVED       PCT  │\n",
        );
        out.push_str(
            "│  ────────      ────────     ──────────     ─────────────   ─────       ───  │\n",
        );

        for lang in &summary.by_language {
            let l_name = if lang.language.len() > 14 {
                &lang.language[..14]
            } else {
                &lang.language
            };
            let l_reqs = format_number(lang.requests);
            let l_raw = format_number(lang.raw_tokens);
            let l_sliced = format_number(lang.sliced_tokens);
            let l_saved = format_number(lang.saved_tokens);
            let l_pct = format_percentage(lang.savings_percentage);

            out.push_str(&format!(
                "│  {:<14} {:>8} {:>14} {:>17} {:>11} {:>5} │\n",
                l_name, l_reqs, l_raw, l_sliced, l_saved, l_pct
            ));
        }
    }

    // Invocation Sources
    if !summary.by_source.is_empty() {
        out.push_str(
            "├─────────────────────────────────────────────────────────────────────────────┤\n",
        );
        out.push_str(
            "│  🔌 INVOCATION SOURCES                                                      │\n",
        );
        out.push_str(
            "├─────────────────────────────────────────────────────────────────────────────┤\n",
        );

        for src in &summary.by_source {
            let label = match src.source.as_str() {
                "mcp_get_symbol_slice" => "MCP Server (get_symbol_slice)",
                "mcp_get_diff_slice" => "MCP Server (get_diff_slice)",
                "cli_slice" => "CLI (ctxcut slice)",
                "cli_diff" => "CLI (ctxcut diff)",
                "cli_route" => "CLI (ctxcut route)",
                other => other,
            };
            let calls_str = format!("{} calls", format_number(src.requests));
            let saved_str = format!(
                "{} saved ({})",
                format_number(src.saved_tokens),
                format_percentage(src.savings_percentage)
            );
            out.push_str(&format!(
                "│  • {:<31} {:>11} │ {:<27} │\n",
                label, calls_str, saved_str
            ));
        }
    }

    // Recent Activity Log
    if !summary.recent_events.is_empty() {
        out.push_str(
            "├─────────────────────────────────────────────────────────────────────────────┤\n",
        );
        let count = summary.recent_events.len().min(5);
        out.push_str(&format!(
            "│  🕒 RECENT ACTIVITY (LAST {} SLICES)                                         │\n",
            count
        ));
        out.push_str(
            "├─────────────────────────────────────────────────────────────────────────────┤\n",
        );

        for ev in summary.recent_events.iter().take(5) {
            // Timestamp short display: "YYYY-MM-DD HH:MM"
            let time_short = if ev.timestamp.len() >= 16 {
                ev.timestamp[..16].replace('T', " ")
            } else {
                ev.timestamp.clone()
            };

            let sym_full = format!("{}:{}", ev.file_path, ev.symbol);
            let sym_truncated = if sym_full.len() > 32 {
                format!("...{}", &sym_full[sym_full.len() - 29..])
            } else {
                sym_full
            };

            let raw_s = format_number(ev.raw_tokens);
            let sliced_s = format_number(ev.sliced_tokens);
            let pct_s = format_percentage(ev.savings_percentage);
            let flow_s = format!("{} -> {}", raw_s, sliced_s);

            out.push_str(&format!(
                "│  {:<16} {:<32} {:>13}  ({:>6})  │\n",
                time_short, sym_truncated, flow_s, pct_s
            ));
        }
    }

    out.push_str(
        "└─────────────────────────────────────────────────────────────────────────────┘\n",
    );
    out.push_str(&format!(
        "  {} {}\n",
        "Telemetry file:".dimmed(),
        path_display.dimmed()
    ));
    out.push_str(&format!(
        "  {} {}\n",
        "Pricing model: ".dimmed(),
        "$3.00 / 1M prompt tokens (Claude 3.5 Sonnet / GPT-4o)".dimmed()
    ));

    out
}

/// Executes the metrics command, printing the dashboard or JSON output.
pub fn run_metrics_command(format: &str) -> Result<()> {
    let metrics_path = TelemetryLogger::resolve_metrics_path();
    let summary = TelemetryLogger::load_summary_from_path(&metrics_path)
        .unwrap_or_else(|_| TelemetryLogger::aggregate(&[]));

    if format.eq_ignore_ascii_case("json") {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("{}", render_dashboard(&summary, &metrics_path));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctxcut_core::TelemetryEvent;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    #[test]
    fn test_empty_dashboard_rendering() {
        let summary = TelemetryLogger::aggregate(&[]);
        let path = Path::new(".ctxcut/metrics.jsonl");
        let rendered = render_dashboard(&summary, path);
        assert!(rendered.contains("CTXCUT TELEMETRY & TOKEN SAVINGS DASHBOARD"));
        assert!(rendered.contains("No telemetry data recorded yet"));
    }

    #[test]
    fn test_populated_dashboard_rendering() {
        let events = vec![
            TelemetryEvent {
                timestamp: "2026-08-16T12:00:00Z".to_string(),
                file_path: "src/auth.ts".to_string(),
                symbol: "login".to_string(),
                language: Some("typescript".to_string()),
                raw_tokens: 1000,
                sliced_tokens: 150,
                saved_tokens: 850,
                savings_percentage: 85.0,
                raw_lines: 80,
                sliced_lines: 15,
                source: Some("cli_slice".to_string()),
                duration_ms: Some(5),
            },
            TelemetryEvent {
                timestamp: "2026-08-16T12:05:00Z".to_string(),
                file_path: "api/routes.py".to_string(),
                symbol: "get_user".to_string(),
                language: Some("python".to_string()),
                raw_tokens: 2000,
                sliced_tokens: 300,
                saved_tokens: 1700,
                savings_percentage: 85.0,
                raw_lines: 120,
                sliced_lines: 25,
                source: Some("mcp_get_symbol_slice".to_string()),
                duration_ms: Some(8),
            },
        ];

        let summary = TelemetryLogger::aggregate(&events);
        let path = Path::new("~/.ctxcut/metrics.jsonl");
        let rendered = render_dashboard(&summary, path);

        assert!(rendered.contains("TOTAL REQUESTS"));
        assert!(rendered.contains("TOKENS SAVED"));
        assert!(rendered.contains("2,550"));
        assert!(rendered.contains("TypeScript"));
        assert!(rendered.contains("Python"));
        assert!(rendered.contains("CLI (ctxcut slice)"));
        assert!(rendered.contains("MCP Server (get_symbol_slice)"));
        assert!(rendered.contains("src/auth.ts:login"));
    }
}
