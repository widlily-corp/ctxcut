//! Axum and Actix-web framework analyzer.
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

/// Axum and Actix-web framework analyzer for Rust.
#[derive(Debug, Default, Clone, Copy)]
pub struct AxumActixAnalyzer;

impl AxumActixAnalyzer {
    /// Creates a new `AxumActixAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all server route endpoints from a Rust source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        let mut parser = Parser::new();
        let lang = tree_sitter_rust::LANGUAGE.into();
        if parser.set_language(&lang).is_err() {
            return routes;
        }

        let tree = match parser.parse(source.as_bytes(), None) {
            Some(t) => t,
            None => return routes,
        };

        let root = tree.root_node();

        // 1. Actix-web attribute macros: #[get("/api/users")], #[post("/api/users")]
        self.extract_actix_macro_routes(root, source, &file_path, &mut routes);

        // 2. Axum route chaining: Router::new().route("/api/users", get(handler).post(handler2))
        self.extract_axum_chained_routes(root, source, &file_path, &mut routes);

        // Fallback line scanner for any subtle variations
        self.scan_rust_route_fallback(source, &file_path, &mut routes);

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

    fn extract_actix_macro_routes(
        &self,
        root: Node<'_>,
        source: &str,
        file_path: &str,
        routes: &mut Vec<ServerRouteEndpoint>,
    ) {
        let fn_nodes = AstUtils::find_descendants_by_kind(root, "function_item");
        for fn_node in fn_nodes {
            let fn_name_node = fn_node.child_by_field_name("name");
            let fn_name = fn_name_node
                .map(|n| AstUtils::node_text(n, source).to_string())
                .unwrap_or_default();
            if fn_name.is_empty() {
                continue;
            }

            let fn_signature = AstUtils::extract_signature_header(fn_node, source);

            // Look for preceding attributes / outer attributes
            let mut prev = fn_node.prev_named_sibling();
            while let Some(sibling) = prev {
                if sibling.kind() == "attribute_item" {
                    let attr_text = AstUtils::node_text(sibling, source).trim();
                    if let Some((method, path)) = parse_actix_attribute(attr_text) {
                        let (req_dto, res_dto) = extract_rust_dtos(fn_node, source, file_path);
                        routes.push(ServerRouteEndpoint {
                            framework: "actix".to_string(),
                            http_method: method.to_uppercase(),
                            route_path: path,
                            handler_file: file_path.to_string(),
                            handler_symbol: fn_name.clone(),
                            handler_signature: fn_signature.clone(),
                            request_dto_type: req_dto,
                            response_dto_type: res_dto,
                        });
                    }
                } else {
                    break;
                }
                prev = sibling.prev_named_sibling();
            }
        }
    }

    fn extract_axum_chained_routes(
        &self,
        root: Node<'_>,
        source: &str,
        file_path: &str,
        routes: &mut Vec<ServerRouteEndpoint>,
    ) {
        let call_nodes = AstUtils::find_descendants_by_kind(root, "call_expression");
        for call in call_nodes {
            let call_text = AstUtils::node_text(call, source);
            if call_text.contains(".route(") {
                // Parse .route("/path", get(handler).post(handler2))
                if let Some(pos) = call_text.find(".route(") {
                    let inside = &call_text[pos + 7..];
                    if let Some((path_part, handlers_part)) = split_first_arg(inside) {
                        let clean_path = path_part.trim_matches(['"', '\'', ' ']);
                        for (method, handler_name) in parse_axum_handlers(handlers_part) {
                            let signature = find_fn_signature_in_source(source, &handler_name)
                                .unwrap_or_else(|| format!("async fn {handler_name}(...)"));
                            let (req_dto, res_dto) = find_dtos_for_handler(source, &handler_name, file_path);
                            routes.push(ServerRouteEndpoint {
                                framework: "axum".to_string(),
                                http_method: method.to_uppercase(),
                                route_path: clean_path.to_string(),
                                handler_file: file_path.to_string(),
                                handler_symbol: handler_name,
                                handler_signature: signature,
                                request_dto_type: req_dto,
                                response_dto_type: res_dto,
                            });
                        }
                    }
                }
            }
        }
    }

