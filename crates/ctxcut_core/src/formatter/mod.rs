//! Formatter module for rendering AST slice outputs as Markdown and JSON.

pub mod json;
pub mod markdown;

pub use json::JsonFormatter;
pub use markdown::{normalize_language_tag, MarkdownFormatter};

/// General formatting trait for context slice results.
pub trait Formatter {
    /// Formats a single `SliceResult`.
    fn format(&self, result: &crate::model::SliceResult) -> crate::error::Result<String>;
}
