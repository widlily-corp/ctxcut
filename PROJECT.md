# Project: ctxcut v3.0 Next-Gen Suite

## Architecture
ctxcut v3.0 Next-Gen Suite is an ultra-fast, polyglot AST context slicing, cross-boundary execution tracing, transactional refactoring, and multi-agent swarm partitioning engine built in Rust.

### Workspace Architecture
```
ctxcut/
├── crates/
│   ├── ctxcut_core/                 # Core domain logic & AST intelligence
│   │   ├── src/
│   │   │   ├── lang/                # Tree-sitter polyglot adapters (TS, Rust, Go, Python, C#, Java, C, CPP)
│   │   │   ├── parser/              # ParserManager, AST navigation & queries
│   │   │   ├── resolver/            # Type hoisting, signature stripping, impact & callers, trace
│   │   │   ├── framework/           # Full-stack framework analyzers (Axum, Actix, Gin, FastAPI, ASP.NET, Spring, React/Next)
│   │   │   ├── schema/              # SQL DDL, Prisma, Drizzle, TypeORM, GraphQL, Proto stitchers
│   │   │   ├── fullstack/           # [R1] Full-stack cross-boundary execution tracing & client detection
│   │   │   ├── intent/              # [R2] BM25 lexical-structural index & hybrid AST intent slicing
│   │   │   ├── refactor/            # [R3] Batch multi-symbol transactional refactoring & AST diagnostic mapper
│   │   │   ├── swarm/               # [R4] Swarm graph clustering, context partitioning & boundary stub synthesizer
│   │   │   ├── verify/              # MultiFileRollbackGuard, compiler dry-runs (cargo check, tsc, go vet, mypy)
│   │   │   ├── index/               # SQLite persistent schema (.ctxcut/index.db) with sub-5ms caching
│   │   │   ├── slice/               # ContextSlicer & adaptive 5-level budget compression (1,500 - 2,000 tokens)
│   │   │   └── tokenizer/           # TokenCounter (cl100k_base via tiktoken-rs)
│   ├── ctxcut_cli/                  # Unified CLI binary (trace-api, slice-intent, refactor batch, pack-agent-context)
│   └── ctxcut_mcp/                  # STDIO JSON-RPC 2.0 MCP server with timeout & panic isolation
└── tests/                           # 5-Tier E2E verification test harness
    ├── tier1_features/              # Tier 1: Unit & feature contracts
    ├── tier2_boundaries/            # Tier 2: Boundary & adversarial error recovery
    ├── tier3_polyglot/              # Tier 3: Cross-language integration (TS + Rust + Go + Python + SQL)
    ├── tier4_benchmarks/            # Tier 4: Latency & token budget benchmarks
    └── tier5_protocol/              # Tier 5: STDIO MCP protocol & dogfooding
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Polyglot Client Detection | Detect `fetch`, `axios`, React Query, `trpc`, `graphql`, `grpc-web` in TS/JS AST | M1 | ORIGINAL_REQUEST §R1 |
| 2 | Polyglot Route Resolver | Resolve routes in Axum, Actix-web, Gin, Chi, FastAPI, Flask, ASP.NET Core, Spring Boot | M1 | ORIGINAL_REQUEST §R1 |
| 3 | DTO & DDL Stitching | Stitch request/response DTOs and SQL migrations (Prisma, Drizzle, TypeORM, SQL DDL) | M1 | ORIGINAL_REQUEST §R1 |
| 4 | Linear Trace & Budgeting | Build 6-step linear execution trace under adaptive 1,500-2,000 token budget | M1 | ORIGINAL_REQUEST §R1 |
| 5 | Persistent Route Index | SQLite index tables (`routes`, `client_endpoints`, `schema_entities`) in `.ctxcut/index.db` | M1 | ORIGINAL_REQUEST §R1 |
| 6 | BM25 Lexical Index | Multi-field term extraction, IDF, and BM25 ranking across symbol names, types, and docs | M2 | ORIGINAL_REQUEST §R2 |
| 7 | Hybrid AST Intent Slicer | Natural language task matching combining BM25 and Tree-sitter dependency traversal | M2 | ORIGINAL_REQUEST §R2 |
| 8 | >85% Token Reduction | Extract minimal critical AST context bundle with verified >85% token savings | M2 | ORIGINAL_REQUEST §R2 |
| 9 | Sub-5ms SQLite Postings | SQLite inverted index tables (`bm25_terms`, `bm25_postings`, `bm25_doc_stats`) | M2 | ORIGINAL_REQUEST §R2 |
| 10 | Batch AST Mutation Engine | Multi-symbol, multi-file atomic AST patcher with reverse byte offset splicing | M3 | ORIGINAL_REQUEST §R3 |
| 11 | Multi-File Rollback Guard | Transactional journal and rollback guard for atomic multi-file disk restoration | M3 | ORIGINAL_REQUEST §R3 |
| 12 | Compiler Dry-Run Engine | Isolated compiler verification (`cargo check`, `tsc`, `go vet`, `mypy`) with rollback | M3 | ORIGINAL_REQUEST §R3 |
| 13 | AST Diagnostic Mapping | Map compiler diagnostics (`VerifyDiagnostic`) back to target AST nodes and patch lines | M3 | ORIGINAL_REQUEST §R3 |
| 14 | Swarm Graph Clustering | Partition repository graph into $K$ isolated, non-overlapping AST clusters | M4 | ORIGINAL_REQUEST §R4 |
| 15 | Boundary Stub Synthesizer | Generate stripped contract interfaces and mock stubs for inter-agent boundaries | M4 | ORIGINAL_REQUEST §R4 |
| 16 | Swarm Context Packager | Package independent agent context bundles with write authority vs immutable contract annotations | M4 | ORIGINAL_REQUEST §R4 |
| 17 | CLI Subcommands | Expose `trace-api`, `slice-intent`, `refactor batch`, `pack-agent-context` in `ctxcut_cli` | M5 | ORIGINAL_REQUEST §R5 |
| 18 | MCP STDIO Server Tools | Expose `get_fullstack_trace`, `get_intent_slice`, `patch_transaction`, `pack_agent_context` in `ctxcut_mcp` | M5 | ORIGINAL_REQUEST §R5 |
| 19 | Zero Clippy & Warnings | Maintain 100% clean compilation and 0 clippy warnings (`-- -D warnings`) | M5 | ORIGINAL_REQUEST §R5 |
| 20 | 5-Tier E2E Test Suite | Comprehensive unit, boundary, polyglot, benchmark, and protocol tests (Tiers 1-5) | E2E | ORIGINAL_REQUEST §R6 |
| 21 | Binary Dogfooding | Verify local installation via `cargo install --path .` and `ctxcut --help` | M6 | ORIGINAL_REQUEST §R6 |
| 22 | Git Conventional Release | Commit changes via Conventional Commits and synchronize to remote GitHub repository | M6 | ORIGINAL_REQUEST §R6 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| E2E | E2E Testing Suite | Multi-tier test harness covering R1-R6 features, boundaries, and MCP protocols | none | DONE (TEST_READY.md published) |
| M1 | Cross-Boundary Full-Stack Tracing | Polyglot client detection, server route resolution, DTO/DDL stitching, trace & budget | none | DONE (Gate PASSED: 100% test pass, CLEAN audit) |
| M2 | Semantic Intent & Hybrid AST Slicing | BM25 lexical engine, AST dependency traversal, >85% token reduction, SQLite index | M1 | DONE (Implemented & verified in ctxcut_core::intent) |
| M3 | Multi-Symbol Transactional Refactoring | Batch AST patcher, MultiFileRollbackGuard, compiler dry-runs, AST diagnostic mapping | none | DONE (Implemented & verified in ctxcut_core::refactor::batch) |
| M4 | Swarm Context Partitioning | Graph clustering, non-overlapping AST slices, boundary contract stub generation | M2 | DONE (Implemented & verified in ctxcut_core::swarm) |
| M5 | CLI & MCP Tooling Integration | `ctxcut_cli` commands, `ctxcut_mcp` tools, zero Clippy warnings, dogfooding | M1, M2, M3, M4 | DONE (CLI subcommands & 19 MCP tools integrated) |
| M6 | Release Pipeline & Git Synchronization | Full test pass verification, `cargo install`, conventional commit & GitHub push | M5, E2E | DONE (1,123 tests pass, cargo install, commit 4a7598e pushed) |

## Interface Contracts

### `crates/ctxcut_core::fullstack` (M1)
```rust
pub struct ClientApiCall {
    pub client_kind: String, // "fetch", "axios", "react_query", "trpc", "graphql", "grpc_web"
    pub http_method: Option<String>,
    pub endpoint_url: Option<String>,
    pub rpc_procedure: Option<String>,
    pub file_path: String,
    pub line_number: usize,
    pub call_snippet: String,
    pub request_dto: Option<String>,
    pub response_dto: Option<String>,
}

