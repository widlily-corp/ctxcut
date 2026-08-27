//! Gin and Chi Go framework analyzer.
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

/// Gin and Chi web framework analyzer for Go.
#[derive(Debug, Default, Clone, Copy)]
pub struct GinChiAnalyzer;

impl GinChiAnalyzer {
    /// Creates a new `GinChiAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all server route endpoints from a Go source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        let mut parser = Parser::new();
        let lang = tree_sitter_go::LANGUAGE.into();
        if parser.set_language(&lang).is_err() {
            return routes;
        }

        let tree = match parser.parse(source.as_bytes(), None) {
            Some(t) => t,
            None => return routes,
        };

        let root = tree.root_node();
        let call_nodes = AstUtils::find_descendants_by_kind(root, "call_expression");

        for call in call_nodes {
            if let Some(fn_node) = call.child_by_field_name("function") {
                let fn_text = AstUtils::node_text(fn_node, source).trim();
                let is_gin = fn_text.contains(".GET")
                    || fn_text.contains(".POST")
                    || fn_text.contains(".PUT")
                    || fn_text.contains(".DELETE")
                    || fn_text.contains(".PATCH")
                    || fn_text.contains(".OPTIONS");
                let is_chi = fn_text.contains(".Get")
                    || fn_text.contains(".Post")
                    || fn_text.contains(".Put")
                    || fn_text.contains(".Delete")
                    || fn_text.contains(".Patch")
                    || fn_text.contains(".Route");

                if is_gin || is_chi {
                    let framework = if is_gin { "gin" } else { "chi" };
                    let method = extract_go_http_method(fn_text);

                    if let Some(args) = call.child_by_field_name("arguments") {
                        let arg_nodes: Vec<Node<'_>> = args.named_children(&mut args.walk()).collect();
                        if arg_nodes.len() >= 2 {
                            let path_node = arg_nodes[0];
                            let handler_node = arg_nodes[arg_nodes.len() - 1];

                            let raw_path = AstUtils::node_text(path_node, source).trim();
                            let clean_path = raw_path.trim_matches(['"', '`']).to_string();
                            let handler_name = AstUtils::node_text(handler_node, source).trim().to_string();

                            if clean_path.starts_with('/') && !handler_name.is_empty() {
                                let sig = find_go_fn_signature(source, &handler_name)
                                    .unwrap_or_else(|| format!("func {handler_name}(...)"));
                                let (req_dto, res_dto) = find_go_dtos(source, &handler_name, &file_path);

                                routes.push(ServerRouteEndpoint {
                                    framework: framework.to_string(),
                                    http_method: method,
                                    route_path: clean_path,
                                    handler_file: file_path.clone(),
                                    handler_symbol: handler_name,
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

        // Fallback text scanner
        self.scan_go_route_fallback(source, &file_path, &mut routes);

        // Deduplicate routes
        let mut unique = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for r in routes {
            let key = (r.http_method.clone(), r.route_path.clone(), r.handler_symbol.clone());
            if seen.insert(key) {
                unique.push(r);
            }
        }

        unique
    }

    fn scan_go_route_fallback(
        &self,
        source: &str,
        file_path: &str,
        routes: &mut Vec<ServerRouteEndpoint>,
    ) {
        for line in source.lines() {
            let trimmed = line.trim();
            for (framework, methods) in [
                ("gin", &["GET", "POST", "PUT", "DELETE", "PATCH"][..]),
                ("chi", &["Get", "Post", "Put", "Delete", "Patch"][..]),
            ] {
                for method in methods {
                    let pat = format!(".{method}(\"");
                    if trimmed.contains(&pat) {
                        if let Some(pos) = trimmed.find(&pat) {
                            let after = &trimmed[pos + pat.len()..];
                            if let Some(end_q) = after.find('"') {
                                let path = &after[..end_q];
                                let rest = &after[end_q + 1..];
                                let handler = rest
                                    .trim_matches([',', ' ', ')', ';'])
                                    .split(['(', ',', ' '])
                                    .next()
                                    .unwrap_or("")
                                    .trim();
                                if !handler.is_empty() {
                                    let sig = find_go_fn_signature(source, handler)
                                        .unwrap_or_else(|| format!("func {handler}(...)"));
                                    let (req_dto, res_dto) = find_go_dtos(source, handler, file_path);
                                    routes.push(ServerRouteEndpoint {
                                        framework: framework.to_string(),
                                        http_method: method.to_uppercase(),
                                        route_path: path.to_string(),
                                        handler_file: file_path.to_string(),
                                        handler_symbol: handler.to_string(),
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
    }
}

impl FrameworkAnalyzer for GinChiAnalyzer {
    fn name(&self) -> &'static str {
        "gin_chi"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let is_go = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "go")
            .unwrap_or(false);

        if !is_go {
            return false;
        }

        source.contains("gin-gonic/gin")
            || source.contains("go-chi/chi")
            || source.contains("gin.Context")
            || source.contains(".GET(")
            || source.contains(".POST(")
            || source.contains(".Get(")
            || source.contains(".Post(")
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

fn extract_go_http_method(fn_text: &str) -> String {
    let upper = fn_text.to_uppercase();
    if upper.contains(".GET") {
        "GET".to_string()
    } else if upper.contains(".POST") {
        "POST".to_string()
    } else if upper.contains(".PUT") {
        "PUT".to_string()
    } else if upper.contains(".DELETE") {
        "DELETE".to_string()
    } else if upper.contains(".PATCH") {
        "PATCH".to_string()
    } else {
        "GET".to_string()
    }
}

fn find_go_fn_signature(source: &str, fn_name: &str) -> Option<String> {
    for line in source.lines() {
        if line.contains(&format!("func {fn_name}")) || (line.contains("func (") && line.contains(fn_name)) {
            return Some(line.trim().to_string());
        }
    }
    None
}

fn find_go_dtos(
    source: &str,
    handler_name: &str,
    file_path: &str,
) -> (Option<ExtractedType>, Option<ExtractedType>) {
    let mut req_dto = None;
    let res_dto = None;

    let mut in_handler = false;
    for line in source.lines() {
        if line.contains(&format!("func {handler_name}")) {
            in_handler = true;
        }
        if in_handler {
            if line.contains("ShouldBindJSON(&") || line.contains("BindJSON(&") || line.contains("Decode(&") {
                if let Some(pos) = line.find("(&") {
                    let after = &line[pos + 2..];
                    let var_name = after.split(')').next().unwrap_or("").trim();
                    if !var_name.is_empty() {
                        req_dto = find_go_struct_in_source(source, var_name, file_path);
                    }
                }
            }
            if line.starts_with('}') && in_handler {
                break;
            }
        }
    }

    (req_dto, res_dto)
}

fn find_go_struct_in_source(source: &str, var_or_type_name: &str, file_path: &str) -> Option<ExtractedType> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("type {var_or_type_name} struct")) {
            return Some(ExtractedType {
                name: var_or_type_name.to_string(),
                kind: "struct".to_string(),
                file_path: file_path.to_string(),
                definition: trimmed.to_string(),
            });
        }
    }
    None
}
