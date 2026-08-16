//! LanguageAdapter implementation for Python.

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
                "type_alias_statement" => {
                    let full_text = node
                        .child_by_field_name("name")
                        .or_else(|| node.named_child(0))
                        .map(|n| AstUtils::node_text(n, source))
                        .unwrap_or("");
                    let base_name = full_text.split('[').next().unwrap_or(full_text).trim();
                    if !base_name.is_empty() {
                        symbols.push(base_name.to_string());
                    }
                }
                "expression_statement" => {
                    let text = AstUtils::node_text(node, source);
                    if let Some((left, _)) = text.split_once('=') {
                        let candidate = left.trim();
                        let base_cand = candidate.split('[').next().unwrap_or(candidate).trim();
                        if base_cand.chars().all(|c| c.is_alphanumeric() || c == '_') && !base_cand.is_empty() {
                            symbols.push(base_cand.to_string());
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
        if !opts.include_types {
            return Ok(Vec::new());
        }

        let mut hoisted = Vec::new();
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        // Collect scoped generics (PEP 695 type parameters)
        let scoped_generics = collect_python_scoped_generics(target_node, source);

        // Collect initial referenced types
        for id in AstUtils::find_descendants_by_kind(target_node, "identifier") {
            let name = AstUtils::node_text(id, source);
            if !is_builtin_python_type(name) && !is_builtin_python_func(name) && !scoped_generics.contains(name) {
                if visited.insert(name.to_string()) {
                    queue.push_back((name.to_string(), 1));
                }
            }
        }

        // Also check string annotations in quotes (e.g. Optional["ModelA"])
        for str_node in AstUtils::find_descendants_by_kind(target_node, "string") {
            let text = AstUtils::node_text(str_node, source).trim_matches('"').trim_matches('\'');
            if !is_builtin_python_type(text) && !scoped_generics.contains(text) && visited.insert(text.to_string()) {
                queue.push_back((text.to_string(), 1));
            }
        }

        let ts_lang = self.tree_sitter_language(file_path);
        let mut known_modules: Vec<PathBuf> = Vec::new();
        known_modules.push(file_path.to_path_buf());

        while let Some((type_name, depth)) = queue.pop_front() {
            if is_builtin_python_type(&type_name) || scoped_generics.contains(&type_name) {
                continue;
            }

            // 1. Search in local file
            if let Some(extracted) = find_class_or_type_in_file(root, source, &type_name, file_path) {
                if depth < opts.depth {
                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, file_path) {
                        let def_generics = collect_python_scoped_generics(tree.root_node(), &extracted.definition);
                        for id in AstUtils::find_descendants_by_kind(tree.root_node(), "identifier") {
                            let name = AstUtils::node_text(id, &extracted.definition);
                            if !is_builtin_python_type(name) && !def_generics.contains(name) && visited.insert(name.to_string()) {
                                queue.push_back((name.to_string(), depth + 1));
                            }
                        }
                        for str_node in AstUtils::find_descendants_by_kind(tree.root_node(), "string") {
                            let text = AstUtils::node_text(str_node, &extracted.definition).trim_matches('"').trim_matches('\'');
                            if !is_builtin_python_type(text) && !def_generics.contains(text) && visited.insert(text.to_string()) {
                                queue.push_back((text.to_string(), depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            // 2. Search via imports from current file and any known imported modules
            let mut found_type = None;
            for mod_path in &known_modules.clone() {
                let mut visited_files = HashSet::new();
                if let Some(extracted) = find_type_in_module_or_reexports(mod_path, &type_name, &ts_lang, &mut visited_files) {
                    found_type = Some((extracted, mod_path.clone()));
                    break;
                }
            }

            // 3. Fallback: Search sibling files in directories of known modules / file_path
            if found_type.is_none() {
                let mut search_dirs = Vec::new();
                if let Some(p) = file_path.parent() {
                    search_dirs.push(p.to_path_buf());
                }
                for km in &known_modules {
                    if let Some(p) = km.parent() {
                        if !search_dirs.contains(&p.to_path_buf()) {
                            search_dirs.push(p.to_path_buf());
                        }
                    }
                }

                for dir in search_dirs {
                    if let Ok(entries) = fs::read_dir(&dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("py") && path != file_path {
                                let mut visited_files = HashSet::new();
                                if let Some(extracted) = find_type_in_module_or_reexports(&path, &type_name, &ts_lang, &mut visited_files) {
                                    found_type = Some((extracted, path));
                                    break;
                                }
                            }
                        }
                    }
                    if found_type.is_some() {
                        break;
                    }
                }
            }

            if let Some((extracted, origin_path)) = found_type {
                let extracted_path = PathBuf::from(&extracted.file_path);
                if !known_modules.contains(&extracted_path) {
                    known_modules.push(extracted_path.clone());
                }
                if !known_modules.contains(&origin_path) {
                    known_modules.push(origin_path);
                }

                if depth < opts.depth {
                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, &extracted_path) {
                        let def_generics = collect_python_scoped_generics(tree.root_node(), &extracted.definition);
                        for id in AstUtils::find_descendants_by_kind(tree.root_node(), "identifier") {
                            let name = AstUtils::node_text(id, &extracted.definition);
                            if !is_builtin_python_type(name) && !def_generics.contains(name) && visited.insert(name.to_string()) {
                                queue.push_back((name.to_string(), depth + 1));
                            }
                        }
                        for str_node in AstUtils::find_descendants_by_kind(tree.root_node(), "string") {
                            let text = AstUtils::node_text(str_node, &extracted.definition).trim_matches('"').trim_matches('\'');
                            if !is_builtin_python_type(text) && !def_generics.contains(text) && visited.insert(text.to_string()) {
                                queue.push_back((text.to_string(), depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
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
                } else {
                    let ts_lang = self.tree_sitter_language(file_path);
                    let mut found = false;

                    // 1. Search via imports
                    let imports = extract_python_imports(root, source);
                    for imp in &imports {
                        if let Some(target_file) = resolve_python_import_path(file_path, imp) {
                            if let Ok(imported_src) = fs::read_to_string(&target_file) {
                                if let Ok(imported_tree) = ParserManager::parse_source(&imported_src, &ts_lang, &target_file) {
                                    if let Some(sig) = find_python_signature(imported_tree.root_node(), &imported_src, call_name) {
                                        stubs.push(CallSignatureStub {
                                            name: call_name.to_string(),
                                            receiver: None,
                                            file_path: Some(target_file.to_string_lossy().to_string()),
                                            signature: sig,
                                        });
                                        found = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // 2. Search sibling files
                    if !found {
                        if let Some(dir) = file_path.parent() {
                            if let Ok(entries) = fs::read_dir(dir) {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("py") && path != file_path {
                                        if let Ok(sibling_src) = fs::read_to_string(&path) {
                                            if let Ok(sibling_tree) = ParserManager::parse_source(&sibling_src, &ts_lang, &path) {
                                                if let Some(sig) = find_python_signature(sibling_tree.root_node(), &sibling_src, call_name) {
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
                            }
                        }
                    }
                }
            }
        }

        Ok(stubs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PythonImportMapping {
    local_name: String,
    imported_name: String,
    module_specifier: String,
    level: usize,
}

fn extract_python_imports(root: Node<'_>, source: &str) -> Vec<PythonImportMapping> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "import_from_statement" => {
                let mut level = 0;
                let mut module_name = String::new();

                if let Some(mod_node) = child.child_by_field_name("module_name") {
                    match mod_node.kind() {
                        "relative_import" => {
                            let text = AstUtils::node_text(mod_node, source);
                            let dots = text.chars().take_while(|c| *c == '.').count();
                            level = dots;
                            if let Some(dotted) = mod_node.child_by_field_name("module_name") {
                                module_name = AstUtils::node_text(dotted, source).to_string();
                            } else {
                                let remainder = &text[dots..];
                                if !remainder.is_empty() {
                                    module_name = remainder.to_string();
                                }
                            }
                        }
                        "dotted_name" => {
                            module_name = AstUtils::node_text(mod_node, source).to_string();
                        }
                        _ => {
                            let text = AstUtils::node_text(mod_node, source);
                            let dots = text.chars().take_while(|c| *c == '.').count();
                            level = dots;
                            module_name = text[dots..].to_string();
                        }
                    }
                } else {
                    let text = AstUtils::node_text(child, source);
                    if let Some(from_part) = text.strip_prefix("from ") {
                        if let Some((mod_part, _)) = from_part.split_once(" import ") {
                            let trimmed = mod_part.trim();
                            let dots = trimmed.chars().take_while(|c| *c == '.').count();
                            level = dots;
                            module_name = trimmed[dots..].to_string();
                        }
                    }
                }

                for name_node in child.children_by_field_name("name", &mut child.walk()) {
                    match name_node.kind() {
                        "dotted_name" | "identifier" => {
                            let name = AstUtils::node_text(name_node, source).to_string();
                            imports.push(PythonImportMapping {
                                local_name: name.clone(),
                                imported_name: name,
                                module_specifier: module_name.clone(),
                                level,
                            });
                        }
                        "aliased_import" => {
                            let orig = name_node
                                .child_by_field_name("name")
                                .map(|n| AstUtils::node_text(n, source).to_string())
                                .unwrap_or_default();
                            let alias = name_node
                                .child_by_field_name("alias")
                                .map(|n| AstUtils::node_text(n, source).to_string())
                                .unwrap_or_else(|| orig.clone());
                            imports.push(PythonImportMapping {
                                local_name: alias,
                                imported_name: orig,
                                module_specifier: module_name.clone(),
                                level,
                            });
                        }
                        "wildcard_import" => {
                            imports.push(PythonImportMapping {
                                local_name: "*".to_string(),
                                imported_name: "*".to_string(),
                                module_specifier: module_name.clone(),
                                level,
                            });
                        }
                        _ => {}
                    }
                }
            }
            "import_statement" => {
                for name_node in child.children_by_field_name("name", &mut child.walk()) {
                    match name_node.kind() {
                        "dotted_name" | "identifier" => {
                            let full_text = AstUtils::node_text(name_node, source);
                            let local = full_text.split('.').next().unwrap_or(full_text).to_string();
                            imports.push(PythonImportMapping {
                                local_name: local,
                                imported_name: "*".to_string(),
                                module_specifier: full_text.to_string(),
                                level: 0,
                            });
                        }
                        "aliased_import" => {
                            let orig = name_node
                                .child_by_field_name("name")
                                .map(|n| AstUtils::node_text(n, source).to_string())
                                .unwrap_or_default();
                            let alias = name_node
                                .child_by_field_name("alias")
                                .map(|n| AstUtils::node_text(n, source).to_string())
                                .unwrap_or_else(|| orig.clone());
                            imports.push(PythonImportMapping {
                                local_name: alias,
                                imported_name: "*".to_string(),
                                module_specifier: orig,
                                level: 0,
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    imports
}

fn resolve_python_import_path(from_file: &Path, import: &PythonImportMapping) -> Option<PathBuf> {
    let current_dir = from_file.parent().unwrap_or_else(|| Path::new("."));

    if import.level > 0 {
        let mut base = current_dir;
        for _ in 1..import.level {
            base = base.parent()?;
        }
        let parts: Vec<&str> = import.module_specifier.split('.').filter(|s| !s.is_empty()).collect();
        let mut p = base.to_path_buf();
        for part in parts {
            p.push(part);
        }
        if import.module_specifier.is_empty() && !import.imported_name.is_empty() && import.imported_name != "*" {
            let specific = p.join(&import.imported_name);
            if let Some(cand) = check_python_candidate(&specific) {
                return Some(cand);
            }
        }
        check_python_candidate(&p)
    } else {
        let parts: Vec<&str> = import.module_specifier.split('.').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return None;
        }

        // Try relative to current_dir first
        let mut rel = current_dir.to_path_buf();
        for part in &parts {
            rel.push(part);
        }
        if let Some(cand) = check_python_candidate(&rel) {
            return Some(cand);
        }

        // Try walking up to project root
        let mut curr = current_dir;
        while let Some(parent) = curr.parent() {
            let mut p = parent.to_path_buf();
            for part in &parts {
                p.push(part);
            }
            if let Some(cand) = check_python_candidate(&p) {
                return Some(cand);
            }
            curr = parent;
        }

        None
    }
}

fn check_python_candidate(path: &Path) -> Option<PathBuf> {
    let py = path.with_extension("py");
    if py.is_file() {
        return Some(py);
    }
    let pyi = path.with_extension("pyi");
    if pyi.is_file() {
        return Some(pyi);
    }
    let init_py = path.join("__init__.py");
    if init_py.is_file() {
        return Some(init_py);
    }
    let init_pyi = path.join("__init__.pyi");
    if init_pyi.is_file() {
        return Some(init_pyi);
    }
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    None
}

fn find_type_in_module_or_reexports(
    file_path: &Path,
    type_name: &str,
    ts_lang: &Language,
    visited_files: &mut HashSet<PathBuf>,
) -> Option<ExtractedType> {
    if !visited_files.insert(file_path.to_path_buf()) {
        return None;
    }

    let source = fs::read_to_string(file_path).ok()?;
    let tree = ParserManager::parse_source(&source, ts_lang, file_path).ok()?;
    let root = tree.root_node();

    // 1. Direct definition
    if let Some(extracted) = find_class_or_type_in_file(root, &source, type_name, file_path) {
        return Some(extracted);
    }

    // 2. Re-exports / imports
    let imports = extract_python_imports(root, &source);
    for imp in imports {
        if imp.local_name == type_name || imp.imported_name == type_name || imp.imported_name == "*" {
            let lookup_name = if imp.local_name == type_name && imp.imported_name != "*" {
                &imp.imported_name
            } else {
                type_name
            };

            if let Some(target_file) = resolve_python_import_path(file_path, &imp) {
                if let Some(extracted) = find_type_in_module_or_reexports(&target_file, lookup_name, ts_lang, visited_files) {
                    return Some(extracted);
                }
            }
        }
    }

    None
}

fn collect_python_scoped_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut generics = HashSet::new();
    for tp_list in AstUtils::find_descendants_by_kind(node, "type_parameters") {
        for child in tp_list.named_children(&mut tp_list.walk()) {
            match child.kind() {
                "type_parameter" => {
                    if let Some(n) = child.child_by_field_name("name").or_else(|| child.named_child(0)) {
                        generics.insert(AstUtils::node_text(n, source).to_string());
                    }
                }
                "type_parameter_vararg" | "type_parameter_kwarg" | "splat_type" => {
                    if let Some(n) = child.child_by_field_name("name").or_else(|| child.named_child(0)) {
                        generics.insert(AstUtils::node_text(n, source).trim_start_matches('*').to_string());
                    } else {
                        let text = AstUtils::node_text(child, source).trim_start_matches('*').trim();
                        if !text.is_empty() {
                            generics.insert(text.to_string());
                        }
                    }
                }
                _ => {
                    if let Some(first) = child.named_children(&mut child.walk()).next() {
                        let text = AstUtils::node_text(first, source).trim_start_matches('*').trim();
                        if !text.is_empty() {
                            generics.insert(text.to_string());
                        }
                    } else {
                        let text = AstUtils::node_text(child, source).trim_start_matches('*').trim();
                        if !text.is_empty() {
                            generics.insert(text.to_string());
                        }
                    }
                }
            }
        }
    }
    generics
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
            "type_alias_statement" => {
                let full_text = decl
                    .child_by_field_name("name")
                    .or_else(|| decl.named_child(0))
                    .map(|n| AstUtils::node_text(n, source))
                    .unwrap_or("");
                let base_name = full_text.split('[').next().unwrap_or(full_text).trim();
                if base_name == target_name {
                    return Some((build_extracted_symbol(full_node, decl, source, file_path, "type"), full_node));
                }
            }
            "expression_statement" => {
                let text = AstUtils::node_text(decl, source);
                if let Some((left, _)) = text.split_once('=') {
                    let candidate = left.trim();
                    let base_cand = candidate.split('[').next().unwrap_or(candidate).trim();
                    if base_cand == target_name {
                        return Some((build_extracted_symbol(full_node, decl, source, file_path, "type"), full_node));
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
        } else if decl.kind() == "type_alias_statement" {
            let full_text = decl
                .child_by_field_name("name")
                .or_else(|| decl.named_child(0))
                .map(|n| AstUtils::node_text(n, source))
                .unwrap_or("");
            let base_name = full_text.split('[').next().unwrap_or(full_text).trim();
            if base_name == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "type".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(child, source).to_string(),
                });
            }
        } else if decl.kind() == "expression_statement" {
            let text = AstUtils::node_text(decl, source);
            if let Some((left, _)) = text.split_once('=') {
                let candidate = left.trim();
                let base_cand = candidate.split('[').next().unwrap_or(candidate).trim();
                if base_cand == target_name {
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
    let raw_name = decl
        .child_by_field_name("name")
        .or_else(|| decl.named_child(0))
        .map(|n| AstUtils::node_text(n, source).to_string())
        .unwrap_or_else(|| "anonymous".to_string());
    let name = raw_name.split('[').next().unwrap_or(&raw_name).trim().to_string();

    let body = AstUtils::node_text(full_node, source).to_string();
    let mut doc_comment = AstUtils::extract_doc_comment(full_node, source);

    if doc_comment.is_none() {
        if let Some(body_node) = decl.child_by_field_name("body") {
            if let Some(first_stmt) = body_node.named_children(&mut body_node.walk()).next() {
                if first_stmt.kind() == "expression_statement" {
                    if let Some(str_node) = first_stmt.named_children(&mut first_stmt.walk()).next() {
                        if str_node.kind() == "string" {
                            let text = AstUtils::node_text(str_node, source).trim();
                            let clean = strip_python_string_quotes(text);
                            doc_comment = Some(clean);
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

fn strip_python_string_quotes(raw: &str) -> String {
    let s = raw.trim();
    let without_prefix = s.trim_start_matches(|c: char| matches!(c, 'r' | 'R' | 'u' | 'U' | 'f' | 'F' | 'b' | 'B'));

    let unquoted = if let Some(inner) = without_prefix.strip_prefix("\"\"\"").and_then(|i| i.strip_suffix("\"\"\"")) {
        inner.trim()
    } else if let Some(inner) = without_prefix.strip_prefix("'''").and_then(|i| i.strip_suffix("'''")) {
        inner.trim()
    } else if let Some(inner) = without_prefix.strip_prefix('"').and_then(|i| i.strip_suffix('"')) {
        inner.trim()
    } else if let Some(inner) = without_prefix.strip_prefix('\'').and_then(|i| i.strip_suffix('\'')) {
        inner.trim()
    } else {
        without_prefix
    };

    unquoted.to_string()
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
        } else if decl.kind() == "class_definition" {
            if let Some(body) = decl.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    let m_decl = unwrap_decorated(member);
                    if m_decl.kind() == "function_definition" {
                        if let Some(m_name) = m_decl.child_by_field_name("name") {
                            if AstUtils::node_text(m_name, source) == func_name {
                                return Some(format!("{}: ...", extract_python_sig(m_decl, source)));
                            }
                        }
                    }
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
