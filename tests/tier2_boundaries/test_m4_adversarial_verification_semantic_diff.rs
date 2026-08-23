//! Adversarial Test Suite for Milestone 4: Verification Guard (`verify-patch`) & Semantic AST Diff (`semantic-diff`).
//!
//! Empirical stress testing covering:
//! - Verification Guard (Feature 9):
//!   1. Malformed syntax pre-validation in replacement code (Rust, TS, Python, Go)
//!   2. Typechecker failure triggering automatic RAII rollback (disk byte-for-byte preservation)
//!   3. Dry-run mode leaving zero modified bytes on disk even on successful typechecking
//!   4. Successful patch apply with atomic commit when dry_run=false
//!   5. Non-existent file and non-existent symbol error handling
//!   6. Process timeout handling and rollback safety
//!   7. MCP `verify_patch` tool interface, diagnostics, and parameter error handling
//! - Semantic AST Diff (Feature 10):
//!   1. Pure whitespace and formatting deltas
//!   2. Comment-only and docstring modifications (`DocstringModified`)
//!   3. Newly added symbols (`Added`) and deleted symbols (`Removed`)
//!   4. Breaking signature changes (`SignatureChanged`) with old/new signature extraction
//!   5. Type/struct/interface definition deltas (`TypeDefinitionChanged`)
//!   6. Top-level import statement deltas (`Added`/`Removed`)
//!   7. Token ROI calculations and multi-tier cost savings under strict token budgets
//!   8. MCP `semantic_diff` tool interface and JSON/Markdown serialization formats

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};
use ctxcut_core::diff::semantic::{
    FileSemanticDiff, ImportChangeKind, SemanticDiffEngine, SymbolChangeKind,
};
use ctxcut_core::model::{SupportedLanguage, VerifyPatchResult};
use ctxcut_core::verify::rollback::RollbackGuard;
use ctxcut_core::verify::typechecker::{DiagnosticParser, TypecheckerRunner};
use ctxcut_core::verify::{PatchVerifier, VerifyPatchOptions};
use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

// =========================================================================
// 1. FEATURE 9: VERIFICATION GUARD ADVERSARIAL TESTS
// =========================================================================

#[test]
fn test_m4_adv_f9_syntax_error_rejection_polyglot() {
    let dir = TempDir::new().expect("TempDir failed");

    // 1. Rust syntax error (unclosed brace, missing type)
    let rs_file = dir.path().join("service.rs");
    fs::write(&rs_file, "pub fn calculate_total(price: f64, tax: f64) -> f64 {\n    price + tax\n}\n").unwrap();

    let bad_rs_patch = "pub fn calculate_total(price: f64, tax: f64) -> f64 {\n    price + // unclosed syntax\n";
    let res_rs = PatchVerifier::verify_patch(
        dir.path(),
        &format!("{}:calculate_total", rs_file.display()),
        bad_rs_patch,
        None,
        true,
    ).expect("verify_patch should return Result with failed verification");

    assert!(!res_rs.success, "Rust syntax error should result in success=false");
    assert!(!res_rs.applied, "Rust syntax error must never be applied");
    assert!(!res_rs.diagnostics.is_empty() || !res_rs.syntax_errors.is_empty());
    assert_eq!(
        fs::read_to_string(&rs_file).unwrap(),
        "pub fn calculate_total(price: f64, tax: f64) -> f64 {\n    price + tax\n}\n",
        "Original Rust file on disk must remain pristine"
    );

    // 2. TypeScript syntax error (malformed function signature)
    let ts_file = dir.path().join("auth.ts");
    fs::write(&ts_file, "export function authenticate(token: string): boolean {\n    return token.length > 0;\n}\n").unwrap();

    let bad_ts_patch = "export function authenticate(token: string): boolean {\n    return token.length > 0\n// unclosed brace";
    let res_ts = PatchVerifier::verify_patch(
        dir.path(),
        &format!("{}:authenticate", ts_file.display()),
        bad_ts_patch,
        None,
        true,
    ).expect("verify_patch should handle TS syntax error gracefully");

    assert!(!res_ts.success, "TypeScript syntax error should result in success=false");
    assert_eq!(
        fs::read_to_string(&ts_file).unwrap(),
        "export function authenticate(token: string): boolean {\n    return token.length > 0;\n}\n",
        "Original TS file on disk must remain pristine"
    );

    // 3. Python syntax error (invalid token / broken def)
    let py_file = dir.path().join("handler.py");
    fs::write(&py_file, "def get_status(code: int) -> str:\n    return f'Status: {code}'\n").unwrap();

    let bad_py_patch = "def get_status(code: int) -> str:\n    return for in while\n";
    let res_py = PatchVerifier::verify_patch(
        dir.path(),
        &format!("{}:get_status", py_file.display()),
        bad_py_patch,
        None,
        true,
    ).expect("verify_patch should handle Python syntax error gracefully");

    assert!(!res_py.success, "Python syntax error should result in success=false");
    assert_eq!(
        fs::read_to_string(&py_file).unwrap(),
        "def get_status(code: int) -> str:\n    return f'Status: {code}'\n"
    );
}

