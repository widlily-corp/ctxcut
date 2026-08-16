//! Comprehensive adversarial stress suite for AST parsing and resolver engine.
//! Validates resilience under complex generics, deep classes, mutual recursion,
//! multi-hop barrel chains, CommonJS/ES6 mix, missing symbols, and TSX generic components.

use std::path::PathBuf;
use std::time::Instant;
use ctxcut_core::{ContextSlicer, CoreError, SliceOptions};

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

fn warmup_engine() -> ContextSlicer {
    let slicer = ContextSlicer::new();
    // Warm up one-time BPE singleton initialization
    let _ = ctxcut_core::tokenizer::count_tokens("const warmup = 42;");
    slicer
}

#[test]
fn test_adversarial_complex_generics() {
    let slicer = warmup_engine();
    let file = fixture_path("adversarial/complex_generics.ts");
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let start = Instant::now();
    let result = slicer
        .slice_symbol(&file, "AdvancedRepository.findAndUnwrap", &opts)
        .expect("Failed to slice AdvancedRepository.findAndUnwrap");
    let elapsed = start.elapsed();

    println!("Complex generics slice elapsed: {:?}", elapsed);
    assert!(elapsed.as_millis() < 50, "Slice should complete in < 50ms, took {:?}", elapsed);

    // Verify target symbol
    assert_eq!(result.target_symbol.name, "AdvancedRepository.findAndUnwrap");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.signature.contains("findAndUnwrap"));
    assert!(result.target_symbol.body.contains("return this.config.defaultValue;"));

    // Verify hoisted types
    let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    println!("Hoisted types in complex generics: {:?}", type_names);

    // Scoped generics MUST NOT be hoisted as unknown types
    assert!(!type_names.contains(&"TEntity"), "Scoped generic TEntity must not be hoisted");
    assert!(!type_names.contains(&"TId"), "Scoped generic TId must not be hoisted");
    assert!(!type_names.contains(&"TConfig"), "Scoped generic TConfig must not be hoisted");
    assert!(!type_names.contains(&"U"), "Scoped generic U must not be hoisted");

    // Real type references MUST be hoisted
    assert!(type_names.contains(&"ConditionalUnwrap"), "Expected ConditionalUnwrap in hoisted types");

    // Also slice whole class
    let class_result = slicer
        .slice_symbol(&file, "AdvancedRepository", &opts)
        .expect("Failed to slice AdvancedRepository class");
    assert_eq!(class_result.target_symbol.name, "AdvancedRepository");
    assert_eq!(class_result.target_symbol.kind, "class");
    let class_types: Vec<&str> = class_result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(class_types.contains(&"Entity"), "Expected Entity hoisted from class definition: {:?}", class_types);
    assert!(class_types.contains(&"RepoConfig"), "Expected RepoConfig hoisted from class definition: {:?}", class_types);
    assert!(class_types.contains(&"DomainMeta"), "Expected DomainMeta hoisted via transitive depth 2: {:?}", class_types);
}

#[test]
fn test_adversarial_deep_classes_and_members() {
    let slicer = warmup_engine();
    let file = fixture_path("adversarial/deep_classes.ts");
    let opts = SliceOptions::default();

    // 1. Static method
    let static_res = slicer
        .slice_symbol(&file, "EngineController.createDefault", &opts)
        .expect("Failed to slice static method");
    assert_eq!(static_res.target_symbol.name, "EngineController.createDefault");
    assert_eq!(static_res.target_symbol.kind, "method");
    assert!(static_res.target_symbol.signature.contains("createDefault"));
    assert!(static_res.target_symbol.body.contains("return new EngineController(5000);"));

    // 2. Getter method
    let getter_res = slicer
        .slice_symbol(&file, "EngineController.isThrottled", &opts)
        .expect("Failed to slice getter");
    assert_eq!(getter_res.target_symbol.name, "EngineController.isThrottled");
    assert_eq!(getter_res.target_symbol.kind, "method");
    assert!(getter_res.target_symbol.body.contains("return this._isThrottled;"));

    // 3. Async generator method
    let gen_res = slicer
        .slice_symbol(&file, "EngineController.streamMetrics", &opts)
        .expect("Failed to slice async generator method");
    assert_eq!(gen_res.target_symbol.name, "EngineController.streamMetrics");
    assert_eq!(gen_res.target_symbol.kind, "method");
    assert!(gen_res.target_symbol.signature.contains("streamMetrics"));
    assert!(gen_res.target_symbol.body.contains("yield {"));
    let gen_types: Vec<&str> = gen_res.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(gen_types.contains(&"SystemMetrics"), "Expected SystemMetrics hoisted: {:?}", gen_types);

    // 4. Bare member query (fallback search across classes)
    let bare_res = slicer
        .slice_symbol(&file, "executeCommand", &opts)
        .expect("Failed to slice bare member name executeCommand");
    assert_eq!(bare_res.target_symbol.name, "EngineController.executeCommand");
    assert_eq!(bare_res.target_symbol.kind, "method");
}

