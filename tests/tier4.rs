//! Tier 4 E2E Integration Test Suite Driver
//!
//! Aggregates and executes all Tier 4 real-world workload simulation test suites:
//! - test_workload_go_auth: Go microservice JWT authentication slice
//! - test_workload_py_billing: Python FastAPI/SQLAlchemy billing pipeline slice
//! - test_workload_rs_inventory: Rust Axum/Tokio concurrent inventory slice
//! - test_workload_ts_ecommerce: TypeScript Next.js/Prisma order refund slice
//! - test_workload_v2_monorepo_refactor: TypeScript Next.js/Prisma monorepo refactoring
//! - test_workload_v2_fullstack_checkout: Vue 3 / Pinia / Drizzle checkout workflow
//! - test_workload_v2_microservice_trace: Rust Axum / SQLx trace & impact workflow

#![allow(dead_code, unused_imports, clippy::all)]

#[path = "tier4_real_world/test_workload_go_auth.rs"]
mod test_workload_go_auth;

#[path = "tier4_real_world/test_workload_py_billing.rs"]
mod test_workload_py_billing;

#[path = "tier4_real_world/test_workload_rs_inventory.rs"]
mod test_workload_rs_inventory;

#[path = "tier4_real_world/test_workload_ts_ecommerce.rs"]
mod test_workload_ts_ecommerce;

#[path = "tier4_real_world/test_workload_v2_monorepo_refactor.rs"]
mod test_workload_v2_monorepo_refactor;

#[path = "tier4_real_world/test_workload_v2_fullstack_checkout.rs"]
mod test_workload_v2_fullstack_checkout;

#[path = "tier4_real_world/test_workload_v2_microservice_trace.rs"]
mod test_workload_v2_microservice_trace;
