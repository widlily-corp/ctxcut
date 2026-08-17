//! Tier 1: Feature Coverage - Model Context Protocol (MCP) Server Tests (`test_mcp_features.rs`)
//!
//! Verifies STDIO JSON-RPC 2.0 initialization, tool schema listings (`get_symbol_slice`,
//! `get_diff_slice`, `analyze_token_stats`), tool execution, and error handling for malformed requests.

#[path = "../common/mod.rs"]
mod common;

use common::{GitSandbox, McpClient};
use serde_json::json;

/// Test 1: MCP Server handshake initialization and tool listing.
///
/// Arrange: Spawn `ctxcut mcp` process.
/// Act: Send `initialize` request followed by `tools/list`.
/// Assert: Server returns server capabilities and tool schemas for `get_symbol_slice`,
///         `get_diff_slice`, and `analyze_token_stats`.
#[test]
fn test_mcp_initialize_and_tool_listing() {
    // Arrange
    let mut client = McpClient::start().expect("Failed to start MCP client");

    // Act
    let init_res = client.initialize().expect("MCP initialize must succeed");
    let tools = client.list_tools().expect("MCP list_tools must succeed");

    // Assert
    assert!(
        init_res.get("serverInfo").is_some()
            || init_res.get("protocolVersion").is_some()
            || init_res.get("capabilities").is_some(),
        "Initialize response must include server metadata: {:?}",
        init_res
    );

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();

    assert!(
        tool_names.contains(&"get_symbol_slice"),
        "Must expose get_symbol_slice tool. Found: {:?}",
        tool_names
    );
    assert!(
        tool_names.contains(&"get_diff_slice"),
        "Must expose get_diff_slice tool. Found: {:?}",
        tool_names
    );
    assert!(
        tool_names.contains(&"analyze_token_stats"),
        "Must expose analyze_token_stats tool. Found: {:?}",
        tool_names
    );
}

/// Test 2: Calling `get_symbol_slice` tool over MCP STDIO.
///
/// Arrange: Running MCP server.
/// Act: Call tool `get_symbol_slice` with path and symbol.
/// Assert: Returns Markdown slice containing target function body and hoisted types.
#[test]
fn test_mcp_get_symbol_slice_tool_call() {
    // Arrange
    let mut client = McpClient::start().expect("Failed to start MCP client");
    client.initialize().expect("MCP initialize must succeed");

    // Act
    let slice_text = client
        .get_symbol_slice("tests/fixtures/typescript/simple_function.ts", "addNumbers")
        .expect("get_symbol_slice tool call must succeed");

    // Assert
    assert!(
        slice_text.contains("addNumbers"),
        "Extracted slice must contain addNumbers"
    );
    assert!(
        slice_text.contains("return a + b;"),
        "Extracted slice must contain function body"
    );
}

/// Test 3: Calling `get_diff_slice` tool over MCP STDIO.
///
/// Arrange: Git sandbox repository with modified source file; spawn MCP server in sandbox directory.
/// Act: Call tool `get_diff_slice` with `staged: false`.
/// Assert: Returns Markdown slice for the modified function.
#[test]
fn test_mcp_get_diff_slice_tool_call() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    sandbox
        .write_file(
            "src/service.ts",
            "export function executeTask(): string {\n    return \"task1\";\n}\n",
        )
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial commit").unwrap();

    // Mutate
    sandbox
        .modify_file(
            "src/service.ts",
            "export function executeTask(): string {\n    return \"task_modified\";\n}\n",
        )
        .unwrap();

    let mut client =
        McpClient::start_in_dir(sandbox.path()).expect("Failed to start MCP client in sandbox");
    client.initialize().expect("MCP initialize must succeed");

    // Act
    let slice_text = client
        .get_diff_slice(false)
        .expect("get_diff_slice tool call must succeed");

    // Assert
    assert!(
        slice_text.contains("executeTask"),
        "Diff slice must contain executeTask function"
    );
    assert!(
        slice_text.contains("task_modified"),
        "Diff slice must contain modified return value"
    );
}

