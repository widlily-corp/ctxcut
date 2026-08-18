//! Comprehensive multi-language integration tests for AstPatcher in `ctxcut_core`.

use ctxcut_core::error::CoreError;
use ctxcut_core::model::SupportedLanguage;
use ctxcut_core::patch::AstPatcher;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

// ============================================================================
// 1. Rust AST Patching
// ============================================================================

#[test]
fn test_rust_patch_free_function() {
    let original = r#"// Global math utilities
/// Calculates tax on an item.
pub fn calculate_tax(amount: f64, rate: f64) -> f64 {
    amount * rate
}

pub fn format_currency(val: f64) -> String {
    format!("${:.2}", val)
}
"#;

    let replacement = r"pub fn calculate_tax(amount: f64, rate: f64) -> f64 {
    let raw = amount * rate;
    (raw * 100.0).round() / 100.0
}";

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Rust,
        &PathBuf::from("src/tax.rs"),
        "calculate_tax",
        replacement,
    )
    .expect("Patching free function should succeed");

    assert_eq!(res.symbol_name, "calculate_tax");
    assert!(!res.applied);
    assert!(res.original_code.contains("amount * rate"));
    assert!(res.patched_code.contains("raw * 100.0"));
    assert!(res.diff.contains("+    let raw = amount * rate;"));

    // Verify surrounding doc comments and sibling functions were not corrupted
    let patched_full = format!(
        "{}{}{}",
        &original[..res.byte_range.0],
        res.patched_code,
        &original[res.byte_range.1..]
    );
    assert!(patched_full.contains("/// Calculates tax on an item."));
    assert!(patched_full.contains("pub fn format_currency"));
}

#[test]
fn test_rust_patch_impl_method() {
    let original = r"pub struct Account {
    balance: f64,
}

impl Account {
    pub fn new(balance: f64) -> Self {
        Self { balance }
    }

    pub fn deposit(&mut self, amount: f64) {
        self.balance += amount;
    }

    pub fn balance(&self) -> f64 {
        self.balance
    }
}
";

    let replacement = r#"pub fn deposit(&mut self, amount: f64) {
    assert!(amount > 0.0, "Deposit must be positive");
    self.balance += amount;
}"#;

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Rust,
        &PathBuf::from("src/account.rs"),
        "Account::deposit",
        replacement,
    )
    .expect("Patching impl method should succeed");

    assert_eq!(res.symbol_name, "deposit");
    assert!(res.diff.contains("+        assert!(amount > 0.0"));

    let patched_full = format!(
        "{}{}{}",
        &original[..res.byte_range.0],
        res.patched_code,
        &original[res.byte_range.1..]
    );
    assert!(patched_full.contains("pub fn new"));
    assert!(patched_full.contains("pub fn balance"));
}

#[test]
fn test_rust_patch_struct() {
    let original = r"pub struct Config {
    pub host: String,
    pub port: u16,
}
";

    let replacement = r"pub struct Config {
    pub host: String,
    pub port: u16,
    pub timeout_ms: u64,
}";

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Rust,
        &PathBuf::from("src/config.rs"),
        "Config",
        replacement,
    )
    .expect("Patching struct should succeed");

    assert_eq!(res.symbol_name, "Config");
    assert!(res.diff.contains("+    pub timeout_ms: u64,"));
}

// ============================================================================
// 2. Python AST Patching & Indentation Preservation
// ============================================================================

#[test]
fn test_python_patch_free_function() {
    let original = r"def add(a: int, b: int) -> int:
    return a + b

def multiply(a: int, b: int) -> int:
    return a * b
";

    let replacement = r#"def add(a: int, b: int) -> int:
    print(f"Adding {a} and {b}")
    return a + b"#;

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Python,
        &PathBuf::from("math_utils.py"),
        "add",
        replacement,
    )
    .expect("Patching Python function should succeed");

    assert_eq!(res.symbol_name, "add");
    assert!(res.diff.contains("+    print(f\"Adding {a} and {b}\")"));
}

