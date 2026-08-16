//! Symbol locator for TypeScript, TSX, and JavaScript ASTs.

use std::path::Path;
use tree_sitter::Node;
use crate::error::{CoreError, Result};
use crate::model::ExtractedSymbol;
use crate::parser::AstUtils;

/// Symbol locator and metadata extractor.
pub struct SymbolLocator;

impl SymbolLocator {
    /// Locates a target symbol in the AST by name or `Container.member` query.
    pub fn locate<'a>(
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
        language: &str,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        let (container_query, member_query) = parse_query(symbol_query);

        // 1. If container is specified, look inside matching container (class, interface)
        if let Some(container_name) = container_query {
            if let Some((sym, node)) = Self::find_in_container(root, source, container_name, member_query, file_path, language) {
                return Ok((sym, node));
            }
        } else {
            // 2. Search top-level declarations
            if let Some((sym, node)) = Self::find_top_level(root, source, member_query, file_path, language) {
                return Ok((sym, node));
            }

            // 3. Fallback: Search inside all classes for method with matching name
            if let Some((sym, node)) = Self::find_any_method(root, source, member_query, file_path, language) {
                return Ok((sym, node));
            }
        }

        let available = Self::list_all_symbols(root, source);
        Err(CoreError::SymbolNotFound {
            symbol: symbol_query.to_string(),
            path: file_path.to_path_buf(),
            available_symbols: available,
        })
    }

    /// Lists all declared symbol names in the file for diagnostics and suggestion.
    pub fn list_all_symbols(root: Node<'_>, source: &str) -> Vec<String> {
        let mut symbols = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            let decl = unwrap_export(child);
            match decl.kind() {
                "function_declaration" | "generator_function_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        symbols.push(AstUtils::node_text(name_node, source).to_string());
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    for declarator in AstUtils::find_children_by_kind(decl, "variable_declarator") {
                        if let Some(name_node) = declarator.child_by_field_name("name") {
                            symbols.push(AstUtils::node_text(name_node, source).to_string());
                        }
                    }
                }
                "class_declaration" | "abstract_class_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let class_name = AstUtils::node_text(name_node, source);
                        symbols.push(class_name.to_string());

                        if let Some(body) = decl.child_by_field_name("body") {
                            for member in body.named_children(&mut body.walk()) {
                                if member.kind() == "method_definition" {
                                    if let Some(m_name) = member.child_by_field_name("name") {
                                        let method_name = AstUtils::node_text(m_name, source);
                                        symbols.push(format!("{class_name}.{method_name}"));
                                    }
                                } else if member.kind() == "public_field_definition"
                                    || member.kind() == "field_definition"
                                    || member.kind() == "property_definition"
                                {
                                    if let Some(prop_name) = member.child_by_field_name("name") {
                                        let field_name = AstUtils::node_text(prop_name, source);
                                        symbols.push(format!("{class_name}.{field_name}"));
                                    }
                                }
                            }
                        }
                    }
                }
                "interface_declaration" | "type_alias_declaration" | "enum_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        symbols.push(AstUtils::node_text(name_node, source).to_string());
                    }
                }
                _ => {}
            }
        }

        symbols
    }

    fn find_top_level<'a>(
        root: Node<'a>,
        source: &'a str,
        target_name: &str,
        file_path: &Path,
        language: &str,
    ) -> Option<(ExtractedSymbol, Node<'a>)> {
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let enclosing = child;
            let decl = unwrap_export(child);

            match decl.kind() {
                "function_declaration" | "generator_function_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        if AstUtils::node_text(name_node, source) == target_name {
                            let sym = build_symbol(enclosing, decl, "function", target_name, source, file_path, language);
                            return Some((sym, decl));
                        }
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    for declarator in AstUtils::find_children_by_kind(decl, "variable_declarator") {
                        if let Some(name_node) = declarator.child_by_field_name("name") {
                            if AstUtils::node_text(name_node, source) == target_name {
                                let kind = if let Some(val) = declarator.child_by_field_name("value") {
                                    if val.kind() == "arrow_function" || val.kind() == "function_expression" {
                                        "function"
                                    } else {
                                        "variable"
                                    }
                                } else {
                                    "variable"
                                };
                                let sym = build_symbol(enclosing, enclosing, kind, target_name, source, file_path, language);
                                return Some((sym, declarator));
                            }
                        }
                    }
                }
                "class_declaration" | "abstract_class_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        if AstUtils::node_text(name_node, source) == target_name {
                            let sym = build_symbol(enclosing, decl, "class", target_name, source, file_path, language);
                            return Some((sym, decl));
                        }
                    }
                }
                "interface_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        if AstUtils::node_text(name_node, source) == target_name {
                            let sym = build_symbol(enclosing, decl, "interface", target_name, source, file_path, language);
                            return Some((sym, decl));
                        }
                    }
                }
                "type_alias_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        if AstUtils::node_text(name_node, source) == target_name {
                            let sym = build_symbol(enclosing, decl, "type", target_name, source, file_path, language);
                            return Some((sym, decl));
                        }
                    }
                }
                "enum_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        if AstUtils::node_text(name_node, source) == target_name {
                            let sym = build_symbol(enclosing, decl, "enum", target_name, source, file_path, language);
                            return Some((sym, decl));
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn find_in_container<'a>(
        root: Node<'a>,
        source: &'a str,
        container_name: &str,
        member_name: &str,
        file_path: &Path,
        language: &str,
    ) -> Option<(ExtractedSymbol, Node<'a>)> {
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let decl = unwrap_export(child);
            if decl.kind() == "class_declaration" || decl.kind() == "abstract_class_declaration" || decl.kind() == "interface_declaration" {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == container_name {
                        if let Some(body) = decl.child_by_field_name("body") {
                            for member in body.named_children(&mut body.walk()) {
                                if member.kind() == "method_definition" {
                                    if let Some(m_name) = member.child_by_field_name("name") {
                                        if AstUtils::node_text(m_name, source) == member_name {
                                            let full_name = format!("{container_name}.{member_name}");
                                            let sym = build_symbol(member, member, "method", &full_name, source, file_path, language);
                                            return Some((sym, member));
                                        }
                                    }
                                } else if member.kind() == "public_field_definition"
                                    || member.kind() == "field_definition"
                                    || member.kind() == "property_definition"
                                {
                                    if let Some(prop_name) = member.child_by_field_name("name") {
                                        if AstUtils::node_text(prop_name, source) == member_name {
                                            let full_name = format!("{container_name}.{member_name}");
                                            let sym = build_symbol(member, member, "method", &full_name, source, file_path, language);
                                            return Some((sym, member));
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
        member_name: &str,
        file_path: &Path,
        language: &str,
    ) -> Option<(ExtractedSymbol, Node<'a>)> {
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let decl = unwrap_export(child);
            if decl.kind() == "class_declaration" || decl.kind() == "abstract_class_declaration" {
                let class_name = decl
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source))
                    .unwrap_or("Class");

                if let Some(body) = decl.child_by_field_name("body") {
                    for member in body.named_children(&mut body.walk()) {
                        if member.kind() == "method_definition" {
                            if let Some(m_name) = member.child_by_field_name("name") {
                                if AstUtils::node_text(m_name, source) == member_name {
                                    let full_name = format!("{class_name}.{member_name}");
                                    let sym = build_symbol(member, member, "method", &full_name, source, file_path, language);
                                    return Some((sym, member));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

fn parse_query(query: &str) -> (Option<&str>, &str) {
    if let Some((container, member)) = query.split_once('.') {
        (Some(container.trim()), member.trim())
    } else if let Some((container, member)) = query.split_once("::") {
        (Some(container.trim()), member.trim())
    } else {
        (None, query.trim())
    }
}

fn unwrap_export(node: Node<'_>) -> Node<'_> {
    AstUtils::unwrap_export(node)
}

fn build_symbol(
    outer_node: Node<'_>,
    decl_node: Node<'_>,
    kind: &str,
    name: &str,
    source: &str,
    file_path: &Path,
    language: &str,
) -> ExtractedSymbol {
    let doc_comment = AstUtils::extract_doc_comment(outer_node, source);
    let mut start_line = outer_node.start_position().row + 1;
    let end_line = outer_node.end_position().row + 1;

    // Adjust start_line if doc comment was found
    if let Some(prev) = outer_node.prev_named_sibling() {
        if prev.kind() == "comment" && doc_comment.is_some() {
            start_line = prev.start_position().row + 1;
        }
    }

    let signature = extract_signature(decl_node, source);
    let body = AstUtils::node_text(outer_node, source).to_string();

    ExtractedSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        start_line,
        end_line,
        doc_comment,
        signature,
        body,
        language: language.to_string(),
    }
}

fn extract_signature(node: Node<'_>, source: &str) -> String {
    let text = AstUtils::node_text(node, source);
    match node.kind() {
        "function_declaration"
        | "generator_function_declaration"
        | "method_definition"
        | "class_declaration"
        | "abstract_class_declaration"
        | "interface_declaration" => {
            if let Some(body) = node.child_by_field_name("body") {
                let start = node.start_byte();
                let body_start = body.start_byte();
                if start <= body_start && body_start <= source.len() {
                    return source[start..body_start].trim().to_string();
                }
            }
        }
        _ => {}
    }
    // Fallback: first line
    text.lines().next().unwrap_or(text).trim().to_string()
}
