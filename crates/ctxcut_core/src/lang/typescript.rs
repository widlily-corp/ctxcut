//! LanguageAdapter implementation for TypeScript, TSX, and JavaScript.

use crate::error::Result;
use crate::lang::LanguageAdapter;
use crate::model::{
    CallSignatureStub, ExtractedImplementor, ExtractedSymbol, ExtractedType, SliceOptions,
    SupportedLanguage,
};
use crate::parser::AstUtils;
use crate::resolver::{SignatureStripper, SymbolLocator, TypeHoister};
use std::path::Path;
use tree_sitter::{Language, Node};

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
            "jsx" | "js" | "mjs" | "cjs" => tree_sitter_javascript::LANGUAGE.into(),
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
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let lang_name = match ext.to_lowercase().as_str() {
            "tsx" => "tsx",
            "jsx" => "jsx",
            "js" | "mjs" | "cjs" => "javascript",
            _ => "typescript",
        };
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

    fn find_implementors<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        interface_name: &str,
        file_path: &Path,
    ) -> Result<Vec<ExtractedImplementor>> {
        let mut implementors = Vec::new();
        let class_nodes = AstUtils::find_descendants_by_kind(root, "class_declaration");

        for class_node in class_nodes {
            if let Some(heritage) = AstUtils::find_child_by_kind(class_node, "class_heritage") {
                let text = AstUtils::node_text(heritage, source);
                let implements_matches = if let Some(impl_clause) =
                    AstUtils::find_child_by_kind(heritage, "implements_clause")
                {
                    let impl_text = AstUtils::node_text(impl_clause, source);
                    impl_text
                        .split(|c: char| c == ',' || c.is_whitespace() || c == '<' || c == '>')
                        .any(|part| part.trim() == interface_name)
                } else {
                    text.contains(&format!("implements {interface_name}"))
                        || text.contains(&format!("implements {interface_name}<"))
                        || text.contains(&format!("{interface_name},"))
                        || text.contains(&format!(", {interface_name}"))
                };

                if implements_matches {
                    if let Some(name_node) = class_node.child_by_field_name("name") {
                        let class_name = AstUtils::node_text(name_node, source).to_string();
                        let stub = extract_ts_class_stub(class_node, source);
                        implementors.push(ExtractedImplementor {
                            interface_name: interface_name.to_string(),
                            implementor_name: class_name,
                            kind: "ts_class".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: stub,
                        });
                    }
                }
            }
        }

        Ok(implementors)
    }
}

fn extract_ts_class_stub(class_node: Node<'_>, source: &str) -> String {
    if let Some(body) = class_node.child_by_field_name("body") {
        let header_end = body.start_byte();
        let header = source[class_node.start_byte()..header_end].trim();
        let mut stubs = Vec::new();

        for member in body.named_children(&mut body.walk()) {
            if member.kind() == "method_definition" {
                if let Some(m_body) = member.child_by_field_name("body") {
                    let sig = source[member.start_byte()..m_body.start_byte()].trim();
                    stubs.push(format!("    {sig} {{ ... }}"));
                }
            }
        }

        if stubs.is_empty() {
            format!("{header} {{ ... }}")
        } else {
            format!("{header} {{\n{}\n}}", stubs.join("\n"))
        }
    } else {
        AstUtils::node_text(class_node, source).trim().to_string()
    }
}
