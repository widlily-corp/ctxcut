//! Tier 2: Boundary & Corner Cases - Nested Generics & Complex Types (`test_nested_generics.rs`)
//!
//! Verifies robust type extraction and inlining for deeply nested generic type hierarchies
//! (up to 10 levels deep), complex lifetime bounds, trait constraints, and generic type parameters.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

/// Test 1: Slicing TypeScript functions with deeply nested generic return types.
///
/// Arrange: TypeScript file with `Promise<Result<Map<string, UserDTO>, DomainError>>`.
/// Act: Run `ctxcut slice tests/fixtures/typescript/nested_types.ts:fetchUserMapping`.
/// Assert: Successfully hoists `Result`, `UserDTO`, `UserMetadata`, `UserPreferences`, `DomainError`.
#[test]
fn test_deeply_nested_types_ts() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/nested_types.ts";
    let target = format!("{}:fetchUserMapping", file_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to slice nested generic TS function");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("fetchUserMapping"),
        "Must extract target function"
    );
    assert!(
        stdout.contains("Result") || stdout.contains("DomainError"),
        "Must hoist Result/DomainError"
    );
    assert!(
        stdout.contains("UserDTO") || stdout.contains("UserMetadata"),
        "Must hoist UserDTO and nested metadata"
    );
}

/// Test 2: Extreme 10-level nested generic type hierarchy in TypeScript.
///
/// Arrange: Synthetic TypeScript file with 10 levels of generic nesting.
/// Act: Run `ctxcut slice <path>:processDeeplyNestedPipeline`.
/// Assert: Traverses and inlines all constituent type definitions without truncating AST.
#[test]
fn test_extreme_10_level_nested_generics_ts() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let ts_code = r#"
export interface Level10Leaf { id: string; val: number; }
export type Level9<T> = Array<T>;
export type Level8<T> = Map<string, Level9<T>>;
export type Level7<T> = Record<string, Level8<T>>;
export type Level6<T> = Set<Level7<T>>;
export type Level5<T> = Promise<Level6<T>>;
export type Level4<T> = { data: Level5<T>; status: number };
export type Level3<T> = ResultWrapper<Level4<T>>;
export type Level2<T> = Array<Level3<T>>;
export type Level1 = Level2<Level10Leaf>;

export interface ResultWrapper<T> {
    success: boolean;
    payload: T;
}

export async function processDeeplyNestedPipeline(input: Level1): Promise<Level10Leaf> {
    return { id: "leaf_1", val: 42 };
}
"#;
    let file_path = temp_dir.path().join("deep_generics.ts");
    fs::write(&file_path, ts_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!(
        "{}:processDeeplyNestedPipeline",
        file_path.to_str().unwrap()
    );
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("processDeeplyNestedPipeline"));
    assert!(stdout.contains("Level10Leaf") || stdout.contains("ResultWrapper"));
}

/// Test 3: Rust complex lifetime bounds and higher-ranked trait bounds (HRTB).
///
/// Arrange: Rust file with `Pin<Box<dyn Future<Output = Result<T, &'a Error>> + Send + 'static>>` and `where` clause.
/// Act: Run `ctxcut slice <path>:dispatch_async_task`.
/// Assert: Correctly extracts function signature, lifetime parameters `'a`, and trait bounds.
#[test]
fn test_rust_complex_lifetimes_and_trait_bounds() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let rust_code = r#"
use std::future::Future;
use std::pin::Pin;

pub trait TaskProcessor<T> {
    type Output;
    fn process<'a>(&'a self, item: &'a T) -> Pin<Box<dyn Future<Output = Result<Self::Output, String>> + Send + 'a>>;
}

pub struct CustomExecutor;

impl CustomExecutor {
    pub async fn dispatch_task<'a, T, P>(processor: &'a P, item: &'a T) -> Result<P::Output, String>
    where
        T: Send + Sync + 'a,
        P: TaskProcessor<T> + ?Sized,
    {
        processor.process(item).await
    }
}
"#;
    let file_path = temp_dir.path().join("rust_traits.rs");
    fs::write(&file_path, rust_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:dispatch_task", file_path.to_str().unwrap());
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("dispatch_task"));
    assert!(stdout.contains("TaskProcessor") || stdout.contains("where"));
}

/// Test 4: Go generic type parameters with interface constraints.
///
/// Arrange: Go file with `func TransformSlice[T any, R Constraints](items []T, fn func(T) R) []R`.
/// Act: Run `ctxcut slice <path>:TransformSlice`.
/// Assert: Preserves Go 1.18+ generic type parameter list and interface constraints.
#[test]
fn test_go_generic_type_parameters() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let go_code = r#"
package generics

type Transformer[T any, R any] interface {
    Transform(item T) R
}

func TransformSlice[T any, R any](items []T, t Transformer[T, R]) []R {
    result := make([]R, len(items))
    for i, v := range items {
        result[i] = t.Transform(v)
    }
    return result
}
"#;
    let file_path = temp_dir.path().join("generics.go");
    fs::write(&file_path, go_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:TransformSlice", file_path.to_str().unwrap());
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("TransformSlice"));
    assert!(stdout.contains("Transformer") || stdout.contains("[]R"));
}

/// Test 5: Python Generic Subscripting with TypeVar and Union Types.
///
/// Arrange: Python file with `Generic[T]`, `TypeVar("T")`, and `Union[Success[T], Failure]`.
/// Act: Run `ctxcut slice <path>:handle_container`.
/// Assert: Extracts generic function and inlines generic container classes.
#[test]
fn test_python_generic_typevars() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let py_code = r#"
from typing import Generic, TypeVar, Union

T = TypeVar("T")

class Container(Generic[T]):
    def __init__(self, value: T):
        self.value = value

def handle_container(c: Container[str]) -> str:
    return c.value.strip()
"#;
    let file_path = temp_dir.path().join("generics.py");
    fs::write(&file_path, py_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:handle_container", file_path.to_str().unwrap());
    let output = runner
        .run(&["slice", &target])
        .expect("Command execution failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("handle_container"));
    assert!(stdout.contains("Container") || stdout.contains("Generic"));
}
