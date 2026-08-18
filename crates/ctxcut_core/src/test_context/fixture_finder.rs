//! Reference test fixture and pattern discovery engine (Milestone 6).

use crate::model::DiscoveredFixture;
use std::fs;
use std::path::Path;

/// Scans nearby directories for existing test patterns and reference fixtures.
pub struct FixtureFinder;

impl FixtureFinder {
    /// Discovers nearby reference test files and extracts representative test snippets.
    pub fn find_fixtures(target_file: &Path) -> Vec<DiscoveredFixture> {
        let mut fixtures = Vec::new();
        let target_ext = target_file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let start_dir = target_file.parent().unwrap_or_else(|| Path::new("."));

        // 1. Look in adjacent directory and up to 3 parent directories
        let mut curr_dir = Some(start_dir);
        let mut search_dirs = Vec::new();

        for _ in 0..4 {
            if let Some(d) = curr_dir {
                search_dirs.push(d.to_path_buf());
                search_dirs.push(d.join("tests"));
                search_dirs.push(d.join("test"));
                search_dirs.push(d.join("__tests__"));
                curr_dir = d.parent();
            } else {
                break;
            }
        }

        for dir in search_dirs {
            if !dir.is_dir() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path != target_file && is_test_file(&path, target_ext) {
                        if let Ok(snippet) = extract_test_snippet(&path) {
                            fixtures.push(DiscoveredFixture {
                                file_path: path.to_string_lossy().to_string(),
                                snippet,
                            });
                            if fixtures.len() >= 3 {
                                return fixtures;
                            }
                        }
                    }
                }
            }
        }

        fixtures
    }
}

fn is_test_file(path: &Path, expected_ext: &str) -> bool {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !expected_ext.is_empty() && ext != expected_ext.to_lowercase() {
        return false;
    }

    file_name.contains("test")
        || file_name.contains("spec")
        || file_name.starts_with("test_")
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.rs")
        || file_name.ends_with(".test.ts")
        || file_name.ends_with(".spec.ts")
        || file_name.ends_with(".test.js")
        || file_name.ends_with(".spec.js")
}

fn extract_test_snippet(path: &Path) -> std::io::Result<String> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() <= 25 {
        return Ok(lines.join("\n"));
    }

    // Find the first test function/block
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[test]")
            || trimmed.starts_with("def test_")
            || trimmed.starts_with("it(")
            || trimmed.starts_with("test(")
            || trimmed.starts_with("func Test")
        {
            let end_idx = (idx + 20).min(lines.len());
            return Ok(lines[idx..end_idx].join("\n"));
        }
    }

    // Fallback first 20 lines
    Ok(lines[..20.min(lines.len())].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_fixture_finder_discovers_adjacent_test_file() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("calculator.ts");
        let test_file = dir.path().join("calculator.test.ts");

        fs::write(
            &src_file,
            "export function add(a: number, b: number) { return a + b; }",
        )
        .unwrap();
        fs::write(
            &test_file,
            "import { describe, it, expect } from 'vitest';\n\ndescribe('add', () => {\n  it('adds two numbers', () => {\n    expect(add(1, 2)).toBe(3);\n  });\n});",
        )
        .unwrap();

        let fixtures = FixtureFinder::find_fixtures(&src_file);
        assert!(!fixtures.is_empty());
        assert!(fixtures[0].file_path.contains("calculator.test.ts"));
        assert!(fixtures[0].snippet.contains("describe('add'"));
    }
}
