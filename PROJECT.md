# Project: ctxcut v2.0 Upgrade

## Architecture
ctxcut is a high-performance, token-efficient AST context slicing and refactoring tool for AI agents and developers.
The workspace consists of three primary crates:
1. `crates/ctxcut_core`: Core AST parsing (tree-sitter), symbol extraction, signature stripping, type hoisting, implementor discovery, impact analysis, execution tracing, schema stitching, verification guard, semantic diff, refactoring, persistent SQLite indexing, and query engine.
2. `crates/ctxcut_cli`: Command-line interface with Clap subcommands (`slice`, `callers`, `trace`, `overview`, `diff`, `semantic-diff`, `patch`, `verify-patch`, `refactor`, `query`, `index`, `tui`, `metrics`, `setup-mcp`, `upgrade`).
3. `crates/ctxcut_mcp`: High-concurrency STDIO Model Context Protocol server exposing AST slicing, impact, trace, schema, semantic diff, verification, refactoring, and query tools.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | `ctxcut callers` & `get_impact_slice` | Upstream caller / reverse impact slicing across multi-crate workspace | M1 | ORIGINAL_REQUEST §R1 |
| 2 | `ctxcut trace` & `get_trace_slice` | End-to-end execution flow tracing under 1,000–2,000 token budget | M1 | ORIGINAL_REQUEST §R1 |
| 3 | Interface & Trait Implementor Hoisting | Hoist concrete implementors in Rust (`impl Trait`), Go (duck typing), TS (`implements`), Python (`Protocol`) | M1 | ORIGINAL_REQUEST §R1 |
| 4 | C / C++ Support | `tree-sitter-c`, `tree-sitter-cpp`, classes, structs, templates, headers, macro stripping | M2 | ORIGINAL_REQUEST §R2 |
| 5 | C# / .NET Support | `tree-sitter-c-sharp`, ASP.NET Core controllers, records, DTOs | M2 | ORIGINAL_REQUEST §R2 |
| 6 | Java / Kotlin Support | `tree-sitter-java`, `tree-sitter-kotlin`, Spring Boot controllers/entities, JPA | M2 | ORIGINAL_REQUEST §R2 |
| 7 | Vue / Svelte / Astro SFCs | Extract `<script setup>` and props while collapsing templates & styles | M2 | ORIGINAL_REQUEST §R2 |
| 8 | ORM & Schema Stitching | Auto-stitch Prisma models, Drizzle schemas, TypeORM, raw SQL with migration DDLs, Proto, GraphQL | M3 | ORIGINAL_REQUEST §R3 |
| 9 | Verification Guard (`verify-patch`) | Typecheck dry-run (`cargo check`, `tsc`, `mypy`, `go vet`) with RAII auto-rollback | M4 | ORIGINAL_REQUEST §R4 |
| 10 | Semantic AST Diff (`semantic-diff`) | Token-efficient structural AST diff calculating signature/type deltas & ROI savings | M4 | ORIGINAL_REQUEST §R4 |
| 11 | AST Symbol Renaming (`refactor rename`) | Multi-file AST-accurate symbol renaming updating declarations, usages, and imports | M4 | ORIGINAL_REQUEST §R4 |
| 12 | Persistent SQLite Indexing | Bundled `rusqlite` WAL cache (`.ctxcut/index.db`) for sub-5ms repository queries | M5 | ORIGINAL_REQUEST §R5 |
| 13 | AST Query Engine (`ctxcut query`) | Structural Tree-sitter S-expression query search with built-in presets | M5 | ORIGINAL_REQUEST §R5 |
| 14 | Interactive TUI Dashboard | `ratatui` + `crossterm` slice preview studio and lifetime token ROI telemetry dashboard | M5 | ORIGINAL_REQUEST §R5 |
| 15 | Release & Self-Upgrade | `ctxcut upgrade`, GitHub Actions workflow, version 2.0.0 bump, installation scripts | M5 | ORIGINAL_REQUEST §R6 |
| 16 | Comprehensive E2E Testing & Hardening | Full 5-tier test suite pass (Tiers 1-4) and Tier 5 adversarial coverage hardening | M_FINAL | ORIGINAL_REQUEST §R6 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| E2E | E2E Testing Track | Requirement-driven opaque-box test suite (Tiers 1-4) & test infrastructure | none | DONE |
| M1 | Deep Graph, Impact & Call-Path Analysis | `callers`, `trace`, and Implementor Hoisting across Rust, Go, TS, Python | none | DONE |
| M2 | Multi-Language & SFC Grammar Expansion | C/C++, C#, Java/Kotlin, Vue/Svelte/Astro SFC adapters | none | DONE |
| M3 | ORM, Database & API Schema Stitching | Prisma, Drizzle, TypeORM, SQL migration DDLs, Proto, GraphQL schema stitchers | M2 | DONE |
| M4 | Verification Guard, Semantic Diff & AST Refactoring | `verify-patch`, `semantic-diff`, `refactor rename` | M1 | DONE |
| M5 | Persistent SQLite Index, AST Query Engine, TUI & Release | `.ctxcut/index.db`, `ctxcut query`, Ratatui TUI, `ctxcut upgrade`, v2.0.0 | M1, M2, M3, M4 | IN_PROGRESS |

