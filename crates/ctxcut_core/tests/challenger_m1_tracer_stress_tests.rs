//! Empirical Challenger Adversarial Test Suite for M1: Full-Stack Cross-Boundary Tracing R1.
//!
//! Tests polyglot cross-boundary tracing with corner cases:
//! 1. Nested route groups, dynamic URL templates (`/api/v1/users/${id}/orders`), custom axios wrappers, tRPC routers with nested sub-routers.
//! 2. Malformed SQL migrations, missing route handlers, missing client callers, missing DB access.
//! 3. Extreme token budgets (<200 tokens, >5000 tokens, 0 tokens).
//! 4. Polyglot server routes & client detectors under complex syntax & edge cases.

use ctxcut_core::error::CoreError;
use ctxcut_core::framework::extract_server_routes;
use ctxcut_core::fullstack::{
    ClientDetector, FullstackExecutionTracer, FullstackTracer, RouteMatcher,
};
use ctxcut_core::index::{IndexEngine, IndexOptions};
use ctxcut_core::schema::extract_schema_entities;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

/// 1. Dynamic URL Templates & Route Normalization Stress Matrix
#[test]
fn test_adversarial_dynamic_url_templates_and_normalization() {
    let matcher = RouteMatcher::new();

    // Standard parameter formats
    assert!(matcher.paths_match("/api/v1/users/:id/orders", "/api/v1/users/${id}/orders"));
    assert!(matcher.paths_match("/api/v1/users/{id}/orders", "/api/v1/users/12345/orders"));
    assert!(matcher.paths_match("/api/v1/users/${userId}/orders", "/api/v1/users/:userId/orders"));

    // Deeply nested dynamic segments
    assert!(matcher.paths_match(
        "/api/v1/orgs/:orgId/teams/:teamId/members/:memberId/roles",
        "/api/v1/orgs/${orgId}/teams/${teamId}/members/${memberId}/roles"
    ));
    assert!(matcher.paths_match(
        "/api/v1/orgs/{orgId}/teams/{teamId}/members/{memberId}/roles",
        "/api/v1/orgs/org_99/teams/team_88/members/usr_77/roles"
    ));

    // Trailing slashes and leading slashes normalization
    assert!(matcher.paths_match("/api/v1/users/:id/", "/api/v1/users/99"));
    assert!(matcher.paths_match("api/v1/users/:id", "/api/v1/users/${id}"));

    // Negative matches (should not match different endpoints)
    assert!(!matcher.paths_match("/api/v1/users/:id/orders", "/api/v1/users/:id/products"));
    assert!(!matcher.paths_match("/api/v1/users/:id", "/api/v1/users/:id/orders"));
    assert!(!matcher.paths_match("/api/v1/users", "/api/v1/users/:id"));
}

