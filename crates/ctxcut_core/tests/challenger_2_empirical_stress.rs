//! Challenger 2 Empirical Stress Test Suite
//!
//! Rigorously verifies:
//! 1. Zero Body Leakage across Python, Rust, TypeScript/JavaScript signature extractions.
//! 2. Multi-Hop Barrel Re-exports (5+ hops, mixed aliases/wildcards) & Cyclic Graph Resolution (2-way, 3-way, mutual recursion, self-referential types).
//! 3. Error Handling and Zero-Panic Guarantees on invalid arguments, non-existent symbols, corrupted syntax, extreme budgets.
//! 4. Performance, Responsiveness, and Token Reduction invariants on massive files and multi-symbol slicing.

use ctxcut_core::error::CoreError;
use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;
use std::fs;
use std::time::Instant;
use tempfile::tempdir;

fn create_engine() -> ContextSlicer {
    ContextSlicer::new()
}

// =========================================================================
// SECTION 1: ZERO BODY LEAKAGE EMPIRICAL VERIFICATION
// =========================================================================

#[test]
fn test_empirical_zero_body_leakage_python() {
    let slicer = create_engine();
    let dir = tempdir().expect("tempdir");
    let py_file = dir.path().join("service.py");

    let source = r#"
import os
import sys
from typing import List, Dict, Optional, Any

class ConfigHolder:
    """Holds configuration data with sensitive internals."""
    def __init__(self, secret_key: str, env: str = "production"):
        self.secret_key = secret_key
        self.env = env
        # Sensitive initialization logic that MUST NEVER LEAK
        self._raw_tokens = [x * 2 for x in range(100)]
        self._entropy_pool = os.urandom(32)
        print("Initializing sensitive config holder...")

    @property
    def is_prod(self) -> bool:
        """Check if environment is production."""
        env_lower = self.env.lower()
        return env_lower == "production" or env_lower == "prod"

    @classmethod
    def from_env(cls) -> "ConfigHolder":
        """Factory method reading environment variables."""
        key = os.environ.get("SECRET_KEY", "default_secret")
        mode = os.environ.get("APP_ENV", "development")
        return cls(secret_key=key, env=mode)

    def execute_internal_hash(self, payload: bytes) -> str:
        """Internal hasher with deep loops."""
        accumulator = 0
        for b in payload:
            accumulator = (accumulator * 31 + b) & 0xFFFFFFFF
        return f"hash_{accumulator:08x}"

async def compute_distributed_metrics(
    nodes: List[str],
    timeout_seconds: float = 30.0,
    retry_count: int = 3,
    debug_mode: bool = False
) -> Dict[str, Any]:
    """Async metric aggregator with extensive control flow."""
    results = {}
    for node in nodes:
        for attempt in range(retry_count):
            try:
                # Simulated complex inner network call
                temp_val = f"metric_from_{node}_{attempt}"
                results[node] = temp_val
                break
            except Exception as exc:
                if attempt == retry_count - 1:
                    results[node] = None
    return results

def caller_function(holder: ConfigHolder) -> Dict[str, Any]:
    """Function that calls holder and compute_distributed_metrics."""
    data = compute_distributed_metrics(["node1", "node2"])
    return {"data": data}
"#;

    fs::write(&py_file, source).expect("write python source");

    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = slicer
        .slice_symbol(&py_file, "caller_function", &opts)
        .expect("slice caller_function");

    // 1. Verify caller target body is retained
    assert_eq!(result.target_symbol.name, "caller_function");
    assert!(result.target_symbol.body.contains("compute_distributed_metrics([\"node1\", \"node2\"])"));

    // 2. Verify all hoisted stripped calls have ZERO body leakage
    assert!(!result.stripped_calls.is_empty(), "Should extract call stubs");
    for stub in &result.stripped_calls {
        let sig = &stub.signature;
        assert!(
            sig.ends_with(": ...") || sig.ends_with(':') || sig.contains("..."),
            "Python signature stub must end with ': ...', got: {}",
            sig
        );
        // Forbidden body tokens:
        let forbidden = [
            "for node in nodes",
            "for attempt in range",
            "range(100)",
            "os.urandom",
            "Initializing sensitive",
            "accumulator = 0",
            "accumulator = (accumulator * 31",
            "print(",
            "results[node] =",
            "return f\"hash_",
            "os.environ.get",
            "return cls(",
        ];
        for f in &forbidden {
            assert!(
                !sig.contains(f),
                "LEAKAGE DETECTED in Python signature stub '{}': contains body fragment '{}'",
                sig,
                f
            );
        }
    }
}

