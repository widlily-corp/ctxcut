//! Workspace-level symbol extractor and token-dense architectural overview generator.

use crate::error::Result;
use crate::lang::LanguageRegistry;
use crate::model::{
    FileOverviewItem, OverviewOptions, SupportedLanguage, SymbolOverviewItem,
    WorkspaceOverviewReport,
};
use crate::parser::{AstUtils, ParserManager};
use crate::tokenizer::{calculate_savings_percentage, count_lines, count_tokens};
use crate::traversal::{LanguageStatItem, ProjectWalker, TraversalConfig};
use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::path::Path;
use tree_sitter::Node;

/// Workspace symbol overview generator.
pub struct WorkspaceOverviewGenerator;

impl WorkspaceOverviewGenerator {
    /// Generates a comprehensive, token-dense symbol overview across the workspace.
    pub fn generate(root: &Path, opts: &OverviewOptions) -> Result<WorkspaceOverviewReport> {
        let mut config = TraversalConfig::default();
        if let Some(depth) = opts.max_depth {
            config.max_file_size_bytes = 10 * 1024 * 1024;
            let _ = depth;
        }

        let file_paths = ProjectWalker::collect_files(root, &config);
        let mut files = Vec::new();
        let mut total_lines = 0;
        let mut total_raw_tokens = 0;
        let mut total_symbols = 0;
        let mut lang_counts: HashMap<String, (usize, usize, usize)> = HashMap::new();

        for path in file_paths {
            let lang = match SupportedLanguage::from_path(&path) {
                Some(l) => l,
                None => continue,
            };

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let line_cnt = count_lines(&content);
            let token_cnt = count_tokens(&content);
            total_lines += line_cnt;
            total_raw_tokens += token_cnt;

            let entry = lang_counts
                .entry(lang.as_str().to_string())
                .or_insert((0, 0, 0));
            entry.0 += 1; // file count
            entry.1 += line_cnt;
            entry.2 += token_cnt;

            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            let symbols = extract_symbols_from_file(&path, lang, &content, opts);
            total_symbols += symbols.len();

            files.push(FileOverviewItem {
                path: rel_path,
                language: lang.as_str().to_string(),
                total_lines: line_cnt,
                total_tokens: token_cnt,
                symbols,
            });
        }

        // Sort files alphabetically for deterministic output
        files.sort_by(|a, b| a.path.cmp(&b.path));

        let mut language_breakdown = Vec::new();
        for (lang, (file_count, lines, tokens)) in lang_counts {
            language_breakdown.push(LanguageStatItem {
                language: lang,
                file_count,
                total_lines: lines,
                estimated_tokens: tokens,
            });
        }
        language_breakdown.sort_by_key(|b| std::cmp::Reverse(b.estimated_tokens));

        let mut report = WorkspaceOverviewReport {
            root_path: root.to_string_lossy().to_string(),
            total_files: files.len(),
            total_lines,
            total_raw_tokens,
            total_overview_tokens: 0,
            token_savings_percentage: 0.0,
            total_symbols,
            language_breakdown,
            files,
        };

        // Render initial overview to calculate tokens
        let rendered_md = format_overview_markdown(&report);
        let overview_tokens = count_tokens(&rendered_md);
        report.total_overview_tokens = overview_tokens;
        report.token_savings_percentage =
            calculate_savings_percentage(total_raw_tokens, overview_tokens);

        // Budget enforcement: compress if needed
        if let Some(budget) = opts.budget {
            if report.total_overview_tokens > budget {
                // Pass 1: strip doc summaries
                for f in &mut report.files {
                    for s in &mut f.symbols {
                        s.doc_summary = None;
                    }
                }
                let pass1_md = format_overview_markdown(&report);
                let pass1_tokens = count_tokens(&pass1_md);
                report.total_overview_tokens = pass1_tokens;
                report.token_savings_percentage =
                    calculate_savings_percentage(total_raw_tokens, pass1_tokens);

                // Pass 2: if still over budget, shorten signatures
                if report.total_overview_tokens > budget {
                    for f in &mut report.files {
                        for s in &mut f.symbols {
                            s.signature = None;
                        }
                    }
                    let pass2_md = format_overview_markdown(&report);
                    let pass2_tokens = count_tokens(&pass2_md);
                    report.total_overview_tokens = pass2_tokens;
                    report.token_savings_percentage =
                        calculate_savings_percentage(total_raw_tokens, pass2_tokens);
                }
            }
        }

        Ok(report)
    }
}

