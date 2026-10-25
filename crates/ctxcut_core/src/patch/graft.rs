//! AST syntax grafting engine for surgically replacing symbols while preserving trivia,
//! indentation, and line endings.

use crate::error::{CoreError, Result};
use crate::lang::{LanguageAdapter, LanguageRegistry};
use crate::model::{PatchResult, SupportedLanguage};
use crate::parser::ParserManager;
use crate::patch::formatting::{
    detect_node_base_indentation, find_common_indentation, generate_unified_diff, LineEnding,
};
use crate::patch::locator::AstNodeLocator;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// The indentation style detected for code alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndentStyle {
    /// Indentation using spaces with a specific column width per level.
    Spaces(usize),
    /// Indentation using tab characters (`\t`).
    Tabs,
}

#[allow(clippy::wildcard_imports)]
mod line_ending_serde {
    use super::*;

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(super) fn serialize<S>(
        le: &LineEnding,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match le {
            LineEnding::Lf => serializer.serialize_str("lf"),
            LineEnding::Crlf => serializer.serialize_str("crlf"),
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> std::result::Result<LineEnding, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "crlf" => Ok(LineEnding::Crlf),
            _ => Ok(LineEnding::Lf),
        }
    }
}

/// Result of an AST syntax grafting operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraftResult {
    /// Target file path.
    pub file_path: PathBuf,
    /// Resolved symbol name (e.g. `"PlayerController.FixedUpdate"`).
    pub symbol_name: String,
    /// Original code of the target node and its trivia.
    pub original_code: String,
    /// Fully grafted and normalized replacement code.
    pub grafted_code: String,
    /// Complete source text with the replacement grafted in-memory.
    pub full_patched_source: String,
    /// Byte range `(start_byte, end_byte)` in the original source that was replaced.
    pub byte_range: (usize, usize),
    /// Unified diff representation of the change.
    pub diff: String,
    /// Detected line ending convention of the source file.
    #[serde(with = "line_ending_serde")]
    pub line_ending: LineEnding,
    /// Detected base indentation string of the target AST node.
    pub base_indentation: String,
    /// Detected indentation style (spaces vs tabs).
    pub indent_style: IndentStyle,
    /// Preserved leading trivia string (e.g. doc comments, attributes), if any was preserved.
    pub preserved_leading_trivia: Option<String>,
}

impl GraftResult {
    /// Converts this graft result into a standard `PatchResult`.
    pub fn to_patch_result(&self, applied: bool) -> PatchResult {
        PatchResult {
            file_path: self.file_path.clone(),
            symbol_name: self.symbol_name.clone(),
            original_code: self.original_code.clone(),
            patched_code: self.grafted_code.clone(),
            byte_range: self.byte_range,
            diff: self.diff.clone(),
            applied,
        }
    }
}

/// AST-guided syntax grafting engine.
///
/// Surgically locates symbols via Tree-Sitter AST, detects formatting and trivia,
/// preserves leading and trailing trivia (attributes, XML comments), normalizes indentation
/// (spaces vs tabs), enforces line endings (`\r\n` vs `\n`), and splices replacement code
/// without corrupting neighboring declarations.
pub struct AstGraftEngine;

impl AstGraftEngine {
    /// Performs AST syntax grafting on `source` in-memory.
    ///
    /// Locates the target symbol via `symbol_query`, extracts and preserves trivia,
    /// normalizes indentation and line endings, and returns `GraftResult`.
    pub fn graft_symbol(
        source: &str,
        language: SupportedLanguage,
        file_path: &Path,
        symbol_query: &str,
        replacement_code: &str,
    ) -> Result<GraftResult> {
        let adapter = LanguageRegistry::for_language(language)?;
        Self::graft_with_adapter(source, &*adapter, file_path, symbol_query, replacement_code)
    }

