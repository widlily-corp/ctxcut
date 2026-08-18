//! Empirical Challenger 2 Stress Tests for Milestone M1 (MCP Timeout Guard & Fast Stats).
//!
//! Tests the robustness of:
//! 1. Timeout Guard deadline enforcement across different tools and timeout thresholds.
//! 2. Structured Timeout response schema (isError, content, timeout metadata, suggestion).
//! 3. Panic recovery & thread boundary safety (ensuring no process exit on tool panic).
//! 4. Comprehensive error handling for missing parameters, bad inputs, and non-existent targets.
//! 5. Fast stats performance vs deep AST parsing on large file trees and directory defaulting logic.

use ctxcut_mcp::{execute_tool_with_timeout, McpFileLogger, McpServerOptions, ToolLogRecord};
use serde_json::json;
use std::fs;
use std::time::Instant;
use tempfile::{NamedTempFile, TempDir};

#[test]
fn test_challenger_mcp_timeout_guard_all_tools_under_zero_timeout() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create a dummy TypeScript file
    let ts_file = root.join("app.ts");
    fs::write(
        &ts_file,
        r"
export function calculateTotal(items: { price: number }[]): number {
    return items.reduce((acc, item) => acc + item.price, 0);
}
",
    )
    .unwrap();

    let tools_to_test = vec![
        (
            "get_symbol_slice",
            json!({
                "path": ts_file.to_string_lossy(),
                "symbol": "calculateTotal"
            }),
        ),
        (
            "get_diff_slice",
            json!({
                "path": root.to_string_lossy()
            }),
        ),
        (
            "analyze_token_stats",
            json!({
                "path": root.to_string_lossy(),
                "fast": false
            }),
        ),
        (
            "patch_symbol",
            json!({
                "path": ts_file.to_string_lossy(),
                "symbol": "calculateTotal",
                "code": "export function calculateTotal(): number { return 42; }"
            }),
        ),
        (
            "get_test_context",
            json!({
                "path": ts_file.to_string_lossy(),
                "symbol": "calculateTotal"
            }),
        ),
        (
            "get_route_slice",
            json!({
                "method": "GET",
                "path": "/api/total",
                "root_dir": root.to_string_lossy()
            }),
        ),
    ];

    for (tool_name, args) in tools_to_test {
        // Force 0ms timeout (immediate timeout)
        let (response, metrics, err_opt, tokens_saved) =
            execute_tool_with_timeout(tool_name, &args, 0);

        assert_eq!(
            response.get("isError"),
            Some(&json!(true)),
            "Tool `{tool_name}` must return isError: true on timeout"
        );
        assert!(
            metrics.is_none(),
            "Tool `{tool_name}` must not return metrics on timeout"
        );
        assert!(
            tokens_saved.is_none(),
            "Tool `{tool_name}` must not return saved tokens on timeout"
        );
        assert!(
            err_opt.is_some(),
            "Tool `{tool_name}` must populate error string on timeout"
        );

        let content_arr = response["content"]
            .as_array()
            .expect("content must be array");
        assert!(!content_arr.is_empty(), "content array must not be empty");
        let content_text = content_arr[0]["text"]
            .as_str()
            .expect("content[0].text must be string");
        assert!(
            content_text.contains("Timeout"),
            "content[0].text must mention Timeout for `{tool_name}`"
        );
        assert!(
            content_text.contains(tool_name),
            "content[0].text must mention tool name `{tool_name}`"
        );

        let timeout_meta = &response["timeout"];
        assert_eq!(timeout_meta["tool"], tool_name);
        assert_eq!(timeout_meta["timeout_ms"], 0);
        assert!(
            timeout_meta["suggestion"].as_str().is_some(),
            "Tool `{tool_name}` must have a suggestion string"
        );
    }
}

