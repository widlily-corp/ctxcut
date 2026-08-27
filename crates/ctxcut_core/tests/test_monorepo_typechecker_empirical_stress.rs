//! Comprehensive Empirical Stress Tests for Milestone 2 (R2):
//! Monorepo & Multi-Manifest Auto-Discovery for Typecheckers.

use ctxcut_core::model::SupportedLanguage;
use ctxcut_core::refactor::batch::{BatchAstPatcher, PatchTransactionRequest, SymbolPatchUnit};
use ctxcut_core::verify::typechecker::TypecheckerDetector;
use ctxcut_core::verify::PatchVerifier;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_deeply_nested_manifest_discovery_5_levels() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Deep Rust Subproject (5 levels down)
    let rust_sub = root.join("apps").join("deep").join("nested").join("service");
    let rust_deep_src = rust_sub.join("src").join("controllers").join("admin");
    fs::create_dir_all(&rust_deep_src).unwrap();
    let rust_manifest = rust_sub.join("Cargo.toml");
    fs::write(&rust_manifest, "[package]\nname = \"deep-service\"\nversion = \"0.1.0\"\n").unwrap();
    let rust_file = rust_deep_src.join("handler.rs");
    fs::write(&rust_file, "pub fn handle_request() -> bool { true }\n").unwrap();

    let rust_res = TypecheckerDetector::detect_resolution(root, &rust_file, SupportedLanguage::Rust)
        .expect("Should resolve 5-level deep Cargo.toml");
    assert_eq!(rust_res.working_dir, rust_sub);
    assert_eq!(rust_res.manifest_path, Some(rust_manifest));
    assert!(rust_res.command.contains("cargo check --manifest-path"));

    // 2. Deep TypeScript Subproject (5 levels down)
    let ts_sub = root.join("packages").join("frontend").join("web").join("portal");
    let ts_deep_src = ts_sub.join("src").join("components").join("atoms");
    fs::create_dir_all(&ts_deep_src).unwrap();
    let ts_manifest = ts_sub.join("tsconfig.json");
    fs::write(&ts_manifest, "{\"compilerOptions\":{}}").unwrap();
    let ts_file = ts_deep_src.join("button.tsx");
    fs::write(&ts_file, "export function Button() { return <button />; }\n").unwrap();

    let ts_res = TypecheckerDetector::detect_resolution(root, &ts_file, SupportedLanguage::TypeScript)
        .expect("Should resolve 5-level deep tsconfig.json");
    assert_eq!(ts_res.working_dir, ts_sub);
    assert_eq!(ts_res.manifest_path, Some(ts_manifest));
    assert_eq!(ts_res.command, "npx tsc --noEmit");

    // 3. Deep Go Subproject (4 levels down)
    let go_sub = root.join("microservices").join("finance").join("billing");
    let go_deep_src = go_sub.join("engine").join("core");
    fs::create_dir_all(&go_deep_src).unwrap();
    let go_manifest = go_sub.join("go.mod");
    fs::write(&go_manifest, "module billing\n\ngo 1.22\n").unwrap();
    let go_file = go_deep_src.join("calculator.go");
    fs::write(&go_file, "package core\n\nfunc Calculate() int { return 42 }\n").unwrap();

    let go_res = TypecheckerDetector::detect_resolution(root, &go_file, SupportedLanguage::Go)
        .expect("Should resolve deep go.mod");
    assert_eq!(go_res.working_dir, go_sub);
    assert_eq!(go_res.manifest_path, Some(go_manifest));
    assert_eq!(go_res.command, "go vet ./...");

    // 4. Deep Python Subproject (4 levels down)
    let py_sub = root.join("ai").join("pipelines").join("training");
    let py_deep_src = py_sub.join("deep").join("models");
    fs::create_dir_all(&py_deep_src).unwrap();
    let py_manifest = py_sub.join("pyproject.toml");
    fs::write(&py_manifest, "[tool.mypy]\nstrict = true\n").unwrap();
    let py_file = py_deep_src.join("transformer.py");
    fs::write(&py_file, "def train(): pass\n").unwrap();

    let py_res = TypecheckerDetector::detect_resolution(root, &py_file, SupportedLanguage::Python)
        .expect("Should resolve deep pyproject.toml");
    assert_eq!(py_res.working_dir, py_sub);
    assert_eq!(py_res.manifest_path, Some(py_manifest));
    assert!(py_res.command.contains("mypy"));
    assert!(py_res.command.contains("deep"));
    assert!(py_res.command.contains("transformer.py"));

    // 5. Deep C# Subproject (.csproj)
    let cs_sub = root.join("enterprise").join("apps").join("core").join("backend");
    let cs_deep_src = cs_sub.join("services");
    fs::create_dir_all(&cs_deep_src).unwrap();
    let cs_manifest = cs_sub.join("Backend.csproj");
    fs::write(&cs_manifest, "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>").unwrap();
    let cs_file = cs_deep_src.join("AccountService.cs");
    fs::write(&cs_file, "public class AccountService {}\n").unwrap();

    let cs_res = TypecheckerDetector::detect_resolution(root, &cs_file, SupportedLanguage::CSharp)
        .expect("Should resolve deep .csproj");
    assert_eq!(cs_res.working_dir, cs_sub);
    assert_eq!(cs_res.manifest_path, Some(cs_manifest));
    assert_eq!(cs_res.command, "dotnet build");

    // 6. Deep Java Subproject (pom.xml)
    let java_sub = root.join("backend").join("modules").join("auth").join("oauth2");
    let java_deep_src = java_sub.join("src").join("main").join("java");
    fs::create_dir_all(&java_deep_src).unwrap();
    let java_manifest = java_sub.join("pom.xml");
    fs::write(&java_manifest, "<project></project>").unwrap();
    let java_file = java_deep_src.join("TokenService.java");
    fs::write(&java_file, "public class TokenService {}\n").unwrap();

    let java_res = TypecheckerDetector::detect_resolution(root, &java_file, SupportedLanguage::Java)
        .expect("Should resolve deep pom.xml");
    assert_eq!(java_res.working_dir, java_sub);
    assert_eq!(java_res.manifest_path, Some(java_manifest));
    assert_eq!(java_res.command, "mvn compile -DskipTests");

    // 7. Deep Kotlin Subproject (build.gradle.kts)
    let kt_sub = root.join("mobile").join("android").join("features").join("feed");
    let kt_deep_src = kt_sub.join("src").join("main").join("kotlin");
    fs::create_dir_all(&kt_deep_src).unwrap();
    let kt_manifest = kt_sub.join("build.gradle.kts");
    fs::write(&kt_manifest, "plugins { kotlin(\"jvm\") }").unwrap();
    let kt_file = kt_deep_src.join("FeedView.kt");
    fs::write(&kt_file, "class FeedView\n").unwrap();

    let kt_res = TypecheckerDetector::detect_resolution(root, &kt_file, SupportedLanguage::Kotlin)
        .expect("Should resolve deep build.gradle.kts");
    assert_eq!(kt_res.working_dir, kt_sub);
    assert_eq!(kt_res.manifest_path, Some(kt_manifest));
    assert_eq!(kt_res.command, "gradle compileKotlin");

    // 8. Deep Vue / Svelte / Astro Subprojects
    let vue_sub = root.join("web").join("client").join("ui");
    fs::create_dir_all(vue_sub.join("components")).unwrap();
    let vue_manifest = vue_sub.join("package.json");
    fs::write(&vue_manifest, "{\"name\":\"ui\"}").unwrap();
    let vue_file = vue_sub.join("components").join("Widget.vue");
    fs::write(&vue_file, "<template><div>Widget</div></template>").unwrap();

    let vue_res = TypecheckerDetector::detect_resolution(root, &vue_file, SupportedLanguage::Vue)
        .expect("Should resolve deep Vue package.json");
    assert_eq!(vue_res.working_dir, vue_sub);
    assert_eq!(vue_res.manifest_path, Some(vue_manifest));
    assert_eq!(vue_res.command, "npx vue-tsc --noEmit");

    // 9. Deep C / C++ Subproject (CMakeLists.txt)
    let cpp_sub = root.join("native").join("src");
    let cpp_deep_src = cpp_sub.join("core").join("render");
    fs::create_dir_all(&cpp_deep_src).unwrap();
    let cpp_manifest = cpp_sub.join("CMakeLists.txt");
    fs::write(&cpp_manifest, "cmake_minimum_required(VERSION 3.10)").unwrap();
    let cpp_file = cpp_deep_src.join("engine.cpp");
    fs::write(&cpp_file, "int render() { return 0; }\n").unwrap();

    let cpp_res = TypecheckerDetector::detect_resolution(root, &cpp_file, SupportedLanguage::Cpp)
        .expect("Should resolve deep CMakeLists.txt");
    assert_eq!(cpp_res.working_dir, cpp_sub);
    assert_eq!(cpp_res.manifest_path, Some(cpp_manifest));
    assert!(cpp_res.command.contains("clang++ -fsyntax-only"));
    assert!(cpp_res.command.contains("engine.cpp"));
}

