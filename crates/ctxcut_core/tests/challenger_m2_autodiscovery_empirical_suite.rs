//! Adversarial Empirical Challenge Suite for Milestone 2 (R2):
//! Monorepo & Multi-Manifest Auto-Discovery for Typecheckers.

use ctxcut_core::model::SupportedLanguage;
use ctxcut_core::refactor::batch::{BatchAstPatcher, PatchTransactionRequest, SymbolPatchUnit};
use ctxcut_core::verify::typechecker::{TypecheckerDetector, TypecheckerRunner};
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_adversarial_deeply_nested_tauri_and_workspace_crates() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Complex directory hierarchy:
    // root/
    //   apps/
    //     desktop/
    //       package.json
    //       src-tauri/
    //         Cargo.toml
    //         src/
    //           nested/
    //             deep/
    //               commands.rs
    //     web/
    //       tsconfig.json
    //       src/
    //         index.tsx
    //   crates/
    //     shared-utils/
    //       Cargo.toml
    //       src/
    //         lib.rs
    let tauri_app = root.join("apps").join("desktop").join("src-tauri");
    let deep_rs_dir = tauri_app.join("src").join("nested").join("deep");
    fs::create_dir_all(&deep_rs_dir).unwrap();
    let tauri_cargo = tauri_app.join("Cargo.toml");
    fs::write(&tauri_cargo, "[package]\nname = \"desktop-tauri\"\n").unwrap();
    let commands_rs = deep_rs_dir.join("commands.rs");
    fs::write(
        &commands_rs,
        "pub fn do_stuff() -> i32 {\n    42\n}\n",
    )
    .unwrap();

    let shared_crate = root.join("crates").join("shared-utils");
    fs::create_dir_all(shared_crate.join("src")).unwrap();
    let shared_cargo = shared_crate.join("Cargo.toml");
    fs::write(&shared_cargo, "[package]\nname = \"shared-utils\"\n").unwrap();
    let shared_rs = shared_crate.join("src").join("lib.rs");
    fs::write(&shared_rs, "pub fn helper() {}\n").unwrap();

    // 1. Verify deeply nested Tauri Rust file resolves to apps/desktop/src-tauri
    let tauri_res = TypecheckerDetector::detect_resolution(root, &commands_rs, SupportedLanguage::Rust)
        .expect("Must resolve deeply nested Tauri Cargo.toml");
    assert_eq!(tauri_res.working_dir, tauri_app);
    assert_eq!(tauri_res.manifest_path, Some(tauri_cargo));
    assert!(tauri_res.command.contains("cargo check"));
    assert!(tauri_res.command.contains("--manifest-path"));
    assert!(tauri_res.command.contains("Cargo.toml"));

    // 2. Verify shared crate Rust file resolves to crates/shared-utils
    let shared_res = TypecheckerDetector::detect_resolution(root, &shared_rs, SupportedLanguage::Rust)
        .expect("Must resolve crates/shared-utils Cargo.toml");
    assert_eq!(shared_res.working_dir, shared_crate);
    assert_eq!(shared_res.manifest_path, Some(shared_cargo));
    assert!(shared_res.command.contains("cargo check"));
    assert!(shared_res.command.contains("--manifest-path"));
}

