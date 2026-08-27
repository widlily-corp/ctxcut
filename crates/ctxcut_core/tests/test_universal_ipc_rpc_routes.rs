//! Universal IPC, RPC & Modern Framework Route Resolution integration tests.
//!
//! Covers:
//! - Tauri `#[tauri::command]` handlers and TS `invoke(...)` calls with snake_case <-> camelCase normalization
//! - Electron `ipcMain.handle` / `ipcMain.on` and renderer `ipcRenderer.invoke` / `ipcRenderer.send`
//! - tRPC router procedures (`query`, `mutation`) and client `useQuery` / `useMutation`
//! - Next.js Server Actions (module-level and function-level `'use server'`) and JSX `<form action={...}>` / `useActionState`

use ctxcut_core::framework::extract_server_routes;
use ctxcut_core::fullstack::{ClientDetector, FullstackExecutionTracer, FullstackTracer, RouteMatcher};
use ctxcut_core::model::SliceOptions;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn test_tauri_command_extraction_and_dto_hoisting() {
    let rust_code = r#"
        use tauri::{AppHandle, State, Window};

        pub struct CalculateTaxRequest {
            pub annual_income: f64,
            pub deductions: f64,
            pub state_code: String,
        }

        pub struct TaxSummary {
            pub gross_income: f64,
            pub total_tax: f64,
            pub effective_rate: f64,
        }

        pub struct AppState {
            pub db_pool: String,
        }

        #[tauri::command]
        pub fn greet(name: &str) -> String {
            format!("Hello, {}!", name)
        }

        #[tauri::command]
        pub async fn calculate_tax(
            req: CalculateTaxRequest,
            state: State<'_, AppState>,
            window: Window,
        ) -> Result<TaxSummary, String> {
            let tax = (req.annual_income - req.deductions) * 0.25;
            Ok(TaxSummary {
                gross_income: req.annual_income,
                total_tax: tax,
                effective_rate: 0.25,
            })
        }

        #[command]
        pub fn close_application(app: AppHandle) {
            std::process::exit(0);
        }
    "#;

    let routes = extract_server_routes(Path::new("src-tauri/src/commands.rs"), rust_code);
    assert_eq!(routes.len(), 3, "Expected 3 Tauri routes, found {}", routes.len());

    let greet = routes.iter().find(|r| r.handler_symbol == "greet").unwrap();
    assert_eq!(greet.framework, "tauri");
    assert_eq!(greet.http_method, "IPC");

    let calc = routes.iter().find(|r| r.handler_symbol == "calculate_tax").unwrap();
    assert_eq!(calc.framework, "tauri");
    assert_eq!(calc.http_method, "IPC");
    assert_eq!(calc.route_path, "calculate_tax");
    assert!(calc.request_dto_type.is_some(), "Expected request DTO for calculate_tax");
    assert_eq!(calc.request_dto_type.as_ref().unwrap().name, "CalculateTaxRequest");
    assert!(calc.response_dto_type.is_some(), "Expected response DTO for calculate_tax");
    assert_eq!(calc.response_dto_type.as_ref().unwrap().name, "TaxSummary");

    let close = routes.iter().find(|r| r.handler_symbol == "close_application").unwrap();
    assert_eq!(close.framework, "tauri");
    assert_eq!(close.http_method, "IPC");
}

