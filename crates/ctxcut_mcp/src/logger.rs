//! Structured JSONL file logger for Model Context Protocol (MCP) server observability.
//!
//! Provides thread-safe, non-blocking file logging for incoming JSON-RPC requests,
//! tool execution timing, token reduction metrics, and error traces without polluting STDOUT.

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde_json::{json, Value};

/// Parameters for logging a tool execution event.
#[derive(Debug, Clone)]
pub struct ToolLogRecord<'a> {
    /// JSON-RPC request ID.
    pub id: Option<&'a Value>,
    /// Tool identifier.
    pub tool: &'a str,
    /// Input arguments passed to the tool.
    pub args: &'a Value,
    /// Execution duration in milliseconds.
    pub duration_ms: f64,
    /// Execution status (`"success"` or `"error"`).
    pub status: &'a str,
    /// Optional token reduction and line metrics.
    pub metrics: Option<&'a Value>,
    /// Optional error message.
    pub error: Option<&'a str>,
}

/// Formats a `SystemTime` into an ISO 8601 / RFC 3339 UTC timestamp string (`YYYY-MM-DDTHH:MM:SS.mmmZ`).
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn format_rfc3339(system_time: SystemTime) -> String {
    let duration = system_time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let total_secs = duration.as_secs();
    let millis = duration.subsec_millis();

    let days = (total_secs / 86_400) as i64;
    let day_secs = (total_secs % 86_400) as u32;

    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    // Howard Hinnant algorithm for Gregorian calendar calculation from epoch days
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = i64::from(yoe) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let final_y = if m <= 2 { y + 1 } else { y };

    format!("{final_y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Thread-safe, fail-safe structured JSONL file logger for MCP server observability.
#[derive(Debug, Clone)]
pub struct McpFileLogger {
    inner: Option<Arc<Mutex<BufWriter<File>>>>,
    log_path: Option<PathBuf>,
}

impl McpFileLogger {
    /// Initializes a new logger.
    ///
    /// If `log_path` is `Some`, opens or creates the file in append mode.
    /// If parent directories do not exist, they will be created automatically.
    /// If `log_path` is `None` or if file creation fails, returns a no-op logger.
    pub fn new(log_path: Option<PathBuf>) -> Self {
        let Some(path) = log_path else {
            return Self {
                inner: None,
                log_path: None,
            };
        };

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => Self {
                inner: Some(Arc::new(Mutex::new(BufWriter::new(file)))),
                log_path: Some(path),
            },
            Err(_) => Self {
                inner: None,
                log_path: None,
            },
        }
    }

    /// Initializes a new logger from an optional path reference.
    pub fn from_path_ref(path: Option<&Path>) -> Self {
        Self::new(path.map(Path::to_path_buf))
    }

    /// Returns `true` if file logging is actively enabled.
    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns the target log file path if configured.
    pub fn log_path(&self) -> Option<&Path> {
        self.log_path.as_deref()
    }

    /// Writes a structured JSON value as a single JSONL line to the log file.
    ///
    /// Never panics or outputs to STDOUT/STDERR on write failure.
    pub fn log_event(&self, event: &Value) {
        if let Some(ref lock) = self.inner {
            if let Ok(mut writer) = lock.lock() {
                if let Ok(line) = serde_json::to_string(event) {
                    let _ = writeln!(writer, "{line}");
                    let _ = writer.flush();
                }
            }
        }
    }

    /// Logs the server startup lifecycle event.
    pub fn log_start(&self) {
        if !self.is_enabled() {
            return;
        }
        let now = format_rfc3339(SystemTime::now());
        let path_str = self
            .log_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        self.log_event(&json!({
            "timestamp": now,
            "level": "INFO",
            "event": "server_start",
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "log_file": path_str
        }));
    }

    /// Logs an incoming JSON-RPC request.
    pub fn log_request(
        &self,
        method: &str,
        id: Option<&Value>,
        tool: Option<&str>,
        args: Option<&Value>,
    ) {
        if !self.is_enabled() {
            return;
        }
        let now = format_rfc3339(SystemTime::now());
        let mut payload = json!({
            "timestamp": now,
            "level": "INFO",
            "event": "rpc_request",
            "method": method
        });
        if let Some(id_val) = id {
            payload["request_id"] = id_val.clone();
        }
        if let Some(tool_name) = tool {
            payload["tool"] = json!(tool_name);
        }
        if let Some(arg_val) = args {
            payload["arguments"] = arg_val.clone();
        }
        self.log_event(&payload);
    }

    /// Logs a JSON-RPC response with duration and token savings.
    pub fn log_response(
        &self,
        method: &str,
        id: Option<&Value>,
        duration_ms: u128,
        tokens_saved: Option<usize>,
        error: Option<&str>,
    ) {
        if !self.is_enabled() {
            return;
        }
        let now = format_rfc3339(SystemTime::now());
        let level = if error.is_some() { "ERROR" } else { "INFO" };
        let mut payload = json!({
            "timestamp": now,
            "level": level,
            "event": "rpc_response",
            "method": method,
            "duration_ms": duration_ms
        });
        if let Some(id_val) = id {
            payload["request_id"] = id_val.clone();
        }
        if let Some(saved) = tokens_saved {
            payload["tokens_saved"] = json!(saved);
        }
        if let Some(err_msg) = error {
            payload["error"] = json!(err_msg);
        }
        self.log_event(&payload);
    }

    /// Logs a detailed tool execution event including timing and reduction metrics.
    pub fn log_tool_execution(&self, record: &ToolLogRecord<'_>) {
        if !self.is_enabled() {
            return;
        }
        let now = format_rfc3339(SystemTime::now());
        let level = if record.status == "error" || record.error.is_some() {
            "ERROR"
        } else {
            "INFO"
        };
        let mut payload = json!({
            "timestamp": now,
            "level": level,
            "event": "tool_call",
            "tool": record.tool,
            "arguments": record.args,
            "duration_ms": record.duration_ms,
            "status": record.status
        });
        if let Some(id_val) = record.id {
            payload["request_id"] = id_val.clone();
        }
        if let Some(m) = record.metrics {
            payload["metrics"] = m.clone();
        }
        if let Some(e) = record.error {
            payload["error"] = json!(e);
        }
        self.log_event(&payload);
    }

    /// Logs a JSON-RPC protocol error (e.g. method not found, parse error).
    pub fn log_rpc_error(&self, id: Option<&Value>, method: &str, code: i64, message: &str) {
        if !self.is_enabled() {
            return;
        }
        let now = format_rfc3339(SystemTime::now());
        let mut payload = json!({
            "timestamp": now,
            "level": "WARN",
            "event": "rpc_error",
            "method": method,
            "error_code": code,
            "error_message": message
        });
        if let Some(id_val) = id {
            payload["request_id"] = id_val.clone();
        }
        self.log_event(&payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use tempfile::NamedTempFile;

    #[test]
    fn test_logger_noop_when_none() {
        let logger = McpFileLogger::new(None);
        assert!(!logger.is_enabled());
        assert!(logger.log_path().is_none());

        // Calling logging methods must not panic
        logger.log_start();
        logger.log_request("tools/call", Some(&json!(1)), Some("test"), None);
        logger.log_response("tools/call", Some(&json!(1)), 10, Some(50), None);
        logger.log_rpc_error(Some(&json!(2)), "invalid", -32601, "Not found");
    }

    #[test]
    fn test_logger_creates_parent_directories() {
        let temp_dir = tempfile::tempdir().expect("tempdir failed");
        let nested_log = temp_dir.path().join("a").join("b").join("server.log");

        let logger = McpFileLogger::new(Some(nested_log.clone()));
        assert!(logger.is_enabled());
        assert_eq!(logger.log_path(), Some(nested_log.as_path()));

        logger.log_start();
        assert!(nested_log.exists());
    }

    #[test]
    fn test_logger_jsonl_valid_schema() {
        let temp_file = NamedTempFile::new().expect("tempfile failed");
        let log_path = temp_file.path().to_path_buf();

        let logger = McpFileLogger::new(Some(log_path.clone()));
        assert!(logger.is_enabled());

        logger.log_start();
        logger.log_request(
            "tools/call",
            Some(&json!(1)),
            Some("get_symbol_slice"),
            Some(&json!({"path": "test.ts", "symbol": "foo"})),
        );
        let args = json!({"path": "test.ts", "symbol": "foo"});
        let metrics = json!({
            "raw_tokens": 100,
            "sliced_tokens": 20,
            "saved_tokens": 80,
            "savings_percentage": 80.0
        });
        logger.log_tool_execution(&ToolLogRecord {
            id: Some(&json!(1)),
            tool: "get_symbol_slice",
            args: &args,
            duration_ms: 4.82,
            status: "success",
            metrics: Some(&metrics),
            error: None,
        });
        logger.log_response("tools/call", Some(&json!(1)), 5, Some(80), None);
        logger.log_rpc_error(Some(&json!(2)), "unknown_method", -32601, "Method not found");

        let file = File::open(&log_path).expect("failed to open log file");
        let lines: Vec<String> = std::io::BufReader::new(file)
            .lines()
            .map(|l| l.expect("failed reading line"))
            .collect();

        assert_eq!(lines.len(), 5);

        for line in &lines {
            let parsed: Value = serde_json::from_str(line).expect("must be valid JSON");
            assert!(parsed.get("timestamp").is_some());
            assert!(parsed.get("level").is_some());
            assert!(parsed.get("event").is_some());
        }

        // Verify tool execution record
        let tool_entry: Value = serde_json::from_str(&lines[2]).unwrap();
        assert_eq!(tool_entry["event"], "tool_call");
        assert_eq!(tool_entry["tool"], "get_symbol_slice");
        assert_eq!(tool_entry["status"], "success");
        assert_eq!(tool_entry["metrics"]["saved_tokens"], 80);
    }

    #[test]
    fn test_rfc3339_formatting() {
        let epoch = UNIX_EPOCH;
        assert_eq!(format_rfc3339(epoch), "1970-01-01T00:00:00.000Z");

        // 2020-01-01T00:00:00.000Z is 1577836800 seconds
        let y2020 = UNIX_EPOCH + Duration::from_secs(1_577_836_800);
        assert_eq!(format_rfc3339(y2020), "2020-01-01T00:00:00.000Z");

        let now_str = format_rfc3339(SystemTime::now());
        assert!(now_str.ends_with('Z'));
        assert!(now_str.contains('T'));
        assert_eq!(now_str.len(), 24);
    }
}
