//! Empirical Challenger CLI stats integration tests for Milestone 1.

use ctxcut_cli::stats::{
    calculate_deep_stats, calculate_fast_stats, calculate_stats, format_stats_text,
};
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_cli_fast_and_deep_stats_complex_ignore_and_pruning() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create a .git repo marker
    fs::create_dir_all(root.join(".git")).unwrap();

    // 1. Ignored directory trees with thousands of dummy files
    let node_modules = root.join("node_modules").join("heavy-lib");
    fs::create_dir_all(&node_modules).unwrap();
    for i in 0..50 {
        fs::write(
            node_modules.join(format!("index_{i}.js")),
            "module.exports = {};",
        )
        .unwrap();
    }

    let target_dir = root.join("target").join("debug");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("app.exe"), [0x4D, 0x5A, 0x00, 0x00]).unwrap();

    let venv_dir = root.join(".venv").join("lib");
    fs::create_dir_all(&venv_dir).unwrap();
    fs::write(venv_dir.join("site.py"), "# venv").unwrap();

    // 2. .ctxcutignore rule
    fs::write(root.join(".ctxcutignore"), "ignored_dir/\n*.gen.ts\n").unwrap();
    let ignored_dir = root.join("ignored_dir");
    fs::create_dir_all(&ignored_dir).unwrap();
    fs::write(ignored_dir.join("skip.ts"), "export const s = 1;").unwrap();
    fs::write(root.join("types.gen.ts"), "export type T = string;").unwrap();

    // 3. Legitimate source files
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("main.rs"),
        r#"
pub struct AppConfig {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    pub max_connections: u32,
}

pub struct ServerMetrics {
    pub request_count: u64,
    pub uptime_seconds: u64,
}

impl AppConfig {
    pub fn new(port: u16, host: String) -> Self {
        Self {
            port,
            host,
            database_url: "postgres://localhost/db".to_string(),
            max_connections: 50,
        }
    }

    pub fn validate(&self) -> bool {
        self.port > 0 && !self.host.is_empty()
    }
}

pub fn start_server(config: AppConfig) {
    println!("Server running on {}:{} with db {}", config.host, config.port, config.database_url);
}
"#,
    )
    .unwrap();

    fs::write(
        src.join("service.ts"),
        r#"
export interface UserRecord {
    id: string;
    email: string;
    roles: string[];
    createdAt: Date;
}

export interface UserService {
    id: string;
    getUser(id: string): Promise<UserRecord | null>;
    createUser(email: string, roles: string[]): Promise<UserRecord>;
    deleteUser(id: string): Promise<boolean>;
}

export class DefaultUserService implements UserService {
    private storage = new Map<string, UserRecord>();

    constructor(public id: string) {}

    public async getUser(id: string): Promise<UserRecord | null> {
        return this.storage.get(id) || null;
    }

    public async createUser(email: string, roles: string[]): Promise<UserRecord> {
        const record: UserRecord = {
            id: "usr_" + Math.random().toString(36).substring(2),
            email,
            roles,
            createdAt: new Date(),
        };
        this.storage.set(record.id, record);
        return record;
    }

    public async deleteUser(id: string): Promise<boolean> {
        return this.storage.delete(id);
    }
}
"#,
    )
    .unwrap();

    // Act 1: Fast stats calculation
    let fast_report = calculate_fast_stats(root).unwrap();
    assert_eq!(fast_report.total_files, 2);
    assert!(fast_report.total_raw_tokens > 50);
    assert!(fast_report.total_sliced_tokens > 0);
    assert!(fast_report.savings_percentage > 0.0);
    assert_eq!(fast_report.files.len(), 2);

    // Format text
    let formatted = format_stats_text(&fast_report);
    assert!(formatted.contains("ctxcut Token Optimization & Context Statistics"));
    assert!(formatted.contains("Total Files Analyzed: 2"));

    // Serialize to JSON and verify roundtrip
    let json_str = serde_json::to_string(&fast_report).unwrap();
    let parsed: Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed["total_files"].as_u64(), Some(2));
    assert!(parsed["savings_percentage"].as_f64().unwrap() > 0.0);

    // Act 2: Deep stats calculation
    let deep_report = calculate_deep_stats(root).unwrap();
    eprintln!("FAST REPORT: {:?}", fast_report);
    eprintln!("DEEP REPORT: {:?}", deep_report);
    assert_eq!(deep_report.total_files, 2);
    assert_eq!(deep_report.total_raw_tokens, fast_report.total_raw_tokens);
    assert!(deep_report.savings_percentage >= 0.0);

    // Act 3: Unified calculate_stats dispatch
    let dispatched_fast = calculate_stats(root, true).unwrap();
    assert_eq!(dispatched_fast.total_files, 2);

    let dispatched_deep = calculate_stats(root, false).unwrap();
    assert_eq!(dispatched_deep.total_files, 2);
}
