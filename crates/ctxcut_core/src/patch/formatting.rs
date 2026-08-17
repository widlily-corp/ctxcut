//! Formatting preservation, indentation normalization, line ending preservation, and unified diff generation.

use similar::TextDiff;
use std::path::Path;

/// Supported line ending conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix/Linux newline (`\n`).
    Lf,
    /// Windows newline (`\r\n`).
    Crlf,
}

impl LineEnding {
    /// Detect the primary line ending used in the source string.
    pub fn detect(source: &str) -> Self {
        if source.contains("\r\n") {
            Self::Crlf
        } else {
            Self::Lf
        }
    }

    /// Returns the string representation of the line ending.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// Detects the base indentation (spaces or tabs) of the line containing `start_byte` in `source`.
///
/// Scans backward from `start_byte` to find the start of the line and extracts all leading
/// whitespace characters (`' '` and `'\t'`).
pub fn detect_node_base_indentation(source: &str, start_byte: usize) -> &str {
    let clamped_start = start_byte.min(source.len());
    let before = &source[..clamped_start];
    let line_start = before.rfind('\n').map_or(0, |idx| idx + 1);
    let prefix = &source[line_start..clamped_start];

    let ws_len = prefix
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(char::len_utf8)
        .sum();

    &prefix[..ws_len]
}

/// Finds the longest common leading whitespace prefix among all non-empty lines.
pub fn find_common_indentation<'a>(lines: &[&'a str]) -> &'a str {
    let non_empty: Vec<&'a str> = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();

    if non_empty.is_empty() {
        return "";
    }

    let first_ws_len = non_empty[0]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(char::len_utf8)
        .sum();
    let mut common = &non_empty[0][..first_ws_len];

    for line in &non_empty[1..] {
        let mut matching_bytes = 0;
        for (c1, c2) in common.chars().zip(line.chars()) {
            if c1 == c2 && (c1 == ' ' || c1 == '\t') {
                matching_bytes += c1.len_utf8();
            } else {
                break;
            }
        }
        common = &common[..matching_bytes];
        if common.is_empty() {
            break;
        }
    }

    common
}

/// Re-indents replacement code for direct byte-range splicing into `source[start_byte..end_byte]`.
///
/// Because `&source[..start_byte]` already contains the target line's base indentation,
/// Line 0 of the replacement does NOT have `base_indent` prepended. Lines 1..N receive `base_indent`.
/// Blank lines are trimmed of all whitespace.
pub fn reindent_for_splice(
    replacement: &str,
    base_indent: &str,
    line_ending: LineEnding,
) -> String {
    if replacement.is_empty() {
        return String::new();
    }

    let raw_lines: Vec<&str> = replacement.lines().collect();
    if raw_lines.is_empty() {
        return String::new();
    }

    let common_indent = find_common_indentation(&raw_lines);
    let mut result = Vec::with_capacity(raw_lines.len());

    for (i, line) in raw_lines.iter().enumerate() {
        if line.trim().is_empty() {
            result.push(String::new());
            continue;
        }

        let stripped = line
            .strip_prefix(common_indent)
            .unwrap_or_else(|| line.trim_start());

        if i == 0 {
            // Line 0 is spliced directly at start_byte, which already follows base_indent in source
            result.push(stripped.to_string());
        } else {
            // Lines 1..N need the enclosing base_indent prepended
            result.push(format!("{base_indent}{stripped}"));
        }
    }

    result.join(line_ending.as_str())
}

/// Normalizes replacement code so that every non-empty line (including Line 0) has `base_indent`.
pub fn normalize_indentation(
    replacement: &str,
    base_indent: &str,
    line_ending: LineEnding,
) -> String {
    if replacement.is_empty() {
        return String::new();
    }

    let raw_lines: Vec<&str> = replacement.lines().collect();
    if raw_lines.is_empty() {
        return String::new();
    }

    let common_indent = find_common_indentation(&raw_lines);
    let mut result = Vec::with_capacity(raw_lines.len());

    for line in raw_lines {
        if line.trim().is_empty() {
            result.push(String::new());
            continue;
        }

        let stripped = line
            .strip_prefix(common_indent)
            .unwrap_or_else(|| line.trim_start());

        result.push(format!("{base_indent}{stripped}"));
    }

    result.join(line_ending.as_str())
}

/// Generates a standard unified diff between `original` and `patched` source text.
pub fn generate_unified_diff(
    original: &str,
    patched: &str,
    file_path: &Path,
    context_radius: usize,
) -> String {
    let file_str = file_path.to_string_lossy();
    let diff = TextDiff::from_lines(original, patched);
    diff.unified_diff()
        .context_radius(context_radius)
        .header(&format!("a/{file_str}"), &format!("b/{file_str}"))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_line_ending() {
        assert_eq!(LineEnding::detect("fn main() {\n}\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("fn main() {\r\n}\r\n"), LineEnding::Crlf);
        assert_eq!(LineEnding::Lf.as_str(), "\n");
        assert_eq!(LineEnding::Crlf.as_str(), "\r\n");
    }

    #[test]
    fn test_detect_base_indentation() {
        let src = "struct Foo {\n    pub fn bar() {}\n}\n";
        let start = src.find("pub fn").unwrap();
        assert_eq!(detect_node_base_indentation(src, start), "    ");

        let src_tab = "func main() {\n\t\tfmt.Println(\"hi\")\n}\n";
        let start_tab = src_tab.find("fmt.Println").unwrap();
        assert_eq!(detect_node_base_indentation(src_tab, start_tab), "\t\t");

        let src_top = "fn top() {}";
        assert_eq!(detect_node_base_indentation(src_top, 0), "");
    }

    #[test]
    fn test_find_common_indentation() {
        let lines = vec!["    def foo():", "        return 42", "    "];
        assert_eq!(find_common_indentation(&lines), "    ");

        let empty: Vec<&str> = vec![];
        assert_eq!(find_common_indentation(&empty), "");
    }

    #[test]
    fn test_reindent_for_splice() {
        let code = "def foo():\n    return 42";
        let spliced = reindent_for_splice(code, "    ", LineEnding::Lf);
        assert_eq!(spliced, "def foo():\n        return 42");

        let code_overindented = "        def foo():\n            return 42";
        let spliced_over = reindent_for_splice(code_overindented, "    ", LineEnding::Lf);
        assert_eq!(spliced_over, "def foo():\n        return 42");
    }

    #[test]
    fn test_normalize_indentation() {
        let code = "def foo():\n    return 42";
        let norm = normalize_indentation(code, "    ", LineEnding::Lf);
        assert_eq!(norm, "    def foo():\n        return 42");
    }

    #[test]
    fn test_generate_unified_diff() {
        let orig = "fn foo() -> i32 {\n    41\n}\n";
        let patched = "fn foo() -> i32 {\n    42\n}\n";
        let diff = generate_unified_diff(orig, patched, &PathBuf::from("src/lib.rs"), 3);
        assert!(diff.contains("--- a/src/lib.rs"));
        assert!(diff.contains("+++ b/src/lib.rs"));
        assert!(diff.contains("-    41"));
        assert!(diff.contains("+    42"));
    }
}