#[test]
fn test_python_patch_decorated_function() {
    let original = r#"from fastapi import FastAPI

app = FastAPI()

@app.get("/users/{user_id}")
def get_user(user_id: int):
    return {"user_id": user_id}
"#;

    let replacement = r#"@app.get("/users/{user_id}", response_model=UserResponse)
async def get_user(user_id: int):
    user = await db.fetch_user(user_id)
    return user"#;

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Python,
        &PathBuf::from("main.py"),
        "get_user",
        replacement,
    )
    .expect("Patching decorated Python function should succeed");

    assert_eq!(res.symbol_name, "get_user");
    assert!(res.diff.contains("-@app.get(\"/users/{user_id}\")"));
    assert!(res
        .diff
        .contains("+@app.get(\"/users/{user_id}\", response_model=UserResponse)"));
}

#[test]
fn test_python_patch_class_method_with_4_space_indentation() {
    let original = r"class UserService:
    def __init__(self, db):
        self.db = db

    def get_profile(self, user_id: str):
        return self.db.find(user_id)

    def delete_user(self, user_id: str):
        self.db.delete(user_id)
";

    // Unindented replacement input should automatically align to 4 spaces inside class
    let replacement = r#"def get_profile(self, user_id: str):
    user = self.db.find(user_id)
    if not user:
        raise ValueError("User not found")
    return user"#;

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Python,
        &PathBuf::from("service.py"),
        "UserService.get_profile",
        replacement,
    )
    .expect("Patching Python class method should succeed");

    assert_eq!(res.symbol_name, "UserService.get_profile");
    let patched_full = format!(
        "{}{}{}",
        &original[..res.byte_range.0],
        res.patched_code,
        &original[res.byte_range.1..]
    );

    assert!(patched_full.contains("    def get_profile(self, user_id: str):"));
    assert!(patched_full.contains("        user = self.db.find(user_id)"));
    assert!(patched_full.contains("        if not user:"));
    assert!(patched_full.contains("            raise ValueError(\"User not found\")"));
    assert!(patched_full.contains("    def delete_user(self, user_id: str):"));
}

// ============================================================================
// 3. TypeScript / JavaScript AST Patching
// ============================================================================

#[test]
fn test_ts_patch_exported_function() {
    let original = r"import { User } from './types';

export async function authenticate(token: string): Promise<User> {
  const payload = verifyToken(token);
  return findUserById(payload.sub);
}

export function logout(): void {
  clearSession();
}
";

    let replacement = r"export async function authenticate(token: string): Promise<User> {
  if (!token) {
    throw new Error('Token is required');
  }
  const payload = await verifyTokenAsync(token);
  return findUserById(payload.sub);
}";

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::TypeScript,
        &PathBuf::from("auth.ts"),
        "authenticate",
        replacement,
    )
    .expect("Patching TS exported function should succeed");

    assert_eq!(res.symbol_name, "authenticate");
    assert!(res.diff.contains("+  if (!token) {"));
    let patched_full = format!(
        "{}{}{}",
        &original[..res.byte_range.0],
        res.patched_code,
        &original[res.byte_range.1..]
    );
    // Ensure no duplicate 'export export' keywords
    assert!(!patched_full.contains("export export"));
    assert!(patched_full.contains("export async function authenticate"));
    assert!(patched_full.contains("export function logout"));
}

#[test]
fn test_ts_patch_exported_arrow_function() {
    let original = r"export const calculateDiscount = (price: number, discount: number): number => {
  return price * (1 - discount);
};
";

    let replacement = r"export const calculateDiscount = (price: number, discount: number): number => {
  if (discount < 0 || discount > 1) {
    throw new RangeError('Invalid discount percentage');
  }
  return price * (1 - discount);
};";

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::TypeScript,
        &PathBuf::from("pricing.ts"),
        "calculateDiscount",
        replacement,
    )
    .expect("Patching TS arrow function should succeed");

    assert_eq!(res.symbol_name, "calculateDiscount");
    assert!(res.diff.contains("+    throw new RangeError"));
}

