//! LanguageAdapter implementation for Go (.go).

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Node};
use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage};
use crate::parser::{AstUtils, ParserManager};

/// Go language adapter supporting Go (.go).
pub struct GoAdapter;

impl LanguageAdapter for GoAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Go
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        let (receiver_query, member_query) = parse_query(symbol_query);

        if let Some(receiver_name) = receiver_query {
            if let Some((sym, node)) = find_method_with_receiver(root, source, receiver_name, member_query, file_path) {
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
                "function_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        symbols.push(AstUtils::node_text(name_node, source).to_string());
                    }
                }
                "method_declaration" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let method_name = AstUtils::node_text(name_node, source);
                        let receiver_type = extract_receiver_type_name(child, source);
                        if let Some(rec) = receiver_type {
                            symbols.push(format!("{rec}.{method_name}"));
                        } else {
                            symbols.push(method_name.to_string());
                        }
                    }
                }
                "type_declaration" => {
                    for spec in AstUtils::find_children_by_kind(child, "type_spec") {
                        if let Some(name_node) = spec.child_by_field_name("name") {
                            symbols.push(AstUtils::node_text(name_node, source).to_string());
                        }
                    }
                    for alias in AstUtils::find_children_by_kind(child, "type_alias") {
                        if let Some(name_node) = alias.child_by_field_name("name") {
                            symbols.push(AstUtils::node_text(name_node, source).to_string());
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
        let mut queue = VecDeque::new();

        // 1. Scoped generic parameter filtering (e.g. [T any, R comparable])
        let scoped_generics = collect_go_generics(target_node, source);

        // 2. Extract initial referenced types from target signature and body
        let initial_types = extract_referenced_go_types(target_node, source, &scoped_generics);
        for t in initial_types {
            if visited.insert(t.clone()) {
                queue.push_back((t, 1usize));
            }
        }

        // 3. Find current package name and collect sibling package files
        let current_pkg = extract_package_name(root, source);
        let sibling_files = find_sibling_package_files(file_path, &current_pkg);

        // 4. BFS transitive resolution
        while let Some((type_name, depth)) = queue.pop_front() {
            // A. Check current file
            if let Some(extracted) = find_local_go_type(root, source, &type_name, file_path) {
                if depth < opts.depth {
                    let ts_lang = self.tree_sitter_language(file_path);
                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, file_path) {
                        let inner_refs = extract_referenced_go_types(tree.root_node(), &extracted.definition, &scoped_generics);
                        for inner in inner_refs {
                            if visited.insert(inner.clone()) {
                                queue.push_back((inner, depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            // B. Check sibling package files
            for sibling_path in &sibling_files {
                if let Ok(sibling_source) = fs::read_to_string(sibling_path) {
                    let ts_lang = self.tree_sitter_language(sibling_path);
                    if let Ok(tree) = ParserManager::parse_source(&sibling_source, &ts_lang, sibling_path) {
                        let sibling_root = tree.root_node();
                        if let Some(extracted) = find_local_go_type(sibling_root, &sibling_source, &type_name, sibling_path) {
                            if depth < opts.depth {
                                let inner_refs = extract_referenced_go_types(sibling_root, &extracted.definition, &scoped_generics);
                                for inner in inner_refs {
                                    if visited.insert(inner.clone()) {
                                        queue.push_back((inner, depth + 1));
                                    }
                                }
                            }
                            hoisted.push(extracted);
                            break;
                        }
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

        let current_pkg = extract_package_name(root, source);
        let sibling_files = find_sibling_package_files(file_path, &current_pkg);
        let call_nodes = AstUtils::find_descendants_by_kind(target_node, "call_expression");

        for call in call_nodes {
            let Some(func_node) = call.child_by_field_name("function") else {
                continue;
            };

            let call_text = AstUtils::node_text(func_node, source).trim();
            if call_text.is_empty() {
                continue;
            }

            let parts: Vec<&str> = call_text.split('.').map(str::trim).collect();
            let call_name = parts.last().copied().unwrap_or(call_text);
            let receiver = if parts.len() > 1 {
                Some(parts[..parts.len() - 1].join("."))
            } else {
                None
            };

            if is_builtin_go_func(call_name)
                || is_stdlib_package(parts.first().copied().unwrap_or(""))
                || !seen.insert(call_name.to_string())
            {
                continue;
            }

            // 1. Check current file
            if let Some(sig) = find_go_signature(root, source, call_name) {
                stubs.push(CallSignatureStub {
                    name: call_name.to_string(),
                    receiver,
                    file_path: Some(file_path.to_string_lossy().to_string()),
                    signature: sig,
                });
                continue;
            }

            // 2. Check sibling package files
            let mut found_sibling = false;
            for sibling_path in &sibling_files {
                if let Ok(sibling_source) = fs::read_to_string(sibling_path) {
                    let ts_lang = self.tree_sitter_language(sibling_path);
                    if let Ok(tree) = ParserManager::parse_source(&sibling_source, &ts_lang, sibling_path) {
                        if let Some(sig) = find_go_signature(tree.root_node(), &sibling_source, call_name) {
                            stubs.push(CallSignatureStub {
                                name: call_name.to_string(),
                                receiver: receiver.clone(),
                                file_path: Some(sibling_path.to_string_lossy().to_string()),
                                signature: sig,
                            });
                            found_sibling = true;
                            break;
                        }
                    }
                }
            }

            if found_sibling {
                continue;
            }

            // 3. Fallback stub if called on a receiver
            if receiver.is_some() {
                stubs.push(CallSignatureStub {
                    name: call_name.to_string(),
                    receiver,
                    file_path: None,
                    signature: format!("func {call_name}(...) interface{{}}"),
                });
            }
        }

        Ok(stubs)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_query(query: &str) -> (Option<&str>, &str) {
    let clean = query.trim_start_matches('*');
    if let Some((container, member)) = clean.split_once('.') {
        (Some(container.trim_start_matches('*').trim()), member.trim())
    } else {
        (None, clean.trim())
    }
}

fn extract_receiver_type_name(method_node: Node<'_>, source: &str) -> Option<String> {
    let receiver = method_node.child_by_field_name("receiver")?;
    for type_id in AstUtils::find_descendants_by_kind(receiver, "type_identifier") {
        let text = AstUtils::node_text(type_id, source).trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
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
            "function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some((build_go_symbol(child, source, file_path, "function"), child));
                    }
                }
            }
            "type_declaration" => {
                for spec in AstUtils::find_children_by_kind(child, "type_spec") {
                    if let Some(name_node) = spec.child_by_field_name("name") {
                        if AstUtils::node_text(name_node, source) == target_name {
                            return Some((build_go_symbol(child, source, file_path, "type"), child));
                        }
                    }
                }
                for alias in AstUtils::find_children_by_kind(child, "type_alias") {
                    if let Some(name_node) = alias.child_by_field_name("name") {
                        if AstUtils::node_text(name_node, source) == target_name {
                            return Some((build_go_symbol(child, source, file_path, "type"), child));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_method_with_receiver<'a>(
    root: Node<'a>,
    source: &'a str,
    receiver_name: &str,
    method_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let clean_receiver = receiver_name.trim_start_matches('*').trim();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "method_declaration" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == method_name {
                    if let Some(rec) = extract_receiver_type_name(child, source) {
                        if rec == clean_receiver {
                            return Some((build_go_symbol(child, source, file_path, "method"), child));
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
        if child.kind() == "method_declaration" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == method_name {
                    return Some((build_go_symbol(child, source, file_path, "method"), child));
                }
            }
        }
    }
    None
}

fn build_go_symbol(node: Node<'_>, source: &str, file_path: &Path, kind: &str) -> ExtractedSymbol {
    let name = node
        .child_by_field_name("name")
        .map(|n| AstUtils::node_text(n, source).to_string())
        .or_else(|| {
            if node.kind() == "type_declaration" {
                for spec in AstUtils::find_children_by_kind(node, "type_spec") {
                    if let Some(n) = spec.child_by_field_name("name") {
                        return Some(AstUtils::node_text(n, source).to_string());
                    }
                }
                for alias in AstUtils::find_children_by_kind(node, "type_alias") {
                    if let Some(n) = alias.child_by_field_name("name") {
                        return Some(AstUtils::node_text(n, source).to_string());
                    }
                }
            }
            None
        })
        .unwrap_or_else(|| "anonymous".to_string());

    let body = AstUtils::node_text(node, source).to_string();
    let doc_comment = AstUtils::extract_doc_comment(node, source);
    let signature = extract_go_sig(node, source);

    ExtractedSymbol {
        name,
        kind: kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        doc_comment,
        signature,
        body,
        language: "go".to_string(),
    }
}

fn extract_go_sig(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let sig_end = body.start_byte();
        let start = node.start_byte();
        if start < sig_end && sig_end <= source.len() {
            return source[start..sig_end].trim().to_string();
        }
    }
    AstUtils::node_text(node, source).lines().next().unwrap_or("").trim().to_string()
}

fn find_go_signature(root: Node<'_>, source: &str, func_name: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if matches!(child.kind(), "function_declaration" | "method_declaration") {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == func_name {
                    return Some(extract_go_sig(child, source));
                }
            }
        } else if child.kind() == "type_declaration" {
            // Check interface method specifications
            for spec in AstUtils::find_children_by_kind(child, "type_spec") {
                for iface in AstUtils::find_descendants_by_kind(spec, "interface_type") {
                    for m_spec in AstUtils::find_descendants_by_kind(iface, "method_spec") {
                        if let Some(m_name) = m_spec.child_by_field_name("name") {
                            if AstUtils::node_text(m_name, source) == func_name {
                                let sig = AstUtils::node_text(m_spec, source).trim();
                                return Some(format!("func {sig}"));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn collect_go_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut generics = HashSet::new();
    if let Some(type_params) = node.child_by_field_name("type_parameters") {
        for id in AstUtils::find_descendants_by_kind(type_params, "type_identifier") {
            generics.insert(AstUtils::node_text(id, source).to_string());
        }
        for id in AstUtils::find_descendants_by_kind(type_params, "identifier") {
            generics.insert(AstUtils::node_text(id, source).to_string());
        }
    }
    generics
}

fn extract_referenced_go_types(node: Node<'_>, source: &str, scoped_generics: &HashSet<String>) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();

    for id in AstUtils::find_descendants_by_kind(node, "type_identifier") {
        let name = AstUtils::node_text(id, source).trim();
        if is_valid_custom_go_type(name, scoped_generics) && seen.insert(name.to_string()) {
            names.push(name.to_string());
        }
    }

    names
}

fn is_valid_custom_go_type(name: &str, scoped_generics: &HashSet<String>) -> bool {
    !name.is_empty()
        && !scoped_generics.contains(name)
        && !is_builtin_go_type(name)
}

fn find_local_go_type(root: Node<'_>, source: &str, type_name: &str, file_path: &Path) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "type_declaration" {
            for spec in AstUtils::find_children_by_kind(child, "type_spec") {
                if let Some(name_node) = spec.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == type_name {
                        return Some(ExtractedType {
                            name: type_name.to_string(),
                            kind: "type".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).to_string(),
                        });
                    }
                }
            }
            for alias in AstUtils::find_children_by_kind(child, "type_alias") {
                if let Some(name_node) = alias.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == type_name {
                        return Some(ExtractedType {
                            name: type_name.to_string(),
                            kind: "type_alias".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).to_string(),
                        });
                    }
                }
            }
        }
    }
    None
}

fn extract_package_name(root: Node<'_>, source: &str) -> String {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_clause" {
            if let Some(pkg_id) = AstUtils::find_descendants_by_kind(child, "package_identifier").first() {
                return AstUtils::node_text(*pkg_id, source).trim().to_string();
            }
        }
    }
    "main".to_string()
}

fn find_sibling_package_files(file_path: &Path, expected_pkg: &str) -> Vec<PathBuf> {
    let mut siblings = Vec::new();
    let parent_dir = match file_path.parent() {
        Some(p) => p,
        None => return siblings,
    };

    let entries = match fs::read_dir(parent_dir) {
        Ok(e) => e,
        Err(_) => return siblings,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path != file_path
            && path.extension().and_then(|e| e.to_str()) == Some("go")
            && !path.file_name().and_then(|n| n.to_str()).unwrap_or("").ends_with("_test.go")
        {
            if let Ok(src) = fs::read_to_string(&path) {
                if let Ok(tree) = ParserManager::parse_source(&src, &tree_sitter_go::LANGUAGE.into(), &path) {
                    let pkg = extract_package_name(tree.root_node(), &src);
                    if pkg == expected_pkg {
                        siblings.push(path);
                    }
                }
            }
        }
    }

    siblings
}

/// Checks if a type name is a Go built-in primitive or standard type.
pub fn is_builtin_go_type(name: &str) -> bool {
    matches!(
        name,
        "string"
            | "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "uintptr"
            | "byte"
            | "rune"
            | "float32"
            | "float64"
            | "complex64"
            | "complex128"
            | "bool"
            | "error"
            | "any"
            | "comparable"
    )
}

/// Checks if a function name is a Go built-in function.
pub fn is_builtin_go_func(name: &str) -> bool {
    matches!(
        name,
        "make" | "new" | "len" | "cap" | "append" | "copy" | "close" | "delete" | "panic" | "recover" | "print" | "println"
    )
}

fn is_stdlib_package(name: &str) -> bool {
    matches!(
        name,
        "fmt" | "time" | "errors" | "context" | "strings" | "math" | "rand" | "sha256" | "hex" | "sync" | "os" | "io" | "http"
    )
}
