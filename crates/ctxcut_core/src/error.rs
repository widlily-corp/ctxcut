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
    #[error("Symbol '{symbol}' was not found in '{path}'. Available symbols: {available_symbols:?}")]
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

    /// BPE Tokenization failure.
    #[error("BPE tokenization error: {0}")]
    TokenizerError(String),

    /// Invalid slicing configuration options.
    #[error("Invalid slice options: {0}")]
    InvalidOptions(String),

    /// JSON serialization or deserialization error.
    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl From<tree_sitter::QueryError> for CoreError {
    fn from(err: tree_sitter::QueryError) -> Self {
        Self::QueryError(format!("{err:?}"))
    }
}

/// A specialized Result type for `ctxcut_core` operations.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;
