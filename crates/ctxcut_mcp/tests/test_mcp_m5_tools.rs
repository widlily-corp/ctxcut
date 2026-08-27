//! Integration tests for Milestone 5 MCP tools:
//! - get_fullstack_trace
//! - get_intent_slice
//! - patch_transaction
//! - pack_agent_context

use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_mcp_get_fullstack_trace() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let client_file = root.join("client.ts");
    fs::write(
        &client_file,
        r#"
export async function createOrder(payload: { item: string }) {
    const res = await fetch("/api/v1/orders", {
        method: "POST",
        body: JSON.stringify(payload),
    });
    return res.json();
}
"#,
    )
    .unwrap();

    let server_file = root.join("server.ts");
    fs::write(
        &server_file,
        r#"
import { Router } from 'express';
const router = Router();

router.post('/api/v1/orders', async (req, res) => {
    const result = await processOrder(req.body);
    res.json(result);
});

export async function processOrder(data: any) {
    return { status: "created", data };
}
"#,
    )
    .unwrap();

    let args = json!({
        "root_dir": root.to_string_lossy(),
        "entry": "POST /api/v1/orders",
        "format": "markdown"
    });

    let (response, metrics, error_opt, tokens_saved) =
        execute_tool_with_timeout("get_fullstack_trace", &args, 5000);

    assert!(error_opt.is_none(), "Expected no error, got: {:?}", error_opt);
    assert_ne!(response.get("isError"), Some(&json!(true)));

    let text = response["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Full-Stack Execution Trace"));
    assert!(text.contains("POST /api/v1/orders") || text.contains("processOrder"));

    assert!(metrics.is_some());
    assert!(tokens_saved.is_some());
}

#[test]
fn test_mcp_get_intent_slice() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let auth_file = root.join("auth.ts");
    fs::write(
        &auth_file,
        r#"
export interface UserSession {
    userId: string;
    token: string;
}

export function authenticateUser(token: string): UserSession {
    return { userId: "user_123", token };
}

export function validateSession(session: UserSession): boolean {
    return session.token.length > 0;
}
"#,
    )
    .unwrap();

    let args = json!({
        "root_dir": root.to_string_lossy(),
        "prompt": "authenticate user token and validate session",
        "format": "markdown"
    });

    let (response, metrics, error_opt, tokens_saved) =
        execute_tool_with_timeout("get_intent_slice", &args, 5000);

    assert!(error_opt.is_none(), "Expected no error, got: {:?}", error_opt);
    assert_ne!(response.get("isError"), Some(&json!(true)));

    let text = response["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Intent Context Slice"));
    assert!(text.contains("authenticateUser") || text.contains("validateSession"));

    assert!(metrics.is_some());
    assert!(tokens_saved.is_some());
}

#[test]
fn test_mcp_patch_transaction() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let calc_file = root.join("calc.ts");
    fs::write(
        &calc_file,
        r#"
export function calculateTax(subtotal: number): number {
    return subtotal * 0.05;
}

export function calculateDiscount(total: number): number {
    return total * 0.10;
}
"#,
    )
    .unwrap();

    // 1. Dry Run Verification
    let dry_run_args = json!({
        "root_dir": root.to_string_lossy(),
        "patches": [
            {
                "file_path": calc_file.to_string_lossy(),
                "symbol_name": "calculateTax",
                "replacement_code": "export function calculateTax(subtotal: number): number {\n    return subtotal * 0.08;\n}"
            }
        ],
        "apply": false
    });

    let (response, metrics, error_opt, _) =
        execute_tool_with_timeout("patch_transaction", &dry_run_args, 5000);

    assert!(error_opt.is_none(), "Expected dry-run success, got: {:?}", error_opt);
    assert_ne!(response.get("isError"), Some(&json!(true)));
    let text = response["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("calculateTax"));
    assert!(metrics.is_some());

    // Verify disk was NOT modified during dry-run
    let content_after_dry = fs::read_to_string(&calc_file).unwrap();
    assert!(content_after_dry.contains("subtotal * 0.05"));

    // 2. Apply Verification
    let apply_args = json!({
        "root_dir": root.to_string_lossy(),
        "patches": [
            {
                "file_path": calc_file.to_string_lossy(),
                "symbol_name": "calculateTax",
                "replacement_code": "export function calculateTax(subtotal: number): number {\n    return subtotal * 0.08;\n}"
            }
        ],
        "apply": true
    });

    let (apply_resp, apply_metrics, apply_err, _) =
        execute_tool_with_timeout("patch_transaction", &apply_args, 5000);

    assert!(apply_err.is_none(), "Expected apply success, got: {:?}", apply_err);
    assert_ne!(apply_resp.get("isError"), Some(&json!(true)));
    assert!(apply_metrics.is_some());

    // Verify disk WAS modified on apply
    let content_after_apply = fs::read_to_string(&calc_file).unwrap();
    assert!(content_after_apply.contains("subtotal * 0.08"));
}

#[test]
fn test_mcp_pack_agent_context() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let auth_file = root.join("auth.ts");
    fs::write(
        &auth_file,
        r#"
export function login(user: string): string { return "token_" + user; }
export function logout(tok: string): boolean { return tok.length > 0; }
"#,
    )
    .unwrap();

    let billing_file = root.join("billing.ts");
    fs::write(
        &billing_file,
        r#"
export function charge(amount: number): boolean { return amount > 0; }
export function refund(id: string): boolean { return id.length > 0; }
"#,
    )
    .unwrap();

    let args = json!({
        "root_dir": root.to_string_lossy(),
        "agents_count": 2,
        "format": "markdown"
    });

    let (response, metrics, error_opt, tokens_saved) =
        execute_tool_with_timeout("pack_agent_context", &args, 5000);

    assert!(error_opt.is_none(), "Expected no error, got: {:?}", error_opt);
    assert_ne!(response.get("isError"), Some(&json!(true)));

    let text = response["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("# Swarm Multi-Agent Context Partition Manifest"));
    assert!(text.contains("agent_0") || text.contains("agent_1"));

    assert!(metrics.is_some());
    assert!(tokens_saved.is_some());
}
