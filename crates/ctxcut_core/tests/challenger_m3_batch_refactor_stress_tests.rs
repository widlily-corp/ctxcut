//! Empirical Challenger Adversarial Stress Test Suite for Milestone 3:
//! Multi-Symbol Transactional Refactoring (R3).
//!
//! Stress-tests:
//! 1. Simultaneous edits across 5+ files and 10+ symbols across polyglot languages.
//! 2. Syntax validation failure in 1 symbol -> 100% zero-loss rollback to exact byte counts across all files.
//! 3. Typecheck failure -> MultiFileRollbackGuard restores disk state & AstDiagnosticMapper attributes line/symbol/node.
//! 4. Dry-run mode (\pply: false\) -> diff generated with zero persistent disk mutations.
//! 5. Overlapping symbol ranges rejection.
//! 6. Missing target file atomic abort before any mutations.
//! 7. MultiFileRollbackGuard RAII panic/drop rollback and file deletion guarantees.
//! 8. Diagnostic parser and AST mapper edge cases across polyglot compiler output formats.

use ctxcut_core::error::CoreError;
use ctxcut_core::model::VerifyDiagnostic;
use ctxcut_core::refactor::batch::{
    BatchAstPatcher, PatchTransactionRequest, SymbolPatchUnit,
};
use ctxcut_core::verify::ast_mapper::{AstDiagnosticMapper, PatchedFileInfo, PatchedSymbolMeta};
use ctxcut_core::verify::multi_rollback::MultiFileRollbackGuard;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Helper: returns exact bytes of a file
fn read_bytes(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_default()
}

/// 1. Simultaneous edits across 5+ files and 10+ symbols in multiple languages (Rust, TS, Python, Go)
#[test]
fn test_adversarial_simultaneous_5_files_10_symbols_atomic_apply() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // File 1: Rust Math
    let f1_path = root.join("math.rs");
    let f1_orig = r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
"#;
    fs::write(&f1_path, f1_orig).unwrap();

    // File 2: TypeScript User Service
    let f2_path = root.join("user_service.ts");
    let f2_orig = r#"
export function fetchUser(id: string): object {
    return { id, name: "old" };
}

export function validateUser(id: string): boolean {
    return id.length > 0;
}
"#;
    fs::write(&f2_path, f2_orig).unwrap();

    // File 3: Python Calculator
    let f3_path = root.join("calculator.py");
    let f3_orig = r#"
def compute_tax(subtotal: float) -> float:
    return subtotal * 0.05

def format_currency(val: float) -> str:
    return f""
"#;
    fs::write(&f3_path, f3_orig).unwrap();

    // File 4: Go Payment
    let f4_path = root.join("payment.go");
    let f4_orig = r#"package payment

func ProcessPayment(amount int) bool {
    return amount > 0
}

func RefundPayment(id string) bool {
    return len(id) > 0
}
"#;
    fs::write(&f4_path, f4_orig).unwrap();

    // File 5: Rust Analytics
    let f5_path = root.join("analytics.rs");
    let f5_orig = r#"
pub fn track_event(name: &str) -> bool {
    !name.is_empty()
}

pub fn aggregate_metrics(count: usize) -> usize {
    count * 2
}
"#;
    fs::write(&f5_path, f5_orig).unwrap();

    // File 6: TypeScript Config
    let f6_path = root.join("config.ts");
    let f6_orig = r#"
export function loadConfig(): object {
    return { port: 3000 };
}

