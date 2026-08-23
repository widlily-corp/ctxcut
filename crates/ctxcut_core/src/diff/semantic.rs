//! Token-efficient structural Semantic AST diff engine.

use crate::error::Result;
use crate::lang::LanguageRegistry;
use crate::model::{
    ExtractedSymbol, SliceOptions, SliceResult, SupportedLanguage, TokenStats,
};
use crate::parser::ParserManager;
use crate::slice::ContextSlicer;
use crate::telemetry::{
    ModelTierSavings, ECONOMY_PRICE_PER_MILLION_TOKENS, FRONTIER_PRICE_PER_MILLION_TOKENS,
    STANDARD_PRICE_PER_MILLION_TOKENS,
};
use crate::tokenizer::{calculate_savings_percentage, count_lines, count_tokens};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Detailed classification of an AST symbol change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SymbolChangeKind {
    /// Newly added symbol.
    Added,
    /// Deleted symbol.
    Removed,
    /// Function/method signature changed (breaking contract delta).
    SignatureChanged {
        /// Previous signature before changes.
        old_signature: String,
        /// New signature after changes.
        new_signature: String,
        /// Summary description of the signature change.
        description: String,
    },
    /// Internal implementation body modified, signature unchanged.
    BodyModified,
    /// Only docstrings, JSDoc, or comments changed.
    DocstringModified,
    /// Type definition fields or enum variants changed.
    TypeDefinitionChanged,
}

/// Diff description for a specific symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolDiffItem {
    /// Identifier name (e.g. `processPayment`, `AuthService.login`).
    pub name: String,
    /// Symbol category: `"function"`, `"method"`, `"class"`, `"interface"`, `"type"`, `"enum"`.
    pub kind: String,
    /// Classification of the change.
    pub change_kind: SymbolChangeKind,
    /// Original symbol before changes (if present).
    pub old_symbol: Option<ExtractedSymbol>,
    /// New symbol after changes (if present).
    pub new_symbol: Option<ExtractedSymbol>,
    /// Compact contextual AST slice of the symbol.
    pub slice: Option<SliceResult>,
}

/// Import statement modification kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportChangeKind {
    /// Newly added import statement.
    Added,
    /// Removed import statement.
    Removed,
    /// Modified import statement.
    Modified,
}

/// Discovered change in top-level module imports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportChangeItem {
    /// Change category.
    pub kind: ImportChangeKind,
    /// Full import statement snippet.
    pub statement: String,
    /// Extracted module specifier (if any).
    pub module_specifier: Option<String>,
}

/// Semantic AST diff report for a single file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileSemanticDiff {
    /// Relative or absolute path to the file.
    pub file_path: String,
    /// Programming language tag.
    pub language: String,
    /// Added symbols.
    pub added_symbols: Vec<SymbolDiffItem>,
    /// Removed symbols.
    pub removed_symbols: Vec<SymbolDiffItem>,
    /// Modified symbols (body, signature, docstring, type).
    pub modified_symbols: Vec<SymbolDiffItem>,
    /// Top-level import modifications.
    pub import_changes: Vec<ImportChangeItem>,
    /// Raw full file token count.
    pub raw_file_tokens: usize,
    /// Sliced semantic diff token count.
    pub diff_tokens: usize,
    /// Token reduction savings percentage.
    pub savings_percentage: f64,
}

/// Multi-tier monetary cost savings and token efficiency metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticDiffRoi {
    /// Total raw tokens across unmodified files.
    pub raw_tokens: usize,
    /// Total tokens in the semantic AST diff.
    pub semantic_diff_tokens: usize,
    /// Absolute token count saved.
    pub tokens_saved: usize,
    /// Percentage token reduction.
    pub savings_percentage: f64,
    /// Estimated dollar savings on standard model.
    pub cost_savings_usd: f64,
    /// Multi-tier savings breakdown.
    pub tier_savings: ModelTierSavings,
}

/// Comprehensive workspace-wide Semantic AST Diff Result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticDiffResult {
    /// Workspace root directory.
    pub workspace_root: String,
    /// Per-file semantic diff details.
    pub files: Vec<FileSemanticDiff>,
    /// Total newly added symbols.
    pub total_added_symbols: usize,
    /// Total removed symbols.
    pub total_removed_symbols: usize,
    /// Total modified symbols.
    pub total_modified_symbols: usize,
    /// Total signature breaking changes.
    pub total_signature_changes: usize,
    /// Total internal body modifications.
    pub total_body_changes: usize,
    /// Token statistics summary.
    pub stats: TokenStats,
    /// Return-on-Investment (ROI) token & cost metrics.
    pub roi: SemanticDiffRoi,
}

