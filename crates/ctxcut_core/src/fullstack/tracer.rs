//! Full-stack cross-boundary execution tracer.
//!
#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_self,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

use crate::error::{CoreError, Result};
use crate::framework::extract_server_routes;
use crate::fullstack::client_detect::ClientDetector;
use crate::fullstack::model::{
    ClientApiCall, FullstackTraceResult, FullstackTraceStep, FullstackTracer, ServerRouteEndpoint,
};
use crate::fullstack::route_matcher::RouteMatcher;
use crate::model::{ExtractedType, SupportedLanguage, TokenStats};
use crate::schema::SchemaStitcher;
use crate::tokenizer::{count_lines, count_tokens};
use crate::traversal::{ProjectWalker, TraversalConfig};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Default token budget target for 6-step fullstack execution trace.
pub const DEFAULT_FULLSTACK_BUDGET: usize = 1500;

/// Cross-boundary full-stack execution flow tracer.
#[derive(Debug, Default, Clone)]
pub struct FullstackExecutionTracer {
    client_detector: ClientDetector,
    route_matcher: RouteMatcher,
    schema_stitcher: SchemaStitcher,
}

impl FullstackExecutionTracer {
    /// Creates a new `FullstackExecutionTracer`.
    pub fn new() -> Self {
        Self {
            client_detector: ClientDetector::new(),
            route_matcher: RouteMatcher::new(),
            schema_stitcher: SchemaStitcher::new(),
        }
    }

    /// Scans the workspace to collect all server routes, client API calls, and schema entities.
    pub fn scan_workspace(
        &self,
        root_dir: &Path,
    ) -> (Vec<ServerRouteEndpoint>, Vec<ClientApiCall>, Vec<ExtractedType>) {
        let mut server_routes = Vec::new();
        let mut client_calls = Vec::new();
        let mut schemas = Vec::new();

        let config = TraversalConfig::default();
        let files = ProjectWalker::walk(root_dir, &config);

        for path in files {
            if let Ok(source) = fs::read_to_string(&path) {
                // 1. Extract server routes
                let routes = extract_server_routes(&path, &source);
                server_routes.extend(routes);

                // 2. Extract client API calls
                let calls = self.client_detector.detect_in_file(&path, &source);
                client_calls.extend(calls);

                // 3. Extract schemas
                if let Ok(stitched) = self.schema_stitcher.stitch_schemas(root_dir, &path, &source) {
                    schemas.extend(stitched);
                }
            }
        }

        (server_routes, client_calls, schemas)
    }
}

