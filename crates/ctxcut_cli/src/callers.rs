//! CLI handler for `ctxcut callers` subcommand.

use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::{ImpactAnalyzer, SliceOptions, TelemetryLogger};
use std::fs;
use std::path::{Path, PathBuf};

/// Executes the `ctxcut callers` command.
#[allow(clippy::too_many_arguments)]
pub fn run_callers_command(
    target: &str,
    path: Option<&Path>,
    root: Option<PathBuf>,
    budget: Option<usize>,
    limit: Option<usize>,
    clip: bool,
    output: Option<&Path>,
    format: &str,
) -> Result<()> {
    let workspace_root =
        root.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let opts = SliceOptions {
        depth: 1,
        budget,
        include_types: true,
        include_calls: true,
    };

    let mut result = ImpactAnalyzer::find_callers(&workspace_root, target, path, &opts)
        .with_context(|| format!("Failed to analyze upstream callers for `{target}`"))?;

    if let Some(lim) = limit {
        result.callers.truncate(lim);
        result.total_callers = result.callers.len();
    }

    let rendered = if format.eq_ignore_ascii_case("json") {
        result.to_json()
    } else {
        result.to_markdown()
    };

    if let Some(out_file) = output {
        fs::write(out_file, &rendered)
            .with_context(|| format!("Failed to write output to `{}`", out_file.display()))?;
        println!(
            "{} Caller impact analysis saved to `{}`",
            "✔".green(),
            out_file.display()
        );
    }

    if clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&rendered).is_ok() {
                eprintln!("{} Caller impact slice copied to clipboard!", "✔".green());
            }
        }
    }

    // Telemetry
    let saved_tokens = result
        .stats
        .raw_file_tokens
        .saturating_sub(result.stats.sliced_tokens);
    TelemetryLogger::record_operation(
        "callers",
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
