//! Comprehensive integration tests for ctxcut_core context slicing engine.

use std::path::PathBuf;
use ctxcut_core::{
    ContextSlicer, CoreError, SliceOptions, SliceResult, SupportedLanguage,
};

fn fixture_path(rel: &str) -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join(rel)
}

#[test]
fn test_language_detection() {
    assert_eq!(
        ContextSlicer::detect_language(&PathBuf::from("service.ts")).unwrap(),
        SupportedLanguage::TypeScript
    );
    assert_eq!(
        ContextSlicer::detect_language(&PathBuf::from("App.tsx")).unwrap(),
        SupportedLanguage::TypeScript
    );
    assert_eq!(
        ContextSlicer::detect_language(&PathBuf::from("index.js")).unwrap(),
        SupportedLanguage::JavaScript
    );
    assert_eq!(
        ContextSlicer::detect_language(&PathBuf::from("main.py")).unwrap(),
        SupportedLanguage::Python
    );
    assert_eq!(
        ContextSlicer::detect_language(&PathBuf::from("server.go")).unwrap(),
        SupportedLanguage::Go
    );
    assert_eq!(
        ContextSlicer::detect_language(&PathBuf::from("lib.rs")).unwrap(),
        SupportedLanguage::Rust
    );
}

#[test]
fn test_slice_typescript_auth_service() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/simple_service/authService.ts");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer.slice_symbol(&file, "registerUser", &opts).expect("Failed to slice registerUser");

    // 1. Target Symbol assertions
    assert_eq!(result.target_symbol.name, "registerUser");
    assert_eq!(result.target_symbol.kind, "function");
    assert!(result.target_symbol.doc_comment.is_some());
    assert!(result.target_symbol.doc_comment.as_ref().unwrap().contains("Registers a new user account"));
    assert!(result.target_symbol.signature.contains("function registerUser"));
    assert!(result.target_symbol.body.contains("return user;"));

    // 2. Hoisted Types assertions
    let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(type_names.contains(&"CreateUserDto"), "Expected CreateUserDto in hoisted types: {:?}", type_names);
    assert!(type_names.contains(&"User"), "Expected User in hoisted types: {:?}", type_names);
    assert!(type_names.contains(&"UserRole"), "Expected UserRole in hoisted types (transitive depth 2): {:?}", type_names);

    // 3. Stripped Calls assertions
    let call_names: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(call_names.contains(&"validateEmail"), "Expected validateEmail in stripped calls: {:?}", call_names);
    assert!(call_names.contains(&"hashPassword"), "Expected hashPassword in stripped calls: {:?}", call_names);

    for call in &result.stripped_calls {
        assert!(call.signature.trim().ends_with(';'), "Signature stub must end with semicolon: {}", call.signature);
        assert!(!call.signature.contains('{'), "Signature stub must not contain body: {}", call.signature);
    }

    // 4. Token metrics
    assert!(result.stats.raw_file_tokens > 0);
    assert!(result.stats.sliced_tokens > 0);
    assert!(result.stats.raw_lines > 0);
    assert!(result.stats.sliced_lines > 0);

    // 5. Formatter Markdown & JSON
    let md = result.to_markdown();
    assert!(md.contains("### Context Slice:"));
    assert!(md.contains("#### 1. Target Implementation (Full Body)"));
    assert!(md.contains("#### 2. Hoisted Types & Data Contracts"));
    assert!(md.contains("#### 3. External Dependencies & Signatures (Body Stripped)"));
    assert!(md.contains("```typescript"));

    let json = result.to_json();
    let deserialized: SliceResult = serde_json::from_str(&json).expect("JSON deserialization failed");
    assert_eq!(result, deserialized);
}

#[test]
fn test_slice_typescript_class_methods() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/classes_and_arrow/payment.ts");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&file, "PaymentProcessor.processCharge", &opts)
        .expect("Failed to slice PaymentProcessor.processCharge");

    assert_eq!(result.target_symbol.name, "PaymentProcessor.processCharge");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.doc_comment.as_ref().unwrap().contains("Executes charge against payment gateway"));

    let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(type_names.contains(&"PaymentRequest"), "Expected PaymentRequest in hoisted types: {:?}", type_names);
    assert!(type_names.contains(&"PaymentReceipt"), "Expected PaymentReceipt in hoisted types: {:?}", type_names);
    assert!(type_names.contains(&"PaymentStatus"), "Expected PaymentStatus in hoisted types: {:?}", type_names);

    let call_names: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(call_names.contains(&"getAuthHeader"), "Expected getAuthHeader in stripped calls: {:?}", call_names);
    assert!(call_names.contains(&"sendToGateway"), "Expected sendToGateway in stripped calls: {:?}", call_names);
}

