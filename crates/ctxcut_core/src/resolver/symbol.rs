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
            // 2. Search top-level declarations (including error recovery inside ERROR nodes)
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
        collect_symbols_recursive(root, source, &mut symbols);
        symbols
    }

    fn find_top_level<'a>(
        root: Node<'a>,
        source: &'a str,
        target_name: &str,
        file_path: &Path,
        language: &str,
    ) -> Option<(ExtractedSymbol, Node<'a>)> {
        find_symbol_recursive(root, source, target_name, file_path, language)
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
            if matches!(
                decl.kind(),
                "class_declaration" | "abstract_class_declaration" | "interface_declaration"
            ) {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == container_name {
                        if let Some(body) = decl.child_by_field_name("body") {
                            for member in body.named_children(&mut body.walk()) {
                                if matches!(
                                    member.kind(),
                                    "method_definition"
                                        | "public_field_definition"
                                        | "field_definition"
                                        | "property_definition"
                                ) {
                                    if let Some(m_name) = member.child_by_field_name("name") {
                                        if AstUtils::node_text(m_name, source) == member_name {
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
            if matches!(
                decl.kind(),
                "class_declaration" | "abstract_class_declaration"
            ) {
                let class_name = decl
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source))
                    .unwrap_or("Class");

                if let Some(body) = decl.child_by_field_name("body") {
                    for member in body.named_children(&mut body.walk()) {
                        if matches!(
                            member.kind(),
                            "method_definition"
                                | "public_field_definition"
                                | "field_definition"
                                | "property_definition"
                        ) {
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

fn collect_symbols_recursive(node: Node<'_>, source: &str, out: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let decl = unwrap_export(child);
        match decl.kind() {
            "function_declaration"
            | "generator_function_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_node, source).to_string();
                    if !out.contains(&name) {
                        out.push(name);
                    }
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                for declarator in AstUtils::find_children_by_kind(decl, "variable_declarator") {
                    if let Some(name_node) = declarator.child_by_field_name("name") {
                        let name = AstUtils::node_text(name_node, source).to_string();
                        if !out.contains(&name) {
                            out.push(name);
                        }
                    }
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    let class_name = AstUtils::node_text(name_node, source);
                    if !out.contains(&class_name.to_string()) {
                        out.push(class_name.to_string());
                    }

                    if let Some(body) = decl.child_by_field_name("body") {
                        for member in body.named_children(&mut body.walk()) {
                            if matches!(
                                member.kind(),
                                "method_definition"
                                    | "public_field_definition"
                                    | "field_definition"
                                    | "property_definition"
                            ) {
                                if let Some(m_name) = member.child_by_field_name("name") {
                                    let member_name = AstUtils::node_text(m_name, source);
                                    let full = format!("{class_name}.{member_name}");
                                    if !out.contains(&full) {
                                        out.push(full);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "ERROR" => {
                collect_symbols_recursive(decl, source, out);
            }
            _ => {}
        }
    }
}

fn find_symbol_recursive<'a>(
    node: Node<'a>,
    source: &'a str,
    target_name: &str,
    file_path: &Path,
    language: &str,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
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
            "ERROR" => {
                if let Some(found) = find_symbol_recursive(decl, source, target_name, file_path, language) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn unwrap_export(node: Node<'_>) -> Node<'_> {
    if node.kind() == "export_statement" {
        if let Some(declaration) = node.child_by_field_name("declaration") {
            return declaration;
        }
        for child in node.named_children(&mut node.walk()) {
            if matches!(
                child.kind(),
                "function_declaration"
                    | "class_declaration"
                    | "interface_declaration"
                    | "type_alias_declaration"
                    | "enum_declaration"
                    | "lexical_declaration"
                    | "variable_declaration"
            ) {
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

fn build_symbol(
    enclosing_node: Node<'_>,
    decl_node: Node<'_>,
    kind: &str,
    name: &str,
    source: &str,
    file_path: &Path,
    language: &str,
) -> ExtractedSymbol {
    let body = AstUtils::node_text(enclosing_node, source).to_string();
    let doc_comment = AstUtils::extract_doc_comment(enclosing_node, source);
    let signature = extract_signature(decl_node, source);

    ExtractedSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        start_line: enclosing_node.start_position().row + 1,
        end_line: enclosing_node.end_position().row + 1,
        doc_comment,
        signature,
        body,
        language: language.to_string(),
    }
}

fn extract_signature(decl: Node<'_>, source: &str) -> String {
    if let Some(body) = decl.child_by_field_name("body") {
        let sig_end = body.start_byte();
        let decl_start = decl.start_byte();
        if decl_start < sig_end && sig_end <= source.len() {
            let sig = source[decl_start..sig_end].trim_end();
            return format!("{};", sig.strip_suffix('{').unwrap_or(sig).trim());
        }
    }
    let text = AstUtils::node_text(decl, source).trim().to_string();
    if !text.ends_with(';') {
        format!("{text};")
    } else {
        text
    }
}
