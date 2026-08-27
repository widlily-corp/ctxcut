//! Comprehensive unit and integration tests for Full-Stack Cross-Boundary Execution Tracing (R1).

use ctxcut_core::framework::extract_server_routes;
use ctxcut_core::fullstack::{
    ClientDetector, FullstackExecutionTracer, FullstackTracer, RouteMatcher,
};
use ctxcut_core::index::{IndexEngine, IndexOptions};
use ctxcut_core::schema::extract_schema_entities;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn test_client_detector_fetch_and_axios() {
    let detector = ClientDetector::new();
    let code = r#"
        import axios from 'axios';

        export async function fetchUsers() {
            const res = await fetch('/api/v1/users', {
                method: 'GET',
                headers: { 'Authorization': 'Bearer xxx' }
            });
            return res.json();
        }

        export async function createUser(data: CreateUserDto) {
            const resp = await axios.post<UserResponse>('/api/v1/users', data);
            return resp.data;
        }
    "#;

    let calls = detector.detect_in_file(Path::new("src/api/users.ts"), code);
    assert_eq!(calls.len(), 2);

    let fetch_call = calls.iter().find(|c| c.client_kind == "fetch").unwrap();
    assert_eq!(fetch_call.endpoint_url.as_deref(), Some("/api/v1/users"));
    assert_eq!(fetch_call.http_method.as_deref(), Some("GET"));

    let axios_call = calls.iter().find(|c| c.client_kind == "axios").unwrap();
    assert_eq!(axios_call.endpoint_url.as_deref(), Some("/api/v1/users"));
    assert_eq!(axios_call.http_method.as_deref(), Some("POST"));
    assert_eq!(axios_call.request_dto.as_deref(), Some("CreateUserDto"));
    assert_eq!(axios_call.response_dto.as_deref(), Some("UserResponse"));
}

#[test]
fn test_client_detector_react_query_and_trpc() {
    let detector = ClientDetector::new();
    let code = r#"
        import { useQuery, useMutation } from '@tanstack/react-query';
        import { trpc } from '../utils/trpc';

        export function useUserData(userId: string) {
            const query = useQuery(['user', userId], () => fetch(`/api/users/${userId}`));
            const mutation = useMutation((newUser) => axios.post('/api/users', newUser));
            const trpcUser = trpc.user.getById.useQuery({ id: userId });
            const trpcLogin = trpc.auth.login.useMutation();

            return { query, mutation, trpcUser, trpcLogin };
        }
    "#;

    let calls = detector.detect_in_file(Path::new("src/hooks/useUser.tsx"), code);
    assert!(calls.len() >= 3, "Expected at least 3 calls, found {}", calls.len());

    let has_trpc = calls.iter().any(|c| c.client_kind == "trpc" && c.rpc_procedure.as_deref() == Some("user.getById"));
    assert!(has_trpc, "Should detect trpc.user.getById call");

    let has_react_query = calls.iter().any(|c| c.client_kind == "react_query" || c.client_kind == "fetch");
    assert!(has_react_query, "Should detect React Query or fetch invocation");
}

#[test]
fn test_client_detector_graphql_and_grpc() {
    let detector = ClientDetector::new();
    let code = r#"
        import { gql, useQuery } from '@apollo/client';

        const GET_USER_PROFILE = gql`
            query GetUserProfile($id: ID!) {
                user(id: $id) { id, name, email }
            }
        `;

        export function UserProfile() {
            const { data } = useQuery(GET_USER_PROFILE);
            userService.getUserProfile({ userId: '123' });
            return null;
        }
    "#;

    let calls = detector.detect_in_file(Path::new("src/components/UserProfile.tsx"), code);
    assert!(!calls.is_empty(), "Should extract GraphQL and/or gRPC calls");

    let has_graphql = calls.iter().any(|c| c.client_kind == "graphql");
    assert!(has_graphql, "Should identify GraphQL query");
}

