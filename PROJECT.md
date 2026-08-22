# Project: ctxcut 6-Pillar Architectural & Functional Engine

## Architecture
`ctxcut` is an ultra-high-performance Rust engine providing AST-guided contextual code slicing, multi-file type hoisting, signature stripping, framework semantic extraction, progressive token budgeting, bidirectional AST patching, test context generation, workspace symbol indexing, and Model Context Protocol (MCP) integration for AI coding agents.

```
ctxcut/
├── Cargo.toml                 # Cargo workspace configuration (edition 2021, rust-version 1.80)
├── src/
│   └── main.rs                # Root binary entry point routing CLI subcommands vs MCP STDIO
├── crates/
│   ├── ctxcut_core/           # Pure AST slicing engine, resolvers, language adapters & telemetry
│   │   ├── src/
│   │   │   ├── lang/          # Language adapters (TS/JS, Python, Go, Rust) & symbol locator
│   │   │   ├── parser/        # Tree-sitter parser manager, AST utilities & grammar bindings
│   │   │   ├── resolver/      # Cross-file ImportResolver, TypeHoister, SignatureStripper
│   │   │   ├── framework/     # Django, FastAPI, React/Next.js, Express, NestJS, Spring
│   │   │   ├── slice/         # ContextSlicer, multi-symbol batching, BudgetCompressor (5 levels)
│   │   │   ├── patch/         # AstPatcher, IndentationAligner, SyntaxValidator guard
│   │   │   ├── test_context/  # TestContextGenerator, FixtureFinder, MockScaffolder
│   │   │   ├── traversal/     # ProjectWalker, ignore engine (.gitignore/.ctxcutignore), fast stats
│   │   │   ├── overview/      # Workspace symbol indexer & body-free architectural outline
│   │   │   ├── tokenizer/     # BPE token counter (tiktoken-rs with cl100k_base)
│   │   │   ├── telemetry/     # Persistent JSONL metrics logger (~/.ctxcut/metrics.jsonl) & ROI
│   │   │   └── formatter/     # Markdown (single & unified batch) and JSON formatters
│   ├── ctxcut_cli/            # CLI binary interface & subcommands
│   │   ├── src/
│   │   │   ├── lib.rs         # Clap CLI definition (slice, diff, patch, test-context, stats, etc.)
│   │   │   ├── diff.rs        # Git diff slicing & modified symbol discovery
│   │   │   ├── metrics.rs     # High-density terminal ROI dashboard
│   │   │   ├── route.rs       # Web framework route handler resolver
│   │   │   ├── setup_mcp.rs   # Automated IDE MCP configuration (Antigravity, Cursor, Claude, VSCode)
│   │   │   └── stats.rs       # Fast repo/file token savings analyzer
│   └── ctxcut_mcp/            # JSON-RPC 2.0 STDIO server with thread-isolated timeout guard
│       ├── src/
│       │   ├── lib.rs         # Protocol loop, tools/list, tools/call dispatch, timeout boundaries
│       │   └── logger.rs      # Structured JSONL request/response logging & latency telemetry
└── tests/
    ├── fixtures/              # Polyglot test fixtures (TS/JS, Python, Go, Rust)
    ├── tier1.rs               # Traversal, ignore rules, binary detection, fast stats
    ├── tier2.rs               # Multi-file imports, type hoisting, signature stripping
    ├── tier3.rs               # Framework extractors, budgeting, multi-symbol batching
    ├── tier4.rs               # Real-world microservice workloads across 4 languages
    └── tier5.rs               # Telemetry, dashboard, IDE setup, adversarial stress testing
```

