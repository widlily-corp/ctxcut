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

        // Section 3: Concrete Implementors (if present)
        if !result.hoisted_implementors.is_empty() {
            out.push_str("#### 3. Concrete Implementors\n");
            let mut seen_imp = HashSet::new();
            let mut unique_imp = Vec::new();
            for imp in &result.hoisted_implementors {
                let key = (&imp.interface_name, &imp.implementor_name);
                if seen_imp.insert(key) {
                    unique_imp.push(imp);
                }
            }

            let _ = writeln!(out, "```{lang_tag}");
            for (idx, imp) in unique_imp.iter().enumerate() {
                if idx > 0 {
                    out.push_str("\n\n");
                }
                out.push_str(imp.definition.trim());
            }
            out.push_str("\n```\n\n");
        }

        // Section 4 (or 3): External Dependencies & Signatures (Body Stripped)
        let dep_sec = if result.hoisted_implementors.is_empty() {
            3
        } else {
            4
        };
        let _ = writeln!(
            out,
            "#### {dep_sec}. External Dependencies & Signatures (Body Stripped)"
        );
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

    /// Formats a `BatchSliceResult` with unified type deduplication and consolidated sections.
    pub fn format_unified_batch(result: &crate::model::BatchSliceResult) -> String {
        let mut out = String::with_capacity(4096);

        let lang_str = result
            .target_symbols
            .first()
            .map(|s| s.language.as_str())
            .unwrap_or("text");
        let lang_tag = normalize_language_tag(lang_str);
        let stats = &result.stats;

        let symbol_names: Vec<&str> = result
            .target_symbols
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        let sym_header = symbol_names.join(", ");

        // Header section with metadata
        let _ = writeln!(out, "### Context Slice: `{}:{}`", result.file_path, sym_header);
        let _ = writeln!(
            out,
            "*Language: `{}` | Lines: `{}` (was `{}`) | Tokens: `{}` (was `{}`) | Savings: `{:.1}%`*\n",
            lang_str,
            stats.sliced_lines,
            stats.raw_lines,
            stats.sliced_tokens,
            stats.raw_file_tokens,
            stats.savings_percentage
        );

        // Section 1: Target Implementations (Full Body)
        out.push_str("#### 1. Target Implementation (Full Body)\n");
        if result.target_symbols.is_empty() {
            out.push_str("*None*\n\n");
        } else {
            for sym in &result.target_symbols {
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
            }
        }

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

        // Section 3: Concrete Implementors (if present)
        if !result.hoisted_implementors.is_empty() {
            out.push_str("#### 3. Concrete Implementors\n");
            let mut seen_imp = HashSet::new();
            let mut unique_imp = Vec::new();
            for imp in &result.hoisted_implementors {
                let key = (&imp.interface_name, &imp.implementor_name);
                if seen_imp.insert(key) {
                    unique_imp.push(imp);
                }
            }

            let _ = writeln!(out, "```{lang_tag}");
            for (idx, imp) in unique_imp.iter().enumerate() {
                if idx > 0 {
                    out.push_str("\n\n");
                }
                out.push_str(imp.definition.trim());
            }
            out.push_str("\n```\n\n");
        }

        // Section 4 (or 3): External Dependencies & Signatures (Body Stripped)
        let dep_sec = if result.hoisted_implementors.is_empty() {
            3
        } else {
            4
        };
        let _ = writeln!(
            out,
            "#### {dep_sec}. External Dependencies & Signatures (Body Stripped)"
        );
        let mut seen_calls = HashSet::new();
        let mut unique_calls = Vec::new();
        for call in &result.stripped_calls {
            let key = (call.receiver.as_deref(), call.name.as_str());
            if seen_calls.insert(key) {
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

    /// Formats an `ImpactSliceResult` into high-density Markdown.
    pub fn format_impact(result: &crate::model::ImpactSliceResult) -> String {
        let mut out = String::with_capacity(2048);

        let _ = writeln!(out, "### Upstream Impact Analysis: `{}`", result.target_symbol);
        if let Some(ref tf) = result.target_file {
            let _ = writeln!(out, "*Target Declaration File: `{tf}`*");
        }
        let _ = writeln!(
            out,
            "*Discovered Callers: `{}` call site(s) | Tokens: `{}` (was `{}`) | Savings: `{:.1}%`*\n",
            result.total_callers,
            result.stats.sliced_tokens,
            result.stats.raw_file_tokens,
            result.stats.savings_percentage
        );

        if result.callers.is_empty() {
            out.push_str("#### Discovered Call Sites\n*No upstream callers found in workspace.*\n");
            return out;
        }

        out.push_str("#### Discovered Call Sites\n\n");
        for (idx, caller) in result.callers.iter().enumerate() {
            let num = idx + 1;
            let _ = writeln!(
                out,
                "{num}. **`{}`** (`{}`) — `{}:{}`",
                caller.caller_symbol, caller.caller_kind, caller.file_path, caller.line_number
            );

            if let Some(ref sig) = caller.caller_signature {
                let _ = writeln!(out, "   - **Caller Signature**: `{}`", sig.trim());
            }
            let _ = writeln!(out, "   - **Call Invocation** (line {}):", caller.line_number);
            let _ = writeln!(out, "     ```");
            for line in caller.call_snippet.lines() {
                let _ = writeln!(out, "     {line}");
            }
            let _ = writeln!(out, "     ```\n");
        }

        out
    }

    /// Formats an end-to-end execution flow trace result into a structured Markdown document.
    pub fn format_trace(trace: &crate::model::TraceResult) -> String {
        let mut out = String::with_capacity(3072);

        let _ = writeln!(out, "# Execution Flow Trace: `{}`\n", trace.entry_point);
        let _ = writeln!(
            out,
            "**Entry File**: `{}` | **Steps**: `{}` | **Tokens**: `{}` (Raw: `{}`, Savings: `{:.1}%`)\n",
            trace.entry_file,
            trace.total_steps,
            trace.stats.sliced_tokens,
            trace.stats.raw_file_tokens,
            trace.stats.savings_percentage
        );

        // Section 1: Topology Flowchart
        out.push_str("## 1. Invocation Spine Topology\n\n```text\n");
        for (i, step) in trace.steps.iter().enumerate() {
            let prefix = if i == 0 {
                format!("[1] {} ({})", step.symbol_name, step.file_path)
            } else {
                let indent = " ".repeat(i * 3);
                format!(
                    "{indent}└──> [{}] {} ({})",
                    step.step_number, step.symbol_name, step.file_path
                )
            };
            out.push_str(&prefix);
            out.push('\n');
        }
        out.push_str("```\n\n");

        // Section 2: Step-by-Step Breakdown
        out.push_str("## 2. Step-by-Step Invocation Pathway\n\n");
        for step in &trace.steps {
            let lang_tag = normalize_language_tag(&step.language);
            let _ = writeln!(
                out,
                "### Step {}: `{}` ({})\n- **File**: `{}:{}-{}`\n- **Signature**: `{}`",
                step.step_number,
                step.symbol_name,
                step.kind,
                step.file_path,
                step.start_line,
                step.end_line,
                step.signature
            );

            if let Some(ref next) = step.next_target {
                let _ = writeln!(out, "- **Next Invocation Spine**: `{}`", next);
            }

            if !step.outgoing_calls.is_empty() {
                let calls_str = step.outgoing_calls.join(", ");
                let _ = writeln!(out, "- **Detected Calls**: `{}`", calls_str);
            }

            out.push('\n');
            let _ = writeln!(out, "```{lang_tag}");
            out.push_str(step.code_snippet.trim());
            out.push_str("\n```\n\n");
        }

        out
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
