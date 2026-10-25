//! Pre-write Tree-Sitter syntax validation guard for AST patch operations.

use crate::error::{CoreError, Result};
use crate::model::SyntaxErrorDetail;
use crate::parser::ParserManager;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Node};

/// Re-exported AST grafting types for convenience.
pub mod graft {
    pub use super::super::graft::*;
}
pub use super::graft::{AstGraftEngine, GraftResult, IndentStyle};

/// Structured report from an in-memory syntax validation check following AST grafting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraftValidationReport {
    /// Whether the patched source syntax parsed cleanly without any errors.
    pub is_valid: bool,
    /// Path of the file being validated.
    pub file_path: PathBuf,
    /// Detailed list of AST syntax errors, if any.
    pub errors: Vec<SyntaxErrorDetail>,
    /// Unified diff preview between original and patched source code.
    pub diff: Option<String>,
}

impl GraftValidationReport {
    /// Returns true if the patched source is syntactically valid with zero errors.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Returns the number of syntax errors detected during verification.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Converts this validation report into a `Result<()>`, returning `CoreError::SyntaxValidationError` if invalid.
    pub fn to_result(&self) -> Result<()> {
        if self.is_valid {
            Ok(())
        } else {
            Err(CoreError::SyntaxValidationError {
                path: self.file_path.clone(),
                errors: self.errors.clone(),
            })
        }
    }
}

/// Syntax validator that checks patched source files for syntactical correctness before writing.
pub struct SyntaxValidator;

impl SyntaxValidator {
    /// Validates that `source` parses cleanly with the given grammar without any syntax errors.
    ///
    /// Performs an $O(1)$ fast check on `tree.root_node().has_error()`. If errors are detected,
    /// a pruned recursive AST traversal collects precise error locations and diagnostic details.
    pub fn validate_source(source: &str, language: &Language, file_path: &Path) -> Result<()> {
        let tree = ParserManager::parse_source(source, language, file_path)?;
        let root = tree.root_node();

        if !root.has_error() {
            return Ok(());
        }

        let mut errors = Vec::new();
        collect_errors_recursive(root, source, &mut errors);

        if errors.is_empty() {
            // Fallback in case root.has_error() was true but no specific ERROR/MISSING node was visited
            errors.push(SyntaxErrorDetail {
                line: 1,
                column: 1,
                byte_offset: 0,
                kind: "SYNTAX_ERROR".to_string(),
                snippet: extract_error_snippet(source, 0, source.len().min(40)),
                is_missing: false,
            });
        }

        Err(CoreError::SyntaxValidationError {
            path: file_path.to_path_buf(),
            errors,
        })
    }

    /// Validates patched source text in-memory without performing any filesystem writes.
    ///
    /// Checks the patched source AST for syntax errors using Tree-Sitter. Returns a structured
    /// `GraftValidationReport` containing validation status, precise error diagnostics (if any),
    /// and a unified diff preview against the original source text.
    pub fn validate_graft_in_memory(
        original_source: &str,
        patched_source: &str,
        language: &Language,
        file_path: &Path,
    ) -> Result<GraftValidationReport> {
        let tree = ParserManager::parse_source(patched_source, language, file_path)?;
        let root = tree.root_node();

        let diff = Some(crate::patch::formatting::generate_unified_diff(
            original_source,
            patched_source,
            file_path,
            3,
        ));

        if !root.has_error() {
            return Ok(GraftValidationReport {
                is_valid: true,
                file_path: file_path.to_path_buf(),
                errors: Vec::new(),
                diff,
            });
        }

        let mut errors = Vec::new();
        collect_errors_recursive(root, patched_source, &mut errors);

        if errors.is_empty() {
            // Fallback in case root.has_error() was true but no specific ERROR/MISSING node was visited
            errors.push(SyntaxErrorDetail {
                line: 1,
                column: 1,
                byte_offset: 0,
                kind: "SYNTAX_ERROR".to_string(),
                snippet: extract_error_snippet(patched_source, 0, patched_source.len().min(40)),
                is_missing: false,
            });
        }

        Ok(GraftValidationReport {
            is_valid: false,
            file_path: file_path.to_path_buf(),
            errors,
            diff,
        })
    }

