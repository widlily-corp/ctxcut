//! Adversarial and stress tests across Python, Go, and Rust AST adapters.

use ctxcut_core::error::CoreError;
use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_adversarial_python_async_decorators_and_pydantic() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/python/fastapi_routes.py");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = slicer
        .slice_symbol(file_path, "get_user_profile", &opts)
        .expect("Should slice get_user_profile with Query parameters and Depends");

    assert_eq!(result.target_symbol.name, "get_user_profile");
    assert!(result.target_symbol.body.contains("Query(default=False"));
    assert!(result
        .target_symbol
        .body
        .contains("Annotated[DatabaseSession, Depends(get_db)]"));

    let hoisted_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted_names.contains(&"UserProfile"),
        "Must hoist UserProfile, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_adversarial_python_pep695_and_type_aliases() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("pep695_sample.py");
    let code = r#"
type Coordinate[T] = tuple[T, T]
type UserMapping = dict[str, int]

def calculate_distance[T: float](p1: Coordinate[T], p2: Coordinate[T]) -> float:
    """Compute euclidean distance between two generic coordinates."""
    dx = p1[0] - p2[0]
    dy = p1[1] - p2[1]
    return (dx ** 2 + dy ** 2) ** 0.5
"#;
    fs::write(&file_path, code).expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = slicer
        .slice_symbol(&file_path, "calculate_distance", &opts)
        .expect("Should slice PEP 695 function");

    assert_eq!(result.target_symbol.name, "calculate_distance");
    assert!(result
        .target_symbol
        .signature
        .contains("def calculate_distance[T: float]"));
    let hoisted_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted_names.contains(&"Coordinate"),
        "Must hoist PEP 695 type alias Coordinate, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_adversarial_go_pointer_and_value_receivers() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/structs_interfaces.go");
    let opts = SliceOptions::default();

    // Slicing with pointer syntax in query `*Service.Execute`
    let res = slicer
        .slice_symbol(file_path, "*Service.Execute", &opts)
        .expect("Should slice *Service.Execute with leading asterisk");

    assert_eq!(res.target_symbol.name, "Execute");
    assert_eq!(res.target_symbol.kind, "method");
}

#[test]
fn test_adversarial_go_sibling_multi_file_package() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/go/realistic_auth_service/service.go");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = slicer
        .slice_symbol(file_path, "AuthService.RefreshToken", &opts)
        .expect("Should slice AuthService.RefreshToken");

    assert_eq!(result.target_symbol.name, "RefreshToken");
    let hoisted_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted_names.contains(&"AuthResult")
            && (hoisted_names.contains(&"User") || hoisted_names.contains(&"Session")),
        "Must resolve types from models.go, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_adversarial_rust_generics_lifetimes_where_clauses() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/traits_generics_lifetimes.rs");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = slicer
        .slice_symbol(file_path, "process_batch", &opts)
        .expect("Should slice complex lifetime and generic function process_batch");

    assert_eq!(result.target_symbol.name, "process_batch");
    assert!(result
        .target_symbol
        .signature
        .contains("pub fn process_batch<'a, 'b, T, K, V>"));
    assert!(result.target_symbol.signature.contains("where"));
}

#[test]
fn test_adversarial_rust_impl_generic_unwrapping() {
    let slicer = ContextSlicer::new();
    let file_path = Path::new("../../tests/fixtures/rust/traits_generics_lifetimes.rs");
    let opts = SliceOptions::default();

    // Query with generic struct name matching impl<T, M> PipelineContainer<T, M>
    let result = slicer
        .slice_symbol(file_path, "PipelineContainer::new", &opts)
        .expect("Should slice PipelineContainer::new by unwrapping generic_type");

    assert_eq!(result.target_symbol.name, "new");
    assert!(result.target_symbol.body.contains("pub fn new"));
}

