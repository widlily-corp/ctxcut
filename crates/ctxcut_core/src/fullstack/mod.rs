//! Full-Stack Cross-Boundary Execution Tracing Subsystem (R1).
//!
//! Provides automated discovery, correlation, and end-to-end tracing across:
//! 1. Frontend Client API Invocations (`fetch`, `axios`, React Query, `tRPC`, GraphQL, `gRPC-web`)
//! 2. Backend Route Entrypoints & Controllers (Axum, Actix-web, Gin, Chi, FastAPI, Flask, ASP.NET Core, Spring Boot)
//! 3. Route Guards, Middleware & Authentication Extractors
//! 4. Domain Service Business Logic
//! 5. Data Access Repositories & Database Queries
//! 6. Database DDL & Data Contracts (SQL Migrations, Prisma, Drizzle, TypeORM, GraphQL SDL, Protobuf)
//!
//! Features adaptive token budgeting (1,500 - 2,000 tokens) with progressive compression.

/// Client-side API call and RPC detector.
pub mod client_detect;
/// Fullstack execution trace data models and contracts.
pub mod model;
/// Server route and RPC procedure matcher.
pub mod route_matcher;
/// 6-step linear execution flow tracer.
pub mod tracer;

pub use client_detect::ClientDetector;
pub use model::{
    ClientApiCall, FullstackTraceResult, FullstackTraceStep, FullstackTracer, ServerRouteEndpoint,
};
pub use route_matcher::RouteMatcher;
pub use tracer::{FullstackExecutionTracer, DEFAULT_FULLSTACK_BUDGET};
