//! tRPC framework analyzer for router procedures and RPC contracts.
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

/// tRPC framework analyzer for extracting backend router procedures and RPC contracts.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrpcAnalyzer;

impl TrpcAnalyzer {
    /// Creates a new `TrpcAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all tRPC router procedure endpoints from a TypeScript/JavaScript source file.
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
        let pair_nodes = AstUtils::find_descendants_by_kind(root, "pair");

        for pair in pair_nodes {
            if let Some(endpoint) = self.inspect_trpc_pair(pair, source, &file_path) {
                routes.push(endpoint);
            }
        }

        // Fallback line scanner for tRPC procedures
        scan_trpc_fallback(source, &file_path, &mut routes);

        // Deduplicate routes by (http_method, route_path, handler_symbol)
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

    fn inspect_trpc_pair(
        &self,
        node: Node<'_>,
        source: &str,
        file_path: &str,
    ) -> Option<ServerRouteEndpoint> {
        let key_node = node.child_by_field_name("key")?;
        let val_node = node.child_by_field_name("value")?;

        let key_text = AstUtils::node_text(key_node, source).trim().trim_matches(['\'', '"', '`']);
        if key_text.is_empty() {
            return None;
        }

        let val_text = AstUtils::node_text(val_node, source).trim();

        let is_query = val_text.contains(".query(") || val_text.ends_with(".query");
        let is_mutation = val_text.contains(".mutation(") || val_text.ends_with(".mutation");
        let is_sub = val_text.contains(".subscription(") || val_text.ends_with(".subscription");

        if !is_query && !is_mutation && !is_sub {
            return None;
        }

        let method = if is_mutation {
            "MUTATION".to_string()
        } else if is_sub {
            "SUBSCRIPTION".to_string()
        } else {
            "QUERY".to_string()
        };

        let route_path = format!("/trpc/{key_text}");
        let sig = format!("{key_text}: procedure.{}(...)", method.to_lowercase());

        let req_dto = extract_trpc_input_schema(val_text, source, file_path);

        Some(ServerRouteEndpoint {
            framework: "trpc".to_string(),
            http_method: method,
            route_path,
            handler_file: file_path.to_string(),
            handler_symbol: key_text.to_string(),
            handler_signature: sig,
            request_dto_type: req_dto,
            response_dto_type: None,
        })
    }
}

impl FrameworkAnalyzer for TrpcAnalyzer {
    fn name(&self) -> &'static str {
        "trpc"
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

        source.contains("@trpc")
            || source.contains("createTRPCRouter")
            || source.contains("publicProcedure")
            || source.contains("protectedProcedure")
            || (source.contains("router({") && (source.contains(".query(") || source.contains(".mutation(")))
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

fn extract_trpc_input_schema(val_text: &str, source: &str, file_path: &str) -> Option<ExtractedType> {
    if let Some(pos) = val_text.find(".input(") {
        let after = &val_text[pos + 7..];
        let inside = after.split(')').next()?.trim();
        let schema_name = inside.split(['(', '<', '{', ' ']).next()?.trim();
        if !schema_name.is_empty() && schema_name != "z" && schema_name != "z.object" {
            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with(&format!("export const {schema_name}"))
                    || trimmed.starts_with(&format!("const {schema_name}"))
                    || trimmed.starts_with(&format!("export type {schema_name}"))
                    || trimmed.starts_with(&format!("type {schema_name}"))
                    || trimmed.starts_with(&format!("export interface {schema_name}"))
                    || trimmed.starts_with(&format!("interface {schema_name}"))
                {
                    return Some(ExtractedType {
                        name: schema_name.to_string(),
                        kind: "schema".to_string(),
                        file_path: file_path.to_string(),
                        definition: trimmed.to_string(),
                    });
                }
            }
        }
    }
    None
}

fn scan_trpc_fallback(
    source: &str,
    file_path: &str,
    routes: &mut Vec<ServerRouteEndpoint>,
) {
    for line in source.lines() {
        let trimmed = line.trim();
        if (trimmed.contains(".query(") || trimmed.contains(".mutation(")) && trimmed.contains(':') {
            let parts: Vec<&str> = trimmed.split(':').collect();
            if parts.len() >= 2 {
                let proc_name = parts[0].trim().trim_matches(['\'', '"', '`']);
                if !proc_name.is_empty() && !proc_name.contains(' ') && !proc_name.starts_with("//") {
                    let is_mutation = trimmed.contains(".mutation(");
                    let method = if is_mutation { "MUTATION" } else { "QUERY" };
                    routes.push(ServerRouteEndpoint {
                        framework: "trpc".to_string(),
                        http_method: method.to_string(),
                        route_path: format!("/trpc/{proc_name}"),
                        handler_file: file_path.to_string(),
                        handler_symbol: proc_name.to_string(),
                        handler_signature: trimmed.to_string(),
                        request_dto_type: None,
                        response_dto_type: None,
                    });
                }
            }
        }
    }
}
