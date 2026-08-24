//! Core data models, extracted AST symbols, slicing configuration, and results.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    /// C (.c, .h)
    C,
    /// C++ (.cpp, .cc, .cxx, .hpp, .hh, .hxx)
    Cpp,
    /// C# (.cs)
    CSharp,
    /// Java (.java)
    Java,
    /// Kotlin (.kt, .kts)
    Kotlin,
    /// Vue Single File Component (.vue)
    Vue,
    /// Svelte Single File Component (.svelte)
    Svelte,
    /// Astro Single File Component (.astro)
    Astro,
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
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some(Self::Cpp),
            "cs" => Some(Self::CSharp),
            "java" => Some(Self::Java),
            "kt" | "kts" => Some(Self::Kotlin),
            "vue" => Some(Self::Vue),
            "svelte" => Some(Self::Svelte),
            "astro" => Some(Self::Astro),
            _ => None,
        }
    }

    /// Parses language from loosely formatted string (e.g. "rust", "rs", "c++", "ts", "golang").
    pub fn from_str_loose(s: &str) -> Option<Self> {
        let clean = s.trim().to_lowercase();
        match clean.as_str() {
            "ts" | "typescript" => Some(Self::TypeScript),
            "js" | "javascript" => Some(Self::JavaScript),
            "py" | "python" => Some(Self::Python),
            "go" | "golang" => Some(Self::Go),
            "rs" | "rust" => Some(Self::Rust),
            "c" => Some(Self::C),
            "cpp" | "c++" | "cxx" | "cc" => Some(Self::Cpp),
            "cs" | "c#" | "csharp" => Some(Self::CSharp),
            "java" => Some(Self::Java),
            "kt" | "kotlin" => Some(Self::Kotlin),
            "vue" => Some(Self::Vue),
            "svelte" => Some(Self::Svelte),
            "astro" => Some(Self::Astro),
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
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Vue => "vue",
            Self::Svelte => "svelte",
            Self::Astro => "astro",
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
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Vue => "vue",
            Self::Svelte => "svelte",
            Self::Astro => "astro",
        }
    }

    /// Returns true if the language is part of the TypeScript/JavaScript family or script SFC.
    pub fn is_typescript_family(&self) -> bool {
        matches!(
            self,
            Self::TypeScript | Self::JavaScript | Self::Vue | Self::Svelte | Self::Astro
        )
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
    /// Adaptive token budget limit for progressive semantic degradation (optional).
    pub budget: Option<usize>,
}

impl Default for SliceOptions {
    fn default() -> Self {
        Self {
            depth: 1,
            include_types: true,
            include_calls: true,
            budget: None,
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
        #[allow(clippy::cast_precision_loss)]
        let savings_percentage = if raw_tokens == 0 || sliced_tokens >= raw_tokens {
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
    /// Inlined concrete implementors for referenced traits/interfaces.
    #[serde(default)]
    pub hoisted_implementors: Vec<ExtractedImplementor>,
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

/// Result of an AST-guided patch operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchResult {
    /// Target file path.
    pub file_path: PathBuf,
    /// Target symbol name matched by query.
    pub symbol_name: String,
    /// Original code before patch.
    pub original_code: String,
    /// Normalized replacement code spliced into source.
    pub patched_code: String,
    /// Byte range `(start_byte, end_byte)` replaced in source.
    pub byte_range: (usize, usize),
    /// Unified diff representation of the change.
    pub diff: String,
    /// Whether changes were written to disk (`!dry_run`).
    pub applied: bool,
}

/// Detailed location and diagnostic for a syntax error found during AST validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxErrorDetail {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
    /// 0-based byte offset in the source.
    pub byte_offset: usize,
    /// Tree-sitter node kind (e.g., `"ERROR"` or `"MISSING ;"`).
    pub kind: String,
    /// Snippet of erroneous source code around the error.
    pub snippet: String,
    /// Whether the error represents a missing token.
    pub is_missing: bool,
}

/// Discovered test fixture or existing reference test pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredFixture {
    /// File path where the reference test was found.
    pub file_path: String,
    /// Extracted test function signature or snippet.
    pub snippet: String,
}

/// Sliced target symbol accompanied by mock scaffolding and unit test harness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestContextResult {
    /// Complete AST context slice of the target symbol and its dependencies.
    pub slice: SliceResult,
    /// Detected or requested test runner / framework (e.g. `"cargo"`, `"pytest"`, `"vitest"`, `"jest"`, `"gotest"`).
    pub test_framework: String,
    /// Generated mock and spy declarations for all stripped calls.
    pub mock_scaffolding: String,
    /// Scaffolding template for writing tests against the target symbol.
    pub test_template: String,
    /// Nearby reference test fixtures discovered in the repository.
    pub reference_fixtures: Vec<DiscoveredFixture>,
}

