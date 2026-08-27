//! Multi-symbol, multi-file transactional AST mutation engine and reverse byte offset patcher.
//!
//! Provides atomic batch refactoring with Tree-sitter syntax validation pre-checks,
//! reverse byte offset splicing per file, RAII multi-file rollback guards,
//! isolated compiler dry-runs, and AST diagnostic mapping.

use crate::error::{CoreError, Result};
use crate::lang::LanguageRegistry;
use crate::model::{SupportedLanguage, SyntaxErrorDetail, VerifyDiagnostic};
use crate::parser::ParserManager;
use crate::patch::formatting::{
    generate_unified_diff, reindent_for_splice, LineEnding,
};
use crate::patch::locator::AstNodeLocator;
use crate::patch::validate::SyntaxValidator;
use crate::verify::ast_mapper::{
    AstDiagnosticMapper, MappedPatchDiagnostic, PatchedFileInfo, PatchedSymbolMeta,
};
use crate::verify::multi_rollback::MultiFileRollbackGuard;
use crate::verify::typechecker::{DiagnosticParser, TypecheckerDetector, TypecheckerRunner};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A single symbol patch instruction targeting a symbol within a specific file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolPatchUnit {
    /// Path to the target source file (absolute or relative to workspace root).
    pub file_path: PathBuf,
    /// Target symbol query (e.g., `"calculate_tax"`, `"AuthService.login"`, `"Calculator"`).
    #[serde(alias = "symbol", alias = "symbol_name")]
    pub symbol_query: String,
    /// Replacement source code for the target symbol.
    #[serde(alias = "code")]
    pub replacement_code: String,
}

/// Request for executing an atomic multi-symbol, multi-file patch transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchTransactionRequest {
    /// Optional workspace root directory.
    pub workspace_root: Option<PathBuf>,
    /// List of symbol patch instructions to apply in the transaction.
    pub patches: Vec<SymbolPatchUnit>,
    /// Optional explicit typechecker command override (e.g. `"cargo check"`, `"npx tsc --noEmit"`).
    pub typechecker: Option<String>,
    /// Whether to commit changes to disk on success (`true`) or dry-run preview (`false`).
    pub apply: bool,
    /// Typechecker execution timeout in milliseconds (default: 30,000ms).
    pub timeout_ms: Option<u64>,
}

/// Unified diff and patched symbol summary for a single modified file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePatchDiff {
    /// Target file path.
    pub file_path: String,
    /// Unified diff representation of all spliced changes in this file.
    pub diff: String,
    /// List of symbol names that were successfully patched in this file.
    pub symbols_patched: Vec<String>,
}

/// Result of executing a multi-symbol, multi-file patch transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchTransactionResult {
    /// Whether the transaction succeeded without syntax or typecheck errors.
    pub success: bool,
    /// Whether modifications were committed and persisted to disk.
    pub applied: bool,
    /// Whether modifications were rolled back to the original disk state.
    pub rolled_back: bool,
    /// Total number of unique files modified in the transaction.
    pub files_modified_count: usize,
    /// Total number of symbols patched across all files.
    pub symbols_patched_count: usize,
    /// Per-file diffs and patched symbol details.
    pub diffs: Vec<FilePatchDiff>,
    /// Typechecker command that was executed, if any.
    pub typechecker_command: Option<String>,
    /// Exit code from the typechecker process.
    pub exit_code: Option<i32>,
    /// Diagnostics mapped to target AST nodes and patch-relative lines.
    pub diagnostics: Vec<MappedPatchDiagnostic>,
    /// Syntax error details if Tree-sitter validation failed.
    pub syntax_errors: Vec<SyntaxErrorDetail>,
    /// Total execution duration in milliseconds.
    pub duration_ms: u64,
}

