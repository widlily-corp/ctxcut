//! Integration tests for MCP server timeout safety guard and fast stats handler.

use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_mcp_timeout_guard_triggers_on_timeout() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create 10 dummy files
    for i in 0..10 {
        fs::write(
            root.join(format!("file_{i}.ts")),
            "export function test(): number { return 1 + 2; }\n",
        )
        .unwrap();
    }

    let args = json!({
        "path": root.to_string_lossy(),
        "fast": false
    });

    // Invoke with 0ms timeout (immediate timeout deadline)
    let (response, _metrics, err_opt, _saved) =
        execute_tool_with_timeout("analyze_token_stats", &args, 0);

    assert_eq!(response.get("isError"), Some(&json!(true)));
    let content_text = response["content"][0]["text"].as_str().unwrap();
    assert!(content_text.contains("Timeout"));
    assert!(content_text.contains("analyze_token_stats"));

    let timeout_meta = &response["timeout"];
    assert_eq!(timeout_meta["tool"], "analyze_token_stats");
    assert_eq!(timeout_meta["timeout_ms"], 0);
    assert!(timeout_meta["suggestion"]
        .as_str()
        .unwrap()
        .contains("fast"));
    assert!(err_opt.is_some());
}

#[test]
fn test_mcp_analyze_token_stats_fast_execution() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::write(
        root.join("service.ts"),
        "export function getStatus(): string { return 'ok'; }\n",
    )
    .unwrap();

    let args = json!({
        "path": root.to_string_lossy(),
        "fast": true
    });

    let (response, metrics, err_opt, saved_opt) =
        execute_tool_with_timeout("analyze_token_stats", &args, 10_000);

    assert_eq!(response.get("isError"), None);
    assert!(err_opt.is_none());
    assert!(metrics.is_some());
    assert!(saved_opt.is_some());

    let content_text = response["content"][0]["text"].as_str().unwrap();
    assert!(content_text.contains("ctxcut Token Optimization & Context Statistics"));
    assert!(content_text.contains("Total Files Analyzed: 1"));
}

#[test]
fn test_mcp_analyze_token_stats_directory_defaults_to_fast() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::write(
        root.join("handler.py"),
        "def handle_event(event: dict) -> bool:\n    return True\n",
    )
    .unwrap();

    // No `fast` argument provided -> should default to `fast: true` for directories
    let args = json!({
        "path": root.to_string_lossy()
    });

    let (response, metrics, err_opt, _saved) =
        execute_tool_with_timeout("analyze_token_stats", &args, 10_000);

    assert_eq!(response.get("isError"), None);
    assert!(err_opt.is_none());
    assert!(metrics.is_some());
}

#[test]
fn test_mcp_unknown_tool_returns_structured_error() {
    let args = json!({});
    let (response, _metrics, err_opt, _saved) =
        execute_tool_with_timeout("non_existent_tool", &args, 1000);

    assert_eq!(response.get("isError"), Some(&json!(true)));
    assert!(err_opt.is_some());
    let content_text = response["content"][0]["text"].as_str().unwrap();
    assert!(content_text.contains("Unknown tool: `non_existent_tool`"));
}
