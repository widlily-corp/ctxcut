//! Tier 1 E2E Integration Test Suite Driver
//!
//! Aggregates and executes all Tier 1 feature coverage test suites:
//! - Baseline: diff, lang parity, mcp, route, slice, stats, multifile
//! - Features F1..F15:
//!   - F1: test_f1_callers
//!   - F2: test_f2_trace
//!   - F3: test_f3_implementors
//!   - F4: test_f4_c_cpp
//!   - F5: test_f5_csharp
//!   - F6: test_f6_java_kotlin
//!   - F7: test_f7_sfc
//!   - F8: test_f8_orm_schema
//!   - F9: test_f9_verify_patch
//!   - F10: test_f10_semantic_diff
//!   - F11: test_f11_refactor_rename
//!   - F12: test_f12_sqlite_index
//!   - F13: test_f13_ast_query
//!   - F14: test_f14_tui_dashboard
//!   - F15: test_f15_upgrade

#![allow(unused_variables, dead_code, unused_imports, clippy::all)]

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

#[path = "tier1_features/test_m2_multifile.rs"]
mod test_m2_multifile;

#[path = "tier1_features/test_f1_callers.rs"]
mod test_f1_callers;

#[path = "tier1_features/test_f2_trace.rs"]
mod test_f2_trace;

#[path = "tier1_features/test_f3_implementors.rs"]
mod test_f3_implementors;

#[path = "tier1_features/test_f4_c_cpp.rs"]
mod test_f4_c_cpp;

#[path = "tier1_features/test_f5_csharp.rs"]
mod test_f5_csharp;

#[path = "tier1_features/test_f6_java_kotlin.rs"]
mod test_f6_java_kotlin;

#[path = "tier1_features/test_f7_sfc.rs"]
mod test_f7_sfc;

#[path = "tier1_features/test_f8_orm_schema.rs"]
mod test_f8_orm_schema;

#[path = "tier1_features/test_f9_verify_patch.rs"]
mod test_f9_verify_patch;

#[path = "tier1_features/test_f10_semantic_diff.rs"]
mod test_f10_semantic_diff;

#[path = "tier1_features/test_f11_refactor_rename.rs"]
mod test_f11_refactor_rename;

#[path = "tier1_features/test_f12_sqlite_index.rs"]
mod test_f12_sqlite_index;

#[path = "tier1_features/test_f13_ast_query.rs"]
mod test_f13_ast_query;

#[path = "tier1_features/test_f14_tui_dashboard.rs"]
mod test_f14_tui_dashboard;

#[path = "tier1_features/test_f15_upgrade.rs"]
mod test_f15_upgrade;
