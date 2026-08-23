//! Fast repo-wide token estimation scanner and data models.

use crate::error::{CoreError, Result};
use crate::model::SupportedLanguage;
use crate::tokenizer::{calculate_savings_percentage, count_lines, count_tokens};
use crate::traversal::config::TraversalConfig;
use crate::traversal::walker::ProjectWalker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

/// Aggregated fast token estimation report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FastStatsReport {
    /// Total source files analyzed.
    pub total_files: usize,
    /// Total lines of code across all files.
    pub total_lines: usize,
    /// Estimated raw BPE token count.
    pub estimated_raw_tokens: usize,
    /// Estimated sliced BPE token count.
    pub estimated_sliced_tokens: usize,
    /// Estimated percentage savings.
    pub estimated_savings_percentage: f64,
    /// Language breakdown distribution.
    pub language_breakdown: Vec<LanguageStatItem>,
    /// Duration of the scan in milliseconds.
    pub scan_duration_ms: u64,
    /// Per-file statistical breakdown.
    pub files: Vec<FastFileStatItem>,
}

/// Language token and line statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageStatItem {
    /// Language identifier string.
    pub language: String,
    /// Number of files in this language.
    pub file_count: usize,
    /// Total lines of code.
    pub total_lines: usize,
    /// Estimated total raw tokens.
    pub estimated_tokens: usize,
}

/// Per-file fast statistics item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastFileStatItem {
    /// Path to the file.
    pub path: String,
    /// Detected programming language if supported.
    pub language: Option<String>,
    /// Number of lines.
    pub lines: usize,
    /// Estimated raw token count.
    pub estimated_tokens: usize,
    /// Estimated sliced token count.
    pub estimated_sliced_tokens: usize,
    /// File size in bytes.
    pub file_size_bytes: u64,
}

/// Calibrated estimation of sliced token count given raw tokens and language.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn estimate_sliced_tokens(
    raw_tokens: usize,
    _lines: usize,
    lang: Option<SupportedLanguage>,
) -> usize {
    if raw_tokens == 0 {
        return 0;
    }
    if raw_tokens <= 20 {
        return raw_tokens;
    }
    let lang_weight = match lang {
        Some(SupportedLanguage::Rust | SupportedLanguage::Cpp) => 1.05,
        Some(
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro
            | SupportedLanguage::C,
        )
        | None => 1.00,
        Some(SupportedLanguage::CSharp | SupportedLanguage::Java) => 1.08,
        Some(SupportedLanguage::Kotlin) => 1.03,
        Some(SupportedLanguage::Python) => 0.95,
        Some(SupportedLanguage::Go) => 1.02,
    };
    let n = raw_tokens as f64;
    let base = 3.2 * n.powf(0.55) + 25.0;
    let estimated = (base * lang_weight).round() as usize;
    estimated.clamp(15, raw_tokens)
}

/// Estimates fast repository statistics.
pub fn estimate_fast_stats_impl(
    root: &Path,
    config: &TraversalConfig,
    timeout_secs: Option<u64>,
) -> Result<FastStatsReport> {
    let start_time = Instant::now();
    let timeout = timeout_secs.map(Duration::from_secs);

    if root.is_file() {
        return estimate_single_file(root, start_time);
    }

    let files = ProjectWalker::collect_files(root, config);
    let mut total_lines = 0;
    let mut total_raw_tokens = 0;
    let mut total_sliced_tokens = 0;
    let mut file_items = Vec::with_capacity(files.len());
    let mut lang_counts: HashMap<String, (usize, usize, usize)> = HashMap::new();

    for path in files {
        if let Some(limit) = timeout {
            if start_time.elapsed() >= limit {
                break;
            }
        }

        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        let file_size = meta.len();
        let lang = SupportedLanguage::from_path(&path);

        if let Ok(source) = std::fs::read_to_string(&path) {
            let lines = count_lines(&source);
            let tokens = count_tokens(&source);
            let sliced = estimate_sliced_tokens(tokens, lines, lang);

            total_lines += lines;
            total_raw_tokens += tokens;
            total_sliced_tokens += sliced;

            let lang_name = lang.map_or_else(|| "other".to_string(), |l| l.as_str().to_string());
            let entry = lang_counts.entry(lang_name).or_insert((0, 0, 0));
            entry.0 += 1;
            entry.1 += lines;
            entry.2 += tokens;

            file_items.push(FastFileStatItem {
                path: path.to_string_lossy().to_string(),
                language: lang.map(|l| l.as_str().to_string()),
                lines,
                estimated_tokens: tokens,
                estimated_sliced_tokens: sliced,
                file_size_bytes: file_size,
            });
        }
    }

    let estimated_savings_percentage =
        calculate_savings_percentage(total_raw_tokens, total_sliced_tokens);

    let mut language_breakdown: Vec<LanguageStatItem> = lang_counts
        .into_iter()
        .map(|(language, (file_count, lines, tokens))| LanguageStatItem {
            language,
            file_count,
            total_lines: lines,
            estimated_tokens: tokens,
        })
        .collect();
    language_breakdown.sort_by_key(|b| std::cmp::Reverse(b.estimated_tokens));

    #[allow(clippy::cast_possible_truncation)]
    let scan_duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(FastStatsReport {
        total_files: file_items.len(),
        total_lines,
        estimated_raw_tokens: total_raw_tokens,
        estimated_sliced_tokens: total_sliced_tokens,
        estimated_savings_percentage,
        language_breakdown,
        scan_duration_ms,
        files: file_items,
    })
}

fn estimate_single_file(file_path: &Path, start_time: Instant) -> Result<FastStatsReport> {
    let source = std::fs::read_to_string(file_path).map_err(|e| CoreError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    let meta = std::fs::metadata(file_path).map_err(|e| CoreError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;

    let lines = count_lines(&source);
    let raw_tokens = count_tokens(&source);
    let lang = SupportedLanguage::from_path(file_path);
    let estimated_sliced = estimate_sliced_tokens(raw_tokens, lines, lang);
    let savings = calculate_savings_percentage(raw_tokens, estimated_sliced);
    let lang_str = lang.map_or_else(|| "other".to_string(), |l| l.as_str().to_string());

    let file_item = FastFileStatItem {
        path: file_path.to_string_lossy().to_string(),
        language: lang.map(|l| l.as_str().to_string()),
        lines,
        estimated_tokens: raw_tokens,
        estimated_sliced_tokens: estimated_sliced,
        file_size_bytes: meta.len(),
    };

    let lang_item = LanguageStatItem {
        language: lang_str,
        file_count: 1,
        total_lines: lines,
        estimated_tokens: raw_tokens,
    };

    #[allow(clippy::cast_possible_truncation)]
    let scan_duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(FastStatsReport {
        total_files: 1,
        total_lines: lines,
        estimated_raw_tokens: raw_tokens,
        estimated_sliced_tokens: estimated_sliced,
        estimated_savings_percentage: savings,
        language_breakdown: vec![lang_item],
        scan_duration_ms,
        files: vec![file_item],
    })
}
