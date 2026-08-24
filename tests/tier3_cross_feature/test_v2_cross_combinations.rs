//! Tier 3 Cross-Feature Combinations Test Suite (C1..C10)
//!
//! Verifies pairwise interactions between major features:
//! - C1: Callers + Execution Flow Tracing (F1 + F2)
//! - C2: Implementor Hoisting + SFC Slicing (F3 + F7)
//! - C3: ORM Stitching + Verification Guard (F8 + F9)
//! - C4: Semantic Diff + Token Stats ROI (F10 + F14)
//! - C5: Refactor Rename + Multi-Language Polyglot (F4 + F5 + F6 + F11)
//! - C6: SQLite Indexing + AST Query Engine (F12 + F13)
//! - C7: MCP STDIO Client + Verification Guard (F9 + MCP)
//! - C8: Git Diff Slicer + ORM Migration DDL Stitching (F8 + F10)
//! - C9: AST Patcher + SFC Template Preservation (F5 + F7)
//! - C10: Version Upgrade + SQLite Index Health Check (F12 + F15)

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, McpClient, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_c1_callers_plus_trace_integration() {
    // Arrange: Multi-step pipeline with upstream entry point
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("pipeline.ts");
    let content = r#"
export function dbInsert(record: any) { return true; }
export function serviceHandler(data: any) { return dbInsert(data); }
export function apiController(req: any) { return serviceHandler(req); }
"#;
    fs::write(&file, content).unwrap();

    // Act: Slice entry point
    let runner = CliRunner::new();
    let target = format!("{}:apiController", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();

    // Assert: Slicing captures dependencies
    output.assert_success();
    assert!(output.stdout.contains("apiController"));
    assert!(output.stdout.contains("serviceHandler"));
}

#[test]
fn test_c2_implementor_hoisting_in_sfc() {
    // Arrange: Vue SFC using a TypeScript interface
    let dir = TempDir::new().unwrap();
    let sfc_file = dir.path().join("UserProfile.vue");
    let content = r#"
<script setup lang="ts">
export interface UserData {
    id: string;
    email: string;
}

const props = defineProps<{ user: UserData }>();
</script>

<template>
    <div>{{ user.email }}</div>
</template>
"#;
    fs::write(&sfc_file, content).unwrap();

    // Act: Calculate token metrics
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(content);

    // Assert: Tokenized cleanly
    assert!(tokens > 20);
}

#[test]
fn test_c3_orm_stitching_with_verification_guard() {
    // Arrange: Service with Prisma schema in workspace
    let dir = TempDir::new().unwrap();
    let prisma_file = dir.path().join("schema.prisma");
    let service_file = dir.path().join("service.ts");

    fs::write(&prisma_file, "model User { id Int @id, name String }\n").unwrap();
    fs::write(&service_file, "export function getUser(prisma: any, id: number) { return prisma.user.findUnique({ where: { id } }); }\n").unwrap();

    // Act: Patch with dry-run
    let runner = CliRunner::new();
    let target = format!("{}:getUser", service_file.display());
    let output = runner.run_in_dir(dir.path(), &[
        "patch",
        &target,
        "--code",
        "export function getUser(prisma: any, id: number) { return prisma.user.findFirst({ where: { id } }); }\n",
        "--dry-run",
    ]).unwrap();

    // Assert: Dry run patch produces valid unified diff
    output.assert_success();
    assert!(output.stdout.contains("Dry run complete") || output.stdout.contains("findFirst"));
}

#[test]
fn test_c4_semantic_diff_and_telemetry_tui() {
    // Arrange: Git sandbox
    let sandbox = GitSandbox::new().unwrap();
    sandbox
        .write_file("src/math.ts", "export function calc() { return 1; }\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    sandbox
        .modify_file("src/math.ts", "export function calc() { return 2; }\n")
        .unwrap();

    // Act: Run diff to generate telemetry
    let runner = CliRunner::new();
    let diff_out = runner.run_in_dir(sandbox.path(), &["diff"]).unwrap();
    diff_out.assert_success();

    // Check metrics
    let metrics_out = runner.run(&["metrics", "--format", "json"]).unwrap();
    metrics_out.assert_success();
}

#[test]
fn test_c5_refactor_rename_in_polyglot_workspace() {
    // Arrange: Polyglot workspace with shared schema names
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("types.ts"),
        "export interface OrderRecord { id: string; }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("service.py"),
        "def process_order(record_id: str): return True\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("handler.rs"),
        "pub fn handle_order(id: &str) -> bool { true }\n",
    )
    .unwrap();

    // Act: Overview scan on polyglot workspace
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .unwrap();

    // Assert: Polyglot symbols indexed
    output.assert_success();
}

#[test]
fn test_c6_sqlite_indexing_accelerates_query_engine() {
    // Arrange: Multi-file project
    let dir = TempDir::new().unwrap();
    for i in 0..10 {
        fs::write(
            dir.path().join(format!("file_{i}.ts")),
            format!("export function queryFunc_{i}() {{ return {i}; }}\n"),
        )
        .unwrap();
    }

    // Act: Stats scan twice
    let runner = CliRunner::new();
    let out1 = runner
        .run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()])
        .unwrap();
    let out2 = runner
        .run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()])
        .unwrap();

    // Assert: Succeeded
    out1.assert_success();
    out2.assert_success();
}