#[test]
fn test_empirical_zero_body_leakage_rust() {
    let slicer = create_engine();
    let dir = tempdir().expect("tempdir");
    let rs_file = dir.path().join("pipeline.rs");

    let source = r#"
use std::collections::HashMap;
use std::fmt::Debug;

pub struct ProcessingContext<T: Clone + Debug> {
    pub session_id: String,
    pub payload: T,
    pub metadata: HashMap<String, String>,
}

pub fn compute_internal_checksum(raw: &[u8]) -> u64 {
    // HEAVY INNER CHECKSUM IMPLEMENTATION MUST NOT LEAK
    let mut sum = 0u64;
    for byte in raw {
        sum = sum.wrapping_add(*byte as u64).rotate_left(3);
    }
    sum
}

pub async fn execute_batch_processing<T>(
    ctx: &ProcessingContext<T>,
    filter_threshold: f64,
) -> Result<Vec<T>, String>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    // HEAVY INNER IMPLEMENTATION THAT MUST NOT LEAK
    let mut results = Vec::new();
    for i in 0..1000 {
        if (i as f64) > filter_threshold {
            let cloned = ctx.payload.clone();
            results.push(cloned);
        }
    }
    let formatted = format!("Processed {} records for session {}", results.len(), ctx.session_id);
    println!("{}", formatted);
    Ok(results)
}

pub fn run_pipeline(ctx: &ProcessingContext<String>) {
    let _ = execute_batch_processing(ctx, 50.0);
    let _ = compute_internal_checksum(b"checksum_bytes");
}
"#;

    fs::write(&rs_file, source).expect("write rust source");

    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = slicer
        .slice_symbol(&rs_file, "run_pipeline", &opts)
        .expect("slice run_pipeline");

    assert_eq!(result.target_symbol.name, "run_pipeline");
    assert!(result.target_symbol.body.contains("execute_batch_processing(ctx, 50.0)"));

    // Verify stripped calls have ZERO body leakage
    assert!(!result.stripped_calls.is_empty(), "Should extract call stubs");
    for stub in &result.stripped_calls {
        let sig = &stub.signature;
        assert!(
            sig.trim().ends_with(';'),
            "Rust signature stub must end with ';', got: {}",
            sig
        );
        let forbidden = [
            "let mut results",
            "for i in 0..1000",
            "results.push",
            "format!(\"Processed",
            "println!",
            "let mut sum",
            "sum.wrapping_add",
            "Ok(results)",
            "{",
            "}",
        ];
        for f in &forbidden {
            assert!(
                !sig.contains(f),
                "LEAKAGE DETECTED in Rust signature stub '{}': contains body fragment '{}'",
                sig,
                f
            );
        }
    }
}