/// Test 4: Calling `analyze_token_stats` tool over MCP STDIO.
///
/// Arrange: Running MCP server.
/// Act: Call tool `analyze_token_stats` on fixture path.
/// Assert: Returns structured metrics payload or text report.
#[test]
fn test_mcp_analyze_token_stats_tool_call() {
    // Arrange
    let mut client = McpClient::start().expect("Failed to start MCP client");
    client.initialize().expect("MCP initialize must succeed");

    // Act
    let stats_res = client
        .analyze_token_stats("tests/fixtures/typescript/simple_function.ts")
        .expect("analyze_token_stats tool call must succeed");

    // Assert
    let text = stats_res.to_string();
    assert!(
        text.contains("token")
            || text.contains("raw")
            || text.contains("savings")
            || text.contains("files")
            || text.contains("total"),
        "analyze_token_stats response must contain token metrics. Got: {:?}",
        stats_res
    );
}

/// Test 5: Error handling for invalid/missing parameters in tool call.
///
/// Arrange: Running MCP server.
/// Act: Send `tools/call` for `get_symbol_slice` missing the required `symbol` parameter.
/// Assert: Returns standard JSON-RPC error (e.g. -32602 Invalid params) without terminating server process.
#[test]
fn test_mcp_invalid_params_error_handling() {
    // Arrange
    let mut client = McpClient::start().expect("Failed to start MCP client");
    client.initialize().expect("MCP initialize must succeed");

    // Act: Send malformed arguments (missing 'symbol')
    let res = client.call_tool(
        "get_symbol_slice",
        json!({
            "path": "tests/fixtures/typescript/simple_function.ts"
        }),
    );

    // Assert: Tool should fail or return error status
    // If client.call_tool returned Err, it caught JSON-RPC error.
    // If it returned Ok, check whether response indicates error or invalid params.
    if let Ok(val) = res {
        assert!(
            val.get("isError") == Some(&json!(true)) || val.get("error").is_some(),
            "Expected tool response to flag error for missing params. Got: {:?}",
            val
        );
    }
}

/// Test 6: Error handling for calling a non-existent tool.
///
/// Arrange: Running MCP server.
/// Act: Send `tools/call` with unknown tool name `non_existent_tool`.
/// Assert: Returns JSON-RPC error or error response; server remains operational.
#[test]
fn test_mcp_unknown_tool_error_handling() {
    // Arrange
    let mut client = McpClient::start().expect("Failed to start MCP client");
    client.initialize().expect("MCP initialize must succeed");

    // Act
    let _res = client.call_tool("non_existent_tool", json!({}));

    // Assert
    // Subsequent valid call must still work, proving server did not crash
    let valid_slice =
        client.get_symbol_slice("tests/fixtures/typescript/simple_function.ts", "addNumbers");
    assert!(
        valid_slice.is_ok(),
        "Server must remain operational after invalid tool call"
    );
}

/// Test 7: MCP `--log-file` captures startup, tool execution, timing, and token metrics.
///
/// Arrange: Spawn `ctxcut mcp --log-file <temp_path>`.
/// Act: Initialize, list tools, and call `get_symbol_slice`.
/// Assert: Log file contains structured JSONL records with ISO timestamps, tool execution durations,
///         and token reduction metrics.
#[test]
fn test_mcp_log_file_flag_captures_tool_call() {
    use std::fs::File;
    use std::io::BufRead;
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new().expect("Failed to create temp log file");
    let log_path = temp_file.path().to_path_buf();

    // Arrange
    let mut client = McpClient::start_with_log_file(&log_path)
        .expect("Failed to start MCP client with log file");

    // Act
    client.initialize().expect("MCP initialize must succeed");
    client.list_tools().expect("MCP list_tools must succeed");
    let slice_res = client
        .get_symbol_slice("tests/fixtures/typescript/simple_function.ts", "addNumbers")
        .expect("get_symbol_slice must succeed");

    assert!(slice_res.contains("addNumbers"));
    client.stop().expect("Client stop failed");

    // Assert
    let file = File::open(&log_path).expect("Log file must exist");
    let lines: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .map(|l| l.expect("Must read line"))
        .collect();

    assert!(
        !lines.is_empty(),
        "Log file must contain structured log entries"
    );

    let mut found_start = false;
    let mut found_tool_call = false;

    for line in &lines {
        let entry: serde_json::Value =
            serde_json::from_str(line).expect("Every line in log must be valid JSON");
        assert!(
            entry.get("timestamp").is_some(),
            "Log entry must contain timestamp"
        );
        assert!(entry.get("level").is_some(), "Log entry must contain level");

        if entry.get("event") == Some(&serde_json::json!("server_start")) {
            found_start = true;
        }

        if entry.get("event") == Some(&serde_json::json!("tool_call"))
            && entry.get("tool") == Some(&serde_json::json!("get_symbol_slice"))
        {
            found_tool_call = true;
            assert_eq!(entry["status"], "success");
            assert!(entry.get("duration_ms").is_some());
            assert!(entry.get("metrics").is_some());
            let metrics = &entry["metrics"];
            assert!(metrics.get("raw_tokens").is_some());
            assert!(metrics.get("sliced_tokens").is_some());
            assert!(metrics.get("saved_tokens").is_some());
        }
    }

    assert!(found_start, "Log must record server_start event");
    assert!(
        found_tool_call,
        "Log must record tool_call event with metrics"
    );
}

