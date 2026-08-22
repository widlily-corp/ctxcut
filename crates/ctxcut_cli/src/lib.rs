//! `ctxcut_cli` — Command-line interface for AST-based context slicing.

pub mod diff;
pub mod metrics;
pub mod route;
pub mod setup_mcp;
pub mod stats;

pub use diff::{run_diff_slicer, run_diff_slicer_in};
pub use metrics::{render_dashboard, run_metrics_command};
pub use setup_mcp::{
    format_setup_report, get_ide_config_paths, merge_mcp_config, run_setup_mcp, safe_merge_json,
    setup_ide_mcp, IdeTarget, MergeStatus, SetupMcpOptions, SetupResult,
};

use anyhow::{bail, Context, Result};
use arboard::Clipboard;
use clap::{Parser, Subcommand};
use colored::Colorize;
use ctxcut_core::{
    AstPatcher, ContextSlicer, MarkdownFormatter, SliceOptions, SliceResult, TelemetryLogger,
    TestContextGenerator,
};
use std::fs;
use std::path::{Path, PathBuf};

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

        /// Adaptive token budget limit (progressive semantic degradation).
        #[arg(long)]
        budget: Option<usize>,

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

    /// High-level workspace symbol indexing and architectural outline without parsing entire file bodies.
    Overview {
        /// Workspace root directory path (defaults to current directory).
        path: Option<PathBuf>,

        /// Maximum directory traversal depth limit.
        #[arg(long)]
        depth: Option<usize>,

        /// Adaptive token budget limit for compressed repository overview.
        #[arg(long)]
        budget: Option<usize>,

        /// Output format (markdown or json).
        #[arg(long, default_value = "markdown")]
        format: String,

        /// Include framework web route endpoints in overview.
        #[arg(long, default_value = "true")]
        include_routes: bool,

        /// Optional target framework filter (e.g. express, fastapi, actix).
        #[arg(long)]
        framework: Option<String>,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save extracted overview Markdown.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract slices for all functions modified in Git diff or staged changes.
    Diff {
        /// Inspect staged changes only (`git diff --staged`).
        #[arg(long)]
        staged: bool,

        /// Adaptive token budget limit.
        #[arg(long)]
        budget: Option<usize>,

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

    /// Surgically patch a function, method, or class in source code using AST boundary alignment.
    Patch {
        /// Target symbol query in format `path/to/file.ts:symbolName`.
        target: String,

        /// Replacement code string.
        #[arg(short, long)]
        code: Option<String>,

        /// Path to file containing replacement code.
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Preview unified diff without writing changes to disk.
        #[arg(long)]
        dry_run: bool,
    },

    /// Generate isolated unit test context with mock scaffolding and AAA test templates.
    #[command(name = "test-context")]
    TestContext {
        /// Target symbol query in format `path/to/file.ts:symbolName`.
        target: String,

        /// Test framework to scaffold for (e.g. vitest, jest, pytest, cargo, gotest).
        #[arg(long)]
        framework: Option<String>,

        /// Adaptive token budget limit.
        #[arg(long)]
        budget: Option<usize>,

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

        /// Enable fast heuristic token estimation scan without deep AST slicing.
        #[arg(short = 'f', long)]
        fast: bool,

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

        /// Adaptive token budget limit.
        #[arg(long)]
        budget: Option<usize>,

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

    /// Configure IDEs to use ctxcut as an MCP server.
    SetupMcp {
        /// Target IDE to configure (antigravity, claude, cursor, vscode, all).
        #[arg(long, default_value = "all")]
        ide: IdeTarget,

        /// Path to custom MCP JSON configuration file to update.
        #[arg(long)]
        custom_path: Option<PathBuf>,

        /// Configure project/workspace level MCP settings instead of global.
        #[arg(long)]
        workspace: bool,

        /// Workspace root directory path (defaults to current directory).
        #[arg(long)]
        workspace_dir: Option<PathBuf>,

        /// Remove/uninstall ctxcut from the MCP configuration instead of adding.
        #[arg(long)]
        remove: bool,

        /// Use absolute executable path instead of `ctxcut` binary name.
        #[arg(long)]
        use_absolute_path: bool,

        /// Preview changes without modifying configuration files on disk.
        #[arg(long)]
        dry_run: bool,
    },

    /// Initialize ctxcut and configure IDE MCP settings (alias for setup-mcp).
    Init {
        /// Target IDE to configure (antigravity, claude, cursor, vscode, all).
        #[arg(long, default_value = "all")]
        ide: IdeTarget,

        /// Path to custom MCP JSON configuration file to update.
        #[arg(long)]
        custom_path: Option<PathBuf>,

        /// Configure project/workspace level MCP settings instead of global.
        #[arg(long)]
        workspace: bool,

        /// Workspace root directory path (defaults to current directory).
        #[arg(long)]
        workspace_dir: Option<PathBuf>,

        /// Use absolute executable path instead of `ctxcut` binary name.
        #[arg(long)]
        use_absolute_path: bool,

        /// Preview changes without modifying configuration files on disk.
        #[arg(long)]
        dry_run: bool,
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
            budget,
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
                budget,
            };

            handle_slice_and_output(&target, &opts, &format, clip, output.as_deref())
        }

        Some(Commands::Overview {
            path,
            depth,
            budget,
            format,
            include_routes,
            framework,
            clip,
            output,
        }) => {
            let target_root = path.unwrap_or_else(|| PathBuf::from("."));
            let opts = ctxcut_core::OverviewOptions {
                budget,
                max_depth: depth,
                include_routes,
                framework,
            };
            let report = ctxcut_core::WorkspaceOverviewGenerator::generate(&target_root, &opts)?;
            let rendered = if format.eq_ignore_ascii_case("json") {
                report.to_json()
            } else {
                report.to_markdown()
            };

            if let Some(out_file) = output.as_deref() {
                fs::write(out_file, &rendered)
                    .with_context(|| format!("Failed to write overview to `{}`", out_file.display()))?;
                println!(
                    "{} Workspace overview saved to `{}`",
                    "✔".green(),
                    out_file.display()
                );
            }
            if clip {
                if let Ok(mut clipboard) = Clipboard::new() {
                    let _ = clipboard.set_text(&rendered);
                    eprintln!("{} Workspace overview copied to clipboard!", "✔".green());
                }
            }
            if output.is_none() {
                println!("{rendered}");
            }
            Ok(())
        }

        Some(Commands::Diff {
            staged,
            budget,
            clip,
            output,
            format,
        }) => {
            let opts = SliceOptions {
                budget,
                ..Default::default()
            };
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

        Some(Commands::Patch {
            target,
            code,
            file,
            dry_run,
        }) => {
            let (file_part, symbol_part) = parse_target(&target)
                .context("Invalid target format. Expected `<file_path>:<symbol_name>` (e.g. `src/calc.rs:add`)")?;
            let file_path = Path::new(file_part);

            let replacement = if let Some(c) = code {
                c
            } else if let Some(f) = file {
                fs::read_to_string(&f)
                    .with_context(|| format!("Failed to read patch file `{}`", f.display()))?
            } else {
                bail!("Missing replacement code. Provide `--code <CODE>` or `--file <PATH>`");
            };

            let patch_result =
                AstPatcher::patch_symbol(file_path, symbol_part, &replacement, dry_run)?;
            if dry_run {
                println!("{}", patch_result.diff);
                println!(
                    "{}",
                    "Dry run complete. No changes written to disk.".yellow()
                );
            } else {
                println!("{}", patch_result.diff);
                println!(
                    "{} Successfully patched `{}` in `{}`",
                    "✔".green(),
                    symbol_part,
                    file_path.display()
                );
            }
            Ok(())
        }

        Some(Commands::TestContext {
            target,
            framework,
            budget,
            clip,
            output,
            format,
        }) => {
            let (file_part, symbol_part) = parse_target(&target)
                .context("Invalid target format. Expected `<file_path>:<symbol_name>` (e.g. `src/orders.ts:payOrder`)")?;
            let file_path = Path::new(file_part);
            let opts = SliceOptions {
                budget,
                ..Default::default()
            };

            let test_ctx = TestContextGenerator::generate(
                file_path,
                symbol_part,
                framework.as_deref(),
                &opts,
            )?;
            let rendered = if format.eq_ignore_ascii_case("json") {
                test_ctx.to_json()
            } else {
                test_ctx.to_markdown()
            };

            if let Some(out_file) = output.as_deref() {
                fs::write(out_file, &rendered)?;
                println!(
                    "{} Test context saved to `{}`",
                    "✔".green(),
                    out_file.display()
                );
            }
            if clip {
                if let Ok(mut clipboard) = Clipboard::new() {
                    let _ = clipboard.set_text(&rendered);
                }
            }
            if output.is_none() {
                println!("{rendered}");
            }
            Ok(())
        }

        Some(Commands::Stats {
            path,
            format,
            fast,
            history,
        }) => {
            if history {
                run_metrics_command(&format)?;
            } else if let Some(target_path) = path {
                let report = stats::calculate_stats(&target_path, fast)?;
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
            budget,
            clip,
            output,
            format,
        }) => {
            let opts = SliceOptions {
                budget,
                ..Default::default()
            };
            let current_dir = std::env::current_dir()?;
            let result = route::resolve_route_slice(&current_dir, &method, &route_path, &opts)?;
            TelemetryLogger::record_slice(&result, "cli_route", None);
            handle_output(&[result], &format, clip, output.as_deref())
        }

        Some(Commands::SetupMcp {
            ide,
            custom_path,
            workspace,
            workspace_dir,
            remove,
            use_absolute_path,
            dry_run,
        }) => {
            let options = SetupMcpOptions {
                ide,
                custom_path,
                workspace,
                workspace_dir,
                remove,
                use_absolute_path,
                dry_run,
            };
            let results = run_setup_mcp(&options)?;
            print!("{}", format_setup_report(&results));
            Ok(())
        }

        Some(Commands::Init {
            ide,
            custom_path,
            workspace,
            workspace_dir,
            use_absolute_path,
            dry_run,
        }) => {
            let options = SetupMcpOptions {
                ide,
                custom_path,
                workspace,
                workspace_dir,
                remove: false,
                use_absolute_path,
                dry_run,
            };
            let results = run_setup_mcp(&options)?;
            print!("{}", format_setup_report(&results));
            Ok(())
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

fn parse_target(target: &str) -> Option<(&str, &str)> {
    let search_start = if target.len() >= 2
        && target.as_bytes()[1] == b':'
        && target.as_bytes()[0].is_ascii_alphabetic()
    {
        2
    } else {
        0
    };
    let colon_idx = target[search_start..].find(':')? + search_start;
    Some((&target[..colon_idx], &target[colon_idx + 1..]))
}

fn handle_slice_and_output(
    target: &str,
    opts: &SliceOptions,
    format: &str,
    clip: bool,
    output_path: Option<&Path>,
) -> Result<()> {
    let (file_part, symbol_part) = parse_target(target)
        .context("Invalid target format. Expected `<file_path>:<symbol_name>` (e.g. `src/orders.ts:payOrder` or `src/calc.ts:add,multiply`)")?;

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
    let rendered = if symbols.len() > 1 {
        let batch = slicer.slice_batch(file_path, &symbols, opts)?;
        for sym in &batch.target_symbols {
            let single_slice = SliceResult {
                target_symbol: sym.clone(),
                hoisted_types: Vec::new(),
                stripped_calls: Vec::new(),
                stats: batch.stats.clone(),
            };
            TelemetryLogger::record_slice(&single_slice, "cli_slice", None);
        }

        if format.eq_ignore_ascii_case("json") {
            batch.to_json()
        } else {
            batch.to_markdown()
        }
    } else {
        let single = slicer.slice_symbol(file_path, symbols[0], opts)?;
        TelemetryLogger::record_slice(&single, "cli_slice", None);
        if format.eq_ignore_ascii_case("json") {
            single.to_json()
        } else {
            single.to_markdown()
        }
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
