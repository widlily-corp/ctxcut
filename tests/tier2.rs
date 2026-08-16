//! Tier 2 E2E Integration Test Suite Driver
//!
//! Aggregates and executes all Tier 2 boundary and corner case test suites:
//! - test_circular_types: Cyclic and mutually recursive type graphs
//! - test_empty_files: Empty files, whitespace-only, comment-only inputs
//! - test_large_files: 10,000+ LOC stress testing and memory limits
//! - test_missing_symbols: Missing, ambiguous, and hallucinated symbol queries
//! - test_nested_generics: Deeply nested generics, HKTs, and complex bounds
//! - test_syntax_errors: Malformed and partial AST error recovery
//! - test_unicode_paths: Non-ASCII, emoji, and deep directory hierarchies

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
