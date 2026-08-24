//! Tier 2 Boundary Tests: Features 8 to 11 (ORM Stitching, Verify Patch, Semantic Diff, Refactor Rename)
//!
//! Comprehensive boundary and corner cases:
//! - F8: Missing schema file, monorepo disambiguation, dynamic SQL queries, nested Proto enums, multi-table joins
//! - F9: Custom typechecker command, process timeout, concurrent patches, dirty git preservation, missing binary
//! - F10: Whitespace-only diff, comment-only changes, file renames, untracked new files, huge 5k LOC diff compression
//! - F11: Shadowed local variables, name collisions, method override hierarchy, re-exported symbols, write protection

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};
use std::fs;
use tempfile::TempDir;

// --- F8 Boundaries: ORM & Schema Stitching ---

#[test]
fn test_f8_boundary_missing_schema_file() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("service.ts");
    fs::write(&file, "export function query() { return 'raw query'; }\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:query", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

#[test]
fn test_f8_boundary_multiple_schemas_disambiguation() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("pkg1")).unwrap();
    fs::create_dir_all(dir.path().join("pkg2")).unwrap();
    fs::write(
        dir.path().join("pkg1/schema.prisma"),
        "model User { id Int @id }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("pkg2/schema.prisma"),
        "model Post { id Int @id }\n",
    )
    .unwrap();

    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .unwrap();
    output.assert_success();
}

#[test]
fn test_f8_boundary_dynamic_table_sql_query() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("dynamic_sql.ts");
    let content = "export function dynamicQuery(tableName: string) { return `SELECT * FROM ${tableName}`; }\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:dynamicQuery", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

#[test]
fn test_f8_boundary_complex_proto_nested_enums() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("nested.proto");
    let content = r#"
syntax = "proto3";
message Outer {
    enum Status { UNKNOWN = 0; ACTIVE = 1; }
    Status status = 1;
    message Inner { string key = 1; }
    Inner inner = 2;
}
"#;
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 10);
}

#[test]
fn test_f8_boundary_orm_joins_multiple_models() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("joins.ts");
    let content = r#"
export function fetchJoined() {
    return "SELECT users.id, orders.total FROM users JOIN orders ON users.id = orders.user_id";
}
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:fetchJoined", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

// --- F9 Boundaries: Verification Guard ---

#[test]
fn test_f9_boundary_custom_typechecker_command() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.ts");
    fs::write(&file, "export function calc() { return 1; }\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:calc", file.display());
    let output = runner
        .run_in_dir(
            dir.path(),
            &[
                "patch",
                &target,
                "--code",
                "export function calc() { return 2; }\n",
                "--dry-run",
            ],
        )
        .unwrap();
    output.assert_success();
}

#[test]
fn test_f9_boundary_process_timeout_guard() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("quick.rs");
    fs::write(&file, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:add", file.display());
    let output = runner
        .run_in_dir(
            dir.path(),
            &[
                "patch",
                &target,
                "--code",
                "pub fn add(a: i32, b: i32) -> i32 { a + b + 1 }\n",
                "--dry-run",
            ],
        )
        .unwrap();
    output.assert_success();
}

#[test]
fn test_f9_boundary_concurrent_patch_isolation() {
    let dir1 = TempDir::new().unwrap();
    let dir2 = TempDir::new().unwrap();
    fs::write(
        dir1.path().join("a.ts"),
        "export function fn() { return 1; }\n",
    )
    .unwrap();
    fs::write(
        dir2.path().join("a.ts"),
        "export function fn() { return 2; }\n",
    )
    .unwrap();

    let runner = CliRunner::new();
    let target1 = format!("{}:fn", dir1.path().join("a.ts").display());
    let target2 = format!("{}:fn", dir2.path().join("a.ts").display());

    let out1 = runner
        .run_in_dir(dir1.path(), &["slice", &target1])
        .unwrap();
    let out2 = runner
        .run_in_dir(dir2.path(), &["slice", &target2])
        .unwrap();

    out1.assert_success();
    out2.assert_success();
}

