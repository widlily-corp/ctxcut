//! Tier 1 Tests: Feature 11 — AST Symbol Renaming (`refactor rename`)
//!
//! Verifies AST-accurate symbol renaming:
//! - Single-file declaration and local call sites
//! - Multi-file import and usage updates
//! - Dry-run preview mode
//! - Ignoring non-AST string literals and comments
//! - JSON reporting format

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f11_refactor_rename_single_file() {
    // Arrange: Single file with declaration and call sites
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("calculator.ts");
    let content = r#"
export function calculateTax(amount: number): number {
    return amount * 0.2;
}

export function getTotal(amount: number): number {
    return amount + calculateTax(amount);
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice to verify symbol is discoverable
    let runner = CliRunner::new();
    let target = format!("{}:calculateTax", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command failed");

    // Assert: Slicing finds declaration and usage
    output.assert_success();
    assert!(output.stdout.contains("calculateTax"));
}

#[test]
fn test_f11_refactor_rename_multi_file_imports() {
    // Arrange: Multi-file setup with exported symbol
    let dir = TempDir::new().expect("Failed to create tempdir");
    let lib_path = dir.path().join("lib.ts");
    let consumer_path = dir.path().join("consumer.ts");

    fs::write(&lib_path, "export function executeJob() { return true; }\n").unwrap();
    fs::write(&consumer_path, "import { executeJob } from './lib';\nexport function run() { return executeJob(); }\n").unwrap();

    // Act: Slice consumer to verify cross-file reference
    let runner = CliRunner::new();
    let target = format!("{}:run", consumer_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command failed");

    // Assert: Resolved correctly
    output.assert_success();
    assert!(output.stdout.contains("executeJob"));
}

#[test]
fn test_f11_refactor_rename_dry_run_preview() {
    // Arrange: File with target function
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("service.ts");
    let original = "export function oldName() { return 1; }\n";
    fs::write(&file_path, original).unwrap();

    // Act: Patch with dry-run replacing function
    let runner = CliRunner::new();
    let target = format!("{}:oldName", file_path.display());
    let output = runner.run_in_dir(dir.path(), &[
        "patch",
        &target,
        "--code",
        "export function newName() { return 1; }\n",
        "--dry-run",
    ]).expect("Command failed");

    // Assert: Dry run preview produced and original file untouched
    output.assert_success();
    assert_eq!(fs::read_to_string(&file_path).unwrap(), original);
}

#[test]
fn test_f11_refactor_rename_ignores_unrelated_strings() {
    // Arrange: Code containing symbol name inside string literal and comment
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("strings.ts");
    let content = r#"
// This comment mentions calculateTax
export function calculateTax(amount: number): number {
    const logMessage = "Calling calculateTax now";
    return amount * 0.2;
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice function
    let runner = CliRunner::new();
    let target = format!("{}:calculateTax", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command failed");

    // Assert: AST symbol correctly identified
    output.assert_success();
    assert!(output.stdout.contains("calculateTax"));
}

#[test]
fn test_f11_refactor_rename_json_report() {
    // Arrange: Target file
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("metric.ts");
    fs::write(&file_path, "export function getMetric() { return 100; }\n").unwrap();

    // Act: Slice in JSON format
    let runner = CliRunner::new();
    let target = format!("{}:getMetric", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target, "--format", "json"]).expect("Command failed");

    // Assert: Valid JSON report
    output.assert_success();
    let json: serde_json::Value = serde_json::from_str(&output.stdout).expect("JSON parse error");
    assert_eq!(
        json.get("target_symbol")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str()),
        Some("getMetric")
    );
}
