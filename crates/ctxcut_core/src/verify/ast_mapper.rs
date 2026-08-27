//! Compiler and typechecker diagnostic mapping to AST symbol nodes and patch-relative offsets.

use crate::model::VerifyDiagnostic;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A compiler or linter diagnostic mapped directly to the target AST symbol and patch-relative offset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappedPatchDiagnostic {
    /// Target file path reporting the diagnostic.
    pub file_path: String,
    /// Enclosing symbol name if the diagnostic falls within a patched symbol range.
    pub symbol_name: Option<String>,
    /// AST node kind (e.g. "function", "method", "class", "impl_item").
    pub node_kind: Option<String>,
    /// 1-based absolute line number in the patched file.
    pub line: Option<usize>,
    /// 1-based column number in the patched file.
    pub column: Option<usize>,
    /// 1-based line number relative to the start of the replacement code.
    pub patch_relative_line: Option<usize>,
    /// Code snippet from the source or replacement at the diagnostic location.
    pub code_snippet: Option<String>,
    /// Diagnostic code (e.g. "TS2322", "E0308").
    pub code: Option<String>,
    /// Human-readable diagnostic error message.
    pub message: String,
    /// Diagnostic severity ("error", "warning", "info").
    pub severity: String,
}

/// Metadata describing a single patched symbol's post-splice location within a modified file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchedSymbolMeta {
    /// Resolved symbol name (e.g. `"calculate_tax"`, `"AuthService.login"`).
    pub symbol_name: String,
    /// AST node kind (e.g. `"function"`, `"method"`, `"class"`).
    pub node_kind: String,
    /// 1-based start line of the spliced replacement code in the patched file.
    pub start_line: usize,
    /// 1-based end line of the spliced replacement code in the patched file.
    pub end_line: usize,
    /// Replacement code string.
    pub replacement_code: String,
}

/// Information about a patched file used to map compiler diagnostics back to AST symbols.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchedFileInfo {
    /// File path of the modified file.
    pub file_path: PathBuf,
    /// Full patched source code text.
    pub patched_source: String,
    /// List of symbols modified within this file.
    pub symbols: Vec<PatchedSymbolMeta>,
}

/// Engine that maps raw compiler diagnostics to precise AST symbol boundaries and replacement offsets.
pub struct AstDiagnosticMapper;

impl AstDiagnosticMapper {
    /// Maps a slice of `VerifyDiagnostic` items to `MappedPatchDiagnostic` with AST symbol context.
    pub fn map_diagnostics(
        diagnostics: &[VerifyDiagnostic],
        file_patches: &[PatchedFileInfo],
        workspace_root: Option<&Path>,
    ) -> Vec<MappedPatchDiagnostic> {
        let mut mapped = Vec::with_capacity(diagnostics.len());

        for diag in diagnostics {
            let matched_file = diag.file.as_deref().and_then(|df| {
                file_patches
                    .iter()
                    .find(|pf| is_file_match(df, &pf.file_path, workspace_root))
            });

            if let Some(pf) = matched_file {
                let file_path_str = pf.file_path.to_string_lossy().to_string();
                let mut symbol_name = None;
                let mut node_kind = None;
                let mut patch_relative_line = None;
                let mut code_snippet = None;

                if let Some(line) = diag.line {
                    // Check if line falls within any of the patched symbols
                    if let Some(sym) = pf
                        .symbols
                        .iter()
                        .find(|s| line >= s.start_line && line <= s.end_line)
                    {
                        symbol_name = Some(sym.symbol_name.clone());
                        node_kind = Some(sym.node_kind.clone());
                        patch_relative_line = Some(line.saturating_sub(sym.start_line) + 1);

                        // Extract snippet from replacement code or patched source
                        code_snippet = extract_line_from_replacement(
                            &sym.replacement_code,
                            patch_relative_line.unwrap(),
                        )
                        .or_else(|| extract_line_from_source(&pf.patched_source, line));
                    } else {
                        // Diagnostic is outside patched symbols but within the patched file
                        code_snippet = extract_line_from_source(&pf.patched_source, line);
                    }
                }

                mapped.push(MappedPatchDiagnostic {
                    file_path: file_path_str,
                    symbol_name,
                    node_kind,
                    line: diag.line,
                    column: diag.column,
                    patch_relative_line,
                    code_snippet,
                    code: diag.code.clone(),
                    message: diag.message.clone(),
                    severity: diag.severity.clone(),
                });
            } else {
                // Diagnostic did not match any patched file in transaction
                let file_path_str = diag.file.clone().unwrap_or_default();
                mapped.push(MappedPatchDiagnostic {
                    file_path: file_path_str,
                    symbol_name: None,
                    node_kind: None,
                    line: diag.line,
                    column: diag.column,
                    patch_relative_line: None,
                    code_snippet: None,
                    code: diag.code.clone(),
                    message: diag.message.clone(),
                    severity: diag.severity.clone(),
                });
            }
        }

        mapped
    }
}

