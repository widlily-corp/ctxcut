//! AST Query Engine data structures and reporting models.

use crate::model::SupportedLanguage;
use serde::{Deserialize, Serialize};

/// A single captured node within a Tree-sitter query match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchCapture {
    /// Capture tag name (e.g. "name", "definition", "body", "method", "path").
    pub name: String,
    /// UTF-8 text content of the captured node.
    pub text: String,
    /// Tree-sitter AST node kind (e.g. "identifier", "function_item", "class_declaration").
    pub node_kind: String,
    /// 1-based start line in source file.
    pub start_line: usize,
    /// 1-based start column in source file.
    pub start_col: usize,
    /// 1-based end line in source file.
    pub end_line: usize,
    /// 1-based end column in source file.
    pub end_col: usize,
    /// 0-based start byte offset in source file.
    pub start_byte: usize,
    /// 0-based end byte offset in source file.
    pub end_byte: usize,
}

/// A matched AST structural occurrence in a source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryMatchResult {
    /// Relative or absolute path to the source file.
    pub file_path: String,
    /// Detected programming language of the file.
    pub language: SupportedLanguage,
    /// Primary symbol or construct identifier name (extracted from `@name` capture or root node).
    pub symbol_name: Option<String>,
    /// Structural category (e.g. "function", "struct", "class", "interface", "enum", "export", "async_fn", "api_route", "error").
    pub kind: String,
    /// 1-based start line of the matched definition node.
    pub start_line: usize,
    /// 1-based end line of the matched definition node.
    pub end_line: usize,
    /// Source code snippet of the matched definition.
    pub snippet: String,
    /// All individual captures extracted from the match pattern.
    pub captures: Vec<MatchCapture>,
}

/// Aggregated query report across workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstQueryReport {
    /// Raw query pattern or preset name.
    pub query: String,
    /// Preset name used (if applicable).
    pub preset: Option<String>,
    /// Total number of matches found across all files.
    pub total_matches: usize,
    /// Total number of files scanned.
    pub files_scanned: usize,
    /// Total number of files containing matches.
    pub files_matched: usize,
    /// All match occurrences.
    pub matches: Vec<QueryMatchResult>,
}

impl AstQueryReport {
    /// Formats the query report into clean, syntax-highlighted Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# AST Query Results: `{}`\n\n",
            self.preset.as_deref().unwrap_or(&self.query)
        ));
        out.push_str(&format!(
            "**Total Matches**: {} across {} file(s) (scanned {} files)\n\n",
            self.total_matches, self.files_matched, self.files_scanned
        ));

        let mut current_file = "";
        for m in &self.matches {
            if m.file_path != current_file {
                current_file = &m.file_path;
                out.push_str(&format!("## {}\n\n", m.file_path));
            }

            let sym_label = m
                .symbol_name
                .as_deref()
                .map(|s| format!(" `{s}`"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- **{}**{} (Lines {}-{}):\n\n",
                m.kind, sym_label, m.start_line, m.end_line
            ));
            out.push_str(&format!(
                "```{lang}\n{code}\n```\n\n",
                lang = m.language.as_str(),
                code = m.snippet.trim()
            ));
        }

        out
    }

    /// Formats report as pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}
