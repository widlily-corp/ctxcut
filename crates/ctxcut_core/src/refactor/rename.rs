//! High-performance, AST-accurate multi-file symbol renamer engine.

use crate::error::{CoreError, Result};
use crate::lang::LanguageRegistry;
use crate::model::SupportedLanguage;
use crate::parser::ParserManager;
use crate::patch::formatting::generate_unified_diff;
use crate::patch::validate::SyntaxValidator;
use crate::refactor::{
    FileRenameResult, MultiFileRenameResult, RenameTargetKind, SymbolRenameOccurrence,
};
use crate::traversal::{ProjectWalker, TraversalConfig};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Engine for multi-file AST symbol renaming across workspace.
pub struct SymbolRenamer;

impl SymbolRenamer {
    /// Renames a target symbol across the workspace.
    pub fn rename_symbol(
        workspace_root: &Path,
        target: &str,
        new_name: &str,
        dry_run: bool,
    ) -> Result<MultiFileRenameResult> {
        let (file_hint, old_symbol_name) = parse_target_string(target);
        let raw_new_name = extract_base_identifier(new_name);

        // 1. Locate the declaring file and target AST declaration
        let (declaring_path, _target_kind) =
            locate_declaration(workspace_root, file_hint.as_deref(), old_symbol_name);

        let all_files = ProjectWalker::collect_files(workspace_root, &TraversalConfig::default());
        let mut file_results = Vec::new();
        let mut total_occurrences = 0;
        let mut pending_writes: Vec<(PathBuf, String)> = Vec::new();

        // 2. Scan each file in workspace for declaration, imports, and call sites
        for file_path in &all_files {
            let Some(lang) = SupportedLanguage::from_path(file_path) else {
                continue;
            };

            let Ok(source) = fs::read_to_string(file_path) else {
                continue;
            };

            // Fast pre-filter: skip if file does not contain old_symbol_name
            if !source.contains(old_symbol_name) {
                continue;
            }

            let Ok(adapter) = LanguageRegistry::for_language(lang) else {
                continue;
            };
            let ts_lang = adapter.tree_sitter_language(file_path);

            let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, file_path) else {
                continue;
            };

            let is_declaring_file = match &declaring_path {
                Some(p) => p == file_path,
                None => false,
            };

            let rel_path = file_path
                .strip_prefix(workspace_root)
                .unwrap_or(file_path)
                .to_string_lossy()
                .replace('\\', "/");

            let occurrences = collect_file_rename_occurrences(
                tree.root_node(),
                &source,
                is_declaring_file,
                old_symbol_name,
            );

            if occurrences.is_empty() {
                continue;
            }

            // 3. Apply edits in descending order of byte start offsets
            let patched_source = apply_edits_reverse(&source, &occurrences, raw_new_name);

            // 4. Pre-write syntax validation guard
            SyntaxValidator::validate_source(&patched_source, &ts_lang, file_path)?;

            let diff = generate_unified_diff(&source, &patched_source, file_path, 3);
            let occurrences_count = occurrences.len();
            total_occurrences += occurrences_count;

            let occurrence_items: Vec<SymbolRenameOccurrence> = occurrences
                .iter()
                .map(|(start, end, kind)| {
                    let (line, col) = get_line_col(&source, *start);
                    let snippet = get_snippet(&source, *start, *end);
                    SymbolRenameOccurrence {
                        line,
                        column: col,
                        kind: kind.clone(),
                        snippet,
                    }
                })
                .collect();

            file_results.push(FileRenameResult {
                file_path: rel_path,
                occurrences_count,
                occurrences: occurrence_items,
                diff,
                applied: !dry_run,
            });

            pending_writes.push((file_path.clone(), patched_source));
        }

        // 5. Atomic persistence if !dry_run
        if !dry_run {
            for (file_path, patched_content) in pending_writes {
                atomic_write_file(&file_path, &patched_content)?;
            }
        }

