//! Context slicing orchestrator module.

pub mod budget;

use crate::error::{CoreError, Result};
use crate::formatter::MarkdownFormatter;
use crate::lang::LanguageRegistry;
use crate::model::{BatchSliceResult, SliceOptions, SliceResult, SupportedLanguage, TokenStats};
use crate::parser::ParserManager;
use crate::tokenizer::compute_stats;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub use budget::{BudgetCompressor, DegradationReport};

/// Context slicer engine coordinating parsing, symbol location, type hoisting,
/// signature stripping, formatting, and token reduction metrics.
#[derive(Debug, Default, Clone)]
pub struct ContextSlicer;

impl ContextSlicer {
    /// Creates a new `ContextSlicer` instance.
    pub fn new() -> Self {
        Self
    }

    /// Detects the supported programming language for a given file path.
    pub fn detect_language(path: &Path) -> Result<SupportedLanguage> {
        SupportedLanguage::from_path(path).ok_or_else(|| {
            let ext = path.extension().and_then(|e| e.to_str()).map(String::from);
            CoreError::UnsupportedLanguage {
                path: path.to_path_buf(),
                extension: ext,
            }
        })
    }

    /// Slices a single symbol from a source file into a self-contained `SliceResult`.
    pub fn slice_symbol(
        &self,
        file_path: &Path,
        symbol_name: &str,
        opts: &SliceOptions,
    ) -> Result<SliceResult> {
        let source = fs::read_to_string(file_path).map_err(|source| CoreError::Io {
            path: file_path.to_path_buf(),
            source,
        })?;

        let adapter = LanguageRegistry::for_path(file_path)?;
        let ts_lang = adapter.tree_sitter_language(file_path);
        let tree = ParserManager::parse_source(&source, &ts_lang, file_path)?;
        let root = tree.root_node();

        // 1. Locate target symbol
        let (target_symbol, target_node) =
            adapter.locate_symbol(root, &source, symbol_name, file_path)?;

        // 2. Hoist referenced types
        let hoisted_types = if opts.include_types {
            adapter.hoist_types(target_node, root, &source, file_path, opts)?
        } else {
            Vec::new()
        };

        // 2.5. Hoist concrete implementors
        let hoisted_implementors = if opts.include_types {
            let ws_root = find_workspace_root(file_path);
            let lang =
                SupportedLanguage::from_path(file_path).unwrap_or(SupportedLanguage::TypeScript);
            crate::resolver::ImplementorHoister::hoist_implementors_for_slice(
                &ws_root,
                file_path,
                &target_symbol,
                &hoisted_types,
                lang,
            )?
        } else {
            Vec::new()
        };

        // 3. Strip external calls
        let stripped_calls = if opts.include_calls {
            adapter.strip_calls(target_node, root, &source, file_path)?
        } else {
            Vec::new()
        };

        // 4. Create initial result structure with placeholder stats
        let mut result = SliceResult {
            target_symbol,
            hoisted_types,
            hoisted_implementors,
            stripped_calls,
            stats: TokenStats::calculate(0, 0, 0, 0),
        };

        // 4.5. Framework-Aware Semantic Enhancement
        let framework_registry = crate::framework::FrameworkRegistry::default();
        let _ = framework_registry.enhance_slice(target_node, &source, file_path, &mut result)?;

        // 4.6. ORM & Database/API Schema Stitching (Milestone 3)
        if opts.include_types {
            let ws_root = find_workspace_root(file_path);
            let schema_stitcher = crate::schema::SchemaStitcher::new();
            if let Ok(schema_types) = schema_stitcher.stitch_schemas(&ws_root, file_path, &source) {
                for st in schema_types {
                    if !result.hoisted_types.iter().any(|t| t.name == st.name) {
                        result.hoisted_types.push(st);
                    }
                }
            }
        }

        if !opts.include_types {
            result.hoisted_types.clear();
            result.hoisted_implementors.clear();
        }
        if !opts.include_calls {
            result.stripped_calls.clear();
        }

        // 5. Adaptive token budgeting if specified
        if let Some(budget_tokens) = opts.budget {
            let _ = BudgetCompressor::compress_slice(&mut result, budget_tokens)?;
            let final_md = MarkdownFormatter::format(&result);
            result.stats = compute_stats(&source, &final_md);
        } else {
            let rendered_markdown = MarkdownFormatter::format(&result);
            let stats = compute_stats(&source, &rendered_markdown);
            result.stats = stats;
        }

        Ok(result)
    }

    /// Slices a symbol with an adaptive token budget, compressing semantic details if necessary.
    pub fn slice_symbol_with_budget(
        &self,
        file_path: &Path,
        symbol_query: &str,
        opts: &SliceOptions,
        budget_tokens: usize,
    ) -> Result<(SliceResult, DegradationReport)> {
        let mut result = self.slice_symbol(file_path, symbol_query, opts)?;
        let report = BudgetCompressor::compress_slice(&mut result, budget_tokens)?;
        let source = fs::read_to_string(file_path).map_err(|source| CoreError::Io {
            path: file_path.to_path_buf(),
            source,
        })?;
        let final_md = MarkdownFormatter::format(&result);
        result.stats = compute_stats(&source, &final_md);
        Ok((result, report))
    }

    /// Slices multiple symbols from a source file into separate `SliceResult` items.
    pub fn slice_symbols(
        &self,
        file_path: &Path,
        symbol_names: &[&str],
        opts: &SliceOptions,
    ) -> Result<Vec<SliceResult>> {
        let mut results = Vec::with_capacity(symbol_names.len());
        for name in symbol_names {
            let res = self.slice_symbol(file_path, name, opts)?;
            results.push(res);
        }
        Ok(results)
    }

