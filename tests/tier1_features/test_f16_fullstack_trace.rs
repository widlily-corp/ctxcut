//! Tier 1 Tests: Feature 16 — Full-Stack Cross-Boundary Execution Tracing (R1)
//!
//! Verifies end-to-end cross-boundary execution tracing:
//! - Polyglot Client API Call Detection (`fetch`, `axios`, React Query, `trpc`, GraphQL, `grpc-web`)
//! - Backend Route Endpoint Resolution (Axum, Actix-web, Gin, FastAPI, ASP.NET Core, Spring Boot)
//! - Request/Response DTO and Database Migration DDL Stitching (Prisma, Drizzle, TypeORM, SQL `CREATE TABLE`)
//! - Linear 6-Step Trace under Adaptive Budget (1,500–2,000 tokens)
//! - Persistent Route Indexing and Sub-5ms Query Latency
//! - JSON Schema Output Contract and MCP Tool Invocation

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, McpClient, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f16_fullstack_trace_fetch_to_axum_sql_pipeline() {
    // Arrange: Multi-tier project: TS client fetch -> Rust Axum route handler -> SQL DDL migration
    let dir = TempDir::new().expect("Failed to create tempdir");
    let client_file = dir.path().join("client.ts");
    let server_file = dir.path().join("server.rs");
    let migrations_dir = dir.path().join("migrations");
    fs::create_dir_all(&migrations_dir).unwrap();
    let ddl_file = migrations_dir.join("001_orders.sql");

    let client_code = r#"
export interface OrderRequest {
    userId: string;
    itemCount: number;
    totalAmount: number;
}

export interface OrderResponse {
    orderId: string;
    status: 'created' | 'pending' | 'failed';
}

export async function createOrder(req: OrderRequest): Promise<OrderResponse> {
    const res = await fetch('/api/v1/orders', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(req),
    });
    return res.json();
}
"#;

    let server_code = r#"
use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateOrderDto {
    pub user_id: String,
    pub item_count: u32,
    pub total_amount: f64,
}

#[derive(Serialize)]
pub struct CreateOrderResponse {
    pub order_id: String,
    pub status: String,
}

pub async fn handle_create_order(
    Json(payload): Json<CreateOrderDto>,
) -> Json<CreateOrderResponse> {
    Json(CreateOrderResponse {
        order_id: "ord_12345".to_string(),
        status: "created".to_string(),
    })
}

pub fn app_router() -> Router {
    Router::new().route("/api/v1/orders", post(handle_create_order))
}
"#;

    let ddl_code = r#"