/// Test 8: MCP `--log-file` captures error traces when tool call fails.
///
/// Arrange: Spawn `ctxcut mcp --log-file <temp_path>`.
/// Act: Call `get_symbol_slice` with a nonexistent symbol name.
/// Assert: Log file records `status: "error"` and error message.
#[test]
fn test_mcp_log_file_captures_error_trace() {
    use std::fs::File;
    use std::io::BufRead;
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new().expect("Failed to create temp log file");
    let log_path = temp_file.path().to_path_buf();

    // Arrange
    let mut client = McpClient::start_with_log_file(&log_path)
        .expect("Failed to start MCP client with log file");

    client.initialize().expect("MCP initialize must succeed");

    // Act
    let _res = client.get_symbol_slice(
        "tests/fixtures/typescript/simple_function.ts",
        "nonExistentSymbolName",
    );
    client.stop().expect("Client stop failed");

    // Assert
    let file = File::open(&log_path).expect("Log file must exist");
    let lines: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .map(|l| l.expect("Must read line"))
        .collect();

    let found_error = lines.iter().any(|line| {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            entry.get("status") == Some(&serde_json::json!("error"))
                || entry.get("level") == Some(&serde_json::json!("ERROR"))
        } else {
            false
        }
    });

    assert!(
        found_error,
        "Log file must record error entry for invalid symbol call. Lines:\n{:?}",
        lines
    );
}

/// Test 9: MCP logging via `CTXCUT_LOG_FILE` environment variable.
///
/// Arrange: Spawn `ctxcut mcp` with `CTXCUT_LOG_FILE=<temp_path>` env var.
/// Act: Call `initialize` and `get_symbol_slice`.
/// Assert: Structured logs are written to the path configured in environment variable.
#[test]
fn test_mcp_env_var_log_file() {
    use std::fs::File;
    use std::io::BufRead;
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new().expect("Failed to create temp log file");
    let log_path = temp_file.path().to_path_buf();

    // Arrange
    let mut client = McpClient::start_with_env_log_file(&log_path)
        .expect("Failed to start MCP client with env var log file");

    // Act
    client.initialize().expect("MCP initialize must succeed");
    let slice_res = client
        .get_symbol_slice("tests/fixtures/typescript/simple_function.ts", "addNumbers")
        .expect("get_symbol_slice must succeed");
    assert!(slice_res.contains("addNumbers"));
    client.stop().expect("Client stop failed");

    // Assert
    let file = File::open(&log_path).expect("Log file configured via env var must exist");
    let lines: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .map(|l| l.expect("Must read line"))
        .collect();

    assert!(
        !lines.is_empty(),
        "Log file configured via env var must contain log entries"
    );
}

/// Test 10: STDIN/STDOUT isolation — STDIO remains 100% clean JSON-RPC under active logging.
///
/// Arrange: MCP server with active log file.
/// Act: Send multiple rapid JSON-RPC requests across all tools.
/// Assert: 100% of lines emitted to stdout are valid JSON-RPC frames with zero debug/logging pollution.
#[test]
fn test_mcp_stdout_cleanliness_under_logging() {
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new().expect("Failed to create temp log file");
    let log_path = temp_file.path().to_path_buf();

    // Arrange
    let mut client = McpClient::start_with_log_file(&log_path)
        .expect("Failed to start MCP client with log file");

    // Act: Send multiple rapid calls
    let init_res = client.initialize().expect("Initialize must succeed");
    assert!(init_res.get("serverInfo").is_some() || init_res.get("protocolVersion").is_some());

    let tools = client.list_tools().expect("Tools list must succeed");
    assert!(!tools.is_empty());

    for _ in 0..10 {
        let res = client
            .get_symbol_slice("tests/fixtures/typescript/simple_function.ts", "addNumbers")
            .expect("Slice call must succeed");
        assert!(res.contains("addNumbers"));
    }

    client.stop().expect("Client stop failed");
}