    /// Validates patched source text in-memory for a given `SupportedLanguage`.
    pub fn validate_graft_for_language(
        original_source: &str,
        patched_source: &str,
        language: crate::model::SupportedLanguage,
        file_path: &Path,
    ) -> Result<GraftValidationReport> {
        let adapter = crate::lang::LanguageRegistry::for_language(language)?;
        let ts_lang = adapter.tree_sitter_language(file_path);
        Self::validate_graft_in_memory(original_source, patched_source, &ts_lang, file_path)
    }
}

fn collect_errors_recursive(node: Node<'_>, source: &str, out: &mut Vec<SyntaxErrorDetail>) {
    if node.is_error() || node.is_missing() {
        let start_pos = node.start_position();
        let line = start_pos.row + 1;
        let column = start_pos.column + 1;
        let byte_offset = node.start_byte();
        let kind = if node.is_missing() {
            format!("MISSING {}", node.kind())
        } else {
            node.kind().to_string()
        };
        let snippet = extract_error_snippet(source, node.start_byte(), node.end_byte());

        out.push(SyntaxErrorDetail {
            line,
            column,
            byte_offset,
            kind,
            snippet,
            is_missing: node.is_missing(),
        });
        return; // Do not descend into children of an ERROR node
    }

    if !node.has_error() {
        return; // Prune clean subtrees
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_errors_recursive(child, source, out);
    }
}

fn extract_error_snippet(source: &str, start_byte: usize, end_byte: usize) -> String {
    let len = source.len();
    if len == 0 {
        return String::new();
    }

    let start = start_byte.min(len);
    let end = end_byte.min(len).max(start);

    let snippet_raw = if start == end {
        let context_end = (start + 30).min(len);
        &source[start..context_end]
    } else {
        let slice_len = (end - start).min(50);
        &source[start..start + slice_len]
    };

    snippet_raw
        .lines()
        .next()
        .unwrap_or(snippet_raw)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_valid_rust_syntax() {
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let lang: Language = tree_sitter_rust::LANGUAGE.into();
        let res = SyntaxValidator::validate_source(code, &lang, &PathBuf::from("test.rs"));
        assert!(res.is_ok());
    }

    #[test]
    fn test_invalid_rust_syntax() {
        let code = "fn main( {\n    println!(\"hello\");\n}\n";
        let lang: Language = tree_sitter_rust::LANGUAGE.into();
        let res = SyntaxValidator::validate_source(code, &lang, &PathBuf::from("test.rs"));
        assert!(res.is_err());
        if let Err(CoreError::SyntaxValidationError { errors, .. }) = res {
            assert!(!errors.is_empty());
        } else {
            panic!("Expected SyntaxValidationError");
        }
    }

    #[test]
    fn test_valid_python_syntax() {
        let code = "def hello():\n    return 42\n";
        let lang: Language = tree_sitter_python::LANGUAGE.into();
        let res = SyntaxValidator::validate_source(code, &lang, &PathBuf::from("test.py"));
        assert!(res.is_ok());
    }

    #[test]
    fn test_invalid_python_syntax() {
        let code = "def hello(\n    return 42\n";
        let lang: Language = tree_sitter_python::LANGUAGE.into();
        let res = SyntaxValidator::validate_source(code, &lang, &PathBuf::from("test.py"));
        assert!(res.is_err());
    }

    #[test]
    fn test_validate_graft_in_memory_valid() {
        let orig = "fn compute() -> i32 { 10 }";
        let patched = "fn compute() -> i32 { 20 }";
        let lang: Language = tree_sitter_rust::LANGUAGE.into();
        let rep = SyntaxValidator::validate_graft_in_memory(
            orig,
            patched,
            &lang,
            &PathBuf::from("calc.rs"),
        )
        .unwrap();
        assert!(rep.is_valid());
        assert_eq!(rep.error_count(), 0);
        assert!(rep.diff.is_some());
    }

    #[test]
    fn test_validate_graft_in_memory_invalid() {
        let orig = "fn compute() -> i32 { 10 }";
        let patched = "fn compute( -> i32 { 20 }";
        let lang: Language = tree_sitter_rust::LANGUAGE.into();
        let rep = SyntaxValidator::validate_graft_in_memory(
            orig,
            patched,
            &lang,
            &PathBuf::from("calc.rs"),
        )
        .unwrap();
        assert!(!rep.is_valid());
        assert!(rep.error_count() > 0);
        assert!(rep.errors[0].line > 0);
        assert!(rep.to_result().is_err());
    }
}
