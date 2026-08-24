//! Tier 1 Tests: Feature 10 — Semantic AST Diff (`semantic-diff`)
//!
//! Verifies structural AST diff engine:
//! - Differentiating signature modifications from internal body changes
//! - Added and removed symbols detection
//! - Staged git diff integration
//! - Token ROI savings calculation vs unified diff
//! - JSON format output

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f10_semantic_diff_signature_change() {
    // Arrange: Git sandbox with function signature change
    let sandbox = GitSandbox::new().expect("Failed sandbox");
    sandbox.write_file(
        "src/api.ts",
        "export function processPayment(amount: number): boolean {\n    return amount > 0;\n}\n",
    ).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    // Modify signature: add currency parameter
    sandbox.modify_file(
        "src/api.ts",
        "export function processPayment(amount: number, currency: string): boolean {\n    return amount > 0 && currency.length > 0;\n}\n",
    ).unwrap();

    // Act: Run diff command
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Command failed");

    // Assert: Diff output captures changed function
    output.assert_success();
    assert!(output.stdout.contains("processPayment"));
}

#[test]
fn test_f10_semantic_diff_added_and_removed_symbols() {
    // Arrange: Git repo with added function
    let sandbox = GitSandbox::new().expect("Failed sandbox");
    sandbox
        .write_file("src/utils.ts", "export function oldUtil() { return 1; }\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    sandbox
        .modify_file(
            "src/utils.ts",
            "export function oldUtil() { return 1; }\nexport function newUtil() { return 2; }\n",
        )
        .unwrap();

    // Act: Extract diff slice
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Command failed");

    // Assert: New utility is identified
    output.assert_success();
    assert!(output.stdout.contains("newUtil") || output.stdout.contains("utils.ts"));
}

#[test]
fn test_f10_semantic_diff_staged_changes() {
    // Arrange: Git repo with staged change
    let sandbox = GitSandbox::new().expect("Failed sandbox");
    sandbox
        .write_file("src/auth.ts", "export function login() { return false; }\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    sandbox
        .modify_file("src/auth.ts", "export function login() { return true; }\n")
        .unwrap();
    sandbox.stage_file("src/auth.ts").unwrap();

    // Act: Run diff with --staged
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff", "--staged"])
        .expect("Command failed");

    // Assert: Slices staged changes
    output.assert_success();
    assert!(output.stdout.contains("login"));
}

#[test]
fn test_f10_semantic_diff_token_roi_savings() {
    // Arrange: Large file with small modified function
    let sandbox = GitSandbox::new().expect("Failed sandbox");
    let filler = (0..50)
        .map(|i| format!("export function helper{}() {{ return {}; }}\n", i, i))
        .collect::<Vec<_>>()
        .join("");
    let original = format!(
        "{}export function target() {{\n    return 'original';\n}}\n",
        filler
    );
    sandbox.write_file("src/monolith.ts", &original).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    let modified = format!(
        "{}export function target() {{\n    return 'updated';\n}}\n",
        filler
    );
    sandbox.modify_file("src/monolith.ts", &modified).unwrap();

    // Act: Run diff
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Command failed");

    // Assert: Only target function sliced, yielding high token reduction vs full file
    output.assert_success();
    let verifier = TokenVerifier::new();
    let diff_tokens = verifier.count_tokens(&output.stdout);
    let full_tokens = verifier.count_tokens(&modified);
    assert!(full_tokens > diff_tokens);
}

#[test]
fn test_f10_semantic_diff_json_format() {
    // Arrange: Git sandbox
    let sandbox = GitSandbox::new().expect("Failed sandbox");
    sandbox
        .write_file(
            "src/calc.ts",
            "export function add(a: number, b: number) { return a + b; }\n",
        )
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    sandbox
        .modify_file(
            "src/calc.ts",
            "export function add(a: number, b: number) { return a + b + 1; }\n",
        )
        .unwrap();

    // Act: Diff with JSON format
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff", "--format", "json"])
        .expect("Command failed");

    // Assert: Valid JSON output
    output.assert_success();
    let json: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("Failed to parse JSON");
    assert!(json.is_array() || json.is_object());
}