impl TestContextResult {
    /// Formats the test context result as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Test Context: `{}` ({})\n\n",
            self.slice.target_symbol.name, self.test_framework
        ));
        out.push_str("## 1. Target Symbol & Context Slice\n\n");
        out.push_str(&self.slice.to_markdown());
        out.push_str("\n\n");

        if !self.mock_scaffolding.trim().is_empty() {
            out.push_str("## 2. Generated Mock Scaffolding\n\n```");
            out.push_str(&self.slice.target_symbol.language);
            out.push('\n');
            out.push_str(self.mock_scaffolding.trim());
            out.push_str("\n```\n\n");
        }

        if !self.test_template.trim().is_empty() {
            out.push_str("## 3. Unit Test Template\n\n```");
            out.push_str(&self.slice.target_symbol.language);
            out.push('\n');
            out.push_str(self.test_template.trim());
            out.push_str("\n```\n\n");
        }

        if !self.reference_fixtures.is_empty() {
            out.push_str("## 4. Reference Fixtures Discovered\n\n");
            for fixture in &self.reference_fixtures {
                out.push_str(&format!(
                    "### `{}`\n```{}\n{}\n```\n\n",
                    fixture.file_path,
                    self.slice.target_symbol.language,
                    fixture.snippet.trim()
                ));
            }
        }

        out
    }

    /// Formats the test context result as pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Result of multi-symbol slicing with globally deduplicated hoisted types and call stubs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchSliceResult {
    /// Path to the source file where target symbols reside.
    pub file_path: String,
    /// Target symbols extracted with full implementations.
    pub target_symbols: Vec<ExtractedSymbol>,
    /// Hoisted types/data contracts globally deduplicated across all target symbols.
    pub hoisted_types: Vec<ExtractedType>,
    /// Inlined concrete implementors for referenced traits/interfaces.
    #[serde(default)]
    pub hoisted_implementors: Vec<ExtractedImplementor>,
    /// External call stubs globally deduplicated across all target symbols.
    pub stripped_calls: Vec<CallSignatureStub>,
    /// Aggregate token savings statistics.
    pub stats: TokenStats,
}

impl BatchSliceResult {
    /// Render unified batch result to prompt-optimized Markdown.
    pub fn to_markdown(&self) -> String {
        crate::formatter::MarkdownFormatter::format_unified_batch(self)
    }

    /// Render unified batch result to structured JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Render unified batch result to compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Summary of an individual AST symbol in the workspace overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolOverviewItem {
    /// Identifier name (e.g. `OrderService`, `processOrder`, `GET /api/v1/orders`).
    pub name: String,
    /// Kind: `"function"`, `"method"`, `"class"`, `"interface"`, `"struct"`, `"enum"`, `"trait"`, `"type"`, `"route"`.
    pub kind: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// Header signature or definition stub.
    pub signature: Option<String>,
    /// Doc comment summary or JSDoc.
    pub doc_summary: Option<String>,
}

/// Overview of symbols extracted from a single source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOverviewItem {
    /// Relative or absolute path to the file.
    pub path: String,
    /// Programming language identifier.
    pub language: String,
    /// Total lines in source file.
    pub total_lines: usize,
    /// Raw BPE token count.
    pub total_tokens: usize,
    /// Extracted symbols in the file.
    pub symbols: Vec<SymbolOverviewItem>,
}

