//! Comprehensive integration tests for Milestone 2 (R2):
//! Monorepo & Multi-Manifest Auto-Discovery for Typecheckers.

use ctxcut_core::model::SupportedLanguage;
use ctxcut_core::refactor::batch::{BatchAstPatcher, PatchTransactionRequest, SymbolPatchUnit};
use ctxcut_core::verify::typechecker::TypecheckerDetector;
use ctxcut_core::verify::PatchVerifier;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_tauri_hybrid_repo_autodiscovery() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Setup Tauri project layout:
    // root/
    //   package.json (web frontend)
    //   src/
    //     App.tsx
    //   src-tauri/
    //     Cargo.toml
    //     src/
    //       main.rs
    fs::write(root.join("package.json"), "{\"name\":\"frontend\"}").unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    let ts_file = root.join("src").join("App.tsx");
    fs::write(&ts_file, "export function App() { return <div>App</div>; }").unwrap();

    let tauri_dir = root.join("src-tauri");
    fs::create_dir_all(tauri_dir.join("src")).unwrap();
    let cargo_toml = tauri_dir.join("Cargo.toml");
    fs::write(&cargo_toml, "[package]\nname = \"tauri-backend\"\nversion = \"0.1.0\"\n").unwrap();
    let rs_file = tauri_dir.join("src").join("main.rs");
    fs::write(
        &rs_file,
        "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n",
    )
    .unwrap();

    // 1. Verify Rust file in src-tauri discovers src-tauri/Cargo.toml
    let rust_res = TypecheckerDetector::detect_resolution(root, &rs_file, SupportedLanguage::Rust)
        .expect("Should resolve nested Cargo.toml for Tauri");
    assert_eq!(rust_res.working_dir, tauri_dir);
    assert_eq!(rust_res.manifest_path, Some(cargo_toml.clone()));
    assert!(rust_res.command.contains("cargo check"));
    assert!(rust_res.command.contains("Cargo.toml"));

    // 2. Verify TypeScript file discovers root package.json
    let ts_res = TypecheckerDetector::detect_resolution(root, &ts_file, SupportedLanguage::TypeScript)
        .expect("Should resolve root package.json for frontend");
    assert_eq!(ts_res.working_dir, root);
    assert_eq!(ts_res.command, "npx tsc --noEmit");

    // 3. Test PatchVerifier with dry-run and mock custom command
    let patch_target = format!("{}:greet", rs_file.to_string_lossy());
    let new_code = "pub fn greet(name: &str) -> String {\n    format!(\"Hi, {}!\", name)\n}";
    let verify_res = PatchVerifier::verify_patch(
        root,
        &patch_target,
        new_code,
        None, // auto-detect
        true, // dry-run
    )
    .expect("verify_patch should execute");

    // In a dry-run without cargo binary running or returning mock, verify command was populated
    assert_eq!(verify_res.typechecker_command, Some(rust_res.command));
    assert!(verify_res.diff.contains("+    format!(\"Hi, {}!\", name)"));
}

