//! Tier 2 E2E Integration Test Suite Driver
//!
//! Aggregates and executes all Tier 2 boundary and corner case test suites:
//! - Baseline: circular types, empty files, large files, missing symbols, nested generics, syntax errors, unicode paths
//! - Features F1..F15 Boundary Modules:
//!   - test_f1_f3_boundaries (Graph, Callers, Tracing, Implementors)
//!   - test_f4_f7_boundaries (C/C++, C#, Java/Kotlin, SFCs)
//!   - test_f8_f11_boundaries (ORM Stitching, Verify Patch, Semantic Diff, Refactor Rename)
//!   - test_f12_f15_boundaries (SQLite Index, AST Query, TUI Dashboard, Upgrade)

#![allow(dead_code, unused_imports, clippy::all)]

#[path = "tier2_boundaries/test_circular_types.rs"]
mod test_circular_types;

#[path = "tier2_boundaries/test_empty_files.rs"]
mod test_empty_files;

#[path = "tier2_boundaries/test_large_files.rs"]
mod test_large_files;

#[path = "tier2_boundaries/test_missing_symbols.rs"]
mod test_missing_symbols;

#[path = "tier2_boundaries/test_nested_generics.rs"]
mod test_nested_generics;

#[path = "tier2_boundaries/test_syntax_errors.rs"]
mod test_syntax_errors;

#[path = "tier2_boundaries/test_unicode_paths.rs"]
mod test_unicode_paths;

#[path = "tier2_boundaries/test_f1_f3_boundaries.rs"]
mod test_f1_f3_boundaries;

#[path = "tier2_boundaries/test_f4_f7_boundaries.rs"]
mod test_f4_f7_boundaries;

#[path = "tier2_boundaries/test_f8_f11_boundaries.rs"]
mod test_f8_f11_boundaries;

#[path = "tier2_boundaries/test_f12_f15_boundaries.rs"]
mod test_f12_f15_boundaries;

#[path = "tier2_boundaries/test_m3_adversarial_proto_graphql.rs"]
mod test_m3_adversarial_proto_graphql;

#[path = "tier2_boundaries/test_m4_adversarial_verification_semantic_diff.rs"]
mod test_m4_adversarial_verification_semantic_diff;

#[path = "tier2_boundaries/test_m4_adversarial_rename.rs"]
mod test_m4_adversarial_rename;
