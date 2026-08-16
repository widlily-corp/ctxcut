//! `setup_mcp` — One-click IDE Model Context Protocol (MCP) configuration engine.
//!
//! Provides automated discovery, path resolution, atomic safe JSON merging,
//! and idempotency checks across Google Antigravity, Claude Desktop, Cursor,
//! and VS Code / Cline / Roo Code.

use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Target IDE for Model Context Protocol (MCP) configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdeTarget {
    /// Google Antigravity IDE (`~/.gemini/config/mcp_config.json`)
    Antigravity,
    /// Claude Desktop (`claude_desktop_config.json`)
    Claude,
    /// Cursor IDE (`~/.cursor/mcp.json` or `<workspace>/.cursor/mcp.json`)
    Cursor,
    /// VS Code / Cline / Roo Code (`.vscode/mcp.json` or cline settings)
    Vscode,
    /// Configure all detected IDEs
    All,
}

impl Default for IdeTarget {
    fn default() -> Self {
        Self::All
    }
}

impl fmt::Display for IdeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Antigravity => write!(f, "antigravity"),
            Self::Claude => write!(f, "claude"),
            Self::Cursor => write!(f, "cursor"),
            Self::Vscode => write!(f, "vscode"),
            Self::All => write!(f, "all"),
        }
    }
}

impl FromStr for IdeTarget {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "antigravity" | "gemini" | "google" | "google-antigravity" => Ok(Self::Antigravity),
            "claude" | "claude-desktop" | "anthropic" => Ok(Self::Claude),
            "cursor" => Ok(Self::Cursor),
            "vscode" | "visual-studio-code" | "cline" | "roo" | "code" => Ok(Self::Vscode),
            "all" | "*" => Ok(Self::All),
            other => bail!(
                "Unknown IDE target `{}`. Supported: antigravity, claude, cursor, vscode, all",
                other
            ),
        }
    }
}

/// Status of an individual configuration merge operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStatus {
    /// New configuration file created with `ctxcut` MCP server entry.
    Created,
    /// Existing configuration file updated with `ctxcut` MCP server entry.
    Updated,
    /// Configuration file already contains identical `ctxcut` entry (idempotent).
    NoChange,
    /// `ctxcut` MCP server entry removed from configuration file.
    Removed,
    /// `ctxcut` entry was not present to remove.
    NoChangeRemoved,
}

impl MergeStatus {
    /// Human-readable status label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Updated => "Updated",
            Self::NoChange => "Up to date",
            Self::Removed => "Removed",
            Self::NoChangeRemoved => "Not present",
        }
    }
}

/// Result of an IDE configuration operation for reporting and telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupResult {
    /// Display name of target IDE.
    pub ide_name: String,
    /// Absolute or relative path to the configuration file.
    pub config_path: PathBuf,
    /// Outcome status of the merge operation.
    pub status: Option<MergeStatus>,
    /// Status description message.
    pub message: String,
    /// Whether the operation succeeded without fatal errors.
    pub success: bool,
}

/// Configuration options for `setup-mcp` execution.
#[derive(Debug, Clone)]
pub struct SetupMcpOptions {
    /// Target IDE to configure.
    pub ide: IdeTarget,
    /// Custom configuration JSON path override.
    pub custom_path: Option<PathBuf>,
    /// Whether to configure workspace-level settings instead of global.
    pub workspace: bool,
    /// Workspace root directory path.
    pub workspace_dir: Option<PathBuf>,
    /// Whether to remove `ctxcut` instead of adding.
    pub remove: bool,
    /// Use absolute executable path instead of `ctxcut` binary name.
    pub use_absolute_path: bool,
    /// Preview operations without writing to disk.
    pub dry_run: bool,
}

impl Default for SetupMcpOptions {
    fn default() -> Self {
        Self {
            ide: IdeTarget::All,
            custom_path: None,
            workspace: false,
            workspace_dir: None,
            remove: false,
            use_absolute_path: false,
            dry_run: false,
        }
    }
}

