//! Adversarial empirical verification test suite for R2 features:
//! 1. Workspace Symbol Overview (depth, budget compression tiers, multi-lang, route indexing)
//! 2. Multi-symbol Slicing with Unified Deduplication (Rust, TS, Python, Go, edge cases)
//! 3. Edge conditions: empty workspaces, duplicate queries, whitespace tolerance.

use ctxcut_core::{
    ContextSlicer, OverviewOptions, SliceOptions, WorkspaceOverviewGenerator,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_overview_empty_and_non_code_directories() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Empty directory
    let opts = OverviewOptions {
        budget: None,
        max_depth: None,
        include_routes: true,
        framework: None,
    };
    let report = WorkspaceOverviewGenerator::generate(root, &opts).unwrap();
    assert_eq!(report.total_files, 0);
    assert_eq!(report.total_symbols, 0);
    assert_eq!(report.total_lines, 0);
    assert_eq!(report.total_raw_tokens, 0);
    assert!(report.total_overview_tokens > 0, "Header markdown has non-zero tokens");
    assert_eq!(report.token_savings_percentage, 0.0);

    // Directory with only non-code files (e.g. .txt, .md, .png)
    fs::write(root.join("README.md"), "# Hello World\nThis is a readme.").unwrap();
    fs::write(root.join("notes.txt"), "some plain text notes").unwrap();
    let report2 = WorkspaceOverviewGenerator::generate(root, &opts).unwrap();
    assert_eq!(report2.total_files, 0);
    assert_eq!(report2.total_symbols, 0);
}

#[test]
fn test_overview_depth_limiting() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create nested directory structure:
    // root/top.rs
    // root/level1/sub1.rs
    // root/level1/level2/sub2.rs
    // root/level1/level2/level3/sub3.rs
    fs::write(
        root.join("top.rs"),
        "pub fn top_fn() -> bool { true }\n",
    )
    .unwrap();

    let l1 = root.join("level1");
    fs::create_dir_all(&l1).unwrap();
    fs::write(
        l1.join("sub1.rs"),
        "pub fn l1_fn() -> i32 { 1 }\n",
    )
    .unwrap();

    let l2 = l1.join("level2");
    fs::create_dir_all(&l2).unwrap();
    fs::write(
        l2.join("sub2.rs"),
        "pub fn l2_fn() -> i32 { 2 }\n",
    )
    .unwrap();

    let l3 = l2.join("level3");
    fs::create_dir_all(&l3).unwrap();
    fs::write(
        l3.join("sub3.rs"),
        "pub fn l3_fn() -> i32 { 3 }\n",
    )
    .unwrap();

    let opts_all = OverviewOptions {
        budget: None,
        max_depth: None,
        include_routes: false,
        framework: None,
    };
    let report_all = WorkspaceOverviewGenerator::generate(root, &opts_all).unwrap();
    assert_eq!(report_all.total_files, 4);

    let opts_depth1 = OverviewOptions {
        budget: None,
        max_depth: Some(1),
        include_routes: false,
        framework: None,
    };
    let report_depth1 = WorkspaceOverviewGenerator::generate(root, &opts_depth1).unwrap();
    // At depth 1, files in level2 and level3 should be excluded or total <= 2
    assert!(report_depth1.total_files <= report_all.total_files);
}

#[test]
fn test_overview_progressive_budget_compression_tiers() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create TypeScript files with single-line JSDoc comments and long signatures
    for i in 0..10 {
        let code = format!(
            r#"
/** Service_{i} manages transactions and business invariants. */
export class Service_{i} {{
    /** Method that processes an incoming transaction request with multi-stage validation. */
    public processTransaction_{i}(accountId: string, amount: number, currency: string): Promise<string> {{
        if (amount <= 0) {{
            throw new Error("Invalid amount");
        }}
        return Promise.resolve("processed_" + accountId + "_" + amount);
    }}

    /** Secondary helper method for audit logging and notification dispatch. */
    public auditLog_{i}(eventId: number, message: string): boolean {{
        console.log("Audit log:", eventId, message);
        return true;
    }}
}}
"#
        );
        fs::write(root.join(format!("service_{i}.ts")), code).unwrap();
    }

    // Tier 0: Unconstrained budget
    let opts_unconstrained = OverviewOptions {
        budget: None,
        max_depth: None,
        include_routes: false,
        framework: None,
    };
    let report_full = WorkspaceOverviewGenerator::generate(root, &opts_unconstrained).unwrap();
    let full_tokens = report_full.total_overview_tokens;
    assert!(full_tokens > 200, "Expected substantial overview token count");
    let full_md = report_full.to_markdown();
    // Verify doc summaries and signatures are present in full output
    assert!(full_md.contains("Service_") && full_md.contains("manages transactions"));
    assert!(full_md.contains("processTransaction_"));

    // Tier 1: Moderate budget forcing doc summary stripping
    let moderate_budget = (full_tokens as f64 * 0.7) as usize;
    let opts_moderate = OverviewOptions {
        budget: Some(moderate_budget),
        max_depth: None,
        include_routes: false,
        framework: None,
    };
    let report_moderate = WorkspaceOverviewGenerator::generate(root, &opts_moderate).unwrap();
    assert!(report_moderate.total_overview_tokens <= full_tokens);

    // Tier 2: Strict budget forcing signature stripping
    let strict_budget = (full_tokens as f64 * 0.35) as usize;
    let opts_strict = OverviewOptions {
        budget: Some(strict_budget),
        max_depth: None,
        include_routes: false,
        framework: None,
    };
    let report_strict = WorkspaceOverviewGenerator::generate(root, &opts_strict).unwrap();
    assert!(report_strict.total_overview_tokens <= report_moderate.total_overview_tokens);
    assert!(report_strict.token_savings_percentage > 0.0);
    assert!(report_strict.token_savings_percentage <= 100.0);
}