#[test]
fn test_monorepo_closest_nested_vs_root_manifest_precedence() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Root Cargo.toml (workspace)
    let root_cargo = root.join("Cargo.toml");
    fs::write(&root_cargo, "[workspace]\nmembers = [\"crates/*\"]\n").unwrap();

    // Nested Crate Cargo.toml
    let crate_dir = root.join("crates").join("core_engine");
    fs::create_dir_all(crate_dir.join("src")).unwrap();
    let sub_cargo = crate_dir.join("Cargo.toml");
    fs::write(&sub_cargo, "[package]\nname = \"core_engine\"\nversion = \"0.1.0\"\n").unwrap();

    let file_in_subcrate = crate_dir.join("src").join("lib.rs");
    fs::write(&file_in_subcrate, "pub fn init() {}\n").unwrap();

    let res = TypecheckerDetector::detect_resolution(root, &file_in_subcrate, SupportedLanguage::Rust)
        .expect("Should resolve to closest subcrate Cargo.toml");

    // Must pick the closer subcrate manifest, NOT the root workspace manifest
    assert_eq!(res.working_dir, crate_dir);
    assert_eq!(res.manifest_path, Some(sub_cargo));
    assert!(res.command.contains("crates/core_engine") || res.command.contains("crates\\core_engine"));
}

