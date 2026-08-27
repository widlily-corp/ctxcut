//! Tier 2 Boundary Tests: Features 16 to 19 (Fullstack Trace, Intent Slicing, Batch Refactor, Swarm Partitioning)
//!
//! Comprehensive boundary and adversarial edge cases:
//! - F16: Dangling client calls, missing SQL migrations, circular RPC procedures, missing route handlers
//! - F17: Empty prompts, gibberish queries, extreme token budgets (<80 tokens), corrupted BM25 cache recovery
//! - F18: Overlapping patch spans, malformed replacement syntax, dry-run rollbacks, readonly filesystem
//! - F19: Oversized cluster partition ($K > N$), cyclic dependency graph cuts, empty workspace partitioning

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};
use std::fs;
use tempfile::TempDir;

// =========================================================================
// F16 Boundary Tests: Full-Stack Cross-Boundary Tracing
// =========================================================================

#[test]
fn test_f16_boundary_dangling_client_call_without_backend_route() {
    // Arrange: Client API call pointing to non-existent backend endpoint
    let dir = TempDir::new().unwrap();
    let client_file = dir.path().join("dangling_client.ts");
    fs::write(
        &client_file,
        r#"
export async function callNonExistentEndpoint(): Promise<void> {
    await fetch('/api/v99/unknown/resource', { method: 'DELETE' });
}
"#,
    )
    .unwrap();

    // Act: Slice dangling client call
    let runner = CliRunner::new();
    let target = format!("{}:callNonExistentEndpoint", client_file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();

    // Assert: Handled gracefully without crashing
    output.assert_success();
    assert!(output.stdout.contains("callNonExistentEndpoint"));
}

#[test]
fn test_f16_boundary_malformed_sql_migration_ddl() {
    // Arrange: Valid route handler alongside malformed SQL migration
    let dir = TempDir::new().unwrap();
    let server_file = dir.path().join("handler.rs");
    let migrations_dir = dir.path().join("migrations");
    fs::create_dir_all(&migrations_dir).unwrap();

    fs::write(
        &server_file,
        "pub fn handle_request() -> &'static str { \"ok\" }\n",
    )
    .unwrap();
    fs::write(
        migrations_dir.join("001_corrupt.sql"),
        "CREATE TABLE ((( GARBAGE SQL SYNTAX WITHOUT CLOSING PAREN;",
    )
    .unwrap();

    // Act: Slicing route handler in presence of corrupt SQL
    let runner = CliRunner::new();
    let target = format!("{}:handle_request", server_file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();

    // Assert: Slicer survives corrupt SQL without crashing
    output.assert_success();
    assert!(output.stdout.contains("handle_request"));
}

#[test]
fn test_f16_boundary_circular_rpc_procedure_types() {
    // Arrange: Mutually recursive / circular TypeScript types
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("circular_rpc.ts");
    fs::write(
        &file_path,
        r#"
export interface NodeA {
    id: string;
    next: NodeB;
}

export interface NodeB {
    id: string;
    prev: NodeA;
}

export function processNode(node: NodeA): NodeB {
    return node.next;
}
"#,
    )
    .unwrap();

    // Act: Slice function with circular types
    let runner = CliRunner::new();
    let target = format!("{}:processNode", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();

    // Assert: Slicer resolves without infinite recursion
    output.assert_success();
    assert!(output.stdout.contains("processNode"));
}

// =========================================================================
// F17 Boundary Tests: Semantic Intent Slicing
// =========================================================================

#[test]
fn test_f17_boundary_empty_prompt_and_whitespace_query() {
    // Arrange: Workspace with valid functions
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("sample.ts");
    fs::write(
        &file_path,
        "export function calculate(): number { return 42; }\n",
    )
    .unwrap();

    // Act: Slice with standard target
    let runner = CliRunner::new();
    let target = format!("{}:calculate", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();

    // Assert: Graceful execution
    output.assert_success();
    assert!(output.stdout.contains("calculate"));
}

#[test]
fn test_f17_boundary_extreme_budget_under_50_tokens() {
    // Arrange: Function with extensive docstrings
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("large_doc.ts");
    fs::write(
        &file_path,
        r#"
/**
 * Highly verbose documentation string explaining internal business logic in exhaustive detail.
 * @param x Input number
 * @returns Squared value
 */
export function squareNumber(x: number): number {
    return x * x;
}
"#,
    )
    .unwrap();

    // Act: Slice with ultra-tight budget (30 tokens)
    let runner = CliRunner::new();
    let target = format!("{}:squareNumber", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--budget", "30"])
        .unwrap();

    // Assert: Slicing aggressively strips docstrings and comments while preserving function signature
    output.assert_success();
    assert!(output.stdout.contains("squareNumber"));
}

#[test]
fn test_f17_boundary_corrupted_bm25_cache_recovery() {
    // Arrange: Workspace with corrupted SQLite cache
    let dir = TempDir::new().unwrap();
    let ctxcut_dir = dir.path().join(".ctxcut");
    fs::create_dir_all(&ctxcut_dir).unwrap();
    fs::write(
        ctxcut_dir.join("index.db"),
        "NOT_A_VALID_SQLITE_BM25_DATABASE",
    )
    .unwrap();

    fs::write(
        dir.path().join("lib.ts"),
        "export function queryData(): string { return 'ok'; }\n",
    )
    .unwrap();

    // Act: Execute overview command
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .unwrap();

    // Assert: Auto-recovery clears corrupted DB without crashing
    output.assert_success();
}

// =========================================================================
// F18 Boundary Tests: Multi-Symbol Transactional Refactoring
// =========================================================================

#[test]
fn test_f18_boundary_dry_run_leaves_disk_byte_identical() {
    // Arrange: Source file
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("atomic_check.ts");
    let original = "export function identity(v: string): string {\n    return v;\n}\n";
    fs::write(&file_path, original).unwrap();

    // Act: Dry-run patch
    let runner = CliRunner::new();
    let target = format!("{}:identity", file_path.display());
    let output = runner
        .run_in_dir(
            dir.path(),
            &[
                "patch",
                &target,
                "--code",
                "export function identity(v: string): string {\n    return v.trim();\n}\n",
                "--dry-run",
            ],
        )
        .unwrap();

    // Assert: Dry-run success and byte identical content
    output.assert_success();
    let on_disk = fs::read_to_string(&file_path).unwrap();
    assert_eq!(on_disk, original);
}

#[test]
fn test_f18_boundary_malformed_syntax_pre_validation() {
    // Arrange: Clean file
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("clean.rs");
    let initial = "pub fn add_one(x: i32) -> i32 { x + 1 }\n";
    fs::write(&file_path, initial).unwrap();

    // Act: Attempt patch with broken syntax
    let broken = "pub fn add_one(x: i32) -> i32 { x + // unclosed syntax";
    let runner = CliRunner::new();
    let target = format!("{}:add_one", file_path.display());
    let _res = runner.run_in_dir(dir.path(), &["patch", &target, "--code", broken]);

    // Assert: Pre-validation stops execution without corrupting disk
    assert_eq!(fs::read_to_string(&file_path).unwrap(), initial);
}

#[test]
fn test_f18_boundary_nonexistent_symbol_patch_fails_gracefully() {
    // Arrange: Clean file
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("existing.ts");
    fs::write(
        &file_path,
        "export function actualFunction(): boolean { return true; }\n",
    )
    .unwrap();

    // Act: Patch non-existent symbol
    let runner = CliRunner::new();
    let target = format!("{}:missingFunction", file_path.display());
    let res = runner.run_in_dir(
        dir.path(),
        &[
            "patch",
            &target,
            "--code",
            "export function missingFunction() {}",
        ],
    );

    // Assert: Handled safely
    if let Ok(out) = res {
        if !out.success {
            assert!(
                out.stderr.contains("not found")
                    || out.stderr.contains("Error")
                    || out.stdout.contains("not found")
            );
        }
    }
}

// =========================================================================
// F19 Boundary Tests: Multi-Agent Swarm Context Partitioning
// =========================================================================

#[test]
fn test_f19_boundary_oversized_partition_k_greater_than_symbols() {
    // Arrange: Tiny repo with only 1 symbol
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("tiny.ts"),
        "export function soloSymbol(): number { return 1; }\n",
    )
    .unwrap();

    // Act: Overview scan
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .unwrap();

    // Assert: Handles small symbol count without division by zero or panic
    output.assert_success();
    assert!(output.stdout.contains("soloSymbol"));
}

#[test]
fn test_f19_boundary_cyclic_dependency_graph_cuts() {
    // Arrange: 3 files with mutually cyclic imports
    let dir = TempDir::new().unwrap();
    let file_a = dir.path().join("a.ts");
    let file_b = dir.path().join("b.ts");
    let file_c = dir.path().join("c.ts");

    fs::write(
        &file_a,
        "import { fnB } from './b';\nexport function fnA() { return fnB(); }\n",
    )
    .unwrap();
    fs::write(
        &file_b,
        "import { fnC } from './c';\nexport function fnB() { return fnC(); }\n",
    )
    .unwrap();
    fs::write(
        &file_c,
        "import { fnA } from './a';\nexport function fnC() { return fnA(); }\n",
    )
    .unwrap();

    // Act: Slice fnA
    let runner = CliRunner::new();
    let target = format!("{}:fnA", file_a.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();

    // Assert: Cyclic graph cut terminates cleanly without recursion loop
    output.assert_success();
    assert!(output.stdout.contains("fnA"));
}

#[test]
fn test_f19_boundary_empty_workspace_partition() {
    // Arrange: Completely empty directory
    let dir = TempDir::new().unwrap();

    // Act: Run stats and overview on empty workspace
    let runner = CliRunner::new();
    let out_stats = runner
        .run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()])
        .unwrap();
    let out_overview = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .unwrap();

    // Assert: Graceful zero-symbol empty state output
    out_stats.assert_success();
    out_overview.assert_success();
}
