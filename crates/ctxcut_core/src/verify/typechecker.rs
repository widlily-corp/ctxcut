//! Language typechecker detection, execution runner with process timeout, and diagnostic parsing.

use crate::model::{SupportedLanguage, VerifyDiagnostic};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Detailed resolution result for auto-detected or configured typechecker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypecheckerResolution {
    /// Command string to execute (e.g. `cargo check --manifest-path "..."`, `npx tsc --noEmit`).
    pub command: String,
    /// Working directory in which the typechecker should be invoked.
    pub working_dir: PathBuf,
    /// Path to the resolved project manifest file, if detected.
    pub manifest_path: Option<PathBuf>,
}

/// Automatic typechecker command detection and working directory resolution for all supported languages.
pub struct TypecheckerDetector;

impl TypecheckerDetector {
    /// Detects the appropriate typechecker command for a given file and workspace (backward-compatible).
    pub fn detect(
        workspace_root: &Path,
        file_path: &Path,
        language: SupportedLanguage,
    ) -> Option<String> {
        Self::detect_resolution(workspace_root, file_path, language).map(|r| r.command)
    }

    /// Detects the structured typechecker resolution (command, dynamic working directory, manifest path)
    /// by traversing upward from `file_path` towards `workspace_root`.
    pub fn detect_resolution(
        workspace_root: &Path,
        file_path: &Path,
        language: SupportedLanguage,
    ) -> Option<TypecheckerResolution> {
        let abs_file = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            workspace_root.join(file_path)
        };

