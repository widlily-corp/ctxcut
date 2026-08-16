//! `ctxcut_cli` — Command-line interface for AST-based context slicing.

pub mod diff;
pub mod metrics;
pub mod route;
pub mod stats;

pub use diff::{run_diff_slicer, run_diff_slicer_in};
pub use metrics::{render_dashboard, run_metrics_command};

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use clap::{Parser, Subcommand};
use colored::Colorize;
use ctxcut_core::{ContextSlicer, MarkdownFormatter, SliceOptions, SliceResult, TelemetryLogger};

/// High-performance AST-based context slicer for LLMs and AI coding agents.
#[derive(Parser, Debug)]
#[command(name = "ctxcut", version, about = "AST-powered contextual code slicer")]
pub struct Cli {
    /// Launch Model Context Protocol (MCP) server over STDIO.
    #[arg(long, global = true)]
    pub mcp: bool,

    /// Optional log file path for structured JSONL observability in MCP mode.
    #[arg(long, global = true, env = "CTXCUT_LOG_FILE")]
    pub log_file: Option<PathBuf>,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Options passed to the MCP server runner.
#[derive(Debug, Clone, Default)]
pub struct McpOptions {
    /// Optional log file path for structured JSONL observability.
    pub log_file: Option<PathBuf>,
}

/// Supported CLI subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Extract minimal AST context slice for target symbol(s).
    Slice {
        /// Target symbol query in format `path/to/file.ts:symbolName` or `path/to/file.ts:sym1,sym2`.
        target: String,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save extracted Markdown.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (markdown or json).
        #[arg(long, default_value = "markdown")]
        format: String,

        /// Type hoisting recursion depth.
        #[arg(long, default_value = "1")]
        depth: usize,

        /// Disable type hoisting.
        #[arg(long)]
        no_types: bool,

        /// Disable signature stripping for external calls.
        #[arg(long)]
        no_calls: bool,
    },

