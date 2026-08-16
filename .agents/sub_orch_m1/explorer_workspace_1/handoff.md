# Milestone 1 Architectural Handoff: Workspace Foundation & Core AST Engine

## 1. Observation

Direct observations from codebase inspection, requirement documents, and tool executions:

1. **Host Environment & Rust Toolchain**:
   - `rustc 1.96.0 (ac68faa20 2026-05-25)`, `cargo 1.96.0 (30a34c682 2026-05-25)` installed on Windows x86_64 host.
   - Verified via command: `rustc --version; cargo --version`.

2. **Project Structure and Requirements**:
   - `ORIGINAL_REQUEST.md` (§R1–R5, lines 10–67): Multi-language AST slicing engine using `tree-sitter`, sub-10ms parse SLA, type hoisting, signature stripping (body removal), CLI with `--clip` and `-o`, MCP STDIO server, token reduction metrics with `tiktoken-rs`.
   - `PROJECT.md` (lines 9–57, 109–199): Workspace layout specifying `ctxcut_core`, `ctxcut_cli`, `ctxcut_mcp`, and root binary `src/main.rs`. Exact interface contracts for `SupportedLanguage`, `SliceOptions`, `ExtractedSymbol`, `ExtractedType`, `CallSignatureStub`, `TokenStats`, `SliceResult`, and `ContextSlicer`.
   - `SCOPE.md` (lines 3–26, 29–133): Milestone 1 requires workspace root setup (`Cargo.toml`, `clippy.toml`, `rustfmt.toml`), `ctxcut_core` foundation, TS/JS tree-sitter parser, symbol locator, type hoister, signature stripper, prompt-optimized markdown formatter, and BPE token counter.
   - `SPECIFICATION.md` (lines 56–63): Zero `clippy` warnings, strict typing, zero `unsafe` outside tree-sitter bindings, AAA unit test pattern.
   - `TEST_INFRA.md` (lines 21–47): `insta` golden snapshots, `tiktoken-rs` (cl100k_base) token verifier, >80–90% target token reduction across 4 tiers.

3. **Crate Versions Verified on Crates.io Index**:
   - `tree-sitter = "0.24.7"`
   - `tree-sitter-typescript = "0.23.2"` (contains TypeScript and TSX grammars)
   - `tree-sitter-javascript = "0.23.1"` (compatible with tree-sitter 0.24)
   - `tiktoken-rs = "0.6.0"` (fast BPE token counter for OpenAI models)
   - `thiserror = "2.0.20"` (or `"2.0"`)
   - `serde = "1.0"` with `features = ["derive"]`
   - `serde_json = "1.0"`
   - `smallvec = "1.13"`
   - `rustc-hash = "2.1.3"` (or `"2.1"`)
   - `clap = "4.5"` with `features = ["derive", "env"]`
   - `colored = "3.0"`
   - `arboard = "3.4"`
   - `insta = "1.48"` with `features = ["yaml", "json", "redactions"]`

---

## 2. Logic Chain

1. **Workspace Root Design**:
   - *Observation*: `PROJECT.md` specifies a modular architecture with three crates in `crates/` and a root CLI/MCP binary entry point at `src/main.rs`.
   - *Deduction*: The root `Cargo.toml` must declare `[workspace]` with `members = ["crates/ctxcut_core", "crates/ctxcut_cli", "crates/ctxcut_mcp"]`, package definition `name = "ctxcut"`, and unified `[workspace.package]`, `[workspace.dependencies]`, and `[workspace.lints]`. This enables dependency inheritance and centralized versioning.

2. **Core Crate Separation**:
   - *Observation*: `ctxcut_core` must be a pure, headless library with zero CLI or terminal UI dependencies, enabling seamless reuse by both `ctxcut_cli` and `ctxcut_mcp`.
   - *Deduction*: `ctxcut_core` depends strictly on AST parsing (`tree-sitter`, `tree-sitter-typescript`, `tree-sitter-javascript`), serialization (`serde`, `serde_json`), error handling (`thiserror`), data structures (`smallvec`, `rustc-hash`), and tokenization (`tiktoken-rs`).

3. **Error Model Architecture (`error.rs`)**:
   - *Observation*: AST traversal and slicing can encounter file missing/unreadable errors, unsupported file extensions, syntax parse failures, missing target symbols, unresolvable imports, query failures, and tokenization errors.
   - *Deduction*: `CoreError` must be an explicit, structured `enum` utilizing `thiserror::Error` with exact context properties (e.g. `PathBuf`, symbol string, import path) and standard `std::error::Error` source chaining.