#[test]
fn test_adversarial_mutual_recursion_and_circular_types() {
    let slicer = warmup_engine();
    let file = fixture_path("adversarial/mutual_recursion.ts");

    for depth in [1, 2, 3, 5, 10] {
        let opts = SliceOptions {
            depth,
            include_types: true,
            include_calls: true,
        };

        let start = Instant::now();
        let result = slicer
            .slice_symbol(&file, "processRecursiveGraph", &opts)
            .expect("Failed to slice processRecursiveGraph with recursion");
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() < 50, "Recursion resolution took too long at depth {}: {:?}", depth, elapsed);

        let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
        // Check that NodeA, NodeB, NodeC are all collected without duplicate entries
        assert!(type_names.contains(&"NodeA"));
        assert!(type_names.contains(&"NodeC"));
        if depth >= 2 {
            assert!(type_names.contains(&"NodeB"));
        }

        // Verify uniqueness
        let mut unique_check = std::collections::HashSet::new();
        for name in &type_names {
            assert!(unique_check.insert(*name), "Duplicate type found in hoisted types: {}", name);
        }
    }
}

#[test]
fn test_adversarial_multi_hop_barrel_reexports() {
    let slicer = warmup_engine();
    let file = fixture_path("adversarial/barrel_hops/consumer.ts");
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let start = Instant::now();
    let result = slicer
        .slice_symbol(&file, "runMultiHopAction", &opts)
        .expect("Failed to slice across 4-hop barrel re-export chain");
    let elapsed = start.elapsed();

    assert!(elapsed.as_millis() < 50, "4-hop traversal took too long: {:?}", elapsed);

    // 1. Verify target
    assert_eq!(result.target_symbol.name, "runMultiHopAction");

    // 2. Verify hoisted type resolved across 4 hops (consumer -> hop3 -> hop2/index -> hop1 -> leaf)
    let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        type_names.contains(&"LeafPayload"),
        "LeafPayload must be hoisted across 4 hops: {:?}",
        type_names
    );

    // 3. Verify stripped call signature resolved across 4 hops
    let call_names: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(
        call_names.contains(&"executeLeafAction"),
        "executeLeafAction must be stripped across 4 hops: {:?}",
        call_names
    );

    for call in &result.stripped_calls {
        assert!(call.signature.trim().ends_with(';'));
        assert!(!call.signature.contains('{'));
    }
}

#[test]
fn test_adversarial_javascript_commonjs_and_es6() {
    let slicer = warmup_engine();
    let file = fixture_path("adversarial/cjs_mixed.js");
    let opts = SliceOptions::default();

    // 1. Slice processOrder
    let result = slicer
        .slice_symbol(&file, "processOrder", &opts)
        .expect("Failed to slice processOrder in CommonJS file");

    assert_eq!(result.target_symbol.name, "processOrder");
    assert_eq!(result.target_symbol.kind, "function");
    assert!(result.target_symbol.body.contains("calculateDiscount(price, discountRate)"));

    let call_names: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(call_names.contains(&"calculateDiscount"));
    assert!(call_names.contains(&"formatCurrency"));

    // 2. Slice formatCurrency
    let fmt_res = slicer
        .slice_symbol(&file, "formatCurrency", &opts)
        .expect("Failed to slice formatCurrency in CommonJS file");
    assert_eq!(fmt_res.target_symbol.name, "formatCurrency");
    assert!(fmt_res.target_symbol.body.contains("'$' + val.toFixed(2)"));
}

