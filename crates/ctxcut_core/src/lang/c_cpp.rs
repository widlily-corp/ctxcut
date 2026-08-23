//! LanguageAdapter implementation for C and C++.

use crate::error::{CoreError, Result};
use crate::lang::LanguageAdapter;
use crate::model::{
    CallSignatureStub, ExtractedImplementor, ExtractedSymbol, ExtractedType, SliceOptions,
    SupportedLanguage,
};
use crate::parser::{AstUtils, ParserManager};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
use tree_sitter::{Language, Node};

/// C language adapter supporting C (.c, .h).
pub struct CAdapter;

/// C++ language adapter supporting C++ (.cpp, .cc, .cxx, .hpp, .hh, .hxx).
pub struct CppAdapter;

impl LanguageAdapter for CAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::C
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_c::LANGUAGE.into()
    }

    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        locate_c_cpp_symbol(root, source, symbol_query, file_path, "c", false)
    }

    fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String> {
        list_c_cpp_symbols(root, source, false)
    }

    fn hoist_types<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>> {
        let ts_lang = self.tree_sitter_language(file_path);
        hoist_c_cpp_types(target_node, root, source, file_path, opts, &ts_lang, false)
    }

    fn strip_calls<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
    ) -> Result<Vec<CallSignatureStub>> {
        let ts_lang = self.tree_sitter_language(file_path);
        strip_c_cpp_calls(target_node, root, source, file_path, &ts_lang)
    }

    fn find_implementors<'a>(
        &self,
        _root: Node<'a>,
        _source: &'a str,
        _interface_name: &str,
        _file_path: &Path,
    ) -> Result<Vec<ExtractedImplementor>> {
        // C has no class inheritance / implementors
        Ok(Vec::new())
    }
}

impl LanguageAdapter for CppAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Cpp
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        locate_c_cpp_symbol(root, source, symbol_query, file_path, "cpp", true)
    }

    fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String> {
        list_c_cpp_symbols(root, source, true)
    }

    fn hoist_types<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>> {
        let ts_lang = self.tree_sitter_language(file_path);
        hoist_c_cpp_types(target_node, root, source, file_path, opts, &ts_lang, true)
    }

    fn strip_calls<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
    ) -> Result<Vec<CallSignatureStub>> {
        let ts_lang = self.tree_sitter_language(file_path);
        strip_c_cpp_calls(target_node, root, source, file_path, &ts_lang)
    }

    fn find_implementors<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        interface_name: &str,
        file_path: &Path,
    ) -> Result<Vec<ExtractedImplementor>> {
        find_cpp_implementors(root, source, interface_name, file_path)
    }
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

fn locate_c_cpp_symbol<'a>(
    root: Node<'a>,
    source: &'a str,
    symbol_query: &str,
    file_path: &Path,
    lang_name: &str,
    is_cpp: bool,
) -> Result<(ExtractedSymbol, Node<'a>)> {
    let (container_query, member_query) = parse_query(symbol_query);

    if let Some(container_name) = container_query {
        if let Some((sym, node)) = find_in_container(root, source, container_name, member_query, file_path, lang_name) {
            return Ok((sym, node));
        }
        // Also check out-of-class definition: Type Container::method(...)
        if let Some((sym, node)) = find_qualified_method(root, source, container_name, member_query, file_path, lang_name) {
            return Ok((sym, node));
        }
    } else {
        if let Some((sym, node)) = find_top_level(root, source, member_query, file_path, lang_name) {
            return Ok((sym, node));
        }
        if is_cpp {
            if let Some((sym, node)) = find_any_method(root, source, member_query, file_path, lang_name) {
                return Ok((sym, node));
            }
        }
        if let Some(sym) = fallback_c_cpp_source_scan(source, member_query, file_path, lang_name) {
            return Ok((sym, root));
        }
    }

    let available = list_c_cpp_symbols(root, source, is_cpp);
    Err(CoreError::SymbolNotFound {
        symbol: symbol_query.to_string(),
        path: file_path.to_path_buf(),
        available_symbols: available,
    })
}

fn find_top_level<'a>(
    root: Node<'a>,
    source: &'a str,
    target_name: &str,
    file_path: &Path,
    lang_name: &str,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    find_symbol_recursive(root, source, target_name, file_path, lang_name)
}

