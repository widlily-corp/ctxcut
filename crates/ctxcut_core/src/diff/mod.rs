//! Semantic AST diff and structural change detection module.

pub mod semantic;

pub use semantic::{
    FileSemanticDiff, ImportChangeItem, ImportChangeKind, SemanticDiffEngine, SemanticDiffResult,
    SemanticDiffRoi, SymbolChangeKind, SymbolDiffItem,
};