impl PatchTransactionResult {
    /// Formats the patch transaction result as a high-density Markdown report.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let status_header = if self.applied {
            "✔ Batch Patch Applied Successfully"
        } else if self.success {
            "ℹ Batch Patch Dry-Run (Preview Only)"
        } else if self.rolled_back {
            "✖ Batch Patch Failed — Rolled Back to Original State"
        } else {
            "✖ Batch Patch Failed — Pre-Write Validation Rejection"
        };

        out.push_str(&format!("# {status_header}\n\n"));
        out.push_str(&format!("- **Overall Success:** `{}`\n", self.success));
        out.push_str(&format!("- **Applied to Disk:** `{}`\n", self.applied));
        out.push_str(&format!("- **Rolled Back:** `{}`\n", self.rolled_back));
        out.push_str(&format!(
            "- **Files Modified:** `{}`\n",
            self.files_modified_count
        ));
        out.push_str(&format!(
            "- **Symbols Patched:** `{}`\n",
            self.symbols_patched_count
        ));
        out.push_str(&format!("- **Duration:** `{}ms`\n", self.duration_ms));

        if let Some(ref cmd) = self.typechecker_command {
            out.push_str(&format!("- **Typechecker:** `{cmd}`\n"));
            if let Some(code) = self.exit_code {
                out.push_str(&format!("- **Exit Code:** `{code}`\n"));
            }
        }
        out.push('\n');

        if !self.syntax_errors.is_empty() {
            out.push_str("## Syntax Validation Errors\n\n");
            for err in &self.syntax_errors {
                out.push_str(&format!(
                    "- [Line {}, Col {}] **{}**: `{}`\n",
                    err.line, err.column, err.kind, err.snippet
                ));
            }
            out.push('\n');
        }

        if !self.diagnostics.is_empty() {
            out.push_str("## Compiler & Linter Diagnostics\n\n");
            for diag in &self.diagnostics {
                let sym_tag = diag
                    .symbol_name
                    .as_deref()
                    .map(|s| format!(" in `{s}`"))
                    .unwrap_or_default();
                let rel_tag = diag
                    .patch_relative_line
                    .map(|r| format!(" (patch-relative line {r})"))
                    .unwrap_or_default();
                let loc_tag = match (diag.line, diag.column) {
                    (Some(l), Some(c)) => format!(" [line {l}, col {c}]"),
                    (Some(l), None) => format!(" [line {l}]"),
                    _ => String::new(),
                };
                let code_tag = diag
                    .code
                    .as_deref()
                    .map(|c| format!(" ({c})"))
                    .unwrap_or_default();

                out.push_str(&format!(
                    "- **{}** `{}`{}{}{}{}: {}\n",
                    diag.severity.to_uppercase(),
                    diag.file_path,
                    sym_tag,
                    loc_tag,
                    rel_tag,
                    code_tag,
                    diag.message
                ));

                if let Some(ref snip) = diag.code_snippet {
                    out.push_str(&format!("  ```\n  {}\n  ```\n", snip.trim()));
                }
            }
            out.push('\n');
        }

        if !self.diffs.is_empty() {
            out.push_str("## Unified Diffs\n\n");
            for diff in &self.diffs {
                out.push_str(&format!(
                    "### `{}` (Patched symbols: `{}`)\n\n```diff\n{}\n```\n\n",
                    diff.file_path,
                    diff.symbols_patched.join("`, `"),
                    diff.diff.trim()
                ));
            }
        }

        out
    }

    /// Formats the patch transaction result as pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the patch transaction result as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Trait defining transactional multi-symbol refactoring capabilities.
pub trait TransactionalPatcher: Send + Sync {
    /// Executes an atomic multi-symbol patch transaction.
    fn patch_transaction(&self, req: &PatchTransactionRequest) -> Result<PatchTransactionResult>;
}

/// Core engine for executing multi-symbol, multi-file batch AST refactoring transactions.
pub struct BatchAstPatcher;

impl TransactionalPatcher for BatchAstPatcher {
    fn patch_transaction(&self, req: &PatchTransactionRequest) -> Result<PatchTransactionResult> {
        Self::apply_transaction(req)
    }
}

