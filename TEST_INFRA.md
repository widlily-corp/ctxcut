# E2E Test Infrastructure & Quality Architecture: ctxcut v2.0

## 1. Test Philosophy & Core Invariants

`ctxcut` is an AST-level contextual code slicer and refactoring tool designed for autonomous AI agents and developers. High fidelity, deterministic output, and strict token reduction guarantees are required.

### Key Testing Principles:
1. **Opaque-Box & Requirement-Driven**: All test cases are derived strictly from specifications in `ORIGINAL_REQUEST.md` and `PROJECT.md`. Tests interact only with public CLI subcommands, MCP STDIO protocol tools, and library APIs.
2. **Deterministic & Non-Facade**: Zero trivial mock assertions or hardcoded dummy returns. Every test exercises real tree-sitter AST parsing, BPE token counting (`tiktoken-rs`), Git repository sandboxes, or JSON-RPC message passing.
3. **Arrange-Act-Assert (AAA) Discipline**: Each test case is completely self-contained, sets up its own isolated temporary environment (`tempfile::TempDir`, `GitSandbox`), executes exactly one primary action, and asserts observable invariant outcomes.
4. **Progressive Testability & Graceful Degradation**: Tests are structured to execute cleanly across development milestones (M1 through M5) with robust fallback assertions.

---

## 2. Test Architecture & Harnesses

The test infrastructure resides under `tests/` and is driven by Rust's standard integration test runner (`cargo test`):

```
tests/
├── common/                     # Shared Test Support Fixtures
│   ├── mod.rs                  # Authoritative re-exports
│   ├── runner.rs               # CliRunner (CLI execution) & McpClient (STDIO JSON-RPC)
│   ├── git_sandbox.rs          # Isolated Git repository fixture
│   ├── token_verifier.rs       # tiktoken-rs BPE (cl100k_base) verifier
│   ├── clipboard.rs            # Thread-safe in-memory clipboard mock
│   └── snapshot.rs             # Cross-platform CRLF/LF & path normalizer
│
├── tier1.rs                    # Tier 1 Test Runner Driver
├── tier1_features/             # Tier 1 Feature Coverage (F1..F15)
│   ├── test_f1_callers.rs      # F1: Upstream caller impact slicing
│   ├── test_f2_trace.rs        # F2: End-to-end execution flow tracing
│   ├── test_f3_implementors.rs # F3: Interface & Trait implementor hoisting
│   ├── test_f4_c_cpp.rs        # F4: C / C++ classes, templates, macros
│   ├── test_f5_csharp.rs       # F5: C# / .NET controllers, records, DTOs
│   ├── test_f6_java_kotlin.rs  # F6: Java / Kotlin Spring Boot, JPA entities
│   ├── test_f7_sfc.rs          # F7: Vue, Svelte, Astro SFCs
│   ├── test_f8_orm_schema.rs   # F8: Prisma, Drizzle, SQL DDL, Proto, GraphQL
│   ├── test_f9_verify_patch.rs # F9: Verification guard & typecheck dry-run
│   ├── test_f10_semantic_diff.rs # F10: Semantic AST diff & token savings
│   ├── test_f11_refactor_rename.rs # F11: Multi-file AST symbol renaming
│   ├── test_f12_sqlite_index.rs # F12: Persistent SQLite WAL cache engine
│   ├── test_f13_ast_query.rs   # F13: Tree-sitter S-expression query engine
│   ├── test_f14_tui_dashboard.rs # F14: Ratatui TUI dashboard & telemetry
│   ├── test_f15_upgrade.rs     # F15: Release & self-upgrade scripts
│   ├── test_diff_features.rs   # Baseline diff features
│   ├── test_lang_parity.rs     # Multi-lang base parity
│   ├── test_mcp_features.rs    # Base MCP features
│   ├── test_route_features.rs  # Web route features
│   ├── test_slice_features.rs  # Context slice features
│   ├── test_stats_features.rs  # Token stats features
│   └── test_m2_multifile.rs    # Multi-file module resolution
│
├── tier2.rs                    # Tier 2 Test Runner Driver
├── tier2_boundaries/           # Tier 2 Boundary & Corner Cases (F1..F15)
│   ├── test_f1_f3_boundaries.rs   # F1..F3 Graph & Implementor edge cases
│   ├── test_f4_f7_boundaries.rs   # F4..F7 Polyglot & SFC edge cases
│   ├── test_f8_f11_boundaries.rs  # F8..F11 Schema & Refactor edge cases
│   ├── test_f12_f15_boundaries.rs # F12..F15 Index & Tooling edge cases
│   ├── test_circular_types.rs     # Recursive cyclic types
│   ├── test_empty_files.rs        # Empty & whitespace files
│   ├── test_large_files.rs        # 10,000+ LOC stress tests
│   ├── test_missing_symbols.rs    # Missing / ambiguous symbols
│   ├── test_nested_generics.rs    # Deep generic bounds
│   ├── test_syntax_errors.rs      # Partial / broken AST error recovery
│   └── test_unicode_paths.rs      # Non-ASCII & emoji paths
│
├── tier3.rs                    # Tier 3 Test Runner Driver
├── tier3_cross_feature/        # Tier 3 Pairwise Cross-Feature Interactions
│   ├── test_v2_cross_combinations.rs # C1..C10 Pairwise combinations
│   ├── test_git_diff_route.rs        # Diff + Route
│   ├── test_ide_setup.rs             # Setup + MCP
│   ├── test_installers.rs            # Installer scripts
│   ├── test_mcp_chaining.rs          # MCP stateful sessions
│   └── test_multi_symbol_clip.rs     # Multi-symbol + Clipboard
│
├── tier4.rs                    # Tier 4 Test Runner Driver
├── tier4_real_world/           # Tier 4 Real-World Microservices & Workflows
│   ├── test_workload_v2_monorepo_refactor.rs # TS/Next.js/Prisma monorepo refactor
│   ├── test_workload_v2_fullstack_checkout.rs# Vue 3/Drizzle/Pinia checkout slice
│   ├── test_workload_v2_microservice_trace.rs# Rust/Axum/SQLx trace & impact
│   ├── test_workload_go_auth.rs              # Go JWT auth service
│   ├── test_workload_py_billing.rs           # Python FastAPI billing service
│   ├── test_workload_rs_inventory.rs         # Rust Axum inventory service
│   └── test_workload_ts_ecommerce.rs         # TypeScript order refund service
│
└── tier5.rs                    # Tier 5 Telemetry & Adversarial Stress Suite
```

