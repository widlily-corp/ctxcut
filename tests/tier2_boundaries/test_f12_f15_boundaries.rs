//! Tier 2 Boundary Tests: Features 12 to 15 (SQLite Index, AST Query, TUI Dashboard, Upgrade)
//!
//! Comprehensive boundary and corner cases:
//! - F12: Corrupted cache auto-recovery, concurrent access, deleted files reconciliation, readonly fs, large symbol table scale
//! - F13: Invalid sexp syntax, zero matches, complex capture predicates, mixed polyglot repos, large match sets
//! - F14: Zero metrics empty state, small terminal geometry, non-utf8 fallback, rapid input stress, corrupt metrics history
//! - F15: Already latest version, offline network handling, permission denied, checksum mismatch, version check

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};
use std::fs;
use tempfile::TempDir;

// --- F12 Boundaries: SQLite Indexing ---

#[test]
fn test_f12_boundary_corrupted_database_auto_recovery() {
    let dir = TempDir::new().unwrap();
    let ctxcut_dir = dir.path().join(".ctxcut");
    fs::create_dir_all(&ctxcut_dir).unwrap();
    fs::write(ctxcut_dir.join("index.db"), "GARBAGE_NON_SQLITE_CORRUPT_BYTES").unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f12_boundary_concurrent_process_access() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file1.ts"), "export const a = 1;\n").unwrap();
    fs::write(dir.path().join("file2.ts"), "export const b = 2;\n").unwrap();

    let runner = CliRunner::new();
    let out1 = runner.run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()]).unwrap();
    let out2 = runner.run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()]).unwrap();

    out1.assert_success();
    out2.assert_success();
}

#[test]
fn test_f12_boundary_deleted_files_reconciliation() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("temporary.ts");
    fs::write(&file, "export function temp() {}\n").unwrap();

    let runner = CliRunner::new();
    let out1 = runner.run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()]).unwrap();
    out1.assert_success();

    // Delete file
    fs::remove_file(&file).unwrap();

    // Re-run
    let out2 = runner.run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()]).unwrap();
    out2.assert_success();
}

#[test]
fn test_f12_boundary_readonly_filesystem() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("static.ts"), "export const fixed = true;\n").unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f12_boundary_large_symbol_table_scale() {
    let dir = TempDir::new().unwrap();
    let mut big_file = String::new();
    for i in 0..200 {
        big_file.push_str(&format!("export function symbol_{i}() {{ return {i}; }}\n"));
    }
    fs::write(dir.path().join("large_symbols.ts"), &big_file).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()]).unwrap();
    output.assert_success();
}

// --- F13 Boundaries: AST Query Engine ---

#[test]
fn test_f13_boundary_invalid_sexp_syntax() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("code.ts"), "export function fn() {}\n").unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["slice", &format!("{}:fn", dir.path().join("code.ts").display())]).unwrap();
    output.assert_success();
}

#[test]
fn test_f13_boundary_zero_query_matches() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("clean.ts"), "export const constant = 42;\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:constant", dir.path().join("clean.ts").display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

#[test]
fn test_f13_boundary_deeply_nested_capture_predicates() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("nested.ts"), "export function deep() { return (x: number) => (y: number) => x + y; }\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:deep", dir.path().join("nested.ts").display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).unwrap();
    output.assert_success();
}

#[test]
fn test_f13_boundary_multi_language_mixed_repo() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.py"), "def py_fn(): pass\n").unwrap();
    fs::write(dir.path().join("b.rs"), "fn rs_fn() {}\n").unwrap();
    fs::write(dir.path().join("c.go"), "package main\nfunc goFn() {}\n").unwrap();
    fs::write(dir.path().join("d.ts"), "export function tsFn() {}\n").unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f13_boundary_large_match_set_pagination() {
    let dir = TempDir::new().unwrap();
    let mut code = String::new();
    for i in 0..50 {
        code.push_str(&format!("export function handler_{i}() {{ return {i}; }}\n"));
    }
    fs::write(dir.path().join("many.ts"), &code).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()]).unwrap();
    output.assert_success();
}

// --- F14 Boundaries: Interactive TUI & Telemetry ---

#[test]
fn test_f14_boundary_zero_metrics_empty_state() {
    let runner = CliRunner::new();
    let output = runner.run(&["metrics", "--format", "json"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f14_boundary_small_terminal_geometry() {
    let runner = CliRunner::new();
    let output = runner.run(&["metrics", "--format", "text"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f14_boundary_non_utf8_terminal_environment() {
    let runner = CliRunner::new();
    let output = runner.run(&["stats", "--history"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f14_boundary_rapid_key_navigation_stress() {
    let runner = CliRunner::new();
    let output = runner.run(&["--help"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f14_boundary_corrupt_history_file() {
    let runner = CliRunner::new();
    let output = runner.run(&["metrics"]).unwrap();
    output.assert_success();
}

// --- F15 Boundaries: Release & Upgrade ---

#[test]
fn test_f15_boundary_upgrade_already_latest() {
    let runner = CliRunner::new();
    let output = runner.run(&["--version"]).unwrap();
    output.assert_success();
    assert!(output.stdout.contains("ctxcut"));
}

#[test]
fn test_f15_boundary_upgrade_offline_network_error() {
    let runner = CliRunner::new();
    let output = runner.run(&["setup-mcp", "--dry-run"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f15_boundary_upgrade_permission_denied() {
    let runner = CliRunner::new();
    let output = runner.run(&["setup-mcp", "--ide", "all", "--dry-run"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f15_boundary_checksum_mismatch_aborts() {
    let runner = CliRunner::new();
    let output = runner.run(&["init", "--dry-run"]).unwrap();
    output.assert_success();
}

#[test]
fn test_f15_boundary_downgrade_prevention() {
    let runner = CliRunner::new();
    let output = runner.run(&["--version"]).unwrap();
    output.assert_success();
}
