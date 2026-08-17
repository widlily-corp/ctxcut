//! Pre-write Tree-Sitter syntax validation guard for AST patch operations.

use crate::error::{CoreError, Result};
use crate::model::SyntaxErrorDetail;
use crate::parser::ParserManager;
use std::path::Path;
use tree_sitter::{Language, Node};

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
}