#[test]
fn test_adversarial_ts_monorepo_priority_hierarchy() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("package.json"), "{\"name\":\"root\"}").unwrap();
    let root_tsconfig = root.join("tsconfig.json");
    fs::write(&root_tsconfig, "{\"compilerOptions\":{}}").unwrap();

    let client_dir = root.join("packages").join("client");
    fs::create_dir_all(client_dir.join("src").join("components")).unwrap();
    fs::write(client_dir.join("package.json"), "{\"name\":\"@mono/client\"}").unwrap();
    let client_tsconfig = client_dir.join("tsconfig.json");
    fs::write(&client_tsconfig, "{\"extends\":\"../../tsconfig.json\"}").unwrap();
    let header_tsx = client_dir.join("src").join("components").join("Header.tsx");
    fs::write(&header_tsx, "export const Header = () => 1;").unwrap();

    let server_dir = root.join("packages").join("server");
    fs::create_dir_all(server_dir.join("src")).unwrap();
    let server_pkg = server_dir.join("package.json");
    fs::write(&server_pkg, "{\"name\":\"@mono/server\"}").unwrap();
    let server_ts = server_dir.join("src").join("index.ts");
    fs::write(&server_ts, "export const port = 3000;").unwrap();

    let shared_dir = root.join("packages").join("shared");
    fs::create_dir_all(shared_dir.join("src")).unwrap();
    let shared_ts = shared_dir.join("src").join("types.ts");
    fs::write(&shared_ts, "export type ID = string;").unwrap();

    // 1. Client TS file must pick closest tsconfig in packages/client
    let client_res = TypecheckerDetector::detect_resolution(root, &header_tsx, SupportedLanguage::TypeScript)
        .expect("Must resolve client tsconfig");
    assert_eq!(client_res.working_dir, client_dir);
    assert_eq!(client_res.manifest_path, Some(client_tsconfig));

    // 2. Server TS file traverses upward and finds root tsconfig.json (priority 1)
    let server_res = TypecheckerDetector::detect_resolution(root, &server_ts, SupportedLanguage::TypeScript)
        .expect("Must resolve server ts");
    assert_eq!(server_res.command, "npx tsc --noEmit");

    // 3. Shared TS file finds root tsconfig
    let shared_res = TypecheckerDetector::detect_resolution(root, &shared_ts, SupportedLanguage::TypeScript)
        .expect("Must resolve shared ts");
    assert_eq!(shared_res.working_dir, root);
    assert_eq!(shared_res.manifest_path, Some(root_tsconfig));
}

#[test]
fn test_adversarial_go_multimodule_and_nested_packages() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let auth_mod_dir = root.join("services").join("auth");
    let crypto_dir = auth_mod_dir.join("internal").join("crypto");
    fs::create_dir_all(&crypto_dir).unwrap();
    let auth_mod = auth_mod_dir.join("go.mod");
    fs::write(&auth_mod, "module company/auth\n\ngo 1.22\n").unwrap();
    let token_go = crypto_dir.join("token.go");
    fs::write(&token_go, "package crypto\n\nfunc Gen() string { return \"\" }\n").unwrap();

    let gw_mod_dir = root.join("services").join("gateway");
    fs::create_dir_all(&gw_mod_dir).unwrap();
    let gw_mod = gw_mod_dir.join("go.mod");
    fs::write(&gw_mod, "module company/gateway\n\ngo 1.22\n").unwrap();
    let gw_main = gw_mod_dir.join("main.go");
    fs::write(&gw_main, "package main\n\nfunc main() {}\n").unwrap();

    // Verify deep nested package resolves to services/auth
    let res = TypecheckerDetector::detect_resolution(root, &token_go, SupportedLanguage::Go)
        .expect("Must resolve auth module");
    assert_eq!(res.working_dir, auth_mod_dir);
    assert_eq!(res.manifest_path, Some(auth_mod));
    assert_eq!(res.command, "go vet ./...");

    // Verify gateway resolves to services/gateway
    let gw_res = TypecheckerDetector::detect_resolution(root, &gw_main, SupportedLanguage::Go)
        .expect("Must resolve gateway module");
    assert_eq!(gw_res.working_dir, gw_mod_dir);
    assert_eq!(gw_res.manifest_path, Some(gw_mod));
}