/// Executes IDE MCP setup according to specified parameters.
pub fn run_setup_mcp(options: &SetupMcpOptions) -> Result<Vec<SetupResult>> {
    let command_str = if options.use_absolute_path {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "ctxcut".to_string())
    } else {
        "ctxcut".to_string()
    };

    let mut results = Vec::new();

    if let Some(ref custom) = options.custom_path {
        let res = apply_config_target(
            "Custom Path",
            custom,
            &command_str,
            &["mcp"],
            options.remove,
            options.dry_run,
        );
        results.push(res);
        return Ok(results);
    }

    let targets = match options.ide {
        IdeTarget::All => vec![
            IdeTarget::Antigravity,
            IdeTarget::Claude,
            IdeTarget::Cursor,
            IdeTarget::Vscode,
        ],
        single => vec![single],
    };

    let resolved_ws = options
        .workspace_dir
        .clone()
        .or_else(|| std::env::current_dir().ok());

    for ide in targets {
        let paths = get_ide_config_paths(ide, options.workspace, resolved_ws.as_deref());
        for (ide_name, path) in paths {
            let res = apply_config_target(
                &ide_name,
                &path,
                &command_str,
                &["mcp"],
                options.remove,
                options.dry_run,
            );
            results.push(res);
        }
    }

    Ok(results)
}

/// Convenience helper conforming to standard interface contract.
pub fn setup_ide_mcp(target: IdeTarget, workspace_dir: Option<PathBuf>) -> Result<Vec<SetupResult>> {
    let options = SetupMcpOptions {
        ide: target,
        workspace_dir,
        ..Default::default()
    };
    run_setup_mcp(&options)
}

fn apply_config_target(
    ide_name: &str,
    path: &Path,
    command: &str,
    args: &[&str],
    remove: bool,
    dry_run: bool,
) -> SetupResult {
    if dry_run {
        let action = if remove { "would remove from" } else { "would configure in" };
        return SetupResult {
            ide_name: ide_name.to_string(),
            config_path: path.to_path_buf(),
            status: Some(MergeStatus::NoChange),
            message: format!("[DRY-RUN] `ctxcut` {} `{}`", action, path.display()),
            success: true,
        };
    }

    match safe_merge_json(path, command, args, remove) {
        Ok(status) => {
            let msg = match status {
                MergeStatus::Created => format!("Created config with ctxcut in `{}`", path.display()),
                MergeStatus::Updated => format!("Updated config with ctxcut in `{}`", path.display()),
                MergeStatus::NoChange => format!("Config already up-to-date in `{}`", path.display()),
                MergeStatus::Removed => format!("Removed ctxcut from `{}`", path.display()),
                MergeStatus::NoChangeRemoved => format!("ctxcut was not present in `{}`", path.display()),
            };

            SetupResult {
                ide_name: ide_name.to_string(),
                config_path: path.to_path_buf(),
                status: Some(status),
                message: msg,
                success: true,
            }
        }
        Err(err) => SetupResult {
            ide_name: ide_name.to_string(),
            config_path: path.to_path_buf(),
            status: None,
            message: format!("Failed to configure `{}`: {}", path.display(), err),
            success: false,
        },
    }
}