fn find_symbol_recursive<'a>(
    node: Node<'a>,
    source: &'a str,
    target_name: &str,
    file_path: &Path,
    lang_name: &str,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "template_declaration" => {
                if let Some(inner) = child.named_children(&mut child.walk()).last() {
                    if let Some(name) = extract_c_cpp_node_name(inner, source) {
                        if name == target_name {
                            return Some((build_c_cpp_symbol(child, source, file_path, lang_name, &name), child));
                        }
                    }
                }
            }
            "function_definition" => {
                if let Some(name) = extract_c_cpp_function_name(child, source) {
                    if name == target_name || name.ends_with(&format!("::{target_name}")) {
                        return Some((build_c_cpp_symbol(child, source, file_path, lang_name, &name), child));
                    }
                }
            }
            "class_specifier" | "struct_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_node, source);
                    if name == target_name {
                        let parent = child.parent().unwrap_or(child);
                        let target = if parent.kind() == "type_definition" || parent.kind() == "declaration" { parent } else { child };
                        return Some((build_c_cpp_symbol(target, source, file_path, lang_name, name), target));
                    }
                }
            }
            "type_definition" => {
                if let Some(name) = extract_typedef_name(child, source) {
                    if name == target_name {
                        return Some((build_c_cpp_symbol(child, source, file_path, lang_name, &name), child));
                    }
                }
            }
            "enum_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_node, source);
                    if name == target_name {
                        let parent = child.parent().unwrap_or(child);
                        let target = if parent.kind() == "type_definition" || parent.kind() == "declaration" { parent } else { child };
                        return Some((build_c_cpp_symbol(target, source, file_path, lang_name, name), target));
                    }
                }
            }
            "namespace_definition" | "linkage_specification" => {
                if let Some(body) = child.child_by_field_name("body").or_else(|| AstUtils::find_child_by_kind(child, "declaration_list")) {
                    if let Some(found) = find_symbol_recursive(body, source, target_name, file_path, lang_name) {
                        return Some(found);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_in_container<'a>(
    root: Node<'a>,
    source: &'a str,
    container_name: &str,
    member_name: &str,
    file_path: &Path,
    lang_name: &str,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let classes = AstUtils::find_descendants_by_kind(root, "class_specifier");
    let structs = AstUtils::find_descendants_by_kind(root, "struct_specifier");

    for node in classes.into_iter().chain(structs) {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == container_name {
                if let Some(body) = node.child_by_field_name("body") {
                    for member in body.named_children(&mut body.walk()) {
                        let (target_node, effective_member) = if member.kind() == "template_declaration" {
                            let last = member.named_children(&mut member.walk()).last();
                            (member, last.unwrap_or(member))
                        } else {
                            (member, member)
                        };

                        if effective_member.kind() == "function_definition" {
                            if let Some(name) = extract_c_cpp_function_name(effective_member, source) {
                                if name == member_name {
                                    let full_name = format!("{container_name}::{member_name}");
                                    return Some((build_c_cpp_symbol(target_node, source, file_path, lang_name, &full_name), target_node));
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

fn find_qualified_method<'a>(
    root: Node<'a>,
    source: &'a str,
    container_name: &str,
    member_name: &str,
    file_path: &Path,
    lang_name: &str,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let qualified_target = format!("{container_name}::{member_name}");
    let functions = AstUtils::find_descendants_by_kind(root, "function_definition");
    for fn_node in functions {
        let parent = fn_node.parent();
        let target = if let Some(p) = parent {
            if p.kind() == "template_declaration" { p } else { fn_node }
        } else {
            fn_node
        };

        if let Some(name) = extract_c_cpp_function_name(fn_node, source) {
            if name == qualified_target {
                return Some((build_c_cpp_symbol(target, source, file_path, lang_name, &qualified_target), target));
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
    lang_name: &str,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let classes = AstUtils::find_descendants_by_kind(root, "class_specifier");
    let structs = AstUtils::find_descendants_by_kind(root, "struct_specifier");

    for node in classes.into_iter().chain(structs) {
        let container_name = node
            .child_by_field_name("name")
            .map(|n| AstUtils::node_text(n, source).to_string())
            .unwrap_or_else(|| "Anonymous".to_string());

        if let Some(body) = node.child_by_field_name("body") {
            for member in body.named_children(&mut body.walk()) {
                let (target_node, effective_member) = if member.kind() == "template_declaration" {
                    let last = member.named_children(&mut member.walk()).last();
                    (member, last.unwrap_or(member))
                } else {
                    (member, member)
                };

                if effective_member.kind() == "function_definition" {
                    if let Some(name) = extract_c_cpp_function_name(effective_member, source) {
                        if name == member_name {
                            let full_name = format!("{container_name}::{member_name}");
                            return Some((build_c_cpp_symbol(target_node, source, file_path, lang_name, &full_name), target_node));
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_c_cpp_node_name(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "function_definition" => extract_c_cpp_function_name(node, source),
        "class_specifier" | "struct_specifier" | "enum_specifier" => {
            node.child_by_field_name("name").map(|n| AstUtils::node_text(n, source).to_string())
        }
        "type_definition" => extract_typedef_name(node, source),
        _ => None,
    }
}

fn extract_c_cpp_function_name(node: Node<'_>, source: &str) -> Option<String> {
    let declarator = node.child_by_field_name("declarator")?;
    find_function_name_in_declarator(declarator, source)
}

fn find_function_name_in_declarator(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" | "qualified_identifier" | "destructor_name" | "operator_name" => {
            Some(AstUtils::node_text(node, source).to_string())
        }
        "function_declarator" | "pointer_declarator" | "reference_declarator" | "parenthesized_declarator" => {
            if let Some(inner) = node.child_by_field_name("declarator") {
                find_function_name_in_declarator(inner, source)
            } else if let Some(first) = node.named_child(0) {
                find_function_name_in_declarator(first, source)
            } else {
                None
            }
        }
        _ => {
            for child in node.named_children(&mut node.walk()) {
                if let Some(found) = find_function_name_in_declarator(child, source) {
                    return Some(found);
                }
            }
            None
        }
    }
}

fn extract_typedef_name(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(declarator) = node.child_by_field_name("declarator") {
        return Some(AstUtils::node_text(declarator, source).to_string());
    }
    if let Some(last) = node.named_children(&mut node.walk()).last() {
        if last.kind() == "type_identifier" || last.kind() == "identifier" {
            return Some(AstUtils::node_text(last, source).to_string());
        }
    }
    None
}

fn build_c_cpp_symbol(
    node: Node<'_>,
    source: &str,
    file_path: &Path,
    lang_name: &str,
    name: &str,
) -> ExtractedSymbol {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let doc_comment = extract_c_cpp_doc_comment(node, source);
    let signature = extract_c_cpp_signature(node, source);
    let body = AstUtils::node_text(node, source).to_string();

    let kind = match node.kind() {
        "class_specifier" => "class",
        "struct_specifier" => "struct",
        "enum_specifier" => "enum",
        "type_definition" => "type",
        "function_definition" => {
            if name.contains("::") { "method" } else { "function" }
        }
        "template_declaration" => {
            if let Some(last) = node.named_children(&mut node.walk()).last() {
                match last.kind() {
                    "class_specifier" => "class",
                    "struct_specifier" => "struct",
                    _ => if name.contains("::") { "method" } else { "function" },
                }
            } else {
                "template"
            }
        }
        _ => "function",
    };

    ExtractedSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        start_line,
        end_line,
        doc_comment,
        signature,
        body,
        language: lang_name.to_string(),
    }
}

fn extract_c_cpp_doc_comment(node: Node<'_>, source: &str) -> Option<String> {
    let mut comments = Vec::new();
    let mut prev = node.prev_named_sibling();

    while let Some(p) = prev {
        if p.kind() == "comment" {
            comments.push(AstUtils::node_text(p, source).trim().to_string());
            prev = p.prev_named_sibling();
        } else {
            break;
        }
    }

    if comments.is_empty() {
        None
    } else {
        comments.reverse();
        Some(comments.join("\n"))
    }
}

fn extract_c_cpp_signature(node: Node<'_>, source: &str) -> String {
    let raw = AstUtils::node_text(node, source);
    if node.kind() == "function_definition" {
        if let Some(body) = node.child_by_field_name("body") {
            let offset = body.start_byte() - node.start_byte();
            return raw[..offset].trim().to_string();
        }
    } else if node.kind() == "template_declaration" {
        if let Some(last) = node.named_children(&mut node.walk()).last() {
            if last.kind() == "function_definition" {
                if let Some(body) = last.child_by_field_name("body") {
                    let offset = body.start_byte() - node.start_byte();
                    return raw[..offset].trim().to_string();
                }
            }
        }
    }
    if let Some(idx) = raw.find('{') {
        raw[..idx].trim().to_string()
    } else if let Some(idx) = raw.find(';') {
        raw[..=idx].trim().to_string()
    } else {
        raw.lines().next().unwrap_or(raw).trim().to_string()
    }
}

fn fallback_c_cpp_source_scan(
    source: &str,
    target_name: &str,
    file_path: &Path,
    lang_name: &str,
) -> Option<ExtractedSymbol> {
    let patterns = [
        format!(" {target_name}("),
        format!(" {target_name} ("),
        format!("::{target_name}("),
        format!("struct {target_name}"),
        format!("class {target_name}"),
        format!("typedef struct {target_name}"),
        "typedef struct {".to_string(),
    ];

    for (line_idx, line) in source.lines().enumerate() {
        for pat in &patterns {
            if line.contains(pat) {
                let start_line = line_idx + 1;
                let body_lines: Vec<&str> = source.lines().skip(line_idx).take(30).collect();
                let body = body_lines.join("\n");
                let signature = line.trim().to_string();

                return Some(ExtractedSymbol {
                    name: target_name.to_string(),
                    kind: "function".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    start_line,
                    end_line: start_line + body_lines.len().saturating_sub(1),
                    doc_comment: None,
                    signature,
                    body,
                    language: lang_name.to_string(),
                });
            }
        }
    }
    None
}

fn list_c_cpp_symbols(root: Node<'_>, source: &str, is_cpp: bool) -> Vec<String> {
    let mut symbols = Vec::new();
    collect_symbols_recursive(root, source, &mut symbols, None, is_cpp);
    symbols
}

fn collect_symbols_recursive(
    node: Node<'_>,
    source: &str,
    symbols: &mut Vec<String>,
    current_container: Option<&str>,
    is_cpp: bool,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "template_declaration" => {
                if let Some(inner) = child.named_children(&mut child.walk()).last() {
                    if let Some(name) = extract_c_cpp_node_name(inner, source) {
                        let sym = match current_container {
                            Some(c) => format!("{c}::{name}"),
                            None => name,
                        };
                        symbols.push(sym);
                    }
                }
            }
            "function_definition" => {
                if let Some(name) = extract_c_cpp_function_name(child, source) {
                    let sym = match current_container {
                        Some(c) if !name.contains("::") => format!("{c}::{name}"),
                        _ => name,
                    };
                    symbols.push(sym);
                }
            }
            "class_specifier" | "struct_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let class_name = AstUtils::node_text(name_node, source).to_string();
                    let full_class = match current_container {
                        Some(c) => format!("{c}::{class_name}"),
                        None => class_name.clone(),
                    };
                    symbols.push(full_class.clone());
                    if is_cpp {
                        if let Some(body) = child.child_by_field_name("body") {
                            collect_symbols_recursive(body, source, symbols, Some(&full_class), is_cpp);
                        }
                    }
                }
            }
            "type_definition" => {
                if let Some(name) = extract_typedef_name(child, source) {
                    symbols.push(name);
                }
            }
            "enum_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    symbols.push(AstUtils::node_text(name_node, source).to_string());
                }
            }
            "namespace_definition" => {
                let ns_name = child
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source).to_string());
                if let Some(body) = child.child_by_field_name("body") {
                    collect_symbols_recursive(body, source, symbols, ns_name.as_deref(), is_cpp);
                }
            }
            "linkage_specification" => {
                if let Some(body) = AstUtils::find_child_by_kind(child, "declaration_list") {
                    collect_symbols_recursive(body, source, symbols, current_container, is_cpp);
                }
            }
            _ => {}
        }
    }
}

fn is_builtin_c_cpp_type(name: &str) -> bool {
    let clean = name.trim().trim_start_matches("struct ").trim_start_matches("class ").trim();
    matches!(
        clean,
        "int" | "char" | "float" | "double" | "void" | "size_t" | "ssize_t"
            | "uint8_t" | "uint16_t" | "uint32_t" | "uint64_t"
            | "int8_t" | "int16_t" | "int32_t" | "int64_t"
            | "bool" | "long" | "short" | "unsigned" | "signed" | "auto"
            | "nullptr_t" | "ptrdiff_t" | "intptr_t" | "uintptr_t"
            | "string" | "std::string" | "vector" | "std::vector"
            | "map" | "std::map" | "unordered_map" | "std::unordered_map"
            | "set" | "std::set" | "unordered_set" | "std::unordered_set"
            | "unique_ptr" | "std::unique_ptr" | "shared_ptr" | "std::shared_ptr"
            | "weak_ptr" | "std::weak_ptr" | "optional" | "std::optional"
            | "pair" | "std::pair" | "tuple" | "std::tuple"
            | "array" | "std::array" | "deque" | "std::deque"
            | "list" | "std::list" | "queue" | "std::queue" | "stack" | "std::stack"
            | "ostream" | "istream" | "iostream" | "stringstream"
            | "cin" | "cout" | "cerr" | "FILE"
    )
}

fn collect_c_cpp_scoped_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut current = Some(node);
    while let Some(n) = current {
        if n.kind() == "template_declaration" {
            if let Some(params) = n.child_by_field_name("parameters") {
                for param in params.named_children(&mut params.walk()) {
                    if let Some(name_node) = param.child_by_field_name("name") {
                        set.insert(AstUtils::node_text(name_node, source).to_string());
                    } else if param.kind() == "type_parameter_declaration" {
                        let text = AstUtils::node_text(param, source);
                        if let Some(last) = text.split_whitespace().last() {
                            set.insert(last.to_string());
                        }
                    }
                }
            }
        }
        current = n.parent();
    }
    set
}

fn hoist_c_cpp_types<'a>(
    target_node: Node<'a>,
    root: Node<'a>,
    source: &'a str,
    file_path: &Path,
    opts: &SliceOptions,
    ts_lang: &Language,
    _is_cpp: bool,
) -> Result<Vec<ExtractedType>> {
    let mut hoisted = Vec::new();
    let mut visited = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    let scoped_generics = collect_c_cpp_scoped_generics(target_node, source);

    for id in AstUtils::find_descendants_by_kind(target_node, "type_identifier") {
        let name = AstUtils::node_text(id, source);
        if !is_builtin_c_cpp_type(name)
            && !scoped_generics.contains(name)
            && visited.insert(name.to_string())
        {
            queue.push_back((name.to_string(), 1));
        }
    }

    let dir = file_path.parent().unwrap_or_else(|| Path::new("."));

    while let Some((type_name, depth)) = queue.pop_front() {
        if is_builtin_c_cpp_type(&type_name) || scoped_generics.contains(&type_name) {
            continue;
        }

        // Check local file
        if let Some(extracted) = find_c_cpp_type_in_file(root, source, &type_name, file_path) {
            if depth < opts.depth {
                if let Ok(tree) = ParserManager::parse_source(&extracted.definition, ts_lang, file_path) {
                    for id in AstUtils::find_descendants_by_kind(tree.root_node(), "type_identifier") {
                        let nested = AstUtils::node_text(id, &extracted.definition);
                        if !is_builtin_c_cpp_type(nested) && visited.insert(nested.to_string()) {
                            queue.push_back((nested.to_string(), depth + 1));
                        }
                    }
                }
            }
            hoisted.push(extracted);
            continue;
        }

        // Check header includes (#include "..." or sibling .h/.hpp)
        if depth <= opts.depth {
            let includes = collect_c_cpp_includes(root, source);
            let mut candidate_files = Vec::new();
            for inc in includes {
                let inc_path = dir.join(&inc);
                if inc_path.is_file() {
                    candidate_files.push(inc_path);
                }
            }

            if candidate_files.is_empty() {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                        if p.is_file() && p != file_path && matches!(ext.as_str(), "h" | "hpp" | "hh" | "hxx") {
                            candidate_files.push(p);
                        }
                    }
                }
            }

            for cand_path in candidate_files {
                if let Ok(cand_source) = fs::read_to_string(&cand_path) {
                    if let Ok(tree) = ParserManager::parse_source(&cand_source, ts_lang, &cand_path) {
                        if let Some(extracted) = find_c_cpp_type_in_file(tree.root_node(), &cand_source, &type_name, &cand_path) {
                            if depth < opts.depth {
                                for id in AstUtils::find_descendants_by_kind(tree.root_node(), "type_identifier") {
                                    let nested = AstUtils::node_text(id, &cand_source);
                                    if !is_builtin_c_cpp_type(nested) && visited.insert(nested.to_string()) {
                                        queue.push_back((nested.to_string(), depth + 1));
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
    }

    Ok(hoisted)
}

fn collect_c_cpp_includes(root: Node<'_>, source: &str) -> Vec<String> {
    let mut includes = Vec::new();
    let preprocs = AstUtils::find_descendants_by_kind(root, "preproc_include");
    for inc in preprocs {
        if let Some(path_node) = inc.child_by_field_name("path") {
            let raw = AstUtils::node_text(path_node, source).trim_matches(['"', '<', '>']);
            includes.push(raw.to_string());
        }
    }
    includes
}

fn find_c_cpp_type_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    for child in AstUtils::find_descendants_by_kind(root, "class_specifier") {
        if let Some(name_node) = child.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                let parent = child.parent().unwrap_or(child);
                let target = if parent.kind() == "type_definition" || parent.kind() == "declaration" { parent } else { child };
                let mut def = AstUtils::node_text(target, source).to_string();
                if !def.ends_with(';') { def.push(';'); }
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "class".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: def,
                });
            }
        }
    }

    for child in AstUtils::find_descendants_by_kind(root, "struct_specifier") {
        if let Some(name_node) = child.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                let parent = child.parent().unwrap_or(child);
                let target = if parent.kind() == "type_definition" || parent.kind() == "declaration" { parent } else { child };
                let mut def = AstUtils::node_text(target, source).to_string();
                if !def.ends_with(';') { def.push(';'); }
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "struct".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: def,
                });
            }
        }
    }

    for child in AstUtils::find_descendants_by_kind(root, "type_definition") {
        if let Some(name) = extract_typedef_name(child, source) {
            if name == target_name {
                let mut def = AstUtils::node_text(child, source).to_string();
                if !def.ends_with(';') { def.push(';'); }
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "type_alias".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: def,
                });
            }
        }
    }

    for child in AstUtils::find_descendants_by_kind(root, "enum_specifier") {
        if let Some(name_node) = child.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                let parent = child.parent().unwrap_or(child);
                let target = if parent.kind() == "type_definition" || parent.kind() == "declaration" { parent } else { child };
                let mut def = AstUtils::node_text(target, source).to_string();
                if !def.ends_with(';') { def.push(';'); }
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "enum".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: def,
                });
            }
        }
    }

    None
}

