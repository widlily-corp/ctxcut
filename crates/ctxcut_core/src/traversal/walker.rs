//! Project walker orchestrating ignore-rule evaluation and directory traversal.

use crate::error::Result;
use crate::traversal::binary::is_binary_file;
use crate::traversal::blacklist::{is_blacklisted_file, is_ignored_directory};
use crate::traversal::config::TraversalConfig;
use crate::traversal::fast_stats::{estimate_fast_stats_impl, FastStatsReport};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// High-performance directory walker honoring gitignore, ctxcutignore, and built-in blacklists.
#[derive(Debug, Clone, Default)]
pub struct ProjectWalker;

impl ProjectWalker {
    /// Creates a new ProjectWalker instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Walks the directory tree starting at `root`, returning an iterator over valid file paths.
    pub fn walk(root: &Path, config: &TraversalConfig) -> impl Iterator<Item = PathBuf> {
        Self::collect_files(root, config).into_iter()
    }

    /// Collects all valid file paths matching the configuration into a `Vec<PathBuf>`.
    #[must_use]
    pub fn collect_files(root: &Path, config: &TraversalConfig) -> Vec<PathBuf> {
        if root.is_file() {
            if is_binary_file(root) {
                return Vec::new();
            }
            return vec![root.to_path_buf()];
        }

        let mut builder = WalkBuilder::new(root);
        builder
            .hidden(!config.include_hidden)
            .parents(true)
            .git_ignore(config.respect_gitignore)
            .git_global(config.respect_gitignore)
            .git_exclude(config.respect_gitignore)
            .follow_links(config.follow_symlinks);

        if config.respect_ctxcutignore {
            builder.add_custom_ignore_filename(".ctxcutignore");
        }

        let custom_dirs = config.custom_ignored_dirs.clone();
        builder.filter_entry(move |entry| {
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let name = entry.file_name().to_string_lossy();
                if is_ignored_directory(&name, &custom_dirs) {
                    return false;
                }
            }
            true
        });

        let walker = builder.build();
        let mut result = Vec::new();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let file_name = path
                .file_name()
                .map(|f| f.to_string_lossy())
                .unwrap_or_default();
            if is_blacklisted_file(&file_name, &config.custom_ignored_files) {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                if meta.len() > config.max_file_size_bytes {
                    continue;
                }
            }

            if is_binary_file(path) {
                continue;
            }

            result.push(path.to_path_buf());
        }

        result
    }

    /// Executes a fast token estimation scan over the target repository or file.
    pub fn estimate_fast_stats(root: &Path, timeout_secs: Option<u64>) -> Result<FastStatsReport> {
        let config = TraversalConfig::default();
        estimate_fast_stats_impl(root, &config, timeout_secs)
    }

    /// Executes a fast token estimation scan with a custom configuration.
    pub fn estimate_fast_stats_with_config(
        root: &Path,
        config: &TraversalConfig,
        timeout_secs: Option<u64>,
    ) -> Result<FastStatsReport> {
        estimate_fast_stats_impl(root, config, timeout_secs)
    }
}