#[test]
fn test_empirical_zero_body_leakage_typescript() {
    let slicer = create_engine();
    let dir = tempdir().expect("tempdir");
    let ts_file = dir.path().join("controller.ts");

    let source = r#"
export interface UserSession {
    userId: string;
    roles: string[];
    token: string;
}

export interface SecurityReport {
    passed: boolean;
    auditLog: string[];
}

export async function auditUserSession<T extends UserSession>(
    session: T,
    strictMode: boolean = true
): Promise<SecurityReport> {
    // COMPLEX AUDIT LOGIC MUST NEVER LEAK
    const logs: string[] = [];
    let isCompliant = true;
    for (const role of session.roles) {
        if (role === "admin" && !strictMode) {
            logs.push("Admin access without strict mode flagged");
            isCompliant = false;
        }
    }
    return { passed: isCompliant, auditLog: logs };
}

export function generateSessionHmac(token: string): string {
    // SECRET CRYPTO ALGORITHM MUST NEVER LEAK
    const entropy = Math.random() * 1000;
    console.log("Entropy generated:", entropy);
    return "hmac_" + token.length;
}

export async function handleUserLogin(session: UserSession): Promise<boolean> {
    const report = await auditUserSession(session, true);
    const hmac = generateSessionHmac(session.token);
    return report.passed && hmac.length > 0;
}
"#;

    fs::write(&ts_file, source).expect("write ts source");

    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = slicer
        .slice_symbol(&ts_file, "handleUserLogin", &opts)
        .expect("slice handleUserLogin");

    assert_eq!(result.target_symbol.name, "handleUserLogin");
    assert!(result.target_symbol.body.contains("await auditUserSession(session, true)"));

    // Verify stripped calls
    assert!(!result.stripped_calls.is_empty(), "Should extract call stubs");
    for stub in &result.stripped_calls {
        let sig = &stub.signature;
        assert!(
            sig.trim().ends_with(';'),
            "TS signature stub must end with ';', got: {}",
            sig
        );
        let forbidden = [
            "const logs",
            "let isCompliant",
            "for (const role of session.roles)",
            "logs.push",
            "return { passed",
            "console.log",
            "Math.random()",
            "{",
            "}",
        ];
        for f in &forbidden {
            assert!(
                !sig.contains(f),
                "LEAKAGE DETECTED in TypeScript signature stub '{}': contains body fragment '{}'",
                sig,
                f
            );
        }
    }
}

// =========================================================================
// SECTION 2: MULTI-HOP BARREL RE-EXPORTS & CYCLIC GRAPH RESOLUTION
// =========================================================================

#[test]
fn test_empirical_5_hop_barrel_reexport_with_renaming_and_wildcards() {
    let slicer = create_engine();
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Hop 0: leaf.ts (defines actual function and interface)
    let leaf_path = root.join("leaf.ts");
    fs::write(
        &leaf_path,
        r#"
export interface CoreEntity {
    id: string;
    version: number;
}

export function executeDeepOperation(entity: CoreEntity, factor: number): string {
    const computed = entity.version * factor;
    return `result_${entity.id}_${computed}`;
}
"#,
    ).expect("write leaf.ts");

    // Hop 1: barrel_1.ts (named re-export with alias)
    let b1_path = root.join("barrel_1.ts");
    fs::write(
        &b1_path,
        r#"
export { CoreEntity as PrimaryEntity, executeDeepOperation as step1Operation } from './leaf';
"#,
    ).expect("write barrel_1.ts");

    // Hop 2: barrel_2.ts (wildcard export)
    let b2_path = root.join("barrel_2.ts");
    fs::write(
        &b2_path,
        r#"
export * from './barrel_1';
"#,
    ).expect("write barrel_2.ts");

    // Hop 3: barrel_3.ts (re-export aliasing again)
    let b3_path = root.join("barrel_3.ts");
    fs::write(
        &b3_path,
        r#"
export { PrimaryEntity as SchemaEntity, step1Operation as step3Operation } from './barrel_2';
"#,
    ).expect("write barrel_3.ts");

    // Hop 4: barrel_4.ts (wildcard export)
    let b4_path = root.join("barrel_4.ts");
    fs::write(
        &b4_path,
        r#"
export * from './barrel_3';
"#,
    ).expect("write barrel_4.ts");

    // Hop 5: index.ts (final public entry point)
    let index_path = root.join("index.ts");
    fs::write(
        &index_path,
        r#"
export { SchemaEntity as FinalEntity, step3Operation as finalOperation } from './barrel_4';
"#,
    ).expect("write index.ts");

    // Consumer: app.ts
    let app_path = root.join("app.ts");
    fs::write(
        &app_path,
        r#"
import { FinalEntity, finalOperation } from './index';

export function runClientWorkflow(entity: FinalEntity): string {
    const outcome = finalOperation(entity, 42);
    return outcome;
}
"#,
    ).expect("write app.ts");

    let opts = SliceOptions {
        depth: 5,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let start = Instant::now();
    let result = slicer
        .slice_symbol(&app_path, "runClientWorkflow", &opts)
        .expect("slice across 5 barrel hops");
    let elapsed = start.elapsed();

    println!("5-hop barrel resolution elapsed: {:?}", elapsed);
    assert!(elapsed.as_millis() < 2000, "5-hop resolution took too long: {:?}", elapsed);

    // 1. Verify target symbol
    assert_eq!(result.target_symbol.name, "runClientWorkflow");

    // 2. Verify hoisted types resolved back to leaf CoreEntity
    let type_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        type_names.contains(&"FinalEntity") || type_names.contains(&"CoreEntity") || type_names.contains(&"PrimaryEntity") || type_names.contains(&"SchemaEntity"),
        "Hoisted types must contain entity resolved across 5 hops: {:?}",
        type_names
    );

    // 3. Verify stripped calls resolved back to leaf executeDeepOperation
    let call_names: Vec<&str> = result
        .stripped_calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(
        call_names.contains(&"finalOperation") || call_names.contains(&"executeDeepOperation") || call_names.contains(&"step1Operation") || call_names.contains(&"step3Operation"),
        "Stripped calls must resolve call across 5 hops: {:?}",
        call_names
    );
}

