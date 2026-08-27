//! `ctxcut_mcp` — Model Context Protocol (MCP) STDIO JSON-RPC 2.0 server.
//!
//! Exposes 6 AST context tools (`get_symbol_slice`, `get_diff_slice`, `analyze_token_stats`,
//! `patch_symbol`, `get_test_context`, `get_route_slice`) to AI coding agents
//! over clean STDIO framing with structured JSONL file logging, timeout safety, and observability.

pub mod logger;

pub use logger::{format_rfc3339, McpFileLogger, ToolLogRecord};

use anyhow::Result;
use ctxcut_core::refactor::batch::{
    BatchAstPatcher, PatchTransactionRequest, SymbolPatchUnit,
};
use ctxcut_core::{
    AstPatcher, AstQueryEngine, ContextSlicer, DefaultIntentSlicer, DefaultSwarmPartitioner,
    ExecutionTracer, FullstackExecutionTracer, ImpactAnalyzer, IndexEngine,
    IndexOptions, IntentSliceOptions, IntentSlicer, MarkdownFormatter, OverviewOptions,
    PatchVerifier, SemanticDiffEngine, SliceOptions, SliceResult, SupportedLanguage,
    SwarmPartitionEngine, SymbolRenamer, TelemetryLogger, TestContextGenerator,
    WorkspaceOverviewGenerator,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

/// Default execution timeout deadline for MCP tool calls (10,000 milliseconds = 10 seconds).
pub const DEFAULT_TOOL_TIMEOUT_MS: u64 = 10_000;

/// Configuration options for the MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerOptions {
    /// Optional destination path for structured JSONL logging.
    pub log_file: Option<PathBuf>,
    /// Tool execution timeout in milliseconds (default: 10,000ms).
    pub tool_timeout_ms: Option<u64>,
}

impl Default for McpServerOptions {
    fn default() -> Self {
        Self {
            log_file: None,
            tool_timeout_ms: Some(DEFAULT_TOOL_TIMEOUT_MS),
        }
    }
}

/// Runs the Model Context Protocol (MCP) server over STDIO with the provided options.
pub fn run_mcp_server(options: McpServerOptions) -> Result<()> {
    let McpServerOptions {
        log_file,
        tool_timeout_ms,
    } = options;
    let logger = McpFileLogger::new(log_file);
    logger.log_start();

    let server_timeout_ms = tool_timeout_ms.unwrap_or(DEFAULT_TOOL_TIMEOUT_MS);

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let start_time = Instant::now();

        let Ok(request) = serde_json::from_str::<Value>(trimmed) else {
            logger.log_rpc_error(None, "parse_error", -32700, "Parse error: invalid JSON");
            let err_response = json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {
                    "code": -32700,
                    "message": "Parse error: invalid JSON"
                }
            });
            let mut out_str = serde_json::to_string(&err_response)?;
            out_str.push('\n');
            stdout.write_all(out_str.as_bytes())?;
            stdout.flush()?;
            continue;
        };

        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let id = request.get("id").cloned();

        // Notification: initialized or exit (do not send JSON-RPC response)
        if method.starts_with("notifications/") || method == "initialized" || method == "exit" {
            logger.log_request(method, None, None, None);
            continue;
        }

        let (response, tokens_saved, error_opt) =
            handle_mcp_request(method, &request, &logger, id.as_ref(), server_timeout_ms);

        let duration = start_time.elapsed();
        let duration_ms_u128 = duration.as_millis();

        // Log JSON-RPC response event
        logger.log_response(
            method,
            id.as_ref(),
            duration_ms_u128,
            tokens_saved,
            error_opt.as_deref(),
        );

        let response_payload = if response.get("error").is_some() {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": response.get("error")
            })
        } else {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": response.get("result").cloned().unwrap_or(response)
            })
        };

        let mut out_str = serde_json::to_string(&response_payload)?;
        out_str.push('\n');
        stdout.write_all(out_str.as_bytes())?;
        stdout.flush()?;
    }

    Ok(())
}

/// Runs the Model Context Protocol (MCP) server over STDIO with default options.
pub fn run_mcp_server_default() -> Result<()> {
    run_mcp_server(McpServerOptions::default())
}

fn handle_mcp_request(
    method: &str,
    req: &Value,
    logger: &McpFileLogger,
    id: Option<&Value>,
    server_timeout_ms: u64,
) -> (Value, Option<usize>, Option<String>) {
    match method {
        "initialize" => {
            logger.log_request(method, id, None, None);
            (
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "ctxcut-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
                None,
                None,
            )
        }

        "tools/list" => {
            logger.log_request(method, id, None, None);
            (build_tools_list_response(), None, None)
        }

        "tools/call" => handle_tools_call(req, logger, id, server_timeout_ms),

        _ => {
            let err_msg = format!("Method not found: `{method}`");
            logger.log_rpc_error(id, method, -32601, &err_msg);
            (
                json!({
                    "error": {
                        "code": -32601,
                        "message": err_msg
                    }
                }),
                None,
                Some(err_msg),
            )
        }
    }
}

