//! Per-agent token budgeting, adaptive context compression, and `TokenStats` computation.

use super::engine::SwarmAgentPack;
use super::graph::WorkspaceGraph;
use crate::model::TokenStats;
use crate::tokenizer::{count_lines, count_tokens};
use std::collections::HashSet;

/// Applies token budget constraints and calculates exact `TokenStats` for an agent pack.
pub struct SwarmBudgetEngine;

impl SwarmBudgetEngine {
    /// Computes token statistics and applies adaptive degradation if `budget` is exceeded.
    pub fn compute_and_apply_budget(
        pack: &mut SwarmAgentPack,
        graph: &WorkspaceGraph,
        budget: Option<usize>,
    ) {
        // Step 1: Identify all touched files (files with internal symbols + boundary stubs)
        let mut touched_files: HashSet<String> = HashSet::new();
        for sym in &pack.internal_symbols {
            touched_files.insert(sym.file_path.clone());
        }
        for stub in &pack.boundary_stubs {
            if let Some(fp) = &stub.file_path {
                touched_files.insert(fp.clone());
            }
        }
        for ty in &pack.boundary_types {
            touched_files.insert(ty.file_path.clone());
        }

        // Calculate raw full file tokens and lines
        let mut raw_tokens = 0;
        let mut raw_lines = 0;

        for fp in &touched_files {
            if let Some(toks) = graph.file_tokens.get(fp) {
                raw_tokens += *toks;
            } else if let Some(content) = graph.file_contents.get(fp) {
                raw_tokens += count_tokens(content);
            }
            if let Some(lines) = graph.file_lines.get(fp) {
                raw_lines += *lines;
            } else if let Some(content) = graph.file_contents.get(fp) {
                raw_lines += count_lines(content);
            }
        }

        // If no files touched, fallback to internal symbol contents
        if raw_tokens == 0 {
            for sym in &pack.internal_symbols {
                raw_tokens += count_tokens(&sym.body) + count_tokens(&sym.signature);
                raw_lines += sym.end_line.saturating_sub(sym.start_line) + 1;
            }
        }

        // Step 2: Render initial code and check budget
        let initial_code = pack.to_annotated_code();
        let mut sliced_tokens = count_tokens(&initial_code);
        let mut sliced_lines = count_lines(&initial_code);

        if let Some(max_budget) = budget {
            if sliced_tokens > max_budget {
                // Degradation Level 1: Compress mock contracts
                pack.mock_contracts = format!(
                    "// Mock contracts compressed for {} (within budget).\n",
                    pack.agent_id
                );
                let pass1_code = pack.to_annotated_code();
                sliced_tokens = count_tokens(&pass1_code);
                sliced_lines = count_lines(&pass1_code);

                // Degradation Level 2: Strip internal symbol docstrings and body comments
                if sliced_tokens > max_budget {
                    for sym in &mut pack.internal_symbols {
                        sym.doc_comment = None;
                        sym.body = strip_comments_from_code(&sym.body);
                    }
                    let pass2_code = pack.to_annotated_code();
                    sliced_tokens = count_tokens(&pass2_code);
                    sliced_lines = count_lines(&pass2_code);
                }

                // Degradation Level 3: Shorten boundary types
                if sliced_tokens > max_budget {
                    for ty in &mut pack.boundary_types {
                        ty.definition = format!("export type {} = any;", ty.name);
                    }
                    let pass3_code = pack.to_annotated_code();
                    sliced_tokens = count_tokens(&pass3_code);
                    sliced_lines = count_lines(&pass3_code);
                }
            }
        }

        // Step 3: Compute final TokenStats
        pack.token_stats =
            TokenStats::calculate(raw_tokens, sliced_tokens, raw_lines, sliced_lines);
    }
}

/// Helper to strip line comments and block comments from source code.
fn strip_comments_from_code(code: &str) -> String {
    let mut lines = Vec::new();
    let mut in_block_comment = false;

    for line in code.lines() {
        let trimmed = line.trim();
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }
        if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with('#') {
            continue;
        }
        lines.push(line);
    }

    lines.join("\n")
}