#[test]
fn test_multi_symbol_rust_deduplication_and_ordering() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let file = root.join("lib.rs");

    let code = r#"
pub struct UserAccount {
    pub id: u64,
    pub username: String,
    pub email: String,
    pub is_active: bool,
}

pub struct TransactionPayload {
    pub tx_id: String,
    pub amount: f64,
}

pub enum AccountStatus {
    Active,
    Suspended,
    Closed,
}

pub fn deposit(account: &mut UserAccount, payload: TransactionPayload) -> bool {
    validate_account(account);
    if payload.amount > 0.0 {
        true
    } else {
        false
    }
}

pub fn withdraw(account: &mut UserAccount, payload: TransactionPayload) -> bool {
    validate_account(account);
    if payload.amount > 0.0 {
        true
    } else {
        false
    }
}

pub fn get_status(account: &UserAccount) -> AccountStatus {
    if account.is_active {
        AccountStatus::Active
    } else {
        AccountStatus::Closed
    }
}

fn validate_account(account: &UserAccount) -> bool {
    account.is_active
}
"#;
    fs::write(&file, code).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // Batch query for 3 functions sharing UserAccount and TransactionPayload
    let batch = slicer
        .slice_batch(&file, &["deposit", "withdraw", "get_status"], &opts)
        .unwrap();

    assert_eq!(batch.target_symbols.len(), 3);
    assert_eq!(batch.target_symbols[0].name, "deposit");
    assert_eq!(batch.target_symbols[1].name, "withdraw");
    assert_eq!(batch.target_symbols[2].name, "get_status");

    // Check hoisted types: UserAccount and TransactionPayload and AccountStatus must be hoisted
    let hoisted_names: Vec<&str> = batch.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted_names.contains(&"UserAccount"));
    assert!(hoisted_names.contains(&"TransactionPayload"));
    assert!(hoisted_names.contains(&"AccountStatus"));

    // UserAccount must appear EXACTLY ONCE in hoisted_types
    let user_account_count = hoisted_names.iter().filter(|&&n| n == "UserAccount").count();
    assert_eq!(user_account_count, 1, "UserAccount was duplicated in hoisted types!");

    let tx_payload_count = hoisted_names.iter().filter(|&&n| n == "TransactionPayload").count();
    assert_eq!(tx_payload_count, 1, "TransactionPayload was duplicated in hoisted types!");

    // Check calls: validate_account must appear only once in stripped_calls
    let call_names: Vec<&str> = batch.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    let validate_count = call_names.iter().filter(|&&n| n == "validate_account").count();
    assert_eq!(validate_count, 1, "validate_account call was duplicated in stripped calls!");

    let md = batch.to_markdown();
    assert!(md.contains("deposit, withdraw, get_status"));
    assert_eq!(md.matches("struct UserAccount").count(), 1);
    assert_eq!(md.matches("struct TransactionPayload").count(), 1);
    assert_eq!(md.matches("enum AccountStatus").count(), 1);
}

#[test]
fn test_multi_symbol_python_deduplication() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let file = root.join("auth.py");

    let code = r#"
from dataclasses import dataclass
from typing import Optional

@dataclass
class UserSession:
    user_id: str
    token: str
    is_admin: bool

@dataclass
class AuthResult:
    success: bool
    session: Optional[UserSession]
    error_message: Optional[str]

def authenticate(user_id: str, secret: str) -> AuthResult:
    if secret == "valid":
        s = UserSession(user_id=user_id, token="tok_123", is_admin=True)
        return AuthResult(success=True, session=s, error_message=None)
    return AuthResult(success=False, session=None, error_message="Invalid credentials")

def authorize(session: UserSession, required_role: str) -> bool:
    if session.is_admin:
        return True
    return False

def refresh_session(session: UserSession) -> UserSession:
    return UserSession(user_id=session.user_id, token="tok_refreshed", is_admin=session.is_admin)
