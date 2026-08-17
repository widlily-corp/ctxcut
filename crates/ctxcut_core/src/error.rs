//! Error types for the `ctxcut_core` library crate.

use std::path::PathBuf;
use thiserror::Error;

/// The primary error type for operations within `ctxcut_core`.
#[derive(Debug, Error)]
pub enum CoreError {
    /// An I/O error occurred while accessing a file or directory.
    #[error("I/O error at '{path}': {source}")]
    Io {
        /// File path where the I/O error occurred.
        path: PathBuf,
        /// The underlying standard I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The target file's language could not be determined or is not supported.
    #[error("Unsupported language for file '{path}' (extension: {extension:?})")]
    UnsupportedLanguage {
        /// File path with unsupported extension.
        path: PathBuf,
        /// The extension that was extracted, if any.
        extension: Option<String>,
    },

    /// Tree-sitter failed to parse the source code.
    #[error("Failed to parse source file '{path}': {message}")]
    ParseError {
        /// File path where parse failure occurred.
        path: PathBuf,
        /// Reason for parse failure.
        message: String,
    },

    /// The requested symbol could not be found in the AST.
    #[error("{}", format_symbol_not_found(.symbol, .path, .available_symbols))]
    SymbolNotFound {
        /// Symbol identifier searched for.
        symbol: String,
        /// Target file searched.
        path: PathBuf,
        /// List of available top-level or method symbols found in the file.
        available_symbols: Vec<String>,
    },

    /// Import resolution failed for an external or relative module.
    #[error("Failed to resolve import '{import_path}' from '{source_file}': {message}")]
    ImportResolutionError {
        /// Raw import path string.
        import_path: String,
        /// File containing the import statement.
        source_file: PathBuf,
        /// Diagnostic error details.
        message: String,
    },

    /// Tree-sitter query creation or execution error.
    #[error("Tree-sitter query error: {0}")]
    QueryError(String),

    /// Clipboard operation failed.
    #[error("Clipboard error: {0}")]
    ClipboardError(String),

    /// Output formatting error.
    #[error("Formatting error: {0}")]
    FormattingError(String),
}

fn format_symbol_not_found(symbol: &str, path: &std::path::Path, available: &[String]) -> String {
    let mut msg = format!("Symbol '{}' was not found in '{}'.", symbol, path.display());

    // Find best fuzzy match
    if let Some(best) = find_best_suggestion(symbol, available) {
        msg.push_str(&format!(" Did you mean '{}'?", best));
    }

    if !available.is_empty() {
        msg.push_str(&format!(" Available symbols: {:?}", available));
    }

    msg
}

fn find_best_suggestion<'a>(query: &str, candidates: &'a [String]) -> Option<&'a str> {
    let query_lower = query.to_lowercase();
    for candidate in candidates {
        let cand_name = candidate.split('.').next_back().unwrap_or(candidate);
        let cand_lower = cand_name.to_lowercase();

        if cand_lower == query_lower
            || cand_lower.starts_with(&query_lower)
            || query_lower.starts_with(&cand_lower)
            || edit_distance(&query_lower, &cand_lower) <= 2
        {
            return Some(candidate);
        }
    }
    None
}

fn edit_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();

    let mut dp = vec![vec![0; len2 + 1]; len1 + 1];

    for (i, row) in dp.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().take(len2 + 1) {
        *cell = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if v1[i - 1] == v2[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[len1][len2]
}

/// Specialized Result alias for `ctxcut_core`.
pub type Result<T> = std::result::Result<T, CoreError>;