fn strip_c_cpp_calls<'a>(
    target_node: Node<'a>,
    root: Node<'a>,
    source: &'a str,
    file_path: &Path,
    _ts_lang: &Language,
) -> Result<Vec<CallSignatureStub>> {
    let mut stubs = Vec::new();
    let mut seen = HashSet::new();

    let calls = AstUtils::find_descendants_by_kind(target_node, "call_expression");
    for call in calls {
        if let Some(fn_node) = call.child_by_field_name("function") {
            let (receiver, func_name) = match fn_node.kind() {
                "identifier" | "type_identifier" => (None, AstUtils::node_text(fn_node, source).to_string()),
                "field_expression" => {
                    let obj = fn_node.child_by_field_name("argument");
                    let field = fn_node.child_by_field_name("field");
                    let obj_name = obj.map(|o| AstUtils::node_text(o, source).to_string());
                    let f_name = field.map(|f| AstUtils::node_text(f, source).to_string()).unwrap_or_default();
                    (obj_name, f_name)
                }
                "qualified_identifier" => {
                    let scope = fn_node.child_by_field_name("scope");
                    let name = fn_node.child_by_field_name("name");
                    let scope_name = scope.map(|s| AstUtils::node_text(s, source).to_string());
                    let n_name = name.map(|n| AstUtils::node_text(n, source).to_string()).unwrap_or_default();
                    (scope_name, n_name)
                }
                _ => (None, AstUtils::node_text(fn_node, source).to_string()),
            };

            if func_name.is_empty() || is_builtin_c_cpp_function(&func_name) {
                continue;
            }

            if seen.insert(func_name.clone()) {
                if let Some(sig) = find_c_cpp_function_signature(root, source, &func_name) {
                    stubs.push(CallSignatureStub {
                        name: func_name,
                        receiver,
                        file_path: Some(file_path.to_string_lossy().to_string()),
                        signature: format!("{sig};"),
                    });
                }
            }
        }
    }

    Ok(stubs)
}