#[test]
fn test_m4_adv_f9_typechecker_failure_triggers_automatic_rollback() {
    let dir = TempDir::new().expect("TempDir failed");
    let file = dir.path().join("calc.ts");
    let original_content = "export function add(a: number, b: number): number {\n    return a + b;\n}\n";
    fs::write(&file, original_content).unwrap();

    // Splicing replacement that has valid AST syntax but fails typechecking
    let replacement = "export function add(a: number, b: number): number {\n    return (a + b) * 2;\n}\n";

    // Simulate failing typechecker command that exits with code 1 and writes diagnostic
    let fail_cmd = if cfg!(target_os = "windows") {
        "powershell -NoProfile -NonInteractive -Command \"Write-Error 'Type mismatch at calc.ts(2,5)'; exit 1\""
    } else {
        "sh -c 'echo \"calc.ts:2:5: error: Type mismatch\" >&2; exit 1'"
    };

    let res = PatchVerifier::verify_patch(
        dir.path(),
        &format!("{}:add", file.display()),
        replacement,
        Some(fail_cmd),
        false, // apply mode attempted
    ).expect("Execution should return result with failure");

    assert!(!res.success, "Typechecker failure must flag success=false");
    assert!(!res.applied, "Typechecker failure must never apply changes");
    assert_eq!(res.exit_code, Some(1));

    // Byte-for-byte verify rollback on disk
    let disk_content = fs::read_to_string(&file).unwrap();
    assert_eq!(
        disk_content, original_content,
        "Disk file MUST be completely rolled back to exact original content byte-for-byte"
    );
}

#[test]
fn test_m4_adv_f9_dry_run_zero_byte_modification_on_disk() {
    let dir = TempDir::new().expect("TempDir failed");
    let file = dir.path().join("logic.rs");
    let original = "pub fn is_valid(flag: bool) -> bool {\n    flag\n}\n";
    fs::write(&file, original).unwrap();

    let valid_replacement = "pub fn is_valid(flag: bool) -> bool {\n    !flag\n}\n";

    // Passing typechecker command (exit 0)
    let pass_cmd = if cfg!(target_os = "windows") {
        "powershell -NoProfile -NonInteractive -Command \"exit 0\""
    } else {
        "sh -c 'exit 0'"
    };

    let res = PatchVerifier::verify_patch(
        dir.path(),
        &format!("{}:is_valid", file.display()),
        valid_replacement,
        Some(pass_cmd),
        true, // dry_run=true
    ).expect("verify_patch failed");

    assert!(res.success, "Dry run with passing typecheck should succeed");
    assert!(!res.applied, "Dry run must have applied=false");
    assert!(res.dry_run, "dry_run field must be true");
    assert!(res.diff.contains("+    !flag"));

    // Verify disk content has EXACTLY zero modified bytes
    let disk_content = fs::read_to_string(&file).unwrap();
    assert_eq!(disk_content, original, "Dry-run mode must leave zero modified bytes on disk");
}

