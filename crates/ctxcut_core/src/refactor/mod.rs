//! AST-guided refactoring, batch multi-symbol mutation, and symbol renaming module.

pub mod batch;
pub mod rename;

pub use batch::{
    BatchAstPatcher, FilePatchDiff, PatchTransactionRequest, PatchTransactionResult,
    SymbolPatchUnit, TransactionalPatcher,
};
pub use rename::SymbolRenamer;
use serde::{Deserialize, Serialize};

/// Type of symbol being renamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameTargetKind {
    /// Function declaration.
    Function,
    /// Method within a class, struct, or interface.
    Method,
    /// Class or struct declaration.
    ClassOrStruct,
    /// Interface or trait declaration.
    InterfaceOrTrait,
    /// Type alias or typedef.
    TypeAlias,
    /// Variable or constant.
    VariableOrConst,
    /// Enum declaration.
    Enum,
    /// Unknown or generic symbol.
    Unknown,
}

/// A specific occurrence of a renamed identifier within a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRenameOccurrence {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
    /// Kind of occurrence: "declaration", "call_site", "import_specifier", "reexport", "type_reference", "attribute".
    pub kind: String,
    /// Exact source code snippet around the occurrence.
    pub snippet: String,
}

/// Refactoring result for a single modified file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRenameResult {
    /// File path relative to workspace root or absolute.
    pub file_path: String,
    /// Number of identifier occurrences renamed in this file.
    pub occurrences_count: usize,
    /// Detailed list of occurrences.
    pub occurrences: Vec<SymbolRenameOccurrence>,
    /// Unified diff representation of the modifications.
    pub diff: String,
    /// Whether changes were written to disk (`!dry_run`).
    pub applied: bool,
}

/// Aggregate multi-file AST symbol renaming result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFileRenameResult {
    /// Original target symbol name.
    pub old_name: String,
    /// New symbol name.
    pub new_name: String,
    /// Declaring file path (if known/provided).
    pub target_file: Option<String>,
    /// Total count of modified files.
    pub total_files_modified: usize,
    /// Total count of renamed identifier occurrences across all files.
    pub total_occurrences: usize,
    /// Per-file refactoring details.
    pub files: Vec<FileRenameResult>,
    /// Whether execution was a dry run.
    pub dry_run: bool,
}

impl MultiFileRenameResult {
    /// Formats the rename result as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let status_str = if self.dry_run {
            "Dry Run (Preview)"
        } else {
            "Applied"
        };
        out.push_str(&format!(
            "# AST Symbol Rename: `{}` -> `{}` ({})\n\n",
            self.old_name, self.new_name, status_str
        ));
        out.push_str(&format!(
            "- **Total Files Modified:** `{}`\n- **Total Occurrences Renamed:** `{}`\n\n",
            self.total_files_modified, self.total_occurrences
        ));

        for file in &self.files {
            out.push_str(&format!(
                "### `{}` ({} occurrences)\n\n```diff\n{}\n```\n\n",
                file.file_path,
                file.occurrences_count,
                file.diff.trim()
            ));
        }
        out
    }

    /// Formats the rename result as pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the rename result as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}
