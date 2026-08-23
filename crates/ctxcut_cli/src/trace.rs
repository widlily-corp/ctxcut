//! CLI handler for `ctxcut trace` subcommand.

use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::{ExecutionTracer, SliceOptions, TelemetryLogger};
use std::fs;
use std::path::{Path, PathBuf};

/// Executes the `ctxcut trace` command.
pub fn run_trace_command(
    entry: &str,
    root_dir: Option<PathBuf>,
    budget: Option<usize>,
    depth: usize,
    clip: bool,
    output: Option<&Path>,
    format: &str,
) -> Result<()> {
    let workspace_root =
        root_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let opts = SliceOptions {
        depth,
        budget,
        include_types: true,
        include_calls: true,
    };

    let trace_result = ExecutionTracer::trace(&workspace_root, entry, &opts)
        .with_context(|| format!("Failed to trace execution flow for `{entry}`"))?;

    let rendered = if format.eq_ignore_ascii_case("json") {
        trace_result.to_json()
    } else {
        trace_result.to_markdown()
    };

    if let Some(out_file) = output {
        fs::write(out_file, &rendered)
            .with_context(|| format!("Failed to write trace output to `{}`", out_file.display()))?;
        println!(
            "{} Execution trace saved to `{}`",
            "✔".green(),
            out_file.display()
        );
    }

    if clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&rendered).is_ok() {
                eprintln!("{} Execution trace copied to clipboard!", "✔".green());
            }
        }
    }

    // Telemetry
    let saved_tokens = trace_result
        .stats
        .raw_file_tokens
        .saturating_sub(trace_result.stats.sliced_tokens);
    TelemetryLogger::record_operation(
        "trace",
        &workspace_root.to_string_lossy(),
        trace_result.stats.raw_file_tokens,
        trace_result.stats.sliced_tokens,
        saved_tokens,
    );

    if output.is_none() {
        println!("{rendered}");
    }

    Ok(())
}
