//! Integration tests for CLI route resolution across modern frameworks (Tauri, Electron, tRPC, Next.js Server Actions).

use ctxcut_cli::route::resolve_route_slice;
use ctxcut_core::SliceOptions;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_cli_resolve_tauri_command_with_casing_normalization() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let tauri_dir = root.join("src-tauri").join("src");
    fs::create_dir_all(&tauri_dir).unwrap();
    fs::write(
        tauri_dir.join("main.rs"),
        r#"
        pub struct TaxInput {
            pub amount: f64,
        }

        pub struct TaxResult {
            pub tax: f64,
        }

        #[tauri::command]
        pub fn calculate_tax(input: TaxInput) -> Result<TaxResult, String> {
            Ok(TaxResult { tax: input.amount * 0.2 })
        }
        "#,
    ).unwrap();

    let opts = SliceOptions::default();

    // Query with exact snake_case
    let slice1 = resolve_route_slice(root, "IPC", "calculate_tax", &opts).unwrap();
    assert_eq!(slice1.target_symbol.name, "calculate_tax");
    assert!(slice1.hoisted_types.iter().any(|t| t.name == "TaxInput"));
    assert!(slice1.hoisted_types.iter().any(|t| t.name == "TaxResult"));

    // Query with camelCase
    let slice2 = resolve_route_slice(root, "IPC", "calculateTax", &opts).unwrap();
    assert_eq!(slice2.target_symbol.name, "calculate_tax");

    // Query with method "ANY"
    let slice3 = resolve_route_slice(root, "ANY", "calculateTax", &opts).unwrap();
    assert_eq!(slice3.target_symbol.name, "calculate_tax");
}

#[test]
fn test_cli_resolve_electron_channel() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let electron_dir = root.join("src").join("main");
    fs::create_dir_all(&electron_dir).unwrap();
    fs::write(
        electron_dir.join("ipc.ts"),
        r#"
        import { ipcMain } from 'electron';

        export async function handleOpenFile() {
            return "/path/to/file";
        }

        ipcMain.handle('dialog:openFile', handleOpenFile);
        "#,
    ).unwrap();

    let opts = SliceOptions::default();

    let slice = resolve_route_slice(root, "IPC_HANDLE", "dialog:openFile", &opts).unwrap();
    assert_eq!(slice.target_symbol.name, "handleOpenFile");
}

#[test]
fn test_cli_resolve_trpc_procedures() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let router_dir = root.join("src").join("server");
    fs::create_dir_all(&router_dir).unwrap();
    fs::write(
        router_dir.join("router.ts"),
        r#"
        import { initTRPC } from '@trpc/server';

        const t = initTRPC.create();
        export const router = t.router;
        export const publicProcedure = t.procedure;

        export const appRouter = router({
            getUser: publicProcedure.query(async () => {
                return { id: 1, name: "Alice" };
            }),
            createUser: publicProcedure.mutation(async () => {
                return { success: true };
            }),
        });
        "#,
    ).unwrap();

    let opts = SliceOptions::default();

    let slice1 = resolve_route_slice(root, "QUERY", "/trpc/getUser", &opts).unwrap();
    assert_eq!(slice1.target_symbol.name, "getUser");

    let slice2 = resolve_route_slice(root, "MUTATION", "createUser", &opts).unwrap();
    assert_eq!(slice2.target_symbol.name, "createUser");
}

#[test]
fn test_cli_resolve_nextjs_server_action() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let actions_dir = root.join("app").join("actions");
    fs::create_dir_all(&actions_dir).unwrap();
    fs::write(
        actions_dir.join("profile.ts"),
        r#"
        'use server';

        export async function updateProfile(formData: FormData) {
            return { success: true };
        }
        "#,
    ).unwrap();

    let opts = SliceOptions::default();

    let slice1 = resolve_route_slice(root, "ACTION", "updateProfile", &opts).unwrap();
    assert_eq!(slice1.target_symbol.name, "updateProfile");

    let slice2 = resolve_route_slice(root, "ANY", "action://updateProfile", &opts).unwrap();
    assert_eq!(slice2.target_symbol.name, "updateProfile");
}