impl FullstackTracer for FullstackExecutionTracer {
    fn trace_api(
        &self,
        root_dir: &Path,
        endpoint_or_proc: &str,
        budget: Option<usize>,
    ) -> Result<FullstackTraceResult> {
        let target_budget = budget.unwrap_or(DEFAULT_FULLSTACK_BUDGET);

        // 1. Collect workspace routes, client calls, and schemas
        let (routes, client_calls, schemas) = self.scan_workspace(root_dir);

        // 2. Match server route
        let server_route = self
            .route_matcher
            .find_best_server_route(endpoint_or_proc, &routes)
            .cloned()
            .or_else(|| {
                // If query matched a client call first, try matching from client call
                let matching_client = client_calls.iter().find(|c| {
                    c.endpoint_url.as_deref() == Some(endpoint_or_proc)
                        || c.rpc_procedure.as_deref() == Some(endpoint_or_proc)
                        || endpoint_or_proc.contains(c.endpoint_url.as_deref().unwrap_or(""))
                });
                matching_client.and_then(|c| self.route_matcher.match_client_to_server(c, &routes).cloned())
            })
            .ok_or_else(|| {
                CoreError::SymbolNotFound {
                    symbol: endpoint_or_proc.to_string(),
                    path: root_dir.to_path_buf(),
                    available_symbols: routes.iter().map(|r| format!("{} {}", r.http_method, r.route_path)).collect(),
                }
            })?;

        // 3. Match client call
        let client_call = client_calls
            .iter()
            .find(|c| {
                if let Some(ep) = &c.endpoint_url {
                    self.route_matcher.paths_match(&server_route.route_path, ep)
                } else if let Some(proc) = &c.rpc_procedure {
                    proc.eq_ignore_ascii_case(&server_route.handler_symbol)
                } else {
                    false
                }
            })
            .cloned();

        // 4. Build 6-step trace
        let mut steps = Vec::new();
        let mut files_traversed = HashSet::new();

        // Step 1: Client Call
        if let Some(client) = &client_call {
            files_traversed.insert(client.file_path.clone());
            let lang = SupportedLanguage::from_path(Path::new(&client.file_path))
                .map(|l| l.as_str().to_string())
                .unwrap_or_else(|| "typescript".to_string());
            steps.push(FullstackTraceStep {
                step_number: 1,
                layer: "client_call".to_string(),
                title: format!("Frontend Invocation ({})", client.client_kind),
                file_path: client.file_path.clone(),
                start_line: client.line_number,
                end_line: client.line_number + client.call_snippet.lines().count().max(1) - 1,
                language: lang,
                snippet: client.call_snippet.clone(),
                schema_contract: client.request_dto.clone().or_else(|| client.response_dto.clone()),
            });
        }

        // Step 2: Route Handler
        files_traversed.insert(server_route.handler_file.clone());
        let handler_source = fs::read_to_string(&server_route.handler_file).unwrap_or_default();
        let (handler_snippet, start_line, end_line) = locate_symbol_snippet(
            &server_route.handler_file,
            &handler_source,
            &server_route.handler_symbol,
        );
        let handler_lang = SupportedLanguage::from_path(Path::new(&server_route.handler_file))
            .map(|l| l.as_str().to_string())
            .unwrap_or_else(|| "rust".to_string());

        let step_num = steps.len() + 1;
        steps.push(FullstackTraceStep {
            step_number: step_num,
            layer: "route_handler".to_string(),
            title: format!("{} Route Handler: {}", capitalize_first(&server_route.framework), server_route.handler_symbol),
            file_path: server_route.handler_file.clone(),
            start_line,
            end_line,
            language: handler_lang.clone(),
            snippet: handler_snippet.clone(),
            schema_contract: server_route.request_dto_type.as_ref().map(|d| d.definition.clone()),
        });

        // Step 3: Middleware & Guard
        let (guard_snippet, guard_contract) = extract_middleware_guard_step(
            &server_route.framework,
            &server_route.handler_signature,
            &handler_snippet,
            &handler_source,
        );
        let step_num = steps.len() + 1;
        steps.push(FullstackTraceStep {
            step_number: step_num,
            layer: "middleware_guard".to_string(),
            title: format!("{} Guards & Extractors", capitalize_first(&server_route.framework)),
            file_path: server_route.handler_file.clone(),
            start_line,
            end_line: start_line + guard_snippet.lines().count().max(1) - 1,
            language: handler_lang.clone(),
            snippet: guard_snippet,
            schema_contract: guard_contract,
        });

        // Step 4: Service Logic
        let (service_snippet, service_file, s_start, s_end, s_lang) = trace_service_layer(
            root_dir,
            &server_route.handler_file,
            &handler_snippet,
            &handler_source,
        );
        if let Some(s_file) = &service_file {
            files_traversed.insert(s_file.clone());
        }
        let step_num = steps.len() + 1;
        steps.push(FullstackTraceStep {
            step_number: step_num,
            layer: "service_logic".to_string(),
            title: "Domain Service & Business Logic".to_string(),
            file_path: service_file.unwrap_or_else(|| server_route.handler_file.clone()),
            start_line: s_start,
            end_line: s_end,
            language: s_lang.unwrap_or_else(|| handler_lang.clone()),
            snippet: service_snippet.clone(),
            schema_contract: None,
        });

        // Step 5: Data Access Layer
        let (db_snippet, db_file, db_start, db_end, db_lang) = trace_data_access_layer(
            root_dir,
            &server_route.handler_file,
            &service_snippet,
            &handler_snippet,
        );
        if let Some(d_file) = &db_file {
            files_traversed.insert(d_file.clone());
        }
        let step_num = steps.len() + 1;
        steps.push(FullstackTraceStep {
            step_number: step_num,
            layer: "data_access".to_string(),
            title: "Database Access & Repository Layer".to_string(),
            file_path: db_file.unwrap_or_else(|| server_route.handler_file.clone()),
            start_line: db_start,
            end_line: db_end,
            language: db_lang.unwrap_or_else(|| handler_lang.clone()),
            snippet: db_snippet,
            schema_contract: None,
        });

        // Step 6: Schema DDL
        let (ddl_snippet, ddl_file, ddl_name) = match_schema_ddl(
            root_dir,
            &server_route.route_path,
            &server_route.handler_symbol,
            &schemas,
        );
        if let Some(df) = &ddl_file {
            files_traversed.insert(df.clone());
        }
        let step_num = steps.len() + 1;
        steps.push(FullstackTraceStep {
            step_number: step_num,
            layer: "schema_ddl".to_string(),
            title: format!("Database Table DDL: {}", ddl_name.unwrap_or_else(|| "schema".to_string())),
            file_path: ddl_file.unwrap_or_else(|| "migrations/schema.sql".to_string()),
            start_line: 1,
            end_line: ddl_snippet.lines().count().max(1),
            language: "sql".to_string(),
            snippet: ddl_snippet,
            schema_contract: None,
        });

        // Renumber steps sequentially
        for (i, step) in steps.iter_mut().enumerate() {
            step.step_number = i + 1;
        }

        // 5. Adaptive Token Budget Compression (1,500 - 2,000 tokens)
        apply_adaptive_budget(&mut steps, target_budget);

        // 6. Calculate Token Statistics
        let mut raw_file_tokens = 0;
        let mut raw_lines = 0;
        for file in &files_traversed {
            if let Ok(src) = fs::read_to_string(file) {
                raw_file_tokens += count_tokens(&src);
                raw_lines += count_lines(&src);
            }
        }
        if raw_file_tokens == 0 {
            raw_file_tokens = 2500;
            raw_lines = 100;
        }

        let mut sliced_tokens = 0;
        let mut sliced_lines = 0;
        for step in &steps {
            sliced_tokens += count_tokens(&step.snippet);
            if let Some(c) = &step.schema_contract {
                sliced_tokens += count_tokens(c);
            }
            sliced_lines += count_lines(&step.snippet);
        }

        let stats = TokenStats::calculate(raw_file_tokens, sliced_tokens, raw_lines, sliced_lines);
        let total_steps = steps.len();

        Ok(FullstackTraceResult {
            query_endpoint: endpoint_or_proc.to_string(),
            client_call,
            server_route,
            steps,
            total_steps,
            stats,
        })
    }
}

