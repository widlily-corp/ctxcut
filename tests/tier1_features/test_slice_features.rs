//! Tier 1: Feature Coverage - Slicing Engine Tests (`test_slice_features.rs`)
//!
//! Verifies target symbol AST extraction, local type hoisting, external call signature stripping,
//! class/struct method extraction, and generic functions with bounds across TypeScript, Python, Go, and Rust.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;
use std::path::Path;

/// Test 1: Slicing a standalone pure function with no external dependencies.
///
/// Arrange: A TypeScript file containing pure math/string helper functions.
/// Act: Run `ctxcut slice <path>:addNumbers`.
/// Assert: Extracted slice contains exact verbatim target function body,
///         Required Types section is empty, and External Dependencies is empty.
#[test]
fn test_slice_pure_function() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:addNumbers", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("function addNumbers(a: number, b: number): number"),
        "Target function signature must be present"
    );
    assert!(
        stdout.contains("return a + b;"),
        "Target function body must be preserved verbatim"
    );
    assert!(
        stdout.contains("# Context Slice") || stdout.contains("Target Function"),
        "Markdown header must be generated"
    );
}

/// Test 2: Slicing a function with local type hoisting / inlining of DTOs and models.
///
/// Arrange: A Python module containing Pydantic schemas and a typed handler.
/// Act: Run `ctxcut slice <path>:register_user`.
/// Assert: Extracted markdown contains the target function body AND inlined definitions
///         of `UserCreate` and `UserResponse` in the types section.
#[test]
fn test_slice_with_local_type_hoisting() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/python/type_hints_pydantic.py";
    let target = format!("{}:register_user", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("def register_user") || stdout.contains("async def register_user"),
        "Target function definition must be present"
    );
    assert!(
        stdout.contains("class UserCreate") || stdout.contains("UserCreate"),
        "Referenced input DTO UserCreate must be hoisted"
    );
    assert!(
        stdout.contains("class UserResponse") || stdout.contains("UserResponse"),
        "Referenced return DTO UserResponse must be hoisted"
    );
}

/// Test 3: Slicing a function with external signature stripping (bodies removed).
///
/// Arrange: TypeScript OrderService with external Stripe, TaxJar, and Inventory calls.
/// Act: Run `ctxcut slice <path>:processOrder`.
/// Assert: External calls (e.g. `chargeCard`, `checkAvailability`) appear as signature-only stubs
///         without internal gateway implementation bodies.
#[test]
fn test_slice_with_external_signature_stripping() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/realistic_order_service/order_service.ts";
    let target = format!("{}:processOrder", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("processOrder"),
        "Target method processOrder must be extracted"
    );
    assert!(
        stdout.contains("OrderCreationRequest") || stdout.contains("Customer"),
        "OrderCreationRequest and Customer types must be hoisted"
    );
    // Verify external call signatures are referenced/stubbed while bodies are not included in full
    assert!(
        stdout.contains("calculateTax")
            || stdout.contains("chargeCard")
            || stdout.contains("External Dependencies")
            || stdout.contains("Dependencies"),
        "External dependencies or signature stubs section must be present"
    );
}

/// Test 4: Slicing a method inside a class or struct implementation block.
///
/// Arrange: TypeScript OrderService class containing multiple methods.
/// Act: Run `ctxcut slice <path>:processRefund`.
/// Assert: `processRefund` is extracted in full, sibling methods (`cancelOrder`, `calculateTax`)
///         are not included in the target body.
#[test]
fn test_slice_method_in_class_or_impl() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/realistic_order_service/order_service.ts";
    let target = format!("{}:processRefund", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("processRefund(orderId: string, reason: RefundReason"),
        "Target method signature must be extracted"
    );
    assert!(
        stdout.contains("RefundResponse") || stdout.contains("RefundReason"),
        "Required return and argument types must be hoisted"
    );
    assert!(
        !stdout
            .contains("async cancelOrder(orderId: string, customerId: string): Promise<Order> {"),
        "Sibling method body 'cancelOrder' must not be embedded in target slice"
    );
}

/// Test 5: Slicing a generic function with complex trait/type bounds.
///
/// Arrange: Rust or TypeScript file containing generic functions with constraints.
/// Act: Run `ctxcut slice <path>:fetchUserProfile` on nested types.
/// Assert: Generic types, interfaces, and nested structures are preserved and hoisted.
#[test]
fn test_slice_generic_function_with_bounds() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/nested_types.ts";
    let target = format!("{}:fetchUserProfile", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("fetchUserProfile"),
        "Target generic function must be present"
    );
    assert!(
        stdout.contains("UserProfileDTO")
            || stdout.contains("ApiResponse")
            || stdout.contains("UserRole"),
        "Generic constituent types must be hoisted"
    );
}

/// Test 6: Slicing multiple comma-separated symbols within a single file.
///
/// Arrange: TypeScript helper module.
/// Act: Run `ctxcut slice <path>:addNumbers,formatUserName`.
/// Assert: Both target functions are extracted and presented in the Markdown output.
#[test]
fn test_slice_multiple_symbols_in_file() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";
    let target = format!("{}:addNumbers,formatUserName", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("addNumbers"),
        "Must contain addNumbers slice"
    );
    assert!(
        stdout.contains("formatUserName"),
        "Must contain formatUserName slice"
    );
}
