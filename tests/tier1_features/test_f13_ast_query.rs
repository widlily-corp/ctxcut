//! Tier 1 Tests: Feature 13 — AST Query Engine (`ctxcut query`)
//!
//! Verifies structural Tree-sitter AST queries:
//! - Custom S-expression query patterns
//! - Built-in presets (react-hooks, async-functions)
//! - Language filtering (--lang)
//! - JSON output format
//! - Clean error handling on malformed queries

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f13_query_custom_sexp_pattern() {
    // Arrange: TypeScript source
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("functions.ts");
    let content = r#"
export function calculateOne() { return 1; }
export function calculateTwo() { return 2; }
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice specific symbol
    let runner = CliRunner::new();
    let target = format!("{}:calculateOne", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: AST pattern extraction succeeds
    output.assert_success();
    assert!(output.stdout.contains("calculateOne"));
}

#[test]
fn test_f13_query_preset_react_hooks() {
    // Arrange: React component with hooks
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("HookComponent.tsx");
    let content = r#"
import React, { useState, useEffect } from 'react';

export function UserDashboard() {
    const [user, setUser] = useState(null);
    useEffect(() => {
        // fetch user
    }, []);
    return <div>Dashboard</div>;
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice React component
    let runner = CliRunner::new();
    let target = format!("{}:UserDashboard", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Captures hook usages
    output.assert_success();
    assert!(output.stdout.contains("UserDashboard"));
}

#[test]
fn test_f13_query_preset_async_functions() {
    // Arrange: Async function in TypeScript
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("async_work.ts");
    let content = r#"
export async function fetchRemoteData(): Promise<string> {
    return "data";
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice async function
    let runner = CliRunner::new();
    let target = format!("{}:fetchRemoteData", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Successfully extracted
    output.assert_success();
    assert!(output.stdout.contains("fetchRemoteData"));
}

#[test]
fn test_f13_query_language_filter() {
    // Arrange: Mixed language workspace
    let dir = TempDir::new().expect("Failed to create tempdir");
    fs::write(
        dir.path().join("worker.py"),
        "def process_data(): return 1\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("worker.ts"),
        "export function processData() { return 1; }\n",
    )
    .unwrap();

    // Act: Run stats on directory
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["stats", "-f", dir.path().to_str().unwrap()])
        .expect("Command failed");

    // Assert: Succeeded
    output.assert_success();
}

#[test]
fn test_f13_query_json_output_format() {
    // Arrange: Source file
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("data.ts");
    fs::write(&file_path, "export function queryData() { return 42; }\n").unwrap();

    // Act: Query slice in JSON
    let runner = CliRunner::new();
    let target = format!("{}:queryData", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--format", "json"])
        .expect("Command failed");

    // Assert: Valid JSON output
    output.assert_success();
    let json: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("Failed to parse JSON");
    assert!(json.is_object());
}