fn extract_symbols_from_file(
    file_path: &Path,
    lang: SupportedLanguage,
    source: &str,
    opts: &OverviewOptions,
) -> Vec<SymbolOverviewItem> {
    let adapter = match LanguageRegistry::for_path(file_path) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    let ts_lang = adapter.tree_sitter_language(file_path);
    let tree = match ParserManager::parse_source(source, &ts_lang, file_path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let root = tree.root_node();
    match lang {
        SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
            extract_ts_overview(root, source, opts)
        }
        SupportedLanguage::Python => extract_py_overview(root, source, opts),
        SupportedLanguage::Rust => extract_rs_overview(root, source, opts),
        SupportedLanguage::Go => extract_go_overview(root, source, opts),
    }
}

fn extract_ts_overview(
    root: Node<'_>,
    source: &str,
    opts: &OverviewOptions,
) -> Vec<SymbolOverviewItem> {
    let mut items = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        let decl = AstUtils::unwrap_export(child);
        let start_line = decl.start_position().row + 1;
        let end_line = decl.end_position().row + 1;

        match decl.kind() {
            "function_declaration" | "generator_function_declaration" => {
                if let Some(name_n) = decl.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    let sig = extract_ts_signature_header(decl, source);
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "function".to_string(),
                        start_line,
                        end_line,
                        signature: Some(sig),
                        doc_summary: doc,
                    });
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(name_n) = decl.child_by_field_name("name") {
                    let class_name = AstUtils::node_text(name_n, source).to_string();
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name: class_name.clone(),
                        kind: "class".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: doc,
                    });

                    if let Some(body) = decl.child_by_field_name("body") {
                        for member in body.named_children(&mut body.walk()) {
                            let m_start = member.start_position().row + 1;
                            let m_end = member.end_position().row + 1;
                            if matches!(member.kind(), "method_definition" | "method_signature") {
                                if let Some(m_name) = member.child_by_field_name("name") {
                                    let m_text = AstUtils::node_text(m_name, source);
                                    let full_name = format!("{class_name}.{m_text}");
                                    let sig = extract_ts_signature_header(member, source);
                                    let m_doc = extract_leading_doc_comment(member, source);
                                    items.push(SymbolOverviewItem {
                                        name: full_name,
                                        kind: "method".to_string(),
                                        start_line: m_start,
                                        end_line: m_end,
                                        signature: Some(sig),
                                        doc_summary: m_doc,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            "interface_declaration" => {
                if let Some(name_n) = decl.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "interface".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: doc,
                    });
                }
            }
            "type_alias_declaration" => {
                if let Some(name_n) = decl.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "type".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: None,
                    });
                }
            }
            "enum_declaration" => {
                if let Some(name_n) = decl.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "enum".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: None,
                    });
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                for declarator in AstUtils::find_children_by_kind(decl, "variable_declarator") {
                    if let (Some(name_n), Some(val)) = (
                        declarator.child_by_field_name("name"),
                        declarator.child_by_field_name("value"),
                    ) {
                        if matches!(val.kind(), "arrow_function" | "function_expression") {
                            let name = AstUtils::node_text(name_n, source).to_string();
                            let sig = extract_ts_signature_header(declarator, source);
                            items.push(SymbolOverviewItem {
                                name,
                                kind: "function".to_string(),
                                start_line,
                                end_line,
                                signature: Some(sig),
                                doc_summary: None,
                            });
                        }
                    }
                }
            }
            "expression_statement" if opts.include_routes => {
                if let Some(route_item) = check_ts_express_route(decl, source) {
                    items.push(route_item);
                }
            }
            _ => {}
        }
    }

    items
}

