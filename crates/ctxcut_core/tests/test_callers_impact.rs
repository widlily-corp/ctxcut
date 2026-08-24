//! Integration tests for Milestone 1 Feature 1: Impact & Upstream Caller Graph Analysis.

use ctxcut_core::model::SliceOptions;
use ctxcut_core::resolver::ImpactAnalyzer;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_impact_analysis_typescript_callers() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let service_file = ws.join("service.ts");
    fs::write(
        &service_file,
        r"
export function calculateTax(amount: number): number {
    return amount * 0.2;
}
",
    )
    .expect("write service");

    let controller_file = ws.join("controller.ts");
    fs::write(
        &controller_file,
        r"
import { calculateTax } from './service';

export class OrderController {
    public handleCheckout(orderId: string, total: number) {
        const tax = calculateTax(total);
        return { orderId, tax };
    }

    public static previewTax(total: number) {
        return calculateTax(total);
    }
}
",
    )
    .expect("write controller");

    let api_file = ws.join("api.ts");
    fs::write(
        &api_file,
        r"
import { calculateTax } from './service';

export const quickQuote = (subtotal: number) => {
    return calculateTax(subtotal);
};
",
    )
    .expect("write api");

    let opts = SliceOptions::default();
    let result = ImpactAnalyzer::find_callers(ws, "calculateTax", Some(&service_file), &opts)
        .expect("find callers");

    assert_eq!(result.target_symbol, "calculateTax");
    assert_eq!(result.total_callers, 3);

    let caller_names: Vec<&str> = result
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();
    assert!(caller_names.contains(&"OrderController.handleCheckout"));
    assert!(caller_names.contains(&"OrderController.previewTax"));
    assert!(caller_names.contains(&"quickQuote"));

    // Verify markdown rendering contains callers
    let md = result.to_markdown();
    assert!(md.contains("### Upstream Impact Analysis: `calculateTax`"));
    assert!(md.contains("OrderController.handleCheckout"));
    assert!(md.contains("quickQuote"));
}

#[test]
fn test_impact_analysis_python_callers() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let repo_file = ws.join("repository.py");
    fs::write(
        &repo_file,
        r#"
def save_user(user_data):
    return {"status": "saved", "data": user_data}
"#,
    )
    .expect("write repo");

    let service_file = ws.join("service.py");
    fs::write(
        &service_file,
        r"
from repository import save_user

class UserService:
    def register_user(self, payload):
        user = save_user(payload)
        return user

def admin_create_user(raw_data):
    return save_user(raw_data)
",
    )
    .expect("write service");

    let opts = SliceOptions::default();
    let result = ImpactAnalyzer::find_callers(ws, "save_user", Some(&repo_file), &opts)
        .expect("find callers");

    assert_eq!(result.target_symbol, "save_user");
    assert_eq!(result.total_callers, 2);

    let caller_names: Vec<&str> = result
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();
    assert!(caller_names.contains(&"UserService.register_user"));
    assert!(caller_names.contains(&"admin_create_user"));
}

#[test]
fn test_impact_analysis_rust_callers() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let utils_file = ws.join("utils.rs");
    fs::write(
        &utils_file,
        r#"
pub fn hash_password(plain: &str) -> String {
    format!("hash_{plain}")
}
"#,
    )
    .expect("write utils");

    let auth_file = ws.join("auth.rs");
    fs::write(
        &auth_file,
        r"
use crate::utils::hash_password;

pub struct AuthManager;

impl AuthManager {
    pub fn sign_up(&self, pass: &str) -> String {
        let hashed = hash_password(pass);
        hashed
    }
}

pub fn reset_password_handler(pass: &str) {
    let _ = hash_password(pass);
}
",
    )
    .expect("write auth");

    let opts = SliceOptions::default();
    let result = ImpactAnalyzer::find_callers(ws, "hash_password", Some(&utils_file), &opts)
        .expect("find callers");

    assert_eq!(result.target_symbol, "hash_password");
    assert_eq!(result.total_callers, 2);

    let caller_names: Vec<&str> = result
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();
    assert!(caller_names.contains(&"AuthManager::sign_up"));
    assert!(caller_names.contains(&"reset_password_handler"));
}

#[test]
fn test_impact_analysis_go_callers() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let db_file = ws.join("db.go");
    fs::write(
        &db_file,
        r"
package main

func ExecuteQuery(q string) error {
    return nil
}
",
    )
    .expect("write db");

    let handler_file = ws.join("handler.go");
    fs::write(
        &handler_file,
        r#"
package main

type UserHandler struct{}

func (h *UserHandler) HandleGet() error {
    return ExecuteQuery("SELECT * FROM users")
}

func HandleHealth() error {
    return ExecuteQuery("PING")
}
"#,
    )
    .expect("write handler");

    let opts = SliceOptions::default();
    let result = ImpactAnalyzer::find_callers(ws, "ExecuteQuery", Some(&db_file), &opts)
        .expect("find callers");

    assert_eq!(result.target_symbol, "ExecuteQuery");
    assert_eq!(result.total_callers, 2);

    let caller_names: Vec<&str> = result
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();
    assert!(caller_names.contains(&"UserHandler.HandleGet"));
    assert!(caller_names.contains(&"HandleHealth"));
}

#[test]
fn test_impact_analysis_budget_compression() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let target_file = ws.join("target.ts");
    fs::write(
        &target_file,
        r"
export function coreAction() {
    return true;
}
",
    )
    .expect("write target");

    let mut callers_src = String::new();
    for i in 1..=20 {
        callers_src.push_str(&format!(
            "export function callerFunction{i}() {{\n    // Detailed comments explaining step {i}\n    // Another line of verbose explanation\n    return coreAction();\n}}\n\n"
        ));
    }
    let callers_file = ws.join("callers.ts");
    fs::write(&callers_file, &callers_src).expect("write callers");

    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: Some(200), // Strict budget forcing compression
    };

    let result = ImpactAnalyzer::find_callers(ws, "coreAction", Some(&target_file), &opts)
        .expect("find callers with budget");

    assert_eq!(result.total_callers, 20);
    assert!(result.stats.sliced_tokens <= 250);
}
