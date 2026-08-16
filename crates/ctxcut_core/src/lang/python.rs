//! LanguageAdapter stub for Python (scheduled for Milestone 2).

use std::path::Path;
use tree_sitter::{Language, Node};
use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage};

/// Python language adapter stub.
pub struct PythonAdapter;

impl LanguageAdapter for PythonAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Python
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        // Will be wired in Milestone 2
        unimplemented!("Python tree-sitter language scheduled for Milestone 2")
    }

    fn locate_symbol<'a>(
        &self,
        _root: Node<'a>,
        _source: &'a str,
        _symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        Err(CoreError::UnsupportedLanguage {
            path: file_path.to_path_buf(),
            extension: Some("py".to_string()),
        })
    }

    fn list_symbols<'a>(&self, _root: Node<'a>, _source: &'a str) -> Vec<String> {
        Vec::new()
    }

    fn hoist_types<'a>(
        &self,
        _target_node: Node<'a>,
        _root: Node<'a>,
        _source: &'a str,
        _file_path: &Path,
        _opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>> {
        Ok(Vec::new())
    }

    fn strip_calls<'a>(
        &self,
        _target_node: Node<'a>,
        _root: Node<'a>,
        _source: &'a str,
        _file_path: &Path,
    ) -> Result<Vec<CallSignatureStub>> {
        Ok(Vec::new())
    }
}