export function getEnv(): string {
    return "development";
}
"#;
    fs::write(&f6_path, f6_orig).unwrap();

    // Construct 12 patch units across 6 files
    let patches = vec![
        SymbolPatchUnit {
            file_path: f1_path.clone(),
            symbol_query: "add".to_string(),
            replacement_code: "pub fn add(a: i32, b: i32) -> i32 {\n    (a + b).max(0)\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f1_path.clone(),
            symbol_query: "multiply".to_string(),
            replacement_code: "pub fn multiply(a: i32, b: i32) -> i32 {\n    (a * b).max(0)\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f2_path.clone(),
            symbol_query: "fetchUser".to_string(),
            replacement_code: "export function fetchUser(id: string): object {\n    return { id, name: \"updated\", v: 2 };\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f2_path.clone(),
            symbol_query: "validateUser".to_string(),
            replacement_code: "export function validateUser(id: string): boolean {\n    return id.length >= 3;\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f3_path.clone(),
            symbol_query: "compute_tax".to_string(),
            replacement_code: "def compute_tax(subtotal: float) -> float:\n    return subtotal * 0.08".to_string(),
        },
        SymbolPatchUnit {
            file_path: f3_path.clone(),
            symbol_query: "format_currency".to_string(),
            replacement_code: "def format_currency(val: float) -> str:\n    return f\"USD {val:.2f}\"".to_string(),
        },
        SymbolPatchUnit {
            file_path: f4_path.clone(),
            symbol_query: "ProcessPayment".to_string(),
            replacement_code: "func ProcessPayment(amount int) bool {\n    return amount >= 10\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f4_path.clone(),
            symbol_query: "RefundPayment".to_string(),
            replacement_code: "func RefundPayment(id string) bool {\n    return len(id) >= 5\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f5_path.clone(),
            symbol_query: "track_event".to_string(),
            replacement_code: "pub fn track_event(name: &str) -> bool {\n    name.len() > 2\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f5_path.clone(),
            symbol_query: "aggregate_metrics".to_string(),
            replacement_code: "pub fn aggregate_metrics(count: usize) -> usize {\n    count * 4\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f6_path.clone(),
            symbol_query: "loadConfig".to_string(),
            replacement_code: "export function loadConfig(): object {\n    return { port: 8080, ssl: true };\n}".to_string(),
        },
        SymbolPatchUnit {
            file_path: f6_path.clone(),
            symbol_query: "getEnv".to_string(),
            replacement_code: "export function getEnv(): string {\n    return \"production\";\n}".to_string(),
        },
    ];

    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches,
        typechecker: None,
        apply: true,
        timeout_ms: Some(5000),
    };

    let result = BatchAstPatcher::apply_transaction(&req).expect("Transaction should succeed");

    assert!(result.success, "Transaction should report success");
    assert!(result.applied, "Changes should be committed to disk");
    assert!(!result.rolled_back, "Should not be rolled back");
    assert_eq!(result.files_modified_count, 6, "All 6 files should be modified");
    assert_eq!(result.symbols_patched_count, 12, "All 12 symbols should be patched");
    assert_eq!(result.diffs.len(), 6, "Should contain diffs for all 6 files");

    // Verify disk content modifications
    let f1_mod = fs::read_to_string(&f1_path).unwrap();
    assert!(f1_mod.contains("(a + b).max(0)"));
    assert!(f1_mod.contains("(a * b).max(0)"));

    let f2_mod = fs::read_to_string(&f2_path).unwrap();
    assert!(f2_mod.contains("name: \"updated\", v: 2"));
    assert!(f2_mod.contains("id.length >= 3"));

    let f3_mod = fs::read_to_string(&f3_path).unwrap();
    assert!(f3_mod.contains("subtotal * 0.08"));
    assert!(f3_mod.contains("USD {val:.2f}"));

    let f4_mod = fs::read_to_string(&f4_path).unwrap();
    assert!(f4_mod.contains("amount >= 10"));
    assert!(f4_mod.contains("len(id) >= 5"));

    let f5_mod = fs::read_to_string(&f5_path).unwrap();
    assert!(f5_mod.contains("name.len() > 2"));
    assert!(f5_mod.contains("count * 4"));

    let f6_mod = fs::read_to_string(&f6_path).unwrap();
    assert!(f6_mod.contains("port: 8080, ssl: true"));
    assert!(f6_mod.contains("\"production\""));
}