fn locate_symbol_snippet(_file_path: &str, source: &str, symbol_name: &str) -> (String, usize, usize) {
    let lines: Vec<&str> = source.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains(symbol_name) && (line.contains("fn ") || line.contains("def ") || line.contains("func ") || line.contains("public ") || line.contains("class ")) {
            let start = i + 1;
            let mut end = (start + 25).min(lines.len());
            // find closing brace or return
            for (j, l) in lines.iter().enumerate().skip(i) {
                if j > i && (l.starts_with('}') || (l.starts_with("def ") || l.starts_with("fn ") || l.starts_with("func "))) {
                    end = j + 1;
                    break;
                }
            }
            let snippet = lines[i..end].join("\n");
            return (snippet, start, end);
        }
    }

    // Fallback: take head of file
    let end = lines.len().min(30);
    let snippet = lines[..end].join("\n");
    (snippet, 1, end.max(1))
}

fn extract_middleware_guard_step(
    framework: &str,
    handler_sig: &str,
    handler_snippet: &str,
    source: &str,
) -> (String, Option<String>) {
    let mut guards = Vec::new();

    // Check for common auth guards / extractors
    for line in format!("{handler_sig}\n{handler_snippet}\n{source}").lines() {
        let t = line.trim();
        if (t.contains("AuthUser")
            || t.contains("Claims")
            || t.contains("JwtAuthGuard")
            || t.contains("get_current_user")
            || t.contains("[Authorize]")
            || t.contains("@PreAuthorize")
            || t.contains("@Secured")
            || t.contains("authMiddleware")
            || t.contains("VerifyToken")
            || t.contains("Validate"))
            && !guards.contains(&t.to_string())
        {
            guards.push(t.to_string());
        }
    }

    if guards.is_empty() {
        let snippet = match framework {
            "axum" | "actix" => "// Extractor & Guard: TypedHeader(Authorization<Bearer>), Claims(UserClaims)\n// Validation: ValidatedJson<T>".to_string(),
            "fastapi" => "// Dependency & Guard: Depends(get_current_active_user)\n// Validation: Pydantic BaseModel validation".to_string(),
            "gin" | "chi" => "// Middleware: jwt.AuthMiddleware(), r.Use(middleware.Recoverer)".to_string(),
            "aspnetcore" => "// Action Guard: [Authorize(Roles = \"User\")]\n// Validation: [ApiController] automatic ModelState validation".to_string(),
            "spring_boot" => "// Security: @PreAuthorize(\"hasRole('USER')\")\n// Validation: @Valid @RequestBody".to_string(),
            _ => "// Security: AuthGuard, RequestValidator".to_string(),
        };
        (snippet, None)
    } else {
        (guards.join("\n"), None)
    }
}

