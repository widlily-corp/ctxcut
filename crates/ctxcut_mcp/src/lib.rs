//! `ctxcut_mcp` — Model Context Protocol (MCP) STDIO JSON-RPC 2.0 server.
//!
//! Exposes AST context slicing tools (`get_symbol_slice`, `get_diff_slice`, `analyze_token_stats`)
//! to AI coding agents over clean STDIO framing with structured JSONL file logging and observability.

pub mod logger;

pub use logger::{format_rfc3339, McpFileLogger, ToolLogRecord};

use anyhow::Result;
use ctxcut_core::{ContextSlicer, MarkdownFormatter, SliceOptions, TelemetryLogger};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
                "description": "Extracts AST-accurate slice of a function/class with hoisted types and signature-only stubs",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Source file path"
                        },
                        "symbol": {
                            "type": "string",
                            "description": "Target symbol name (e.g. `calculateTax` or `OrderService.refund`)"
                        }
                    },
                    "required": ["path", "symbol"]
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
                        }
                    }
                }
            },
            {
                "name": "analyze_token_stats",
                "description": "Calculates repository or file token savings and optimization statistics",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File or directory path to analyze"
                        },
                        "fast": {
                            "type": "boolean",
                            "description": "Enable shallow fast estimation scan mode without deep AST slicing (default: true for directories, false for single files)"
                        }
                    },
                    "required": ["path"]
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
        "get_diff_slice" => execute_diff_slice(args),
        "analyze_token_stats" => execute_stats_slice(args),
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

    let start_time = Instant::now();
    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();
    match slicer.slice_symbol(Path::new(file_path_str), symbol, &opts) {
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
                "content": [{ "type": "text", "text": MarkdownFormatter::format(&slice) }]
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

fn execute_diff_slice(args: &Value) -> (Value, Option<Value>, Option<String>, Option<usize>) {
    let staged = args.get("staged").and_then(Value::as_bool).unwrap_or(false);
    let path_opt = args.get("path").and_then(Value::as_str);
    let repo_path = path_opt.map(Path::new);
    let opts = SliceOptions::default();

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
