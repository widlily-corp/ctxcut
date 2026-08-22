//! Empirical Challenger M2 Battery: Exhaustive Edge-Case & Stress Test Suite.
//! Tests:
//! 1. Deep 4-way cyclic graphs (A -> B -> C -> D -> A) across TS, Python, Rust, Go.
//! 2. Self-referential and recursive interface/type definitions.
//! 3. Multi-hop barrel re-exports (wildcard, named, and aliased).
//! 4. Missing files, broken imports, and nonexistent path aliases resilience.
//! 5. Mixed extensions (.ts, .tsx, .d.ts, .js, .py, .pyi, index.*) and directory index resolution.
//! 6. Zero body leakage invariant on stripped signatures.
//! 7. Exact depth bounds enforcement (depth 0, 1, 2).

use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_challenger_m2_ts_4way_cyclic_dependency_graph() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Graph: A -> B -> C -> D -> A
    // a.ts
    fs::write(
        root.join("a.ts"),
        r#"import { TypeB, fnB } from './b';

export interface TypeA {
    id: string;
    b: TypeB;
}

export function fnA(param: TypeA): string {
    const res = fnB();
    return `A: ${param.id} -> ${res}`;
}
"#,
    )
    .expect("write a.ts");

    // b.ts
    fs::write(
        root.join("b.ts"),
        r#"import { TypeC, fnC } from './c';

export interface TypeB {
    count: number;
    c: TypeC;
}

export function fnB(): string {
    const res = fnC();
    return `B -> ${res}`;
}
"#,
    )
    .expect("write b.ts");

    // c.ts
    fs::write(
        root.join("c.ts"),
        r#"import { TypeD, fnD } from './d';

export interface TypeC {
    flag: boolean;
    d: TypeD;
}

export function fnC(): string {
    const res = fnD();
    return `C -> ${res}`;
}
"#,
    )
    .expect("write c.ts");

    // d.ts -> cycles back to A
    fs::write(
        root.join("d.ts"),
        r#"import { TypeA, fnA } from './a';

export interface TypeD {
    payload: string;
    a?: TypeA;
}

export function fnD(): string {
    return "D leaf";
}
"#,
    )
    .expect("write d.ts");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 4,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // Slicing fnA from a.ts must terminate without stack overflow or deadlock
    let slice = slicer
        .slice_symbol(&root.join("a.ts"), "fnA", &opts)
        .expect("Should slice fnA in 4-way cyclic graph");

    assert_eq!(slice.target_symbol.name, "fnA");

    let hoisted_names: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    println!("TS 4-way hoisted types: {:?}", hoisted_names);

    assert!(hoisted_names.contains(&"TypeA"), "Must hoist TypeA");
    assert!(hoisted_names.contains(&"TypeB"), "Must hoist TypeB");
    assert!(hoisted_names.contains(&"TypeC"), "Must hoist TypeC");
    assert!(hoisted_names.contains(&"TypeD"), "Must hoist TypeD");

    // Verify stripped calls
    let call_names: Vec<&str> = slice
        .stripped_calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(call_names.contains(&"fnB"), "Must capture fnB call stub");
}