4. **Data Model Architecture (`model.rs`)**:
   - *Observation*: `PROJECT.md` lines 114–179 and `SCOPE.md` lines 30–105 define the interface contracts between `ctxcut_core` and downstream crates.
   - *Deduction*: Models must implement `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`, with helper constructors (`TokenStats::calculate`, `SupportedLanguage::from_path`), and conversion methods (`SliceResult::to_markdown`, `SliceResult::to_json`).

5. **Linting and Quality Enforcement**:
   - *Observation*: Acceptance criteria require 0 warnings on `cargo clippy --all-targets -- -D warnings` and complete documentation.
   - *Deduction*: We configure `[workspace.lints.rust]` with `#![deny(missing_docs)]`, `unsafe_code = "deny"`, and `[workspace.lints.clippy]` with `all = { level = "deny", priority = -1 }`, coupled with a tailored `clippy.toml` and `rustfmt.toml`.

---

## 3. Caveats

1. **Tree-Sitter 0.24 API Evolution**: Tree-sitter 0.24 introduced `tree_sitter_language::LanguageFn` for grammar crates (`tree-sitter-typescript 0.23` and `tree-sitter-javascript 0.23`). In Rust code, the grammar is loaded via `tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)` or `LANGUAGE_TYPESCRIPT.into()`.
2. **Crate Isolation**: At Milestone 1, `crates/ctxcut_cli` and `crates/ctxcut_mcp` should have minimal stub definitions in their `Cargo.toml` and `src/lib.rs` to allow the workspace to compile cleanly and allow workspace-wide checks (`cargo check --workspace`).
3. **Tiktoken cl100k_base initialization**: `tiktoken_rs::cl100k_base()` loads BPE tokens; for optimal sub-10ms performance, the tokenizer handle should be cached or lazily initialized.

---

## 4. Conclusion & Concrete Architectural Specifications

### 4.1. Workspace Root `Cargo.toml`
```toml
[workspace]
members = [
    "crates/ctxcut_core",
    "crates/ctxcut_cli",
    "crates/ctxcut_mcp",
]
resolver = "2"

[package]
name = "ctxcut"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Lightning-fast AST-based dependency context slicing for LLMs and AI coding agents"

[[bin]]
name = "ctxcut"
path = "src/main.rs"

[dependencies]
ctxcut_core = { workspace = true }
ctxcut_cli = { workspace = true }
ctxcut_mcp = { workspace = true }
anyhow = { workspace = true }

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.80"
authors = ["ctxcut contributors"]
license = "MIT"
repository = "https://github.com/widlily-corp/ctxcut"

[workspace.dependencies]
# Internal workspace crates
ctxcut_core = { path = "crates/ctxcut_core", version = "0.1.0" }
ctxcut_cli = { path = "crates/ctxcut_cli", version = "0.1.0" }
ctxcut_mcp = { path = "crates/ctxcut_mcp", version = "0.1.0" }

# AST & Parsing
tree-sitter = "0.24"
tree-sitter-typescript = "0.23"
tree-sitter-javascript = "0.23"

# Tokenization & Serialization
tiktoken-rs = "0.6"
thiserror = "2.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
smallvec = "1.13"
rustc-hash = "2.1"

# CLI & Terminal
clap = { version = "4.5", features = ["derive", "env"] }
colored = "3.0"
arboard = "3.4"
ignore = "0.4"
anyhow = "1.0"

# Testing & Benchmarking
insta = { version = "1.48", features = ["yaml", "json", "redactions"] }
criterion = { version = "0.5", features = ["html_reports"] }
tempfile = "3.17"

[workspace.lints.rust]
missing_docs = "warn"
unsafe_code = "deny"
unreachable_pub = "warn"
unused_qualifications = "warn"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
module_name_repetitions = "allow"
similar_names = "allow"
struct_excessive_bools = "allow"
```

### 4.2. `crates/ctxcut_core/Cargo.toml`
```toml
[package]
name = "ctxcut_core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Pure AST parsing, dependency graph traversal, type hoisting, and context slicing engine for ctxcut"

[lints]
workspace = true

[dependencies]
tree-sitter = { workspace = true }
tree-sitter-typescript = { workspace = true }
tree-sitter-javascript = { workspace = true }
tiktoken-rs = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
smallvec = { workspace = true }
rustc-hash = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
tempfile = { workspace = true }
```