#[test]
fn test_server_route_extractors_polyglot() {
    // 1. Axum (Rust)
    let axum_code = r#"
        use axum::{routing::{get, post}, Router, Json};

        pub async fn list_users() -> Json<Vec<UserDto>> { ... }
        pub async fn create_user(Json(payload): Json<CreateUserRequest>) -> Json<UserDto> { ... }

        pub fn app() -> Router {
            Router::new()
                .route("/api/users", get(list_users).post(create_user))
        }
    "#;
    let axum_routes = extract_server_routes(Path::new("src/routes.rs"), axum_code);
    assert_eq!(axum_routes.len(), 2);
    assert!(axum_routes.iter().any(|r| r.framework == "axum" && r.http_method == "GET" && r.route_path == "/api/users"));
    assert!(axum_routes.iter().any(|r| r.framework == "axum" && r.http_method == "POST" && r.handler_symbol == "create_user"));

    // 2. Actix-web (Rust)
    let actix_code = r#"
        use actix_web::{get, post, web, HttpResponse};

        #[get("/api/items")]
        pub async fn get_items() -> HttpResponse { ... }

        #[post("/api/items")]
        pub async fn add_item(body: web::Json<AddItemDto>) -> HttpResponse { ... }
    "#;
    let actix_routes = extract_server_routes(Path::new("src/handlers.rs"), actix_code);
    assert_eq!(actix_routes.len(), 2);
    assert_eq!(actix_routes[0].framework, "actix");

    // 3. Gin (Go)
    let gin_code = r#"
        package main
        import "github.com/gin-gonic/gin"

        func CreateOrder(c *gin.Context) { ... }
        func SetupRouter() *gin.Engine {
            r := gin.Default()
            r.POST("/api/orders", CreateOrder)
            return r
        }
    "#;
    let gin_routes = extract_server_routes(Path::new("main.go"), gin_code);
    assert_eq!(gin_routes.len(), 1);
    assert_eq!(gin_routes[0].framework, "gin");
    assert_eq!(gin_routes[0].http_method, "POST");
    assert_eq!(gin_routes[0].route_path, "/api/orders");
    assert_eq!(gin_routes[0].handler_symbol, "CreateOrder");

    // 4. FastAPI (Python)
    let fastapi_code = r#"
        from fastapi import FastAPI, Depends
        app = FastAPI()

        @app.post("/api/checkout", response_model=CheckoutResponse)
        async def checkout(order: OrderDto, user: User = Depends(get_current_user)):
            return {"status": "ok"}
    "#;
    let fastapi_routes = extract_server_routes(Path::new("app/main.py"), fastapi_code);
    assert_eq!(fastapi_routes.len(), 1);
    assert_eq!(fastapi_routes[0].framework, "fastapi");
    assert_eq!(fastapi_routes[0].http_method, "POST");
    assert_eq!(fastapi_routes[0].route_path, "/api/checkout");

    // 5. ASP.NET Core (C#)
    let aspnet_code = r#"
        using Microsoft.AspNetCore.Mvc;

        [ApiController]
        [Route("api/[controller]")]
        public class UsersController : ControllerBase {
            [HttpGet("{id}")]
            public async Task<IActionResult> GetUser(string id) { ... }

            [HttpPost]
            public async Task<IActionResult> CreateUser([FromBody] CreateUserDto dto) { ... }
        }
    "#;
    let aspnet_routes = extract_server_routes(Path::new("Controllers/UsersController.cs"), aspnet_code);
    assert_eq!(aspnet_routes.len(), 2);
    assert_eq!(aspnet_routes[0].framework, "aspnetcore");

    // 6. Spring Boot (Java)
    let spring_code = r#"
        package com.example.demo;
        import org.springframework.web.bind.annotation.*;

        @RestController
        @RequestMapping("/api/products")
        public class ProductController {
            @GetMapping
            public List<ProductDto> listProducts() { ... }

            @PostMapping
            public ProductDto createProduct(@RequestBody CreateProductDto dto) { ... }
        }
    "#;
    let spring_routes = extract_server_routes(Path::new("src/main/java/ProductController.java"), spring_code);
    assert_eq!(spring_routes.len(), 2);
    assert_eq!(spring_routes[0].framework, "spring_boot");
}