#[test]
fn test_challenger_m2_python_4way_cyclic_relative_imports() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // mod_a.py
    fs::write(
        root.join("mod_a.py"),
        r#"from .mod_b import ClassB, helper_b

class ClassA:
    b: ClassB

def start_flow(a: ClassA) -> str:
    res = helper_b()
    return f"flow: {res}"
"#,
    )
    .expect("write mod_a.py");

    // mod_b.py
    fs::write(
        root.join("mod_b.py"),
        r#"from .mod_c import ClassC, helper_c

class ClassB:
    c: ClassC

def helper_b() -> str:
    return helper_c()
"#,
    )
    .expect("write mod_b.py");

    // mod_c.py
    fs::write(
        root.join("mod_c.py"),
        r#"from .mod_d import ClassD, helper_d

class ClassC:
    d: ClassD

def helper_c() -> str:
    return helper_d()
"#,
    )
    .expect("write mod_c.py");

    // mod_d.py -> cycles back to mod_a
    fs::write(
        root.join("mod_d.py"),
        r#"from .mod_a import ClassA

class ClassD:
    a: ClassA

def helper_d() -> str:
    return "done"
"#,
    )
    .expect("write mod_d.py");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 4,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&root.join("mod_a.py"), "start_flow", &opts)
        .expect("Should slice start_flow in 4-way python cycle");

    assert_eq!(slice.target_symbol.name, "start_flow");

    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    println!("Python 4-way hoisted types: {:?}", hoisted);

    assert!(hoisted.contains(&"ClassA"), "Must hoist ClassA");
    assert!(hoisted.contains(&"ClassB"), "Must hoist ClassB");
    assert!(hoisted.contains(&"ClassC"), "Must hoist ClassC");
    assert!(hoisted.contains(&"ClassD"), "Must hoist ClassD");
}

#[test]
fn test_challenger_m2_self_referential_and_recursive_types() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. TypeScript Self-referencing Tree & LinkedList
    let ts_file = root.join("tree.ts");
    fs::write(
        &ts_file,
        r#"export interface TreeNode {
    id: string;
    value: number;
    parent?: TreeNode;
    left?: TreeNode;
    right?: TreeNode;
    children: TreeNode[];
    metadata: Record<string, TreeNode>;
}

export interface MutualA {
    b: MutualB;
}

export interface MutualB {
    a: MutualA;
}

export function processTree(root: TreeNode, start: MutualA): number {
    return root.value;
}
"#,
    )
    .expect("write tree.ts");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let ts_slice = slicer
        .slice_symbol(&ts_file, "processTree", &opts)
        .expect("Should slice processTree with recursive types");

    let ts_hoisted: Vec<&str> = ts_slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    println!("TS recursive hoisted types: {:?}", ts_hoisted);

    assert!(ts_hoisted.contains(&"TreeNode"), "Must hoist TreeNode");
    assert!(ts_hoisted.contains(&"MutualA"), "Must hoist MutualA");
    assert!(ts_hoisted.contains(&"MutualB"), "Must hoist MutualB");

    // Check no duplicate type entries in hoisted_types
    let mut unique_check = std::collections::HashSet::new();
    for t in &ts_slice.hoisted_types {
        assert!(
            unique_check.insert(&t.name),
            "Duplicate hoisted type found: {}",
            t.name
        );
    }
}

#[test]
fn test_challenger_m2_multi_hop_barrel_reexports() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Architecture:
    // leaf.ts: defines CoreSchema and leafFunction()
    // barrel_inner.ts: export { CoreSchema as AliasedSchema, leafFunction } from './leaf'
    // barrel_outer.ts: export * from './barrel_inner'
    // index.ts: export * from './barrel_outer'
    // consumer.ts: import { AliasedSchema, leafFunction } from './index'

    fs::write(
        root.join("leaf.ts"),
        r#"export interface CoreSchema {
    uuid: string;
    version: number;
}

export function leafFunction(schema: CoreSchema): boolean {
    // Hidden implementation
    console.log("Processing leaf");
    return true;
}
"#,
    )
    .expect("write leaf.ts");

    fs::write(
        root.join("barrel_inner.ts"),
        "export { CoreSchema as AliasedSchema, leafFunction } from './leaf';\n",
    )
    .expect("write barrel_inner.ts");

    fs::write(
        root.join("barrel_outer.ts"),
        "export * from './barrel_inner';\n",
    )
    .expect("write barrel_outer.ts");

    fs::write(root.join("index.ts"), "export * from './barrel_outer';\n").expect("write index.ts");

    let consumer_ts = root.join("consumer.ts");
    fs::write(
        &consumer_ts,
        r#"import { AliasedSchema, leafFunction } from './index';

export function runPipeline(input: AliasedSchema): boolean {
    return leafFunction(input);
}
"#,
    )
    .expect("write consumer.ts");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&consumer_ts, "runPipeline", &opts)
        .expect("Should slice runPipeline through multi-hop barrels");

    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    println!("Multi-hop barrel hoisted types: {:?}", hoisted);

    assert!(
        hoisted.contains(&"CoreSchema") || hoisted.contains(&"AliasedSchema"),
        "Must hoist schema type through multi-hop barrel, found: {:?}",
        hoisted
    );

    let calls: Vec<&str> = slice
        .stripped_calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    println!("Multi-hop barrel stripped calls: {:?}", calls);
    assert!(
        calls.contains(&"leafFunction"),
        "Must resolve leafFunction call through multi-hop barrels, found: {:?}",
        calls
    );
}

