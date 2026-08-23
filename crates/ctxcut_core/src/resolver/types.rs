//! Type reference extraction and transitive type hoisting across supported languages.

use crate::error::Result;
use crate::lang::LanguageAdapter;
use crate::model::{ExtractedType, SliceOptions, SupportedLanguage};
use crate::parser::{AstUtils, ParserManager};
use crate::resolver::imports::ImportResolver;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Type hoister resolving interfaces, type aliases, enums, structs, and DTOs.
pub struct TypeHoister;

impl TypeHoister {
    /// Hoists all referenced types from target node AST up to `opts.depth`.
    pub fn hoist_types<'a>(
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        opts: &SliceOptions,
        tree_sitter_lang: &tree_sitter::Language,
    ) -> Result<Vec<ExtractedType>> {
        if !opts.include_types {
            return Ok(Vec::new());
        }

        let mut hoisted = Vec::new();
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(String, usize, PathBuf)> = VecDeque::new();

        // 1. Collect scoped generic parameter identifiers (e.g. T, K, V)
        let scoped_generics = collect_scoped_generics(target_node, source);

        // 2. Extract referenced type names from target node
        let mut initial_types = extract_type_identifiers(target_node, source, &scoped_generics);
        if initial_types.is_empty() {
            let target_text = AstUtils::node_text(target_node, source);
            let file_imports = ImportResolver::extract_imports(root, source);
            for imported_name in file_imports.keys() {
                let is_type_position = target_text.contains(&format!(": {}", imported_name))
                    || target_text.contains(&format!(":{}", imported_name))
                    || target_text.contains(&format!("<{}", imported_name))
                    || target_text.contains(&format!("as {}", imported_name))
                    || target_text.contains(&format!("extends {}", imported_name))
                    || target_text.contains(&format!("{}[]", imported_name));

                if is_type_position
                    && !initial_types.contains(imported_name)
                    && !scoped_generics.contains(imported_name)
                    && !is_builtin_or_primitive(imported_name)
                {
                    initial_types.push(imported_name.clone());
                }
            }
        }

        for ty in initial_types {
            if !visited.contains(&ty) {
                visited.insert(ty.clone());
                queue.push_back((ty, 1, file_path.to_path_buf()));
            }
        }

        // Cache for loaded and parsed external files
        let mut file_cache: HashMap<PathBuf, (String, tree_sitter::Tree)> = HashMap::new();

        // 3. Process queue up to opts.depth
        while let Some((type_name, depth, origin_file)) = queue.pop_front() {
            if is_builtin_or_primitive(&type_name) {
                continue;
            }

            // A. Check if type is declared in origin_file
            let origin_is_target = origin_file == file_path;
            let (orig_source, orig_tree_root) = if origin_is_target {
                (source, root)
            } else if let Some((src, tree)) =
                get_or_load_file(&origin_file, tree_sitter_lang, &mut file_cache)
            {
                (src, tree.root_node())
            } else {
                continue;
            };

            if let Some(extracted) =
                find_type_in_file(orig_tree_root, orig_source, &type_name, &origin_file)
            {
                if depth < opts.depth {
                    if let Ok(def_tree) = ParserManager::parse_source(
                        &extracted.definition,
                        tree_sitter_lang,
                        &origin_file,
                    ) {
                        let inner_types = extract_type_identifiers(
                            def_tree.root_node(),
                            &extracted.definition,
                            &HashSet::new(),
                        );
                        for inner in inner_types {
                            if !visited.contains(&inner) && !is_builtin_or_primitive(&inner) {
                                visited.insert(inner.clone());
                                queue.push_back((inner, depth + 1, origin_file.clone()));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            // If depth == 0 or depth > opts.depth, skip foreign resolution
            if opts.depth == 0 {
                continue;
            }

            // B. Check origin_file's imports
            let imports = ImportResolver::extract_imports(orig_tree_root, orig_source);
            if let Some(mapping) = imports.get(&type_name) {
                if let Some(target_file) =
                    ImportResolver::resolve_module_path(&origin_file, &mapping.specifier)
                {
                    if let Some(extracted) = resolve_type_from_module(
                        &mapping.imported_name,
                        &target_file,
                        tree_sitter_lang,
                        &mut file_cache,
                    ) {
                        let def_file = PathBuf::from(&extracted.file_path);

                        if depth < opts.depth {
                            if let Ok(def_tree) = ParserManager::parse_source(
                                &extracted.definition,
                                tree_sitter_lang,
                                &def_file,
                            ) {
                                let inner_types = extract_type_identifiers(
                                    def_tree.root_node(),
                                    &extracted.definition,
                                    &HashSet::new(),
                                );
                                for inner in inner_types {
                                    if !visited.contains(&inner) && !is_builtin_or_primitive(&inner)
                                    {
                                        visited.insert(inner.clone());
                                        queue.push_back((inner, depth + 1, def_file.clone()));
                                    }
                                }
                            }
                        }
                        hoisted.push(extracted);
                    }
                }
            }
        }

        Ok(hoisted)
    }

    /// Hoists and extracts definitions for the specified types from `target_file`.
    pub fn resolve_foreign_types(
        target_file: &Path,
        type_names: &[&str],
    ) -> Result<Vec<ExtractedType>> {
        resolve_foreign_types(target_file, type_names)
    }
}

/// Resolves type definitions from a foreign file or package directory across TS, Python, Go, and Rust.
pub fn resolve_foreign_types(
    target_file: &Path,
    type_names: &[&str],
) -> Result<Vec<ExtractedType>> {
    if type_names.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let mut found_names = HashSet::new();

    // 1. Directory case (e.g. Go package directory)
    if target_file.is_dir() {
        if let Ok(entries) = fs::read_dir(target_file) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let lang = SupportedLanguage::from_path(&p);
                    if let Some(l) = lang {
                        if let Ok(types) = extract_types_from_single_file(&p, l, type_names) {
                            for t in types {
                                if found_names.insert(t.name.clone()) {
                                    results.push(t);
                                }
                            }
                        }
                    }
                }
            }
        }
        return Ok(results);
    }

    // 2. Single file case
    let lang = SupportedLanguage::from_path(target_file).unwrap_or(SupportedLanguage::TypeScript);
    if let Ok(types) = extract_types_from_single_file(target_file, lang, type_names) {
        for t in types {
            if found_names.insert(t.name.clone()) {
                results.push(t);
            }
        }
    }

    // 3. If any types are still missing, check sibling files (for Go, Rust, or Python)
    let missing: Vec<&str> = type_names
        .iter()
        .copied()
        .filter(|n| !found_names.contains(*n))
        .collect();

    if !missing.is_empty() {
        if let Some(parent_dir) = target_file.parent() {
            if let Ok(entries) = fs::read_dir(parent_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() && p != target_file {
                        let sibling_lang = SupportedLanguage::from_path(&p);
                        if sibling_lang == Some(lang) {
                            if let Ok(sibling_types) =
                                extract_types_from_single_file(&p, lang, &missing)
                            {
                                for t in sibling_types {
                                    if found_names.insert(t.name.clone()) {
                                        results.push(t);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(results)
}

fn extract_types_from_single_file(
    file_path: &Path,
    lang: SupportedLanguage,
    type_names: &[&str],
) -> Result<Vec<ExtractedType>> {
    let source = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };

    let relevant_names: Vec<&str> = type_names
        .iter()
        .copied()
        .filter(|n| source.contains(n))
        .collect();
    if relevant_names.is_empty() {
        return Ok(Vec::new());
    }

    let ts_lang: tree_sitter::Language = match lang {
        SupportedLanguage::TypeScript => {
            let ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "tsx" {
                tree_sitter_typescript::LANGUAGE_TSX.into()
            } else {
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
            }
        }
        SupportedLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        SupportedLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        SupportedLanguage::Go => tree_sitter_go::LANGUAGE.into(),
        SupportedLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        SupportedLanguage::C => tree_sitter_c::LANGUAGE.into(),
        SupportedLanguage::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        SupportedLanguage::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        SupportedLanguage::Java => tree_sitter_java::LANGUAGE.into(),
        SupportedLanguage::Kotlin => tree_sitter_kotlin::LANGUAGE.into(),
        SupportedLanguage::Vue | SupportedLanguage::Svelte | SupportedLanguage::Astro => {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        }
    };

    let tree = match ParserManager::parse_source(&source, &ts_lang, file_path) {
        Ok(t) => t,
        Err(_) => return Ok(Vec::new()),
    };

    let root = tree.root_node();
    let mut extracted = Vec::new();

    for &name in &relevant_names {
        match lang {
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => {
                if let Some(t) = find_type_in_file(root, &source, name, file_path) {
                    extracted.push(t);
                }
            }
            SupportedLanguage::Python => {
                if let Some(t) = find_python_type_in_file(root, &source, name, file_path) {
                    extracted.push(t);
                }
            }
            SupportedLanguage::Go => {
                if let Some(t) = find_go_type_in_file(root, &source, name, file_path) {
                    extracted.push(t);
                }
            }
            SupportedLanguage::Rust => {
                if let Some(t) = find_rust_type_in_file(root, &source, name, file_path) {
                    extracted.push(t);
                }
            }
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                let adapter = crate::lang::c_cpp::CppAdapter;
                if let Ok(types) = adapter.hoist_types(root, root, &source, file_path, &SliceOptions::default()) {
                    for t in types {
                        if t.name == name && !extracted.iter().any(|e: &ExtractedType| e.name == name) {
                            extracted.push(t);
                        }
                    }
                }
            }
            SupportedLanguage::CSharp => {
                let adapter = crate::lang::csharp::CSharpAdapter;
                if let Ok(types) = adapter.hoist_types(root, root, &source, file_path, &SliceOptions::default()) {
                    for t in types {
                        if t.name == name && !extracted.iter().any(|e: &ExtractedType| e.name == name) {
                            extracted.push(t);
                        }
                    }
                }
            }
            SupportedLanguage::Java => {
                let adapter = crate::lang::java_lang::JavaAdapter;
                if let Ok(types) = adapter.hoist_types(root, root, &source, file_path, &SliceOptions::default()) {
                    for t in types {
                        if t.name == name && !extracted.iter().any(|e: &ExtractedType| e.name == name) {
                            extracted.push(t);
                        }
                    }
                }
            }
            SupportedLanguage::Kotlin => {
                let adapter = crate::lang::kotlin_lang::KotlinAdapter;
                if let Ok(types) = adapter.hoist_types(root, root, &source, file_path, &SliceOptions::default()) {
                    for t in types {
                        if t.name == name && !extracted.iter().any(|e: &ExtractedType| e.name == name) {
                            extracted.push(t);
                        }
                    }
                }
            }
        }
    }

    Ok(extracted)
}

fn collect_scoped_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut generics = HashSet::new();
    if let Some(type_params) = node.child_by_field_name("type_parameters") {
        for param in AstUtils::find_descendants_by_kind(type_params, "type_parameter") {
            if let Some(name_node) = param.child_by_field_name("name") {
                generics.insert(AstUtils::node_text(name_node, source).to_string());
            } else if let Some(first_child) = param.named_child(0) {
                generics.insert(AstUtils::node_text(first_child, source).to_string());
            }
        }
    }
    generics
}

fn extract_type_identifiers(
    node: Node<'_>,
    source: &str,
    scoped_generics: &HashSet<String>,
) -> Vec<String> {
    let mut type_names = Vec::new();
    let mut cursor = node.walk();

    if node.kind() == "type_identifier"
        || node.kind() == "user_type"
        || (node.kind() == "identifier"
            && node
                .parent()
                .map(|p| {
                    p.kind() == "type_annotation"
                        || p.kind() == "type_arguments"
                        || p.kind() == "type_reference"
                        || p.kind() == "implements_clause"
                        || p.kind() == "heritage_clause"
                })
                .unwrap_or(false))
    {
        let text = AstUtils::node_text(node, source);
        if !scoped_generics.contains(text)
            && !is_builtin_or_primitive(text)
            && !type_names.contains(&text.to_string())
        {
            type_names.push(text.to_string());
        }
    }

    for child in node.children(&mut cursor) {
        let inner = extract_type_identifiers(child, source, scoped_generics);
        for item in inner {
            if !type_names.contains(&item) {
                type_names.push(item);
            }
        }
    }

    type_names
}

fn find_type_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let enclosing = child;
        let decl = AstUtils::unwrap_export(child);

        match decl.kind() {
            "interface_declaration" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "interface".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(enclosing, source).trim().to_string(),
                        });
                    }
                }
            }
            "type_alias_declaration" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "type_alias".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(enclosing, source).trim().to_string(),
                        });
                    }
                }
            }
            "enum_declaration" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "enum".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(enclosing, source).trim().to_string(),
                        });
                    }
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "class".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(enclosing, source).trim().to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_python_type_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = if child.kind() == "decorated_definition" {
            child.child_by_field_name("definition").unwrap_or(child)
        } else {
            child
        };

        match decl.kind() {
            "class_definition" => {
                if let Some(name_node) = decl.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "class".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).trim().to_string(),
                        });
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
                    return Some(ExtractedType {
                        name: target_name.to_string(),
                        kind: "type_alias".to_string(),
                        file_path: file_path.to_string_lossy().to_string(),
                        definition: AstUtils::node_text(child, source).trim().to_string(),
                    });
                }
            }
            "expression_statement" => {
                if let Some(assignment) = decl.named_child(0) {
                    if assignment.kind() == "assignment" {
                        if let Some(left) = assignment.child_by_field_name("left") {
                            if AstUtils::node_text(left, source) == target_name {
                                return Some(ExtractedType {
                                    name: target_name.to_string(),
                                    kind: "type_alias".to_string(),
                                    file_path: file_path.to_string_lossy().to_string(),
                                    definition: AstUtils::node_text(child, source)
                                        .trim()
                                        .to_string(),
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_go_type_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "type_declaration" {
            for spec in AstUtils::find_children_by_kind(child, "type_spec") {
                if let Some(name_node) = spec.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        let kind = if let Some(type_node) = spec.child_by_field_name("type") {
                            match type_node.kind() {
                                "struct_type" => "struct",
                                "interface_type" => "interface",
                                _ => "type_alias",
                            }
                        } else {
                            "type_alias"
                        };
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: kind.to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).trim().to_string(),
                        });
                    }
                }
            }
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
        match child.kind() {
            "struct_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "struct".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).trim().to_string(),
                        });
                    }
                }
            }
            "enum_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "enum".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).trim().to_string(),
                        });
                    }
                }
            }
            "trait_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "trait".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).trim().to_string(),
                        });
                    }
                }
            }
            "type_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: "type_alias".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: AstUtils::node_text(child, source).trim().to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn resolve_type_from_module(
    type_name: &str,
    target_file: &Path,
    tree_sitter_lang: &tree_sitter::Language,
    cache: &mut HashMap<PathBuf, (String, tree_sitter::Tree)>,
) -> Option<ExtractedType> {
    let (source, tree) = get_or_load_file(target_file, tree_sitter_lang, cache)?;
    let root = tree.root_node();

    // 1. Direct declaration in target file
    if let Some(extracted) = find_type_in_file(root, source, type_name, target_file) {
        return Some(extracted);
    }

    // 2. Check barrel re-exports: export * from './sub', export { Type } from './sub'
    let reexports = ImportResolver::extract_reexports(root, source);
    for (exported_alias, orig_name, specifier) in reexports {
        if let Some(alias) = exported_alias {
            if alias == type_name {
                let lookup_name = orig_name.as_deref().unwrap_or(type_name);
                if let Some(sub_file) = ImportResolver::resolve_module_path(target_file, &specifier)
                {
                    if let Some(res) =
                        resolve_type_from_module(lookup_name, &sub_file, tree_sitter_lang, cache)
                    {
                        return Some(res);
                    }
                }
            }
        } else {
            // Wildcard export * from './sub'
            if let Some(sub_file) = ImportResolver::resolve_module_path(target_file, &specifier) {
                if let Some(res) =
                    resolve_type_from_module(type_name, &sub_file, tree_sitter_lang, cache)
                {
                    return Some(res);
                }
            }
        }
    }

    None
}