#[test]
fn test_m4_adv_f9_apply_mode_atomic_commit() {
    let dir = TempDir::new().expect("TempDir failed");
    let file = dir.path().join("state.ts");
    let original = "export function getState(): string {\n    return 'initial';\n}\n";
    fs::write(&file, original).unwrap();

    let replacement = "export function getState(): string {\n    return 'updated';\n}\n";

    let pass_cmd = if cfg!(target_os = "windows") {
        "powershell -NoProfile -NonInteractive -Command \"exit 0\""
    } else {
        "sh -c 'exit 0'"
    };

    let res = PatchVerifier::verify_patch(
        dir.path(),
        &format!("{}:getState", file.display()),
        replacement,
        Some(pass_cmd),
        false, // dry_run=false -> apply
    ).expect("verify_patch failed");

    assert!(res.success);
    assert!(res.applied, "When dry_run=false and check passes, applied must be true");

    let disk_content = fs::read_to_string(&file).unwrap();
    assert!(disk_content.contains("return 'updated';"));
}

#[test]
fn test_m4_adv_f9_nonexistent_file_and_symbol_handling() {
    let dir = TempDir::new().expect("TempDir failed");

    // 1. Non-existent file
    let bad_file = dir.path().join("non_existent_file.rs");
    let res_missing_file = PatchVerifier::verify_patch(
        dir.path(),
        &format!("{}:someSymbol", bad_file.display()),
        "pub fn someSymbol() {}",
        None,
        true,
    );
    assert!(res_missing_file.is_err(), "Non-existent file must return Err");

    // 2. Non-existent symbol in existing file
    let real_file = dir.path().join("real.ts");
    fs::write(&real_file, "export function existingFunc() { return 1; }\n").unwrap();
    let res_missing_sym = PatchVerifier::verify_patch(
        dir.path(),
        &format!("{}:nonExistentFunc", real_file.display()),
        "export function nonExistentFunc() { return 2; }",
        None,
        true,
    );
    assert!(res_missing_sym.is_err(), "Non-existent symbol must return Err(SymbolNotFound)");
}

#[test]
fn test_m4_adv_f9_process_timeout_and_raii_drop_revert() {
    let dir = TempDir::new().expect("TempDir failed");
    let file = dir.path().join("timeout_target.rs");
    let original = "pub fn quick_fn() -> i32 { 100 }\n";
    fs::write(&file, original).unwrap();

    let replacement = "pub fn quick_fn() -> i32 { 200 }\n";

    // Hang command (sleeps for 10 seconds)
    let hang_cmd = if cfg!(target_os = "windows") {
        "powershell -NoProfile -NonInteractive -Command \"Start-Sleep -Seconds 10\""
    } else {
        "sleep 10"
    };

    let opts = VerifyPatchOptions {
        workspace_root: dir.path(),
        file_path: &file,
        symbol: "quick_fn",
        replacement_code: replacement,
        typechecker: Some(hang_cmd),
        dry_run: false,
        timeout_ms: Some(200), // Short 200ms timeout
    };

    let res = PatchVerifier::verify_patch_with_options(&opts)
        .expect("verify_patch_with_options should catch timeout gracefully");

    assert!(!res.success, "Timeout must mark verification as unsuccessful");
    assert!(!res.applied, "Timeout must not apply changes");
    assert!(res.stderr.contains("timed out") || res.exit_code == Some(124));

    // RAII guard reverted disk file
    let disk_content = fs::read_to_string(&file).unwrap();
    assert_eq!(disk_content, original, "File must be reverted after typechecker timeout");
}

