//! Core data models, extracted AST symbols, slicing configuration, and results.

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
    /// Detect programming language from a file path.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        Self::from_extension(&ext)
    }

    /// Detect programming language from a file extension string.
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

    /// Returns the Markdown code fence syntax identifier.
    pub fn markdown_fence(&self) -> &'static str {
        match self {
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Rust => "rust",
        }
    }

    /// Returns true if the language is part of the TypeScript/JavaScript family.
    pub fn is_typescript_family(&self) -> bool {
        matches!(self, Self::TypeScript | Self::JavaScript)
    }
}

/// Configuration options controlling AST slicing depth and extraction behaviors.
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

/// Extracted target AST symbol (e.g. function, method, class, interface, type).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedSymbol {
    /// Identifier name of the symbol (e.g., `login`, `AuthService.validate`).
    pub name: String,
    /// Symbol category: `"function"`, `"method"`, `"class"`, `"interface"`, `"type"`, `"enum"`.
    pub kind: String,
    /// Path to the source file where the symbol resides.
    pub file_path: String,
    /// 1-based start line in source code.
    pub start_line: usize,
    /// 1-based end line in source code.
    pub end_line: usize,
    /// Extracted documentation comments or JSDoc if present.
    pub doc_comment: Option<String>,
    /// Header signature (e.g. `export async function login(dto: LoginDto): Promise<Token>`).
    pub signature: String,
    /// Complete verbatim source text / implementation body of the symbol.
    pub body: String,
    /// Source language identifier (e.g. `"typescript"`, `"javascript"`).
    pub language: String,
}

/// Referenced type definition hoisted from the local file or imported modules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedType {
    /// Identifier name of the hoisted type (e.g., `UserDto`, `PaymentStatus`).
    pub name: String,
    /// Kind: `"interface"`, `"type_alias"`, `"enum"`, `"class"`, `"struct"`.
    pub kind: String,
    /// Source file path where this type is declared.
    pub file_path: String,
    /// Verbatim declaration text.
    pub definition: String,
}

/// External called function or method with its implementation body stripped (0% body, 100% signature).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSignatureStub {
    /// Function or method identifier name.
    pub name: String,
    /// Receiver expression or object name (e.g., `stripe.charges`, `db.user`).
    pub receiver: Option<String>,
    /// Source file path where the stub was located, if resolved.
    pub file_path: Option<String>,
    /// Body-stripped signature declaration stub.
    pub signature: String,
}

/// Token count and line statistics comparing raw full source file against sliced output.
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
    pub fn calculate(
        raw_tokens: usize,
        sliced_tokens: usize,
        raw_lines: usize,
        sliced_lines: usize,
    ) -> Self {
        let savings_percentage = if raw_tokens == 0 {
            0.0
        } else if sliced_tokens >= raw_tokens {
            0.0
        } else {
            let pct = ((raw_tokens - sliced_tokens) as f64 / raw_tokens as f64) * 100.0;
            (pct * 100.0).round() / 100.0
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

/// Complete AST context slice result containing target symbol, hoisted types, stubs, and stats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceResult {
    /// Full implementation and signature of the target symbol.
    pub target_symbol: ExtractedSymbol,
    /// Inlined/hoisted types referenced by the symbol.
    pub hoisted_types: Vec<ExtractedType>,
    /// Body-stripped signature stubs of external called functions/methods.
    pub stripped_calls: Vec<CallSignatureStub>,
    /// Token reduction and line metrics.
    pub stats: TokenStats,
}

impl SliceResult {
    /// Formats the slice result as prompt-optimized Markdown.
    pub fn to_markdown(&self) -> String {
        crate::formatter::MarkdownFormatter::format(self)
    }

    /// Formats the slice result as pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the slice result as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}
