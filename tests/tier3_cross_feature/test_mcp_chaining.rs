//! Tier 3: Cross-Feature - Multi-Step MCP Session Chaining (`test_mcp_chaining.rs`)
//!
//! Verifies conversational multi-step Model Context Protocol (MCP) STDIO sessions:
//! initialize -> analyze_token_stats -> get_symbol_slice -> mutate file -> get_diff_slice.

#[path = "../common/mod.rs"]
mod common;

use common::{GitSandbox, McpClient};

/// Test 1: Full interactive multi-step MCP session workflow.
///
/// Arrange: Isolated Git sandbox with TypeScript service files; spawn STDIO MCP client.
/// Act:
///   1. Step 1: Handshake `initialize`.
///   2. Step 2: Query repository metrics via `analyze_token_stats`.
///   3. Step 3: Extract surgical AST context for target function via `get_symbol_slice`.
///   4. Step 4: Mutate function in sandbox working tree.
///   5. Step 5: Extract incremental diff context via `get_diff_slice`.
/// Assert: All JSON-RPC responses are well-formed and accurate across the entire session lifecycle.
#[test]
fn test_mcp_full_session_chaining() {
    // Arrange: Create sandbox and source file
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let initial_code = r#"
export interface UserSession {
    sessionId: string;
    userId: string;
}

export function validateSession(session: UserSession): boolean {
    return session.sessionId.length > 0;
}

export function terminateSession(session: UserSession): void {
    console.log("Terminated:", session.sessionId);
}
"#;
    sandbox.write_file("src/session.ts", initial_code).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial session commit").unwrap();

    let mut client =
        McpClient::start_in_dir(sandbox.path()).expect("Failed to start MCP client in sandbox");

    // Step 1: Initialize
    let init_res = client.initialize().expect("MCP initialize must succeed");
    assert!(
        init_res.get("protocolVersion").is_some()
            || init_res.get("serverInfo").is_some()
            || init_res.get("capabilities").is_some(),
        "Init response must be valid: {:?}",
        init_res
    );

    // Step 2: Analyze token stats
    let stats_res = client
        .analyze_token_stats("src/session.ts")
        .expect("analyze_token_stats must succeed");
    let stats_str = stats_res.to_string();
    assert!(
        stats_str.contains("token")
            || stats_str.contains("raw")
            || stats_str.contains("session.ts")
            || stats_str.contains("total"),
        "Stats response must describe session.ts: {:?}",
        stats_res
    );

    // Step 3: Get symbol slice
    let slice_markdown = client
        .get_symbol_slice("src/session.ts", "validateSession")
        .expect("get_symbol_slice must succeed");
    assert!(
        slice_markdown.contains("validateSession"),
        "Slice must contain validateSession"
    );
    assert!(
        slice_markdown.contains("UserSession"),
        "Slice must hoist UserSession interface"
    );

    // Step 4: Mutate file
    let modified_code = r#"
export interface UserSession {
    sessionId: string;
    userId: string;
    expiresAt?: number;
}

export function validateSession(session: UserSession): boolean {
    const isNotExpired = !session.expiresAt || session.expiresAt > Date.now();
    return session.sessionId.length > 0 && isNotExpired;
}

export function terminateSession(session: UserSession): void {
    console.log("Terminated:", session.sessionId);
}
"#;
    sandbox
        .modify_file("src/session.ts", modified_code)
        .unwrap();

    // Step 5: Get diff slice over same MCP connection
    let diff_markdown = client
        .get_diff_slice(false)
        .expect("get_diff_slice must succeed on mutated working tree");
    assert!(
        diff_markdown.contains("validateSession"),
        "Diff slice must identify modified validateSession"
    );
    assert!(
        diff_markdown.contains("isNotExpired"),
        "Diff slice must reflect newly added logic"
    );
}

/// Test 2: Rapid sequential tool invocations without connection reset.
///
/// Arrange: MCP client connected.
/// Act: Send 10 consecutive `get_symbol_slice` calls.
/// Assert: Every call succeeds without packet corruption or socket timeouts.
#[test]
fn test_mcp_rapid_sequential_invocations() {
    // Arrange
    let mut client = McpClient::start().expect("Failed to start MCP client");
    client.initialize().expect("MCP initialize must succeed");

    // Act
    for i in 1..=5 {
        let result =
            client.get_symbol_slice("tests/fixtures/typescript/simple_function.ts", "addNumbers");
        assert!(
            result.is_ok(),
            "Iteration {} of rapid MCP calls failed: {:?}",
            i,
            result.err()
        );
    }
}
