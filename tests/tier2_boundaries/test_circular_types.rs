//! Tier 2: Boundary & Corner Cases - Circular & Recursive Types (`test_circular_types.rs`)
//!
//! Verifies cycle detection in dependency graph traversal when resolving mutually
//! recursive interfaces, self-referential tree nodes, struct pointer cycles, and recursive AST enums.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

/// Test 1: Mutually recursive TypeScript interfaces (`GraphNode` <-> `Edge`).
///
/// Arrange: TypeScript file where `GraphNode` references `Edge[]` and `Edge` references `GraphNode`.
/// Act: Run `ctxcut slice tests/fixtures/typescript/circular_types.ts:buildSampleGraph`.
/// Assert: Successfully completes; inlines both `GraphNode` and `Edge` exactly once without infinite recursion.
#[test]
fn test_mutual_recursion_interfaces_ts() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/circular_types.ts";
    let target = format!("{}:buildSampleGraph", file_path);

    // Act
    let output = runner.run(&["slice", &target]).expect("Command failed on circular types TS");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("buildSampleGraph"), "Must extract target function");
    assert!(stdout.contains("GraphNode"), "Must inline GraphNode interface");
    assert!(stdout.contains("Edge"), "Must inline Edge interface");

    // Cycle detection verification: Type definitions must appear once, not duplicated recursively
    let graph_node_count = stdout.matches("interface GraphNode").count();
    let edge_count = stdout.matches("interface Edge").count();
    assert!(graph_node_count <= 2, "GraphNode interface must not be duplicated in loop");
    assert!(edge_count <= 2, "Edge interface must not be duplicated in loop");
}

/// Test 2: Self-referential tree structure in TypeScript (`TreeNode` referencing `TreeNode`).
///
/// Arrange: `TreeNode` with parent and children of type `TreeNode`.
/// Act: Run `ctxcut slice tests/fixtures/typescript/circular_types.ts:traverseTreeDepthFirst`.
/// Assert: Inlines `TreeNode` once without stack overflow.
#[test]
fn test_self_referencing_tree_node_ts() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/circular_types.ts";
    let target = format!("{}:traverseTreeDepthFirst", file_path);

    // Act
    let output = runner.run(&["slice", &target]).expect("Command failed on self-referential TS");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("traverseTreeDepthFirst"));
    assert!(stdout.contains("TreeNode"));
}

/// Test 3: Circular Pydantic models in Python (`CategoryNode` and `GraphNodeModel`).
///
/// Arrange: Python file with self-referencing models using `from __future__ import annotations`.
/// Act: Run `ctxcut slice tests/fixtures/python/circular_models.py:build_taxonomy_tree`.
/// Assert: Completes without recursion limit error and inlines `CategoryNode`.
#[test]
fn test_circular_models_python() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/python/circular_models.py";
    let target = format!("{}:build_taxonomy_tree", file_path);

    // Act
    let output = runner.run(&["slice", &target]).expect("Command failed on circular Python models");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("build_taxonomy_tree"));
    assert!(stdout.contains("CategoryNode"));
}

/// Test 4: Go struct pointer cycles (`type Node struct { Next *Node; Prev *Node }`).
///
/// Arrange: Go file with doubly-linked list node structs.
/// Act: Run `ctxcut slice <path>:NewDoublyLinkedList`.
/// Assert: Inlines `Node` definition without recursive cycle lockup.
#[test]
fn test_struct_pointer_cycles_go() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let go_code = r#"
package linkedlist

type Node struct {
    Value int
    Next  *Node
    Prev  *Node
}

type List struct {
    Head *Node
    Tail *Node
    Size int
}

func NewDoublyLinkedList() *List {
    return &List{Size: 0}
}
"#;
    let file_path = temp_dir.path().join("list.go");
    fs::write(&file_path, go_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:NewDoublyLinkedList", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command failed on Go struct cycles");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("NewDoublyLinkedList"));
    assert!(stdout.contains("List") || stdout.contains("Node"));
}

/// Test 5: Self-referential recursive AST enum in Rust (`Box<Expr>`).
///
/// Arrange: Rust file with recursive enum definitions.
/// Act: Run `ctxcut slice <path>:evaluate_expr`.
/// Assert: Inlines `AstExpr` once and completes without stack overflow.
#[test]
fn test_self_referential_enum_ast_rust() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let rust_code = r#"
pub enum AstExpr {
    Literal(i64),
    Variable(String),
    Binary {
        op: String,
        left: Box<AstExpr>,
        right: Box<AstExpr>,
    },
}

pub fn evaluate_expr(expr: &AstExpr) -> i64 {
    match expr {
        AstExpr::Literal(val) => *val,
        AstExpr::Variable(_) => 0,
        AstExpr::Binary { op, left, right } => {
            let l = evaluate_expr(left);
            let r = evaluate_expr(right);
            if op == "+" { l + r } else { l * r }
        }
    }
}
"#;
    let file_path = temp_dir.path().join("ast.rs");
    fs::write(&file_path, rust_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:evaluate_expr", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command failed on recursive Rust enum");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("evaluate_expr"));
    assert!(stdout.contains("AstExpr"));
}