#[test]
fn test_files_outside_any_manifest_graceful_fallback() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Rust file with NO Cargo.toml anywhere
    let standalone_rust = root.join("scratch").join("test.rs");
    fs::create_dir_all(root.join("scratch")).unwrap();
    fs::write(&standalone_rust, "fn test() {}\n").unwrap();

    let rust_res = TypecheckerDetector::detect_resolution(root, &standalone_rust, SupportedLanguage::Rust);
    assert!(rust_res.is_none(), "Rust without Cargo.toml should return None without panicking");

    // 2. TypeScript file with NO tsconfig.json or package.json
    let standalone_ts = root.join("scratch").join("script.ts");
    fs::write(&standalone_ts, "console.log('hi');\n").unwrap();

    let ts_res = TypecheckerDetector::detect_resolution(root, &standalone_ts, SupportedLanguage::TypeScript);
    assert!(ts_res.is_none(), "TypeScript without manifest should return None without panicking");

    // 3. Python file with NO manifest: defaults to python -m py_compile in workspace_root
    let standalone_py = root.join("scratch").join("script.py");
    fs::write(&standalone_py, "print('hi')\n").unwrap();

    let py_res = TypecheckerDetector::detect_resolution(root, &standalone_py, SupportedLanguage::Python)
        .expect("Python should fallback to py_compile");
    assert_eq!(py_res.working_dir, root);
    assert_eq!(py_res.manifest_path, None);
    assert!(py_res.command.contains("python -m py_compile"));
    assert!(py_res.command.contains("script.py"));

    // 4. C++ file with NO CMakeLists.txt or Makefile
    let standalone_cpp = root.join("scratch").join("temp.cpp");
    fs::write(&standalone_cpp, "int main() {}\n").unwrap();

    let cpp_res = TypecheckerDetector::detect_resolution(root, &standalone_cpp, SupportedLanguage::Cpp);
    assert!(cpp_res.is_none(), "C++ without manifest should return None without panicking");
}

