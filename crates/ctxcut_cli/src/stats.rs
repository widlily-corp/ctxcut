//! Repository and file token statistics calculator.

use anyhow::Result;
use ctxcut_core::{
    calculate_savings_percentage, count_lines, count_tokens, estimate_sliced_tokens, ContextSlicer,
    LanguageRegistry, ParserManager, ProjectWalker, SliceOptions, SupportedLanguage,
    TraversalConfig,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Summary report of token statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Calculates token statistics for a single file or directory.
/// When `fast` is true, performs a rapid heuristic scan without full AST symbol parsing.
pub fn calculate_stats(target_path: &Path, fast: bool) -> Result<StatsReport> {
    if fast {
        calculate_fast_stats(target_path)
    } else {
        calculate_deep_stats(target_path)
    }
}

/// Rapid token estimation scan mode using calibrated heuristic model.
pub fn calculate_fast_stats(target_path: &Path) -> Result<StatsReport> {
    if target_path.is_file() {
        let item = analyze_single_file_fast(target_path)?;
        let total_raw = item.raw_tokens;
        let total_sliced = item.sliced_tokens;
        let total_lines = item.lines;
        let savings = item.savings_percentage;

        return Ok(StatsReport {
            total_files: 1,
            total_raw_tokens: total_raw,
            total_sliced_tokens: total_sliced,
            savings_percentage: savings,
            total_lines,
            files: vec![item],
        });
    }

    let config = TraversalConfig::default();
    let file_paths = ProjectWalker::collect_files(target_path, &config);

    let mut file_items = Vec::new();
    let mut total_raw = 0;
    let mut total_sliced = 0;
    let mut total_lines = 0;

    for path in file_paths {
        if SupportedLanguage::from_path(&path).is_some() {
            if let Ok(item) = analyze_single_file_fast(&path) {
                total_raw += item.raw_tokens;
                total_sliced += item.sliced_tokens;
                total_lines += item.lines;
                file_items.push(item);
            }
        }
    }

    file_items.sort_by_key(|b| std::cmp::Reverse(b.raw_tokens));

    let savings_percentage = calculate_savings_percentage(total_raw, total_sliced);

    Ok(StatsReport {
        total_files: file_items.len(),
        total_raw_tokens: total_raw,
        total_sliced_tokens: total_sliced,
        savings_percentage,
        total_lines,
        files: file_items,
    })
}

/// Comprehensive deep scan mode performing full AST parsing and symbol slicing.
pub fn calculate_deep_stats(target_path: &Path) -> Result<StatsReport> {
    if target_path.is_file() {
        let item = analyze_single_file_deep(target_path)?;
        let total_raw = item.raw_tokens;
        let total_sliced = item.sliced_tokens;
        let total_lines = item.lines;
        let savings = item.savings_percentage;

        return Ok(StatsReport {
            total_files: 1,
            total_raw_tokens: total_raw,
            total_sliced_tokens: total_sliced,
            savings_percentage: savings,
            total_lines,
            files: vec![item],
        });
    }

    let config = TraversalConfig::default();
    let file_paths = ProjectWalker::collect_files(target_path, &config);

    let mut file_items = Vec::new();
    let mut total_raw = 0;
    let mut total_sliced = 0;
    let mut total_lines = 0;

    for path in file_paths {
        if SupportedLanguage::from_path(&path).is_some() {
            if let Ok(item) = analyze_single_file_deep(&path) {
                total_raw += item.raw_tokens;
                total_sliced += item.sliced_tokens;
                total_lines += item.lines;
                file_items.push(item);
            }
        }
    }

    file_items.sort_by_key(|b| std::cmp::Reverse(b.raw_tokens));

    let savings_percentage = calculate_savings_percentage(total_raw, total_sliced);

    Ok(StatsReport {
        total_files: file_items.len(),
        total_raw_tokens: total_raw,
        total_sliced_tokens: total_sliced,
        savings_percentage,
        total_lines,
        files: file_items,
    })
}

fn analyze_single_file_fast(file_path: &Path) -> Result<FileStatItem> {
    let source = fs::read_to_string(file_path)?;
    let lines = count_lines(&source);
    let raw_tokens = count_tokens(&source);
    let lang = SupportedLanguage::from_path(file_path);
    let sliced_tokens = estimate_sliced_tokens(raw_tokens, lines, lang);
    let savings_percentage = calculate_savings_percentage(raw_tokens, sliced_tokens);

    Ok(FileStatItem {
        path: file_path.to_string_lossy().to_string(),
        lines,
        raw_tokens,
        sliced_tokens,
        savings_percentage,
    })
}

fn analyze_single_file_deep(file_path: &Path) -> Result<FileStatItem> {
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
                let clean_name = sym.split('.').next_back().unwrap_or(sym);
                if let Ok(slice) = slicer.slice_symbol(file_path, clean_name, &opts) {
                    total_slice_tokens += slice.stats.sliced_tokens;
                    symbol_count += 1;
                }
            }
        }
    }

    let lang = SupportedLanguage::from_path(file_path);
    let avg_sliced_tokens = total_slice_tokens
        .checked_div(symbol_count)
        .unwrap_or_else(|| estimate_sliced_tokens(raw_tokens, lines, lang));

    let savings_percentage = calculate_savings_percentage(raw_tokens, avg_sliced_tokens);

    Ok(FileStatItem {
        path: file_path.to_string_lossy().to_string(),
        lines,
        raw_tokens,
        sliced_tokens: avg_sliced_tokens,
        savings_percentage,
    })
}

/// Formats a `StatsReport` into human-readable terminal table output.
#[must_use]
pub fn format_stats_text(report: &StatsReport) -> String {
    let mut out = String::new();
    out.push_str("\n📊 ctxcut Token Optimization & Context Statistics\n");
    out.push_str("======================================================\n");
    out.push_str(&format!("Total Files Analyzed: {}\n", report.total_files));
    out.push_str(&format!("Total Lines of Code:  {}\n", report.total_lines));
    out.push_str(&format!(
        "Full Context Tokens:  {} tokens\n",
        report.total_raw_tokens
    ));
    out.push_str(&format!(
        "Target Sliced Tokens: {} tokens\n",
        report.total_sliced_tokens
    ));
    out.push_str(&format!(
        "Estimated Savings:    {:.1}%\n",
        report.savings_percentage
    ));
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