        let target_file_rel = declaring_path.map(|p| {
            p.strip_prefix(workspace_root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/")
        });

        Ok(MultiFileRenameResult {
            old_name: old_symbol_name.to_string(),
            new_name: raw_new_name.to_string(),
            target_file: target_file_rel,
            total_files_modified: file_results.len(),
            total_occurrences,
            files: file_results,
            dry_run,
        })
    }
}

fn parse_target_string(target: &str) -> (Option<String>, &str) {
    let search_start = if target.len() >= 2
        && target.as_bytes()[1] == b':'
        && target.as_bytes()[0].is_ascii_alphabetic()
    {
        2
    } else {
        0
    };

    if let Some(colon_idx) = target[search_start..].find(':') {
        let actual_idx = search_start + colon_idx;
        (
            Some(target[..actual_idx].to_string()),
            &target[actual_idx + 1..],
        )
    } else {
        (None, target)
    }
}

fn extract_base_identifier(new_name: &str) -> &str {
    let trimmed = new_name.trim();
    if let Some(colon_idx) = trimmed.rfind(':') {
        &trimmed[colon_idx + 1..]
    } else {
        trimmed
    }
}

fn locate_declaration(
    workspace_root: &Path,
    file_hint: Option<&str>,
    symbol_name: &str,
) -> (Option<PathBuf>, RenameTargetKind) {
    if let Some(hint) = file_hint {
        let p = if Path::new(hint).is_absolute() {
            PathBuf::from(hint)
        } else {
            workspace_root.join(hint)
        };
        if p.exists() {
            return (Some(p), RenameTargetKind::Function);
        }
    }

    // Search workspace
    let all_files = ProjectWalker::collect_files(workspace_root, &TraversalConfig::default());
    for file_path in all_files {
        let Some(lang) = SupportedLanguage::from_path(&file_path) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(&file_path) else {
            continue;
        };
        if !source.contains(symbol_name) {
            continue;
        }

        let Ok(adapter) = LanguageRegistry::for_language(lang) else {
            continue;
        };
        let ts_lang = adapter.tree_sitter_language(&file_path);
        let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, &file_path) else {
            continue;
        };

        if let Ok((_sym, _node)) =
            adapter.locate_symbol(tree.root_node(), &source, symbol_name, &file_path)
        {
            return (Some(file_path), RenameTargetKind::Function);
        }
    }

    (None, RenameTargetKind::Unknown)
}

fn collect_file_rename_occurrences(
    root: Node,
    source: &str,
    is_declaring_file: bool,
    old_symbol_name: &str,
) -> Vec<(usize, usize, String)> {
    let mut occurrences = Vec::new();
    traverse_ast_for_rename(
        root,
        source,
        is_declaring_file,
        old_symbol_name,
        &mut occurrences,
    );
    occurrences
}

