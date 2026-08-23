//! LanguageAdapter implementation for Kotlin.

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

/// Kotlin language adapter supporting Kotlin (.kt, .kts).
pub struct KotlinAdapter;

impl LanguageAdapter for KotlinAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Kotlin
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_kotlin::LANGUAGE.into()
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
            // 1. Container.member or Container.Companion.member
            if let Some((sym, node)) = find_in_container(root, source, container_name, member_query, file_path) {
                return Ok((sym, node));
            }
            // 2. Extension function Receiver.func
            if let Some((sym, node)) = find_extension_function(root, source, container_name, member_query, file_path) {
                return Ok((sym, node));
            }
        } else {
            // 3. Top-level function, class, object, or interface
            if let Some((sym, node)) = find_top_level(root, source, member_query, file_path) {
                return Ok((sym, node));
            }
            // 4. Any method inside a class
            if let Some((sym, node)) = find_any_method(root, source, member_query, file_path) {
                return Ok((sym, node));
            }
            // 5. Fallback scan
            if let Some(sym) = fallback_kotlin_source_scan(source, member_query, file_path) {
                return Ok((sym, root));
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
        collect_symbols_recursive(root, source, &mut symbols, None);
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

        let scoped_generics = collect_kotlin_scoped_generics(target_node, source);
        let ts_lang = self.tree_sitter_language(file_path);

        for user_type in AstUtils::find_descendants_by_kind(target_node, "user_type") {
            let text = AstUtils::node_text(user_type, source);
            let name = text.trim_end_matches('?').split('<').next().unwrap_or(text).trim();
            if !is_builtin_kotlin_type(name)
                && !scoped_generics.contains(name)
                && visited.insert(name.to_string())
            {
                queue.push_back((name.to_string(), 1));
            }
        }

        for id in AstUtils::find_descendants_by_kind(target_node, "simple_identifier") {
            let name = AstUtils::node_text(id, source);
            let first_char = name.chars().next().unwrap_or('_');
            if first_char.is_uppercase()
                && !is_builtin_kotlin_type(name)
                && !scoped_generics.contains(name)
                && visited.insert(name.to_string())
            {
                queue.push_back((name.to_string(), 1));
            }
        }

        let dir = file_path.parent().unwrap_or_else(|| Path::new("."));

        while let Some((type_name, depth)) = queue.pop_front() {
            if is_builtin_kotlin_type(&type_name) || scoped_generics.contains(&type_name) {
                continue;
            }

            // 1. Check local file
            if let Some(extracted) = find_kotlin_type_in_file(root, source, &type_name, file_path) {
                if depth < opts.depth {
                    if let Ok(tree) = ParserManager::parse_source(&extracted.definition, &ts_lang, file_path) {
                        for id in AstUtils::find_descendants_by_kind(tree.root_node(), "simple_identifier") {
                            let nested = AstUtils::node_text(id, &extracted.definition);
                            let first = nested.chars().next().unwrap_or('_');
                            if first.is_uppercase() && !is_builtin_kotlin_type(nested) && visited.insert(nested.to_string()) {
                                queue.push_back((nested.to_string(), depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            // 2. Check sibling .kt / .kts files
            if depth <= opts.depth {
                let mut candidate_files = Vec::new();
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() && p != file_path {
                            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                            if ext == "kt" || ext == "kts" {
                                candidate_files.push(p);
                            }
                        }
                    }
                }

                for cand_path in candidate_files {
                    if let Ok(cand_source) = fs::read_to_string(&cand_path) {
                        if cand_source.contains(&type_name) {
                            if let Ok(tree) = ParserManager::parse_source(&cand_source, &ts_lang, &cand_path) {
                                if let Some(extracted) = find_kotlin_type_in_file(tree.root_node(), &cand_source, &type_name, &cand_path) {
                                    if depth < opts.depth {
                                        for id in AstUtils::find_descendants_by_kind(tree.root_node(), "simple_identifier") {
                                            let nested = AstUtils::node_text(id, &cand_source);
                                            let first = nested.chars().next().unwrap_or('_');
                                            if first.is_uppercase() && !is_builtin_kotlin_type(nested) && visited.insert(nested.to_string()) {
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

        let calls = AstUtils::find_descendants_by_kind(target_node, "call_expression");
        for call in calls {
            if let Some(fn_node) = call.named_child(0) {
                let (receiver, func_name) = match fn_node.kind() {
                    "simple_identifier" => (None, AstUtils::node_text(fn_node, source).to_string()),
                    "navigation_expression" => {
                        let parts: Vec<&str> = AstUtils::node_text(fn_node, source).split('.').collect();
                        if parts.len() >= 2 {
                            let obj = parts[..parts.len() - 1].join(".");
                            let name = parts.last().copied().unwrap_or("").to_string();
                            (Some(obj), name)
                        } else {
                            (None, AstUtils::node_text(fn_node, source).to_string())
                        }
                    }
                    _ => (None, AstUtils::node_text(fn_node, source).to_string()),
                };

                if func_name.is_empty() || is_builtin_kotlin_method(&func_name) {
                    continue;
                }

                if seen.insert(func_name.clone()) {
                    if let Some(sig) = find_kotlin_function_signature(root, source, &func_name) {
                        stubs.push(CallSignatureStub {
                            name: func_name,
                            receiver,
                            file_path: Some(file_path.to_string_lossy().to_string()),
                            signature: sig,
                        });
                    }
                }
            }
        }

        Ok(stubs)
    }

    fn find_implementors<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        interface_name: &str,
        file_path: &Path,
    ) -> Result<Vec<ExtractedImplementor>> {
        let mut implementors = Vec::new();
        let decls = AstUtils::find_descendants_by_kind(root, "class_declaration");
        let objects = AstUtils::find_descendants_by_kind(root, "object_declaration");

        for node in decls.into_iter().chain(objects) {
            let text = AstUtils::node_text(node, source);
            let has_delegation = AstUtils::find_descendants_by_kind(node, "delegation_specifier")
                .into_iter()
                .chain(AstUtils::find_descendants_by_kind(node, "delegation_specifiers"))
                .chain(AstUtils::find_descendants_by_kind(node, "user_type"))
                .any(|d| {
                    let d_text = AstUtils::node_text(d, source);
                    d_text
                        .split(|c: char| c == ',' || c == ':' || c.is_whitespace() || c == '<' || c == '>' || c == '(' || c == ')')
                        .any(|part| part.trim() == interface_name)
                })
                || text.contains(&format!(": {interface_name}"))
                || text.contains(&format!(", {interface_name}"));

            if has_delegation {
                if let Some(class_name) = get_kotlin_name(node, source) {
                    if class_name != interface_name {
                        let stub = extract_kotlin_class_stub(node, source);
                        implementors.push(ExtractedImplementor {
                            interface_name: interface_name.to_string(),
                            implementor_name: class_name,
                            kind: "kotlin_class".to_string(),
                            file_path: file_path.to_string_lossy().to_string(),
                            definition: stub,
                        });
                    }
                }
            }
        }

        Ok(implementors)
    }
}

fn parse_query(query: &str) -> (Option<&str>, &str) {
    if let Some((container, member)) = query.split_once('.') {
        (Some(container.trim()), member.trim())
    } else {
        (None, query.trim())
    }
}

fn get_kotlin_name(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(name_n) = node.child_by_field_name("name") {
        return Some(AstUtils::node_text(name_n, source).to_string());
    }
    for desc in AstUtils::find_descendants_by_kind(node, "simple_identifier") {
        let t = AstUtils::node_text(desc, source);
        if t != "class" && t != "fun" && t != "interface" && t != "object" && t != "override" && t != "data" && t != "private" && t != "public" && t != "val" && t != "var" {
            return Some(t.to_string());
        }
    }
    for desc in AstUtils::find_descendants_by_kind(node, "type_identifier") {
        let t = AstUtils::node_text(desc, source);
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
    let decls = AstUtils::find_descendants_by_kind(root, "class_declaration");
    let objects = AstUtils::find_descendants_by_kind(root, "object_declaration");
    let functions = AstUtils::find_descendants_by_kind(root, "function_declaration");

    for node in decls.into_iter().chain(objects).chain(functions) {
        if let Some(name) = get_kotlin_name(node, source) {
            if name == target_name {
                return Some((build_kotlin_symbol(node, source, file_path, target_name), node));
            }
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
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let decls = AstUtils::find_descendants_by_kind(root, "class_declaration");
    let objects = AstUtils::find_descendants_by_kind(root, "object_declaration");

    for node in decls.into_iter().chain(objects) {
        if let Some(c_name) = get_kotlin_name(node, source) {
            if c_name == container_name {
                let functions = AstUtils::find_descendants_by_kind(node, "function_declaration");
                for fn_node in functions {
                    if let Some(f_name) = get_kotlin_name(fn_node, source) {
                        if f_name == member_name {
                            let full_name = format!("{container_name}.{member_name}");
                            return Some((build_kotlin_symbol(fn_node, source, file_path, &full_name), fn_node));
                        }
                    }
                }
            }
        }
    }
    None
}

fn find_extension_function<'a>(
    root: Node<'a>,
    source: &'a str,
    receiver_name: &str,
    member_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let functions = AstUtils::find_descendants_by_kind(root, "function_declaration");
    for fn_node in functions {
        let fn_text = AstUtils::node_text(fn_node, source);
        let receiver_pat = format!("fun {receiver_name}.{member_name}");
        let receiver_pat_suspend = format!("suspend fun {receiver_name}.{member_name}");
        if fn_text.contains(&receiver_pat) || fn_text.contains(&receiver_pat_suspend) {
            let full_name = format!("{receiver_name}.{member_name}");
            return Some((build_kotlin_symbol(fn_node, source, file_path, &full_name), fn_node));
        }
    }
    None
}

fn find_any_method<'a>(
    root: Node<'a>,
    source: &'a str,
    member_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let functions = AstUtils::find_descendants_by_kind(root, "function_declaration");
    for fn_node in functions {
        if let Some(name_node) = AstUtils::find_child_by_kind(fn_node, "simple_identifier") {
            if AstUtils::node_text(name_node, source) == member_name {
                let container_name = find_enclosing_kotlin_class_name(fn_node, source);
                let full_name = match container_name {
                    Some(c) => format!("{c}.{member_name}"),
                    None => member_name.to_string(),
                };
                return Some((build_kotlin_symbol(fn_node, source, file_path, &full_name), fn_node));
            }
        }
    }
    None
}

fn find_enclosing_kotlin_class_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut current = node.parent();
    while let Some(n) = current {
        if matches!(n.kind(), "class_declaration" | "object_declaration") {
            if let Some(name_node) = AstUtils::find_child_by_kind(n, "simple_identifier") {
                return Some(AstUtils::node_text(name_node, source).to_string());
            }
        }
        current = n.parent();
    }
    None
}

fn build_kotlin_symbol(
    node: Node<'_>,
    source: &str,
    file_path: &Path,
    name: &str,
) -> ExtractedSymbol {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let doc_comment = extract_kotlin_doc_comment(node, source);
    let signature = extract_kotlin_signature(node, source);
    let body = AstUtils::node_text(node, source).to_string();

    let kind = match node.kind() {
        "class_declaration" => {
            let text = AstUtils::node_text(node, source);
            if text.contains("data class") {
                "data_class"
            } else if text.contains("interface ") {
                "interface"
            } else if text.contains("enum class") {
                "enum"
            } else {
                "class"
            }
        }
        "object_declaration" | "companion_object" => "object",
        "type_alias" => "type_alias",
        "function_declaration" if name.contains('.') => "method",
        "function_declaration" => "function",
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
        language: "kotlin".to_string(),
    }
}

fn extract_kotlin_doc_comment(node: Node<'_>, source: &str) -> Option<String> {
    let mut comments = Vec::new();
    let mut prev = node.prev_named_sibling();

    while let Some(p) = prev {
        if p.kind() == "comment" || p.kind() == "line_comment" || p.kind() == "multiline_comment" {
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

fn extract_kotlin_signature(node: Node<'_>, source: &str) -> String {
    let raw = AstUtils::node_text(node, source);
    if let Some(body) = AstUtils::find_child_by_kind(node, "function_body").or_else(|| AstUtils::find_child_by_kind(node, "block")) {
        let offset = body.start_byte().saturating_sub(node.start_byte());
        if offset > 0 && offset <= raw.len() {
            return raw[..offset].trim().to_string();
        }
    }
    if let Some(idx) = raw.find('{') {
        raw[..idx].trim().to_string()
    } else if let Some(idx) = raw.find(" = ") {
        raw[..idx].trim().to_string()
    } else {
        raw.lines().next().unwrap_or(raw).trim().to_string()
    }
}

fn fallback_kotlin_source_scan(
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedSymbol> {
    let patterns = [
        format!("fun {target_name}("),
        format!("fun {target_name} ("),
        format!(".{target_name}("),
        format!("class {target_name}"),
        format!("data class {target_name}"),
        format!("interface {target_name}"),
        format!("object {target_name}"),
    ];

    for (line_idx, line) in source.lines().enumerate() {
        for pat in &patterns {
            if line.contains(pat) {
                let start_line = line_idx + 1;
                let body_lines: Vec<&str> = source.lines().skip(line_idx).take(40).collect();
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
                    language: "kotlin".to_string(),
                });
            }
        }
    }
    None
}

fn collect_symbols_recursive(
    root: Node<'_>,
    source: &str,
    symbols: &mut Vec<String>,
    _current_container: Option<&str>,
) {
    let decls = AstUtils::find_descendants_by_kind(root, "class_declaration");
    let objects = AstUtils::find_descendants_by_kind(root, "object_declaration");
    let functions = AstUtils::find_descendants_by_kind(root, "function_declaration");

    for node in decls.into_iter().chain(objects) {
        if let Some(c_name) = get_kotlin_name(node, source) {
            if !symbols.contains(&c_name) {
                symbols.push(c_name.clone());
            }
            let methods = AstUtils::find_descendants_by_kind(node, "function_declaration");
            for m in methods {
                if let Some(m_name) = get_kotlin_name(m, source) {
                    let full = format!("{c_name}.{m_name}");
                    if !symbols.contains(&full) {
                        symbols.push(full);
                    }
                }
            }
        }
    }

    for fn_node in functions {
        if let Some(f_name) = get_kotlin_name(fn_node, source) {
            if !symbols.contains(&f_name) {
                symbols.push(f_name);
            }
        }
    }
}

fn is_builtin_kotlin_type(name: &str) -> bool {
    matches!(
        name,
        "Int" | "Long" | "Short" | "Byte" | "Float" | "Double" | "Boolean" | "Char" | "String"
            | "Unit" | "Nothing" | "Any" | "List" | "MutableList" | "Set" | "MutableSet"
            | "Map" | "MutableMap" | "Array" | "Sequence" | "Flow" | "Deferred" | "Job"
            | "Result" | "Pair" | "Triple" | "Throwable" | "Exception"
            | "CoroutineScope" | "CoroutineContext" | "BigDecimal" | "BigInteger"
            | "UUID" | "LocalDate" | "LocalDateTime" | "Instant"
            | "RestController" | "Service" | "Repository" | "Component" | "Autowired"
            | "GetMapping" | "PostMapping" | "PutMapping" | "DeleteMapping"
            | "RequestBody" | "PathVariable" | "RequestParam"
    )
}

fn is_builtin_kotlin_method(name: &str) -> bool {
    matches!(
        name,
        "println" | "print" | "listOf" | "mutableListOf" | "setOf" | "mutableSetOf"
            | "mapOf" | "mutableMapOf" | "arrayOf" | "let" | "apply" | "also" | "run" | "with"
            | "launch" | "async" | "await" | "map" | "filter" | "forEach" | "first" | "firstOrNull"
            | "take" | "drop" | "count" | "any" | "all" | "none" | "contains"
            | "toString" | "equals" | "hashCode"
    )
}

fn collect_kotlin_scoped_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut current = Some(node);
    while let Some(n) = current {
        if let Some(type_params) = AstUtils::find_child_by_kind(n, "type_parameters") {
            for param in type_params.named_children(&mut type_params.walk()) {
                if let Some(name_node) = AstUtils::find_child_by_kind(param, "simple_identifier") {
                    set.insert(AstUtils::node_text(name_node, source).to_string());
                }
            }
        }
        current = n.parent();
    }
    set
}

fn find_kotlin_type_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let decls = AstUtils::find_descendants_by_kind(root, "class_declaration");
    let objects = AstUtils::find_descendants_by_kind(root, "object_declaration");
    let typealiases = AstUtils::find_descendants_by_kind(root, "type_alias");

    for node in decls {
        if let Some(name) = get_kotlin_name(node, source) {
            if name == target_name {
                let text = AstUtils::node_text(node, source);
                let kind = if text.contains("data class") {
                    "data_class"
                } else if text.contains("interface ") {
                    "interface"
                } else {
                    "class"
                };
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: kind.to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: text.to_string(),
                });
            }
        }
    }

    for node in objects {
        if let Some(name) = get_kotlin_name(node, source) {
            if name == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "object".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    for node in typealiases {
        if let Some(name) = get_kotlin_name(node, source) {
            if name == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "type_alias".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    None
}

fn find_kotlin_function_signature(root: Node<'_>, source: &str, target_name: &str) -> Option<String> {
    let functions = AstUtils::find_descendants_by_kind(root, "function_declaration");
    for fn_node in functions {
        if let Some(name) = get_kotlin_name(fn_node, source) {
            if name == target_name {
                let sig = extract_kotlin_signature(fn_node, source);
                return Some(sig);
            }
        }
    }
    None
}

fn extract_kotlin_class_stub(node: Node<'_>, source: &str) -> String {
    if let Some(body) = AstUtils::find_child_by_kind(node, "class_body") {
        let header_end = body.start_byte();
        let header = source[node.start_byte()..header_end].trim();
        let mut stubs = Vec::new();

        for member in body.named_children(&mut body.walk()) {
            if member.kind() == "function_declaration" {
                let sig = extract_kotlin_signature(member, source);
                stubs.push(format!("    {sig} {{ ... }}"));
            }
        }

        format!("{header} {{\n{}\n}}", stubs.join("\n"))
    } else {
        AstUtils::node_text(node, source).to_string()
    }
}
