//! CLI handler for `ctxcut slice-intent` subcommand (R2 Semantic Intent-Driven AST Slicing).

use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::{DefaultIntentSlicer, IntentSliceOptions, IntentSlicer, TelemetryLogger};
use std::fs;
use std::path::Path;

/// CLI options for `slice-intent` command.
#[derive(Debug, Clone, Copy)]
pub struct SliceIntentCliOptions<'a> {
    /// Natural language prompt query.
    pub prompt: &'a str,
    /// Root directory path.
    pub root_dir: Option<&'a Path>,
    /// Token budget limit.
    pub budget: Option<usize>,
    /// Max target symbols to extract.
    pub max_symbols: Option<usize>,
    /// Traversal depth.
    pub depth: Option<usize>,
    /// Copy output to clipboard.
    pub clip: bool,
    /// Output file path to save extracted Markdown/JSON.
    pub output: Option<&'a Path>,
    /// Output format.
    pub format: &'a str,
}

/// Executes the `ctxcut slice-intent` command.
pub fn run_slice_intent_command(opts: SliceIntentCliOptions<'_>) -> Result<()> {
    let workspace_root = opts.root_dir.unwrap_or_else(|| Path::new("."));

    let intent_opts = IntentSliceOptions {
        prompt: opts.prompt.to_string(),
        budget: opts.budget,
        max_target_symbols: opts.max_symbols.unwrap_or(5),
        depth: opts.depth.unwrap_or(1),
    };

    let slicer = DefaultIntentSlicer::new();
    let result = slicer
        .slice_intent(workspace_root, &intent_opts)
        .with_context(|| format!("Failed to extract semantic intent slice for \"{}\"", opts.prompt))?;

    let rendered = if opts.format.eq_ignore_ascii_case("json") {
        result.to_json()
    } else {
        result.to_markdown()
    };

    if let Some(out_file) = opts.output {
        fs::write(out_file, &rendered).with_context(|| {
            format!(
                "Failed to write intent slice output to `{}`",
                out_file.display()
            )
        })?;
        println!(
            "{} Intent context slice saved to `{}`",
            "✔".green(),
            out_file.display()
        );
    }

    if opts.clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&rendered).is_ok() {
                eprintln!(
                    "{} Intent context slice copied to clipboard!",
                    "✔".green()
                );
            }
        }
    }

    // Record Telemetry
    let saved_tokens = result
        .stats
        .raw_file_tokens
        .saturating_sub(result.stats.sliced_tokens);
    TelemetryLogger::record_operation(
        "slice_intent",
        &workspace_root.to_string_lossy(),
        result.stats.raw_file_tokens,
        result.stats.sliced_tokens,
        saved_tokens,
    );

    if opts.output.is_none() {
        println!("{rendered}");
    }

    Ok(())
}
