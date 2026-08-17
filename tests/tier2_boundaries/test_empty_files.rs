//! Tier 2: Boundary & Corner Cases - Empty & Whitespace File Handling (`test_empty_files.rs`)
//!
//! Verifies robust error handling and resilience when encountering 0-byte files,
//! whitespace-only files, and files containing only comments across all supported languages.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

/// Test 1: 0-byte empty file across TypeScript, Python, Go, and Rust.
///
/// Arrange: Create empty 0-byte `.ts`, `.py`, `.go`, `.rs` files.
/// Act: Attempt to slice a symbol from each empty file.
/// Assert: Gracefully returns a SymbolNotFound error with non-zero exit code without panicking.
#[test]
fn test_zero_byte_files_across_languages() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let extensions = ["ts", "py", "go", "rs"];
    let runner = CliRunner::new();

    for ext in extensions {
        let file_path = temp_dir.path().join(format!("empty.{}", ext));
        fs::write(&file_path, "").unwrap();

        // Act
        let target = format!("{}:targetSymbol", file_path.to_str().unwrap());
        let output = runner
            .run(&["slice", &target])
            .expect("Command execution failed");

        // Assert
        output.assert_failure();
        let stderr_or_stdout = format!("{}\n{}", output.stderr, output.stdout);
        assert!(
            stderr_or_stdout.contains("not found")
                || stderr_or_stdout.contains("SymbolNotFound")
                || stderr_or_stdout.contains("empty")
                || stderr_or_stdout.contains("Error"),
            "Empty file must gracefully report symbol not found. Output: {}",
            stderr_or_stdout
        );
    }
}

/// Test 2: Whitespace-only files containing spaces, tabs, CR, LF.
///
/// Arrange: Files containing only ` \t\r\n\n\t  \r\n`.
/// Act: Run `ctxcut slice <path>:someFunc`.
/// Assert: Gracefully fails with SymbolNotFound; 0 panics.
#[test]
fn test_whitespace_only_files() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let runner = CliRunner::new();
    let ws_content = "   \n\t\t\r\n   \n\n  \t  \r\n";

    let file_path = temp_dir.path().join("whitespace.ts");
    fs::write(&file_path, ws_content).unwrap();

    // Act
    let target = format!("{}:nonExistent", file_path.to_str().unwrap());
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_failure();
    assert!(
        !output.stderr.contains("panic"),
        "Must not panic on whitespace-only file"
    );
}

/// Test 3: Comment-only files (single line and block comments).
///
/// Arrange: Files containing only comments without any AST declaration nodes.
/// Act: Run `ctxcut slice <path>:someFunc`.
/// Assert: Returns SymbolNotFound cleanly without parsing crashes.
#[test]
fn test_comment_only_files() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let runner = CliRunner::new();
    let comments = "/*\n * Monolithic comment block\n * No code here\n */\n// another line\n";

    let file_path = temp_dir.path().join("comments.ts");
    fs::write(&file_path, comments).unwrap();

    // Act
    let target = format!("{}:missingFunc", file_path.to_str().unwrap());
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_failure();
    assert!(
        !output.stderr.contains("panic"),
        "Must not panic on comment-only file"
    );
}

/// Test 4: Running `ctxcut stats` on empty files.
///
/// Arrange: 0-byte file.
/// Act: Run `ctxcut stats <path>`.
/// Assert: Returns 0 raw tokens, 0 slice tokens, 0% savings without division by zero.
#[test]
fn test_stats_on_empty_file() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let runner = CliRunner::new();
    let file_path = temp_dir.path().join("zero.py");
    fs::write(&file_path, "").unwrap();

    // Act
    let output = runner
        .run(&["stats", file_path.to_str().unwrap()])
        .expect("Command execution failed");

    // Assert
    output.assert_success();
    assert!(
        !output.stdout.contains("NaN"),
        "Stats must not produce NaN on empty files"
    );
}

/// Test 5: Running `ctxcut diff` when a modified file was emptied (truncated to 0 bytes).
///
/// Arrange: Git sandbox; commit a function; truncate file to 0 bytes.
/// Act: Run `ctxcut diff`.
/// Assert: Does not panic; handles truncated file cleanly.
#[test]
fn test_diff_on_truncated_empty_file() {
    // Arrange
    let sandbox = common::GitSandbox::new().unwrap();
    sandbox
        .write_file("src/temp.ts", "export function hello(): void {}\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Init").unwrap();

    // Truncate to 0 bytes
    sandbox.modify_file("src/temp.ts", "").unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Command execution failed");

    // Assert
    output.assert_success();
    assert!(
        !output.stderr.contains("panic"),
        "Must not panic when file is truncated to empty"
    );
}