impl SemanticDiffResult {
    /// Formats the semantic diff result as clean, high-density Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Semantic AST Diff\n\n");
        out.push_str(&format!(
            "*Workspace: `{}` | Files Changed: `{}` | Added: `{}` | Removed: `{}` | Modified: `{}` (Signatures: `{}`, Bodies: `{}`)*\n",
            self.workspace_root,
            self.files.len(),
            self.total_added_symbols,
            self.total_removed_symbols,
            self.total_modified_symbols,
            self.total_signature_changes,
            self.total_body_changes
        ));
        out.push_str(&format!(
            "*Token Reduction: `{}` → `{}` tokens (`{:.1}%` savings) | Estimated Savings: `${:.4}` (Standard), `${:.4}` (Frontier)*\n\n",
            self.stats.raw_file_tokens,
            self.stats.sliced_tokens,
            self.stats.savings_percentage,
            self.roi.tier_savings.standard_sonnet_gpt4o,
            self.roi.tier_savings.frontier_opus
        ));
        out.push_str("---\n\n");

        for file in &self.files {
            out.push_str(&format!(
                "### 📄 `{}` ({})\n",
                file.file_path, file.language
            ));
            out.push_str(&format!(
                "*Tokens: {} → {} ({:.1}% savings)*\n\n",
                file.raw_file_tokens, file.diff_tokens, file.savings_percentage
            ));

            if !file.added_symbols.is_empty() {
                out.push_str("#### ➕ Added Symbols\n");
                for item in &file.added_symbols {
                    out.push_str(&format!("- `{}` ({})\n", item.name, item.kind));
                }
                out.push('\n');
            }

            if !file.removed_symbols.is_empty() {
                out.push_str("#### ➖ Removed Symbols\n");
                for item in &file.removed_symbols {
                    out.push_str(&format!("- `{}` ({})\n", item.name, item.kind));
                }
                out.push('\n');
            }

            if !file.modified_symbols.is_empty() {
                out.push_str("#### 🔄 Modified Symbols\n\n");
                for item in &file.modified_symbols {
                    let change_desc = match &item.change_kind {
                        SymbolChangeKind::SignatureChanged { .. } => "Signature Changed",
                        SymbolChangeKind::BodyModified => "Body Modified",
                        SymbolChangeKind::DocstringModified => "Docstring / Comments",
                        SymbolChangeKind::TypeDefinitionChanged => "Type Definition Changed",
                        _ => "Modified",
                    };

                    out.push_str(&format!(
                        "##### `{}` ({}) — *{}*\n\n",
                        item.name, item.kind, change_desc
                    ));

                    if let SymbolChangeKind::SignatureChanged {
                        old_signature,
                        new_signature,
                        ..
                    } = &item.change_kind
                    {
                        out.push_str("```diff\n");
                        out.push_str(&format!("- {old_signature}\n"));
                        out.push_str(&format!("+ {new_signature}\n"));
                        out.push_str("```\n\n");
                    }

                    if let Some(ref slice) = item.slice {
                        out.push_str("**Context Slice:**\n```");
                        out.push_str(&file.language);
                        out.push('\n');
                        out.push_str(slice.target_symbol.body.trim());
                        out.push_str("\n```\n\n");
                    }
                }
            }

            if !file.import_changes.is_empty() {
                out.push_str("#### 📦 Import Changes\n");
                for imp in &file.import_changes {
                    let tag = match imp.kind {
                        ImportChangeKind::Added => "+",
                        ImportChangeKind::Removed => "-",
                        ImportChangeKind::Modified => "~",
                    };
                    out.push_str(&format!("- `{tag} {}`\n", imp.statement));
                }
                out.push('\n');
            }

            out.push_str("---\n\n");
        }

        out
    }

    /// Formats the semantic diff result as pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the semantic diff result as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Semantic AST diff computation engine.
pub struct SemanticDiffEngine;

