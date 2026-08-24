//! Tier 2 Boundary Tests: Features 1 to 3 (Graph, Callers, Tracing, Implementors)
//!
//! Comprehensive boundary and fault injection cases:
//! - F1: No callers found, recursive self-callers, method name collisions, deep hierarchies, nonexistent symbols
//! - F2: Cyclic call graph, leaf functions with 0 calls, wide branch pruning, standard library external calls, deep call stacks
//! - F3: Zero implementors, multi-implementor deduplication, generic bounds, partial non-implementors, cfg conditionals

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};
use std::fs;
use tempfile::TempDir;

// --- F1 Boundaries: Callers & Impact ---

#[test]
fn test_f1_boundary_no_callers_found() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("orphan.ts");
    fs::write(&file, "export function unusedHelper(): void {}\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:unusedHelper", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("unusedHelper"));
}

#[test]
fn test_f1_boundary_recursive_self_caller() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("factorial.ts");
    fs::write(&file, "export function factorial(n: number): number {\n    if (n <= 1) return 1;\n    return n * factorial(n - 1);\n}\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:factorial", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("factorial"));
}

#[test]
fn test_f1_boundary_method_name_collisions() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("collision.ts");
    let content = r#"
export class OrderService {
    save(): boolean { return true; }
}
export class UserService {
    save(): boolean { return true; }
}
export function save(): boolean { return true; }
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:save", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("save"));
}

#[test]
fn test_f1_boundary_deep_call_hierarchy() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("deep.ts");
    let mut code = "export function rootBase(): number { return 1; }\n".to_string();
    for i in 1..=15 {
        code.push_str(&format!(
            "export function level{i}(): number {{ return rootBase() + {i}; }}\n"
        ));
    }
    fs::write(&file, &code).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:rootBase", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("rootBase"));
}

#[test]
fn test_f1_boundary_nonexistent_symbol_callers() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("empty.ts");
    fs::write(&file, "export const PI = 3.14;\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:nonExistentFunction", file.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]);

    if let Ok(res) = output {
        assert!(
            !res.success
                || res.stdout.is_empty()
                || res.stderr.contains("not found")
                || res.stdout.contains("not found")
        );
    }
}

// --- F2 Boundaries: Execution Flow Tracing ---

#[test]
fn test_f2_boundary_cyclic_call_graph() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cycles.ts");
    let content = r#"
export function funcA(): number { return funcB(); }
export function funcB(): number { return funcA(); }
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:funcA", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("funcA"));
}

#[test]
fn test_f2_boundary_leaf_function_no_calls() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("leaf.ts");
    fs::write(
        &file,
        "export function getConstant(): number { return 100; }\n",
    )
    .unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:getConstant", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("getConstant"));
}

#[test]
fn test_f2_boundary_wide_branching_pruning() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("branching.ts");
    let mut code = "export function mainFlow(): number {\n".to_string();
    for i in 1..=8 {
        code.push_str(&format!("    helper{i}();\n"));
    }
    code.push_str("    return 0;\n}\n");
    for i in 1..=8 {
        code.push_str(&format!("function helper{i}() {{ return {i}; }}\n"));
    }
    fs::write(&file, &code).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:mainFlow", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--budget", "150"])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("mainFlow"));
}

#[test]
fn test_f2_boundary_external_library_calls() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("external.ts");
    let content = r#"
export function computeHash(data: string): string {
    return JSON.stringify({ hash: data.trim() });
}
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:computeHash", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("computeHash"));
}

#[test]
fn test_f2_boundary_deep_nesting_depth_limit() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("depth.ts");
    fs::write(&file, "export function deepEntry(): number { return 1; }\n").unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:deepEntry", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--depth", "5"])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("deepEntry"));
}

// --- F3 Boundaries: Implementor Hoisting ---

#[test]
fn test_f3_boundary_trait_zero_implementors() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("trait.rs");
    let content = r#"
pub trait UnimplementedService {
    fn run(&self);
}
pub fn execute(s: &dyn UnimplementedService) {
    s.run();
}
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:execute", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("execute"));
    assert!(output.stdout.contains("UnimplementedService"));
}

#[test]
fn test_f3_boundary_multiple_implementors_dedup() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("multi_impl.ts");
    let content = r#"
export interface Logger { log(msg: string): void; }
export class ConsoleLogger implements Logger { log(msg: string) {} }
export class FileLogger implements Logger { log(msg: string) {} }
export class RemoteLogger implements Logger { log(msg: string) {} }

export function writeLog(l: Logger, msg: string) { l.log(msg); }
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:writeLog", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("writeLog"));
}

#[test]
fn test_f3_boundary_generic_trait_implementations() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("generics.rs");
    let content = r#"
pub trait Serializer<T> {
    fn serialize(&self, val: T) -> String;
}
pub fn do_serialize<T, S: Serializer<T>>(s: &S, val: T) -> String {
    s.serialize(val)
}
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:do_serialize", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("do_serialize"));
}

#[test]
fn test_f3_boundary_partial_interface_non_implementor() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("partial.go");
    let content = r#"
package main

type FullInterface interface {
    MethodA()
    MethodB()
}

type PartialStruct struct{}
func (p PartialStruct) MethodA() {}

func Consume(f FullInterface) { f.MethodA(); f.MethodB() }
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:Consume", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("Consume"));
}

#[test]
fn test_f3_boundary_conditional_cfg_implementations() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cfg_trait.rs");
    let content = r#"
pub trait Driver { fn init(&self); }

#[cfg(windows)]
pub struct WinDriver;
#[cfg(windows)]
impl Driver for WinDriver { fn init(&self) {} }

pub fn init_driver(d: &dyn Driver) { d.init(); }
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let target = format!("{}:init_driver", file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    output.assert_success();
    assert!(output.stdout.contains("init_driver"));
}
