//! Integration tests for Rust AST parsing, symbol location, type hoisting, and slicing.

use ctxcut_core::error::CoreError;
use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;
use std::path::Path;

#[test]
fn test_slice_rust_standalone_function() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/simple_fn.rs");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "add_numbers", &opts)
        .expect("Should slice add_numbers");

    assert_eq!(result.target_symbol.name, "add_numbers");
    assert_eq!(result.target_symbol.kind, "function");
    assert_eq!(result.target_symbol.language, "rust");
    assert!(result
        .target_symbol
        .signature
        .contains("pub fn add_numbers(a: i64, b: i64) -> i64"));
    assert!(result.target_symbol.body.contains("a + b"));
    assert_eq!(
        result.target_symbol.doc_comment.as_deref(),
        Some("/// Pure mathematical addition of two numbers.")
    );
}

#[test]
fn test_slice_rust_function_with_lifetimes_and_options() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/simple_fn.rs");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "format_user_name", &opts)
        .expect("Should slice format_user_name");

    assert_eq!(result.target_symbol.name, "format_user_name");
    assert!(result.target_symbol.signature.contains("pub fn format_user_name<'a>(first: &'a str, last: &'a str, prefix: Option<&'a str>) -> String"));
    assert!(result.target_symbol.body.contains("match prefix"));
}

#[test]
fn test_slice_rust_result_and_enum_hoisting() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/simple_fn.rs");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "divide_safe", &opts)
        .expect("Should slice divide_safe");

    assert_eq!(result.target_symbol.name, "divide_safe");
    assert!(result.target_symbol.signature.contains(
        "pub fn divide_safe(numerator: f64, denominator: f64) -> Result<f64, MathError>"
    ));

    let hoisted_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted_names.contains(&"MathError"),
        "Must hoist MathError enum, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_slice_rust_generic_where_clause_and_trait_bounds() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/traits_generics_lifetimes.rs");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "transform", &opts)
        .expect("Should slice transform generic function");

    assert_eq!(result.target_symbol.name, "transform");
    assert!(result
        .target_symbol
        .signature
        .contains("pub fn transform<T, R>(input: T) -> R"));

    let hoisted_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted_names.contains(&"Transformable"),
        "Must hoist Transformable trait, found: {:?}",
        hoisted_names
    );
    // Generic parameters T, R must NOT be in hoisted types
    assert!(
        !hoisted_names.contains(&"T"),
        "Generic T must be filtered out"
    );
    assert!(
        !hoisted_names.contains(&"R"),
        "Generic R must be filtered out"
    );
}

#[test]
fn test_slice_rust_inherent_impl_method() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/traits_generics_lifetimes.rs");
    let opts = SliceOptions::default();

    // 1. Qualified query with ::
    let res1 = slicer
        .slice_symbol(file_path, "PipelineContainer::new", &opts)
        .expect("Should slice PipelineContainer::new");

    assert_eq!(res1.target_symbol.name, "new");
    assert_eq!(res1.target_symbol.kind, "method");
    assert!(res1
        .target_symbol
        .signature
        .contains("pub fn new(payload: T, trace_id: impl Into<String>) -> Self"));

    // 2. Method map_payload
    let res2 = slicer
        .slice_symbol(file_path, "PipelineContainer::map_payload", &opts)
        .expect("Should slice PipelineContainer::map_payload");

    assert_eq!(res2.target_symbol.name, "map_payload");
    assert!(res2
        .target_symbol
        .signature
        .contains("pub fn map_payload<F, U>(self, f: F) -> PipelineContainer<U, M>"));
}

#[test]
fn test_slice_rust_cross_module_resolution() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/realistic_inventory_service/inventory.rs");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "InventoryService::reserve_stock", &opts)
        .expect("Should slice InventoryService::reserve_stock");

    assert_eq!(result.target_symbol.name, "reserve_stock");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result
        .target_symbol
        .signature
        .contains("pub async fn reserve_stock"));

    // Cross-module models: ReservationRequest, StockReservation, InventoryError, Product
    let hoisted_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted_names.contains(&"ReservationRequest")
            || hoisted_names.contains(&"StockReservation")
            || hoisted_names.contains(&"InventoryError"),
        "Must hoist cross-module models from models.rs, found: {:?}",
        hoisted_names
    );

    // Call stubs
    assert!(
        !result.stripped_calls.is_empty(),
        "Must strip external calls"
    );
}

#[test]
fn test_slice_rust_call_stripping_semicolon_termination() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/realistic_inventory_service/inventory.rs");
    let opts = SliceOptions {
        depth: 1,
        include_types: false,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "InventoryService::reserve_stock", &opts)
        .expect("Should slice InventoryService::reserve_stock");

    for stub in &result.stripped_calls {
        assert!(
            stub.signature.trim().ends_with(';'),
            "Rust call signature stub must end with semicolon: {}",
            stub.signature
        );
    }
}

#[test]
fn test_slice_rust_circular_recursive_enum() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/circular_types.rs");
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "Expr::eval_constant", &opts)
        .expect("Should slice Expr::eval_constant without infinite recursion");

    assert_eq!(result.target_symbol.name, "eval_constant");
    let hoisted_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted_names.contains(&"Expr"),
        "Must hoist recursive Expr enum, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_slice_rust_struct_symbol_query() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/traits_generics_lifetimes.rs");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "PipelineContainer", &opts)
        .expect("Should slice PipelineContainer struct");

    assert_eq!(result.target_symbol.name, "PipelineContainer");
    assert_eq!(result.target_symbol.kind, "struct");
    assert!(result
        .target_symbol
        .body
        .contains("pub struct PipelineContainer<T, M>"));
    assert!(result.target_symbol.body.contains("pub payload: T"));
}

#[test]
fn test_slice_rust_symbol_not_found_returns_available() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/simple_fn.rs");
    let opts = SliceOptions::default();

    let err = slicer
        .slice_symbol(file_path, "nonexistent_fn", &opts)
        .expect_err("Should error on nonexistent symbol");

    match err {
        CoreError::SymbolNotFound {
            symbol,
            available_symbols,
            ..
        } => {
            assert_eq!(symbol, "nonexistent_fn");
            assert!(available_symbols.contains(&"add_numbers".to_string()));
            assert!(available_symbols.contains(&"divide_safe".to_string()));
        }
        _ => panic!("Expected SymbolNotFound error, got: {:?}", err),
    }
}

#[test]
fn test_slice_rust_disabled_options() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/traits_generics_lifetimes.rs");
    let opts = SliceOptions {
        depth: 1,
        include_types: false,
        include_calls: false,
    };

    let result = slicer
        .slice_symbol(file_path, "PipelineContainer::new", &opts)
        .expect("Should slice PipelineContainer::new");

    assert!(
        result.hoisted_types.is_empty(),
        "Hoisted types must be empty when disabled"
    );
    assert!(
        result.stripped_calls.is_empty(),
        "Stripped calls must be empty when disabled"
    );
}

#[test]
fn test_slice_rust_syntax_error_recovery() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/syntax_errors.rs");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "valid_header_function", &opts)
        .expect("Should slice valid_header_function despite adjacent syntax errors");

    assert_eq!(result.target_symbol.name, "valid_header_function");
    assert!(result.target_symbol.body.contains("x + y"));
}