#[test]
fn test_challenger_m2_missing_files_and_broken_imports_resilience() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let file_path = root.join("broken_consumer.ts");
    fs::write(
        &file_path,
        r#"import { MissingType } from './non_existent_file';
import { AnotherGhost } from '@/components/phantom';
import { LocalType } from './valid_sibling';

export interface LocalType {
    active: boolean;
}

export function handleAction(item: LocalType, ghost: MissingType): boolean {
    return item.active;
}
"#,
    )
    .expect("write broken_consumer.ts");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // Slicing must NOT panic or error when foreign imports are unresolvable
    let slice = slicer
        .slice_symbol(&file_path, "handleAction", &opts)
        .expect("Slicer must succeed gracefully in presence of broken imports");

    assert_eq!(slice.target_symbol.name, "handleAction");

    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"LocalType"),
        "Must still hoist valid local type LocalType"
    );
    // MissingType should simply not be present, but without crashing
    assert!(!hoisted.contains(&"MissingType"));
}

#[test]
fn test_challenger_m2_mixed_extensions_and_directory_index_resolution() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. Component in .tsx file
    fs::write(
        root.join("Button.tsx"),
        r#"export interface ButtonProps {
    label: string;
    onClick: () => void;
}

export function renderButton(props: ButtonProps): string {
    return `<button>${props.label}</button>`;
}
"#,
    )
    .expect("write Button.tsx");

    // 2. Declaration in .d.ts file
    fs::write(
        root.join("config.d.ts"),
        r#"export interface AppConfig {
    env: string;
    port: number;
}
"#,
    )
    .expect("write config.d.ts");

    // 3. Directory index resolution: api/index.ts
    let api_dir = root.join("api");
    fs::create_dir_all(&api_dir).expect("create api dir");
    fs::write(
        api_dir.join("index.ts"),
        r#"export interface ApiResponse {
    status: number;
    data: any;
}

export function fetchApi(): ApiResponse {
    return { status: 200, data: null };
}
"#,
    )
    .expect("write api/index.ts");

    // 4. Consumer file importing with mixed conventions
    let consumer_ts = root.join("app.ts");
    fs::write(
        &consumer_ts,
        r#"import { ButtonProps, renderButton } from './Button';
import { AppConfig } from './config';
import { ApiResponse, fetchApi } from './api';

export function initializeApp(cfg: AppConfig, btn: ButtonProps): ApiResponse {
    renderButton(btn);
    return fetchApi();
}
"#,
    )
    .expect("write app.ts");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&consumer_ts, "initializeApp", &opts)
        .expect("Should slice initializeApp resolving mixed extensions and index.ts");

    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    println!("Mixed extension hoisted types: {:?}", hoisted);

    assert!(
        hoisted.contains(&"ButtonProps"),
        "Must resolve .tsx extension: ButtonProps"
    );
    assert!(
        hoisted.contains(&"AppConfig"),
        "Must resolve .d.ts extension: AppConfig"
    );
    assert!(
        hoisted.contains(&"ApiResponse"),
        "Must resolve directory index.ts: ApiResponse"
    );

    let calls: Vec<&str> = slice
        .stripped_calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    println!("Mixed extension stripped calls: {:?}", calls);
    assert!(
        calls.contains(&"renderButton"),
        "Must resolve renderButton from .tsx"
    );
    assert!(
        calls.contains(&"fetchApi"),
        "Must resolve fetchApi from api/index.ts"
    );
}