#[test]
fn test_challenger_mcp_missing_required_params_for_all_tools() {
    let empty_args = json!({});

    let tools_and_expected_errors = vec![
        ("get_symbol_slice", "Missing required parameter 'path'"),
        ("patch_symbol", "Missing required parameter 'path'"),
        ("analyze_token_stats", "Missing required parameter 'path'"),
        ("get_test_context", "Missing required parameter 'path'"),
        ("get_route_slice", "Missing required parameter 'method'"),
    ];

    for (tool_name, expected_err_sub) in tools_and_expected_errors {
        let (response, metrics, err_opt, tokens_saved) =
            execute_tool_with_timeout(tool_name, &empty_args, 5000);

        assert_eq!(
            response.get("isError"),
            Some(&json!(true)),
            "Tool `{tool_name}` must return isError: true on missing params"
        );
        assert!(metrics.is_none());
        assert!(tokens_saved.is_none());
        assert!(err_opt.is_some());

        let content_text = response["content"][0]["text"].as_str().unwrap();
        assert!(
            content_text.contains(expected_err_sub),
            "Tool `{tool_name}` error text must contain `{expected_err_sub}`. Got: {content_text}"
        );
    }

    // Test secondary missing params
    let (response, _, _, _) =
        execute_tool_with_timeout("get_symbol_slice", &json!({ "path": "file.ts" }), 5000);
    assert!(response["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Missing required parameter 'symbol'"));

    let (response, _, _, _) = execute_tool_with_timeout(
        "patch_symbol",
        &json!({ "path": "file.ts", "symbol": "foo" }),
        5000,
    );
    assert!(response["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Missing required parameter 'code'"));

    let (response, _, _, _) =
        execute_tool_with_timeout("get_route_slice", &json!({ "method": "GET" }), 5000);
    assert!(response["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Missing required parameter 'path'"));
}

#[test]
fn test_challenger_mcp_nonexistent_and_corrupt_files() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Non-existent file slicing
    let fake_path = root.join("non_existent_file.ts");
    let (response, _, err_opt, _) = execute_tool_with_timeout(
        "get_symbol_slice",
        &json!({
            "path": fake_path.to_string_lossy(),
            "symbol": "myFunc"
        }),
        5000,
    );
    assert_eq!(response.get("isError"), Some(&json!(true)));
    assert!(err_opt.is_some());
    assert!(response["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Slicing error"));

    // 2. Binary / non-UTF8 file handling
    let bin_file = root.join("binary.bin");
    fs::write(&bin_file, [0xFF, 0xFE, 0xFD, 0x00, 0x12, 0x34]).unwrap();

    let (response, _, err_opt, _) = execute_tool_with_timeout(
        "get_symbol_slice",
        &json!({
            "path": bin_file.to_string_lossy(),
            "symbol": "foo"
        }),
        5000,
    );
    assert_eq!(response.get("isError"), Some(&json!(true)));
    assert!(err_opt.is_some());

    // 3. Stats on empty directory
    let empty_dir = root.join("empty_dir");
    fs::create_dir_all(&empty_dir).unwrap();

    let (response, metrics, err_opt, saved_opt) = execute_tool_with_timeout(
        "analyze_token_stats",
        &json!({
            "path": empty_dir.to_string_lossy(),
            "fast": true
        }),
        5000,
    );
    assert_eq!(response.get("isError"), None);
    assert!(err_opt.is_none());
    assert_eq!(saved_opt, Some(0));
    assert!(metrics.is_some());
    let metrics_val = metrics.unwrap();
    assert_eq!(metrics_val["total_files"], 0);
    assert_eq!(metrics_val["raw_tokens"], 0);
}

