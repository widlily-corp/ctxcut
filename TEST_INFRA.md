# E2E Test Infra: ctxcut v3.0 Next-Gen Suite

## Test Philosophy
- Opaque-box, requirement-driven testing.
- Derives all test cases from `ORIGINAL_REQUEST.md` (R1–R6) independently of internal implementation design.
- Methodology: Category-Partition + Boundary Value Analysis + Pairwise Combinatorial + Real-World Workload Testing.

## Feature Inventory
| # | Feature | Source (Requirement) | Tier 1 (Unit/Feature) | Tier 2 (Boundaries) | Tier 3 (Polyglot) | Tier 4 (Bench/Workload) | Tier 5 (MCP/E2E) |
|---|---------|----------------------|:---------------------:|:-------------------:|:-----------------:|:-----------------------:|:----------------:|
| 1 | Full-Stack Trace (Client to DB DDL) | ORIGINAL_REQUEST §R1 | ≥5 | ≥5 | ✓ | ✓ | ✓ |
| 2 | Semantic Intent AST Slicing (BM25 + AST) | ORIGINAL_REQUEST §R2 | ≥5 | ≥5 | ✓ | ✓ | ✓ |
| 3 | Transactional Multi-Symbol Refactor | ORIGINAL_REQUEST §R3 | ≥5 | ≥5 | ✓ | ✓ | ✓ |
| 4 | Swarm Context Partitioning & Stubs | ORIGINAL_REQUEST §R4 | ≥5 | ≥5 | ✓ | ✓ | ✓ |
| 5 | Dogfooding, CLI & MCP Tooling | ORIGINAL_REQUEST §R5 | ≥5 | ≥5 | ✓ | ✓ | ✓ |
| 6 | Zero-Clippy & Git Release Pipeline | ORIGINAL_REQUEST §R6 | ✓ | ✓ | ✓ | ✓ | ✓ |

## Test Architecture
- **Test Runner**: Standard Rust cargo test harness (`cargo test --workspace --all-targets`).
- **Pass/Fail Semantics**: 100% of tests must pass with exit code 0.
- **Directory Layout**:
  - `tests/tier1_features/test_f16_fullstack_trace.rs`: R1 client calls ↔ server routes ↔ SQL DDL tests.
  - `tests/tier1_features/test_f17_intent_slice.rs`: R2 BM25 + AST intent slicing & >85% token reduction tests.
  - `tests/tier1_features/test_f18_batch_refactor.rs`: R3 multi-file patch transaction, shadow dry-run, rollback & AST diagnostic mapping tests.
  - `tests/tier1_features/test_f19_swarm_partition.rs`: R4 swarm graph clustering, non-overlapping AST slices & boundary contract stubs tests.
  - `tests/tier2_boundaries/test_f16_f19_boundaries.rs`: Tier 2 boundary, syntax corruption, timeout, and adversarial edge cases.
  - `tests/tier3_polyglot/`: Tier 3 cross-language projects (TS + Rust + Go + Python + SQL).
  - `tests/tier4_benchmarks/`: Tier 4 latency (<5ms SQLite) and token budget compression tests.
  - `tests/tier5_protocol/`: Tier 5 JSON-RPC 2.0 stdio MCP server tools and CLI end-to-end execution tests.

## Coverage Thresholds
- **Tier 1**: ≥5 unit/feature test cases per feature.
- **Tier 2**: ≥5 boundary/edge test cases per feature (syntax errors, compiler failures, empty prompts, cyclic graph cuts).
- **Tier 3**: Polyglot multi-language integration tests.
- **Tier 4**: Latency & token budget compliance benchmarks.
- **Tier 5**: Full STDIO MCP tool protocol and CLI binary execution tests.
