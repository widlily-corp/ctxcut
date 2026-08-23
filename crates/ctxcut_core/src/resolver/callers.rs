//! Upstream caller and reverse impact analysis engine across polyglot workspaces.

use crate::error::Result;
use crate::lang::LanguageRegistry;
use crate::model::{
    ImpactCallerItem, ImpactSliceResult, SliceOptions, SupportedLanguage, TokenStats,
};
use crate::parser::{AstUtils, ParserManager};
use crate::resolver::imports::ImportResolver;
use crate::tokenizer::{count_lines, count_tokens, TokenCounter};
use crate::traversal::{ProjectWalker, TraversalConfig};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tree_sitter::Node;

/// Analyzer for locating all upstream callers and invocation sites of a target symbol.
pub struct ImpactAnalyzer;

impl ImpactAnalyzer {
    /// Discovers all upstream call sites of `target_symbol` across the workspace.
    pub fn find_callers(
        workspace_root: &Path,
        target_symbol: &str,
        target_file: Option<&Path>,
        opts: &SliceOptions,
    ) -> Result<ImpactSliceResult> {
        let (target_container, target_name) = parse_target_symbol(target_symbol);
        let files = ProjectWalker::collect_files(workspace_root, &TraversalConfig::default());

        let mut discovered_callers: Vec<ImpactCallerItem> = Vec::new();
        let mut total_raw_tokens = 0;
        let mut total_raw_lines = 0;
        let mut files_with_callers = HashSet::new();

        // Canonical target file relative to workspace root if provided
        let canonical_target_file = target_file.and_then(|tf| {
            if tf.is_absolute() {
                tf.strip_prefix(workspace_root).ok().map(|p| p.to_path_buf())
            } else {
                Some(tf.to_path_buf())
            }
        });

        for file_path in &files {
            let Some(lang) = SupportedLanguage::from_path(file_path) else {
                continue;
            };

            let Ok(source) = fs::read_to_string(file_path) else {
                continue;
            };

            // Fast pre-filter: skip file if it doesn't contain target_name substring
            if !source.contains(&target_name) {
                continue;
            }

            let Ok(adapter) = LanguageRegistry::for_language(lang) else {
                continue;
            };
            let ts_lang = adapter.tree_sitter_language(file_path);

            let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, file_path) else {
                continue;
            };

            let root_node = tree.root_node();
            let rel_path = file_path
                .strip_prefix(workspace_root)
                .unwrap_or(file_path)
                .to_string_lossy()
                .replace('\\', "/");

            let file_callers = scan_file_for_callers(
                root_node,
                &source,
                &rel_path,
                &target_name,
                target_container.as_deref(),
                canonical_target_file.as_deref(),
                file_path,
                lang,
            );

            if !file_callers.is_empty() {
                files_with_callers.insert(rel_path);
                total_raw_tokens += TokenCounter::count(&source);
                total_raw_lines += count_lines(&source);
                discovered_callers.extend(file_callers);
            }
        }

        // Deduplicate callers by (file_path, caller_symbol, line_number)
        let mut seen = HashSet::new();
        discovered_callers.retain(|c| {
            seen.insert(format!("{}:{}:{}", c.file_path, c.caller_symbol, c.line_number))
        });

        let total_callers_count = discovered_callers.len();

        // Apply budget compression if requested
        if let Some(budget) = opts.budget {
            let tf_str = canonical_target_file.as_ref().map(|p| p.to_string_lossy());
            apply_impact_budget(
                target_symbol,
                tf_str.as_deref(),
                &mut discovered_callers,
                budget,
            );
        }

        let intermediate_result = ImpactSliceResult {
            target_symbol: target_symbol.to_string(),
            target_file: canonical_target_file.map(|p| p.to_string_lossy().replace('\\', "/")),
            callers: discovered_callers.clone(),
            total_callers: total_callers_count,
            stats: TokenStats::calculate(total_raw_tokens, 0, total_raw_lines, 0),
        };

        let rendered_markdown = intermediate_result.to_markdown();
        let sliced_tokens = count_tokens(&rendered_markdown);
        let sliced_lines = count_lines(&rendered_markdown);

        let final_stats = TokenStats::calculate(
            total_raw_tokens,
            sliced_tokens,
            total_raw_lines,
            sliced_lines,
        );

        Ok(ImpactSliceResult {
            target_symbol: target_symbol.to_string(),
            target_file: intermediate_result.target_file,
            callers: discovered_callers,
            total_callers: total_callers_count,
            stats: final_stats,
        })
    }
}