| M_FINAL | Final Milestone: E2E Test Pass & Adversarial Hardening | Pass 100% E2E test suite (Phase 1) + Tier 5 Adversarial Hardening (Phase 2), build/install & commit/push | M1, M2, M3, M4, M5, E2E | PLANNED |


## Interface Contracts

### M1 ↔ Core / Slicing
- `ImpactSliceResult`: `pub target_symbol: String`, `pub callers: Vec<ImpactCallerItem>`, `pub total_callers: usize`, `pub stats: TokenStats`
- `TraceResult`: `pub entry_point: String`, `pub steps: Vec<TraceStep>`, `pub total_steps: usize`, `pub stats: TokenStats`
- `ExtractedImplementor`: `pub interface_name: String`, `pub implementor_name: String`, `pub kind: String`, `pub file_path: String`, `pub definition: String`
- `ContextSlicer::slice_symbol` and `slice_batch` include `hoisted_implementors: Vec<ExtractedImplementor>` in `SliceResult`.

### M2 ↔ Slicing & Core
- `SupportedLanguage`: `TypeScript`, `JavaScript`, `Python`, `Go`, `Rust`, `C`, `Cpp`, `CSharp`, `Java`, `Kotlin`, `Vue`, `Svelte`, `Astro`.
- `LanguageAdapter::find_implementors(...) -> Result<Vec<ExtractedImplementor>>`
- SFC Segmenter: splits SFC into script block + collapsed template/style summaries.

### M3 ↔ Slicing & Core
- `SchemaStitcher::stitch_schemas(workspace_root: &Path, ast_root: Node, source: &str, calls: &[CallSignatureStub]) -> Result<Vec<ExtractedType>>`
- Injected automatically into `ContextSlicer` pipeline.

### M4 ↔ CLI & MCP
- `PatchVerifier::verify_patch(workspace_root: &Path, target: &str, new_code: &str, typechecker: Option<&str>, dry_run: bool) -> Result<VerifyPatchResult>`
- `SemanticDiffEngine::compute_diff(workspace_root: &Path, staged: bool, file_path: Option<&Path>, budget: Option<usize>) -> Result<SemanticDiffResult>`
- `SymbolRenamer::rename_symbol(workspace_root: &Path, target: &str, new_name: &str, dry_run: bool) -> Result<MultiFileRenameResult>`

### M5 ↔ Core / CLI / Release
- `IndexEngine::open_or_create(workspace_root: &Path) -> Result<IndexEngine>`
- `AstQueryEngine::query_pattern(workspace_root: &Path, pattern: &str, lang: Option<SupportedLanguage>, preset: Option<&str>) -> Result<Vec<MatchCapture>>`

## Code Layout
```
crates/
├── ctxcut_core/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── model.rs
│   │   ├── parser/
│   │   ├── lang/
│   │   │   ├── mod.rs
│   │   │   ├── rust_lang.rs, go.rs, python.rs, typescript.rs
│   │   │   ├── c_cpp.rs, csharp.rs, java_lang.rs, kotlin_lang.rs
│   │   │   └── sfc/
│   │   ├── resolver/
│   │   │   ├── symbol.rs, imports.rs, types.rs, calls.rs
│   │   │   ├── callers.rs, trace.rs, implementors.rs
│   │   ├── schema/
│   │   │   ├── mod.rs, prisma.rs, drizzle.rs, typeorm.rs, sql_migrations.rs, proto.rs, graphql.rs
│   │   ├── verify/
│   │   │   ├── mod.rs, typechecker.rs, rollback.rs
│   │   ├── diff/
│   │   │   ├── mod.rs, semantic.rs
│   │   ├── refactor/
│   │   │   ├── mod.rs, rename.rs
│   │   ├── index/
│   │   │   ├── mod.rs, sqlite.rs, query.rs
│   │   ├── query/
│   │   │   ├── mod.rs, presets.rs
│   │   ├── slice/
│   │   ├── overview/
│   │   ├── patch/
│   │   ├── formatter/
│   │   └── telemetry/
├── ctxcut_cli/
│   ├── src/
│   │   ├── lib.rs, main.rs
│   │   ├── callers.rs, trace.rs, verify.rs, semantic_diff.rs, refactor.rs, query.rs, index.rs, upgrade.rs
│   │   └── tui/
│   │       ├── mod.rs, app.rs, views/, widgets/
└── ctxcut_mcp/
    └── src/
        └── lib.rs
```