#[test]
fn test_schema_entities_extraction() {
    // 1. SQL Migrations
    let sql_ddl = r#"
        CREATE TABLE users (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            email VARCHAR(255) NOT NULL UNIQUE,
            password_hash VARCHAR(255) NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        );
    "#;
    let sql_schemas = extract_schema_entities(Path::new("migrations/001_create_users.sql"), sql_ddl);
    assert_eq!(sql_schemas.len(), 1);
    assert_eq!(sql_schemas[0].schema_kind, "sql_table");
    assert_eq!(sql_schemas[0].entity_name, "users");

    // 2. Prisma Schema
    let prisma_code = r#"
        model Account {
            id        String   @id @default(cuid())
            userId    String
            createdAt DateTime @default(now())
        }
    "#;
    let prisma_schemas = extract_schema_entities(Path::new("prisma/schema.prisma"), prisma_code);
    assert_eq!(prisma_schemas.len(), 1);
    assert_eq!(prisma_schemas[0].schema_kind, "prisma_model");
    assert_eq!(prisma_schemas[0].entity_name, "Account");

    // 3. Drizzle Table
    let drizzle_code = r#"
        import { pgTable, serial, text, timestamp } from 'drizzle-orm/pg-core';

        export const orders = pgTable('orders', {
            id: serial('id').primaryKey(),
            total: text('total').notNull(),
            createdAt: timestamp('created_at').defaultNow(),
        });
    "#;
    let drizzle_schemas = extract_schema_entities(Path::new("src/schema/orders.ts"), drizzle_code);
    assert_eq!(drizzle_schemas.len(), 1);
    assert_eq!(drizzle_schemas[0].schema_kind, "drizzle_table");
    assert_eq!(drizzle_schemas[0].entity_name, "orders");
}

#[test]
fn test_route_matcher_normalization() {
    let matcher = RouteMatcher::new();

    assert!(matcher.paths_match("/api/users/:id", "/api/users/123"));
    assert!(matcher.paths_match("/api/users/{id}", "/api/users/456"));
    assert!(matcher.paths_match("/api/v1/orders/${orderId}", "/api/v1/orders/abc-999"));
    assert!(matcher.paths_match("/api/posts/:postId/comments/:commentId", "/api/posts/10/comments/20"));
    assert!(!matcher.paths_match("/api/users", "/api/products"));
}

#[test]
fn test_end_to_end_6_step_fullstack_trace() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Create client file
    let client_dir = root.join("frontend/src");
    fs::create_dir_all(&client_dir).unwrap();
    let mut client_file = File::create(client_dir.join("api.ts")).unwrap();
    writeln!(
        client_file,
        r#"
        import axios from 'axios';
        export async function createNewUser(payload: CreateUserDto) {{
            const resp = await axios.post<UserResponse>('/api/v1/users', payload);
            return resp.data;
        }}
        "#
    ).unwrap();

    // 2. Create server route handler file
    let server_dir = root.join("backend/src");
    fs::create_dir_all(&server_dir).unwrap();
    let mut server_file = File::create(server_dir.join("main.rs")).unwrap();
    writeln!(
        server_file,
        r#"
        use axum::{{routing::post, Router, Json}};

        pub struct CreateUserDto {{ pub email: String, pub name: String }}

        pub async fn create_user(
            auth: AuthUser,
            Json(payload): Json<CreateUserDto>
        ) -> Json<UserResponse> {{
            let user = UserService::create_user(payload).await;
            Json(user)
        }}

        pub fn create_router() -> Router {{
            Router::new().route("/api/v1/users", post(create_user))
        }}
        "#
    ).unwrap();

    // 3. Create service file
    let mut service_file = File::create(server_dir.join("service.rs")).unwrap();
    writeln!(
        service_file,
        r#"
        pub struct UserService;
        impl UserService {{
            pub async fn create_user(dto: CreateUserDto) -> UserResponse {{
                let db_user = UserRepository::insert_user(&dto.email).await;
                UserResponse {{ id: db_user.id, email: dto.email }}
            }}
        }}
        "#
    ).unwrap();

    // 4. Create repository file
    let mut repo_file = File::create(server_dir.join("repository.rs")).unwrap();
    writeln!(
        repo_file,
        r#"
        pub struct UserRepository;
        impl UserRepository {{
            pub async fn insert_user(email: &str) -> DbUser {{
                sqlx::query!("INSERT INTO users (email) VALUES ($1)", email);
                DbUser {{ id: 1, email: email.to_string() }}
            }}
        }}
        "#
    ).unwrap();

    // 5. Create SQL migration DDL file
    let mig_dir = root.join("migrations");
    fs::create_dir_all(&mig_dir).unwrap();
    let mut ddl_file = File::create(mig_dir.join("0001_create_users.sql")).unwrap();
    writeln!(
        ddl_file,
        r#"
        CREATE TABLE users (
            id SERIAL PRIMARY KEY,
            email VARCHAR(255) NOT NULL UNIQUE,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        );
        "#
    ).unwrap();

    // Trace the API endpoint
    let tracer = FullstackExecutionTracer::new();
    let trace_result = tracer.trace_api(root, "/api/v1/users", Some(1800)).unwrap();

    assert_eq!(trace_result.query_endpoint, "/api/v1/users");
    assert_eq!(trace_result.total_steps, 6, "Must produce a complete 6-step execution flow");
    assert_eq!(trace_result.steps.len(), 6);

    // Verify all 6 layers are populated in order
    assert_eq!(trace_result.steps[0].layer, "client_call");
    assert_eq!(trace_result.steps[1].layer, "route_handler");
    assert_eq!(trace_result.steps[2].layer, "middleware_guard");
    assert_eq!(trace_result.steps[3].layer, "service_logic");
    assert_eq!(trace_result.steps[4].layer, "data_access");
    assert_eq!(trace_result.steps[5].layer, "schema_ddl");

    // Check token budget (1,500 - 2,000 tokens)
    let total_tokens = trace_result.stats.sliced_tokens;
    assert!(total_tokens <= 2000, "Token count {} should be <= 2000 tokens budget", total_tokens);
    assert!(trace_result.stats.savings_percentage > 0.0, "Should achieve token reduction vs raw files");
}

