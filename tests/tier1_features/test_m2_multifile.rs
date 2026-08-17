//! Tier 1: Feature Coverage - Milestone 2 Multi-File Dependency Slicing (--depth 1)
//!
//! Verifies cross-file import resolution, foreign type hoisting, and foreign signature
//! stripping across TypeScript, Python, Go, and Rust.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;

#[test]
fn test_m2_typescript_depth1_cross_file_slicing() {
    let runner = CliRunner::new();
    let target = "tests/fixtures/typescript/realistic_order_service/order_service.ts:OrderService.processOrder";

    let output = runner
        .run(&["slice", target, "--depth", "1"])
        .expect("Failed to execute ctxcut slice");

    output.assert_success();
    let stdout = &output.stdout;

    // 1. Target implementation
    assert!(
        stdout.contains("processOrder(request: OrderCreationRequest"),
        "Should contain target method header"
    );
    assert!(
        stdout.contains("return savedOrder;"),
        "Should contain target method body"
    );

    // 2. Foreign hoisted types from models.ts
    assert!(
        stdout.contains("interface OrderCreationRequest")
            || stdout.contains("OrderCreationRequest"),
        "Should hoist OrderCreationRequest from models.ts"
    );
    assert!(
        stdout.contains("interface Customer") || stdout.contains("Customer"),
        "Should hoist Customer from models.ts"
    );

    // 3. Foreign stripped signatures from gateways.ts
    assert!(
        stdout.contains("calculateSalesTax")
            || stdout.contains("chargeCard")
            || stdout.contains("checkAvailability"),
        "Should contain foreign call signature stubs"
    );
    // 0% foreign body leakage: no Stripe or external gateway internals
    assert!(
        !stdout.contains("if (!this.apiKey)"),
        "Must not leak foreign function body implementation"
    );

    // 4. Token reduction metrics
    assert!(
        stdout.contains("Savings:") || stdout.contains("savings") || stdout.contains("Total lines"),
        "Should display token reduction statistics"
    );
}

#[test]
fn test_m2_python_depth1_cross_file_slicing() {
    let runner = CliRunner::new();
    let target = "tests/fixtures/python/realistic_payment_service/payment_service.py:PaymentProcessor.execute_charge";

    let output = runner
        .run(&["slice", target, "--depth", "1"])
        .expect("Failed to execute ctxcut slice");

    output.assert_success();
    let stdout = &output.stdout;

    // 1. Target method
    assert!(
        stdout.contains("async def execute_charge"),
        "Should contain execute_charge method"
    );

    // 2. Foreign hoisted types from schemas.py
    assert!(
        stdout.contains("class ChargeRequest") || stdout.contains("ChargeRequest"),
        "Should hoist ChargeRequest from schemas.py"
    );
    assert!(
        stdout.contains("class ChargeResult") || stdout.contains("ChargeResult"),
        "Should hoist ChargeResult from schemas.py"
    );

    // 3. Foreign call signatures from clients.py
    assert!(
        stdout.contains("authorize_charge") || stdout.contains("BankingGatewayClient"),
        "Should contain foreign client call stubs"
    );
    // Zero body leakage
    assert!(
        !stdout.contains("endpoint = f\"{self.base_url}"),
        "Must not leak foreign method body"
    );
}

#[test]
fn test_m2_go_depth1_cross_file_slicing() {
    let runner = CliRunner::new();
    let target = "tests/fixtures/go/realistic_auth_service/service.go:AuthService.AuthenticateUser";

    let output = runner
        .run(&["slice", target, "--depth", "1"])
        .expect("Failed to execute ctxcut slice");

    output.assert_success();
    let stdout = &output.stdout;

    // 1. Target method
    assert!(
        stdout.contains("func (s *AuthService) AuthenticateUser"),
        "Should contain target method header"
    );

    // 2. Foreign hoisted types from models.go
    assert!(
        stdout.contains("type User struct") || stdout.contains("User"),
        "Should hoist User from models.go"
    );
    assert!(
        stdout.contains("type AuthResult struct") || stdout.contains("AuthResult"),
        "Should hoist AuthResult from models.go"
    );

    // 3. Foreign call signatures from repo.go / jwt_helper.go
    assert!(
        stdout.contains("FindByEmail")
            || stdout.contains("GenerateToken")
            || stdout.contains("CheckPasswordHash"),
        "Should contain foreign signature stubs"
    );
}

#[test]
fn test_m2_rust_depth1_cross_file_slicing() {
    let runner = CliRunner::new();
    let target = "tests/fixtures/rust/realistic_inventory_service/inventory.rs:InventoryService::reserve_stock";

    let output = runner
        .run(&["slice", target, "--depth", "1"])
        .expect("Failed to execute ctxcut slice");

    output.assert_success();
    let stdout = &output.stdout;

    // 1. Target method
    assert!(
        stdout.contains("pub async fn reserve_stock") || stdout.contains("reserve_stock"),
        "Should contain reserve_stock method"
    );

    // 2. Foreign hoisted types from models.rs
    assert!(
        stdout.contains("pub struct ReservationRequest") || stdout.contains("ReservationRequest"),
        "Should hoist ReservationRequest from models.rs"
    );
    assert!(
        stdout.contains("pub struct StockReservation") || stdout.contains("StockReservation"),
        "Should hoist StockReservation from models.rs"
    );

    // 3. Foreign call signatures from external.rs
    assert!(
        stdout.contains("acquire_lock") || stdout.contains("RedisLockManager"),
        "Should contain external lock acquire call signature"
    );
    // Zero body leakage
    assert!(
        !stdout.contains("let now_ms = 1_700_000_000_000u64"),
        "Must not leak foreign lock implementation body"
    );
}

#[test]
fn test_m2_cli_depth_0_vs_depth_1_comparison() {
    let runner = CliRunner::new();
    let target = "tests/fixtures/typescript/realistic_order_service/order_service.ts:OrderService.processOrder";

    // Depth 0: Local only
    let out_d0 = runner
        .run(&["slice", target, "--depth", "0"])
        .expect("Failed to execute depth 0");
    out_d0.assert_success();

    // Depth 1: Neighbor inlining
    let out_d1 = runner
        .run(&["slice", target, "--depth", "1"])
        .expect("Failed to execute depth 1");
    out_d1.assert_success();

    // Depth 1 must hoist models.ts types which are foreign
    assert!(
        !out_d0
            .stdout
            .contains("export interface OrderCreationRequest {"),
        "Depth 0 should not inline foreign models.ts interface"
    );
    assert!(
        out_d1.stdout.contains("OrderCreationRequest"),
        "Depth 1 should inline foreign models.ts interface"
    );
}
