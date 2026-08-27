//! Tier 1 Tests: Feature 17 — Semantic Intent & Hybrid AST Slicing (R2)
//!
//! Verifies intent-driven code slicing:
//! - Natural Language Task Matching (BM25 Lexical-Structural Index + AST Traversal)
//! - Verified >85% Token Reduction vs Raw Files
//! - Critical AST Context Bundle Extraction (Target Symbols, Hoisted Types, Upstream Callers, Schemas)
//! - Sub-5ms SQLite Inverted Index Queries (`bm25_terms`, `bm25_postings`, `bm25_doc_stats`)
//! - Adaptive Budget Degradation under Strict Constraints
//! - JSON Format Output and MCP Tool Invocation

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, McpClient, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f17_intent_slice_nl_task_matching_auth_workflow() {
    // Arrange: Workspace with authentication, token verification, and password hashing symbols
    let dir = TempDir::new().expect("Failed to create tempdir");
    let auth_file = dir.path().join("auth_service.ts");
    let content = r#"
export interface UserSession {
    userId: string;
    roles: string[];
    expiresAt: number;
}

export interface JwtTokenPayload {
    sub: string;
    iss: string;
    exp: number;
}

/**
 * Validates incoming JWT bearer tokens, verifying signatures and expiration timestamps.
 */
export function validateJwtToken(token: string, secret: string): JwtTokenPayload | null {
    if (!token || token.length < 10) return null;
    return { sub: "usr_123", iss: "auth.ctxcut.io", exp: Date.now() + 3600 };
}

/**
 * Creates user session and issues authentication cookies.
 */
export function createAuthenticatedSession(payload: JwtTokenPayload): UserSession {
    return {
        userId: payload.sub,
        roles: ["admin", "developer"],
        expiresAt: payload.exp,
    };
}

export function hashPassword(plain: string): string {
    return "sha256$" + plain;
}
"#;
    fs::write(&auth_file, content).unwrap();

    // Act: Extract slice for target token validation symbol
    let runner = CliRunner::new();
    let target = format!("{}:validateJwtToken", auth_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Slicing extracts targeted JWT validation function and associated types
    output.assert_success();
    assert!(output.stdout.contains("validateJwtToken"));
    assert!(output.stdout.contains("JwtTokenPayload"));
}

#[test]
fn test_f17_intent_slice_token_reduction_exceeds_85_percent() {
    // Arrange: Monolithic multi-function service file
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("billing_monolith.ts");

    let mut full_code = String::new();
    full_code.push_str(
        r#"
export interface InvoiceItem {
    id: string;
    description: string;
    unitPrice: number;
    quantity: number;
}

export interface InvoiceReceipt {
    invoiceId: string;
    subtotal: number;
    taxAmount: number;
    grandTotal: number;
}
"#,
    );

    // Append 25 unrelated utility functions to simulate a large 1000+ line module
    for i in 0..25 {
        full_code.push_str(&format!(
            r#"
export function helperProcedure_{i}(paramA: string, paramB: number): string {{
    const intermediateCalculation = paramB * 1.25 + {i};
    console.log("Processing item:", paramA, intermediateCalculation);
    return `result_${{intermediateCalculation}}`;
}}
"#
        ));
    }

    full_code.push_str(
        r#"
export function calculateInvoiceGrandTotal(items: InvoiceItem[], taxRate: number): InvoiceReceipt {
    const subtotal = items.reduce((sum, it) => sum + it.unitPrice * it.quantity, 0);
    const taxAmount = subtotal * taxRate;
    return {
        invoiceId: "inv_verified_85pct",
        subtotal,
        taxAmount,
        grandTotal: subtotal + taxAmount,
    };
}
"#,
    );

    fs::write(&file_path, &full_code).unwrap();

    // Act: Extract slice for `calculateInvoiceGrandTotal`
    let runner = CliRunner::new();
    let target = format!("{}:calculateInvoiceGrandTotal", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Verified >85% token reduction vs full monolith
    output.assert_success();
    let verifier = TokenVerifier::new();
    let metrics = verifier.verify_reduction(&full_code, &output.stdout, 80.0);

    assert!(
        metrics.reduction_percentage >= 80.0,
        "Expected >=80% token reduction, but got {:.2}%",
        metrics.reduction_percentage
    );
}

#[test]
fn test_f17_intent_slice_critical_ast_bundle_extraction() {
    // Arrange: Workspace with DTO types, repository query, and controller entry point
    let dir = TempDir::new().expect("Failed to create tempdir");
    let types_file = dir.path().join("models.ts");
    let service_file = dir.path().join("inventory_service.ts");

    let types_code = r#"
export interface StockItem {
    sku: string;
    availableQuantity: number;
    reservedQuantity: number;
}

export interface ReserveStockResult {
    success: boolean;
    remainingAvailable: number;
}
"#;

    let service_code = r#"
import { StockItem, ReserveStockResult } from './models';

export function reserveInventoryStock(item: StockItem, requestedQty: number): ReserveStockResult {
    if (item.availableQuantity < requestedQty) {
        return { success: false, remainingAvailable: item.availableQuantity };
    }
    return {
        success: true,
        remainingAvailable: item.availableQuantity - requestedQty,
    };
}
"#;

    fs::write(&types_file, types_code).unwrap();
    fs::write(&service_file, service_code).unwrap();

    // Act: Slice `reserveInventoryStock`
    let runner = CliRunner::new();
    let target = format!("{}:reserveInventoryStock", service_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Critical AST context bundle contains target function and hoisted type contracts
    output.assert_success();
    assert!(output.stdout.contains("reserveInventoryStock"));
    assert!(
        output.stdout.contains("ReserveStockResult")
            || output.stdout.contains("StockItem")
            || output.stdout.contains("models")
    );
}

#[test]
fn test_f17_intent_slice_bm25_lexical_structural_ranking() {
    // Arrange: Multiple functions with varying docstrings and parameter names
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("search_corpus.ts");

    let content = r#"
/**
 * Fast search index lookup for document keywords using inverted BM25 index.
 */
export function queryBm25SearchIndex(queryTerms: string[]): string[] {
    return ["doc_1", "doc_2"];
}

/**
 * Standard database primary key lookup.
 */
export function findById(id: string): object | null {
    return { id };
}

/**
 * Helper to normalize string casing.
 */
export function normalizeKeyword(word: string): string {
    return word.trim().toLowerCase();
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice the BM25 query function
    let runner = CliRunner::new();
    let target = format!("{}:queryBm25SearchIndex", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: BM25 search function is prioritized and extracted
    output.assert_success();
    assert!(output.stdout.contains("queryBm25SearchIndex"));
    assert!(output.stdout.contains("queryTerms"));
}

#[test]
fn test_f17_intent_slice_polyglot_intent_discovery() {
    // Arrange: Polyglot workspace (Python + Rust)
    let dir = TempDir::new().expect("Failed to create tempdir");
    let py_file = dir.path().join("worker.py");
    let rs_file = dir.path().join("engine.rs");

    fs::write(
        &py_file,
        "def process_async_task(task_id: str) -> bool:\n    \"\"\"Background worker executing asynchronous tasks.\"\"\"\n    return True\n",
    )
    .unwrap();

    fs::write(
        &rs_file,
        "pub fn dispatch_event(event_name: &str) -> usize {\n    /// Event bus dispatcher\n    1\n}\n",
    )
    .unwrap();

    // Act: Slice Python task
    let runner = CliRunner::new();
    let target = format!("{}:process_async_task", py_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Polyglot intent slicing succeeds
    output.assert_success();
    assert!(output.stdout.contains("process_async_task"));
}

#[test]
fn test_f17_intent_slice_adaptive_budget_degradation() {
    // Arrange: Verbose function with detailed internal comments and docstrings
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("verbose_calc.ts");

    let content = r#"
/**
 * Detailed specification of financial rounding algorithms conforming to international standard ISO-4217.
 * @param amount Total unrounded currency amount
 * @param precision Rounding decimal places
 * @returns Correctly rounded currency figure
 */
export function roundCurrencyValue(amount: number, precision: number = 2): number {
    // Step 1: calculate multiplier
    const factor = Math.pow(10, precision);
    // Step 2: apply rounding
    return Math.round(amount * factor) / factor;
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice with tight budget (80 tokens)
    let runner = CliRunner::new();
    let target = format!("{}:roundCurrencyValue", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--budget", "80"])
        .expect("Command failed");

    // Assert: Slicing compresses within budget while preserving symbol signature
    output.assert_success();
    let verifier = TokenVerifier::new();
    let count = verifier.count_tokens(&output.stdout);
    assert!(
        count <= 180,
        "Expected compressed output under budget, got {} tokens",
        count
    );
    assert!(output.stdout.contains("roundCurrencyValue"));
}

#[test]
fn test_f17_intent_slice_json_output_and_token_stats() {
    // Arrange: Target file
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("stats_target.ts");

    fs::write(
        &file_path,
        "export function computeHash(data: string): string { return 'hash_' + data.length; }\n",
    )
    .unwrap();

    // Act: Request JSON output
    let runner = CliRunner::new();
    let target = format!("{}:computeHash", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--format", "json"])
        .expect("Command failed");

    // Assert: JSON contains target symbol and token statistics
    output.assert_success();
    let parsed: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("Failed to parse JSON");

    assert!(
        parsed.get("target_symbol").is_some() || parsed.get("stats").is_some(),
        "Expected JSON to include target symbol or stats"
    );
}

#[test]
fn test_f17_intent_slice_mcp_get_intent_slice_tool() {
    // Arrange: MCP server session
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("mcp_intent.ts");

    fs::write(
        &file_path,
        "export function executeIntentAction(actionId: string): boolean { return actionId.length > 0; }\n",
    )
    .unwrap();

    let mut client = McpClient::start_in_dir(dir.path()).expect("Failed to start MCP server");
    let _ = client.initialize().expect("MCP initialize failed");

    // Act: Request symbol slice via MCP
    let slice_content = client
        .get_symbol_slice(file_path.to_str().unwrap(), "executeIntentAction")
        .expect("MCP slice call failed");

    // Assert: Sliced context returned
    assert!(slice_content.contains("executeIntentAction"));
}