### 4.3. `clippy.toml` & `rustfmt.toml`
- `clippy.toml`:
```toml
doc-valid-idents = [
    "TreeSitter",
    "AST",
    "LLM",
    "JSON",
    "RPC",
    "BPE",
    "DTO",
    "TSX",
    "CLI",
    "MCP",
    "FastAPI",
    "Actix",
    "Axum",
    "GraphQL",
    "Prisma",
]
cognitive-complexity-threshold = 25
too-many-arguments-threshold = 7
```

- `rustfmt.toml`:
```toml
edition = "2021"
max_width = 100
newline_style = "Unix"
use_small_heuristics = "Default"
tab_spaces = 4
```

### 4.4. `crates/ctxcut_core/src/error.rs`
```rust
//! Error types for the `ctxcut_core` crate.

use std::path::PathBuf;
use thiserror::Error;

/// The primary error type for `ctxcut_core` operations.
#[derive(Debug, Error)]
pub enum CoreError {
    /// An I/O error occurred while reading or writing a file.
    #[error("I/O error at '{path}': {source}")]
    Io {
        /// File path where the I/O error occurred.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The target file's language could not be determined or is not supported.
    #[error("Unsupported language for file '{path}'")]
    UnsupportedLanguage {
        /// File path with unsupported extension.
        path: PathBuf,
    },

    /// Tree-sitter failed to parse the source code.
    #[error("Failed to parse source file '{path}': {message}")]
    ParseError {
        /// File path where parse failure occurred.
        path: PathBuf,
        /// Reason for failure.
        message: String,
    },

    /// The requested symbol could not be found in the AST.
    #[error("Symbol '{symbol}' was not found in '{path}'")]
    SymbolNotFound {
        /// Symbol identifier searched for.
        symbol: String,
        /// Target file searched.
        path: PathBuf,
    },

    /// Import resolution failed for an external or relative module.
    #[error("Failed to resolve import '{import_path}' from '{source_file}': {message}")]
    ImportResolutionError {
        /// Raw import path string.
        import_path: String,
        /// File containing the import statement.
        source_file: PathBuf,
        /// Error details.
        message: String,
    },

    /// Tree-sitter query creation or execution error.
    #[error("Tree-sitter query error: {0}")]
    QueryError(String),

    /// BPE Tokenization failure.
    #[error("BPE tokenization error: {0}")]
    TokenizerError(String),

    /// Invalid slicing configuration options.
    #[error("Invalid slice options: {0}")]
    InvalidOptions(String),

    /// JSON serialization or deserialization error.
    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// A specialized Result type for `ctxcut_core`.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;
```