#[test]
fn test_tauri_frontend_client_detection_and_casing_matching() {
    let ts_code = r#"
        import { invoke } from '@tauri-apps/api/core';

        interface TaxSummary {
            gross_income: number;
            total_tax: number;
            effective_rate: number;
        }

        export async function computeTaxes(income: number, deductions: number) {
            const greeting = await invoke('greet', { name: 'Alice' });
            const taxResult = await invoke<TaxSummary>('calculateTax', {
                annual_income: income,
                deductions,
                state_code: 'CA'
            });
            await invoke('close_application');
            return { greeting, taxResult };
        }
    "#;

    let detector = ClientDetector::new();
    let calls = detector.detect_in_file(Path::new("src/services/tauriClient.ts"), ts_code);
    assert!(calls.len() >= 3, "Expected at least 3 Tauri calls, found {}", calls.len());

    let calc_call = calls.iter().find(|c| c.rpc_procedure.as_deref() == Some("calculateTax")).unwrap();
    assert_eq!(calc_call.client_kind, "tauri");
    assert_eq!(calc_call.http_method.as_deref(), Some("IPC"));
    assert_eq!(calc_call.response_dto.as_deref(), Some("TaxSummary"));

    // Verify bidirectional normalization in RouteMatcher
    let rust_code = r#"
        #[tauri::command]
        pub fn calculate_tax(req: CalculateTaxRequest) -> Result<TaxSummary, String> {
            Ok(TaxSummary { gross_income: 0.0, total_tax: 0.0, effective_rate: 0.0 })
        }
    "#;
    let routes = extract_server_routes(Path::new("src-tauri/src/main.rs"), rust_code);
    assert_eq!(routes.len(), 1);

    let matcher = RouteMatcher::new();
    let matched = matcher.match_client_to_server(calc_call, &routes);
    assert!(matched.is_some(), "Expected client invoke('calculateTax') to match server calculate_tax");
    assert_eq!(matched.unwrap().handler_symbol, "calculate_tax");

    // Also test find_best_server_route with queries
    let query_match1 = matcher.find_best_server_route("IPC calculateTax", &routes);
    assert!(query_match1.is_some());
    assert_eq!(query_match1.unwrap().handler_symbol, "calculate_tax");

    let query_match2 = matcher.find_best_server_route("calculate_tax", &routes);
    assert!(query_match2.is_some());
    assert_eq!(query_match2.unwrap().handler_symbol, "calculate_tax");
}

#[test]
fn test_electron_ipc_extraction_and_resolution() {
    let main_code = r#"
        import { app, BrowserWindow, ipcMain, dialog } from 'electron';

        ipcMain.handle('dialog:openFile', async (event, options) => {
            const { canceled, filePaths } = await dialog.showOpenDialog(options);
            if (!canceled) {
                return filePaths[0];
            }
        });

        ipcMain.handle('app:get-version', () => {
            return app.getVersion();
        });

        ipcMain.on('ping-sync', (event, arg) => {
            event.returnValue = 'pong';
        });
    "#;

    let routes = extract_server_routes(Path::new("electron/main.ts"), main_code);
    assert_eq!(routes.len(), 3, "Expected 3 Electron IPC routes, found {}", routes.len());

    let open_file = routes.iter().find(|r| r.route_path == "dialog:openFile").unwrap();
    assert_eq!(open_file.framework, "electron");
    assert_eq!(open_file.http_method, "IPC_HANDLE");

    let get_ver = routes.iter().find(|r| r.route_path == "app:get-version").unwrap();
    assert_eq!(get_ver.framework, "electron");
    assert_eq!(get_ver.http_method, "IPC_HANDLE");

    let ping = routes.iter().find(|r| r.route_path == "ping-sync").unwrap();
    assert_eq!(ping.framework, "electron");
    assert_eq!(ping.http_method, "IPC_ON");

    // Test client calls
    let renderer_code = r#"
        import { ipcRenderer } from 'electron';

        export async function openDocument() {
            const filePath = await ipcRenderer.invoke('dialog:openFile', { properties: ['openFile'] });
            const version = await ipcRenderer.invoke('app:get-version');
            const pong = ipcRenderer.send('ping-sync', 'ping');
            return { filePath, version, pong };
        }
    "#;

    let detector = ClientDetector::new();
    let calls = detector.detect_in_file(Path::new("src/renderer.ts"), renderer_code);
    assert!(calls.len() >= 3, "Expected at least 3 Electron client calls, found {}", calls.len());

    let invoke_open = calls.iter().find(|c| c.rpc_procedure.as_deref() == Some("dialog:openFile")).unwrap();
    assert_eq!(invoke_open.client_kind, "electron");
    assert_eq!(invoke_open.http_method.as_deref(), Some("IPC_HANDLE"));

    let matcher = RouteMatcher::new();
    let matched = matcher.match_client_to_server(invoke_open, &routes);
    assert!(matched.is_some(), "Expected Electron invoke match");
    assert_eq!(matched.unwrap().route_path, "dialog:openFile");
}