        match language {
            SupportedLanguage::Rust => {
                // Find closest Cargo.toml upward
                let manifest = find_file_upward(&abs_file, "Cargo.toml").or_else(|| {
                    let root_manifest = workspace_root.join("Cargo.toml");
                    if root_manifest.exists() {
                        Some(root_manifest)
                    } else {
                        None
                    }
                })?;

                let working_dir = manifest
                    .parent()
                    .unwrap_or(workspace_root)
                    .to_path_buf();

                let command = if working_dir != workspace_root {
                    format!(
                        "cargo check --manifest-path \"{}\"",
                        manifest.to_string_lossy()
                    )
                } else {
                    "cargo check".to_string()
                };

                Some(TypecheckerResolution {
                    command,
                    working_dir,
                    manifest_path: Some(manifest),
                })
            }
            SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
                // Priority 1: tsconfig.json upward
                if let Some(manifest) = find_file_upward(&abs_file, "tsconfig.json").or_else(|| {
                    let root_ts = workspace_root.join("tsconfig.json");
                    if root_ts.exists() {
                        Some(root_ts)
                    } else {
                        None
                    }
                }) {
                    let working_dir = manifest
                        .parent()
                        .unwrap_or(workspace_root)
                        .to_path_buf();
                    return Some(TypecheckerResolution {
                        command: "npx tsc --noEmit".to_string(),
                        working_dir,
                        manifest_path: Some(manifest),
                    });
                }

                // Priority 2: package.json upward
                if let Some(manifest) = find_file_upward(&abs_file, "package.json").or_else(|| {
                    let root_pkg = workspace_root.join("package.json");
                    if root_pkg.exists() {
                        Some(root_pkg)
                    } else {
                        None
                    }
                }) {
                    let working_dir = manifest
                        .parent()
                        .unwrap_or(workspace_root)
                        .to_path_buf();
                    return Some(TypecheckerResolution {
                        command: "npx tsc --noEmit".to_string(),
                        working_dir,
                        manifest_path: Some(manifest),
                    });
                }

                None
            }
            SupportedLanguage::Go => {
                // Find closest go.mod upward
                let manifest = find_file_upward(&abs_file, "go.mod").or_else(|| {
                    let root_mod = workspace_root.join("go.mod");
                    if root_mod.exists() {
                        Some(root_mod)
                    } else {
                        None
                    }
                })?;

                let working_dir = manifest
                    .parent()
                    .unwrap_or(workspace_root)
                    .to_path_buf();

                Some(TypecheckerResolution {
                    command: "go vet ./...".to_string(),
                    working_dir,
                    manifest_path: Some(manifest),
                })
            }
            SupportedLanguage::Python => {
                // Find closest mypy.ini, pyproject.toml, or setup.py upward
                let manifest = find_file_upward(&abs_file, "mypy.ini")
                    .or_else(|| find_file_upward(&abs_file, "pyproject.toml"))
                    .or_else(|| find_file_upward(&abs_file, "setup.py"))
                    .or_else(|| {
                        let r = workspace_root.join("mypy.ini");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("pyproject.toml");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("setup.py");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    });

                if let Some(ref m) = manifest {
                    let working_dir = m.parent().unwrap_or(workspace_root).to_path_buf();
                    let rel_file = abs_file.strip_prefix(&working_dir).unwrap_or(&abs_file);
                    let rel_str = rel_file.to_string_lossy();
                    let file_name = m.file_name().and_then(|n| n.to_str()).unwrap_or_default();

                    let command = if file_name == "mypy.ini" || file_name == "pyproject.toml" {
                        format!("mypy \"{rel_str}\"")
                    } else {
                        format!("python -m py_compile \"{rel_str}\"")
                    };

                    Some(TypecheckerResolution {
                        command,
                        working_dir,
                        manifest_path: Some(m.clone()),
                    })
                } else {
                    let rel_file = abs_file.strip_prefix(workspace_root).unwrap_or(&abs_file);
                    let rel_str = rel_file.to_string_lossy();
                    Some(TypecheckerResolution {
                        command: format!("python -m py_compile \"{rel_str}\""),
                        working_dir: workspace_root.to_path_buf(),
                        manifest_path: None,
                    })
                }
            }
            SupportedLanguage::CSharp => {
                // Find closest *.csproj or *.sln upward
                let manifest = find_file_matching_upward(&abs_file, is_csharp_manifest).or_else(
                    || find_matching_file_in_dir(workspace_root, is_csharp_manifest),
                )?;

                let working_dir = manifest
                    .parent()
                    .unwrap_or(workspace_root)
                    .to_path_buf();

                Some(TypecheckerResolution {
                    command: "dotnet build".to_string(),
                    working_dir,
                    manifest_path: Some(manifest),
                })
            }
            SupportedLanguage::Java => {
                // Priority 1: pom.xml
                if let Some(manifest) = find_file_upward(&abs_file, "pom.xml").or_else(|| {
                    let root_pom = workspace_root.join("pom.xml");
                    if root_pom.exists() {
                        Some(root_pom)
                    } else {
                        None
                    }
                }) {
                    let working_dir = manifest
                        .parent()
                        .unwrap_or(workspace_root)
                        .to_path_buf();
                    return Some(TypecheckerResolution {
                        command: "mvn compile -DskipTests".to_string(),
                        working_dir,
                        manifest_path: Some(manifest),
                    });
                }

                // Priority 2: build.gradle / build.gradle.kts
                if let Some(manifest) = find_file_upward(&abs_file, "build.gradle")
                    .or_else(|| find_file_upward(&abs_file, "build.gradle.kts"))
                    .or_else(|| {
                        let r = workspace_root.join("build.gradle");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("build.gradle.kts");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                {
                    let working_dir = manifest
                        .parent()
                        .unwrap_or(workspace_root)
                        .to_path_buf();
                    return Some(TypecheckerResolution {
                        command: "gradle compileJava".to_string(),
                        working_dir,
                        manifest_path: Some(manifest),
                    });
                }

                None
            }
            SupportedLanguage::Kotlin => {
                // Priority 1: build.gradle.kts / build.gradle
                if let Some(manifest) = find_file_upward(&abs_file, "build.gradle.kts")
                    .or_else(|| find_file_upward(&abs_file, "build.gradle"))
                    .or_else(|| {
                        let r = workspace_root.join("build.gradle.kts");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("build.gradle");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                {
                    let working_dir = manifest
                        .parent()
                        .unwrap_or(workspace_root)
                        .to_path_buf();
                    return Some(TypecheckerResolution {
                        command: "gradle compileKotlin".to_string(),
                        working_dir,
                        manifest_path: Some(manifest),
                    });
                }

                // Priority 2: pom.xml
                if let Some(manifest) = find_file_upward(&abs_file, "pom.xml").or_else(|| {
                    let root_pom = workspace_root.join("pom.xml");
                    if root_pom.exists() {
                        Some(root_pom)
                    } else {
                        None
                    }
                }) {
                    let working_dir = manifest
                        .parent()
                        .unwrap_or(workspace_root)
                        .to_path_buf();
                    return Some(TypecheckerResolution {
                        command: "mvn compile -DskipTests".to_string(),
                        working_dir,
                        manifest_path: Some(manifest),
                    });
                }

                None
            }
            SupportedLanguage::C => {
                let manifest = find_file_upward(&abs_file, "CMakeLists.txt")
                    .or_else(|| find_file_upward(&abs_file, "Makefile"))
                    .or_else(|| {
                        let r = workspace_root.join("CMakeLists.txt");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("Makefile");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    });

                if let Some(m) = manifest {
                    let working_dir = m.parent().unwrap_or(workspace_root).to_path_buf();
                    let rel_file = abs_file.strip_prefix(&working_dir).unwrap_or(&abs_file);
                    let rel_str = rel_file.to_string_lossy();
                    Some(TypecheckerResolution {
                        command: format!("clang -fsyntax-only \"{rel_str}\""),
                        working_dir,
                        manifest_path: Some(m),
                    })
                } else {
                    None
                }
            }
            SupportedLanguage::Cpp => {
                let manifest = find_file_upward(&abs_file, "CMakeLists.txt")
                    .or_else(|| find_file_upward(&abs_file, "Makefile"))
                    .or_else(|| {
                        let r = workspace_root.join("CMakeLists.txt");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("Makefile");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    });

                if let Some(m) = manifest {
                    let working_dir = m.parent().unwrap_or(workspace_root).to_path_buf();
                    let rel_file = abs_file.strip_prefix(&working_dir).unwrap_or(&abs_file);
                    let rel_str = rel_file.to_string_lossy();
                    Some(TypecheckerResolution {
                        command: format!("clang++ -fsyntax-only \"{rel_str}\""),
                        working_dir,
                        manifest_path: Some(m),
                    })
                } else {
                    None
                }
            }
            SupportedLanguage::Vue => {
                let manifest = find_file_upward(&abs_file, "package.json")
                    .or_else(|| find_file_upward(&abs_file, "tsconfig.json"))
                    .or_else(|| {
                        let r = workspace_root.join("package.json");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("tsconfig.json");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    });

                manifest.map(|m| {
                    let working_dir = m.parent().unwrap_or(workspace_root).to_path_buf();
                    TypecheckerResolution {
                        command: "npx vue-tsc --noEmit".to_string(),
                        working_dir,
                        manifest_path: Some(m),
                    }
                })
            }
            SupportedLanguage::Svelte => {
                let manifest = find_file_upward(&abs_file, "package.json")
                    .or_else(|| find_file_upward(&abs_file, "tsconfig.json"))
                    .or_else(|| {
                        let r = workspace_root.join("package.json");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("tsconfig.json");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    });

                manifest.map(|m| {
                    let working_dir = m.parent().unwrap_or(workspace_root).to_path_buf();
                    TypecheckerResolution {
                        command: "npx svelte-check".to_string(),
                        working_dir,
                        manifest_path: Some(m),
                    }
                })
            }
            SupportedLanguage::Astro => {
                let manifest = find_file_upward(&abs_file, "package.json")
                    .or_else(|| find_file_upward(&abs_file, "tsconfig.json"))
                    .or_else(|| {
                        let r = workspace_root.join("package.json");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        let r = workspace_root.join("tsconfig.json");
                        if r.exists() {
                            Some(r)
                        } else {
                            None
                        }
                    });

                manifest.map(|m| {
                    let working_dir = m.parent().unwrap_or(workspace_root).to_path_buf();
                    TypecheckerResolution {
                        command: "npx astro check".to_string(),
                        working_dir,
                        manifest_path: Some(m),
                    }
                })
            }
        }
    }
}

/// Traverses upward from `start` to locate the nearest file named `file_name`.
pub fn find_file_upward(start: &Path, file_name: &str) -> Option<PathBuf> {
    let start_dir = if start.is_dir() {
        start.to_path_buf()
    } else if let Some(parent) = start.parent() {
        parent.to_path_buf()
    } else {
        start.to_path_buf()
    };

    let mut current = start_dir;
    loop {
        let candidate = current.join(file_name);
        if candidate.exists() {
            return Some(candidate);
        }
        match current.parent() {
            Some(p) if p != current => current = p.to_path_buf(),
            _ => break,
        }
    }
    None
}

/// Traverses upward from `start` to locate the nearest file satisfying `predicate`.
pub fn find_file_matching_upward<F>(start: &Path, predicate: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let start_dir = if start.is_dir() {
        start.to_path_buf()
    } else if let Some(parent) = start.parent() {
        parent.to_path_buf()
    } else {
        start.to_path_buf()
    };

    let mut current = start_dir;
    loop {
        if let Ok(entries) = fs::read_dir(&current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && predicate(&path) {
                    return Some(path);
                }
            }
        }
        match current.parent() {
            Some(p) if p != current => current = p.to_path_buf(),
            _ => break,
        }
    }
    None
}

/// Checks direct children of `dir` for any file matching `predicate`.
pub fn find_matching_file_in_dir<F>(dir: &Path, predicate: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && predicate(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn is_csharp_manifest(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        ext.eq_ignore_ascii_case("csproj") || ext.eq_ignore_ascii_case("sln")
    } else {
        false
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
                file: if file_part.is_empty() {
                    None
                } else {
                    Some(file_part.to_string())
                },
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
        let code_len = code_part
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .count();
        let code = code_part[..code_len].to_string();
        let msg = rest
            .replace(&code, "")
            .trim_start_matches(':')
            .trim()
            .to_string();
        return (severity, Some(code), msg);
    }

    (severity, None, rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_rust_nested_cargo_toml_tauri() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let tauri_dir = root.join("src-tauri");
        fs::create_dir_all(tauri_dir.join("src")).unwrap();
        let cargo_toml = tauri_dir.join("Cargo.toml");
        fs::write(&cargo_toml, "[package]\nname = \"tauri-app\"\n").unwrap();
        let target_file = tauri_dir.join("src").join("main.rs");
        fs::write(&target_file, "fn main() {}\n").unwrap();

        let res = TypecheckerDetector::detect_resolution(root, &target_file, SupportedLanguage::Rust)
            .expect("Should resolve nested Tauri Cargo.toml");

        assert_eq!(res.working_dir, tauri_dir);
        assert_eq!(res.manifest_path, Some(cargo_toml.clone()));
        assert!(res.command.contains("cargo check"));
        assert!(res.command.contains("Cargo.toml"));
    }

    #[test]
    fn test_rust_root_cargo_toml() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let cargo_toml = root.join("Cargo.toml");
        fs::write(&cargo_toml, "[package]\nname = \"root-app\"\n").unwrap();
        let target_file = root.join("src").join("lib.rs");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(&target_file, "pub fn run() {}\n").unwrap();

        let res = TypecheckerDetector::detect_resolution(root, &target_file, SupportedLanguage::Rust)
            .expect("Should resolve root Cargo.toml");

        assert_eq!(res.working_dir, root);
        assert_eq!(res.manifest_path, Some(cargo_toml));
        assert_eq!(res.command, "cargo check");
    }

    #[test]
    fn test_ts_monorepo_packages_tsconfig() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let pkg_dir = root.join("packages").join("web");
        fs::create_dir_all(pkg_dir.join("src")).unwrap();
        let tsconfig = pkg_dir.join("tsconfig.json");
        fs::write(&tsconfig, "{}").unwrap();
        let target_file = pkg_dir.join("src").join("index.ts");
        fs::write(&target_file, "export const x = 1;").unwrap();

        let res = TypecheckerDetector::detect_resolution(root, &target_file, SupportedLanguage::TypeScript)
            .expect("Should resolve package tsconfig.json");

        assert_eq!(res.working_dir, pkg_dir);
        assert_eq!(res.manifest_path, Some(tsconfig));
        assert_eq!(res.command, "npx tsc --noEmit");
    }