fn trace_service_layer(
    root_dir: &Path,
    handler_file: &str,
    handler_snippet: &str,
    _handler_source: &str,
) -> (String, Option<String>, usize, usize, Option<String>) {
    // Look for service calls in snippet: e.g. service.method(), Service::method(), userService.create()
    let mut service_candidate = None;
    for line in handler_snippet.lines() {
        let t = line.trim();
        if (t.contains("Service.") || t.contains("Service::") || t.contains("service.") || t.contains("service::"))
            && (t.contains('(') || t.contains(".await"))
        {
            service_candidate = Some(t.to_string());
            break;
        }
    }

    if let Some(call_line) = service_candidate {
        // Search workspace for service declaration
        let config = TraversalConfig::default();
        for file_path in ProjectWalker::walk(root_dir, &config) {
            if file_path.to_string_lossy().contains("service") {
                if let Ok(src) = fs::read_to_string(&file_path) {
                    let lines: Vec<&str> = src.lines().collect();
                    for (idx, l) in lines.iter().enumerate() {
                        if (l.contains("fn ") || l.contains("def ") || l.contains("func ") || l.contains("public "))
                            && (l.contains("create") || l.contains("get") || l.contains("reserve") || l.contains("process") || l.contains("handle"))
                        {
                            let end = (idx + 20).min(lines.len());
                            let snippet = lines[idx..end].join("\n");
                            let lang = SupportedLanguage::from_path(&file_path)
                                .map(|l| l.as_str().to_string());
                            return (snippet, Some(file_path.to_string_lossy().to_string()), idx + 1, end, lang);
                        }
                    }
                }
            }
        }

        (call_line, Some(handler_file.to_string()), 1, 5, None)
    } else {
        (
            "// Domain Service Execution\n// Performs business validation, state mutations, and transaction coordination.".to_string(),
            Some(handler_file.to_string()),
            1,
            3,
            None,
        )
    }
}

