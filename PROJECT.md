# Project: ctxcut Architectural & Functional Upgrade

## Architecture
The `ctxcut` project is organized as a high-performance Rust workspace (edition 2021) comprising:
- `crates/ctxcut_core`: Core AST parsing engine (tree-sitter 0.24 for TS/JS/Python/Go/Rust), language adapters, import resolvers, type hoisters, signature strippers, framework analyzers, budget compressor, AST patcher, test context generator, smart traversal walker, and BPE tokenizer (tiktoken-rs cl100k_base).
- `crates/ctxcut_cli`: Command-line frontend (clap 4.5) providing `slice`, `patch`, `test-context`, `diff`, `stats` (--fast), `metrics`, `route`, `setup-mcp`, and `init` commands.
- `crates/ctxcut_mcp`: Model Context Protocol (MCP) JSON-RPC 2.0 stdio server exposing 6 core tools with timeout safety guards and structured diagnostic logging.
- `tests/`: 5-Tier test suite covering functional features (Tier 1), boundary & error conditions (Tier 2), cross-module combinations (Tier 3), real-world multi-framework workloads (Tier 4), and adversarial/mutation scenarios (Tier 5).

## Feature Inventory
Every requirement from ORIGINAL_REQUEST.md is enumerated below with assigned milestones:

| # | Feature | Description | Milestone | Source |
|---|---|---|---|---|
| 1 | .gitignore & .ctxcutignore Support | Honor .gitignore and .ctxcutignore rules during project traversal | M1 | R1 |
| 2 | Built-in Vendor/Build Blacklist | Automatically ignore node_modules, .git, target, dist, .pytest_cache, .venv, etc. | M1 | R1 |
| 3 | Fast Token Estimation Scan Mode | Shallow repository token estimation scan (`--fast`) without full AST builds | M1 | R1 |
| 4 | MCP Execution Timeout Safety | Guard MCP tool invocations with deadlines, returning structured partial results/errors | M1 | R1 |
| 5 | Cross-File Import Resolution | Resolve local project file imports across TS/JS, Python, Go, and Rust | M2 | R2 |
| 6 | Multi-File Signature Extraction | Extract verbatim stripped signatures and interfaces from imported neighbor files | M2 | R2 |
| 7 | Cross-File Type Hoisting (--depth 1) | Hoist types and data contracts across imported module boundaries | M2 | R2 |
| 8 | FastAPI & Pydantic Extractor | Extract route parameters, Pydantic schemas, and Depends dependencies | M3 | R3 |
| 9 | Django & DRF Extractor | Capture serializers, models, permission classes, and viewset schemas | M3 | R3 |
| 10 | React & Next.js Extractor | Extract Component Props interfaces and referenced custom hooks | M3 | R3 |
| 11 | JSX Branch Collapser | Collapse deep secondary JSX rendering branches to compact stubs | M3 | R3 |
| 12 | Express/NestJS/Spring DTOs | Capture route DTOs, controllers, and middleware chains | M3 | R3 |
| 13 | Adaptive Token Budgeting Flag | Support `--budget <N>` parameter in CLI and MCP slicing tools | M4 | R4 |
| 14 | Progressive Semantic Degradation | Deterministic 5-level compression fallback to fit strict token constraints | M4 | R4 |
| 15 | AST Node Locator & Byte Range | Exact AST node boundary identification preserving surrounding indentation & whitespace | M5 | R5 |
| 16 | Surgical AST Code Replacement | In-memory replacement splicing with indentation normalization | M5 | R5 |
| 17 | AST Syntax Validation Guard | Tree-sitter validation ensuring no syntax regressions or AST errors before writing | M5 | R5 |
| 18 | CLI & MCP `patch` Tooling | CLI `ctxcut patch` and MCP `patch_symbol` with dry-run diff capabilities | M5 | R5 |
| 19 | Isolated Test Context Generator | Assemble target symbol, parameter/return types, and mock signatures | M6 | R6 |
| 20 | Test Mock & Spy Scaffolding | Generate mock/spy declarations tailored to jest/vitest/pytest/cargo/gotest | M6 | R6 |
| 21 | Reference Fixture Finder | Automatically discover and incorporate nearby project test patterns | M6 | R6 |
| 22 | CLI & MCP `test-context` Tooling | CLI `ctxcut test-context` and MCP `get_test_context` tools | M6 | R6 |
| 23 | CLI Subcommands & Flags Update | Update clap CLI with all new subcommands (`patch`, `test-context`) and flags (`--budget`, `--fast`, `--depth`) | M7 | AC |
| 24 | Complete 6-Pillar MCP Tools Suite | Expose `get_symbol_slice`, `get_diff_slice`, `analyze_token_stats`, `patch_symbol`, `get_test_context`, `get_route_slice` | M7 | AC |
| 25 | Strict Zero-Clippy Compliance | Resolve all clippy warnings (`cargo clippy --all-targets --all-features -- -D warnings`) | M7 | AC |
| 26 | E2E Multi-Framework Integration Suite | Pass 100% E2E tests across Django, FastAPI, React/Next.js, and TypeScript backends | Final | AC |

## Milestones

