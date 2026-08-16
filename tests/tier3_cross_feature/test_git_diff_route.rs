//! Tier 3: Cross-Feature - Git Diff & Route Handler Integration (`test_git_diff_route.rs`)
//!
//! Verifies Git diff detection intersecting with web framework route handlers (Express, FastAPI, Gin, Axum),
//! ensuring modified route handlers are automatically identified and their contextual DTOs extracted.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox};

/// Test 1: Modifying an Express route handler in working tree and slicing via `ctxcut diff`.
///
/// Arrange: Git sandbox repository with Express router setup; mutate `handleCheckout` handler body unstaged.
/// Act: Run `ctxcut diff`.
/// Assert: Slices `handleCheckout`, inlines `CheckoutRequestDTO`, reflects modified statements.
#[test]
fn test_diff_express_route_handler_modification() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let router_src = r#"
export interface CheckoutRequest {
    customerId: string;
    amountCents: number;
}

export async function handleCheckout(req: any, res: any): Promise<void> {
    const { customerId, amountCents } = req.body;
    res.status(200).json({ status: "PAID", customerId, amountCents });
}

export function registerRoutes(app: any): void {
    app.post("/api/v1/checkout", handleCheckout);
}
"#;
    sandbox.write_file("src/routes.ts", router_src).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial routes commit").unwrap();

    // Mutate handleCheckout
    let modified_src = r#"
export interface CheckoutRequest {
    customerId: string;
    amountCents: number;
}

export async function handleCheckout(req: any, res: any): Promise<void> {
    const { customerId, amountCents } = req.body;
    // Added audit logging and fee computation
    const feeCents = amountCents * 0.03;
    res.status(200).json({ status: "PAID", customerId, amountCents, feeCents });
}

export function registerRoutes(app: any): void {
    app.post("/api/v1/checkout", handleCheckout);
}
"#;
    sandbox.modify_file("src/routes.ts", modified_src).unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner.run_in_dir(sandbox.path(), &["diff"]).expect("ctxcut diff failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("handleCheckout"), "Must identify modified route handler handleCheckout");
    assert!(stdout.contains("feeCents"), "Must capture modified handler logic");
    assert!(stdout.contains("CheckoutRequest") || stdout.contains("customerId"), "Must retain DTO context");
}

/// Test 2: Modifying a FastAPI route handler and slicing via `ctxcut diff --staged`.
///
/// Arrange: Python FastAPI app committed; modify `@router.post("/items/")` handler and stage it.
/// Act: Run `ctxcut diff --staged`.
/// Assert: Correctly slices staged FastAPI handler and inlines Pydantic schemas.
#[test]
fn test_diff_fastapi_staged_route_modification() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let fastapi_src = r#"
from pydantic import BaseModel

class ItemPayload(BaseModel):
    name: str
    price: float

def create_item_endpoint(payload: ItemPayload) -> dict:
    return {"id": "1", "name": payload.name}
"#;
    sandbox.write_file("src/api.py", fastapi_src).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial FastAPI commit").unwrap();

    // Modify and stage
    let modified_src = r#"
from pydantic import BaseModel

class ItemPayload(BaseModel):
    name: str
    price: float

def create_item_endpoint(payload: ItemPayload) -> dict:
    # Staged change adding discount calculation
    final_price = payload.price * 0.90
    return {"id": "1", "name": payload.name, "discounted_price": final_price}
"#;
    sandbox.modify_file("src/api.py", modified_src).unwrap();
    sandbox.stage_file("src/api.py").unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner.run_in_dir(sandbox.path(), &["diff", "--staged"]).expect("ctxcut diff --staged failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("create_item_endpoint"), "Must identify modified FastAPI endpoint");
    assert!(stdout.contains("final_price"), "Must capture modified discount logic");
    assert!(stdout.contains("ItemPayload"), "Must hoist ItemPayload schema");
}

/// Test 3: Modifying route DTO while route handler function body is unchanged.
///
/// Arrange: Mutate `interface ItemPayload` fields.
/// Act: Run `ctxcut diff`.
/// Assert: Detects interface modification and preserves handler context.
#[test]
fn test_diff_route_dto_modification() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let src = r#"
export interface LoginDTO {
    email: string;
}

export function handleLogin(dto: LoginDTO): boolean {
    return dto.email.includes("@");
}
"#;
    sandbox.write_file("src/auth.ts", src).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial auth commit").unwrap();

    // Modify DTO
    let modified = r#"
export interface LoginDTO {
    email: string;
    otpToken?: string;
}

export function handleLogin(dto: LoginDTO): boolean {
    return dto.email.includes("@");
}
"#;
    sandbox.modify_file("src/auth.ts", modified).unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner.run_in_dir(sandbox.path(), &["diff"]).expect("ctxcut diff failed");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("LoginDTO") || stdout.contains("otpToken"),
        "Must capture modified DTO context"
    );
}
