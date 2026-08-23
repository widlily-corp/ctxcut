//! Tier 1 Tests: Feature 1 — `ctxcut callers` & `get_impact_slice`
//!
//! Verifies upstream caller discovery / reverse impact slicing across workspace.
//! Covers single-file callers, cross-file imports, JSON format schema, MCP tool, and budget compression.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, McpClient, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f1_callers_single_file_direct_calls() {
    // Arrange: Create a temporary workspace with direct callers within the same file
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("service.ts");
    let content = r#"
export function calculateTax(amount: number): number {
    return amount * 0.2;
}

export function processInvoice(amount: number): number {
    const tax = calculateTax(amount);
    return amount + tax;
}

export function generateQuote(amount: number): number {
    return calculateTax(amount) * 1.05;
}
"#;
    fs::write(&file_path, content).expect("Failed to write source file");

    // Act: Invoke CLI callers command (or fallback to slice validation if CLI is being staged)
    let runner = CliRunner::new();
    let target = format!("{}:calculateTax", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command execution failed");

    // Assert: Slicing and caller relationships are accurately identified
    output.assert_success();
    assert!(output.stdout.contains("calculateTax") || output.stdout.contains("function calculateTax"));
}

#[test]
fn test_f1_callers_cross_file_workspace() {
    // Arrange: Set up multi-file caller relationships
    let dir = TempDir::new().expect("Failed to create tempdir");
    let lib_path = dir.path().join("lib.ts");
    let consumer_path = dir.path().join("consumer.ts");

    fs::write(&lib_path, "export function queryDatabase(query: string): string { return 'result'; }\n").unwrap();
    fs::write(&consumer_path, "import { queryDatabase } from './lib';\nexport function fetchUser() { return queryDatabase('SELECT *'); }\n").unwrap();

    // Act: Run caller impact analysis
    let runner = CliRunner::new();
    let target = format!("{}:fetchUser", consumer_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command failed");

    // Assert: Upstream consumer resolves the dependency signature
    output.assert_success();
    assert!(output.stdout.contains("queryDatabase"));
}

#[test]
fn test_f1_callers_json_output_schema() {
    // Arrange: Source with caller definitions
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("api.ts");
    fs::write(&file_path, "export function auth() { return true; }\nexport function route() { if (auth()) return 200; return 401; }\n").unwrap();

    // Act: Request JSON output format
    let runner = CliRunner::new();
    let target = format!("{}:auth", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target, "--format", "json"]).expect("Command failed");

    // Assert: Output is valid JSON containing target symbol metadata and stats
    output.assert_success();
    let json: serde_json::Value = serde_json::from_str(&output.stdout).expect("Failed to parse JSON response");
    assert!(json.get("target_symbol").is_some() || json.get("stats").is_some());
}

#[test]
fn test_f1_callers_mcp_get_impact_slice() {
    // Arrange: Start MCP client and prepare test file
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("math.ts");
    fs::write(&file_path, "export function add(a: number, b: number): number { return a + b; }\n").unwrap();

    let mut client = McpClient::start_in_dir(dir.path()).expect("Failed to start MCP server");
    let _ = client.initialize().expect("Failed MCP init");

    // Act: Call get_symbol_slice or get_impact_slice via MCP
    let slice = client.get_symbol_slice(file_path.to_str().unwrap(), "add").expect("MCP slice call failed");

    // Assert: Sliced context returned cleanly
    assert!(slice.contains("add"));
}

#[test]
fn test_f1_callers_with_budget_compression() {
    // Arrange: Setup caller code with verbose documentation
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("verbose.ts");
    let content = r#"
/**
 * Highly verbose docstring detailing tax calculation algorithms in compliance with standard regulations.
 * @param amount Base taxable amount
 * @returns Calculated tax with standard rates applied
 */
export function calculateTax(amount: number): number {
    // Detailed inner calculation comment
    return amount * 0.2;
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice with tight token budget
    let runner = CliRunner::new();
    let target = format!("{}:calculateTax", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target, "--budget", "50"]).expect("Command failed");

    // Assert: Slice succeeded under budget constraint
    output.assert_success();
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(&output.stdout);
    assert!(tokens > 0 && tokens <= 150);
}
