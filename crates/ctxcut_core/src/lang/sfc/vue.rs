//! LanguageAdapter implementation for Vue Single File Components (.vue).

use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{
    CallSignatureStub, ExtractedImplementor, ExtractedSymbol, ExtractedType, SliceOptions,
    SupportedLanguage,
};
use crate::resolver::{SignatureStripper, SymbolLocator, TypeHoister};
use std::path::Path;
use tree_sitter::{Language, Node};

/// Language adapter supporting Vue Single File Components (.vue).
pub struct VueAdapter;

impl LanguageAdapter for VueAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Vue
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
        // 1. Check if query targets props / defineProps
        if symbol_query == "defineProps" || symbol_query == "props" {
            if let Some(props_sym) = find_vue_props_symbol(source, file_path) {
                return Ok((props_sym, root));
            }
        }

        // 2. Locate symbol directly in root/source
        if let Ok((mut sym, node)) = SymbolLocator::locate(root, source, symbol_query, file_path, "vue") {
            sym.language = "vue".to_string();
            Ok((sym, node))
        } else {
            if let Some(props_sym) = find_vue_props_symbol(source, file_path) {
                if props_sym.name == symbol_query || symbol_query.contains("props") || symbol_query.contains("Props") {
                    return Ok((props_sym, root));
                }
            }
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
        if source.contains("defineProps") && !symbols.contains(&"defineProps".to_string()) {
            symbols.push("defineProps".to_string());
        }
        if source.contains("defineEmits") && !symbols.contains(&"defineEmits".to_string()) {
            symbols.push("defineEmits".to_string());
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
        let mut types = TypeHoister::hoist_types(target_node, root, source, file_path, opts, &ts_lang)?;

        if let Some(start_idx) = source.find("defineProps<") {
            let after = &source[start_idx + 12..];
            if let Some(end_idx) = after.find('>') {
                let type_param = after[..end_idx].trim();
                if !types.iter().any(|t| t.name == type_param) {
                    if let Ok(mut prop_types) = TypeHoister::hoist_types(root, root, source, file_path, opts, &ts_lang) {
                        for pt in prop_types.drain(..) {
                            if !types.iter().any(|t| t.name == pt.name) {
                                types.push(pt);
                            }
                        }
                    }
                }
            }
        }

        // Also check module-level imports referenced by module variables used in target_node
        let target_text = crate::parser::AstUtils::node_text(target_node, source);
        let file_imports = crate::resolver::imports::ImportResolver::extract_imports(root, source);
        for imported_name in file_imports.keys() {
            if !types.iter().any(|t| &t.name == imported_name) {
                for line in source.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains(imported_name) && !trimmed.starts_with("import ") {
                        let var_name = trimmed
                            .split(|c: char| c == '=' || c == ':' || c.is_whitespace())
                            .find(|s| !s.is_empty() && *s != "const" && *s != "let" && *s != "var" && *s != "ref")
                            .unwrap_or("");
                        if !var_name.is_empty() && target_text.contains(var_name) {
                            if let Ok(mut resolved) = TypeHoister::resolve_foreign_types(file_path, &[imported_name]) {
                                for t in resolved.drain(..) {
                                    if !types.iter().any(|existing| existing.name == t.name) {
                                        types.push(t);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(types)
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

fn find_vue_props_symbol(script_source: &str, file_path: &Path) -> Option<ExtractedSymbol> {
    for (line_idx, line) in script_source.lines().enumerate() {
        if line.contains("defineProps") {
            let start_line = line_idx + 1;
            let body_lines: Vec<&str> = script_source.lines().skip(line_idx).take(20).collect();
            let body = body_lines.join("\n");
            let signature = line.trim().to_string();

            return Some(ExtractedSymbol {
                name: "defineProps".to_string(),
                kind: "function".to_string(),
                file_path: file_path.to_string_lossy().to_string(),
                start_line,
                end_line: start_line + body_lines.len().saturating_sub(1),
                doc_comment: None,
                signature,
                body,
                language: "vue".to_string(),
            });
        }
    }
    None
}
