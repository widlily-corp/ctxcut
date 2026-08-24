//! Integration tests for MCP get_workspace_overview, get_metrics, and batch get_symbol_slice.

use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_mcp_get_workspace_overview() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let ts_file = root.join("calculator.ts");
    fs::write(
        &ts_file,
        r#"
export class Calculator {
    public add(a: number, b: number): number {
        return a + b;
    }
    public subtract(a: number, b: number): number {
        return a - b;
    }
}
"#,
    )
    .unwrap();

    let args = json!({
        "path": root.to_string_lossy(),
        "format": "markdown"
    });

    let (response, metrics, error_opt, tokens_saved) =
        execute_tool_with_timeout("get_workspace_overview", &args, 5000);

    assert!(
        error_opt.is_none(),
        "Expected no error, got {:?}",
        error_opt
    );
    assert_ne!(response.get("isError"), Some(&json!(true)));

    let content_text = response["content"][0]["text"].as_str().unwrap();
    assert!(content_text.contains("# Workspace Symbol Overview"));
    assert!(content_text.contains("Calculator"));
    assert!(content_text.contains("Calculator.add"));

    assert!(metrics.is_some());
    assert!(tokens_saved.is_some());
}

#[test]
fn test_mcp_get_symbol_slice_batch_comma_separated() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let ts_file = root.join("math.ts");
    fs::write(
        &ts_file,
        r#"
export interface Point {
    x: number;
    y: number;
}

export function distance(p1: Point, p2: Point): number {
    return Math.sqrt((p1.x - p2.x) ** 2 + (p1.y - p2.y) ** 2);
}

export function midpoint(p1: Point, p2: Point): Point {
    return { x: (p1.x + p2.x) / 2, y: (p1.y + p2.y) / 2 };
}
"#,
    )
    .unwrap();

    let args = json!({
        "path": ts_file.to_string_lossy(),
        "symbol": "distance, midpoint"
    });

    let (response, metrics, error_opt, tokens_saved) =
        execute_tool_with_timeout("get_symbol_slice", &args, 5000);

    assert!(
        error_opt.is_none(),
        "Expected no error, got {:?}",
        error_opt
    );
    assert_ne!(response.get("isError"), Some(&json!(true)));

    let text = response["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("distance, midpoint"));
    assert!(text.contains("function distance"));
    assert!(text.contains("function midpoint"));
    assert!(text.contains("interface Point"));

    // Point should be hoisted once
    assert_eq!(text.matches("interface Point").count(), 1);

    assert!(metrics.is_some());
    assert!(tokens_saved.is_some());
}

#[test]
fn test_mcp_get_metrics_and_analyze_stats_history() {
    // 1. Test get_metrics directly
    let args = json!({
        "format": "markdown"
    });
    let (response, metrics, error_opt, _) = execute_tool_with_timeout("get_metrics", &args, 5000);

    assert!(error_opt.is_none());
    assert_ne!(response.get("isError"), Some(&json!(true)));
    let text = response["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("ctxcut Lifetime Telemetry"));
    assert!(metrics.is_some());

    // 2. Test analyze_token_stats with history=true
    let history_args = json!({
        "history": true,
        "format": "json"
    });
    let (h_response, h_metrics, h_error_opt, _) =
        execute_tool_with_timeout("analyze_token_stats", &history_args, 5000);

    assert!(h_error_opt.is_none());
    assert_ne!(h_response.get("isError"), Some(&json!(true)));
    assert!(h_metrics.is_some());
}
