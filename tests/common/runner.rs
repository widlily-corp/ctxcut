//! Command and MCP Test Runner utilities for ctxcut E2E test suite.
//!
//! Provides `CliRunner` for invoking `ctxcut` CLI subcommands with fluent assertions,
//! and `McpClient` for testing Model Context Protocol STDIO JSON-RPC 2.0 communication.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Structured output from executing a CLI command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Captured STDOUT string.
    pub stdout: String,
    /// Captured STDERR string.
    pub stderr: String,
    /// Process exit code (0 if success).
    pub exit_code: i32,
    /// Whether the command exited with status code 0.
    pub success: bool,
}

impl CommandOutput {
    /// Asserts that the command succeeded (exit code 0).
    pub fn assert_success(&self) -> &Self {
        assert!(
            self.success,
            "Expected command to succeed, but failed with exit code: {}\nSTDOUT:\n{}\nSTDERR:\n{}",
            self.exit_code, self.stdout, self.stderr
        );
        self
    }

    /// Asserts that the command failed (non-zero exit code).
    pub fn assert_failure(&self) -> &Self {
        assert!(
            !self.success,
            "Expected command to fail, but succeeded with exit code: {}\nSTDOUT:\n{}\nSTDERR:\n{}",
            self.exit_code, self.stdout, self.stderr
        );
        self
    }

    /// Asserts that STDOUT contains the given substring.
    pub fn assert_stdout_contains(&self, substr: &str) -> &Self {
        assert!(
            self.stdout.contains(substr),
            "Expected STDOUT to contain {:?}\nActual STDOUT:\n{}",
            substr,
            self.stdout
        );
        self
    }

    /// Asserts that STDERR contains the given substring.
    pub fn assert_stderr_contains(&self, substr: &str) -> &Self {
        assert!(
            self.stderr.contains(substr),
            "Expected STDERR to contain {:?}\nActual STDERR:\n{}",
            substr,
            self.stderr
        );
        self
    }

    /// Deserializes STDOUT JSON into target type `T`.
    pub fn parse_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.stdout)
    }
}

/// CLI runner for executing `ctxcut` binary in tests.
#[derive(Debug, Clone)]
pub struct CliRunner {
    bin_path: Option<PathBuf>,
}

impl Default for CliRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CliRunner {
    /// Creates a new `CliRunner` resolving `ctxcut` binary from target directory or env.
    pub fn new() -> Self {
        let bin_path = Self::find_binary();
        Self { bin_path }
    }

    /// Creates a `CliRunner` with an explicit binary path.
    pub fn with_bin_path(path: impl Into<PathBuf>) -> Self {
        Self {
            bin_path: Some(path.into()),
        }
    }

    /// Discovers `ctxcut` binary path using Cargo environment or target directories.
    fn find_binary() -> Option<PathBuf> {
        if let Ok(exe) = std::env::var("CARGO_BIN_EXE_ctxcut") {
            let path = PathBuf::from(exe);
            if path.exists() {
                return Some(path);
            }
        }

        // Search relative to cargo manifest dir or current working dir
        let search_roots = [
            std::env::current_dir().ok(),
            std::env::var("CARGO_MANIFEST_DIR").ok().map(PathBuf::from),
        ];

        for root in search_roots.into_iter().flatten() {
            let candidates = [
                root.join("target").join("debug").join(if cfg!(windows) { "ctxcut.exe" } else { "ctxcut" }),
                root.join("target").join("release").join(if cfg!(windows) { "ctxcut.exe" } else { "ctxcut" }),
                root.join("..").join("target").join("debug").join(if cfg!(windows) { "ctxcut.exe" } else { "ctxcut" }),
                root.join("..").join("target").join("release").join(if cfg!(windows) { "ctxcut.exe" } else { "ctxcut" }),
            ];

            for candidate in candidates {
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
    }

    /// Runs `ctxcut` with the provided arguments in the current working directory.
    pub fn run(&self, args: &[&str]) -> io::Result<CommandOutput> {
        self.run_with_env(None, args, &[])
    }

    /// Runs `ctxcut` with the provided arguments inside `cwd`.
    pub fn run_in_dir(&self, cwd: impl AsRef<Path>, args: &[&str]) -> io::Result<CommandOutput> {
        self.run_with_env(Some(cwd.as_ref()), args, &[])
    }

    /// Runs `ctxcut` with arguments, working directory, and environment overrides.
    pub fn run_with_env(
        &self,
        cwd: Option<&Path>,
        args: &[&str],
        envs: &[(&str, &str)],
    ) -> io::Result<CommandOutput> {
        let mut cmd = if let Some(ref bin) = self.bin_path {
            let mut c = Command::new(bin);
            c.args(args);
            c
        } else {
            let mut c = Command::new("cargo");
            c.args(["run", "--quiet", "--bin", "ctxcut", "--"]);
            c.args(args);
            c
        };

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        for (k, v) in envs {
            cmd.env(k, v);
        }

        let output = cmd.output()?;
        let exit_code = output.status.code().unwrap_or(-1);
        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(CommandOutput {
            stdout,
            stderr,
            exit_code,
            success,
        })
    }
}

/// Unified test runner alias.
pub type TestRunner = CliRunner;

/// STDIO Model Context Protocol (MCP) JSON-RPC 2.0 test client.
pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    request_id: AtomicU64,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("request_id", &self.request_id.load(Ordering::SeqCst))
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'a str,
    id: u64,
    method: &'a str,
    #[serde(skip_serializing_if = "Value::is_null")]
    params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<Value>,
}

impl McpClient {
    /// Starts `ctxcut mcp` child process in default directory.
    pub fn start() -> io::Result<Self> {
        Self::start_in_dir(std::env::current_dir()?)
    }