impl SemanticDiffEngine {
    /// Computes a semantic AST diff comparing working tree / staged changes against Git baseline.
    pub fn compute_diff(
        workspace_root: &Path,
        staged: bool,
        file_path: Option<&Path>,
        budget: Option<usize>,
    ) -> Result<SemanticDiffResult> {
        let changed_files = if let Some(p) = file_path {
            let full_p = if p.is_absolute() {
                p.to_path_buf()
            } else {
                workspace_root.join(p)
            };
            vec![full_p]
        } else {
            get_git_changed_files(workspace_root, staged)?
        };

        let mut file_diffs = Vec::new();
        let mut total_added = 0;
        let mut total_removed = 0;
        let mut total_modified = 0;
        let mut total_sig_changes = 0;
        let mut total_body_changes = 0;

        let mut total_raw_tokens = 0;
        let mut total_diff_tokens = 0;
        let mut total_raw_lines = 0;
        let mut total_diff_lines = 0;

        for path in &changed_files {
            let Some(lang) = SupportedLanguage::from_path(path) else {
                continue;
            };

            let rel_path = path
                .strip_prefix(workspace_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            let old_source = get_git_file_content(workspace_root, &rel_path, staged).unwrap_or_default();
            let new_source = if staged {
                get_git_staged_content(workspace_root, &rel_path).unwrap_or_default()
            } else if path.exists() {
                fs::read_to_string(path).unwrap_or_default()
            } else {
                String::new()
            };

            if old_source.is_empty() && new_source.is_empty() {
                continue;
            }

            let file_diff = Self::diff_sources_internal(
                &old_source,
                &new_source,
                path,
                lang,
                &rel_path,
                budget,
            )?;

            total_added += file_diff.added_symbols.len();
            total_removed += file_diff.removed_symbols.len();
            for item in &file_diff.modified_symbols {
                total_modified += 1;
                match &item.change_kind {
                    SymbolChangeKind::SignatureChanged { .. } => total_sig_changes += 1,
                    SymbolChangeKind::BodyModified => total_body_changes += 1,
                    _ => {}
                }
            }

            total_raw_tokens += file_diff.raw_file_tokens;
            total_diff_tokens += file_diff.diff_tokens;
            total_raw_lines += count_lines(&new_source);
            total_diff_lines += count_lines(&format!("{file_diff:?}"));

            file_diffs.push(file_diff);
        }

        let tokens_saved = total_raw_tokens.saturating_sub(total_diff_tokens);

        let savings_percentage = calculate_savings_percentage(total_raw_tokens, total_diff_tokens);
        let standard_savings = (tokens_saved as f64 / 1_000_000.0) * STANDARD_PRICE_PER_MILLION_TOKENS;
        let frontier_savings = (tokens_saved as f64 / 1_000_000.0) * FRONTIER_PRICE_PER_MILLION_TOKENS;
        let economy_savings = (tokens_saved as f64 / 1_000_000.0) * ECONOMY_PRICE_PER_MILLION_TOKENS;

        let roi = SemanticDiffRoi {
            raw_tokens: total_raw_tokens,
            semantic_diff_tokens: total_diff_tokens,
            tokens_saved,
            savings_percentage,
            cost_savings_usd: standard_savings,
            tier_savings: ModelTierSavings {
                standard_sonnet_gpt4o: standard_savings,
                frontier_opus: frontier_savings,
                economy_haiku_mini: economy_savings,
            },
        };

        let stats = TokenStats {
            raw_file_tokens: total_raw_tokens,
            sliced_tokens: total_diff_tokens,
            savings_percentage,
            raw_lines: total_raw_lines,
            sliced_lines: total_diff_lines,
        };

        Ok(SemanticDiffResult {
            workspace_root: workspace_root.to_string_lossy().to_string(),
            files: file_diffs,
            total_added_symbols: total_added,
            total_removed_symbols: total_removed,
            total_modified_symbols: total_modified,
            total_signature_changes: total_sig_changes,
            total_body_changes,
            stats,
            roi,
        })
    }

    /// Compares two versions of a source file and computes a semantic AST diff.
    pub fn diff_sources(
        old_source: &str,
        new_source: &str,
        file_path: &Path,
        budget: Option<usize>,
    ) -> Result<FileSemanticDiff> {
        let lang = SupportedLanguage::from_path(file_path).unwrap_or(SupportedLanguage::TypeScript);
        let rel_path = file_path.to_string_lossy().to_string();
        Self::diff_sources_internal(old_source, new_source, file_path, lang, &rel_path, budget)
    }

    fn diff_sources_internal(
        old_source: &str,
        new_source: &str,
        file_path: &Path,
        lang: SupportedLanguage,
        rel_path: &str,
        budget: Option<usize>,
    ) -> Result<FileSemanticDiff> {
        let adapter = LanguageRegistry::for_language(lang)?;
        let ts_lang = adapter.tree_sitter_language(file_path);

        let old_tree = if !old_source.trim().is_empty() {
            ParserManager::parse_source(old_source, &ts_lang, file_path).ok()
        } else {
            None
        };

        let new_tree = if !new_source.trim().is_empty() {
            ParserManager::parse_source(new_source, &ts_lang, file_path).ok()
        } else {
            None
        };

        let mut old_symbols: BTreeMap<String, ExtractedSymbol> = BTreeMap::new();
        if let (Some(tree), false) = (&old_tree, old_source.trim().is_empty()) {
            let names = adapter.list_symbols(tree.root_node(), old_source);
            for name in names {
                if let Ok((sym, _)) = adapter.locate_symbol(tree.root_node(), old_source, &name, file_path) {
                    old_symbols.insert(name, sym);
                }
            }
        }

        let mut new_symbols: BTreeMap<String, ExtractedSymbol> = BTreeMap::new();
        if let (Some(tree), false) = (&new_tree, new_source.trim().is_empty()) {
            let names = adapter.list_symbols(tree.root_node(), new_source);
            for name in names {
                if let Ok((sym, _)) = adapter.locate_symbol(tree.root_node(), new_source, &name, file_path) {
                    new_symbols.insert(name, sym);
                }
            }
        }

        let mut added_items = Vec::new();
        let mut removed_items = Vec::new();
        let mut modified_items = Vec::new();

        let old_keys: BTreeSet<&String> = old_symbols.keys().collect();
        let new_keys: BTreeSet<&String> = new_symbols.keys().collect();

        // 1. Added symbols
        for key in new_keys.difference(&old_keys) {
            if let Some(new_sym) = new_symbols.get(*key) {
                added_items.push(SymbolDiffItem {
                    name: (*key).clone(),
                    kind: new_sym.kind.clone(),
                    change_kind: SymbolChangeKind::Added,
                    old_symbol: None,
                    new_symbol: Some(new_sym.clone()),
                    slice: None,
                });
            }
        }

        // 2. Removed symbols
        for key in old_keys.difference(&new_keys) {
            if let Some(old_sym) = old_symbols.get(*key) {
                removed_items.push(SymbolDiffItem {
                    name: (*key).clone(),
                    kind: old_sym.kind.clone(),
                    change_kind: SymbolChangeKind::Removed,
                    old_symbol: Some(old_sym.clone()),
                    new_symbol: None,
                    slice: None,
                });
            }
        }

        // 3. Modified symbols
        let slicer = ContextSlicer::new();
        let slice_opts = SliceOptions {
            budget,
            include_types: true,
            include_calls: true,
            depth: 1,
        };

        for key in old_keys.intersection(&new_keys) {
            let old_sym = &old_symbols[*key];
            let new_sym = &new_symbols[*key];

            let (is_changed, change_kind) = classify_symbol_delta(old_sym, new_sym);
            if is_changed {
                let slice = if file_path.exists() {
                    slicer.slice_symbol(file_path, key, &slice_opts).ok()
                } else {
                    None
                };

                modified_items.push(SymbolDiffItem {
                    name: (*key).clone(),
                    kind: new_sym.kind.clone(),
                    change_kind,
                    old_symbol: Some(old_sym.clone()),
                    new_symbol: Some(new_sym.clone()),
                    slice,
                });
            }
        }

        // 4. Import changes
        let import_changes = extract_import_changes(old_source, new_source);

        let raw_file_tokens = count_tokens(new_source);
        let mut diff_tokens = 0;
        for item in &added_items {
            if let Some(s) = &item.new_symbol {
                diff_tokens += count_tokens(&s.body);
            }
        }
        for item in &modified_items {
            if let Some(s) = &item.slice {
                diff_tokens += s.stats.sliced_tokens;
            } else if let Some(s) = &item.new_symbol {
                diff_tokens += count_tokens(&s.body);
            }
        }
        if diff_tokens == 0 {
            diff_tokens = raw_file_tokens.min(20);
        }

        let savings_percentage = calculate_savings_percentage(raw_file_tokens, diff_tokens);

        Ok(FileSemanticDiff {
            file_path: rel_path.to_string(),
            language: lang.as_str().to_string(),
            added_symbols: added_items,
            removed_symbols: removed_items,
            modified_symbols: modified_items,
            import_changes,
            raw_file_tokens,
            diff_tokens,
            savings_percentage,
        })
    }
}

fn classify_symbol_delta(
    old_sym: &ExtractedSymbol,
    new_sym: &ExtractedSymbol,
) -> (bool, SymbolChangeKind) {
    if old_sym.body == new_sym.body {
        return (false, SymbolChangeKind::BodyModified);
    }

    let old_sig = extract_symbol_signature(old_sym);
    let new_sig = extract_symbol_signature(new_sym);

    if normalize_whitespace(&old_sig) != normalize_whitespace(&new_sig) {
        return (
            true,
            SymbolChangeKind::SignatureChanged {
                old_signature: old_sig,
                new_signature: new_sig,
                description: "Signature parameters or return types modified".to_string(),
            },
        );
    }

    let old_stripped = strip_comments_and_normalize(&old_sym.body);
    let new_stripped = strip_comments_and_normalize(&new_sym.body);

    if old_stripped == new_stripped {
        return (true, SymbolChangeKind::DocstringModified);
    }

    if old_sym.kind.contains("type") || old_sym.kind.contains("interface") || old_sym.kind.contains("struct") {
        return (true, SymbolChangeKind::TypeDefinitionChanged);
    }

    (true, SymbolChangeKind::BodyModified)
}

fn extract_symbol_signature(sym: &ExtractedSymbol) -> String {
    if !sym.signature.trim().is_empty() {
        return sym.signature.clone();
    }
    sym.body.lines().next().unwrap_or("").trim().to_string()
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_comments_and_normalize(code: &str) -> String {
    let mut out = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }
        if !trimmed.is_empty() {
            out.push(trimmed);
        }
    }
    out.join(" ")
}

