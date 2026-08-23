//! Persistent SQLite index CLI command handler (`ctxcut index`).

use anyhow::{Context, Result};
use colored::Colorize;
use ctxcut_core::{IndexEngine, IndexOptions, IndexStatus, IndexSyncResult};
use std::path::{Path, PathBuf};

/// CLI options for `index` command.
#[derive(Debug, Clone, Default)]
pub struct IndexCliOptions {
    /// Workspace root directory path.
    pub path: Option<PathBuf>,
    /// Force full rebuild from scratch.
    pub rebuild: bool,
    /// Display index status and metrics without syncing.
    pub status: bool,
    /// Clean and remove database files from disk.
    pub clean: bool,
    /// Format output as JSON.
    pub json: bool,
}

/// Executes `ctxcut index` command.
pub fn run_index_command(opts: IndexCliOptions) -> Result<()> {
    let ws_root = opts.path.unwrap_or_else(|| PathBuf::from("."));
    let ws_root = ws_root
        .canonicalize()
        .unwrap_or(ws_root);

    if opts.clean {
        IndexEngine::clean(&ws_root)?;
        if opts.json {
            println!(
                "{}",
                serde_json::json!({
                    "status": "cleaned",
                    "workspace": ws_root.display().to_string()
                })
            );
        } else {
            println!(
                "{} Persistent SQLite index cleaned from `{}`",
                "✔".green(),
                ws_root.join(".ctxcut").display()
            );
        }
        return Ok(());
    }

    let mut engine = IndexEngine::open_or_create(&ws_root)
        .with_context(|| format!("Failed to open index for workspace `{}`", ws_root.display()))?;

    if opts.status {
        let status = engine.status()?;
        if opts.json {
            println!("{}", serde_json::to_string_pretty(&status)?);
        } else {
            print_index_status(&status, &ws_root);
        }
        return Ok(());
    }

    let sync_opts = IndexOptions {
        rebuild: opts.rebuild,
        ..Default::default()
    };

    let result = engine.sync_incremental(&sync_opts)?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_sync_result(&result, opts.rebuild);
    }

    Ok(())
}

fn print_index_status(status: &IndexStatus, ws_root: &Path) {
    println!(
        "\n{} {}",
        "⚡ CTXCUT PERSISTENT INDEX STATUS".bold().cyan(),
        format!("[{}]", ws_root.display()).dimmed()
    );
    println!("{}", "─".repeat(60).dimmed());
    println!("  {:<25} {}", "Database File:".bold(), status.db_path.display());
    println!(
        "  {:<25} {}",
        "Storage Engine:".bold(),
        if status.is_wal_mode {
            "WAL (Write-Ahead Logging)".green()
        } else if status.in_memory {
            "In-Memory Fallback".yellow()
        } else {
            "Rollback Journal".normal()
        }
    );
    println!("  {:<25} {}", "Database Size:".bold(), format_bytes(status.db_size_bytes));
    println!("  {:<25} {}", "Total Files:".bold(), status.total_files.to_string().cyan());
    println!("  {:<25} {}", "Total Symbols:".bold(), status.total_symbols.to_string().green());
    println!("  {:<25} {}", "Call Sites:".bold(), status.total_callers.to_string().yellow());
    println!("  {:<25} {}", "Implementors:".bold(), status.total_implementors.to_string().magenta());
    println!(
        "  {:<25} {}",
        "Last Synchronized:".bold(),
        status
            .last_indexed_at
            .as_deref()
            .unwrap_or("Never")
            .dimmed()
    );
    println!("  {:<25} v{}", "Schema Version:".bold(), status.schema_version);
    println!("{}", "─".repeat(60).dimmed());
}

fn print_sync_result(res: &IndexSyncResult, rebuild: bool) {
    let action_str = if rebuild { "Rebuilt" } else { "Synchronized" };
    println!(
        "\n{} {} workspace index in {}ms",
        "✔".green().bold(),
        action_str,
        res.duration_ms
    );
    println!(
        "  {} added | {} updated | {} deleted | {} unchanged",
        res.files_added.to_string().green(),
        res.files_updated.to_string().yellow(),
        res.files_deleted.to_string().red(),
        res.files_unchanged.to_string().cyan()
    );
    println!(
        "  Total indexed symbols: {}",
        res.total_symbols.to_string().bold()
    );
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