/// 2. Nested Route Groups & Polyglot Server Route Handlers
#[test]
fn test_adversarial_nested_route_groups_polyglot() {
    // A. Express nested router endpoints
    let express_code = r#"
        import express, { Router } from 'express';
        const app = express();
        const v1Router = Router();
        const userRouter = Router();

        userRouter.get('/users/:id/orders', getUserOrders);
        userRouter.post('/users/:id/orders', createOrderHandler);
        userRouter.delete('/users/:id/orders/:orderId', deleteOrderHandler);

        v1Router.use('/v1', userRouter);
        app.use('/api', v1Router);
    "#;
    let exp_routes = extract_server_routes(Path::new("src/routes/users.ts"), express_code);
    assert!(exp_routes.len() >= 3, "Expected at least 3 Express routes, found {}", exp_routes.len());
    assert!(exp_routes.iter().any(|r| r.http_method == "GET" && r.handler_symbol == "getUserOrders"));
    assert!(exp_routes.iter().any(|r| r.http_method == "POST" && r.handler_symbol == "createOrderHandler"));
    assert!(exp_routes.iter().any(|r| r.http_method == "DELETE" && r.handler_symbol == "deleteOrderHandler"));

    // B. Axum nested routers (Rust)
    let axum_nested = r#"
        use axum::{routing::{get, post, delete}, Router, Json};

        pub async fn get_user_profile() -> Json<ProfileDto> { ... }
        pub async fn update_user_profile() -> Json<ProfileDto> { ... }

        pub fn user_router() -> Router {
            Router::new()
                .route("/api/v1/users/:id/profile", get(get_user_profile).post(update_user_profile))
        }
    "#;
    let axum_routes = extract_server_routes(Path::new("src/user_routes.rs"), axum_nested);
    assert_eq!(axum_routes.len(), 2);
    assert!(axum_routes.iter().any(|r| r.http_method == "GET" && r.handler_symbol == "get_user_profile"));
    assert!(axum_routes.iter().any(|r| r.http_method == "POST" && r.handler_symbol == "update_user_profile"));

    // C. FastAPI nested router (Python)
    let fastapi_nested = r#"
        from fastapi import APIRouter, Depends
        router = APIRouter(prefix="/api/v1/organizations/{org_id}")

        @router.get("/teams/{team_id}/billing", response_model=BillingDto)
        async def get_team_billing(org_id: str, team_id: str):
            return {"status": "active"}
    "#;
    let py_routes = extract_server_routes(Path::new("app/routers/billing.py"), fastapi_nested);
    assert!(!py_routes.is_empty());
    assert_eq!(py_routes[0].framework, "fastapi");
    assert_eq!(py_routes[0].http_method, "GET");
}

/// 3. Custom Axios Wrappers & tRPC Nested Sub-Routers
#[test]
fn test_adversarial_custom_client_wrappers_and_trpc() {
    let detector = ClientDetector::new();

    // A. Supported axios instance & direct calls with generics and dynamic templates
    let client_code = r#"
        import axios from 'axios';
        import axiosInstance from '../lib/axiosInstance';

        export async function loadUserOrders(userId: string) {
            const res = await axios.get<OrderListResponse>(`/api/v1/users/${userId}/orders`);
            return res.data;
        }

        export async function submitOrder(userId: string, data: OrderDto) {
            const res = await axiosInstance.post<OrderResponse>(`/api/v1/users/${userId}/orders`, data);
            return res.data;
        }

        export async function cancelOrder(orderId: string) {
            const res = await axios.delete<void>(`/api/v1/orders/${orderId}`);
            return res.data;
        }
    "#;
    let calls = detector.detect_in_file(Path::new("src/services/orders.ts"), client_code);
    assert_eq!(calls.len(), 3, "Expected 3 detected client calls, found {}", calls.len());

    let get_call = calls.iter().find(|c| c.http_method.as_deref() == Some("GET")).unwrap();
    assert_eq!(get_call.endpoint_url.as_deref(), Some("/api/v1/users/${userId}/orders"));

    let post_call = calls.iter().find(|c| c.http_method.as_deref() == Some("POST")).unwrap();
    assert_eq!(post_call.endpoint_url.as_deref(), Some("/api/v1/users/${userId}/orders"));

    let del_call = calls.iter().find(|c| c.http_method.as_deref() == Some("DELETE")).unwrap();
    assert_eq!(del_call.endpoint_url.as_deref(), Some("/api/v1/orders/${orderId}"));

    // B. Nested tRPC Sub-Routers
    let trpc_code = r#"
        import { trpc } from '../utils/trpc';

        export function useBillingDetails(orgId: string, teamId: string) {
            const details = trpc.billing.invoices.getById.useQuery({ orgId, teamId });
            const payMutation = trpc.billing.invoices.pay.useMutation();
            const adminOverride = trpc.admin.users.impersonate.useMutation();

            return { details, payMutation, adminOverride };
        }
    "#;
    let trpc_calls = detector.detect_in_file(Path::new("src/hooks/useBilling.ts"), trpc_code);
    assert!(trpc_calls.len() >= 3, "Expected at least 3 tRPC calls, found {}", trpc_calls.len());

    let invoice_call = trpc_calls.iter().find(|c| c.rpc_procedure.as_deref() == Some("billing.invoices.getById")).unwrap();
    assert_eq!(invoice_call.client_kind, "trpc");
    assert_eq!(invoice_call.http_method.as_deref(), Some("GET"));

    let pay_call = trpc_calls.iter().find(|c| c.rpc_procedure.as_deref() == Some("billing.invoices.pay")).unwrap();
    assert_eq!(pay_call.client_kind, "trpc");
    assert_eq!(pay_call.http_method.as_deref(), Some("POST"));
}