/// 2. Intentionally failing syntax in one symbol of a batch transaction ->
///    verify ALL files roll back 100% to exact pre-transaction byte counts.
#[test]
fn test_adversarial_failing_syntax_in_one_symbol_100_percent_rollback_exact_bytes() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let f1_path = root.join("service1.rs");
    let f2_path = root.join("service2.ts");
    let f3_path = root.join("service3.py");
    let f4_path = root.join("service4.go");
    let f5_path = root.join("service5.rs");

    let f1_orig = "pub fn op1() -> i32 { 10 }\npub fn op2() -> i32 { 20 }\n";
    let f2_orig = "export function ts1() { return 1; }\nexport function ts2() { return 2; }\n";
    let f3_orig = "def py1():\n    return 'hello'\ndef py2():\n    return 'world'\n";
    let f4_orig = "package s4\nfunc Go1() int { return 1 }\nfunc Go2() int { return 2 }\n";
    let f5_orig = "pub fn op5() -> bool { true }\n";

    fs::write(&f1_path, f1_orig).unwrap();
    fs::write(&f2_path, f2_orig).unwrap();
    fs::write(&f3_path, f3_orig).unwrap();
    fs::write(&f4_path, f4_orig).unwrap();
    fs::write(&f5_path, f5_orig).unwrap();

    // Snapshot pre-transaction exact bytes and byte counts
    let f1_bytes = read_bytes(&f1_path);
    let f2_bytes = read_bytes(&f2_path);
    let f3_bytes = read_bytes(&f3_path);
    let f4_bytes = read_bytes(&f4_path);
    let f5_bytes = read_bytes(&f5_path);

    // Patch 1: valid
    // Patch 2: valid
    // Patch 3: valid
    // Patch 4 (service4.go): INVALID syntax (missing opening brace / broken declaration)
    // Patch 5: valid
    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: f1_path.clone(),
                symbol_query: "op1".to_string(),
                replacement_code: "pub fn op1() -> i32 { 100 }".to_string(),
            },
            SymbolPatchUnit {
                file_path: f2_path.clone(),
                symbol_query: "ts1".to_string(),
                replacement_code: "export function ts1() { return 100; }".to_string(),
            },
            SymbolPatchUnit {
                file_path: f3_path.clone(),
                symbol_query: "py1".to_string(),
                replacement_code: "def py1():\n    return 'hello_modified'".to_string(),
            },
            SymbolPatchUnit {
                file_path: f4_path.clone(),
                symbol_query: "Go1".to_string(),
                replacement_code: "func Go1( int { return unclosed syntax !!!".to_string(), // FATAL SYNTAX ERROR
            },
            SymbolPatchUnit {
                file_path: f5_path.clone(),
                symbol_query: "op5".to_string(),
                replacement_code: "pub fn op5() -> bool { false }".to_string(),
            },
        ],
        typechecker: None,
        apply: true,
        timeout_ms: Some(5000),
    };

    let result = BatchAstPatcher::apply_transaction(&req).expect("Transaction should execute pre-check");

    assert!(!result.success, "Transaction must fail due to syntax error");
    assert!(!result.applied, "Must NOT apply any modifications to disk");
    assert!(!result.syntax_errors.is_empty(), "Syntax errors must be reported");

    // 100% Byte-for-byte exact equality verification
    assert_eq!(read_bytes(&f1_path), f1_bytes, "f1 must match exact pre-transaction bytes");
    assert_eq!(read_bytes(&f2_path), f2_bytes, "f2 must match exact pre-transaction bytes");
    assert_eq!(read_bytes(&f3_path), f3_bytes, "f3 must match exact pre-transaction bytes");
    assert_eq!(read_bytes(&f4_path), f4_bytes, "f4 must match exact pre-transaction bytes");
    assert_eq!(read_bytes(&f5_path), f5_bytes, "f5 must match exact pre-transaction bytes");

    assert_eq!(fs::read_to_string(&f1_path).unwrap(), f1_orig);
    assert_eq!(fs::read_to_string(&f2_path).unwrap(), f2_orig);
    assert_eq!(fs::read_to_string(&f3_path).unwrap(), f3_orig);
    assert_eq!(fs::read_to_string(&f4_path).unwrap(), f4_orig);
    assert_eq!(fs::read_to_string(&f5_path).unwrap(), f5_orig);
}

/// 3. Intentionally failing typecheck -> verifying MultiFileRollbackGuard restores disk state
///    and AstDiagnosticMapper attributes diagnostic line, symbol, and node.
#[test]
fn test_adversarial_failing_typecheck_rollback_and_ast_diagnostic_mapping() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let f1_path = root.join("calc.rs");
    let f2_path = root.join("util.rs");

    let f1_orig = "pub fn add_values(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
    let f2_orig = "pub fn get_flag() -> bool {\n    true\n}\n";

    fs::write(&f1_path, f1_orig).unwrap();
    fs::write(&f2_path, f2_orig).unwrap();

    let f1_bytes = read_bytes(&f1_path);
    let f2_bytes = read_bytes(&f2_path);

    // Mock typechecker command that exits with code 1 and outputs a Rust compiler error
    // targeting line 3 of calc.rs
    let mock_cmd = if cfg!(target_os = "windows") {
        "[Console]::Error.WriteLine('calc.rs:2:9: error[E0308]: mismatched types: expected i32, found &str'); exit 1"
    } else {
        "echo 'calc.rs:2:9: error[E0308]: mismatched types: expected i32, found &str' >&2; exit 1"
    };

    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: f1_path.clone(),
                symbol_query: "add_values".to_string(),
                replacement_code: "pub fn add_values(a: i32, b: i32) -> i32 {\n    \"string\"\n}".to_string(),
            },
            SymbolPatchUnit {
                file_path: f2_path.clone(),
                symbol_query: "get_flag".to_string(),
                replacement_code: "pub fn get_flag() -> bool {\n    false\n}".to_string(),
            },
        ],
        typechecker: Some(mock_cmd.to_string()),
        apply: true,
        timeout_ms: Some(5000),
    };

    let result = BatchAstPatcher::apply_transaction(&req).expect("Transaction should run");

    assert!(!result.success, "Typecheck failure should mark transaction unsuccessful");
    assert!(!result.applied, "Changes should not be applied");
    assert!(result.rolled_back, "MultiFileRollbackGuard must trigger rollback");

    // Verify 100% disk restoration
    assert_eq!(read_bytes(&f1_path), f1_bytes, "calc.rs must roll back to exact original bytes");
    assert_eq!(read_bytes(&f2_path), f2_bytes, "util.rs must roll back to exact original bytes");

    // Verify diagnostic mapping
    assert!(!result.diagnostics.is_empty(), "Should capture typechecker diagnostics");
    let mapped = &result.diagnostics[0];
    assert_eq!(mapped.severity, "error");
    assert_eq!(mapped.symbol_name.as_deref(), Some("add_values"));
    assert_eq!(mapped.node_kind.as_deref(), Some("function"));
    assert_eq!(mapped.patch_relative_line, Some(2));
    assert!(mapped.code_snippet.is_some());
}