#[test]
fn test_adversarial_missing_symbols_and_invalid_queries() {
    let slicer = warmup_engine();
    let valid_file = fixture_path("adversarial/deep_classes.ts");
    let empty_file = fixture_path("typescript/edge_cases/empty.ts");
    let comments_file = fixture_path("typescript/edge_cases/comments_only.ts");
    let malformed_file = fixture_path("typescript/malformed_syntax.ts");
    let opts = SliceOptions::default();

    // 1. Non-existent symbol in valid file -> returns SymbolNotFound with available symbols
    let err = slicer.slice_symbol(&valid_file, "nonExistentMember", &opts).unwrap_err();
    match err {
        CoreError::SymbolNotFound { symbol, available_symbols, .. } => {
            assert_eq!(symbol, "nonExistentMember");
            assert!(available_symbols.contains(&"EngineController".to_string()));
            assert!(available_symbols.contains(&"EngineController.createDefault".to_string()));
            assert!(available_symbols.contains(&"EngineController.streamMetrics".to_string()));
        }
        other => panic!("Expected SymbolNotFound, got {:?}", other),
    }

    // 2. Missing container member query
    let err_container = slicer.slice_symbol(&valid_file, "EngineController.ghostMethod", &opts).unwrap_err();
    assert!(matches!(err_container, CoreError::SymbolNotFound { .. }));

    // 3. Edge case query strings: empty, whitespace, delimiters
    for invalid_query in ["", "   ", "::", ".", "...", "A.B.C", "Invalid:::Method"] {
        let res = slicer.slice_symbol(&valid_file, invalid_query, &opts);
        assert!(res.is_err(), "Query '{}' should fail gracefully", invalid_query);
        assert!(matches!(res.unwrap_err(), CoreError::SymbolNotFound { .. }));
    }

    // 4. Empty file
    let empty_err = slicer.slice_symbol(&empty_file, "anySymbol", &opts).unwrap_err();
    match empty_err {
        CoreError::SymbolNotFound { available_symbols, .. } => {
            assert!(available_symbols.is_empty());
        }
        other => panic!("Expected SymbolNotFound on empty file, got {:?}", other),
    }

    // 5. Comments-only file
    let comments_err = slicer.slice_symbol(&comments_file, "anySymbol", &opts).unwrap_err();
    match comments_err {
        CoreError::SymbolNotFound { available_symbols, .. } => {
            assert!(available_symbols.is_empty());
        }
        other => panic!("Expected SymbolNotFound on comments file, got {:?}", other),
    }

    // 6. Tree-sitter error recovery on syntactically malformed file
    let valid_header_res = slicer.slice_symbol(&malformed_file, "ValidHeaderInterface", &opts);
    assert!(
        valid_header_res.is_ok(),
        "Tree-sitter must extract ValidHeaderInterface from partially malformed file: {:?}",
        valid_header_res.err()
    );
    let sym = valid_header_res.unwrap().target_symbol;
    assert_eq!(sym.name, "ValidHeaderInterface");
    assert_eq!(sym.kind, "interface");

    // Slicing unparseable symbol returns graceful SymbolNotFound without panic
    let broken_res = slicer.slice_symbol(&malformed_file, "brokenFunctionOne", &opts);
    assert!(broken_res.is_err());
    assert!(matches!(broken_res.unwrap_err(), CoreError::SymbolNotFound { .. }));
}

#[test]
fn test_adversarial_tsx_components_and_generic_arrows() {
    let slicer = warmup_engine();
    let file = fixture_path("adversarial/GenericComponent.tsx");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let start = Instant::now();
    let result = slicer
        .slice_symbol(&file, "GenericTable", &opts)
        .expect("Failed to slice GenericTable TSX component");
    let elapsed = start.elapsed();

    assert!(elapsed.as_millis() < 50, "TSX slicing took too long: {:?}", elapsed);

    // 1. Verify target
    assert_eq!(result.target_symbol.name, "GenericTable");
    assert_eq!(result.target_symbol.kind, "function");
    assert!(result.target_symbol.doc_comment.as_ref().unwrap().contains("Generic Table TSX component"));
    assert!(result.target_symbol.body.contains("<div className=\"table-container\">"));
    assert!(result.target_symbol.body.contains("onClick={() => props.onRowSelect"));

    // 2. Verify hoisted types
    let type_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(type_names.contains(&"TableProps"), "Expected TableProps: {:?}", type_names);
    assert!(type_names.contains(&"RowItem"), "Expected RowItem (transitive depth 2): {:?}", type_names);
    assert!(!type_names.contains(&"T"), "Generic type T must not be hoisted");

    // 3. Verify stripped calls
    let call_names: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(call_names.contains(&"useTableSort"), "Expected useTableSort hook call: {:?}", call_names);

    // 4. Verify formatting
    let md = result.to_markdown();
    assert!(md.contains("```tsx"));
    assert!(md.contains("Generic Table TSX component"));
}

#[test]
fn test_adversarial_stress_performance_and_savings() {
    let slicer = warmup_engine();
    let fixtures = [
        (fixture_path("adversarial/complex_generics.ts"), "AdvancedRepository.findAndUnwrap"),
        (fixture_path("adversarial/deep_classes.ts"), "EngineController.streamMetrics"),
        (fixture_path("adversarial/mutual_recursion.ts"), "processRecursiveGraph"),
        (fixture_path("adversarial/barrel_hops/consumer.ts"), "runMultiHopAction"),
        (fixture_path("adversarial/cjs_mixed.js"), "processOrder"),
        (fixture_path("adversarial/GenericComponent.tsx"), "GenericTable"),
        (fixture_path("typescript/nested_types.ts"), "queryUsersWithFilter"),
    ];

    let opts = SliceOptions::default();

    for (file, symbol) in &fixtures {
        let start = Instant::now();
        let res = slicer
            .slice_symbol(file, symbol, &opts)
            .unwrap_or_else(|e| panic!("Failed slicing {}: {:?}", symbol, e));
        let elapsed = start.elapsed();

        println!("Bench [{}] -> {:?} (tokens: {} raw, {} sliced, savings: {:.1}%)",
            symbol, elapsed, res.stats.raw_file_tokens, res.stats.sliced_tokens, res.stats.savings_percentage
        );

        assert!(elapsed.as_millis() < 30, "Symbol {} slice took {}ms (>30ms)", symbol, elapsed.as_millis());
        assert!(res.stats.raw_file_tokens > 0);
        assert!(res.stats.sliced_tokens > 0);
        assert!(res.stats.savings_percentage >= 0.0 && res.stats.savings_percentage <= 100.0);
    }
}