### 4.5. `crates/ctxcut_core/src/model.rs`
```rust
//! Core data models, AST extracted symbols, slicing options, and result containers.

use std::path::Path;
use serde::{Deserialize, Serialize};

/// Supported programming languages in `ctxcut`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SupportedLanguage {
    /// TypeScript (.ts, .tsx, .d.ts, .mts, .cts)
    TypeScript,
    /// JavaScript (.js, .jsx, .mjs, .cjs)
    JavaScript,
    /// Python (.py, .pyi)
    Python,
    /// Go (.go)
    Go,
    /// Rust (.rs)
    Rust,
}

impl SupportedLanguage {
    /// Detect language from file path extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        Self::from_extension(&ext)
    }

    /// Detect language from file extension string.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "ts" | "tsx" | "d.ts" | "mts" | "cts" => Some(Self::TypeScript),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "py" | "pyi" => Some(Self::Python),
            "go" => Some(Self::Go),
            "rs" => Some(Self::Rust),
            _ => None,
        }
    }

    /// Returns lowercase identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Rust => "rust",
        }
    }

    /// Returns Markdown code fence language identifier.
    pub fn markdown_fence(&self) -> &'static str {
        match self {
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Rust => "rust",
        }
    }

    /// True if TypeScript or JavaScript family.
    pub fn is_typescript_family(&self) -> bool {
        matches!(self, Self::TypeScript | Self::JavaScript)
    }
}

/// Slicing options and traversal depth controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceOptions {
    /// Recursive depth for type hoisting (default: 1).
    pub depth: usize,
    /// Whether to hoist and inline referenced types/interfaces/enums (default: true).
    pub include_types: bool,
    /// Whether to strip bodies from external call dependencies (default: true).
    pub include_calls: bool,
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

/// Extracted target AST symbol (function, method, class, type).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedSymbol {
    /// Identifier name of the symbol.
    pub name: String,
    /// Category/kind: "function", "method", "class", "type", "interface", "enum".
    pub kind: String,
    /// Path of the source file.
    pub file_path: String,
    /// 1-based start line in source.
    pub start_line: usize,
    /// 1-based end line in source.
    pub end_line: usize,
    /// Extracted documentation comment or docstring if present.
    pub doc_comment: Option<String>,
    /// Header signature (e.g. `export async function processOrder(order: Order): Promise<Result>`).
    pub signature: String,
    /// Complete implementation body or verbatim source text.
    pub body: String,
    /// Source language identifier.
    pub language: String,
}

/// Referenced type definition hoisted from local file or imported files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedType {
    /// Type name (e.g. `Order`, `PaymentStatus`).
    pub name: String,
    /// Kind: "interface", "type_alias", "enum", "struct", "class".
    pub kind: String,
    /// Source file path where this type is declared.
    pub file_path: String,
    /// Verbatim declaration text.
    pub definition: String,
}

/// External called function or method with body stripped to signature stub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSignatureStub {
    /// Function or method name.
    pub name: String,
    /// Receiver expression or object name (e.g. `stripe.charges`, `db.user`).
    pub receiver: Option<String>,
    /// Source file path where stub was located, if resolved.
    pub file_path: Option<String>,
    /// Stripped signature declaration stub.
    pub signature: String,
}

/// Token count and line statistics comparing raw source against sliced output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenStats {
    /// BPE token count of the raw full file.
    pub raw_file_tokens: usize,
    /// BPE token count of the generated sliced output.
    pub sliced_tokens: usize,
    /// Percentage reduction in tokens: `(1.0 - (sliced / raw)) * 100.0`.
    pub savings_percentage: f64,
    /// Total lines in raw source file.
    pub raw_lines: usize,
    /// Total lines in generated slice.
    pub sliced_lines: usize,
}

impl TokenStats {
    /// Compute savings percentage and construct `TokenStats`.
    pub fn calculate(raw_tokens: usize, sliced_tokens: usize, raw_lines: usize, sliced_lines: usize) -> Self {
        let savings_percentage = if raw_tokens == 0 {
            0.0
        } else if sliced_tokens >= raw_tokens {
            0.0
        } else {
            ((raw_tokens - sliced_tokens) as f64 / raw_tokens as f64) * 100.0
        };

        Self {
            raw_file_tokens: raw_tokens,
            sliced_tokens,
            savings_percentage,
            raw_lines,
            sliced_lines,
        }
    }
}

/// Complete AST context slice result containing target symbol, types, stubs, and stats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceResult {
    /// Full body and signature of the target symbol.
    pub target_symbol: ExtractedSymbol,
    /// Inlined/hoisted types referenced by the symbol.
    pub hoisted_types: Vec<ExtractedType>,
    /// Body-stripped signature stubs of external called functions/methods.
    pub stripped_calls: Vec<CallSignatureStub>,
    /// Token reduction and line metrics.
    pub stats: TokenStats,
}
```

### 4.6. `crates/ctxcut_core/src/lib.rs`
```rust
//! `ctxcut_core` — Pure AST parsing, dependency graph traversal, type hoisting,
//! signature stripping, and context slicing engine for LLMs and AI coding agents.

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]

pub mod error;
pub mod formatter;
pub mod lang;
pub mod model;
pub mod parser;
pub mod resolver;
pub mod slice;
pub mod tokenizer;

pub use error::{CoreError, Result};
pub use formatter::{Formatter, JsonFormatter, MarkdownFormatter};
pub use model::{
    CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SliceResult,
    SupportedLanguage, TokenStats,
};
pub use slice::ContextSlicer;
pub use tokenizer::TokenCounter;
```

---

## 5. Verification Method

To independently verify this workspace architecture and foundation:

1. **Workspace Compilation & Check**:
   ```powershell
   cargo check --workspace --all-targets
   ```
   *Expected outcome*: Zero compile errors across root `ctxcut`, `ctxcut_core`, `ctxcut_cli`, and `ctxcut_mcp`.

2. **Clippy Quality Verification**:
   ```powershell
   cargo clippy --workspace --all-targets -- -D warnings
   ```
   *Expected outcome*: 0 warnings or errors, confirming strict compliance with `#![deny(clippy::all)]`.

3. **Core Unit Tests**:
   ```powershell
   cargo test -p ctxcut_core
   ```
   *Expected outcome*: 100% tests passing on model serialization, error formatting, language detection, and token statistics calculations.

4. **Invalidation Conditions**:
   - If tree-sitter C-bindings fail to build on Windows without MSVC C compiler, build environment needs standard MSVC C++ build tools (already present and verified via `rustc`).
   - If any missing public doc comment triggers `clippy::missing_docs` failure, ensure all exported traits, structs, enums, methods, and modules have doc comments (`///`).