#[test]
fn test_m4_adv_f9_mcp_verify_patch_tool_execution() {
    let dir = TempDir::new().expect("TempDir failed");
    let file = dir.path().join("mcp_target.ts");
    fs::write(&file, "export function greet(name: string) { return 'Hello ' + name; }\n").unwrap();

    // 1. Successful dry-run via MCP
    let args = json!({
        "target": format!("{}:greet", file.display()),
        "new_code": "export function greet(name: string) { return 'Hi ' + name; }",
        "dry_run": true,
        "typechecker": if cfg!(target_os = "windows") { "powershell -Command \"exit 0\"" } else { "true" }
    });

    let (resp, _metrics, err, _tokens_saved) = execute_tool_with_timeout("verify_patch", &args, 5000);
    assert!(err.is_none());
    assert_eq!(resp.get("isError"), None);
    let verify_res = resp.get("verify_result").expect("verify_result key missing in MCP output");
    assert_eq!(verify_res.get("success"), Some(&json!(true)));
    assert_eq!(verify_res.get("applied"), Some(&json!(false)));

    // 2. Missing parameter error
    let bad_args = json!({ "target": "missing_code.ts:fn" });
    let (resp_err, _, err_msg, _) = execute_tool_with_timeout("verify_patch", &bad_args, 5000);
    assert_eq!(resp_err.get("isError"), Some(&json!(true)));
    assert!(err_msg.is_some());
}

// =========================================================================
// 2. FEATURE 10: SEMANTIC AST DIFF ADVERSARIAL TESTS
// =========================================================================

#[test]
fn test_m4_adv_f10_whitespace_and_comment_only_modifications() {
    let file_path = Path::new("src/demo.ts");

    // 1. Whitespace only modification
    let old_src = "export function compute(a: number, b: number): number {\n    return a + b;\n}\n";
    let new_ws_src = "export function compute(a: number, b: number): number {\n\n    return    a + b  ;\n}\n";

    let diff_ws = SemanticDiffEngine::diff_sources(old_src, new_ws_src, file_path, None).unwrap();
    assert_eq!(diff_ws.modified_symbols.len(), 1);
    let mod_sym = &diff_ws.modified_symbols[0];
    assert_eq!(mod_sym.name, "compute");
    assert_eq!(mod_sym.change_kind, SymbolChangeKind::BodyModified);

    // 2. Comment-only modification
    let old_comment_src = "export function calculate(): number {\n    return 42;\n}\n";
    let new_comment_src = "export function calculate(): number {\n    // Updated detailed comment\n    /* multi-line note */\n    return 42;\n}\n";

    let diff_comment = SemanticDiffEngine::diff_sources(old_comment_src, new_comment_src, file_path, None).unwrap();
    assert_eq!(diff_comment.modified_symbols.len(), 1);
    let comment_mod = &diff_comment.modified_symbols[0];
    assert_eq!(comment_mod.name, "calculate");
    assert_eq!(comment_mod.change_kind, SymbolChangeKind::DocstringModified);
}

#[test]
fn test_m4_adv_f10_added_and_removed_symbols_and_types() {
    let file_path = Path::new("src/models.rs");

    let old_src = r#"
pub struct UserAccount {
    pub id: u64,
    pub username: String,
}

pub fn delete_user(id: u64) -> bool {
    id > 0
}
"#;

    let new_src = r#"
pub struct UserAccount {
    pub id: u64,
    pub username: String,
    pub email: String,
}

pub fn create_user(name: String) -> u64 {
    42
}
"#;

    let diff = SemanticDiffEngine::diff_sources(old_src, new_src, file_path, None).unwrap();

    // Check added symbols
    let added_names: Vec<&str> = diff.added_symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(added_names.contains(&"create_user"));

    // Check removed symbols
    let removed_names: Vec<&str> = diff.removed_symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(removed_names.contains(&"delete_user"));

    // Check type definition modification
    let modified_user = diff.modified_symbols.iter().find(|s| s.name == "UserAccount").unwrap();
    assert_eq!(modified_user.change_kind, SymbolChangeKind::TypeDefinitionChanged);
}

