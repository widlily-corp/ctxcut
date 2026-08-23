//! `ctxcut_core` — Pure AST parsing, dependency graph traversal, type hoisting,
//! signature stripping, and context slicing engine for LLMs and AI coding agents.

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]

pub mod diff;
pub mod error;
pub mod formatter;
pub mod framework;
pub mod index;
pub mod lang;
pub mod model;
pub mod overview;
pub mod parser;
pub mod patch;
pub mod query;
pub mod refactor;
pub mod resolver;
pub mod schema;
pub mod slice;
pub mod telemetry;
pub mod test_context;
pub mod tokenizer;
pub mod traversal;
pub mod verify;

pub use diff::{
    FileSemanticDiff, ImportChangeItem, ImportChangeKind, SemanticDiffEngine, SemanticDiffResult,
    SemanticDiffRoi, SymbolChangeKind, SymbolDiffItem,
};
pub use error::{CoreError, Result};
pub use formatter::{normalize_language_tag, Formatter, JsonFormatter, MarkdownFormatter};
pub use framework::{
    DjangoFastApiAnalyzer, ExpressAnalyzer, ExpressNestSpringAnalyzer, FrameworkAnalyzer,
    FrameworkRegistry, NestJsAnalyzer, ReactNextAnalyzer, SpringAnalyzer,
};
pub use index::{
    IndexEngine, IndexOptions, IndexStatus, IndexSyncResult, CURRENT_SCHEMA_VERSION,
};
pub use lang::{
    GoAdapter, LanguageAdapter, LanguageRegistry, PythonAdapter, RustAdapter, TypeScriptAdapter,
};
pub use model::{
    BatchSliceResult, CallSignatureStub, DiscoveredFixture, ExtractedImplementor, ExtractedSymbol,
    ExtractedType, FileOverviewItem, ImpactCallerItem, ImpactSliceResult, OverviewOptions,
    PatchResult, SliceOptions, SliceResult, SupportedLanguage, SymbolOverviewItem,
    SyntaxErrorDetail, TestContextResult, TokenStats, TraceResult, TraceStep,
    VerifyDiagnostic, VerifyPatchResult, WorkspaceOverviewReport,
};
pub use overview::{format_overview_markdown, WorkspaceOverviewGenerator};
pub use parser::{AstUtils, ParserManager};
pub use patch::AstPatcher;
pub use query::{
    AstQueryEngine, AstQueryReport, MatchCapture, PresetRegistry, QueryMatchResult,
};
pub use refactor::{
    FileRenameResult, MultiFileRenameResult, RenameTargetKind, SymbolRenameOccurrence, SymbolRenamer,
};
pub use resolver::{
    DefaultForeignSymbolLocator, ExecutionTracer, ForeignSymbolLocator, ImpactAnalyzer,
    ImplementorHoister, ImportMapping, ImportResolver, SignatureStripper, SymbolLocator,
    TypeHoister,
};
pub use schema::SchemaStitcher;
pub use slice::{budget::BudgetCompressor, budget::DegradationReport, ContextSlicer};
pub use telemetry::{
    current_rfc3339_timestamp, format_rfc3339, LanguageMetric, ModelTierSavings, SourceMetric,
    TelemetryEvent, TelemetryLogger, TelemetrySummary, ECONOMY_PRICE_PER_MILLION_TOKENS,
    FRONTIER_PRICE_PER_MILLION_TOKENS, STANDARD_PRICE_PER_MILLION_TOKENS,
};
pub use test_context::{FixtureFinder, TestContextGenerator};
pub use tokenizer::{
    calculate_savings_percentage, count_lines, count_tokens, get_bpe_tokenizer, TokenCounter,
};
pub use traversal::{
    estimate_sliced_tokens, is_binary_bytes, is_binary_file, is_blacklisted_file,
    is_ignored_directory, FastFileStatItem, FastStatsReport, LanguageStatItem, ProjectWalker,
    TraversalConfig, DEFAULT_IGNORED_DIRS, DEFAULT_IGNORED_FILES,
};
pub use verify::{
    DiagnosticParser, PatchVerifier, RollbackGuard, TypecheckExecutionResult, TypecheckerDetector,
    TypecheckerRunner, VerifyPatchOptions,
};

