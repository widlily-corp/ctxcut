//! Tier 5 E2E Integration Test Suite — Telemetry & Metrics Dashboard
//!
//! Verifies:
//! 1. Persistent telemetry recording across all entry points (CLI slice, diff, route, MCP server).
//! 2. Multi-language aggregation, compression ratio %, and ROI cost calculations.
//! 3. Resilient JSONL append operations, path resolution, and corrupt line fault tolerance.
//! 4. Terminal dashboard rendering and `--format json` output.

#![allow(dead_code, unused_imports, clippy::all)]

#[path = "common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use common::{CliRunner, GitSandbox, McpClient};
use ctxcut_cli::metrics::{format_currency, format_number, format_percentage, render_dashboard};
use ctxcut_core::{
    current_rfc3339_timestamp, format_rfc3339, LanguageMetric, ModelTierSavings, SourceMetric,
    TelemetryEvent, TelemetryLogger, TelemetrySummary,
};
use serde_json::{json, Value};
use tempfile::NamedTempFile;

#[test]
fn test_telemetry_event_serialization_and_deserialization() {
    let event = TelemetryEvent {
        timestamp: "2026-08-16T12:00:00Z".to_string(),
        file_path: "src/auth.ts".to_string(),
        symbol: "AuthService.validateToken".to_string(),
        language: Some("typescript".to_string()),
        raw_tokens: 1450,
        sliced_tokens: 182,
        saved_tokens: 1268,
        savings_percentage: 87.45,
        raw_lines: 112,
        sliced_lines: 18,
        source: Some("mcp_get_symbol_slice".to_string()),
        duration_ms: Some(4),
    };

    let serialized = serde_json::to_string(&event).expect("Serialization must succeed");
    let deserialized: TelemetryEvent =
        serde_json::from_str(&serialized).expect("Deserialization must succeed");

    assert_eq!(event, deserialized);
}

#[test]
fn test_telemetry_path_resolution_with_env_vars() {
    let custom_file = PathBuf::from("target/custom_metrics.jsonl");
    std::env::set_var("CTXCUT_METRICS_FILE", &custom_file);
    assert_eq!(TelemetryLogger::resolve_metrics_path(), custom_file);
    std::env::remove_var("CTXCUT_METRICS_FILE");
}

#[test]
fn test_telemetry_file_append_and_read() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let path = temp_file.path();

    let event1 = TelemetryEvent {
        timestamp: "2026-08-16T10:00:00Z".to_string(),
        file_path: "lib/billing.rs".to_string(),
        symbol: "process_invoice".to_string(),
        language: Some("rust".to_string()),
        raw_tokens: 3000,
        sliced_tokens: 450,
        saved_tokens: 2550,
        savings_percentage: 85.0,
        raw_lines: 180,
        sliced_lines: 30,
        source: Some("cli_slice".to_string()),
        duration_ms: Some(3),
    };

    let event2 = TelemetryEvent {
        timestamp: "2026-08-16T10:05:00Z".to_string(),
        file_path: "pkg/api.go".to_string(),
        symbol: "ServeHTTP".to_string(),
        language: Some("go".to_string()),
        raw_tokens: 1500,
        sliced_tokens: 250,
        saved_tokens: 1250,
        savings_percentage: 83.33,
        raw_lines: 100,
        sliced_lines: 20,
        source: Some("mcp_get_symbol_slice".to_string()),
        duration_ms: Some(6),
    };

    TelemetryLogger::record_event_to_path(path, &event1);
    TelemetryLogger::record_event_to_path(path, &event2);

    let events = TelemetryLogger::read_events_from_path(path).expect("read_events must succeed");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], event1);
    assert_eq!(events[1], event2);

    let summary = TelemetryLogger::load_summary_from_path(path).expect("load_summary must succeed");
    assert_eq!(summary.total_requests, 2);
    assert_eq!(summary.total_raw_tokens, 4500);
    assert_eq!(summary.total_sliced_tokens, 700);
    assert_eq!(summary.total_saved_tokens, 3800);
    assert_eq!(summary.compression_percentage, 84.44);
    assert_eq!(summary.language_breakdown.get("Rust"), Some(&2550));
    assert_eq!(summary.language_breakdown.get("Go"), Some(&1250));
    assert_eq!(summary.by_language.len(), 2);
    assert_eq!(summary.by_source.len(), 2);
}

