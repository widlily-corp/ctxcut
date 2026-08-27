//! Route matching and path correlation.
//!
#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_self,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

use crate::fullstack::model::{ClientApiCall, ServerRouteEndpoint};

/// Matcher for correlating frontend API / IPC / RPC calls with server route handlers.
#[derive(Debug, Default, Clone, Copy)]
pub struct RouteMatcher;

impl RouteMatcher {
    /// Creates a new `RouteMatcher`.
    pub fn new() -> Self {
        Self
    }

    /// Matches a query string (e.g. `"/api/users"`, `"POST /api/users"`, `"user.getById"`, `"IPC calculate_tax"`, `"calculateTax"`, `"dialog:openFile"`) against a list of server routes.
    pub fn find_best_server_route<'a>(
        &self,
        query: &str,
        routes: &'a [ServerRouteEndpoint],
    ) -> Option<&'a ServerRouteEndpoint> {
        let (method_opt, raw_target) = parse_query_method_and_path(query);
        let clean_target = clean_route_identifier(&raw_target);

        // 1. Exact match with compatible method and exact/casing identifier match
        if let Some(method) = &method_opt {
            for route in routes {
                if methods_compatible(method, &route.http_method)
                    && identifiers_match(&clean_target, &route.handler_symbol, &route.route_path)
                {
                    return Some(route);
                }
            }
        }

        // 2. Exact or casing match on handler_symbol or clean route_path (method-agnostic)
        for route in routes {
            if identifiers_match(&clean_target, &route.handler_symbol, &route.route_path) {
                return Some(route);
            }
        }

        // 3. Path pattern match with path parameters (:id, {id}, ${id})
        for route in routes {
            if let Some(method) = &method_opt {
                if methods_compatible(method, &route.http_method)
                    && self.paths_match(&route.route_path, &raw_target)
                {
                    return Some(route);
                }
            } else if self.paths_match(&route.route_path, &raw_target) {
                return Some(route);
            }
        }

        // 4. Suffix or dot-notation procedure match (e.g. "getById" <-> "user.getById", "/trpc/user.getById")
        for route in routes {
            let norm_route = clean_route_identifier(&route.route_path);
            let norm_symbol = clean_route_identifier(&route.handler_symbol);
            if norm_route.ends_with(&clean_target)
                || clean_target.ends_with(&norm_route)
                || norm_symbol.ends_with(&clean_target)
                || clean_target.ends_with(&norm_symbol)
            {
                if let Some(method) = &method_opt {
                    if methods_compatible(method, &route.http_method) {
                        return Some(route);
                    }
                } else {
                    return Some(route);
                }
            }
        }

        // 5. Loose substring match across route path, handler symbol, and query target
        let target_lower = clean_target.to_lowercase();
        let target_stem = target_lower.trim_end_matches('s');
        for route in routes {
            let path_clean = clean_route_identifier(&route.route_path).to_lowercase();
            let symbol_clean = clean_route_identifier(&route.handler_symbol).to_lowercase();
            let path_stem = path_clean.trim_end_matches('s');
            let symbol_stem = symbol_clean.trim_end_matches('s');

            let matches = path_clean == target_lower
                || symbol_clean == target_lower
                || path_clean.contains(&target_lower)
                || target_lower.contains(&path_clean)
                || symbol_clean.contains(&target_lower)
                || target_lower.contains(&symbol_clean)
                || (!path_stem.is_empty() && target_lower.contains(path_stem))
                || (!symbol_stem.is_empty() && target_lower.contains(symbol_stem))
                || (!target_stem.is_empty() && (path_clean.contains(target_stem) || symbol_clean.contains(target_stem)));

            if matches {
                if let Some(method) = &method_opt {
                    if methods_compatible(method, &route.http_method) {
                        return Some(route);
                    }
                } else {
                    return Some(route);
                }
            }
        }

        None
    }

    /// Matches a `ClientApiCall` against a list of server routes.
    pub fn match_client_to_server<'a>(
        &self,
        client: &ClientApiCall,
        routes: &'a [ServerRouteEndpoint],
    ) -> Option<&'a ServerRouteEndpoint> {
        // 1. Match by RPC procedure / command identifier (Tauri, Electron, tRPC, Server Actions)
        if let Some(proc) = &client.rpc_procedure {
            let clean_proc = clean_route_identifier(proc);
            for route in routes {
                let method_matches = match &client.http_method {
                    Some(m) => methods_compatible(m, &route.http_method),
                    None => true,
                };
                if method_matches && identifiers_match(&clean_proc, &route.handler_symbol, &route.route_path) {
                    return Some(route);
                }
            }

            // Also check suffix matching for procedures like "user.getById" vs "getById"
            for route in routes {
                let norm_symbol = clean_route_identifier(&route.handler_symbol);
                let norm_path = clean_route_identifier(&route.route_path);
                if norm_symbol.ends_with(&clean_proc)
                    || clean_proc.ends_with(&norm_symbol)
                    || norm_path.ends_with(&clean_proc)
                    || clean_proc.ends_with(&norm_path)
                {
                    return Some(route);
                }
            }
        }

        // 2. Match by endpoint URL
        if let Some(endpoint) = &client.endpoint_url {
            let clean_endpoint = clean_route_identifier(endpoint);
            for route in routes {
                let method_matches = match &client.http_method {
                    Some(m) => methods_compatible(m, &route.http_method),
                    None => true,
                };
                if method_matches && identifiers_match(&clean_endpoint, &route.handler_symbol, &route.route_path) {
                    return Some(route);
                }
            }

            if let Some(route) = self.find_best_server_route(endpoint, routes) {
                if let Some(client_method) = &client.http_method {
                    if methods_compatible(client_method, &route.http_method) {
                        return Some(route);
                    }
                }
                return Some(route);
            }
        }

        // 3. Fallback: match by call snippet contents
        for route in routes {
            if client.call_snippet.contains(&route.handler_symbol) {
                return Some(route);
            }
            let camel = snake_to_camel(&route.handler_symbol);
            if client.call_snippet.contains(&camel) {
                return Some(route);
            }
        }

        None
    }

    /// Returns true if two route paths match, taking path parameters (`:id`, `{id}`, `${id}`) into account.
    pub fn paths_match(&self, route_pattern: &str, query_path: &str) -> bool {
        let norm_pattern = normalize_route_path(route_pattern);
        let norm_query = normalize_route_path(query_path);

        if norm_pattern == norm_query {
            return true;
        }

        let p_segments: Vec<&str> = norm_pattern.trim_matches('/').split('/').collect();
        let q_segments: Vec<&str> = norm_query.trim_matches('/').split('/').collect();

        if p_segments.len() != q_segments.len() {
            return false;
        }

        for (p_seg, q_seg) in p_segments.iter().zip(q_segments.iter()) {
            let is_p_param = p_seg.starts_with(':')
                || (p_seg.starts_with('{') && p_seg.ends_with('}'))
                || (p_seg.starts_with("${") && p_seg.ends_with('}'));
            let is_q_param = q_seg.starts_with(':')
                || (q_seg.starts_with('{') && q_seg.ends_with('}'))
                || (q_seg.starts_with("${") && q_seg.ends_with('}'));

            if is_p_param || is_q_param {
                continue;
            }

            if !p_seg.eq_ignore_ascii_case(q_seg) {
                return false;
            }
        }

        true
    }
}

