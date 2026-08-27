//! Frontend client API detection module.
//!
#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_self,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

use crate::fullstack::model::ClientApiCall;
use crate::parser::AstUtils;
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Polyglot frontend API call scanner and detector.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClientDetector;

impl ClientDetector {
    /// Creates a new `ClientDetector`.
    pub fn new() -> Self {
        Self
    }

    /// Detects all client API calls in a source file.
    pub fn detect_in_file(&self, path: &Path, source: &str) -> Vec<ClientApiCall> {
        let mut results = Vec::new();

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(
            ext.as_str(),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "vue" | "svelte" | "astro"
        ) {
            return results;
        }

        let mut parser = Parser::new();
        let lang = tree_sitter_typescript::LANGUAGE_TSX.into();
        if parser.set_language(&lang).is_err() {
            return results;
        }

        let tree = match parser.parse(source.as_bytes(), None) {
            Some(t) => t,
            None => return results,
        };

        let root = tree.root_node();
        let file_path_str = path.to_string_lossy().to_string();

        self.traverse_node(root, source, &file_path_str, &mut results);

        // Fallback text regex/scanning for high-fidelity discovery in case AST had minor parse issues
        scan_fallback_patterns(source, &file_path_str, &mut results);

        // Deduplicate calls by file_path and line_number
        let mut unique = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for call in results {
            let key = (call.file_path.clone(), call.line_number, call.client_kind.clone(), call.endpoint_url.clone(), call.rpc_procedure.clone());
            if seen.insert(key) {
                unique.push(call);
            }
        }

        unique
    }

    fn traverse_node(
        &self,
        node: Node<'_>,
        source: &str,
        file_path: &str,
        results: &mut Vec<ClientApiCall>,
    ) {
        if node.kind() == "call_expression" {
            if let Some(call) = self.inspect_call_expression(node, source, file_path) {
                results.push(call);
            }
        }

        for child in node.children(&mut node.walk()) {
            self.traverse_node(child, source, file_path, results);
        }
    }

    fn inspect_call_expression(
        &self,
        node: Node<'_>,
        source: &str,
        file_path: &str,
    ) -> Option<ClientApiCall> {
        let function_node = node.child_by_field_name("function")?;
        let func_text = AstUtils::node_text(function_node, source).trim().to_string();
        let line_number = node.start_position().row + 1;
        let call_snippet = AstUtils::node_text(node, source).trim().to_string();

        // 1. fetch calls: fetch('/api/...', { method: 'POST' })
        if func_text == "fetch" || func_text.ends_with(".fetch") {
            let args_node = node.child_by_field_name("arguments")?;
            let (endpoint, method, req_dto, res_dto) = self.parse_fetch_args(args_node, source);
            if endpoint.is_some() || method.is_some() {
                return Some(ClientApiCall {
                    client_kind: "fetch".to_string(),
                    http_method: Some(method.unwrap_or_else(|| "GET".to_string())),
                    endpoint_url: endpoint,
                    rpc_procedure: None,
                    file_path: file_path.to_string(),
                    line_number,
                    call_snippet,
                    request_dto: req_dto,
                    response_dto: res_dto,
                });
            }
        }

        // 2. axios calls: axios.get(...), axios.post(...), apiClient.get(...)
        if func_text.contains("axios")
            || func_text.starts_with("apiClient.")
            || func_text.starts_with("api.")
            || func_text.starts_with("client.")
            || func_text.starts_with("http.")
            || func_text.starts_with("request.")
        {
            if let Some(call) = self.parse_axios_call(node, &func_text, source, file_path, line_number, &call_snippet) {
                return Some(call);
            }
        }

        // 3. React Query & Apollo GraphQL hooks: useQuery, useMutation, useInfiniteQuery
        if func_text == "useQuery" || func_text == "useMutation" || func_text == "useInfiniteQuery" {
            let is_graphql = source.contains("@apollo")
                || source.contains("urql")
                || source.contains("gql`")
                || source.contains("gql(")
                || source.contains("GET_")
                || source.contains("MUTATION");

            if is_graphql {
                if let Some(call) = self.parse_graphql_call(node, &func_text, source, file_path, line_number, &call_snippet) {
                    return Some(call);
                }
            } else if let Some(call) = self.parse_react_query_call(node, &func_text, source, file_path, line_number, &call_snippet) {
                return Some(call);
            }
        }

        // 4. tRPC calls: trpc.user.getById.useQuery, trpcClient.user.getById.query, api.user.get.useQuery
        if (func_text.starts_with("trpc.") || func_text.starts_with("trpcClient.") || func_text.starts_with("api."))
            && (func_text.ends_with(".useQuery")
                || func_text.ends_with(".useMutation")
                || func_text.ends_with(".query")
                || func_text.ends_with(".mutate"))
        {
            let rpc_procedure = self.extract_trpc_procedure(&func_text);
            let method = if func_text.ends_with("Mutation") || func_text.ends_with("mutate") {
                "POST".to_string()
            } else {
                "GET".to_string()
            };
            return Some(ClientApiCall {
                client_kind: "trpc".to_string(),
                http_method: Some(method),
                endpoint_url: rpc_procedure.as_ref().map(|p| format!("/trpc/{p}")),
                rpc_procedure,
                file_path: file_path.to_string(),
                line_number,
                call_snippet,
                request_dto: None,
                response_dto: None,
            });
        }

        // 5. GraphQL client calls: client.query, client.mutate, useQuery(GQL), useMutation(GQL)
        if func_text.contains("graphql") || func_text.ends_with(".query") || func_text.ends_with(".mutate") {
            if let Some(call) = self.parse_graphql_call(node, &func_text, source, file_path, line_number, &call_snippet) {
                return Some(call);
            }
        }

        // 6. gRPC-web calls: grpc.unary, client.getUser(req, ...), userService.createUser(req, ...)
        let lower_func = func_text.to_lowercase();
        if lower_func.starts_with("grpc.") || lower_func.contains("client.") || lower_func.contains("service.") {
            if let Some(call) = self.parse_grpc_web_call(node, &func_text, source, file_path, line_number, &call_snippet) {
                return Some(call);
            }
        }

        None
    }