#[test]
fn test_m4_adv_f10_breaking_signature_change_detection() {
    let file_path = Path::new("src/api.ts");

    let old_src = "export function sendNotification(userId: string, msg: string): Promise<boolean> {\n    return api.post(userId, msg);\n}\n";
    let new_src = "export function sendNotification(userId: string, msg: string, priority: number = 1, options?: object): Promise<{ sent: boolean, id: string }> {\n    return api.post(userId, msg, priority);\n}\n";

    let diff = SemanticDiffEngine::diff_sources(old_src, new_src, file_path, None).unwrap();
    assert_eq!(diff.modified_symbols.len(), 1);

    let item = &diff.modified_symbols[0];
    match &item.change_kind {
        SymbolChangeKind::SignatureChanged {
            old_signature,
            new_signature,
            description,
        } => {
            assert!(old_signature.contains("sendNotification(userId: string, msg: string)"));
            assert!(new_signature.contains("priority: number"));
            assert!(description.contains("Signature parameters"));
        }
        _ => panic!("Expected SignatureChanged but got {:?}", item.change_kind),
    }
}

#[test]
fn test_m4_adv_f10_import_statement_modifications() {
    let file_path = Path::new("src/index.ts");

    let old_src = r#"
import { oldHelper } from './legacy';
import { commonUtil } from './util';

export function run() {
    return commonUtil();
}
"#;

    let new_src = r#"
import { newHelper, shinyUtil } from './modern';
import { commonUtil } from './util';

export function run() {
    return commonUtil() + shinyUtil();
}
"#;

    let diff = SemanticDiffEngine::diff_sources(old_src, new_src, file_path, None).unwrap();
    assert!(!diff.import_changes.is_empty());

    let added_imports: Vec<&str> = diff.import_changes.iter()
        .filter(|i| i.kind == ImportChangeKind::Added)
        .map(|i| i.statement.as_str())
        .collect();
    let removed_imports: Vec<&str> = diff.import_changes.iter()
        .filter(|i| i.kind == ImportChangeKind::Removed)
        .map(|i| i.statement.as_str())
        .collect();

    assert!(added_imports.iter().any(|s| s.contains("shinyUtil")));
    assert!(removed_imports.iter().any(|s| s.contains("oldHelper")));
}

