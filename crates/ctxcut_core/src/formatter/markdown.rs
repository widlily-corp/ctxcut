//! Markdown formatter for generating prompt-optimized context slices.

use crate::model::SliceResult;
use std::collections::HashSet;
use std::fmt::Write;

/// Formats AST slice results into prompt-optimized Markdown documents for LLMs.
pub struct MarkdownFormatter;

impl MarkdownFormatter {
    /// Formats a single `SliceResult` into prompt-optimized Markdown.
    pub fn format(result: &SliceResult) -> String {
        let mut out = String::with_capacity(2048);

        let lang_tag = normalize_language_tag(&result.target_symbol.language);
        let sym = &result.target_symbol;
        let stats = &result.stats;

        // Header section with metadata
        let _ = writeln!(out, "### Context Slice: `{}:{}`", sym.file_path, sym.name);
        let _ = writeln!(
            out,
            "*Language: `{}` | Lines: `{}` (was `{}`) | Tokens: `{}` (was `{}`) | Savings: `{:.1}%`*\n",
            sym.language,
            stats.sliced_lines,
            stats.raw_lines,
            stats.sliced_tokens,
            stats.raw_file_tokens,
            stats.savings_percentage
        );

        // Section 1: Target Implementation
        out.push_str("#### 1. Target Implementation (Full Body)\n");
        let _ = writeln!(out, "```{lang_tag}");
        if let Some(ref doc) = sym.doc_comment {
            let trimmed_doc = doc.trim();
            if !trimmed_doc.is_empty() && !sym.body.contains(trimmed_doc) {
                out.push_str(trimmed_doc);
                out.push('\n');
            }
        }
        out.push_str(sym.body.trim());
        out.push_str("\n```\n\n");

        // Section 2: Hoisted Types & Data Contracts
        out.push_str("#### 2. Hoisted Types & Data Contracts\n");
        let mut seen_types = HashSet::new();
        let mut unique_types = Vec::new();
        for ty in &result.hoisted_types {
            if seen_types.insert(&ty.name) {
                unique_types.push(ty);
            }
        }

        if unique_types.is_empty() {
            out.push_str("*None*\n\n");
        } else {
            let _ = writeln!(out, "```{lang_tag}");
            for (idx, ty) in unique_types.iter().enumerate() {
                if idx > 0 {
                    out.push_str("\n\n");
                }
                out.push_str(ty.definition.trim());
            }
            out.push_str("\n```\n\n");
        }

        // Section 3: External Dependencies & Signatures (Body Stripped)
        out.push_str("#### 3. External Dependencies & Signatures (Body Stripped)\n");
        let mut seen_calls = HashSet::new();
        let mut unique_calls = Vec::new();
        for call in &result.stripped_calls {
            if seen_calls.insert(&call.name) {
                unique_calls.push(call);
            }
        }

        if unique_calls.is_empty() {
            out.push_str("*None*\n");
        } else {
            let _ = writeln!(out, "```{lang_tag}");
            for (idx, call) in unique_calls.iter().enumerate() {
                if idx > 0 {
                    out.push('\n');
                }
                out.push_str(call.signature.trim());
            }
            out.push_str("\n```\n");
        }

        out
    }

    /// Formats a batch of `SliceResult` items into a combined Markdown document.
    pub fn format_batch(results: &[SliceResult]) -> String {
        results
            .iter()
            .map(SliceResult::to_markdown)
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }
}

/// Normalizes language identifier to standard Markdown code-fence syntax tag.
pub fn normalize_language_tag(lang: &str) -> &str {
    if lang.eq_ignore_ascii_case("typescript") || lang.eq_ignore_ascii_case("ts") {
        "typescript"
    } else if lang.eq_ignore_ascii_case("tsx") {
        "tsx"
    } else if lang.eq_ignore_ascii_case("javascript") || lang.eq_ignore_ascii_case("js") {
        "javascript"
    } else if lang.eq_ignore_ascii_case("jsx") {
        "jsx"
    } else if lang.eq_ignore_ascii_case("python") || lang.eq_ignore_ascii_case("py") {
        "python"
    } else if lang.eq_ignore_ascii_case("go") || lang.eq_ignore_ascii_case("golang") {
        "go"
    } else if lang.eq_ignore_ascii_case("rust") || lang.eq_ignore_ascii_case("rs") {
        "rust"
    } else {
        lang
    }
}
