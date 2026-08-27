//! Next.js Server Actions framework analyzer.
//!
#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_self,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

use crate::error::Result;
use crate::framework::FrameworkAnalyzer;
use crate::fullstack::model::ServerRouteEndpoint;
use crate::model::{ExtractedType, SliceResult};
use crate::parser::AstUtils;
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Next.js Server Action analyzer for extracting module-level and function-level `'use server'` actions.
#[derive(Debug, Default, Clone, Copy)]
pub struct NextServerActionAnalyzer;

impl NextServerActionAnalyzer {
    /// Creates a new `NextServerActionAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all Next.js Server Action endpoints from a TypeScript/JavaScript source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(
            ext.as_str(),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs"
        ) {
            return routes;
        }

        let is_module_server = is_module_level_use_server(source);

        let mut parser = Parser::new();
        let lang = tree_sitter_typescript::LANGUAGE_TSX.into();
        if parser.set_language(&lang).is_err() {
            return routes;
        }

        let tree = match parser.parse(source.as_bytes(), None) {
            Some(t) => t,
            None => return routes,
        };

        let root = tree.root_node();

        if is_module_server {
            // All exported async functions in module-level 'use server' file are Server Actions
            self.extract_exported_actions(root, source, &file_path, &mut routes);
        } else {
            // Function-level 'use server'
            self.extract_inline_actions(root, source, &file_path, &mut routes);
        }

        // Fallback line scanner
        scan_actions_fallback(source, &file_path, is_module_server, &mut routes);

        // Deduplicate routes by handler_symbol
        let mut unique = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for r in routes {
            let key = (r.framework.clone(), r.http_method.clone(), r.handler_symbol.clone());
            if seen.insert(key) {
                unique.push(r);
            }
        }

        unique
    }

    fn extract_exported_actions(
        &self,
        root: Node<'_>,
        source: &str,
        file_path: &str,
        routes: &mut Vec<ServerRouteEndpoint>,
    ) {
        let export_nodes = AstUtils::find_descendants_by_kind(root, "export_statement");
        for export_stmt in export_nodes {
            let stmt_text = AstUtils::node_text(export_stmt, source).trim();
            if stmt_text.contains("function") || stmt_text.contains("=>") {
                if let Some(fn_decl) = export_stmt.child_by_field_name("declaration") {
                    if let Some(fn_name_node) = fn_decl.child_by_field_name("name") {
                        let fn_name = AstUtils::node_text(fn_name_node, source).trim().to_string();
                        if !fn_name.is_empty() {
                            let sig = AstUtils::extract_signature_header(fn_decl, source);
                            let (req_dto, res_dto) = extract_action_dtos(fn_decl, source, file_path);
                            routes.push(ServerRouteEndpoint {
                                framework: "nextjs_server_action".to_string(),
                                http_method: "ACTION".to_string(),
                                route_path: format!("action://{fn_name}"),
                                handler_file: file_path.to_string(),
                                handler_symbol: fn_name,
                                handler_signature: sig,
                                request_dto_type: req_dto,
                                response_dto_type: res_dto,
                            });
                        }
                    }
                }
            }
        }
    }

    fn extract_inline_actions(
        &self,
        root: Node<'_>,
        source: &str,
        file_path: &str,
        routes: &mut Vec<ServerRouteEndpoint>,
    ) {
        let fn_nodes = AstUtils::find_descendants_by_kind(root, "function_declaration");
        for fn_node in fn_nodes {
            let body_node = fn_node.child_by_field_name("body");
            if let Some(body) = body_node {
                let body_text = AstUtils::node_text(body, source).trim();
                if body_text.starts_with("{ 'use server'")
                    || body_text.starts_with("{\n  'use server'")
                    || body_text.starts_with("{\n    'use server'")
                    || body_text.starts_with("{\"use server\"")
                    || body_text.starts_with("{\n  \"use server\"")
                    || body_text.starts_with("{\n    \"use server\"")
                    || body_text.contains("'use server'")
                    || body_text.contains("\"use server\"")
                {
                    if let Some(name_node) = fn_node.child_by_field_name("name") {
                        let fn_name = AstUtils::node_text(name_node, source).trim().to_string();
                        if !fn_name.is_empty() {
                            let sig = AstUtils::extract_signature_header(fn_node, source);
                            let (req_dto, res_dto) = extract_action_dtos(fn_node, source, file_path);
                            routes.push(ServerRouteEndpoint {
                                framework: "nextjs_server_action".to_string(),
                                http_method: "ACTION".to_string(),
                                route_path: format!("action://{fn_name}"),
                                handler_file: file_path.to_string(),
                                handler_symbol: fn_name,
                                handler_signature: sig,
                                request_dto_type: req_dto,
                                response_dto_type: res_dto,
                            });
                        }
                    }
                }
            }
        }
    }
}