    /// Extract slices for all functions modified in Git diff or staged changes.
    Diff {
        /// Inspect staged changes only (`git diff --staged`).
        #[arg(long)]
        staged: bool,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save extracted Markdown.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (markdown or json).
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// Analyze repository or file token savings and optimization statistics, or view persistent history.
    Stats {
        /// File or directory path to scan (optional when --history is provided).
        path: Option<PathBuf>,

        /// Output format (text or json).
        #[arg(long, default_value = "text")]
        format: String,

        /// Display persistent lifetime telemetry history and ROI dashboard.
        #[arg(long)]
        history: bool,
    },

    /// Display persistent lifetime token savings and ROI metrics dashboard.
    Metrics {
        /// Output format (text or json).
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Resolve web framework route handler (Express, FastAPI, Gin, Axum).
    Route {
        /// HTTP Method (GET, POST, PUT, DELETE, etc.).
        method: String,

        /// Route URL path (e.g. `/api/v1/checkout`).
        path: String,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save extracted Markdown.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (markdown or json).
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// Launch Model Context Protocol (MCP) server over STDIO.
    Mcp {
        /// Optional log file path for structured JSONL observability.
        #[arg(long, env = "CTXCUT_LOG_FILE")]
        log_file: Option<PathBuf>,
    },
}

/// Executes the CLI application with a custom MCP server runner.
pub fn run_cli_handler<F>(mcp_runner: F) -> Result<()>
where
    F: FnOnce(McpOptions) -> Result<()>,
{
    let cli = Cli::parse();

    if cli.mcp {
        return mcp_runner(McpOptions {
            log_file: cli.log_file,
        });
    }

    match cli.command {
        Some(Commands::Mcp { log_file }) => {
            let resolved_log = log_file.or(cli.log_file);
            mcp_runner(McpOptions {
                log_file: resolved_log,
            })
        }

        Some(Commands::Slice {
            target,
            clip,
            output,
            format,
            depth,
            no_types,
            no_calls,
        }) => {
            let opts = SliceOptions {
                depth,
                include_types: !no_types,
                include_calls: !no_calls,
            };

            let results = handle_slice_command(&target, &opts)?;
            handle_output(&results, &format, clip, output.as_deref())
        }

        Some(Commands::Diff {
            staged,
            clip,
            output,
            format,
        }) => {
            let opts = SliceOptions::default();
            let results = run_diff_slicer(staged, &opts)?;

            for slice in &results {
                TelemetryLogger::record_slice(slice, "cli_diff", None);
            }

            if results.is_empty() {
                println!("No modified symbols detected in git diff.");
            } else {
                handle_output(&results, &format, clip, output.as_deref())?;
            }
            Ok(())
        }

        Some(Commands::Stats {
            path,
            format,
            history,
        }) => {
            if history {
                run_metrics_command(&format)?;
            } else if let Some(target_path) = path {
                let report = stats::calculate_stats(&target_path)?;
                if format.eq_ignore_ascii_case("json") {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!("{}", stats::format_stats_text(&report));
                }
            } else {
                bail!("Missing required argument `<PATH>`. Usage: `ctxcut stats <PATH>` or `ctxcut stats --history`");
            }
            Ok(())
        }

        Some(Commands::Metrics { format }) => {
            run_metrics_command(&format)?;
            Ok(())
        }

        Some(Commands::Route {
            method,
            path: route_path,
            clip,
            output,
            format,
        }) => {
            let opts = SliceOptions::default();
            let current_dir = std::env::current_dir()?;
            let result = route::resolve_route_slice(&current_dir, &method, &route_path, &opts)?;
            TelemetryLogger::record_slice(&result, "cli_route", None);
            handle_output(&[result], &format, clip, output.as_deref())
        }

        None => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
    }
}

/// Executes the CLI application.
pub fn run_cli() -> Result<()> {
    run_cli_handler(|opts| {
        eprintln!(
            "{} MCP server runner invoked without stdio handler. Use the `ctxcut` binary.",
            "Error:".red().bold()
        );
        let _ = opts;
        Ok(())
    })
}

fn handle_slice_command(target: &str, opts: &SliceOptions) -> Result<Vec<SliceResult>> {
    let (file_part, symbol_part) = target
        .rsplit_once(':')
        .context("Invalid target format. Expected `<file_path>:<symbol_name>` (e.g. `src/orders.ts:payOrder`)")?;

    let file_path = Path::new(file_part);
    if !file_path.exists() {
        bail!("Source file not found: `{}`", file_path.display());
    }

    let symbols: Vec<&str> = symbol_part
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if symbols.is_empty() {
        bail!("No symbol name specified in target: `{}`", target);
    }

    let slicer = ContextSlicer::new();
    let results = slicer.slice_symbols(file_path, &symbols, opts)?;
    for slice in &results {
        TelemetryLogger::record_slice(slice, "cli_slice", None);
    }
    Ok(results)
}

fn handle_output(
    results: &[SliceResult],
    format: &str,
    clip: bool,
    output_path: Option<&Path>,
) -> Result<()> {
    let rendered = if format.eq_ignore_ascii_case("json") {
        if results.len() == 1 {
            results[0].to_json()
        } else {
            serde_json::to_string_pretty(results)?
        }
    } else {
        MarkdownFormatter::format_batch(results)
    };

    if let Some(out_file) = output_path {
        fs::write(out_file, &rendered)
            .with_context(|| format!("Failed to write output to `{}`", out_file.display()))?;
        println!(
            "{} Sliced context saved to `{}`",
            "✔".green(),
            out_file.display()
        );
    }

    if clip {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&rendered).is_ok() {
                eprintln!("{} Context slice copied to clipboard!", "✔".green());
            }
        }
    }

    if output_path.is_none() {
        println!("{rendered}");
    }

    Ok(())
}