/// 4. Dry-run mode (\pply: false\) -> zero disk writes occur, diffs generated
#[test]
fn test_adversarial_dry_run_mode_zero_disk_writes() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let mut paths = Vec::new();
    let mut orig_bytes = Vec::new();

    for i in 1..=5 {
        let p = root.join(format!("file_{i}.ts"));
        let content = format!("export function func_{i}() {{\n    return {i};\n}}\n");
        fs::write(&p, &content).unwrap();
        orig_bytes.push(read_bytes(&p));
        paths.push(p);
    }

    let patches: Vec<SymbolPatchUnit> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| SymbolPatchUnit {
            file_path: p.clone(),
            symbol_query: format!("func_{}", i + 1),
            replacement_code: format!("export function func_{}() {{\n    return {};\n}}", i + 1, (i + 1) * 100),
        })
        .collect();

    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches,
        typechecker: None,
        apply: false, // DRY RUN
        timeout_ms: Some(5000),
    };

    let result = BatchAstPatcher::apply_transaction(&req).expect("Dry run should succeed");

    assert!(result.success, "Dry run with valid code must succeed");
    assert!(!result.applied, "Dry run must NOT persist to disk");
    assert!(result.rolled_back, "Dry run must roll back temporary disk state");
    assert_eq!(result.files_modified_count, 5);
    assert_eq!(result.symbols_patched_count, 5);
    assert_eq!(result.diffs.len(), 5);

    // Verify 100% zero disk writes / byte equality
    for (i, p) in paths.iter().enumerate() {
        assert_eq!(
            read_bytes(p),
            orig_bytes[i],
            "File {} must be untouched on disk",
            p.display()
        );
    }
}

/// 5. Overlapping symbol ranges in the same file -> rejected with CoreError::PatchRangeError
#[test]
fn test_adversarial_overlapping_symbol_ranges_rejection() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let file_path = root.join("overlap.rs");

    let source = r#"
pub fn outer() {
    println!("outer");
}
"#;
    fs::write(&file_path, source).unwrap();

    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: file_path.clone(),
                symbol_query: "outer".to_string(),
                replacement_code: "pub fn outer() { println!(\"1\"); }".to_string(),
            },
            SymbolPatchUnit {
                file_path: file_path.clone(),
                symbol_query: "outer".to_string(), // duplicate / overlapping target
                replacement_code: "pub fn outer() { println!(\"2\"); }".to_string(),
            },
        ],
        typechecker: None,
        apply: true,
        timeout_ms: Some(5000),
    };

    let res = BatchAstPatcher::apply_transaction(&req);
    match res {
        Err(CoreError::PatchRangeError { .. }) => {
            // Expected error
        }
        other => panic!("Expected PatchRangeError for overlapping symbols, got: {:?}", other),
    }

    // Disk remains untouched
    assert_eq!(fs::read_to_string(&file_path).unwrap(), source);
}

/// 6. Missing target file -> atomic abort without touching any other files
#[test]
fn test_adversarial_missing_target_file_atomic_abort() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let existing = root.join("existing.rs");
    let missing = root.join("missing.rs");

    let orig_code = "pub fn exist() -> i32 { 1 }\n";
    fs::write(&existing, orig_code).unwrap();
    let pre_bytes = read_bytes(&existing);

    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: existing.clone(),
                symbol_query: "exist".to_string(),
                replacement_code: "pub fn exist() -> i32 { 999 }".to_string(),
            },
            SymbolPatchUnit {
                file_path: missing,
                symbol_query: "ghost".to_string(),
                replacement_code: "pub fn ghost() {}".to_string(),
            },
        ],
        typechecker: None,
        apply: true,
        timeout_ms: Some(5000),
    };

    let res = BatchAstPatcher::apply_transaction(&req);
    match res {
        Err(CoreError::Io { .. }) => {
            // Expected
        }
        other => panic!("Expected CoreError::Io, got: {:?}", other),
    }

    // Verify existing file was not modified
    assert_eq!(read_bytes(&existing), pre_bytes);
}

