//! Tier 3: Cross-Feature - Multi-Symbol Slicing, File Output & Clipboard Integration (`test_multi_symbol_clip.rs`)
//!
//! Verifies slicing multiple symbols simultaneously across one or more files, saving output
//! to a designated file with `-o`, clipboard copying with `--clip`, and deduplicating shared hoisted types.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

/// Test 1: Slicing multiple symbols from the same file with type deduplication.
///
/// Arrange: TypeScript OrderService containing `processOrder` and `processRefund`.
///          Both methods reference `OrderStatus` and `Customer`.
/// Act: Run `ctxcut slice <path>:processOrder,processRefund`.
/// Assert: Output contains both function bodies, and shared types (`OrderStatus`, `Customer`)
///         are inlined exactly ONCE in the combined Required Types section (deduplicated).
#[test]
fn test_multi_symbol_slicing_with_type_deduplication() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/realistic_order_service/order_service.ts";
    let target = format!("{}:processOrder,processRefund", file_path);

    // Act
    let output = runner.run(&["slice", &target]).expect("Failed to execute multi-symbol slice");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;

    // Both target methods must be present
    assert!(stdout.contains("processOrder"), "Must contain processOrder method");
    assert!(stdout.contains("processRefund"), "Must contain processRefund method");

    // Shared types must be deduplicated
    let order_status_count = stdout.matches("enum OrderStatus").count();
    let customer_count = stdout.matches("interface Customer").count();
    assert!(
        order_status_count <= 1,
        "Shared enum OrderStatus must be deduplicated (found {} times)",
        order_status_count
    );
    assert!(
        customer_count <= 1,
        "Shared interface Customer must be deduplicated (found {} times)",
        customer_count
    );
}

/// Test 2: Writing multi-symbol slice output to a file using `-o <out_file>`.
///
/// Arrange: Temporary directory for output file.
/// Act: Run `ctxcut slice <path>:addNumbers,clamp -o <temp_file>`.
/// Assert: The destination file is created, is non-empty, and contains the generated Markdown slice.
#[test]
fn test_slice_file_output_flag() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let out_file = temp_dir.path().join("output_slice.md");
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:addNumbers,clamp", file_path);

    // Act
    let output = runner
        .run(&["slice", &target, "-o", out_file.to_str().unwrap()])
        .expect("Failed to execute slice with -o flag");

    // Assert
    output.assert_success();
    assert!(out_file.exists(), "Output file must be created at specified path");

    let saved_content = fs::read_to_string(&out_file).expect("Must read created output file");
    assert!(saved_content.contains("addNumbers"), "Saved file must contain addNumbers");
    assert!(saved_content.contains("clamp"), "Saved file must contain clamp");
}

/// Test 3: Slicing with `--clip` (clipboard copy) in headless or desktop environments.
///
/// Arrange: Slicing a helper function with `--clip`.
/// Act: Run `ctxcut slice <path>:addNumbers --clip`.
/// Assert: Completes successfully with exit code 0; gracefully handles headless environment without crashing.
#[test]
fn test_slice_clipboard_flag_execution() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:addNumbers", file_path);

    // Act
    let output = runner
        .run(&["slice", &target, "--clip"])
        .expect("Failed to execute slice with --clip flag");

    // Assert
    output.assert_success();
    assert!(
        output.stdout.contains("addNumbers") || output.stderr.contains("clipboard") || output.stdout.contains("clipboard") || output.stdout.contains("Copied"),
        "Must either output markdown or report clipboard copy"
    );
}

/// Test 4: Combined `-o` file output AND `--clip` clipboard copy.
///
/// Arrange: Temporary file path.
/// Act: Run `ctxcut slice <path>:formatUserName -o <out_file> --clip`.
/// Assert: Output file is written AND clipboard operation succeeds.
#[test]
fn test_slice_combined_file_output_and_clip() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let out_file = temp_dir.path().join("combined_slice.md");
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:formatUserName", file_path);

    // Act
    let output = runner
        .run(&["slice", &target, "-o", out_file.to_str().unwrap(), "--clip"])
        .expect("Failed to execute slice with -o and --clip");

    // Assert
    output.assert_success();
    assert!(out_file.exists(), "Output file must exist");
    let saved = fs::read_to_string(&out_file).unwrap();
    assert!(saved.contains("formatUserName"), "Saved file must contain formatUserName");
}

/// Test 5: Multi-symbol slicing where one symbol is a class and another is a standalone function.
///
/// Arrange: TypeScript OrderService file with class `OrderService` and interface/function.
/// Act: Run `ctxcut slice <path>:OrderService,IOrderRepository`.
/// Assert: Slices class and interface structures cleanly into unified document.
#[test]
fn test_multi_symbol_class_and_interface_slicing() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/realistic_order_service/order_service.ts";
    let target = format!("{}:OrderService,IOrderRepository", file_path);

    // Act
    let output = runner.run(&["slice", &target]).expect("Command failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("OrderService"));
    assert!(stdout.contains("IOrderRepository"));
}
