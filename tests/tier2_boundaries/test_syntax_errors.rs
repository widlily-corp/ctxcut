//! Tier 2: Boundary & Corner Cases - Syntax Errors & Parser Error Recovery (`test_syntax_errors.rs`)
//!
//! Verifies Tree-sitter error recovery resilience when extracting valid target symbols
//! from files containing unclosed brackets, broken indentation, missing colons/braces,
//! and corrupted surrounding code without crashing.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

/// Test 1: Extracting an intact TypeScript function surrounded by unclosed braces and malformed expressions.
///
/// Arrange: TypeScript file `malformed_syntax.ts` with broken functions above and below `intactTargetFunction`.
/// Act: Run `ctxcut slice tests/fixtures/typescript/malformed_syntax.ts:intactTargetFunction`.
/// Assert: Tree-sitter error recovery extracts `intactTargetFunction` with exact body `return x * y + 42;`.
#[test]
fn test_unclosed_brackets_ts_error_recovery() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/malformed_syntax.ts";
    let target = format!("{}:intactTargetFunction", file_path);

    // Act
    let output = runner.run(&["slice", &target]).expect("Failed to execute slice on malformed TS");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("intactTargetFunction"),
        "Must extract intact function despite surrounding syntax errors"
    );
    assert!(
        stdout.contains("return x * y + 42;"),
        "Must preserve intact function body verbatim"
    );
}

/// Test 2: Extracting a valid Python function from a module with severe indentation and syntax faults.
///
/// Arrange: Python file `syntax_errors.py` with broken indentation and missing colons.
/// Act: Run `ctxcut slice tests/fixtures/python/syntax_errors.py:valid_header_function`.
/// Assert: Extracts `valid_header_function` without parsing panic.
#[test]
fn test_python_indentation_fault_recovery() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/python/syntax_errors.py";
    let target = format!("{}:valid_header_function", file_path);

    // Act
    let output = runner.run(&["slice", &target]).expect("Failed to execute slice on malformed Python");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("valid_header_function"),
        "Must extract valid function from noisy Python file"
    );
    assert!(
        stdout.contains("return x + y"),
        "Must preserve valid function body"
    );
}

/// Test 3: Slicing a valid Go function in a file with missing closing braces in other functions.
///
/// Arrange: Go file with a corrupted function followed by a valid function.
/// Act: Run `ctxcut slice <temp_file>:ValidGoFunction`.
/// Assert: Extracts `ValidGoFunction` cleanly.
#[test]
fn test_go_syntax_error_recovery() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let go_code = r#"
package main

func BrokenFunc(a int) int {
    if a > 0 {
        return a * 2
    // Missing closing braces

func ValidGoFunction(x int, y int) int {
    return x + y + 100
}
"#;
    let file_path = temp_dir.path().join("broken.go");
    fs::write(&file_path, go_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:ValidGoFunction", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Failed to execute slice on malformed Go");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("ValidGoFunction"), "Must extract ValidGoFunction");
    assert!(stdout.contains("return x + y + 100"), "Must preserve Go body");
}

/// Test 4: Completely unparseable garbage / binary tokens.
///
/// Arrange: File containing random non-code binary bytes.
/// Act: Attempt to slice a symbol.
/// Assert: Fails cleanly with error; 0 panics or memory corruption.
#[test]
fn test_completely_unparseable_garbage() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let garbage_path = temp_dir.path().join("garbage.ts");
    fs::write(&garbage_path, &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF, 0xFE, 0x12, 0x34]).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:nonExistent", garbage_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command execution failed");

    // Assert
    output.assert_failure();
    assert!(!output.stderr.contains("panic"), "Must not panic on binary garbage");
}

/// Test 5: Corrupted type definition in same file does not crash target function extraction.
///
/// Arrange: TypeScript file with broken interface definition above valid target function.
/// Act: Run `ctxcut slice <path>:targetWithBrokenNeighborType`.
/// Assert: Target function is extracted without panic.
#[test]
fn test_corrupted_type_definition_tolerance() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let ts_code = r#"
export interface BrokenInterface<T {
    key: T
    invalid syntax ::: here

export function targetWithBrokenNeighborType(val: string): string {
    return `Hello ${val}`;
}
"#;
    let file_path = temp_dir.path().join("corrupted_type.ts");
    fs::write(&file_path, ts_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:targetWithBrokenNeighborType", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command execution failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("targetWithBrokenNeighborType"));
    assert!(stdout.contains("Hello ${val}"));
}