/// Parses target symbol into (container, member_name).
/// Handles: "AuthService.validate", "AuthService::validate", "validate".
fn parse_target_symbol(query: &str) -> (Option<String>, String) {
    if let Some((container, member)) = query.split_once("::") {
        (Some(container.trim().to_string()), member.trim().to_string())
    } else if let Some((container, member)) = query.split_once('.') {
        (Some(container.trim().to_string()), member.trim().to_string())
    } else {
        (None, query.trim().to_string())
    }
}

/// Scans a single AST file for call sites matching target.
#[allow(clippy::too_many_arguments)]
fn scan_file_for_callers(
    root: Node<'_>,
    source: &str,
    rel_path: &str,
    target_name: &str,
    target_container: Option<&str>,
    target_file: Option<&Path>,
    abs_file_path: &Path,
    lang: SupportedLanguage,
) -> Vec<ImpactCallerItem> {
    let mut callers = Vec::new();
    let imports = ImportResolver::extract_imports(root, source);

    // If target_file is specified, verify this file either IS the target file or imports target
    if let Some(tf) = target_file {
        let is_same_file = abs_file_path.ends_with(tf) || rel_path == tf.to_string_lossy();
        if !is_same_file {
            let has_matching_import = imports.values().any(|imp| {
                if let Some(resolved) =
                    ImportResolver::resolve_module_path(abs_file_path, &imp.specifier)
                {
                    resolved.ends_with(tf)
                } else {
                    false
                }
            });

            if !has_matching_import && imports.contains_key(target_name) {
                return Vec::new();
            }
        }
    }

    let call_nodes = collect_call_nodes(root, lang);

    for call_node in call_nodes {
        if let Some((receiver, func_name)) = extract_call_name(call_node, source, lang) {
            if func_name == target_name {
                // Container verification if target_container is specified
                if let Some(expected_container) = target_container {
                    if let Some(ref rec) = receiver {
                        let matches_rec = rec == expected_container
                            || rec.ends_with(expected_container)
                            || rec.to_lowercase().contains(&expected_container.to_lowercase());
                        if !matches_rec {
                            continue;
                        }
                    }
                }

                // Locate enclosing caller function
                let (caller_sym, caller_kind, caller_sig) =
                    find_enclosing_caller(call_node, source, lang);
                let line_number = call_node.start_position().row + 1;
                let call_snippet = extract_call_snippet(call_node, source);

                callers.push(ImpactCallerItem {
                    caller_symbol: caller_sym,
                    caller_kind,
                    file_path: rel_path.to_string(),
                    line_number,
                    call_snippet,
                    caller_signature: caller_sig,
                });
            }
        }
    }

    callers
}

