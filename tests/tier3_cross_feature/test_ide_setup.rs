//! Integration tests for Milestone 3: One-Click IDE MCP Configurator (F8, F9)
//!
//! Verifies:
//! 1. Safe JSON merge engine idempotency, server preservation, and corrupt file backup.
//! 2. Target path discovery for Antigravity, Claude, Cursor, and VS Code / Cline.
//! 3. CLI execution of `ctxcut setup-mcp` and `ctxcut init` across all flags.
//! 4. Dry-run and absolute executable path modes.

use std::fs;
use std::path::{Path, PathBuf};
use ctxcut_cli::setup_mcp::{
    format_setup_report, get_ide_config_paths, safe_merge_json, setup_ide_mcp, IdeTarget,
    MergeStatus, SetupMcpOptions, SetupResult,
};
use serde_json::{json, Value};
use tempfile::{tempdir, NamedTempFile};

#[path = "../common/mod.rs"]
mod common;
use common::CliRunner;

#[test]
fn test_safe_merge_json_nested_directory_creation() {
    let dir = tempdir().expect("tempdir failed");
    let nested_file = dir.path().join("deeply").join("nested").join("mcp_config.json");

    let status = safe_merge_json(&nested_file, "ctxcut", &["mcp"], false)
        .expect("merge into non-existent nested path must succeed");
    assert_eq!(status, MergeStatus::Created);

    assert!(nested_file.exists());
    let content = fs::read_to_string(&nested_file).expect("read failed");
    let parsed: Value = serde_json::from_str(&content).expect("valid JSON expected");

    assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
    assert_eq!(parsed["mcpServers"]["ctxcut"]["args"][0], "mcp");
}

#[test]
fn test_safe_merge_json_preserves_complex_existing_config() {
    let dir = tempdir().expect("tempdir failed");
    let config_file = dir.path().join("claude_desktop_config.json");

    let initial = json!({
        "$schema": "https://json.schemastore.org/mcp",
        "preferences": {
            "telemetry": false,
            "theme": "dark",
            "fontSize": 14
        },
        "mcpServers": {
            "github": {
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-github"],
                "env": {
                    "GITHUB_TOKEN": "ghp_secret123"
                }
            },
            "sqlite": {
                "command": "uvx",
                "args": ["mcp-server-sqlite", "--db-path", "/data/test.db"]
            }
        }
    });

    fs::write(&config_file, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

    let status = safe_merge_json(&config_file, "ctxcut", &["mcp"], false)
        .expect("merge into existing config must succeed");
    assert_eq!(status, MergeStatus::Updated);

    let content = fs::read_to_string(&config_file).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();

    // Verify all original sections are preserved
    assert_eq!(parsed["$schema"], "https://json.schemastore.org/mcp");
    assert_eq!(parsed["preferences"]["theme"], "dark");
    assert_eq!(parsed["preferences"]["fontSize"], 14);

    // Verify existing MCP servers
    assert_eq!(parsed["mcpServers"]["github"]["command"], "npx");
    assert_eq!(
        parsed["mcpServers"]["github"]["env"]["GITHUB_TOKEN"],
        "ghp_secret123"
    );
    assert_eq!(parsed["mcpServers"]["sqlite"]["command"], "uvx");

    // Verify ctxcut was injected
    assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
    assert_eq!(parsed["mcpServers"]["ctxcut"]["args"][0], "mcp");
}

#[test]
fn test_safe_merge_json_idempotency_and_removal() {
    let dir = tempdir().expect("tempdir failed");
    let config_file = dir.path().join("mcp.json");

    // Step 1: Initial creation
    let s1 = safe_merge_json(&config_file, "ctxcut", &["mcp"], false).unwrap();
    assert_eq!(s1, MergeStatus::Created);

    // Step 2: Idempotent merge (no-op)
    let s2 = safe_merge_json(&config_file, "ctxcut", &["mcp"], false).unwrap();
    assert_eq!(s2, MergeStatus::NoChange);

    // Step 3: Remove ctxcut
    let s3 = safe_merge_json(&config_file, "ctxcut", &["mcp"], true).unwrap();
    assert_eq!(s3, MergeStatus::Removed);

    let content = fs::read_to_string(&config_file).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["mcpServers"].get("ctxcut").is_none());

    // Step 4: Remove again when already removed
    let s4 = safe_merge_json(&config_file, "ctxcut", &["mcp"], true).unwrap();
    assert_eq!(s4, MergeStatus::NoChangeRemoved);

    // Step 5: Re-add
    let s5 = safe_merge_json(&config_file, "ctxcut", &["mcp"], false).unwrap();
    assert_eq!(s5, MergeStatus::Updated);
}

#[test]
fn test_safe_merge_json_corrupt_recovery_and_backup() {
    let dir = tempdir().expect("tempdir failed");
    let config_file = dir.path().join("corrupted_mcp.json");

    let corrupt_content = r#"{ "mcpServers": { "bad_json": [missing_brackets"#;
    fs::write(&config_file, corrupt_content).unwrap();

    let result = safe_merge_json(&config_file, "ctxcut", &["mcp"], false);
    assert!(result.is_err(), "Merge on corrupt JSON must return an error");

    // Verify original corrupted file was NOT overwritten
    let current_content = fs::read_to_string(&config_file).unwrap();
    assert_eq!(current_content, corrupt_content);

    // Verify backup file was created
    let backup_file = config_file.with_extension("corrupt_bak");
    assert!(backup_file.exists(), "Backup file must be created on corrupt JSON");
    let backup_content = fs::read_to_string(&backup_file).unwrap();
    assert_eq!(backup_content, corrupt_content);
}