---

## 3. Feature Inventory & 4-Tier Test Mapping

| # | Feature | Requirement Source | Tier 1 (Coverage) | Tier 2 (Boundaries) | Tier 3 (Cross-Feature) | Tier 4 (Workload) |
|---|---------|-------------------|:-----------------:|:-------------------:|:----------------------:|:-----------------:|
| **F1** | `ctxcut callers` & `get_impact_slice` | ORIGINAL_REQUEST §R1 | ≥5 tests | ≥5 tests | C1 (Callers+Trace) | Scenario 1 |
| **F2** | `ctxcut trace` & `get_trace_slice` | ORIGINAL_REQUEST §R1 | ≥5 tests | ≥5 tests | C1 (Callers+Trace) | Scenario 1 |
| **F3** | Interface & Trait Implementor Hoisting | ORIGINAL_REQUEST §R1 | ≥5 tests | ≥5 tests | C2 (Impl+SFC) | Scenario 3 |
| **F4** | C / C++ Grammar & Slicing Support | ORIGINAL_REQUEST §R2 | ≥5 tests | ≥5 tests | C5 (Polyglot Rename) | Scenario 3 |
| **F5** | C# / .NET Controllers, Records, DTOs | ORIGINAL_REQUEST §R2 | ≥5 tests | ≥5 tests | C5 (Polyglot Rename) | Scenario 3 |
| **F6** | Java / Kotlin Spring Boot & JPA | ORIGINAL_REQUEST §R2 | ≥5 tests | ≥5 tests | C5 (Polyglot Rename) | Scenario 3 |
| **F7** | Vue, Svelte, Astro SFC Adapters | ORIGINAL_REQUEST §R2 | ≥5 tests | ≥5 tests | C2, C9 (SFC+Patch) | Scenario 4 |
| **F8** | ORM & Schema Stitching (Prisma/Drizzle/SQL/Proto/GQL) | ORIGINAL_REQUEST §R3 | ≥5 tests | ≥5 tests | C3, C8 (ORM+Diff) | Scenario 2, 4 |
| **F9** | Verification Guard (`verify-patch`) & Dry-Run Rollback | ORIGINAL_REQUEST §R4 | ≥5 tests | ≥5 tests | C3, C7 (Verify+MCP) | Scenario 6 |
| **F10**| Semantic AST Diff (`semantic-diff`) & ROI Calculation | ORIGINAL_REQUEST §R4 | ≥5 tests | ≥5 tests | C4, C8 (Diff+Stats) | Scenario 2 |
| **F11**| AST Symbol Renaming (`refactor rename`) | ORIGINAL_REQUEST §R4 | ≥5 tests | ≥5 tests | C5 (Rename+Polyglot) | Scenario 2 |
| **F12**| Persistent SQLite WAL Cache (`.ctxcut/index.db`) | ORIGINAL_REQUEST §R5 | ≥5 tests | ≥5 tests | C6, C10 (Index+Query) | Scenario 5 |
| **F13**| AST Query Engine (`ctxcut query` & presets) | ORIGINAL_REQUEST §R5 | ≥5 tests | ≥5 tests | C6 (Index+Query) | Scenario 5 |
| **F14**| Interactive Ratatui TUI Dashboard & Telemetry | ORIGINAL_REQUEST §R5 | ≥5 tests | ≥5 tests | C4 (Diff+TUI) | Scenario 7 |
| **F15**| Release Automation & Self-Upgrade (`ctxcut upgrade`) | ORIGINAL_REQUEST §R6 | ≥5 tests | ≥5 tests | C10 (Upgrade+Index) | Scenario 7 |

