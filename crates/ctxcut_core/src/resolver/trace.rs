//! End-to-end execution flow tracer isolating invocation pathways from entry to sinks.

use crate::error::{CoreError, Result};
use crate::lang::LanguageRegistry;
use crate::model::{SliceOptions, SupportedLanguage, TokenStats, TraceResult, TraceStep};
use crate::parser::{AstUtils, ParserManager};
use crate::resolver::imports::ImportResolver;
use crate::tokenizer::{count_lines, count_tokens};
use crate::traversal::{ProjectWalker, TraversalConfig};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Tree};

/// Default maximum traversal depth for execution flow tracing.
pub const DEFAULT_MAX_TRACE_DEPTH: usize = 8;
/// Default target token budget for execution flow tracing.
pub const DEFAULT_TRACE_BUDGET: usize = 1500;

/// End-to-end execution flow tracer.
pub struct ExecutionTracer;

impl ExecutionTracer {
    /// Traces end-to-end execution flow starting from an entry point down to services and database sinks.
    pub fn trace(
        workspace_root: &Path,
        entry_query: &str,
        opts: &SliceOptions,
    ) -> Result<TraceResult> {
        let max_depth = if opts.depth > 0 {
            opts.depth
        } else {
            DEFAULT_MAX_TRACE_DEPTH
        };
        let budget = opts.budget.unwrap_or(DEFAULT_TRACE_BUDGET);

        // 1. Resolve Entry Point
        let (entry_file, _entry_symbol_name, entry_symbol) =
            resolve_entry_point(workspace_root, entry_query)?;

        let mut steps = Vec::new();
        let mut visited = HashSet::new();
        let mut file_cache: HashMap<PathBuf, (String, Tree)> = HashMap::new();

        let mut current_file = entry_file.clone();
        let mut current_symbol = entry_symbol;
        let mut step_num = 1;

        visited.insert((current_file.clone(), current_symbol.name.clone()));

        // 2. Traversal Loop
        while step_num <= max_depth {
            let (source, tree) = load_ast(&current_file, &mut file_cache)?;
            let root = tree.root_node();

            let lang = SupportedLanguage::from_path(&current_file)
                .unwrap_or(SupportedLanguage::TypeScript);
            let adapter = LanguageRegistry::for_language(lang)?;
            let (_extracted, symbol_node) = adapter.locate_symbol(
                root,
                source,
                &current_symbol.name,
                &current_file,
            )?;

            // Extract outgoing domain calls
            let outgoing_candidates = extract_outgoing_calls(symbol_node, source, lang);
            let scored_calls = score_and_rank_callees(&outgoing_candidates);

            let outgoing_names: Vec<String> =
                scored_calls.iter().map(|c| c.full_name.clone()).collect();
            let mut next_step_target = None;
            let mut next_resolved = None;

            // Find next unvisited high-priority spine target
            for candidate in &scored_calls {
                if let Some((target_path, target_sym)) = resolve_callee_definition(
                    workspace_root,
                    &current_file,
                    candidate,
                    &mut file_cache,
                ) {
                    let visit_key = (target_path.clone(), target_sym.name.clone());
                    if visited.contains(&visit_key) {
                        next_step_target =
                            Some(format!("{} (cycle detected)", candidate.full_name));
                        break;
                    }

                    next_step_target = Some(candidate.full_name.clone());
                    next_resolved = Some((target_path, target_sym));
                    break;
                }
            }

            let kind =
                determine_symbol_kind(&current_symbol.name, step_num, next_resolved.is_none());

            steps.push(TraceStep {
                step_number: step_num,
                symbol_name: current_symbol.name.clone(),
                kind,
                file_path: current_file
                    .strip_prefix(workspace_root)
                    .unwrap_or(&current_file)
                    .to_string_lossy()
                    .replace('\\', "/"),
                start_line: current_symbol.start_line,
                end_line: current_symbol.end_line,
                language: current_symbol.language.clone(),
                signature: current_symbol.signature.clone(),
                code_snippet: current_symbol.body.clone(),
                outgoing_calls: outgoing_names,
                next_target: next_step_target,
            });

            // Advance to next step or terminate
            if let Some((next_path, next_sym)) = next_resolved {
                visited.insert((next_path.clone(), next_sym.name.clone()));
                current_file = next_path;
                current_symbol = next_sym;
                step_num += 1;
            } else {
                break;
            }
        }

        let total_steps = steps.len();
        let mut total_raw_tokens = 0;
        let mut total_raw_lines = 0;
        let mut counted_files = HashSet::new();
        for step in &steps {
            if counted_files.insert(step.file_path.clone()) {
                let full_path = workspace_root.join(&step.file_path);
                if let Ok(src) = fs::read_to_string(&full_path) {
                    total_raw_tokens += count_tokens(&src);
                    total_raw_lines += count_lines(&src);
                }
            }
        }
        if total_raw_tokens == 0 {
            total_raw_tokens = steps.iter().map(|s| count_tokens(&s.code_snippet)).sum();
            total_raw_lines = steps.iter().map(|s| s.end_line.saturating_sub(s.start_line) + 1).sum();
        }

        let mut trace_result = TraceResult {
            entry_point: entry_query.to_string(),
            entry_file: entry_file
                .strip_prefix(workspace_root)
                .unwrap_or(&entry_file)
                .to_string_lossy()
                .replace('\\', "/"),
            steps,
            total_steps,
            stats: TokenStats::calculate(total_raw_tokens, total_raw_tokens, total_raw_lines, total_raw_lines),
        };

        // 3. Progressive Token Budgeting
        compress_trace_budget(&mut trace_result, budget)?;

        // Recalculate stats
        let final_md = trace_result.to_markdown();
        let sliced_tokens = count_tokens(&final_md);
        let sliced_lines = count_lines(&final_md);
        trace_result.stats =
            TokenStats::calculate(total_raw_tokens, sliced_tokens, total_raw_lines, sliced_lines);

        Ok(trace_result)
    }
}

