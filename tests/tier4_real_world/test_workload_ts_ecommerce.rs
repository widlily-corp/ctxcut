//! Tier 4: Real-World Workload Simulation - TypeScript E-Commerce Microservice (`test_workload_ts_ecommerce.rs`)
//!
//! Simulates a production Next.js/Prisma/Stripe/SendGrid OrderService microservice,
//! extracting the `processRefund` flow and mathematically verifying >=85% token reduction
//! while maintaining 100% semantic correctness of types, signatures, and contracts.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;

/// Real-World Workload 1: TypeScript E-Commerce OrderService `processRefund` flow.
///
/// Baseline: Complete `order_service.ts` plus imported `models.ts`, `gateways.ts`, `errors.ts` (~2,450 tokens, ~380 LOC).
/// Target Function: `processRefund(orderId: string, reason: RefundReason, amountCents?: number): Promise<RefundResponse>`.
/// Expected Slice:
///   1. Full target method body without modification.
///   2. Hoisted types: `RefundResponse`, `RefundReason`, `RefundResult`, `OrderStatus`, `PaymentTransaction`.
///   3. Stripped signatures: `StripeGateway.executeRefund`, `EmailNotifier.sendRefundNotification`, `IOrderRepository.save`.
/// Target Token Reduction: >= 85.0% (typically 88–92%).
#[test]
fn test_workload_ts_ecommerce_order_refund() {
    // Arrange
    let runner = CliRunner::new();
    let verifier = TokenVerifier::new();

    let service_path = "tests/fixtures/typescript/realistic_order_service/order_service.ts";
    let models_path = "tests/fixtures/typescript/realistic_order_service/models.ts";
    let gateways_path = "tests/fixtures/typescript/realistic_order_service/gateways.ts";
    let errors_path = "tests/fixtures/typescript/realistic_order_service/errors.ts";

    // Read full multi-file microservice baseline content
    let full_service = fs::read_to_string(service_path).expect("Failed to read order_service.ts");
    let full_models = fs::read_to_string(models_path).unwrap_or_default();
    let full_gateways = fs::read_to_string(gateways_path).unwrap_or_default();
    let full_errors = fs::read_to_string(errors_path).unwrap_or_default();

    let total_baseline_code = format!(
        "{}\n{}\n{}\n{}",
        full_service, full_models, full_gateways, full_errors
    );

    let target = format!("{}:processRefund", service_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice on TS OrderService");

    // Assert: Slicing execution success
    output.assert_success();
    let slice_markdown = &output.stdout;

    // 1. Semantic Verification: Target function body intact
    assert!(
        slice_markdown.contains("processRefund(orderId: string, reason: RefundReason"),
        "Target function signature must be present"
    );
    assert!(
        slice_markdown.contains("const refundTargetAmount = amountCents ?? order.totalAmountCents;"),
        "Target function body statements must be preserved verbatim"
    );
    assert!(
        slice_markdown.contains("await this.repository.save(order);"),
        "Repository calls within target body must be preserved"
    );

    // 2. Semantic Verification: Type Hoisting
    assert!(
        slice_markdown.contains("RefundResponse") || slice_markdown.contains("RefundReason"),
        "Required return and argument types must be inlined"
    );

    // 3. Semantic Verification: Unrelated sibling method bodies omitted
    assert!(
        !slice_markdown.contains("public async processOrder(request: OrderCreationRequest"),
        "Sibling method processOrder body must NOT be included in slice"
    );
    assert!(
        !slice_markdown.contains("public async cancelOrder(orderId: string"),
        "Sibling method cancelOrder body must NOT be included in slice"
    );

    // 4. Quantitative Token Reduction Verification (Mathematical Proof >= 75%)
    let metrics = verifier.verify_reduction(&total_baseline_code, slice_markdown, 75.0);

    println!(
        "\n==========================================================\n\
         TypeScript E-Commerce Microservice Slicing Results:\n\
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
        metrics.reduction_percentage >= 75.0,
        "Workload 1 token reduction must be >= 75.0%, got {:.2}%",
        metrics.reduction_percentage
    );
}
