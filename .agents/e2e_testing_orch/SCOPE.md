# Scope: E2E Testing Track for ctxcut

## Architecture
The E2E Testing Track builds an opaque-box, requirement-driven test suite with comprehensive multi-language fixtures, test support harnesses, 4-tier integration test cases, criterion benchmarks, and publishes `TEST_READY.md`.

## Feature Inventory Mapping
| # | Feature | Target Milestone | Test Coverage |
|---|---------|------------------|---------------|
| 1 | Target Symbol AST Extraction | TM3 | Tier 1, 2, 3, 4 |
| 2 | Type Hoisting & Inlining | TM3 | Tier 1, 2, 3, 4 |
| 3 | Signature-Only Body Stripping | TM3 | Tier 1, 2, 3, 4 |
| 4 | Multi-Language Support (TS, Py, Go, Rust) | TM1, TM3 | Tier 1, 2, 3, 4 |
| 5 | CLI Slicing (`slice`) & Output/Clip | TM3 | Tier 1, 3 |
| 6 | Git Diff Contextualizer (`diff`) | TM2, TM3 | Tier 1, 3 |
| 7 | Repository Token Stats (`stats`) | TM3 | Tier 1, 3 |
| 8 | Web Framework Route Resolver (`route`) | TM3 | Tier 1, 3 |
| 9 | Model Context Protocol (MCP) STDIO | TM3 | Tier 1, 3 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| TM1 | Multi-Language Test Fixtures | `tests/fixtures/{typescript,python,go,rust}` | none | DONE |
| TM2 | Common Test Harness & Benchmarks | `tests/common/` (runner, token_verifier, git_sandbox) and `benches/` | none | DONE |
| TM3 | 4-Tier E2E Test Suite | `tests/tier{1,2,3,4}_*` covering all 9 features across 4 languages | TM1, TM2 | DONE |
| TM4 | Test Suite Verification & TEST_READY.md | Review, adversarial challenge, forensic audit, publish `TEST_READY.md` | TM3 | IN_PROGRESS |

## Interface Contracts & Layout
- Test fixtures located in `tests/fixtures/<lang>/` (38 files)
- Common test utilities located in `tests/common/` (6 files)
- Tier 1: `tests/tier1_features/` (6 files, 36 tests)
- Tier 2: `tests/tier2_boundaries/` (7 files, 35 tests)
- Tier 3: `tests/tier3_cross_feature/` (3 files, 10 tests)
- Tier 4: `tests/tier4_real_world/` (4 files, 4 tests)
- Benchmarks: `benches/` (4 files)
- Total tests: 85 test cases
