# E2E Test Infra: ctxcut

## Test Philosophy
- Opaque-box, requirement-driven. No dependency on internal module internals.
- Methodology: Category-Partition + Boundary Value Analysis + Pairwise + Real-World Workload Testing.
- Target: 100% test pass rate, 0 warnings on `cargo clippy --all-targets -- -D warnings`, verified >80-90% token reduction across TypeScript, Python, Go, and Rust.

## Feature Inventory
| # | Feature | Source (requirement) | Tier 1 | Tier 2 | Tier 3 | Tier 4 |
|---|---------|---------------------|:------:|:------:|:------:|:------:|
| 1 | Target Symbol AST Extraction | ORIGINAL_REQUEST §R1, R2 | 5 | 5 | ✓ | ✓ |
| 2 | Type Hoisting & Inlining | ORIGINAL_REQUEST §R2 | 5 | 5 | ✓ | ✓ |
| 3 | Signature-Only Body Stripping | ORIGINAL_REQUEST §R2 | 5 | 5 | ✓ | ✓ |
| 4 | Multi-Language Support (TS, Py, Go, Rust) | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ | ✓ |
| 5 | CLI Slicing (`slice`) & Output/Clip | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 6 | Git Diff Contextualizer (`diff`) | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 7 | Repository Token Stats (`stats`) | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 8 | Web Framework Route Resolver (`route`) | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 9 | Model Context Protocol (MCP) STDIO | ORIGINAL_REQUEST §R4 | 5 | 5 | ✓ | ✓ |

## Test Architecture
- Test runner: `cargo test --workspace --all-targets`
- Golden Snapshot harness: `insta` (`tests/snapshots/`) with normalized `\n` line endings and Unix `/` paths.
- Token Verifier: `tiktoken-rs` (cl100k_base) measuring exact baseline vs sliced tokens.
- Directory layout:
  - `tests/common/`: Test helpers, git sandbox fixture generator, token verifier, clipboard mock.
  - `tests/fixtures/`: Realistic multi-file source fixtures for TypeScript, Python, Go, and Rust.
  - `tests/tier1_features/`: Feature coverage tests (>=5 tests per feature).
  - `tests/tier2_boundaries/`: Adversarial & boundary tests (empty files, syntax error recovery, deep generics, circular types, fuzzy symbol matching, large 10k LOC files).
  - `tests/tier3_cross_feature/`: Cross-feature combinations (multi-symbol + clipboard, git diff + route handler, full MCP sessions).
  - `tests/tier4_real_world/`: Real-world microservice workloads proving >80-90% token reduction.

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Language / Framework | Features Exercised | Target Reduction |
|---|----------|----------------------|--------------------|------------------|
| 1 | Order Refund & Stripe Webhook | TypeScript / Next.js + Prisma | F1, F2, F3, F4, F5 | 89.2% (2450 -> 265 tokens) |
| 2 | Payment Processing & Billing | Python / FastAPI + SQLAlchemy | F1, F2, F3, F4, F8 | 89.4% (1980 -> 210 tokens) |
| 3 | User Session & JWT Authentication | Go / Gin + GORM | F1, F2, F3, F4, F8 | 88.8% (2150 -> 240 tokens) |
| 4 | Inventory Reservation & ERP Sync | Rust / Axum + SQLx | F1, F2, F3, F4, F8 | 89.0% (2820 -> 310 tokens) |

## Coverage Thresholds
- Tier 1: ≥5 per feature (Total ≥ 45 feature tests)
- Tier 2: ≥5 boundary / fault-injection cases per language & subsystem (Total ≥ 12 boundary tests)
- Tier 3: Pairwise combinations (multi-symbol + clip, diff + route, mcp session)
- Tier 4: ≥4 realistic microservice workloads
- Total minimum: ≥ 65 comprehensive test cases across workspace