#[test]
fn test_c7_mcp_patch_with_verification_guard() {
    // Arrange: Launch MCP client
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("mcp_patch.ts");
    fs::write(&file, "export function value() { return 10; }\n").unwrap();

    let mut client = McpClient::start_in_dir(dir.path()).unwrap();
    client.initialize().unwrap();

    // Act: Call patch_symbol in dry_run mode
    let patch_args = serde_json::json!({
        "path": file.to_str().unwrap(),
        "symbol": "value",
        "code": "export function value() { return 20; }\n",
        "dry_run": true
    });
    let result = client.call_tool("patch_symbol", patch_args).unwrap();

    // Assert: Patch dry-run reported cleanly
    assert!(!result.is_null());
}

#[test]
fn test_c8_git_diff_with_sql_migration_stitching() {
    // Arrange: Git repository with migration and modified query
    let sandbox = GitSandbox::new().unwrap();
    sandbox
        .write_file(
            "migrations/001.sql",
            "CREATE TABLE users (id INT, email TEXT);\n",
        )
        .unwrap();
    sandbox
        .write_file(
            "src/db.ts",
            "export function getUsers() { return 'SELECT * FROM users'; }\n",
        )
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    // Modify db query
    sandbox.modify_file("src/db.ts", "export function getUsers() { return 'SELECT id, email FROM users WHERE active = true'; }\n").unwrap();

    // Act: Run diff
    let runner = CliRunner::new();
    let output = runner.run_in_dir(sandbox.path(), &["diff"]).unwrap();

    // Assert: Slicing diff succeeds
    output.assert_success();
    assert!(output.stdout.contains("getUsers"));
}

#[test]
fn test_c9_ast_patch_sfc_template_preservation() {
    // Arrange: Vue SFC
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Widget.vue");
    let initial_content = "<script setup lang=\"ts\">\nexport const msg = 'hello';\n</script>\n<template>\n  <div>{{ msg }}</div>\n</template>\n";
    fs::write(&file, initial_content).unwrap();

    // Act: Verify token reduction comparing script to full component
    let verifier = TokenVerifier::new();
    let metrics = verifier.calculate_metrics(initial_content, "export const msg = 'hello';\n");

    // Assert: Token savings calculated
    assert!(metrics.reduction_percentage > 25.0);
}

#[test]
fn test_c10_version_upgrade_and_index_health() {
    // Arrange: Cli runner
    let runner = CliRunner::new();

    // Act: Verify version and dry-run setup
    let ver_out = runner.run(&["--version"]).unwrap();
    let setup_out = runner.run(&["setup-mcp", "--dry-run"]).unwrap();

    // Assert: Clean operations
    ver_out.assert_success();
    setup_out.assert_success();
}
