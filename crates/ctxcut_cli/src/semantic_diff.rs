//! Semantic AST Diff CLI handler (`ctxcut semantic-diff`).

use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::diff::SemanticDiffEngine;
use std::fs;
use std::path::PathBuf;

/// Options for `semantic-diff` command.
#[derive(Debug, Clone)]
pub struct SemanticDiffOptions {
    /// Workspace root directory path.
    pub root: Option<PathBuf>,
    /// Specific file path to diff.
    pub file: Option<PathBuf>,
    /// Compare staged changes only.
    pub staged: bool,
    /// Adaptive token budget limit.
    pub budget: Option<usize>,
    /// Copy output to clipboard.
    pub clip: bool,
    /// Output file path to save diff report.
    pub output: Option<PathBuf>,
    /// Output format ("markdown" or "json").
    pub format: String,
}

/// Executes `semantic-diff` command.
pub fn run_semantic_diff(opts: SemanticDiffOptions) -> Result<()> {
    let root = opts.root.unwrap_or_else(|| PathBuf::from("."));

    let result =
        SemanticDiffEngine::compute_diff(&root, opts.staged, opts.file.as_deref(), opts.budget)
            .context("Failed to compute semantic AST diff")?;

    let rendered = if opts.format.eq_ignore_ascii_case("json") {
        result.to_json()
    } else {
        result.to_markdown()
    };

    if let Some(ref out_file) = opts.output {
        fs::write(out_file, &rendered).with_context(|| {
            format!(
                "Failed to write semantic diff report to `{}`",
                out_file.display()
            )
        })?;
        println!(
            "{} Semantic diff saved to `{}`",
            "✔".green(),
            out_file.display()
        );
    }

    if opts.clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(&rendered);
            eprintln!("{} Semantic diff copied to clipboard!", "✔".green());
        }
    }

    if opts.output.is_none() {
        println!("{rendered}");
    }

    Ok(())
}
