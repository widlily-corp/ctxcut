//! LanguageAdapter implementation for Rust (.rs).

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
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
                    let base_type = extract_impl_type_name(child, source).unwrap_or_else(|| "impl".to_string());

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
        let mut queue = VecDeque::new();

        // 1. Collect scoped generic parameters (e.g. <T, K, V>)
        let scoped_generics = collect_rust_generics(target_node, source);

        // 2. Extract initial referenced types
        let initial_types = extract_referenced_rust_types(target_node, source, &scoped_generics);
        for t in initial_types {
            if visited.insert(t.clone()) {
                queue.push_back((t, 1usize));
            }
        }

        // 3. Sibling module candidate files (e.g. models.rs, external.rs)
        let sibling_modules = find_sibling_rust_modules(file_path);

        // 4. BFS transitive resolution
        while let Some((type_name, depth)) = queue.pop_front() {
            // A. Check current file
            if let Some(extracted) = find_local_rust_type(root, source, &type_name, file_path) {
                if depth < opts.depth {
                    let ts_lang = self.tree_sitter_language(file_path);
                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, file_path) {
                        let inner_refs = extract_referenced_rust_types(tree.root_node(), &extracted.definition, &scoped_generics);
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

            // B. Check sibling modules
            for mod_path in &sibling_modules {
                if let Ok(mod_source) = fs::read_to_string(mod_path) {
                    let ts_lang = self.tree_sitter_language(mod_path);
                    if let Ok(tree) = ParserManager::parse_source(&mod_source, &ts_lang, mod_path) {
                        let mod_root = tree.root_node();
                        if let Some(extracted) = find_local_rust_type(mod_root, &mod_source, &type_name, mod_path) {
                            if depth < opts.depth {
                                let inner_refs = extract_referenced_rust_types(mod_root, &extracted.definition, &scoped_generics);
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

        let sibling_modules = find_sibling_rust_modules(file_path);
        let call_nodes = AstUtils::find_descendants_by_kind(target_node, "call_expression");

        for call in call_nodes {
            let Some(func_node) = call.child_by_field_name("function") else {
                continue;
            };

            let call_text = AstUtils::node_text(func_node, source).trim();
            if call_text.is_empty() {
                continue;
            }

            let parts: Vec<&str> = if call_text.contains("::") {
                call_text.split("::").map(str::trim).collect()
            } else {
                call_text.split('.').map(str::trim).collect()
            };

            let call_name = parts.last().copied().unwrap_or(call_text);
            let receiver = if parts.len() > 1 {
                Some(parts[..parts.len() - 1].join("::"))
            } else {
                None
            };

            if is_builtin_rust_method(call_name) || !seen.insert(call_name.to_string()) {
                continue;
            }

            // 1. Check current file
            if let Some(sig) = find_rust_signature(root, source, call_name) {
                stubs.push(CallSignatureStub {
                    name: call_name.to_string(),
                    receiver,
                    file_path: Some(file_path.to_string_lossy().to_string()),
                    signature: sig,
                });
                continue;
            }

            // 2. Check sibling modules
            let mut found_sibling = false;
            for mod_path in &sibling_modules {
                if let Ok(mod_source) = fs::read_to_string(mod_path) {
                    let ts_lang = self.tree_sitter_language(mod_path);
                    if let Ok(tree) = ParserManager::parse_source(&mod_source, &ts_lang, mod_path) {
                        if let Some(sig) = find_rust_signature(tree.root_node(), &mod_source, call_name) {
                            stubs.push(CallSignatureStub {
                                name: call_name.to_string(),
                                receiver: receiver.clone(),
                                file_path: Some(mod_path.to_string_lossy().to_string()),
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
                    signature: format!("pub fn {call_name}(&self, ...);"),
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
    if let Some((container, member)) = query.split_once("::") {
        (Some(container.trim()), member.trim())
    } else if let Some((container, member)) = query.split_once('.') {
        (Some(container.trim()), member.trim())
    } else {
        (None, query.trim())
    }
}

fn extract_impl_type_name(impl_node: Node<'_>, source: &str) -> Option<String> {
    let type_node = impl_node.child_by_field_name("type")?;
    if type_node.kind() == "generic_type" {
        if let Some(type_id) = type_node.child_by_field_name("type").or_else(|| type_node.named_child(0)) {
            return Some(AstUtils::node_text(type_id, source).trim().to_string());
        }
    }
    if type_node.kind() == "type_identifier" {
        return Some(AstUtils::node_text(type_node, source).trim().to_string());
    }
    for id in AstUtils::find_descendants_by_kind(type_node, "type_identifier") {
        let t = AstUtils::node_text(id, source).trim();
        if !t.is_empty() {
            return Some(t.to_string());
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
            "trait_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some((build_rust_symbol(child, source, file_path, "trait"), child));
                    }
                }
            }
            "type_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some((build_rust_symbol(child, source, file_path, "type"), child));
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
    let clean_impl = impl_type.split('<').next().unwrap_or(impl_type).trim();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "impl_item" {
            if let Some(type_name) = extract_impl_type_name(child, source) {
                if type_name == clean_impl {
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
        if child.kind() == "impl_item" || child.kind() == "trait_item" {
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
            return sig;
        }
    }
    let full = AstUtils::node_text(node, source).trim();
    if full.ends_with(';') {
        full.strip_suffix(';').unwrap_or(full).trim().to_string()
    } else {
        full.lines().next().unwrap_or("").trim().to_string()
    }
}

fn find_rust_signature(root: Node<'_>, source: &str, func_name: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "function_item" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == func_name {
                    let sig = extract_rust_sig(child, source);
                    return Some(format_rust_stub(&sig));
                }
            }
        } else if child.kind() == "impl_item" || child.kind() == "trait_item" {
            if let Some(body) = child.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    if member.kind() == "function_item" {
                        if let Some(name_node) = member.child_by_field_name("name") {
                            if AstUtils::node_text(name_node, source) == func_name {
                                let sig = extract_rust_sig(member, source);
                                return Some(format_rust_stub(&sig));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn format_rust_stub(sig: &str) -> String {
    let trimmed = sig.trim();
    if trimmed.ends_with(';') {
        trimmed.to_string()
    } else {
        format!("{trimmed};")
    }
}

fn collect_rust_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut generics = HashSet::new();
    if let Some(type_params) = node.child_by_field_name("type_parameters") {
        for id in AstUtils::find_descendants_by_kind(type_params, "type_identifier") {
            generics.insert(AstUtils::node_text(id, source).to_string());
        }
        for lt in AstUtils::find_descendants_by_kind(type_params, "lifetime") {
            generics.insert(AstUtils::node_text(lt, source).to_string());
        }
    }
    generics
}

fn extract_referenced_rust_types(node: Node<'_>, source: &str, scoped_generics: &HashSet<String>) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();

    for id in AstUtils::find_descendants_by_kind(node, "type_identifier") {
        let name = AstUtils::node_text(id, source).trim();
        if is_valid_custom_rust_type(name, scoped_generics) && seen.insert(name.to_string()) {
            names.push(name.to_string());
        }
    }

    names
}

fn is_valid_custom_rust_type(name: &str, scoped_generics: &HashSet<String>) -> bool {
    !name.is_empty()
        && !scoped_generics.contains(name)
        && !is_builtin_rust_type(name)
}

fn find_local_rust_type(root: Node<'_>, source: &str, type_name: &str, file_path: &Path) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if matches!(child.kind(), "struct_item" | "enum_item" | "trait_item" | "type_item") {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == type_name {
                    return Some(ExtractedType {
                        name: type_name.to_string(),
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

fn find_sibling_rust_modules(file_path: &Path) -> Vec<PathBuf> {
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
            && path.extension().and_then(|e| e.to_str()) == Some("rs")
        {
            siblings.push(path);
        }
    }

    siblings
}

/// Checks if a type name is a Rust primitive or standard library core type.
pub fn is_builtin_rust_type(name: &str) -> bool {
    matches!(
        name,
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
            | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
            | "f32" | "f64" | "bool" | "char" | "str" | "String"
            | "Option" | "Result" | "Vec" | "Box" | "Arc" | "Rc"
            | "Path" | "PathBuf" | "Self" | "HashMap" | "HashSet"
            | "BTreeMap" | "BTreeSet" | "RwLock" | "Mutex" | "Duration"
            | "Error" | "Display" | "Debug" | "Clone" | "Copy" | "Send" | "Sync"
            | "Default" | "PartialEq" | "Eq" | "PartialOrd" | "Ord" | "Hash"
            | "Sized" | "Into" | "From" | "TryInto" | "TryFrom" | "AsRef" | "AsMut"
            | "Fn" | "FnMut" | "FnOnce"
    )
}

/// Checks if a method or macro name is a Rust standard library built-in method or common utility.
pub fn is_builtin_rust_method(name: &str) -> bool {
    matches!(
        name,
        "clone" | "unwrap" | "expect" | "map" | "and_then" | "is_some" | "is_none"
            | "ok_or_else" | "insert" | "get" | "get_mut" | "push" | "len" | "write"
            | "read" | "trim" | "split" | "to_string" | "into" | "from" | "as_str"
            | "as_bytes" | "iter" | "into_iter" | "drain" | "collect" | "contains"
            | "starts_with" | "ends_with" | "find" | "matches" | "format" | "println"
            | "eprintln" | "panic" | "todo" | "unimplemented" | "unreachable"
    )
}
