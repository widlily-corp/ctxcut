//! Context slicing orchestrator module.

use crate::error::{CoreError, Result};
use crate::formatter::MarkdownFormatter;
use crate::lang::LanguageRegistry;
use crate::model::{SliceOptions, SliceResult, SupportedLanguage, TokenStats};
use crate::parser::ParserManager;
use crate::tokenizer::compute_stats;
use std::fs;
use std::path::Path;

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
            stripped_calls,
            stats: TokenStats::calculate(0, 0, 0, 0),
        };

        // 5. Generate Markdown and calculate exact BPE token reduction stats
        let rendered_markdown = MarkdownFormatter::format(&result);
        let stats = compute_stats(&source, &rendered_markdown);
        result.stats = stats;

        Ok(result)
    }

    /// Slices multiple symbols from a source file.
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
}