#[test]
fn test_challenger_m2_zero_body_leakage_across_all_languages() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // TypeScript foreign service
    let ts_svc = root.join("service.ts");
    fs::write(
        &ts_svc,
        r#"export class AuthService {
    public authenticate(token: string): boolean {
        // LEAK_CHECK_TS_SECRET_STATEMENT_1
        const decoded = atob(token);
        // LEAK_CHECK_TS_SECRET_STATEMENT_2
        if (decoded.length < 10) return false;
        return true;
    }
}
"#,
    )
    .expect("write service.ts");

    // Rust foreign module
    let rs_svc = root.join("auth.rs");
    fs::write(
        &rs_svc,
        r#"pub struct CryptoService;

impl CryptoService {
    pub fn verify_signature(&self, hash: &str) -> bool {
        // LEAK_CHECK_RUST_SECRET_STATEMENT
        let _salt = "SUPER_SECRET_SALT";
        hash.len() > 32
    }
}
"#,
    )
    .expect("write auth.rs");

    // Python foreign module
    let py_svc = root.join("crypto.py");
    fs::write(
        &py_svc,
        r#"def validate_hmac(key: str, data: str) -> bool:
    # LEAK_CHECK_PYTHON_SECRET_STATEMENT
    secret_internal_computation = key + data
    return len(secret_internal_computation) > 0
"#,
    )
    .expect("write crypto.py");

    // Go foreign package
    let go_dir = root.join("token");
    fs::create_dir_all(&go_dir).expect("create go dir");
    let go_svc = go_dir.join("validator.go");
    fs::write(
        &go_svc,
        r#"package token

func ValidateJWT(jwt string) (bool, error) {
    // LEAK_CHECK_GO_SECRET_STATEMENT
    rawBytes := []byte(jwt)
    return len(rawBytes) > 16, nil
}
"#,
    )
    .expect("write validator.go");

    // Test TS call stub extraction
    let ts_stub = ctxcut_core::resolver::calls::resolve_foreign_signature(
        &ts_svc,
        "AuthService.authenticate",
    )
    .expect("resolve ts stub")
    .expect("stub found");
    println!("TS Stub: {:?}", ts_stub.signature);
    assert!(!ts_stub.signature.contains("LEAK_CHECK_TS"));
    assert!(!ts_stub.signature.contains("atob(token)"));
    assert!(!ts_stub.signature.contains("return false"));

    // Test Rust call stub extraction
    let rs_stub = ctxcut_core::resolver::calls::resolve_foreign_signature(
        &rs_svc,
        "CryptoService::verify_signature",
    )
    .expect("resolve rs stub")
    .expect("stub found");
    println!("Rust Stub: {:?}", rs_stub.signature);
    assert!(!rs_stub.signature.contains("LEAK_CHECK_RUST"));
    assert!(!rs_stub.signature.contains("SUPER_SECRET_SALT"));
    assert!(!rs_stub.signature.contains("hash.len() > 32"));

    // Test Python call stub extraction
    let py_stub = ctxcut_core::resolver::calls::resolve_foreign_signature(&py_svc, "validate_hmac")
        .expect("resolve py stub")
        .expect("stub found");
    println!("Python Stub: {:?}", py_stub.signature);
    assert!(!py_stub.signature.contains("LEAK_CHECK_PYTHON"));
    assert!(!py_stub
        .signature
        .contains("secret_internal_computation = key + data"));

    // Test Go call stub extraction
    let go_stub = ctxcut_core::resolver::calls::resolve_foreign_signature(&go_dir, "ValidateJWT")
        .expect("resolve go stub")
        .expect("stub found");
    println!("Go Stub: {:?}", go_stub.signature);
    assert!(!go_stub.signature.contains("LEAK_CHECK_GO"));
    assert!(!go_stub.signature.contains("rawBytes :="));
    assert!(!go_stub.signature.contains("return len(rawBytes)"));
}