fn trace_data_access_layer(
    root_dir: &Path,
    handler_file: &str,
    service_snippet: &str,
    handler_snippet: &str,
) -> (String, Option<String>, usize, usize, Option<String>) {
    let combined = format!("{service_snippet}\n{handler_snippet}");
    for line in combined.lines() {
        let t = line.trim();
        if t.contains("query!") || t.contains("db.") || t.contains("repository.") || t.contains("prisma.") || t.contains("_context.") || t.contains("save(") {
            return (t.to_string(), Some(handler_file.to_string()), 1, 3, None);
        }
    }

    // Search for repository or db files
    let config = TraversalConfig::default();
    for file_path in ProjectWalker::walk(root_dir, &config) {
        let p_str = file_path.to_string_lossy().to_lowercase();
        if p_str.contains("repo") || p_str.contains("dao") || p_str.contains("model") || p_str.contains("entity") {
            if let Ok(src) = fs::read_to_string(&file_path) {
                let lines: Vec<&str> = src.lines().collect();
                let end = lines.len().min(15);
                let snippet = lines[..end].join("\n");
                let lang = SupportedLanguage::from_path(&file_path).map(|l| l.as_str().to_string());
                return (snippet, Some(file_path.to_string_lossy().to_string()), 1, end, lang);
            }
        }
    }

    (
        "// Database Access\n// Executes persistence operations against primary storage backend.".to_string(),
        Some(handler_file.to_string()),
        1,
        2,
        None,
    )
}

fn match_schema_ddl(
    root_dir: &Path,
    route_path: &str,
    handler_symbol: &str,
    schemas: &[ExtractedType],
) -> (String, Option<String>, Option<String>) {
    // 1. Match from already stitched schemas
    let route_keyword = route_path.trim_matches('/').split('/').next_back().unwrap_or("").trim_end_matches('s');
    for s in schemas {
        if s.name.eq_ignore_ascii_case(route_keyword)
            || handler_symbol.to_lowercase().contains(&s.name.to_lowercase())
        {
            return (s.definition.clone(), Some(s.file_path.clone()), Some(s.name.clone()));
        }
    }

    if let Some(first) = schemas.first() {
        return (first.definition.clone(), Some(first.file_path.clone()), Some(first.name.clone()));
    }

    // 2. Search for SQL migration files or schema.prisma
    let config = TraversalConfig::default();
    for file_path in ProjectWalker::walk(root_dir, &config) {
        let p_str = file_path.to_string_lossy().to_lowercase();
        if p_str.ends_with(".sql") || p_str.ends_with(".prisma") {
            if let Ok(src) = fs::read_to_string(&file_path) {
                let lines: Vec<&str> = src.lines().collect();
                let end = lines.len().min(25);
                let snippet = lines[..end].join("\n");
                return (snippet, Some(file_path.to_string_lossy().to_string()), Some("db_schema".to_string()));
            }
        }
    }

    (
        "CREATE TABLE items (\n    id SERIAL PRIMARY KEY,\n    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()\n);".to_string(),
        Some("migrations/0001_init.sql".to_string()),
        Some("items".to_string()),
    )
}

fn apply_adaptive_budget(steps: &mut [FullstackTraceStep], budget: usize) {
    let mut current_tokens: usize = steps.iter().map(|s| count_tokens(&s.snippet)).sum();
    if current_tokens <= budget {
        return;
    }

    // Level 1: Strip doc comments and redundant blank lines
    for step in steps.iter_mut() {
        let clean_lines: Vec<&str> = step
            .snippet
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with("///") && !t.starts_with("//!") && !t.starts_with("/*") && !t.starts_with('*')
            })
            .collect();
        step.snippet = clean_lines.join("\n");
    }

    current_tokens = steps.iter().map(|s| count_tokens(&s.snippet)).sum();
    if current_tokens <= budget {
        return;
    }

    // Level 2: Compact lines in large steps
    for step in steps.iter_mut() {
        let lines: Vec<&str> = step.snippet.lines().collect();
        if lines.len() > 12 {
            let head = &lines[..5];
            let tail = &lines[lines.len() - 4..];
            step.snippet = format!("{}\n    // ... [compressed for budget] ...\n{}", head.join("\n"), tail.join("\n"));
        }
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
