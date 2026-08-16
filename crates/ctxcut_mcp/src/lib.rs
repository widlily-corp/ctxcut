//! `ctxcut_mcp` — Model Context Protocol (MCP) STDIO JSON-RPC 2.0 server.

use std::io::{self, BufRead, Write};
use std::path::Path;
use anyhow::Result;
use ctxcut_core::{ContextSlicer, MarkdownFormatter, SliceOptions};
use serde_json::{json, Value};

/// Runs the Model Context Protocol (MCP) server over STDIO.
pub fn run_mcp_server() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(request) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };

        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let id = request.get("id").cloned();

        // Notification: initialized or exit
        if method.starts_with("notifications/") || method == "initialized" {
            continue;
        }

        let response = handle_mcp_request(method, &request);

        let response_payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": response.get("result").cloned().unwrap_or(response)
        });

        let mut out_str = serde_json::to_string(&response_payload)?;
        out_str.push('\n');
        stdout.write_all(out_str.as_bytes())?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_mcp_request(method: &str, req: &Value) -> Value {
    match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "ctxcut-mcp",
                "version": "0.1.0"
            }
        }),

        "tools/list" => json!({
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
                            }
                        },
                        "required": ["path"]
                    }
                }
            ]
        }),

        "tools/call" => {
            let params = req.get("params").unwrap_or(&Value::Null);
            let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").unwrap_or(&Value::Null);

            execute_tool_call(tool_name, args)
        }

        _ => json!({
            "error": {
                "code": -32601,
                "message": format!("Method not found: `{method}`")
            }
        }),
    }
}

fn execute_tool_call(name: &str, args: &Value) -> Value {
    match name {
        "get_symbol_slice" => {
            let Some(file_path_str) = args.get("path").and_then(Value::as_str) else {
                return json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": "Missing required parameter 'path'" }]
                });
            };
            let Some(symbol) = args.get("symbol").and_then(Value::as_str) else {
                return json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": "Missing required parameter 'symbol'" }]
                });
            };

            let slicer = ContextSlicer::new();
            let opts = SliceOptions::default();
            match slicer.slice_symbol(Path::new(file_path_str), symbol, &opts) {
                Ok(slice) => json!({
                    "content": [{ "type": "text", "text": MarkdownFormatter::format(&slice) }]
                }),
                Err(e) => json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": format!("Slicing error: {e}") }]
                }),
            }
        }

        "get_diff_slice" => {
            let staged = args.get("staged").and_then(Value::as_bool).unwrap_or(false);
            let opts = SliceOptions::default();

            match ctxcut_cli::diff::run_diff_slicer(staged, &opts) {
                Ok(slices) => {
                    let rendered = if slices.is_empty() {
                        "No modified symbols detected in git diff.".to_string()
                    } else {
                        MarkdownFormatter::format_batch(&slices)
                    };
                    json!({
                        "content": [{ "type": "text", "text": rendered }]
                    })
                }
                Err(e) => json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": format!("Diff error: {e}") }]
                }),
            }
        }

        "analyze_token_stats" => {
            let Some(path_str) = args.get("path").and_then(Value::as_str) else {
                return json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": "Missing required parameter 'path'" }]
                });
            };

            match ctxcut_cli::stats::calculate_stats(Path::new(path_str)) {
                Ok(report) => json!({
                    "content": [{ "type": "text", "text": ctxcut_cli::stats::format_stats_text(&report) }],
                    "stats": report
                }),
                Err(e) => json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": format!("Stats calculation error: {e}") }]
                }),
            }
        }

        _ => json!({
            "isError": true,
            "content": [{ "type": "text", "text": format!("Unknown tool: `{name}`") }]
        }),
    }
}
