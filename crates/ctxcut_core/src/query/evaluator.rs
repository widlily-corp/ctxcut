//! AST Query execution engine across multi-language workspace files.

use super::model::{AstQueryReport, MatchCapture, QueryMatchResult};
use super::presets::PresetRegistry;
use crate::error::{CoreError, Result};
use crate::lang::sfc::{SfcBlockKind, SfcDocument};
use crate::lang::LanguageRegistry;
use crate::model::SupportedLanguage;
use crate::parser::{AstUtils, ParserManager};
use crate::traversal::{ProjectWalker, TraversalConfig};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

/// AST Query Execution Engine.
pub struct AstQueryEngine;

impl AstQueryEngine {
    /// Queries the workspace files using a Tree-sitter S-expression query or a built-in preset.
    pub fn query_workspace(
        workspace_root: &Path,
        pattern: Option<&str>,
        lang_filter: Option<SupportedLanguage>,
        preset_name: Option<&str>,
        max_matches: Option<usize>,
    ) -> Result<AstQueryReport> {
        if pattern.is_none() && preset_name.is_none() {
            return Err(CoreError::InvalidQuery(
                "Either a custom S-expression pattern or a preset name must be provided".to_string(),
            ));
        }

        let config = TraversalConfig::default();
        let file_paths = ProjectWalker::collect_files(workspace_root, &config);

        let mut matches = Vec::new();
        let mut files_scanned = 0;
        let mut matched_files_set = HashSet::new();

        for file_path in file_paths {
            let Some(lang) = SupportedLanguage::from_path(&file_path) else {
                continue;
            };

            if let Some(expected_lang) = lang_filter {
                if lang != expected_lang {
                    continue;
                }
            }

            files_scanned += 1;

            let Ok(content) = fs::read_to_string(&file_path) else {
                continue;
            };

            let rel_path = file_path
                .strip_prefix(workspace_root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .replace('\\', "/");

            let file_matches = match lang {
                SupportedLanguage::Vue | SupportedLanguage::Svelte | SupportedLanguage::Astro => {
                    Self::query_sfc_file(
                        &rel_path,
                        lang,
                        &content,
                        pattern,
                        preset_name,
                        lang_filter.is_some(),
                    )?
                }
                _ => Self::query_source_file(
                    &file_path,
                    &rel_path,
                    lang,
                    &content,
                    pattern,
                    preset_name,
                    lang_filter.is_some(),
                )?,
            };

            if !file_matches.is_empty() {
                matched_files_set.insert(rel_path.clone());
                for m in file_matches {
                    matches.push(m);
                    if let Some(limit) = max_matches {
                        if matches.len() >= limit {
                            break;
                        }
                    }
                }
            }

            if let Some(limit) = max_matches {
                if matches.len() >= limit {
                    break;
                }
            }
        }

        let total_matches = matches.len();
        let files_matched = matched_files_set.len();
        let query_str = preset_name
            .map(|p| format!("preset:{p}"))
            .or_else(|| pattern.map(|p| p.to_string()))
            .unwrap_or_default();

        Ok(AstQueryReport {
            query: query_str,
            preset: preset_name.map(|p| p.to_string()),
            total_matches,
            files_scanned,
            files_matched,
            matches,
        })
    }

    fn query_source_file(
        abs_path: &Path,
        rel_path: &str,
        lang: SupportedLanguage,
        content: &str,
        pattern: Option<&str>,
        preset_name: Option<&str>,
        strict_errors: bool,
    ) -> Result<Vec<QueryMatchResult>> {
        let adapter = match LanguageRegistry::for_path(abs_path) {
            Ok(a) => a,
            Err(e) => {
                if strict_errors {
                    return Err(e);
                }
                return Ok(Vec::new());
            }
        };

        let ts_lang = adapter.tree_sitter_language(abs_path);

        let query_str = if let Some(preset) = preset_name {
            match PresetRegistry::get_query(preset, lang) {
                Some(q) => q,
                None => return Ok(Vec::new()),
            }
        } else if let Some(pat) = pattern {
            pat
        } else {
            return Ok(Vec::new());
        };

        let query = match Query::new(&ts_lang, query_str) {
            Ok(q) => q,
            Err(e) => {
                if strict_errors {
                    return Err(CoreError::InvalidQuery(format!(
                        "Invalid Tree-sitter query for {lang:?}: {e}"
                    )));
                }
                return Ok(Vec::new());
            }
        };

        let tree = match ParserManager::parse_source(content, &ts_lang, abs_path) {
            Ok(t) => t,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        let mut cursor = QueryCursor::new();
        let root = tree.root_node();

        let mut matches_iter = cursor.matches(&query, root, content.as_bytes());

        while let Some(m) = matches_iter.next() {
            let mut captures = Vec::new();
            let mut symbol_name = None;
            let mut def_node: Option<Node<'_>> = None;

            for c in m.captures {
                let cap_name = query.capture_names()[c.index as usize];
                let node = c.node;
                let text = AstUtils::node_text(node, content).to_string();
                let node_kind = node.kind().to_string();

                let start_line = node.start_position().row + 1;
                let start_col = node.start_position().column + 1;
                let end_line = node.end_position().row + 1;
                let end_col = node.end_position().column + 1;
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();

                if cap_name == "name" && symbol_name.is_none() {
                    symbol_name = Some(text.clone());
                }

                if cap_name == "definition" && def_node.is_none() {
                    def_node = Some(node);
                }

                captures.push(MatchCapture {
                    name: cap_name.to_string(),
                    text,
                    node_kind,
                    start_line,
                    start_col,
                    end_line,
                    end_col,
                    start_byte,
                    end_byte,
                });
            }

            let main_node = def_node.unwrap_or_else(|| {
                m.captures.first().map(|c| c.node).unwrap_or(root)
            });

            let start_line = main_node.start_position().row + 1;
            let end_line = main_node.end_position().row + 1;
            let snippet = AstUtils::node_text(main_node, content).to_string();

            let kind = preset_name
                .map(|p| p.to_string())
                .unwrap_or_else(|| main_node.kind().to_string());

            results.push(QueryMatchResult {
                file_path: rel_path.to_string(),
                language: lang,
                symbol_name,
                kind,
                start_line,
                end_line,
                snippet,
                captures,
            });
        }

        Ok(results)
    }

    fn query_sfc_file(
        rel_path: &str,
        lang: SupportedLanguage,
        content: &str,
        pattern: Option<&str>,
        preset_name: Option<&str>,
        strict_errors: bool,
    ) -> Result<Vec<QueryMatchResult>> {
        let sfc_doc = match lang {
            SupportedLanguage::Vue => SfcDocument::parse_vue(content),
            SupportedLanguage::Svelte => SfcDocument::parse_svelte(content),
            SupportedLanguage::Astro => SfcDocument::parse_astro(content),
            _ => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        let target_lang = if sfc_doc.is_typescript {
            SupportedLanguage::TypeScript
        } else {
            SupportedLanguage::JavaScript
        };

        let ts_lang = if sfc_doc.is_typescript {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        } else {
            tree_sitter_javascript::LANGUAGE.into()
        };

        let query_str = if let Some(preset) = preset_name {
            match PresetRegistry::get_query(preset, target_lang) {
                Some(q) => q,
                None => return Ok(Vec::new()),
            }
        } else if let Some(pat) = pattern {
            pat
        } else {
            return Ok(Vec::new());
        };

        let query = match Query::new(&ts_lang, query_str) {
            Ok(q) => q,
            Err(e) => {
                if strict_errors {
                    return Err(CoreError::InvalidQuery(format!(
                        "Invalid Tree-sitter query for SFC: {e}"
                    )));
                }
                return Ok(Vec::new());
            }
        };

        for block in &sfc_doc.blocks {
            if !matches!(
                block.kind,
                SfcBlockKind::Script | SfcBlockKind::ScriptSetup | SfcBlockKind::Frontmatter
            ) {
                continue;
            }

            let script_src = &block.content;
            let mut parser = tree_sitter::Parser::new();
            if parser.set_language(&ts_lang).is_err() {
                continue;
            }

            let Some(tree) = parser.parse(script_src, None) else {
                continue;
            };

            let mut cursor = QueryCursor::new();
            let root = tree.root_node();
            let mut matches_iter = cursor.matches(&query, root, script_src.as_bytes());

            while let Some(m) = matches_iter.next() {
                let mut captures = Vec::new();
                let mut symbol_name = None;
                let mut def_node: Option<Node<'_>> = None;

                for c in m.captures {
                    let cap_name = query.capture_names()[c.index as usize];
                    let node = c.node;
                    let text = AstUtils::node_text(node, script_src).to_string();
                    let node_kind = node.kind().to_string();

                    let start_line = block.start_line + node.start_position().row;
                    let start_col = node.start_position().column + 1;
                    let end_line = block.start_line + node.end_position().row;
                    let end_col = node.end_position().column + 1;
                    let start_byte = block.start_byte + node.start_byte();
                    let end_byte = block.start_byte + node.end_byte();

                    if cap_name == "name" && symbol_name.is_none() {
                        symbol_name = Some(text.clone());
                    }

                    if cap_name == "definition" && def_node.is_none() {
                        def_node = Some(node);
                    }

                    captures.push(MatchCapture {
                        name: cap_name.to_string(),
                        text,
                        node_kind,
                        start_line,
                        start_col,
                        end_line,
                        end_col,
                        start_byte,
                        end_byte,
                    });
                }

                let main_node = def_node.unwrap_or_else(|| {
                    m.captures.first().map(|c| c.node).unwrap_or(root)
                });

                let start_line = block.start_line + main_node.start_position().row;
                let end_line = block.start_line + main_node.end_position().row;
                let snippet = AstUtils::node_text(main_node, script_src).to_string();

                let kind = preset_name
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| main_node.kind().to_string());

                results.push(QueryMatchResult {
                    file_path: rel_path.to_string(),
                    language: lang,
                    symbol_name,
                    kind,
                    start_line,
                    end_line,
                    snippet,
                    captures,
                });
            }
        }

        Ok(results)
    }
}