fn extract_py_overview(
    root: Node<'_>,
    source: &str,
    opts: &OverviewOptions,
) -> Vec<SymbolOverviewItem> {
    let mut items = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        let decl = if child.kind() == "decorated_definition" {
            child.child_by_field_name("definition").unwrap_or(child)
        } else {
            child
        };

        let start_line = child.start_position().row + 1;
        let end_line = child.end_position().row + 1;

        match decl.kind() {
            "function_definition" => {
                if let Some(name_n) = decl.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    let sig = extract_py_signature_header(decl, source);
                    let doc = extract_py_docstring(decl, source);

                    if opts.include_routes {
                        if let Some(route_sig) = check_py_route(child, source) {
                            items.push(SymbolOverviewItem {
                                name: route_sig,
                                kind: "route".to_string(),
                                start_line,
                                end_line,
                                signature: Some(sig.clone()),
                                doc_summary: doc.clone(),
                            });
                        }
                    }

                    items.push(SymbolOverviewItem {
                        name,
                        kind: "function".to_string(),
                        start_line,
                        end_line,
                        signature: Some(sig),
                        doc_summary: doc,
                    });
                }
            }
            "class_definition" => {
                if let Some(name_n) = decl.child_by_field_name("name") {
                    let class_name = AstUtils::node_text(name_n, source).to_string();
                    let doc = extract_py_docstring(decl, source);
                    items.push(SymbolOverviewItem {
                        name: class_name.clone(),
                        kind: "class".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: doc,
                    });

                    if let Some(body) = decl.child_by_field_name("body") {
                        for member in body.named_children(&mut body.walk()) {
                            let m_node = if member.kind() == "decorated_definition" {
                                member.child_by_field_name("definition").unwrap_or(member)
                            } else {
                                member
                            };

                            if m_node.kind() == "function_definition" {
                                if let Some(m_name) = m_node.child_by_field_name("name") {
                                    let m_text = AstUtils::node_text(m_name, source);
                                    let full_name = format!("{class_name}.{m_text}");
                                    let m_start = member.start_position().row + 1;
                                    let m_end = member.end_position().row + 1;
                                    let sig = extract_py_signature_header(m_node, source);
                                    let m_doc = extract_py_docstring(m_node, source);
                                    items.push(SymbolOverviewItem {
                                        name: full_name,
                                        kind: "method".to_string(),
                                        start_line: m_start,
                                        end_line: m_end,
                                        signature: Some(sig),
                                        doc_summary: m_doc,
                                    });
                                }
                            }
                        }
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
                if !base_name.is_empty() {
                    items.push(SymbolOverviewItem {
                        name: base_name.to_string(),
                        kind: "type".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: None,
                    });
                }
            }
            _ => {}
        }
    }

    items
}