    /// Starts `ctxcut mcp` child process with specific working directory.
    pub fn start_in_dir(cwd: impl AsRef<Path>) -> io::Result<Self> {
        Self::start_with_options(Some(cwd.as_ref()), &[], &[])
    }

    /// Starts `ctxcut mcp` child process with explicit directory, extra CLI arguments, and environment variables.
    pub fn start_with_options(
        cwd: Option<&Path>,
        extra_args: &[&str],
        envs: &[(&str, &str)],
    ) -> io::Result<Self> {
        let runner = CliRunner::new();
        let mut cmd = if let Some(ref bin) = runner.bin_path {
            let mut c = Command::new(bin);
            c.arg("mcp");
            c.args(extra_args);
            c
        } else {
            let mut c = Command::new("cargo");
            c.args(["run", "--quiet", "--bin", "ctxcut", "--", "mcp"]);
            c.args(extra_args);
            c
        };

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        for (k, v) in envs {
            cmd.env(k, v);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "Failed to capture MCP child stdin")
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "Failed to capture MCP child stdout")
        })?;

        Ok(Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            request_id: AtomicU64::new(1),
        })
    }

    /// Starts `ctxcut mcp` with `--log-file <path>`.
    pub fn start_with_log_file(log_file: impl AsRef<Path>) -> io::Result<Self> {
        let path_str = log_file.as_ref().to_string_lossy().to_string();
        Self::start_with_options(None, &["--log-file", &path_str], &[])
    }

    /// Starts `ctxcut mcp` with `CTXCUT_LOG_FILE=<path>` environment variable.
    pub fn start_with_env_log_file(log_file: impl AsRef<Path>) -> io::Result<Self> {
        let path_str = log_file.as_ref().to_string_lossy().to_string();
        Self::start_with_options(None, &[], &[("CTXCUT_LOG_FILE", &path_str)])
    }

    /// Sends a raw JSON string line to MCP stdin and reads JSON-RPC response.
    pub fn send_raw_line(&mut self, line: &str) -> io::Result<Value> {
        let trimmed = line.trim();
        writeln!(self.stdin, "{}", trimmed)?;
        self.stdin.flush()?;

        let mut response_line = String::new();
        self.reader.read_line(&mut response_line)?;

        if response_line.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "MCP server closed stream or returned empty line",
            ));
        }

        let parsed: Value = serde_json::from_str(&response_line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse MCP JSON-RPC response: {}. Line was: {}", e, response_line),
            )
        })?;

        Ok(parsed)
    }

    /// Sends a structured JSON-RPC 2.0 request and returns the `result` field.
    pub fn send_request(&mut self, method: &str, params: Value) -> io::Result<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let req_json = serde_json::to_string(&req).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("Serialization error: {}", e))
        })?;

        let response = self.send_raw_line(&req_json)?;

        if let Some(err) = response.get("error") {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("MCP JSON-RPC Error: {}", err),
            ));
        }

        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Sends MCP `initialize` handshake request.
    pub fn initialize(&mut self) -> io::Result<Value> {
        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "ctxcut-test-runner",
                "version": "0.1.0"
            }
        });
        self.send_request("initialize", init_params)
    }

    /// Sends MCP `tools/list` request to discover available tools.
    pub fn list_tools(&mut self) -> io::Result<Vec<Value>> {
        let result = self.send_request("tools/list", serde_json::json!({}))?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(tools)
    }

    /// Calls a specific tool by name with arguments.
    pub fn call_tool(&mut self, tool_name: &str, arguments: Value) -> io::Result<Value> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        });
        self.send_request("tools/call", params)
    }

    /// Convenience helper for `get_symbol_slice` tool.
    pub fn get_symbol_slice(&mut self, path: &str, symbol: &str) -> io::Result<String> {
        let args = serde_json::json!({
            "path": path,
            "symbol": symbol
        });
        let result = self.call_tool("get_symbol_slice", args)?;
        extract_text_content(&result)
    }

    /// Convenience helper for `get_diff_slice` tool.
    pub fn get_diff_slice(&mut self, staged: bool) -> io::Result<String> {
        let args = serde_json::json!({
            "staged": staged
        });
        let result = self.call_tool("get_diff_slice", args)?;
        extract_text_content(&result)
    }

    /// Convenience helper for `analyze_token_stats` tool.
    pub fn analyze_token_stats(&mut self, path: &str) -> io::Result<Value> {
        let args = serde_json::json!({
            "path": path
        });
        self.call_tool("analyze_token_stats", args)
    }

    /// Closes child process gracefully.
    pub fn stop(&mut self) -> io::Result<()> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Helper alias for McpClient.
pub type McpRunner = McpClient;

fn extract_text_content(result: &Value) -> io::Result<String> {
    if let Some(content) = result.get("content").and_then(Value::as_array) {
        for item in content {
            if item.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    return Ok(text.to_string());
                }
            }
        }
    }
    Ok(result.to_string())
}
