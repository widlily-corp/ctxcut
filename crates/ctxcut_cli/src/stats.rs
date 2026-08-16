//! Repository and file token statistics calculator.

use std::fs;
use std::path::Path;
use anyhow::Result;
use ctxcut_core::{count_lines, count_tokens, ContextSlicer, LanguageRegistry, ParserManager, SliceOptions, SupportedLanguage};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

/// Summary report of token statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsReport {
    /// Total number of source files analyzed.
    pub total_files: usize,
    /// Total raw tokens across all analyzed files.
    pub total_raw_tokens: usize,
    /// Total estimated slice tokens.
    pub total_sliced_tokens: usize,
    /// Percentage savings: `(1.0 - sliced/raw) * 100.0`.
    pub savings_percentage: f64,
    /// Total lines of code in raw files.
    pub total_lines: usize,
    /// Detailed stats per file.
    pub files: Vec<FileStatItem>,
}

/// Statistics for an individual file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatItem {
    /// Relative or absolute path to the file.
    pub path: String,
    /// Lines in raw source file.
    pub lines: usize,
    /// Raw BPE token count.
    pub raw_tokens: usize,
    /// Sliced BPE token count.
    pub sliced_tokens: usize,
    /// Token reduction percentage.
    pub savings_percentage: f64,
}

/// Calculates token statistics for a single file or an entire directory.
pub fn calculate_stats(target_path: &Path) -> Result<StatsReport> {
    if target_path.is_file() {
        let item = analyze_single_file(target_path)?;
        let total_raw = item.raw_tokens;
        let total_sliced = item.sliced_tokens;
        let total_lines = item.lines;
        let savings = item.savings_percentage;

        Ok(StatsReport {
            total_files: 1,
            total_raw_tokens: total_raw,
            total_sliced_tokens: total_sliced,
            savings_percentage: savings,
            total_lines,
            files: vec![item],
        })
    } else {
        let mut file_items = Vec::new();
        let mut total_raw = 0;
        let mut total_sliced = 0;
        let mut total_lines = 0;

        let walker = WalkBuilder::new(target_path)
            .hidden(true)
            .parents(true)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_file() && SupportedLanguage::from_path(path).is_some() {
                if let Ok(item) = analyze_single_file(path) {
                    total_raw += item.raw_tokens;
                    total_sliced += item.sliced_tokens;
                    total_lines += item.lines;
                    file_items.push(item);
                }
            }
        }

        #[allow(clippy::cast_precision_loss)]
        let savings_percentage = if total_raw == 0 || total_sliced >= total_raw {
            0.0
        } else {
            let pct = ((total_raw - total_sliced) as f64 / total_raw as f64) * 100.0;
            (pct * 100.0).round() / 100.0
        };

        Ok(StatsReport {
            total_files: file_items.len(),
            total_raw_tokens: total_raw,
            total_sliced_tokens: total_sliced,
            savings_percentage,
            total_lines,
            files: file_items,
        })
    }
}

fn analyze_single_file(file_path: &Path) -> Result<FileStatItem> {
    let source = fs::read_to_string(file_path)?;
    let lines = count_lines(&source);
    let raw_tokens = count_tokens(&source);

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    let mut total_slice_tokens = 0;
    let mut symbol_count = 0;

    if let Ok(adapter) = LanguageRegistry::for_path(file_path) {
        let ts_lang = adapter.tree_sitter_language(file_path);
        if let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, file_path) {
            let root = tree.root_node();
            let symbols = adapter.list_symbols(root, &source);

            for sym in symbols.iter().take(5) {
                let clean_name = sym.split('.').last().unwrap_or(sym);
                if let Ok(slice) = slicer.slice_symbol(file_path, clean_name, &opts) {
                    total_slice_tokens += slice.stats.sliced_tokens;
                    symbol_count += 1;
                }
            }
        }
    }

    let avg_sliced_tokens = if symbol_count > 0 {
        total_slice_tokens / symbol_count
    } else {
        // Fallback for one-liners / minimal files
        (raw_tokens / 5).max(1).min(raw_tokens)
    };

    #[allow(clippy::cast_precision_loss)]
    let savings_percentage = if raw_tokens == 0 || avg_sliced_tokens >= raw_tokens {
        0.0
    } else {
        let pct = ((raw_tokens - avg_sliced_tokens) as f64 / raw_tokens as f64) * 100.0;
        (pct * 100.0).round() / 100.0
    };

    Ok(FileStatItem {
        path: file_path.to_string_lossy().to_string(),
        lines,
        raw_tokens,
        sliced_tokens: avg_sliced_tokens,
        savings_percentage,
    })
}

/// Formats a `StatsReport` into human-readable terminal table output.
pub fn format_stats_text(report: &StatsReport) -> String {
    let mut out = String::new();
    out.push_str("\n📊 ctxcut Token Optimization & Context Statistics\n");
    out.push_str("======================================================\n");
    out.push_str(&format!("Total Files Analyzed: {}\n", report.total_files));
    out.push_str(&format!("Total Lines of Code:  {}\n", report.total_lines));
    out.push_str(&format!("Full Context Tokens:  {} tokens\n", report.total_raw_tokens));
    out.push_str(&format!("Target Sliced Tokens: {} tokens\n", report.total_sliced_tokens));
    out.push_str(&format!("Estimated Savings:    {:.1}%\n", report.savings_percentage));
    out.push_str("======================================================\n");

    if !report.files.is_empty() {
        out.push_str("\nTop File Breakdown:\n");
        for f in report.files.iter().take(10) {
            out.push_str(&format!(
                "  • {:<40} {:>5} lines | {:>6} -> {:>5} tokens ({:.1}%)\n",
                f.path, f.lines, f.raw_tokens, f.sliced_tokens, f.savings_percentage
            ));
        }
    }

    out
}
