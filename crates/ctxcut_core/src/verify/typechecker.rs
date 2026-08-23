//! Language typechecker detection, execution runner with process timeout, and diagnostic parsing.

use crate::model::{SupportedLanguage, VerifyDiagnostic};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Automatic typechecker command detection for all supported languages.
pub struct TypecheckerDetector;

impl TypecheckerDetector {
    /// Detects the appropriate typechecker command for a given file and workspace.
    pub fn detect(
        workspace_root: &Path,
        file_path: &Path,
        language: SupportedLanguage,
    ) -> Option<String> {
        let rel_file = file_path
            .strip_prefix(workspace_root)
            .unwrap_or(file_path);
        let rel_file_str = rel_file.to_string_lossy();

        match language {
            SupportedLanguage::Rust => {
                if workspace_root.join("Cargo.toml").exists()
                    || find_file_upward(file_path, "Cargo.toml").is_some()
                {
                    Some("cargo check".to_string())
                } else {
                    Some(format!("rustc --crate-type lib --emit=metadata -o NUL \"{rel_file_str}\""))
                }
            }
            SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
                Some("npx tsc --noEmit".to_string())
            }
            SupportedLanguage::Python => {
                if workspace_root.join("mypy.ini").exists()
                    || workspace_root.join("pyproject.toml").exists()
                    || find_file_upward(file_path, "mypy.ini").is_some()
                    || find_file_upward(file_path, "pyproject.toml").is_some()
                {
                    Some(format!("mypy \"{rel_file_str}\""))
                } else {
                    Some(format!("python -m py_compile \"{rel_file_str}\""))
                }
            }
            SupportedLanguage::Go => {
                if workspace_root.join("go.mod").exists()
                    || find_file_upward(file_path, "go.mod").is_some()
                {
                    Some("go vet ./...".to_string())
                } else {
                    Some(format!("go vet \"{rel_file_str}\""))
                }
            }
            SupportedLanguage::CSharp => Some("dotnet build".to_string()),
            SupportedLanguage::Java => {
                if workspace_root.join("pom.xml").exists()
                    || find_file_upward(file_path, "pom.xml").is_some()
                {
                    Some("mvn compile -DskipTests".to_string())
                } else if workspace_root.join("build.gradle").exists()
                    || workspace_root.join("build.gradle.kts").exists()
                    || find_file_upward(file_path, "build.gradle").is_some()
                {
                    Some("gradle compileJava".to_string())
                } else {
                    Some(format!("javac \"{rel_file_str}\""))
                }
            }
            SupportedLanguage::Kotlin => {
                if workspace_root.join("build.gradle.kts").exists()
                    || find_file_upward(file_path, "build.gradle.kts").is_some()
                {
                    Some("gradle compileKotlin".to_string())
                } else {
                    Some(format!("kotlinc \"{rel_file_str}\" -nowarn"))
                }
            }
            SupportedLanguage::C => Some(format!("clang -fsyntax-only \"{rel_file_str}\"")),
            SupportedLanguage::Cpp => Some(format!("clang++ -fsyntax-only \"{rel_file_str}\"")),
            SupportedLanguage::Vue => Some("npx vue-tsc --noEmit".to_string()),
            SupportedLanguage::Svelte => Some("npx svelte-check".to_string()),
            SupportedLanguage::Astro => Some("npx astro check".to_string()),
        }
    }
}

fn find_file_upward(start: &Path, file_name: &str) -> Option<PathBuf> {
    let mut current = start.parent()?;
    loop {
        let candidate = current.join(file_name);
        if candidate.exists() {
            return Some(candidate);
        }
        current = current.parent()?;
    }
}

/// Output from executing a typechecker command.
#[derive(Debug, Clone)]
pub struct TypecheckExecutionResult {
    /// Whether the typechecker returned success (exit code 0).
    pub success: bool,
    /// Process exit code.
    pub exit_code: Option<i32>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
}

/// Process runner for typecheckers with timeout handling.
pub struct TypecheckerRunner;

