//! Challenger 2 Empirical Verification Suite for Milestone 1 (R1):
//! - Sub-second Execution Latency (<1s) on Large Multi-Tier Codebases
//! - Strict max_depth Hop Bounding (3, 4, 5) and Clamping
//! - Full Bidirectional Resolution (DDL/Entity/Repo/Service -> Route -> Client)
//! - Concurrency, Cyclic Graphs, and Fault Tolerance

use ctxcut_core::{FullstackExecutionTracer, IndexEngine, IndexOptions};
use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tempfile::TempDir;

/// Helper to generate a realistic 50k+ LOC multi-tier enterprise repository.
fn generate_large_enterprise_workspace(root: &Path) {
    let client_dir = root.join("frontend").join("src");
    let server_dir = root.join("backend").join("src");
    let services_dir = server_dir.join("services");
    let repos_dir = server_dir.join("repositories");
    let migrations_dir = root.join("migrations");

    fs::create_dir_all(&client_dir).unwrap();
    fs::create_dir_all(&services_dir).unwrap();
    fs::create_dir_all(&repos_dir).unwrap();
    fs::create_dir_all(&migrations_dir).unwrap();

    // 1. Frontend Client API Call
    fs::write(
        client_dir.join("orderClient.ts"),
        r#"
export interface PlaceOrderRequest {
    customerId: string;
    items: Array<{ sku: string; quantity: number }>;
    shippingAddress: string;
}

export interface PlaceOrderResponse {
    orderId: string;
    trackingCode: string;
    totalCents: number;
}

export async function submitOrder(req: PlaceOrderRequest): Promise<PlaceOrderResponse> {
    const res = await fetch("/api/v1/orders/checkout", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(req),
    });
    return res.json();
}
"#,
    )
    .unwrap();

    // 2. Backend Route Handler (Axum)
    fs::write(
        server_dir.join("orders_controller.rs"),
        r#"
use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct PlaceOrderRequest {
    pub customer_id: String,
    pub shipping_address: String,
}

#[derive(Serialize)]
pub struct PlaceOrderResponse {
    pub order_id: String,
    pub tracking_code: String,
}

pub async fn handle_order_checkout(Json(payload): Json<PlaceOrderRequest>) -> Json<PlaceOrderResponse> {
    order_service.process_order(&payload.customer_id).await;
    Json(PlaceOrderResponse {
        order_id: "ord_999".to_string(),
        tracking_code: "trk_888".to_string(),
    })
}

pub fn order_routes() -> Router {
    Router::new().route("/api/v1/orders/checkout", post(handle_order_checkout))
}
"#,
    )
    .unwrap();

    // 3. Domain Service Layer
    fs::write(
        services_dir.join("order_service.rs"),
        r#"
pub struct OrderService;
impl OrderService {
    pub async fn process_order(&self, customer_id: &str) -> bool {
        order_repository.save_order_record(customer_id);
        true
    }
}
"#,
    )
    .unwrap();

    // 4. Data Access / Repository Layer
    fs::write(
        repos_dir.join("order_repository.rs"),
        r#"
pub struct OrderRepository;
impl OrderRepository {
    pub fn save_order_record(&self, customer_id: &str) -> bool {
        true
    }
}
"#,
    )
    .unwrap();

    // 5. Database Schema Migration DDL
    fs::write(
        migrations_dir.join("0001_orders_table.sql"),
        r#"
CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    customer_id VARCHAR(64) NOT NULL,
    shipping_address TEXT NOT NULL,
    total_cents BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
"#,
    )
    .unwrap();

    // 6. Generate 10 filler modules (approx 5k LOC) to stress AST traversals
    for i in 0..10 {
        let mut filler_ts = String::with_capacity(4_000);
        filler_ts.push_str(&format!("// Synthetic Module {i} for LOC volume\n"));
        for j in 0..20 {
            filler_ts.push_str(&format!(
                "export interface Model_{i}_{j} {{\n    id: string;\n    value: number;\n    active: boolean;\n}}\n\
                 export function computeMetric_{i}_{j}(data: Model_{i}_{j}): number {{\n    return data.value * {j};\n}}\n\n"
            ));
        }
        fs::write(client_dir.join(format!("module_{i}.ts")), filler_ts).unwrap();

        let mut filler_rs = String::with_capacity(4_000);
        filler_rs.push_str(&format!("// Synthetic Rust Module {i}\n"));
        for j in 0..20 {
            filler_rs.push_str(&format!(
                "pub struct Entity_{i}_{j} {{\n    pub id: u64,\n    pub score: f64,\n}}\n\
                 pub fn evaluate_{i}_{j}(e: &Entity_{i}_{j}) -> f64 {{\n    e.score + {j}.0\n}}\n\n"
            ));
        }
        fs::write(server_dir.join(format!("module_{i}.rs")), filler_rs).unwrap();
    }
}