#[test]
fn test_ts_patch_class_method() {
    let original = r"export class PaymentController {
  private gateway: PaymentGateway;

  constructor(gateway: PaymentGateway) {
    this.gateway = gateway;
  }

  async processPayment(amount: number): Promise<boolean> {
    return this.gateway.charge(amount);
  }
}
";

    let replacement = r"async processPayment(amount: number): Promise<boolean> {
  if (amount <= 0) {
    return false;
  }
  return this.gateway.charge(amount);
}";

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::TypeScript,
        &PathBuf::from("controller.ts"),
        "PaymentController.processPayment",
        replacement,
    )
    .expect("Patching TS class method should succeed");

    assert_eq!(res.symbol_name, "PaymentController.processPayment");
    assert!(res.diff.contains("+    if (amount <= 0) {"));
}

// ============================================================================
// 4. Go AST Patching & Tab Indentation Preservation
// ============================================================================

#[test]
fn test_go_patch_free_function() {
    let original = r#"package main

import "fmt"

func Greet(name string) string {
	return fmt.Sprintf("Hello, %s!", name)
}
"#;

    let replacement = r#"func Greet(name string) string {
	if name == "" {
		return "Hello, anonymous!"
	}
	return fmt.Sprintf("Hello, %s!", name)
}"#;

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Go,
        &PathBuf::from("greet.go"),
        "Greet",
        replacement,
    )
    .expect("Patching Go free function should succeed");

    assert_eq!(res.symbol_name, "Greet");
    assert!(res.diff.contains("+\t\treturn \"Hello, anonymous!\""));
}

#[test]
fn test_go_patch_method_with_pointer_receiver() {
    let original = r"package server

type Server struct {
	addr string
	running bool
}

func (s *Server) Start() error {
	s.running = true
	return nil
}
";

    let replacement = r#"func (s *Server) Start() error {
	if s.running {
		return fmt.Errorf("server already running")
	}
	s.running = true
	return nil
}"#;

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Go,
        &PathBuf::from("server.go"),
        "*Server.Start",
        replacement,
    )
    .expect("Patching Go method with pointer receiver should succeed");

    assert_eq!(res.symbol_name, "Server.Start");
    assert!(res
        .diff
        .contains("+\t\treturn fmt.Errorf(\"server already running\")"));
}

// ============================================================================
// 5. Error Handling: Symbol Not Found & Syntax Validation Guard
// ============================================================================

#[test]
fn test_error_symbol_not_found_with_fuzzy_suggestions() {
    let source = r"
pub fn calculate_total(a: i32, b: i32) -> i32 {
    a + b
}

pub fn calculate_tax(a: f64) -> f64 {
    a * 0.1
}
";

    let err = AstPatcher::patch_source(
        source,
        SupportedLanguage::Rust,
        &PathBuf::from("calc.rs"),
        "calculate_totl",
        "fn calculate_totl() {}",
    )
    .unwrap_err();

    match &err {
        CoreError::SymbolNotFound {
            symbol,
            available_symbols,
            ..
        } => {
            assert_eq!(symbol, "calculate_totl");
            assert!(available_symbols.contains(&"calculate_total".to_string()));
            assert!(available_symbols.contains(&"calculate_tax".to_string()));
            let err_msg = err.to_string();
            assert!(err_msg.contains("Did you mean 'calculate_total'?"));
        }
        _ => panic!("Expected SymbolNotFound error"),
    }
}

#[test]
fn test_error_syntax_validation_rejection_rust() {
    let source = r"pub fn process() -> i32 {
    100
}
";

    // Malformed Rust replacement (unclosed brace and broken expression)
    let malformed = r"pub fn process() -> i32 {
    let x = 