#[derive(Debug, Clone)]
struct CalleeCandidate {
    receiver: Option<String>,
    method_name: String,
    full_name: String,
    score: i32,
}

fn resolve_entry_point(
    workspace_root: &Path,
    entry_query: &str,
) -> Result<(PathBuf, String, crate::model::ExtractedSymbol)> {
    let trimmed = entry_query.trim();

    // 1. HTTP Route Pattern (e.g. "POST /api/v1/orders", "GET /users")
    if let Some((method, route_path)) = trimmed.split_once(' ') {
        let method_upper = method.trim().to_uppercase();
        if matches!(
            method_upper.as_str(),
            "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS"
        ) {
            let files = ProjectWalker::collect_files(workspace_root, &TraversalConfig::default());
            let target_path_clean = route_path.trim().trim_matches('/');
            let method_lower = method_upper.to_lowercase();

            for file in files {
                let Some(lang) = SupportedLanguage::from_path(&file) else {
                    continue;
                };
                let Ok(source) = fs::read_to_string(&file) else {
                    continue;
                };

                if let Some(handler_sym) =
                    extract_route_handler(&source, &method_upper, &method_lower, target_path_clean)
                {
                    if let Ok(adapter) = LanguageRegistry::for_language(lang) {
                        let ts_lang = adapter.tree_sitter_language(&file);
                        if let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, &file) {
                            if let Ok((sym, _)) =
                                adapter.locate_symbol(tree.root_node(), &source, &handler_sym, &file)
                            {
                                return Ok((file, handler_sym, sym));
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. File + Symbol Pattern (e.g. "src/order.ts:createOrder")
    if let Some((file_part, sym_part)) = trimmed.split_once(':') {
        let file_path = if Path::new(file_part).is_absolute() {
            PathBuf::from(file_part)
        } else {
            workspace_root.join(file_part)
        };

        if file_path.exists() {
            let source = fs::read_to_string(&file_path).map_err(|e| CoreError::Io {
                path: file_path.clone(),
                source: e,
            })?;
            let lang = SupportedLanguage::from_path(&file_path)
                .unwrap_or(SupportedLanguage::TypeScript);
            let adapter = LanguageRegistry::for_language(lang)?;
            let ts_lang = adapter.tree_sitter_language(&file_path);
            let tree = ParserManager::parse_source(&source, &ts_lang, &file_path)?;
            let (sym, _) = adapter.locate_symbol(
                tree.root_node(),
                &source,
                sym_part.trim(),
                &file_path,
            )?;
            return Ok((file_path, sym_part.trim().to_string(), sym));
        }
    }

    // 3. Unqualified or Qualified Symbol Query across workspace (e.g. "OrderController.createOrder", "createOrder")
    let files = ProjectWalker::collect_files(workspace_root, &TraversalConfig::default());
    let (container_opt, base_name) = parse_query_parts(trimmed);

    let mut best_match = None;

    for file in files {
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };
        if !source.contains(base_name) {
            continue;
        }
        let Some(lang) = SupportedLanguage::from_path(&file) else {
            continue;
        };
        let Ok(adapter) = LanguageRegistry::for_language(lang) else {
            continue;
        };
        let ts_lang = adapter.tree_sitter_language(&file);
        let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, &file) else {
            continue;
        };

        if let Ok((sym, _)) = adapter.locate_symbol(tree.root_node(), &source, trimmed, &file) {
            return Ok((file, trimmed.to_string(), sym));
        }

        if let Ok((sym, _)) = adapter.locate_symbol(tree.root_node(), &source, base_name, &file) {
            if let Some(c) = container_opt {
                if sym.name.contains(c) || file.to_string_lossy().contains(c) {
                    return Ok((file, sym.name.clone(), sym));
                }
            }
            if best_match.is_none() {
                best_match = Some((file, sym.name.clone(), sym));
            }
        }
    }

    if let Some(matched) = best_match {
        return Ok(matched);
    }

    Err(CoreError::SymbolNotFound {
        symbol: entry_query.to_string(),
        path: workspace_root.to_path_buf(),
        available_symbols: vec![],
    })
}

fn parse_query_parts(query: &str) -> (Option<&str>, &str) {
    if let Some((c, m)) = query.split_once("::") {
        (Some(c.trim()), m.trim())
    } else if let Some((c, m)) = query.split_once('.') {
        (Some(c.trim()), m.trim())
    } else {
        (None, query.trim())
    }
}

fn load_ast<'a>(
    path: &Path,
    cache: &'a mut HashMap<PathBuf, (String, Tree)>,
) -> Result<(&'a str, &'a Tree)> {
    if !cache.contains_key(path) {
        let source = fs::read_to_string(path).map_err(|e| CoreError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let lang =
            SupportedLanguage::from_path(path).unwrap_or(SupportedLanguage::TypeScript);
        let adapter = LanguageRegistry::for_language(lang)?;
        let ts_lang = adapter.tree_sitter_language(path);
        let tree = ParserManager::parse_source(&source, &ts_lang, path)?;
        cache.insert(path.to_path_buf(), (source, tree));
    }
    let (s, t) = cache.get(path).unwrap();
    Ok((s.as_str(), t))
}

fn extract_outgoing_calls(
    node: Node<'_>,
    source: &str,
    lang: SupportedLanguage,
) -> Vec<CalleeCandidate> {
    let mut candidates = Vec::new();
    let call_nodes = match lang {
        SupportedLanguage::TypeScript
        | SupportedLanguage::JavaScript
        | SupportedLanguage::Vue
        | SupportedLanguage::Svelte
        | SupportedLanguage::Astro => {
            let mut calls = AstUtils::find_descendants_by_kind(node, "call_expression");
            calls.extend(AstUtils::find_descendants_by_kind(node, "new_expression"));
            calls
        }
        SupportedLanguage::Python => AstUtils::find_descendants_by_kind(node, "call"),
        SupportedLanguage::Go => AstUtils::find_descendants_by_kind(node, "call_expression"),
        SupportedLanguage::Rust => AstUtils::find_descendants_by_kind(node, "call_expression"),
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            AstUtils::find_descendants_by_kind(node, "call_expression")
        }
        SupportedLanguage::CSharp => {
            let mut calls = AstUtils::find_descendants_by_kind(node, "invocation_expression");
            calls.extend(AstUtils::find_descendants_by_kind(node, "object_creation_expression"));
            calls
        }
        SupportedLanguage::Java => {
            let mut calls = AstUtils::find_descendants_by_kind(node, "method_invocation");
            calls.extend(AstUtils::find_descendants_by_kind(node, "object_creation_expression"));
            calls
        }
        SupportedLanguage::Kotlin => {
            let mut calls = AstUtils::find_descendants_by_kind(node, "call_expression");
            calls.extend(AstUtils::find_descendants_by_kind(node, "navigation_expression"));
            calls
        }
    };

    for call in call_nodes {
        if let Some((receiver, method_name)) = extract_call_parts(call, source, lang) {
            if is_noise_or_builtin(receiver.as_deref(), &method_name, lang) {
                continue;
            }
            let full_name = match &receiver {
                Some(r) => format!("{r}.{method_name}"),
                None => method_name.clone(),
            };
            let score = score_call(receiver.as_deref(), &method_name);
            candidates.push(CalleeCandidate {
                receiver,
                method_name,
                full_name,
                score,
            });
        }
    }

    candidates
}

fn extract_call_parts(
    call: Node<'_>,
    source: &str,
    lang: SupportedLanguage,
) -> Option<(Option<String>, String)> {
    match lang {
        SupportedLanguage::TypeScript
        | SupportedLanguage::JavaScript
        | SupportedLanguage::Vue
        | SupportedLanguage::Svelte
        | SupportedLanguage::Astro => {
            if call.kind() == "call_expression" {
                let func = call.child_by_field_name("function")?;
                if func.kind() == "identifier" {
                    Some((None, AstUtils::node_text(func, source).to_string()))
                } else if func.kind() == "member_expression" {
                    let obj = func.child_by_field_name("object")?;
                    let prop = func.child_by_field_name("property")?;
                    Some((
                        Some(AstUtils::node_text(obj, source).to_string()),
                        AstUtils::node_text(prop, source).to_string(),
                    ))
                } else {
                    None
                }
            } else if call.kind() == "new_expression" {
                let ctor = call.child_by_field_name("constructor")?;
                Some((None, AstUtils::node_text(ctor, source).to_string()))
            } else {
                None
            }
        }
        SupportedLanguage::Python => {
            let func = call.child_by_field_name("function")?;
            if func.kind() == "identifier" {
                Some((None, AstUtils::node_text(func, source).to_string()))
            } else if func.kind() == "attribute" {
                let obj = func.child_by_field_name("object")?;
                let attr = func.child_by_field_name("attribute")?;
                Some((
                    Some(AstUtils::node_text(obj, source).to_string()),
                    AstUtils::node_text(attr, source).to_string(),
                ))
            } else {
                None
            }
        }
        SupportedLanguage::Go => {
            let func = call.child_by_field_name("function")?;
            if func.kind() == "identifier" {
                Some((None, AstUtils::node_text(func, source).to_string()))
            } else if func.kind() == "selector_expression" {
                let operand = func.child_by_field_name("operand")?;
                let field = func.child_by_field_name("field")?;
                Some((
                    Some(AstUtils::node_text(operand, source).to_string()),
                    AstUtils::node_text(field, source).to_string(),
                ))
            } else {
                None
            }
        }
        SupportedLanguage::Rust => {
            let func = call.child_by_field_name("function")?;
            if func.kind() == "identifier" {
                Some((None, AstUtils::node_text(func, source).to_string()))
            } else if func.kind() == "scoped_identifier" {
                let path = func.child_by_field_name("path")?;
                let name = func.child_by_field_name("name")?;
                Some((
                    Some(AstUtils::node_text(path, source).to_string()),
                    AstUtils::node_text(name, source).to_string(),
                ))
            } else if func.kind() == "field_expression" {
                let val = func.child_by_field_name("value")?;
                let field = func.child_by_field_name("field")?;
                Some((
                    Some(AstUtils::node_text(val, source).to_string()),
                    AstUtils::node_text(field, source).to_string(),
                ))
            } else {
                None
            }
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            if call.kind() == "call_expression" {
                let func = call.child_by_field_name("function")?;
                if func.kind() == "identifier" {
                    Some((None, AstUtils::node_text(func, source).to_string()))
                } else if func.kind() == "field_expression" {
                    let arg = func.child_by_field_name("argument")?;
                    let field = func.child_by_field_name("field")?;
                    Some((
                        Some(AstUtils::node_text(arg, source).to_string()),
                        AstUtils::node_text(field, source).to_string(),
                    ))
                } else if func.kind() == "qualified_identifier" {
                    let scope = func.child_by_field_name("scope")?;
                    let name = func.child_by_field_name("name")?;
                    Some((
                        Some(AstUtils::node_text(scope, source).to_string()),
                        AstUtils::node_text(name, source).to_string(),
                    ))
                } else {
                    Some((None, AstUtils::node_text(func, source).to_string()))
                }
            } else {
                None
            }
        }
        SupportedLanguage::CSharp => {
            if call.kind() == "invocation_expression" {
                let fn_node = call.child_by_field_name("expression").or_else(|| call.named_child(0))?;
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
            } else if call.kind() == "object_creation_expression" {
                let ty = call.child_by_field_name("type")?;
                Some((None, AstUtils::node_text(ty, source).to_string()))
            } else {
                None
            }
        }
        SupportedLanguage::Java => {
            if call.kind() == "method_invocation" {
                let name = call.child_by_field_name("name")?;
                let obj = call.child_by_field_name("object");
                Some((
                    obj.map(|o| AstUtils::node_text(o, source).to_string()),
                    AstUtils::node_text(name, source).to_string(),
                ))
            } else if call.kind() == "object_creation_expression" {
                let ty = call.child_by_field_name("type")?;
                Some((None, AstUtils::node_text(ty, source).to_string()))
            } else {
                None
            }
        }
        SupportedLanguage::Kotlin => {
            if call.kind() == "call_expression" {
                let fn_node = call.named_child(0)?;
                Some((None, AstUtils::node_text(fn_node, source).to_string()))
            } else if call.kind() == "navigation_expression" {
                let parts: Vec<&str> = AstUtils::node_text(call, source).split('.').collect();
                if parts.len() >= 2 {
                    let obj = parts[..parts.len() - 1].join(".");
                    let name = parts.last().copied().unwrap_or("").to_string();
                    Some((Some(obj), name))
                } else {
                    Some((None, AstUtils::node_text(call, source).to_string()))
                }
            } else {
                None
            }
        }
    }
}

fn is_noise_or_builtin(
    receiver: Option<&str>,
    method_name: &str,
    _lang: SupportedLanguage,
) -> bool {
    let method_lower = method_name.to_lowercase();
    if matches!(
        method_lower.as_str(),
        "log"
            | "info"
            | "warn"
            | "error"
            | "debug"
            | "trace"
            | "println"
            | "print"
            | "printf"
            | "sprintf"
            | "format"
            | "json"
            | "send"
            | "status"
            | "header"
            | "cookie"
            | "to_string"
            | "tostring"
            | "into"
            | "clone"
            | "unwrap"
            | "expect"
            | "len"
            | "make"
            | "append"
            | "str"
            | "int"
            | "float"
            | "dict"
            | "list"
            | "set"
            | "range"
            | "enumerate"
            | "isinstance"
            | "parse"
            | "stringify"
            | "push"
            | "pop"
            | "shift"
            | "unshift"
            | "slice"
            | "splice"
            | "map"
            | "filter"
            | "reduce"
            | "foreach"
            | "includes"
            | "indexof"
            | "trim"
            | "split"
            | "join"
            | "replace"
    ) {
        if let Some(r) = receiver {
            let r_lower = r.to_lowercase();
            if matches!(
                r_lower.as_str(),
                "console" | "math" | "json" | "res" | "response" | "req" | "request" | "fmt" | "log" | "logger"
            ) {
                return true;
            }
        } else {
            return true;
        }
    }

    if let Some(r) = receiver {
        let r_lower = r.to_lowercase();
        if matches!(
            r_lower.as_str(),
            "console" | "math" | "json" | "object" | "array" | "promise" | "fmt" | "logging"
        ) {
            return true;
        }
    }

    false
}

fn score_call(receiver: Option<&str>, method_name: &str) -> i32 {
    let lower_m = method_name.to_lowercase();
    let lower_r = receiver.unwrap_or("").to_lowercase();

    // 100: DB / Storage / Repository
    if lower_r.contains("db")
        || lower_r.contains("repo")
        || lower_r.contains("prisma")
        || lower_r.contains("drizzle")
        || lower_r.contains("sql")
        || lower_m.starts_with("save")
        || lower_m.starts_with("find")
        || lower_m.starts_with("insert")
        || lower_m.starts_with("update")
        || lower_m.starts_with("delete")
        || lower_m.starts_with("query")
        || lower_m.starts_with("select")
        || lower_m.starts_with("execute")
    {
        return 100;
    }

    // 80: Service / Business Core
    if lower_r.contains("service")
        || lower_r.contains("manager")
        || lower_r.contains("handler")
        || lower_r.contains("usecase")
        || lower_r.contains("processor")
        || lower_m.starts_with("process")
        || lower_m.starts_with("handle")
        || lower_m.starts_with("dispatch")
        || lower_m.contains("service")
    {
        return 80;
    }

    // 60: External RPC / HTTP Client
    if lower_r.contains("client")
        || lower_r.contains("stripe")
        || lower_r.contains("payment")
        || lower_r.contains("http")
        || lower_r.contains("axios")
        || lower_r.contains("grpc")
    {
        return 60;
    }

    // 40: Validator / Checker
    if lower_m.starts_with("validate")
        || lower_m.starts_with("check")
        || lower_m.starts_with("verify")
        || lower_m.starts_with("calculate")
    {
        return 40;
    }

    // 10: Utility / Helper
    10
}

fn score_and_rank_callees(candidates: &[CalleeCandidate]) -> Vec<CalleeCandidate> {
    let mut ranked = candidates.to_vec();
    ranked.sort_by_key(|b| std::cmp::Reverse(b.score));
    ranked
}

fn resolve_callee_definition(
    workspace_root: &Path,
    current_file: &Path,
    candidate: &CalleeCandidate,
    cache: &mut HashMap<PathBuf, (String, Tree)>,
) -> Option<(PathBuf, crate::model::ExtractedSymbol)> {
    let (source, tree) = load_ast(current_file, cache).ok()?;
    let root = tree.root_node();
    let lang = SupportedLanguage::from_path(current_file).unwrap_or(SupportedLanguage::TypeScript);
    let adapter = LanguageRegistry::for_language(lang).ok()?;

    // A. Check local file for method or receiver.method or class method
    if let Ok((sym, _)) =
        adapter.locate_symbol(root, source, &candidate.method_name, current_file)
    {
        return Some((current_file.to_path_buf(), sym));
    }
    if let Ok((sym, _)) = adapter.locate_symbol(root, source, &candidate.full_name, current_file) {
        return Some((current_file.to_path_buf(), sym));
    }

    // B. Check imports in current file
    let imports = ImportResolver::extract_imports(root, source);
    let search_names = [
        candidate.method_name.as_str(),
        candidate.receiver.as_deref().unwrap_or(""),
    ];

    for name in search_names {
        if name.is_empty() {
            continue;
        }
        if let Some(imp) = imports.get(name) {
            if let Some(resolved_path) =
                ImportResolver::resolve_module_path(current_file, &imp.specifier)
            {
                if let Ok((imp_src, imp_tree)) = load_ast(&resolved_path, cache) {
                    let imp_lang = SupportedLanguage::from_path(&resolved_path)
                        .unwrap_or(SupportedLanguage::TypeScript);
                    if let Ok(imp_adapter) = LanguageRegistry::for_language(imp_lang) {
                        let imp_root = imp_tree.root_node();
                        if let Ok((sym, _)) = imp_adapter.locate_symbol(
                            imp_root,
                            imp_src,
                            &candidate.method_name,
                            &resolved_path,
                        ) {
                            return Some((resolved_path, sym));
                        }
                        if let Ok((sym, _)) = imp_adapter.locate_symbol(
                            imp_root,
                            imp_src,
                            &candidate.full_name,
                            &resolved_path,
                        ) {
                            return Some((resolved_path, sym));
                        }
                    }
                }
            }
        }
    }

    // Also check all imported files for candidate symbol
    for imp in imports.values() {
        if let Some(resolved_path) =
            ImportResolver::resolve_module_path(current_file, &imp.specifier)
        {
            if let Ok((imp_src, imp_tree)) = load_ast(&resolved_path, cache) {
                let imp_lang = SupportedLanguage::from_path(&resolved_path)
                    .unwrap_or(SupportedLanguage::TypeScript);
                if let Ok(imp_adapter) = LanguageRegistry::for_language(imp_lang) {
                    let imp_root = imp_tree.root_node();
                    if let Ok((sym, _)) = imp_adapter.locate_symbol(
                        imp_root,
                        imp_src,
                        &candidate.method_name,
                        &resolved_path,
                    ) {
                        return Some((resolved_path, sym));
                    }
                    if let Ok((sym, _)) = imp_adapter.locate_symbol(
                        imp_root,
                        imp_src,
                        &candidate.full_name,
                        &resolved_path,
                    ) {
                        return Some((resolved_path, sym));
                    }
                }
            }
        }
    }

    // C. Check sibling files in the same directory
    if let Some(dir) = current_file.parent() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p != current_file && p.is_file() {
                    if let Some(s_lang) = SupportedLanguage::from_path(&p) {
                        if let Ok((s_src, s_tree)) = load_ast(&p, cache) {
                            if s_src.contains(&candidate.method_name) {
                                if let Ok(s_adapter) = LanguageRegistry::for_language(s_lang) {
                                    let s_root = s_tree.root_node();
                                    if let Ok((sym, _)) = s_adapter.locate_symbol(
                                        s_root,
                                        s_src,
                                        &candidate.method_name,
                                        &p,
                                    ) {
                                        return Some((p, sym));
                                    }
                                    if let Ok((sym, _)) = s_adapter.locate_symbol(
                                        s_root,
                                        s_src,
                                        &candidate.full_name,
                                        &p,
                                    ) {
                                        return Some((p, sym));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // D. Scan workspace files
    let config = TraversalConfig::default();
    let all_files = ProjectWalker::collect_files(workspace_root, &config);
    for p in all_files {
        if p != current_file {
            if let Some(w_lang) = SupportedLanguage::from_path(&p) {
                if let Ok((w_src, w_tree)) = load_ast(&p, cache) {
                    if w_src.contains(&candidate.method_name) {
                        if let Ok(w_adapter) = LanguageRegistry::for_language(w_lang) {
                            let w_root = w_tree.root_node();
                            if let Ok((sym, _)) =
                                w_adapter.locate_symbol(w_root, w_src, &candidate.method_name, &p)
                            {
                                return Some((p, sym));
                            }
                            if let Ok((sym, _)) =
                                w_adapter.locate_symbol(w_root, w_src, &candidate.full_name, &p)
                            {
                                return Some((p, sym));
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn determine_symbol_kind(name: &str, step_num: usize, is_leaf: bool) -> String {
    let lower = name.to_lowercase();
    if step_num == 1 {
        if lower.contains("controller") || lower.contains("route") || lower.contains("handle") {
            "controller".to_string()
        } else {
            "entry_point".to_string()
        }
    } else if is_leaf
        && (lower.contains("query")
            || lower.contains("save")
            || lower.contains("find")
            || lower.contains("repo")
            || lower.contains("db"))
    {
        "database_sink".to_string()
    } else if lower.contains("repo") {
        "repository".to_string()
    } else if lower.contains("service") || lower.contains("manager") || lower.contains("process") {
        "service".to_string()
    } else {
        "function".to_string()
    }
}

fn compress_trace_budget(trace: &mut TraceResult, budget: usize) -> Result<()> {
    let md = trace.to_markdown();
    if count_tokens(&md) <= budget {
        return Ok(());
    }

    // Level 1: Strip comments & empty lines
    for step in &mut trace.steps {
        step.code_snippet = strip_comments(&step.code_snippet);
    }
    if count_tokens(&trace.to_markdown()) <= budget {
        return Ok(());
    }

    // Level 2: Keep Entry (Step 1) and Sink (Step N) full, fold intermediate steps (Steps 2..N-1)
    let total = trace.steps.len();
    if total > 2 {
        for i in 1..(total - 1) {
            trace.steps[i].code_snippet = fold_to_call_snippet(
                &trace.steps[i].code_snippet,
                trace.steps[i].next_target.as_deref(),
            );
        }
    }
    if count_tokens(&trace.to_markdown()) <= budget {
        return Ok(());
    }

    // Level 3: Fold all steps to invocation stubs
    for step in &mut trace.steps {
        step.code_snippet =
            fold_to_call_snippet(&step.code_snippet, step.next_target.as_deref());
    }
    if count_tokens(&trace.to_markdown()) <= budget {
        return Ok(());
    }

    // Level 4: Signature-only collapsed stubs
    for step in &mut trace.steps {
        let sig = step.signature.trim().trim_end_matches(';');
        step.code_snippet = format!("{sig};\n// [Collapsed for token budget]");
    }

    Ok(())
}

fn strip_comments(source: &str) -> String {
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        if !trimmed.is_empty() {
            out.push(line);
        }
    }
    out.join("\n")
}

fn fold_to_call_snippet(source: &str, target_call: Option<&str>) -> String {
    if let Some(target) = target_call {
        let base_target = target.split('.').next_back().unwrap_or(target);
        let mut matching_lines = Vec::new();
        for line in source.lines() {
            if line.contains(base_target) {
                matching_lines.push(line.trim());
            }
        }
        if !matching_lines.is_empty() {
            return format!("// ...\n{}\n// ...", matching_lines.join("\n"));
        }
    }

    let first = source.lines().next().unwrap_or("").trim();
    format!("{first} ...")
}

fn extract_route_handler(
    source: &str,
    method_upper: &str,
    method_lower: &str,
    target_path_clean: &str,
) -> Option<String> {
    let last_segment = target_path_clean
        .split('/')
        .next_back()
        .unwrap_or(target_path_clean);

    for line in source.lines() {
        let line_trimmed = line.trim();

        // 1. Express / Koa / Node: router.post('/...', ..., handler) or app.get('/...', handler)
        if (line_trimmed.contains(&format!(".{method_lower}("))
            || line_trimmed.contains(&format!(".{method_upper}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            if let Some(before_paren) = line_trimmed.rsplit_once(')').map(|x| x.0) {
                if let Some(last_arg) = before_paren.rsplit(',').next().map(str::trim) {
                    if let Some(ident) = last_arg
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .find(|s| !s.is_empty())
                    {
                        return Some(ident.to_string());
                    }
                }
            }
        }

        // 2. FastAPI / Flask / Python
        if line_trimmed.starts_with('@')
            && (line_trimmed.contains(&format!(".{method_lower}("))
                || line_trimmed.contains(&format!(".{method_upper}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            let mut found_line = false;
            for l in source.lines() {
                if !found_line {
                    if l.trim() == line_trimmed {
                        found_line = true;
                    }
                } else {
                    let l_trim = l.trim();
                    if l_trim.starts_with("def ") || l_trim.starts_with("async def ") {
                        let after_def = l_trim.split("def ").nth(1)?;
                        let fn_name = after_def.split('(').next()?.trim();
                        return Some(fn_name.to_string());
                    }
                }
            }
        }

        // 3. Gin / Go
        if (line_trimmed.contains(&format!(".{method_upper}("))
            || line_trimmed.contains(&format!(".{method_lower}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            if let Some(before_paren) = line_trimmed.rsplit_once(')').map(|x| x.0) {
                if let Some(last_arg) = before_paren.rsplit(',').next().map(str::trim) {
                    if let Some(ident) = last_arg
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .find(|s| !s.is_empty())
                    {
                        return Some(ident.to_string());
                    }
                }
            }
        }

        // 4. Axum / Actix (Rust)
        if line_trimmed.contains("route(")
            && (line_trimmed.contains(&format!("{method_lower}("))
                || line_trimmed.contains(&format!("{method_upper}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            let marker = format!("{method_lower}(");
            if let Some(after) = line_trimmed.split(&marker).nth(1) {
                if let Some(inside) = after.split(')').next().map(str::trim) {
                    if let Some(ident) = inside
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .find(|s| !s.is_empty())
                    {
                        return Some(ident.to_string());
                    }
                }
            }
        }
    }

    None
}
