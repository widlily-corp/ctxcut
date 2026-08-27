//! Integration tests for MCP get_route_slice with Universal IPC, RPC & Modern Frameworks.

use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_mcp_get_route_slice_tauri() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let tauri_dir = root.join("src-tauri").join("src");
    fs::create_dir_all(&tauri_dir).unwrap();
    fs::write(
        tauri_dir.join("main.rs"),
        r#"
        pub struct UserPayload {
            pub name: String,
        }

        #[tauri::command]
        pub fn register_user(payload: UserPayload) -> Result<String, String> {
            Ok(format!("Registered {}", payload.name))
        }
        "#,
    )
    .unwrap();

    // Query with camelCase "registerUser"
    let args = json!({
        "root_dir": root.to_string_lossy(),
        "path": "registerUser",
        "method": "IPC"
    });

    let (response, _metrics, error_opt, _tokens) =
        execute_tool_with_timeout("get_route_slice", &args, 5000);

    assert!(error_opt.is_none(), "Expected no error, got: {:?}", error_opt);
    let text = response["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("register_user"), "Expected text to contain register_user handler, got: {}", text);
    assert!(text.contains("UserPayload"), "Expected hoisted type UserPayload in slice");
}

#[test]
fn test_mcp_get_route_slice_electron() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let electron_dir = root.join("src").join("main");
    fs::create_dir_all(&electron_dir).unwrap();
    fs::write(
        electron_dir.join("index.ts"),
        r#"
        import { ipcMain } from 'electron';

        export async function fetchSystemMetrics() {
            return { memory: "16GB", cpu: "80%" };
        }

        ipcMain.handle('metrics:get-system', fetchSystemMetrics);
        "#,
    )
    .unwrap();

    let args = json!({
        "root_dir": root.to_string_lossy(),
        "channel": "metrics:get-system"
    });

    let (response, _metrics, error_opt, _tokens) =
        execute_tool_with_timeout("get_route_slice", &args, 5000);

    assert!(error_opt.is_none(), "Expected no error, got: {:?}", error_opt);
    let text = response["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("fetchSystemMetrics"), "Expected fetchSystemMetrics symbol in slice");
}

#[test]
fn test_mcp_get_route_slice_trpc() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let router_dir = root.join("server");
    fs::create_dir_all(&router_dir).unwrap();
    fs::write(
        router_dir.join("api.ts"),
        r#"
        import { initTRPC } from '@trpc/server';
        const t = initTRPC.create();
        export const router = t.router;
        export const publicProcedure = t.procedure;

        export const appRouter = router({
            getHealth: publicProcedure.query(() => {
                return { status: "ok" };
            }),
        });
        "#,
    )
    .unwrap();

    let args = json!({
        "root_dir": root.to_string_lossy(),
        "procedure": "/trpc/getHealth",
        "method": "QUERY"
    });

    let (response, _metrics, error_opt, _tokens) =
        execute_tool_with_timeout("get_route_slice", &args, 5000);

    assert!(error_opt.is_none(), "Expected no error, got: {:?}", error_opt);
    let text = response["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("getHealth"), "Expected getHealth procedure in slice");
}

#[test]
fn test_mcp_get_route_slice_next_server_action() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let action_dir = root.join("app").join("actions");
    fs::create_dir_all(&action_dir).unwrap();
    fs::write(
        action_dir.join("auth.ts"),
        r#"
        'use server';

        export async function logoutUser(sessionId: string) {
            return { loggedOut: true };
        }
        "#,
    )
    .unwrap();

    let args = json!({
        "root_dir": root.to_string_lossy(),
        "path": "logoutUser",
        "method": "ACTION"
    });

    let (response, _metrics, error_opt, _tokens) =
        execute_tool_with_timeout("get_route_slice", &args, 5000);

    assert!(error_opt.is_none(), "Expected no error, got: {:?}", error_opt);
    let text = response["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("logoutUser"), "Expected logoutUser action in slice");
}