#[test]
fn test_adversarial_python_relative_path_and_manifest_formats() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let ml_dir = root.join("apps").join("ml_service");
    let pipelines_dir = ml_dir.join("src").join("pipelines");
    fs::create_dir_all(&pipelines_dir).unwrap();
    let pyproject = ml_dir.join("pyproject.toml");
    fs::write(&pyproject, "[tool.mypy]\nstrict = true\n").unwrap();
    let train_py = pipelines_dir.join("train.py");
    fs::write(&train_py, "def train(): pass\n").unwrap();

    let api_dir = root.join("apps").join("api");
    fs::create_dir_all(&api_dir).unwrap();
    let setup_py = api_dir.join("setup.py");
    fs::write(&setup_py, "# setup.py\n").unwrap();
    let routes_py = api_dir.join("routes.py");
    fs::write(&routes_py, "def route(): pass\n").unwrap();

    // 1. Pyproject with subpath: mypy relative command verification
    let ml_res = TypecheckerDetector::detect_resolution(root, &train_py, SupportedLanguage::Python)
        .expect("Must resolve pyproject.toml");
    assert_eq!(ml_res.working_dir, ml_dir);
    assert_eq!(ml_res.manifest_path, Some(pyproject));
    assert!(ml_res.command.starts_with("mypy \""));
    assert!(ml_res.command.contains("src"));
    assert!(ml_res.command.contains("train.py"));

    // 2. Setup.py uses python -m py_compile
    let api_res = TypecheckerDetector::detect_resolution(root, &routes_py, SupportedLanguage::Python)
        .expect("Must resolve setup.py");
    assert_eq!(api_res.working_dir, api_dir);
    assert_eq!(api_res.manifest_path, Some(setup_py));
    assert!(api_res.command.starts_with("python -m py_compile \""));
    assert!(api_res.command.contains("routes.py"));
}

#[test]
fn test_adversarial_other_languages_monorepo() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Test Java Maven vs Kotlin Gradle multi-project
    let java_sub = root.join("services").join("java-service");
    fs::create_dir_all(java_sub.join("src")).unwrap();
    let pom_xml = java_sub.join("pom.xml");
    fs::write(&pom_xml, "<project></project>").unwrap();
    let java_file = java_sub.join("src").join("Service.java");
    fs::write(&java_file, "class Service {}").unwrap();

    let java_res = TypecheckerDetector::detect_resolution(root, &java_file, SupportedLanguage::Java)
        .expect("Must resolve pom.xml");
    assert_eq!(java_res.working_dir, java_sub);
    assert_eq!(java_res.command, "mvn compile -DskipTests");

    // Test Vue monorepo package
    let vue_pkg = root.join("packages").join("vue-components");
    fs::create_dir_all(vue_pkg.join("src")).unwrap();
    let vue_manifest = vue_pkg.join("package.json");
    fs::write(&vue_manifest, "{\"name\":\"vue-pkg\"}").unwrap();
    let vue_file = vue_pkg.join("src").join("Modal.vue");
    fs::write(&vue_file, "<template><div>Modal</div></template>").unwrap();

    let vue_res = TypecheckerDetector::detect_resolution(root, &vue_file, SupportedLanguage::Vue)
        .expect("Must resolve vue package.json");
    assert_eq!(vue_res.working_dir, vue_pkg);
    assert_eq!(vue_res.command, "npx vue-tsc --noEmit");

    // Test Svelte monorepo package
    let svelte_pkg = root.join("packages").join("svelte-app");
    fs::create_dir_all(svelte_pkg.join("src")).unwrap();
    let svelte_manifest = svelte_pkg.join("tsconfig.json");
    fs::write(&svelte_manifest, "{}").unwrap();
    let svelte_file = svelte_pkg.join("src").join("App.svelte");
    fs::write(&svelte_file, "<script></script>").unwrap();

    let svelte_res = TypecheckerDetector::detect_resolution(root, &svelte_file, SupportedLanguage::Svelte)
        .expect("Must resolve svelte tsconfig");
    assert_eq!(svelte_res.working_dir, svelte_pkg);
    assert_eq!(svelte_res.command, "npx svelte-check");

    // Test Astro monorepo package
    let astro_pkg = root.join("packages").join("docs-site");
    fs::create_dir_all(astro_pkg.join("src")).unwrap();
    let astro_manifest = astro_pkg.join("package.json");
    fs::write(&astro_manifest, "{\"name\":\"docs\"}").unwrap();
    let astro_file = astro_pkg.join("src").join("index.astro");
    fs::write(&astro_file, "---\n---\n<html></html>").unwrap();

    let astro_res = TypecheckerDetector::detect_resolution(root, &astro_file, SupportedLanguage::Astro)
        .expect("Must resolve astro package.json");
    assert_eq!(astro_res.working_dir, astro_pkg);
    assert_eq!(astro_res.command, "npx astro check");
}

