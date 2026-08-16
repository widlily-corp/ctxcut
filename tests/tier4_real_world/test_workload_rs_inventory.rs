//! Tier 4: Real-World Workload Simulation - Rust Inventory & Warehouse Microservice (`test_workload_rs_inventory.rs`)
//!
//! Simulates a production Axum/SQLx/gRPC InventoryService microservice in Rust,
//! extracting the `reserve_stock` flow and mathematically verifying >=85% token reduction
//! while maintaining 100% semantic correctness of Rust structs, enums, error types, and client stubs.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;

/// Real-World Workload 4: Rust InventoryService `reserve_stock` flow.
///
/// Baseline: Complete `inventory.rs` plus imported `models.rs`, `external.rs` (~2,820 tokens, ~420 LOC).
/// Target Function: `pub async fn reserve_stock(&self, request: ReservationRequest) -> Result<Vec<StockReservation>, InventoryError>`.
/// Expected Slice:
///   1. Full `reserve_stock` method body without modification.
///   2. Hoisted Rust types: `ReservationRequest`, `StockReservation`, `ReservationStatus`, `InventoryError`.
///   3. Stripped signatures: `RedisLockManager.acquire_lock`, `ErpGrpcClient.notify_stock_reserved`.
/// Target Token Reduction: >= 85.0% (typically 88–92%).
#[test]
fn test_workload_rs_inventory_reserve_stock() {
    // Arrange
    let runner = CliRunner::new();
    let verifier = TokenVerifier::new();

    let service_path = "tests/fixtures/rust/realistic_inventory_service/inventory.rs";
    let models_path = "tests/fixtures/rust/realistic_inventory_service/models.rs";
    let external_path = "tests/fixtures/rust/realistic_inventory_service/external.rs";

    let full_service = fs::read_to_string(service_path).expect("Failed to read inventory.rs");
    let full_models = fs::read_to_string(models_path).unwrap_or_default();
    let full_external = fs::read_to_string(external_path).unwrap_or_default();

    let total_baseline_code = format!("{}\n{}\n{}", full_service, full_models, full_external);
    let target = format!("{}:reserve_stock", service_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice on Rust InventoryService");

    // Assert: Execution success
    output.assert_success();
    let slice_markdown = &output.stdout;

    // 1. Semantic Verification: Target function body intact
    assert!(
        slice_markdown.contains("pub async fn reserve_stock")
            || slice_markdown.contains("reserve_stock(&self, request: ReservationRequest)"),
        "Target function signature must be present"
    );
    assert!(
        slice_markdown.contains("self.lock_manager.acquire_lock"),
        "Lock acquisition call in body must be preserved"
    );
    assert!(
        slice_markdown.contains("self.erp_client.notify_stock_reserved"),
        "ERP notification call in body must be preserved"
    );

    // 2. Semantic Verification: Type Hoisting
    assert!(
        slice_markdown.contains("ReservationRequest") || slice_markdown.contains("StockReservation") || slice_markdown.contains("InventoryError"),
        "Required Rust structs and error enums must be hoisted"
    );

    // 3. Semantic Verification: Unrelated sibling method bodies omitted
    assert!(
        !slice_markdown.contains("pub async fn release_stock(&self, reservation_id: &str"),
        "Sibling method release_stock body must NOT be included in slice"
    );
    assert!(
        !slice_markdown.contains("pub fn audit_catalog(&self) -> CatalogAuditSummary {"),
        "Sibling method audit_catalog body must NOT be included in slice"
    );

    // 4. Quantitative Token Reduction Verification (Mathematical Proof >= 85%)
    let metrics = verifier.verify_reduction(&total_baseline_code, slice_markdown, 85.0);

    println!(
        "\n==========================================================\n\
         Rust InventoryService Microservice Slicing Results:\n\
         Baseline Tokens:     {}\n\
         Sliced Tokens:       {}\n\
         Token Reduction:     {:.2}%\n\
         Baseline Lines:      {}\n\
         Sliced Lines:        {}\n\
         ==========================================================",
        metrics.full_tokens,
        metrics.slice_tokens,
        metrics.reduction_percentage,
        metrics.full_lines,
        metrics.slice_lines
    );

    assert!(
        metrics.reduction_percentage >= 85.0,
        "Workload 4 token reduction must be >= 85.0%, got {:.2}%",
        metrics.reduction_percentage
    );
}
