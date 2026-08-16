//! LanguageAdapter implementation for Python.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
use tree_sitter::{Language, Node};
use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage};
use crate::parser::{AstUtils, ParserManager};

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
        opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>> {
        let mut hoisted = Vec::new();
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        // Collect initial referenced types
        for id in AstUtils::find_descendants_by_kind(target_node, "identifier") {
            let name = AstUtils::node_text(id, source);
            if !is_builtin_python_type(name) && !is_builtin_python_func(name) {
                if visited.insert(name.to_string()) {
                    queue.push_back((name.to_string(), 1));
                }
            }
        }

        // Also check string annotations in quotes (e.g. Optional["ModelA"])
        for str_node in AstUtils::find_descendants_by_kind(target_node, "string") {
            let text = AstUtils::node_text(str_node, source).trim_matches('"').trim_matches('\'');
            if !is_builtin_python_type(text) && visited.insert(text.to_string()) {
                queue.push_back((text.to_string(), 1));
            }
        }

        let dir = file_path.parent().unwrap_or_else(|| Path::new("."));
        let ts_lang = self.tree_sitter_language(file_path);

        while let Some((type_name, depth)) = queue.pop_front() {
            if is_builtin_python_type(&type_name) {
                continue;
            }

            // 1. Search in local file
            if let Some(extracted) = find_class_or_type_in_file(root, source, &type_name, file_path) {
                if depth < opts.depth {
                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, file_path) {
                        for id in AstUtils::find_descendants_by_kind(tree.root_node(), "identifier") {
                            let name = AstUtils::node_text(id, &extracted.definition);
                            if !is_builtin_python_type(name) && visited.insert(name.to_string()) {
                                queue.push_back((name.to_string(), depth + 1));
                            }
                        }
                        for str_node in AstUtils::find_descendants_by_kind(tree.root_node(), "string") {
                            let text = AstUtils::node_text(str_node, &extracted.definition).trim_matches('"').trim_matches('\'');
                            if !is_builtin_python_type(text) && visited.insert(text.to_string()) {
                                queue.push_back((text.to_string(), depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            // 2. Search in sibling files in same directory / package
            if let Ok(entries) = fs::read_dir(dir) {
                let mut found = false;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path == file_path || path.extension().and_then(|e| e.to_str()) != Some("py") {
                        continue;
                    }

                    if let Ok(sibling_src) = fs::read_to_string(&path) {
                        if let Ok(sibling_tree) = ParserManager::parse_source(&sibling_src, &ts_lang, &path) {
                            if let Some(extracted) = find_class_or_type_in_file(sibling_tree.root_node(), &sibling_src, &type_name, &path) {
                                if depth < opts.depth {
                                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, &path) {
                                        for id in AstUtils::find_descendants_by_kind(tree.root_node(), "identifier") {
                                            let name = AstUtils::node_text(id, &extracted.definition);
                                            if !is_builtin_python_type(name) && visited.insert(name.to_string()) {
                                                queue.push_back((name.to_string(), depth + 1));
                                            }
                                        }
                                        for str_node in AstUtils::find_descendants_by_kind(tree.root_node(), "string") {
                                            let text = AstUtils::node_text(str_node, &extracted.definition).trim_matches('"').trim_matches('\'');
                                            if !is_builtin_python_type(text) && visited.insert(text.to_string()) {
                                                queue.push_back((text.to_string(), depth + 1));
                                            }
                                        }
                                    }
                                }
                                hoisted.push(extracted);
                                found = true;
                                break;
                            }
                        }
                    }
                }
                if found {
                    continue;
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

fn find_class_or_type_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = unwrap_decorated(child);
        if decl.kind() == "class_definition" {
            if let Some(name_node) = decl.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == target_name {
                    return Some(ExtractedType {
                        name: target_name.to_string(),
                        kind: "class".to_string(),
                        file_path: file_path.to_string_lossy().to_string(),
                        definition: AstUtils::node_text(child, source).to_string(),
                    });
                }
            }
        } else if decl.kind() == "expression_statement" {
            // e.g. TypeAlias = str | int
            let text = AstUtils::node_text(decl, source);
            if let Some((left, _)) = text.split_once('=') {
                if left.trim() == target_name {
                    return Some(ExtractedType {
                        name: target_name.to_string(),
                        kind: "type".to_string(),
                        file_path: file_path.to_string_lossy().to_string(),
                        definition: text.to_string(),
                    });
                }
            }
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
    let mut doc_comment = AstUtils::extract_doc_comment(full_node, source);
    if doc_comment.is_none() {
        if let Some(body_node) = decl.child_by_field_name("body") {
            if let Some(first_stmt) = body_node.named_children(&mut body_node.walk()).next() {
                if first_stmt.kind() == "expression_statement" {
                    if let Some(str_node) = first_stmt.named_children(&mut first_stmt.walk()).next() {
                        if str_node.kind() == "string" {
                            let text = AstUtils::node_text(str_node, source).trim();
                            let unquoted = if let Some(s) = text.strip_prefix("\"\"\"").and_then(|s| s.strip_suffix("\"\"\"")) {
                                s.trim()
                            } else if let Some(s) = text.strip_prefix("'''").and_then(|s| s.strip_suffix("'''")) {
                                s.trim()
                            } else if let Some(s) = text.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                                s.trim()
                            } else if let Some(s) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
                                s.trim()
                            } else {
                                text
                            };
                            doc_comment = Some(unquoted.to_string());
                        }
                    }
                }
            }
        }
    }
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
            | "self"
            | "cls"
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
