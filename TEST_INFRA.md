# E2E Test Infra: ctxcut 6-Pillar Upgrade

## Test Philosophy
- Opaque-box, requirement-driven. Derived from `ORIGINAL_REQUEST.md`.
- Methodology: Category-Partition + Boundary Value Analysis (BVA) + Pairwise Combinatorial + Real-World Workload Testing.

## Feature Inventory
| # | Feature | Source (Requirement) | Tier 1 | Tier 2 | Tier 3 |
|---|---------|----------------------|:------:|:------:|:------:|
| 1 | .gitignore & .ctxcutignore | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ |
| 2 | Binary/Lockfile Filtering | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ |
| 3 | Fast Token Estimation Scan | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ |
| 4 | MCP Timeout Safety Guard | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ |
| 5 | Cross-File Module Resolution | ORIGINAL_REQUEST § R2 | 5 | 5 | ✓ |
| 6 | Transitive Type Hoisting | ORIGINAL_REQUEST § R2 | 5 | 5 | ✓ |
| 7 | Body Stripping & Signature Stubs | ORIGINAL_REQUEST § R2 | 5 | 5 | ✓ |
| 8 | Multi-Language Adapter Parity | ORIGINAL_REQUEST § R2 | 5 | 5 | ✓ |
| 9 | Django Semantic Extractor | ORIGINAL_REQUEST § R3 | 5 | 5 | ✓ |
| 10 | FastAPI Semantic Extractor | ORIGINAL_REQUEST § R3 | 5 | 5 | ✓ |
| 11 | React / Next.js Extractor | ORIGINAL_REQUEST § R3 | 5 | 5 | ✓ |
| 12 | Express / NestJS / Spring Extractor | ORIGINAL_REQUEST § R3 | 5 | 5 | ✓ |
| 13 | Exact BPE Token Counting | ORIGINAL_REQUEST § R4 | 5 | 5 | ✓ |
| 14 | Progressive Token Compression | ORIGINAL_REQUEST § R4 | 5 | 5 | ✓ |
| 15 | AST Node Range Locator | ORIGINAL_REQUEST § R5 | 5 | 5 | ✓ |
| 16 | Whitespace & Indent Normalization | ORIGINAL_REQUEST § R5 | 5 | 5 | ✓ |
| 17 | Pre-Write Syntax Validator | ORIGINAL_REQUEST § R5 | 5 | 5 | ✓ |
| 18 | Atomic Disk Persistence | ORIGINAL_REQUEST § R5 | 5 | 5 | ✓ |
| 19 | Test Context Bundle Assembler | ORIGINAL_REQUEST § R6 | 5 | 5 | ✓ |
| 20 | Multi-Runner Mock Scaffolding | ORIGINAL_REQUEST § R6 | 5 | 5 | ✓ |
| 21 | Workspace Fixture Discovery | ORIGINAL_REQUEST § R6 | 5 | 5 | ✓ |
| 22 | CLI & MCP Interface Parity | ORIGINAL_REQUEST § Acceptance | 5 | 5 | ✓ |

## Test Architecture
- Test runner: `cargo test --all`, `cargo test --test tier1`, `cargo test --test tier2`, `cargo test --test tier3`, `cargo test --test tier4`, `cargo test --test tier5`
- Test fixtures: `tests/fixtures/` containing Python (Django, FastAPI), TypeScript/React, Express/NestJS, Rust, and Go sample codebases.

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | Full Django REST Framework API Endpoint Slicing | F1, F5, F6, F7, F9, F13 | High |
| 2 | FastAPI Dependency-Injected Endpoint with Pydantic Slicing | F1, F5, F6, F7, F10, F13 | High |
| 3 | React Complex Dashboard Component with Hooks & Sub-trees | F1, F5, F6, F11, F13, F14 | High |
| 4 | Token-Constrained Multi-File Slice under Tight Budget (500 tokens) | F5, F6, F7, F13, F14 | High |
| 5 | Surgical AST Patching on Indented Multi-Method Class with Syntax Guard | F15, F16, F17, F18 | High |
| 6 | Unit Test Context Generation for Service with External DB/HTTP Calls | F5, F6, F7, F19, F20, F21 | High |
| 7 | Full MCP Server Tool Execution Pipeline over Stdio | F4, F13, F22 | High |

## Coverage Thresholds
- Tier 1: ≥5 test cases per feature (Happy-path / isolated feature coverage)
- Tier 2: ≥5 test cases per feature (Boundary, corner cases, empty, max limits, syntax errors)
- Tier 3: Pairwise combinations of major feature interactions
- Tier 4: ≥7 realistic application scenarios
- Tier 5: Adversarial edge cases and stress testing
