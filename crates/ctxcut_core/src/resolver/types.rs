//! Type reference extraction and transitive type hoisting for TypeScript and TSX.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
use tree_sitter::Node;
use crate::error::Result;
use crate::model::{ExtractedType, SliceOptions};
use crate::parser::{AstUtils, ParserManager};
use crate::resolver::imports::ImportResolver;

/// Type hoister resolving interfaces, type aliases, enums, and DTOs.
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
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        // 1. Collect scoped generic parameter identifiers (e.g. T, K, V)
        let scoped_generics = collect_scoped_generics(target_node, source);

        // 2. Extract referenced type names from target node
        let initial_types = extract_type_identifiers(target_node, source, &scoped_generics);
        for ty in initial_types {
            if !visited.contains(&ty) {
                visited.insert(ty.clone());
                queue.push_back((ty, 1));
            }
        }

        // Cache for loaded and parsed external files
        let mut file_cache: std::collections::HashMap<std::path::PathBuf, (String, tree_sitter::Tree)> =
            std::collections::HashMap::new();

        // 3. Process queue up to opts.depth
        while let Some((type_name, depth)) = queue.pop_front() {
            if is_builtin_or_primitive(&type_name) {
                continue;
            }

            // Attempt local file resolution first
            if let Some(extracted) = find_type_in_file(root, source, &type_name, file_path) {
                // If depth < opts.depth, parse definition and enqueue referenced types
                if depth < opts.depth {
                    if let Ok(def_tree) = ParserManager::parse_source(&extracted.definition, tree_sitter_lang, file_path) {
                        let inner_types = extract_type_identifiers(def_tree.root_node(), &extracted.definition, &HashSet::new());
                        for inner in inner_types {
                            if !visited.contains(&inner) && !is_builtin_or_primitive(&inner) {
                                visited.insert(inner.clone());
                                queue.push_back((inner, depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            // Attempt imported resolution
            let imports = ImportResolver::extract_imports(root, source);
            if let Some(mapping) = imports.get(&type_name) {
                if let Some(target_file) = ImportResolver::resolve_module_path(file_path, &mapping.specifier) {
                    if let Some(extracted) = resolve_type_from_module(
                        &mapping.imported_name,
                        &target_file,
                        tree_sitter_lang,
                        &mut file_cache,
                    ) {
                        if depth < opts.depth {
                            if let Ok(def_tree) = ParserManager::parse_source(&extracted.definition, tree_sitter_lang, &target_file) {
                                let inner_types = extract_type_identifiers(def_tree.root_node(), &extracted.definition, &HashSet::new());
                                for inner in inner_types {
                                    if !visited.contains(&inner) && !is_builtin_or_primitive(&inner) {
                                        visited.insert(inner.clone());
                                        queue.push_back((inner, depth + 1));
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

fn extract_type_identifiers(node: Node<'_>, source: &str, scoped_generics: &HashSet<String>) -> Vec<String> {
    let mut type_names = Vec::new();
    let mut cursor = node.walk();

    // Check if node is type_identifier
    if node.kind() == "type_identifier" {
        let text = AstUtils::node_text(node, source);
        if !scoped_generics.contains(text) && !is_builtin_or_primitive(text) {
            type_names.push(text.to_string());
        }
    }

    for child in node.children(&mut cursor) {
        let inner = extract_type_identifiers(child, source, scoped_generics);
        type_names.extend(inner);
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
        let decl = if child.kind() == "export_statement" {
            child.child_by_field_name("declaration").unwrap_or(child)
        } else {
            child
        };

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

fn resolve_type_from_module(
    type_name: &str,
    target_file: &Path,
    tree_sitter_lang: &tree_sitter::Language,
    cache: &mut std::collections::HashMap<std::path::PathBuf, (String, tree_sitter::Tree)>,
) -> Option<ExtractedType> {
    let (source, tree) = get_or_load_file(target_file, tree_sitter_lang, cache)?;
    let root = tree.root_node();

    // 1. Direct declaration in target file
    if let Some(extracted) = find_type_in_file(root, source, type_name, target_file) {
        return Some(extracted);
    }

    // 2. Check barrel re-exports: export * from './sub', export { Type } from './sub'
    let reexports = ImportResolver::extract_reexports(root, source);
    for (exported_alias, specifier) in reexports {
        if let Some(alias) = exported_alias {
            if alias == type_name {
                if let Some(sub_file) = ImportResolver::resolve_module_path(target_file, &specifier) {
                    if let Some(res) = resolve_type_from_module(type_name, &sub_file, tree_sitter_lang, cache) {
                        return Some(res);
                    }
                }
            }
        } else {
            // Wildcard export * from './sub'
            if let Some(sub_file) = ImportResolver::resolve_module_path(target_file, &specifier) {
                if let Some(res) = resolve_type_from_module(type_name, &sub_file, tree_sitter_lang, cache) {
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
    cache: &'a mut std::collections::HashMap<std::path::PathBuf, (String, tree_sitter::Tree)>,
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
    match name {
        "string" | "number" | "boolean" | "symbol" | "bigint" | "void" | "null" | "undefined"
        | "never" | "unknown" | "any" | "object" | "Function" | "true" | "false" | "this" => true,
        "Array" | "ReadonlyArray" | "Promise" | "Map" | "Set" | "WeakMap" | "WeakSet" | "Date"
        | "RegExp" | "Error" | "TypeError" | "RangeError" | "SyntaxError" | "Uint8Array"
        | "Int8Array" | "Uint16Array" | "Int16Array" | "Uint32Array" | "Int32Array"
        | "Float32Array" | "Float64Array" | "BigInt64Array" | "BigUint64Array" | "ArrayBuffer"
        | "SharedArrayBuffer" | "DataView" | "Blob" | "File" | "FormData" | "URL"
        | "URLSearchParams" | "Headers" | "Request" | "Response" | "AbortController"
        | "AbortSignal" | "Event" | "CustomEvent" | "EventListener" | "NodeJS" | "Buffer"
        | "Process" | "Console" | "JSON" | "Math" | "Reflect" | "Proxy" | "Symbol" | "Object"
        | "String" | "Number" | "Boolean" | "BigInt" => true,
        "Partial" | "Required" | "Readonly" | "Record" | "Pick" | "Omit" | "Exclude" | "Extract"
        | "NonNullable" | "Parameters" | "ConstructorParameters" | "ReturnType" | "InstanceType"
        | "ThisParameterType" | "OmitThisParameter" | "ThisType" | "Uppercase" | "Lowercase"
        | "Capitalize" | "Uncapitalize" | "Awaited" | "JSX" | "Element" | "ReactElement"
        | "ReactNode" => true,
        _ => false,
    }
}