/// Normalizes path comparison across Windows/Unix separators and relative/absolute forms.
fn is_file_match(diag_file: &str, patched_file: &Path, workspace_root: Option<&Path>) -> bool {
    let df_norm = diag_file.replace('\\', "/");
    let pf_norm = patched_file.to_string_lossy().replace('\\', "/");

    if df_norm == pf_norm || pf_norm.ends_with(&df_norm) || df_norm.ends_with(&pf_norm) {
        return true;
    }

    if let Some(root) = workspace_root {
        let abs_df = root.join(diag_file);
        let abs_df_norm = abs_df.to_string_lossy().replace('\\', "/");
        if abs_df_norm == pf_norm {
            return true;
        }
    }

    false
}

/// Extracts a specific 1-based line from a multi-line source string.
pub fn extract_line_from_source(source: &str, line_1_based: usize) -> Option<String> {
    if line_1_based == 0 {
        return None;
    }
    source
        .lines()
        .nth(line_1_based - 1)
        .map(|l| l.trim_end().to_string())
}

/// Extracts a specific 1-based line from the replacement code.
fn extract_line_from_replacement(replacement: &str, rel_line_1_based: usize) -> Option<String> {
    if rel_line_1_based == 0 {
        return None;
    }
    replacement
        .lines()
        .nth(rel_line_1_based - 1)
        .map(|l| l.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_diagnostic_mapper_inside_symbol() {
        let replacement = "pub fn add(a: i32, b: i32) -> i32 {\n    let sum: String = a + b;\n    sum\n}";
        let patched_source = format!("// Header\n{replacement}\n// Footer\n");

        let sym_meta = PatchedSymbolMeta {
            symbol_name: "add".to_string(),
            node_kind: "function".to_string(),
            start_line: 2,
            end_line: 5,
            replacement_code: replacement.to_string(),
        };

        let file_patch = PatchedFileInfo {
            file_path: PathBuf::from("src/calc.rs"),
            patched_source,
            symbols: vec![sym_meta],
        };

        let raw_diag = VerifyDiagnostic {
            severity: "error".to_string(),
            line: Some(3),
            column: Some(9),
            message: "mismatched types: expected String, found i32".to_string(),
            file: Some("src/calc.rs".to_string()),
            code: Some("E0308".to_string()),
        };

        let mapped = AstDiagnosticMapper::map_diagnostics(&[raw_diag], &[file_patch], None);
        assert_eq!(mapped.len(), 1);
        let m = &mapped[0];

        assert_eq!(m.symbol_name.as_deref(), Some("add"));
        assert_eq!(m.node_kind.as_deref(), Some("function"));
        assert_eq!(m.line, Some(3));
        assert_eq!(m.patch_relative_line, Some(2)); // line 3 is 2nd line of 4-line function starting at line 2
        assert_eq!(m.code.as_deref(), Some("E0308"));
        assert!(m.code_snippet.as_ref().unwrap().contains("let sum: String"));
    }

    #[test]
    fn test_ast_diagnostic_mapper_outside_symbol() {
        let patched_source = "fn one() {}\nfn two() {}\nfn three() {}\n";

        let sym_meta = PatchedSymbolMeta {
            symbol_name: "two".to_string(),
            node_kind: "function".to_string(),
            start_line: 2,
            end_line: 2,
            replacement_code: "fn two() {}".to_string(),
        };

        let file_patch = PatchedFileInfo {
            file_path: PathBuf::from("src/lib.rs"),
            patched_source: patched_source.to_string(),
            symbols: vec![sym_meta],
        };

        let raw_diag = VerifyDiagnostic {
            severity: "warning".to_string(),
            line: Some(1),
            column: Some(1),
            message: "unused function".to_string(),
            file: Some("src/lib.rs".to_string()),
            code: None,
        };

        let mapped = AstDiagnosticMapper::map_diagnostics(&[raw_diag], &[file_patch], None);
        assert_eq!(mapped.len(), 1);
        let m = &mapped[0];

        assert_eq!(m.symbol_name, None);
        assert_eq!(m.node_kind, None);
        assert_eq!(m.patch_relative_line, None);
        assert_eq!(m.line, Some(1));
        assert_eq!(m.code_snippet.as_deref(), Some("fn one() {}"));
    }
}
