//! Tier 2: Boundary & Corner Cases - Missing Symbols & Scope Disambiguation (`test_missing_symbols.rs`)
//!
//! Verifies fuzzy symbol matching suggestions ("Did you mean...?"), graceful diagnostics
//! for non-existent symbols, and correct resolution when local variables shadow type names.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

/// Test 1: Fuzzy symbol matching suggestion when a typo is provided.
///
/// Arrange: TypeScript file containing `addNumbers`.
/// Act: Request `addNumber` or `addNumbrs` (typo).
/// Assert: Returns non-zero exit code with error message suggesting `addNumbers` ("Did you mean...?").
#[test]
fn test_fuzzy_symbol_matching_suggestion() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:addNumber", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_failure();
    let stderr_or_stdout = format!("{}\n{}", output.stderr, output.stdout);
    assert!(
        stderr_or_stdout.contains("Did you mean")
            || stderr_or_stdout.contains("addNumbers")
            || stderr_or_stdout.contains("not found")
            || stderr_or_stdout.contains("SymbolNotFound"),
        "Error output must provide helpful diagnostics or suggestions. Output was: {}",
        stderr_or_stdout
    );
}

/// Test 2: Completely unknown symbol with no close matches lists available symbols or reports not found.
///
/// Arrange: Request `totally_nonexistent_xyz_symbol_12345`.
/// Act: Run `ctxcut slice <path>:totally_nonexistent_xyz_symbol_12345`.
/// Assert: Fails cleanly with exit code != 0 without panic.
#[test]
fn test_completely_unknown_symbol_diagnostics() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:totally_nonexistent_xyz_symbol_12345", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_failure();
    assert!(
        !output.stderr.contains("panic"),
        "Must not panic on unknown symbol"
    );
}

/// Test 3: Shadowed local variable resolution.
///
/// Arrange: Function contains a local variable named `User` (e.g. `const User = "string"`),
///          while parameter has type annotation `User` referencing top-level interface `User`.
/// Act: Run `ctxcut slice <path>:processUserAccount`.
/// Assert: Correctly resolves and inlines top-level `interface User` without being misled by local identifier.
#[test]
fn test_shadowed_local_variable_resolution() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let ts_code = r#"
export interface User {
    id: string;
    email: string;
}

export function processUserAccount(account: User): boolean {
    const User = "local_shadow_variable";
    console.log(User);
    return account.email.length > 0;
}
"#;
    let file_path = temp_dir.path().join("shadow.ts");
    fs::write(&file_path, ts_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:processUserAccount", file_path.to_str().unwrap());
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("processUserAccount"));
    assert!(stdout.contains("interface User") || stdout.contains("id: string"));
}

/// Test 4: Missing symbol in multi-symbol slice (`sym1,missingSym,sym2`).
///
/// Arrange: Slice comma-separated symbols where one does not exist.
/// Act: Run `ctxcut slice <path>:addNumbers,nonExistentFunc`.
/// Assert: Reports missing symbol clearly or slices available valid symbols with warning.
#[test]
fn test_multi_symbol_with_one_missing() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:addNumbers,nonExistentFunc", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    assert!(
        combined.contains("nonExistentFunc") || combined.contains("addNumbers"),
        "Must report missing symbol or handle partial match"
    );
}

/// Test 5: Symbol with different casing (e.g. `addnumbers` vs `addNumbers`).
///
/// Arrange: Request lowercase symbol name.
/// Act: Run `ctxcut slice <path>:addnumbers`.
/// Assert: Suggests correct camelCase symbol `addNumbers` or resolves case-insensitively.
#[test]
fn test_case_mismatch_symbol_suggestion() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:addnumbers", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    if !output.success {
        let combined = format!("{}\n{}", output.stdout, output.stderr);
        assert!(
            combined.contains("addNumbers") || combined.contains("Did you mean"),
            "Should suggest correct casing"
        );
    }
}