#[test]
fn test_telemetry_corrupt_line_fault_tolerance() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let path = temp_file.path();

    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("Open failed");
    writeln!(
        file,
        "{{\"timestamp\":\"2026-08-16T09:00:00Z\",\"file_path\":\"a.py\",\"symbol\":\"func1\",\"raw_tokens\":500,\"sliced_tokens\":100,\"saved_tokens\":400}}"
    )
    .unwrap();
    writeln!(file, "{{corrupt-json-string-without-closing-brace").unwrap();
    writeln!(file, "   ").unwrap();
    writeln!(
        file,
        "{{\"timestamp\":\"2026-08-16T09:01:00Z\",\"file_path\":\"b.py\",\"symbol\":\"func2\",\"raw_tokens\":600,\"sliced_tokens\":120,\"saved_tokens\":480}}"
    )
    .unwrap();

    let events = TelemetryLogger::read_events_from_path(path).expect("Reading corrupt file must not fail");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].symbol, "func1");
    assert_eq!(events[1].symbol, "func2");
}

#[test]
fn test_telemetry_multi_language_aggregation() {
    let events = vec![
        TelemetryEvent {
            timestamp: "2026-08-16T01:00:00Z".to_string(),
            file_path: "src/auth.ts".to_string(),
            symbol: "login".to_string(),
            language: Some("typescript".to_string()),
            raw_tokens: 3410200,
            sliced_tokens: 492100,
            saved_tokens: 2918100,
            savings_percentage: 85.57,
            raw_lines: 1000,
            sliced_lines: 150,
            source: Some("mcp_get_symbol_slice".to_string()),
            duration_ms: Some(5),
        },
        TelemetryEvent {
            timestamp: "2026-08-16T02:00:00Z".to_string(),
            file_path: "api/routes.py".to_string(),
            symbol: "get_user".to_string(),
            language: Some("python".to_string()),
            raw_tokens: 1280000,
            sliced_tokens: 215000,
            saved_tokens: 1065000,
            savings_percentage: 83.2,
            raw_lines: 500,
            sliced_lines: 80,
            source: Some("mcp_get_symbol_slice".to_string()),
            duration_ms: Some(4),
        },
        TelemetryEvent {
            timestamp: "2026-08-16T03:00:00Z".to_string(),
            file_path: "pkg/net.go".to_string(),
            symbol: "Dial".to_string(),
            language: Some("go".to_string()),
            raw_tokens: 640000,
            sliced_tokens: 98000,
            saved_tokens: 542000,
            savings_percentage: 84.69,
            raw_lines: 300,
            sliced_lines: 40,
            source: Some("cli_slice".to_string()),
            duration_ms: Some(3),
        },
        TelemetryEvent {
            timestamp: "2026-08-16T04:00:00Z".to_string(),
            file_path: "src/engine.rs".to_string(),
            symbol: "run".to_string(),
            language: Some("rust".to_string()),
            raw_tokens: 390000,
            sliced_tokens: 93710,
            saved_tokens: 296290,
            savings_percentage: 75.97,
            raw_lines: 200,
            sliced_lines: 50,
            source: Some("cli_diff".to_string()),
            duration_ms: Some(7),
        },
    ];

    let summary = TelemetryLogger::aggregate(&events);

    assert_eq!(summary.total_requests, 4);
    assert_eq!(summary.total_raw_tokens, 5720200);
    assert_eq!(summary.total_sliced_tokens, 898810);
    assert_eq!(summary.total_saved_tokens, 4821390);
    assert_eq!(summary.compression_percentage, 84.29);
    assert_eq!(summary.estimated_cost_savings_usd, 14.46);
    assert_eq!(summary.cost_savings_by_tier.standard_sonnet_gpt4o, 14.46);
    assert_eq!(summary.cost_savings_by_tier.frontier_opus, 72.32);
    assert_eq!(summary.cost_savings_by_tier.economy_haiku_mini, 2.41);

    assert_eq!(summary.by_language.len(), 4);
    assert_eq!(summary.by_language[0].language, "TypeScript");
    assert_eq!(summary.by_language[0].saved_tokens, 2918100);

    assert_eq!(summary.by_source.len(), 3);
}