    /// Performs AST syntax grafting on `source` in-memory using an explicit `LanguageAdapter`.
    pub fn graft_with_adapter(
        source: &str,
        adapter: &dyn LanguageAdapter,
        file_path: &Path,
        symbol_query: &str,
        replacement_code: &str,
    ) -> Result<GraftResult> {
        let ts_lang = adapter.tree_sitter_language(file_path);
        let tree = ParserManager::parse_source(source, &ts_lang, file_path)?;

        let located =
            AstNodeLocator::locate(tree.root_node(), source, symbol_query, adapter, file_path)?;

        let trivia = extract_node_trivia(located.node, source);
        let lang = adapter.language();
        let has_repl_doc = replacement_has_doc_comments(replacement_code);
        let has_repl_attr = replacement_has_attributes(replacement_code, lang);

        let (splice_start, splice_end, preserved_trivia) = determine_splice_boundaries(
            &trivia,
            located.node.start_byte(),
            has_repl_doc,
            has_repl_attr,
        );

        if splice_start > splice_end || splice_end > source.len() {
            return Err(CoreError::PatchRangeError {
                path: file_path.to_path_buf(),
                start: splice_start,
                end: splice_end,
                total_bytes: source.len(),
            });
        }

        let line_ending = LineEnding::detect(source);
        let base_indent = detect_node_base_indentation(source, splice_start).to_string();
        let indent_style = Self::detect_indent_style(source, &base_indent);

        let aligned_replacement = Self::normalize_graft_indentation(
            replacement_code,
            &base_indent,
            indent_style,
            line_ending,
        );

        let full_patched_source = format!(
            "{}{}{}",
            &source[..splice_start],
            aligned_replacement,
            &source[splice_end..]
        );

        let original_code = source[splice_start..splice_end].to_string();
        let diff = generate_unified_diff(source, &full_patched_source, file_path, 3);

        Ok(GraftResult {
            file_path: file_path.to_path_buf(),
            symbol_name: located.symbol_name,
            original_code,
            grafted_code: aligned_replacement,
            full_patched_source,
            byte_range: (splice_start, splice_end),
            diff,
            line_ending,
            base_indentation: base_indent,
            indent_style,
            preserved_leading_trivia: preserved_trivia,
        })
    }

    /// Performs AST syntax grafting and runs in-memory pre-flight syntax validation.
    ///
    /// Returns both the `GraftResult` and the `GraftValidationReport`. If syntax validation
    /// fails, no changes are committed to disk and the report contains precise diagnostic details.
    pub fn graft_and_validate(
        source: &str,
        language: SupportedLanguage,
        file_path: &Path,
        symbol_query: &str,
        replacement_code: &str,
    ) -> Result<(GraftResult, crate::patch::validate::GraftValidationReport)> {
        let adapter = LanguageRegistry::for_language(language)?;
        let graft_result =
            Self::graft_with_adapter(source, &*adapter, file_path, symbol_query, replacement_code)?;
        let ts_lang = adapter.tree_sitter_language(file_path);

        let report = crate::patch::validate::SyntaxValidator::validate_graft_in_memory(
            source,
            &graft_result.full_patched_source,
            &ts_lang,
            file_path,
        )?;

        Ok((graft_result, report))
    }

    /// Detects the indentation style (tabs vs spaces) from base indentation or source context.
    pub fn detect_indent_style(source: &str, base_indent: &str) -> IndentStyle {
        if base_indent.contains('\t') {
            IndentStyle::Tabs
        } else if base_indent.contains(' ') {
            IndentStyle::Spaces(4)
        } else {
            let mut tab_count = 0;
            let mut space_count = 0;
            for line in source.lines() {
                if line.starts_with('\t') {
                    tab_count += 1;
                } else if line.starts_with("  ") {
                    space_count += 1;
                }
            }
            if tab_count > space_count {
                IndentStyle::Tabs
            } else {
                IndentStyle::Spaces(4)
            }
        }
    }

    /// Normalizes replacement code indentation and line endings according to the target node's style.
    ///
    /// Converts relative indentation to match `indent_style` (spaces vs tabs) and prepends `base_indent`
    /// to all subsequent lines. Line 0 does not receive `base_indent` because the source slice prior to
    /// the splice position already provides the target line's indentation.
    pub fn normalize_graft_indentation(
        replacement: &str,
        base_indent: &str,
        indent_style: IndentStyle,
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

            // Extract relative indentation on this line (beyond common_indent)
            let rel_indent_len = stripped
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .map(char::len_utf8)
                .sum();
            let rel_indent_raw = &stripped[..rel_indent_len];
            let content = &stripped[rel_indent_len..];

            let converted_rel_indent = convert_indentation(rel_indent_raw, indent_style);

            if i == 0 {
                // Line 0 is spliced at the splice byte, which already follows the line's base indentation
                result.push(format!("{converted_rel_indent}{content}"));
            } else {
                // Lines 1..N receive base_indent + converted relative indentation
                result.push(format!("{base_indent}{converted_rel_indent}{content}"));
            }
        }

        result.join(line_ending.as_str())
    }
}

/// Converts relative indentation string to the target `IndentStyle`.
fn convert_indentation(indent_str: &str, style: IndentStyle) -> String {
    if indent_str.is_empty() {
        return String::new();
    }

    // Calculate total effective spaces (1 tab = 4 spaces)
    let mut total_spaces = 0;
    for c in indent_str.chars() {
        match c {
            '\t' => total_spaces += 4,
            ' ' => total_spaces += 1,
            _ => {}
        }
    }

    match style {
        IndentStyle::Tabs => {
            let tabs = total_spaces / 4;
            let rem_spaces = total_spaces % 4;
            let mut out = "\t".repeat(tabs);
            if rem_spaces > 0 {
                out.push_str(&" ".repeat(rem_spaces));
            }
            out
        }
        IndentStyle::Spaces(width) => {
            let levels = total_spaces / 4;
            let rem_spaces = total_spaces % 4;
            let spaces = levels * width + rem_spaces;
            " ".repeat(spaces)
        }
    }
}

