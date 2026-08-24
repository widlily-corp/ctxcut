//! LanguageAdapter implementation for Java.

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

/// Java language adapter supporting Java (.java).
pub struct JavaAdapter;

impl LanguageAdapter for JavaAdapter {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::Java
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_java::LANGUAGE.into()
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
            if let Some((sym, node)) =
                find_in_container(root, source, container_name, member_query, file_path)
            {
                return Ok((sym, node));
            }
        } else {
            if let Some((sym, node)) = find_top_level(root, source, member_query, file_path) {
                return Ok((sym, node));
            }
            if let Some((sym, node)) = find_any_method(root, source, member_query, file_path) {
                return Ok((sym, node));
            }
            if let Some(sym) = fallback_java_source_scan(source, member_query, file_path) {
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

        let scoped_generics = collect_java_scoped_generics(target_node, source);
        let ts_lang = self.tree_sitter_language(file_path);

        for id in AstUtils::find_descendants_by_kind(target_node, "type_identifier") {
            let name = AstUtils::node_text(id, source);
            if !is_builtin_java_type(name)
                && !scoped_generics.contains(name)
                && visited.insert(name.to_string())
            {
                queue.push_back((name.to_string(), 1));
            }
        }

        let dir = file_path.parent().unwrap_or_else(|| Path::new("."));

        while let Some((type_name, depth)) = queue.pop_front() {
            if is_builtin_java_type(&type_name) || scoped_generics.contains(&type_name) {
                continue;
            }

            // 1. Local file
            if let Some(extracted) = find_java_type_in_file(root, source, &type_name, file_path) {
                if depth < opts.depth {
                    if let Ok(tree) =
                        ParserManager::parse_source(&extracted.definition, &ts_lang, file_path)
                    {
                        for id in
                            AstUtils::find_descendants_by_kind(tree.root_node(), "type_identifier")
                        {
                            let nested = AstUtils::node_text(id, &extracted.definition);
                            if !is_builtin_java_type(nested) && visited.insert(nested.to_string()) {
                                queue.push_back((nested.to_string(), depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            // 2. Sibling .java files in package or imported classes
            if depth <= opts.depth {
                let mut candidate_files = Vec::new();
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file()
                            && p != file_path
                            && p.extension().and_then(|e| e.to_str()) == Some("java")
                        {
                            candidate_files.push(p);
                        }
                    }
                }

                for cand_path in candidate_files {
                    if let Ok(cand_source) = fs::read_to_string(&cand_path) {
                        if cand_source.contains(&type_name) {
                            if let Ok(tree) =
                                ParserManager::parse_source(&cand_source, &ts_lang, &cand_path)
                            {
                                if let Some(extracted) = find_java_type_in_file(
                                    tree.root_node(),
                                    &cand_source,
                                    &type_name,
                                    &cand_path,
                                ) {
                                    if depth < opts.depth {
                                        for id in AstUtils::find_descendants_by_kind(
                                            tree.root_node(),
                                            "type_identifier",
                                        ) {
                                            let nested = AstUtils::node_text(id, &cand_source);
                                            if !is_builtin_java_type(nested)
                                                && visited.insert(nested.to_string())
                                            {
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

        let invocations = AstUtils::find_descendants_by_kind(target_node, "method_invocation");
        for inv in invocations {
            let name_node = inv.child_by_field_name("name");
            let obj_node = inv.child_by_field_name("object");

            if let Some(n_node) = name_node {
                let func_name = AstUtils::node_text(n_node, source).to_string();
                let receiver = obj_node.map(|o| AstUtils::node_text(o, source).to_string());

                if func_name.is_empty() || is_builtin_java_method(&func_name) {
                    continue;
                }

                if seen.insert(func_name.clone()) {
                    if let Some(sig) = find_java_method_signature(root, source, &func_name) {
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
        let records = AstUtils::find_descendants_by_kind(root, "record_declaration");

        for node in decls.into_iter().chain(records) {
            let mut matches_interface = false;
            if let Some(interfaces) = AstUtils::find_child_by_kind(node, "super_interfaces") {
                let if_text = AstUtils::node_text(interfaces, source);
                if if_text
                    .split(|c: char| c == ',' || c.is_whitespace() || c == '<' || c == '>')
                    .any(|part| part.trim() == interface_name)
                {
                    matches_interface = true;
                }
            }
            if let Some(superclass) = AstUtils::find_child_by_kind(node, "superclass") {
                let super_text = AstUtils::node_text(superclass, source);
                if super_text
                    .split(|c: char| c == ',' || c.is_whitespace() || c == '<' || c == '>')
                    .any(|part| part.trim() == interface_name)
                {
                    matches_interface = true;
                }
            }

            if matches_interface {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let class_name = AstUtils::node_text(name_node, source).to_string();
                    let stub = extract_java_class_stub(node, source);
                    implementors.push(ExtractedImplementor {
                        interface_name: interface_name.to_string(),
                        implementor_name: class_name,
                        kind: "java_class".to_string(),
                        file_path: file_path.to_string_lossy().to_string(),
                        definition: stub,
                    });
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

fn find_top_level<'a>(
    root: Node<'a>,
    source: &'a str,
    target_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    find_symbol_recursive(root, source, target_name, file_path)
}

fn find_symbol_recursive<'a>(
    node: Node<'a>,
    source: &'a str,
    target_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_declaration"
            | "interface_declaration"
            | "record_declaration"
            | "enum_declaration"
            | "annotation_type_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some((
                            build_java_symbol(child, source, file_path, target_name),
                            child,
                        ));
                    }
                }
            }
            "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        return Some((
                            build_java_symbol(child, source, file_path, target_name),
                            child,
                        ));
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
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let decls = AstUtils::find_descendants_by_kind(root, "class_declaration");
    let records = AstUtils::find_descendants_by_kind(root, "record_declaration");
    let interfaces = AstUtils::find_descendants_by_kind(root, "interface_declaration");

    for node in decls.into_iter().chain(records).chain(interfaces) {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == container_name {
                if let Some(body) = node.child_by_field_name("body") {
                    for member in body.named_children(&mut body.walk()) {
                        if matches!(
                            member.kind(),
                            "method_declaration"
                                | "constructor_declaration"
                                | "compact_constructor_declaration"
                        ) {
                            if let Some(m_name_node) = member.child_by_field_name("name") {
                                if AstUtils::node_text(m_name_node, source) == member_name {
                                    let full_name = format!("{container_name}.{member_name}");
                                    return Some((
                                        build_java_symbol(member, source, file_path, &full_name),
                                        member,
                                    ));
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
    member_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let methods = AstUtils::find_descendants_by_kind(root, "method_declaration");
    for method in methods {
        if let Some(name_node) = method.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == member_name {
                let container_name = find_enclosing_class_name(method, source);
                let full_name = match container_name {
                    Some(c) => format!("{c}.{member_name}"),
                    None => member_name.to_string(),
                };
                return Some((
                    build_java_symbol(method, source, file_path, &full_name),
                    method,
                ));
            }
        }
    }
    None
}

fn find_enclosing_class_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut current = node.parent();
    while let Some(n) = current {
        if matches!(
            n.kind(),
            "class_declaration"
                | "record_declaration"
                | "interface_declaration"
                | "enum_declaration"
        ) {
            if let Some(name_node) = n.child_by_field_name("name") {
                return Some(AstUtils::node_text(name_node, source).to_string());
            }
        }
        current = n.parent();
    }
    None
}

fn build_java_symbol(
    node: Node<'_>,
    source: &str,
    file_path: &Path,
    name: &str,
) -> ExtractedSymbol {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let doc_comment = extract_java_doc_comment(node, source);
    let signature = extract_java_signature(node, source);
    let body = AstUtils::node_text(node, source).to_string();

    let kind = match node.kind() {
        "class_declaration" => "class",
        "record_declaration" => "record",
        "interface_declaration" => "interface",
        "enum_declaration" => "enum",
        "annotation_type_declaration" => "annotation",
        "method_declaration" => {
            if name.contains('.') {
                "method"
            } else {
                "function"
            }
        }
        "constructor_declaration" | "compact_constructor_declaration" => "constructor",
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
        language: "java".to_string(),
    }
}

fn extract_java_doc_comment(node: Node<'_>, source: &str) -> Option<String> {
    let mut comments = Vec::new();
    let mut prev = node.prev_named_sibling();

    while let Some(p) = prev {
        if p.kind() == "block_comment" || p.kind() == "line_comment" || p.kind() == "comment" {
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

fn extract_java_signature(node: Node<'_>, source: &str) -> String {
    let raw = AstUtils::node_text(node, source);
    if let Some(body) = node.child_by_field_name("body") {
        let offset = body.start_byte() - node.start_byte();
        return raw[..offset].trim().to_string();
    }
    if let Some(idx) = raw.find('{') {
        raw[..idx].trim().to_string()
    } else if let Some(idx) = raw.find(';') {
        raw[..=idx].trim().to_string()
    } else {
        raw.lines().next().unwrap_or(raw).trim().to_string()
    }
}

fn fallback_java_source_scan(
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedSymbol> {
    let patterns = [
        format!(" {target_name}("),
        format!(" {target_name} ("),
        format!("class {target_name}"),
        format!("record {target_name}"),
        format!("interface {target_name}"),
        format!("enum {target_name}"),
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
                    language: "java".to_string(),
                });
            }
        }
    }
    None
}

fn collect_symbols_recursive(
    node: Node<'_>,
    source: &str,
    symbols: &mut Vec<String>,
    current_container: Option<&str>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_declaration"
            | "record_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "annotation_type_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let type_name = AstUtils::node_text(name_node, source).to_string();
                    let full_type = match current_container {
                        Some(c) => format!("{c}.{type_name}"),
                        None => type_name.clone(),
                    };
                    symbols.push(full_type.clone());
                    if let Some(body) = child.child_by_field_name("body") {
                        collect_symbols_recursive(body, source, symbols, Some(&full_type));
                    }
                }
            }
            "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let m_name = AstUtils::node_text(name_node, source);
                    let sym = match current_container {
                        Some(c) => format!("{c}.{m_name}"),
                        None => m_name.to_string(),
                    };
                    symbols.push(sym);
                }
            }
            _ => {}
        }
    }
}

fn is_builtin_java_type(name: &str) -> bool {
    matches!(
        name,
        "byte"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "boolean"
            | "char"
            | "void"
            | "Byte"
            | "Short"
            | "Integer"
            | "Long"
            | "Float"
            | "Double"
            | "Boolean"
            | "Character"
            | "String"
            | "Object"
            | "Class"
            | "List"
            | "ArrayList"
            | "LinkedList"
            | "Set"
            | "HashSet"
            | "TreeSet"
            | "Map"
            | "HashMap"
            | "TreeMap"
            | "Collection"
            | "Iterable"
            | "Iterator"
            | "Optional"
            | "ResponseEntity"
            | "HttpStatus"
            | "Arrays"
            | "Collections"
            | "Objects"
            | "UUID"
            | "Throwable"
            | "Exception"
            | "RuntimeException"
            | "StringBuilder"
            | "StringBuffer"
            | "BigDecimal"
            | "BigInteger"
            | "Date"
            | "LocalDate"
            | "LocalDateTime"
            | "Instant"
            | "CompletableFuture"
            | "Stream"
            | "Autowired"
            | "RestController"
            | "Controller"
            | "Service"
            | "Repository"
            | "Component"
            | "GetMapping"
            | "PostMapping"
            | "PutMapping"
            | "DeleteMapping"
            | "RequestMapping"
            | "RequestBody"
            | "PathVariable"
            | "RequestParam"
            | "Valid"
            | "NotNull"
            | "NotEmpty"
            | "Entity"
            | "Table"
            | "Id"
            | "GeneratedValue"
            | "Column"
            | "ManyToOne"
            | "OneToMany"
    )
}

fn is_builtin_java_method(name: &str) -> bool {
    matches!(
        name,
        "equals"
            | "hashCode"
            | "toString"
            | "getClass"
            | "notify"
            | "notifyAll"
            | "wait"
            | "get"
            | "set"
            | "add"
            | "remove"
            | "clear"
            | "size"
            | "isEmpty"
            | "contains"
            | "stream"
            | "map"
            | "filter"
            | "collect"
            | "toList"
            | "toSet"
            | "forEach"
            | "of"
            | "builder"
            | "build"
            | "println"
            | "print"
            | "format"
            | "ok"
            | "body"
            | "status"
            | "badRequest"
            | "notFound"
            | "orElse"
            | "orElseThrow"
            | "orElseGet"
            | "isPresent"
            | "ifPresent"
    )
}

fn collect_java_scoped_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut current = Some(node);
    while let Some(n) = current {
        if let Some(type_params) = n.child_by_field_name("type_parameters") {
            for param in type_params.named_children(&mut type_params.walk()) {
                if param.kind() == "type_parameter" {
                    if let Some(name_node) = param
                        .child_by_field_name("name")
                        .or_else(|| param.named_child(0))
                    {
                        set.insert(AstUtils::node_text(name_node, source).to_string());
                    }
                }
            }
        }
        current = n.parent();
    }
    set
}

fn find_java_type_in_file(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let decls = AstUtils::find_descendants_by_kind(root, "class_declaration");
    let records = AstUtils::find_descendants_by_kind(root, "record_declaration");
    let interfaces = AstUtils::find_descendants_by_kind(root, "interface_declaration");
    let enums = AstUtils::find_descendants_by_kind(root, "enum_declaration");

    for node in decls {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "class".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    for node in records {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "record".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    for node in interfaces {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "interface".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    for node in enums {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "enum".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    None
}

fn find_java_method_signature(root: Node<'_>, source: &str, target_name: &str) -> Option<String> {
    let methods = AstUtils::find_descendants_by_kind(root, "method_declaration");
    for method in methods {
        if let Some(name_node) = method.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                let sig = extract_java_signature(method, source);
                return Some(format!("{sig};"));
            }
        }
    }
    None
}

fn extract_java_class_stub(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let header_end = body.start_byte();
        let header = source[node.start_byte()..header_end].trim();
        let mut stubs = Vec::new();

        for member in body.named_children(&mut body.walk()) {
            if member.kind() == "method_declaration" {
                let sig = extract_java_signature(member, source);
                stubs.push(format!("    {sig} {{ ... }}"));
            }
        }

        format!("{header} {{\n{}\n}}", stubs.join("\n"))
    } else {
        AstUtils::node_text(node, source).to_string()
    }
}
