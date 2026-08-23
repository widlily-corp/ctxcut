//! Verification Guard CLI handler (`ctxcut verify-patch`).

use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::verify::PatchVerifier;
use std::fs;
use std::path::Path;

/// Options for `verify-patch` command.
#[derive(Debug, Clone, Copy)]
pub struct VerifyPatchOptions<'a> {
    /// Target symbol query in format `path/to/file.ts:symbolName`.
    pub target: &'a str,
    /// Replacement code from argument.
    pub with_code: Option<&'a str>,
    /// Direct replacement code.
    pub code: Option<&'a str>,
    /// Path to file containing replacement code.
    pub file: Option<&'a Path>,
    /// Optional typechecker command override (e.g. `cargo check`, `npx tsc --noEmit`).
    pub typecheck_cmd: Option<&'a str>,
    /// Persist changes to disk if verification succeeds.
    pub apply: bool,
    /// Dry run mode.
    pub dry_run: bool,
    /// Copy output to clipboard.
    pub clip: bool,
    /// Output file path to save report.
    pub output: Option<&'a Path>,
    /// Output format (markdown or json).
    pub format: &'a str,
}

/// Executes `verify-patch` command.
pub fn run_verify_patch(opts: VerifyPatchOptions) -> Result<()> {
    let replacement = if let Some(w) = opts.with_code {
        let p = Path::new(w);
        if p.exists() && p.is_file() {
            fs::read_to_string(p)
                .with_context(|| format!("Failed to read replacement code from `{}`", p.display()))?
        } else {
            w.to_string()
        }
    } else if let Some(c) = opts.code {
        c.to_string()
    } else if let Some(f) = opts.file {
        fs::read_to_string(f)
            .with_context(|| format!("Failed to read replacement code from `{}`", f.display()))?
    } else {
        bail!("Missing replacement code. Provide `--with <CODE_OR_FILE>`, `--code <CODE>`, or `--file <PATH>`");
    };

    let dry_run = !opts.apply;

    let current_dir = std::env::current_dir()?;
    let result = PatchVerifier::verify_patch(
        &current_dir,
        opts.target,
        &replacement,
        opts.typecheck_cmd,
        dry_run,
    )?;

    let rendered = if opts.format.eq_ignore_ascii_case("json") {
        result.to_json()
    } else {
        result.to_markdown()
    };

    if let Some(out_file) = opts.output {
        fs::write(out_file, &rendered)
            .with_context(|| format!("Failed to write verification report to `{}`", out_file.display()))?;
        println!("{} Verification report saved to `{}`", "✔".green(), out_file.display());
    }

    if opts.clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(&rendered);
            eprintln!("{} Verification report copied to clipboard!", "✔".green());
        }
    }

    if opts.output.is_none() {
        println!("{rendered}");
    }

    if !result.success {
        bail!("Patch verification failed");
    }

    Ok(())
}
