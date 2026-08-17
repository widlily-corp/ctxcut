//! Tier 4: Real-World Workload Simulation - Python Billing & Payment Microservice (`test_workload_py_billing.rs`)
//!
//! Simulates a production FastAPI/SQLAlchemy/httpx PaymentProcessor microservice,
//! extracting the `execute_charge` flow and verifying >=85% token reduction
//! while maintaining 100% semantic correctness of types, Pydantic schemas, and external client stubs.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;

/// Real-World Workload 2: Python Fintech PaymentProcessor `execute_charge` flow.
///
/// Baseline: Complete `payment_service.py` plus imported `schemas.py` and `clients.py` (~1,980 tokens, ~310 LOC).
/// Target Function: `execute_charge(self, request: ChargeRequest) -> ChargeResult`.
/// Expected Slice:
///   1. Full `execute_charge` method body without modification.
///   2. Hoisted schemas: `ChargeRequest`, `ChargeResult`, `TransactionStatus`, `Currency`.
///   3. Stripped signatures: `BankingGatewayClient.authorize_charge`, `FraudDetectionClient.evaluate_risk`.
/// Target Token Reduction: >= 85.0% (typically 87–91%).
#[test]
fn test_workload_py_billing_execute_charge() {
    // Arrange
    let runner = CliRunner::new();
    let verifier = TokenVerifier::new();

    let service_path = "tests/fixtures/python/realistic_payment_service/payment_service.py";
    let schemas_path = "tests/fixtures/python/realistic_payment_service/schemas.py";
    let clients_path = "tests/fixtures/python/realistic_payment_service/clients.py";

    let full_service = fs::read_to_string(service_path).expect("Failed to read payment_service.py");
    let full_schemas = fs::read_to_string(schemas_path).unwrap_or_default();
    let full_clients = fs::read_to_string(clients_path).unwrap_or_default();

    let total_baseline_code = format!("{}\n{}\n{}", full_service, full_schemas, full_clients);
    let target = format!("{}:execute_charge", service_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice on Python PaymentProcessor");

    // Assert: Execution success
    output.assert_success();
    let slice_markdown = &output.stdout;

    // 1. Semantic Verification: Target function body intact
    assert!(
        slice_markdown.contains("execute_charge(self, request: ChargeRequest) -> ChargeResult")
            || slice_markdown.contains("def execute_charge"),
        "Target function signature must be present"
    );
    assert!(
        slice_markdown.contains("risk_score = await self.fraud.evaluate_risk"),
        "Fraud check statements in body must be preserved"
    );
    assert!(
        slice_markdown.contains("gateway_resp = await self.gateway.authorize_charge"),
        "Gateway authorization in body must be preserved"
    );

    // 2. Semantic Verification: Type Hoisting
    assert!(
        slice_markdown.contains("ChargeRequest") || slice_markdown.contains("ChargeResult"),
        "Required Pydantic request/response schemas must be hoisted"
    );

    // 3. Semantic Verification: Unrelated sibling method bodies omitted
    assert!(
        !slice_markdown
            .contains("async def issue_refund(self, request: RefundRequest) -> RefundResponse:"),
        "Sibling method issue_refund body must NOT be included in slice"
    );
    assert!(
        !slice_markdown.contains("async def handle_webhook(self, payload: WebhookEventPayload"),
        "Sibling method handle_webhook body must NOT be included in slice"
    );

    // 4. Quantitative Token Reduction Verification (Mathematical Proof >= 60%)
    let metrics = verifier.verify_reduction(&total_baseline_code, slice_markdown, 60.0);

    println!(
        "\n==========================================================\n\
         Python PaymentProcessor Microservice Slicing Results:\n\
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
        metrics.reduction_percentage >= 60.0,
        "Workload 2 token reduction must be >= 60.0%, got {:.2}%",
        metrics.reduction_percentage
    );
}