impl FrameworkAnalyzer for NextServerActionAnalyzer {
    fn name(&self) -> &'static str {
        "nextjs_server_action"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(ext.as_str(), "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs") {
            return false;
        }

        source.contains("'use server'") || source.contains("\"use server\"")
    }

    fn enhance_slice(
        &self,
        _target_node: Node<'_>,
        _source: &str,
        _path: &Path,
        _slice: &mut SliceResult,
    ) -> Result<()> {
        Ok(())
    }
}

fn is_module_level_use_server(source: &str) -> bool {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }
        return trimmed == "'use server';"
            || trimmed == "'use server'"
            || trimmed == "\"use server\";"
            || trimmed == "\"use server\"";
    }
    false
}

fn extract_action_dtos(
    fn_node: Node<'_>,
    source: &str,
    file_path: &str,
) -> (Option<ExtractedType>, Option<ExtractedType>) {
    let mut req_dto = None;
    let mut res_dto = None;

    if let Some(params_node) = fn_node.child_by_field_name("parameters") {
        for param in params_node.named_children(&mut params_node.walk()) {
            if let Some(type_node) = param.child_by_field_name("type") {
                let type_text = AstUtils::node_text(type_node, source).trim().trim_start_matches(':').trim();
                if !type_text.is_empty() && type_text != "FormData" && !is_ts_primitive(type_text) {
                    if req_dto.is_none() {
                        req_dto = find_ts_type(source, type_text, file_path);
                    }
                }
            }
        }
    }

    if let Some(ret_node) = fn_node.child_by_field_name("return_type") {
        let ret_text = AstUtils::node_text(ret_node, source).trim().trim_start_matches(':').trim();
        let unwrapped = unwrap_promise_type(ret_text);
        if !unwrapped.is_empty() && !is_ts_primitive(&unwrapped) {
            res_dto = find_ts_type(source, &unwrapped, file_path);
        }
    }

    (req_dto, res_dto)
}

fn unwrap_promise_type(s: &str) -> String {
    let t = s.trim();
    if t.starts_with("Promise<") {
        if let Some(start) = t.find('<') {
            if let Some(end) = t.rfind('>') {
                return t[start + 1..end].trim().to_string();
            }
        }
    }
    t.to_string()
}

fn is_ts_primitive(t: &str) -> bool {
    matches!(
        t,
        "string" | "number" | "boolean" | "void" | "null" | "undefined" | "any" | "unknown" | "never"
            | "FormData" | "Request" | "Response" | "Record<string, any>"
    )
}

fn find_ts_type(source: &str, type_name: &str, file_path: &str) -> Option<ExtractedType> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("export interface {type_name}"))
            || trimmed.starts_with(&format!("interface {type_name}"))
            || trimmed.starts_with(&format!("export type {type_name}"))
            || trimmed.starts_with(&format!("type {type_name}"))
        {
            return Some(ExtractedType {
                name: type_name.to_string(),
                kind: if trimmed.contains("interface") { "interface".to_string() } else { "type".to_string() },
                file_path: file_path.to_string(),
                definition: trimmed.to_string(),
            });
        }
    }
    None
}

fn scan_actions_fallback(
    source: &str,
    file_path: &str,
    is_module_server: bool,
    routes: &mut Vec<ServerRouteEndpoint>,
) {
    if !source.contains("'use server'") && !source.contains("\"use server\"") {
        return;
    }

    for line in source.lines() {
        let trimmed = line.trim();
        if is_module_server {
            if (trimmed.starts_with("export async function ") || trimmed.starts_with("export function "))
                && trimmed.contains('(')
            {
                let after = if let Some(a) = trimmed.strip_prefix("export async function ") {
                    a
                } else {
                    trimmed.strip_prefix("export function ").unwrap_or("")
                };
                let fn_name = after.split(['(', '<', ' ']).next().unwrap_or("").trim();
                if !fn_name.is_empty() {
                    routes.push(ServerRouteEndpoint {
                        framework: "nextjs_server_action".to_string(),
                        http_method: "ACTION".to_string(),
                        route_path: format!("action://{fn_name}"),
                        handler_file: file_path.to_string(),
                        handler_symbol: fn_name.to_string(),
                        handler_signature: trimmed.to_string(),
                        request_dto_type: None,
                        response_dto_type: None,
                    });
                }
            }
        }
    }
}
