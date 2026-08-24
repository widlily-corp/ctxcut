//! Adversarial Challenger 2 Stress Tests for MCP `get_impact_slice` and `get_trace_slice`.
//!
//! Tests the robustness of:
//! 1. `get_impact_slice` and `get_trace_slice` under JSON-RPC execution.
//! 2. Timeout deadline enforcement (0ms and low timeout thresholds).
//! 3. Missing parameter and invalid argument diagnostics.
//! 4. High concurrency (20 simultaneous threads executing impact and trace slices).
//! 5. Cyclic execution flow handling under MCP framing.

#![allow(clippy::needless_raw_string_hashes)]

use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use std::thread;
use tempfile::TempDir;

/// Creates a multi-file workspace with call graphs and execution traces across services.
fn setup_sample_workspace() -> TempDir {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Controller / Route layer
    fs::write(
        root.join("controller.ts"),
        r#"
import { OrderService } from './service';

export class OrderController {
    private service: OrderService = new OrderService();

    // POST /api/v1/orders
    public async createOrder(req: { body: any }): Promise<any> {
        return await this.service.processOrder(req.body);
    }
}
"#,
    )
    .unwrap();

    // 2. Service layer
    fs::write(
        root.join("service.ts"),
        r#"
import { OrderRepository } from './repository';

export class OrderService {
    private repo: OrderRepository = new OrderRepository();

    public async processOrder(order: any): Promise<any> {
        const validated = this.validateOrder(order);
        return await this.repo.saveOrder(validated);
    }

    public validateOrder(order: any): any {
        return order;
    }
}
"#,
    )
    .unwrap();

    // 3. Repository layer
    fs::write(
        root.join("repository.ts"),
        r#"
export class OrderRepository {
    public async saveOrder(order: any): Promise<any> {
        return { id: "order_123", status: "created", ...order };
    }
}
"#,
    )
    .unwrap();

    // 4. Recursive cycle files for testing loop protection
    fs::write(
        root.join("cyclic_a.ts"),
        r#"
import { stepB } from './cyclic_b';

export function stepA(n: number): number {
    if (n <= 0) return 0;
    return stepB(n - 1);
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("cyclic_b.ts"),
        r#"
import { stepA } from './cyclic_a';

export function stepB(n: number): number {
    if (n <= 0) return 0;
    return stepA(n - 1);
}
"#,
    )
    .unwrap();

    temp
}

#[test]
fn test_mcp_get_impact_slice_execution_and_formats() {
    let ws = setup_sample_workspace();
    let root = ws.path();

    // Markdown format
    let args_md = json!({
        "target": "saveOrder",
        "root_dir": root.to_string_lossy(),
        "format": "markdown"
    });

    let (resp_md, metrics_md, err_md, saved_md) =
        execute_tool_with_timeout("get_impact_slice", &args_md, 10_000);

    assert_eq!(resp_md.get("isError"), None);
    assert!(err_md.is_none());
    assert!(metrics_md.is_some());
    assert!(saved_md.is_some());

    let content_text = resp_md["content"][0]["text"].as_str().unwrap();
    assert!(
        content_text.contains("Impact Analysis") || content_text.contains("Callers"),
        "Markdown output must contain impact headers: {content_text}"
    );
    assert!(content_text.contains("processOrder"));

    // JSON format
    let args_json = json!({
        "target": "saveOrder",
        "root_dir": root.to_string_lossy(),
        "format": "json"
    });

    let (resp_json, _, _, _) = execute_tool_with_timeout("get_impact_slice", &args_json, 10_000);
    assert_eq!(resp_json.get("isError"), None);
    assert!(resp_json.get("impact").is_some());
    let impact_obj = &resp_json["impact"];
    assert_eq!(impact_obj["target_symbol"], "saveOrder");
    assert!(impact_obj["total_callers"].as_u64().unwrap() >= 1);
}

#[test]
fn test_mcp_get_trace_slice_execution_and_formats() {
    let ws = setup_sample_workspace();
    let root = ws.path();

    // Markdown format
    let args_md = json!({
        "entry_point": "OrderController.createOrder",
        "root_dir": root.to_string_lossy(),
        "format": "markdown"
    });

    let (resp_md, metrics_md, err_md, saved_md) =
        execute_tool_with_timeout("get_trace_slice", &args_md, 10_000);

    assert_eq!(resp_md.get("isError"), None);
    assert!(err_md.is_none());
    assert!(metrics_md.is_some());
    assert!(saved_md.is_some());

    let content_text = resp_md["content"][0]["text"].as_str().unwrap();
    assert!(
        content_text.contains("Execution Flow Trace"),
        "Markdown output must contain trace header: {content_text}"
    );

    // JSON format
    let args_json = json!({
        "entry_point": "OrderController.createOrder",
        "root_dir": root.to_string_lossy(),
        "format": "json"
    });

    let (resp_json, _, _, _) = execute_tool_with_timeout("get_trace_slice", &args_json, 10_000);
    assert_eq!(resp_json.get("isError"), None);
    assert!(resp_json.get("trace").is_some());
    let trace_obj = &resp_json["trace"];
    assert_eq!(trace_obj["entry_point"], "OrderController.createOrder");
    assert!(trace_obj["total_steps"].as_u64().unwrap() >= 2);
}

#[test]
fn test_mcp_impact_and_trace_missing_parameters() {
    // 1. Missing target in get_impact_slice
    let (resp_imp, _, err_imp, _) = execute_tool_with_timeout("get_impact_slice", &json!({}), 5000);
    assert_eq!(resp_imp.get("isError"), Some(&json!(true)));
    assert!(err_imp.is_some());
    assert!(resp_imp["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Missing required parameter 'target'"));

    // 2. Missing entry_point in get_trace_slice
    let (resp_trace, _, err_trace, _) =
        execute_tool_with_timeout("get_trace_slice", &json!({}), 5000);
    assert_eq!(resp_trace.get("isError"), Some(&json!(true)));
    assert!(err_trace.is_some());
    assert!(resp_trace["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Missing required parameter 'entry_point'"));
}

#[test]
fn test_mcp_impact_and_trace_timeout_enforcement() {
    let ws = setup_sample_workspace();
    let root = ws.path();

    // 1. 0ms timeout on get_impact_slice
    let args_imp = json!({
        "target": "saveOrder",
        "root_dir": root.to_string_lossy()
    });
    let (resp_imp, _, err_imp, _) = execute_tool_with_timeout("get_impact_slice", &args_imp, 0);
    assert_eq!(resp_imp.get("isError"), Some(&json!(true)));
    assert!(err_imp.is_some());
    assert!(resp_imp["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Timeout"));
    assert_eq!(resp_imp["timeout"]["tool"], "get_impact_slice");

    // 2. 0ms timeout on get_trace_slice
    let args_trace = json!({
        "entry_point": "OrderController.createOrder",
        "root_dir": root.to_string_lossy()
    });
    let (resp_trace, _, err_trace, _) =
        execute_tool_with_timeout("get_trace_slice", &args_trace, 0);
    assert_eq!(resp_trace.get("isError"), Some(&json!(true)));
    assert!(err_trace.is_some());
    assert!(resp_trace["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Timeout"));
    assert_eq!(resp_trace["timeout"]["tool"], "get_trace_slice");
}

#[test]
fn test_mcp_trace_cycle_detection_resilience() {
    let ws = setup_sample_workspace();
    let root = ws.path();

    let args = json!({
        "entry_point": "stepA",
        "root_dir": root.to_string_lossy(),
        "depth": 10
    });

    // Should complete cleanly without stack overflow or infinite loop
    let (resp, metrics, err, _) = execute_tool_with_timeout("get_trace_slice", &args, 5000);
    assert_eq!(resp.get("isError"), None);
    assert!(err.is_none());
    assert!(metrics.is_some());
    let trace_obj = &resp["trace"];
    assert!(trace_obj["total_steps"].as_u64().unwrap() >= 2);
}

#[test]
fn test_mcp_concurrency_stress_20_threads_simultaneous_calls() {
    let ws = setup_sample_workspace();
    let root_path = ws.path().to_string_lossy().to_string();
    let root_arc = Arc::new(root_path);

    let mut handles = Vec::new();

    // Spawn 20 threads: 10 calling get_impact_slice and 10 calling get_trace_slice
    for i in 0..20 {
        let root_clone = Arc::clone(&root_arc);
        let handle = thread::spawn(move || {
            if i % 2 == 0 {
                let args = json!({
                    "target": "saveOrder",
                    "root_dir": root_clone.as_str(),
                    "format": "json"
                });
                let (resp, _, err, _) =
                    execute_tool_with_timeout("get_impact_slice", &args, 10_000);
                assert_eq!(
                    resp.get("isError"),
                    None,
                    "Thread {i} impact failed: {:?}",
                    err
                );
                assert!(resp.get("impact").is_some());
            } else {
                let args = json!({
                    "entry_point": "OrderController.createOrder",
                    "root_dir": root_clone.as_str(),
                    "format": "json"
                });
                let (resp, _, err, _) = execute_tool_with_timeout("get_trace_slice", &args, 10_000);
                assert_eq!(
                    resp.get("isError"),
                    None,
                    "Thread {i} trace failed: {:?}",
                    err
                );
                assert!(resp.get("trace").is_some());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle
            .join()
            .expect("Worker thread panicked under concurrency");
    }
}
