//! Integration tests for Milestone 5 CLI subcommands:
//! - trace-api
//! - slice-intent
//! - refactor batch
//! - pack-agent-context

use ctxcut_cli::{
    run_pack_agent_context_command, run_refactor_batch, run_slice_intent_command,
    run_trace_api_command, RefactorBatchOptions, SliceIntentCliOptions,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_cli_trace_api_command() {
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

    let out_file = root.join("trace_output.md");

    let res = run_trace_api_command(
        "POST /api/v1/orders",
        Some(root.to_path_buf()),
        Some(1500),
        None,
        false,
        Some(&out_file),
        "markdown",
    );

    assert!(res.is_ok(), "trace_api command failed: {:?}", res);
    assert!(out_file.exists());
    let content = fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("Full-Stack Execution Trace"));
    assert!(content.contains("POST /api/v1/orders") || content.contains("processOrder"));
}

#[test]
fn test_cli_slice_intent_command() {
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

    let out_file = root.join("intent_output.md");

    let opts = SliceIntentCliOptions {
        prompt: "authenticate user and validate session token",
        root_dir: Some(root),
        budget: Some(1500),
        max_symbols: Some(5),
        depth: Some(1),
        clip: false,
        output: Some(&out_file),
        format: "markdown",
    };

    let res = run_slice_intent_command(opts);
    assert!(res.is_ok(), "slice_intent command failed: {:?}", res);
    assert!(out_file.exists());
    let content = fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("Intent Context Slice"));
    assert!(content.contains("authenticateUser") || content.contains("validateSession"));
}

#[test]
fn test_cli_refactor_batch_command() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let file_path = root.join("calc.ts");
    fs::write(
        &file_path,
        r#"
export function calculateTax(subtotal: number): number {
    return subtotal * 0.05;
}
"#,
    )
    .unwrap();

    let patches_json = format!(
        r#"[{{"file_path": "{}", "symbol_name": "calculateTax", "replacement_code": "export function calculateTax(subtotal: number): number {{\n    return subtotal * 0.08;\n}}"}}]"#,
        file_path.display().to_string().replace('\\', "/")
    );

    let out_file = root.join("batch_report.md");

    let opts = RefactorBatchOptions {
        patches: Some(&patches_json),
        file: None,
        root: Some(root),
        typechecker: None,
        apply: true,
        dry_run: false,
        timeout_ms: Some(5000),
        format: "markdown",
        clip: false,
        output: Some(&out_file),
    };

    let res = run_refactor_batch(opts);
    assert!(res.is_ok(), "refactor batch command failed: {:?}", res);
    assert!(out_file.exists());

    // Verify disk change
    let modified = fs::read_to_string(&file_path).unwrap();
    assert!(modified.contains("subtotal * 0.08"));
}

#[test]
fn test_cli_pack_agent_context_command() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let auth_file = root.join("auth.ts");
    fs::write(
        &auth_file,
        r#"
export function login(user: string): string { return "tok_" + user; }
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

    let out_file = root.join("swarm_manifest.json");

    let res = run_pack_agent_context_command(
        Some(root.to_path_buf()),
        Some(2),
        None,
        Some(1500),
        false,
        Some(&out_file),
        "json",
    );

    assert!(res.is_ok(), "pack_agent_context command failed: {:?}", res);
    assert!(out_file.exists());
    let content = fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("agent_0"));
    assert!(content.contains("total_agents"));
}