fn build_tools_list_response() -> Value {
    json!({
        "tools": [
            {
                "name": "get_symbol_slice",
                "description": "Extracts AST-accurate slice of target function(s), method(s), or class(es) with hoisted types, stripped signatures, and optional token budgeting. Supports comma-separated multi-symbol batching with unified type deduplication.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Source file path"
                        },
                        "symbol": {
                            "type": "string",
                            "description": "Target symbol name(s) (e.g. `calculateTax`, `OrderService.refund`, or comma-separated `processOrder,validateToken`)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional token budget limit for progressive semantic degradation"
                        },
                        "depth": {
                            "type": "integer",
                            "description": "Type hoisting recursion depth (default: 1)"
                        },
                        "no_types": {
                            "type": "boolean",
                            "description": "Disable type hoisting (default: false)"
                        },
                        "no_calls": {
                            "type": "boolean",
                            "description": "Disable signature stripping for external calls (default: false)"
                        }
                    },
                    "required": ["path", "symbol"]
                }
            },
            {
                "name": "get_workspace_overview",
                "description": "Indexes workspace symbols and generates a token-dense architectural outline of all declarations, interfaces, and routes without parsing entire file bodies",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Workspace root directory path (defaults to current directory)"
                        },
                        "depth": {
                            "type": "integer",
                            "description": "Maximum directory traversal depth (default: unlimited)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional token budget limit for compressed repository overview"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        },
                        "include_routes": {
                            "type": "boolean",
                            "description": "Whether to detect and index web framework routes (default: true)"
                        }
                    }
                }
            },
            {
                "name": "get_metrics",
                "description": "Inspects cumulative token reduction telemetry, ROI analytics, language breakdowns, and estimated API cost savings",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default dashboard) or 'json' (raw metrics payload)",
                            "enum": ["markdown", "text", "json"]
                        },
                        "clear": {
                            "type": "boolean",
                            "description": "Clear persistent telemetry history (default: false)"
                        }
                    }
                }
            },
            {
                "name": "get_diff_slice",
                "description": "Extracts slices for all functions modified in Git diff or staged changes",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Optional repository path (defaults to current working directory)"
                        },
                        "staged": {
                            "type": "boolean",
                            "description": "Whether to inspect staged changes only (default: false)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional token budget limit per slice"
                        }
                    }
                }
            },
            {
                "name": "analyze_token_stats",
                "description": "Calculates repository or file token savings and optimization statistics, or inspects lifetime history",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File or directory path to analyze (optional when history=true)"
                        },
                        "fast": {
                            "type": "boolean",
                            "description": "Enable shallow fast estimation scan mode without deep AST slicing (default: true for directories, false for single files)"
                        },
                        "history": {
                            "type": "boolean",
                            "description": "Display persistent lifetime telemetry history and ROI dashboard (default: false)"
                        }
                    }
                }
            },
            {
                "name": "patch_symbol",
                "description": "Surgically replaces a function, method, or class in source code using AST node alignment with syntax validation",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Source file path"
                        },
                        "symbol": {
                            "type": "string",
                            "description": "Target symbol name to replace"
                        },
                        "code": {
                            "type": "string",
                            "description": "Replacement code"
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "Preview unified diff without writing changes to disk (default: false)"
                        }
                    },
                    "required": ["path", "symbol", "code"]
                }
            },
            {
                "name": "get_test_context",
                "description": "Generates isolated unit test context with mock scaffolding, AAA test templates, and nearby reference fixtures",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Source file path"
                        },
                        "symbol": {
                            "type": "string",
                            "description": "Target symbol name to generate test context for"
                        },
                        "framework": {
                            "type": "string",
                            "description": "Test runner / framework (e.g. vitest, jest, pytest, cargo, gotest)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional token budget limit"
                        }
                    },
                    "required": ["path", "symbol"]
                }
            },
            {
                "name": "get_route_slice",
                "description": "Resolves web, IPC, and RPC framework route handlers, controllers, DTOs, and procedures (Tauri #[tauri::command], Electron ipcMain, tRPC, Next.js Server Actions, Express, FastAPI, Gin, Axum, Actix, Spring, ASP.NET Core)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "method": {
                            "type": "string",
                            "description": "HTTP, IPC, or RPC Method (GET, POST, PUT, DELETE, IPC, ACTION, QUERY, MUTATION, etc. Defaults to 'ANY' if omitted)"
                        },
                        "path": {
                            "type": "string",
                            "description": "Route URL path, Electron channel, Tauri command, or RPC procedure (e.g. `/api/v1/checkout`, `calculate_tax`, `dialog:openFile`, `user.getById`, `updateProfile`)"
                        },
                        "root_dir": {
                            "type": "string",
                            "description": "Root workspace directory to search within (defaults to current directory)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional token budget limit"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "get_impact_slice",
                "description": "Performs upstream caller and reverse impact analysis across the workspace to locate all call sites and enclosing functions consuming a target symbol.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Target symbol name to trace callers for (e.g. `validate_token`, `AuthService.validate`)"
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional path to the file declaring the target symbol"
                        },
                        "root_dir": {
                            "type": "string",
                            "description": "Workspace root directory to search within (defaults to current directory)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional adaptive token budget limit"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of caller sites to return"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Optional execution timeout in milliseconds (default: 10000)"
                        }
                    },
                    "required": ["target"]
                }
            },
            {
                "name": "get_trace_slice",
                "description": "Traces end-to-end execution flow from an entry point (HTTP route, CLI main, or controller symbol) down to services and database layers within a progressive 1,000–2,000 token budget, pruning irrelevant sibling branches.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entry_point": {
                            "type": "string",
                            "description": "Entry point query (e.g. `POST /api/v1/orders`, `main`, or `OrderController.createOrder`)"
                        },
                        "root_dir": {
                            "type": "string",
                            "description": "Workspace root directory to search within (defaults to current directory)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional token budget limit (default: 1500 tokens)"
                        },
                        "depth": {
                            "type": "integer",
                            "description": "Maximum call chain depth hops (default: 8)"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Optional execution timeout in milliseconds (default: 10000)"
                        }
                    },
                    "required": ["entry_point"]
                }
            },
            {
                "name": "verify_patch",
                "description": "Applies a code replacement to a target symbol with Tree-Sitter syntax validation, triggers language typecheckers (cargo check, tsc, mypy, go vet, dotnet build, javac) with RAII auto-rollback safety, and returns a structured verification report.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Target symbol in `path/to/file:symbol` format (e.g. `src/math.rs:compute`)"
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional source file path if not included in target"
                        },
                        "symbol": {
                            "type": "string",
                            "description": "Optional target symbol name if not included in target"
                        },
                        "new_code": {
                            "type": "string",
                            "description": "Replacement code to splice into the AST symbol"
                        },
                        "code": {
                            "type": "string",
                            "description": "Alias for `new_code`"
                        },
                        "typechecker": {
                            "type": "string",
                            "description": "Optional custom typechecker command override (e.g. `cargo check`, `npx tsc --noEmit`)"
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "Whether to perform a dry-run without persisting changes to disk (default: true)"
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Optional typechecker execution timeout in milliseconds (default: 30000)"
                        }
                    }
                }
            },
            {
                "name": "semantic_diff",
                "description": "Performs token-efficient structural AST diff comparing working tree or staged changes against Git baseline, detecting added/removed/modified functions, classes, interfaces, signature changes, and token ROI savings.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Workspace root directory path (defaults to current directory)"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Optional path to a specific file to diff"
                        },
                        "staged": {
                            "type": "boolean",
                            "description": "Whether to inspect staged changes only (default: false)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional token budget limit for progressive semantic degradation"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        }
                    }
                }
            },
            {
                "name": "refactor_rename",
                "description": "Performs AST-accurate multi-file symbol renaming across the workspace, updating declarations, usage call sites, imports, and re-exports with pre-write syntax validation and dry-run preview.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Target symbol query (e.g. `src/calc.rs:calculate_tax` or `calculateTax`)"
                        },
                        "new_name": {
                            "type": "string",
                            "description": "New identifier name (e.g. `compute_tax`)"
                        },
                        "root_dir": {
                            "type": "string",
                            "description": "Workspace root directory (defaults to current directory)"
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "Preview diff without writing changes to disk (default: false)"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        }
                    },
                    "required": ["target", "new_name"]
                }
            },
            {
                "name": "index_workspace",
                "description": "Builds, updates, or checks the persistent SQLite index (.ctxcut/index.db) for the workspace. Enables sub-5ms incremental indexing, instant symbol lookups, caller discovery, and workspace overview queries.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Workspace root directory path (defaults to current working directory)"
                        },
                        "rebuild": {
                            "type": "boolean",
                            "description": "Force a complete index rebuild from scratch (default: false)"
                        },
                        "status_only": {
                            "type": "boolean",
                            "description": "Check index status and health without re-indexing (default: false)"
                        }
                    }
                }
            },
            {
                "name": "query_ast",
                "description": "Performs structural Tree-sitter AST queries across workspace files using custom S-expression patterns or built-in presets (functions, structs, classes, interfaces, enums, exports, async_fns, api_routes, errors, react_hooks).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Custom Tree-sitter S-expression query pattern (e.g. `(function_item name: (identifier) @name)`)"
                        },
                        "preset": {
                            "type": "string",
                            "description": "Built-in AST query preset name (`functions`, `structs`, `classes`, `interfaces`, `enums`, `exports`, `async_fns`, `api_routes`, `errors`, `react_hooks`)",
                            "enum": [
                                "functions",
                                "structs",
                                "classes",
                                "interfaces",
                                "enums",
                                "exports",
                                "async_fns",
                                "api_routes",
                                "errors",
                                "react_hooks"
                            ]
                        },
                        "language": {
                            "type": "string",
                            "description": "Optional programming language filter (e.g. `rust`, `typescript`, `javascript`, `python`, `go`, `c`, `cpp`, `csharp`, `java`, `kotlin`, `vue`, `svelte`, `astro`)"
                        },
                        "root_dir": {
                            "type": "string",
                            "description": "Workspace root directory path to search within (defaults to current directory)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of matches to return (default: unlimited)"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        }
                    }
                }
            },
            {
                "name": "get_fullstack_trace",
                "description": "Traces end-to-end cross-boundary execution flows connecting client-side API calls, server route endpoints, controller actions, service logic, repository queries, and database schemas (DDL/Prisma/SQL).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entry": {
                            "type": "string",
                            "description": "Entry point identifier (e.g. `POST /api/v1/orders`, `src/client.ts:createOrder`, or `billing.chargeInvoice`)"
                        },
                        "root_dir": {
                            "type": "string",
                            "description": "Workspace root directory path to search within (defaults to current directory)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Optional token budget limit for degraded trace output (default: 1500 tokens)"
                        },
                        "max_depth": {
                            "type": "integer",
                            "description": "Configurable traversal depth and hop bounding (3..5, default: 5)",
                            "minimum": 3,
                            "maximum": 5
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        }
                    },
                    "required": ["entry"]
                }
            },
            {
                "name": "get_intent_slice",
                "description": "Extracts high-density semantic context slice matching natural language task intent using hybrid BM25 lexical ranking and Tree-sitter AST dependency expansion.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Natural language task or intent prompt describing desired functionality"
                        },
                        "root_dir": {
                            "type": "string",
                            "description": "Workspace root directory path to search within (defaults to current directory)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Target token budget limit (default: 1500 tokens)"
                        },
                        "max_symbols": {
                            "type": "integer",
                            "description": "Maximum number of primary target symbols to extract (default: 5)"
                        },
                        "depth": {
                            "type": "integer",
                            "description": "AST dependency traversal depth (default: 1)"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        }
                    },
                    "required": ["prompt"]
                }
            },
            {
                "name": "patch_transaction",
                "description": "Executes atomic multi-symbol, multi-file AST refactoring transactions with compiler dry-run verification and automatic rollback on syntax/type errors.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "patches": {
                            "type": "array",
                            "description": "Array of symbol patch units to apply atomically across workspace files",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "file_path": { "type": "string", "description": "Target source file path" },
                                    "symbol_name": { "type": "string", "description": "Target symbol identifier" },
                                    "replacement_code": { "type": "string", "description": "New replacement code snippet" },
                                    "expected_old_hash": { "type": "string", "description": "Optional SHA-256 hash of existing symbol body for CAS concurrency check" }
                                },
                                "required": ["file_path", "symbol_name", "replacement_code"]
                            }
                        },
                        "root_dir": {
                            "type": "string",
                            "description": "Workspace root directory path (defaults to current directory)"
                        },
                        "typechecker": {
                            "type": "string",
                            "description": "Optional custom typechecker command override (e.g. `cargo check`, `tsc --noEmit`)"
                        },
                        "apply": {
                            "type": "boolean",
                            "description": "Whether to persist changes to disk on success (default: false for dry-run verification)"
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Optional typechecker execution timeout in milliseconds (default: 30000)"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json", "text"]
                        }
                    },
                    "required": ["patches"]
                }
            },
            {
                "name": "pack_agent_context",
                "description": "Partitions workspace into $K$ isolated, non-overlapping AST context clusters for multi-agent swarms with write authority tagging and mock contract synthesis.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root_dir": {
                            "type": "string",
                            "description": "Workspace root directory path (defaults to current directory)"
                        },
                        "agents_count": {
                            "type": "integer",
                            "description": "Total number of agent clusters to partition workspace into (default: 2)"
                        },
                        "seed_symbols": {
                            "type": "array",
                            "description": "Optional seed symbol names to anchor cluster centroids",
                            "items": { "type": "string" }
                        },
                        "budget_per_agent": {
                            "type": "integer",
                            "description": "Target token budget limit per individual agent bundle (default: 1500 tokens)"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'markdown' (default) or 'json'",
                            "enum": ["markdown", "json"]
                        }
                    }
                }
            }
        ]
    })
}

