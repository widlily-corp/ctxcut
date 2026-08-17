//! Built-in default ignore blacklists for vendor directories and non-code files.

use std::path::Path;

/// Default directory blacklist automatically pruned during traversal.
pub const DEFAULT_IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    ".pytest_cache",
    ".venv",
    "venv",
    "env",
    ".git",
    ".next",
    ".turbo",
    "build",
    "out",
    "coverage",
    "__pycache__",
    ".mypy_cache",
    ".tox",
    "vendor",
    ".idea",
    ".vscode",
];

/// Default file blacklist patterns ignored during file collection.
pub const DEFAULT_IGNORED_FILES: &[&str] = &[
    "*.lock",
    "package-lock.json",
    "Cargo.lock",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "*.min.js",
    "*.bundle.js",
    "*.map",
    "*.wasm",
    "*.pyc",
];

/// Determines if a directory name should be ignored based on default and custom blacklists.
#[must_use]
pub fn is_ignored_directory(dir_name: &str, custom: &[String]) -> bool {
    DEFAULT_IGNORED_DIRS.contains(&dir_name) || custom.iter().any(|d| d == dir_name)
}

/// Determines if a file name matches default or custom ignore patterns.
#[must_use]
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn is_blacklisted_file(file_name: &str, custom: &[String]) -> bool {
    if matches_default_file_blacklist(file_name) {
        return true;
    }
    custom.iter().any(|pat| matches_pattern(file_name, pat))
}

fn matches_default_file_blacklist(name: &str) -> bool {
    if matches!(
        name,
        "package-lock.json" | "Cargo.lock" | "yarn.lock" | "pnpm-lock.yaml" | "poetry.lock"
    ) {
        return true;
    }

    if let Some(ext) = Path::new(name).extension() {
        if ext.eq_ignore_ascii_case("lock")
            || ext.eq_ignore_ascii_case("map")
            || ext.eq_ignore_ascii_case("wasm")
            || ext.eq_ignore_ascii_case("pyc")
        {
            return true;
        }
    }

    name.ends_with(".min.js") || name.ends_with(".bundle.js")
}

fn matches_pattern(name: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix('*') {
        name.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        name == pattern
    }
}