impl BatchAstPatcher {
    /// Executes a batch patch transaction according to the provided `PatchTransactionRequest`.
    pub fn apply_transaction(req: &PatchTransactionRequest) -> Result<PatchTransactionResult> {
        let start_time = Instant::now();

        if req.patches.is_empty() {
            return Ok(PatchTransactionResult {
                success: true,
                applied: false,
                rolled_back: false,
                files_modified_count: 0,
                symbols_patched_count: 0,
                diffs: Vec::new(),
                typechecker_command: None,
                exit_code: Some(0),
                diagnostics: Vec::new(),
                syntax_errors: Vec::new(),
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        // 1. Group patch units by canonical/normalized file path
        let mut file_groups: HashMap<PathBuf, Vec<&SymbolPatchUnit>> = HashMap::new();
        for patch in &req.patches {
            let abs_path = if patch.file_path.is_absolute() {
                patch.file_path.clone()
            } else if let Some(ref root) = req.workspace_root {
                root.join(&patch.file_path)
            } else {
                patch.file_path.clone()
            };
            file_groups.entry(abs_path).or_default().push(patch);
        }

        let mut prepared_files: Vec<PreparedFilePatch> = Vec::new();
        let mut all_syntax_errors: Vec<SyntaxErrorDetail> = Vec::new();
        let mut first_lang: Option<SupportedLanguage> = None;

        // 2. Perform in-memory reverse byte offset splicing and syntax validation for each file
        for (file_path, patches) in &file_groups {
            if !file_path.exists() {
                return Err(CoreError::Io {
                    path: file_path.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Target file not found",
                    ),
                });
            }

            let original_source = fs::read_to_string(file_path).map_err(|e| CoreError::Io {
                path: file_path.clone(),
                source: e,
            })?;

            let adapter = LanguageRegistry::for_path(file_path)?;
            let lang = adapter.language();
            if first_lang.is_none() {
                first_lang = Some(lang);
            }

            let ts_lang = adapter.tree_sitter_language(file_path);
            let tree = ParserManager::parse_source(&original_source, &ts_lang, file_path)?;

            // Locate each target symbol in AST
            let mut located_targets: Vec<LocatedPatchTarget> = Vec::new();
            for patch in patches {
                let located = AstNodeLocator::locate(
                    tree.root_node(),
                    &original_source,
                    &patch.symbol_query,
                    &*adapter,
                    file_path,
                )?;

                let (start, end) = located.byte_range;
                if start > end || end > original_source.len() {
                    return Err(CoreError::PatchRangeError {
                        path: file_path.clone(),
                        start,
                        end,
                        total_bytes: original_source.len(),
                    });
                }

                located_targets.push(LocatedPatchTarget {
                    symbol_name: located.symbol_name,
                    node_kind: located.kind,
                    byte_range: (start, end),
                    base_indentation: located.base_indentation,
                    replacement_code: patch.replacement_code.clone(),
                    aligned_replacement: String::new(),
                });
            }

            // Check for overlapping byte ranges in the same file
            for i in 0..located_targets.len() {
                for j in (i + 1)..located_targets.len() {
                    let r1 = located_targets[i].byte_range;
                    let r2 = located_targets[j].byte_range;
                    if r1.0 < r2.1 && r2.0 < r1.1 {
                        return Err(CoreError::PatchRangeError {
                            path: file_path.clone(),
                            start: r1.0.min(r2.0),
                            end: r1.1.max(r2.1),
                            total_bytes: original_source.len(),
                        });
                    }
                }
            }

            // Re-indent replacement code for each target
            let line_ending = LineEnding::detect(&original_source);
            for target in &mut located_targets {
                target.aligned_replacement = reindent_for_splice(
                    &target.replacement_code,
                    &target.base_indentation,
                    line_ending,
                );
            }

            // Sort targets in ascending order of original start byte to track post-splice ranges
            located_targets.sort_by_key(|t| t.byte_range.0);

            // Reverse byte offset splicing: splice from highest start byte down to lowest
            let mut patched_source = original_source.clone();
            let mut reverse_targets = located_targets.clone();
            reverse_targets.sort_by_key(|b| std::cmp::Reverse(b.byte_range.0));

            for target in &reverse_targets {
                let (start, end) = target.byte_range;
                patched_source.replace_range(start..end, &target.aligned_replacement);
            }

            // Verify Tree-sitter syntax on spliced file source
            if let Err(CoreError::SyntaxValidationError { errors, .. }) =
                SyntaxValidator::validate_source(&patched_source, &ts_lang, file_path)
            {
                all_syntax_errors.extend(errors);
            }

            // Compute post-splice symbol start/end lines for diagnostic mapping
            let mut symbol_metas: Vec<PatchedSymbolMeta> = Vec::new();
            let mut current_offset_shift: i64 = 0;
            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            for target in &located_targets {
                let orig_len = (target.byte_range.1 - target.byte_range.0) as i64;
                let new_len = target.aligned_replacement.len() as i64;
                let post_start_byte =
                    ((target.byte_range.0 as i64) + current_offset_shift).max(0) as usize;
                let post_end_byte = post_start_byte + target.aligned_replacement.len();

                let prefix_to_start = if post_start_byte <= patched_source.len() {
                    &patched_source[..post_start_byte]
                } else {
                    &patched_source
                };
                let last_byte_idx = post_end_byte.saturating_sub(1).min(patched_source.len());
                let prefix_to_last = &patched_source[..last_byte_idx];

                let start_line = prefix_to_start.bytes().filter(|&b| b == b'\n').count() + 1;
                let end_line = prefix_to_last.bytes().filter(|&b| b == b'\n').count() + 1;

                symbol_metas.push(PatchedSymbolMeta {
                    symbol_name: target.symbol_name.clone(),
                    node_kind: target.node_kind.clone(),
                    start_line,
                    end_line: end_line.max(start_line),
                    replacement_code: target.replacement_code.clone(),
                });

                current_offset_shift += new_len - orig_len;
            }

            let diff = generate_unified_diff(&original_source, &patched_source, file_path, 3);
            let symbols_patched = located_targets.iter().map(|t| t.symbol_name.clone()).collect();

            prepared_files.push(PreparedFilePatch {
                file_path: file_path.clone(),
                original_source,
                patched_source,
                diff,
                symbols_patched,
                symbols_meta: symbol_metas,
            });
        }

        // If any syntax validation errors occurred, abort before disk mutations
        if !all_syntax_errors.is_empty() {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            return Ok(PatchTransactionResult {
                success: false,
                applied: false,
                rolled_back: false,
                files_modified_count: 0,
                symbols_patched_count: 0,
                diffs: Vec::new(),
                typechecker_command: None,
                exit_code: Some(1),
                diagnostics: Vec::new(),
                syntax_errors: all_syntax_errors,
                duration_ms,
            });
        }

        // 3. Atomically write all modified files to disk under MultiFileRollbackGuard
        let mut guard = MultiFileRollbackGuard::new();
        for prep in &prepared_files {
            guard.record_file_with_content(&prep.file_path, Some(prep.original_source.clone()));
            guard
                .write_file(&prep.file_path, &prep.patched_source)
                .map_err(|e| CoreError::Io {
                    path: prep.file_path.clone(),
                    source: e,
                })?;
        }

        // 4. Typecheck execution & compiler dry-run
        let workspace_root = req
            .workspace_root
            .as_deref()
            .unwrap_or_else(|| Path::new("."));
        let first_file = prepared_files
            .first()
            .map(|f| f.file_path.as_path())
            .unwrap_or(workspace_root);
        let detected_lang = first_lang.unwrap_or(SupportedLanguage::TypeScript);

        let resolution =
            TypecheckerDetector::detect_resolution(workspace_root, first_file, detected_lang);

        let (typecheck_cmd, typecheck_cwd) = if let Some(cmd_override) = req.typechecker.clone() {
            let cwd = resolution
                .as_ref()
                .map(|r| r.working_dir.clone())
                .unwrap_or_else(|| workspace_root.to_path_buf());
            (Some(cmd_override), cwd)
        } else if let Some(res) = resolution {
            (Some(res.command), res.working_dir)
        } else {
            (None, workspace_root.to_path_buf())
        };

        let mut typecheck_success = true;
        let mut exit_code = Some(0);
        let mut raw_diagnostics: Vec<VerifyDiagnostic> = Vec::new();

        if let Some(ref cmd) = typecheck_cmd {
            let timeout = Duration::from_millis(req.timeout_ms.unwrap_or(30_000));
            let run_res = TypecheckerRunner::run(cmd, &typecheck_cwd, timeout);

            typecheck_success = run_res.success;
            exit_code = run_res.exit_code;

            let combined_output = format!("{}\n{}", run_res.stdout, run_res.stderr);
            raw_diagnostics = DiagnosticParser::parse(&combined_output);
        }

        // 5. Map diagnostics back to AST symbols and replacement line offsets
        let patched_file_infos: Vec<PatchedFileInfo> = prepared_files
            .iter()
            .map(|pf| PatchedFileInfo {
                file_path: pf.file_path.clone(),
                patched_source: pf.patched_source.clone(),
                symbols: pf.symbols_meta.clone(),
            })
            .collect();

        let mapped_diagnostics = AstDiagnosticMapper::map_diagnostics(
            &raw_diagnostics,
            &patched_file_infos,
            req.workspace_root.as_deref(),
        );

        let overall_success = typecheck_success;
        let should_apply = overall_success && req.apply;

        // 6. Transaction Commit or Atomic Rollback
        let mut rolled_back = false;
        if should_apply {
            guard.commit();
        } else {
            guard.rollback().map_err(|e| CoreError::Io {
                path: workspace_root.to_path_buf(),
                source: e,
            })?;
            rolled_back = true;
        }

        let total_symbols: usize = prepared_files.iter().map(|f| f.symbols_patched.len()).sum();
        let diffs: Vec<FilePatchDiff> = prepared_files
            .into_iter()
            .map(|pf| FilePatchDiff {
                file_path: pf.file_path.to_string_lossy().to_string(),
                diff: pf.diff,
                symbols_patched: pf.symbols_patched,
            })
            .collect();

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(PatchTransactionResult {
            success: overall_success,
            applied: should_apply,
            rolled_back,
            files_modified_count: diffs.len(),
            symbols_patched_count: total_symbols,
            diffs,
            typechecker_command: typecheck_cmd,
            exit_code,
            diagnostics: mapped_diagnostics,
            syntax_errors: Vec::new(),
            duration_ms,
        })
    }
}

#[derive(Debug, Clone)]
struct LocatedPatchTarget {
    symbol_name: String,
    node_kind: String,
    byte_range: (usize, usize),
    base_indentation: String,
    replacement_code: String,
    aligned_replacement: String,
}

#[derive(Debug)]
struct PreparedFilePatch {
    file_path: PathBuf,
    original_source: String,
    patched_source: String,
    diff: String,
    symbols_patched: Vec<String>,
    symbols_meta: Vec<PatchedSymbolMeta>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_batch_patch_multiple_symbols_single_file_rust() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("math.rs");

        let original = "\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn mul(a: i32, b: i32) -> i32 {\n    a * b\n}\n";
        fs::write(&file_path, original).unwrap();

        let req = PatchTransactionRequest {
            workspace_root: Some(temp_dir.path().to_path_buf()),
            patches: vec![
                SymbolPatchUnit {
                    file_path: file_path.clone(),
                    symbol_query: "add".to_string(),
                    replacement_code: "pub fn add(a: i32, b: i32) -> i32 {\n    (a + b).max(0)\n}".to_string(),
                },
                SymbolPatchUnit {
                    file_path: file_path.clone(),
                    symbol_query: "mul".to_string(),
                    replacement_code: "pub fn mul(a: i32, b: i32) -> i32 {\n    (a * b).max(0)\n}".to_string(),
                },
            ],
            typechecker: None,
            apply: true,
            timeout_ms: Some(5000),
        };

        let res = BatchAstPatcher::apply_transaction(&req).unwrap();
        assert!(res.success);
        assert!(res.applied);
        assert!(!res.rolled_back);
        assert_eq!(res.files_modified_count, 1);
        assert_eq!(res.symbols_patched_count, 2);

        let modified = fs::read_to_string(&file_path).unwrap();
        assert!(modified.contains("(a + b).max(0)"));
        assert!(modified.contains("(a * b).max(0)"));
    }

    #[test]
    fn test_batch_patch_multiple_files_dry_run_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("service.ts");
        let file2 = temp_dir.path().join("controller.ts");

        let orig1 = "export function fetchUsers() {\n    return [];\n}\n";
        let orig2 = "export function handleRequest() {\n    return 'ok';\n}\n";

        fs::write(&file1, orig1).unwrap();
        fs::write(&file2, orig2).unwrap();

        let req = PatchTransactionRequest {
            workspace_root: Some(temp_dir.path().to_path_buf()),
            patches: vec![
                SymbolPatchUnit {
                    file_path: file1.clone(),
                    symbol_query: "fetchUsers".to_string(),
                    replacement_code: "export function fetchUsers() {\n    return [{ id: 1 }];\n}".to_string(),
                },
                SymbolPatchUnit {
                    file_path: file2.clone(),
                    symbol_query: "handleRequest".to_string(),
                    replacement_code: "export function handleRequest() {\n    return 'handled';\n}".to_string(),
                },
            ],
            typechecker: None,
            apply: false, // dry-run preview
            timeout_ms: Some(5000),
        };

        let res = BatchAstPatcher::apply_transaction(&req).unwrap();
        assert!(res.success);
        assert!(!res.applied);
        assert!(res.rolled_back);
        assert_eq!(res.files_modified_count, 2);
        assert_eq!(res.symbols_patched_count, 2);

        // Verify disk contents reverted to original
        assert_eq!(fs::read_to_string(&file1).unwrap(), orig1);
        assert_eq!(fs::read_to_string(&file2).unwrap(), orig2);
    }

    #[test]
    fn test_batch_patch_syntax_error_abort() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("syntax.rs");
        let original = "pub fn calculate() -> i32 {\n    42\n}\n";
        fs::write(&file_path, original).unwrap();

        let req = PatchTransactionRequest {
            workspace_root: Some(temp_dir.path().to_path_buf()),
            patches: vec![SymbolPatchUnit {
                file_path: file_path.clone(),
                symbol_query: "calculate".to_string(),
                replacement_code: "pub fn calculate() -> i32 {\n    42 // unclosed".to_string(),
            }],
            typechecker: None,
            apply: true,
            timeout_ms: Some(5000),
        };

        let res = BatchAstPatcher::apply_transaction(&req).unwrap();
        assert!(!res.success);
        assert!(!res.applied);
        assert!(!res.syntax_errors.is_empty());

        // File on disk remains intact
        assert_eq!(fs::read_to_string(&file_path).unwrap(), original);
    }
}