#[test]
fn test_typescript_turborepo_monorepo_discovery() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Setup monorepo layout:
    // root/
    //   package.json (root workspace)
    //   packages/
    //     ui/
    //       tsconfig.json
    //       src/
    //         Button.tsx
    //     core/
    //       package.json
    //       src/
    //         utils.ts
    fs::write(root.join("package.json"), "{\"workspaces\":[\"packages/*\"]}").unwrap();

    let ui_dir = root.join("packages").join("ui");
    fs::create_dir_all(ui_dir.join("src")).unwrap();
    let ui_tsconfig = ui_dir.join("tsconfig.json");
    fs::write(&ui_tsconfig, "{\"compilerOptions\":{}}").unwrap();
    let ui_file = ui_dir.join("src").join("Button.tsx");
    fs::write(
        &ui_file,
        "export function Button() {\n    return <button>Click</button>;\n}\n",
    )
    .unwrap();

    let core_dir = root.join("packages").join("core");
    fs::create_dir_all(core_dir.join("src")).unwrap();
    let core_pkg = core_dir.join("package.json");
    fs::write(&core_pkg, "{\"name\":\"@mono/core\"}").unwrap();
    let core_file = core_dir.join("src").join("utils.ts");
    fs::write(
        &core_file,
        "export function add(a: number, b: number): number {\n    return a + b;\n}\n",
    )
    .unwrap();

    // 1. Check UI package resolves to packages/ui with tsconfig.json
    let ui_res = TypecheckerDetector::detect_resolution(root, &ui_file, SupportedLanguage::TypeScript)
        .expect("Should resolve packages/ui tsconfig");
    assert_eq!(ui_res.working_dir, ui_dir);
    assert_eq!(ui_res.manifest_path, Some(ui_tsconfig));
    assert_eq!(ui_res.command, "npx tsc --noEmit");

    // 2. Check Core package resolves to packages/core with package.json
    let core_res = TypecheckerDetector::detect_resolution(root, &core_file, SupportedLanguage::TypeScript)
        .expect("Should resolve packages/core package.json");
    assert_eq!(core_res.working_dir, core_dir);
    assert_eq!(core_res.manifest_path, Some(core_pkg));
    assert_eq!(core_res.command, "npx tsc --noEmit");

    // 3. BatchAstPatcher dry run across monorepo packages
    let req = PatchTransactionRequest {
        workspace_root: Some(root.to_path_buf()),
        patches: vec![
            SymbolPatchUnit {
                file_path: ui_file.clone(),
                symbol_query: "Button".to_string(),
                replacement_code: "export function Button() {\n    return <button className=\"btn\">Click</button>;\n}".to_string(),
            },
            SymbolPatchUnit {
                file_path: core_file.clone(),
                symbol_query: "add".to_string(),
                replacement_code: "export function add(a: number, b: number): number {\n    return (a + b) | 0;\n}".to_string(),
            },
        ],
        typechecker: None,
        apply: false,
        timeout_ms: Some(5000),
    };

    let batch_res = BatchAstPatcher::apply_transaction(&req).unwrap();
    assert_eq!(batch_res.files_modified_count, 2);
    assert_eq!(batch_res.symbols_patched_count, 2);
    assert!(batch_res.rolled_back);
    assert!(batch_res.typechecker_command.is_some());
}

#[test]
fn test_go_multi_module_monorepo_discovery() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Setup Go multi-module monorepo:
    // root/
    //   services/
    //     auth/
    //       go.mod
    //       cmd/main.go
    //     billing/
    //       go.mod
    //       cmd/main.go
    let auth_dir = root.join("services").join("auth");
    fs::create_dir_all(auth_dir.join("cmd")).unwrap();
    let auth_mod = auth_dir.join("go.mod");
    fs::write(&auth_mod, "module company.com/auth\n\ngo 1.22\n").unwrap();
    let auth_main = auth_dir.join("cmd").join("main.go");
    fs::write(&auth_main, "package main\n\nfunc main() {}\n").unwrap();

    let billing_dir = root.join("services").join("billing");
    fs::create_dir_all(billing_dir.join("cmd")).unwrap();
    let billing_mod = billing_dir.join("go.mod");
    fs::write(&billing_mod, "module company.com/billing\n\ngo 1.22\n").unwrap();
    let billing_main = billing_dir.join("cmd").join("main.go");
    fs::write(&billing_main, "package main\n\nfunc main() {}\n").unwrap();

    let auth_res = TypecheckerDetector::detect_resolution(root, &auth_main, SupportedLanguage::Go)
        .expect("Should resolve auth go.mod");
    assert_eq!(auth_res.working_dir, auth_dir);
    assert_eq!(auth_res.manifest_path, Some(auth_mod));
    assert_eq!(auth_res.command, "go vet ./...");

    let billing_res = TypecheckerDetector::detect_resolution(root, &billing_main, SupportedLanguage::Go)
        .expect("Should resolve billing go.mod");
    assert_eq!(billing_res.working_dir, billing_dir);
    assert_eq!(billing_res.manifest_path, Some(billing_mod));
    assert_eq!(billing_res.command, "go vet ./...");
}