#[test]
fn test_adversarial_cross_language_circular_references() {
    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // 1. Python circular
    let py_path = Path::new("../../tests/fixtures/python/circular_models.py");
    let py_res = slicer.slice_symbol(py_path, "build_taxonomy_tree", &opts);
    assert!(py_res.is_ok(), "Python circular slicing must not loop");

    // 2. Go circular
    let go_path = Path::new("../../tests/fixtures/go/circular_types.go");
    let go_res = slicer.slice_symbol(go_path, "BuildSampleDoublyLinkedList", &opts);
    assert!(go_res.is_ok(), "Go circular slicing must not loop");

    // 3. Rust circular
    let rs_path = Path::new("../../tests/fixtures/rust/circular_types.rs");
    let rs_res = slicer.slice_symbol(rs_path, "Expr::eval_constant", &opts);
    assert!(rs_res.is_ok(), "Rust circular slicing must not loop");
}

#[test]
fn test_adversarial_syntax_corruption_resilience_all_languages() {
    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    // 1. Python syntax errors
    let py_path = Path::new("../../tests/fixtures/python/syntax_errors.py");
    let py_res = slicer.slice_symbol(py_path, "valid_header_function", &opts);
    assert!(
        py_res.is_ok(),
        "Python error recovery should locate valid function"
    );

    // 2. Go syntax errors
    let go_path = Path::new("../../tests/fixtures/go/syntax_errors.go");
    let go_res = slicer.slice_symbol(go_path, "ValidHeaderFunc", &opts);
    assert!(
        go_res.is_ok(),
        "Go error recovery should locate valid function"
    );

    // 3. Rust syntax errors
    let rs_path = Path::new("../../tests/fixtures/rust/syntax_errors.rs");
    let rs_res = slicer.slice_symbol(rs_path, "valid_header_function", &opts);
    assert!(
        rs_res.is_ok(),
        "Rust error recovery should locate valid function"
    );
}

#[test]
fn test_adversarial_unicode_and_special_identifiers() {
    let dir = tempdir().expect("tempdir");

    // Python Unicode
    let py_file = dir.path().join("unicode_test.py");
    fs::write(
        &py_file,
        "def calculate_π_рассчитать(число: float) -> float:\n    return число * 3.14159\n",
    )
    .expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();
    let py_res = slicer.slice_symbol(&py_file, "calculate_π_рассчитать", &opts);
    assert!(
        py_res.is_ok(),
        "Python unicode identifier should slice cleanly"
    );
    assert_eq!(py_res.unwrap().target_symbol.name, "calculate_π_рассчитать");

    // Rust Unicode
    let rs_file = dir.path().join("unicode_test.rs");
    fs::write(
        &rs_file,
        "pub fn calculate_π(r: f64) -> f64 {\n    r * 3.14159\n}\n",
    )
    .expect("write");
    let rs_res = slicer.slice_symbol(&rs_file, "calculate_π", &opts);
    assert!(
        rs_res.is_ok(),
        "Rust unicode identifier should slice cleanly"
    );
    assert_eq!(rs_res.unwrap().target_symbol.name, "calculate_π");
}

#[test]
fn test_adversarial_empty_and_whitespace_files() {
    let dir = tempdir().expect("tempdir");
    let opts = SliceOptions::default();
    let slicer = ContextSlicer::new();

    for (ext, name) in [("py", "empty.py"), ("go", "empty.go"), ("rs", "empty.rs")] {
        let file_path = dir.path().join(name);
        fs::write(&file_path, "   \n\n\t  \n").expect("write");

        let res = slicer.slice_symbol(&file_path, "target_func", &opts);
        assert!(res.is_err(), "Empty file must return error: {ext}");
        match res.unwrap_err() {
            CoreError::SymbolNotFound {
                available_symbols, ..
            } => {
                assert!(available_symbols.is_empty());
            }
            err => panic!("Expected SymbolNotFound, got: {:?}", err),
        }
    }
}