## Feature Inventory
| # | Category | Feature | Description | Milestone | Source |
|---|----------|---------|-------------|-----------|--------|
| 1 | Traversal | .gitignore & .ctxcutignore Support | Traversal honors ignore files and built-in vendor blacklists | M1 | R1 |
| 2 | Traversal | Binary & Artifact Ignore Filter | Automatically detects and skips binary/lock/cache files | M1 | R1 |
| 3 | Traversal | Fast Token Estimation Scan | `--fast` shallow scan for millisecond repo-wide token estimation | M1 | R1 |
| 4 | MCP | Thread-Isolated Timeout Guard | Thread-isolated timeout guard preventing hangs on large repos (default: 10s) | M1 | R1 |
| 5 | Slicing | Cross-File Module Resolution | Resolves relative and package imports across TS, Python, Rust, Go | M2 | R2 |
| 6 | Slicing | Transitive Type Hoisting | Recursively hoists referenced types without circular recursion loops | M2 | R2 |
| 7 | Slicing | Signature Stripping & Call Stubs | Strips 100% of foreign function bodies to prevent token leakage | M2 | R2 |
| 8 | Slicing | Multi-Language Adapter Parity | Consistent AST representations across TS/JS, Python, Go, Rust | M2 | R2 |
| 9 | Slicing | Multi-Symbol Batch Slicing | Slices multiple target symbols (`path:sym1,sym2`) with unified type deduplication | M8 | R2 |
| 10 | Framework | Django / DRF Semantic Extractor | Captures serializers, models, permissions, filter backends, pagination | M3 | R3 |
| 11 | Framework | FastAPI Semantic Extractor | Captures Pydantic schemas, `Depends(...)`, `Security(...)`, route params | M3 | R3 |
| 12 | Framework | React & Next.js Extractor | Extracts Props interfaces, custom hooks; collapses secondary JSX branches | M3 | R3 |
| 13 | Framework | Express / NestJS / Spring Extractor | Extracts route DTOs, parameter decorators, middleware chains, `@UseGuards` | M3 | R3 |
| 14 | Budgeting | Exact BPE Token Counting | Accurate token metrics using tiktoken `cl100k_base` BPE tokenizer | M4 | R4 |
| 15 | Budgeting | Progressive 5-Level Token Compression | Deterministic 5-level semantic degradation pipeline under `--budget <N>` | M4 | R4 |
| 16 | Patching | AST Node Range Locator | Pinpoints target AST node boundaries for surgical replacement | M5 | R5 |
| 17 | Patching | Whitespace & Indent Normalization | Preserves surrounding indentation, comments, and line endings (CRLF/LF) | M5 | R5 |
| 18 | Patching | Pre-Write Syntax Validator Guard | Tree-sitter AST validation preventing corrupted disk writes | M5 | R5 |
| 19 | Patching | Atomic Disk File Modification | Safe temporary file generation and atomic disk replacement | M5 | R5 |
| 20 | Testing | Test Context Bundle Assembler | Bundles target symbol, param/return types, mock signatures, and contracts | M6 | R6 |
| 21 | Testing | Multi-Runner Spy/Mock Scaffolding | Synthesizes mock declarations for Vitest, Jest, Pytest, Cargo, Go test | M6 | R6 |
| 22 | Testing | Workspace Fixture Discovery | Discovers and extracts reference test patterns from nearby test files | M6 | R6 |
| 23 | Git | Git Diff Slicing Engine | Automatically discovers modified symbols in working tree/staged changes | M6 | R6 |
| 24 | Routing | Web Route Handler Slicing | Maps HTTP Method + Route Path to controller AST slice and DTOs | M6 | R6 |
| 25 | Telemetry | Persistent Telemetry Logging | Records all slice invocations to append-only `~/.ctxcut/metrics.jsonl` | M6 | R6 |
| 26 | Telemetry | Terminal ROI Dashboard | Interactive ASCII dashboard with lifetime token savings and dollar ROI | M6 | R6 |
| 27 | MCP | Automated IDE MCP Setup | Configures MCP settings in Antigravity, Cursor, Claude, VS Code, Roo Code | M6 | R6 |
| 28 | Overview | Workspace Symbol Overview | High-level workspace symbol indexing without parsing entire file bodies | M8 | R2 |
| 29 | Telemetry | MCP Telemetry Metrics Inspection | Direct query tool (`get_metrics`) for cumulative token reduction & ROI | M8 | R2 |
| 30 | Verification | E2E Tier 1-4 Test Verification | 100% test pass on all unit and multi-language integration test suites | M7 | Verification |
| 31 | Verification | Tier 5 Adversarial Hardening | Adversarial stress testing, fuzzing, token invariants, and memory safety | M7 | Verification |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Smart Traversal and Timeout Guard (R1) | Ignore rules, binary detection, fast stats, MCP timeout safety | none | DONE |
| M2 | Multi-File Dependency Slicing (R2) | Cross-file imports, type hoisting, signature stripping, test fixes | M1 | DONE |
| M3 | Framework-Aware Intelligence (R3) | Django, FastAPI, React/Next.js, Express/NestJS/Spring extractors | M2 | DONE |
| M4 | Adaptive Token Budgeting (R4) | Budget constraints, 5-level progressive semantic compression | M2 | DONE |
| M5 | Bidirectional AST Patcher (R5) | Surgical AST patching, indentation aligner, syntax validator | M1 | DONE |
| M6 | Isolated Test Context Generator & CLI Extensions (R6) | Mock scaffolding, AAA test templates, fixture discovery, diff, route, metrics, setup-mcp | M2, M3 | DONE |
| M7 | Final E2E Pass & Adversarial Hardening (M7) | 100% E2E test pass (Tiers 1-4) + Tier 5 Adversarial Hardening (428+ tests) | M1-M6 | DONE |
| M8 | Workspace Symbol Overview, Batch Slicing & MCP Metrics (Expansion) | High-level symbol overview (`ctxcut overview`), multi-symbol batching (`sym1,sym2`), MCP metrics tool (`get_metrics`) | M1-M7 | DONE |

