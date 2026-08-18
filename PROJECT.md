# Project: ctxcut 6-Pillar Architectural & Functional Upgrade

## Architecture
`ctxcut` is a high-performance Rust engine providing AST-guided code slicing, multi-file dependency resolution, framework intelligence, adaptive token budgeting, AST patching, and mock context generation for LLMs and MCP clients.

```
ctxcut/
├── crates/
│   ├── ctxcut_core/           # Core library & AST analysis engine
│   │   ├── src/
│   │   │   ├── lang/          # Language adapters (TS/JS, Python, Go, Rust)
│   │   │   ├── parser/        # Tree-sitter parser manager & AST utilities
│   │   │   ├── resolver/      # Import resolver, type hoister, signature stripper
│   │   │   ├── framework/     # Django, FastAPI, React/Next.js, Express, NestJS, Spring
│   │   │   ├── slice/         # Slicing pipeline & adaptive token budgeting
│   │   │   ├── patch/         # AST patcher & syntax validation guard
│   │   │   ├── test_context/  # Mock context generator & fixture finder
│   │   │   ├── traversal/     # Ignore engine, binary detector, fast stats
│   │   │   └── tokenizer/     # BPE token counter (tiktoken)
│   ├── ctxcut_cli/            # CLI binary interface & subcommands
│   └── ctxcut_mcp/            # JSON-RPC 2.0 stdio server with timeout isolation
└── tests/
    ├── fixtures/              # Representative multi-language test fixtures
    └── *.rs                   # Tiered integration test suites
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | .gitignore & .ctxcutignore Support | Traversal honors ignore files and built-in vendor blacklists | M1 | R1 |
| 2 | Binary & Artifact Ignore | Automatically detects and skips binary/lock/cache files | M1 | R1 |
| 3 | Fast Token Estimation Scan | `--fast` shallow scan for millisecond repo-wide token estimation | M1 | R1 |
| 4 | MCP Timeout Guard | Thread-isolated timeout guard preventing hangs on large codebases | M1 | R1 |
| 5 | Cross-File Module Resolution | Resolves relative and package imports across TS, Python, Rust, Go | M2 | R2 |
| 6 | Transitive Type Hoisting | Recursively hoists referenced types without circular loops | M2 | R2 |
| 7 | Body Stripping & Signature Stubs | Strips 100% of foreign function bodies to prevent token leakage | M2 | R2 |
| 8 | Multi-Language Adapter Parity | Consistent slice representations across TS, Python, Go, Rust | M2 | R2 |
| 9 | Django Semantic Extractor | Captures serializers, models, permissions; strips method bodies | M3 | R3 |
| 10 | FastAPI Semantic Extractor | Captures Pydantic schemas, dependency injection providers | M3 | R3 |
| 11 | React / Next.js Extractor | Extracts Props interfaces, custom hooks; collapses secondary JSX | M3 | R3 |
| 12 | Express / NestJS / Spring Extractor | Extracts route DTOs, parameter decorators, middleware chains | M3 | R3 |
| 13 | Exact BPE Token Counting | Accurate token metrics using tiktoken `cl100k_base` | M4 | R4 |
| 14 | Progressive Token Compression | 5-level deterministic degradation pipeline under `--budget <N>` | M4 | R4 |
| 15 | AST Node Range Locator | Pinpoints target AST node boundaries for surgical replacement | M5 | R5 |
| 16 | Whitespace & Indent Normalization | Preserves surrounding indentation and formatting during patching | M5 | R5 |
| 17 | Pre-Write Syntax Validator | Tree-sitter AST validation preventing corrupted disk writes | M5 | R5 |
| 18 | Atomic Disk Persistence | Safe temporary file and atomic rename file modifications | M5 | R5 |
| 19 | Test Context Bundle Assembler | Bundles target symbol, param/return types, mock signatures | M6 | R6 |
| 20 | Multi-Runner Spy/Mock Scaffolding | Synthesizes mock declarations for Vitest, Jest, Pytest, Cargo, Go | M6 | R6 |
| 21 | Workspace Fixture Discovery | Heuristically finds and extracts reference test patterns | M6 | R6 |
| 22 | CLI & MCP Interface Parity | Exposes all 6 capabilities via CLI subcommands and MCP tools | M1-M6 | Criteria |
| 23 | E2E Tier 1-4 Test Verification | 100% test pass on all unit and integration test tiers | M7 | Verification |
| 24 | Tier 5 Adversarial Coverage Hardening | Adversarial stress testing, edge cases, and robustness verification | M7 | Verification |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Smart Traversal and Timeout Guard (R1) | Ignore rules, binary detection, fast stats, MCP timeout safety | none | DONE |
| 2 | Multi-File Dependency Slicing (R2) | Cross-file imports, type hoisting, signature stripping, test fixes | M1 | IN_PROGRESS |
| 3 | Framework-Aware Intelligence (R3) | Django, FastAPI, React/Next.js, Express/NestJS/Spring extractors | M2 | PLANNED |
| 4 | Adaptive Token Budgeting (R4) | Budget constraints, 5-level progressive semantic compression | M2 | PLANNED |
| 5 | Bidirectional AST Patcher (R5) | Surgical AST patching, indentation aligner, syntax validator | M1 | PLANNED |
| 6 | Isolated Test Context Generator (R6) | Mock scaffolding, AAA test templates, fixture discovery | M2, M3 | PLANNED |
| 7 | Final E2E Pass & Adversarial Hardening (M7) | 100% E2E test pass (Tiers 1-4) + Tier 5 Adversarial Hardening | M1-M6 | PLANNED |

## Interface Contracts
### `ctxcut_core` ↔ `ctxcut_cli`
- `ContextSlicer::slice_symbol(path, symbol, opts)` -> `Result<SliceResult>`
- `AstPatcher::patch(path, symbol, replacement, opts)` -> `Result<PatchResult>`
- `TestContextGenerator::generate(path, symbol, opts)` -> `Result<TestContextResult>`
- `ProjectWalker::walk(root, config)` -> `Result<TraversalReport>`
- `fast_stats::estimate_fast_stats(root, config)` -> `Result<FastStatsReport>`

### `ctxcut_core` ↔ `ctxcut_mcp`
- Tool `get_symbol_slice`: arguments `path`, `symbol`, `depth`, `budget`, `timeout_ms`
- Tool `get_diff_slice`: arguments `repo_path`, `base_ref`, `depth`, `budget`, `timeout_ms`
- Tool `analyze_token_stats`: arguments `path`, `fast`, `timeout_ms`
- Tool `patch_symbol`: arguments `path`, `symbol`, `replacement`, `dry_run`, `timeout_ms`
- Tool `get_test_context`: arguments `path`, `symbol`, `framework`, `budget`, `timeout_ms`
- Tool `get_route_slice`: arguments `path`, `symbol`, `framework`, `budget`, `timeout_ms`

## Code Layout
- `crates/ctxcut_core/src/`: Core engine library
- `crates/ctxcut_cli/src/`: CLI frontend binary
- `crates/ctxcut_mcp/src/`: JSON-RPC 2.0 MCP server
- `tests/fixtures/`: Multi-language test fixtures
- `tests/`: Integration test suites
