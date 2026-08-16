//! Integration tests for Python AST parsing, symbol location, type hoisting, and slicing.

use std::path::Path;
use ctxcut_core::error::CoreError;
use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;

#[test]
fn test_slice_python_standalone_function() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/simple_function.py");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "add_numbers", &opts)
        .expect("Should slice add_numbers");

    assert_eq!(result.target_symbol.name, "add_numbers");
    assert_eq!(result.target_symbol.kind, "function");
    assert_eq!(result.target_symbol.language, "python");
    assert!(result.target_symbol.signature.contains("def add_numbers(a: int | float, b: int | float) -> int | float"));
    assert!(result.target_symbol.body.contains("return a + b"));
    assert_eq!(
        result.target_symbol.doc_comment.as_deref(),
        Some("Add two numbers supporting ints and floats.")
    );
}

#[test]
fn test_slice_python_format_user_name_docstring() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/simple_function.py");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "format_user_name", &opts)
        .expect("Should slice format_user_name");

    assert_eq!(result.target_symbol.name, "format_user_name");
    assert!(result.target_symbol.signature.contains("def format_user_name"));
    assert_eq!(
        result.target_symbol.doc_comment.as_deref(),
        Some("Format a user's full name with an optional honorific prefix.")
    );
}

#[test]
fn test_slice_python_pydantic_local_hoisting() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/type_hints_pydantic.py");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "register_user", &opts)
        .expect("Should slice register_user");

    assert_eq!(result.target_symbol.name, "register_user");
    assert!(result.target_symbol.body.contains("def register_user"));

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"UserCreate"),
        "Must hoist UserCreate, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"APIEnvelope"),
        "Must hoist APIEnvelope, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"UserResponse"),
        "Must hoist UserResponse, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_slice_python_fastapi_route_with_decorators() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/fastapi_routes.py");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "create_item", &opts)
        .expect("Should slice create_item");

    assert_eq!(result.target_symbol.name, "create_item");
    assert!(result.target_symbol.signature.contains("async def create_item"));
    assert!(result.target_symbol.body.contains("@router.post"));
    assert!(result.target_symbol.body.contains("response_model=ItemResponse"));

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"ItemCreate") || hoisted_names.contains(&"ItemResponse"),
        "Must hoist ItemCreate/ItemResponse, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_slice_python_cross_file_payment_service() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/realistic_payment_service/payment_service.py");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "PaymentProcessor.execute_charge", &opts)
        .expect("Should slice PaymentProcessor.execute_charge");

    assert_eq!(result.target_symbol.name, "execute_charge");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.signature.contains("async def execute_charge"));
    assert!(result.target_symbol.body.contains("self.gateway.authorize_charge"));

    // Verify cross-file type hoisting from .schemas
    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"ChargeRequest") || hoisted_names.contains(&"ChargeResult"),
        "Must hoist ChargeRequest and ChargeResult across files, found: {:?}",
        hoisted_names
    );

    // Verify call stubs
    assert!(!result.stripped_calls.is_empty(), "Must strip external calls");
}

#[test]
fn test_slice_python_class_method_qualified_and_unqualified() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/circular_models.py");
    let opts = SliceOptions::default();

    // 1. Qualified query
    let res1 = slicer
        .slice_symbol(file_path, "OrganizationUnit.total_subordinate_count", &opts)
        .expect("Should slice OrganizationUnit.total_subordinate_count");
    assert_eq!(res1.target_symbol.name, "total_subordinate_count");
    assert_eq!(res1.target_symbol.kind, "method");

    // 2. Unqualified query fallback
    let res2 = slicer
        .slice_symbol(file_path, "total_subordinate_count", &opts)
        .expect("Should slice total_subordinate_count via fallback");
    assert_eq!(res2.target_symbol.name, "total_subordinate_count");
}

#[test]
fn test_slice_python_circular_models_cycle_protection() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/circular_models.py");
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "build_taxonomy_tree", &opts)
        .expect("Should slice build_taxonomy_tree without infinite loop");

    assert_eq!(result.target_symbol.name, "build_taxonomy_tree");
    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"CategoryNode"),
        "Must hoist CategoryNode, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_slice_python_class_symbol_query() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/realistic_payment_service/payment_service.py");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "PaymentRepository", &opts)
        .expect("Should slice PaymentRepository class");

    assert_eq!(result.target_symbol.name, "PaymentRepository");
    assert_eq!(result.target_symbol.kind, "class");
    assert!(result.target_symbol.body.contains("class PaymentRepository"));
}

#[test]
fn test_slice_python_symbol_not_found_returns_available() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/simple_function.py");
    let opts = SliceOptions::default();

    let err = slicer
        .slice_symbol(file_path, "nonexistent_function", &opts)
        .expect_err("Should error on nonexistent symbol");

    match err {
        CoreError::SymbolNotFound { symbol, available_symbols, .. } => {
            assert_eq!(symbol, "nonexistent_function");
            assert!(available_symbols.contains(&"add_numbers".to_string()));
            assert!(available_symbols.contains(&"format_user_name".to_string()));
        }
        _ => panic!("Expected SymbolNotFound error, got: {:?}", err),
    }
}

#[test]
fn test_slice_python_disabled_options() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/type_hints_pydantic.py");
    let opts = SliceOptions {
        depth: 1,
        include_types: false,
        include_calls: false,
    };

    let result = slicer
        .slice_symbol(file_path, "register_user", &opts)
        .expect("Should slice register_user");

    assert!(result.hoisted_types.is_empty(), "Hoisted types must be empty when disabled");
    assert!(result.stripped_calls.is_empty(), "Stripped calls must be empty when disabled");
}

#[test]
fn test_slice_python_batch_symbols() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/simple_function.py");
    let opts = SliceOptions::default();

    let results = slicer
        .slice_symbols(file_path, &["add_numbers", "calculate_discount"], &opts)
        .expect("Batch slice should succeed");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].target_symbol.name, "add_numbers");
    assert_eq!(results[1].target_symbol.name, "calculate_discount");
}

#[test]
fn test_slice_python_syntax_error_recovery() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/syntax_errors.py");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "valid_header_function", &opts)
        .expect("Should slice valid_header_function despite adjacent syntax errors");

    assert_eq!(result.target_symbol.name, "valid_header_function");
    assert!(result.target_symbol.body.contains("return x + y"));
}
