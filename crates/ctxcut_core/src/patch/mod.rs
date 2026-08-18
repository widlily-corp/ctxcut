//! AST-guided patch and replace engine.
//!
//! Provides surgical symbol location, indentation normalization, pre-write syntax validation,
//! unified diff generation, and atomic live updates.

pub mod formatting;
pub mod locator;
pub mod validate;

use crate::error::{CoreError, Result};
use crate::lang::{LanguageAdapter, LanguageRegistry};
use crate::model::{PatchResult, SupportedLanguage};
use crate::parser::ParserManager;
use std::fs;
use std::io::Write;
use std::path::Path;

pub use formatting::{
    detect_node_base_indentation, generate_unified_diff, normalize_indentation,
    reindent_for_splice, LineEnding,
};
pub use locator::{AstNodeLocator, LocatedNode};
pub use validate::SyntaxValidator;

/// Bidirectional AST Patcher for surgically replacing functions, classes, and methods.
pub struct AstPatcher;

impl AstPatcher {
    /// Surgically patches a symbol in a file on disk.
    ///
    /// If `dry_run` is `false`, changes are atomically persisted to disk via a temporary file.
    /// If `dry_run` is `true`, no disk writes occur and `PatchResult.applied` is `false`.
    pub fn patch_symbol(
        file_path: &Path,
        symbol_query: &str,
        replacement_code: &str,
        dry_run: bool,
    ) -> Result<PatchResult> {
        let source = fs::read_to_string(file_path).map_err(|e| CoreError::Io {
            path: file_path.to_path_buf(),
            source: e,
        })?;

        let adapter = LanguageRegistry::for_path(file_path)?;
        let (mut patch_result, full_patched_source) = Self::patch_source_internal(
            &source,
            &*adapter,
            file_path,
            symbol_query,
            replacement_code,
        )?;

        if !dry_run {
            let parent_dir = file_path.parent().unwrap_or_else(|| Path::new("."));
            let mut temp_file = tempfile::Builder::new()
                .prefix(".ctxcut_patch_")
                .tempfile_in(parent_dir)
                .map_err(|e| CoreError::Io {
                    path: file_path.to_path_buf(),
                    source: e,
                })?;

            temp_file
                .write_all(full_patched_source.as_bytes())
                .map_err(|e| CoreError::Io {
                    path: file_path.to_path_buf(),
                    source: e,
                })?;

            temp_file.flush().map_err(|e| CoreError::Io {
                path: file_path.to_path_buf(),
                source: e,
            })?;

            temp_file.persist(file_path).map_err(|e| CoreError::Io {
                path: file_path.to_path_buf(),
                source: e.error,
            })?;

            patch_result.applied = true;
        }

        Ok(patch_result)
    }

    /// Surgically patches a symbol in source code in-memory, returning the `PatchResult`.
    pub fn patch_source(
        source: &str,
        language: SupportedLanguage,
        file_path: &Path,
        symbol_query: &str,
        replacement_code: &str,
    ) -> Result<PatchResult> {
        let adapter = LanguageRegistry::for_language(language)?;
        let (patch_result, _) = Self::patch_source_internal(
            source,
            &*adapter,
            file_path,
            symbol_query,
            replacement_code,
        )?;
        Ok(patch_result)
    }

    /// Internal core patch engine that calculates byte offsets, re-indents, validates syntax, and returns diff.
    fn patch_source_internal(
        source: &str,
        adapter: &dyn LanguageAdapter,
        file_path: &Path,
        symbol_query: &str,
        replacement_code: &str,
    ) -> Result<(PatchResult, String)> {
        let ts_lang = adapter.tree_sitter_language(file_path);
        let tree = ParserManager::parse_source(source, &ts_lang, file_path)?;

        let located =
            AstNodeLocator::locate(tree.root_node(), source, symbol_query, adapter, file_path)?;

        let (start, end) = located.byte_range;
        if start > end || end > source.len() {
            return Err(CoreError::PatchRangeError {
                path: file_path.to_path_buf(),
                start,
                end,
                total_bytes: source.len(),
            });
        }

        let line_ending = LineEnding::detect(source);
        let aligned_replacement =
            reindent_for_splice(replacement_code, &located.base_indentation, line_ending);

        let full_patched_source = format!(
            "{}{}{}",
            &source[..start],
            aligned_replacement,
            &source[end..]
        );

        // Pre-write syntax validation guard
        SyntaxValidator::validate_source(&full_patched_source, &ts_lang, file_path)?;

        // Generate unified diff preview
        let diff = generate_unified_diff(source, &full_patched_source, file_path, 3);
        let original_code = source[start..end].to_string();

        let patch_result = PatchResult {
            file_path: file_path.to_path_buf(),
            symbol_name: located.symbol_name,
            original_code,
            patched_code: aligned_replacement,
            byte_range: (start, end),
            diff,
            applied: false,
        };

        Ok((patch_result, full_patched_source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_patch_source_rust_fn() {
        let original = r"
fn calculate_total(price: f64, tax_rate: f64) -> f64 {
    price * (1.0 + tax_rate)
}
";
        let replacement = r"fn calculate_total(price: f64, tax_rate: f64) -> f64 {
    let subtotal = price * (1.0 + tax_rate);
    subtotal.round()
}";

        let res = AstPatcher::patch_source(
            original,
            SupportedLanguage::Rust,
            &PathBuf::from("src/calc.rs"),
            "calculate_total",
            replacement,
        )
        .unwrap();

        assert_eq!(res.symbol_name, "calculate_total");
        assert!(!res.applied);
        assert!(res.diff.contains("+    let subtotal"));
    }
}
