//! CLI handler for `ctxcut pack-agent-context` subcommand (R4 Swarm Context Partitioning).

use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use ctxcut_core::{DefaultSwarmPartitioner, SwarmPartitionEngine, TelemetryLogger};
use std::fs;
use std::path::{Path, PathBuf};

/// Executes the `ctxcut pack-agent-context` command.
pub fn run_pack_agent_context_command(
    root_dir: Option<PathBuf>,
    agents_count: Option<usize>,
    seeds: Option<&str>,
    budget: Option<usize>,
    clip: bool,
    output: Option<&Path>,
    format: &str,
) -> Result<()> {
    let workspace_root =
        root_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let seed_symbols: Vec<String> = seeds
        .map(|s| {
            s.split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let agents = agents_count.unwrap_or(2).max(1);

    let partitioner = DefaultSwarmPartitioner::new();
    let manifest = partitioner
        .partition_workspace(&workspace_root, agents, &seed_symbols, budget)
        .with_context(|| {
            format!(
                "Failed to partition workspace context into {} agent packs",
                agents
            )
        })?;

    let rendered = if format.eq_ignore_ascii_case("json") {
        manifest.to_json()
    } else {
        manifest.to_markdown()
    };

    if let Some(out_file) = output {
        fs::write(out_file, &rendered).with_context(|| {
            format!(
                "Failed to write swarm partition manifest to `{}`",
                out_file.display()
            )
        })?;
        println!(
            "{} Swarm partition manifest saved to `{}`",
            "✔".green(),
            out_file.display()
        );
    }

    if clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&rendered).is_ok() {
                eprintln!(
                    "{} Swarm partition manifest copied to clipboard!",
                    "✔".green()
                );
            }
        }
    }

    // Record Telemetry
    let total_raw: usize = manifest.packs.iter().map(|p| p.token_stats.raw_file_tokens).sum();
    let total_sliced: usize = manifest.packs.iter().map(|p| p.token_stats.sliced_tokens).sum();
    let saved_tokens = total_raw.saturating_sub(total_sliced);
    TelemetryLogger::record_operation(
        "pack_agent_context",
        &workspace_root.to_string_lossy(),
        total_raw,
        total_sliced,
        saved_tokens,
    );

    if output.is_none() {
        println!("{rendered}");
    }

    Ok(())
}