    /// Slices multiple target symbols from a source file into a unified `BatchSliceResult` with globally deduplicated types and calls.
    pub fn slice_batch(
        &self,
        file_path: &Path,
        symbol_names: &[&str],
        opts: &SliceOptions,
    ) -> Result<BatchSliceResult> {
        let source = fs::read_to_string(file_path).map_err(|source| CoreError::Io {
            path: file_path.to_path_buf(),
            source,
        })?;

        let adapter = LanguageRegistry::for_path(file_path)?;
        let ts_lang = adapter.tree_sitter_language(file_path);
        let tree = ParserManager::parse_source(&source, &ts_lang, file_path)?;
        let root = tree.root_node();

        let mut target_symbols = Vec::with_capacity(symbol_names.len());
        let mut all_hoisted = Vec::new();
        let mut seen_type_names = HashSet::new();
        let mut all_calls = Vec::new();
        let mut seen_call_keys = HashSet::new();

        let framework_registry = crate::framework::FrameworkRegistry::default();

        for name in symbol_names {
            let clean_name = name.trim();
            if clean_name.is_empty() {
                continue;
            }
            let (target_symbol, target_node) =
                adapter.locate_symbol(root, &source, clean_name, file_path)?;

            // Hoist types if enabled
            if opts.include_types {
                let types = adapter.hoist_types(target_node, root, &source, file_path, opts)?;
                for ty in types {
                    if seen_type_names.insert(ty.name.clone()) {
                        all_hoisted.push(ty);
                    }
                }
            }

            // Strip calls if enabled
            if opts.include_calls {
                let calls = adapter.strip_calls(target_node, root, &source, file_path)?;
                for call in calls {
                    let key = (call.receiver.clone(), call.name.clone());
                    if seen_call_keys.insert(key) {
                        all_calls.push(call);
                    }
                }
            }

            // Create temporary single slice to run framework enhancement
            let mut temp_slice = SliceResult {
                target_symbol: target_symbol.clone(),
                hoisted_types: Vec::new(),
                hoisted_implementors: Vec::new(),
                stripped_calls: Vec::new(),
                stats: TokenStats::calculate(0, 0, 0, 0),
            };
            let _ = framework_registry.enhance_slice(
                target_node,
                &source,
                file_path,
                &mut temp_slice,
            )?;
            if opts.include_types {
                for ty in temp_slice.hoisted_types {
                    if seen_type_names.insert(ty.name.clone()) {
                        all_hoisted.push(ty);
                    }
                }
            }
            if opts.include_calls {
                for call in temp_slice.stripped_calls {
                    let key = (call.receiver.clone(), call.name.clone());
                    if seen_call_keys.insert(key) {
                        all_calls.push(call);
                    }
                }
            }

            target_symbols.push(target_symbol);
        }

        let mut all_implementors = Vec::new();
        let mut seen_imp_keys = HashSet::new();
        if opts.include_types {
            let ws_root = find_workspace_root(file_path);
            let lang =
                SupportedLanguage::from_path(file_path).unwrap_or(SupportedLanguage::TypeScript);
            for sym in &target_symbols {
                if let Ok(imps) = crate::resolver::ImplementorHoister::hoist_implementors_for_slice(
                    &ws_root,
                    file_path,
                    sym,
                    &all_hoisted,
                    lang,
                ) {
                    for imp in imps {
                        let key = (imp.implementor_name.clone(), imp.file_path.clone());
                        if seen_imp_keys.insert(key) {
                            all_implementors.push(imp);
                        }
                    }
                }
            }

            let schema_stitcher = crate::schema::SchemaStitcher::new();
            if let Ok(schema_types) = schema_stitcher.stitch_schemas(&ws_root, file_path, &source) {
                for st in schema_types {
                    if seen_type_names.insert(st.name.clone()) {
                        all_hoisted.push(st);
                    }
                }
            }
        }

        let mut batch_result = BatchSliceResult {
            file_path: file_path.to_string_lossy().to_string(),
            target_symbols,
            hoisted_types: all_hoisted,
            hoisted_implementors: all_implementors,
            stripped_calls: all_calls,
            stats: TokenStats::calculate(0, 0, 0, 0),
        };

        // Budget compression if specified
        if let Some(budget_tokens) = opts.budget {
            let rendered_md = MarkdownFormatter::format_unified_batch(&batch_result);
            let current_tokens = crate::tokenizer::count_tokens(&rendered_md);
            if current_tokens > budget_tokens {
                for sym in &mut batch_result.target_symbols {
                    sym.doc_comment = None;
                }
                let compressed_md = MarkdownFormatter::format_unified_batch(&batch_result);
                let compressed_tokens = crate::tokenizer::count_tokens(&compressed_md);
                if compressed_tokens > budget_tokens {
                    batch_result.stripped_calls.clear();
                }
            }
        }

        let rendered_markdown = MarkdownFormatter::format_unified_batch(&batch_result);
        batch_result.stats = compute_stats(&source, &rendered_markdown);

        Ok(batch_result)
    }
}

fn find_workspace_root(path: &Path) -> std::path::PathBuf {
    let mut curr = path.parent();
    while let Some(dir) = curr {
        if dir.join(".git").exists()
            || dir.join("Cargo.toml").exists()
            || dir.join("package.json").exists()
            || dir.join("go.mod").exists()
            || dir.join("pyproject.toml").exists()
        {
            return dir.to_path_buf();
        }
        curr = dir.parent();
    }
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}
