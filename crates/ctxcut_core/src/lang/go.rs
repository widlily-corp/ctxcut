//! LanguageAdapter implementation for Go.

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
            if let Some((sym, node)) =
                find_method_with_receiver(root, source, receiver_name, member_query, file_path)
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

            if let Some(sym) = fallback_go_source_scan(source, member_query, file_path) {
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
                        let receiver_type = extract_receiver_type(child, source);
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
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        // Collect scoped generics
        let scoped_generics = collect_go_scoped_generics(target_node, source);

        // Check type identifiers in target node
        for id in AstUtils::find_descendants_by_kind(target_node, "type_identifier") {
            let name = AstUtils::node_text(id, source);
            if !is_builtin_go_type(name)
                && !scoped_generics.contains(name)
                && visited.insert(name.to_string())
            {
                queue.push_back((name.to_string(), 1));
            }
        }

        let dir = file_path.parent().unwrap_or_else(|| Path::new("."));
        let ts_lang = self.tree_sitter_language(file_path);

        while let Some((type_name, depth)) = queue.pop_front() {
            if is_builtin_go_type(&type_name) || scoped_generics.contains(&type_name) {
                continue;
            }

            // 1. Check local file
            if let Some(extracted) = find_go_type_in_file(root, source, &type_name, file_path) {
                if depth < opts.depth {
                    if let Ok(tree) =
                        ParserManager::parse_source(&extracted.definition, &ts_lang, file_path)
                    {
                        let def_generics =
                            collect_go_scoped_generics(tree.root_node(), &extracted.definition);
                        for id in
                            AstUtils::find_descendants_by_kind(tree.root_node(), "type_identifier")
                        {
                            let name = AstUtils::node_text(id, &extracted.definition);
                            if !is_builtin_go_type(name)
                                && !def_generics.contains(name)
                                && visited.insert(name.to_string())
                            {
                                queue.push_back((name.to_string(), depth + 1));
                            }
                        }
                    }
                }
                hoisted.push(extracted);
                continue;
            }

            if opts.depth == 0 {
                continue;
            }

            // 2. Check sibling .go files in the same package/directory
            if let Ok(entries) = fs::read_dir(dir) {
                let mut found = false;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path == file_path || path.extension().and_then(|e| e.to_str()) != Some("go")
                    {
                        continue;
                    }

                    if let Ok(sibling_src) = fs::read_to_string(&path) {
                        if let Ok(sibling_tree) =
                            ParserManager::parse_source(&sibling_src, &ts_lang, &path)
                        {
                            if let Some(extracted) = find_go_type_in_file(
                                sibling_tree.root_node(),
                                &sibling_src,
                                &type_name,
                                &path,
                            ) {
                                if depth < opts.depth {
                                    if let Ok(tree) = ParserManager::parse_source(
                                        &extracted.definition,
                                        &ts_lang,
                                        &path,
                                    ) {
                                        let def_generics = collect_go_scoped_generics(
                                            tree.root_node(),
                                            &extracted.definition,
                                        );
                                        for id in AstUtils::find_descendants_by_kind(
                                            tree.root_node(),
                                            "type_identifier",
                                        ) {
                                            let name =
                                                AstUtils::node_text(id, &extracted.definition);
                                            if !is_builtin_go_type(name)
                                                && !def_generics.contains(name)
                                                && visited.insert(name.to_string())
                                            {
                                                queue.push_back((name.to_string(), depth + 1));
                                            }
                                        }
                                    }
                                }
                                hoisted.push(extracted);
                                found = true;
                                break;
                            }
                        }
                    }
                }
                if found {
                    continue;
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

        let dir = file_path.parent().unwrap_or_else(|| Path::new("."));
        let ts_lang = self.tree_sitter_language(file_path);

        let call_nodes = AstUtils::find_descendants_by_kind(target_node, "call_expression");
        for call in call_nodes {
            if let Some(func_node) = call.child_by_field_name("function") {
                let call_text = AstUtils::node_text(func_node, source);
                let call_name = call_text.split('.').next_back().unwrap_or(call_text);

                if !seen.insert(call_name.to_string()) || is_builtin_go_func(call_name) {
                    continue;
                }

                if let Some(sig) = find_go_signature(root, source, call_name) {
                    stubs.push(CallSignatureStub {
                        name: call_name.to_string(),
                        receiver: None,
                        file_path: Some(file_path.to_string_lossy().to_string()),
                        signature: sig,
                    });
                } else {
                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path == file_path
                                || path.extension().and_then(|e| e.to_str()) != Some("go")
                            {
                                continue;
                            }

                            if let Ok(sibling_src) = fs::read_to_string(&path) {
                                if let Ok(sibling_tree) =
                                    ParserManager::parse_source(&sibling_src, &ts_lang, &path)
                                {
                                    if let Some(sig) = find_go_signature(
                                        sibling_tree.root_node(),
                                        &sibling_src,
                                        call_name,
                                    ) {
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
                    // If not found in local or sibling package files, do not emit dummy stubs for builtins/external packages
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

        // 1. Extract method names required by the interface
        let mut required_methods = extract_go_interface_methods(root, source, interface_name);
        if required_methods.is_empty() {
            if let Some(parent) = file_path.parent() {
                if let Ok(entries) = fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p != file_path && p.extension().and_then(|e| e.to_str()) == Some("go") {
                            if let Ok(sib_src) = fs::read_to_string(&p) {
                                if sib_src.contains(interface_name) {
                                    if let Ok(tree) = ParserManager::parse_source(
                                        &sib_src,
                                        &tree_sitter_go::LANGUAGE.into(),
                                        &p,
                                    ) {
                                        let methods = extract_go_interface_methods(
                                            tree.root_node(),
                                            &sib_src,
                                            interface_name,
                                        );
                                        if !methods.is_empty() {
                                            required_methods = methods;
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

        if required_methods.is_empty() {
            return Ok(implementors);
        }

        // 2. Map concrete receiver types to their implemented methods
        let receiver_methods = collect_go_concrete_methods(root, source);

        // 3. Match structs that implement all required methods
        for (receiver_name, methods) in receiver_methods {
            let matches_all = required_methods
                .iter()
                .all(|req| methods.iter().any(|(m, _)| m == req));
            if matches_all {
                let stubs: Vec<String> = methods
                    .iter()
                    .filter(|(m, _)| required_methods.contains(m))
                    .map(|(_, sig)| format!("{sig} {{ ... }}"))
                    .collect();

                let definition = format!(
                    "type {receiver_name} struct {{\n    // ...\n}}\n\n{}",
                    stubs.join("\n")
                );
                implementors.push(ExtractedImplementor {
                    interface_name: interface_name.to_string(),
                    implementor_name: receiver_name,
                    kind: "go_struct".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition,
                });
            }
        }

        Ok(implementors)
    }
}

fn extract_go_interface_methods(
    root: Node<'_>,
    source: &str,
    interface_name: &str,
) -> HashSet<String> {
    let mut methods = HashSet::new();
    let type_specs = AstUtils::find_descendants_by_kind(root, "type_spec");
    for spec in type_specs {
        if let Some(name_node) = spec.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == interface_name {
                if let Some(type_node) = spec.child_by_field_name("type") {
                    if type_node.kind() == "interface_type" {
                        for child in type_node.named_children(&mut type_node.walk()) {
                            if let Some(m_name) = child.child_by_field_name("name") {
                                methods.insert(AstUtils::node_text(m_name, source).to_string());
                            } else {
                                for id in
                                    AstUtils::find_descendants_by_kind(child, "field_identifier")
                                {
                                    methods.insert(AstUtils::node_text(id, source).to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    methods
}

fn collect_go_concrete_methods(
    root: Node<'_>,
    source: &str,
) -> std::collections::HashMap<String, Vec<(String, String)>> {
    let mut map = std::collections::HashMap::new();
    let method_decls = AstUtils::find_descendants_by_kind(root, "method_declaration");
    for decl in method_decls {
        if let Some(receiver_name) = extract_receiver_type(decl, source) {
            if let Some(name_node) = decl.child_by_field_name("name") {
                let method_name = AstUtils::node_text(name_node, source).to_string();
                let sig = extract_go_method_signature(decl, source);
                map.entry(receiver_name)
                    .or_insert_with(Vec::new)
                    .push((method_name, sig));
            }
        }
    }
    map
}

fn extract_go_method_signature(method_node: Node<'_>, source: &str) -> String {
    if let Some(body) = method_node.child_by_field_name("body") {
        let start = method_node.start_byte();
        let body_start = body.start_byte();
        if start < body_start && body_start <= source.len() {
            return source[start..body_start].trim().to_string();
        }
    }
    let text = AstUtils::node_text(method_node, source);
    text.lines().next().unwrap_or(text).trim().to_string()
}

fn collect_go_scoped_generics(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut generics = HashSet::new();
    for tp_list in AstUtils::find_descendants_by_kind(node, "type_parameter_list") {
        for param in AstUtils::find_children_by_kind(tp_list, "type_parameter_declaration") {
            for id in AstUtils::find_children_by_kind(param, "identifier") {
                generics.insert(AstUtils::node_text(id, source).to_string());
            }
        }
    }
    for tp in AstUtils::find_descendants_by_kind(node, "type_parameter_declaration") {
        for id in AstUtils::find_children_by_kind(tp, "identifier") {
            generics.insert(AstUtils::node_text(id, source).to_string());
        }
    }
    generics
}

fn parse_query(query: &str) -> (Option<&str>, &str) {
    if let Some((container, member)) = query.split_once('.') {
        (Some(container.trim()), member.trim())
    } else {
        (None, query.trim())
    }
}

fn extract_receiver_type(method_node: Node<'_>, source: &str) -> Option<String> {
    if let Some(receiver) = method_node.child_by_field_name("receiver") {
        if let Some(type_id) = AstUtils::find_descendants_by_kind(receiver, "type_identifier")
            .into_iter()
            .next()
        {
            return Some(AstUtils::node_text(type_id, source).to_string());
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
                        return Some((
                            build_go_symbol(child, source, file_path, "function"),
                            child,
                        ));
                    }
                }
            }
            "type_declaration" => {
                for spec in AstUtils::find_children_by_kind(child, "type_spec") {
                    if let Some(name_node) = spec.child_by_field_name("name") {
                        if AstUtils::node_text(name_node, source) == target_name {
                            return Some((
                                build_go_symbol(child, source, file_path, "type"),
                                child,
                            ));
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
                            if type_node.kind() == "interface_type" {
                                "interface"
                            } else if type_node.kind() == "struct_type" {
                                "struct"
                            } else {
                                "type"
                            }
                        } else {
                            "type"
                        };
                        return Some(ExtractedType {
                            name: target_name.to_string(),
                            kind: kind.to_string(),
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

fn find_method_with_receiver<'a>(
    root: Node<'a>,
    source: &'a str,
    receiver_name: &str,
    method_name: &str,
    file_path: &Path,
) -> Option<(ExtractedSymbol, Node<'a>)> {
    let clean_query_receiver = receiver_name.trim_start_matches('*').trim();

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "method_declaration" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == method_name {
                    if let Some(rec) = extract_receiver_type(child, source) {
                        let clean_rec = rec.trim_start_matches('*').trim();
                        if rec == receiver_name || clean_rec == clean_query_receiver {
                            return Some((
                                build_go_symbol(child, source, file_path, "method"),
                                child,
                            ));
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
    AstUtils::node_text(node, source).to_string()
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
        }
    }
    None
}

fn is_builtin_go_type(name: &str) -> bool {
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

fn is_builtin_go_func(name: &str) -> bool {
    matches!(
        name,
        "make"
            | "new"
            | "len"
            | "cap"
            | "append"
            | "copy"
            | "close"
            | "delete"
            | "panic"
            | "recover"
    )
}

fn fallback_go_source_scan(
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedSymbol> {
    let patterns = [
        format!("func {target_name}("),
        format!("func {target_name} "),
        format!("type {target_name} "),
    ];

    let mut start_line = 1;
    let mut found_kind = "function";
    let mut start_offset = None;

    for (line_idx, line) in source.lines().enumerate() {
        for pat in &patterns {
            if line.contains(pat) {
                start_line = line_idx + 1;
                if pat.contains("type") {
                    found_kind = "type";
                }

                let mut offset = 0;
                for (i, l) in source.lines().enumerate() {
                    if i == line_idx {
                        break;
                    }
                    offset += l.len() + 1;
                }
                start_offset = Some(offset);
                break;
            }
        }
        if start_offset.is_some() {
            break;
        }
    }

    let start_b = start_offset?;
    let remainder = &source[start_b..];

    let mut brace_count = 0;
    let mut started_brace = false;
    let mut end_b = remainder.len();

    for (idx, ch) in remainder.char_indices() {
        if ch == '{' {
            brace_count += 1;
            started_brace = true;
        } else if ch == '}' {
            brace_count -= 1;
            if started_brace && brace_count == 0 {
                end_b = idx + 1;
                break;
            }
        }
    }

    let body = remainder[..end_b].trim_end().to_string();
    let end_line = start_line + body.lines().count().max(1) - 1;

    let signature = if let Some((sig, _)) = body.split_once('{') {
        sig.trim().to_string()
    } else {
        body.clone()
    };

    Some(ExtractedSymbol {
        name: target_name.to_string(),
        kind: found_kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        start_line,
        end_line,
        doc_comment: None,
        signature,
        body,
        language: "go".to_string(),
    })
}