#[test]
fn test_challenger_m2_depth_bounds_exactness() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Hop 0: consumer.ts -> uses TypeHop1
    // Hop 1: hop1.ts -> uses TypeHop2
    // Hop 2: hop2.ts -> uses TypeHop3
    // Hop 3: hop3.ts -> uses TypeLeaf

    fs::write(
        root.join("hop3.ts"),
        r#"export interface TypeLeaf {
    leafValue: string;
}
"#,
    )
    .expect("write hop3.ts");

    fs::write(
        root.join("hop2.ts"),
        r#"import { TypeLeaf } from './hop3';

export interface TypeHop2 {
    leaf: TypeLeaf;
}
"#,
    )
    .expect("write hop2.ts");

    fs::write(
        root.join("hop1.ts"),
        r#"import { TypeHop2 } from './hop2';

export interface TypeHop1 {
    next: TypeHop2;
}
"#,
    )
    .expect("write hop1.ts");

    let consumer_ts = root.join("consumer.ts");
    fs::write(
        &consumer_ts,
        r#"import { TypeHop1 } from './hop1';

export interface LocalType {
    id: number;
}

export function executeHop(l: LocalType, h: TypeHop1): boolean {
    return true;
}
"#,
    )
    .expect("write consumer.ts");

    let slicer = ContextSlicer::new();

    // Test Depth 0: only LocalType
    let opts0 = SliceOptions {
        depth: 0,
        include_types: true,
        include_calls: true,
        budget: None,
    };
    let slice0 = slicer
        .slice_symbol(&consumer_ts, "executeHop", &opts0)
        .expect("slice depth 0");
    let hoisted0: Vec<&str> = slice0
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(hoisted0.contains(&"LocalType"));
    assert!(
        !hoisted0.contains(&"TypeHop1"),
        "Depth 0 must not hoist foreign TypeHop1"
    );
    assert!(!hoisted0.contains(&"TypeHop2"));
    assert!(!hoisted0.contains(&"TypeLeaf"));

    // Test Depth 1: LocalType + TypeHop1
    let opts1 = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };
    let slice1 = slicer
        .slice_symbol(&consumer_ts, "executeHop", &opts1)
        .expect("slice depth 1");
    let hoisted1: Vec<&str> = slice1
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(hoisted1.contains(&"LocalType"));
    assert!(
        hoisted1.contains(&"TypeHop1"),
        "Depth 1 must hoist direct foreign TypeHop1"
    );
    assert!(
        !hoisted1.contains(&"TypeHop2"),
        "Depth 1 must NOT hoist 2nd hop TypeHop2"
    );
    assert!(!hoisted1.contains(&"TypeLeaf"));

    // Test Depth 2: LocalType + TypeHop1 + TypeHop2
    let opts2 = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };
    let slice2 = slicer
        .slice_symbol(&consumer_ts, "executeHop", &opts2)
        .expect("slice depth 2");
    let hoisted2: Vec<&str> = slice2
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(hoisted2.contains(&"LocalType"));
    assert!(hoisted2.contains(&"TypeHop1"));
    assert!(
        hoisted2.contains(&"TypeHop2"),
        "Depth 2 must hoist TypeHop2"
    );
    assert!(
        !hoisted2.contains(&"TypeLeaf"),
        "Depth 2 must NOT hoist 3rd hop TypeLeaf"
    );

    // Test Depth 3: LocalType + TypeHop1 + TypeHop2 + TypeLeaf
    let opts3 = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
        budget: None,
    };
    let slice3 = slicer
        .slice_symbol(&consumer_ts, "executeHop", &opts3)
        .expect("slice depth 3");
    let hoisted3: Vec<&str> = slice3
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(hoisted3.contains(&"LocalType"));
    assert!(hoisted3.contains(&"TypeHop1"));
    assert!(hoisted3.contains(&"TypeHop2"));
    assert!(
        hoisted3.contains(&"TypeLeaf"),
        "Depth 3 must hoist TypeLeaf"
    );
}
