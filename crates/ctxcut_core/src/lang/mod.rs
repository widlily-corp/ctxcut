//! Language adapter traits and registry for multi-language AST extraction.

pub mod go;
pub mod python;
pub mod rust_lang;
pub mod typescript;

use crate::error::{CoreError, Result};
use crate::model::{
    CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage,
};
use std::path::Path;
use tree_sitter::{Language, Node};

pub use go::GoAdapter;
pub use python::PythonAdapter;
pub use rust_lang::RustAdapter;
pub use typescript::TypeScriptAdapter;

/// Common trait implemented by each target programming language parser & resolver.
pub trait LanguageAdapter: Send + Sync {
    /// Returns the high-level `SupportedLanguage` enum variant.
    fn language(&self) -> SupportedLanguage;

    /// Returns the tree-sitter `Language` definition for the given file path.
    fn tree_sitter_language(&self, path: &Path) -> Language;

    /// Locates the AST node and extracted metadata for a target symbol query.
    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)>;

    /// Lists all available symbols in the file for diagnostics and error reporting.
    fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String>;

    /// Extracts referenced types from the symbol signature and body.
    fn hoist_types<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>>;

    /// Identifies call expressions and extracts body-stripped signatures.
    fn strip_calls<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
    ) -> Result<Vec<CallSignatureStub>>;
}

/// Registry and factory for resolving language adapters.
pub struct LanguageRegistry;

impl LanguageRegistry {
    /// Returns the appropriate `LanguageAdapter` for a given file path.
    pub fn for_path(path: &Path) -> Result<Box<dyn LanguageAdapter>> {
        let lang = SupportedLanguage::from_path(path).ok_or_else(|| {
            let ext = path.extension().and_then(|e| e.to_str()).map(String::from);
            CoreError::UnsupportedLanguage {
                path: path.to_path_buf(),
                extension: ext,
            }
        })?;

        Self::for_language(lang)
    }

    /// Returns the appropriate `LanguageAdapter` for a given `SupportedLanguage`.
    pub fn for_language(language: SupportedLanguage) -> Result<Box<dyn LanguageAdapter>> {
        match language {
            SupportedLanguage::TypeScript => Ok(Box::new(TypeScriptAdapter::new_typescript())),
            SupportedLanguage::JavaScript => Ok(Box::new(TypeScriptAdapter::new_javascript())),
            SupportedLanguage::Python => Ok(Box::new(PythonAdapter)),
            SupportedLanguage::Go => Ok(Box::new(GoAdapter)),
            SupportedLanguage::Rust => Ok(Box::new(RustAdapter)),
        }
    }
}