#[test]
fn test_trpc_router_extraction_and_resolution() {
    let backend_code = r#"
        import { initTRPC } from '@trpc/server';
        import { z } from 'zod';

        const t = initTRPC.create();
        export const router = t.router;
        export const publicProcedure = t.procedure;

        export const UserSchema = z.object({
            id: z.string(),
            name: z.string(),
        });

        export const appRouter = router({
            getUser: publicProcedure
                .input(UserSchema)
                .query(async ({ input }) => {
                    return { id: input.id, name: 'Alice' };
                }),
            createUser: publicProcedure
                .input(z.object({ name: z.string() }))
                .mutation(async ({ input }) => {
                    return { id: '456', name: input.name };
                }),
        });
    "#;

    let routes = extract_server_routes(Path::new("src/server/routers/app.ts"), backend_code);
    assert_eq!(routes.len(), 2, "Expected 2 tRPC routes, found {}", routes.len());

    let get_user = routes.iter().find(|r| r.handler_symbol == "getUser").unwrap();
    assert_eq!(get_user.framework, "trpc");
    assert_eq!(get_user.http_method, "QUERY");
    assert_eq!(get_user.route_path, "/trpc/getUser");
    assert!(get_user.request_dto_type.is_some());
    assert_eq!(get_user.request_dto_type.as_ref().unwrap().name, "UserSchema");

    let create_user = routes.iter().find(|r| r.handler_symbol == "createUser").unwrap();
    assert_eq!(create_user.framework, "trpc");
    assert_eq!(create_user.http_method, "MUTATION");
    assert_eq!(create_user.route_path, "/trpc/createUser");

    // Client hooks
    let frontend_code = r#"
        import { trpc } from '../utils/trpc';

        export function UserProfile({ userId }: { userId: string }) {
            const { data } = trpc.user.getUser.useQuery({ id: userId });
            const createMutation = trpc.user.createUser.useMutation();
            return <div>{data?.name}</div>;
        }
    "#;

    let detector = ClientDetector::new();
    let calls = detector.detect_in_file(Path::new("src/components/UserProfile.tsx"), frontend_code);
    assert!(calls.len() >= 2, "Expected at least 2 tRPC client calls, found {}", calls.len());

    let matcher = RouteMatcher::new();
    let query_call = calls.iter().find(|c| c.rpc_procedure.as_deref() == Some("user.getUser")).unwrap();
    let matched = matcher.match_client_to_server(query_call, &routes);
    assert!(matched.is_some(), "Expected tRPC client call to match server procedure");
    assert_eq!(matched.unwrap().handler_symbol, "getUser");
}