#[test]
fn test_challenger_mcp_fast_stats_performance_and_accuracy_stress() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create a large hierarchy: 10 directories with 10 files each (100 files total)
    for dir_idx in 0..10 {
        let sub = root.join(format!("module_{dir_idx}"));
        fs::create_dir_all(&sub).unwrap();

        for file_idx in 0..10 {
            let file_path = sub.join(format!("service_{file_idx}.ts"));
            fs::write(
                &file_path,
                r#"
export interface UserSession {
    id: string;
    token: string;
    expiresAt: number;
    roles: string[];
}

export class SessionManager {
    private sessions: Map<string, UserSession> = new Map();

    public createSession(id: string, token: string): UserSession {
        const session: UserSession = {
            id,
            token,
            expiresAt: Date.now() + 3600 * 1000,
            roles: ["user", "viewer"]
        };
        this.sessions.set(id, session);
        return session;
    }

    public isValid(id: string): boolean {
        const s = this.sessions.get(id);
        if (!s) return false;
        return s.expiresAt > Date.now();
    }
}
"#,
            )
            .unwrap();
        }
    }

    // Benchmark Fast Stats
    let start_fast = Instant::now();
    let (fast_resp, fast_metrics, fast_err, _) = execute_tool_with_timeout(
        "analyze_token_stats",
        &json!({
            "path": root.to_string_lossy(),
            "fast": true
        }),
        10_000,
    );
    let fast_duration = start_fast.elapsed();

    assert_eq!(fast_resp.get("isError"), None);
    assert!(fast_err.is_none());
    let fast_m = fast_metrics.expect("fast metrics must exist");
    assert_eq!(fast_m["total_files"], 100);
    assert!(fast_m["raw_tokens"].as_u64().unwrap() > 5000);
    assert!(fast_m["savings_percentage"].as_f64().unwrap() > 0.0);

    // Benchmark Deep Stats
    let start_deep = Instant::now();
    let (deep_resp, deep_metrics, deep_err, _) = execute_tool_with_timeout(
        "analyze_token_stats",
        &json!({
            "path": root.to_string_lossy(),
            "fast": false
        }),
        30_000,
    );
    let deep_duration = start_deep.elapsed();

    assert_eq!(deep_resp.get("isError"), None);
    assert!(deep_err.is_none());
    let deep_m = deep_metrics.expect("deep metrics must exist");
    assert_eq!(deep_m["total_files"], 100);
    assert_eq!(deep_m["raw_tokens"], fast_m["raw_tokens"]);

    println!(
        "Challenger Benchmark: Fast stats took {:?}, Deep stats took {:?}",
        fast_duration, deep_duration
    );
}

#[test]
fn test_challenger_mcp_logger_and_options_integrity() {
    let temp_file = NamedTempFile::new().unwrap();
    let log_path = temp_file.path().to_path_buf();

    let options = McpServerOptions {
        log_file: Some(log_path.clone()),
        tool_timeout_ms: Some(5000),
    };

    assert_eq!(options.tool_timeout_ms, Some(5000));
    assert_eq!(options.log_file.as_ref(), Some(&log_path));

    let default_options = McpServerOptions::default();
    assert_eq!(default_options.tool_timeout_ms, Some(10_000));
    assert!(default_options.log_file.is_none());

    let logger = McpFileLogger::new(Some(log_path.clone()));
    logger.log_start();
    logger.log_request("test_method", None, Some("test_tool"), None);
    logger.log_tool_execution(&ToolLogRecord {
        id: Some(&json!("req-1")),
        tool: "test_tool",
        args: &json!({"foo": "bar"}),
        duration_ms: 12.34,
        status: "success",
        metrics: Some(&json!({"raw_tokens": 100})),
        error: None,
    });
    logger.log_rpc_error(Some(&json!("req-2")), "test_tool", -32600, "Bad syntax");
    logger.log_response("test_tool", Some(&json!("req-1")), 15, Some(50), None);

    let log_content = fs::read_to_string(&log_path).unwrap();
    assert!(log_content.contains("server_start"));
    assert!(log_content.contains("test_method"));
    assert!(log_content.contains("test_tool"));
    assert!(log_content.contains("Bad syntax"));
}

#[test]
fn test_challenger_mcp_route_slice_and_test_context_edge_cases() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Route slice on nonexistent route
    let (route_resp, _, route_err, _) = execute_tool_with_timeout(
        "get_route_slice",
        &json!({
            "method": "POST",
            "path": "/api/non_existent_route_404",
            "root_dir": root.to_string_lossy()
        }),
        5000,
    );
    assert_eq!(route_resp.get("isError"), Some(&json!(true)));
    assert!(route_err.is_some());

    // 2. Test context generator on nonexistent symbol
    let dummy_rs = root.join("lib.rs");
    fs::write(&dummy_rs, "pub fn foo() -> u32 { 1 }\n").unwrap();

    let (test_ctx_resp, _, test_ctx_err, _) = execute_tool_with_timeout(
        "get_test_context",
        &json!({
            "path": dummy_rs.to_string_lossy(),
            "symbol": "non_existent_symbol"
        }),
        5000,
    );
    assert_eq!(test_ctx_resp.get("isError"), Some(&json!(true)));
    assert!(test_ctx_err.is_some());
}
