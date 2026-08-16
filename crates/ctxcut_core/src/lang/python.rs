//! LanguageAdapter implementation for Python (.py, .pyi).

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
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
            let decl = unwrap_decorated(child);
            match decl.kind() {
                "function_definition" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        symbols.push(AstUtils::node_text(name_node, source).to_string());
                    }
                }
                "class_definition" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let class_name = AstUtils::node_text(name_node, source);
                        symbols.push(class_name.to_string());

                        if let Some(body) = decl.child_by_field_name("body") {
                            for member in body.named_children(&mut body.walk()) {
                                let m_decl = unwrap_decorated(member);
                                if m_decl.kind() == "function_definition" {
                                    if let Some(m_name) = m_decl.child_by_field_name("name") {
                                        let member_name = AstUtils::node_text(m_name, source);
                                        symbols.push(format!("{class_name}.{member_name}"));
                                    }
                                }
                            }
                        }
                    }
                }
                "type_alias_statement" => {
                    if let Some(left) = decl.child_by_field_name("left") {
                        let alias_name = AstUtils::node_text(left, source).trim();
                        let clean_name = alias_name.split('[').next().unwrap_or(alias_name).trim();
                        if !clean_name.is_empty() {
                            symbols.push(clean_name.to_string());
                        }
                    }
                }
                "assignment" => {
                    if let Some(left) = decl.child_by_field_name("left") {
                        let name = AstUtils::node_text(left, source).trim();
                        if is_type_or_constant_assignment(decl, source) {
                            symbols.push(name.to_string());
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

        // 1. Collect scoped generic parameters (e.g. [T, U] in PEP 695 or TypeVar)
        let scoped_generics = collect_scoped_generics(target_node, source);

        // 2. Extract initial referenced type names from target node
        let initial_types = extract_referenced_type_names(target_node, source, &scoped_generics);
        for t in initial_types {
            if visited.insert(t.clone()) {
                queue.push_back((t, 1usize));
            }
        }

        // 3. Cache imports from the root file for cross-file resolution
        let imports = extract_python_imports(root, source);

        // 4. BFS queue traversal with cycle protection
        while let Some((type_name, depth)) = queue.pop_front() {
            // A. Check local file first
            if let Some(extracted) = find_local_type(root, source, &type_name, file_path) {
                // If depth < opts.depth, parse referenced types in this definition
                if depth < opts.depth {
                    let ts_lang = self.tree_sitter_language(file_path);
                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, file_path) {
                        let inner_refs = extract_referenced_type_names(tree.root_node(), &extracted.definition, &scoped_generics);
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

            // B. Check cross-file imports if available
            if let Some(import_info) = imports.get(&type_name) {
                if let Some(resolved_path) = resolve_python_module_path(file_path, &import_info.specifier) {
                    if let Ok(imported_source) = fs::read_to_string(&resolved_path) {
                        let ts_lang = self.tree_sitter_language(&resolved_path);
                        if let Ok(imported_tree) = ParserManager::parse_source(&imported_source, &ts_lang, &resolved_path) {
                            let imported_root = imported_tree.root_node();
                            let lookup_name = if import_info.imported_name == "*" {
                                &type_name
                            } else {
                                &import_info.imported_name
                            };

                            if let Some(extracted) = find_local_type(imported_root, &imported_source, lookup_name, &resolved_path) {
                                if depth < opts.depth {
                                    let inner_refs = extract_referenced_type_names(imported_root, &extracted.definition, &scoped_generics);
                                    for inner in inner_refs {
                                        if visited.insert(inner.clone()) {
                                            queue.push_back((inner, depth + 1));
                                        }
                                    }
                                }
                                hoisted.push(extracted);
                            }
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

        let imports = extract_python_imports(root, source);
        let call_nodes = AstUtils::find_descendants_by_kind(target_node, "call");

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

            if is_builtin_python_func(call_name) || !seen.insert(call_name.to_string()) {
                continue;
            }

            // 1. Check if defined in current file
            if let Some(sig) = find_python_signature(root, source, call_name) {
                stubs.push(CallSignatureStub {
                    name: call_name.to_string(),
                    receiver,
                    file_path: Some(file_path.to_string_lossy().to_string()),
                    signature: sig,
                });
                continue;
            }

            // 2. Check if imported from another module
            if let Some(import_info) = imports.get(call_name) {
                if let Some(resolved_path) = resolve_python_module_path(file_path, &import_info.specifier) {
                    if let Ok(imported_source) = fs::read_to_string(&resolved_path) {
                        let ts_lang = self.tree_sitter_language(&resolved_path);
                        if let Ok(tree) = ParserManager::parse_source(&imported_source, &ts_lang, &resolved_path) {
                            let lookup_name = if import_info.imported_name == "*" {
                                call_name
                            } else {
                                &import_info.imported_name
                            };
                            if let Some(sig) = find_python_signature(tree.root_node(), &imported_source, lookup_name) {
                                stubs.push(CallSignatureStub {
                                    name: call_name.to_string(),
                                    receiver,
                                    file_path: Some(resolved_path.to_string_lossy().to_string()),
                                    signature: sig,
                                });
                                continue;
                            }
                        }
                    }
                }
            }

            // 3. Fallback stub if it's a method on a receiver (e.g. self.gateway.authorize_charge)
            if receiver.is_some() {
                stubs.push(CallSignatureStub {
                    name: call_name.to_string(),
                    receiver,
                    file_path: None,
                    signature: format!("def {call_name}(*args: Any, **kwargs: Any) -> Any: ..."),
                });
            }
        }

        Ok(stubs)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
            "type_alias_statement" => {
                if let Some(left) = decl.child_by_field_name("left") {
                    let alias_text = AstUtils::node_text(left, source).trim();
                    let clean_name = alias_text.split('[').next().unwrap_or(alias_text).trim();
                    if clean_name == target_name {
                        return Some((build_extracted_symbol(full_node, decl, source, file_path, "type"), full_node));
                    }
                }
            }
            "assignment" => {
                if let Some(left) = decl.child_by_field_name("left") {
                    let name = AstUtils::node_text(left, source).trim();
                    if name == target_name {
                        return Some((build_extracted_symbol(full_node, decl, source, file_path, "type"), full_node));
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
        .or_else(|| decl.child_by_field_name("left"))
        .map_or_else(
            || "anonymous".to_string(),
            |n| {
                let t = AstUtils::node_text(n, source).trim();
                t.split('[').next().unwrap_or(t).trim().to_string()
            },
        );

    let body = AstUtils::node_text(full_node, source).to_string();
    let doc_comment = extract_python_doc_comment(full_node, decl, source);
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

/// Extracts docstrings from Python functions and classes:
/// 1. In-body docstring: first statement inside `body` (`block`) as `expression_statement -> string`.
/// 2. Preceding `#` comments as fallback.
fn extract_python_doc_comment(full_node: Node<'_>, decl: Node<'_>, source: &str) -> Option<String> {
    if let Some(body) = decl.child_by_field_name("body") {
        if let Some(first_stmt) = body.named_child(0) {
            if first_stmt.kind() == "expression_statement" {
                if let Some(str_node) = first_stmt.named_child(0) {
                    if matches!(str_node.kind(), "string" | "concatenated_string") {
                        let raw = AstUtils::node_text(str_node, source).trim();
                        let stripped = strip_python_string_quotes(raw);
                        if !stripped.is_empty() {
                            return Some(stripped);
                        }
                    }
                }
            }
        }
    }

    AstUtils::extract_doc_comment(full_node, source)
}

fn strip_python_string_quotes(raw: &str) -> String {
    let s = raw.trim();
    // Strip prefix like r, f, b, u, rf, fr
    let trimmed_prefix = s.trim_start_matches(['r', 'R', 'f', 'F', 'b', 'B', 'u', 'U']);

    if trimmed_prefix.starts_with("\"\"\"") && trimmed_prefix.ends_with("\"\"\"") && trimmed_prefix.len() >= 6 {
        return trimmed_prefix[3..trimmed_prefix.len() - 3].trim().to_string();
    }
    if trimmed_prefix.starts_with("'''") && trimmed_prefix.ends_with("'''") && trimmed_prefix.len() >= 6 {
        return trimmed_prefix[3..trimmed_prefix.len() - 3].trim().to_string();
    }
    if trimmed_prefix.starts_with('"') && trimmed_prefix.ends_with('"') && trimmed_prefix.len() >= 2 {
        return trimmed_prefix[1..trimmed_prefix.len() - 1].trim().to_string();
    }
    if trimmed_prefix.starts_with('\'') && trimmed_prefix.ends_with('\'') && trimmed_prefix.len() >= 2 {
        return trimmed_prefix[1..trimmed_prefix.len() - 1].trim().to_string();
    }
    trimmed_prefix.trim().to_string()
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
    AstUtils::node_text(decl, source).lines().next().unwrap_or("").trim().to_string()
}

fn find_python_signature(root: Node<'_>, source: &str, func_name: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = unwrap_decorated(child);
        if decl.kind() == "function_definition" {
            if let Some(name_node) = decl.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == func_name {
                    let sig = extract_python_sig(decl, source);
                    return Some(format!("{sig}: ..."));
                }
            }
        } else if decl.kind() == "class_definition" {
            if let Some(body) = decl.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    let m_decl = unwrap_decorated(member);
                    if m_decl.kind() == "function_definition" {
                        if let Some(name_node) = m_decl.child_by_field_name("name") {
                            if AstUtils::node_text(name_node, source) == func_name {
                                let sig = extract_python_sig(m_decl, source);
                                return Some(format!("{sig}: ..."));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn collect_scoped_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut generics = HashSet::new();
    if let Some(type_params) = node.child_by_field_name("type_parameters") {
        for id in AstUtils::find_descendants_by_kind(type_params, "identifier") {
            generics.insert(AstUtils::node_text(id, source).to_string());
        }
        for id in AstUtils::find_descendants_by_kind(type_params, "type_parameter") {
            let txt = AstUtils::node_text(id, source).trim();
            let name = txt.split(':').next().unwrap_or(txt).trim();
            if !name.is_empty() {
                generics.insert(name.to_string());
            }
        }
    }
    generics
}

fn extract_referenced_type_names(node: Node<'_>, source: &str, scoped_generics: &HashSet<String>) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();

    // 1. All identifiers within `type` AST nodes
    for t_node in AstUtils::find_descendants_by_kind(node, "type") {
        for id in AstUtils::find_descendants_by_kind(t_node, "identifier") {
            let name = AstUtils::node_text(id, source).trim();
            if is_valid_custom_type(name, scoped_generics) && seen.insert(name.to_string()) {
                names.push(name.to_string());
            }
        }
    }

    // 2. Return type annotations
    if let Some(ret_type) = node.child_by_field_name("return_type") {
        for id in AstUtils::find_descendants_by_kind(ret_type, "identifier") {
            let name = AstUtils::node_text(id, source).trim();
            if is_valid_custom_type(name, scoped_generics) && seen.insert(name.to_string()) {
                names.push(name.to_string());
            }
        }
    }

    // 3. Capitalized identifiers in annotations or expressions
    for id in AstUtils::find_descendants_by_kind(node, "identifier") {
        let name = AstUtils::node_text(id, source).trim();
        if name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && is_valid_custom_type(name, scoped_generics)
            && seen.insert(name.to_string())
        {
            names.push(name.to_string());
        }
    }

    names
}

fn is_valid_custom_type(name: &str, scoped_generics: &HashSet<String>) -> bool {
    !name.is_empty()
        && !scoped_generics.contains(name)
        && !is_builtin_python_type(name)
}

fn find_local_type(root: Node<'_>, source: &str, type_name: &str, file_path: &Path) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = unwrap_decorated(child);
        match decl.kind() {
            "class_definition" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == type_name {
                        return Some(ExtractedType {
                            name: type_name.to_string(),
                            kind: "class".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).to_string(),
                        });
                    }
                }
            }
            "type_alias_statement" => {
                if let Some(left) = decl.child_by_field_name("left") {
                    let alias_text = AstUtils::node_text(left, source).trim();
                    let clean_name = alias_text.split('[').next().unwrap_or(alias_text).trim();
                    if clean_name == type_name {
                        return Some(ExtractedType {
                            name: type_name.to_string(),
                            kind: "type_alias".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).to_string(),
                        });
                    }
                }
            }
            "assignment" => {
                if let Some(left) = decl.child_by_field_name("left") {
                    let name = AstUtils::node_text(left, source).trim();
                    if name == type_name && is_type_or_constant_assignment(decl, source) {
                        return Some(ExtractedType {
                            name: type_name.to_string(),
                            kind: "type_alias".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn is_type_or_constant_assignment(node: Node<'_>, source: &str) -> bool {
    let text = AstUtils::node_text(node, source);
    text.contains("TypeVar")
        || text.contains("NewType")
        || text.contains("TypeAlias")
        || text.contains("NamedTuple")
        || node.child_by_field_name("type").is_some()
        || text.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

#[derive(Debug, Clone)]
struct PythonImport {
    imported_name: String,
    specifier: String,
}

fn extract_python_imports(root: Node<'_>, source: &str) -> std::collections::HashMap<String, PythonImport> {
    let mut map = std::collections::HashMap::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "import_statement" {
            // e.g. import os, import numpy as np
            for alias in AstUtils::find_descendants_by_kind(child, "aliased_import") {
                if let Some(name_node) = alias.child_by_field_name("name") {
                    let full_name = AstUtils::node_text(name_node, source).trim();
                    let alias_name = alias
                        .child_by_field_name("alias")
                        .map_or(full_name, |a| AstUtils::node_text(a, source).trim());
                    map.insert(
                        alias_name.to_string(),
                        PythonImport {
                            imported_name: full_name.to_string(),
                            specifier: full_name.to_string(),
                        },
                    );
                }
            }
            for dot_name in AstUtils::find_children_by_kind(child, "dotted_name") {
                let name = AstUtils::node_text(dot_name, source).trim();
                map.insert(
                    name.to_string(),
                    PythonImport {
                        imported_name: name.to_string(),
                        specifier: name.to_string(),
                    },
                );
            }
        } else if child.kind() == "import_from_statement" {
            // e.g. from .clients import BankingGatewayClient, FraudDetectionClient as FDC
            let module_spec = child
                .child_by_field_name("module_name")
                .map(|m| AstUtils::node_text(m, source).trim())
                .or_else(|| {
                    AstUtils::find_children_by_kind(child, "relative_import")
                        .first()
                        .map(|r| AstUtils::node_text(*r, source).trim())
                })
                .or_else(|| {
                    AstUtils::find_children_by_kind(child, "dotted_name")
                        .first()
                        .map(|d| AstUtils::node_text(*d, source).trim())
                })
                .unwrap_or("");

            if module_spec.is_empty() {
                continue;
            }

            // Check wildcard import: `from x import *`
            if !AstUtils::find_descendants_by_kind(child, "wildcard_import").is_empty()
                || child.children(&mut child.walk()).any(|c| c.kind() == "*")
            {
                map.insert(
                    "*".to_string(),
                    PythonImport {
                        imported_name: "*".to_string(),
                        specifier: module_spec.to_string(),
                    },
                );
            }

            for alias in AstUtils::find_descendants_by_kind(child, "aliased_import") {
                if let Some(name_node) = alias.child_by_field_name("name") {
                    let orig_name = AstUtils::node_text(name_node, source).trim();
                    let local_name = alias
                        .child_by_field_name("alias")
                        .map_or(orig_name, |a| AstUtils::node_text(a, source).trim());

                    map.insert(
                        local_name.to_string(),
                        PythonImport {
                            imported_name: orig_name.to_string(),
                            specifier: module_spec.to_string(),
                        },
                    );
                }
            }

            for dot_name in AstUtils::find_children_by_kind(child, "dotted_name") {
                let name = AstUtils::node_text(dot_name, source).trim();
                // Avoid capturing the module name itself if it was matched
                if name != module_spec {
                    map.insert(
                        name.to_string(),
                        PythonImport {
                            imported_name: name.to_string(),
                            specifier: module_spec.to_string(),
                        },
                    );
                }
            }
        }
    }

    map
}

/// Resolves a Python import specifier to its target file path on disk.
/// Resolves a Python relative or local module specifier (e.g. `.schemas`) to a file path on disk.
pub fn resolve_python_module_path(from_file: &Path, specifier: &str) -> Option<PathBuf> {
    let parent_dir = from_file.parent().unwrap_or_else(|| Path::new("."));

    if specifier.starts_with('.') {
        let dots_count = specifier.chars().take_while(|&c| c == '.').count();
        let rel_spec = &specifier[dots_count..];

        let mut base_dir = parent_dir.to_path_buf();
        for _ in 1..dots_count {
            if let Some(parent) = base_dir.parent() {
                base_dir = parent.to_path_buf();
            }
        }

        let rel_path = rel_spec.replace('.', "/");
        let candidate_base = if rel_path.is_empty() {
            base_dir
        } else {
            base_dir.join(rel_path)
        };

        // 1. Check <candidate_base>.py
        let py_file = candidate_base.with_extension("py");
        if py_file.is_file() {
            return Some(py_file);
        }

        // 2. Check <candidate_base>/__init__.py
        let init_file = candidate_base.join("__init__.py");
        if init_file.is_file() {
            return Some(init_file);
        }

        // 3. Check <candidate_base>.pyi
        let pyi_file = candidate_base.with_extension("pyi");
        if pyi_file.is_file() {
            return Some(pyi_file);
        }
    } else {
        // Absolute import relative to file's directory
        let rel_path = specifier.replace('.', "/");
        let py_file = parent_dir.join(&rel_path).with_extension("py");
        if py_file.is_file() {
            return Some(py_file);
        }
        let init_file = parent_dir.join(&rel_path).join("__init__.py");
        if init_file.is_file() {
            return Some(init_file);
        }
    }

    None
}

/// Returns true if the type name matches a Python built-in, exception, or typing construct.
pub fn is_builtin_python_type(name: &str) -> bool {
    matches!(
        name,
        // Primitives & Core
        "int" | "float" | "complex" | "str" | "bool" | "bytes" | "bytearray" | "memoryview"
        | "object" | "type" | "None" | "Ellipsis" | "NotImplemented" | "Any" | "self" | "cls"
        
        // Built-in Collections
        | "list" | "dict" | "set" | "frozenset" | "tuple" | "range" | "slice"

        // Built-in Exceptions
        | "BaseException" | "Exception" | "ValueError" | "TypeError" | "KeyError" | "IndexError"
        | "AttributeError" | "RuntimeError" | "IOError" | "OSError" | "StopIteration"
        | "StopAsyncIteration" | "SyntaxError" | "AssertionError" | "ImportError"

        // typing / typing_extensions
        | "Optional" | "Union" | "Generic" | "TypeVar" | "ParamSpec" | "TypeVarTuple"
        | "Concatenate" | "Literal" | "Final" | "ClassVar" | "Annotated" | "TypeAlias"
        | "Self" | "Never" | "NoReturn" | "Protocol" | "TypedDict" | "NamedTuple"
        | "NewType" | "Tuple" | "List" | "Dict" | "Set" | "FrozenSet" | "Sequence"
        | "Mapping" | "Iterable" | "Iterator" | "Generator" | "AsyncIterable" | "AsyncIterator"
        | "AsyncGenerator" | "Coroutine" | "Awaitable" | "Callable" | "Type" | "Pattern" | "Match"

        // Common Standard Framework / Library Base Types
        | "BaseModel" | "Enum" | "IntEnum" | "StrEnum" | "Flag" | "Field" | "field"
        | "Depends" | "Query" | "Path" | "Body" | "Header" | "Cookie" | "HTTPException"
        | "status" | "APIRouter" | "FastAPI" | "Response" | "Request" | "datetime" | "date"
        | "time" | "timedelta" | "UUID" | "Decimal"
    )
}

/// Returns true if the identifier matches a Python built-in function or common logger method.
pub fn is_builtin_python_func(name: &str) -> bool {
    matches!(
        name,
        "print" | "len" | "range" | "enumerate" | "isinstance" | "issubclass" | "round"
        | "str" | "int" | "float" | "dict" | "list" | "set" | "tuple" | "min" | "max"
        | "sum" | "any" | "all" | "zip" | "map" | "filter" | "open" | "id" | "repr"
        | "iter" | "next" | "hash" | "callable" | "hasattr" | "getattr" | "setattr"
        | "delattr" | "super" | "type" | "vars" | "dir" | "abs" | "pow" | "divmod"
        | "format" | "sorted" | "reversed" | "slice" | "cast" | "Field" | "field"
        | "field_validator" | "model_validator" | "validator" | "Depends" | "Query"
        | "info" | "warning" | "error" | "debug" | "critical" | "exception"
    )
}