#[test]
fn test_verify_patch_dry_run_and_apply_modes_empirical() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let subproject = root.join("services").join("auth");
    fs::create_dir_all(subproject.join("src")).unwrap();
    fs::write(subproject.join("Cargo.toml"), "[package]\nname = \"auth\"\nversion = \"0.1.0\"\n").unwrap();

    let target_file = subproject.join("src").join("token.rs");
    let original_content = "pub fn generate_token(user_id: u64) -> String {\n    format!(\"tok_{}\", user_id)\n}\n";
    fs::write(&target_file, original_content).unwrap();

    let target_str = format!("{}:generate_token", target_file.to_string_lossy());
    let valid_patch = "pub fn generate_token(user_id: u64) -> String {\n    format!(\"jwt_v2_{}\", user_id)\n}";

    // 1. Test DRY-RUN mode with custom benign typechecker command (exit 0)
    let ok_command = if cfg!(target_os = "windows") { "exit 0" } else { "true" };
    let dry_run_res = PatchVerifier::verify_patch(
        root,
        &target_str,
        valid_patch,
        Some(ok_command),
        true, // dry-run
    )
    .expect("verify_patch dry-run should succeed");

    assert!(dry_run_res.success);
    assert!(!dry_run_res.applied, "In dry_run mode, applied must be false");
    assert!(dry_run_res.dry_run);
    assert!(dry_run_res.diff.contains("+    format!(\"jwt_v2_{}\", user_id)"));

    // Empirically verify disk content was NOT modified in dry-run mode
    let disk_content_after_dry_run = fs::read_to_string(&target_file).unwrap();
    assert_eq!(disk_content_after_dry_run, original_content, "Disk content MUST be unchanged after dry run");

    // 2. Test APPLY mode with custom benign typechecker command (exit 0)
    let apply_res = PatchVerifier::verify_patch(
        root,
        &target_str,
        valid_patch,
        Some(ok_command),
        false, // apply mode
    )
    .expect("verify_patch apply should succeed");

    assert!(apply_res.success);
    assert!(apply_res.applied, "In apply mode with exit 0, applied must be true");
    assert!(!apply_res.dry_run);

    // Empirically verify disk content WAS updated in apply mode
    let disk_content_after_apply = fs::read_to_string(&target_file).unwrap();
    assert!(disk_content_after_apply.contains("jwt_v2_"), "Disk content MUST be updated after apply mode");
}

#[test]
fn test_verify_patch_syntax_error_rejection_and_disk_safety() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let subproject = root.join("packages").join("math");
    fs::create_dir_all(subproject.join("src")).unwrap();
    fs::write(subproject.join("package.json"), "{\"name\":\"math\"}").unwrap();

    let target_file = subproject.join("src").join("calc.ts");
    let original_content = "export function add(a: number, b: number): number {\n    return a + b;\n}\n";
    fs::write(&target_file, original_content).unwrap();

    let target_str = format!("{}:add", target_file.to_string_lossy());
    // Broken TypeScript syntax: unclosed brace and missing token
    let syntax_error_patch = "export function add(a: number, b: number): number {\n    return a + ; // broken";

    let verify_res = PatchVerifier::verify_patch(
        root,
        &target_str,
        syntax_error_patch,
        None,
        false, // try to apply
    )
    .expect("verify_patch handles syntax error as structured failure");

    assert!(!verify_res.success, "Syntax error must cause verification failure");
    assert!(!verify_res.applied, "Syntax error must never be applied to disk");
    assert!(!verify_res.syntax_errors.is_empty(), "Syntax errors list must not be empty");

    // Empirically verify disk file remains intact
    let disk_content = fs::read_to_string(&target_file).unwrap();
    assert_eq!(disk_content, original_content, "Disk file must remain pristine after syntax error");
}

