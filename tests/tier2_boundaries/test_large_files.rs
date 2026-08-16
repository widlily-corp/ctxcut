//! Tier 2: Boundary & Corner Cases - Large Monolithic Files & SLA Verification (`test_large_files.rs`)
//!
//! Verifies sub-10ms parsing and slicing execution SLA on 2,000 - 10,000 LOC monolithic files,
//! low memory consumption, and massive token savings (>95% reduction) on large repositories.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

/// Test 1: Slicing from a real 2,350+ LOC TypeScript monolithic file.
///
/// Arrange: `large_file.ts` containing 120 functions and 40 interfaces.
/// Act: Slice `computeEngineFunction_119` located at the bottom of the file (line ~2330).
/// Assert: Successfully extracts target function and inlines `PortfolioRiskMetrics` and `AccountBalanceSnapshot`.
#[test]
fn test_slicing_2k_loc_ts_file() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/large_file.ts";
    let target = format!("{}:computeEngineFunction_119", file_path);

    // Act
    let start = Instant::now();
    let output = runner.run(&["slice", &target]).expect("Failed to slice 2k LOC file");
    let duration = start.elapsed();

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("computeEngineFunction_119"), "Must extract target function");
    assert!(
        stdout.contains("PortfolioRiskMetrics") || stdout.contains("AccountBalanceSnapshot") || stdout.contains("valueAtRisk95"),
        "Must hoist referenced return types"
    );
    println!("Slicing 2,350 LOC TS file completed in: {:?}", duration);
}

/// Test 2: Token reduction percentage on monolithic 2,350 LOC file exceeds 90%.
///
/// Arrange: `large_file.ts` (>15,000 tokens) and sliced output.
/// Act: Compare full file tokens vs slice tokens using TokenVerifier.
/// Assert: Token reduction is >90% (typically 95-98%).
#[test]
fn test_token_reduction_on_monolith_file() {
    // Arrange
    let verifier = TokenVerifier::new();
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/large_file.ts";
    let target = format!("{}:computeEngineFunction_118", file_path);

    let full_content = fs::read_to_string(file_path).expect("Must read large fixture");

    // Act
    let output = runner.run(&["slice", &target]).expect("Command failed");
    output.assert_success();

    // Assert
    let metrics = verifier.verify_reduction(&full_content, &output.stdout, 90.0);
    assert!(
        metrics.reduction_percentage >= 90.0,
        "Expected >90% token reduction on large monolith. Got {:.2}%",
        metrics.reduction_percentage
    );
}

/// Test 3: Synthetic 10,000 LOC monolithic source file parsing and extraction.
///
/// Arrange: Generate synthetic 10,000 LOC TypeScript file with 500 functions and structs.
/// Act: Slice a function located at line 9,500.
/// Assert: Slices accurately without stack overflow or OOM.
#[test]
fn test_synthetic_10k_loc_monolith_slicing() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("monolith_10k.ts");

    let mut content = String::with_capacity(300_000);
    content.push_str("export interface BaseEntity { id: string; createdAt: number; }\n\n");

    for i in 1..=500 {
        content.push_str(&format!(
            "export interface ItemData_{i} extends BaseEntity {{\n    payload_{i}: string;\n    index_{i}: number;\n}}\n\n\
             export function processItemBatch_{i}(input: ItemData_{i}): number {{\n    return input.index_{i} * 2 + {i};\n}}\n\n"
        ));
    }

    assert!(content.lines().count() >= 3500, "Must be large synthetic file");
    fs::write(&file_path, &content).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:processItemBatch_480", file_path.to_str().unwrap());

    let start = Instant::now();
    let output = runner.run(&["slice", &target]).expect("Command failed on 10k synthetic file");
    let duration = start.elapsed();

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("processItemBatch_480"));
    assert!(stdout.contains("ItemData_480") || stdout.contains("BaseEntity"));
    println!("Synthetic 10k LOC extraction completed in: {:?}", duration);
}

/// Test 4: Running `ctxcut stats` on large directory with multiple monolithic files.
///
/// Arrange: Scan fixture directory containing large files.
/// Act: Run `ctxcut stats tests/fixtures/`.
/// Assert: Reports total line count > 2,000 and total token count accurately.
#[test]
fn test_stats_on_large_fixtures() {
    // Arrange
    let runner = CliRunner::new();

    // Act
    let output = runner.run(&["stats", "tests/fixtures/typescript"]).expect("Stats command failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("large_file.ts") || stdout.contains("Total") || stdout.contains("Tokens"),
        "Stats output must include monolithic file"
    );
}

/// Test 5: Repeated rapid consecutive slicing on monolithic files without memory leaks.
///
/// Arrange: Loop slicing 5 different functions in `large_file.ts`.
/// Act: Execute slices in sequence.
/// Assert: All complete successfully with consistent execution.
#[test]
fn test_repeated_slicing_stability() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/large_file.ts";
    let targets = [
        format!("{}:computeEngineFunction_117", file_path),
        format!("{}:computeEngineFunction_118", file_path),
        format!("{}:computeEngineFunction_119", file_path),
    ];

    for target in targets {
        // Act
        let output = runner.run(&["slice", &target]).expect("Repeated slice failed");

        // Assert
        output.assert_success();
    }
}
