# Project: ctxcut

> Lightning-fast Rust CLI and Model Context Protocol (MCP) server for AST-based dependency slicing across TypeScript/JavaScript, Python, Go, and Rust. Zero token bloat.

---

## Architecture

`ctxcut` is structured as a high-performance modular Rust workspace consisting of three specialized crates and a root CLI/MCP binary entry point:

```
ctxcut/
├── Cargo.toml                  # Workspace root manifest
├── crates/
│   ├── ctxcut_core/            # Pure AST engine, dependency graph traversal, slicing, markdown formatter, BPE tokenizer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs          # Public library API (ContextSlicer, SliceOptions, SliceResult)
│   │       ├── error.rs        # CoreError enum (thiserror)
│   │       ├── model.rs        # Data structures (SymbolLocation, ExtractedSymbol, ExtractedType, CallSignatureStub)
│   │       ├── lang/           # Language trait & grammar adapters
│   │       │   ├── mod.rs      # LanguageAdapter trait
│   │       │   ├── typescript.rs # TS/JS tree-sitter adapter
│   │       │   ├── python.rs   # Python tree-sitter adapter
│   │       │   ├── go.rs       # Go tree-sitter adapter
│   │       │   └── rust_lang.rs# Rust tree-sitter adapter
│   │       ├── parser/         # Tree-sitter wrapper & query runner
│   │       ├── resolver/       # Symbol locator, import resolution, type hoisting, signature stripping
│   │       ├── slice/          # ContextSlicer orchestration pipeline
│   │       ├── formatter/      # Prompt-optimized Markdown & JSON output generators
│   │       └── tokenizer/      # BPE token counter & estimation metrics (tiktoken-rs)
│   │
│   ├── ctxcut_cli/             # CLI frontend (clap derive, arboard clipboard, colored terminal UI, git diff, route resolver)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs          # Public CLI interface (run_cli)
│   │       ├── args.rs         # Clap derive CLI arguments
│   │       ├── clip.rs         # Arboard clipboard wrapper with headless fallback
│   │       ├── ui.rs           # Terminal formatting & tables
│   │       ├── git/            # Git diff parsing & AST symbol intersection
│   │       ├── routes/         # Web framework route resolvers (Express, FastAPI, Actix, Gin, Axum)
│   │       └── commands/       # Subcommand handlers (slice, diff, stats, route)
│   │
│   └── ctxcut_mcp/             # Model Context Protocol (MCP) stdio JSON-RPC server
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs          # Public MCP runner (run_mcp_server)
│           ├── protocol.rs     # JSON-RPC 2.0 types
│           ├── schema.rs       # MCP Tool schemas & capabilities
│           ├── server.rs       # STDIO read/write event loop
│           └── tools/          # Tool executors (get_symbol_slice, get_diff_slice, analyze_token_stats)
│
├── src/
│   └── main.rs                 # Root binary: passes CLI/MCP invocation to crates
├── tests/                      # 4-Tier E2E integration test suite & fixtures
└── benches/                    # Criterion performance benchmark suite
```

---

## Feature Inventory

| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Workspace Setup & Cargo Configuration | Multi-crate workspace manifest, dependency inheritance, strict clippy policies | M1 | Survey |
| 2 | Tree-Sitter AST Core & TS/JS Grammar | AST parsing engine for TypeScript/JavaScript (.ts, .tsx, .js, .jsx) | M1 | R1, §3.1 |
| 3 | Symbol Locator (TS/JS) | Locate target function, method, class, or type by name or range | M1 | R1, §2.1 |
| 4 | Type Hoister (TS/JS) | Extract and inline referenced interfaces, type aliases, enums, DTOs | M1 | R2, §2.1 |
| 5 | Signature Stripper (TS/JS) | Strip 100% of bodies from called external functions, retaining signatures | M1 | R2, §2.1 |
| 6 | Markdown & JSON Formatter | Render prompt-optimized Markdown slices with metrics and JSON output | M1 | R2, §2.1 |
| 7 | BPE Token Counter | Calculate exact OpenAI BPE token savings percentage using tiktoken-rs | M1 | R2, §2.1 |
| 8 | Python AST Grammar & Slicing | Python tree-sitter adapter: def, async def, class, Pydantic, PEP 695 type aliases | M2 | R1, R2 |
| 9 | Go AST Grammar & Slicing | Go tree-sitter adapter: func, receiver methods, struct, interface, package resolution | M2 | R1, R2 |
| 10 | Rust AST Grammar & Slicing | Rust tree-sitter adapter: fn, async fn, impl methods, trait definitions, struct/enum | M2 | R1, R2 |
| 11 | CLI Framework (`ctxcut_cli`) | Clap derive CLI with colorful formatting and global error handling | M3 | R3, §3.2 |
| 12 | `ctxcut slice` Command | Extract slice for single or comma-separated symbols with `-o` and `--clip` | M3 | R3, §3.2 |
| 13 | Clipboard Integration (`arboard`) | Direct copy to OS clipboard with graceful headless CI fallback | M3 | R3, §3.2 |
| 14 | `ctxcut diff` Command | Parse git diff / staged changes, intersect with AST, batch slice touched functions | M3 | R3, §3.2 |
| 15 | `ctxcut stats` Command | Scan repo using ignore walker, calculate total tokens and savings report | M3 | R3, §3.2 |
| 16 | `ctxcut route` Command | Multi-framework route handler resolver (Express, FastAPI, Actix, Gin, Axum) | M3 | R3, §3.2 |
| 17 | Root Binary (`main.rs`) | Unified entry point routing subcommands to `ctxcut_cli` or `ctxcut_mcp` | M3 | Architecture |
| 18 | MCP STDIO Server (`ctxcut_mcp`)| Model Context Protocol JSON-RPC 2.0 server over STDIO | M4 | R4, §3.3 |
| 19 | MCP Tool: `get_symbol_slice` | MCP tool extracting AST context slice by file path and symbol name | M4 | R4, §3.3 |
| 20 | MCP Tool: `get_diff_slice` | MCP tool extracting AST slices for git diff changes | M4 | R4, §3.3 |
| 21 | MCP Tool: `analyze_token_stats`| MCP tool reporting token savings metrics across repository path | M4 | R4, §3.3 |
| 22 | E2E Test Suite (Tiers 1-4) | Comprehensive test suite with fixtures, boundary tests, and workloads | M5 / Test Track | R5, §6.1 |
| 23 | Golden Snapshot Suite | `insta` golden snapshot testing for normalized Markdown output | M5 / Test Track | R5, §6.1 |
| 24 | Adversarial Hardening (Tier 5) | White-box adversarial testing, edge-case coverage | M6 | Project Pattern |
| 25 | Criterion Benchmarks | Benchmarking suite verifying sub-10ms parse and slicing SLA | M6 | R5, §6.1 |
| 26 | Zero-Lint Quality Verification | Verification of 0 warnings on `cargo clippy --all-targets -- -D warnings` | M6 | Acceptance Criteria |

---

## Milestones

| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Workspace Foundation & Core AST Engine (TS/JS) | Workspace root setup, `ctxcut_core` foundation, TS/JS tree-sitter parser, symbol locator, type hoister, signature stripper, markdown formatter, BPE token counter, core unit tests | none | IN_PROGRESS |
| M2 | Multi-Language AST Support (Python, Go, Rust) | Language adapters for Python, Go, and Rust in `ctxcut_core`, grammar queries, language-specific hoisting & stripping rules, unit tests | M1 | PLANNED |
| M3 | CLI, Clipboard, Git Diff & Route Resolver | `crates/ctxcut_cli` (clap derive, `slice`, `diff`, `stats`, `route`), `arboard` clipboard, multi-framework route heuristics, `src/main.rs` | M2 | PLANNED |
| M4 | Model Context Protocol (MCP) STDIO Server | `crates/ctxcut_mcp` (JSON-RPC 2.0 stdio server, tool schemas & handlers for `get_symbol_slice`, `get_diff_slice`, `analyze_token_stats`), `ctxcut mcp` | M3 | PLANNED |
| M5 | Final Milestone Phase 1: E2E Test Suite Pass | Integrate test suite (Tiers 1-4), fix all edge cases, pass 100% of integration tests and golden snapshots | M4, TEST_READY | PLANNED |
| M6 | Final Milestone Phase 2: Adversarial Hardening, Benchmarks & Zero-Lint Polish | Tier 5 white-box adversarial testing, Criterion benchmark suite (<10ms SLA), `cargo clippy --all-targets -- -D warnings` validation | M5 | PLANNED |

---

## Interface Contracts

### `ctxcut_core` ↔ `ctxcut_cli` & `ctxcut_mcp`