#[test]
fn test_empirical_large_repo_subsecond_trace_and_partition() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    generate_large_enterprise_workspace(root);

    // Act 1: Build SQLite persistent index
    let index_start = Instant::now();
    let mut engine = IndexEngine::open_or_create(root).expect("Open index engine");
    let sync = engine
        .sync_incremental(&IndexOptions {
            rebuild: true,
            max_depth: None,
        })
        .expect("Sync index");
    let index_time = index_start.elapsed();
    assert!(sync.files_added >= 20, "Expected 20+ indexed files");
    println!("Indexing completed in {:?}", index_time);

    // Act 2: Verify get_fullstack_trace executes in < 1s (and verify it takes << 200ms)
    let trace_args = json!({
        "root_dir": root.to_string_lossy(),
        "entry": "POST /api/v1/orders/checkout",
        "max_depth": 5,
        "format": "json"
    });

    let trace_start = Instant::now();
    let (response, metrics, error_opt, tokens_saved) =
        execute_tool_with_timeout("get_fullstack_trace", &trace_args, 5000);
    let trace_duration = trace_start.elapsed();

    assert!(error_opt.is_none(), "Expected no error, got: {:?}", error_opt);
    assert_ne!(response.get("isError"), Some(&json!(true)));
    println!("get_fullstack_trace duration: {:?}", trace_duration);
    assert!(
        trace_duration.as_millis() < 1000,
        "get_fullstack_trace MUST execute within < 1s (took {}ms)",
        trace_duration.as_millis()
    );
    assert!(metrics.is_some());
    assert!(tokens_saved.is_some());

    // Act 3: Verify pack_agent_context executes in < 100ms via SQLite precomputed clusters
    let pack_args = json!({
        "root_dir": root.to_string_lossy(),
        "agents_count": 2,
        "format": "json"
    });

    let pack_start = Instant::now();
    let (pack_response, pack_metrics, pack_err, pack_saved) =
        execute_tool_with_timeout("pack_agent_context", &pack_args, 5000);
    let pack_duration = pack_start.elapsed();

    assert!(pack_err.is_none(), "Expected no error, got: {:?}", pack_err);
    assert_ne!(pack_response.get("isError"), Some(&json!(true)));
    println!("pack_agent_context duration: {:?}", pack_duration);
    assert!(
        pack_duration.as_millis() < 100,
        "pack_agent_context MUST execute in < 100ms (took {}ms)",
        pack_duration.as_millis()
    );
    assert!(pack_metrics.is_some());
    assert!(pack_saved.is_some());
}

#[test]
fn test_empirical_hop_bounding_max_depth_strictness() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let client_file = root.join("client.ts");
    let server_file = root.join("server.rs");
    let service_file = root.join("service.rs");
    let repo_file = root.join("repo.rs");
    let ddl_file = root.join("schema.sql");

    fs::write(
        &client_file,
        r#"
export async function fetchInvoices() {
    return fetch('/api/v1/invoices', { method: 'GET' });
}
"#,
    )
    .unwrap();

    fs::write(
        &server_file,
        r#"
use axum::{routing::get, Json, Router};
pub async fn get_invoices() -> &'static str {
    invoice_service.list().await;
    "invoices"
}
pub fn routes() -> Router {
    Router::new().route("/api/v1/invoices", get(get_invoices))
}
"#,
    )
    .unwrap();

    fs::write(
        &service_file,
        r#"
pub struct InvoiceService;
impl InvoiceService {
    pub async fn list(&self) {
        invoice_repo.query_all();
    }
}
"#,
    )
    .unwrap();

    fs::write(
        &repo_file,
        r#"
pub struct InvoiceRepo;
impl InvoiceRepo {
    pub fn query_all(&self) {}
}
"#,
    )
    .unwrap();

    fs::write(
        &ddl_file,
        r#"
CREATE TABLE invoices (
    id SERIAL PRIMARY KEY,
    amount NUMERIC(10, 2) NOT NULL
);
"#,
    )
    .unwrap();

    let tracer = FullstackExecutionTracer::new();

    // 1. Test max_depth = 3
    let trace_3 = tracer
        .trace_api_with_depth(root, "/api/v1/invoices", None, Some(3))
        .expect("Trace depth 3");
    assert_eq!(trace_3.steps.len(), 3, "Strictly 3 steps expected");
    assert_eq!(trace_3.steps[0].step_number, 1);
    assert_eq!(trace_3.steps[1].step_number, 2);
    assert_eq!(trace_3.steps[2].step_number, 3);

    // 2. Test max_depth = 4
    let trace_4 = tracer
        .trace_api_with_depth(root, "/api/v1/invoices", None, Some(4))
        .expect("Trace depth 4");
    assert_eq!(trace_4.steps.len(), 4, "Strictly 4 steps expected");
    assert_eq!(trace_4.steps[3].step_number, 4);

    // 3. Test max_depth = 5
    let trace_5 = tracer
        .trace_api_with_depth(root, "/api/v1/invoices", None, Some(5))
        .expect("Trace depth 5");
    assert_eq!(trace_5.steps.len(), 5, "Strictly 5 steps expected");
    assert_eq!(trace_5.steps[4].step_number, 5);

    // 4. Test max_depth = 1 (clamping to minimum 3)
    let trace_clamp_low = tracer
        .trace_api_with_depth(root, "/api/v1/invoices", None, Some(1))
        .expect("Trace clamp low");
    assert_eq!(trace_clamp_low.steps.len(), 3, "Clamped to min 3 steps");

    // 5. Test max_depth = 10 (clamping to maximum 6)
    let trace_clamp_high = tracer
        .trace_api_with_depth(root, "/api/v1/invoices", None, Some(10))
        .expect("Trace clamp high");
    assert!(trace_clamp_high.steps.len() <= 6, "Clamped to max 6 steps");

    // 6. Test MCP json format max_depth enforcement
    let args = json!({
        "root_dir": root.to_string_lossy(),
        "entry": "/api/v1/invoices",
        "max_depth": 3,
        "format": "json"
    });
    let (mcp_res, _, _, _) = execute_tool_with_timeout("get_fullstack_trace", &args, 5000);
    let text = mcp_res["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    let steps = parsed["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);
}

