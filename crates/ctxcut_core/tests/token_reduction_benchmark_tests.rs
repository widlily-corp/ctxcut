//! Token reduction and context savings benchmark validation across Python, Go, and Rust.

use std::path::Path;
use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;

#[test]
fn test_benchmark_python_realistic_service_token_reduction() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/realistic_payment_service/payment_service.py");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "PaymentProcessor.execute_charge", &opts)
        .expect("Slice should succeed");

    assert!(result.stats.raw_file_tokens > 0);
    assert!(result.stats.sliced_tokens > 0);
    assert!(
        result.stats.raw_file_tokens > result.stats.sliced_tokens,
        "Raw tokens ({}) must exceed sliced tokens ({})",
        result.stats.raw_file_tokens,
        result.stats.sliced_tokens
    );
    assert!(
        result.stats.savings_percentage >= 20.0,
        "Savings percentage must be >= 20%, got: {:.2}%",
        result.stats.savings_percentage
    );
}

#[test]
fn test_benchmark_go_realistic_auth_service_token_reduction() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/realistic_auth_service/service.go");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "AuthService.AuthenticateUser", &opts)
        .expect("Slice should succeed");

    assert!(result.stats.raw_file_tokens > 0);
    assert!(result.stats.sliced_tokens > 0);
    assert!(
        result.stats.raw_file_tokens > result.stats.sliced_tokens,
        "Raw tokens ({}) must exceed sliced tokens ({})",
        result.stats.raw_file_tokens,
        result.stats.sliced_tokens
    );
    assert!(
        result.stats.savings_percentage >= 20.0,
        "Savings percentage must be >= 20%, got: {:.2}%",
        result.stats.savings_percentage
    );
}

#[test]
fn test_benchmark_rust_realistic_inventory_service_token_reduction() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/realistic_inventory_service/inventory.rs");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "InventoryService::reserve_stock", &opts)
        .expect("Slice should succeed");

    assert!(result.stats.raw_file_tokens > 0);
    assert!(result.stats.sliced_tokens > 0);
    assert!(
        result.stats.raw_file_tokens > result.stats.sliced_tokens,
        "Raw tokens ({}) must exceed sliced tokens ({})",
        result.stats.raw_file_tokens,
        result.stats.sliced_tokens
    );
    assert!(
        result.stats.savings_percentage >= 20.0,
        "Savings percentage must be >= 20%, got: {:.2}%",
        result.stats.savings_percentage
    );
}

#[test]
fn test_benchmark_python_large_file_savings() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/large_file.py");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "analytics_module_fn_001", &opts)
        .expect("Slice should succeed on large python file");

    assert!(
        result.stats.savings_percentage >= 90.0,
        "Large file token reduction must be >= 90%, got: {:.2}%",
        result.stats.savings_percentage
    );
}

#[test]
fn test_benchmark_go_large_file_savings() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/large_file.go");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "ComputeGoClusterMetric_001", &opts)
        .expect("Slice should succeed on large go file");

    assert!(
        result.stats.savings_percentage >= 90.0,
        "Large file token reduction must be >= 90%, got: {:.2}%",
        result.stats.savings_percentage
    );
}

#[test]
fn test_benchmark_rust_large_file_savings() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/large_file.rs");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "compute_rust_engine_fn_001", &opts)
        .expect("Slice should succeed on large rust file");

    assert!(
        result.stats.savings_percentage >= 90.0,
        "Large file token reduction must be >= 90%, got: {:.2}%",
        result.stats.savings_percentage
    );
}
