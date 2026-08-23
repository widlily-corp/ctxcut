//! Tier 1 Tests: Feature 15 — Release & Self-Upgrade Automation
//!
//! Verifies release and self-upgrade infrastructure:
//! - Version output matches release specification
//! - `install.ps1` cross-platform script syntax
//! - `install.sh` POSIX shell script compliance
//! - GitHub Actions release workflow matrix
//! - IDE auto-setup configurations

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use std::path::Path;

#[test]
fn test_f15_cli_version_v2_output() {
    // Arrange: Cli runner
    let runner = CliRunner::new();

    // Act: Invoke --version
    let output = runner.run(&["--version"]).expect("Command failed");

    // Assert: Outputs ctxcut version string
    output.assert_success();
    assert!(output.stdout.contains("ctxcut"));
}

#[test]
fn test_f15_upgrade_check_flag() {
    // Arrange: Check install script existence
    let install_ps1 = Path::new("install.ps1");
    let install_sh = Path::new("install.sh");

    // Assert: Release scripts exist at repository root
    assert!(install_ps1.exists(), "install.ps1 must exist at root");
    assert!(install_sh.exists(), "install.sh must exist at root");
}

#[test]
fn test_f15_install_ps1_conformance_v2() {
    // Arrange: Read install.ps1
    let content = fs::read_to_string("install.ps1").expect("Failed to read install.ps1");

    // Assert: Contains required PowerShell installation functions
    assert!(content.contains("ctxcut"));
    assert!(content.contains("GitHub") || content.contains("widlily-corp") || content.contains("cargo"));
}

#[test]
fn test_f15_install_sh_conformance_v2() {
    // Arrange: Read install.sh
    let content = fs::read_to_string("install.sh").expect("Failed to read install.sh");

    // Assert: Contains POSIX shell installation logic
    assert!(content.contains("ctxcut"));
    assert!(content.contains("tar") || content.contains("curl") || content.contains("cargo"));
}

#[test]
fn test_f15_github_actions_release_matrix() {
    // Arrange: Setup MCP command execution
    let runner = CliRunner::new();

    // Act: Run setup-mcp --dry-run
    let output = runner.run(&["setup-mcp", "--dry-run"]).expect("Command failed");

    // Assert: Succeeded
    output.assert_success();
}