#[test]
fn test_adversarial_typechecker_runner_cwd_execution() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let sub_dir = root.join("nested_workspace");
    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(sub_dir.join("marker.txt"), "IN_SUBDIR").unwrap();

    // Run powershell/sh command checking current directory contains marker.txt
    let test_cmd = if cfg!(target_os = "windows") {
        "Test-Path marker.txt"
    } else {
        "test -f marker.txt"
    };

    let result = TypecheckerRunner::run(test_cmd, &sub_dir, Duration::from_secs(5));
    assert!(result.success, "Command should succeed in sub_dir: {:?}", result);
    if cfg!(target_os = "windows") {
        assert!(result.stdout.to_lowercase().contains("true"));
    }
}

#[test]
fn test_adversarial_batch_transaction_polyglot_atomic_rollback() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Multi-module repo with Rust subproject and TS subproject
    let rs_dir = root.join("services").join("engine");
    fs::create_dir_all(rs_dir.join("src")).unwrap();
    fs::write(rs_dir.join("Cargo.toml"), "[package]\nname=\"engine\"\n").unwrap();
    let rs_file = rs_dir.join("src").join("lib.rs");
    let orig_rs = "pub fn calculate(val: i32) -> i32 {\n    val * 2\n}\n";
    fs::write(&rs_file, orig_rs).unwrap();

    let ts_dir = root.join("services").join("frontend");
    fs::create_dir_all(ts_dir.join("src")).unwrap();
    fs::write(ts_dir.join("tsconfig.json"), "{}").unwrap();
    let ts_file = ts_dir.join("src").join("main.ts");
    let orig_ts = "export function init() {\n    return true;\n}\n";
    fs::write(&ts_file, orig_ts).unwrap();

    // Case 1: Transaction dry-run with mock successful typechecker override
    let success_cmd = if cfg!(target_os = "windows") {
        "Write-Output 'OK'"
    } else {
        "echo OK"
    };

    let req_dry = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: rs_file.clone(),
                symbol_query: "calculate".to_string(),
                replacement_code: "pub fn calculate(val: i32) -> i32 {\n    val * 4\n}".to_string(),
            },
            SymbolPatchUnit {
                file_path: ts_file.clone(),
                symbol_query: "init".to_string(),
                replacement_code: "export function init() {\n    return false;\n}".to_string(),
            },
        ],
        typechecker: Some(success_cmd.to_string()),
        apply: false, // dry-run
        timeout_ms: Some(5000),
    };

    let tx_res = BatchAstPatcher::apply_transaction(&req_dry).unwrap();
    assert!(tx_res.success);
    assert!(!tx_res.applied);
    assert!(tx_res.rolled_back);
    assert_eq!(tx_res.files_modified_count, 2);
    assert_eq!(tx_res.symbols_patched_count, 2);
    assert_eq!(fs::read_to_string(&rs_file).unwrap(), orig_rs);
    assert_eq!(fs::read_to_string(&ts_file).unwrap(), orig_ts);

    // Case 2: Transaction apply: true with mock failing typechecker -> must rollback!
    let fail_cmd = if cfg!(target_os = "windows") {
        "exit 1"
    } else {
        "exit 1"
    };

    let req_fail = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: rs_file.clone(),
                symbol_query: "calculate".to_string(),
                replacement_code: "pub fn calculate(val: i32) -> i32 {\n    val * 8\n}".to_string(),
            },
        ],
        typechecker: Some(fail_cmd.to_string()),
        apply: true, // requested apply, but typechecker fails
        timeout_ms: Some(5000),
    };

    let fail_res = BatchAstPatcher::apply_transaction(&req_fail).unwrap();
    assert!(!fail_res.success);
    assert!(!fail_res.applied);
    assert!(fail_res.rolled_back);
    // Ensure file on disk was preserved
    assert_eq!(fs::read_to_string(&rs_file).unwrap(), orig_rs);
}
