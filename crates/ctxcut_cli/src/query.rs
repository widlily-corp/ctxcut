//! AST Structural Query Engine CLI handler (`ctxcut query`).

use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::{AstQueryEngine, SupportedLanguage};
use std::fs;
use std::path::{Path, PathBuf};

/// Options for `query` command.
#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    /// Tree-sitter S-expression AST query pattern.
    pub pattern: Option<String>,
    /// Built-in query preset name.
    pub preset: Option<String>,
    /// Language filter.
    pub lang: Option<String>,
    /// Repository root path.
    pub root: Option<PathBuf>,
    /// Maximum matches limit.
    pub limit: Option<usize>,
    /// Output format (markdown or json).
    pub format: String,
    /// Copy output to clipboard.
    pub clip: bool,
    /// Output file path.
    pub output: Option<PathBuf>,
}

/// Executes `ctxcut query` command.
pub fn run_query_command(opts: &QueryOptions) -> Result<()> {
    let ws_root = opts.root.as_deref().unwrap_or_else(|| Path::new("."));

    let lang_filter = opts
        .lang
        .as_deref()
        .and_then(SupportedLanguage::from_str_loose);

    let report = AstQueryEngine::query_workspace(
        ws_root,
        opts.pattern.as_deref(),
        lang_filter,
        opts.preset.as_deref(),
        opts.limit,
    )
    .with_context(|| "Failed to execute AST structural query")?;

    let rendered = if opts.format.eq_ignore_ascii_case("json") {
        report.to_json()
    } else {
        report.to_markdown()
    };

    if let Some(out_path) = opts.output.as_deref() {
        fs::write(out_path, &rendered)
            .with_context(|| format!("Failed to write query output to `{}`", out_path.display()))?;
        println!(
            "{} Query results saved to `{}`",
            "✔".green(),
            out_path.display()
        );
    }

    if opts.clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&rendered).is_ok() {
                eprintln!("{} AST query results copied to clipboard!", "✔".green());
            }
        }
    }

    if opts.output.is_none() {
        println!("{rendered}");
    }

    Ok(())
}