fn handle_tools_call(
    req: &Value,
    logger: &McpFileLogger,
    id: Option<&Value>,
    server_timeout_ms: u64,
) -> (Value, Option<usize>, Option<String>) {
    let params = req.get("params").unwrap_or(&Value::Null);
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").unwrap_or(&Value::Null);

    logger.log_request("tools/call", id, Some(tool_name), Some(args));

    let effective_timeout_ms = args
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            std::env::var("CTXCUT_MCP_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(server_timeout_ms)
        });

    let start_tool = Instant::now();
    let (response, tool_metrics, error_opt, tokens_saved) =
        execute_tool_with_timeout(tool_name, args, effective_timeout_ms);
    let duration_ms = start_tool.elapsed().as_secs_f64() * 1000.0;

    let status = if error_opt.is_some() || response.get("isError") == Some(&json!(true)) {
        "error"
    } else {
        "success"
    };

    logger.log_tool_execution(&ToolLogRecord {
        id,
        tool: tool_name,
        args,
        duration_ms,
        status,
        metrics: tool_metrics.as_ref(),
        error: error_opt.as_deref(),
    });

    (response, tokens_saved, error_opt)
}

/// Executes an MCP tool call inside a thread boundary guarded with a timeout and panic boundary.
pub fn execute_tool_with_timeout(
    name: &str,
    args: &Value,
    timeout_ms: u64,
) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let name_owned = name.to_string();
    let args_owned = args.clone();

    let spawn_res = std::thread::Builder::new()
        .name(format!("mcp-worker-{name}"))
        .spawn(move || {
            let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute_tool_call(&name_owned, &args_owned)
            }));
            let _ = tx.send(panic_res);
        });

    if let Err(e) = spawn_res {
        let err_msg = format!("Failed to spawn worker thread: {e}");
        let response = json!({
            "isError": true,
            "content": [{ "type": "text", "text": err_msg }]
        });
        return (response, None, Some(err_msg), None);
    }

    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(Ok(tool_result)) => tool_result,
        Ok(Err(_panic_payload)) => {
            let err_msg =
                format!("Internal error: unexpected panic during tool `{name}` execution");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err_msg }]
            });
            (response, None, Some(err_msg), None)
        }
        Err(RecvTimeoutError::Timeout) => {
            let suggestion = match name {
                "analyze_token_stats" => {
                    "Pass `\"fast\": true` for rapid repository-wide estimation or specify a narrower subdirectory."
                }
                "get_diff_slice" => {
                    "Specify a narrower path or review staged changes only with `\"staged\": true`."
                }
                _ => "Try narrowing the query target or checking repository size.",
            };
            let timeout_msg = format!(
                "⏳ Timeout: Tool `{name}` execution timed out after {timeout_ms}ms.\nSuggestion: {suggestion}"
            );
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": timeout_msg }],
                "timeout": {
                    "tool": name,
                    "timeout_ms": timeout_ms,
                    "suggestion": suggestion
                }
            });
            (
                response,
                None,
                Some(format!(
                    "Tool `{name}` execution timed out after {timeout_ms}ms"
                )),
                None,
            )
        }
        Err(RecvTimeoutError::Disconnected) => {
            let err_msg = format!("Worker thread disconnected unexpectedly during tool `{name}`");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err_msg }]
            });
            (response, None, Some(err_msg), None)
        }
    }
}