| # | Name | Scope | Dependencies | Status |
|---|---|---|---|---|
| M1 | Smart Traversal, Ignore Rules & Timeout Guard | `ctxcut_core::traversal`, `ctxcut_cli::stats` fast scan, MCP timeout safety | none | DONE |
| M2 | Multi-File Dependency Slicing (--depth 1) | `ctxcut_core::resolver` cross-file import locator & signature stripper | none | PLANNED |
| M3 | Framework-Aware Semantic Intelligence | `ctxcut_core::framework` for Django, FastAPI, React/Next.js, Express/NestJS/Spring | none | PLANNED |
| M4 | Adaptive Token Budgeting | `ctxcut_core::slice::budget` progressive 5-tier semantic degradation engine | M2, M3 | PLANNED |
| M5 | Bidirectional AST Patcher | `ctxcut_core::patch` node locator, indentation aligner, syntax validator | none | PLANNED |
| M6 | Isolated Test Context Generator | `ctxcut_core::test_context` mock generator & test fixture finder | M2, M3 | PLANNED |
| M7 | CLI & MCP Server Integration & Clippy Clean | `ctxcut_cli`, `ctxcut_mcp` tool registrations, schemas, error formats, clippy | M1-M6 | PLANNED |
| Final | E2E Multi-Framework Verification & Adversarial Hardening | Pass 100% E2E test suite (Tiers 1-4) & Tier 5 adversarial coverage hardening | M7 | PLANNED |

## Interface Contracts

### Traversal & Fast Scan (`ctxcut_core::traversal`)
```rust
pub struct TraversalConfig {
    pub respect_gitignore: bool,
    pub respect_ctxcutignore: bool,
    pub max_file_size_bytes: u64,
}

pub struct ProjectWalker;
impl ProjectWalker {
    pub fn walk(root: &Path, config: &TraversalConfig) -> impl Iterator<Item = PathBuf>;
    pub fn estimate_fast_stats(root: &Path, timeout_secs: Option<u64>) -> Result<FastStatsReport>;
}
```

### Cross-File Import Resolver (`ctxcut_core::resolver`)
```rust
pub trait ForeignSymbolLocator: Send + Sync {
    fn resolve_import_path(&self, current_file: &Path, import_spec: &str) -> Option<PathBuf>;
    fn locate_foreign_signature(&self, target_file: &Path, symbol_name: &str) -> Result<Option<CallSignatureStub>>;
}
```

### Framework Semantic Analyzer (`ctxcut_core::framework`)
```rust
pub trait FrameworkAnalyzer: Send + Sync {
    fn matches_framework(&self, path: &Path, source: &str) -> bool;
    fn enhance_slice(&self, target_node: Node, source: &str, path: &Path, slice: &mut SliceResult) -> Result<()>;
    fn collapse_jsx_branches(&self, source: &str, node: Node) -> Option<String>;
}
```

### Adaptive Budgeting (`ctxcut_core::slice::budget`)
```rust
pub struct BudgetCompressor;
impl BudgetCompressor {
    pub fn compress_slice(slice: &mut SliceResult, budget_tokens: usize) -> Result<DegradationReport>;
}
```

### Bidirectional AST Patcher (`ctxcut_core::patch`)
```rust
pub struct AstPatcher;
impl AstPatcher {
    pub fn patch_symbol(file_path: &Path, symbol_query: &str, replacement_code: &str, dry_run: bool) -> Result<PatchResult>;
}
```

### Isolated Test Context (`ctxcut_core::test_context`)
```rust
pub struct TestContextGenerator;
impl TestContextGenerator {
    pub fn generate(file_path: &Path, symbol_query: &str, framework: Option<&str>, opts: &SliceOptions) -> Result<TestContextResult>;
}
```

## Code Layout
```
crates/ctxcut_core/src/
├── lib.rs
├── model.rs                    # Extended SliceOptions, PatchResult, TestContextResult
├── error.rs                    # CoreError variants for patch/traversal/framework
├── parser/mod.rs
├── lang/                       # TS/JS, Python, Go, Rust adapters
├── resolver/                   # Cross-file imports, type hoister, signature stripper
├── traversal/mod.rs            # [M1] TraversalConfig, ProjectWalker, fast scanner
├── framework/                  # [M3] Django, FastAPI, React/Next, Express/Nest/Spring
│   ├── mod.rs
│   ├── django_fastapi.rs
│   ├── react_next.rs
│   └── express_nest_spring.rs
├── slice/
│   ├── mod.rs                  # ContextSlicer
│   └── budget.rs               # [M4] Progressive budget degradation
├── patch/mod.rs                # [M5] AstPatcher & indentation normalizer
└── test_context/               # [M6] TestContextGenerator & mock scaffolder
    ├── mod.rs
    └── fixture_finder.rs

crates/ctxcut_cli/src/
├── lib.rs                      # Cli struct & subcommand router
├── stats.rs                    # Fast & deep stats calculation
├── commands/                   # Subcommand handlers (slice, patch, test_context, diff, stats)
└── route.rs

crates/ctxcut_mcp/src/
├── lib.rs                      # JSON-RPC server with timeout safety wrapper
├── tools/                      # 6 MCP tools implementations
└── logger.rs                   # Clean STDIO logger
```