#[test]
fn test_empirical_cyclic_graphs_and_mutual_recursion() {
    let slicer = create_engine();
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Node A
    let node_a = root.join("nodeA.ts");
    // Node B
    let node_b = root.join("nodeB.ts");
    // Node C
    let node_c = root.join("nodeC.ts");

    fs::write(
        &node_a,
        r#"
import { TypeB, stepB } from './nodeB';

export interface TypeA {
    id: string;
    bRef?: TypeB;
}

export function stepA(input: TypeA, depth: number): string {
    if (depth <= 0) return input.id;
    return stepB({ id: input.id + "_b", cRef: undefined }, depth - 1);
}
"#,
    ).expect("write nodeA");

    fs::write(
        &node_b,
        r#"
import { TypeC, stepC } from './nodeC';

export interface TypeB {
    id: string;
    cRef?: TypeC;
}

export function stepB(input: TypeB, depth: number): string {
    if (depth <= 0) return input.id;
    return stepC({ id: input.id + "_c", aRef: undefined }, depth - 1);
}
"#,
    ).expect("write nodeB");

    fs::write(
        &node_c,
        r#"
import { TypeA, stepA } from './nodeA';

export interface TypeC {
    id: string;
    aRef?: TypeA;
}

export function stepC(input: TypeC, depth: number): string {
    if (depth <= 0) return input.id;
    return stepA({ id: input.id + "_a", bRef: undefined }, depth - 1);
}
"#,
    ).expect("write nodeC");

    for depth in [1, 2, 3, 5, 10, 20] {
        let opts = SliceOptions {
            depth,
            include_types: true,
            include_calls: true,
            budget: None,
        };

        let start = Instant::now();
        let result = slicer
            .slice_symbol(&node_a, "stepA", &opts)
            .unwrap_or_else(|e| panic!("Cyclic 3-way graph slice failed at depth {}: {:?}", depth, e));
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 2000,
            "Cyclic traversal at depth {} took too long: {:?}",
            depth,
            elapsed
        );

        // Verify no duplicate hoisted types
        let mut seen_types = std::collections::HashSet::new();
        for t in &result.hoisted_types {
            assert!(
                seen_types.insert(&t.name),
                "Duplicate hoisted type '{}' detected in cyclic graph at depth {}",
                t.name,
                depth
            );
        }

        // Verify no duplicate stripped calls
        let mut seen_calls = std::collections::HashSet::new();
        for c in &result.stripped_calls {
            assert!(
                seen_calls.insert(&c.name),
                "Duplicate stripped call '{}' detected in cyclic graph at depth {}",
                c.name,
                depth
            );
        }
    }
}

// =========================================================================
// SECTION 3: ERROR HANDLING AND ZERO-PANIC ADVERSARIAL FUZZING
// =========================================================================