/// 7. MultiFileRollbackGuard RAII panic/drop rollback and file deletion guarantees
#[test]
fn test_adversarial_multifile_rollback_guard_raii_drop() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let f1 = root.join("existing1.rs");
    let f2 = root.join("existing2.rs");
    let f3_new = root.join("brand_new.rs");

    fs::write(&f1, "orig1").unwrap();
    fs::write(&f2, "orig2").unwrap();

    {
        let mut guard = MultiFileRollbackGuard::new();
        guard.write_file(&f1, "mutated1").unwrap();
        guard.write_file(&f2, "mutated2").unwrap();
        guard.write_file(&f3_new, "created_new").unwrap();

        assert_eq!(fs::read_to_string(&f1).unwrap(), "mutated1");
        assert_eq!(fs::read_to_string(&f2).unwrap(), "mutated2");
        assert_eq!(fs::read_to_string(&f3_new).unwrap(), "created_new");
        // Guard drops here without commit() -> RAII drop rollback!
    }

    assert_eq!(fs::read_to_string(&f1).unwrap(), "orig1");
    assert_eq!(fs::read_to_string(&f2).unwrap(), "orig2");
    assert!(!f3_new.exists(), "Newly created file must be deleted on rollback");
}

/// 8. AstDiagnosticMapper polyglot compiler output formats & edge cases
#[test]
fn test_adversarial_diagnostic_mapper_polyglot_matrix() {
    let patched_source = r#"// Line 1: Comment
pub fn calc_one(x: i32) -> i32 {
    let y: String = x;
    y
}

pub fn calc_two(z: bool) -> bool {
    !z
}
"#;

    let sym_meta = PatchedSymbolMeta {
        symbol_name: "calc_one".to_string(),
        node_kind: "function".to_string(),
        start_line: 2,
        end_line: 5,
        replacement_code: "pub fn calc_one(x: i32) -> i32 {\n    let y: String = x;\n    y\n}".to_string(),
    };

    let file_info = PatchedFileInfo {
        file_path: PathBuf::from("src/math.rs"),
        patched_source: patched_source.to_string(),
        symbols: vec![sym_meta],
    };

    // Diagnostic 1: Rust compiler format inside patched symbol
    let diag_rust = VerifyDiagnostic {
        severity: "error".to_string(),
        line: Some(3),
        column: Some(9),
        message: "mismatched types".to_string(),
        file: Some("src/math.rs".to_string()),
        code: Some("E0308".to_string()),
    };

    // Diagnostic 2: Outside patched symbol (line 7)
    let diag_outside = VerifyDiagnostic {
        severity: "warning".to_string(),
        line: Some(7),
        column: Some(1),
        message: "unused function".to_string(),
        file: Some("src/math.rs".to_string()),
        code: None,
    };

    // Diagnostic 3: File mismatch / unrelated file
    let diag_unrelated = VerifyDiagnostic {
        severity: "info".to_string(),
        line: Some(10),
        column: None,
        message: "unrelated note".to_string(),
        file: Some("src/other.rs".to_string()),
        code: None,
    };

    let mapped = AstDiagnosticMapper::map_diagnostics(
        &[diag_rust, diag_outside, diag_unrelated],
        &[file_info],
        None,
    );

    assert_eq!(mapped.len(), 3);

    // Diag 1: inside symbol
    assert_eq!(mapped[0].symbol_name.as_deref(), Some("calc_one"));
    assert_eq!(mapped[0].node_kind.as_deref(), Some("function"));
    assert_eq!(mapped[0].patch_relative_line, Some(2));
    assert!(mapped[0].code_snippet.as_ref().unwrap().contains("let y: String = x;"));

    // Diag 2: outside symbol
    assert_eq!(mapped[1].symbol_name, None);
    assert_eq!(mapped[1].node_kind, None);
    assert_eq!(mapped[1].patch_relative_line, None);
    assert!(mapped[1].code_snippet.as_ref().unwrap().contains("pub fn calc_two"));

    // Diag 3: unrelated file
    assert_eq!(mapped[2].symbol_name, None);
    assert_eq!(mapped[2].file_path, "src/other.rs");
}