/// Collects call expression AST nodes per language.
fn collect_call_nodes(root: Node<'_>, lang: SupportedLanguage) -> Vec<Node<'_>> {
    match lang {
        SupportedLanguage::TypeScript
        | SupportedLanguage::JavaScript
        | SupportedLanguage::Vue
        | SupportedLanguage::Svelte
        | SupportedLanguage::Astro => {
            let mut calls = AstUtils::find_descendants_by_kind(root, "call_expression");
            calls.extend(AstUtils::find_descendants_by_kind(root, "new_expression"));
            calls.extend(AstUtils::find_descendants_by_kind(root, "jsx_self_closing_element"));
            calls.extend(AstUtils::find_descendants_by_kind(root, "jsx_opening_element"));
            calls
        }
        SupportedLanguage::Python => AstUtils::find_descendants_by_kind(root, "call"),
        SupportedLanguage::Go => AstUtils::find_descendants_by_kind(root, "call_expression"),
        SupportedLanguage::Rust => {
            let mut calls = AstUtils::find_descendants_by_kind(root, "call_expression");
            calls.extend(AstUtils::find_descendants_by_kind(root, "macro_invocation"));
            calls
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            AstUtils::find_descendants_by_kind(root, "call_expression")
        }
        SupportedLanguage::CSharp => {
            let mut calls = AstUtils::find_descendants_by_kind(root, "invocation_expression");
            calls.extend(AstUtils::find_descendants_by_kind(root, "object_creation_expression"));
            calls
        }
        SupportedLanguage::Java => {
            let mut calls = AstUtils::find_descendants_by_kind(root, "method_invocation");
            calls.extend(AstUtils::find_descendants_by_kind(root, "object_creation_expression"));
            calls
        }
        SupportedLanguage::Kotlin => {
            let mut calls = AstUtils::find_descendants_by_kind(root, "call_expression");
            calls.extend(AstUtils::find_descendants_by_kind(root, "navigation_expression"));
            calls
        }
    }
}

/// Extracts (Option<receiver>, func_name) from a call node.
fn extract_call_name(
    node: Node<'_>,
    source: &str,
    lang: SupportedLanguage,
) -> Option<(Option<String>, String)> {
    match lang {
        SupportedLanguage::TypeScript
        | SupportedLanguage::JavaScript
        | SupportedLanguage::Vue
        | SupportedLanguage::Svelte
        | SupportedLanguage::Astro => {
            if node.kind() == "call_expression" {
                let fn_node = node.child_by_field_name("function")?;
                if fn_node.kind() == "identifier" {
                    Some((None, AstUtils::node_text(fn_node, source).to_string()))
                } else if fn_node.kind() == "member_expression" {
                    let obj = fn_node.child_by_field_name("object")?;
                    let prop = fn_node.child_by_field_name("property")?;
                    Some((
                        Some(AstUtils::node_text(obj, source).to_string()),
                        AstUtils::node_text(prop, source).to_string(),
                    ))
                } else {
                    None
                }
            } else if node.kind() == "new_expression" {
                let ctor = node.child_by_field_name("constructor")?;
                Some((None, AstUtils::node_text(ctor, source).to_string()))
            } else if matches!(node.kind(), "jsx_self_closing_element" | "jsx_opening_element") {
                let name_node = node.child_by_field_name("name")?;
                Some((None, AstUtils::node_text(name_node, source).to_string()))
            } else {
                None
            }
        }
        SupportedLanguage::Python => {
            let fn_node = node.child_by_field_name("function")?;
            if fn_node.kind() == "identifier" {
                Some((None, AstUtils::node_text(fn_node, source).to_string()))
            } else if fn_node.kind() == "attribute" {
                let obj = fn_node.child_by_field_name("object")?;
                let attr = fn_node.child_by_field_name("attribute")?;
                Some((
                    Some(AstUtils::node_text(obj, source).to_string()),
                    AstUtils::node_text(attr, source).to_string(),
                ))
            } else {
                None
            }
        }
        SupportedLanguage::Go => {
            let fn_node = node.child_by_field_name("function")?;
            if fn_node.kind() == "identifier" {
                Some((None, AstUtils::node_text(fn_node, source).to_string()))
            } else if fn_node.kind() == "selector_expression" {
                let operand = fn_node.child_by_field_name("operand")?;
                let field = fn_node.child_by_field_name("field")?;
                Some((
                    Some(AstUtils::node_text(operand, source).to_string()),
                    AstUtils::node_text(field, source).to_string(),
                ))
            } else {
                None
            }
        }
        SupportedLanguage::Rust => {
            if node.kind() == "call_expression" {
                let fn_node = node.child_by_field_name("function")?;
                if fn_node.kind() == "identifier" {
                    Some((None, AstUtils::node_text(fn_node, source).to_string()))
                } else if fn_node.kind() == "scoped_identifier" {
                    let path = fn_node.child_by_field_name("path")?;
                    let name = fn_node.child_by_field_name("name")?;
                    Some((
                        Some(AstUtils::node_text(path, source).to_string()),
                        AstUtils::node_text(name, source).to_string(),
                    ))
                } else if fn_node.kind() == "field_expression" {
                    let val = fn_node.child_by_field_name("value")?;
                    let field = fn_node.child_by_field_name("field")?;
                    Some((
                        Some(AstUtils::node_text(val, source).to_string()),
                        AstUtils::node_text(field, source).to_string(),
                    ))
                } else {
                    None
                }
            } else if node.kind() == "macro_invocation" {
                let macro_node = node.child_by_field_name("macro")?;
                Some((None, AstUtils::node_text(macro_node, source).to_string()))
            } else {
                None
            }
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            if node.kind() == "call_expression" {
                let fn_node = node.child_by_field_name("function")?;
                match fn_node.kind() {
                    "identifier" => Some((None, AstUtils::node_text(fn_node, source).to_string())),
                    "field_expression" => {
                        let arg = fn_node.child_by_field_name("argument")?;
                        let field = fn_node.child_by_field_name("field")?;
                        Some((
                            Some(AstUtils::node_text(arg, source).to_string()),
                            AstUtils::node_text(field, source).to_string(),
                        ))
                    }
                    "qualified_identifier" => {
                        let scope = fn_node.child_by_field_name("scope")?;
                        let name = fn_node.child_by_field_name("name")?;
                        Some((
                            Some(AstUtils::node_text(scope, source).to_string()),
                            AstUtils::node_text(name, source).to_string(),
                        ))
                    }
                    _ => Some((None, AstUtils::node_text(fn_node, source).to_string())),
                }
            } else {
                None
            }
        }
        SupportedLanguage::CSharp => {
            if node.kind() == "invocation_expression" {
                let fn_node = node.child_by_field_name("expression").or_else(|| node.named_child(0))?;
                if fn_node.kind() == "identifier" {
                    Some((None, AstUtils::node_text(fn_node, source).to_string()))
                } else if fn_node.kind() == "member_access_expression" {
                    let obj = fn_node.child_by_field_name("expression")?;
                    let name = fn_node.child_by_field_name("name")?;
                    Some((
                        Some(AstUtils::node_text(obj, source).to_string()),
                        AstUtils::node_text(name, source).to_string(),
                    ))
                } else {
                    Some((None, AstUtils::node_text(fn_node, source).to_string()))
                }
            } else if node.kind() == "object_creation_expression" {
                let ty = node.child_by_field_name("type")?;
                Some((None, AstUtils::node_text(ty, source).to_string()))
            } else {
                None
            }
        }
        SupportedLanguage::Java => {
            if node.kind() == "method_invocation" {
                let name = node.child_by_field_name("name")?;
                let obj = node.child_by_field_name("object");
                Some((
                    obj.map(|o| AstUtils::node_text(o, source).to_string()),
                    AstUtils::node_text(name, source).to_string(),
                ))
            } else if node.kind() == "object_creation_expression" {
                let ty = node.child_by_field_name("type")?;
                Some((None, AstUtils::node_text(ty, source).to_string()))
            } else {
                None
            }
        }
        SupportedLanguage::Kotlin => {
            if node.kind() == "call_expression" {
                let fn_node = node.named_child(0)?;
                Some((None, AstUtils::node_text(fn_node, source).to_string()))
            } else if node.kind() == "navigation_expression" {
                let parts: Vec<&str> = AstUtils::node_text(node, source).split('.').collect();
                if parts.len() >= 2 {
                    let obj = parts[..parts.len() - 1].join(".");
                    let name = parts.last().copied().unwrap_or("").to_string();
                    Some((Some(obj), name))
                } else {
                    Some((None, AstUtils::node_text(node, source).to_string()))
                }
            } else {
                None
            }
        }
    }
}

/// Climbs up the AST to discover the enclosing function, method, or class.
fn find_enclosing_caller(
    call_node: Node<'_>,
    source: &str,
    _lang: SupportedLanguage,
) -> (String, String, Option<String>) {
    let mut current = call_node.parent();

    while let Some(parent) = current {
        match parent.kind() {
            // TypeScript / JS / Vue / Svelte / Astro
            "function_declaration" | "generator_function_declaration" => {
                let name = parent
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source).to_string())
                    .unwrap_or_else(|| "anonymous".to_string());
                let sig = extract_signature_until_body(parent, source);
                return (name, "function".to_string(), sig);
            }
            "method_definition" => {
                let method_name = parent
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source).to_string())
                    .unwrap_or_else(|| "method".to_string());

                let class_name = find_parent_class_name(parent, source);
                let full_name = match class_name {
                    Some(c) => format!("{c}.{method_name}"),
                    None => method_name,
                };
                let sig = extract_signature_until_body(parent, source);
                return (full_name, "method".to_string(), sig);
            }
            "variable_declarator" => {
                if let Some(val) = parent.child_by_field_name("value") {
                    if matches!(val.kind(), "arrow_function" | "function_expression" | "function") {
                        let name = parent
                            .child_by_field_name("name")
                            .map(|n| AstUtils::node_text(n, source).to_string())
                            .unwrap_or_else(|| "anonymous".to_string());
                        let sig = extract_signature_until_body(val, source)
                            .or_else(|| extract_signature_until_body(parent, source));
                        return (name, "function".to_string(), sig);
                    }
                }
            }

            // Python
            "function_definition" => {
                let fn_name = parent
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source).to_string())
                    .unwrap_or_else(|| "function".to_string());

                let class_name = find_parent_class_name(parent, source);
                let full_name = match &class_name {
                    Some(c) => format!("{c}.{fn_name}"),
                    None => fn_name,
                };
                let sig = extract_python_signature(parent, source);
                let kind = if class_name.is_some() { "method" } else { "function" };
                return (full_name, kind.to_string(), Some(sig));
            }

            // Go / Java / C# Method & Function
            "method_declaration" | "constructor_declaration" => {
                let name = parent
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source).to_string())
                    .unwrap_or_else(|| "method".to_string());

                let full_name = if let Some(r) = parent.child_by_field_name("receiver") {
                    let t = AstUtils::node_text(r, source).trim();
                    let clean_rec = t
                        .trim_matches(['(', ')'])
                        .split_whitespace()
                        .last()
                        .unwrap_or(t)
                        .trim_start_matches('*');
                    if clean_rec.is_empty() {
                        name
                    } else {
                        format!("{clean_rec}.{name}")
                    }
                } else if let Some(c) = find_parent_class_name(parent, source) {
                    format!("{c}.{name}")
                } else {
                    name
                };

                let sig = extract_signature_until_body(parent, source);
                return (full_name, "method".to_string(), sig);
            }

            // Rust
            "function_item" => {
                let name = parent
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source).to_string())
                    .unwrap_or_else(|| "fn".to_string());

                let impl_type = find_parent_rust_impl_type(parent, source);
                let full_name = match &impl_type {
                    Some(t) => format!("{t}::{name}"),
                    None => name,
                };
                let sig = extract_rust_signature_header(parent, source);
                let kind = if impl_type.is_some() { "method" } else { "function" };
                return (full_name, kind.to_string(), Some(sig));
            }

            _ => {}
        }
        current = parent.parent();
    }

    ("(module)".to_string(), "module".to_string(), None)
}