/// Safely merges or removes the `ctxcut` entry in the `mcpServers` object of a JSON file.
///
/// Ensures atomic write via temporary file replace, preserves preexisting servers and root keys,
/// and handles non-existent or empty files cleanly.
pub fn safe_merge_json(
    path: &Path,
    command: &str,
    args: &[&str],
    remove: bool,
) -> Result<MergeStatus> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory `{}`", parent.display()))?;
        }
    }

    let file_exists = path.exists();
    let mut is_new_file = !file_exists;

    let mut root: Value = if file_exists {
        let raw_content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file `{}`", path.display()))?;
        let trimmed = raw_content.trim();
        if trimmed.is_empty() {
            is_new_file = true;
            json!({})
        } else {
            match serde_json::from_str::<Value>(trimmed) {
                Ok(val) => val,
                Err(err) => {
                    let bak_path = path.with_extension("corrupt_bak");
                    let _ = fs::copy(path, &bak_path);
                    bail!(
                        "Invalid JSON syntax in `{}`: {}. Backup saved to `{}` without modifying original.",
                        path.display(),
                        err,
                        bak_path.display()
                    );
                }
            }
        }
    } else {
        json!({})
    };

    if !root.is_object() {
        root = json!({});
    }

    let root_obj = root
        .as_object_mut()
        .context("Root JSON structure is not an object")?;

    let servers = root_obj
        .entry("mcpServers")
        .or_insert_with(|| json!({}));

    if !servers.is_object() {
        *servers = json!({});
    }

    let servers_obj = servers
        .as_object_mut()
        .context("mcpServers property is not a JSON object")?;

    let status = if remove {
        if servers_obj.remove("ctxcut").is_some() {
            MergeStatus::Removed
        } else {
            return Ok(MergeStatus::NoChangeRemoved);
        }
    } else {
        let new_server_entry = json!({
            "command": command,
            "args": args
        });

        if let Some(existing) = servers_obj.get("ctxcut") {
            if existing == &new_server_entry {
                return Ok(MergeStatus::NoChange);
            }
            servers_obj.insert("ctxcut".to_string(), new_server_entry);
            MergeStatus::Updated
        } else {
            servers_obj.insert("ctxcut".to_string(), new_server_entry);
            if is_new_file {
                MergeStatus::Created
            } else {
                MergeStatus::Updated
            }
        }
    };

    let mut formatted = serde_json::to_string_pretty(&root)?;
    formatted.push('\n');

    // Atomic write via temporary file
    let tmp_path = format!("{}.tmp.{}", path.display(), std::process::id());
    let tmp_path = PathBuf::from(tmp_path);

    fs::write(&tmp_path, formatted.as_bytes())
        .with_context(|| format!("Failed to write temporary file `{}`", tmp_path.display()))?;

    if let Err(err) = fs::rename(&tmp_path, path) {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "Failed to atomically replace `{}` with temporary file `{}`: {}",
                path.display(),
                tmp_path.display(),
                err
            )
        })?;
    }

    Ok(status)
}

/// Convenience alias for `safe_merge_json`.
pub fn merge_mcp_config(
    path: &Path,
    command: &str,
    args: &[&str],
    remove: bool,
) -> Result<MergeStatus> {
    safe_merge_json(path, command, args, remove)
}

/// Resolves configuration file paths for a given IDE target and environment.
pub fn get_ide_config_paths(
    target: IdeTarget,
    workspace: bool,
    workspace_root: Option<&Path>,
) -> Vec<(String, PathBuf)> {
    let mut list = Vec::new();
    let home = get_home_dir();

    match target {
        IdeTarget::Antigravity => {
            if let Some(h) = &home {
                list.push((
                    "Google Antigravity".to_string(),
                    h.join(".gemini").join("config").join("mcp_config.json"),
                ));
            }
        }
        IdeTarget::Claude => {
            if let Some(path) = get_claude_desktop_path() {
                list.push(("Claude Desktop".to_string(), path));
            }
        }
        IdeTarget::Cursor => {
            if workspace {
                if let Some(ws) = workspace_root {
                    list.push((
                        "Cursor (Workspace)".to_string(),
                        ws.join(".cursor").join("mcp.json"),
                    ));
                }
            } else {
                if let Some(h) = &home {
                    list.push((
                        "Cursor (Global)".to_string(),
                        h.join(".cursor").join("mcp.json"),
                    ));
                }
                if let Some(ws) = workspace_root {
                    list.push((
                        "Cursor (Workspace)".to_string(),
                        ws.join(".cursor").join("mcp.json"),
                    ));
                }
            }
        }
        IdeTarget::Vscode => {
            if workspace {
                if let Some(ws) = workspace_root {
                    list.push((
                        "VS Code (Workspace)".to_string(),
                        ws.join(".vscode").join("mcp.json"),
                    ));
                }
            } else {
                if let Some(cline_path) = get_cline_settings_path() {
                    list.push(("Cline (Global)".to_string(), cline_path));
                }
                if let Some(roo_path) = get_roo_code_settings_path() {
                    list.push(("Roo Code (Global)".to_string(), roo_path));
                }
                if let Some(ws) = workspace_root {
                    list.push((
                        "VS Code (Workspace)".to_string(),
                        ws.join(".vscode").join("mcp.json"),
                    ));
                }
            }
        }
        IdeTarget::All => {}
    }

    list
}

