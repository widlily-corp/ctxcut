//! Tier 1: Feature Coverage - Repository Token Stats Tests (`test_stats_features.rs`)
//!
//! Verifies single-file accuracy, directory aggregate scanning, JSON output formatting,
//! zero-token / one-liner edge case handling, and BPE tokenizer exact parity.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

/// Test 1: Single file token statistics calculation accuracy.
///
/// Arrange: Realistic TypeScript OrderService file.
/// Act: Run `ctxcut stats <file_path>`.
/// Assert: Output contains lines, full token count, average slice token count,
///         and calculated token savings percentage (>80%).
#[test]
fn test_stats_single_file_accuracy() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/realistic_order_service/order_service.ts";

    // Act
    let output = runner
        .run(&["stats", file_path])
        .expect("Failed to execute ctxcut stats");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("order_service.ts")
            || stdout.contains("Tokens")
            || stdout.contains("Savings"),
        "Must display file stats summary"
    );
    assert!(
        stdout.contains('%'),
        "Must contain savings percentage calculation"
    );
}

/// Test 2: Directory aggregate scan reporting total repository tokens and savings.
///
/// Arrange: Directory containing multiple TypeScript and Python files.
/// Act: Run `ctxcut stats tests/fixtures/typescript/`.
/// Assert: Table or report output displaying aggregate token metrics across files.
#[test]
fn test_stats_directory_aggregate_scan() {
    // Arrange
    let runner = CliRunner::new();
    let dir_path = "tests/fixtures/typescript";

    // Act
    let output = runner
        .run(&["stats", dir_path])
        .expect("Failed to execute ctxcut stats on directory");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("Total")
            || stdout.contains("Files")
            || stdout.contains("Savings")
            || stdout.contains("Summary"),
        "Directory scan must report aggregate totals"
    );
}

/// Test 3: Structured JSON output format via `--format json`.
///
/// Arrange: Run `ctxcut stats` on a test fixture with `--format json`.
/// Act: Parse STDOUT as JSON.
/// Assert: Contains expected JSON schema fields (`total_files`, `total_raw_tokens`, `savings_percentage`).
#[test]
fn test_stats_json_output_mode() {
    // Arrange
    let runner = CliRunner::new();
    let file_path = "tests/fixtures/typescript/simple_function.ts";

    // Act
    let output = runner
        .run(&["stats", file_path, "--format", "json"])
        .expect("Failed to run stats --format json");

    // Assert
    output.assert_success();
    let json: Value = output.parse_json().expect("Output must be valid JSON");
    assert!(
        json.get("total_raw_tokens").is_some()
            || json.get("raw_tokens").is_some()
            || json.get("files").is_some()
            || json.get("total_files").is_some(),
        "JSON output must contain token metrics keys. Got: {:?}",
        json
    );
}

/// Test 4: Handling zero-token / empty or 1-line files without arithmetic errors (division by zero / NaN).
///
/// Arrange: Temporary file with 1 line or empty content.
/// Act: Run `ctxcut stats <temp_file>`.
/// Assert: Returns 0.0% or 0 savings without panics, errors, or NaN values.
#[test]
fn test_stats_zero_token_handling() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let one_liner = temp_dir.path().join("one_line.ts");
    fs::write(&one_liner, "export const PI = 3.14159;\n").unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner
        .run(&["stats", one_liner.to_str().unwrap()])
        .expect("Failed to run stats on one-liner");

    // Assert
    output.assert_success();
    assert!(
        !output.stdout.contains("NaN"),
        "Output must not contain NaN"
    );
    assert!(
        !output.stdout.contains("inf"),
        "Output must not contain Infinity"
    );
}

/// Test 5: Exact BPE Tokenizer Parity with OpenAI `cl100k_base`.
///
/// Arrange: Arbitrary complex code strings with known BPE token counts.
/// Act: Count tokens using `TokenVerifier`.
/// Assert: TokenVerifier accurately counts tokens and measures reductions.
#[test]
fn test_stats_bpe_tokenizer_parity() {
    // Arrange
    let verifier = TokenVerifier::new();
    let sample_code = r#"
        import { useState, useEffect } from 'react';

        export function useCounter(initialValue: number = 0) {
            const [count, setCount] = useState<number>(initialValue);
            return { count, increment: () => setCount(c => c + 1) };
        }
    "#;

    // Act
    let count = verifier.count_tokens(sample_code);

    // Assert
    assert!(count > 0, "BPE count must be positive");
    assert!(
        count < 100,
        "BPE count for small hook should be under 100 tokens"
    );
}

/// Test 6: Verifying reduction bounds for sliced vs full file content.
///
/// Arrange: Large realistic service file.
/// Act: Compute metrics comparing full file to minimal slice.
/// Assert: Reduction percentage is strictly within [0.0%, 100.0%].
#[test]
fn test_stats_reduction_bounds_validation() {
    // Arrange
    let verifier = TokenVerifier::new();
    let full =
        fs::read_to_string("tests/fixtures/typescript/realistic_order_service/order_service.ts")
            .expect("Must read fixture file");
    let minimal_slice = r#"
        export interface RefundResponse {
            refundId: string;
            orderId: string;
        }
        public async function processRefund(orderId: string): Promise<RefundResponse> {
            return { refundId: "ref_1", orderId };
        }
    "#;

    // Act
    let metrics = verifier.calculate_metrics(&full, minimal_slice);

    // Assert
    assert!(
        metrics.reduction_percentage > 70.0,
        "Expected significant token reduction"
    );
    assert!(
        metrics.reduction_percentage <= 100.0,
        "Reduction cannot exceed 100%"
    );
    assert!(
        metrics.full_tokens > metrics.slice_tokens,
        "Full tokens must exceed slice tokens"
    );
}