---

## 4. Four-Tier Coverage Plan

### Tier 1: Feature Coverage (Isolated Functional Verification)
- **Goal**: Verify core happy-path behavior and contract adherence for each feature F1..F15 in isolation.
- **Requirement**: At least 5 distinct test cases per feature.
- **Total Tier 1 Target**: ≥ 75 new feature tests + 36 baseline tests = ≥ 111 tests.

### Tier 2: Boundary & Corner Cases (Fault Injection & Limits)
- **Goal**: Subject every feature to extreme edge conditions: cyclic dependencies, empty files, deep recursion, syntax faults, missing files, name shadowing, Unicode paths, and memory limits.
- **Requirement**: At least 5 distinct boundary test cases per feature category.
- **Total Tier 2 Target**: ≥ 75 boundary tests + 35 baseline tests = ≥ 110 tests.

### Tier 3: Cross-Feature Combinations (Pairwise Interaction Matrix)
- **Goal**: Verify multi-feature pipelines and end-to-end composite actions.
- **Combinations**:
  - `C1`: Callers impact slice fed into linear downstream execution trace (`F1 + F2`).
  - `C2`: Implementor hoisting within Vue/Svelte Single File Components (`F3 + F7`).
  - `C3`: ORM query patch with typecheck verification against stitched schemas (`F8 + F9`).
  - `C4`: Semantic AST diff telemetry feeding into TUI ROI metrics dashboard (`F10 + F14`).
  - `C5`: Cross-language symbol refactoring across polyglot microservices (`F4 + F5 + F6 + F11`).
  - `C6`: SQLite WAL index acceleration for Tree-sitter S-expression query search (`F12 + F13`).
  - `C7`: STDIO MCP client calling `patch_symbol` with verification guard auto-rollback (`F9 + MCP`).
  - `C8`: Git diff change detection with automatic SQL migration DDL stitching (`F8 + F10`).
  - `C9`: AST patching in `<script setup>` while preserving template/style markup (`F5 + F7`).
  - `C10`: Version upgrade check followed by SQLite index health verification (`F12 + F15`).

### Tier 4: Real-World Application Workloads
- **Goal**: Execute complex developer workflows on full multi-file microservices across diverse tech stacks, asserting ≥85% token reduction and zero data loss.
- **Scenarios**:
  - Scenario 1: Rust/Axum + SQLx microservice impact analysis and execution flow trace.
  - Scenario 2: TypeScript/Next.js/Prisma monorepo cross-package refactoring and semantic diff.
  - Scenario 3: Polyglot gRPC & REST pipeline (Go gateway, C# service, Python worker).
  - Scenario 4: Vue 3 + Pinia + Drizzle full-stack e-commerce checkout slicing.
  - Scenario 5: Large repository cold/warm indexing (<5ms SLA) and AST security invariant query scan.
  - Scenario 6: Surgical AST patch bugfix with verification guard auto-rollback on type mismatch.
  - Scenario 7: Full AI agent MCP STDIO tool session with lifetime telemetry and TUI ROI metrics.

---

## 5. Execution Commands & Verification

```bash
# Execute entire test suite across all crates and integration targets
cargo test --all-targets

# Execute individual tiers
cargo test --test tier1   # Tier 1 Feature Coverage
cargo test --test tier2   # Tier 2 Boundary & Corner Cases
cargo test --test tier3   # Tier 3 Cross-Feature Combinations
cargo test --test tier4   # Tier 4 Real-World Application Scenarios
cargo test --test tier5   # Tier 5 Telemetry & Adversarial Invariants

# Execute benchmarks
cargo bench --no-run
```
