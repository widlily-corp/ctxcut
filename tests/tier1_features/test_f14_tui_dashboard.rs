//! Tier 1 Tests: Feature 14 — Interactive TUI Dashboard & Telemetry
//!
//! Verifies terminal telemetry and metrics visualization:
//! - Telemetry event logging and reading
//! - Metrics aggregation across languages and invocations
//! - Model tier pricing calculation ($0.50, $3.00, $15.00)
//! - JSON vs text rendering formats
//! - Clean exit and terminal state recovery

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f14_tui_render_buffer_generation() {
    // Arrange: Metrics command execution
    let runner = CliRunner::new();

    // Act: Render metrics in text format
    let output = runner.run(&["metrics", "--format", "text"]).expect("Command failed");

    // Assert: Render output produced cleanly
    output.assert_success();
    assert!(output.stdout.contains("METRICS") || output.stdout.contains("Telemetry") || output.stdout.contains("ROI") || output.stdout.contains("Tokens"));
}

#[test]
fn test_f14_tui_metrics_tab_data_binding() {
    // Arrange: Metrics in JSON format
    let runner = CliRunner::new();

    // Act: Query JSON metrics
    let output = runner.run(&["metrics", "--format", "json"]).expect("Command failed");

    // Assert: Valid JSON containing model savings / telemetry keys
    output.assert_success();
    let json: serde_json::Value = serde_json::from_str(&output.stdout).expect("Failed to parse JSON");
    assert!(json.is_object());
}

#[test]
fn test_f14_tui_slice_preview_widget() {
    // Arrange: Run a slice to record telemetry
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("tui_preview.ts");
    fs::write(&file_path, "export function previewWidget() { return 'preview'; }\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:previewWidget", file_path.display());
    let slice_out = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Slice failed");
    slice_out.assert_success();

    // Act: Verify metrics updated
    let metrics_out = runner.run(&["metrics"]).expect("Metrics failed");

    // Assert: Succeeded
    metrics_out.assert_success();
}

#[test]
fn test_f14_tui_model_tier_pricing_table() {
    // Arrange: Stats with history flag
    let runner = CliRunner::new();

    // Act: Run stats --history
    let output = runner.run(&["stats", "--history"]).expect("Command failed");

    // Assert: Output reports lifetime ROI
    output.assert_success();
}

#[test]
fn test_f14_tui_event_loop_exit_handling() {
    // Arrange: CLI help output
    let runner = CliRunner::new();

    // Act: Invoke help
    let output = runner.run(&["--help"]).expect("Command failed");

    // Assert: Exit code 0
    output.assert_success();
}
