//! Integration tests for `ctxcut_cli::setup_mcp` module.

use std::fs;
use std::path::PathBuf;
use ctxcut_cli::setup_mcp::{
    get_ide_config_paths, safe_merge_json, setup_ide_mcp, IdeTarget, MergeStatus,
    SetupMcpOptions, SetupResult,
};
use serde_json::{json, Value};
use tempfile::{tempdir, NamedTempFile};

#[test]
fn test_safe_merge_json_basic_creation() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path().to_path_buf();
    let _ = fs::remove_file(&path);

    let status = safe_merge_json(&path, "ctxcut", &["mcp"], false).unwrap();
    assert_eq!(status, MergeStatus::Created);

    let content = fs::read_to_string(&path).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
    assert_eq!(parsed["mcpServers"]["ctxcut"]["args"][0], "mcp");
}

#[test]
fn test_safe_merge_json_preserves_other_servers() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mcp_config.json");

    let initial = json!({
        "mcpServers": {
            "server1": { "command": "cmd1", "args": ["arg1"] },
            "server2": { "command": "cmd2", "args": ["arg2"] }
        }
    });

    fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

    let status = safe_merge_json(&path, "ctxcut", &["mcp"], false).unwrap();
    assert_eq!(status, MergeStatus::Updated);

    let content = fs::read_to_string(&path).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["mcpServers"]["server1"]["command"], "cmd1");
    assert_eq!(parsed["mcpServers"]["server2"]["command"], "cmd2");
    assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
}

#[test]
fn test_safe_merge_json_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mcp.json");

    let s1 = safe_merge_json(&path, "ctxcut", &["mcp"], false).unwrap();
    assert_eq!(s1, MergeStatus::Created);

    let s2 = safe_merge_json(&path, "ctxcut", &["mcp"], false).unwrap();
    assert_eq!(s2, MergeStatus::NoChange);
}

#[test]
fn test_safe_merge_json_corrupt_recovery() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupted.json");

    fs::write(&path, "invalid { json ]").unwrap();

    let res = safe_merge_json(&path, "ctxcut", &["mcp"], false);
    assert!(res.is_err());

    let bak = path.with_extension("corrupt_bak");
    assert!(bak.exists());
}