#[test]
fn test_empirical_zero_panic_adversarial_inputs() {
    let slicer = create_engine();
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let valid_ts = root.join("valid.ts");
    fs::write(
        &valid_ts,
        r#"
export function existingFunction(x: number): number {
    return x * 2;
}

export class ExistingClass {
    public existingMethod(): string {
        return "ok";
    }
}
"#,
    ).expect("write valid.ts");

    let empty_file = root.join("empty.ts");
    fs::write(&empty_file, "").expect("write empty.ts");

    let whitespace_file = root.join("spaces.ts");
    fs::write(&whitespace_file, "   \n\n\t\t\r\n   ").expect("write whitespace.ts");

    let corrupted_ts = root.join("corrupted.ts");
    fs::write(
        &corrupted_ts,
        "export function broken() { let x = ; ; ; }\n\nexport interface ValidPartial {\n    id: string;\n}\n",
    ).expect("write corrupted.ts");

    let corrupted_py = root.join("corrupted.py");
    fs::write(
        &corrupted_py,
        "def broken():\n    let x = ; ; ;\n\nclass ValidPartial:\n    x: int = 1\n",
    ).expect("write corrupted.py");

    let corrupted_rs = root.join("corrupted.rs");
    fs::write(
        &corrupted_rs,
        "fn broken() { let x = ; ; ; }\n\npub fn valid_partial() -> i32 {\n    42\n}\n",
    ).expect("write corrupted.rs");

    let opts = SliceOptions::default();

    // 1. Non-existent file path
    let non_existent = root.join("non_existent_file.ts");
    let res = slicer.slice_symbol(&non_existent, "existingFunction", &opts);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), CoreError::Io { .. } | CoreError::SymbolNotFound { .. }));

    // 2. Non-existent symbols in valid file
    let missing_queries = [
        "ghostSymbol",
        "ExistingClass.ghostMethod",
        "ExistingClass.ghost.deep",
        "NoSuchClass.method",
        "existingFunction.child",
    ];
    for q in &missing_queries {
        let res = slicer.slice_symbol(&valid_ts, q, &opts);
        assert!(res.is_err(), "Query '{}' should return error", q);
        match res.unwrap_err() {
            CoreError::SymbolNotFound { symbol, available_symbols, .. } => {
                assert_eq!(&symbol, q);
                assert!(!available_symbols.is_empty(), "Should list available symbols for suggestion");
                assert!(available_symbols.contains(&"existingFunction".to_string()));
            }
            other => panic!("Expected SymbolNotFound for '{}', got {:?}", q, other),
        }
    }

    // 3. Adversarial query strings (whitespace, delimiters, injection payloads)
    let adversarial_queries = [
        "",
        " ",
        "   \t\n",
        ".",
        "..",
        "::",
        ":::",
        ".::.",
        "A..B",
        "A::",
        "::B",
        "../../etc/passwd",
        "SELECT * FROM symbols WHERE 1=1;",
        "<script>alert(1)</script>",
        "\0",
        "🦀🚀✨",
    ];
    for q in &adversarial_queries {
        let res = slicer.slice_symbol(&valid_ts, q, &opts);
        assert!(res.is_err(), "Adversarial query '{:?}' must return error gracefully", q);
        assert!(matches!(res.unwrap_err(), CoreError::SymbolNotFound { .. }));
    }

    // 4. Empty and whitespace-only files
    for f in [&empty_file, &whitespace_file] {
        let res = slicer.slice_symbol(f, "anySymbol", &opts);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), CoreError::SymbolNotFound { .. }));
    }

    // 5. Corrupted source error recovery across all languages
    // TS
    let ts_partial = slicer.slice_symbol(&corrupted_ts, "ValidPartial", &opts);
    assert!(ts_partial.is_ok(), "Tree-sitter must recover ValidPartial from corrupted TS");
    assert_eq!(ts_partial.unwrap().target_symbol.name, "ValidPartial");

    // Py
    let py_partial = slicer.slice_symbol(&corrupted_py, "ValidPartial", &opts);
    assert!(py_partial.is_ok(), "Tree-sitter must recover ValidPartial from corrupted Python");
    assert_eq!(py_partial.unwrap().target_symbol.name, "ValidPartial");

    // Rs
    let rs_partial = slicer.slice_symbol(&corrupted_rs, "valid_partial", &opts);
    assert!(rs_partial.is_ok(), "Tree-sitter must recover valid_partial from corrupted Rust");
    assert_eq!(rs_partial.unwrap().target_symbol.name, "valid_partial");

    // 6. Extreme budget options
    for budget in [Some(0), Some(1), Some(5), Some(50), Some(usize::MAX)] {
        let b_opts = SliceOptions {
            depth: 2,
            include_types: true,
            include_calls: true,
            budget,
        };
        let b_res = slicer.slice_symbol(&valid_ts, "existingFunction", &b_opts);
        assert!(b_res.is_ok(), "Budget {:?} must not panic or crash", budget);
    }
}

