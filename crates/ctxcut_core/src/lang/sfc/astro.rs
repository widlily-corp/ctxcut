//! LanguageAdapter implementation for Astro components (.astro).

use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{
    CallSignatureStub, ExtractedImplementor, ExtractedSymbol, ExtractedType, SliceOptions,
    SupportedLanguage,
};
use crate::resolver::{SignatureStripper, SymbolLocator, TypeHoister};
use std::path::Path;
use tree_sitter::{Language, Node};

/// Language adapter supporting Astro components (.astro).
pub struct AstroAdapter;

impl LanguageAdapter for AstroAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Astro
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    }

    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        // 1. Check for Astro Props or frontmatter variables
        if symbol_query == "Props" || symbol_query == "props" {
            if let Some(props_sym) = find_astro_props_symbol(source, file_path) {
                return Ok((props_sym, root));
            }
        }

        // 2. TypeScript AST lookup
        if let Ok((mut sym, node)) =
            SymbolLocator::locate(root, source, symbol_query, file_path, "astro")
        {
            sym.language = "astro".to_string();
            Ok((sym, node))
        } else {
            let available = self.list_symbols(root, source);
            Err(CoreError::SymbolNotFound {
                symbol: symbol_query.to_string(),
                path: file_path.to_path_buf(),
                available_symbols: available,
            })
        }
    }

    fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String> {
        let mut symbols = SymbolLocator::list_all_symbols(root, source);

        if source.contains("Astro.props") && !symbols.contains(&"Props".to_string()) {
            symbols.push("Props".to_string());
        }

        symbols
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

    fn find_implementors<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        interface_name: &str,
        file_path: &Path,
    ) -> Result<Vec<ExtractedImplementor>> {
        let adapter = crate::lang::typescript::TypeScriptAdapter::new_typescript();
        adapter.find_implementors(root, source, interface_name, file_path)
    }
}

fn find_astro_props_symbol(frontmatter: &str, file_path: &Path) -> Option<ExtractedSymbol> {
    for (line_idx, line) in frontmatter.lines().enumerate() {
        if line.contains("Astro.props") || line.contains("interface Props") {
            let start_line = line_idx + 1;
            let body_lines: Vec<&str> = frontmatter.lines().skip(line_idx).take(20).collect();
            let body = body_lines.join("\n");
            let signature = line.trim().to_string();

            return Some(ExtractedSymbol {
                name: "Props".to_string(),
                kind: "interface".to_string(),
                file_path: file_path.to_string_lossy().to_string(),
                start_line,
                end_line: start_line + body_lines.len().saturating_sub(1),
                doc_comment: None,
                signature,
                body,
                language: "astro".to_string(),
            });
        }
    }
    None
}