    fn scan_rust_route_fallback(
        &self,
        source: &str,
        file_path: &str,
        routes: &mut Vec<ServerRouteEndpoint>,
    ) {
        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Actix: #[get("/api/...")]
            for method in &["get", "post", "put", "delete", "patch"] {
                let pattern = format!("#[{method}(\"");
                if trimmed.contains(&pattern) {
                    if let Some(path) = extract_quote_inside(trimmed, &pattern) {
                        // next line usually contains async fn handler_name
                        let next_line = lines.get(i + 1).unwrap_or(&"");
                        let fn_name = extract_fn_name(next_line).unwrap_or_else(|| "handler".to_string());
                        let (req_dto, res_dto) = find_dtos_for_handler(source, &fn_name, file_path);
                        routes.push(ServerRouteEndpoint {
                            framework: "actix".to_string(),
                            http_method: method.to_uppercase(),
                            route_path: path,
                            handler_file: file_path.to_string(),
                            handler_symbol: fn_name.clone(),
                            handler_signature: next_line.trim().to_string(),
                            request_dto_type: req_dto,
                            response_dto_type: res_dto,
                        });
                    }
                }
            }

            // Axum: .route("/api/...", get(handler))
            if trimmed.contains(".route(") && trimmed.contains('"') {
                if let Some(start_q) = trimmed.find('"') {
                    if let Some(end_q) = trimmed[start_q + 1..].find('"') {
                        let path = &trimmed[start_q + 1..start_q + 1 + end_q];
                        let after = &trimmed[start_q + 1 + end_q + 1..];
                        for m in &["get", "post", "put", "delete", "patch"] {
                            let m_pat = format!("{m}(");
                            if after.contains(&m_pat) {
                                if let Some(m_pos) = after.find(&m_pat) {
                                    let handler_part = &after[m_pos + m_pat.len()..];
                                    let handler_name = handler_part
                                        .split(['(', ')', ',', ' ', ';'])
                                        .next()
                                        .unwrap_or("handler")
                                        .trim();
                                    if !handler_name.is_empty() {
                                        let sig = find_fn_signature_in_source(source, handler_name)
                                            .unwrap_or_else(|| format!("async fn {handler_name}(...)"));
                                        let (req_dto, res_dto) = find_dtos_for_handler(source, handler_name, file_path);
                                        routes.push(ServerRouteEndpoint {
                                            framework: "axum".to_string(),
                                            http_method: m.to_uppercase(),
                                            route_path: path.to_string(),
                                            handler_file: file_path.to_string(),
                                            handler_symbol: handler_name.to_string(),
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
}

impl FrameworkAnalyzer for AxumActixAnalyzer {
    fn name(&self) -> &'static str {
        "axum_actix"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let is_rust = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "rs")
            .unwrap_or(false);

        if !is_rust {
            return false;
        }

        source.contains("axum")
            || source.contains("actix_web")
            || source.contains("#[get(")
            || source.contains("#[post(")
            || source.contains(".route(")
            || source.contains("Router::new")
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

fn parse_actix_attribute(attr: &str) -> Option<(String, String)> {
    for method in &["get", "post", "put", "delete", "patch", "head"] {
        let pat = format!("{method}(\"");
        if attr.contains(&pat) {
            if let Some(pos) = attr.find(&pat) {
                let after = &attr[pos + pat.len()..];
                if let Some(end) = after.find('"') {
                    let path = &after[..end];
                    return Some(((*method).to_string(), path.to_string()));
                }
            }
        }
    }
    None
}

fn split_first_arg(s: &str) -> Option<(&str, &str)> {
    let comma = s.find(',')?;
    Some((s[..comma].trim(), s[comma + 1..].trim()))
}

fn parse_axum_handlers(s: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for m in &["get", "post", "put", "delete", "patch"] {
        let pat = format!("{m}(");
        if let Some(pos) = s.find(&pat) {
            let after = &s[pos + pat.len()..];
            let name = after.split(['(', ')', ',', ' ']).next().unwrap_or("").trim();
            if !name.is_empty() {
                result.push(((*m).to_string(), name.to_string()));
            }
        }
    }
    result
}

fn extract_quote_inside(s: &str, prefix: &str) -> Option<String> {
    let pos = s.find(prefix)?;
    let after = &s[pos + prefix.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

fn extract_fn_name(line: &str) -> Option<String> {
    if let Some(pos) = line.find("fn ") {
        let after = &line[pos + 3..];
        let name = after.split(['(', '<', ' ']).next()?.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

fn find_fn_signature_in_source(source: &str, fn_name: &str) -> Option<String> {
    for line in source.lines() {
        if line.contains(&format!("fn {fn_name}")) {
            return Some(line.trim().to_string());
        }
    }
    None
}

fn extract_rust_dtos(
    fn_node: Node<'_>,
    source: &str,
    file_path: &str,
) -> (Option<ExtractedType>, Option<ExtractedType>) {
    let text = AstUtils::node_text(fn_node, source);
    let mut req_dto = None;
    let res_dto = None;

    // Search for Json<T>, web::Json<T>, Path<T>, Query<T>
    for tag in &["Json<", "web::Json<", "Query<", "web::Query<", "Path<", "web::Path<"] {
        if let Some(pos) = text.find(tag) {
            let after = &text[pos + tag.len()..];
            if let Some(end) = after.find('>') {
                let type_name = after[..end].trim().to_string();
                if !type_name.is_empty() && req_dto.is_none() {
                    req_dto = find_type_in_source(source, &type_name, file_path);
                }
            }
        }
    }

    (req_dto, res_dto)
}

fn find_dtos_for_handler(
    source: &str,
    handler_name: &str,
    file_path: &str,
) -> (Option<ExtractedType>, Option<ExtractedType>) {
    let mut req_dto = None;
    let res_dto = None;

    for line in source.lines() {
        if line.contains(handler_name) && (line.contains("Json<") || line.contains("web::Json<")) {
            for tag in &["Json<", "web::Json<"] {
                if let Some(pos) = line.find(tag) {
                    let after = &line[pos + tag.len()..];
                    if let Some(end) = after.find('>') {
                        let name = after[..end].trim();
                        if !name.is_empty() {
                            req_dto = find_type_in_source(source, name, file_path);
                        }
                    }
                }
            }
        }
    }

    (req_dto, res_dto)
}

fn find_type_in_source(source: &str, name: &str, file_path: &str) -> Option<ExtractedType> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("pub struct {name}"))
            || trimmed.starts_with(&format!("struct {name}"))
            || trimmed.starts_with(&format!("pub enum {name}"))
            || trimmed.starts_with(&format!("enum {name}"))
        {
            return Some(ExtractedType {
                name: name.to_string(),
                kind: "struct".to_string(),
                file_path: file_path.to_string(),
                definition: trimmed.to_string(),
            });
        }
    }
    None
}