";

    let err = AstPatcher::patch_source(
        source,
        SupportedLanguage::Rust,
        &PathBuf::from("process.rs"),
        "process",
        malformed,
    )
    .unwrap_err();

    match err {
        CoreError::SyntaxValidationError { path, errors } => {
            assert_eq!(path, PathBuf::from("process.rs"));
            assert!(!errors.is_empty());
            let first = &errors[0];
            assert!(first.line > 0);
        }
        _ => panic!("Expected SyntaxValidationError, got {:?}", err),
    }
}

#[test]
fn test_error_syntax_validation_rejection_python() {
    let source = r"def compute(x: int) -> int:
    return x * 2
";

    // Malformed Python (missing colon and broken syntax)
    let malformed = r"def compute(x: int)
    return x * 2";

    let err = AstPatcher::patch_source(
        source,
        SupportedLanguage::Python,
        &PathBuf::from("compute.py"),
        "compute",
        malformed,
    )
    .unwrap_err();

    assert!(matches!(err, CoreError::SyntaxValidationError { .. }));
}

// ============================================================================
// 6. Live vs Dry Run: Atomic Persistence & Safety
// ============================================================================

#[test]
fn test_patch_symbol_dry_run_leaves_disk_untouched() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("lib.rs");

    let original_content = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
    fs::write(&file_path, original_content).unwrap();

    let replacement = "pub fn add(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    sum\n}";

    let result = AstPatcher::patch_symbol(&file_path, "add", replacement, true)
        .expect("Dry run should succeed");

    assert!(!result.applied);
    assert!(result.diff.contains("+    let sum = a + b;"));

    // File on disk MUST be unchanged
    let current_disk_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(current_disk_content, original_content);
}

#[test]
fn test_patch_symbol_live_run_atomically_updates_disk() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("lib.rs");

    let original_content = "pub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n";
    fs::write(&file_path, original_content).unwrap();

    let replacement =
        "pub fn multiply(a: i32, b: i32) -> i32 {\n    // Multiplies two integers\n    a * b\n}";

    let result = AstPatcher::patch_symbol(&file_path, "multiply", replacement, false)
        .expect("Live patch should succeed");

    assert!(result.applied);

    // File on disk MUST be updated
    let current_disk_content = fs::read_to_string(&file_path).unwrap();
    assert!(current_disk_content.contains("// Multiplies two integers"));
}

#[test]
fn test_patch_symbol_syntax_error_never_touches_disk() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("handler.go");

    let original_content = "package main\n\nfunc Handle() string {\n\treturn \"OK\"\n}\n";
    fs::write(&file_path, original_content).unwrap();

    let broken_replacement = "func Handle( {\n\treturn \"BROKEN\"\n}";

    let err = AstPatcher::patch_symbol(&file_path, "Handle", broken_replacement, false)
        .expect_err("Malformed replacement must be rejected");

    assert!(matches!(err, CoreError::SyntaxValidationError { .. }));

    // File on disk MUST remain 100% original
    let current_disk_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(current_disk_content, original_content);
}

// ============================================================================
// 7. Formatting & UTF-8 Multi-Byte Robustness
// ============================================================================

#[test]
fn test_crlf_and_multibyte_utf8_preservation() {
    let original = "// Привет, мир! 🚀\r\nfn greet() -> &'static str {\r\n    \"Привет\"\r\n}\r\n\r\n// 🌟 End of file\r\n";

    let replacement = "fn greet() -> &'static str {\r\n    \"Здравствуйте, мир! 🚀\"\r\n}";

    let res = AstPatcher::patch_source(
        original,
        SupportedLanguage::Rust,
        &PathBuf::from("greeting.rs"),
        "greet",
        replacement,
    )
    .expect("Multi-byte and CRLF patch should succeed");

    assert_eq!(res.symbol_name, "greet");
    let patched_full = format!(
        "{}{}{}",
        &original[..res.byte_range.0],
        res.patched_code,
        &original[res.byte_range.1..]
    );

    assert!(patched_full.contains("// Привет, мир! 🚀"));
    assert!(patched_full.contains("Здравствуйте, мир! 🚀"));
    assert!(patched_full.contains("// 🌟 End of file"));
    assert!(patched_full.contains("\r\n"));
}