fn traverse_ast_for_rename(
    node: Node,
    source: &str,
    is_declaring_file: bool,
    old_symbol_name: &str,
    occurrences: &mut Vec<(usize, usize, String)>,
) {
    let kind = node.kind();

    // 1. Skip comments and string literals
    if is_comment_or_string(kind) {
        return;
    }

    // 2. Check if node is an identifier matching old_symbol_name
    if is_identifier_node(kind) {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        if text == old_symbol_name {
            // Check if node is shadowed locally
            if !is_node_shadowed(node, source, old_symbol_name, is_declaring_file) {
                let occurrence_kind = classify_occurrence_kind(node);
                occurrences.push((node.start_byte(), node.end_byte(), occurrence_kind));
            }
            return;
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        traverse_ast_for_rename(
            child,
            source,
            is_declaring_file,
            old_symbol_name,
            occurrences,
        );
    }
}

fn is_comment_or_string(kind: &str) -> bool {
    kind.contains("comment")
        || kind.contains("string")
        || kind.contains("literal_string")
        || kind == "template_string"
        || kind == "template_literal"
        || kind == "string_fragment"
        || kind == "raw_string_literal"
        || kind == "line_comment"
        || kind == "block_comment"
}

fn is_identifier_node(kind: &str) -> bool {
    kind == "identifier"
        || kind == "type_identifier"
        || kind == "property_identifier"
        || kind == "field_identifier"
        || kind == "shorthand_property_identifier"
        || kind == "shorthand_property_identifier_pattern"
}

fn classify_occurrence_kind(node: Node) -> String {
    let mut current = node.parent();
    while let Some(p) = current {
        let p_kind = p.kind();
        if p_kind.contains("import") {
            return "import_specifier".to_string();
        }
        if p_kind.contains("export") {
            return "reexport".to_string();
        }
        if p_kind.contains("call") || p_kind.contains("invocation") {
            return "call_site".to_string();
        }
        if p_kind.contains("function")
            || p_kind.contains("method")
            || p_kind.contains("class")
            || p_kind.contains("interface")
            || p_kind.contains("struct")
            || p_kind.contains("trait")
        {
            return "declaration".to_string();
        }
        current = p.parent();
    }
    "identifier".to_string()
}

fn is_node_shadowed(
    node: Node,
    source: &str,
    symbol_name: &str,
    _is_declaring_file: bool,
) -> bool {
    // Walk up the AST to look for an enclosing function/block that introduces a local shadow
    let mut current = node.parent();
    while let Some(parent) = current {
        let p_kind = parent.kind();
        if p_kind == "function_declaration"
            || p_kind == "function_item"
            || p_kind == "arrow_function"
            || p_kind == "method_definition"
            || p_kind == "lexical_declaration"
            || p_kind == "variable_declaration"
        {
            // If parent declares a local variable or parameter with symbol_name that is inner,
            // check if this node is inside that inner scope and is not the outer declaration.
            if has_inner_shadow_declaration(parent, node, source, symbol_name) {
                return true;
            }
        }
        current = parent.parent();
    }
    false
}

fn has_inner_shadow_declaration(
    scope_node: Node,
    target_node: Node,
    source: &str,
    symbol_name: &str,
) -> bool {
    let mut cursor = scope_node.walk();
    for child in scope_node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "lexical_declaration" || kind == "variable_declaration" {
            // Check if this declaration is different from target_node and occurs before/around it
            if child.start_byte() < target_node.start_byte() {
                let decl_text = child.utf8_text(source.as_bytes()).unwrap_or("");
                if decl_text.contains(&format!("const {symbol_name}"))
                    || decl_text.contains(&format!("let {symbol_name}"))
                    || decl_text.contains(&format!("var {symbol_name}"))
                {
                    return true;
                }
            }
        }
    }
    false
}

fn apply_edits_reverse(
    source: &str,
    occurrences: &[(usize, usize, String)],
    new_name: &str,
) -> String {
    let mut sorted_ranges: Vec<(usize, usize)> =
        occurrences.iter().map(|(s, e, _)| (*s, *e)).collect();
    sorted_ranges.sort_by_key(|b| std::cmp::Reverse(b.0));
    sorted_ranges.dedup();

    let mut result = source.to_string();
    for (start, end) in sorted_ranges {
        if start <= end && end <= result.len() {
            result.replace_range(start..end, new_name);
        }
    }
    result
}

fn atomic_write_file(target_path: &Path, content: &str) -> Result<()> {
    let parent_dir = target_path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp_file = tempfile::Builder::new()
        .prefix(".ctxcut_rename_")
        .tempfile_in(parent_dir)
        .map_err(|e| CoreError::Io {
            path: target_path.to_path_buf(),
            source: e,
        })?;

    temp_file
        .write_all(content.as_bytes())
        .map_err(|e| CoreError::Io {
            path: target_path.to_path_buf(),
            source: e,
        })?;

    temp_file.flush().map_err(|e| CoreError::Io {
        path: target_path.to_path_buf(),
        source: e,
    })?;

    temp_file.persist(target_path).map_err(|e| CoreError::Io {
        path: target_path.to_path_buf(),
        source: e.error,
    })?;

    Ok(())
}

fn get_line_col(source: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, b) in source.bytes().enumerate() {
        if i >= byte_offset {
            break;
        }
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn get_snippet(source: &str, start_byte: usize, end_byte: usize) -> String {
    let start = start_byte.saturating_sub(20);
    let end = (end_byte + 20).min(source.len());
    source[start..end].replace('\n', " ").trim().to_string()
}