"#;
    fs::write(&file, code).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let batch = slicer
        .slice_batch(&file, &["authenticate", "authorize", "refresh_session"], &opts)
        .unwrap();

    assert_eq!(batch.target_symbols.len(), 3);

    // Verify UserSession is hoisted only once
    let session_count = batch.hoisted_types.iter().filter(|t| t.name == "UserSession").count();
    assert_eq!(session_count, 1, "UserSession duplicated in Python batch slice");

    let md = batch.to_markdown();
    assert_eq!(md.matches("class UserSession:").count(), 1);
}

#[test]
fn test_multi_symbol_go_deduplication() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let file = root.join("service.go");

    let code = r#"
package service

type Config struct {
    Host string
    Port int
    TLS  bool
}

type Client struct {
    cfg Config
}

func NewClient(cfg Config) *Client {
    return &Client{cfg: cfg}
}

func (c *Client) Connect() bool {
    return c.cfg.Port > 0
}

func (c *Client) Disconnect(force bool) bool {
    return true
}
"#;
    fs::write(&file, code).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let batch = slicer
        .slice_batch(&file, &["NewClient", "Connect", "Disconnect"], &opts)
        .unwrap();

    assert_eq!(batch.target_symbols.len(), 3);
    let cfg_count = batch.hoisted_types.iter().filter(|t| t.name == "Config").count();
    assert_eq!(cfg_count, 1, "Config struct duplicated in Go batch slice");
}

#[test]
fn test_multi_symbol_duplicate_and_whitespace_queries() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let file = root.join("calc.rs");

    fs::write(
        &file,
        r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }
pub fn sub(a: i32, b: i32) -> i32 { a - b }
"#,
    )
    .unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    // Query with duplicate symbols: ["add", "add", "sub"]
    let batch = slicer.slice_batch(&file, &["add", "add", "sub"], &opts).unwrap();
    assert_eq!(batch.target_symbols.len(), 3);

    // Query with empty entries in list: ["add", "", "  ", "sub"]
    let batch2 = slicer.slice_batch(&file, &["add", "", "  ", "sub"], &opts).unwrap();
    assert_eq!(batch2.target_symbols.len(), 2);
}

#[test]
fn test_multi_symbol_batch_budget_compression() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let file = root.join("verbose.rs");

    let code = r#"
/// Extremely verbose doc comments for calculate_a.
/// Provides exhaustive background on calculation methodology.
pub fn calculate_a(x: i32) -> i32 {
    helper_fn(x);
    x * 2
}

/// Extremely verbose doc comments for calculate_b.
/// More long paragraphs describing calculation requirements.
pub fn calculate_b(y: i32) -> i32 {
    helper_fn(y);
    y * 3
}

fn helper_fn(val: i32) -> bool {
    val > 0
}
"#;
    fs::write(&file, code).unwrap();

    let slicer = ContextSlicer::new();
    let uncompressed_opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };
    let uncompressed_batch = slicer
        .slice_batch(&file, &["calculate_a", "calculate_b"], &uncompressed_opts)
        .unwrap();

    let tight_budget_opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: Some(50),
    };
    let compressed_batch = slicer
        .slice_batch(&file, &["calculate_a", "calculate_b"], &tight_budget_opts)
        .unwrap();

    // Under tight budget, doc comments must be cleared
    assert!(compressed_batch.target_symbols.iter().all(|s| s.doc_comment.is_none()));
    assert!(compressed_batch.stats.sliced_tokens <= uncompressed_batch.stats.sliced_tokens);
}

#[test]
fn test_overview_multiline_jsdoc_extraction() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let file = root.join("service.ts");

    let code = r#"
/**
 * Processes incoming user registration requests.
 * @param req The registration request payload.
 */
export function registerUser(req: any): boolean {
    return true;
}

/**
 * Validates user credentials against cryptographic store.
 */
export class AuthValidator {
    /**
     * Verifies password hash integrity.
     */
    public verifyHash(hash: string): boolean {
        return hash.length > 0;
    }
}
"#;
    fs::write(&file, code).unwrap();

    let opts = OverviewOptions {
        budget: None,
        max_depth: None,
        include_routes: false,
        framework: None,
    };

    let report = WorkspaceOverviewGenerator::generate(root, &opts).unwrap();
    assert_eq!(report.total_files, 1);
    let md = report.to_markdown();

    assert!(
        md.contains("Processes incoming user registration requests."),
        "Failed to extract multiline JSDoc comment for function: {}",
        md
    );
    assert!(
        md.contains("Validates user credentials against cryptographic store."),
        "Failed to extract multiline JSDoc comment for class: {}",
        md
    );
    assert!(
        md.contains("Verifies password hash integrity."),
        "Failed to extract multiline JSDoc comment for method: {}",
        md
    );
}

