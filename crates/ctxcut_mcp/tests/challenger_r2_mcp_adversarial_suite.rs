//! Adversarial MCP JSON-RPC stress test suite for R2 features:
//! 1. `get_workspace_overview` tool execution under varied parameters
//! 2. `get_metrics` telemetry tool execution & clear operations
//! 3. `get_symbol_slice` multi-symbol queries (comma-separated string & array formats)
//! 4. Protocol integrity, error formatting, and timeout boundaries.

use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_mcp_overview_json_format_and_budget() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    for i in 0..5 {
        let file = root.join(format!("handler_{i}.ts"));
        let code = format!(
            r#"
/** Service handler {i} documentation. */
export class Handler{i} {{
    public process(): string {{
        return "step_{i}";
    }}
}}
"#
        );
        fs::write(&file, code).unwrap();
    }

    // Call get_workspace_overview with format = "json" and budget = 200
    let args = json!({
        "path": root.to_string_lossy(),
        "format": "json",
        "budget": 200
    });

    let (response, metrics, error_opt, tokens_saved) =
        execute_tool_with_timeout("get_workspace_overview", &args, 5000);

    assert!(error_opt.is_none(), "Unexpected error: {:?}", error_opt);
    assert_ne!(response.get("isError"), Some(&json!(true)));

    let json_text = response["content"][0]["text"].as_str().unwrap();
    // Validate it is valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(json_text).expect("Must return valid JSON");
    assert_eq!(parsed["total_files"].as_u64(), Some(5));
    assert!(metrics.is_some());
    assert!(tokens_saved.is_some());
}

#[test]
fn test_mcp_symbol_slice_array_and_comma_separated() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let file = root.join("auth.ts");

    fs::write(
        &file,
        r#"
export interface TokenConfig {
    secret: string;
    expiresIn: number;
}

export function generateToken(userId: string, cfg: TokenConfig): string {
    return "tok_" + userId;
}

export function verifyToken(token: string, cfg: TokenConfig): boolean {
    return token.startsWith("tok_");
}
"#,
    )
    .unwrap();

    // Mode A: Comma-separated string in "symbol"
    let args_comma = json!({
        "path": file.to_string_lossy(),
        "symbol": "generateToken, verifyToken"
    });
    let (res_a, metrics_a, err_a, saved_a) =
        execute_tool_with_timeout("get_symbol_slice", &args_comma, 5000);
    assert!(err_a.is_none());
    assert_ne!(res_a.get("isError"), Some(&json!(true)));
    let text_a = res_a["content"][0]["text"].as_str().unwrap();
    assert!(text_a.contains("generateToken, verifyToken"));
    assert_eq!(text_a.matches("interface TokenConfig").count(), 1);
    assert!(metrics_a.is_some());
    assert!(saved_a.is_some());

    // Mode B: Array of strings in "symbols"
    let args_arr = json!({
        "path": file.to_string_lossy(),
        "symbols": ["generateToken", "verifyToken"]
    });
    let (res_b, metrics_b, err_b, saved_b) =
        execute_tool_with_timeout("get_symbol_slice", &args_arr, 5000);
    assert!(err_b.is_none());
    assert_ne!(res_b.get("isError"), Some(&json!(true)));
    let text_b = res_b["content"][0]["text"].as_str().unwrap();
    assert!(text_b.contains("generateToken, verifyToken"));
    assert_eq!(text_b.matches("interface TokenConfig").count(), 1);
    assert!(metrics_b.is_some());
    assert!(saved_b.is_some());
}

#[test]
fn test_mcp_metrics_json_and_clear() {
    // 1. Clear operation
    let args_clear = json!({ "clear": true });
    let (res_clr, _, err_clr, _) = execute_tool_with_timeout("get_metrics", &args_clear, 5000);
    assert!(err_clr.is_none());
    assert_ne!(res_clr.get("isError"), Some(&json!(true)));

    // 2. JSON format on clean state
    let args_json = json!({ "format": "json" });
    let (res_json, metrics_json, err_json, _) =
        execute_tool_with_timeout("get_metrics", &args_json, 5000);
    assert!(err_json.is_none());
    assert_ne!(res_json.get("isError"), Some(&json!(true)));
    assert!(metrics_json.is_some());

    // 3. Markdown format on clean state
    let args_md = json!({ "format": "markdown" });
    let (res_md, metrics_md, err_md, _) = execute_tool_with_timeout("get_metrics", &args_md, 5000);
    assert!(err_md.is_none());
    assert_ne!(res_md.get("isError"), Some(&json!(true)));
    assert!(metrics_md.is_some());
}

#[test]
fn test_mcp_error_handling_invalid_inputs() {
    // 1. Missing path parameter
    let args_no_path = json!({ "symbol": "foo" });
    let (res_err1, _, err1, _) = execute_tool_with_timeout("get_symbol_slice", &args_no_path, 5000);
    assert_eq!(res_err1.get("isError"), Some(&json!(true)));
    assert!(err1.is_some());

    // 2. Missing symbol parameter
    let args_no_sym = json!({ "path": "test.ts" });
    let (res_err2, _, err2, _) = execute_tool_with_timeout("get_symbol_slice", &args_no_sym, 5000);
    assert_eq!(res_err2.get("isError"), Some(&json!(true)));
    assert!(err2.is_some());

    // 3. Non-existent file path
    let args_bad_file = json!({
        "path": "non_existent_directory_12345/missing_file.ts",
        "symbol": "foo"
    });
    let (res_err3, _, err3, _) =
        execute_tool_with_timeout("get_symbol_slice", &args_bad_file, 5000);
    assert_eq!(res_err3.get("isError"), Some(&json!(true)));
    assert!(err3.is_some());

    // 4. Unknown tool name
    let (res_unknown, _, err_unknown, _) =
        execute_tool_with_timeout("unknown_tool_xyz", &json!({}), 5000);
    assert_eq!(res_unknown.get("isError"), Some(&json!(true)));
    assert!(err_unknown.is_some());
}
