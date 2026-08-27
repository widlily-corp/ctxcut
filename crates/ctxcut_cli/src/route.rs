//! Web, IPC, and RPC framework route handler resolver.

use anyhow::{bail, Result};
use ctxcut_core::{
    extract_server_routes, ContextSlicer, ProjectWalker, RouteMatcher, SliceOptions, SliceResult,
    SupportedLanguage, TraversalConfig,
};
use std::fs;
use std::path::Path;

/// Resolves a route handler by HTTP/IPC/RPC method and URL/channel/command path, returning its contextual slice.
pub fn resolve_route_slice(
    search_root: &Path,
    method: &str,
    route_path: &str,
    opts: &SliceOptions,
) -> Result<SliceResult> {
    let config = TraversalConfig::default();
    let files = ProjectWalker::collect_files(search_root, &config);
    let slicer = ContextSlicer::new();
    let matcher = RouteMatcher::new();

    // 1. Extract AST server routes across all candidate files in the workspace
    let mut all_routes = Vec::new();
    for path in &files {
        if SupportedLanguage::from_path(path).is_none() {
            continue;
        }

        if let Ok(source) = fs::read_to_string(path) {
            let routes = extract_server_routes(path, &source);
            all_routes.extend(routes);
        }
    }

    // 2. Query RouteMatcher for best matching route
    let query_with_method = if method.is_empty() || method.eq_ignore_ascii_case("ANY") {
        route_path.to_string()
    } else {
        format!("{method} {route_path}")
    };

    let matched_route = matcher
        .find_best_server_route(&query_with_method, &all_routes)
        .or_else(|| matcher.find_best_server_route(route_path, &all_routes));

    if let Some(route) = matched_route {
        let target_path = Path::new(&route.handler_file);
        if let Ok(mut slice) = slicer.slice_symbol(target_path, &route.handler_symbol, opts) {
            // Hoist DTO models if available and not yet present in slice
            if let Some(req_dto) = &route.request_dto_type {
                if !slice.hoisted_types.iter().any(|t| t.name == req_dto.name) {
                    slice.hoisted_types.push(req_dto.clone());
                }
            }
            if let Some(res_dto) = &route.response_dto_type {
                if !slice.hoisted_types.iter().any(|t| t.name == res_dto.name) {
                    slice.hoisted_types.push(res_dto.clone());
                }
            }
            return Ok(slice);
        }
    }

    // 3. Fallback: line-by-line inspection for ad-hoc or unparsed frameworks
    let method_upper = method.to_uppercase();
    let method_lower = method.to_lowercase();
    let target_path_clean = route_path.trim_matches('/');

    for path in &files {
        if SupportedLanguage::from_path(path).is_none() {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };

        // Check if file contains method or route path match
        if !source.contains(target_path_clean) && !source.contains(route_path) {
            let last_segment = target_path_clean
                .split('/')
                .next_back()
                .unwrap_or(target_path_clean);
            if !source.contains(last_segment) {
                continue;
            }
        }

        if let Some(symbol_name) =
            extract_route_handler_symbol(&source, &method_upper, &method_lower, target_path_clean)
        {
            if let Ok(slice) = slicer.slice_symbol(path, &symbol_name, opts) {
                return Ok(slice);
            }
        }
    }

    bail!(
        "No route found matching `{} {}` in `{}`",
        method.to_uppercase(),
        route_path,
        search_root.display()
    );
}

fn extract_route_handler_symbol(
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
            if let Some(handler) = extract_last_identifier_before_closing_paren(line_trimmed) {
                return Some(handler);
            }
        }

        // 2. FastAPI / Flask: @router.get("/...", ...) or @app.post("/...", ...)
        if line_trimmed.starts_with('@')
            && (line_trimmed.contains(&format!(".{method_lower}("))
                || line_trimmed.contains(&format!(".{method_upper}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            if let Some(def_name) = find_next_function_def(source, line) {
                return Some(def_name);
            }
        }

        // 3. Gin / Go: r.POST("/...", handler)
        if (line_trimmed.contains(&format!(".{method_upper}("))
            || line_trimmed.contains(&format!(".{method_lower}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            if let Some(handler) = extract_last_identifier_before_closing_paren(line_trimmed) {
                return Some(handler);
            }
        }

        // 4. Axum / Actix (Rust): route("/...", post(handler)) or web::post().to(handler)
        if line_trimmed.contains("route(")
            && (line_trimmed.contains(&format!("{method_lower}("))
                || line_trimmed.contains(&format!("{method_upper}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            if let Some(handler) = extract_inside_method_call(line_trimmed, method_lower) {
                return Some(handler);
            }
        }
    }

    None
}

fn extract_last_identifier_before_closing_paren(line: &str) -> Option<String> {
    let before_paren = line.rsplit_once(')')?.0;
    let last_arg = before_paren.rsplit(',').next()?.trim();
    let ident = last_arg
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .find(|s| !s.is_empty())?;
    Some(ident.to_string())
}

fn extract_inside_method_call(line: &str, method: &str) -> Option<String> {
    let marker = format!("{method}(");
    let after = line.split(&marker).nth(1)?;
    let inside = after.split(')').next()?.trim();
    let ident = inside
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .find(|s| !s.is_empty())?;
    Some(ident.to_string())
}

fn find_next_function_def(source: &str, decorator_line: &str) -> Option<String> {
    let mut found_decorator = false;
    for line in source.lines() {
        if line.trim() == decorator_line.trim() {
            found_decorator = true;
            continue;
        }
        if found_decorator {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let after_def = if let Some(a) = trimmed.strip_prefix("async def ") {
                    a
                } else if let Some(a) = trimmed.strip_prefix("def ") {
                    a
                } else {
                    trimmed
                };
                let name = after_def.split('(').next()?.trim();
                return Some(name.to_string());
            }
        }
    }
    None
}