#[test]
fn test_m4_adv_f10_token_roi_savings_under_strict_budget() {
    let sandbox = GitSandbox::new().expect("Failed sandbox");
    let mut large_content = String::new();

    for i in 0..100 {
        large_content.push_str(&format!(
            "export function helperFunction_{i}(param: number): number {{\n    const intermediate = param * {i};\n    return intermediate + 10;\n}}\n\n"
        ));
    }
    large_content.push_str("export function targetOperation(x: number): number {\n    return x * 10;\n}\n");

    sandbox.write_file("src/large_service.ts", &large_content).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    // Modify ONLY targetOperation signature and body
    let mut modified_content = String::new();
    for i in 0..100 {
        modified_content.push_str(&format!(
            "export function helperFunction_{i}(param: number): number {{\n    const intermediate = param * {i};\n    return intermediate + 10;\n}}\n\n"
        ));
    }
    modified_content.push_str("export function targetOperation(x: number, multiplier: number = 2): number {\n    return x * multiplier * 10;\n}\n");

    sandbox.modify_file("src/large_service.ts", &modified_content).unwrap();

    // Test with strict budget (50 tokens)
    let diff_budget_50 = SemanticDiffEngine::compute_diff(
        sandbox.path(),
        false,
        Some(Path::new("src/large_service.ts")),
        Some(50),
    ).expect("compute_diff failed");

    assert_eq!(diff_budget_50.files.len(), 1);
    let file_diff = &diff_budget_50.files[0];
    assert_eq!(file_diff.modified_symbols.len(), 1);
    assert!(diff_budget_50.roi.raw_tokens > 1000);
    assert!(diff_budget_50.roi.semantic_diff_tokens < diff_budget_50.roi.raw_tokens);
    assert!(diff_budget_50.roi.tokens_saved > 500);
    assert!(diff_budget_50.roi.savings_percentage > 50.0);
    assert!(diff_budget_50.roi.tier_savings.standard_sonnet_gpt4o > 0.0);
    assert!(diff_budget_50.roi.tier_savings.frontier_opus > diff_budget_50.roi.tier_savings.standard_sonnet_gpt4o);

    // Verify Markdown format contains ROI headers
    let md = diff_budget_50.to_markdown();
    assert!(md.contains("# Semantic AST Diff"));
    assert!(md.contains("Token Reduction:"));
    assert!(md.contains("Signature Changed"));
    assert!(md.contains("targetOperation"));

    // Verify JSON format serializes cleanly
    let json_str = diff_budget_50.to_json();
    let parsed_json: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");
    assert_eq!(parsed_json["total_modified_symbols"], 1);
    assert_eq!(parsed_json["total_signature_changes"], 1);
}

#[test]
fn test_m4_adv_f10_mcp_semantic_diff_tool_execution() {
    let sandbox = GitSandbox::new().expect("Failed sandbox");
    sandbox.write_file("src/calc.ts", "export function add(a: number, b: number) { return a + b; }\n").unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    sandbox.modify_file("src/calc.ts", "export function add(a: number, b: number, c: number = 0) { return a + b + c; }\n").unwrap();

    let args = json!({
        "path": sandbox.path().to_str().unwrap(),
        "format": "json"
    });

    let (resp, metrics, err, _tokens_saved) = execute_tool_with_timeout("semantic_diff", &args, 5000);
    assert!(err.is_none());
    assert_eq!(resp.get("isError"), None);
    assert!(metrics.is_some());
    let diff_data = resp.get("semantic_diff").expect("semantic_diff property in MCP output");
    assert_eq!(diff_data["total_signature_changes"], 1);
}

#[test]
fn test_m4_adv_cli_verify_patch_and_semantic_diff_e2e() {
    let sandbox = GitSandbox::new().expect("Failed sandbox");
    sandbox.write_file("src/app.ts", "export function startApp() { return 'running'; }\n").unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("init").unwrap();

    let runner = CliRunner::new();

    // 1. CLI verify-patch with dry-run
    let target = format!("{}:startApp", sandbox.resolve_path("src/app.ts").display());
    let patch_out = runner.run_in_dir(
        sandbox.path(),
        &[
            "verify-patch",
            &target,
            "--code",
            "export function startApp() { return 'stopped'; }",
            "--typechecker",
            if cfg!(target_os = "windows") { "powershell -Command \"exit 0\"" } else { "true" },
            "--format",
            "json",
        ],
    ).expect("verify-patch CLI failed");

    patch_out.assert_success();
    assert!(patch_out.stdout.contains("\"success\": true") || patch_out.stdout.contains("\"success\":true"));

    // 2. CLI semantic-diff
    sandbox.modify_file("src/app.ts", "export function startApp(mode: string = 'prod') { return mode; }\n").unwrap();
    let diff_out = runner.run_in_dir(
        sandbox.path(),
        &["semantic-diff", "--path", sandbox.path().to_str().unwrap(), "--format", "json"],
    ).expect("semantic-diff CLI failed");

    diff_out.assert_success();
    assert!(diff_out.stdout.contains("startApp"));
    assert!(diff_out.stdout.contains("SignatureChanged") || diff_out.stdout.contains("signature_changed"));
}
