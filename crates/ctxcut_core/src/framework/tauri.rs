//! Tauri framework analyzer for Rust backend commands and IPC resolution.
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

/// Tauri framework analyzer for extracting Rust `#[tauri::command]` functions and DTO contracts.
#[derive(Debug, Default, Clone, Copy)]
pub struct TauriAnalyzer;

impl TauriAnalyzer {
    /// Creates a new `TauriAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all Tauri IPC command endpoints from a Rust source file.
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
        let fn_nodes = AstUtils::find_descendants_by_kind(root, "function_item");

        for fn_node in fn_nodes {
            if is_tauri_command_function(fn_node, source) {
                let fn_name_node = fn_node.child_by_field_name("name");
                let fn_name = fn_name_node
                    .map(|n| AstUtils::node_text(n, source).to_string())
                    .unwrap_or_default();
                if fn_name.is_empty() {
                    continue;
                }

                let fn_signature = AstUtils::extract_signature_header(fn_node, source);
                let (req_dto, res_dto) = extract_tauri_dtos(fn_node, source, &file_path);

                routes.push(ServerRouteEndpoint {
                    framework: "tauri".to_string(),
                    http_method: "IPC".to_string(),
                    route_path: fn_name.clone(),
                    handler_file: file_path.clone(),
                    handler_symbol: fn_name,
                    handler_signature: fn_signature,
                    request_dto_type: req_dto,
                    response_dto_type: res_dto,
                });
            }
        }

        // Fallback line scanner for any subtle syntax variations
        scan_tauri_fallback(source, &file_path, &mut routes);

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
}

impl FrameworkAnalyzer for TauriAnalyzer {
    fn name(&self) -> &'static str {
        "tauri"
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

        source.contains("tauri::command")
            || source.contains("#[command")
            || source.contains("tauri::")
            || source.contains("use tauri")
            || source.contains("generate_handler!")
    }

    fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let file_path = path.to_string_lossy();
        let (req_dto, res_dto) = extract_tauri_dtos(target_node, source, &file_path);
        if let Some(req) = req_dto {
            if !slice.hoisted_types.iter().any(|t| t.name == req.name) {
                slice.hoisted_types.push(req);
            }
        }
        if let Some(res) = res_dto {
            if !slice.hoisted_types.iter().any(|t| t.name == res.name) {
                slice.hoisted_types.push(res);
            }
        }
        Ok(())
    }
}

fn is_tauri_command_function(fn_node: Node<'_>, source: &str) -> bool {
    let mut prev = fn_node.prev_named_sibling();
    while let Some(sibling) = prev {
        if sibling.kind() == "attribute_item" {
            let attr_text = AstUtils::node_text(sibling, source).trim();
            if attr_text.contains("tauri::command") || attr_text == "#[command]" || attr_text.starts_with("#[command(") {
                return true;
            }
        } else {
            break;
        }
        prev = sibling.prev_named_sibling();
    }
    false
}

fn extract_tauri_dtos(
    fn_node: Node<'_>,
    source: &str,
    file_path: &str,
) -> (Option<ExtractedType>, Option<ExtractedType>) {
    let mut req_dto = None;
    let mut res_dto = None;

    // 1. Request DTO from function parameters
    if let Some(params_node) = fn_node.child_by_field_name("parameters") {
        for param in params_node.named_children(&mut params_node.walk()) {
            if param.kind() == "parameter" {
                if let Some(type_node) = param.child_by_field_name("type") {
                    let type_text = AstUtils::node_text(type_node, source).trim();
                    let clean_type = clean_rust_type_name(type_text);
                    if !clean_type.is_empty() && !is_tauri_framework_type(&clean_type) && !is_primitive_type(&clean_type) {
                        if req_dto.is_none() {
                            req_dto = find_type_in_source(source, &clean_type, file_path);
                        }
                    }
                }
            }
        }
    }

    // 2. Response DTO from return type
    if let Some(ret_node) = fn_node.child_by_field_name("return_type") {
        let ret_text = AstUtils::node_text(ret_node, source).trim();
        let unwrapped = unwrap_result_type(ret_text);
        let clean_type = clean_rust_type_name(&unwrapped);
        if !clean_type.is_empty() && !is_tauri_framework_type(&clean_type) && !is_primitive_type(&clean_type) {
            res_dto = find_type_in_source(source, &clean_type, file_path);
        }
    }

    (req_dto, res_dto)
}

fn clean_rust_type_name(type_str: &str) -> String {
    let mut s = type_str.trim().trim_start_matches('&').trim();
    if let Some(pos) = s.find('<') {
        s = &s[..pos].trim();
    }
    s.split("::").last().unwrap_or(s).trim().to_string()
}

fn unwrap_result_type(ret_str: &str) -> String {
    let s = ret_str.trim_start_matches("->").trim();
    if s.starts_with("Result<") || s.starts_with("std::result::Result<") {
        if let Some(start) = s.find('<') {
            if let Some(comma) = s[start + 1..].find(',') {
                return s[start + 1..start + 1 + comma].trim().to_string();
            }
            if let Some(end) = s.rfind('>') {
                return s[start + 1..end].trim().to_string();
            }
        }
    }
    s.to_string()
}

fn is_tauri_framework_type(t: &str) -> bool {
    matches!(
        t,
        "AppHandle"
            | "Window"
            | "WebviewWindow"
            | "Webview"
            | "State"
            | "Manager"
            | "Runtime"
            | "Position"
            | "Size"
            | "EventLoopMessage"
            | "Menu"
    )
}

fn is_primitive_type(t: &str) -> bool {
    matches!(
        t,
        "()" | "bool" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
            | "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
            | "f32" | "f64" | "char" | "str" | "String" | "&str"
            | "Value" | "serde_json::Value" | "Vec<u8>" | "Option"
    )
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
                kind: if trimmed.contains("struct") { "struct".to_string() } else { "enum".to_string() },
                file_path: file_path.to_string(),
                definition: trimmed.to_string(),
            });
        }
    }
    None
}

fn scan_tauri_fallback(
    source: &str,
    file_path: &str,
    routes: &mut Vec<ServerRouteEndpoint>,
) {
    let lines: Vec<&str> = source.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.contains("#[tauri::command") || trimmed.contains("#[command") {
            // Find function declaration on next few lines
            for j in 1..=4 {
                if let Some(next_line) = lines.get(i + j) {
                    let next_trimmed = next_line.trim();
                    if next_trimmed.starts_with("pub fn ")
                        || next_trimmed.starts_with("pub async fn ")
                        || next_trimmed.starts_with("fn ")
                        || next_trimmed.starts_with("async fn ")
                    {
                        if let Some(fn_name) = extract_fn_name_from_sig(next_trimmed) {
                            let (req_dto, res_dto) = (None, None);
                            routes.push(ServerRouteEndpoint {
                                framework: "tauri".to_string(),
                                http_method: "IPC".to_string(),
                                route_path: fn_name.clone(),
                                handler_file: file_path.to_string(),
                                handler_symbol: fn_name,
                                handler_signature: next_trimmed.to_string(),
                                request_dto_type: req_dto,
                                response_dto_type: res_dto,
                            });
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn extract_fn_name_from_sig(sig: &str) -> Option<String> {
    if let Some(pos) = sig.find("fn ") {
        let after = &sig[pos + 3..];
        let name = after.split(['(', '<', ' ']).next()?.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}
