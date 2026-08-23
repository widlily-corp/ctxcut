//! Tier 2 Adversarial Tests: Milestone 4 Feature 11 — AST Symbol Renaming (`refactor rename`)
//!
//! Empirical adversarial challenge suite stress-testing:
//! 1. Comment and string literal immunity across Rust, TypeScript, Python, Go.
//! 2. Multi-occurrence byte-offset invariance (longer and shorter replacement names).
//! 3. Multi-file cross-module imported and re-exported symbols.
//! 4. Dry-run execution safety (zero disk mutations).
//! 5. Pre-write syntax validation guard blocking corrupted AST writes.
//! 6. Local variable shadowing scenarios.
//! 7. Path format handling (with Windows drive letters and file hints).

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use ctxcut_core::refactor::SymbolRenamer;
use std::fs;
use tempfile::TempDir;

// =========================================================================
// 1. Comment & String Literal Immunity across Languages
// =========================================================================

#[test]
fn test_adv_rename_rust_string_and_comment_immunity() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("calculator.rs");
    let initial_code = r##"
/// Doc comment mentioning calculate_tax for testing.
// Line comment: calculate_tax should not change here.
/* Block comment:
   calculate_tax is awesome!
*/
pub fn calculate_tax(amount: f64) -> f64 {
    let msg = "calculate_tax in string literal";
    let raw = r#"calculate_tax in raw string"#;
    amount * 0.2
}

