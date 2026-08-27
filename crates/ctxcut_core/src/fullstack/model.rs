//! Data models, trace steps, and traits for full-stack cross-boundary execution tracing.

use crate::error::Result;
use crate::model::{ExtractedType, TokenStats};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Detected frontend API invocation or RPC client call in TypeScript/JavaScript/Vue/Svelte/Astro source files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientApiCall {
    /// Client framework/library kind: `"fetch"`, `"axios"`, `"react_query"`, `"trpc"`, `"graphql"`, `"grpc_web"`.
    pub client_kind: String,
    /// HTTP method if applicable (e.g. `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`).
    pub http_method: Option<String>,
    /// Target endpoint URL or path pattern (e.g. `"/api/users"`, `"/api/v1/orders/${id}"`).
    pub endpoint_url: Option<String>,
    /// RPC procedure or query name (e.g. `"user.getById"`, `"UserService.GetUser"`, `"GetUsers"`).
    pub rpc_procedure: Option<String>,
    /// Source file path where the client invocation was detected.
    pub file_path: String,
    /// 1-based line number of the client invocation.
    pub line_number: usize,
    /// Verbatim code snippet of the client API call.
    pub call_snippet: String,
    /// Inferred or explicit request DTO type name.
    pub request_dto: Option<String>,
    /// Inferred or explicit response DTO type name.
    pub response_dto: Option<String>,
}

/// Resolved backend server route entrypoint and controller handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerRouteEndpoint {
    /// Server framework: `"axum"`, `"actix"`, `"gin"`, `"chi"`, `"fastapi"`, `"flask"`, `"aspnetcore"`, `"spring_boot"`, `"express"`, `"nestjs"`.
    pub framework: String,
    /// HTTP method (e.g. `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"`, or `"RPC"`).
    pub http_method: String,
    /// Registered route path pattern (e.g. `"/api/users"`, `"/api/users/:id"`, `"/items/{id}"`).
    pub route_path: String,
    /// Path to the source file where the route handler is defined.
    pub handler_file: String,
    /// Identifier name of the controller / handler function or method.
    pub handler_symbol: String,
    /// Complete signature header of the handler.
    pub handler_signature: String,
    /// Extracted or hoisted request DTO model.
    pub request_dto_type: Option<ExtractedType>,
    /// Extracted or hoisted response DTO model.
    pub response_dto_type: Option<ExtractedType>,
}

/// Single discrete step in the 6-step cross-boundary linear execution trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullstackTraceStep {
    /// 1-based sequential step index (1 through 6).
    pub step_number: usize,
    /// Layer category: `"client_call"`, `"route_handler"`, `"middleware_guard"`, `"service_logic"`, `"data_access"`, `"schema_ddl"`.
    pub layer: String,
    /// Human-readable step title (e.g. `"Frontend Invocation (fetch)"`, `"Axum Route Handler: create_user"`).
    pub title: String,
    /// Source file path where this step originates.
    pub file_path: String,
    /// 1-based start line in source file.
    pub start_line: usize,
    /// 1-based end line in source file.
    pub end_line: usize,
    /// Programming language / syntax tag (e.g. `"typescript"`, `"rust"`, `"python"`, `"go"`, `"csharp"`, `"java"`, `"sql"`).
    pub language: String,
    /// Verbatim or compressed code snippet.
    pub snippet: String,
    /// Associated DTO or database schema contract, if available.
    pub schema_contract: Option<String>,
}

/// Complete full-stack cross-boundary execution trace result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullstackTraceResult {
    /// Queried endpoint or procedure string.
    pub query_endpoint: String,
    /// Linked client API call if discovered in workspace.
    pub client_call: Option<ClientApiCall>,
    /// Matched server route handler.
    pub server_route: ServerRouteEndpoint,
    /// Ordered linear execution steps (up to 6 layers).
    pub steps: Vec<FullstackTraceStep>,
    /// Total number of execution steps captured.
    pub total_steps: usize,
    /// Token reduction and compression statistics.
    pub stats: TokenStats,
}

impl FullstackTraceResult {
    /// Formats the full-stack trace result as prompt-optimized Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Full-Stack Execution Trace: {}\n\n", self.query_endpoint));
        out.push_str(&format!("- **Framework**: {}\n", self.server_route.framework));
        out.push_str(&format!("- **HTTP Method**: {}\n", self.server_route.http_method));
        out.push_str(&format!("- **Route Path**: {}\n", self.server_route.route_path));
        out.push_str(&format!("- **Handler**: `{}` ({}:{})\n", self.server_route.handler_symbol, self.server_route.handler_file, self.server_route.handler_signature));
        if let Some(client) = &self.client_call {
            out.push_str(&format!("- **Client Call**: {} at {}:{}\n", client.client_kind, client.file_path, client.line_number));
        }
        out.push_str(&format!("- **Tokens**: {} (raw: {}, savings: {:.1}%)\n\n", self.stats.sliced_tokens, self.stats.raw_file_tokens, self.stats.savings_percentage));

        for step in &self.steps {
            out.push_str(&format!("## Step {}: {} (`{}`)\n", step.step_number, step.title, step.layer));
            out.push_str(&format!("**File**: `{}:{}-{}`\n\n", step.file_path, step.start_line, step.end_line));
            out.push_str(&format!("```{}\n{}\n```\n\n", step.language, step.snippet.trim()));
            if let Some(contract) = &step.schema_contract {
                out.push_str(&format!("**Schema Contract**:\n```\n{}\n```\n\n", contract.trim()));
            }
        }
        out
    }

    /// Formats the result as pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Trait for full-stack cross-boundary API execution tracing.
pub trait FullstackTracer: Send + Sync {
    /// Traces end-to-end execution flow from client calls to backend route handlers, middleware, service logic, data access, and DB DDL.
    fn trace_api(
        &self,
        root_dir: &Path,
        endpoint_or_proc: &str,
        budget: Option<usize>,
    ) -> Result<FullstackTraceResult>;
}