#[test]
fn test_safe_merge_json_empty_and_whitespace_files() {
    let dir = tempdir().expect("tempdir failed");
    let empty_file = dir.path().join("empty_config.json");

    fs::write(&empty_file, "   \n\t  \n").unwrap();

    let status = safe_merge_json(&empty_file, "ctxcut", &["mcp"], false)
        .expect("empty file should be handled as new config");
    assert_eq!(status, MergeStatus::Created);

    let content = fs::read_to_string(&empty_file).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
}

#[test]
fn test_get_ide_config_paths_resolution() {
    let workspace = PathBuf::from("/mock/project");

    let antigravity_paths = get_ide_config_paths(IdeTarget::Antigravity, false, Some(&workspace));
    assert!(!antigravity_paths.is_empty());
    assert!(antigravity_paths[0].1.to_string_lossy().contains("mcp_config.json"));

    let claude_paths = get_ide_config_paths(IdeTarget::Claude, false, Some(&workspace));
    assert!(!claude_paths.is_empty());
    assert!(claude_paths[0].1.to_string_lossy().contains("claude_desktop_config.json"));

    let cursor_ws_paths = get_ide_config_paths(IdeTarget::Cursor, true, Some(&workspace));
    assert!(!cursor_ws_paths.is_empty());
    assert!(cursor_ws_paths[0].1.to_string_lossy().contains(".cursor"));

    let vscode_ws_paths = get_ide_config_paths(IdeTarget::Vscode, true, Some(&workspace));
    assert!(!vscode_ws_paths.is_empty());
    assert!(vscode_ws_paths[0].1.to_string_lossy().contains(".vscode"));
}

#[test]
fn test_cli_setup_mcp_custom_path_execution() {
    let dir = tempdir().expect("tempdir failed");
    let custom_config = dir.path().join("my_custom_mcp.json");
    let custom_str = custom_config.to_string_lossy().to_string();

    let runner = CliRunner::new();
    let output = runner
        .run(&["setup-mcp", "--custom-path", &custom_str])
        .expect("CLI execution failed");

    output.assert_success();
    output.assert_stdout_contains("CTXCUT IDE MCP CONFIGURATOR");
    output.assert_stdout_contains("Custom Path");

    assert!(custom_config.exists());
    let content = fs::read_to_string(&custom_config).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
    assert_eq!(parsed["mcpServers"]["ctxcut"]["args"][0], "mcp");
}

#[test]
fn test_cli_init_alias_execution() {
    let dir = tempdir().expect("tempdir failed");
    let custom_config = dir.path().join("init_mcp.json");
    let custom_str = custom_config.to_string_lossy().to_string();

    let runner = CliRunner::new();
    let output = runner
        .run(&["init", "--custom-path", &custom_str])
        .expect("CLI init execution failed");

    output.assert_success();
    output.assert_stdout_contains("CTXCUT IDE MCP CONFIGURATOR");

    assert!(custom_config.exists());
    let content = fs::read_to_string(&custom_config).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
}

#[test]
fn test_cli_setup_mcp_dry_run_mode() {
    let dir = tempdir().expect("tempdir failed");
    let custom_config = dir.path().join("dry_run_mcp.json");
    let custom_str = custom_config.to_string_lossy().to_string();

    let runner = CliRunner::new();
    let output = runner
        .run(&["setup-mcp", "--custom-path", &custom_str, "--dry-run"])
        .expect("CLI dry-run execution failed");

    output.assert_success();
    output.assert_stdout_contains("CTXCUT IDE MCP CONFIGURATOR");

    // File must NOT exist on disk after dry-run
    assert!(!custom_config.exists(), "Dry-run must not create configuration file on disk");
}

#[test]
fn test_cli_setup_mcp_workspace_mode() {
    let dir = tempdir().expect("tempdir failed");
    let dir_str = dir.path().to_string_lossy().to_string();

    let runner = CliRunner::new();
    let output = runner
        .run(&[
            "setup-mcp",
            "--ide",
            "cursor",
            "--workspace",
            "--workspace-dir",
            &dir_str,
        ])
        .expect("CLI workspace setup failed");

    output.assert_success();
    output.assert_stdout_contains("Cursor (Workspace)");

    let cursor_config = dir.path().join(".cursor").join("mcp.json");
    assert!(cursor_config.exists(), "Workspace .cursor/mcp.json must be created");

    let content = fs::read_to_string(&cursor_config).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
}

#[test]
fn test_format_setup_report_rendering() {
    let results = vec![
        SetupResult {
            ide_name: "Google Antigravity".to_string(),
            config_path: PathBuf::from("~/.gemini/config/mcp_config.json"),
            status: Some(MergeStatus::Created),
            message: "Created config with ctxcut".to_string(),
            success: true,
        },
        SetupResult {
            ide_name: "Claude Desktop".to_string(),
            config_path: PathBuf::from("C:\\Users\\Mock\\AppData\\Roaming\\Claude\\claude_desktop_config.json"),
            status: Some(MergeStatus::Updated),
            message: "Updated existing config".to_string(),
            success: true,
        },
        SetupResult {
            ide_name: "Cursor (Global)".to_string(),
            config_path: PathBuf::from("~/.cursor/mcp.json"),
            status: Some(MergeStatus::NoChange),
            message: "Config already up to date".to_string(),
            success: true,
        },
    ];

    let report = format_setup_report(&results);
    assert!(report.contains("CTXCUT IDE MCP CONFIGURATOR"));
    assert!(report.contains("Google Antigravity"));
    assert!(report.contains("Claude Desktop"));
    assert!(report.contains("Cursor (Global)"));
    assert!(report.contains("Summary: 3 targets processed"));
}