#[test]
fn test_slice_typescript_arrow_function() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/classes_and_arrow/payment.ts");
    let opts = SliceOptions::default();

    let result = slicer.slice_symbol(&file, "calculateTax", &opts).expect("Failed to slice calculateTax");
    assert_eq!(result.target_symbol.name, "calculateTax");
    assert_eq!(result.target_symbol.kind, "function");
    assert!(result.target_symbol.body.contains("amount * rate"));
}

#[test]
fn test_slice_tsx_component() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/tsx_components/UserProfile.tsx");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer.slice_symbol(&file, "UserProfile", &opts).expect("Failed to slice UserProfile TSX");
    assert_eq!(result.target_symbol.name, "UserProfile");
    assert_eq!(result.target_symbol.kind, "function");

    let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(type_names.contains(&"UserProfileProps"), "Expected UserProfileProps: {:?}", type_names);
    assert!(type_names.contains(&"User"), "Expected User: {:?}", type_names);

    let md = result.to_markdown();
    assert!(md.contains("```tsx"));
}

#[test]
fn test_barrel_reexports_traversal() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/barrel_imports/service.ts");
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
    };

    let result = slicer.slice_symbol(&file, "evaluateModel", &opts).expect("Failed to slice evaluateModel");
    assert_eq!(result.target_symbol.name, "evaluateModel");

    let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(type_names.contains(&"DeepModel"), "Expected DeepModel hoisted via barrel re-export: {:?}", type_names);

    let call_names: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(call_names.contains(&"computeScore"), "Expected computeScore stripped via barrel re-export: {:?}", call_names);
}

#[test]
fn test_circular_types_cycle_protection() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/edge_cases/circular.ts");
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let result = slicer.slice_symbol(&file, "findRoot", &opts).expect("Failed to slice findRoot with circular types");
    assert_eq!(result.target_symbol.name, "findRoot");

    let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(type_names.contains(&"TreeNode"));
    assert!(type_names.contains(&"TreeChild"));
}

#[test]
fn test_symbol_not_found_returns_available_symbols() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/simple_service/authService.ts");
    let opts = SliceOptions::default();

    let err = slicer.slice_symbol(&file, "nonExistentFunction", &opts).unwrap_err();
    match err {
        CoreError::SymbolNotFound { symbol, available_symbols, .. } => {
            assert_eq!(symbol, "nonExistentFunction");
            assert!(available_symbols.contains(&"registerUser".to_string()));
            assert!(available_symbols.contains(&"helperInternal".to_string()));
        }
        other => panic!("Expected SymbolNotFound, got {:?}", other),
    }
}

#[test]
fn test_javascript_commonjs_slice() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("javascript/commonjs_es6.js");
    let opts = SliceOptions::default();

    let result = slicer.slice_symbol(&file, "processCheckout", &opts).expect("Failed to slice processCheckout in JS");
    assert_eq!(result.target_symbol.name, "processCheckout");
    assert_eq!(result.target_symbol.kind, "function");

    let call_names: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(call_names.contains(&"calculateDiscount"), "Expected calculateDiscount in stripped calls: {:?}", call_names);
    assert!(call_names.contains(&"formatCurrency"), "Expected formatCurrency in stripped calls: {:?}", call_names);
}

#[test]
fn test_slice_symbols_batch() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/simple_service/authService.ts");
    let opts = SliceOptions::default();

    let results = slicer.slice_symbols(&file, &["registerUser", "helperInternal"], &opts).expect("Batch slice failed");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].target_symbol.name, "registerUser");
    assert_eq!(results[1].target_symbol.name, "helperInternal");
}

#[test]
fn test_slice_options_disabled_flags() {
    let slicer = ContextSlicer::new();
    let file = fixture_path("typescript/simple_service/authService.ts");
    let opts = SliceOptions {
        depth: 1,
        include_types: false,
        include_calls: false,
    };

    let result = slicer.slice_symbol(&file, "registerUser", &opts).expect("Failed to slice registerUser");
    assert!(result.hoisted_types.is_empty());
    assert!(result.stripped_calls.is_empty());
}