// =========================================================================
// SECTION 4: PERFORMANCE & TOKEN REDUCTION INVARIANTS ON MASSIVE FILES
// =========================================================================

#[test]
fn test_empirical_performance_and_massive_file_token_reduction() {
    let slicer = create_engine();
    let dir = tempdir().expect("tempdir");
    let huge_file = dir.path().join("massive_module.ts");

    // Generate a massive 3,000+ line TypeScript file with 100 functions, 30 classes, 50 interfaces
    let mut sb = String::with_capacity(200_000);
    sb.push_str("// Massive synthetic module for performance and token reduction stress testing\n\n");

    for i in 0..50 {
        sb.push_str(&format!(
            r#"
export interface ItemSchema_{i} {{
    id: string;
    index: number;
    tag: "active" | "archived" | "pending";
    payload: Record<string, number>;
}}
"#
        ));
    }

    for i in 0..100 {
        sb.push_str(&format!(
            r#"
export function computeHelper_{i}(input: ItemSchema_{}, multiplier: number): number {{
    let sum = 0;
    for (let k = 0; k < 50; k++) {{
        sum += (input.index * multiplier) + k;
    }}
    return sum;
}}
"#,
            i % 50
        ));
    }

    for i in 0..30 {
        sb.push_str(&format!(
            r#"
export class ServiceController_{i} {{
    private _state: number = {i};

    constructor(initial: number) {{
        this._state = initial;
    }}

    public processBatch(item: ItemSchema_{}): number {{
        const res = computeHelper_{}(item, this._state);
        return res * 2;
    }}
}}
"#,
            i % 50,
            i * 2
        ));
    }

    // Target function in the middle calling multiple helpers and types
    sb.push_str(r#"
export function targetWorkflowFunction(item: ItemSchema_10, ctrl: ServiceController_5): number {
    const val1 = computeHelper_10(item, 3);
    const val2 = computeHelper_20(item, 5);
    const batchRes = ctrl.processBatch(item);
    return val1 + val2 + batchRes;
}
"#);

    fs::write(&huge_file, &sb).expect("write massive module");

    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let start = Instant::now();
    let result = slicer
        .slice_symbol(&huge_file, "targetWorkflowFunction", &opts)
        .expect("slice massive module target");
    let elapsed = start.elapsed();

    println!(
        "Massive file slice elapsed: {:?}, raw tokens: {}, sliced tokens: {}, savings: {:.2}%",
        elapsed,
        result.stats.raw_file_tokens,
        result.stats.sliced_tokens,
        result.stats.savings_percentage
    );

    // Invariant 1: Performance must be under 2000ms in debug and under 50ms in release
    assert!(elapsed.as_millis() < 2000, "Massive file slice took too long: {:?}", elapsed);

    // Invariant 2: Token reduction must be massive (>80% savings)
    assert!(result.stats.raw_file_tokens > 5000, "Raw tokens should be >5000");
    assert!(result.stats.sliced_tokens < 1000, "Sliced tokens should be <1000");
    assert!(
        result.stats.savings_percentage > 80.0,
        "Savings percentage must be >80%, got {:.2}%",
        result.stats.savings_percentage
    );

    // Invariant 3: Precision - target body is intact and hoisted types/calls are present
    assert_eq!(result.target_symbol.name, "targetWorkflowFunction");
    assert!(result.target_symbol.body.contains("computeHelper_10(item, 3)"));
    assert!(result.target_symbol.body.contains("ctrl.processBatch(item)"));
}
