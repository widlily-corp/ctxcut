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

/// Matcher for correlating frontend API calls with server route handlers.
#[derive(Debug, Default, Clone, Copy)]
pub struct RouteMatcher;

impl RouteMatcher {
    /// Creates a new `RouteMatcher`.
    pub fn new() -> Self {
        Self
    }

    /// Matches a query string (e.g. `"/api/users"`, `"POST /api/users"`, `"user.getById"`) against a list of server routes.
    pub fn find_best_server_route<'a>(
        &self,
        query: &str,
        routes: &'a [ServerRouteEndpoint],
    ) -> Option<&'a ServerRouteEndpoint> {
        let (method_opt, path_or_proc) = parse_query_method_and_path(query);

        // 1. Exact match with method (if specified)
        if let Some(method) = &method_opt {
            for route in routes {
                if route.http_method.eq_ignore_ascii_case(method)
                    && self.paths_match(&route.route_path, &path_or_proc)
                {
                    return Some(route);
                }
            }
        }

        // 2. Exact or pattern path match
        for route in routes {
            if self.paths_match(&route.route_path, &path_or_proc) {
                return Some(route);
            }
        }

        // 3. RPC procedure / symbol name match
        for route in routes {
            if route.handler_symbol.eq_ignore_ascii_case(&path_or_proc)
                || route.route_path.ends_with(&path_or_proc)
                || path_or_proc.ends_with(&route.handler_symbol)
            {
                return Some(route);
            }
        }

        // 4. Loose substring match
        for route in routes {
            let norm_route = normalize_route_path(&route.route_path);
            let norm_query = normalize_route_path(&path_or_proc);
            if norm_route == norm_query
                || norm_route.contains(&norm_query)
                || norm_query.contains(&norm_route)
            {
                return Some(route);
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
        // 1. Match by endpoint URL
        if let Some(endpoint) = &client.endpoint_url {
            if let Some(route) = self.find_best_server_route(endpoint, routes) {
                if let Some(client_method) = &client.http_method {
                    if route.http_method.eq_ignore_ascii_case(client_method) {
                        return Some(route);
                    }
                }
                return Some(route);
            }
        }

        // 2. Match by RPC procedure
        if let Some(proc) = &client.rpc_procedure {
            if let Some(route) = self.find_best_server_route(proc, routes) {
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

fn parse_query_method_and_path(query: &str) -> (Option<String>, String) {
    let clean = query.trim();
    for method in &["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"] {
        if clean.to_uppercase().starts_with(method) {
            let after = clean[method.len()..].trim();
            if !after.is_empty() {
                return (Some((*method).to_string()), after.to_string());
            }
        }
    }
    (None, clean.to_string())
}

fn normalize_route_path(path: &str) -> String {
    let mut clean = path.trim().to_string();
    if !clean.starts_with('/') && !clean.contains('.') {
        clean = format!("/{clean}");
    }
    clean
}
