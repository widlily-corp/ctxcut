//! Tier 3 E2E Integration Test Suite Driver
//!
//! Aggregates and executes all Tier 3 cross-feature integration test suites:
//! - test_git_diff_route: Git diff detection combined with route handler extraction
//! - test_mcp_chaining: Sequential MCP tool calls and stateful server sessions
//! - test_multi_symbol_clip: Multi-symbol clipping and clipboard integration

#![allow(dead_code, unused_imports, clippy::all)]

#[path = "tier3_cross_feature/test_git_diff_route.rs"]
mod test_git_diff_route;

#[path = "tier3_cross_feature/test_mcp_chaining.rs"]
mod test_mcp_chaining;

#[path = "tier3_cross_feature/test_multi_symbol_clip.rs"]
mod test_multi_symbol_clip;

#[path = "tier3_cross_feature/test_installers.rs"]
mod test_installers;

#[path = "tier3_cross_feature/test_ide_setup.rs"]
mod test_ide_setup;
