//! LanguageAdapter implementation for Rust.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
use tree_sitter::{Language, Node};
use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage};
use crate::parser::{AstUtils, ParserManager};

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

                    let base_type = type_name.split('<').next().unwrap_or(&type_name).trim();

                    if let Some(body) = child.child_by_field_name("body") {
                        for item in body.named_children(&mut body.walk()) {
                            if item.kind() == "function_item" {
                                if let Some(m_name) = item.child_by_field_name("name") {
                                    let method_name = AstUtils::node_text(m_name, source);
                                    symbols.push(format!("{base_type}::{method_name}"));
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

        // Collect scoped generic parameter identifiers
        let scoped_generics = collect_rust_scoped_generics(target_node, source);

        // 1. If target node is a method inside an impl block, hoist the enclosing struct/type first!
        if let Some(parent) = target_node.parent() {
            if parent.kind() == "declaration_list" {
                if let Some(impl_node) = parent.parent() {
                    if impl_node.kind() == "impl_item" {
                        if let Some(t_node) = impl_node.child_by_field_name("type") {
                            let raw_type = AstUtils::node_text(t_node, source);
                            let type_name = raw_type.split('<').next().unwrap_or(raw_type).trim();
                            if !is_builtin_rust_type(type_name) && !scoped_generics.contains(type_name) && visited.insert(type_name.to_string()) {
                                queue.push_back((type_name.to_string(), 1));
                            }
                        }
                    }
                }
            }
        }

        // 2. Collect type identifiers in target node
        for id in AstUtils::find_descendants_by_kind(target_node, "type_identifier") {
            let name = AstUtils::node_text(id, source);
            if !is_builtin_rust_type(name) && !scoped_generics.contains(name) && visited.insert(name.to_string()) {
                queue.push_back((name.to_string(), 1));
            }
        }

        let dir = file_path.parent().unwrap_or_else(|| Path::new("."));
        let ts_lang = self.tree_sitter_language(file_path);

        while let Some((type_name, depth)) = queue.pop_front() {
            if is_builtin_rust_type(&type_name) || scoped_generics.contains(&type_name) {
                continue;
            }

            // A. Check local file
            if let Some(extracted) = find_rust_type_in_file(root, source, &type_name, file_path) {
                if depth < opts.depth {
                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, file_path) {
                        let def_generics = collect_rust_scoped_generics(tree.root_node(), &extracted.definition);
                        for id in AstUtils::find_descendants_by_kind(tree.root_node(), "type_identifier") {
                            let name = AstUtils::node_text(id, &extracted.definition);
                            if !is_builtin_rust_type(name) && !def_generics.contains(name) && visited.insert(name.to_string()) {
                                queue.push_back((name.to_string(), depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            // B. Check sibling .rs files in the same directory/crate
            if let Ok(entries) = fs::read_dir(dir) {
                let mut found = false;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path == file_path || path.extension().and_then(|e| e.to_str()) != Some("rs") {
                        continue;
                    }

                    if let Ok(sibling_src) = fs::read_to_string(&path) {
                        if let Ok(sibling_tree) = ParserManager::parse_source(&sibling_src, &ts_lang, &path) {
                            if let Some(extracted) = find_rust_type_in_file(sibling_tree.root_node(), &sibling_src, &type_name, &path) {
                                if depth < opts.depth {
                                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, &path) {
                                        let def_generics = collect_rust_scoped_generics(tree.root_node(), &extracted.definition);
                                        for id in AstUtils::find_descendants_by_kind(tree.root_node(), "type_identifier") {
                                            let name = AstUtils::node_text(id, &extracted.definition);
                                            if !is_builtin_rust_type(name) && !def_generics.contains(name) && visited.insert(name.to_string()) {
                                                queue.push_back((name.to_string(), depth + 1));
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

        let call_nodes = AstUtils::find_descendants_by_kind(target_node, "call_expression");
        for call in call_nodes {
            if let Some(func_node) = call.child_by_field_name("function") {
                let call_text = AstUtils::node_text(func_node, source);
                let call_name = if func_node.kind() == "field_expression" {
                    if let Some(field) = func_node.child_by_field_name("field") {
                        AstUtils::node_text(field, source)
                    } else {
                        call_text.split('.').last().unwrap_or(call_text)
                    }
                } else {
                    call_text.split("::").last().unwrap_or(call_text)
                };

                if is_builtin_rust_method(call_name) || !seen.insert(call_name.to_string()) {
                    continue;
                }

                if let Some(sig) = find_rust_signature(root, source, call_name) {
                    stubs.push(CallSignatureStub {
                        name: call_name.to_string(),
                        receiver: None,
                        file_path: Some(file_path.to_string_lossy().to_string()),
                        signature: sig,
                    });
                } else {
                    let dir = file_path.parent().unwrap_or_else(|| Path::new("."));
                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path == file_path || path.extension().and_then(|e| e.to_str()) != Some("rs") {
                                continue;
                            }
                            if let Ok(sibling_src) = fs::read_to_string(&path) {
                                let ts_lang = self.tree_sitter_language(&path);
                                if let Ok(tree) = ParserManager::parse_source(&sibling_src, &ts_lang, &path) {
                                    if let Some(sig) = find_rust_signature(tree.root_node(), &sibling_src, call_name) {
                                        stubs.push(CallSignatureStub {
                                            name: call_name.to_string(),
                                            receiver: None,
                                            file_path: Some(path.to_string_lossy().to_string()),
                                            signature: sig,
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // If not found in module or sibling files, do not emit dummy stubs for standard library/builtins
                }
            }
        }

        Ok(stubs)
    }
}

fn collect_rust_scoped_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut generics = HashSet::new();
    for tp in AstUtils::find_descendants_by_kind(node, "type_parameters") {
        for child in tp.named_children(&mut tp.walk()) {
            if child.kind() == "type_identifier" {
                generics.insert(AstUtils::node_text(child, source).to_string());
            } else if child.kind() == "constrained_type_parameter" {
                if let Some(left) = child.child_by_field_name("left").or_else(|| child.named_child(0)) {
                    generics.insert(AstUtils::node_text(left, source).to_string());
                }
            }
        }
    }
    generics
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

fn find_rust_type_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if matches!(child.kind(), "struct_item" | "enum_item" | "trait_item" | "type_item") {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == target_name {
                    return Some(ExtractedType {
                        name: target_name.to_string(),
                        kind: child.kind().replace("_item", ""),
                        file_path: file_path.to_string_lossy().to_string(),
                        definition: AstUtils::node_text(child, source).to_string(),
                    });
                }
            }
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
    let clean_impl = impl_type.split('<').next().unwrap_or(impl_type).trim();

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "impl_item" {
            if let Some(t_node) = child.child_by_field_name("type") {
                let t_text = AstUtils::node_text(t_node, source);
                let base_t = t_text.split('<').next().unwrap_or(t_text).trim();

                if base_t == clean_impl || t_text == impl_type {
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
            let sig = source[start..sig_end].trim().to_string();
            if !sig.ends_with(';') {
                return format!("{sig};");
            }
            return sig;
        }
    }
    let text = AstUtils::node_text(node, source).trim().to_string();
    if !text.ends_with(';') {
        return format!("{text};");
    }
    text
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
        } else if child.kind() == "impl_item" {
            if let Some(body) = child.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    if member.kind() == "function_item" {
                        if let Some(name_node) = member.child_by_field_name("name") {
                            if AstUtils::node_text(name_node, source) == func_name {
                                return Some(extract_rust_sig(member, source));
                            }
                        }
                    }
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
            | "Rc" | "Path" | "PathBuf" | "Self" | "self" | "Send" | "Sync" | "Clone" | "Copy" | "Debug"
            | "Display" | "Default" | "Error" | "AsRef" | "AsMut" | "From" | "Into" | "Fn" | "FnMut" | "FnOnce"
    )
}

fn is_builtin_rust_method(name: &str) -> bool {
    matches!(
        name,
        "unwrap_or"
            | "unwrap_or_default"
            | "unwrap_or_else"
            | "is_ok"
            | "is_err"
            | "as_ref"
            | "as_mut"
            | "is_empty"
            | "take"
            | "clone"
            | "to_string"
            | "to_str"
            | "as_str"
            | "as_bytes"
            | "unwrap"
            | "expect"
            | "is_some"
            | "is_none"
            | "len"
            | "push"
            | "pop"
            | "insert"
            | "remove"
            | "contains"
            | "get"
            | "map"
            | "and_then"
            | "iter"
            | "into_iter"
            | "collect"
            | "ok"
            | "err"
            | "into"
            | "from"
            | "default"
            | "new"
    )
}