/// Converts snake_case string to camelCase.
/// E.g. `"calculate_tax"` -> `"calculateTax"`, `"get_user_profile"` -> `"getUserProfile"`.
pub fn snake_to_camel(s: &str) -> String {
    let mut out = String::new();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            out.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Converts camelCase string to snake_case.
/// E.g. `"calculateTax"` -> `"calculate_tax"`, `"getUserProfile"` -> `"get_user_profile"`.
pub fn camel_to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_query_method_and_path(query: &str) -> (Option<String>, String) {
    let clean = query.trim();
    for method in &[
        "GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD",
        "IPC_HANDLE", "IPC_ON", "IPC", "QUERY", "MUTATION", "SUBSCRIPTION", "ACTION", "RPC", "ANY"
    ] {
        if clean.to_uppercase().starts_with(method) {
            let after = clean[method.len()..].trim();
            if !after.is_empty() {
                return (Some((*method).to_string()), after.to_string());
            }
        }
    }
    (None, clean.to_string())
}

fn clean_route_identifier(s: &str) -> String {
    let mut clean = s.trim();
    for prefix in &["tauri://", "electron://", "action://", "rpc://", "/trpc/", "trpc/", "/grpc/", "grpc/"] {
        if clean.starts_with(prefix) {
            clean = &clean[prefix.len()..];
        }
    }
    clean.trim_matches('/').to_string()
}

fn identifiers_match(query_ident: &str, handler_symbol: &str, route_path: &str) -> bool {
    let clean_symbol = clean_route_identifier(handler_symbol);
    let clean_path = clean_route_identifier(route_path);

    // 1. Direct case-insensitive match
    if query_ident.eq_ignore_ascii_case(&clean_symbol) || query_ident.eq_ignore_ascii_case(&clean_path) {
        return true;
    }

    // 2. Tauri snake_case <-> camelCase normalization
    let query_camel = snake_to_camel(query_ident);
    let query_snake = camel_to_snake(query_ident);

    if clean_symbol == query_camel || clean_symbol == query_snake
        || clean_path == query_camel || clean_path == query_snake
    {
        return true;
    }

    let symbol_camel = snake_to_camel(&clean_symbol);
    let symbol_snake = camel_to_snake(&clean_symbol);

    if symbol_camel == query_ident || symbol_snake == query_ident {
        return true;
    }

    // 3. Dot-separated namespace matching (e.g. "user.getById" <-> "getById")
    if let Some(pos) = query_ident.rfind('.') {
        let after_dot = &query_ident[pos + 1..];
        if after_dot.eq_ignore_ascii_case(&clean_symbol) || after_dot.eq_ignore_ascii_case(&clean_path) {
            return true;
        }
    }

    if let Some(pos) = clean_symbol.rfind('.') {
        let after_dot = &clean_symbol[pos + 1..];
        if after_dot.eq_ignore_ascii_case(query_ident) {
            return true;
        }
    }

    false
}

fn methods_compatible(query_method: &str, route_method: &str) -> bool {
    let q = query_method.to_uppercase();
    let r = route_method.to_uppercase();

    if q == "ANY" || r == "ANY" || q == r {
        return true;
    }

    if q == "IPC" && (r == "IPC" || r == "IPC_HANDLE" || r == "IPC_ON") {
        return true;
    }

    if (q == "IPC_HANDLE" || q == "IPC_ON") && r == "IPC" {
        return true;
    }

    if q == "RPC" && (r == "QUERY" || r == "MUTATION" || r == "SUBSCRIPTION" || r == "RPC" || r == "IPC") {
        return true;
    }

    if (q == "QUERY" && r == "GET") || (q == "GET" && r == "QUERY") {
        return true;
    }

    if (q == "MUTATION" && r == "POST") || (q == "POST" && r == "MUTATION") {
        return true;
    }

    false
}

fn normalize_route_path(path: &str) -> String {
    let mut clean = path.trim().to_string();
    if !clean.starts_with('/') && !clean.contains('.') && !clean.contains(':') {
        clean = format!("/{clean}");
    }
    clean
}