fn extract_rs_overview(
    root: Node<'_>,
    source: &str,
    _opts: &OverviewOptions,
) -> Vec<SymbolOverviewItem> {
    let mut items = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        let start_line = child.start_position().row + 1;
        let end_line = child.end_position().row + 1;

        match child.kind() {
            "function_item" => {
                if let Some(name_n) = child.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    let sig = extract_rs_signature_header(child, source);
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "function".to_string(),
                        start_line,
                        end_line,
                        signature: Some(sig),
                        doc_summary: doc,
                    });
                }
            }
            "struct_item" => {
                if let Some(name_n) = child.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "struct".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: doc,
                    });
                }
            }
            "enum_item" => {
                if let Some(name_n) = child.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "enum".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: doc,
                    });
                }
            }
            "trait_item" => {
                if let Some(name_n) = child.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "trait".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: doc,
                    });
                }
            }
            "type_item" => {
                if let Some(name_n) = child.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "type".to_string(),
                        start_line,
                        end_line,
                        signature: None,
                        doc_summary: None,
                    });
                }
            }
            "impl_item" => {
                let trait_prefix = child
                    .child_by_field_name("trait")
                    .map(|t| format!("{} for ", AstUtils::node_text(t, source)));
                let struct_name = child
                    .child_by_field_name("type")
                    .map(|t| AstUtils::node_text(t, source))
                    .unwrap_or("Self");
                let base_struct = struct_name.split('<').next().unwrap_or(struct_name).trim();

                if let Some(body) = child.child_by_field_name("body") {
                    for member in body.named_children(&mut body.walk()) {
                        if member.kind() == "function_item" {
                            if let Some(m_name) = member.child_by_field_name("name") {
                                let m_text = AstUtils::node_text(m_name, source);
                                let m_start = member.start_position().row + 1;
                                let m_end = member.end_position().row + 1;
                                let full_name = match &trait_prefix {
                                    Some(tr) => format!("<{base_struct} as {tr}>::{m_text}"),
                                    None => format!("{base_struct}::{m_text}"),
                                };
                                let sig = extract_rs_signature_header(member, source);
                                let m_doc = extract_leading_doc_comment(member, source);
                                items.push(SymbolOverviewItem {
                                    name: full_name,
                                    kind: "method".to_string(),
                                    start_line: m_start,
                                    end_line: m_end,
                                    signature: Some(sig),
                                    doc_summary: m_doc,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    items
}

fn extract_go_overview(
    root: Node<'_>,
    source: &str,
    _opts: &OverviewOptions,
) -> Vec<SymbolOverviewItem> {
    let mut items = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        let start_line = child.start_position().row + 1;
        let end_line = child.end_position().row + 1;

        match child.kind() {
            "function_declaration" => {
                if let Some(name_n) = child.child_by_field_name("name") {
                    let name = AstUtils::node_text(name_n, source).to_string();
                    let sig = extract_go_signature_header(child, source);
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name,
                        kind: "function".to_string(),
                        start_line,
                        end_line,
                        signature: Some(sig),
                        doc_summary: doc,
                    });
                }
            }
            "method_declaration" => {
                if let Some(name_n) = child.child_by_field_name("name") {
                    let method_name = AstUtils::node_text(name_n, source);
                    let receiver = child
                        .child_by_field_name("receiver")
                        .map(|r| {
                            let t = AstUtils::node_text(r, source);
                            t.trim_matches(['(', ')', '*', ' '])
                                .split_whitespace()
                                .last()
                                .unwrap_or(t)
                        })
                        .unwrap_or("Receiver");
                    let full_name = format!("{receiver}.{method_name}");
                    let sig = extract_go_signature_header(child, source);
                    let doc = extract_leading_doc_comment(child, source);
                    items.push(SymbolOverviewItem {
                        name: full_name,
                        kind: "method".to_string(),
                        start_line,
                        end_line,
                        signature: Some(sig),
                        doc_summary: doc,
                    });
                }
            }
            "type_declaration" => {
                for spec in AstUtils::find_children_by_kind(child, "type_spec") {
                    if let Some(name_n) = spec.child_by_field_name("name") {
                        let name = AstUtils::node_text(name_n, source).to_string();
                        let kind = if let Some(type_n) = spec.child_by_field_name("type") {
                            match type_n.kind() {
                                "struct_type" => "struct",
                                "interface_type" => "interface",
                                _ => "type",
                            }
                        } else {
                            "type"
                        };
                        let doc = extract_leading_doc_comment(child, source);
                        items.push(SymbolOverviewItem {
                            name,
                            kind: kind.to_string(),
                            start_line,
                            end_line,
                            signature: None,
                            doc_summary: doc,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    items
}

fn extract_ts_signature_header(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let start = node.start_byte();
        let body_start = body.start_byte();
        if start <= body_start && body_start <= source.len() {
            let sig = source[start..body_start].trim();
            return sig.trim_end_matches('{').trim().to_string();
        }
    }
    let text = AstUtils::node_text(node, source);
    let first = text.lines().next().unwrap_or(text).trim();
    first.trim_end_matches('{').trim().to_string()
}

fn extract_py_signature_header(node: Node<'_>, source: &str) -> String {
    let decl_start = node.start_byte();
    if let Some(end_node) = node
        .child_by_field_name("return_type")
        .or_else(|| node.child_by_field_name("parameters"))
    {
        let search_start = end_node.end_byte();
        if search_start <= source.len() {
            if let Some(colon_rel) = source[search_start..].find(':') {
                let sig_end = search_start + colon_rel;
                if decl_start < sig_end && sig_end <= source.len() {
                    let sig = source[decl_start..sig_end].trim();
                    let clean = sig.split('#').next().unwrap_or(sig).trim();
                    return format!("{clean}: ...");
                }
            }
        }
    }
    if let Some(body) = node.child_by_field_name("body") {
        let start = node.start_byte();
        let body_start = body.start_byte();
        if start <= body_start && body_start <= source.len() {
            let sig = source[start..body_start].trim();
            let before_comment = sig.split('#').next().unwrap_or(sig).trim();
            let clean = before_comment.trim_end_matches(':').trim();
            return format!("{clean}: ...");
        }
    }
    let text = AstUtils::node_text(node, source);
    let first = text.lines().next().unwrap_or(text).trim();
    let before_comment = first.split('#').next().unwrap_or(first).trim();
    let clean = before_comment.trim_end_matches(':').trim();
    format!("{clean}: ...")
}

fn extract_rs_signature_header(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let start = node.start_byte();
        let body_start = body.start_byte();
        if start <= body_start && body_start <= source.len() {
            let sig = source[start..body_start].trim();
            return format!("{sig};");
        }
    }
    let text = AstUtils::node_text(node, source);
    let first = text.lines().next().unwrap_or(text).trim();
    let trimmed = first.trim_end_matches('{').trim();
    format!("{trimmed};")
}

fn extract_go_signature_header(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let start = node.start_byte();
        let body_start = body.start_byte();
        if start <= body_start && body_start <= source.len() {
            return source[start..body_start].trim().to_string();
        }
    }
    let text = AstUtils::node_text(node, source);
    let first = text.lines().next().unwrap_or(text).trim();
    first.trim_end_matches('{').trim().to_string()
}

fn extract_leading_doc_comment(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(prev) = node.prev_sibling() {
        if prev.kind() == "comment" || prev.kind() == "line_comment" || prev.kind() == "block_comment"
        {
            let text = AstUtils::node_text(prev, source);
            for line in text.lines() {
                let cleaned = line
                    .trim()
                    .trim_start_matches(['/', '*', ' ', '#'])
                    .trim_end_matches(['/', '*', ' '])
                    .trim();
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }
    None
}

fn extract_py_docstring(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(body) = node.child_by_field_name("body") {
        if let Some(first_stmt) = body.named_child(0) {
            if first_stmt.kind() == "expression_statement" {
                if let Some(str_node) = first_stmt.named_child(0) {
                    if str_node.kind() == "string" {
                        let text = AstUtils::node_text(str_node, source);
                        let clean = text.trim_matches(['"', '\'', ' ']);
                        for line in clean.lines() {
                            let l = line.trim();
                            if !l.is_empty() {
                                return Some(l.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn check_ts_express_route(node: Node<'_>, source: &str) -> Option<SymbolOverviewItem> {
    let text = AstUtils::node_text(node, source);
    let lower = text.to_lowercase();
    for method in ["get", "post", "put", "delete", "patch", "options", "head"] {
        let pattern = format!(".{method}(");
        if let Some(idx) = lower.find(&pattern) {
            let after = &text[idx + pattern.len()..];
            if let Some(first_arg) = after.split(',').next() {
                let route_path = first_arg.trim().trim_matches(['\'', '"', '`']);
                if route_path.starts_with('/') {
                    let start_line = node.start_position().row + 1;
                    let end_line = node.end_position().row + 1;
                    return Some(SymbolOverviewItem {
                        name: format!("{} {route_path}", method.to_uppercase()),
                        kind: "route".to_string(),
                        start_line,
                        end_line,
                        signature: Some(text.lines().next().unwrap_or("").trim().to_string()),
                        doc_summary: None,
                    });
                }
            }
        }
    }
    None
}

fn check_py_route(node: Node<'_>, source: &str) -> Option<String> {
    let text = AstUtils::node_text(node, source);
    for method in ["get", "post", "put", "delete", "patch"] {
        let pattern = format!(".{method}(");
        if let Some(idx) = text.find(&pattern) {
            let after = &text[idx + pattern.len()..];
            if let Some(path_arg) = after.split([',', ')']).next() {
                let route_path = path_arg.trim().trim_matches(['\'', '"']);
                if route_path.starts_with('/') {
                    return Some(format!("{} {route_path}", method.to_uppercase()));
                }
            }
        }
    }
    None
}

/// Formats a `WorkspaceOverviewReport` into high-density Markdown.
pub fn format_overview_markdown(report: &WorkspaceOverviewReport) -> String {
    let mut out = String::with_capacity(8192);

    let _ = writeln!(out, "# Workspace Symbol Overview\n");
    let _ = writeln!(
        out,
        "**Root:** `{}` | **Files:** `{}` | **Symbols:** `{}` | **Lines:** `{}`",
        report.root_path, report.total_files, report.total_symbols, report.total_lines
    );
    let _ = writeln!(
        out,
        "**Tokens:** `{}` (Raw: `{}`) | **Token Savings:** `{:.1}%`\n",
        report.total_overview_tokens,
        report.total_raw_tokens,
        report.token_savings_percentage
    );

    if !report.language_breakdown.is_empty() {
        out.push_str("### Language Breakdown\n");
        for lang in &report.language_breakdown {
            let _ = writeln!(
                out,
                "- **{}:** {} files ({} lines, {} tokens)",
                lang.language, lang.file_count, lang.total_lines, lang.estimated_tokens
            );
        }
        out.push('\n');
    }

    out.push_str("---\n\n");

    for file in &report.files {
        if file.symbols.is_empty() {
            continue;
        }

        let _ = writeln!(
            out,
            "### `{}` ({}, {} lines, {} tokens)",
            file.path, file.language, file.total_lines, file.total_tokens
        );

        for sym in &file.symbols {
            let doc_str = sym
                .doc_summary
                .as_ref()
                .map(|d| format!(" — *{d}*"))
                .unwrap_or_default();

            if let Some(ref sig) = sym.signature {
                let _ = writeln!(
                    out,
                    "- `{} {}` (`{}`) (L{}-L{}){}",
                    sym.kind, sym.name, sig, sym.start_line, sym.end_line, doc_str
                );
            } else {
                let _ = writeln!(
                    out,
                    "- `{} {}` (L{}-L{}){}",
                    sym.kind, sym.name, sym.start_line, sym.end_line, doc_str
                );
            }
        }
        out.push('\n');
    }

    out
}