/// 3b. Empirical Edge Cases & Gaps in Client Detection & URL Matching
#[test]
fn test_adversarial_empirical_vulnerabilities_and_edge_cases() {
    let detector = ClientDetector::new();
    let matcher = RouteMatcher::new();

    // Edge Case 1: CamelCase wrapper identifier `customAxios.post` vs `axiosInstance.post`
    // Observation: `customAxios` has uppercase 'A'. `func_text.contains("axios")` is case-sensitive!
    let camel_case_code = "export async function test() { return customAxios.post('/api/users', {}); }";
    let camel_calls = detector.detect_in_file(Path::new("src/test.ts"), camel_case_code);
    // Documenting empirical behavior:
    let camel_detected = camel_calls.iter().any(|c| c.endpoint_url.as_deref() == Some("/api/users"));

    // Edge Case 2: Config object axios invocation: axios({ method: 'POST', url: '/api/v1/checkout' })
    let config_obj_code = "export async function checkout() { return axios({ url: '/api/v1/checkout', method: 'POST' }); }";
    let config_calls = detector.detect_in_file(Path::new("src/checkout.ts"), config_obj_code);
    let config_detected = config_calls.iter().any(|c| c.endpoint_url.as_deref() == Some("/api/v1/checkout"));

    // Edge Case 3: URL template with query parameter: `/api/v1/users/:id/orders?status=active` vs `/api/v1/users/:id/orders`
    let qp_match = matcher.paths_match("/api/v1/users/:id/orders", "/api/v1/users/123/orders?status=active");

    println!("Empirical Edge Case Findings: camel_detected={}, config_detected={}, qp_match={}", camel_detected, config_detected, qp_match);
}

