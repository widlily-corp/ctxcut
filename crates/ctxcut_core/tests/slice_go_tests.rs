//! Integration tests for Go AST parsing, symbol location, type hoisting, and slicing.

use std::path::Path;
use ctxcut_core::error::CoreError;
use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;

#[test]
fn test_slice_go_standalone_function() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/simple_func.go");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "AddNumbers", &opts)
        .expect("Should slice AddNumbers");

    assert_eq!(result.target_symbol.name, "AddNumbers");
    assert_eq!(result.target_symbol.kind, "function");
    assert_eq!(result.target_symbol.language, "go");
    assert!(result.target_symbol.signature.contains("func AddNumbers(a int, b int) int"));
    assert!(result.target_symbol.body.contains("return a + b"));
    assert_eq!(
        result.target_symbol.doc_comment.as_deref(),
        Some("// AddNumbers returns the sum of two integers.")
    );
}

#[test]
fn test_slice_go_multiple_returns_and_named_parameters() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/simple_func.go");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "DivideWithRemainder", &opts)
        .expect("Should slice DivideWithRemainder");

    assert_eq!(result.target_symbol.name, "DivideWithRemainder");
    assert!(result.target_symbol.signature.contains("func DivideWithRemainder(numerator, denominator int) (quotient int, remainder int, err error)"));
    assert!(result.target_symbol.body.contains("return quotient, remainder, nil"));
}

#[test]
fn test_slice_go_struct_receivers_and_pointers() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/structs_interfaces.go");
    let opts = SliceOptions::default();

    // 1. Qualified query with pointer receiver
    let res1 = slicer
        .slice_symbol(file_path, "Service.Execute", &opts)
        .expect("Should slice Service.Execute");

    assert_eq!(res1.target_symbol.name, "Execute");
    assert_eq!(res1.target_symbol.kind, "method");
    assert!(res1.target_symbol.signature.contains("func (s *Service) Execute(ctx context.Context, req ExecutionRequest) (*ExecutionResponse, error)"));

    // 2. Value receiver method
    let res2 = slicer
        .slice_symbol(file_path, "Service.Status", &opts)
        .expect("Should slice Service.Status");

    assert_eq!(res2.target_symbol.name, "Status");
    assert!(res2.target_symbol.signature.contains("func (s *Service) Status() string"));
}

#[test]
fn test_slice_go_struct_and_interface_hoisting() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/structs_interfaces.go");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "Service.Execute", &opts)
        .expect("Should slice Service.Execute");

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"ExecutionRequest"),
        "Must hoist ExecutionRequest, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"ExecutionResponse"),
        "Must hoist ExecutionResponse, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_slice_go_sibling_package_resolution() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/realistic_auth_service/service.go");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "AuthService.AuthenticateUser", &opts)
        .expect("Should slice AuthService.AuthenticateUser");

    assert_eq!(result.target_symbol.name, "AuthenticateUser");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.signature.contains("func (s *AuthService) AuthenticateUser(ctx context.Context, creds LoginCredentials) (*AuthResult, error)"));

    // Sibling models.go hoisted types: LoginCredentials, AuthResult, User, Session, Role
    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"LoginCredentials"),
        "Must hoist LoginCredentials from sibling models.go, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"AuthResult"),
        "Must hoist AuthResult from sibling models.go, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"User") || hoisted_names.contains(&"Session") || hoisted_names.contains(&"Role"),
        "Must hoist transitive User/Session/Role types, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_slice_go_call_stripping() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/realistic_auth_service/service.go");
    let opts = SliceOptions {
        depth: 1,
        include_types: false,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "AuthService.AuthenticateUser", &opts)
        .expect("Should slice AuthService.AuthenticateUser");

    assert!(!result.stripped_calls.is_empty(), "Must strip external calls");
    let stub_names: Vec<&str> = result.stripped_calls.iter().map(|s| s.name.as_str()).collect();
    assert!(
        stub_names.contains(&"hashPassword") || stub_names.contains(&"generateRandomToken") || stub_names.contains(&"GenerateAccessToken"),
        "Must strip internal and external call stubs, found: {:?}",
        stub_names
    );
}

#[test]
fn test_slice_go_circular_types_cycle_protection() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/circular_types.go");
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(file_path, "BuildSampleDoublyLinkedList", &opts)
        .expect("Should slice BuildSampleDoublyLinkedList without infinite recursion");

    assert_eq!(result.target_symbol.name, "BuildSampleDoublyLinkedList");
    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"Node"),
        "Must hoist recursive Node struct, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_slice_go_type_declaration_symbol() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/realistic_auth_service/models.go");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "User", &opts)
        .expect("Should slice User struct type declaration");

    assert_eq!(result.target_symbol.name, "User");
    assert_eq!(result.target_symbol.kind, "type");
    assert!(result.target_symbol.body.contains("type User struct"));
    assert!(result.target_symbol.body.contains("PasswordHash"));
}

#[test]
fn test_slice_go_constructor_function() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/structs_interfaces.go");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "NewService", &opts)
        .expect("Should slice NewService constructor");

    assert_eq!(result.target_symbol.name, "NewService");
    assert!(result.target_symbol.signature.contains("func NewService(id, name, version string) *Service"));
}

#[test]
fn test_slice_go_symbol_not_found_returns_available() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/simple_func.go");
    let opts = SliceOptions::default();

    let err = slicer
        .slice_symbol(file_path, "NonExistentFunc", &opts)
        .expect_err("Should error on non-existent Go symbol");

    match err {
        CoreError::SymbolNotFound { symbol, available_symbols, .. } => {
            assert_eq!(symbol, "NonExistentFunc");
            assert!(available_symbols.contains(&"AddNumbers".to_string()));
            assert!(available_symbols.contains(&"FormatUserName".to_string()));
        }
        _ => panic!("Expected SymbolNotFound error, got: {:?}", err),
    }
}

#[test]
fn test_slice_go_disabled_options() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/structs_interfaces.go");
    let opts = SliceOptions {
        depth: 1,
        include_types: false,
        include_calls: false,
    };

    let result = slicer
        .slice_symbol(file_path, "Service.Execute", &opts)
        .expect("Should slice Service.Execute");

    assert!(result.hoisted_types.is_empty(), "Hoisted types must be empty when disabled");
    assert!(result.stripped_calls.is_empty(), "Stripped calls must be empty when disabled");
}

#[test]
fn test_slice_go_syntax_error_recovery() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/syntax_errors.go");
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(file_path, "ValidHeaderFunc", &opts)
        .expect("Should slice ValidHeaderFunc despite adjacent syntax errors");

    assert_eq!(result.target_symbol.name, "ValidHeaderFunc");
    assert!(result.target_symbol.body.contains("return a + b"));
}
