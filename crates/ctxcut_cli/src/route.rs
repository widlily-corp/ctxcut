//! Web framework route handler resolver.

use std::fs;
use std::path::Path;
use anyhow::{bail, Result};
use ctxcut_core::{ContextSlicer, SliceOptions, SliceResult, SupportedLanguage};
use ignore::WalkBuilder;

/// Resolves a route handler by HTTP method and URL path, returning its contextual slice.
pub fn resolve_route_slice(
    search_root: &Path,
    method: &str,
    route_path: &str,
    opts: &SliceOptions,
) -> Result<SliceResult> {
    let method_upper = method.to_uppercase();
    let method_lower = method.to_lowercase();
    let target_path_clean = route_path.trim_matches('/');

    let walker = WalkBuilder::new(search_root)
        .hidden(true)
        .parents(true)
        .git_ignore(true)
        .build();

    let slicer = ContextSlicer::new();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() || SupportedLanguage::from_path(path).is_none() {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };

        // Check if file contains method or route path match
        if !source.contains(target_path_clean) && !source.contains(route_path) {
            // Check segment match (e.g. "/checkout" inside "/api/v1/checkout")
            let last_segment = target_path_clean.split('/').last().unwrap_or(target_path_clean);
            if !source.contains(last_segment) {
                continue;
            }
        }

        // Try resolving in this file
        if let Some(symbol_name) = extract_route_handler_symbol(&source, &method_upper, &method_lower, target_path_clean) {
            if let Ok(slice) = slicer.slice_symbol(path, &symbol_name, opts) {
                return Ok(slice);
            }
        }
    }

    bail!("No route found matching `{} {}` in `{}`", method.to_uppercase(), route_path, search_root.display());
}

fn extract_route_handler_symbol(
    source: &str,
    method_upper: &str,
    method_lower: &str,
    target_path_clean: &str,
) -> Option<String> {
    let last_segment = target_path_clean.split('/').last().unwrap_or(target_path_clean);

    for line in source.lines() {
        let line_trimmed = line.trim();

        // 1. Express / Koa / Node: router.post('/...', ..., handler) or app.get('/...', handler)
        if (line_trimmed.contains(&format!(".{method_lower}(")) || line_trimmed.contains(&format!(".{method_upper}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            if let Some(handler) = extract_last_identifier_before_closing_paren(line_trimmed) {
                return Some(handler);
            }
        }

        // 2. FastAPI / Flask: @router.get("/...", ...) or @app.post("/...", ...)
        if line_trimmed.starts_with('@')
            && (line_trimmed.contains(&format!(".{method_lower}(")) || line_trimmed.contains(&format!(".{method_upper}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            // The handler is the def/async def on the next lines
            if let Some(def_name) = find_next_function_def(source, line) {
                return Some(def_name);
            }
        }

        // 3. Gin / Go: r.POST("/...", handler)
        if (line_trimmed.contains(&format!(".{method_upper}(")) || line_trimmed.contains(&format!(".{method_lower}(")))
            && (line_trimmed.contains(target_path_clean) || line_trimmed.contains(last_segment))
        {
            if let Some(handler) = extract_last_identifier_before_closing_paren(line_trimmed) {
                return Some(handler);
            }
        }

        // 4. Axum / Actix (Rust): route("/...", post(handler)) or web::post().to(handler)
        if line_trimmed.contains("route(")
            && (line_trimmed.contains(&format!("{method_lower}(")) || line_trimmed.contains(&format!("{method_upper}(")))
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
    let ident = last_arg.split(|c: char| !c.is_alphanumeric() && c != '_').find(|s| !s.is_empty())?;
    Some(ident.to_string())
}

fn extract_inside_method_call(line: &str, method: &str) -> Option<String> {
    let marker = format!("{method}(");
    let after = line.split(&marker).nth(1)?;
    let inside = after.split(')').next()?.trim();
    let ident = inside.split(|c: char| !c.is_alphanumeric() && c != '_').find(|s| !s.is_empty())?;
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
