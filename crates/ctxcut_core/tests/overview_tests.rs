//! Integration tests for workspace symbol overview extraction and formatting.

use ctxcut_core::{OverviewOptions, WorkspaceOverviewGenerator};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_workspace_overview_multilingual_extraction() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Create a TypeScript file with substantial bodies
    let ts_path = root.join("auth.ts");
    let mut ts_code = String::from(
        "/** Service for managing user authentication. */\nexport class AuthService {\n",
    );
    ts_code.push_str("    public login(user: string, pass: string): boolean {\n");
    for i in 0..20 {
        ts_code.push_str(&format!(
            "        console.log('Validating user step {i}', user);\n"
        ));
    }
    ts_code.push_str("        return user === 'admin' && pass === 'secret';\n    }\n\n");
    ts_code.push_str("    public logout(token: string): void {\n");
    for i in 0..20 {
        ts_code.push_str(&format!(
            "        console.log('Revoking session step {i}', token);\n"
        ));
    }
    ts_code.push_str("    }\n}\n\n");
    ts_code.push_str(
        "export interface UserSession {\n    token: string;\n    expiresAt: number;\n}\n\n",
    );
    ts_code.push_str(
        "export function createToken(userId: string): string {\n    return 'token_' + userId;\n}\n",
    );
    fs::write(&ts_path, &ts_code).unwrap();

    // 2. Create a Rust file with substantial bodies
    let rs_path = root.join("calc.rs");
    let mut rs_code =
        String::from("/// Math calculator struct.\npub struct Calculator;\n\nimpl Calculator {\n");
    rs_code.push_str(
        "    /// Adds two numbers together.\n    pub fn add(&self, a: i32, b: i32) -> i32 {\n",
    );
    for i in 0..20 {
        rs_code.push_str(&format!("        let _step_{i} = a + {i};\n"));
    }
    rs_code.push_str("        a + b\n    }\n}\n\npub trait Computable {\n    fn compute(&self) -> i32;\n}\n\npub fn global_helper() -> bool {\n    true\n}\n");
    fs::write(&rs_path, &rs_code).unwrap();

    // 3. Create a Python file with substantial bodies
    let py_path = root.join("service.py");
    let mut py_code = String::from("class PaymentProcessor:\n    \"\"\"Processes online payments.\"\"\"\n    def charge(self, amount: float) -> bool:\n        \"\"\"Charges a card.\"\"\"\n");
    for i in 0..20 {
        py_code.push_str(&format!(
            "        print(f'Checking fraud step {i}', amount)\n"
        ));
    }
    py_code.push_str("        return amount > 0\n\ndef format_currency(val: float) -> str:\n    \"\"\"Formats money.\"\"\"\n    return f'${val:.2f}'\n");
    fs::write(&py_path, &py_code).unwrap();

    // 4. Generate overview
    let opts = OverviewOptions {
        budget: None,
        max_depth: None,
        include_routes: true,
        framework: None,
    };

    let report = WorkspaceOverviewGenerator::generate(root, &opts).unwrap();

    assert_eq!(report.total_files, 3);
    assert!(report.total_symbols >= 8);
    assert!(report.total_raw_tokens > 0);
    assert!(report.total_overview_tokens > 0);
    assert!(report.token_savings_percentage > 0.0);

    let md = report.to_markdown();
    assert!(md.contains("# Workspace Symbol Overview"));
    assert!(md.contains("AuthService"));
    assert!(md.contains("AuthService.login"));
    assert!(md.contains("Calculator"));
    assert!(md.contains("Calculator::add"));
    assert!(md.contains("PaymentProcessor"));
    assert!(md.contains("PaymentProcessor.charge"));

    // Verify JSON serialization
    let json_str = report.to_json();
    assert!(json_str.contains("AuthService"));
}

#[test]
fn test_workspace_overview_budget_compression() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let ts_path = root.join("large.ts");
    let mut ts_code = String::new();
    for i in 0..30 {
        ts_code.push_str(&format!(
            "/** Doc comment description for function {i} with lots of context and extra words. */\nexport function longFunctionIdentifierNumber{i}(argOne: string, argTwo: number, argThree: boolean): Promise<string> {{\n    return Promise.resolve(argOne);\n}}\n\n"
        ));
    }
    fs::write(&ts_path, &ts_code).unwrap();

    // Generate without budget
    let full_opts = OverviewOptions {
        budget: None,
        max_depth: None,
        include_routes: true,
        framework: None,
    };
    let full_report = WorkspaceOverviewGenerator::generate(root, &full_opts).unwrap();
    let uncompressed_tokens = full_report.total_overview_tokens;

    // Generate with tight budget (e.g. 50% of tokens)
    let tight_budget = uncompressed_tokens / 2;
    let budgeted_opts = OverviewOptions {
        budget: Some(tight_budget),
        max_depth: None,
        include_routes: true,
        framework: None,
    };
    let budgeted_report = WorkspaceOverviewGenerator::generate(root, &budgeted_opts).unwrap();

    assert!(budgeted_report.total_overview_tokens <= uncompressed_tokens);
    assert!(budgeted_report.token_savings_percentage > full_report.token_savings_percentage);
}
