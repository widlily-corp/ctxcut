//! Persistent token savings telemetry logger and metrics analytics engine.
//!
//! Automatically records AST context slicing events into an append-only JSON Lines file (`~/.ctxcut/metrics.jsonl`)
//! and computes aggregated ROI statistics, compression rates, language breakdowns, and estimated API cost savings.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::model::SliceResult;

/// Baseline prompt token cost for standard coding agent models ($3.00 per 1,000,000 tokens).
pub const STANDARD_PRICE_PER_MILLION_TOKENS: f64 = 3.00;
/// Frontier model prompt token cost ($15.00 per 1,000,000 tokens).
pub const FRONTIER_PRICE_PER_MILLION_TOKENS: f64 = 15.00;
/// Economy model prompt token cost ($0.50 per 1,000,000 tokens).
pub const ECONOMY_PRICE_PER_MILLION_TOKENS: f64 = 0.50;

/// Telemetry record representing a single AST slicing invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryEvent {
    /// UTC timestamp in RFC 3339 format (`YYYY-MM-DDTHH:MM:SSZ`).
    pub timestamp: String,
    /// Path to target source file.
    pub file_path: String,
    /// Extracted symbol name or comma-separated symbol list.
    pub symbol: String,
    /// Programming language identifier (e.g. `typescript`, `python`, `go`, `rust`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Full source file token count.
    pub raw_tokens: usize,
    /// Extracted slice token count.
    pub sliced_tokens: usize,
    /// Tokens saved: `raw_tokens.saturating_sub(sliced_tokens)`.
    pub saved_tokens: usize,
    /// Percentage reduction: `(saved_tokens / raw_tokens) * 100.0`.
    #[serde(default)]
    pub savings_percentage: f64,
    /// Total lines in raw file.
    #[serde(default)]
    pub raw_lines: usize,
    /// Total lines in generated slice.
    #[serde(default)]
    pub sliced_lines: usize,
    /// Source invocation channel (`cli_slice`, `cli_diff`, `cli_route`, `mcp_get_symbol_slice`, `mcp_get_diff_slice`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Slicing execution latency in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Aggregated telemetry metrics for a specific programming language.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageMetric {
    /// Language name (e.g., "TypeScript", "Python", "Go", "Rust").
    pub language: String,
    /// Total slicing requests for this language.
    pub requests: usize,
    /// Total raw tokens ingested.
    pub raw_tokens: usize,
    /// Total sliced tokens delivered.
    pub sliced_tokens: usize,
    /// Total tokens saved.
    pub saved_tokens: usize,
    /// Average reduction percentage for this language.
    pub savings_percentage: f64,
    /// Estimated cost savings in USD for this language.
    pub estimated_cost_savings_usd: f64,
}

/// Aggregated telemetry metrics for an invocation source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceMetric {
    /// Invocation source identifier.
    pub source: String,
    /// Total requests from this source.
    pub requests: usize,
    /// Total tokens saved.
    pub saved_tokens: usize,
    /// Average savings percentage.
    pub savings_percentage: f64,
}

/// Multi-tier pricing cost comparisons.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModelTierSavings {
    /// Standard tier (Claude 3.5 Sonnet / GPT-4o: $3.00 / 1M tokens).
    pub standard_sonnet_gpt4o: f64,
    /// Frontier tier (Claude 3.7 Opus / GPT-4: $15.00 / 1M tokens).
    pub frontier_opus: f64,
    /// Economy tier (Claude 3.5 Haiku / GPT-4o-mini: $0.50 / 1M tokens).
    pub economy_haiku_mini: f64,
}

/// Comprehensive aggregated summary of lifetime token savings and ROI metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TelemetrySummary {
    /// Total slicing operations recorded.
    pub total_requests: usize,
    /// Total raw tokens ingested.
    pub total_raw_tokens: usize,
    /// Total sliced tokens delivered.
    pub total_sliced_tokens: usize,
    /// Total tokens saved.
    pub total_saved_tokens: usize,
    /// Lifetime context compression percentage: `(total_saved / total_raw) * 100.0`.
    pub compression_percentage: f64,
    /// Estimated LLM API cost savings in USD (at $3.00 / 1M tokens saved).
    pub estimated_cost_savings_usd: f64,
    /// Multi-tier pricing comparisons.
    pub cost_savings_by_tier: ModelTierSavings,
    /// Language breakdown mapping language identifier to saved token count.
    pub language_breakdown: BTreeMap<String, usize>,
    /// Detailed per-language metrics.
    pub by_language: Vec<LanguageMetric>,
    /// Detailed per-source metrics.
    pub by_source: Vec<SourceMetric>,
    /// Chronological recent activity list (most recent events, newest first).
    pub recent_events: Vec<TelemetryEvent>,
}