#[test]
fn test_python_polyglot_and_services_discovery() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Setup Python multi-service repo:
    // root/
    //   services/
    //     api/
    //       pyproject.toml
    //       app.py
    //     worker/
    //       mypy.ini
    //       tasks.py
    //   scripts/
    //     deploy.py (no manifest)
    let api_dir = root.join("services").join("api");
    fs::create_dir_all(&api_dir).unwrap();
    let api_toml = api_dir.join("pyproject.toml");
    fs::write(&api_toml, "[tool.mypy]\nstrict = true\n").unwrap();
    let api_file = api_dir.join("app.py");
    fs::write(&api_file, "def start(): pass\n").unwrap();

    let worker_dir = root.join("services").join("worker");
    fs::create_dir_all(&worker_dir).unwrap();
    let worker_ini = worker_dir.join("mypy.ini");
    fs::write(&worker_ini, "[mypy]\n").unwrap();
    let worker_file = worker_dir.join("tasks.py");
    fs::write(&worker_file, "def run(): pass\n").unwrap();

    let script_file = root.join("scripts").join("deploy.py");
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(&script_file, "print('Deploying')\n").unwrap();

    let api_res = TypecheckerDetector::detect_resolution(root, &api_file, SupportedLanguage::Python)
        .expect("Should resolve api pyproject.toml");
    assert_eq!(api_res.working_dir, api_dir);
    assert_eq!(api_res.manifest_path, Some(api_toml));
    assert_eq!(api_res.command, "mypy \"app.py\"");

    let worker_res = TypecheckerDetector::detect_resolution(root, &worker_file, SupportedLanguage::Python)
        .expect("Should resolve worker mypy.ini");
    assert_eq!(worker_res.working_dir, worker_dir);
    assert_eq!(worker_res.manifest_path, Some(worker_ini));
    assert_eq!(worker_res.command, "mypy \"tasks.py\"");

    let script_res = TypecheckerDetector::detect_resolution(root, &script_file, SupportedLanguage::Python)
        .expect("Should resolve scripts fallback");
    assert_eq!(script_res.working_dir, root);
    assert_eq!(script_res.manifest_path, None);
    assert!(script_res.command.contains("python -m py_compile"));
    assert!(script_res.command.contains("deploy.py"));
}

#[test]
fn test_csharp_multi_project_sln_discovery() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // root/
    //   Company.sln
    //   src/
    //     Orders/
    //       Orders.csproj
    //       OrderService.cs
    fs::write(root.join("Company.sln"), "Microsoft Visual Studio Solution File").unwrap();
    let orders_dir = root.join("src").join("Orders");
    fs::create_dir_all(&orders_dir).unwrap();
    let orders_csproj = orders_dir.join("Orders.csproj");
    fs::write(&orders_csproj, "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>").unwrap();
    let order_service = orders_dir.join("OrderService.cs");
    fs::write(&order_service, "public class OrderService {}").unwrap();

    let res = TypecheckerDetector::detect_resolution(root, &order_service, SupportedLanguage::CSharp)
        .expect("Should resolve closest csproj");
    assert_eq!(res.working_dir, orders_dir);
    assert_eq!(res.manifest_path, Some(orders_csproj));
    assert_eq!(res.command, "dotnet build");
}

#[test]
fn test_custom_typechecker_override_preserves_discovered_cwd() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let tauri_dir = root.join("src-tauri");
    fs::create_dir_all(tauri_dir.join("src")).unwrap();
    fs::write(tauri_dir.join("Cargo.toml"), "[package]\nname = \"tauri\"\n").unwrap();
    let rs_file = tauri_dir.join("src").join("main.rs");
    fs::write(
        &rs_file,
        "pub fn compute(x: i32) -> i32 {\n    x * 2\n}\n",
    )
    .unwrap();

    // Verify custom typechecker override with PatchVerifier
    let patch_target = format!("{}:compute", rs_file.to_string_lossy());
    let new_code = "pub fn compute(x: i32) -> i32 {\n    x * 4\n}";

    let res = PatchVerifier::verify_patch(
        root,
        &patch_target,
        new_code,
        Some("cargo clippy --fix"), // custom override
        true,
    )
    .expect("verify_patch with override should succeed");

    assert_eq!(res.typechecker_command, Some("cargo clippy --fix".to_string()));
    assert!(res.diff.contains("+    x * 4"));
}
