//! CLI handler for `ctxcut trace-api` subcommand (R1 Full-Stack Cross-Boundary Execution Tracing).

use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::{FullstackExecutionTracer, TelemetryLogger};
use std::fs;
use std::path::{Path, PathBuf};

/// Executes the `ctxcut trace-api` command.
pub fn run_trace_api_command(
    entry: &str,
    root_dir: Option<PathBuf>,
    budget: Option<usize>,
    depth: Option<usize>,
    clip: bool,
    output: Option<&Path>,
    format: &str,
) -> Result<()> {
    let workspace_root =
        root_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let tracer = FullstackExecutionTracer::new();
    let result = tracer
        .trace_api_with_depth(&workspace_root, entry, budget, depth)
        .with_context(|| format!("Failed to trace cross-boundary execution for `{entry}`"))?;

    let rendered = if format.eq_ignore_ascii_case("json") {
        result.to_json()
    } else {
        result.to_markdown()
    };

    if let Some(out_file) = output {
        fs::write(out_file, &rendered).with_context(|| {
            format!(
                "Failed to write fullstack trace output to `{}`",
                out_file.display()
            )
        })?;
        println!(
            "{} Full-stack execution trace saved to `{}`",
            "✔".green(),
            out_file.display()
        );
    }

    if clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&rendered).is_ok() {
                eprintln!(
                    "{} Full-stack execution trace copied to clipboard!",
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
        "trace_api",
        &workspace_root.to_string_lossy(),
        result.stats.raw_file_tokens,
        result.stats.sliced_tokens,
        saved_tokens,
    );

    if output.is_none() {
        println!("{rendered}");
    }

    Ok(())
}