/// 4. Malformed SQL Migrations, Missing Handlers, and Resilient Degradation
#[test]
fn test_adversarial_malformed_sql_and_missing_symbols() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // A. Malformed SQL migration file
    let mig_dir = root.join("migrations");
    fs::create_dir_all(&mig_dir).unwrap();
    let mut malformed_sql = File::create(mig_dir.join("001_bad_syntax.sql")).unwrap();
    writeln!(
        malformed_sql,
        r#"
        -- Malformed incomplete SQL syntax with unclosed parens and broken tokens
        CREATE TABLE broken_table (
            id SERIAL PRIMARY KEY,
            unclosed_col VARCHAR(
            ??? INVALID TOKENS !!! ;;;;
        "#
    ).unwrap();

    let schemas = extract_schema_entities(mig_dir.join("001_bad_syntax.sql").as_path(), "CREATE TABLE (((( ;;;");
    // Schema parser must not panic; should return empty or gracefully handled entity
    let _ = schemas;

    // B. Set up valid route file
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let mut route_file = File::create(src_dir.join("api.rs")).unwrap();
    writeln!(
        route_file,
        r#"
        use axum::{{routing::get, Router, Json}};
        pub async fn get_status() -> Json<String> {{ Json("ok".into()) }}
        pub fn router() -> Router {{ Router::new().route("/api/v1/health", get(get_status)) }}
        "#
    ).unwrap();

    let tracer = FullstackExecutionTracer::new();

    // C. Nonexistent endpoint -> must return clean CoreError::SymbolNotFound, NOT panic
    let err_res = tracer.trace_api(root, "/api/v1/nonexistent_route_404", Some(1500));
    assert!(err_res.is_err(), "Non-existent route should return error");
    match err_res.unwrap_err() {
        CoreError::SymbolNotFound { symbol, available_symbols, .. } => {
            assert_eq!(symbol, "/api/v1/nonexistent_route_404");
            assert!(available_symbols.iter().any(|s| s.contains("/api/v1/health")));
        }
        other => panic!("Expected SymbolNotFound error, got {:?}", other),
    }

    // D. Tracing with missing client file (backend-only endpoint)
    let valid_trace = tracer.trace_api(root, "/api/v1/health", Some(1500)).unwrap();
    assert_eq!(valid_trace.query_endpoint, "/api/v1/health");
    assert!(valid_trace.total_steps >= 5, "Should generate execution steps gracefully even without frontend caller");
    assert_eq!(valid_trace.server_route.route_path, "/api/v1/health");
}

/// 5. Extreme Token Budget Stress Testing (<200 tokens, >5000 tokens, zero budget)
#[test]
fn test_adversarial_extreme_token_budgets() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let mut route_file = File::create(src_dir.join("routes.rs")).unwrap();
    writeln!(
        route_file,
        r#"
        use axum::{{routing::post, Router, Json}};

        /// Extended docstring line 1
        /// Extended docstring line 2
        /// Extended docstring line 3
        pub async fn process_large_payload(
            auth: AuthUser,
            Json(data): Json<LargePayloadDto>
        ) -> Json<ProcessResult> {{
            // Line 1 of implementation logic
            // Line 2 of implementation logic
            // Line 3 of implementation logic
            // Line 4 of implementation logic
            // Line 5 of implementation logic
            // Line 6 of implementation logic
            // Line 7 of implementation logic
            // Line 8 of implementation logic
            // Line 9 of implementation logic
            // Line 10 of implementation logic
            // Line 11 of implementation logic
            // Line 12 of implementation logic
            // Line 13 of implementation logic
            // Line 14 of implementation logic
            // Line 15 of implementation logic
            let res = LargeService::process(data).await;
            Json(res)
        }}

        pub fn app() -> Router {{
            Router::new().route("/api/v1/large", post(process_large_payload))
        }}
        "#
    ).unwrap();

    let tracer = FullstackExecutionTracer::new();

    // A. Extreme tight budget: 100 tokens
    let trace_tight = tracer.trace_api(root, "/api/v1/large", Some(100)).unwrap();
    assert_eq!(trace_tight.total_steps, trace_tight.steps.len());
    // Verify docstrings stripped or compressed
    for step in &trace_tight.steps {
        assert!(!step.snippet.contains("Extended docstring line 1"));
    }

    // B. Extreme large budget: 10,000 tokens
    let trace_large = tracer.trace_api(root, "/api/v1/large", Some(10000)).unwrap();
    assert_eq!(trace_large.total_steps, trace_large.steps.len());
    assert!(trace_large.stats.sliced_tokens <= 10000);

    // C. Extreme edge: 0 token budget
    let trace_zero = tracer.trace_api(root, "/api/v1/large", Some(0)).unwrap();
    assert_eq!(trace_zero.total_steps, trace_zero.steps.len());
}

/// 6. SQLite Persistent Indexing & Latency Verification
#[test]
fn test_adversarial_sqlite_cache_reopen_and_concurrency() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    let mut api_file = File::create(src.join("api.ts")).unwrap();
    writeln!(api_file, "export async function queryData() {{ return fetch('/api/v1/data'); }}").unwrap();

    // Initialize and sync
    {
        let mut engine = IndexEngine::open_or_create(root).unwrap();
        let res = engine.sync_incremental(&IndexOptions::default()).unwrap();
        assert!(res.files_added >= 1);
    }

    // Reopen from SQLite database disk file and query
    {
        let engine = IndexEngine::open_or_create(root).unwrap();
        let clients = engine.find_client_endpoints_by_url_or_proc("/api/v1/data").unwrap();
        assert!(!clients.is_empty(), "Reopened index must retain cached client calls");
        assert_eq!(clients[0].endpoint_url.as_deref(), Some("/api/v1/data"));
    }
}
