//! Unit test suite for `ctxcut_core::telemetry`.

use ctxcut_core::{current_rfc3339_timestamp, TelemetryEvent, TelemetryLogger};
use std::fs::OpenOptions;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_iso8601_timestamp_format() {
    let ts = current_rfc3339_timestamp();
    assert_eq!(
        ts.len(),
        20,
        "Timestamp must be 20 chars (YYYY-MM-DDTHH:MM:SSZ)"
    );
    assert!(ts.ends_with('Z'));
    assert!(ts.contains('T'));
}

#[test]
fn test_telemetry_event_roundtrip() {
    let event = TelemetryEvent {
        timestamp: "2026-08-16T12:00:00Z".to_string(),
        file_path: "src/auth.ts".to_string(),
        symbol: "login".to_string(),
        language: Some("typescript".to_string()),
        raw_tokens: 1500,
        sliced_tokens: 200,
        saved_tokens: 1300,
        savings_percentage: 86.67,
        raw_lines: 100,
        sliced_lines: 20,
        source: Some("cli_slice".to_string()),
        duration_ms: Some(5),
    };

    let json_str = serde_json::to_string(&event).unwrap();
    let decoded: TelemetryEvent = serde_json::from_str(&json_str).unwrap();
    assert_eq!(event, decoded);
}

#[test]
fn test_telemetry_append_read_summary() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();

    let e1 = TelemetryEvent {
        timestamp: "2026-08-16T12:01:00Z".to_string(),
        file_path: "src/a.ts".to_string(),
        symbol: "funcA".to_string(),
        language: Some("typescript".to_string()),
        raw_tokens: 1000,
        sliced_tokens: 100,
        saved_tokens: 900,
        savings_percentage: 90.0,
        raw_lines: 50,
        sliced_lines: 10,
        source: Some("cli_slice".to_string()),
        duration_ms: Some(2),
    };

    let e2 = TelemetryEvent {
        timestamp: "2026-08-16T12:02:00Z".to_string(),
        file_path: "api/b.py".to_string(),
        symbol: "funcB".to_string(),
        language: Some("python".to_string()),
        raw_tokens: 2000,
        sliced_tokens: 400,
        saved_tokens: 1600,
        savings_percentage: 80.0,
        raw_lines: 120,
        sliced_lines: 30,
        source: Some("mcp_get_symbol_slice".to_string()),
        duration_ms: Some(4),
    };

    TelemetryLogger::record_event_to_path(path, &e1);
    TelemetryLogger::record_event_to_path(path, &e2);

    let events = TelemetryLogger::read_events_from_path(path).unwrap();
    assert_eq!(events.len(), 2);

    let summary = TelemetryLogger::load_summary_from_path(path).unwrap();
    assert_eq!(summary.total_requests, 2);
    assert_eq!(summary.total_raw_tokens, 3000);
    assert_eq!(summary.total_sliced_tokens, 500);
    assert_eq!(summary.total_saved_tokens, 2500);
    assert_eq!(summary.compression_percentage, 83.33);
    assert_eq!(summary.estimated_cost_savings_usd, 0.01);
}

#[test]
fn test_telemetry_corrupt_lines_skip() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();

    let mut file = OpenOptions::new().write(true).open(path).unwrap();
    writeln!(file, "{{\"timestamp\":\"2026-08-16T00:00:00Z\",\"file_path\":\"a.rs\",\"symbol\":\"sym1\",\"raw_tokens\":100,\"sliced_tokens\":20,\"saved_tokens\":80}}").unwrap();
    writeln!(file, "not-json").unwrap();
    writeln!(file, "{{\"timestamp\":\"2026-08-16T00:01:00Z\",\"file_path\":\"b.rs\",\"symbol\":\"sym2\",\"raw_tokens\":200,\"sliced_tokens\":40,\"saved_tokens\":160}}").unwrap();

    let events = TelemetryLogger::read_events_from_path(path).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].symbol, "sym1");
    assert_eq!(events[1].symbol, "sym2");
}

#[test]
fn test_telemetry_empty_aggregation() {
    let summary = TelemetryLogger::aggregate(&[]);
    assert_eq!(summary.total_requests, 0);
    assert_eq!(summary.total_raw_tokens, 0);
    assert_eq!(summary.total_saved_tokens, 0);
    assert_eq!(summary.compression_percentage, 0.0);
    assert_eq!(summary.estimated_cost_savings_usd, 0.0);
    assert!(summary.by_language.is_empty());
    assert!(summary.by_source.is_empty());
    assert!(summary.recent_events.is_empty());
}
