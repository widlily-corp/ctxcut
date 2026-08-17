//! Traversal configuration options for filtering and directory walking.

use serde::{Deserialize, Serialize};

/// Configuration controlling repository file traversal and ignore-rule filtering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversalConfig {
    /// Whether to respect `.gitignore` files encountered during traversal (default: `true`).
    pub respect_gitignore: bool,
    /// Whether to respect `.ctxcutignore` files encountered during traversal (default: `true`).
    pub respect_ctxcutignore: bool,
    /// Maximum allowed file size in bytes before skipping (default: 10 MB = 10,485,760 bytes).
    pub max_file_size_bytes: u64,
    /// Custom directory names to ignore in addition to default blacklist.
    pub custom_ignored_dirs: Vec<String>,
    /// Custom file patterns to ignore in addition to default blacklist.
    pub custom_ignored_files: Vec<String>,
    /// Whether to include hidden files and directories (default: `false`).
    pub include_hidden: bool,
    /// Whether to follow filesystem symlinks (default: `false`).
    pub follow_symlinks: bool,
}

impl Default for TraversalConfig {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            respect_ctxcutignore: true,
            max_file_size_bytes: 10 * 1024 * 1024,
            custom_ignored_dirs: Vec::new(),
            custom_ignored_files: Vec::new(),
            include_hidden: false,
            follow_symlinks: false,
        }
    }
}

impl TraversalConfig {
    /// Creates a new default traversal configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to respect `.gitignore` files.
    #[must_use]
    pub fn with_respect_gitignore(mut self, respect: bool) -> Self {
        self.respect_gitignore = respect;
        self
    }

    /// Sets whether to respect `.ctxcutignore` files.
    #[must_use]
    pub fn with_respect_ctxcutignore(mut self, respect: bool) -> Self {
        self.respect_ctxcutignore = respect;
        self
    }

    /// Sets the maximum allowed file size in bytes.
    #[must_use]
    pub fn with_max_file_size_bytes(mut self, max_bytes: u64) -> Self {
        self.max_file_size_bytes = max_bytes;
        self
    }

    /// Adds custom ignored directories.
    #[must_use]
    pub fn with_custom_ignored_dirs(mut self, dirs: Vec<String>) -> Self {
        self.custom_ignored_dirs = dirs;
        self
    }

    /// Adds custom ignored file patterns.
    #[must_use]
    pub fn with_custom_ignored_files(mut self, files: Vec<String>) -> Self {
        self.custom_ignored_files = files;
        self
    }

    /// Sets whether to include hidden files.
    #[must_use]
    pub fn with_include_hidden(mut self, include: bool) -> Self {
        self.include_hidden = include;
        self
    }

    /// Sets whether to follow symlinks.
    #[must_use]
    pub fn with_follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }
}
