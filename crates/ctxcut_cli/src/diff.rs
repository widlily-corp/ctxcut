//! Git diff contextualizer module.

use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{bail, Result};
use ctxcut_core::{ContextSlicer, LanguageRegistry, ParserManager, SliceOptions, SliceResult};
use tree_sitter::Node;

/// Extracts contextual slices for symbols modified in Git diff.
pub fn run_diff_slicer(staged: bool, opts: &SliceOptions) -> Result<Vec<SliceResult>> {
    let diff_output = get_git_diff(staged)?;
    if diff_output.trim().is_empty() {
        return Ok(Vec::new());
    }

    let modified_ranges = parse_git_diff(&diff_output);
    if modified_ranges.is_empty() {
        return Ok(Vec::new());
    }

    let slicer = ContextSlicer::new();
    let mut results = Vec::new();

    for (file_rel_path, changed_lines) in modified_ranges {
        let file_path = PathBuf::from(&file_rel_path);
        if !file_path.exists() {
            continue;
        }

        let Ok(source) = std::fs::read_to_string(&file_path) else {
            continue;
        };

        let Ok(adapter) = LanguageRegistry::for_path(&file_path) else {
            continue;
        };

        let ts_lang = adapter.tree_sitter_language(&file_path);
        let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, &file_path) else {
            continue;
        };

        let root = tree.root_node();
        let symbols = find_symbols_intersecting_lines(root, &source, &changed_lines, &file_path, &*adapter);

        for sym_name in symbols {
            if let Ok(slice) = slicer.slice_symbol(&file_path, &sym_name, opts) {
                results.push(slice);
            }
        }
    }

    Ok(results)
}

fn get_git_diff(staged: bool) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.arg("diff");
    if staged {
        cmd.arg("--staged");
    }
    cmd.arg("-U0"); // zero context lines for precise line mapping

    let output = cmd.output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("Git diff failed: {}", err);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_git_diff(diff: &str) -> Vec<(String, Vec<usize>)> {
    let mut results: Vec<(String, Vec<usize>)> = Vec::new();
    let mut current_file: Option<String> = None;
    let mut current_lines: Vec<usize> = Vec::new();

    for line in diff.lines() {
        if line.starts_with("+++ b/") {
            if let Some(file) = current_file.take() {
                if !current_lines.is_empty() {
                    results.push((file, current_lines));
                    current_lines = Vec::new();
                }
            }
            current_file = Some(line[6..].to_string());
        } else if line.starts_with("@@ ") {
            // e.g. @@ -10,3 +10,5 @@ or @@ -5 +5,2 @@
            if let Some(hunk) = parse_hunk_header(line) {
                for l in hunk.0..hunk.0 + hunk.1.max(1) {
                    current_lines.push(l);
                }
            }
        }
    }

    if let Some(file) = current_file {
        if !current_lines.is_empty() {
            results.push((file, current_lines));
        }
    }

    results
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    // Format: @@ -old,count +new,count @@
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    let new_part = parts[2]; // +new,count
    if !new_part.starts_with('+') {
        return None;
    }

    let range_str = &new_part[1..];
    if let Some((start_s, count_s)) = range_str.split_once(',') {
        let start = start_s.parse::<usize>().ok()?;
        let count = count_s.parse::<usize>().ok()?;
        Some((start, count))
    } else {
        let start = range_str.parse::<usize>().ok()?;
        Some((start, 1))
    }
}

fn find_symbols_intersecting_lines(
    root: Node<'_>,
    source: &str,
    changed_lines: &[usize],
    file_path: &Path,
    adapter: &dyn ctxcut_core::LanguageAdapter,
) -> Vec<String> {
    let mut matched = Vec::new();
    let symbols = adapter.list_symbols(root, source);

    for sym in symbols {
        let clean_name = sym.split('.').last().unwrap_or(&sym);
        let Ok((extracted, _)) = adapter.locate_symbol(root, source, clean_name, file_path) else {
            continue;
        };

        let sym_start = extracted.start_line;
        let sym_end = extracted.end_line;

        let intersects = changed_lines.iter().any(|&l| l >= sym_start && l <= sym_end);
        if intersects && !matched.contains(&clean_name.to_string()) {
            matched.push(clean_name.to_string());
        }
    }

    // Fallback: If no top-level symbol matched but lines changed, search any top-level functions or classes
    if matched.is_empty() {
        for sym in adapter.list_symbols(root, source) {
            let clean_name = sym.split('.').last().unwrap_or(&sym);
            if let Ok((extracted, _)) = adapter.locate_symbol(root, source, clean_name, file_path) {
                if changed_lines.iter().any(|&l| l >= extracted.start_line && l <= extracted.end_line) {
                    if !matched.contains(&clean_name.to_string()) {
                        matched.push(clean_name.to_string());
                    }
                }
            }
        }
    }

    matched
}