#[test]
fn test_cli_slice_records_telemetry() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let metrics_path = temp_file.path().to_path_buf();
    let metrics_str = metrics_path.to_string_lossy().to_string();

    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let code = r#"
export interface Item {
    id: string;
    price: number;
    name: string;
    category: string;
}

export function computeTotal(items: Item[]): number {
    return items.reduce((sum, item) => sum + item.price, 0);
}

export function applyDiscount(total: number, discount: number): number {
    if (discount < 0) return total;
    return Math.max(0, total - discount);
}

export function calculateShipping(items: Item[]): number {
    return items.length * 5;
}

export function processOrder(items: Item[]): void {
    console.log("Order processed with count:", items.length);
}

export function validateCart(items: Item[]): boolean {
    return items.length > 0 && items.every(i => i.price >= 0);
}

export function formatCartSummary(items: Item[]): string {
    return `Cart has ${items.length} items totaling $${computeTotal(items)}`;
}

export function filterByCategory(items: Item[], category: string): Item[] {
    return items.filter(i => i.category === category);
}

export function findItemById(items: Item[], id: string): Item | undefined {
    return items.find(i => i.id === id);
}
"#;
    sandbox.write_file("src/cart.ts", code).unwrap();

    let runner = CliRunner::new();
    let output = runner
        .run_with_env(
            Some(sandbox.path()),
            &["slice", "src/cart.ts:computeTotal"],
            &[("CTXCUT_METRICS_FILE", &metrics_str)],
        )
        .expect("run_with_env failed");
    output.assert_success();

    let events = TelemetryLogger::read_events_from_path(&metrics_path).unwrap();
    assert!(!events.is_empty());
    assert_eq!(events[0].symbol, "computeTotal");
    assert_eq!(events[0].source.as_deref(), Some("cli_slice"));
    assert!(events[0].saved_tokens > 0);
}

#[test]
fn test_cli_diff_records_telemetry() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let metrics_path = temp_file.path().to_path_buf();
    let metrics_str = metrics_path.to_string_lossy().to_string();

    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let initial_code = r#"
export function calculateTax(amount: number): number {
    return amount * 0.1;
}
"#;
    sandbox.write_file("src/tax.ts", initial_code).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial tax commit").unwrap();

    let updated_code = r#"
export function calculateTax(amount: number): number {
    return amount * 0.15;
}
"#;
    sandbox.write_file("src/tax.ts", updated_code).unwrap();

    let runner = CliRunner::new();
    let output = runner
        .run_with_env(
            Some(sandbox.path()),
            &["diff"],
            &[("CTXCUT_METRICS_FILE", &metrics_str)],
        )
        .expect("run_with_env failed");
    output.assert_success();

    let events = TelemetryLogger::read_events_from_path(&metrics_path).unwrap();
    assert!(!events.is_empty());
    assert_eq!(events[0].symbol, "calculateTax");
    assert_eq!(events[0].source.as_deref(), Some("cli_diff"));
}

#[test]
fn test_cli_route_records_telemetry() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let metrics_path = temp_file.path().to_path_buf();
    let metrics_str = metrics_path.to_string_lossy().to_string();

    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let route_code = r#"
import express from 'express';
const router = express.Router();

export interface CheckoutDto {
    cartId: string;
}

export function handleCheckout(req: any, res: any) {
    res.json({ status: 'ok' });
}

router.post('/api/v1/checkout', handleCheckout);
"#;
    sandbox.write_file("src/routes.ts", route_code).unwrap();

    let runner = CliRunner::new();
    let output = runner
        .run_with_env(
            Some(sandbox.path()),
            &["route", "POST", "/api/v1/checkout"],
            &[("CTXCUT_METRICS_FILE", &metrics_str)],
        )
        .expect("run_with_env failed");
    output.assert_success();

    let events = TelemetryLogger::read_events_from_path(&metrics_path).unwrap();
    assert!(!events.is_empty());
    assert_eq!(events[0].symbol, "handleCheckout");
    assert_eq!(events[0].source.as_deref(), Some("cli_route"));
}