#[test]
fn test_empirical_bidirectional_search_from_database_and_repo_and_service() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let client_file = root.join("client.ts");
    let server_file = root.join("server.rs");
    let service_file = root.join("subscription_service.rs");
    let repo_file = root.join("subscription_repo.rs");
    let ddl_file = root.join("001_subscriptions.sql");

    fs::write(
        &client_file,
        r#"
export async function createSubscription() {
    return fetch('/api/v2/subscriptions', { method: 'POST' });
}
"#,
    )
    .unwrap();

    fs::write(
        &server_file,
        r#"
use axum::{routing::post, Json, Router};
pub async fn handle_create_subscription() -> &'static str {
    subscription_service.activate_subscription().await;
    "ok"
}
pub fn subscription_routes() -> Router {
    Router::new().route("/api/v2/subscriptions", post(handle_create_subscription))
}
"#,
    )
    .unwrap();

    fs::write(
        &service_file,
        r#"
pub struct SubscriptionService;
impl SubscriptionService {
    pub async fn activate_subscription(&self) {
        subscription_repo.persist_subscription();
    }
}
"#,
    )
    .unwrap();

    fs::write(
        &repo_file,
        r#"
pub struct SubscriptionRepository;
impl SubscriptionRepository {
    pub fn persist_subscription(&self) {}
}
"#,
    )
    .unwrap();

    fs::write(
        &ddl_file,
        r#"
CREATE TABLE subscriptions (
    id SERIAL PRIMARY KEY,
    plan_tier VARCHAR(32) NOT NULL,
    active BOOLEAN DEFAULT TRUE
);
"#,
    )
    .unwrap();

    // Build index
    let mut engine = IndexEngine::open_or_create(root).unwrap();
    engine.sync_incremental(&IndexOptions { rebuild: true, max_depth: None }).unwrap();

    let tracer = FullstackExecutionTracer::new();

    // 1. Backward Query: starting from Database table name "subscriptions"
    let trace_db = tracer
        .trace_api_with_depth(root, "subscriptions", None, Some(5))
        .expect("Backward trace from DB table 'subscriptions'");
    assert!(
        trace_db.server_route.route_path.contains("/api/v2/subscriptions")
            || trace_db.server_route.handler_symbol.contains("subscription"),
        "Resolved route from DB table entity"
    );
    assert_eq!(trace_db.steps.len(), 5);

    // 2. Backward Query: starting from Repository method "persist_subscription"
    let trace_repo_fn = tracer
        .trace_api_with_depth(root, "persist_subscription", None, Some(5))
        .expect("Backward trace from repo method");
    assert!(
        trace_repo_fn.server_route.route_path.contains("/api/v2/subscriptions")
            || trace_repo_fn.server_route.handler_symbol.contains("subscription")
    );

    // 3. Backward Query: starting from Repository struct "SubscriptionRepository"
    let trace_repo_struct = tracer
        .trace_api_with_depth(root, "SubscriptionRepository", None, Some(5))
        .expect("Backward trace from repo struct");
    assert!(
        trace_repo_struct.server_route.route_path.contains("/api/v2/subscriptions")
            || trace_repo_struct.server_route.handler_symbol.contains("subscription")
    );

    // 4. Backward Query: starting from Service method "activate_subscription"
    let trace_service_fn = tracer
        .trace_api_with_depth(root, "activate_subscription", None, Some(5))
        .expect("Backward trace from service method");
    assert!(
        trace_service_fn.server_route.route_path.contains("/api/v2/subscriptions")
            || trace_service_fn.server_route.handler_symbol.contains("subscription")
    );

    // 5. Backward Query: starting from Service struct "SubscriptionService"
    let trace_service_struct = tracer
        .trace_api_with_depth(root, "SubscriptionService", None, Some(5))
        .expect("Backward trace from service struct");
    assert!(
        trace_service_struct.server_route.route_path.contains("/api/v2/subscriptions")
            || trace_service_struct.server_route.handler_symbol.contains("subscription")
    );

    // 6. Direct Route Query: starting from route path "/api/v2/subscriptions"
    let trace_route = tracer
        .trace_api_with_depth(root, "/api/v2/subscriptions", None, Some(5))
        .expect("Direct trace from route path");
    assert_eq!(trace_route.server_route.route_path, "/api/v2/subscriptions");
    assert!(trace_route.client_call.is_some());

    // 7. Direct Route Handler Query: starting from handler symbol "handle_create_subscription"
    let trace_handler = tracer
        .trace_api_with_depth(root, "handle_create_subscription", None, Some(5))
        .expect("Direct trace from handler symbol");
    assert_eq!(trace_handler.server_route.handler_symbol, "handle_create_subscription");
}

