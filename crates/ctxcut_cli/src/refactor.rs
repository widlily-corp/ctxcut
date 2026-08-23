//! AST-guided Symbol Refactoring and Renaming CLI handler (`ctxcut refactor`).

use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::refactor::SymbolRenamer;
use std::fs;
use std::path::Path;

/// Options for `refactor rename` command.
#[derive(Debug, Clone, Copy)]
pub struct RefactorRenameOptions<'a> {
    /// Target symbol in format `path/to/file.ts:symbolName` or symbol name across project.
    pub target: &'a str,
    /// New symbol name.
    pub to: &'a str,
    /// Repository root path.
    pub root: Option<&'a Path>,
    /// Dry run mode.
    pub dry_run: bool,
    /// Output format ("text", "markdown", "json").
    pub format: &'a str,
    /// Copy output to clipboard.
    pub clip: bool,
    /// Output file path to save report.
    pub output: Option<&'a Path>,
}

/// Executes `refactor rename` command.
pub fn run_refactor_rename(opts: RefactorRenameOptions) -> Result<()> {
    let ws_root = opts.root.unwrap_or_else(|| Path::new("."));

    let result = SymbolRenamer::rename_symbol(ws_root, opts.target, opts.to, opts.dry_run)
        .context("Failed to execute AST symbol refactoring")?;

    let rendered = if opts.format.eq_ignore_ascii_case("json") {
        result.to_json()
    } else if opts.format.eq_ignore_ascii_case("markdown") {
        result.to_markdown()
    } else {
        let mut out = String::new();
        for file in &result.files {
            out.push_str(&format!(
                "\n{} {} ({} occurrences)\n",
                if opts.dry_run { "ℹ".cyan() } else { "✔".green() },
                file.file_path.bold(),
                file.occurrences_count
            ));
            out.push_str(&file.diff);
            out.push('\n');
        }

        if opts.dry_run {
            out.push_str(&format!(
                "\n{} Dry run complete: would rename `{}` -> `{}` across {} occurrence(s) in {} file(s)\n",
                "ℹ".cyan(),
                result.old_name,
                result.new_name,
                result.total_occurrences,
                result.total_files_modified
            ));
        } else {
            out.push_str(&format!(
                "\n{} Successfully renamed `{}` -> `{}` across {} occurrence(s) in {} file(s)\n",
                "✔".green(),
                result.old_name,
                result.new_name,
                result.total_occurrences,
                result.total_files_modified
            ));
        }
        out
    };

    if let Some(out_file) = opts.output {
        fs::write(out_file, &rendered)
            .with_context(|| format!("Failed to write refactor report to `{}`", out_file.display()))?;
        println!("{} Refactor report saved to `{}`", "✔".green(), out_file.display());
    }

    if opts.clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(&rendered);
            eprintln!("{} Refactor report copied to clipboard!", "✔".green());
        }
    }

    if opts.output.is_none() {
        print!("{rendered}");
    }

    Ok(())
}