fn execute_tool_call(
    name: &str,
    args: &Value,
) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    match name {
        "get_symbol_slice" => execute_symbol_slice(args),
        "get_workspace_overview" => execute_workspace_overview(args),
        "get_metrics" => execute_metrics(args),
        "get_diff_slice" => execute_diff_slice(args),
        "analyze_token_stats" => execute_stats_slice(args),
        "patch_symbol" => execute_patch_symbol(args),
        "get_test_context" => execute_get_test_context(args),
        "get_route_slice" => execute_get_route_slice(args),
        "get_impact_slice" => execute_impact_slice(args),
        "get_trace_slice" => execute_trace_slice(args),
        "verify_patch" => execute_verify_patch(args),
        "semantic_diff" => execute_semantic_diff(args),
        "refactor_rename" => execute_refactor_rename(args),
        "index_workspace" => execute_index_workspace(args),
        "query_ast" => execute_query_ast(args),
        "get_fullstack_trace" => execute_fullstack_trace(args),
        "get_intent_slice" => execute_intent_slice(args),
        "patch_transaction" => execute_patch_transaction(args),
        "pack_agent_context" => execute_pack_agent_context(args),
        _ => {
            let err = format!("Unknown tool: `{name}`");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_symbol_slice(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let Some(file_path_str) = args.get("path").and_then(Value::as_str) else {
        let err = "Missing required parameter 'path'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let symbols_vec: Vec<String> = if let Some(sym_str) = args.get("symbol").and_then(Value::as_str)
    {
        sym_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else if let Some(sym_arr) = args.get("symbols").and_then(Value::as_array) {
        sym_arr
            .iter()
            .filter_map(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        let err = "Missing required parameter 'symbol'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    if symbols_vec.is_empty() {
        let err = "No valid symbol name provided in parameter 'symbol'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    }

    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);
    let depth = args.get("depth").and_then(Value::as_u64).unwrap_or(1) as usize;
    let no_types = args
        .get("no_types")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let no_calls = args
        .get("no_calls")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let opts = SliceOptions {
        depth,
        include_types: !no_types,
        include_calls: !no_calls,
        budget,
    };

    let start_time = Instant::now();
    let slicer = ContextSlicer::new();

    if symbols_vec.len() > 1 {
        let sym_refs: Vec<&str> = symbols_vec.iter().map(|s| s.as_str()).collect();
        match slicer.slice_batch(Path::new(file_path_str), &sym_refs, &opts) {
            Ok(batch) => {
                #[allow(clippy::cast_possible_truncation)]
                let duration_ms = start_time.elapsed().as_millis() as u64;
                for sym in &batch.target_symbols {
                    let single = SliceResult {
                        target_symbol: sym.clone(),
                        hoisted_types: Vec::new(),
                        hoisted_implementors: Vec::new(),
                        stripped_calls: Vec::new(),
                        stats: batch.stats.clone(),
                    };
                    TelemetryLogger::record_slice(
                        &single,
                        "mcp_get_symbol_slice",
                        Some(duration_ms),
                    );
                }

                let raw_tokens = batch.stats.raw_file_tokens;
                let sliced_tokens = batch.stats.sliced_tokens;
                let saved_tokens = raw_tokens.saturating_sub(sliced_tokens);
                let metrics = json!({
                    "raw_tokens": raw_tokens,
                    "sliced_tokens": sliced_tokens,
                    "saved_tokens": saved_tokens,
                    "savings_percentage": batch.stats.savings_percentage,
                    "raw_lines": batch.stats.raw_lines,
                    "sliced_lines": batch.stats.sliced_lines,
                    "symbols_count": batch.target_symbols.len()
                });
                let response = json!({
                    "content": [{ "type": "text", "text": batch.to_markdown() }],
                    "slice": batch
                });
                (response, Some(metrics), None, Some(saved_tokens))
            }
            Err(e) => {
                let err = format!("Batch slicing error: {e}");
                let response = json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": err }]
                });
                (response, None, Some(err), None)
            }
        }
    } else {
        match slicer.slice_symbol(Path::new(file_path_str), &symbols_vec[0], &opts) {
            Ok(slice) => {
                #[allow(clippy::cast_possible_truncation)]
                let duration_ms = start_time.elapsed().as_millis() as u64;
                TelemetryLogger::record_slice(&slice, "mcp_get_symbol_slice", Some(duration_ms));

                let raw_tokens = slice.stats.raw_file_tokens;
                let sliced_tokens = slice.stats.sliced_tokens;
                let saved_tokens = raw_tokens.saturating_sub(sliced_tokens);
                let metrics = json!({
                    "raw_tokens": raw_tokens,
                    "sliced_tokens": sliced_tokens,
                    "saved_tokens": saved_tokens,
                    "savings_percentage": slice.stats.savings_percentage,
                    "raw_lines": slice.stats.raw_lines,
                    "sliced_lines": slice.stats.sliced_lines
                });
                let response = json!({
                    "content": [{ "type": "text", "text": MarkdownFormatter::format(&slice) }],
                    "slice": slice
                });
                (response, Some(metrics), None, Some(saved_tokens))
            }
            Err(e) => {
                let err = format!("Slicing error: {e}");
                let response = json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": err }]
                });
                (response, None, Some(err), None)
            }
        }
    }
}

fn execute_diff_slice(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let staged = args.get("staged").and_then(Value::as_bool).unwrap_or(false);
    let path_opt = args.get("path").and_then(Value::as_str);
    let repo_path = path_opt.map(Path::new);
    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);

    let opts = SliceOptions {
        budget,
        ..Default::default()
    };

    let start_time = Instant::now();
    match ctxcut_cli::run_diff_slicer_in(repo_path, staged, &opts) {
        Ok(slices) => {
            #[allow(clippy::cast_possible_truncation)]
            let duration_ms = start_time.elapsed().as_millis() as u64;
            for slice in &slices {
                TelemetryLogger::record_slice(slice, "mcp_get_diff_slice", Some(duration_ms));
            }

            let total_raw: usize = slices.iter().map(|s| s.stats.raw_file_tokens).sum();
            let total_sliced: usize = slices.iter().map(|s| s.stats.sliced_tokens).sum();
            let total_saved: usize = total_raw.saturating_sub(total_sliced);
            let total_raw_lines: usize = slices.iter().map(|s| s.stats.raw_lines).sum();
            let total_sliced_lines: usize = slices.iter().map(|s| s.stats.sliced_lines).sum();

            let savings_pct = if total_raw > 0 {
                #[allow(clippy::cast_precision_loss)]
                let pct = (total_saved as f64 / total_raw as f64) * 100.0;
                pct
            } else {
                0.0
            };

            let metrics = json!({
                "raw_tokens": total_raw,
                "sliced_tokens": total_sliced,
                "saved_tokens": total_saved,
                "savings_percentage": (savings_pct * 100.0).round() / 100.0,
                "raw_lines": total_raw_lines,
                "sliced_lines": total_sliced_lines,
                "symbols_count": slices.len()
            });

            let rendered = if slices.is_empty() {
                "No modified symbols detected in git diff.".to_string()
            } else {
                MarkdownFormatter::format_batch(&slices)
            };

            let response = json!({
                "content": [{ "type": "text", "text": rendered }]
            });

            (response, Some(metrics), None, Some(total_saved))
        }
        Err(e) => {
            let err = format!("Diff error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_stats_slice(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    if args.get("history").and_then(Value::as_bool) == Some(true) {
        return execute_metrics(args);
    }

    let Some(path_str) = args.get("path").and_then(Value::as_str) else {
        let err = "Missing required parameter 'path'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let target_path = Path::new(path_str);
    let fast = args
        .get("fast")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| target_path.is_dir());

    match ctxcut_cli::stats::calculate_stats(target_path, fast) {
        Ok(report) => {
            let saved = report
                .total_raw_tokens
                .saturating_sub(report.total_sliced_tokens);
            let metrics = json!({
                "raw_tokens": report.total_raw_tokens,
                "sliced_tokens": report.total_sliced_tokens,
                "saved_tokens": saved,
                "savings_percentage": report.savings_percentage,
                "total_files": report.total_files,
                "total_lines": report.total_lines
            });
            let response = json!({
                "content": [{ "type": "text", "text": ctxcut_cli::stats::format_stats_text(&report) }],
                "stats": report
            });
            (response, Some(metrics), None, Some(saved))
        }
        Err(e) => {
            let err = format!("Stats calculation error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_workspace_overview(
    args: &Value,
) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let path_str = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let target_root = Path::new(path_str);

    let max_depth = args
        .get("depth")
        .and_then(Value::as_u64)
        .map(|d| d as usize);
    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");
    let include_routes = args
        .get("include_routes")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let framework = args
        .get("framework")
        .and_then(Value::as_str)
        .map(String::from);

    let opts = OverviewOptions {
        budget,
        max_depth,
        include_routes,
        framework,
    };

    match WorkspaceOverviewGenerator::generate(target_root, &opts) {
        Ok(report) => {
            let raw_tokens = report.total_raw_tokens;
            let overview_tokens = report.total_overview_tokens;
            let saved_tokens = raw_tokens.saturating_sub(overview_tokens);
            let metrics = json!({
                "total_files": report.total_files,
                "total_symbols": report.total_symbols,
                "total_lines": report.total_lines,
                "raw_tokens": raw_tokens,
                "overview_tokens": overview_tokens,
                "saved_tokens": saved_tokens,
                "savings_percentage": report.token_savings_percentage
            });

            let rendered = if format_str.eq_ignore_ascii_case("json") {
                report.to_json()
            } else {
                report.to_markdown()
            };

            let response = json!({
                "content": [{ "type": "text", "text": rendered }],
                "overview": report
            });
            (response, Some(metrics), None, Some(saved_tokens))
        }
        Err(e) => {
            let err = format!("Workspace overview error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_metrics(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");
    let clear = args.get("clear").and_then(Value::as_bool).unwrap_or(false);

    let metrics_path = TelemetryLogger::resolve_metrics_path();

    if clear {
        let _ = fs::remove_file(&metrics_path);
    }

    let summary =
        TelemetryLogger::load_summary().unwrap_or_else(|_| ctxcut_core::TelemetrySummary {
            total_requests: 0,
            total_raw_tokens: 0,
            total_sliced_tokens: 0,
            total_saved_tokens: 0,
            compression_percentage: 0.0,
            estimated_cost_savings_usd: 0.0,
            cost_savings_by_tier: ctxcut_core::ModelTierSavings {
                standard_sonnet_gpt4o: 0.0,
                frontier_opus: 0.0,
                economy_haiku_mini: 0.0,
            },
            language_breakdown: std::collections::BTreeMap::new(),
            by_language: Vec::new(),
            by_source: Vec::new(),
            recent_events: Vec::new(),
        });

    let metrics = json!({
        "total_requests": summary.total_requests,
        "total_raw_tokens": summary.total_raw_tokens,
        "total_sliced_tokens": summary.total_sliced_tokens,
        "total_tokens_saved": summary.total_saved_tokens,
        "savings_percentage": summary.compression_percentage,
        "estimated_usd_savings": {
            "economy": summary.cost_savings_by_tier.economy_haiku_mini,
            "standard": summary.cost_savings_by_tier.standard_sonnet_gpt4o,
            "frontier": summary.cost_savings_by_tier.frontier_opus
        }
    });

    let rendered = if format_str.eq_ignore_ascii_case("json") {
        serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".to_string())
    } else if format_str.eq_ignore_ascii_case("text") {
        ctxcut_cli::render_dashboard(&summary, &metrics_path)
    } else {
        format!(
            "# ctxcut Lifetime Telemetry & Token Savings\n\n\
             - **Total Slicing Requests:** `{}`\n\
             - **Raw File Tokens Ingested:** `{}`\n\
             - **Sliced Tokens Delivered:** `{}`\n\
             - **Cumulative Tokens Saved:** `{}` (`{:.1}%`)\n\n\
             ### Estimated API Cost Savings\n\
             - **Economy Tier ($0.50/1M):** `${:.4}`\n\
             - **Standard Tier ($3.00/1M):** `${:.4}`\n\
             - **Frontier Tier ($15.00/1M):** `${:.4}`\n",
            summary.total_requests,
            summary.total_raw_tokens,
            summary.total_sliced_tokens,
            summary.total_saved_tokens,
            summary.compression_percentage,
            summary.cost_savings_by_tier.economy_haiku_mini,
            summary.cost_savings_by_tier.standard_sonnet_gpt4o,
            summary.cost_savings_by_tier.frontier_opus,
        )
    };

    let response = json!({
        "content": [{ "type": "text", "text": rendered }],
        "metrics": summary
    });
    (
        response,
        Some(metrics),
        None,
        Some(summary.total_saved_tokens),
    )
}

fn execute_patch_symbol(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let Some(path_str) = args.get("path").and_then(Value::as_str) else {
        let err = "Missing required parameter 'path'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };
    let Some(symbol) = args.get("symbol").and_then(Value::as_str) else {
        let err = "Missing required parameter 'symbol'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };
    let Some(code) = args.get("code").and_then(Value::as_str) else {
        let err = "Missing required parameter 'code'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let dry_run = args
        .get("dry_run")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    match AstPatcher::patch_symbol(Path::new(path_str), symbol, code, dry_run) {
        Ok(patch_res) => {
            let response = json!({
                "content": [{ "type": "text", "text": patch_res.diff }],
                "patch": patch_res
            });
            (response, None, None, None)
        }
        Err(e) => {
            let err = format!("Patch error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_get_test_context(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let Some(path_str) = args.get("path").and_then(Value::as_str) else {
        let err = "Missing required parameter 'path'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };
    let Some(symbol) = args.get("symbol").and_then(Value::as_str) else {
        let err = "Missing required parameter 'symbol'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let framework = args.get("framework").and_then(Value::as_str);
    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);

    let opts = SliceOptions {
        budget,
        ..Default::default()
    };

    match TestContextGenerator::generate(Path::new(path_str), symbol, framework, &opts) {
        Ok(test_ctx) => {
            let response = json!({
                "content": [{ "type": "text", "text": test_ctx.to_markdown() }],
                "test_context": test_ctx
            });
            (response, None, None, None)
        }
        Err(e) => {
            let err = format!("Test context error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_get_route_slice(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let has_ipc_key = args.get("procedure").is_some()
        || args.get("command").is_some()
        || args.get("channel").is_some();

    let method = if let Some(m) = args.get("method").and_then(Value::as_str) {
        m
    } else if has_ipc_key || args.get("path").is_some() || args.get("route_path").is_some() {
        "ANY"
    } else {
        let err = "Missing required parameter 'method' and 'path' (or 'procedure' / 'command' / 'channel')".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let Some(route_path) = args
        .get("path")
        .or_else(|| args.get("route_path"))
        .or_else(|| args.get("procedure"))
        .or_else(|| args.get("command"))
        .or_else(|| args.get("channel"))
        .and_then(Value::as_str)
    else {
        let err = "Missing required parameter 'path' (or 'procedure' / 'command' / 'channel')".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let root_dir = args
        .get("root_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);

    let opts = SliceOptions {
        budget,
        ..Default::default()
    };

    match ctxcut_cli::route::resolve_route_slice(&root_dir, method, route_path, &opts) {
        Ok(slice) => {
            let response = json!({
                "content": [{ "type": "text", "text": MarkdownFormatter::format(&slice) }],
                "slice": slice
            });
            (response, None, None, None)
        }
        Err(e) => {
            let err = format!("Route resolution error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_impact_slice(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let Some(target) = args.get("target").and_then(Value::as_str) else {
        let err = "Missing required parameter 'target'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let target_file = args.get("path").and_then(Value::as_str).map(PathBuf::from);
    let root_dir = args
        .get("root_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|l| l as usize);

    let opts = SliceOptions {
        depth: 1,
        budget,
        include_types: true,
        include_calls: true,
    };

    match ImpactAnalyzer::find_callers(&root_dir, target, target_file.as_deref(), &opts) {
        Ok(mut result) => {
            if let Some(lim) = limit {
                result.callers.truncate(lim);
                result.total_callers = result.callers.len();
            }

            let saved_tokens = result
                .stats
                .raw_file_tokens
                .saturating_sub(result.stats.sliced_tokens);
            TelemetryLogger::record_operation(
                "mcp_impact",
                &root_dir.to_string_lossy(),
                result.stats.raw_file_tokens,
                result.stats.sliced_tokens,
                saved_tokens,
            );

            let text_output = result.to_markdown();
            let metrics_val = json!({
                "raw_tokens": result.stats.raw_file_tokens,
                "sliced_tokens": result.stats.sliced_tokens,
                "savings_pct": result.stats.savings_percentage,
                "total_callers": result.total_callers,
            });

            let response = json!({
                "content": [{ "type": "text", "text": text_output }],
                "impact": result
            });

            (response, Some(metrics_val), None, Some(saved_tokens))
        }
        Err(e) => {
            let err = format!("Impact analysis error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_trace_slice(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let Some(entry_point) = args.get("entry_point").and_then(Value::as_str) else {
        let err = "Missing required parameter 'entry_point'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let root_dir = args
        .get("root_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);
    let depth = args
        .get("depth")
        .and_then(Value::as_u64)
        .map(|d| d as usize)
        .unwrap_or(8);

    let opts = SliceOptions {
        depth,
        budget,
        include_types: true,
        include_calls: true,
    };

    match ExecutionTracer::trace(&root_dir, entry_point, &opts) {
        Ok(trace) => {
            let saved_tokens = trace
                .stats
                .raw_file_tokens
                .saturating_sub(trace.stats.sliced_tokens);
            TelemetryLogger::record_operation(
                "mcp_trace",
                &root_dir.to_string_lossy(),
                trace.stats.raw_file_tokens,
                trace.stats.sliced_tokens,
                saved_tokens,
            );

            let text_output = trace.to_markdown();
            let metrics_val = json!({
                "raw_tokens": trace.stats.raw_file_tokens,
                "sliced_tokens": trace.stats.sliced_tokens,
                "savings_pct": trace.stats.savings_percentage,
                "total_steps": trace.total_steps,
            });

            let response = json!({
                "content": [{ "type": "text", "text": text_output }],
                "trace": trace
            });

            (response, Some(metrics_val), None, Some(saved_tokens))
        }
        Err(e) => {
            let err = format!("Execution trace error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_verify_patch(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let target = if let Some(t) = args.get("target").and_then(Value::as_str) {
        t.to_string()
    } else if let (Some(p), Some(s)) = (
        args.get("path").and_then(Value::as_str),
        args.get("symbol").and_then(Value::as_str),
    ) {
        format!("{p}:{s}")
    } else {
        let err = "Missing required parameter: provide 'target' (e.g. `src/lib.rs:foo`) or 'path' and 'symbol'".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let replacement_code = if let Some(code) = args.get("new_code").and_then(Value::as_str) {
        code
    } else if let Some(code) = args.get("code").and_then(Value::as_str) {
        code
    } else {
        let err = "Missing required parameter 'new_code' (or 'code')".to_string();
        return (
            json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            }),
            None,
            Some(err),
            None,
        );
    };

    let dry_run = args.get("dry_run").and_then(Value::as_bool).unwrap_or(true);
    let typechecker = args.get("typechecker").and_then(Value::as_str);
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match PatchVerifier::verify_patch(
        &current_dir,
        &target,
        replacement_code,
        typechecker,
        dry_run,
    ) {
        Ok(verify_res) => {
            let response = json!({
                "content": [{ "type": "text", "text": verify_res.to_markdown() }],
                "verify_result": verify_res
            });
            (response, None, None, None)
        }
        Err(e) => {
            let err = format!("Verification Guard Error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_semantic_diff(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let root_str = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let file_path = args.get("file_path").and_then(Value::as_str).map(Path::new);
    let staged = args.get("staged").and_then(Value::as_bool).unwrap_or(false);
    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);
    let format = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");

    match SemanticDiffEngine::compute_diff(Path::new(root_str), staged, file_path, budget) {
        Ok(result) => {
            let rendered = if format.eq_ignore_ascii_case("json") {
                result.to_json()
            } else {
                result.to_markdown()
            };
            let metrics = json!({
                "raw_tokens": result.roi.raw_tokens,
                "diff_tokens": result.roi.semantic_diff_tokens,
                "tokens_saved": result.roi.tokens_saved,
                "savings_percentage": result.roi.savings_percentage,
                "total_files": result.files.len()
            });
            let tokens_saved = result.roi.tokens_saved;
            let response = json!({
                "content": [{ "type": "text", "text": rendered }],
                "semantic_diff": result
            });
            (response, Some(metrics), None, Some(tokens_saved))
        }
        Err(e) => {
            let err = format!("Semantic diff error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_refactor_rename(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let Some(target) = args.get("target").and_then(Value::as_str) else {
        let err = "Missing required parameter 'target'".to_string();
        return (
            json!({ "isError": true, "content": [{ "type": "text", "text": err }] }),
            None,
            Some(err),
            None,
        );
    };
    let Some(new_name) = args.get("new_name").and_then(Value::as_str) else {
        let err = "Missing required parameter 'new_name'".to_string();
        return (
            json!({ "isError": true, "content": [{ "type": "text", "text": err }] }),
            None,
            Some(err),
            None,
        );
    };

    let root_str = args.get("root_dir").and_then(Value::as_str).unwrap_or(".");
    let dry_run = args
        .get("dry_run")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");

    match SymbolRenamer::rename_symbol(Path::new(root_str), target, new_name, dry_run) {
        Ok(result) => {
            let rendered = if format_str.eq_ignore_ascii_case("json") {
                result.to_json()
            } else {
                result.to_markdown()
            };
            let metrics = json!({
                "files_modified": result.total_files_modified,
                "occurrences_renamed": result.total_occurrences,
                "dry_run": result.dry_run
            });
            let response = json!({
                "content": [{ "type": "text", "text": rendered }],
                "rename": result
            });
            (response, Some(metrics), None, None)
        }
        Err(e) => {
            let err = format!("Refactor rename error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_index_workspace(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let root_str = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let ws_root = PathBuf::from(root_str);
    let rebuild = args
        .get("rebuild")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status_only = args
        .get("status_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut engine = match IndexEngine::open_or_create(&ws_root) {
        Ok(e) => e,
        Err(e) => {
            let err = format!("Failed to open index: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            return (response, None, Some(err), None);
        }
    };

    if status_only {
        match engine.status() {
            Ok(status) => {
                let metrics = json!({
                    "total_files": status.total_files,
                    "total_symbols": status.total_symbols,
                    "total_callers": status.total_callers,
                    "total_implementors": status.total_implementors,
                    "is_wal_mode": status.is_wal_mode,
                    "in_memory": status.in_memory
                });
                let response = json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&status).unwrap_or_default() }],
                    "status": status
                });
                (response, Some(metrics), None, None)
            }
            Err(e) => {
                let err = format!("Failed to get index status: {e}");
                let response = json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": err }]
                });
                (response, None, Some(err), None)
            }
        }
    } else {
        let opts = IndexOptions {
            rebuild,
            ..Default::default()
        };
        match engine.sync_incremental(&opts) {
            Ok(sync_result) => {
                let metrics = json!({
                    "files_added": sync_result.files_added,
                    "files_updated": sync_result.files_updated,
                    "files_deleted": sync_result.files_deleted,
                    "files_unchanged": sync_result.files_unchanged,
                    "total_symbols": sync_result.total_symbols,
                    "duration_ms": sync_result.duration_ms
                });
                let summary_text = format!(
                    "✔ Workspace index synchronized in {}ms ({} added, {} updated, {} deleted, {} unchanged, {} total symbols)",
                    sync_result.duration_ms,
                    sync_result.files_added,
                    sync_result.files_updated,
                    sync_result.files_deleted,
                    sync_result.files_unchanged,
                    sync_result.total_symbols
                );
                let response = json!({
                    "content": [{ "type": "text", "text": summary_text }],
                    "sync_result": sync_result
                });
                (response, Some(metrics), None, None)
            }
            Err(e) => {
                let err = format!("Failed to synchronize index: {e}");
                let response = json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": err }]
                });
                (response, None, Some(err), None)
            }
        }
    }
}

fn execute_query_ast(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let pattern = args.get("pattern").and_then(Value::as_str);
    let preset = args.get("preset").and_then(Value::as_str);
    let lang_str = args.get("language").and_then(Value::as_str);
    let root_str = args.get("root_dir").and_then(Value::as_str).unwrap_or(".");
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|l| l as usize);
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");

    let lang_filter = lang_str.and_then(SupportedLanguage::from_str_loose);

    match AstQueryEngine::query_workspace(Path::new(root_str), pattern, lang_filter, preset, limit)
    {
        Ok(report) => {
            let rendered = if format_str.eq_ignore_ascii_case("json") {
                report.to_json()
            } else {
                report.to_markdown()
            };

            let metrics = json!({
                "total_matches": report.total_matches,
                "files_scanned": report.files_scanned,
                "files_matched": report.files_matched
            });

            let response = json!({
                "content": [{ "type": "text", "text": rendered }],
                "query_report": report
            });
            (response, Some(metrics), None, None)
        }
        Err(e) => {
            let err = format!("AST Query error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_fullstack_trace(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let Some(entry) = args
        .get("entry")
        .or_else(|| args.get("entry_point"))
        .and_then(Value::as_str)
    else {
        let err = "Missing required parameter 'entry'".to_string();
        return (
            json!({ "isError": true, "content": [{ "type": "text", "text": err }] }),
            None,
            Some(err),
            None,
        );
    };

    let root_str = args.get("root_dir").and_then(Value::as_str).unwrap_or(".");
    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);
    let max_depth = args
        .get("max_depth")
        .or_else(|| args.get("depth"))
        .and_then(Value::as_u64)
        .map(|d| d as usize);
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");

    let tracer = FullstackExecutionTracer::new();
    match tracer.trace_api_with_depth(Path::new(root_str), entry, budget, max_depth) {
        Ok(result) => {
            let saved_tokens = result
                .stats
                .raw_file_tokens
                .saturating_sub(result.stats.sliced_tokens);
            TelemetryLogger::record_operation(
                "mcp_fullstack_trace",
                root_str,
                result.stats.raw_file_tokens,
                result.stats.sliced_tokens,
                saved_tokens,
            );

            let rendered = if format_str.eq_ignore_ascii_case("json") {
                result.to_json()
            } else {
                result.to_markdown()
            };

            let metrics = json!({
                "raw_tokens": result.stats.raw_file_tokens,
                "sliced_tokens": result.stats.sliced_tokens,
                "saved_tokens": saved_tokens,
                "savings_pct": result.stats.savings_percentage,
                "total_steps": result.total_steps,
                "query_endpoint": result.query_endpoint
            });

            let response = json!({
                "content": [{ "type": "text", "text": rendered }],
                "trace": result
            });

            (response, Some(metrics), None, Some(saved_tokens))
        }
        Err(e) => {
            let err = format!("Full-stack trace error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_intent_slice(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let Some(prompt) = args
        .get("prompt")
        .or_else(|| args.get("query"))
        .and_then(Value::as_str)
    else {
        let err = "Missing required parameter 'prompt'".to_string();
        return (
            json!({ "isError": true, "content": [{ "type": "text", "text": err }] }),
            None,
            Some(err),
            None,
        );
    };

    let root_str = args.get("root_dir").and_then(Value::as_str).unwrap_or(".");
    let budget = args
        .get("budget")
        .and_then(Value::as_u64)
        .map(|b| b as usize);
    let max_symbols = args
        .get("max_symbols")
        .and_then(Value::as_u64)
        .map(|s| s as usize)
        .unwrap_or(5);
    let depth = args
        .get("depth")
        .and_then(Value::as_u64)
        .map(|d| d as usize)
        .unwrap_or(1);
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");

    let opts = IntentSliceOptions {
        prompt: prompt.to_string(),
        budget,
        max_target_symbols: max_symbols,
        depth,
    };

    let slicer = DefaultIntentSlicer::new();
    match slicer.slice_intent(Path::new(root_str), &opts) {
        Ok(result) => {
            let saved_tokens = result
                .stats
                .raw_file_tokens
                .saturating_sub(result.stats.sliced_tokens);
            TelemetryLogger::record_operation(
                "mcp_intent_slice",
                root_str,
                result.stats.raw_file_tokens,
                result.stats.sliced_tokens,
                saved_tokens,
            );

            let rendered = if format_str.eq_ignore_ascii_case("json") {
                result.to_json()
            } else {
                result.to_markdown()
            };

            let metrics = json!({
                "raw_tokens": result.stats.raw_file_tokens,
                "sliced_tokens": result.stats.sliced_tokens,
                "saved_tokens": saved_tokens,
                "savings_pct": result.stats.savings_percentage,
                "target_symbols_count": result.target_symbols.len(),
                "hoisted_types_count": result.hoisted_types.len(),
                "prompt": result.prompt
            });

            let response = json!({
                "content": [{ "type": "text", "text": rendered }],
                "intent_slice": result
            });

            (response, Some(metrics), None, Some(saved_tokens))
        }
        Err(e) => {
            let err = format!("Intent slicing error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_patch_transaction(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let patches_val = args.get("patches").unwrap_or(&Value::Null);
    let patches: Vec<SymbolPatchUnit> = match serde_json::from_value(patches_val.clone()) {
        Ok(p) => p,
        Err(e) => {
            let err = format!("Invalid 'patches' array: {e}");
            return (
                json!({ "isError": true, "content": [{ "type": "text", "text": err }] }),
                None,
                Some(err),
                None,
            );
        }
    };

    if patches.is_empty() {
        let err = "Parameter 'patches' array cannot be empty".to_string();
        return (
            json!({ "isError": true, "content": [{ "type": "text", "text": err }] }),
            None,
            Some(err),
            None,
        );
    }

    let root_opt = args
        .get("root_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let typechecker = args
        .get("typechecker")
        .and_then(Value::as_str)
        .map(String::from);
    let apply = args.get("apply").and_then(Value::as_bool).unwrap_or(false);
    let timeout_ms = args.get("timeout_ms").and_then(Value::as_u64);
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");

    let req = PatchTransactionRequest {
        workspace_root: root_opt,
        patches,
        typechecker,
        apply,
        timeout_ms,
    };

    match BatchAstPatcher::apply_transaction(&req) {
        Ok(result) => {
            let rendered = if format_str.eq_ignore_ascii_case("json") {
                result.to_json()
            } else if format_str.eq_ignore_ascii_case("text") {
                let mut out = String::new();
                if result.applied {
                    out.push_str(&format!(
                        "✔ Successfully applied batch patches across {} file(s) ({} symbol(s))\n",
                        result.files_modified_count, result.symbols_patched_count
                    ));
                } else if result.success {
                    out.push_str(&format!(
                        "ℹ Dry-run verified successfully for {} file(s) ({} symbol(s))\n",
                        result.files_modified_count, result.symbols_patched_count
                    ));
                } else if result.rolled_back {
                    out.push_str("✖ Batch refactor failed and was rolled back cleanly\n");
                } else {
                    out.push_str("✖ Pre-write validation rejected the patch\n");
                }
                for diff in &result.diffs {
                    out.push_str(&format!("\n--- {}\n", diff.file_path));
                    out.push_str(&diff.diff);
                }
                out
            } else {
                result.to_markdown()
            };

            let metrics = json!({
                "success": result.success,
                "applied": result.applied,
                "rolled_back": result.rolled_back,
                "files_modified_count": result.files_modified_count,
                "symbols_patched_count": result.symbols_patched_count,
                "diagnostics_count": result.diagnostics.len()
            });

            let is_error = !result.success;
            let response = json!({
                "isError": is_error,
                "content": [{ "type": "text", "text": rendered }],
                "transaction": result
            });

            (response, Some(metrics), None, None)
        }
        Err(e) => {
            let err = format!("Batch patch transaction error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}

fn execute_pack_agent_context(
    args: &Value,
) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let root_str = args.get("root_dir").and_then(Value::as_str).unwrap_or(".");
    let agents_count = args
        .get("agents_count")
        .and_then(Value::as_u64)
        .map(|a| a as usize)
        .unwrap_or(2)
        .max(1);
    let seeds_vec: Vec<String> = args
        .get("seed_symbols")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let budget_per_agent = args
        .get("budget_per_agent")
        .and_then(Value::as_u64)
        .map(|b| b as usize);
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");

    let partitioner = DefaultSwarmPartitioner::new();
    match partitioner.partition_workspace(
        Path::new(root_str),
        agents_count,
        &seeds_vec,
        budget_per_agent,
    ) {
        Ok(manifest) => {
            let total_raw: usize = manifest
                .packs
                .iter()
                .map(|p| p.token_stats.raw_file_tokens)
                .sum();
            let total_sliced: usize = manifest
                .packs
                .iter()
                .map(|p| p.token_stats.sliced_tokens)
                .sum();
            let total_saved = total_raw.saturating_sub(total_sliced);
            TelemetryLogger::record_operation(
                "mcp_swarm_partition",
                root_str,
                total_raw,
                total_sliced,
                total_saved,
            );

            let rendered = if format_str.eq_ignore_ascii_case("json") {
                manifest.to_json()
            } else {
                manifest.to_markdown()
            };

            let metrics = json!({
                "total_agents": manifest.total_agents,
                "total_symbols": manifest.total_symbols,
                "boundary_contracts_count": manifest.boundary_contracts_count,
                "total_raw_tokens": total_raw,
                "total_sliced_tokens": total_sliced,
                "total_saved_tokens": total_saved
            });

            let response = json!({
                "content": [{ "type": "text", "text": rendered }],
                "manifest": manifest
            });

            (response, Some(metrics), None, Some(total_saved))
        }
        Err(e) => {
            let err = format!("Swarm context packaging error: {e}");
            let response = json!({
                "isError": true,
                "content": [{ "type": "text", "text": err }]
            });
            (response, None, Some(err), None)
        }
    }
}