fn find_parent_class_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut curr = node.parent();
    while let Some(n) = curr {
        if matches!(n.kind(), "class_declaration" | "class_definition" | "class") {
            if let Some(name_node) = n.child_by_field_name("name") {
                return Some(AstUtils::node_text(name_node, source).to_string());
            }
        }
        curr = n.parent();
    }
    None
}

fn find_parent_rust_impl_type(node: Node<'_>, source: &str) -> Option<String> {
    let mut curr = node.parent();
    while let Some(n) = curr {
        if n.kind() == "impl_item" {
            if let Some(t_node) = n.child_by_field_name("type") {
                let text = AstUtils::node_text(t_node, source);
                let base = text.split('<').next().unwrap_or(text).trim();
                return Some(base.to_string());
            }
        }
        curr = n.parent();
    }
    None
}

fn extract_signature_until_body(node: Node<'_>, source: &str) -> Option<String> {
    let body_node = node.child_by_field_name("body")?;
    let start = node.start_byte();
    let body_start = body_node.start_byte();
    if start < body_start && body_start <= source.len() {
        let sig = source[start..body_start].trim();
        Some(sig.to_string())
    } else {
        None
    }
}

fn extract_python_signature(node: Node<'_>, source: &str) -> String {
    let text = AstUtils::node_text(node, source);
    if let Some(colon_pos) = text.find(':') {
        text[..=colon_pos].trim().to_string()
    } else {
        text.lines().next().unwrap_or(text).trim().to_string()
    }
}