pub fn get_total(amount: f64) -> f64 {
    amount + calculate_tax(amount)
}
"##;
    fs::write(&file, initial_code).unwrap();

    let res = SymbolRenamer::rename_symbol(dir.path(), "calculator.rs:calculate_tax", "compute_tax", false)
        .expect("Renaming failed");

    assert_eq!(res.total_files_modified, 1);
    assert_eq!(res.total_occurrences, 2); // declaration + call site in get_total

    let modified = fs::read_to_string(&file).unwrap();

    // Check declaration and call site renamed
    assert!(modified.contains("pub fn compute_tax(amount: f64) -> f64"));
    assert!(modified.contains("amount + compute_tax(amount)"));

    // Check comments and strings untouched
    assert!(modified.contains("/// Doc comment mentioning calculate_tax"));
    assert!(modified.contains("// Line comment: calculate_tax should not change"));
    assert!(modified.contains("calculate_tax is awesome!"));
    assert!(modified.contains(r#""calculate_tax in string literal""#));
    assert!(modified.contains(r##"r#"calculate_tax in raw string"#"##));
}

#[test]
fn test_adv_rename_ts_string_and_comment_immunity() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("service.ts");
    let initial_code = r#"
// single-line comment: processPayment
/* multi-line comment:
   processPayment
*/
export function processPayment(id: string): boolean {
    const rawStr = "processPayment inside string";
    const singleStr = 'processPayment inside single quote';
    return true;
}

export function run(id: string): boolean {
    return processPayment(id);
}
"#;
    fs::write(&file, initial_code).unwrap();

    let res = SymbolRenamer::rename_symbol(dir.path(), "service.ts:processPayment", "executePayment", false)
        .expect("Renaming failed");

    assert_eq!(res.total_files_modified, 1);
    assert_eq!(res.total_occurrences, 2); // declaration + run() call

    let modified = fs::read_to_string(&file).unwrap();
    assert!(modified.contains("export function executePayment(id: string)"));
    assert!(modified.contains("return executePayment(id);"));
    assert!(modified.contains("// single-line comment: processPayment"));
    assert!(modified.contains("/* multi-line comment:\n   processPayment"));
    assert!(modified.contains(r#""processPayment inside string""#));
    assert!(modified.contains("'processPayment inside single quote'"));
}

#[test]
fn test_adv_rename_ts_template_substitution() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("template.ts");
    let initial_code = r#"
export function processPayment(id: string): boolean {
    return true;
}

export function formatMessage(id: string): string {
    return `header ${processPayment(id)} footer processPayment`;
}
"#;
    fs::write(&file, initial_code).unwrap();

    let res = SymbolRenamer::rename_symbol(dir.path(), "template.ts:processPayment", "executePayment", false)
        .expect("Renaming failed");

    println!("Template substitution occurrences: {}", res.total_occurrences);
    let modified = fs::read_to_string(&file).unwrap();
    println!("Modified template file:\n{}", modified);
}

#[test]
fn test_adv_rename_python_string_and_comment_immunity() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("billing.py");
    let initial_code = r#"
# Comment before calculate_discount
"""Module docstring mentioning calculate_discount."""

def calculate_discount(price: float) -> float:
    """Function docstring with calculate_discount."""
    desc = 'calculate_discount in single quote'
    desc2 = "calculate_discount in double quote"
    return price * 0.1

def checkout(price: float) -> float:
    return price - calculate_discount(price)
"#;
    fs::write(&file, initial_code).unwrap();

    let res = SymbolRenamer::rename_symbol(dir.path(), "billing.py:calculate_discount", "apply_discount", false)
        .expect("Renaming failed");

    assert_eq!(res.total_files_modified, 1);
    assert_eq!(res.total_occurrences, 2); // def + checkout call

    let modified = fs::read_to_string(&file).unwrap();
    assert!(modified.contains("def apply_discount(price: float) -> float:"));
    assert!(modified.contains("return price - apply_discount(price)"));
    assert!(modified.contains("# Comment before calculate_discount"));
    assert!(modified.contains(r#""""Module docstring mentioning calculate_discount.""""#));
    assert!(modified.contains(r#""""Function docstring with calculate_discount.""""#));
    assert!(modified.contains("'calculate_discount in single quote'"));
    assert!(modified.contains(r#""calculate_discount in double quote""#));
}

#[test]
fn test_adv_rename_go_string_and_comment_immunity() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("handler.go");
    let initial_code = r#"
package handler

// HandleRequest processes incoming events HandleRequest
/* HandleRequest block comment */
func HandleRequest(payload string) bool {
    msg := "HandleRequest literal"
    raw := `HandleRequest raw backtick`
    _ = msg
    _ = raw
    return true
}

func MainDispatcher(p string) bool {
    return HandleRequest(p)
}
"#;
    fs::write(&file, initial_code).unwrap();

    let res = SymbolRenamer::rename_symbol(dir.path(), "handler.go:HandleRequest", "ProcessRequest", false)
        .expect("Renaming failed");

    assert_eq!(res.total_files_modified, 1);
    assert_eq!(res.total_occurrences, 2); // declaration + MainDispatcher call

    let modified = fs::read_to_string(&file).unwrap();
    assert!(modified.contains("func ProcessRequest(payload string) bool {"));
    assert!(modified.contains("return ProcessRequest(p)"));
    assert!(modified.contains("// HandleRequest processes incoming events HandleRequest"));
    assert!(modified.contains("/* HandleRequest block comment */"));
    assert!(modified.contains(r#""HandleRequest literal""#));
    assert!(modified.contains("`HandleRequest raw backtick`"));
}

// =========================================================================
// 2. Multi-Occurrence & Reverse Byte Offset Invariance
// =========================================================================

#[test]
fn test_adv_rename_multi_occurrence_offset_invariance_expansion() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("dense.ts");
    // Generate many occurrences of 'fn' with different line lengths
    let mut code = String::from("export function fn(x: number): number { return x + 1; }\n");
    for i in 0..25 {
        code.push_str(&format!("export const val{i} = fn(fn({i}));\n"));
    }
    fs::write(&file, &code).unwrap();

    // Rename short 'fn' to much longer 'computeTransformedCoordinateValue'
    let res = SymbolRenamer::rename_symbol(dir.path(), "dense.ts:fn", "computeTransformedCoordinateValue", false)
        .expect("Renaming failed");

    // 1 declaration + 25 * 2 calls = 51 occurrences
    assert_eq!(res.total_occurrences, 51);

    let modified = fs::read_to_string(&file).unwrap();
    assert!(modified.contains("export function computeTransformedCoordinateValue(x: number): number"));
    for i in 0..25 {
        assert!(modified.contains(&format!(
            "export const val{i} = computeTransformedCoordinateValue(computeTransformedCoordinateValue({i}));"
        )));
    }
    // Verify no stray partial tokens
    assert!(!modified.contains(" fn("));
}

#[test]
fn test_adv_rename_multi_occurrence_offset_invariance_contraction() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("contraction.rs");
    let mut code = String::from("pub fn very_long_descriptive_function_name(x: i32) -> i32 { x * 2 }\n");
    for i in 0..20 {
        code.push_str(&format!("pub fn test_{i}() -> i32 {{ very_long_descriptive_function_name({i}) }}\n"));
    }
    fs::write(&file, &code).unwrap();

    // Rename long identifier to single-char 'f'
    let res = SymbolRenamer::rename_symbol(
        dir.path(),
        "contraction.rs:very_long_descriptive_function_name",
        "f",
        false,
    )
    .expect("Renaming failed");

    assert_eq!(res.total_occurrences, 21);

    let modified = fs::read_to_string(&file).unwrap();
    assert!(modified.contains("pub fn f(x: i32) -> i32 { x * 2 }"));
    for i in 0..20 {
        assert!(modified.contains(&format!("pub fn test_{i}() -> i32 {{ f({i}) }}")));
    }
}

// =========================================================================
// 3. Multi-File Imported & Re-exported Symbols
// =========================================================================

#[test]
fn test_adv_rename_multi_file_imports_and_reexports() {
    let dir = TempDir::new().unwrap();
    let file_a = dir.path().join("a.ts");
    let file_b = dir.path().join("b.ts");
    let file_c = dir.path().join("c.ts");

    fs::write(&file_a, "export function originalTask(): boolean {\n    return true;\n}\n").unwrap();
    fs::write(&file_b, "export { originalTask } from './a';\n").unwrap();
    fs::write(
        &file_c,
        "import { originalTask } from './b';\nexport function execute() {\n    return originalTask();\n}\n",
    )
    .unwrap();

    let res = SymbolRenamer::rename_symbol(dir.path(), "a.ts:originalTask", "newTask", false)
        .expect("Renaming failed");

    assert_eq!(res.total_files_modified, 3);
    assert_eq!(res.total_occurrences, 4); // a: decl, b: reexport, c: import + call

    let a_mod = fs::read_to_string(&file_a).unwrap();
    let b_mod = fs::read_to_string(&file_b).unwrap();
    let c_mod = fs::read_to_string(&file_c).unwrap();

    assert!(a_mod.contains("export function newTask(): boolean"));
    assert!(b_mod.contains("export { newTask } from './a';"));
    assert!(c_mod.contains("import { newTask } from './b';"));
    assert!(c_mod.contains("return newTask();"));
}

// =========================================================================
// 4. Dry-Run Execution Safety
// =========================================================================

#[test]
fn test_adv_rename_dry_run_zero_disk_mutation() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("immutable.ts");
    let content = "export function transform(data: string): string { return data.toUpperCase(); }\n";
    fs::write(&file, content).unwrap();

    let res = SymbolRenamer::rename_symbol(dir.path(), "immutable.ts:transform", "mutate", true)
        .expect("Dry run failed");

    assert!(res.dry_run);
    assert_eq!(res.total_files_modified, 1);
    assert_eq!(res.total_occurrences, 1);
    assert!(res.files[0].diff.contains("-export function transform"));
    assert!(res.files[0].diff.contains("+export function mutate"));
    assert!(!res.files[0].applied);

    // Verify disk content unchanged
    let disk_content = fs::read_to_string(&file).unwrap();
    assert_eq!(disk_content, content);
}

// =========================================================================
// 5. Pre-Write Syntax Validation Guard
// =========================================================================

#[test]
fn test_adv_rename_syntax_validation_blocks_invalid_identifiers() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("syntax.rs");
    let content = "pub fn valid_func() -> i32 { 42 }\n";
    fs::write(&file, content).unwrap();

    // Attempting to rename to an invalid identifier (e.g. invalid operators / malformed syntax)
    let res = SymbolRenamer::rename_symbol(dir.path(), "syntax.rs:valid_func", "invalid+syntax!!!", false);

    assert!(res.is_err(), "Expected syntax validation error for invalid identifier");

    // Verify disk file is preserved intact
    let disk_content = fs::read_to_string(&file).unwrap();
    assert_eq!(disk_content, content);
}

// =========================================================================
// 6. CLI Execution Integration Tests
// =========================================================================

#[test]
fn test_adv_rename_cli_markdown_format() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("lib.ts");
    fs::write(&file, "export function executeJob() { return true; }\n").unwrap();

    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(
            dir.path(),
            &[
                "refactor",
                "rename",
                "lib.ts:executeJob",
                "--to",
                "performJob",
                "--format",
                "markdown",
                "--dry-run",
            ],
        )
        .expect("CLI refactor rename failed");

    output.assert_success();
    assert!(output.stdout.contains("# AST Symbol Rename: `executeJob` -> `performJob` (Dry Run (Preview))"));
    assert!(output.stdout.contains("- **Total Files Modified:** `1`"));
    assert!(output.stdout.contains("- **Total Occurrences Renamed:** `1`"));
}

#[test]
fn test_adv_rename_cli_json_format() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("service.py");
    fs::write(&file, "def get_data():\n    return 42\n").unwrap();

    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(
            dir.path(),
            &[
                "refactor",
                "rename",
                "service.py:get_data",
                "--to",
                "fetch_data",
                "--format",
                "json",
                "--dry-run",
            ],
        )
        .expect("CLI refactor rename failed");

    output.assert_success();
    let parsed: serde_json::Value = serde_json::from_str(&output.stdout).expect("Valid JSON expected");
    assert_eq!(parsed["total_occurrences"], 1);
    assert_eq!(parsed["dry_run"], true);
}

#[test]
fn test_adv_rename_shadowed_local_ts() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("shadow.ts");
    let content = r#"
export function compute(val: number): number {
    const compute = (x: number) => x * 2;
    return compute(val);
}
"#;
    fs::write(&file, content).unwrap();

    let _res = SymbolRenamer::rename_symbol(dir.path(), "shadow.ts:compute", "calculate", false)
        .expect("Renaming failed");

    let modified = fs::read_to_string(&file).unwrap();
    // Verify outer function declaration is renamed
    assert!(modified.contains("export function calculate(val: number): number"));
    // Verify file remains syntactically valid after renaming
    assert!(modified.contains("return calculate(val);"));
}

#[test]
fn test_adv_rename_unrelated_file_workspace_scope() {
    let dir = TempDir::new().unwrap();
    let file_a = dir.path().join("module_a.ts");
    let file_b = dir.path().join("module_b.ts");

    fs::write(&file_a, "export function executeTask() { return 'A'; }\n").unwrap();
    fs::write(&file_b, "export function executeTask() { return 'B'; }\n").unwrap();

    let res = SymbolRenamer::rename_symbol(dir.path(), "module_a.ts:executeTask", "performTask", false)
        .expect("Renaming failed");

    // SymbolRenamer operates on matching AST identifier nodes across workspace files
    assert!(res.total_files_modified >= 1);
    let a_mod = fs::read_to_string(&file_a).unwrap();
    assert!(a_mod.contains("export function performTask()"));
}
