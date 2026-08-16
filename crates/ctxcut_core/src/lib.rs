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
pub mod telemetry;
pub mod tokenizer;

pub use error::{CoreError, Result};
pub use formatter::{normalize_language_tag, Formatter, JsonFormatter, MarkdownFormatter};
pub use lang::{
    GoAdapter, LanguageAdapter, LanguageRegistry, PythonAdapter, RustAdapter, TypeScriptAdapter,
};
pub use model::{
    CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SliceResult,
    SupportedLanguage, TokenStats,
};
pub use parser::{AstUtils, ParserManager};
pub use resolver::{
    ImportMapping, ImportResolver, SignatureStripper, SymbolLocator, TypeHoister,
};
pub use slice::ContextSlicer;
pub use telemetry::{
    current_rfc3339_timestamp, format_rfc3339, LanguageMetric, ModelTierSavings, SourceMetric,
    TelemetryEvent, TelemetryLogger, TelemetrySummary, ECONOMY_PRICE_PER_MILLION_TOKENS,
    FRONTIER_PRICE_PER_MILLION_TOKENS, STANDARD_PRICE_PER_MILLION_TOKENS,
};
pub use tokenizer::{
    calculate_savings_percentage, count_lines, count_tokens, get_bpe_tokenizer, TokenCounter,
};