pub struct ServerRouteEndpoint {
    pub framework: String,
    pub http_method: String,
    pub route_path: String,
    pub handler_file: String,
    pub handler_symbol: String,
    pub handler_signature: String,
    pub request_dto_type: Option<ExtractedType>,
    pub response_dto_type: Option<ExtractedType>,
}

pub struct FullstackTraceStep {
    pub step_number: usize,
    pub layer: String, // "client_call", "route_handler", "middleware_guard", "service_logic", "data_access", "schema_ddl"
    pub title: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub snippet: String,
    pub schema_contract: Option<String>,
}

pub struct FullstackTraceResult {
    pub query_endpoint: String,
    pub client_call: Option<ClientApiCall>,
    pub server_route: ServerRouteEndpoint,
    pub steps: Vec<FullstackTraceStep>,
    pub total_steps: usize,
    pub stats: TokenStats,
}

pub trait FullstackTracer {
    fn trace_api(&self, root_dir: &Path, endpoint_or_proc: &str, budget: Option<usize>) -> Result<FullstackTraceResult>;
}
```

### `crates/ctxcut_core::intent` (M2)
```rust
pub struct IntentSliceOptions {
    pub prompt: String,
    pub budget: Option<usize>, // Target budget (default: 1500)
    pub max_target_symbols: usize,
    pub depth: usize,
}