```rust
// Public API of ctxcut_core

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportedLanguage {
    TypeScript,
    JavaScript,
    Python,
    Go,
    Rust,
}

#[derive(Debug, Clone)]
pub struct SliceOptions {
    pub depth: usize,              // Type hoisting traversal depth (default 1)
    pub include_types: bool,       // Include type hoisting (default true)
    pub include_calls: bool,       // Include signature stripping (default true)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedSymbol {
    pub name: String,
    pub kind: String,              // "function", "method", "class", "type"
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub doc_comment: Option<String>,
    pub signature: String,
    pub body: String,
    pub language: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedType {
    pub name: String,
    pub kind: String,              // "interface", "type_alias", "enum", "struct"
    pub file_path: String,
    pub definition: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CallSignatureStub {
    pub name: String,
    pub receiver: Option<String>,
    pub file_path: Option<String>,
    pub signature: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenStats {
    pub raw_file_tokens: usize,
    pub sliced_tokens: usize,
    pub savings_percentage: f64,
    pub raw_lines: usize,
    pub sliced_lines: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SliceResult {
    pub target_symbol: ExtractedSymbol,
    pub hoisted_types: Vec<ExtractedType>,
    pub stripped_calls: Vec<CallSignatureStub>,
    pub stats: TokenStats,
}

impl SliceResult {
    pub fn to_markdown(&self) -> String;
    pub fn to_json(&self) -> String;
}

pub struct ContextSlicer;

impl ContextSlicer {
    pub fn new() -> Self;
    pub fn detect_language(path: &std::path::Path) -> Result<SupportedLanguage, CoreError>;
    pub fn slice_symbol(
        &self,
        file_path: &std::path::Path,
        symbol_name: &str,
        opts: &SliceOptions,
    ) -> Result<SliceResult, CoreError>;
    pub fn slice_symbols(
        &self,
        file_path: &std::path::Path,
        symbol_names: &[&str],
        opts: &SliceOptions,
    ) -> Result<Vec<SliceResult>, CoreError>;
}
```

### `ctxcut_cli` ↔ Root Binary (`main.rs`)

```rust
// Public API of ctxcut_cli
pub fn run_cli() -> Result<(), anyhow::Error>;
```

### `ctxcut_mcp` ↔ Root Binary (`main.rs`)

```rust
// Public API of ctxcut_mcp
pub fn run_mcp_server() -> Result<(), anyhow::Error>;
```

---

## Code Layout

```
ctxcut/
├── Cargo.toml
├── Cargo.lock
├── clippy.toml
├── rustfmt.toml
├── src/
│   └── main.rs
├── crates/
│   ├── ctxcut_core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       ├── model.rs
│   │       ├── lang/
│   │       │   ├── mod.rs
│   │       │   ├── typescript.rs
│   │       │   ├── python.rs
│   │       │   ├── go.rs
│   │       │   └── rust_lang.rs
│   │       ├── parser/
│   │       │   └── mod.rs
│   │       ├── resolver/
│   │       │   ├── mod.rs
│   │       │   ├── symbol.rs
│   │       │   ├── imports.rs
│   │       │   ├── types.rs
│   │       │   └── calls.rs
│   │       ├── slice/
│   │       │   └── mod.rs
│   │       ├── formatter/
│   │       │   └── mod.rs
│   │       └── tokenizer/
│   │           └── mod.rs
│   ├── ctxcut_cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── args.rs
│   │       ├── clip.rs
│   │       ├── ui.rs
│   │       ├── git/
│   │       │   ├── mod.rs
│   │       │   └── diff.rs
│   │       ├── routes/
│   │       │   ├── mod.rs
│   │       │   ├── express.rs
│   │       │   ├── fastapi.rs
│   │       │   ├── actix.rs
│   │       │   ├── gin.rs
│   │       │   └── axum.rs
│   │       └── commands/
│   │           ├── mod.rs
│   │           ├── slice.rs
│   │           ├── diff.rs
│   │           ├── stats.rs
│   │           └── route.rs
│   └── ctxcut_mcp/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── protocol.rs
│           ├── schema.rs
│           ├── server.rs
│           └── tools/
│               ├── mod.rs
│               ├── get_symbol_slice.rs
│               ├── get_diff_slice.rs
│               └── analyze_token_stats.rs
├── tests/
│   ├── common/
│   ├── fixtures/
│   │   ├── typescript/
│   │   ├── python/
│   │   ├── go/
│   │   └── rust/
│   ├── tier1_features/
│   ├── tier2_boundaries/
│   ├── tier3_cross_feature/
│   ├── tier4_real_world/
│   └── snapshots/
└── benches/
    ├── parse_benchmark.rs
    ├── extraction_benchmark.rs
    ├── hoisting_benchmark.rs
    └── e2e_slice_benchmark.rs
```
