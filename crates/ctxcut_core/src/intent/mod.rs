//! Semantic intent-driven AST slicing combining BM25 lexical-structural ranking
//! with Tree-sitter AST dependency traversal and critical context bundle assembly.

pub mod bm25;
pub mod bundle;
pub mod ranker;
pub mod slicer;
pub mod tokenizer;

pub use bm25::{compute_idf, Bm25Index, Bm25Params, Posting};
pub use bundle::{assemble_critical_bundle, CriticalAstBundle};
pub use ranker::{HybridAstRanker, ScoredCandidate};
pub use slicer::{DefaultIntentSlicer, IntentSliceOptions, IntentSliceResult, IntentSlicer};
pub use tokenizer::{
    extract_query_keywords, extract_symbol_tokens, tokenize_nl_and_code, FieldKind,
    SymbolTokenDocument,
};
