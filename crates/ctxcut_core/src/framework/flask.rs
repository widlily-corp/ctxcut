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
use std::path::Path;

/// Flask web framework analyzer for Python.
#[derive(Debug, Default, Clone, Copy)]
pub struct FlaskAnalyzer;

impl FlaskAnalyzer {
    /// Creates a new `FlaskAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all server route endpoints from a Flask Python source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // @app.route('/api/...', methods=['GET', 'POST']) or @bp.route(...)
            if trimmed.starts_with('@') && (trimmed.contains(".route(") || trimmed.contains(".get(") || trimmed.contains(".post(") || trimmed.contains(".put(") || trimmed.contains(".delete(")) {
                let (path_opt, methods) = parse_flask_decorator(trimmed);
                if let Some(route_path) = path_opt {
                    // Find following def handler_name(...)
                    let mut handler_name = "handler".to_string();
                    let mut handler_sig = String::new();
                    for next_line in lines.iter().skip(i + 1) {
                        let t = next_line.trim();
                        if t.starts_with("def ") || t.starts_with("async def ") {
                            handler_sig = t.to_string();
                            let clean = t.trim_start_matches("async ").trim_start_matches("def ");
                            handler_name = clean.split(['(', ':']).next().unwrap_or("handler").trim().to_string();
                            break;
                        }
                    }

                    for m in methods {
                        routes.push(ServerRouteEndpoint {
                            framework: "flask".to_string(),
                            http_method: m,
                            route_path: route_path.clone(),
                            handler_file: file_path.clone(),
                            handler_symbol: handler_name.clone(),
                            handler_signature: handler_sig.clone(),
                            request_dto_type: None,
                            response_dto_type: None,
                        });
                    }
                }
            }
        }

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
}

impl FrameworkAnalyzer for FlaskAnalyzer {
    fn name(&self) -> &'static str {
        "flask"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let is_python = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext, "py" | "pyi"))
            .unwrap_or(false);

        if !is_python {
            return false;
        }

        source.contains("flask")
            || source.contains("Flask(__name__)")
            || source.contains("Blueprint(")
            || (source.contains("@app.route") && !source.contains("fastapi"))
    }

    fn enhance_slice(
        &self,
        _target_node: tree_sitter::Node<'_>,
        _source: &str,
        _path: &Path,
        _slice: &mut SliceResult,
    ) -> Result<()> {
        Ok(())
    }
}

fn parse_flask_decorator(line: &str) -> (Option<String>, Vec<String>) {
    let mut route_path = None;
    let mut methods = Vec::new();

    // Extract path inside quotes
    for quote in ['\'', '"'] {
        let pat = format!("({quote}");
        if let Some(pos) = line.find(&pat) {
            let after = &line[pos + pat.len()..];
            if let Some(end) = after.find(quote) {
                route_path = Some(after[..end].to_string());
                break;
            }
        }
    }

    // Extract methods: methods=['GET', 'POST'] or specific method helper @app.get
    if line.contains(".get(") {
        methods.push("GET".to_string());
    } else if line.contains(".post(") {
        methods.push("POST".to_string());
    } else if line.contains(".put(") {
        methods.push("PUT".to_string());
    } else if line.contains(".delete(") {
        methods.push("DELETE".to_string());
    } else if line.contains(".patch(") {
        methods.push("PATCH".to_string());
    } else if line.contains("methods=") {
        if let Some(pos) = line.find("methods=") {
            let after = &line[pos + 8..];
            if let Some(end) = after.find(']') {
                let inside = &after[..end];
                for m in ["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"] {
                    if inside.to_uppercase().contains(m) {
                        methods.push(m.to_string());
                    }
                }
            }
        }
    }

    if methods.is_empty() {
        methods.push("GET".to_string());
    }

    (route_path, methods)
}
