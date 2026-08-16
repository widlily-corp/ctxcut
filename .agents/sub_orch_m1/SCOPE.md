# Scope: Milestone 1 — Workspace Foundation & Core AST Engine (TS/JS)

## Architecture
- Workspace root configuration: `Cargo.toml`, `clippy.toml`, `rustfmt.toml`.
- Library crate: `crates/ctxcut_core` containing:
  - `src/lib.rs`: Public API (`ContextSlicer`, `SliceOptions`, `SliceResult`, `SupportedLanguage`, `ExtractedSymbol`, `ExtractedType`, `CallSignatureStub`, `TokenStats`).
  - `src/error.rs`: `CoreError` with `thiserror`.
  - `src/model.rs`: Core data structures and AST node location models.
  - `src/lang/mod.rs` & `src/lang/typescript.rs`: LanguageAdapter trait and TypeScript / TSX / JavaScript grammar bindings.
  - `src/parser/mod.rs`: Tree-sitter parser manager, caching, and tree traversal helpers.
  - `src/resolver/mod.rs`, `symbol.rs`, `imports.rs`, `types.rs`, `calls.rs`: Symbol locator, relative/named import resolver, type reference extraction and hoisting, external call identification and body stripper.
  - `src/slice/mod.rs`: Slicing orchestration pipeline executing locator -> hoister -> stripper -> tokenizer -> formatter.
  - `src/formatter/mod.rs`: Prompt-optimized Markdown and structured JSON formatting.
  - `src/tokenizer/mod.rs`: BPE token counting with `tiktoken-rs` (cl100k_base), raw vs sliced token counts, lines, and savings percentages.

## Feature Inventory (Milestone 1)
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Workspace Setup & Cargo Configuration | Multi-crate workspace manifest, dependency inheritance, strict clippy policies | M1 | Survey |
| 2 | Tree-Sitter AST Core & TS/JS Grammar | AST parsing engine for TypeScript/JavaScript (.ts, .tsx, .js, .jsx) | M1 | R1, §3.1 |
| 3 | Symbol Locator (TS/JS) | Locate target function, method, class, or type by name or range | M1 | R1, §2.1 |
| 4 | Type Hoister (TS/JS) | Extract and inline referenced interfaces, type aliases, enums, DTOs | M1 | R2, §2.1 |
| 5 | Signature Stripper (TS/JS) | Strip 100% of bodies from called external functions, retaining signatures | M1 | R2, §2.1 |
| 6 | Markdown & JSON Formatter | Render prompt-optimized Markdown slices with metrics and JSON output | M1 | R2, §2.1 |
| 7 | BPE Token Counter | Calculate exact OpenAI BPE token savings percentage using tiktoken-rs | M1 | R2, §2.1 |

## Interface Contracts

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SupportedLanguage {
    TypeScript,
    JavaScript,
    Python,
    Go,
    Rust,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SliceOptions {
    pub depth: usize,              // Type hoisting traversal depth (default 1)
    pub include_types: bool,       // Include type hoisting (default true)
    pub include_calls: bool,       // Include signature stripping (default true)
}

impl Default for SliceOptions {
    fn default() -> Self {
        Self {
            depth: 1,
            include_types: true,
            include_calls: true,
        }
    }
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

## Verification Criteria
- `cargo check --workspace` passes.
- `cargo clippy --all-targets -- -D warnings` gives 0 warnings.
- `cargo test -p ctxcut_core` passes 100% of unit and integration tests.
- Formatter produces clean, prompt-optimized markdown and valid JSON.
- BPE token counter produces exact counts via `tiktoken-rs`.
