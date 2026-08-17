//! External call expression extractor and signature stripper supporting TypeScript, Python, Go, and Rust.

use crate::error::Result;
use crate::model::{CallSignatureStub, SupportedLanguage};
use crate::parser::{AstUtils, ParserManager};
use crate::resolver::imports::ImportResolver;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Extracts call expressions and strips implementations to pure signatures.
pub struct SignatureStripper;

impl SignatureStripper {
    /// Extracts called functions/methods and returns 100% body-stripped signatures.
    pub fn strip_calls<'a>(
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        tree_sitter_lang: &tree_sitter::Language,
    ) -> Result<Vec<CallSignatureStub>> {
        let mut stubs = Vec::new();
        let mut seen = HashSet::new();

        let call_nodes = AstUtils::find_descendants_by_kind(target_node, "call_expression");
        let new_nodes = AstUtils::find_descendants_by_kind(target_node, "new_expression");

        let mut all_calls: Vec<(Option<String>, String)> = Vec::new();

        // 1. Process call_expressions
        for call in call_nodes {
            if let Some(fn_node) = call.child_by_field_name("function") {
                if fn_node.kind() == "identifier" {
                    let fn_name = AstUtils::node_text(fn_node, source).to_string();
                    if !is_builtin_global(&fn_name) {
                        all_calls.push((None, fn_name));
                    }
                } else if fn_node.kind() == "member_expression" {
                    if let (Some(obj), Some(prop)) = (
                        fn_node.child_by_field_name("object"),
                        fn_node.child_by_field_name("property"),
                    ) {
                        let receiver = AstUtils::node_text(obj, source).to_string();
                        let method = AstUtils::node_text(prop, source).to_string();
                        if !is_builtin_receiver_or_method(&receiver, &method) {
                            all_calls.push((Some(receiver), method));
                        }
                    }
                }
            }
        }

        // 2. Process new_expressions
        for new_node in new_nodes {
            if let Some(ctor_node) = new_node.child_by_field_name("constructor") {
                if ctor_node.kind() == "identifier" {
                    let ctor_name = AstUtils::node_text(ctor_node, source).to_string();
                    if !is_builtin_global(&ctor_name) {
                        all_calls.push((None, ctor_name));
                    }
                }
            }
        }

        let mut file_cache: HashMap<PathBuf, (String, tree_sitter::Tree)> = HashMap::new();
        let imports = ImportResolver::extract_imports(root, source);

        // 3. Resolve definitions for each call
        for (receiver, name) in all_calls {
            let key = format!("{:?}:{}", receiver, name);
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);

            // A. Check local file (e.g. this.method or local helper function)
            if receiver.as_deref() == Some("this") {
                if let Some(stub) =
                    find_method_in_class(target_node, root, source, &name, file_path)
                {
                    stubs.push(stub);
                    continue;
                }
            }

            if receiver.is_none() {
                if let Some(stub) = find_function_in_file(root, source, &name, file_path) {
                    stubs.push(stub);
                    continue;
                }
            }

            // B. Check imports directly for function / method
            let lookup_name = receiver.as_deref().unwrap_or(&name);
            let mut resolved = false;

            if let Some(mapping) = imports.get(lookup_name) {
                if let Some(target_file) =
                    ImportResolver::resolve_module_path(file_path, &mapping.specifier)
                {
                    if let Some(stub) = resolve_call_from_module(
                        &name,
                        &target_file,
                        tree_sitter_lang,
                        &mut file_cache,
                    ) {
                        stubs.push(stub);
                        resolved = true;
                    }
                }
            }

            // C. If not resolved, search all imported module files for matching method
            if !resolved {
                for mapping in imports.values() {
                    if let Some(target_file) =
                        ImportResolver::resolve_module_path(file_path, &mapping.specifier)
                    {
                        if let Some(stub) = resolve_call_from_module(
                            &name,
                            &target_file,
                            tree_sitter_lang,
                            &mut file_cache,
                        ) {
                            stubs.push(stub);
                            resolved = true;
                            break;
                        }
                    }
                }
            }

            // D. If receiver is this.field, try to resolve field type from class
            if !resolved {
                if let Some(ref r) = receiver {
                    if r.starts_with("this.") {
                        let field_name = r.strip_prefix("this.").unwrap_or(r);
                        if let Some(type_name) =
                            find_field_type_in_class(target_node, root, source, field_name)
                        {
                            if let Some(mapping) = imports.get(&type_name) {
                                if let Some(target_file) = ImportResolver::resolve_module_path(
                                    file_path,
                                    &mapping.specifier,
                                ) {
                                    if let Some(stub) = resolve_call_from_module(
                                        &name,
                                        &target_file,
                                        tree_sitter_lang,
                                        &mut file_cache,
                                    ) {
                                        stubs.push(stub);
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

    /// Locates a function or method signature in `target_file`, stripping its implementation body.
    pub fn resolve_foreign_signature(
        target_file: &Path,
        symbol_name: &str,
    ) -> Result<Option<CallSignatureStub>> {
        resolve_foreign_signature(target_file, symbol_name)
    }
}

/// Resolves a function or method signature from a target foreign file or directory across TS, Python, Go, and Rust.
pub fn resolve_foreign_signature(
    target_file: &Path,
    symbol_name: &str,
) -> Result<Option<CallSignatureStub>> {
    let clean_symbol = symbol_name.trim();
    let (container_query, member_query) = parse_query(clean_symbol);

    // 1. Directory case (e.g. Go package directory)
    if target_file.is_dir() {
        if let Ok(entries) = fs::read_dir(target_file) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let lang = SupportedLanguage::from_path(&p);
                    if let Some(l) = lang {
                        if let Ok(Some(stub)) =
                            extract_signature_from_single_file(&p, l, container_query, member_query)
                        {
                            return Ok(Some(stub));
                        }
                    }
                }
            }
        }
        return Ok(None);
    }

    // 2. Single file case
    let lang = SupportedLanguage::from_path(target_file).unwrap_or(SupportedLanguage::TypeScript);
    if let Ok(Some(stub)) =
        extract_signature_from_single_file(target_file, lang, container_query, member_query)
    {
        return Ok(Some(stub));
    }

    // 3. Fallback: Search sibling files in parent directory
    if let Some(parent_dir) = target_file.parent() {
        if let Ok(entries) = fs::read_dir(parent_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p != target_file {
                    let sibling_lang = SupportedLanguage::from_path(&p);
                    if sibling_lang == Some(lang) {
                        if let Ok(Some(stub)) = extract_signature_from_single_file(
                            &p,
                            lang,
                            container_query,
                            member_query,
                        ) {
                            return Ok(Some(stub));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
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

fn extract_signature_from_single_file(
    file_path: &Path,
    lang: SupportedLanguage,
    container_query: Option<&str>,
    member_query: &str,
) -> Result<Option<CallSignatureStub>> {
    let source = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };

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
    };

    let tree = match ParserManager::parse_source(&source, &ts_lang, file_path) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };

    let root = tree.root_node();

    match lang {
        SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
            if let Some(container) = container_query {
                if let Some(stub) = find_method_in_specific_container(
                    root,
                    &source,
                    container,
                    member_query,
                    file_path,
                ) {
                    return Ok(Some(stub));
                }
            }
            if let Some(stub) = find_function_in_file(root, &source, member_query, file_path) {
                return Ok(Some(stub));
            }
            if let Some(stub) = find_method_in_any_container(root, &source, member_query, file_path)
            {
                return Ok(Some(stub));
            }
        }
        SupportedLanguage::Python => {
            if let Some(stub) =
                find_python_signature(root, &source, container_query, member_query, file_path)
            {
                return Ok(Some(stub));
            }
        }
        SupportedLanguage::Go => {
            if let Some(stub) =
                find_go_signature(root, &source, container_query, member_query, file_path)
            {
                return Ok(Some(stub));
            }
        }
        SupportedLanguage::Rust => {
            if let Some(stub) =
                find_rust_signature(root, &source, container_query, member_query, file_path)
            {
                return Ok(Some(stub));
            }
        }
    }

    Ok(None)
}

fn find_field_type_in_class(
    target_node: Node<'_>,
    _root: Node<'_>,
    source: &str,
    field_name: &str,
) -> Option<String> {
    let mut current = target_node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "class_declaration" || parent.kind() == "abstract_class_declaration" {
            if let Some(body) = parent.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    if member.kind() == "property_definition"
                        || member.kind() == "public_field_definition"
                        || member.kind() == "field_definition"
                    {
                        if let Some(name_node) = member.child_by_field_name("name") {
                            if AstUtils::node_text(name_node, source) == field_name {
                                if let Some(type_node) = member.child_by_field_name("type") {
                                    let type_text = AstUtils::node_text(type_node, source)
                                        .trim_start_matches(':')
                                        .trim()
                                        .to_string();
                                    return Some(type_text);
                                }
                            }
                        }
                    } else if member.kind() == "method_definition" {
                        if let Some(name_node) = member.child_by_field_name("name") {
                            if AstUtils::node_text(name_node, source) == "constructor" {
                                if let Some(params) = member.child_by_field_name("parameters") {
                                    for p in params.named_children(&mut params.walk()) {
                                        if let Some(p_name) = p
                                            .child_by_field_name("name")
                                            .or_else(|| p.child_by_field_name("pattern"))
                                        {
                                            if AstUtils::node_text(p_name, source) == field_name {
                                                if let Some(t_node) = p.child_by_field_name("type")
                                                {
                                                    let type_text =
                                                        AstUtils::node_text(t_node, source)
                                                            .trim_start_matches(':')
                                                            .trim()
                                                            .to_string();
                                                    return Some(type_text);
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
            break;
        }
        current = parent;
    }
    None
}

fn find_function_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<CallSignatureStub> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = AstUtils::unwrap_export(child);

        if decl.kind() == "function_declaration" || decl.kind() == "generator_function_declaration"
        {
            if let Some(name_node) = decl.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == target_name {
                    let sig = extract_ts_signature_stub(child, decl, source);
                    return Some(CallSignatureStub {
                        name: target_name.to_string(),
                        receiver: None,
                        file_path: Some(file_path.to_string_lossy().to_string()),
                        signature: sig,
                    });
                }
            }
        }
    }
    None
}

fn find_method_in_class(
    target_node: Node<'_>,
    root: Node<'_>,
    source: &str,
    method_name: &str,
    file_path: &Path,
) -> Option<CallSignatureStub> {
    let mut current = target_node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "class_declaration" || parent.kind() == "abstract_class_declaration" {
            if let Some(body) = parent.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    if member.kind() == "method_definition" {
                        if let Some(name_node) = member.child_by_field_name("name") {
                            if AstUtils::node_text(name_node, source) == method_name {
                                let sig = extract_ts_signature_stub(member, member, source);
                                return Some(CallSignatureStub {
                                    name: method_name.to_string(),
                                    receiver: Some("this".to_string()),
                                    file_path: Some(file_path.to_string_lossy().to_string()),
                                    signature: sig,
                                });
                            }
                        }
                    }
                }
            }
            break;
        }
        current = parent;
    }

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = AstUtils::unwrap_export(child);
        if decl.kind() == "class_declaration" || decl.kind() == "abstract_class_declaration" {
            if let Some(body) = decl.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    if member.kind() == "method_definition" {
                        if let Some(m_name) = member.child_by_field_name("name") {
                            if AstUtils::node_text(m_name, source) == method_name {
                                let sig = extract_ts_signature_stub(member, member, source);
                                return Some(CallSignatureStub {
                                    name: method_name.to_string(),
                                    receiver: Some("this".to_string()),
                                    file_path: Some(file_path.to_string_lossy().to_string()),
                                    signature: sig,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn find_method_in_specific_container(
    root: Node<'_>,
    source: &str,
    container_name: &str,
    target_method: &str,
    file_path: &Path,
) -> Option<CallSignatureStub> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = AstUtils::unwrap_export(child);
        if matches!(
            decl.kind(),
            "class_declaration" | "abstract_class_declaration" | "interface_declaration"
        ) {
            let name = decl
                .child_by_field_name("name")
                .map(|n| AstUtils::node_text(n, source))
                .unwrap_or("");

            if name == container_name {
                if let Some(body) = decl.child_by_field_name("body") {
                    for member in body.named_children(&mut body.walk()) {
                        if matches!(
                            member.kind(),
                            "method_definition" | "method_signature" | "property_signature"
                        ) {
                            if let Some(m_name) = member.child_by_field_name("name") {
                                if AstUtils::node_text(m_name, source) == target_method {
                                    let sig = extract_ts_signature_stub(child, member, source);
                                    return Some(CallSignatureStub {
                                        name: target_method.to_string(),
                                        receiver: Some(container_name.to_string()),
                                        file_path: Some(file_path.to_string_lossy().to_string()),
                                        signature: sig,
                                    });
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

fn find_method_in_any_container(
    root: Node<'_>,
    source: &str,
    target_method: &str,
    file_path: &Path,
) -> Option<CallSignatureStub> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = AstUtils::unwrap_export(child);
        if matches!(
            decl.kind(),
            "class_declaration" | "abstract_class_declaration" | "interface_declaration"
        ) {
            let container_name = decl
                .child_by_field_name("name")
                .map(|n| AstUtils::node_text(n, source))
                .unwrap_or("Unknown");

            if let Some(body) = decl.child_by_field_name("body") {
                for member in body.named_children(&mut body.walk()) {
                    if matches!(
                        member.kind(),
                        "method_definition" | "method_signature" | "property_signature"
                    ) {
                        if let Some(m_name) = member.child_by_field_name("name") {
                            if AstUtils::node_text(m_name, source) == target_method {
                                let sig = extract_ts_signature_stub(child, member, source);
                                return Some(CallSignatureStub {
                                    name: target_method.to_string(),
                                    receiver: Some(container_name.to_string()),
                                    file_path: Some(file_path.to_string_lossy().to_string()),
                                    signature: sig,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn find_python_signature(
    root: Node<'_>,
    source: &str,
    container_query: Option<&str>,
    member_query: &str,
    file_path: &Path,
) -> Option<CallSignatureStub> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let node = if child.kind() == "decorated_definition" {
            child.child_by_field_name("definition").unwrap_or(child)
        } else {
            child
        };

        if node.kind() == "function_definition" && container_query.is_none() {
            if let Some(name_node) = node.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == member_query {
                    let sig = extract_python_signature_stub(node, source);
                    return Some(CallSignatureStub {
                        name: member_query.to_string(),
                        receiver: None,
                        file_path: Some(file_path.to_string_lossy().to_string()),
                        signature: sig,
                    });
                }
            }
        } else if node.kind() == "class_definition" {
            let class_name = node
                .child_by_field_name("name")
                .map(|n| AstUtils::node_text(n, source))
                .unwrap_or("");

            if container_query.is_none() || container_query == Some(class_name) {
                if let Some(body) = node.child_by_field_name("body") {
                    for member in body.named_children(&mut body.walk()) {
                        let m_node = if member.kind() == "decorated_definition" {
                            member.child_by_field_name("definition").unwrap_or(member)
                        } else {
                            member
                        };

                        if m_node.kind() == "function_definition" {
                            if let Some(m_name) = m_node.child_by_field_name("name") {
                                if AstUtils::node_text(m_name, source) == member_query {
                                    let sig = extract_python_signature_stub(m_node, source);
                                    return Some(CallSignatureStub {
                                        name: member_query.to_string(),
                                        receiver: Some(class_name.to_string()),
                                        file_path: Some(file_path.to_string_lossy().to_string()),
                                        signature: sig,
                                    });
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

fn find_go_signature(
    root: Node<'_>,
    source: &str,
    container_query: Option<&str>,
    member_query: &str,
    file_path: &Path,
) -> Option<CallSignatureStub> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "function_declaration" && container_query.is_none() {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == member_query {
                    let sig = extract_go_signature_stub(child, source);
                    return Some(CallSignatureStub {
                        name: member_query.to_string(),
                        receiver: None,
                        file_path: Some(file_path.to_string_lossy().to_string()),
                        signature: sig,
                    });
                }
            }
        } else if child.kind() == "method_declaration" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == member_query {
                    let rec_name = extract_go_receiver_type(child, source);
                    if container_query.is_none()
                        || container_query == rec_name.as_deref()
                        || container_query.map(|c| c.trim_start_matches('*'))
                            == rec_name.as_deref().map(|c| c.trim_start_matches('*'))
                    {
                        let sig = extract_go_signature_stub(child, source);
                        return Some(CallSignatureStub {
                            name: member_query.to_string(),
                            receiver: rec_name,
                            file_path: Some(file_path.to_string_lossy().to_string()),
                            signature: sig,
                        });
                    }
                }
            }
        }
    }
    None
}

fn find_rust_signature(
    root: Node<'_>,
    source: &str,
    container_query: Option<&str>,
    member_query: &str,
    file_path: &Path,
) -> Option<CallSignatureStub> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "function_item" && container_query.is_none() {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == member_query {
                    let sig = extract_rust_signature_stub(child, source);
                    return Some(CallSignatureStub {
                        name: member_query.to_string(),
                        receiver: None,
                        file_path: Some(file_path.to_string_lossy().to_string()),
                        signature: sig,
                    });
                }
            }
        } else if child.kind() == "impl_item" {
            let type_name = child
                .child_by_field_name("type")
                .map(|t| AstUtils::node_text(t, source).to_string())
                .unwrap_or_default();
            let base_type = type_name.split('<').next().unwrap_or(&type_name).trim();

            if container_query.is_none() || container_query == Some(base_type) {
                if let Some(body) = child.child_by_field_name("body") {
                    for item in body.named_children(&mut body.walk()) {
                        if item.kind() == "function_item" {
                            if let Some(m_name) = item.child_by_field_name("name") {
                                if AstUtils::node_text(m_name, source) == member_query {
                                    let sig = extract_rust_signature_stub(item, source);
                                    return Some(CallSignatureStub {
                                        name: member_query.to_string(),
                                        receiver: Some(base_type.to_string()),
                                        file_path: Some(file_path.to_string_lossy().to_string()),
                                        signature: sig,
                                    });
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

fn extract_ts_signature_stub(outer_node: Node<'_>, decl_node: Node<'_>, source: &str) -> String {
    let is_export = outer_node.kind() == "export_statement";
    let prefix = if is_export { "export " } else { "" };

    if decl_node.kind() == "function_declaration"
        || decl_node.kind() == "generator_function_declaration"
    {
        if let Some(body) = decl_node.child_by_field_name("body") {
            let start = decl_node.start_byte();
            let body_start = body.start_byte();
            if start <= body_start && body_start <= source.len() {
                let sig = source[start..body_start].trim();
                return format!("{prefix}{sig};");
            }
        }
    } else if decl_node.kind() == "method_definition" {
        if let Some(body) = decl_node.child_by_field_name("body") {
            let start = decl_node.start_byte();
            let body_start = body.start_byte();
            if start <= body_start && body_start <= source.len() {
                let sig = source[start..body_start].trim();
                return format!("{sig};");
            }
        }
    } else if decl_node.kind() == "variable_declarator" {
        if let Some(val) = decl_node.child_by_field_name("value") {
            if val.kind() == "arrow_function" {
                if let (Some(name_n), Some(params)) = (
                    decl_node.child_by_field_name("name"),
                    val.child_by_field_name("parameters"),
                ) {
                    let name_text = AstUtils::node_text(name_n, source);
                    let params_text = AstUtils::node_text(params, source);
                    let ret = val
                        .child_by_field_name("return_type")
                        .map(|r| AstUtils::node_text(r, source))
                        .unwrap_or("");
                    return format!("{prefix}const {name_text}: ({params_text}){ret};");
                }
            }
        }
    }

    let text = AstUtils::node_text(decl_node, source);
    let first_line = text.lines().next().unwrap_or(text).trim();
    let trimmed = first_line.trim_end_matches('{').trim();
    format!("{prefix}{trimmed};")
}

fn extract_python_signature_stub(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let start = node.start_byte();
        let body_start = body.start_byte();
        if start <= body_start && body_start <= source.len() {
            let sig = source[start..body_start].trim();
            let clean = sig.trim_end_matches(':').trim();
            return format!("{clean}: ...");
        }
    }
    let text = AstUtils::node_text(node, source);
    let first_line = text.lines().next().unwrap_or(text).trim();
    let clean = first_line.trim_end_matches(':').trim();
    format!("{clean}: ...")
}

fn extract_go_signature_stub(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let start = node.start_byte();
        let body_start = body.start_byte();
        if start <= body_start && body_start <= source.len() {
            return source[start..body_start].trim().to_string();
        }
    }
    let text = AstUtils::node_text(node, source);
    let first_line = text.lines().next().unwrap_or(text).trim();
    first_line.trim_end_matches('{').trim().to_string()
}

fn extract_go_receiver_type(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(receiver) = node.child_by_field_name("receiver") {
        for param in AstUtils::find_children_by_kind(receiver, "parameter_declaration") {
            if let Some(type_node) = param.child_by_field_name("type") {
                let rec = AstUtils::node_text(type_node, source);
                return Some(rec.trim_start_matches('*').to_string());
            }
        }
    }
    None
}

fn extract_rust_signature_stub(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let start = node.start_byte();
        let body_start = body.start_byte();
        if start <= body_start && body_start <= source.len() {
            let sig = source[start..body_start].trim();
            return format!("{sig};");
        }
    }
    let text = AstUtils::node_text(node, source);
    let first_line = text.lines().next().unwrap_or(text).trim();
    let trimmed = first_line.trim_end_matches('{').trim();
    format!("{trimmed};")
}

fn resolve_call_from_module(
    name: &str,
    target_file: &Path,
    tree_sitter_lang: &tree_sitter::Language,
    cache: &mut HashMap<PathBuf, (String, tree_sitter::Tree)>,
) -> Option<CallSignatureStub> {
    let (source, tree) = get_or_load_file(target_file, tree_sitter_lang, cache)?;
    let root = tree.root_node();

    // 1. Direct function in target file
    if let Some(stub) = find_function_in_file(root, source, name, target_file) {
        return Some(stub);
    }

    // 2. Direct method in any class or interface in target file
    if let Some(stub) = find_method_in_any_container(root, source, name, target_file) {
        return Some(stub);
    }

    // 3. Check barrel re-exports
    let reexports = ImportResolver::extract_reexports(root, source);
    for (exported_alias, specifier) in reexports {
        if let Some(alias) = exported_alias {
            if alias == name {
                if let Some(sub_file) = ImportResolver::resolve_module_path(target_file, &specifier)
                {
                    if let Some(res) =
                        resolve_call_from_module(name, &sub_file, tree_sitter_lang, cache)
                    {
                        return Some(res);
                    }
                }
            }
        } else {
            // Wildcard export *
            if let Some(sub_file) = ImportResolver::resolve_module_path(target_file, &specifier) {
                if let Some(res) =
                    resolve_call_from_module(name, &sub_file, tree_sitter_lang, cache)
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

fn is_builtin_global(name: &str) -> bool {
    matches!(
        name,
        "parseInt"
            | "parseFloat"
            | "isNaN"
            | "isFinite"
            | "encodeURIComponent"
            | "decodeURIComponent"
            | "encodeURI"
            | "decodeURI"
            | "setTimeout"
            | "clearTimeout"
            | "setInterval"
            | "clearInterval"
            | "fetch"
            | "structuredClone"
            | "atob"
            | "btoa"
            | "require"
            | "import"
            | "super"
            | "Error"
            | "TypeError"
            | "RangeError"
            | "SyntaxError"
            | "Date"
            | "RegExp"
            | "Map"
            | "Set"
            | "Promise"
            | "Array"
            | "Object"
            | "String"
            | "Number"
            | "Boolean"
            | "Symbol"
            | "BigInt"
    )
}

fn is_builtin_receiver_or_method(receiver: &str, method: &str) -> bool {
    matches!(
        receiver,
        "console"
            | "Math"
            | "JSON"
            | "Object"
            | "Array"
            | "String"
            | "Number"
            | "Promise"
            | "Reflect"
            | "process"
    ) || matches!(
        method,
        "log"
            | "warn"
            | "error"
            | "info"
            | "debug"
            | "trace"
            | "map"
            | "filter"
            | "reduce"
            | "forEach"
            | "some"
            | "every"
            | "find"
            | "findIndex"
            | "includes"
            | "slice"
            | "splice"
            | "concat"
            | "join"
            | "push"
            | "pop"
            | "shift"
            | "unshift"
            | "flat"
            | "flatMap"
            | "sort"
            | "reverse"
            | "toLowerCase"
            | "toUpperCase"
            | "trim"
            | "trimStart"
            | "trimEnd"
            | "split"
            | "replace"
            | "replaceAll"
            | "substring"
            | "startsWith"
            | "endsWith"
            | "indexOf"
            | "padStart"
            | "padEnd"
            | "charAt"
            | "charCodeAt"
            | "match"
            | "search"
            | "keys"
            | "values"
            | "entries"
            | "assign"
            | "all"
            | "resolve"
            | "reject"
            | "allSettled"
            | "race"
            | "toString"
            | "toISOString"
            | "toUTCString"
            | "toLocaleDateString"
            | "getTime"
            | "getDate"
            | "getFullYear"
            | "getMonth"
            | "getDay"
            | "getHours"
            | "getMinutes"
            | "getSeconds"
            | "now"
            | "parse"
            | "stringify"
            | "valueOf"
            | "hasOwnProperty"
    )
}