    #[test]
    fn test_go_nested_submodule_go_mod() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let backend_dir = root.join("backend");
        fs::create_dir_all(backend_dir.join("api")).unwrap();
        let go_mod = backend_dir.join("go.mod");
        fs::write(&go_mod, "module backend\n\ngo 1.21\n").unwrap();
        let target_file = backend_dir.join("api").join("server.go");
        fs::write(&target_file, "package api\n").unwrap();

        let res = TypecheckerDetector::detect_resolution(root, &target_file, SupportedLanguage::Go)
            .expect("Should resolve nested go.mod");

        assert_eq!(res.working_dir, backend_dir);
        assert_eq!(res.manifest_path, Some(go_mod));
        assert_eq!(res.command, "go vet ./...");
    }

    #[test]
    fn test_python_nested_pyproject_toml() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let service_dir = root.join("services").join("worker");
        fs::create_dir_all(service_dir.join("src")).unwrap();
        let pyproject = service_dir.join("pyproject.toml");
        fs::write(&pyproject, "[tool.mypy]\nstrict = true\n").unwrap();
        let target_file = service_dir.join("src").join("task.py");
        fs::write(&target_file, "def work(): pass\n").unwrap();

        let res = TypecheckerDetector::detect_resolution(root, &target_file, SupportedLanguage::Python)
            .expect("Should resolve nested pyproject.toml");

        assert_eq!(res.working_dir, service_dir);
        assert_eq!(res.manifest_path, Some(pyproject));
        assert!(res.command.starts_with("mypy"));
    }

    #[test]
    fn test_csharp_nested_csproj() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let api_dir = root.join("src").join("Api");
        fs::create_dir_all(api_dir.join("Controllers")).unwrap();
        let csproj = api_dir.join("Api.csproj");
        fs::write(&csproj, "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>").unwrap();
        let target_file = api_dir.join("Controllers").join("UserController.cs");
        fs::write(&target_file, "public class UserController {}").unwrap();

        let res = TypecheckerDetector::detect_resolution(root, &target_file, SupportedLanguage::CSharp)
            .expect("Should resolve nested .csproj");

        assert_eq!(res.working_dir, api_dir);
        assert_eq!(res.manifest_path, Some(csproj));
        assert_eq!(res.command, "dotnet build");
    }

    #[test]
    fn test_java_nested_pom_xml() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let core_dir = root.join("modules").join("core");
        fs::create_dir_all(core_dir.join("src").join("main").join("java")).unwrap();
        let pom = core_dir.join("pom.xml");
        fs::write(&pom, "<project></project>").unwrap();
        let target_file = core_dir.join("src").join("main").join("java").join("App.java");
        fs::write(&target_file, "public class App {}").unwrap();

        let res = TypecheckerDetector::detect_resolution(root, &target_file, SupportedLanguage::Java)
            .expect("Should resolve nested pom.xml");

        assert_eq!(res.working_dir, core_dir);
        assert_eq!(res.manifest_path, Some(pom));
        assert_eq!(res.command, "mvn compile -DskipTests");
    }

    #[test]
    fn test_kotlin_nested_build_gradle_kts() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let app_dir = root.join("app");
        fs::create_dir_all(app_dir.join("src")).unwrap();
        let gradle = app_dir.join("build.gradle.kts");
        fs::write(&gradle, "plugins { kotlin(\"jvm\") }").unwrap();
        let target_file = app_dir.join("src").join("Main.kt");
        fs::write(&target_file, "fun main() {}").unwrap();

        let res = TypecheckerDetector::detect_resolution(root, &target_file, SupportedLanguage::Kotlin)
            .expect("Should resolve nested build.gradle.kts");

        assert_eq!(res.working_dir, app_dir);
        assert_eq!(res.manifest_path, Some(gradle));
        assert_eq!(res.command, "gradle compileKotlin");
    }
}

