//! Normalized Snapshot utilities for ctxcut cross-platform golden testing.
//!
//! Normalizes line endings (CRLF -> LF), Windows backslash file paths to Unix forward slashes,
//! and trims non-deterministic trailing whitespace to ensure snapshots pass identically
//! on Windows, macOS, and Linux.

/// Cross-platform snapshot normalizer for deterministic Markdown and text comparisons.
pub struct NormalizedSnapshot;

impl NormalizedSnapshot {
    /// Normalizes CRLF and CR line endings to standard Unix LF (`\n`).
    pub fn normalize_line_endings(input: &str) -> String {
        input.replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Normalizes Windows path separators `\` to Unix `/` in markdown headers and source paths.
    pub fn normalize_paths(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        for line in input.lines() {
            if line.starts_with("# File:")
                || line.starts_with("### File:")
                || line.starts_with("<!-- File:")
                || line.starts_with("path:")
                || line.starts_with("file_path:")
                || line.contains("tests\\fixtures\\")
                || line.contains("src\\")
                || line.contains("crates\\")
            {
                result.push_str(&line.replace('\\', "/"));
            } else {
                result.push_str(line);
            }
            result.push('\n');
        }
        result
    }

    /// Trims trailing whitespace from each line.
    pub fn trim_line_endings(input: &str) -> String {
        input
            .lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Performs full cross-platform normalization on markdown or text snapshots.
    pub fn normalize(input: &str) -> String {
        let normalized_lf = Self::normalize_line_endings(input);
        let normalized_paths = Self::normalize_paths(&normalized_lf);
        let trimmed = Self::trim_line_endings(&normalized_paths);
        trimmed.trim().to_string()
    }

    /// Asserts that two snapshot strings are equal after cross-platform normalization.
    pub fn assert_eq_normalized(actual: &str, expected: &str) {
        let norm_actual = Self::normalize(actual);
        let norm_expected = Self::normalize(expected);
        assert_eq!(
            norm_actual, norm_expected,
            "Normalized snapshot mismatch!\n\n--- ACTUAL ---\n{}\n\n--- EXPECTED ---\n{}\n",
            norm_actual, norm_expected
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_crlf_and_trailing_whitespace() {
        let input = "# Header  \r\n\r\nLine 1   \r\nLine 2\t\r\n";
        let normalized = NormalizedSnapshot::normalize(input);
        assert_eq!(normalized, "# Header\n\nLine 1\nLine 2");
    }

    #[test]
    fn test_normalize_windows_paths() {
        let input = "### File: tests\\fixtures\\typescript\\order.ts\r\nLine 1\r\n";
        let normalized = NormalizedSnapshot::normalize(input);
        assert_eq!(
            normalized,
            "### File: tests/fixtures/typescript/order.ts\nLine 1"
        );
    }

    #[test]
    fn test_assert_eq_normalized() {
        let win = "# Title\r\n### File: tests\\fixtures\\rust\\lib.rs\r\nfn test() {}\r\n";
        let unix = "# Title\n### File: tests/fixtures/rust/lib.rs\nfn test() {}\n";
        NormalizedSnapshot::assert_eq_normalized(win, unix);
    }
}