pub struct IntentSliceResult {
    pub prompt: String,
    pub matched_intent_keywords: Vec<String>,
    pub target_symbols: Vec<ExtractedSymbol>,
    pub hoisted_types: Vec<ExtractedType>,
    pub upstream_callers: Vec<ImpactCallerItem>,
    pub database_schemas: Vec<ExtractedType>,
    pub stats: TokenStats,
    pub token_savings_pct: f64,
    pub degradation_level: u8,
}

pub trait IntentSlicer {
    fn slice_intent(&self, root_dir: &Path, opts: &IntentSliceOptions) -> Result<IntentSliceResult>;
}
```

### `crates/ctxcut_core::refactor::batch` & `verify` (M3)
```rust
pub struct SymbolPatchUnit {
    pub file_path: PathBuf,
    pub symbol_query: String,
    pub replacement_code: String,
}

pub struct PatchTransactionRequest {
    pub workspace_root: Option<PathBuf>,
    pub patches: Vec<SymbolPatchUnit>,
    pub typechecker: Option<String>,
    pub apply: bool,
    pub timeout_ms: Option<u64>,
}

pub struct MappedPatchDiagnostic {
    pub file_path: String,
    pub symbol_name: Option<String>,
    pub node_kind: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub patch_relative_line: Option<usize>,
    pub code_snippet: Option<String>,
    pub code: Option<String>,
    pub message: String,
    pub severity: String,
}

pub struct PatchTransactionResult {
    pub success: bool,
    pub applied: bool,
    pub rolled_back: bool,
    pub files_modified_count: usize,
    pub symbols_patched_count: usize,
    pub diffs: Vec<FilePatchDiff>,
    pub typechecker_command: Option<String>,
    pub exit_code: Option<i32>,
    pub diagnostics: Vec<MappedPatchDiagnostic>,
    pub syntax_errors: Vec<SyntaxErrorDetail>,
    pub duration_ms: u64,
}

pub trait TransactionalPatcher {
    fn patch_transaction(&self, req: &PatchTransactionRequest) -> Result<PatchTransactionResult>;
}
```

### `crates/ctxcut_core::swarm` (M4)
```rust
pub struct SwarmAgentPack {
    pub agent_id: String,
    pub cluster_name: String,
    pub internal_symbols: Vec<ExtractedSymbol>,
    pub boundary_stubs: Vec<CallSignatureStub>,
    pub boundary_types: Vec<ExtractedType>,
    pub mock_contracts: String,
    pub token_stats: TokenStats,
}

pub struct SwarmPartitionManifest {
    pub total_agents: usize,
    pub total_symbols: usize,
    pub boundary_contracts_count: usize,
    pub packs: Vec<SwarmAgentPack>,
}

pub trait SwarmPartitionEngine {
    fn partition_workspace(&self, root_dir: &Path, agents_count: usize, seed_symbols: &[String], budget_per_agent: Option<usize>) -> Result<SwarmPartitionManifest>;
}
```

## Code Layout
- `crates/ctxcut_core/src/fullstack/`: Polyglot client AST scanners, server route matchers, and execution trace builder.
- `crates/ctxcut_core/src/intent/`: BM25 tokenizer, inverted index postings, hybrid AST ranker, and critical bundle slicer.
- `crates/ctxcut_core/src/refactor/batch.rs`: Multi-file transactional AST patcher and reverse offset byte splicing engine.
- `crates/ctxcut_core/src/verify/multi_rollback.rs`: `MultiFileRollbackGuard` and transactional filesystem journal.
- `crates/ctxcut_core/src/verify/ast_mapper.rs`: Compiler diagnostic to AST node & patch-relative line mapper.
- `crates/ctxcut_core/src/swarm/`: Graph builder, community clustering, non-overlapping partitioner, and boundary stub generator.
- `crates/ctxcut_core/src/index/schema.rs` & `sqlite.rs`: Extended SQLite tables for routes, clients, entities, and BM25 postings.
- `crates/ctxcut_cli/src/`: CLI subcommands routing for `trace-api`, `slice-intent`, `refactor batch`, `pack-agent-context`.
- `crates/ctxcut_mcp/src/`: MCP tool handlers for `get_fullstack_trace`, `get_intent_slice`, `patch_transaction`, `pack_agent_context`.
- `tests/tier1_features/` through `tests/tier5_protocol/`: 5-tier test suites.
