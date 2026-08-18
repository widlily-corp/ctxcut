# E2E Test Infra: ctxcut Upgrade

## Test Philosophy
- Opaque-box, requirement-driven verification derived strictly from ORIGINAL_REQUEST.md.
- Methodology: Category-Partition + Boundary Value Analysis + Pairwise Combinations + Real-World Workload Testing.
- Zero reliance on internal implementation details; tests verify observable CLI and MCP contracts across fixtures.

## Feature Inventory
| # | Feature | Source (Requirement) | Tier 1 (Features) | Tier 2 (Boundaries) | Tier 3 (Cross-Module) | Tier 4 (Workloads) |
|---|---|---|:---:|:---:|:---:|:---:|
| 1 | .gitignore & .ctxcutignore ignore rules | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ | ✓ |
| 2 | Fast Token Estimation scan mode (--fast) | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ | ✓ |
| 3 | MCP Execution Timeout Safety Guard | ORIGINAL_REQUEST §R1 | 5 | 5 | ✓ | ✓ |
| 4 | Multi-file import resolution & signatures | ORIGINAL_REQUEST §R2 | 5 | 5 | ✓ | ✓ |
| 5 | Cross-file type hoisting (--depth 1) | ORIGINAL_REQUEST §R2 | 5 | 5 | ✓ | ✓ |
| 6 | Django / DRF serializers & models extractor | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 7 | FastAPI & Pydantic schemas extractor | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 8 | React Props & custom hooks extractor | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 9 | JSX secondary branch collapser | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 10 | Express/NestJS/Spring DTOs extractor | ORIGINAL_REQUEST §R3 | 5 | 5 | ✓ | ✓ |
| 11 | Adaptive token budgeting (--budget <N>) | ORIGINAL_REQUEST §R4 | 5 | 5 | ✓ | ✓ |
| 12 | Progressive 5-level semantic degradation | ORIGINAL_REQUEST §R4 | 5 | 5 | ✓ | ✓ |
| 13 | AST node locator & byte range precision | ORIGINAL_REQUEST §R5 | 5 | 5 | ✓ | ✓ |
| 14 | Surgical code replacement & indentation | ORIGINAL_REQUEST §R5 | 5 | 5 | ✓ | ✓ |
| 15 | AST syntax validation guard on patch | ORIGINAL_REQUEST §R5 | 5 | 5 | ✓ | ✓ |
| 16 | CLI & MCP AST patch command/tool | ORIGINAL_REQUEST §R5 | 5 | 5 | ✓ | ✓ |
| 17 | Isolated test context bundle generator | ORIGINAL_REQUEST §R6 | 5 | 5 | ✓ | ✓ |
| 18 | Mock/spy stubs generation across frameworks | ORIGINAL_REQUEST §R6 | 5 | 5 | ✓ | ✓ |
| 19 | Project test fixture reference discovery | ORIGINAL_REQUEST §R6 | 5 | 5 | ✓ | ✓ |
| 20 | CLI & MCP test-context command/tool | ORIGINAL_REQUEST §R6 | 5 | 5 | ✓ | ✓ |
| 21 | Complete 6-Pillar MCP Tools Suite | ORIGINAL_REQUEST §AC | 5 | 5 | ✓ | ✓ |
| 22 | Strict Zero-Clippy Warning Compliance | ORIGINAL_REQUEST §AC | 5 | 5 | ✓ | ✓ |

## Test Architecture
- **Test Runner**: Standard `cargo test --all` and individual tier targets (`cargo test --test tier1`, `tier2`, `tier3`, `tier4`, `tier5`).
- **Harnesses**:
  - `tests/common/runner.rs`: `CliRunner` for CLI subcommands, `McpClient` for JSON-RPC STDIO communications.
  - `tests/common/git_sandbox.rs`: `GitSandbox` for temporary git repositories and ignore-rule testing.
  - `tests/common/token_verifier.rs`: `TokenVerifier` for BPE token limit validation.
- **Fixture Project Layout**:
  ```
  tests/fixtures/
  ├── traversal/
  │   ├── with_gitignore/
  │   ├── with_ctxcutignore/
  │   ├── vendor_dirs/ (node_modules, target, .venv)
  │   └── binary_files/
  ├── multi_file/
  │   ├── typescript_service/
  │   ├── python_package/
  │   └── rust_crate/
  ├── frameworks/
  │   ├── django_app/ (models.py, serializers.py, views.py)
  │   ├── fastapi_app/ (main.py, schemas.py, routers.py, deps.py)
  │   ├── react_next_app/ (UserProfile.tsx, useAuth.ts, layout.tsx)
  │   └── ts_backend_app/ (user.controller.ts, create-user.dto.ts)
  ├── patching/
  │   ├── ts_functions/
  │   ├── py_classes/
  │   └── invalid_syntax_cases/
  └── test_context/
      ├── jest_project/
      └── pytest_project/
  ```

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Framework / Stack | Complexity |
|---|---|---|---|---|
| 1 | E-Commerce Checkout API Endpoint | FastAPI, Pydantic v2 schemas, multi-file service dependencies, token budget 250 | Python / FastAPI | High |
| 2 | DRF ModelViewSet Order Management | Django models, ModelSerializer, permission classes, multi-file imports | Python / Django REST | High |
| 3 | Next.js 14 Dashboard UI Component | Next.js Server/Client component, TypeScript Props, custom hooks, JSX branch collapsing | TypeScript / React / Next.js | High |
| 4 | NestJS Payment Gateway Controller | NestJS controller decorators, DTO validation, guard middleware, mock test-context | TypeScript / NestJS | High |
| 5 | Monorepo Repo-Wide Scan & AST Patch | Smart traversal across 500+ files, fast token stats, surgical AST patching of core function | Multi-language Workspace | High |

## Coverage Thresholds
- Tier 1 (Feature Coverage): $\ge 5$ test cases per feature (Total $\ge 110$ tests)
- Tier 2 (Boundary & Corner Cases): $\ge 5$ test cases per feature category ($\ge 50$ tests)
- Tier 3 (Cross-Feature Combinations): Pairwise combinations of all 6 pillars ($\ge 25$ tests)
- Tier 4 (Real-World Application Scenarios): 5 comprehensive multi-framework end-to-end integration scenarios
- Tier 5 (Adversarial Coverage Hardening): Mutation, edge case, and syntax corruption stress tests