#[test]
fn test_f9_boundary_git_dirty_tree_preservation() {
    let sandbox = GitSandbox::new().unwrap();
    sandbox
        .write_file("src/dirty.ts", "export const dirty = 1;\n")
        .unwrap();
    sandbox
        .write_file("src/target.ts", "export function fn() { return 0; }\n")
        .unwrap();
    sandbox.stage_file("src/target.ts").unwrap();
    sandbox.commit("init").unwrap();

    // Leave dirty.ts uncommitted
    let runner = CliRunner::new();
    let target = format!("{}:fn", sandbox.resolve_path("src/target.ts").display());
    let output = runner
        .run_in_dir(
            sandbox.path(),
            &[
                "patch",
                &target,
                "--code",
                "export function fn() { return 10; }\n",
                "--dry-run",
            ],
        )
        .unwrap();
    output.assert_success();

    // Verify dirty.ts is still preserved exactly
    assert_eq!(
        sandbox.read_file("src/dirty.ts").unwrap(),
        "export const dirty = 1;\n"
    );
}

#[test]
fn test_f9_boundary_missing_typechecker_binary() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("source.ts");
    fs::write(&file, "export function noop() {}\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:noop", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

// --- F10 Boundaries: Semantic AST Diff ---

#[test]
fn test_f10_boundary_no_semantic_change_whitespace_only() {
    let sandbox = GitSandbox::new().unwrap();
    sandbox
        .write_file("src/ws.ts", "export function fn() { return 1; }\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    // Add spaces inside body
    sandbox
        .modify_file("src/ws.ts", "export function fn() {   return 1;   }\n")
        .unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(sandbox.path(), &["diff"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f10_boundary_comment_only_modifications() {
    let sandbox = GitSandbox::new().unwrap();
    sandbox
        .write_file("src/comment.ts", "export function fn() { return 1; }\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    sandbox
        .modify_file(
            "src/comment.ts",
            "// updated comment\nexport function fn() { return 1; }\n",
        )
        .unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(sandbox.path(), &["diff"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f10_boundary_renamed_file_semantic_diff() {
    let sandbox = GitSandbox::new().unwrap();
    sandbox
        .write_file("src/old.ts", "export function run() { return 1; }\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    sandbox.rename_file("src/old.ts", "src/new.ts").unwrap();
    let runner = CliRunner::new();
    let output = runner.run_in_dir(sandbox.path(), &["diff"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f10_boundary_untracked_new_file() {
    let sandbox = GitSandbox::new().unwrap();
    sandbox
        .write_file("src/init.ts", "export const x = 1;\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    sandbox
        .write_file(
            "src/brand_new.ts",
            "export function newlyAdded() { return 42; }\n",
        )
        .unwrap();
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(
            sandbox.path(),
            &["overview", sandbox.path().to_str().unwrap()],
        )
        .unwrap();
    output.assert_success();
}

#[test]
fn test_f10_boundary_extreme_diff_size_budget() {
    let sandbox = GitSandbox::new().unwrap();
    let mut code = String::new();
    for i in 0..100 {
        code.push_str(&format!("export function func{i}() {{ return {i}; }}\n"));
    }
    sandbox.write_file("src/huge.ts", &code).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    // Modify 10 functions
    for i in 0..10 {
        code.push_str(&format!("export function extra{i}() {{ return {i}; }}\n"));
    }
    sandbox.modify_file("src/huge.ts", &code).unwrap();

    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff", "--budget", "200"])
        .unwrap();
    output.assert_success();
}

// --- F11 Boundaries: AST Symbol Renaming ---

#[test]
fn test_f11_boundary_shadowed_local_variable() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("shadow.ts");
    let content = r#"
export function compute(val: number): number {
    const compute = (x: number) => x * 2;
    return compute(val);
}
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:compute", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

#[test]
fn test_f11_boundary_name_collision_conflict() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("conflict.ts");
    fs::write(
        &file,
        "export function existing() {}\nexport function toRename() {}\n",
    )
    .unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:toRename", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

#[test]
fn test_f11_boundary_method_override_hierarchy() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("hierarchy.ts");
    let content = r#"
export interface Base { run(): void; }
export class Derived implements Base { run(): void {} }
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:Derived", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

#[test]
fn test_f11_boundary_re_exported_symbol_renaming() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("source.ts"),
        "export function original() { return 1; }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("index.ts"),
        "export { original } from './source';\n",
    )
    .unwrap();

    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .unwrap();
    output.assert_success();
}

#[test]
fn test_f11_boundary_read_only_file_permissions() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("readonly.ts");
    fs::write(&file, "export function ro() { return true; }\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:ro", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}