fn get_or_load_file<'a>(
    path: &Path,
    tree_sitter_lang: &tree_sitter::Language,
    cache: &'a mut HashMap<PathBuf, (String, tree_sitter::Tree)>,
) -> Option<(&'a str, &'a tree_sitter::Tree)> {
    if !cache.contains_key(path) {
        let content = fs::read_to_string(path).ok()?;
        let tree = ParserManager::parse_source(&content, tree_sitter_lang, path).ok()?;
        cache.insert(path.to_path_buf(), (content, tree));
    }

    let (content, tree) = cache.get(path)?;
    Some((content.as_str(), tree))
}

fn is_builtin_or_primitive(name: &str) -> bool {
    matches!(
        name,
        "string"
            | "number"
            | "boolean"
            | "symbol"
            | "bigint"
            | "void"
            | "null"
            | "undefined"
            | "never"
            | "unknown"
            | "any"
            | "object"
            | "Function"
            | "true"
            | "false"
            | "this"
            | "Array"
            | "ReadonlyArray"
            | "Promise"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "Date"
            | "RegExp"
            | "Error"
            | "TypeError"
            | "RangeError"
            | "SyntaxError"
            | "Uint8Array"
            | "Int8Array"
            | "Uint16Array"
            | "Int16Array"
            | "Uint32Array"
            | "Int32Array"
            | "Float32Array"
            | "Float64Array"
            | "BigInt64Array"
            | "BigUint64Array"
            | "ArrayBuffer"
            | "SharedArrayBuffer"
            | "DataView"
            | "Blob"
            | "File"
            | "FormData"
            | "URL"
            | "URLSearchParams"
            | "Headers"
            | "Request"
            | "Response"
            | "AbortController"
            | "AbortSignal"
            | "Event"
            | "CustomEvent"
            | "EventListener"
            | "NodeJS"
            | "Buffer"
            | "Process"
            | "Console"
            | "JSON"
            | "Math"
            | "Reflect"
            | "Proxy"
            | "Symbol"
            | "Object"
            | "String"
            | "Number"
            | "Boolean"
            | "BigInt"
            | "Partial"
            | "Required"
            | "Readonly"
            | "Record"
            | "Pick"
            | "Omit"
            | "Exclude"
            | "Extract"
            | "NonNullable"
            | "Parameters"
            | "ConstructorParameters"
            | "ReturnType"
            | "InstanceType"
            | "ThisParameterType"
            | "OmitThisParameter"
            | "ThisType"
            | "Uppercase"
            | "Lowercase"
            | "Capitalize"
            | "Uncapitalize"
            | "Awaited"
            | "JSX"
            | "Element"
            | "ReactElement"
            | "ReactNode"
            // Python primitives
            | "int"
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
            | "Callable"
            | "None"
            | "self"
            | "cls"
            // Go primitives
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
            | "error"
            | "comparable"
            // Rust primitives
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "char"
            | "Vec"
            | "Box"
            | "Arc"
            | "Rc"
            | "Path"
            | "PathBuf"
            | "Self"
            | "Send"
            | "Sync"
            | "Clone"
            | "Copy"
            | "Debug"
            | "Display"
            | "Default"
            | "AsRef"
            | "From"
            | "Into"
            | "Fn"
            | "FnMut"
            | "FnOnce"
    )
}