fn is_builtin_c_cpp_function(name: &str) -> bool {
    matches!(
        name,
        "printf" | "sprintf" | "snprintf" | "fprintf" | "scanf" | "sscanf"
            | "malloc" | "calloc" | "realloc" | "free"
            | "memcpy" | "memset" | "memmove" | "memcmp"
            | "strlen" | "strcpy" | "strncpy" | "strcmp" | "strncmp"
            | "strcat" | "strncat" | "strchr" | "strstr"
            | "abs" | "min" | "max" | "exit" | "abort" | "assert"
            | "sizeof" | "alignof" | "move" | "forward"
            | "make_unique" | "make_shared"
            | "push_back" | "emplace_back" | "pop_back" | "insert" | "erase"
            | "clear" | "size" | "empty" | "begin" | "end" | "c_str"
            | "data" | "find" | "substr" | "reserve" | "resize"
    )
}

fn find_c_cpp_function_signature(root: Node<'_>, source: &str, target_name: &str) -> Option<String> {
    for fn_node in AstUtils::find_descendants_by_kind(root, "function_definition") {
        if let Some(name) = extract_c_cpp_function_name(fn_node, source) {
            if name == target_name || name.ends_with(&format!("::{target_name}")) {
                return Some(extract_c_cpp_signature(fn_node, source));
            }
        }
    }
    for decl_node in AstUtils::find_descendants_by_kind(root, "declaration") {
        if let Some(declarator) = decl_node.child_by_field_name("declarator") {
            if let Some(name) = find_function_name_in_declarator(declarator, source) {
                if name == target_name {
                    return Some(AstUtils::node_text(decl_node, source).trim_end_matches(';').trim().to_string());
                }
            }
        }
    }
    None
}