#[test]
fn test_patch_transaction_multi_file_rollback_on_typechecker_failure() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Nested package A
    let pkg_a = root.join("packages").join("pkg-a");
    fs::create_dir_all(pkg_a.join("src")).unwrap();
    fs::write(pkg_a.join("package.json"), "{\"name\":\"pkg-a\"}").unwrap();
    let file_a = pkg_a.join("src").join("a.ts");
    let orig_a = "export function funcA(): string {\n    return 'original_a';\n}\n";
    fs::write(&file_a, orig_a).unwrap();

    // Nested package B
    let pkg_b = root.join("packages").join("pkg-b");
    fs::create_dir_all(pkg_b.join("src")).unwrap();
    fs::write(pkg_b.join("package.json"), "{\"name\":\"pkg-b\"}").unwrap();
    let file_b = pkg_b.join("src").join("b.ts");
    let orig_b = "export function funcB(): string {\n    return 'original_b';\n}\n";
    fs::write(&file_b, orig_b).unwrap();

    // Typechecker command that simulates failure (exit 1)
    let fail_command = if cfg!(target_os = "windows") { "exit 1" } else { "false" };

    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: file_a.clone(),
                symbol_query: "funcA".to_string(),
                replacement_code: "export function funcA(): string {\n    return 'modified_a';\n}".to_string(),
            },
            SymbolPatchUnit {
                file_path: file_b.clone(),
                symbol_query: "funcB".to_string(),
                replacement_code: "export function funcB(): string {\n    return 'modified_b';\n}".to_string(),
            },
        ],
        typechecker: Some(fail_command.to_string()),
        apply: true, // Attempt to apply, but typechecker will fail
        timeout_ms: Some(5000),
    };

    let result = BatchAstPatcher::apply_transaction(&req).expect("apply_transaction should return result");

    assert!(!result.success, "Result success must be false when typechecker fails");
    assert!(!result.applied, "Result applied must be false when typechecker fails");
    assert!(result.rolled_back, "Result rolled_back must be true on typechecker failure");
    assert_eq!(result.exit_code, Some(1));

    // Empirically verify ALL files on disk were rolled back to original content
    assert_eq!(fs::read_to_string(&file_a).unwrap(), orig_a, "File A must be rolled back to original");
    assert_eq!(fs::read_to_string(&file_b).unwrap(), orig_b, "File B must be rolled back to original");
}

#[test]
fn test_patch_transaction_multi_file_success_commit() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let pkg_a = root.join("packages").join("pkg-a");
    fs::create_dir_all(pkg_a.join("src")).unwrap();
    fs::write(pkg_a.join("package.json"), "{\"name\":\"pkg-a\"}").unwrap();
    let file_a = pkg_a.join("src").join("a.ts");
    fs::write(&file_a, "export function funcA(): string {\n    return 'old_a';\n}\n").unwrap();

    let pkg_b = root.join("packages").join("pkg-b");
    fs::create_dir_all(pkg_b.join("src")).unwrap();
    fs::write(pkg_b.join("package.json"), "{\"name\":\"pkg-b\"}").unwrap();
    let file_b = pkg_b.join("src").join("b.ts");
    fs::write(&file_b, "export function funcB(): string {\n    return 'old_b';\n}\n").unwrap();

    let ok_command = if cfg!(target_os = "windows") { "exit 0" } else { "true" };

    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: file_a.clone(),
                symbol_query: "funcA".to_string(),
                replacement_code: "export function funcA(): string {\n    return 'new_a_committed';\n}".to_string(),
            },
            SymbolPatchUnit {
                file_path: file_b.clone(),
                symbol_query: "funcB".to_string(),
                replacement_code: "export function funcB(): string {\n    return 'new_b_committed';\n}".to_string(),
            },
        ],
        typechecker: Some(ok_command.to_string()),
        apply: true,
        timeout_ms: Some(5000),
    };

    let result = BatchAstPatcher::apply_transaction(&req).expect("apply_transaction should succeed");

    assert!(result.success);
    assert!(result.applied);
    assert!(!result.rolled_back);
    assert_eq!(result.files_modified_count, 2);
    assert_eq!(result.symbols_patched_count, 2);

    // Empirically verify ALL files on disk were updated
    assert!(fs::read_to_string(&file_a).unwrap().contains("new_a_committed"));
    assert!(fs::read_to_string(&file_b).unwrap().contains("new_b_committed"));
}