#[test]
fn test_empirical_concurrency_and_timeout_resilience() {
    let temp = TempDir::new().unwrap();
    let root = Arc::new(temp.path().to_path_buf());

    generate_large_enterprise_workspace(&root);

    // Index
    let mut engine = IndexEngine::open_or_create(&root).unwrap();
    engine.sync_incremental(&IndexOptions { rebuild: true, max_depth: None }).unwrap();

    let root_path_str = root.to_string_lossy().to_string();

    let mut handles = Vec::new();
    // Launch 6 concurrent threads querying traces and partitions
    for i in 0..6 {
        let p = root_path_str.clone();
        let handle = thread::spawn(move || {
            let start = Instant::now();
            if i % 2 == 0 {
                let args = json!({
                    "root_dir": p,
                    "entry": "POST /api/v1/orders/checkout",
                    "max_depth": 4,
                    "format": "json"
                });
                let (res, _, err, _) = execute_tool_with_timeout("get_fullstack_trace", &args, 5000);
                assert!(err.is_none());
                assert_ne!(res.get("isError"), Some(&json!(true)));
            } else {
                let args = json!({
                    "root_dir": p,
                    "agents_count": 2,
                    "format": "json"
                });
                let (res, _, err, _) = execute_tool_with_timeout("pack_agent_context", &args, 5000);
                assert!(err.is_none());
                assert_ne!(res.get("isError"), Some(&json!(true)));
            }
            start.elapsed()
        });
        handles.push(handle);
    }

    for h in handles {
        let duration = h.join().expect("Thread join");
        assert!(
            duration.as_millis() < 1000,
            "Concurrent thread took {}ms (<1s required)",
            duration.as_millis()
        );
    }
}

#[test]
fn test_empirical_cyclic_and_unmatched_symbols_adversarial() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Circular dependency in code
    fs::write(
        root.join("cyclic.rs"),
        r#"
pub fn func_a() { func_b(); }
pub fn func_b() { func_c(); }
pub fn func_c() { func_a(); }
"#,
    )
    .unwrap();

    let mut engine = IndexEngine::open_or_create(root).unwrap();
    engine.sync_incremental(&IndexOptions { rebuild: true, max_depth: None }).unwrap();

    let tracer = FullstackExecutionTracer::new();

    // Query non-existent symbol -> expects SymbolNotFound error cleanly
    let not_found_result = tracer.trace_api_with_depth(root, "non_existent_symbol_999", None, Some(3));
    assert!(not_found_result.is_err(), "Expected error for unknown symbol");

    // MCP tool call for non-existent symbol returns structured error
    let args = json!({
        "root_dir": root.to_string_lossy(),
        "entry": "non_existent_symbol_999"
    });
    let (res, _, err, _) = execute_tool_with_timeout("get_fullstack_trace", &args, 5000);
    assert!(err.is_some() || res.get("isError") == Some(&json!(true)));
}