fn find_cpp_implementors(
    root: Node<'_>,
    source: &str,
    interface_name: &str,
    file_path: &Path,
) -> Result<Vec<ExtractedImplementor>> {
    let mut implementors = Vec::new();
    let classes = AstUtils::find_descendants_by_kind(root, "class_specifier");
    let structs = AstUtils::find_descendants_by_kind(root, "struct_specifier");

    for node in classes.into_iter().chain(structs) {
        if let Some(base_clause) = AstUtils::find_child_by_kind(node, "base_class_clause") {
            let base_text = AstUtils::node_text(base_clause, source);
            if base_text.split(|c: char| c == ',' || c.is_whitespace() || c == ':').any(|part| part.trim() == interface_name) {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let class_name = AstUtils::node_text(name_node, source).to_string();
                    let stub = extract_cpp_class_stub(node, source);
                    implementors.push(ExtractedImplementor {
                        interface_name: interface_name.to_string(),
                        implementor_name: class_name,
                        kind: "cpp_class".to_string(),
                        file_path: file_path.to_string_lossy().to_string(),
                        definition: stub,
                    });
                }
            }
        }
    }

    Ok(implementors)
}

fn extract_cpp_class_stub(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let header_end = body.start_byte();
        let header = source[node.start_byte()..header_end].trim();
        let mut stubs = Vec::new();

        for member in body.named_children(&mut body.walk()) {
            if member.kind() == "function_definition" {
                let sig = extract_c_cpp_signature(member, source);
                stubs.push(format!("    {sig} {{ ... }}"));
            } else if member.kind() == "declaration" {
                let decl = AstUtils::node_text(member, source).trim();
                stubs.push(format!("    {decl}"));
            }
        }

        format!("{header} {{\n{}\n}};", stubs.join("\n"))
    } else {
        AstUtils::node_text(node, source).to_string()
    }
}