## Interface Contracts

### `ctxcut_core` ↔ `ctxcut_cli`
- `ContextSlicer::slice_symbol(path: &Path, symbol: &str, opts: &SliceOptions)` -> `Result<SliceResult>`
- `ContextSlicer::slice_symbols(path: &Path, symbols: &[&str], opts: &SliceOptions)` -> `Result<Vec<SliceResult>>`
- `ContextSlicer::slice_batch(path: &Path, symbols: &[&str], opts: &SliceOptions)` -> `Result<BatchSliceResult>`
- `AstPatcher::patch(path: &Path, symbol: &str, code: &str, opts: &PatchOptions)` -> `Result<PatchResult>`
- `TestContextGenerator::generate(path: &Path, symbol: &str, opts: &TestContextOptions)` -> `Result<TestContextResult>`
- `ProjectWalker::walk(root: &Path, config: &TraversalConfig)` -> `Result<TraversalReport>`
- `fast_stats::estimate_fast_stats(root: &Path, config: &TraversalConfig)` -> `Result<FastStatsReport>`
- `WorkspaceOverview::generate(root: &Path, opts: &OverviewOptions)` -> `Result<WorkspaceOverviewReport>`
- `TelemetryLogger::record(event: &TelemetryEvent)` -> `Result<()>`
- `TelemetryLogger::load_summary()` -> `Result<TelemetrySummary>`

### `ctxcut_core` ↔ `ctxcut_mcp`
- **Tool `get_symbol_slice`:**
  - Arguments: `path: String` (required), `symbol: String` (required, single symbol or comma-separated `sym1,sym2`), `depth: Option<usize>` (default: 1), `budget: Option<usize>`, `no_types: Option<bool>`, `no_calls: Option<bool>`, `timeout_ms: Option<u64>` (default: 10000)
- **Tool `get_diff_slice`:**
  - Arguments: `path: Option<String>` (defaults to current working directory), `staged: Option<bool>` (default: false), `budget: Option<usize>`, `timeout_ms: Option<u64>` (default: 10000)
- **Tool `analyze_token_stats`:**
  - Arguments: `path: String` (required), `fast: Option<bool>` (default: true for directories, false for single files), `timeout_ms: Option<u64>` (default: 10000)
- **Tool `patch_symbol`:**
  - Arguments: `path: String` (required), `symbol: String` (required), `code: String` (required), `dry_run: Option<bool>` (default: false), `timeout_ms: Option<u64>` (default: 10000)
- **Tool `get_test_context`:**
  - Arguments: `path: String` (required), `symbol: String` (required), `framework: Option<String>`, `budget: Option<usize>`, `timeout_ms: Option<u64>` (default: 10000)
- **Tool `get_route_slice`:**
  - Arguments: `method: String` (required), `path: String` (required), `root_dir: Option<String>`, `budget: Option<usize>`, `timeout_ms: Option<u64>` (default: 10000)
- **Tool `get_workspace_overview`:**
  - Arguments: `path: Option<String>` (defaults to current working directory), `depth: Option<usize>`, `budget: Option<usize>`, `timeout_ms: Option<u64>` (default: 10000)
- **Tool `get_metrics`:**
  - Arguments: `format: Option<String>` ("text" or "json", default: "text"), `clear: Option<bool>` (default: false), `timeout_ms: Option<u64>` (default: 10000)

## Code Layout
- `crates/ctxcut_core/src/`: Pure AST analysis engine library, traversal, resolvers, framework extractors, budget compressor, patcher, overview indexer, and telemetry logger
- `crates/ctxcut_cli/src/`: High-performance CLI interface, subcommands, formatters, clipboard integration, dashboard, and IDE MCP setup
- `crates/ctxcut_mcp/src/`: JSON-RPC 2.0 Model Context Protocol server over STDIO with thread-isolated timeout boundaries and JSONL logging
- `tests/fixtures/`: Polyglot test fixtures across TypeScript, JavaScript, Python, Go, and Rust
- `tests/`: Multi-tier unit, integration, framework, and adversarial test suites
