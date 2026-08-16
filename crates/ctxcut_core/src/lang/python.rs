//! LanguageAdapter implementation for Python.

use std::collections::HashSet;
use std::path::Path;
use tree_sitter::{Language, Node};
use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage};
use crate::parser::AstUtils;

/// Python language adapter supporting Python (.py, .pyi).
pub struct PythonAdapter;

impl LanguageAdapter for PythonAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Python
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        let (container_query, member_query) = parse_query(symbol_query);

        if let Some(container_name) = container_query {
            if let Some((sym, node)) = find_in_class(root, source, container_name, member_query, file_path) {
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
            let node = unwrap_decorated(child);
            match node.kind() {
                "function_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        symbols.push(AstUtils::node_text(name_node, source).to_string());
                    }
                }
                "class_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let class_name = AstUtils::node_text(name_node, source);
                        symbols.push(class_name.to_string());

                        if let Some(body) = node.child_by_field_name("body") {
                            for member in body.named_children(&mut body.walk()) {
                                let m_node = unwrap_decorated(member);
                                if m_node.kind() == "function_definition" {
                                    if let Some(m_name) = m_node.child_by_field_name("name") {
                                        let member_name = AstUtils::node_text(m_name, source);
                                        symbols.push(format!("{class_name}.{member_name}"));
                                    }
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

        // Collect all type identifiers referenced in parameter and return annotations
        let type_nodes = AstUtils::find_descendants_by_kind(target_node, "type");
        for t_node in type_nodes {
            for id in AstUtils::find_descendants_by_kind(t_node, "identifier") {
                let name = AstUtils::node_text(id, source);
                if !is_builtin_python_type(name) {
                    referenced_names.insert(name);
                }
            }
        }

        // Also check any standalone identifiers that match class names
        for id in AstUtils::find_descendants_by_kind(target_node, "identifier") {
            let name = AstUtils::node_text(id, source);
            if name.chars().next().map_or(false, |c| c.is_ascii_uppercase()) && !is_builtin_python_type(name) {
                referenced_names.insert(name);
            }
        }

        let mut hoisted = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            let decl = unwrap_decorated(child);
            if decl.kind() == "class_definition" {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    let class_name = AstUtils::node_text(name_node, source);
                    if referenced_names.contains(class_name) {
                        let full_decl = AstUtils::node_text(child, source).to_string();

                        hoisted.push(ExtractedType {
                            name: class_name.to_string(),
                            kind: "class".to_string(),
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

        let call_nodes = AstUtils::find_descendants_by_kind(target_node, "call");
        for call in call_nodes {
            if let Some(func_node) = call.child_by_field_name("function") {
                let call_text = AstUtils::node_text(func_node, source);
                let call_name = call_text.split('.').last().unwrap_or(call_text);

                if !seen.insert(call_name.to_string()) || is_builtin_python_func(call_name) {
                    continue;
                }

                if let Some(sig) = find_python_signature(root, source, call_name) {
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

fn unwrap_decorated(node: Node<'_>) -> Node<'_> {
    if node.kind() == "decorated_definition" {
        if let Some(definition) = node.child_by_field_name("definition") {
            return definition;
        }
        for child in node.named_children(&mut node.walk()) {
            if matches!(child.kind(), "function_definition" | "class_definition") {
                return child;
            }
        }
    }
    node
}

fn parse_query(query: &str) -> (Option<&str>, &str) {
    if let Some((container, member)) = query.split_once('.') {
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
        let full_node = child;
        let decl = unwrap_decorated(child);

        match decl.kind() {
            "function_definition" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_node, source);
                    if name == target_name {
                        return Some((build_extracted_symbol(full_node, decl, source, file_path, "function"), full_node));
                    }
                }
            }
            "class_definition" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_node, source);
                    if name == target_name {
                        return Some((build_extracted_symbol(full_node, decl, source, file_path, "class"), full_node));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_in_class<'a>(
    root: Node<'a>,
    source: &'a str,
    class_name: &str,
    member_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = unwrap_decorated(child);
        if decl.kind() == "class_definition" {
            if let Some(name_node) = decl.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == class_name {
                    if let Some(body) = decl.child_by_field_name("body") {
                        for member in body.named_children(&mut body.walk()) {
                            let m_full = member;
                            let m_decl = unwrap_decorated(member);
                            if m_decl.kind() == "function_definition" {
                                if let Some(m_name) = m_decl.child_by_field_name("name") {
                                    if AstUtils::node_text(m_name, source) == member_name {
                                        return Some((build_extracted_symbol(m_full, m_decl, source, file_path, "method"), m_full));
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
        let decl = unwrap_decorated(child);
        if decl.kind() == "class_definition" {
            if let Some(body) = decl.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    let m_full = member;
                    let m_decl = unwrap_decorated(member);
                    if m_decl.kind() == "function_definition" {
                        if let Some(m_name) = m_decl.child_by_field_name("name") {
                            if AstUtils::node_text(m_name, source) == method_name {
                                return Some((build_extracted_symbol(m_full, m_decl, source, file_path, "method"), m_full));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn build_extracted_symbol(
    full_node: Node<'_>,
    decl: Node<'_>,
    source: &str,
    file_path: &Path,
    kind: &str,
) -> ExtractedSymbol {
    let name = decl
        .child_by_field_name("name")
        .map(|n| AstUtils::node_text(n, source).to_string())
        .unwrap_or_else(|| "anonymous".to_string());

    let body = AstUtils::node_text(full_node, source).to_string();
    let doc_comment = AstUtils::extract_doc_comment(full_node, source);
    let signature = extract_python_sig(decl, source);

    ExtractedSymbol {
        name,
        kind: kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        start_line: full_node.start_position().row + 1,
        end_line: full_node.end_position().row + 1,
        doc_comment,
        signature,
        body,
        language: "python".to_string(),
    }
}

fn extract_python_sig(decl: Node<'_>, source: &str) -> String {
    if let Some(body) = decl.child_by_field_name("body") {
        let sig_end = body.start_byte();
        let decl_start = decl.start_byte();
        if decl_start < sig_end && sig_end <= source.len() {
            let sig = source[decl_start..sig_end].trim_end();
            return sig.strip_suffix(':').unwrap_or(sig).trim().to_string();
        }
    }
    AstUtils::node_text(decl, source).to_string()
}

fn find_python_signature(root: Node<'_>, source: &str, func_name: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = unwrap_decorated(child);
        if decl.kind() == "function_definition" {
            if let Some(name_node) = decl.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == func_name {
                    return Some(format!("{}: ...", extract_python_sig(decl, source)));
                }
            }
        }
    }
    None
}

fn is_builtin_python_type(name: &str) -> bool {
    matches!(
        name,
        "int"
            | "float"
            | "str"
            | "bool"
            | "bytes"
            | "list"
            | "dict"
            | "set"
            | "tuple"
            | "Optional"
            | "Union"
            | "Any"
            | "Sequence"
            | "Iterable"
            | "Mapping"
            | "None"
    )
}

fn is_builtin_python_func(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "len"
            | "range"
            | "enumerate"
            | "isinstance"
            | "issubclass"
            | "round"
            | "str"
            | "int"
            | "float"
            | "dict"
            | "list"
            | "set"
    )
}
