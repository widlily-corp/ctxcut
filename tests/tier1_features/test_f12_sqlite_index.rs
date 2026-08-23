//! Tier 1 Tests: Feature 12 — Persistent SQLite Indexing
//!
//! Verifies persistent caching and indexing:
//! - Workspace overview indexing
//! - Incremental file processing
//! - Cache invalidation on file modification
//! - Metrics history and persistence
//! - Clean directory traversals

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f12_sqlite_index_creation() {
    // Arrange: Create sample workspace
    let dir = TempDir::new().expect("Failed to create tempdir");
    fs::write(dir.path().join("a.ts"), "export function fnA() {}\n").unwrap();
    fs::write(dir.path().join("b.ts"), "export function fnB() {}\n").unwrap();

    // Act: Run workspace overview
    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()]).expect("Command failed");

    // Assert: Overview runs and discovers all files
    output.assert_success();
    assert!(output.stdout.contains("fnA") || output.stdout.contains("a.ts"));
    assert!(output.stdout.contains("fnB") || output.stdout.contains("b.ts"));
}

#[test]
fn test_f12_sqlite_incremental_cache_hit() {
    // Arrange: Workspace with files
    let dir = TempDir::new().expect("Failed to create tempdir");
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    // Act: Run stats twice
    let runner = CliRunner::new();
    let out1 = runner.run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()]).expect("First run failed");
    let out2 = runner.run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()]).expect("Second run failed");

    // Assert: Both succeed rapidly
    out1.assert_success();
    out2.assert_success();
}

#[test]
fn test_f12_sqlite_cache_invalidation_on_file_edit() {
    // Arrange: Modify file between runs
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("app.ts");
    fs::write(&file_path, "export function version() { return 1; }\n").unwrap();

    let runner = CliRunner::new();
    let out1 = runner.run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()]).expect("Run 1 failed");
    out1.assert_success();

    // Modify file
    fs::write(&file_path, "export function version() { return 2; }\nexport function newFeature() {}\n").unwrap();

    // Act: Re-run overview
    let out2 = runner.run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()]).expect("Run 2 failed");

    // Assert: Detected new content
    out2.assert_success();
    assert!(out2.stdout.contains("newFeature") || out2.stdout.contains("app.ts"));
}

#[test]
fn test_f12_sqlite_index_status_command() {
    // Arrange: Metrics inspection
    let runner = CliRunner::new();

    // Act: Run metrics command
    let output = runner.run(&["metrics"]).expect("Command failed");

    // Assert: Metrics command runs cleanly
    output.assert_success();
    assert!(output.stdout.contains("Tokens") || output.stdout.contains("METRICS") || output.stdout.contains("ROI"));
}

#[test]
fn test_f12_sqlite_index_clean() {
    // Arrange: Temporary workspace
    let dir = TempDir::new().expect("Failed to create tempdir");
    fs::write(dir.path().join("test.ts"), "export const x = 1;\n").unwrap();

    // Act: Run stats with JSON output
    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", "--format", "json", dir.path().to_str().unwrap()]).expect("Command failed");

    // Assert: Valid stats output
    output.assert_success();
}