/// Complete workspace symbol overview report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceOverviewReport {
    /// Root path of the workspace.
    pub root_path: String,
    /// Total files indexed.
    pub total_files: usize,
    /// Total lines across indexed files.
    pub total_lines: usize,
    /// Total raw tokens across indexed files.
    pub total_raw_tokens: usize,
    /// Total tokens in the generated overview document.
    pub total_overview_tokens: usize,
    /// Percentage token reduction: `(1.0 - (overview / raw)) * 100.0`.
    pub token_savings_percentage: f64,
    /// Total symbols indexed.
    pub total_symbols: usize,
    /// Language distribution statistics.
    pub language_breakdown: Vec<crate::traversal::LanguageStatItem>,
    /// Per-file symbol overviews.
    pub files: Vec<FileOverviewItem>,
}

impl WorkspaceOverviewReport {
    /// Formats the overview report into a high-density Markdown document.
    pub fn to_markdown(&self) -> String {
        crate::overview::format_overview_markdown(self)
    }

    /// Serializes report to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Serializes report to compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Options controlling workspace overview generation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OverviewOptions {
    /// Optional token budget limit to bound output size.
    pub budget: Option<usize>,
    /// Optional directory recursion depth limit.
    pub max_depth: Option<usize>,
    /// Whether to include framework web routes (default: true).
    pub include_routes: bool,
    /// Optional target framework filter (e.g. "express", "fastapi", "actix").
    pub framework: Option<String>,
}

/// Extracted concrete implementor of a trait, interface, or protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedImplementor {
    /// Name of the trait or interface being implemented.
    pub interface_name: String,
    /// Name of the concrete struct/class implementing the interface.
    pub implementor_name: String,
    /// Language-specific kind (e.g. `rust_impl`, `go_struct`, `ts_class`, `py_class`).
    pub kind: String,
    /// File path where the implementor is defined.
    pub file_path: String,
    /// Extracted signature/body stub of the concrete implementation.
    pub definition: String,
}

/// Discovered caller item in reverse impact analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactCallerItem {
    /// Enclosing caller symbol name (e.g. `OrderController.checkout`, `validate_order`).
    pub caller_symbol: String,
    /// Kind of caller (e.g. `function`, `method`, `controller`, `middleware`).
    pub caller_kind: String,
    /// Relative or absolute file path containing the invocation.
    pub file_path: String,
    /// 1-based line number of the call expression.
    pub line_number: usize,
    /// Exact call invocation snippet (e.g. `authService.validate(token)`).
    pub call_snippet: String,
    /// Signature of the enclosing caller function.
    pub caller_signature: Option<String>,
}

/// Complete upstream caller and reverse impact analysis slice result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpactSliceResult {
    /// Target symbol query that was traced.
    pub target_symbol: String,
    /// File declaring the target symbol (if known or provided).
    pub target_file: Option<String>,
    /// Discovered upstream caller sites.
    pub callers: Vec<ImpactCallerItem>,
    /// Total number of unique callers found.
    pub total_callers: usize,
    /// Token and line reduction metrics.
    pub stats: TokenStats,
}

impl ImpactSliceResult {
    /// Formats the impact slice result as Markdown.
    pub fn to_markdown(&self) -> String {
        crate::formatter::MarkdownFormatter::format_impact(self)
    }

    /// Formats the impact slice result as pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the impact slice result as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Step within an end-to-end execution flow trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceStep {
    /// 1-based sequence index along the invocation spine.
    pub step_number: usize,
    /// Symbol name at this execution step.
    pub symbol_name: String,
    /// Architectural role / kind (e.g. `entry_point`, `controller`, `service`, `database_sink`).
    pub kind: String,
    /// Source file containing this step.
    pub file_path: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// Language tag (e.g. `typescript`, `python`, `go`, `rust`).
    pub language: String,
    /// Function/method signature.
    pub signature: String,
    /// Extracted or compressed code snippet.
    pub code_snippet: String,
    /// Detected outgoing calls from this function.
    pub outgoing_calls: Vec<String>,
    /// Next target in the invocation spine (if resolved).
    pub next_target: Option<String>,
}