#[test]
fn test_sqlite_persistent_caching_sub_5ms() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();

    // Create sample files
    let src_dir = ws_root.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let mut route_file = File::create(src_dir.join("api.rs")).unwrap();
    writeln!(
        route_file,
        r#"
        use axum::{{routing::get, Router, Json}};
        pub async fn list_items() -> Json<Vec<String>> {{ Json(vec![]) }}
        pub fn router() -> Router {{ Router::new().route("/api/v1/items", get(list_items)) }}
        "#
    ).unwrap();

    let mut client_file = File::create(src_dir.join("client.ts")).unwrap();
    writeln!(
        client_file,
        r#"
        export async function getItems() {{
            return fetch('/api/v1/items');
        }}
        "#
    ).unwrap();

    let mig_dir = ws_root.join("migrations");
    fs::create_dir_all(&mig_dir).unwrap();
    let mut ddl_file = File::create(mig_dir.join("001_items.sql")).unwrap();
    writeln!(
        ddl_file,
        "CREATE TABLE items ( id SERIAL PRIMARY KEY, name TEXT NOT NULL );"
    ).unwrap();

    // Initialize IndexEngine
    let mut engine = IndexEngine::open_or_create(ws_root).unwrap();
    let sync_res = engine.sync_incremental(&IndexOptions::default()).unwrap();
    assert!(sync_res.files_added >= 2);

    // Measure lookup latency for find_routes_by_path
    let t0 = Instant::now();
    let routes = engine.find_routes_by_path("/api/v1/items").unwrap();
    let d_routes = t0.elapsed();
    assert!(!routes.is_empty(), "Should find indexed route");
    assert!(d_routes.as_millis() < 5, "Route lookup should be sub-5ms, was {:?}", d_routes);

    // Measure lookup latency for find_client_endpoints_by_url_or_proc
    let t1 = Instant::now();
    let clients = engine.find_client_endpoints_by_url_or_proc("/api/v1/items").unwrap();
    let d_clients = t1.elapsed();
    assert!(!clients.is_empty(), "Should find indexed client endpoint");
    assert!(d_clients.as_millis() < 5, "Client lookup should be sub-5ms, was {:?}", d_clients);

    // Measure lookup latency for find_schema_entities
    let t2 = Instant::now();
    let schemas = engine.find_schema_entities("items").unwrap();
    let d_schemas = t2.elapsed();
    assert!(!schemas.is_empty(), "Should find indexed schema entity");
    assert!(d_schemas.as_millis() < 5, "Schema lookup should be sub-5ms, was {:?}", d_schemas);
}