/// Resolves user home directory across Windows and Unix platforms.
pub fn get_home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| {
                let drive = std::env::var_os("HOMEDRIVE")?;
                let path = std::env::var_os("HOMEPATH")?;
                let mut pb = PathBuf::from(drive);
                pb.push(path);
                Some(pb)
            })
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// Resolves Claude Desktop configuration path across operating systems.
pub fn get_claude_desktop_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| get_home_dir().map(|h| h.join("AppData").join("Roaming")))
            .map(|p| p.join("Claude").join("claude_desktop_config.json"))
    }
    #[cfg(target_os = "macos")]
    {
        get_home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Claude")
                .join("claude_desktop_config.json")
        })
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| get_home_dir().map(|h| h.join(".config")))
            .map(|p| p.join("Claude").join("claude_desktop_config.json"))
    }
}

/// Resolves VS Code Cline MCP extension configuration path.
pub fn get_cline_settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|p| {
            PathBuf::from(p)
                .join("Code")
                .join("User")
                .join("globalStorage")
                .join("saoudrizwan.claude-dev")
                .join("settings")
                .join("cline_mcp_settings.json")
        })
    }
    #[cfg(target_os = "macos")]
    {
        get_home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Code")
                .join("User")
                .join("globalStorage")
                .join("saoudrizwan.claude-dev")
                .join("settings")
                .join("cline_mcp_settings.json")
        })
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        get_home_dir().map(|h| {
            h.join(".config")
                .join("Code")
                .join("User")
                .join("globalStorage")
                .join("saoudrizwan.claude-dev")
                .join("settings")
                .join("cline_mcp_settings.json")
        })
    }
}

/// Resolves VS Code Roo Code MCP extension configuration path.
pub fn get_roo_code_settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|p| {
            PathBuf::from(p)
                .join("Code")
                .join("User")
                .join("globalStorage")
                .join("rooveterinaryinc.roo-cline")
                .join("settings")
                .join("cline_mcp_settings.json")
        })
    }
    #[cfg(target_os = "macos")]
    {
        get_home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Code")
                .join("User")
                .join("globalStorage")
                .join("rooveterinaryinc.roo-cline")
                .join("settings")
                .join("cline_mcp_settings.json")
        })
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        get_home_dir().map(|h| {
            h.join(".config")
                .join("Code")
                .join("User")
                .join("globalStorage")
                .join("rooveterinaryinc.roo-cline")
                .join("settings")
                .join("cline_mcp_settings.json")
        })
    }
}

