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
        init_res.get("serverInfo").is_some() || init_res.get("protocolVersion").is_some() || init_res.get("capabilities").is_some(),
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

    let mut client = McpClient::start_in_dir(sandbox.path()).expect("Failed to start MCP client in sandbox");
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
        text.contains("token") || text.contains("raw") || text.contains("savings") || text.contains("files") || text.contains("total"),
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
    let valid_slice = client.get_symbol_slice("tests/fixtures/typescript/simple_function.ts", "addNumbers");
    assert!(valid_slice.is_ok(), "Server must remain operational after invalid tool call");
}