#[test]
fn test_mcp_symbol_and_diff_slice_records_telemetry() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let metrics_path = temp_file.path().to_path_buf();
    let metrics_str = metrics_path.to_string_lossy().to_string();

    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let code = r#"
export interface Config {
    port: number;
}

export function startServer(cfg: Config): void {
    console.log("Listening on", cfg.port);
}
"#;
    sandbox.write_file("src/server.ts", code).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial server commit").unwrap();

    let mut client = McpClient::start_with_options(
        Some(sandbox.path()),
        &[],
        &[("CTXCUT_METRICS_FILE", &metrics_str)],
    )
    .expect("Failed to start MCP client");
    client.initialize().expect("Initialize must succeed");

    // Call get_symbol_slice
    let slice_res = client
        .get_symbol_slice("src/server.ts", "startServer")
        .expect("get_symbol_slice must succeed");
    assert!(slice_res.to_string().contains("startServer"));

    // Mutate file and call get_diff_slice
    let mutated = r#"
export interface Config {
    port: number;
}

export function startServer(cfg: Config): void {
    console.log("Listening securely on", cfg.port);
}
"#;
    sandbox.write_file("src/server.ts", mutated).unwrap();

    let diff_res = client.get_diff_slice(false).expect("get_diff_slice must succeed");
    assert!(diff_res.to_string().contains("startServer"));

    let events = TelemetryLogger::read_events_from_path(&metrics_path).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].source.as_deref(), Some("mcp_get_symbol_slice"));
    assert_eq!(events[1].source.as_deref(), Some("mcp_get_diff_slice"));
}

#[test]
fn test_dashboard_text_render_output() {
    let events = vec![TelemetryEvent {
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
    }];

    let summary = TelemetryLogger::aggregate(&events);
    let path = Path::new("~/.ctxcut/metrics.jsonl");
    let dashboard = render_dashboard(&summary, path);

    assert!(dashboard.contains("CTXCUT TELEMETRY & TOKEN SAVINGS DASHBOARD"));
    assert!(dashboard.contains("TOTAL REQUESTS"));
    assert!(dashboard.contains("TOKENS SAVED"));
    assert!(dashboard.contains("850"));
    assert!(dashboard.contains("TypeScript"));
    assert!(dashboard.contains("CLI (ctxcut slice)"));
    assert!(dashboard.contains("src/auth.ts:login"));
}

#[test]
fn test_dashboard_empty_state_output() {
    let summary = TelemetryLogger::aggregate(&[]);
    let path = Path::new("~/.ctxcut/metrics.jsonl");
    let dashboard = render_dashboard(&summary, path);

    assert!(dashboard.contains("CTXCUT TELEMETRY & TOKEN SAVINGS DASHBOARD"));
    assert!(dashboard.contains("No telemetry data recorded yet"));
    assert!(dashboard.contains("Start saving tokens by running"));
}

#[test]
fn test_dashboard_json_format_output() {
    let events = vec![TelemetryEvent {
        timestamp: "2026-08-16T12:00:00Z".to_string(),
        file_path: "src/billing.py".to_string(),
        symbol: "charge".to_string(),
        language: Some("python".to_string()),
        raw_tokens: 2000,
        sliced_tokens: 200,
        saved_tokens: 1800,
        savings_percentage: 90.0,
        raw_lines: 100,
        sliced_lines: 10,
        source: Some("mcp_get_symbol_slice".to_string()),
        duration_ms: Some(4),
    }];

    let summary = TelemetryLogger::aggregate(&events);
    let json_str = serde_json::to_string_pretty(&summary).expect("Must serialize to JSON");
    let parsed: Value = serde_json::from_str(&json_str).expect("Must parse valid JSON");

    assert_eq!(parsed["total_requests"], 1);
    assert_eq!(parsed["total_raw_tokens"], 2000);
    assert_eq!(parsed["total_sliced_tokens"], 200);
    assert_eq!(parsed["total_saved_tokens"], 1800);
    assert_eq!(parsed["compression_percentage"], 90.0);
    assert!(parsed["estimated_cost_savings_usd"].is_number());
    assert!(parsed["language_breakdown"]["Python"].is_number());
}