/// Internal trivia metadata extracted from an AST node and its surroundings.
struct ExtractedNodeTrivia<'a> {
    doc_comments_start: usize,
    doc_comments: Option<&'a str>,
    attributes: Option<&'a str>,
    core_start: usize,
    end: usize,
}

/// Extracts leading trivia (doc comments, attributes) and core byte positions from an AST node.
fn extract_node_trivia<'a>(node: Node<'a>, source: &'a str) -> ExtractedNodeTrivia<'a> {
    let mut doc_start = node.start_byte();
    let mut prev = node.prev_named_sibling();

    while let Some(p) = prev {
        if p.kind() == "comment" || p.kind() == "block_comment" || p.kind() == "line_comment" {
            let between = &source[p.end_byte()..doc_start];
            if between.trim().is_empty() {
                doc_start = p.start_byte();
                prev = p.prev_named_sibling();
                continue;
            }
        }
        break;
    }

    let doc_comments = if doc_start < node.start_byte() {
        Some(&source[doc_start..node.start_byte()])
    } else {
        None
    };

    let mut attr_start = None;
    let mut attr_end = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "attribute_list"
                | "attribute_item"
                | "inner_attribute_item"
                | "decorator"
                | "annotation"
                | "marker_annotation"
        ) {
            if attr_start.is_none() {
                attr_start = Some(child.start_byte());
            }
            attr_end = Some(child.end_byte());
        }
    }

    let (attributes, core_start) = if let (Some(s), Some(e)) = (attr_start, attr_end) {
        let attr_text = &source[s..e];
        let rest = &source[e..node.end_byte()];
        let ws_len = rest.len() - rest.trim_start().len();
        (Some(attr_text), e + ws_len)
    } else {
        (None, node.start_byte())
    };

    ExtractedNodeTrivia {
        doc_comments_start: doc_start,
        doc_comments,
        attributes,
        core_start,
        end: node.end_byte(),
    }
}

/// Checks whether replacement code provides leading doc comments.
fn replacement_has_doc_comments(replacement: &str) -> bool {
    for line in replacement.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("///")
            || trimmed.starts_with("/**")
            || trimmed.starts_with("/*")
            || (trimmed.starts_with("//") && !trimmed.starts_with("// TODO"))
        {
            return true;
        }
        break;
    }
    false
}

/// Checks whether replacement code provides attributes or decorators.
fn replacement_has_attributes(replacement: &str, lang: SupportedLanguage) -> bool {
    for line in replacement.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("///")
            || trimmed.starts_with("/**")
            || trimmed.starts_with("/*")
            || trimmed.starts_with("//")
        {
            continue;
        }

        match lang {
            SupportedLanguage::CSharp if trimmed.starts_with('[') && trimmed.contains(']') => {
                return true;
            }
            SupportedLanguage::Rust if trimmed.starts_with("#[") || trimmed.starts_with("#![") => {
                return true;
            }
            SupportedLanguage::Python
            | SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Java
            | SupportedLanguage::Kotlin
                if trimmed.starts_with('@') =>
            {
                return true;
            }
            _ => {}
        }
        break;
    }
    false
}

/// Determines exact splice boundaries and collects description of preserved trivia.
fn determine_splice_boundaries(
    trivia: &ExtractedNodeTrivia<'_>,
    node_start_byte: usize,
    has_repl_doc: bool,
    has_repl_attr: bool,
) -> (usize, usize, Option<String>) {
    if !has_repl_doc && !has_repl_attr {
        // Replacement is pure body/signature without trivia.
        // If original had attributes, splice starts at core_start (preserving attributes & doc comments).
        if trivia.attributes.is_some() {
            let mut preserved = Vec::new();
            if let Some(doc) = trivia.doc_comments {
                preserved.push(doc.trim().to_string());
            }
            if let Some(attr) = trivia.attributes {
                preserved.push(attr.trim().to_string());
            }
            let info = if preserved.is_empty() {
                None
            } else {
                Some(preserved.join("\n"))
            };
            return (trivia.core_start, trivia.end, info);
        }

        // If original had no attributes but had doc comments, splice starts at node_start_byte
        if let Some(doc) = trivia.doc_comments {
            return (node_start_byte, trivia.end, Some(doc.trim().to_string()));
        }

        (node_start_byte, trivia.end, None)
    } else if !has_repl_doc && has_repl_attr {
        // Replacement provides attributes, but no doc comments.
        // Preserve doc comments, replace attributes.
        let info = trivia.doc_comments.map(|d| d.trim().to_string());
        (node_start_byte, trivia.end, info)
    } else {
        // Replacement provides doc comments (and optionally attributes).
        // Replace from the beginning of doc comments.
        (trivia.doc_comments_start, trivia.end, None)
    }
}
