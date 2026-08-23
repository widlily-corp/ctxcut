//! LanguageAdapter implementation for Svelte Single File Components (.svelte).

use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{
    CallSignatureStub, ExtractedImplementor, ExtractedSymbol, ExtractedType, SliceOptions,
    SupportedLanguage,
};
use crate::resolver::{SignatureStripper, SymbolLocator, TypeHoister};
use std::path::Path;
use tree_sitter::{Language, Node};

/// Language adapter supporting Svelte Single File Components (.svelte).
pub struct SvelteAdapter;

impl LanguageAdapter for SvelteAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Svelte
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
        // 1. Check for Svelte exported props (e.g. export let count)
        if let Some(prop_sym) = find_svelte_prop_symbol(source, symbol_query, file_path) {
            return Ok((prop_sym, root));
        }

        // 2. Standard TypeScript AST lookup
        if let Ok((mut sym, node)) = SymbolLocator::locate(root, source, symbol_query, file_path, "svelte") {
            sym.language = "svelte".to_string();
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

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("export let ") {
                let var_part = trimmed.trim_start_matches("export let ").trim();
                let var_name = var_part.split(|c: char| c == ':' || c == '=' || c.is_whitespace()).next().unwrap_or("").trim();
                if !var_name.is_empty() && !symbols.contains(&var_name.to_string()) {
                    symbols.push(var_name.to_string());
                }
            } else if trimmed.contains("$props()") || trimmed.contains("$state(") {
                if let Some(var_name) = trimmed.split(|c: char| c == '=' || c == ':' || c.is_whitespace()).find(|s| !s.is_empty() && *s != "let" && *s != "const" && *s != "var") {
                    if !symbols.contains(&var_name.to_string()) {
                        symbols.push(var_name.to_string());
                    }
                }
            }
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

fn find_svelte_prop_symbol(script_source: &str, target_name: &str, file_path: &Path) -> Option<ExtractedSymbol> {
    for (line_idx, line) in script_source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("export let ") && trimmed.contains(target_name) {
            let start_line = line_idx + 1;
            let signature = trimmed.to_string();
            let body = format!("{trimmed}\n");

            return Some(ExtractedSymbol {
                name: target_name.to_string(),
                kind: "property".to_string(),
                file_path: file_path.to_string_lossy().to_string(),
                start_line,
                end_line: start_line,
                doc_comment: None,
                signature,
                body,
                language: "svelte".to_string(),
            });
        }
    }
    None
}