    fn parse_fetch_args(&self, args_node: Node<'_>, source: &str) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
        let mut endpoint = None;
        let mut method = None;
        let mut req_dto = None;
        let res_dto = None;

        for (child_count, child) in args_node.named_children(&mut args_node.walk()).enumerate() {
            if child_count == 0 {
                endpoint = self.extract_url_literal(child, source);
            } else if child_count == 1 && child.kind() == "object" {
                // Parse options object: { method: 'POST', body: ... }
                let obj_text = AstUtils::node_text(child, source);
                if let Some(m) = extract_property_string(obj_text, "method") {
                    method = Some(m.to_uppercase());
                }
                if obj_text.contains("body:") {
                    if let Some(pos) = obj_text.find("body:") {
                        let after = &obj_text[pos + 5..].trim();
                        let candidate = after.split_whitespace().next().unwrap_or("").trim_matches([',', '}']);
                        if !candidate.is_empty() {
                            req_dto = Some(candidate.to_string());
                        }
                    }
                }
            }
        }

        (endpoint, method, req_dto, res_dto)
    }

    fn parse_axios_call(
        &self,
        node: Node<'_>,
        func_text: &str,
        source: &str,
        file_path: &str,
        line_number: usize,
        call_snippet: &str,
    ) -> Option<ClientApiCall> {
        let method = if func_text.ends_with(".get") {
            Some("GET".to_string())
        } else if func_text.ends_with(".post") {
            Some("POST".to_string())
        } else if func_text.ends_with(".put") {
            Some("PUT".to_string())
        } else if func_text.ends_with(".delete") {
            Some("DELETE".to_string())
        } else if func_text.ends_with(".patch") {
            Some("PATCH".to_string())
        } else {
            None
        };

        let args_node = node.child_by_field_name("arguments")?;
        let first_arg = args_node.named_child(0)?;
        let endpoint = self.extract_url_literal(first_arg, source);

        if endpoint.is_none() && method.is_none() {
            return None;
        }

        // Extract type arguments: axios.get<ResponseDto, AxiosResponse<...>, RequestDto>(...)
        let (mut req_dto, res_dto) = self.extract_type_arguments(node, source);
        if req_dto.is_none() {
            if let Some(args_n) = node.child_by_field_name("arguments") {
                if args_n.named_child_count() >= 2 {
                    let second_arg = args_n.named_child(1).unwrap();
                    let arg_text = AstUtils::node_text(second_arg, source).trim();
                    for line in source.lines() {
                        let t = line.trim();
                        if t.contains(arg_text) && t.contains(':') {
                            if let Some(pos) = t.find(&format!("{arg_text}:")) {
                                let after = &t[pos + arg_text.len() + 1..].trim();
                                let candidate = after.split([',', ')', ';', '{', ' ']).next().unwrap_or("").trim();
                                if !candidate.is_empty() && candidate.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                                    req_dto = Some(candidate.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        Some(ClientApiCall {
            client_kind: "axios".to_string(),
            http_method: method.or_else(|| Some("GET".to_string())),
            endpoint_url: endpoint,
            rpc_procedure: None,
            file_path: file_path.to_string(),
            line_number,
            call_snippet: call_snippet.to_string(),
            request_dto: req_dto,
            response_dto: res_dto,
        })
    }

    fn parse_react_query_call(
        &self,
        node: Node<'_>,
        func_text: &str,
        source: &str,
        file_path: &str,
        line_number: usize,
        call_snippet: &str,
    ) -> Option<ClientApiCall> {
        let is_mutation = func_text == "useMutation";
        let default_method = if is_mutation { "POST" } else { "GET" };

        let snippet = AstUtils::node_text(node, source);
        let mut endpoint = None;
        let mut method = Some(default_method.to_string());

        // Search for nested url literal in queryFn/mutationFn or queryKey
        for line in snippet.lines() {
            if line.contains("/api/") || line.contains("/v1/") || line.contains("/v2/") || line.contains("/graphql") {
                if let Some(url) = extract_first_path_string(line) {
                    endpoint = Some(url);
                    break;
                }
            }
        }

        if snippet.contains(".post(") || snippet.contains("method: 'POST'") || snippet.contains("method: \"POST\"") {
            method = Some("POST".to_string());
        } else if snippet.contains(".put(") {
            method = Some("PUT".to_string());
        } else if snippet.contains(".delete(") {
            method = Some("DELETE".to_string());
        }

        let (req_dto, res_dto) = self.extract_type_arguments(node, source);

        Some(ClientApiCall {
            client_kind: "react_query".to_string(),
            http_method: method,
            endpoint_url: endpoint,
            rpc_procedure: None,
            file_path: file_path.to_string(),
            line_number,
            call_snippet: call_snippet.to_string(),
            request_dto: req_dto,
            response_dto: res_dto,
        })
    }

    fn parse_graphql_call(
        &self,
        node: Node<'_>,
        _func_text: &str,
        source: &str,
        file_path: &str,
        line_number: usize,
        call_snippet: &str,
    ) -> Option<ClientApiCall> {
        let snippet = AstUtils::node_text(node, source);
        let mut operation_name = None;

        for line in snippet.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("query ") || trimmed.starts_with("mutation ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() > 1 {
                    let op = parts[1].split(['(', '{']).next().unwrap_or(parts[1]).trim();
                    if !op.is_empty() {
                        operation_name = Some(op.to_string());
                        break;
                    }
                }
            }
        }

        if operation_name.is_none() {
            if let Some(args_node) = node.child_by_field_name("arguments") {
                if let Some(first_arg) = args_node.named_child(0) {
                    let arg_text = AstUtils::node_text(first_arg, source).trim().to_string();
                    if !arg_text.is_empty() {
                        operation_name = Some(arg_text);
                    }
                }
            }
        }

        let method = "POST".to_string(); // GraphQL HTTP POST

        Some(ClientApiCall {
            client_kind: "graphql".to_string(),
            http_method: Some(method),
            endpoint_url: Some("/graphql".to_string()),
            rpc_procedure: operation_name,
            file_path: file_path.to_string(),
            line_number,
            call_snippet: call_snippet.to_string(),
            request_dto: None,
            response_dto: None,
        })
    }

    fn parse_grpc_web_call(
        &self,
        node: Node<'_>,
        func_text: &str,
        source: &str,
        file_path: &str,
        line_number: usize,
        call_snippet: &str,
    ) -> Option<ClientApiCall> {
        let procedure = if func_text.starts_with("grpc.unary") {
            let args_node = node.child_by_field_name("arguments")?;
            let first_arg = args_node.named_child(0)?;
            Some(AstUtils::node_text(first_arg, source).trim().to_string())
        } else {
            let parts: Vec<&str> = func_text.split('.').collect();
            if parts.len() >= 2 {
                Some(format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]))
            } else {
                Some(func_text.to_string())
            }
        };

        Some(ClientApiCall {
            client_kind: "grpc_web".to_string(),
            http_method: Some("POST".to_string()),
            endpoint_url: procedure.as_ref().map(|p| format!("/grpc/{p}")),
            rpc_procedure: procedure,
            file_path: file_path.to_string(),
            line_number,
            call_snippet: call_snippet.to_string(),
            request_dto: None,
            response_dto: None,
        })
    }

    fn extract_trpc_procedure(&self, func_text: &str) -> Option<String> {
        let clean = func_text
            .trim_start_matches("trpc.")
            .trim_start_matches("trpcClient.")
            .trim_start_matches("api.")
            .trim_end_matches(".useQuery")
            .trim_end_matches(".useMutation")
            .trim_end_matches(".query")
            .trim_end_matches(".mutate");
        if clean.is_empty() {
            None
        } else {
            Some(clean.to_string())
        }
    }

    fn extract_url_literal(&self, node: Node<'_>, source: &str) -> Option<String> {
        let text = AstUtils::node_text(node, source).trim();
        if text.starts_with('\'') || text.starts_with('"') || text.starts_with('`') {
            let clean = text.trim_matches(['\'', '"', '`']);
            if clean.starts_with('/') || clean.starts_with("http://") || clean.starts_with("https://") {
                return Some(clean.to_string());
            }
        }
        if node.kind() == "template_string" {
            return Some(text.trim_matches('`').to_string());
        }
        None
    }

    fn extract_type_arguments(&self, node: Node<'_>, source: &str) -> (Option<String>, Option<String>) {
        if let Some(type_args) = node.child_by_field_name("type_arguments") {
            let types: Vec<String> = type_args
                .named_children(&mut type_args.walk())
                .map(|n| AstUtils::node_text(n, source).trim().to_string())
                .collect();
            if types.len() == 1 {
                return (None, Some(types[0].clone()));
            } else if types.len() >= 2 {
                return (Some(types[1].clone()), Some(types[0].clone()));
            }
        }
        (None, None)
    }
}

fn scan_fallback_patterns(
    source: &str,
    file_path: &str,
    results: &mut Vec<ClientApiCall>,
) {
    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        // fetch pattern: fetch('/api/...')
        if trimmed.contains("fetch(")
            && (trimmed.contains("'/") || trimmed.contains("\"/") || trimmed.contains("`/"))
            && !results.iter().any(|r| r.line_number == line_num)
        {
            if let Some(url) = extract_first_path_string(trimmed) {
                let method = if trimmed.contains("POST") { "POST" } else { "GET" };
                results.push(ClientApiCall {
                    client_kind: "fetch".to_string(),
                    http_method: Some(method.to_string()),
                    endpoint_url: Some(url),
                    rpc_procedure: None,
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    call_snippet: trimmed.to_string(),
                    request_dto: None,
                    response_dto: None,
                });
            }
        }

        // axios pattern: axios.get('/api/...') / axios.post(...)
        if (trimmed.contains("axios.") || trimmed.contains("apiClient."))
            && (trimmed.contains("'/") || trimmed.contains("\"/") || trimmed.contains("`/"))
            && !results.iter().any(|r| r.line_number == line_num)
        {
            if let Some(url) = extract_first_path_string(trimmed) {
                let method = if trimmed.contains(".post") {
                    "POST"
                } else if trimmed.contains(".put") {
                    "PUT"
                } else if trimmed.contains(".delete") {
                    "DELETE"
                } else {
                    "GET"
                };
                results.push(ClientApiCall {
                    client_kind: "axios".to_string(),
                    http_method: Some(method.to_string()),
                    endpoint_url: Some(url),
                    rpc_procedure: None,
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    call_snippet: trimmed.to_string(),
                    request_dto: None,
                    response_dto: None,
                });
            }
        }

        // trpc pattern: trpc.<something>.useQuery / mutate
        if trimmed.contains("trpc.")
            && (trimmed.contains(".useQuery") || trimmed.contains(".useMutation") || trimmed.contains(".query(") || trimmed.contains(".mutate("))
            && !results.iter().any(|r| r.line_number == line_num)
        {
            let proc = extract_trpc_proc_from_line(trimmed);
            let method = if trimmed.contains("Mutation") || trimmed.contains("mutate") { "POST" } else { "GET" };
            results.push(ClientApiCall {
                client_kind: "trpc".to_string(),
                http_method: Some(method.to_string()),
                endpoint_url: proc.as_ref().map(|p| format!("/trpc/{p}")),
                rpc_procedure: proc,
                file_path: file_path.to_string(),
                line_number: line_num,
                call_snippet: trimmed.to_string(),
                request_dto: None,
                response_dto: None,
            });
        }
    }
}

fn extract_property_string(obj_str: &str, prop_name: &str) -> Option<String> {
    let pattern = format!("{prop_name}:");
    if let Some(pos) = obj_str.find(&pattern) {
        let after = &obj_str[pos + pattern.len()..].trim();
        let clean = after.split([',', '}', ';', '\n']).next().unwrap_or(after).trim();
        let val = clean.trim_matches(['\'', '"', '`']).trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    None
}

fn extract_first_path_string(s: &str) -> Option<String> {
    for quote in ['\'', '"', '`'] {
        if let Some(first) = s.find(quote) {
            if let Some(second) = s[first + 1..].find(quote) {
                let candidate = &s[first + 1..first + 1 + second];
                if candidate.starts_with('/') {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    None
}

fn extract_trpc_proc_from_line(line: &str) -> Option<String> {
    if let Some(pos) = line.find("trpc.") {
        let after = &line[pos + 5..];
        let token = after.split(['(', ' ', ',', ';', '{']).next().unwrap_or(after);
        let proc = token
            .trim_end_matches(".useQuery")
            .trim_end_matches(".useMutation")
            .trim_end_matches(".query")
            .trim_end_matches(".mutate");
        if !proc.is_empty() {
            return Some(proc.to_string());
        }
    }
    None
}
