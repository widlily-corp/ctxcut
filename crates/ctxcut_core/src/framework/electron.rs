//! Electron framework analyzer for Main Process IPC channel handlers.
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
use crate::model::SliceResult;
use crate::parser::AstUtils;
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Electron framework analyzer for extracting `ipcMain.handle` and `ipcMain.on` IPC channels.
#[derive(Debug, Default, Clone, Copy)]
pub struct ElectronAnalyzer;

impl ElectronAnalyzer {
    /// Creates a new `ElectronAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all Electron IPC channels from a TypeScript/JavaScript source file.
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
        let call_nodes = AstUtils::find_descendants_by_kind(root, "call_expression");

        for call in call_nodes {
            if let Some(endpoint) = self.inspect_electron_call(call, source, &file_path) {
                routes.push(endpoint);
            }
        }

        // Fallback line scanner for any missed patterns
        scan_electron_fallback(source, &file_path, &mut routes);

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

    fn inspect_electron_call(
        &self,
        node: Node<'_>,
        source: &str,
        file_path: &str,
    ) -> Option<ServerRouteEndpoint> {
        let func_node = node.child_by_field_name("function")?;
        let func_text = AstUtils::node_text(func_node, source).trim();

        let (is_handle, is_on) = if func_text == "ipcMain.handle"
            || func_text.ends_with(".ipcMain.handle")
            || func_text == "ipcMain.handleOnce"
            || func_text.ends_with(".ipcMain.handleOnce")
        {
            (true, false)
        } else if func_text == "ipcMain.on"
            || func_text.ends_with(".ipcMain.on")
            || func_text == "ipcMain.once"
            || func_text.ends_with(".ipcMain.once")
        {
            (false, true)
        } else {
            return None;
        };

        let args_node = node.child_by_field_name("arguments")?;
        let first_arg = args_node.named_child(0)?;
        let channel_name = extract_channel_literal(first_arg, source)?;

        let mut handler_symbol = channel_name.clone();
        if let Some(second_arg) = args_node.named_child(1) {
            let arg_kind = second_arg.kind();
            if arg_kind == "identifier" {
                handler_symbol = AstUtils::node_text(second_arg, source).trim().to_string();
            } else if arg_kind == "function_declaration" || arg_kind == "function" {
                if let Some(name_node) = second_arg.child_by_field_name("name") {
                    let named = AstUtils::node_text(name_node, source).trim().to_string();
                    if !named.is_empty() {
                        handler_symbol = named;
                    }
                }
            }
        }

        let http_method = if is_handle {
            "IPC_HANDLE".to_string()
        } else if is_on {
            "IPC_ON".to_string()
        } else {
            "IPC".to_string()
        };

        let sig = format!("ipcMain.{}({:?}, {})", if is_handle { "handle" } else { "on" }, channel_name, handler_symbol);

        Some(ServerRouteEndpoint {
            framework: "electron".to_string(),
            http_method,
            route_path: channel_name,
            handler_file: file_path.to_string(),
            handler_symbol,
            handler_signature: sig,
            request_dto_type: None,
            response_dto_type: None,
        })
    }
}

impl FrameworkAnalyzer for ElectronAnalyzer {
    fn name(&self) -> &'static str {
        "electron"
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

        source.contains("ipcMain")
            || (source.contains("electron") && (source.contains(".handle(") || source.contains(".on(")))
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

fn extract_channel_literal(node: Node<'_>, source: &str) -> Option<String> {
    let text = AstUtils::node_text(node, source).trim();
    if text.starts_with('\'') || text.starts_with('"') || text.starts_with('`') {
        return Some(text.trim_matches(['\'', '"', '`']).to_string());
    }
    if node.kind() == "identifier" || node.kind() == "member_expression" {
        return Some(text.to_string());
    }
    None
}

fn scan_electron_fallback(
    source: &str,
    file_path: &str,
    routes: &mut Vec<ServerRouteEndpoint>,
) {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.contains("ipcMain.handle(") || trimmed.contains("ipcMain.handleOnce(") {
            if let Some(channel) = extract_string_in_parens(trimmed, "ipcMain.handle") {
                let sig = trimmed.to_string();
                routes.push(ServerRouteEndpoint {
                    framework: "electron".to_string(),
                    http_method: "IPC_HANDLE".to_string(),
                    route_path: channel.clone(),
                    handler_file: file_path.to_string(),
                    handler_symbol: channel,
                    handler_signature: sig,
                    request_dto_type: None,
                    response_dto_type: None,
                });
            }
        } else if trimmed.contains("ipcMain.on(") || trimmed.contains("ipcMain.once(") {
            if let Some(channel) = extract_string_in_parens(trimmed, "ipcMain.on") {
                let sig = trimmed.to_string();
                routes.push(ServerRouteEndpoint {
                    framework: "electron".to_string(),
                    http_method: "IPC_ON".to_string(),
                    route_path: channel.clone(),
                    handler_file: file_path.to_string(),
                    handler_symbol: channel,
                    handler_signature: sig,
                    request_dto_type: None,
                    response_dto_type: None,
                });
            }
        }
    }
}

fn extract_string_in_parens(line: &str, prefix: &str) -> Option<String> {
    let pos = line.find(prefix)?;
    let after = &line[pos + prefix.len()..];
    let open_paren = after.find('(')?;
    let inside = &after[open_paren + 1..];
    for quote in ['\'', '"', '`'] {
        if let Some(start) = inside.find(quote) {
            if let Some(end) = inside[start + 1..].find(quote) {
                return Some(inside[start + 1..start + 1 + end].to_string());
            }
        }
    }
    None
}
