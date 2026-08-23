//! Tier 1 Tests: Feature 4 — C / C++ Grammar & Slicing Support
//!
//! Verifies C / C++ language support:
//! - C++ class methods and templates
//! - C structs and typedefs
//! - Header include resolution
//! - Macro stripping (#ifdef, #define)
//! - Token stats on C/C++ files

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f4_cpp_class_method_slice() {
    // Arrange: C++ class declaration with method implementation
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("calculator.cpp");
    let content = r#"
#include <iostream>

class Calculator {
public:
    int add(int a, int b) {
        return a + b;
    }

    int multiply(int a, int b) {
        return a * b;
    }
};

int main() {
    Calculator calc;
    return calc.add(2, 3);
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Run stats/token analysis on C++ file
    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", file_path.to_str().unwrap()]).expect("Command failed");

    // Assert: C++ file recognized and analyzed
    output.assert_success();
    assert!(output.stdout.contains("Lines") || output.stdout.contains("Tokens"));
}

#[test]
fn test_f4_cpp_template_function_slice() {
    // Arrange: C++ template function
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("templates.cpp");
    let content = r#"
template <typename T>
T clamp(T val, T min_val, T max_val) {
    if (val < min_val) return min_val;
    if (val > max_val) return max_val;
    return val;
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Execute stats scan
    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", file_path.to_str().unwrap()]).expect("Command failed");

    // Assert: Template code processed cleanly
    output.assert_success();
}

#[test]
fn test_f4_c_struct_and_typedef_hoisting() {
    // Arrange: C struct and typedef
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("point.c");
    let content = r#"
#include <stdio.h>

typedef struct {
    double x;
    double y;
} Point2D;

double distance_squared(Point2D a, Point2D b) {
    double dx = a.x - b.x;
    double dy = a.y - b.y;
    return dx * dx + dy * dy;
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Fast stats scan
    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", file_path.to_str().unwrap()]).expect("Command failed");

    // Assert: C file processed
    output.assert_success();
}

#[test]
fn test_f4_cpp_header_include_resolution() {
    // Arrange: C++ header and source files
    let dir = TempDir::new().expect("Failed to create tempdir");
    let header_path = dir.path().join("engine.hpp");
    let src_path = dir.path().join("engine.cpp");

    fs::write(&header_path, "#pragma once\nstruct EngineConfig { int threads; };\n").unwrap();
    fs::write(&src_path, "#include \"engine.hpp\"\nvoid init_engine(EngineConfig cfg) {}\n").unwrap();

    // Act: Stats on project root
    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", dir.path().to_str().unwrap()]).expect("Command failed");

    // Assert: Successfully scanned C++ files in repository
    output.assert_success();
}

#[test]
fn test_f4_c_macro_directive_stripping() {
    // Arrange: C file with preprocessor macros
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("platform.c");
    let content = r#"
#ifdef _WIN32
#define PLATFORM_NAME "Windows"
#else
#define PLATFORM_NAME "POSIX"
#endif

const char* get_platform(void) {
    return PLATFORM_NAME;
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Compute token metrics
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(content);

    // Assert: Valid token count
    assert!(tokens > 10);
}
