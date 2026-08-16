//! Tier 4 E2E Integration Test Suite Driver
//!
//! Aggregates and executes all Tier 4 real-world workload simulation test suites:
//! - test_workload_go_auth: Go microservice JWT authentication slice
//! - test_workload_py_billing: Python FastAPI/SQLAlchemy billing pipeline slice
//! - test_workload_rs_inventory: Rust Axum/Tokio concurrent inventory slice
//! - test_workload_ts_ecommerce: TypeScript Next.js/Prisma order refund slice

#![allow(dead_code, unused_imports, clippy::all)]

#[path = "tier4_real_world/test_workload_go_auth.rs"]
mod test_workload_go_auth;

#[path = "tier4_real_world/test_workload_py_billing.rs"]
mod test_workload_py_billing;

#[path = "tier4_real_world/test_workload_rs_inventory.rs"]
mod test_workload_rs_inventory;

#[path = "tier4_real_world/test_workload_ts_ecommerce.rs"]
mod test_workload_ts_ecommerce;
