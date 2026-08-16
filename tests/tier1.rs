//! Tier 1 E2E Integration Test Suite Driver
//!
//! Aggregates and executes all Tier 1 feature coverage test suites:
//! - test_diff_features: Git diff contextualizer and staged slicing
//! - test_lang_parity: Multi-language parity across TS, Python, Go, Rust
//! - test_mcp_features: Model Context Protocol STDIO tools and resources
//! - test_route_features: Web framework route handler detection and extraction
//! - test_slice_features: AST dependency extraction and type hoisting
//! - test_stats_features: Token metrics, reduction statistics, and telemetry

#[path = "tier1_features/test_diff_features.rs"]
mod test_diff_features;

#[path = "tier1_features/test_lang_parity.rs"]
mod test_lang_parity;

#[path = "tier1_features/test_mcp_features.rs"]
mod test_mcp_features;

#[path = "tier1_features/test_route_features.rs"]
mod test_route_features;

#[path = "tier1_features/test_slice_features.rs"]
mod test_slice_features;

#[path = "tier1_features/test_stats_features.rs"]
mod test_stats_features;