/// Formats setup results into a terminal table.
pub fn format_setup_report(results: &[SetupResult]) -> String {
    let mut out = String::new();
    out.push('\n');
    out.push_str(&"=".repeat(80));
    out.push('\n');
    out.push_str("                       CTXCUT IDE MCP CONFIGURATOR                      \n");
    out.push_str(&"=".repeat(80));
    out.push('\n');
    let _ = write!(
        out,
        " {:<22} {:<16} {}\n",
        "TARGET IDE".bold(),
        "STATUS".bold(),
        "CONFIG PATH".bold()
    );
    out.push_str(&"-".repeat(80));
    out.push('\n');

    let mut created_count = 0;
    let mut updated_count = 0;
    let mut up_to_date_count = 0;
    let mut removed_count = 0;
    let mut failed_count = 0;

    for res in results {
        let status_str = match res.status {
            Some(MergeStatus::Created) => {
                created_count += 1;
                "✔ Configured".green().bold().to_string()
            }
            Some(MergeStatus::Updated) => {
                updated_count += 1;
                "✔ Updated".green().bold().to_string()
            }
            Some(MergeStatus::NoChange) => {
                up_to_date_count += 1;
                "✔ Up to date".cyan().to_string()
            }
            Some(MergeStatus::Removed) => {
                removed_count += 1;
                "✔ Removed".yellow().to_string()
            }
            Some(MergeStatus::NoChangeRemoved) => {
                up_to_date_count += 1;
                "• Not present".white().to_string()
            }
            None => {
                failed_count += 1;
                "✖ Failed".red().bold().to_string()
            }
        };

        let _ = writeln!(
            out,
            " {:<22} {:<16} {}",
            res.ide_name,
            status_str,
            res.config_path.display()
        );
    }

    out.push_str(&"-".repeat(80));
    out.push('\n');
    let _ = writeln!(
        out,
        " Summary: {} targets processed ({} configured, {} updated, {} up-to-date, {} removed, {} failed)",
        results.len(),
        created_count,
        updated_count,
        up_to_date_count,
        removed_count,
        failed_count
    );
    out.push_str(&"=".repeat(80));
    out.push('\n');

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_ide_target_from_str() {
        assert_eq!(IdeTarget::from_str("antigravity").unwrap(), IdeTarget::Antigravity);
        assert_eq!(IdeTarget::from_str("gemini").unwrap(), IdeTarget::Antigravity);
        assert_eq!(IdeTarget::from_str("claude").unwrap(), IdeTarget::Claude);
        assert_eq!(IdeTarget::from_str("cursor").unwrap(), IdeTarget::Cursor);
        assert_eq!(IdeTarget::from_str("vscode").unwrap(), IdeTarget::Vscode);
        assert_eq!(IdeTarget::from_str("all").unwrap(), IdeTarget::All);
        assert!(IdeTarget::from_str("unknown_ide").is_err());
    }

    #[test]
    fn test_safe_merge_json_new_file() {
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
    fn test_safe_merge_json_preserves_existing_servers_and_keys() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let initial_json = json!({
            "$schema": "https://json.schemastore.org/mcp",
            "globalSettings": {
                "theme": "dark"
            },
            "mcpServers": {
                "github": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-github"]
                },
                "chrome-devtools": {
                    "command": "node",
                    "args": ["server.js"]
                }
            }
        });

        fs::write(path, serde_json::to_string_pretty(&initial_json).unwrap()).unwrap();

        let status = safe_merge_json(path, "ctxcut", &["mcp"], false).unwrap();
        assert_eq!(status, MergeStatus::Updated);

        let content = fs::read_to_string(path).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["$schema"], "https://json.schemastore.org/mcp");
        assert_eq!(parsed["globalSettings"]["theme"], "dark");
        assert_eq!(parsed["mcpServers"]["github"]["command"], "npx");
        assert_eq!(parsed["mcpServers"]["chrome-devtools"]["command"], "node");
        assert_eq!(parsed["mcpServers"]["ctxcut"]["command"], "ctxcut");
        assert_eq!(parsed["mcpServers"]["ctxcut"]["args"][0], "mcp");
    }

    #[test]
    fn test_safe_merge_json_idempotent() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let status1 = safe_merge_json(path, "ctxcut", &["mcp"], false).unwrap();
        assert_eq!(status1, MergeStatus::Created);

        let status2 = safe_merge_json(path, "ctxcut", &["mcp"], false).unwrap();
        assert_eq!(status2, MergeStatus::NoChange);
    }

    #[test]
    fn test_safe_merge_json_remove() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        safe_merge_json(path, "ctxcut", &["mcp"], false).unwrap();

        let status_remove = safe_merge_json(path, "ctxcut", &["mcp"], true).unwrap();
        assert_eq!(status_remove, MergeStatus::Removed);

        let content = fs::read_to_string(path).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["mcpServers"].get("ctxcut").is_none());

        let status_remove_again = safe_merge_json(path, "ctxcut", &["mcp"], true).unwrap();
        assert_eq!(status_remove_again, MergeStatus::NoChangeRemoved);
    }

    #[test]
    fn test_safe_merge_json_corrupt_file_recovery() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        fs::write(path, "{ corrupt json without closing brace").unwrap();

        let res = safe_merge_json(path, "ctxcut", &["mcp"], false);
        assert!(res.is_err());

        let bak_path = path.with_extension("corrupt_bak");
        assert!(bak_path.exists());
        let _ = fs::remove_file(&bak_path);
    }
}