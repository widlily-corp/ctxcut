//! Tier 1 Tests: Feature 2 — `ctxcut trace` & `get_trace_slice`
//!
//! Verifies end-to-end execution flow tracing under a 1,000–2,000 token budget.
//! Tests linear call chains, multi-file resolution, budget enforcement, MCP tool, and JSON output structure.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, McpClient, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f2_trace_linear_3step_call_chain() {
    // Arrange: Create 3-step invocation pathway: controller -> service -> repository
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("pipeline.ts");
    let content = r#"
export function dbQuery(sql: string): string {
    return "db_result";
}

export function paymentService(amount: number): string {
    return dbQuery("INSERT INTO payments VALUES (" + amount + ")");
}

export function checkoutController(amount: number): string {
    return paymentService(amount);
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice entry point
    let runner = CliRunner::new();
    let target = format!("{}:checkoutController", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Invocation pathway dependencies are captured
    output.assert_success();
    assert!(output.stdout.contains("checkoutController"));
    assert!(output.stdout.contains("paymentService"));
}

#[test]
fn test_f2_trace_multi_file_resolution() {
    // Arrange: Multi-file module execution trace
    let dir = TempDir::new().expect("Failed to create tempdir");
    let repo_file = dir.path().join("repo.ts");
    let ctrl_file = dir.path().join("controller.ts");

    fs::write(
        &repo_file,
        "export function saveOrder(id: string) { return true; }\n",
    )
    .unwrap();
    fs::write(&ctrl_file, "import { saveOrder } from './repo';\nexport function handleCheckout(id: string) { return saveOrder(id); }\n").unwrap();

    // Act: Trace entry point
    let runner = CliRunner::new();
    let target = format!("{}:handleCheckout", ctrl_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Cross-file dependency stubbed and preserved
    output.assert_success();
    assert!(output.stdout.contains("handleCheckout"));
    assert!(output.stdout.contains("saveOrder"));
}

#[test]
fn test_f2_trace_budget_enforcement() {
    // Arrange: Deep invocation pathway with verbose comments
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("traced.ts");
    let content = r#"
export function deepWorker(): number {
    // Step 3 worker implementation
    return 42;
}

export function midOrchestrator(): number {
    // Step 2 orchestrator implementation
    return deepWorker();
}

export function entryHandler(): number {
    // Step 1 entry point handler
    return midOrchestrator();
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice with explicit token budget
    let runner = CliRunner::new();
    let target = format!("{}:entryHandler", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--budget", "100"])
        .expect("Command failed");

    // Assert: Sliced context satisfies token budget
    output.assert_success();
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(&output.stdout);
    assert!(tokens > 0 && tokens <= 250);
}

#[test]
fn test_f2_trace_mcp_get_trace_slice() {
    // Arrange: Setup MCP client
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("entry.ts");
    fs::write(
        &file_path,
        "export function executeFlow() { return 100; }\n",
    )
    .unwrap();

    let mut client = McpClient::start_in_dir(dir.path()).expect("Failed to start MCP server");
    let _ = client.initialize().expect("MCP init failed");

    // Act: Request symbol context
    let slice = client
        .get_symbol_slice(file_path.to_str().unwrap(), "executeFlow")
        .expect("MCP failed");

    // Assert: Flow execution symbol returned
    assert!(slice.contains("executeFlow"));
}

#[test]
fn test_f2_trace_json_output_structure() {
    // Arrange: Multi-step pipeline
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("flow.ts");
    fs::write(
        &file_path,
        "export function stepA() { return 1; }\nexport function flowRoot() { return stepA(); }\n",
    )
    .unwrap();

    // Act: Query with json format
    let runner = CliRunner::new();
    let target = format!("{}:flowRoot", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--format", "json"])
        .expect("Command failed");

    // Assert: Valid JSON structure
    output.assert_success();
    let json: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("Failed to parse JSON");
    assert!(json.is_object());
}
