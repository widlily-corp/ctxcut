//! Smart repository traversal, blacklist pruning, and ignore engine.

pub mod binary;
pub mod blacklist;
pub mod config;
pub mod fast_stats;
pub mod walker;

pub use binary::{is_binary_bytes, is_binary_file};
pub use blacklist::{
    is_blacklisted_file, is_ignored_directory, DEFAULT_IGNORED_DIRS, DEFAULT_IGNORED_FILES,
};
pub use config::TraversalConfig;
pub use fast_stats::{estimate_sliced_tokens, FastFileStatItem, FastStatsReport, LanguageStatItem};
pub use walker::ProjectWalker;