fn extract_import_changes(old_source: &str, new_source: &str) -> Vec<ImportChangeItem> {
    let old_imports: BTreeSet<String> = old_source
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("import ") || l.starts_with("from ") || l.starts_with("use ") || (l.starts_with("const ") && l.contains("require(")))
        .map(String::from)
        .collect();

    let new_imports: BTreeSet<String> = new_source
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("import ") || l.starts_with("from ") || l.starts_with("use ") || (l.starts_with("const ") && l.contains("require(")))
        .map(String::from)
        .collect();

    let mut changes = Vec::new();
    for imp in new_imports.difference(&old_imports) {
        changes.push(ImportChangeItem {
            kind: ImportChangeKind::Added,
            statement: imp.clone(),
            module_specifier: None,
        });
    }
    for imp in old_imports.difference(&new_imports) {
        changes.push(ImportChangeItem {
            kind: ImportChangeKind::Removed,
            statement: imp.clone(),
            module_specifier: None,
        });
    }
    changes
}

fn get_git_changed_files(workspace_root: &Path, staged: bool) -> Result<Vec<PathBuf>> {
    let mut args = vec!["diff", "--name-only"];
    if staged {
        args.push("--staged");
    }

    let output = Command::new("git")
        .args(&args)
        .current_dir(workspace_root)
        .output();

    let mut paths = Vec::new();
    if let Ok(out) = output {
        if out.status.success() {
            let stdout_str = String::from_utf8_lossy(&out.stdout);
            for line in stdout_str.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    paths.push(workspace_root.join(trimmed));
                }
            }
        }
    }

    // Also include untracked files if any
    let untracked_out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workspace_root)
        .output();

    if let Ok(u_out) = untracked_out {
        if u_out.status.success() {
            let u_str = String::from_utf8_lossy(&u_out.stdout);
            for line in u_str.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("??") {
                    let file_part = trimmed.trim_start_matches("??").trim();
                    let full_p = workspace_root.join(file_part);
                    if !paths.contains(&full_p) {
                        paths.push(full_p);
                    }
                }
            }
        }
    }

    Ok(paths)
}

fn get_git_file_content(workspace_root: &Path, rel_path: &str, _staged: bool) -> Option<String> {
    let target_ref = format!("HEAD:{rel_path}");

    let output = Command::new("git")
        .args(["show", &target_ref])
        .current_dir(workspace_root)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

fn get_git_staged_content(workspace_root: &Path, rel_path: &str) -> Option<String> {
    let target_ref = format!(":{rel_path}");
    let output = Command::new("git")
        .args(["show", &target_ref])
        .current_dir(workspace_root)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}