/// Formats a `SystemTime` into an ISO 8601 / RFC 3339 UTC timestamp string (`YYYY-MM-DDTHH:MM:SSZ`).
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn format_rfc3339(system_time: SystemTime) -> String {
    let duration = system_time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let total_secs = duration.as_secs();

    let days = (total_secs / 86_400) as i64;
    let day_secs = (total_secs % 86_400) as u32;

    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    // Howard Hinnant algorithm for Gregorian calendar calculation from epoch days
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = i64::from(yoe) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let final_y = if m <= 2 { y + 1 } else { y };

    format!("{final_y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Returns the current UTC timestamp formatted as an RFC 3339 string.
pub fn current_rfc3339_timestamp() -> String {
    format_rfc3339(SystemTime::now())
}

/// Persistent telemetry logger for recording and aggregating token savings.
pub struct TelemetryLogger;

impl TelemetryLogger {
    /// Resolves the file path for the telemetry metrics log file.
    ///
    /// Resolves in order:
    /// 1. `CTXCUT_METRICS_FILE` environment variable.
    /// 2. `CTXCUT_DIR` or `CTXCUT_HOME` environment variable (`<DIR>/metrics.jsonl`).
    /// 3. `HOME` or `USERPROFILE` environment variable (`~/.ctxcut/metrics.jsonl`).
    /// 4. Current working directory fallback (`.ctxcut/metrics.jsonl`).
    pub fn resolve_metrics_path() -> PathBuf {
        if let Some(file_override) = env::var_os("CTXCUT_METRICS_FILE") {
            if !file_override.is_empty() {
                return PathBuf::from(file_override);
            }
        }

        if let Some(dir_override) = env::var_os("CTXCUT_DIR").or_else(|| env::var_os("CTXCUT_HOME"))
        {
            if !dir_override.is_empty() {
                return PathBuf::from(dir_override).join("metrics.jsonl");
            }
        }

        let home_opt = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"));
        if let Some(home) = home_opt {
            if !home.is_empty() {
                return PathBuf::from(home).join(".ctxcut").join("metrics.jsonl");
            }
        }

        PathBuf::from(".ctxcut").join("metrics.jsonl")
    }

    /// Records a telemetry event safely to the default metrics file.
    ///
    /// Fail-safe: any I/O or serialization error is ignored and will never panic or halt execution.
    pub fn record_event(event: &TelemetryEvent) {
        let path = Self::resolve_metrics_path();
        Self::record_event_to_path(&path, event);
    }

    /// Records a telemetry event safely to a specific destination path.
    pub fn record_event_to_path(path: &Path, event: &TelemetryEvent) {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            if let Ok(json_line) = serde_json::to_string(event) {
                let _ = writeln!(file, "{json_line}");
                let _ = file.flush();
            }
        }
    }

    /// Records a slicing result to telemetry with metadata.
    pub fn record_slice(slice: &SliceResult, source: &str, duration_ms: Option<u64>) {
        let path = Self::resolve_metrics_path();
        Self::record_slice_to_path(&path, slice, source, duration_ms);
    }

    /// Records a slicing result to a specific metrics path with metadata.
    pub fn record_slice_to_path(
        path: &Path,
        slice: &SliceResult,
        source: &str,
        duration_ms: Option<u64>,
    ) {
        let raw_tokens = slice.stats.raw_file_tokens;
        let sliced_tokens = slice.stats.sliced_tokens;
        let saved_tokens = raw_tokens.saturating_sub(sliced_tokens);

        let event = TelemetryEvent {
            timestamp: current_rfc3339_timestamp(),
            file_path: slice.target_symbol.file_path.clone(),
            symbol: slice.target_symbol.name.clone(),
            language: Some(slice.target_symbol.language.clone()),
            raw_tokens,
            sliced_tokens,
            saved_tokens,
            savings_percentage: slice.stats.savings_percentage,
            raw_lines: slice.stats.raw_lines,
            sliced_lines: slice.stats.sliced_lines,
            source: Some(source.to_string()),
            duration_ms,
        };

        Self::record_event_to_path(path, &event);
    }

    /// Records generic operation metrics to telemetry.
    pub fn record_operation(
        op: &str,
        file_path: &str,
        raw_tokens: usize,
        sliced_tokens: usize,
        saved_tokens: usize,
    ) {
        let pct = if raw_tokens > 0 {
            (saved_tokens as f64 / raw_tokens as f64) * 100.0
        } else {
            0.0
        };

        let event = TelemetryEvent {
            timestamp: current_rfc3339_timestamp(),
            file_path: file_path.to_string(),
            symbol: op.to_string(),
            language: None,
            raw_tokens,
            sliced_tokens,
            saved_tokens,
            savings_percentage: pct,
            raw_lines: 0,
            sliced_lines: 0,
            source: Some(op.to_string()),
            duration_ms: None,
        };

        Self::record_event(&event);
    }

    /// Reads all recorded telemetry events from the default metrics file.
    pub fn read_events() -> io::Result<Vec<TelemetryEvent>> {
        let path = Self::resolve_metrics_path();
        Self::read_events_from_path(&path)
    }

    /// Reads all recorded telemetry events from a specified file path.
    ///
    /// Malformed or empty lines are skipped gracefully.
    pub fn read_events_from_path(path: &Path) -> io::Result<Vec<TelemetryEvent>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<TelemetryEvent>(trimmed) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Loads and computes the aggregated telemetry summary from the default metrics file.
    pub fn load_summary() -> io::Result<TelemetrySummary> {
        let path = Self::resolve_metrics_path();
        Self::load_summary_from_path(&path)
    }

    /// Loads and computes the aggregated telemetry summary from a specified file path.
    pub fn load_summary_from_path(path: &Path) -> io::Result<TelemetrySummary> {
        let events = Self::read_events_from_path(path)?;
        Ok(Self::aggregate(&events))
    }

    /// Aggregates a slice of telemetry events into a comprehensive `TelemetrySummary`.
    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    pub fn aggregate(events: &[TelemetryEvent]) -> TelemetrySummary {
        let total_requests = events.len();
        let mut total_raw_tokens: usize = 0;
        let mut total_sliced_tokens: usize = 0;
        let mut total_saved_tokens: usize = 0;

        // Language aggregation: (requests, raw, sliced, saved)
        let mut lang_map: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
        // Source aggregation: (requests, saved, sum_savings_pct)
        let mut source_map: BTreeMap<String, (usize, usize, f64)> = BTreeMap::new();

        for ev in events {
            total_raw_tokens += ev.raw_tokens;
            total_sliced_tokens += ev.sliced_tokens;
            total_saved_tokens += ev.saved_tokens;

            let lang_key = ev
                .language
                .as_deref()
                .map_or_else(|| "Unknown".to_string(), normalize_language_name);

            let lang_entry = lang_map.entry(lang_key).or_insert((0, 0, 0, 0));
            lang_entry.0 += 1;
            lang_entry.1 += ev.raw_tokens;
            lang_entry.2 += ev.sliced_tokens;
            lang_entry.3 += ev.saved_tokens;

            let src_key = ev.source.as_deref().unwrap_or("unknown").to_string();
            let src_entry = source_map.entry(src_key).or_insert((0, 0, 0.0));
            src_entry.0 += 1;
            src_entry.1 += ev.saved_tokens;
            src_entry.2 += ev.savings_percentage;
        }

        let compression_percentage = if total_raw_tokens == 0 {
            0.0
        } else {
            let pct = (total_saved_tokens as f64 / total_raw_tokens as f64) * 100.0;
            (pct * 100.0).round() / 100.0
        };

        let estimated_cost_savings_usd =
            ((total_saved_tokens as f64 / 1_000_000.0) * STANDARD_PRICE_PER_MILLION_TOKENS * 100.0)
                .round()
                / 100.0;

        let cost_savings_by_tier = ModelTierSavings {
            standard_sonnet_gpt4o: ((total_saved_tokens as f64 / 1_000_000.0)
                * STANDARD_PRICE_PER_MILLION_TOKENS
                * 100.0)
                .round()
                / 100.0,
            frontier_opus: ((total_saved_tokens as f64 / 1_000_000.0)
                * FRONTIER_PRICE_PER_MILLION_TOKENS
                * 100.0)
                .round()
                / 100.0,
            economy_haiku_mini: ((total_saved_tokens as f64 / 1_000_000.0)
                * ECONOMY_PRICE_PER_MILLION_TOKENS
                * 100.0)
                .round()
                / 100.0,
        };

        let mut language_breakdown = BTreeMap::new();
        let mut by_language = Vec::new();

        for (lang, (reqs, raw, sliced, saved)) in lang_map {
            language_breakdown.insert(lang.clone(), saved);

            let pct = if raw == 0 {
                0.0
            } else {
                ((saved as f64 / raw as f64) * 100.0 * 100.0).round() / 100.0
            };

            let lang_cost =
                ((saved as f64 / 1_000_000.0) * STANDARD_PRICE_PER_MILLION_TOKENS * 100.0).round()
                    / 100.0;

            by_language.push(LanguageMetric {
                language: lang,
                requests: reqs,
                raw_tokens: raw,
                sliced_tokens: sliced,
                saved_tokens: saved,
                savings_percentage: pct,
                estimated_cost_savings_usd: lang_cost,
            });
        }

        // Sort by saved_tokens descending
        by_language.sort_by_key(|b| std::cmp::Reverse(b.saved_tokens));

        let mut by_source = Vec::new();
        for (src, (reqs, saved, sum_pct)) in source_map {
            let avg_pct = if reqs == 0 {
                0.0
            } else {
                ((sum_pct / reqs as f64) * 100.0).round() / 100.0
            };

            by_source.push(SourceMetric {
                source: src,
                requests: reqs,
                saved_tokens: saved,
                savings_percentage: avg_pct,
            });
        }
        by_source.sort_by_key(|b| std::cmp::Reverse(b.saved_tokens));

        // Up to 10 most recent events (newest first)
        let recent_events: Vec<TelemetryEvent> = events.iter().rev().take(10).cloned().collect();

        TelemetrySummary {
            total_requests,
            total_raw_tokens,
            total_sliced_tokens,
            total_saved_tokens,
            compression_percentage,
            estimated_cost_savings_usd,
            cost_savings_by_tier,
            language_breakdown,
            by_language,
            by_source,
            recent_events,
        }
    }
}

fn normalize_language_name(name: &str) -> String {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "typescript" | "ts" | "tsx" => "TypeScript".to_string(),
        "javascript" | "js" | "jsx" => "JavaScript".to_string(),
        "python" | "py" => "Python".to_string(),
        "go" | "golang" => "Go".to_string(),
        "rust" | "rs" => "Rust".to_string(),
        _ => {
            let mut c = name.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_rfc3339_timestamp() {
        let ts = current_rfc3339_timestamp();
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert_eq!(ts.len(), 20); // YYYY-MM-DDTHH:MM:SSZ
    }

    #[test]
    fn test_record_and_read_events() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let event1 = TelemetryEvent {
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
        };

        let event2 = TelemetryEvent {
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
        };

        TelemetryLogger::record_event_to_path(path, &event1);
        TelemetryLogger::record_event_to_path(path, &event2);

        let events = TelemetryLogger::read_events_from_path(path).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], event1);
        assert_eq!(events[1], event2);

        let summary = TelemetryLogger::load_summary_from_path(path).unwrap();
        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.total_raw_tokens, 3000);
        assert_eq!(summary.total_sliced_tokens, 450);
        assert_eq!(summary.total_saved_tokens, 2550);
        assert_eq!(summary.compression_percentage, 85.0);
        assert_eq!(summary.language_breakdown.get("TypeScript"), Some(&850));
        assert_eq!(summary.language_breakdown.get("Python"), Some(&1700));
        assert_eq!(summary.by_language.len(), 2);
        assert_eq!(summary.recent_events.len(), 2);
        assert_eq!(summary.recent_events[0], event2); // newest first
    }

    #[test]
    fn test_corrupt_line_handling() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write a good line, a corrupt line, empty line, and another good line
        let mut file = OpenOptions::new().write(true).open(path).unwrap();
        writeln!(file, "{{\"timestamp\":\"2026-08-16T12:00:00Z\",\"file_path\":\"a.rs\",\"symbol\":\"foo\",\"raw_tokens\":100,\"sliced_tokens\":20,\"saved_tokens\":80}}").unwrap();
        writeln!(file, "{{corrupted json line...").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "{{\"timestamp\":\"2026-08-16T12:01:00Z\",\"file_path\":\"b.rs\",\"symbol\":\"bar\",\"raw_tokens\":200,\"sliced_tokens\":50,\"saved_tokens\":150}}").unwrap();

        let events = TelemetryLogger::read_events_from_path(path).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].symbol, "foo");
        assert_eq!(events[1].symbol, "bar");
    }

    #[test]
    fn test_empty_aggregation() {
        let summary = TelemetryLogger::aggregate(&[]);
        assert_eq!(summary.total_requests, 0);
        assert_eq!(summary.total_raw_tokens, 0);
        assert_eq!(summary.total_saved_tokens, 0);
        assert_eq!(summary.compression_percentage, 0.0);
        assert_eq!(summary.estimated_cost_savings_usd, 0.0);
        assert!(summary.by_language.is_empty());
        assert!(summary.recent_events.is_empty());
    }
}
