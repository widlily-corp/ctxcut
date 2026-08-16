//! LanguageAdapter implementation for TypeScript, TSX, and JavaScript.

use std::path::Path;
use tree_sitter::{Language, Node};
use crate::error::Result;
use crate::lang::LanguageAdapter;
use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage};
use crate::resolver::{SignatureStripper, SymbolLocator, TypeHoister};

/// Language adapter supporting TypeScript (.ts, .mts, .cts, .d.ts), TSX (.tsx), and JavaScript (.js, .jsx, .mjs, .cjs).
pub struct TypeScriptAdapter {
    language_variant: SupportedLanguage,
}

impl TypeScriptAdapter {
    /// Creates a new `TypeScriptAdapter` for TypeScript.
    pub fn new_typescript() -> Self {
        Self {
            language_variant: SupportedLanguage::TypeScript,
        }
    }

    /// Creates a new `TypeScriptAdapter` for JavaScript.
    pub fn new_javascript() -> Self {
        Self {
            language_variant: SupportedLanguage::JavaScript,
        }
    }
}

impl LanguageAdapter for TypeScriptAdapter {
    fn language(&self) -> SupportedLanguage {
        self.language_variant
    }

    fn tree_sitter_language(&self, path: &Path) -> Language {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            "js" | "jsx" | "mjs" | "cjs" => tree_sitter_javascript::LANGUAGE.into(),
            _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }

    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        let lang_name = self.language_variant.as_str();
        SymbolLocator::locate(root, source, symbol_query, file_path, lang_name)
    }

    fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String> {
        SymbolLocator::list_all_symbols(root, source)
    }

    fn hoist_types<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>> {
        let ts_lang = self.tree_sitter_language(file_path);
        TypeHoister::hoist_types(target_node, root, source, file_path, opts, &ts_lang)
    }

    fn strip_calls<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
    ) -> Result<Vec<CallSignatureStub>> {
        let ts_lang = self.tree_sitter_language(file_path);
        SignatureStripper::strip_calls(target_node, root, source, file_path, &ts_lang)
    }
}