impl TypecheckerRunner {
    /// Runs a typechecker command in a subprocess with timeout protection.
    pub fn run(command_str: &str, cwd: &Path, timeout: Duration) -> TypecheckExecutionResult {
        let (tx, rx) = mpsc::channel();
        let cmd_string = command_str.to_string();
        let working_dir = cwd.to_path_buf();

        thread::spawn(move || {
            let mut cmd = if cfg!(target_os = "windows") {
                let mut c = Command::new("powershell");
                c.args(["-NoProfile", "-NonInteractive", "-Command", &cmd_string]);
                c
            } else {
                let mut c = Command::new("sh");
                c.args(["-c", &cmd_string]);
                c
            };

            cmd.current_dir(&working_dir);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let output_res = cmd.output();
            let _ = tx.send(output_res);
        });

        match rx.recv_timeout(timeout) {
            Ok(Ok(output)) => {
                let exit_code = output.status.code();
                let success = output.status.success();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                TypecheckExecutionResult {
                    success,
                    exit_code,
                    stdout,
                    stderr,
                }
            }
            Ok(Err(e)) => TypecheckExecutionResult {
                success: false,
                exit_code: Some(1),
                stdout: String::new(),
                stderr: format!("Failed to spawn typechecker process: {e}"),
            },
            Err(mpsc::RecvTimeoutError::Timeout) => TypecheckExecutionResult {
                success: false,
                exit_code: Some(124),
                stdout: String::new(),
                stderr: format!("Typechecker timed out after {}s", timeout.as_secs()),
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => TypecheckExecutionResult {
                success: false,
                exit_code: Some(1),
                stdout: String::new(),
                stderr: "Typechecker worker thread disconnected unexpectedly".to_string(),
            },
        }
    }
}

/// Parses compiler output into structured diagnostics.
pub struct DiagnosticParser;

impl DiagnosticParser {
    /// Extracts structured `VerifyDiagnostic` entries from raw compiler stdout/stderr.
    pub fn parse(output: &str) -> Vec<VerifyDiagnostic> {
        let mut diagnostics = Vec::new();
        for line in output.lines() {
            let line_trimmed = line.trim();
            if line_trimmed.is_empty() {
                continue;
            }

            // TypeScript / GCC format: path/file.ts(line,col): error TS1234: Message
            // or path/file.ts:line:col: error: Message
            if let Some(diag) = parse_line_diagnostic(line_trimmed) {
                diagnostics.push(diag);
                continue;
            }

            // Rust format: error[E0308]: mismatched types
            if let Some(diag) = parse_rust_error_code(line_trimmed) {
                diagnostics.push(diag);
                continue;
            }

            // Generic error: / warning: lines
            if line_trimmed.contains("error:") || line_trimmed.contains("error[") {
                diagnostics.push(VerifyDiagnostic {
                    severity: "error".to_string(),
                    line: None,
                    column: None,
                    message: line_trimmed.to_string(),
                    file: None,
                    code: None,
                });
            } else if line_trimmed.contains("warning:") || line_trimmed.contains("warning[") {
                diagnostics.push(VerifyDiagnostic {
                    severity: "warning".to_string(),
                    line: None,
                    column: None,
                    message: line_trimmed.to_string(),
                    file: None,
                    code: None,
                });
            }
        }
        diagnostics
    }
}

fn parse_line_diagnostic(line: &str) -> Option<VerifyDiagnostic> {
    // Example: src/calc.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.
    if let Some(paren_open) = line.find('(') {
        if let Some(paren_close) = line[paren_open..].find(')') {
            let paren_close = paren_open + paren_close;
            let file_part = line[..paren_open].trim();
            let coords_part = &line[paren_open + 1..paren_close];
            let rest = line[paren_close + 1..].trim_start_matches(':').trim();

            let mut coords = coords_part.split(',');
            let line_num = coords.next()?.trim().parse::<usize>().ok();
            let col_num = coords.next().and_then(|c| c.trim().parse::<usize>().ok());

            let (severity, code, message) = parse_severity_code_message(rest);

            return Some(VerifyDiagnostic {
                severity,
                line: line_num,
                column: col_num,
                message,
                file: if file_part.is_empty() { None } else { Some(file_part.to_string()) },
                code,
            });
        }
    }

    // Example: src/calc.py:10: error: Incompatible return value type
    // Example: src/main.rs:12:5: error: mismatched types
    let parts: Vec<&str> = line.splitn(4, ':').collect();
    if parts.len() >= 3 {
        let file_part = parts[0].trim();
        if let Ok(line_num) = parts[1].trim().parse::<usize>() {
            let (col_num, rest) = if parts.len() >= 4 {
                if let Ok(col) = parts[2].trim().parse::<usize>() {
                    (Some(col), parts[3].trim().to_string())
                } else {
                    (None, format!("{}:{}", parts[2].trim(), parts[3].trim()))
                }
            } else {
                (None, parts[2].trim().to_string())
            };

            let (severity, code, message) = parse_severity_code_message(&rest);

            return Some(VerifyDiagnostic {
                severity,
                line: Some(line_num),
                column: col_num,
                message,
                file: Some(file_part.to_string()),
                code,
            });
        }
    }

    None
}

fn parse_rust_error_code(line: &str) -> Option<VerifyDiagnostic> {
    // Example: error[E0308]: mismatched types
    if line.starts_with("error[") {
        if let Some(end_bracket) = line.find(']') {
            let code = line[6..end_bracket].to_string();
            let rest = line[end_bracket + 1..].trim_start_matches(':').trim();
            return Some(VerifyDiagnostic {
                severity: "error".to_string(),
                line: None,
                column: None,
                message: rest.to_string(),
                file: None,
                code: Some(code),
            });
        }
    } else if line.starts_with("warning[") {
        if let Some(end_bracket) = line.find(']') {
            let code = line[8..end_bracket].to_string();
            let rest = line[end_bracket + 1..].trim_start_matches(':').trim();
            return Some(VerifyDiagnostic {
                severity: "warning".to_string(),
                line: None,
                column: None,
                message: rest.to_string(),
                file: None,
                code: Some(code),
            });
        }
    }
    None
}

fn parse_severity_code_message(rest: &str) -> (String, Option<String>, String) {
    let lower = rest.to_lowercase();
    let severity = if lower.contains("error") {
        "error".to_string()
    } else if lower.contains("warning") {
        "warning".to_string()
    } else {
        "info".to_string()
    };

    // Check for TS codes like error TS2322: ...
    if let Some(ts_idx) = rest.find("TS") {
        let code_part = &rest[ts_idx..];
        let code_len = code_part.chars().take_while(|c| c.is_ascii_alphanumeric()).count();
        let code = code_part[..code_len].to_string();
        let msg = rest.replace(&code, "").trim_start_matches(':').trim().to_string();
        return (severity, Some(code), msg);
    }

    (severity, None, rest.to_string())
}