/// End-to-end execution flow trace result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceResult {
    /// Entry point query (route, CLI entry, or function).
    pub entry_point: String,
    /// Entry file path.
    pub entry_file: String,
    /// Linear execution steps from entry to sinks.
    pub steps: Vec<TraceStep>,
    /// Total number of steps in the trace chain.
    pub total_steps: usize,
    /// Token and line reduction metrics.
    pub stats: TokenStats,
}

impl TraceResult {
    /// Formats the trace result as Markdown.
    pub fn to_markdown(&self) -> String {
        crate::formatter::MarkdownFormatter::format_trace(self)
    }

    /// Formats the trace result as pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the trace result as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Diagnostic item extracted from compiler or syntax validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyDiagnostic {
    /// Severity level ("error", "warning", "info").
    pub severity: String,
    /// 1-based line number.
    pub line: Option<usize>,
    /// 1-based column number.
    pub column: Option<usize>,
    /// Diagnostic description.
    pub message: String,
    /// Referenced file path.
    pub file: Option<String>,
    /// Diagnostic code (e.g. "TS2322", "E0308").
    pub code: Option<String>,
}

/// Result of an AST-guided patch verification guard operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyPatchResult {
    /// Target file path.
    pub file_path: PathBuf,
    /// Target symbol name.
    pub symbol_name: String,
    /// Whether the verification succeeded without errors.
    pub success: bool,
    /// Whether changes were persisted to disk (`!dry_run && success`).
    pub applied: bool,
    /// Whether dry-run mode was requested.
    pub dry_run: bool,
    /// Unified diff preview of the patch.
    pub diff: String,
    /// Typechecker command that was executed, if any.
    pub typechecker_command: Option<String>,
    /// Exit code from typechecker process.
    pub exit_code: Option<i32>,
    /// Raw standard output from typechecker.
    pub stdout: String,
    /// Raw standard error from typechecker.
    pub stderr: String,
    /// Extracted diagnostics/error messages.
    pub diagnostics: Vec<VerifyDiagnostic>,
    /// Syntax error details if Tree-Sitter validation failed.
    pub syntax_errors: Vec<SyntaxErrorDetail>,
    /// Verification duration in milliseconds.
    pub duration_ms: u64,
}

impl VerifyPatchResult {
    /// Formats the verification result as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        if self.success {
            out.push_str(&format!(
                "### ✔ Patch Verified Successfully (`{}` in `{}`)\n\n",
                self.symbol_name,
                self.file_path.display()
            ));
        } else {
            out.push_str(&format!(
                "### ✖ Patch Verification Failed (`{}` in `{}`)\n\n",
                self.symbol_name,
                self.file_path.display()
            ));
        }

        out.push_str(&format!("- **Applied to disk**: `{}`\n", self.applied));
        out.push_str(&format!("- **Dry run**: `{}`\n", self.dry_run));
        out.push_str(&format!("- **Duration**: `{}ms`\n", self.duration_ms));
        if let Some(cmd) = &self.typechecker_command {
            out.push_str(&format!("- **Typechecker command**: `{cmd}`\n"));
            if let Some(code) = self.exit_code {
                out.push_str(&format!("- **Exit code**: `{code}`\n"));
            }
        }
        out.push('\n');

        if !self.diff.is_empty() {
            out.push_str("#### Unified Diff\n```diff\n");
            out.push_str(&self.diff);
            out.push_str("\n```\n\n");
        }

        if !self.diagnostics.is_empty() {
            out.push_str("#### Diagnostics\n");
            for diag in &self.diagnostics {
                let loc = match (diag.line, diag.column) {
                    (Some(l), Some(c)) => format!(" [line {l}, col {c}]"),
                    (Some(l), None) => format!(" [line {l}]"),
                    _ => String::new(),
                };
                let code_tag = diag
                    .code
                    .as_deref()
                    .map(|c| format!(" ({c})"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "- **{}**{loc}{code_tag}: {}\n",
                    diag.severity.to_uppercase(),
                    diag.message
                ));
            }
            out.push('\n');
        }

        if !self.stderr.is_empty() && !self.success {
            out.push_str("#### Compiler Output\n```\n");
            out.push_str(&self.stderr);
            out.push_str("\n```\n");
        }

        out
    }

    /// Formats the verification result as pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the verification result as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}
