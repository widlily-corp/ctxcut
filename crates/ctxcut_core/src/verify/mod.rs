//! AST patch verification guard with typechecking and RAII auto-rollback.

pub mod rollback;
pub mod typechecker;

pub use rollback::RollbackGuard;
pub use typechecker::{
    DiagnosticParser, TypecheckExecutionResult, TypecheckerDetector, TypecheckerRunner,
};

use crate::error::{CoreError, Result};
use crate::model::{SupportedLanguage, VerifyDiagnostic, VerifyPatchResult};
use crate::patch::AstPatcher;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Verification Guard orchestrator.
pub struct PatchVerifier;

/// Configuration options for `PatchVerifier`.
#[derive(Debug, Clone)]
pub struct VerifyPatchOptions<'a> {
    /// Workspace root directory.
    pub workspace_root: &'a Path,
    /// Target file path.
    pub file_path: &'a Path,
    /// Target symbol name.
    pub symbol: &'a str,
    /// Replacement code string.
    pub replacement_code: &'a str,
    /// Optional typechecker command override.
    pub typechecker: Option<&'a str>,
    /// Whether dry-run mode is enabled (default: true).
    pub dry_run: bool,
    /// Execution timeout in milliseconds (default: 30,000ms).
    pub timeout_ms: Option<u64>,
}

impl PatchVerifier {
    /// Verifies a patch with AST syntax validation, typechecking, and RAII auto-rollback.
    pub fn verify_patch(
        workspace_root: &Path,
        target: &str,
        new_code: &str,
        typechecker: Option<&str>,
        dry_run: bool,
    ) -> Result<VerifyPatchResult> {
        let (file_part, symbol_part) =
            parse_target_str(target).ok_or_else(|| CoreError::SymbolNotFound {
                symbol: target.to_string(),
                path: PathBuf::from(target),
                available_symbols: Vec::new(),
            })?;

        let file_path = if Path::new(file_part).is_absolute() {
            PathBuf::from(file_part)
        } else {
            workspace_root.join(file_part)
        };

        let opts = VerifyPatchOptions {
            workspace_root,
            file_path: &file_path,
            symbol: symbol_part,
            replacement_code: new_code,
            typechecker,
            dry_run,
            timeout_ms: Some(30_000),
        };

        Self::verify_patch_with_options(&opts)
    }

    /// Executes verification using explicit options.
    pub fn verify_patch_with_options(opts: &VerifyPatchOptions) -> Result<VerifyPatchResult> {
        let start_time = Instant::now();
        let file_path = opts.file_path;

        if !file_path.exists() {
            return Err(CoreError::Io {
                path: file_path.to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"),
            });
        }

        let original_source = fs::read_to_string(file_path).map_err(|e| CoreError::Io {
            path: file_path.to_path_buf(),
            source: e,
        })?;

        let lang = SupportedLanguage::from_path(file_path).unwrap_or(SupportedLanguage::TypeScript);

        // 1. AST Syntax validation and diff computation in memory
        let patch_res = match AstPatcher::patch_source(
            &original_source,
            lang,
            file_path,
            opts.symbol,
            opts.replacement_code,
        ) {
            Ok(res) => res,
            Err(CoreError::SyntaxValidationError { errors, .. }) => {
                let diagnostics: Vec<VerifyDiagnostic> = errors
                    .iter()
                    .map(|e| VerifyDiagnostic {
                        severity: "error".to_string(),
                        line: Some(e.line),
                        column: Some(e.column),
                        message: format!("Syntax error: {}", e.snippet),
                        file: Some(file_path.to_string_lossy().to_string()),
                        code: Some(e.kind.clone()),
                    })
                    .collect();

                return Ok(VerifyPatchResult {
                    file_path: file_path.to_path_buf(),
                    symbol_name: opts.symbol.to_string(),
                    success: false,
                    applied: false,
                    dry_run: opts.dry_run,
                    diff: String::new(),
                    typechecker_command: None,
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: "AST Syntax Validation Failed".to_string(),
                    diagnostics,
                    syntax_errors: errors,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                });
            }
            Err(e) => return Err(e),
        };

        // 2. Instantiate RAII RollbackGuard
        let mut guard = RollbackGuard::new(file_path, original_source);

        // 3. Temporarily apply patch to disk
        fs::write(file_path, &patch_res.patched_code).map_err(|e| CoreError::Io {
            path: file_path.to_path_buf(),
            source: e,
        })?;

        // 4. Determine & run typechecker
        let typecheck_cmd = opts
            .typechecker
            .map(String::from)
            .or_else(|| TypecheckerDetector::detect(opts.workspace_root, file_path, lang));

        let mut typecheck_success = true;
        let mut exit_code = Some(0);
        let mut stdout_str = String::new();
        let mut stderr_str = String::new();
        let mut diagnostics = Vec::new();

        if let Some(ref cmd_str) = typecheck_cmd {
            let timeout_dur = Duration::from_millis(opts.timeout_ms.unwrap_or(30_000));
            let run_res = TypecheckerRunner::run(cmd_str, opts.workspace_root, timeout_dur);

            exit_code = run_res.exit_code;
            stdout_str = run_res.stdout;
            stderr_str = run_res.stderr;
            typecheck_success = run_res.success;

            let combined_output = format!("{stdout_str}\n{stderr_str}");
            diagnostics = DiagnosticParser::parse(&combined_output);
        }

        let overall_success = typecheck_success;
        let applied = overall_success && !opts.dry_run;

        // 5. Safe Commit or Rollback
        if applied {
            guard.disarm(); // Persist changes
        } else {
            let _ = guard.rollback(); // Revert back to original content
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(VerifyPatchResult {
            file_path: file_path.to_path_buf(),
            symbol_name: opts.symbol.to_string(),
            success: overall_success,
            applied,
            dry_run: opts.dry_run,
            diff: patch_res.diff,
            typechecker_command: typecheck_cmd,
            exit_code,
            stdout: stdout_str,
            stderr: stderr_str,
            diagnostics,
            syntax_errors: Vec::new(),
            duration_ms,
        })
    }
}

fn parse_target_str(target: &str) -> Option<(&str, &str)> {
    let search_start = if target.len() >= 2
        && target.as_bytes()[1] == b':'
        && target.as_bytes()[0].is_ascii_alphabetic()
    {
        2
    } else {
        0
    };
    let colon_idx = target[search_start..].find(':')? + search_start;
    Some((&target[..colon_idx], &target[colon_idx + 1..]))
}