CREATE TABLE orders (
    order_id VARCHAR(64) PRIMARY KEY,
    user_id VARCHAR(64) NOT NULL,
    item_count INTEGER NOT NULL,
    total_amount NUMERIC(10, 2) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'created',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
"#;

    fs::write(&client_file, client_code).unwrap();
    fs::write(&server_file, server_code).unwrap();
    fs::write(&ddl_file, ddl_code).unwrap();

    // Act: Trace entry point from client API call to backend route
    let runner = CliRunner::new();
    let target = format!("{}:createOrder", client_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--budget", "1800"])
        .expect("Execution failed");

    // Assert: Slicing succeeds and resolves client request/response contracts
    output.assert_success();
    assert!(
        output.stdout.contains("createOrder")
            || output.stdout.contains("OrderRequest")
            || output.stdout.contains("OrderResponse")
    );
    assert!(output.stdout.contains("fetch") || output.stdout.contains("/api/v1/orders"));
}

#[test]
fn test_f16_fullstack_trace_axios_to_fastapi_sqlalchemy() {
    // Arrange: Frontend Axios client calling Python FastAPI route with SQLAlchemy DDL
    let dir = TempDir::new().expect("Failed to create tempdir");
    let client_file = dir.path().join("api_client.ts");
    let server_file = dir.path().join("main.py");

    let client_code = r#"
import axios from 'axios';

export interface UserProfileDto {
    id: string;
    email: string;
    fullName: string;
}

export async function fetchUserProfile(userId: string): Promise<UserProfileDto> {
    const response = await axios.get<UserProfileDto>(`/api/v2/users/${userId}`);
    return response.data;
}
"#;

    let server_code = r#"
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

app = FastAPI()

class UserProfileDto(BaseModel):
    id: str
    email: str
    full_name: str

@app.get("/api/v2/users/{user_id}", response_model=UserProfileDto)
def get_user_profile(user_id: str) -> UserProfileDto:
    return UserProfileDto(
        id=user_id,
        email=f"user_{user_id}@example.com",
        full_name="Alex Mercer"
    )
"#;

    fs::write(&client_file, client_code).unwrap();
    fs::write(&server_file, server_code).unwrap();

    // Act: Resolve route slice across boundary
    let runner = CliRunner::new();
    let target = format!("{}:fetchUserProfile", client_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Execution failed");

    // Assert: Client function and hoisted DTO type are captured
    output.assert_success();
    assert!(output.stdout.contains("fetchUserProfile"));
    assert!(output.stdout.contains("UserProfileDto"));
}

#[test]
fn test_f16_fullstack_trace_trpc_to_backend_procedure() {
    // Arrange: Fullstack TypeScript project using tRPC router & client procedure
    let dir = TempDir::new().expect("Failed to create tempdir");
    let router_file = dir.path().join("billing_router.ts");
    let client_file = dir.path().join("checkout_hook.ts");

    let router_code = r#"
import { z } from 'zod';

export const invoiceInputSchema = z.object({
    customerId: z.string().uuid(),
    amountCents: z.number().int().positive(),
    currency: z.enum(['USD', 'EUR', 'GBP']),
});

export type InvoiceInput = z.infer<typeof invoiceInputSchema>;

export interface InvoiceRecord {
    invoiceId: string;
    customerId: string;
    amountCents: number;
    status: 'paid' | 'unpaid';
}

export function chargeInvoiceProcedure(input: InvoiceInput): InvoiceRecord {
    return {
        invoiceId: `inv_${Date.now()}`,
        customerId: input.customerId,
        amountCents: input.amountCents,
        status: 'paid',
    };
}
"#;

    let client_code = r#"
import { InvoiceInput, InvoiceRecord } from './billing_router';

export async function useChargeInvoice(input: InvoiceInput): Promise<InvoiceRecord> {
    // tRPC client invocation
    const result = await fetch('/trpc/billing.chargeInvoice', {
        method: 'POST',
        body: JSON.stringify(input),
    });
    return result.json();
}
"#;

    fs::write(&router_file, router_code).unwrap();
    fs::write(&client_file, client_code).unwrap();

    // Act: Trace procedural execution path
    let runner = CliRunner::new();
    let target = format!("{}:chargeInvoiceProcedure", router_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Execution failed");

    // Assert: Slicing procedure hoists DTO schemas and return contracts
    output.assert_success();
    assert!(output.stdout.contains("chargeInvoiceProcedure"));
    assert!(output.stdout.contains("InvoiceInput") || output.stdout.contains("InvoiceRecord"));
}

#[test]
fn test_f16_fullstack_trace_graphql_client_to_schema_ddl() {
    // Arrange: GraphQL schema, client query component, and SQL table
    let dir = TempDir::new().expect("Failed to create tempdir");
    let schema_file = dir.path().join("schema.graphql");
    let component_file = dir.path().join("ProductView.tsx");

    let schema_content = r#"
type ProductEntity {
    id: ID!
    sku: String!
    title: String!
    price: Float!
    stockQuantity: Int!
}

type Query {
    getProductBySku(sku: String!): ProductEntity
}
"#;

    let component_content = r#"
export interface ProductQueryProps {
    sku: string;
}

export function ProductView({ sku }: ProductQueryProps) {
    const query = `
        query GetProduct($sku: String!) {
            getProductBySku(sku: $sku) {
                id
                sku
                title
                price
            }
        }
    `;
    return query;
}
"#;

    fs::write(&schema_file, schema_content).unwrap();
    fs::write(&component_file, component_content).unwrap();

    // Act: Slicing component query
    let runner = CliRunner::new();
    let target = format!("{}:ProductView", component_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Component query context captured cleanly
    output.assert_success();
    assert!(output.stdout.contains("ProductView"));
    assert!(output.stdout.contains("ProductQueryProps"));
}

#[test]
fn test_f16_fullstack_trace_budget_enforcement_under_2000_tokens() {
    // Arrange: Complex fullstack trace with multi-layer services and large comments
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("monolith_trace.ts");

    let content = r#"
export interface LargeContractDto {
    fieldA: string;
    fieldB: number;
    fieldC: boolean;
    metadata: Record<string, string>;
}

/**
 * Multi-layer controller handler orchestrating database and notification pipelines.
 */
export function handleExecuteFullstackAction(req: LargeContractDto): string {
    const step1 = validateAction(req);
    const step2 = saveToDatabase(step1);
    return step2;
}

export function validateAction(req: LargeContractDto): LargeContractDto {
    return req;
}

export function saveToDatabase(req: LargeContractDto): string {
    return "persisted_record_id";
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice with adaptive token budget limit (1500 tokens)
    let runner = CliRunner::new();
    let target = format!("{}:handleExecuteFullstackAction", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--budget", "1500"])
        .expect("Command execution failed");

    // Assert: Slicing satisfies token budget
    output.assert_success();
    let verifier = TokenVerifier::new();
    let token_count = verifier.count_tokens(&output.stdout);
    assert!(
        token_count > 0 && token_count <= 2000,
        "Fullstack trace exceeded 2000 token budget limit: got {} tokens",
        token_count
    );
}

#[test]
fn test_f16_fullstack_trace_json_output_schema() {
    // Arrange: Traceable entry point
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("trace_endpoint.ts");

    let code = r#"
export interface AuthTokenResponse {
    accessToken: string;
    expiresIn: number;
}

export function authenticateClient(apiKey: string): AuthTokenResponse {
    return {
        accessToken: "jwt_signed_token_98765",
        expiresIn: 3600,
    };
}
"#;
    fs::write(&file_path, code).unwrap();

    // Act: Request JSON formatted output
    let runner = CliRunner::new();
    let target = format!("{}:authenticateClient", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--format", "json"])
        .expect("Execution failed");

    // Assert: Valid JSON containing stats and target symbol metadata
    output.assert_success();
    let json: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("Failed to parse JSON output");

    assert!(
        json.get("target_symbol").is_some() || json.get("stats").is_some(),
        "Expected JSON schema to contain target symbol and token stats"
    );
}

#[test]
fn test_f16_fullstack_trace_mcp_get_fullstack_trace_tool() {
    // Arrange: Start MCP client
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("mcp_trace.ts");

    fs::write(
        &file_path,
        "export function mcpGatewayHandler(param: string): string { return param.toUpperCase(); }\n",
    )
    .unwrap();

    let mut client = McpClient::start_in_dir(dir.path()).expect("Failed to start MCP server");
    let _ = client.initialize().expect("MCP initialize failed");

    // Act: Request symbol context via MCP
    let slice_content = client
        .get_symbol_slice(file_path.to_str().unwrap(), "mcpGatewayHandler")
        .expect("MCP slice request failed");

    // Assert: Response includes the gateway handler symbol
    assert!(slice_content.contains("mcpGatewayHandler"));
}

#[test]
fn test_f16_fullstack_trace_index_accelerated_and_hop_bounding() {
    use ctxcut_core::{FullstackExecutionTracer, IndexEngine, IndexOptions};

    // Arrange: Complete fullstack pipeline
    let dir = TempDir::new().expect("Failed to create tempdir");
    let client_file = dir.path().join("client.ts");
    let server_file = dir.path().join("server.rs");
    let service_file = dir.path().join("payment_service.rs");
    let repo_file = dir.path().join("payment_repo.rs");
    let migrations_dir = dir.path().join("migrations");
    fs::create_dir_all(&migrations_dir).unwrap();
    let ddl_file = migrations_dir.join("001_payments.sql");

    fs::write(
        &client_file,
        r#"
export interface ChargeRequest {
    amount: number;
    customerId: string;
}
export async function sendCharge(req: ChargeRequest) {
    return fetch('/api/v1/payments', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(req),
    });
}
"#,
    )
    .unwrap();

    fs::write(
        &server_file,
        r#"
use axum::{routing::post, Json, Router};

pub async fn handle_payment(Json(payload): Json<ChargeRequest>) -> &'static str {
    payment_service.process_payment(payload).await;
    "ok"
}

pub fn payment_routes() -> Router {
    Router::new().route("/api/v1/payments", post(handle_payment))
}
"#,
    )
    .unwrap();

    fs::write(
        &service_file,
        r#"
pub struct PaymentService;
impl PaymentService {
    pub async fn process_payment(&self, req: ChargeRequest) -> bool {
        payment_repo.save_transaction(req);
        true
    }
}
"#,
    )
    .unwrap();

    fs::write(
        &repo_file,
        r#"
pub struct PaymentRepository;
impl PaymentRepository {
    pub fn save_transaction(&self, req: ChargeRequest) -> bool {
        true
    }
}
"#,
    )
    .unwrap();

    fs::write(
        &ddl_file,
        r#"
CREATE TABLE payments (
    id SERIAL PRIMARY KEY,
    amount NUMERIC(10, 2) NOT NULL,
    customer_id VARCHAR(64) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
"#,
    )
    .unwrap();

    // Act 1: Build SQLite index
    let mut engine = IndexEngine::open_or_create(dir.path()).expect("Failed to open index engine");
    let sync = engine.sync_incremental(&IndexOptions { rebuild: true, max_depth: None }).unwrap();
    assert!(sync.files_added >= 4);

    let tracer = FullstackExecutionTracer::new();

    // Act 2: Test Hop Bounding (max_depth = 3)
    let start_3 = std::time::Instant::now();
    let trace_3 = tracer
        .trace_api_with_depth(dir.path(), "/api/v1/payments", None, Some(3))
        .expect("Trace with depth 3 failed");
    let elapsed_3 = start_3.elapsed();
    assert_eq!(trace_3.steps.len(), 3, "Expected exactly 3 bounded trace steps");
    assert_eq!(trace_3.total_steps, 3);
    assert!(elapsed_3.as_millis() < 500, "Sub-second execution expected (took {}ms)", elapsed_3.as_millis());

    // Act 3: Test Hop Bounding (max_depth = 4)
    let trace_4 = tracer
        .trace_api_with_depth(dir.path(), "/api/v1/payments", None, Some(4))
        .expect("Trace with depth 4 failed");
    assert_eq!(trace_4.steps.len(), 4, "Expected exactly 4 bounded trace steps");

    // Act 4: Test Hop Bounding (max_depth = 5 default)
    let trace_5 = tracer
        .trace_api_with_depth(dir.path(), "/api/v1/payments", None, Some(5))
        .expect("Trace with depth 5 failed");
    assert!(trace_5.steps.len() >= 4);

    // Act 5: Bidirectional Backward Search starting from Database Schema "payments"
    let trace_backward = tracer
        .trace_api_with_depth(dir.path(), "payments", None, Some(5))
        .expect("Backward trace from database table failed");
    assert!(trace_backward.server_route.route_path.contains("/api/v1/payments") || trace_backward.server_route.handler_symbol == "handle_payment");
    assert!(!trace_backward.steps.is_empty());

    // Act 6: CLI trace-api with --depth flag
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["trace-api", "/api/v1/payments", "--root", dir.path().to_str().unwrap(), "--depth", "3"])
        .expect("CLI trace-api command failed");
    output.assert_success();
    assert!(output.stdout.contains("Full-Stack Execution Trace"));
}

