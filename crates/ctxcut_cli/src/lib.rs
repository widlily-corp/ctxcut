//! `ctxcut_cli` — Command-line interface for AST-based context slicing.

pub mod callers;
pub mod diff;
pub mod index;
pub mod metrics;
pub mod query;
pub mod refactor;
pub mod route;
pub mod semantic_diff;
pub mod setup_mcp;
pub mod stats;
pub mod trace;
pub mod tui;
pub mod upgrade;
pub mod verify;

pub use callers::run_callers_command;
pub use diff::{run_diff_slicer, run_diff_slicer_in};
pub use index::{run_index_command, IndexCliOptions};
pub use metrics::{render_dashboard, run_metrics_command};
pub use query::{run_query_command, QueryOptions};
pub use refactor::{run_refactor_rename, RefactorRenameOptions};
pub use semantic_diff::{run_semantic_diff, SemanticDiffOptions};
pub use setup_mcp::{
    format_setup_report, get_ide_config_paths, merge_mcp_config, run_setup_mcp, safe_merge_json,
    setup_ide_mcp, IdeTarget, MergeStatus, SetupMcpOptions, SetupResult,
};
pub use trace::run_trace_command;
pub use tui::run_tui;
pub use upgrade::{run_upgrade_command, UpgradeOptions};
pub use verify::{run_verify_patch, VerifyPatchOptions};

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

    /// Upstream reverse caller impact analysis across workspace.
    Callers {
        /// Target symbol name to trace callers for (e.g. `validate_token`, `AuthService.validate`).
        target: String,

        /// Optional path to the file declaring the target symbol.
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Workspace root directory path (defaults to current directory).
        #[arg(short, long)]
        root: Option<PathBuf>,

        /// Adaptive token budget limit for caller output.
        #[arg(long)]
        budget: Option<usize>,

        /// Maximum number of callers to return.
        #[arg(long)]
        limit: Option<usize>,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save extracted Markdown/JSON.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (markdown or json).
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// End-to-end execution flow tracer from entry point down to services and database sinks.
    Trace {
        /// Entry point query (e.g. `POST /api/v1/orders`, `main`, or `OrderController.createOrder`).
        entry: String,

        /// Workspace root directory path (defaults to current directory).
        #[arg(short, long)]
        root: Option<PathBuf>,

        /// Adaptive token budget limit (default: 1500 tokens).
        #[arg(long)]
        budget: Option<usize>,

        /// Maximum execution trace depth hops (default: 8).
        #[arg(long, default_value = "8")]
        depth: usize,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save extracted Markdown/JSON.
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

    /// Verify a patch using AST syntax validation and language typecheckers with RAII auto-rollback guard.
    #[command(name = "verify-patch")]
    VerifyPatch {
        /// Target symbol query in format `path/to/file.ts:symbolName`.
        target: String,

        /// Replacement code string or path to replacement code file.
        #[arg(long = "with")]
        with_code: Option<String>,

        /// Direct replacement code string.
        #[arg(short, long)]
        code: Option<String>,

        /// Path to file containing replacement code.
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Typechecker command override (e.g. `cargo check`, `tsc --noEmit`, `mypy`).
        #[arg(long, alias = "typecheck-cmd")]
        typechecker: Option<String>,

        /// Persist changes to disk if verification succeeds.
        #[arg(long)]
        apply: bool,

        /// Run in dry-run mode without writing changes to disk (default if --apply is not set).
        #[arg(long)]
        dry_run: bool,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save verification report.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (markdown or json).
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// Token-efficient structural AST diff calculating signature/type deltas & ROI savings.
    #[command(name = "semantic-diff")]
    SemanticDiff {
        /// Workspace directory path (defaults to current directory).
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Target specific file path for diffing.
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Inspect staged changes only (`git diff --staged`).
        #[arg(long)]
        staged: bool,

        /// Adaptive token budget limit.
        #[arg(long)]
        budget: Option<usize>,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save extracted diff.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (markdown or json).
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// AST-guided multi-file symbol refactoring and renaming.
    Refactor {
        /// Refactoring subcommand to execute.
        #[command(subcommand)]
        command: RefactorSubcommands,
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

    /// Manage persistent SQLite index (.ctxcut/index.db) for sub-5ms repository queries.
    Index {
        /// Workspace root directory path (defaults to current directory).
        path: Option<PathBuf>,

        /// Rebuild index from scratch, invalidating all cached data.
        #[arg(short, long)]
        rebuild: bool,

        /// Display index health, total symbols, and cache status without syncing.
        #[arg(short, long)]
        status: bool,

        /// Remove and delete index database files from disk.
        #[arg(long)]
        clean: bool,

        /// Output status or sync results in JSON format.
        #[arg(long)]
        json: bool,
    },

    /// Search workspace code using structural Tree-sitter AST queries or built-in presets.
    Query {
        /// Tree-sitter S-expression AST query pattern.
        pattern: Option<String>,

        /// Built-in query preset (e.g. functions, structs, classes, interfaces, enums, exports, async_fns, api_routes, errors, react-hooks).
        #[arg(short, long)]
        preset: Option<String>,

        /// Programming language filter (e.g. rust, typescript, python, go, c, cpp, csharp, java, kotlin).
        #[arg(short, long)]
        lang: Option<String>,

        /// Workspace root directory (defaults to current directory).
        #[arg(short, long)]
        root: Option<PathBuf>,

        /// Maximum number of matches to return.
        #[arg(long)]
        limit: Option<usize>,

        /// Output format (markdown or json).
        #[arg(long, default_value = "markdown")]
        format: String,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save query results.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Launch interactive Terminal UI (TUI) Dashboard & AST Context Studio.
    Tui {
        /// Workspace root directory path (defaults to current directory).
        path: Option<PathBuf>,
    },

    /// Alias for interactive TUI Dashboard (`ctxcut tui`).
    Dashboard {
        /// Workspace root directory path (defaults to current directory).
        path: Option<PathBuf>,
    },

    /// Check for updates and self-upgrade ctxcut to the latest release version.
    Upgrade {
        /// Check for updates without installing.
        #[arg(long)]
        check: bool,

        /// Upgrade to a specific version tag (e.g. `2.0.0` or `v2.0.0`).
        #[arg(long)]
        version: Option<String>,

        /// Force re-installation even if already at latest version.
        #[arg(long)]
        force: bool,

        /// Allow installing an older version (bypasses downgrade prevention).
        #[arg(long)]
        allow_downgrade: bool,
    },
}

/// Subcommands for AST-guided refactoring.
#[derive(Subcommand, Debug)]
pub enum RefactorSubcommands {
    /// Rename a symbol across the workspace with AST accuracy.
    Rename {
        /// Target symbol query (e.g. `src/calc.rs:calculate_tax` or `calculateTax`).
        target: String,

        /// New identifier name.
        #[arg(long)]
        to: String,

        /// Workspace root directory (defaults to current directory).
        #[arg(short, long)]
        root: Option<PathBuf>,

        /// Preview unified diff without writing changes to disk.
        #[arg(long)]
        dry_run: bool,

        /// Copy output directly to system clipboard.
        #[arg(long)]
        clip: bool,

        /// Output file path to save refactor report.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output format (text, markdown, or json).
        #[arg(long, default_value = "text")]
        format: String,
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
                fs::write(out_file, &rendered).with_context(|| {
                    format!("Failed to write overview to `{}`", out_file.display())
                })?;
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

        Some(Commands::Callers {
            target,
            path,
            root,
            budget,
            limit,
            clip,
            output,
            format,
        }) => run_callers_command(
            &target,
            path.as_deref(),
            root,
            budget,
            limit,
            clip,
            output.as_deref(),
            &format,
        ),

        Some(Commands::Trace {
            entry,
            root,
            budget,
            depth,
            clip,
            output,
            format,
        }) => run_trace_command(
            &entry,
            root,
            budget,
            depth,
            clip,
            output.as_deref(),
            &format,
        ),

        Some(Commands::VerifyPatch {
            target,
            with_code,
            code,
            file,
            typechecker,
            apply,
            dry_run,
            clip,
            output,
            format,
        }) => {
            let opts = VerifyPatchOptions {
                target: &target,
                with_code: with_code.as_deref(),
                code: code.as_deref(),
                file: file.as_deref(),
                typecheck_cmd: typechecker.as_deref(),
                apply,
                dry_run,
                clip,
                output: output.as_deref(),
                format: &format,
            };
            run_verify_patch(opts)
        }

        Some(Commands::SemanticDiff {
            path,
            file,
            staged,
            budget,
            clip,
            output,
            format,
        }) => {
            let opts = SemanticDiffOptions {
                root: path,
                file,
                staged,
                budget,
                clip,
                output,
                format,
            };
            run_semantic_diff(opts)
        }

        Some(Commands::Refactor {
            command:
                RefactorSubcommands::Rename {
                    target,
                    to,
                    root,
                    dry_run,
                    clip,
                    output,
                    format,
                },
        }) => {
            let opts = RefactorRenameOptions {
                target: &target,
                to: &to,
                root: root.as_deref(),
                dry_run,
                format: &format,
                clip,
                output: output.as_deref(),
            };
            run_refactor_rename(opts)
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

        Some(Commands::Index {
            path,
            rebuild,
            status,
            clean,
            json,
        }) => {
            let opts = IndexCliOptions {
                path,
                rebuild,
                status,
                clean,
                json,
            };
            run_index_command(opts)
        }

        Some(Commands::Query {
            pattern,
            preset,
            lang,
            root,
            limit,
            format,
            clip,
            output,
        }) => {
            let opts = QueryOptions {
                pattern,
                preset,
                lang,
                root,
                limit,
                format,
                clip,
                output,
            };
            run_query_command(&opts)
        }

        Some(Commands::Tui { path } | Commands::Dashboard { path }) => run_tui(path),

        Some(Commands::Upgrade {
            check,
            version,
            force,
            allow_downgrade,
        }) => {
            let opts = UpgradeOptions {
                check,
                version,
                force,
                allow_downgrade,
            };
            run_upgrade_command(&opts)
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
                hoisted_implementors: Vec::new(),
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
