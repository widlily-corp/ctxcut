//! Tests for Milestone 4: Distribution Installers & Release Workflow (F10, F11, F12)

use std::fs;
use std::path::PathBuf;

fn get_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.join("install.ps1").exists() {
        manifest_dir
    } else if let Some(parent) = manifest_dir.parent() {
        if parent.join("install.ps1").exists() {
            parent.to_path_buf()
        } else if let Some(grandparent) = parent.parent() {
            grandparent.to_path_buf()
        } else {
            manifest_dir
        }
    } else {
        manifest_dir
    }
}

#[test]
fn test_install_ps1_structure_and_syntax() {
    let root = get_workspace_root();
    let ps1_path = root.join("install.ps1");
    assert!(ps1_path.exists(), "install.ps1 must exist in repository root");

    let content = fs::read_to_string(&ps1_path)
        .expect("Failed to read install.ps1");

    // Required components for Windows PowerShell zero-friction installer
    assert!(content.contains("widlily-corp/ctxcut"), "Missing repository slug");
    assert!(content.contains("x86_64-pc-windows-msvc"), "Missing Windows target architecture");
    assert!(content.contains("$Version"), "Missing Version parameter");
    assert!(content.contains("$InstallDir"), "Missing InstallDir parameter");
    assert!(content.contains("$NoSetupMcp"), "Missing NoSetupMcp parameter");
    assert!(content.contains("SecurityProtocol"), "Missing TLS 1.2/1.3 security protocol setup");
    assert!(content.contains("Expand-Archive"), "Missing archive expansion logic");
    assert!(content.contains("SetEnvironmentVariable"), "Missing PATH persistence logic");
    assert!(content.contains("setup-mcp"), "Missing setup-mcp hook");
    assert!(content.contains("Quickstart"), "Missing quickstart instructions");
}

#[test]
fn test_install_sh_structure_and_conformance() {
    let root = get_workspace_root();
    let sh_path = root.join("install.sh");
    assert!(sh_path.exists(), "install.sh must exist in repository root");

    let content = fs::read_to_string(&sh_path)
        .expect("Failed to read install.sh");

    // POSIX shell headers and safety
    assert!(content.starts_with("#!/usr/bin/env sh") || content.starts_with("#!/bin/sh"), "Must have POSIX sh shebang");
    assert!(content.contains("set -eu"), "Must enable strict error checking (set -eu)");

    // Multi-platform detection
    assert!(content.contains("uname -s"), "Must detect OS via uname -s");
    assert!(content.contains("uname -m"), "Must detect architecture via uname -m");
    assert!(content.contains("x86_64-unknown-linux-gnu"), "Must support Linux x86_64");
    assert!(content.contains("x86_64-apple-darwin"), "Must support macOS x86_64");
    assert!(content.contains("aarch64-apple-darwin"), "Must support macOS ARM64");

    // Installation paths and downloaders
    assert!(content.contains("/usr/local/bin"), "Must consider /usr/local/bin");
    assert!(content.contains(".local/bin"), "Must consider .local/bin");
    assert!(content.contains("curl") && content.contains("wget"), "Must support both curl and wget");
    assert!(content.contains("setup-mcp"), "Must trigger setup-mcp hook");
}

#[test]
fn test_release_workflow_matrix_and_jobs() {
    let root = get_workspace_root();
    let workflow_path = root.join(".github").join("workflows").join("release.yml");
    assert!(workflow_path.exists(), ".github/workflows/release.yml must exist");

    let content = fs::read_to_string(&workflow_path)
        .expect("Failed to read release.yml");

    // Trigger specifications
    assert!(content.contains("tags:"), "Workflow must trigger on git tags");
    assert!(content.contains("'v*'"), "Workflow must match v* tags");
    assert!(content.contains("workflow_dispatch:"), "Workflow must support manual trigger");

    // Permissions
    assert!(content.contains("contents: write"), "Must have contents: write permission");

    // Build matrix coverage
    assert!(content.contains("x86_64-unknown-linux-gnu"), "Matrix must build Linux x86_64");
    assert!(content.contains("x86_64-apple-darwin"), "Matrix must build macOS x86_64");
    assert!(content.contains("aarch64-apple-darwin"), "Matrix must build macOS aarch64");
    assert!(content.contains("x86_64-pc-windows-msvc"), "Matrix must build Windows x86_64");

    // Packaging & Checksums
    assert!(content.contains("checksums.txt"), "Must generate consolidated checksums.txt");
    assert!(content.contains("softprops/action-gh-release@v2"), "Must publish using softprops/action-gh-release");
}