#[test]
fn test_nextjs_server_actions_extraction_and_resolution() {
    let action_file = r#"
        'use server';

        export interface UpdateProfileInput {
            displayName: string;
            bio: string;
        }

        export interface ActionResult {
            success: boolean;
            message?: string;
        }

        export async function updateProfile(formData: FormData): Promise<ActionResult> {
            const name = formData.get('displayName') as string;
            return { success: true, message: `Updated ${name}` };
        }

        export async function deleteAccount(userId: string): Promise<ActionResult> {
            return { success: true };
        }
    "#;

    let routes = extract_server_routes(Path::new("app/actions/profile.ts"), action_file);
    assert_eq!(routes.len(), 2, "Expected 2 Server Actions, found {}", routes.len());

    let update = routes.iter().find(|r| r.handler_symbol == "updateProfile").unwrap();
    assert_eq!(update.framework, "nextjs_server_action");
    assert_eq!(update.http_method, "ACTION");
    assert_eq!(update.route_path, "action://updateProfile");

    let delete = routes.iter().find(|r| r.handler_symbol == "deleteAccount").unwrap();
    assert_eq!(delete.framework, "nextjs_server_action");
    assert_eq!(delete.http_method, "ACTION");

    // Client form and action hooks
    let component_file = r#"
        'use client';

        import { updateProfile, deleteAccount } from '@/app/actions/profile';
        import { useActionState } from 'react';

        export function ProfileForm() {
            const [state, formAction] = useActionState(updateProfile, null);

            return (
                <div>
                    <form action={formAction}>
                        <input name="displayName" />
                        <button type="submit">Save</button>
                    </form>
                    <form action={updateProfile}>
                        <button formAction={deleteAccount}>Delete</button>
                    </form>
                </div>
            );
        }
    "#;

    let detector = ClientDetector::new();
    let calls = detector.detect_in_file(Path::new("app/components/ProfileForm.tsx"), component_file);
    assert!(calls.len() >= 2, "Expected at least 2 Server Action client invocations, found {}", calls.len());

    let update_call = calls.iter().find(|c| c.rpc_procedure.as_deref() == Some("updateProfile")).unwrap();
    assert_eq!(update_call.client_kind, "nextjs_server_action");
    assert_eq!(update_call.http_method.as_deref(), Some("ACTION"));

    let matcher = RouteMatcher::new();
    let matched = matcher.match_client_to_server(update_call, &routes);
    assert!(matched.is_some(), "Expected Server Action match");
    assert_eq!(matched.unwrap().handler_symbol, "updateProfile");
}

#[test]
fn test_fullstack_tracer_tauri_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // 1. Tauri Rust backend
    let tauri_dir = root.join("src-tauri").join("src");
    fs::create_dir_all(&tauri_dir).unwrap();
    let commands_rs = tauri_dir.join("commands.rs");
    fs::write(
        &commands_rs,
        r#"
        use tauri::State;

        pub struct OrderPayload {
            pub item_id: String,
            pub quantity: u32,
        }

        pub struct OrderReceipt {
            pub order_id: String,
            pub total_price: f64,
        }

        #[tauri::command]
        pub async fn submit_order(
            payload: OrderPayload,
        ) -> Result<OrderReceipt, String> {
            Ok(OrderReceipt {
                order_id: "ORD-999".to_string(),
                total_price: 150.0,
            })
        }
        "#,
    ).unwrap();

    // 2. React frontend
    let ui_dir = root.join("src").join("ui");
    fs::create_dir_all(&ui_dir).unwrap();
    let checkout_tsx = ui_dir.join("Checkout.tsx");
    fs::write(
        &checkout_tsx,
        r#"
        import { invoke } from '@tauri-apps/api/core';

        export async function onCheckout(itemId: string, qty: number) {
            const receipt = await invoke('submitOrder', {
                item_id: itemId,
                quantity: qty
            });
            return receipt;
        }
        "#,
    ).unwrap();

    // 3. Trace execution
    let tracer = FullstackExecutionTracer::new();
    let result = tracer.trace_api(root, "submit_order", Some(2000)).unwrap();

    assert_eq!(result.server_route.framework, "tauri");
    assert_eq!(result.server_route.handler_symbol, "submit_order");
    assert!(result.client_call.is_some(), "Expected client call to be discovered and linked");
    assert_eq!(result.client_call.as_ref().unwrap().client_kind, "tauri");
    assert!(!result.steps.is_empty(), "Expected execution steps");

    // Slicing via ContextSlicer
    let opts = SliceOptions::default();
    let slicer = ctxcut_core::slice::ContextSlicer::new();
    let slice = slicer.slice_symbol(&commands_rs, "submit_order", &opts).unwrap();
    assert_eq!(slice.target_symbol.name, "submit_order");
    assert!(slice.target_symbol.body.contains("submit_order"));
}
