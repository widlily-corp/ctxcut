//! AST-guided Symbol Refactoring and Renaming CLI handler (`ctxcut refactor`).

use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::refactor::batch::{
    BatchAstPatcher, PatchTransactionRequest, SymbolPatchUnit,
};
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

/// Options for `refactor batch` command.
#[derive(Debug, Clone, Copy)]
pub struct RefactorBatchOptions<'a> {
    /// JSON string of patch instructions.
    pub patches: Option<&'a str>,
    /// Path to JSON file containing patch array or transaction request.
    pub file: Option<&'a Path>,
    /// Workspace root directory.
    pub root: Option<&'a Path>,
    /// Custom typechecker command override (e.g. `cargo check`, `tsc --noEmit`).
    pub typechecker: Option<&'a str>,
    /// Commit changes to disk on success (default: false).
    pub apply: bool,
    /// Dry run preview without disk mutations.
    pub dry_run: bool,
    /// Typechecker execution timeout in milliseconds.
    pub timeout_ms: Option<u64>,
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
                if opts.dry_run {
                    "ℹ".cyan()
                } else {
                    "✔".green()
                },
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
        fs::write(out_file, &rendered).with_context(|| {
            format!(
                "Failed to write refactor report to `{}`",
                out_file.display()
            )
        })?;
        println!(
            "{} Refactor report saved to `{}`",
            "✔".green(),
            out_file.display()
        );
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

/// Executes `refactor batch` command.
pub fn run_refactor_batch(opts: RefactorBatchOptions) -> Result<()> {
    let ws_root = opts.root.unwrap_or_else(|| Path::new("."));

    let patch_units: Vec<SymbolPatchUnit> = if let Some(ref f) = opts.file {
        let content = fs::read_to_string(f)
            .with_context(|| format!("Failed to read batch patch file `{}`", f.display()))?;
        if let Ok(req) = serde_json::from_str::<PatchTransactionRequest>(&content) {
            req.patches
        } else {
            serde_json::from_str::<Vec<SymbolPatchUnit>>(&content)
                .with_context(|| format!("Failed to parse JSON patches in `{}`", f.display()))?
        }
    } else if let Some(p_str) = opts.patches {
        serde_json::from_str::<Vec<SymbolPatchUnit>>(p_str)
            .with_context(|| "Failed to parse `--patches` JSON string. Expected array of SymbolPatchUnit.")?
    } else {
        bail!("Missing patch instructions. Provide `--patches <JSON>` or `--file <PATH>`");
    };

    let should_apply = opts.apply && !opts.dry_run;

    let req = PatchTransactionRequest {
        workspace_root: Some(ws_root.to_path_buf()),
        patches: patch_units,
        typechecker: opts.typechecker.map(String::from),
        apply: should_apply,
        timeout_ms: opts.timeout_ms,
    };

    let result = BatchAstPatcher::apply_transaction(&req)
        .context("Failed to execute batch AST patch transaction")?;

    let rendered = if opts.format.eq_ignore_ascii_case("json") {
        result.to_json()
    } else if opts.format.eq_ignore_ascii_case("markdown") {
        result.to_markdown()
    } else {
        let mut out = String::new();
        if result.applied {
            out.push_str(&format!(
                "{} Successfully applied batch patches across {} file(s) ({} symbol(s))\n",
                "✔".green(),
                result.files_modified_count,
                result.symbols_patched_count
            ));
        } else if result.success {
            out.push_str(&format!(
                "{} Dry-run verified successfully for {} file(s) ({} symbol(s))\n",
                "ℹ".cyan(),
                result.files_modified_count,
                result.symbols_patched_count
            ));
        } else if result.rolled_back {
            out.push_str(&format!(
                "{} Batch refactor failed and was rolled back cleanly\n",
                "✖".red()
            ));
        } else {
            out.push_str(&format!(
                "{} Pre-write validation rejected the patch\n",
                "✖".red()
            ));
        }

        for diff in &result.diffs {
            out.push_str(&format!(
                "\n{} `{}` (symbols: `{}`)\n",
                "diff:".bold(),
                diff.file_path,
                diff.symbols_patched.join("`, `")
            ));
            out.push_str(&diff.diff);
            out.push('\n');
        }

        if !result.diagnostics.is_empty() {
            out.push_str("\nDiagnostics:\n");
            for diag in &result.diagnostics {
                out.push_str(&format!(
                    "- [{}] {}: {}\n",
                    diag.severity, diag.file_path, diag.message
                ));
            }
        }

        out
    };

    if let Some(out_file) = opts.output {
        fs::write(out_file, &rendered).with_context(|| {
            format!(
                "Failed to write batch refactor report to `{}`",
                out_file.display()
            )
        })?;
        println!(
            "{} Batch refactor report saved to `{}`",
            "✔".green(),
            out_file.display()
        );
    }

    if opts.clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(&rendered);
            eprintln!("{} Batch refactor report copied to clipboard!", "✔".green());
        }
    }

    if opts.output.is_none() {
        print!("{rendered}");
    }

    Ok(())
}
