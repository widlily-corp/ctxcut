//! LanguageAdapter implementation for Rust.

use std::collections::HashSet;
use std::path::Path;
use tree_sitter::{Language, Node};
use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage};
use crate::parser::AstUtils;

/// Rust language adapter supporting Rust (.rs).
pub struct RustAdapter;

impl LanguageAdapter for RustAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Rust
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        let (impl_query, member_query) = parse_query(symbol_query);

        if let Some(impl_type) = impl_query {
            if let Some((sym, node)) = find_in_impl(root, source, impl_type, member_query, file_path) {
                return Ok((sym, node));
            }
        } else {
            if let Some((sym, node)) = find_top_level(root, source, member_query, file_path) {
                return Ok((sym, node));
            }

            if let Some((sym, node)) = find_any_method(root, source, member_query, file_path) {
                return Ok((sym, node));
            }
        }

        let available = self.list_symbols(root, source);
        Err(CoreError::SymbolNotFound {
            symbol: symbol_query.to_string(),
            path: file_path.to_path_buf(),
            available_symbols: available,
        })
    }

    fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String> {
        let mut symbols = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            match child.kind() {
                "function_item" | "struct_item" | "enum_item" | "trait_item" | "type_item" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        symbols.push(AstUtils::node_text(name_node, source).to_string());
                    }
                }
                "impl_item" => {
                    let type_name = child
                        .child_by_field_name("type")
                        .map(|t| AstUtils::node_text(t, source).to_string())
                        .unwrap_or_else(|| "impl".to_string());

                    if let Some(body) = child.child_by_field_name("body") {
                        for item in body.named_children(&mut body.walk()) {
                            if item.kind() == "function_item" {
                                if let Some(m_name) = item.child_by_field_name("name") {
                                    let method_name = AstUtils::node_text(m_name, source);
                                    symbols.push(format!("{type_name}::{method_name}"));
                                }
                            }
                        }
                    }
                }
                _ => {}
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
        _opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>> {
        let mut referenced_names = HashSet::new();

        for id in AstUtils::find_descendants_by_kind(target_node, "type_identifier") {
            let name = AstUtils::node_text(id, source);
            if !is_builtin_rust_type(name) {
                referenced_names.insert(name);
            }
        }

        let mut hoisted = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if matches!(child.kind(), "struct_item" | "enum_item" | "trait_item" | "type_item") {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let type_name = AstUtils::node_text(name_node, source);
                    if referenced_names.contains(type_name) {
                        let full_decl = AstUtils::node_text(child, source).to_string();

                        hoisted.push(ExtractedType {
                            name: type_name.to_string(),
                            kind: child.kind().replace("_item", ""),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: full_decl,
                        });
                    }
                }
            }
        }

        Ok(hoisted)
    }

    fn strip_calls<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
    ) -> Result<Vec<CallSignatureStub>> {
        let mut stubs = Vec::new();
        let mut seen = HashSet::new();

        let call_nodes = AstUtils::find_descendants_by_kind(target_node, "call_expression");
        for call in call_nodes {
            if let Some(func_node) = call.child_by_field_name("function") {
                let call_text = AstUtils::node_text(func_node, source);
                let call_name = call_text.split("::").last().unwrap_or(call_text);

                if !seen.insert(call_name.to_string()) {
                    continue;
                }

                if let Some(sig) = find_rust_signature(root, source, call_name) {
                    stubs.push(CallSignatureStub {
                        name: call_name.to_string(),
                        receiver: None,
                        file_path: Some(file_path.to_string_lossy().to_string()),
                        signature: sig,
                    });
                }
            }
        }

        Ok(stubs)
    }
}

fn parse_query(query: &str) -> (Option<&str>, &str) {
    if let Some((container, member)) = query.split_once("::") {
        (Some(container.trim()), member.trim())
    } else if let Some((container, member)) = query.split_once('.') {
        (Some(container.trim()), member.trim())
    } else {
        (None, query.trim())
    }
}

fn find_top_level<'a>(
    root: Node<'a>,
    source: &'a str,
    target_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some((build_rust_symbol(child, source, file_path, "function"), child));
                    }
                }
            }
            "struct_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some((build_rust_symbol(child, source, file_path, "struct"), child));
                    }
                }
            }
            "enum_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some((build_rust_symbol(child, source, file_path, "enum"), child));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_in_impl<'a>(
    root: Node<'a>,
    source: &'a str,
    impl_type: &str,
    method_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "impl_item" {
            if let Some(t_node) = child.child_by_field_name("type") {
                if AstUtils::node_text(t_node, source) == impl_type {
                    if let Some(body) = child.child_by_field_name("body") {
                        for member in body.named_children(&mut body.walk()) {
                            if member.kind() == "function_item" {
                                if let Some(m_name) = member.child_by_field_name("name") {
                                    if AstUtils::node_text(m_name, source) == method_name {
                                        return Some((build_rust_symbol(member, source, file_path, "method"), member));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn find_any_method<'a>(
    root: Node<'a>,
    source: &'a str,
    method_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "impl_item" {
            if let Some(body) = child.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    if member.kind() == "function_item" {
                        if let Some(m_name) = member.child_by_field_name("name") {
                            if AstUtils::node_text(m_name, source) == method_name {
                                return Some((build_rust_symbol(member, source, file_path, "method"), member));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn build_rust_symbol(node: Node<'_>, source: &str, file_path: &Path, kind: &str) -> ExtractedSymbol {
    let name = node
        .child_by_field_name("name")
        .map(|n| AstUtils::node_text(n, source).to_string())
        .unwrap_or_else(|| "anonymous".to_string());

    let body = AstUtils::node_text(node, source).to_string();
    let doc_comment = AstUtils::extract_doc_comment(node, source);
    let signature = extract_rust_sig(node, source);

    ExtractedSymbol {
        name,
        kind: kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        doc_comment,
        signature,
        body,
        language: "rust".to_string(),
    }
}

fn extract_rust_sig(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let sig_end = body.start_byte();
        let start = node.start_byte();
        if start < sig_end && sig_end <= source.len() {
            return source[start..sig_end].trim().to_string();
        }
    }
    AstUtils::node_text(node, source).to_string()
}

fn find_rust_signature(root: Node<'_>, source: &str, func_name: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "function_item" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == func_name {
                    return Some(extract_rust_sig(child, source));
                }
            }
        }
    }
    None
}

fn is_builtin_rust_type(name: &str) -> bool {
    matches!(
        name,
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
            | "f32" | "f64" | "bool" | "char" | "str" | "String" | "Option" | "Result" | "Vec" | "Box" | "Arc"
            | "Rc" | "Path" | "PathBuf"
    )
}