fn extract_rust_signature_header(node: Node<'_>, source: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        let start = node.start_byte();
        let body_start = body.start_byte();
        if start < body_start && body_start <= source.len() {
            return source[start..body_start].trim().to_string();
        }
    }
    let text = AstUtils::node_text(node, source);
    text.lines().next().unwrap_or(text).trim().to_string()
}

fn extract_call_snippet(call_node: Node<'_>, source: &str) -> String {
    let text = AstUtils::node_text(call_node, source);
    if text.lines().count() <= 3 {
        text.trim().to_string()
    } else {
        let first = text.lines().next().unwrap_or("").trim();
        format!("{first} ...")
    }
}

fn apply_impact_budget(
    target_symbol: &str,
    target_file: Option<&str>,
    callers: &mut Vec<ImpactCallerItem>,
    budget: usize,
) {
    let dummy_stats = TokenStats::calculate(0, 0, 0, 0);
    let mut check_res = ImpactSliceResult {
        target_symbol: target_symbol.to_string(),
        target_file: target_file.map(String::from),
        callers: callers.clone(),
        total_callers: callers.len(),
        stats: dummy_stats,
    };

    if count_tokens(&check_res.to_markdown()) <= budget {
        return;
    }

    // Level 1: Fold snippets to 1 line
    for c in callers.iter_mut() {
        let first = c.call_snippet.lines().next().unwrap_or(&c.call_snippet).trim();
        c.call_snippet = first.to_string();
    }
    check_res.callers.clone_from(callers);
    if count_tokens(&check_res.to_markdown()) <= budget {
        return;
    }

    // Level 2: Truncate callers progressively
    while callers.len() > 1 {
        callers.pop();
        check_res.callers.clone_from(callers);
        if count_tokens(&check_res.to_markdown()) <= budget {
            return;
        }
    }
}
