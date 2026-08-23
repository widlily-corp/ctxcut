//! Single File Component (SFC) segmentation, parsing, and collapsing engine for Vue, Svelte, and Astro.

pub mod astro;
pub mod svelte;
pub mod vue;

pub use astro::AstroAdapter;
pub use svelte::SvelteAdapter;
pub use vue::VueAdapter;

use std::collections::HashMap;

/// Kinds of blocks within Single File Components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfcBlockKind {
    /// Vue `<script>` or Svelte `<script>`
    Script,
    /// Vue `<script setup>`
    ScriptSetup,
    /// Vue `<template>`
    Template,
    /// `<style>` or `<style scoped>`
    Style,
    /// Astro `---` frontmatter block
    Frontmatter,
    /// Non-script HTML/JSX markup (in Svelte or Astro)
    Markup,
}

/// Represents a segmented block within an SFC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfcBlock {
    /// Category of this block.
    pub kind: SfcBlockKind,
    /// Block attributes (e.g. `lang="ts"`, `scoped`, `setup`).
    pub attributes: HashMap<String, String>,
    /// Content inside the block (excluding outer tags/fences).
    pub content: String,
    /// 1-based start line in the original SFC file.
    pub start_line: usize,
    /// 1-based end line in the original SFC file.
    pub end_line: usize,
    /// 0-based start byte in original source.
    pub start_byte: usize,
    /// 0-based end byte in original source.
    pub end_byte: usize,
}

/// Structured representation of a parsed Single File Component.
#[derive(Debug, Clone)]
pub struct SfcDocument {
    /// All parsed blocks in document order.
    pub blocks: Vec<SfcBlock>,
    /// Virtual script source text prepared for tree-sitter parsing.
    pub combined_script: String,
    /// Whether any script block uses TypeScript.
    pub is_typescript: bool,
}

impl SfcDocument {
    /// Parses a Vue Single File Component (.vue).
    pub fn parse_vue(source: &str) -> Self {
        let mut blocks = Vec::new();
        let mut is_typescript = false;
        let mut combined_script = String::new();

        let mut pos = 0;
        let source_len = source.len();

        while pos < source_len {
            if let Some(tag_start) = source[pos..].find('<') {
                let abs_tag_start = pos + tag_start;
                if let Some(tag_end) = source[abs_tag_start..].find('>') {
                    let abs_tag_end = abs_tag_start + tag_end;
                    let tag_header = &source[abs_tag_start + 1..abs_tag_end];
                    let tag_name = tag_header.split_whitespace().next().unwrap_or("").trim_start_matches('/');

                    if tag_name == "script" {
                        let is_setup = tag_header.contains("setup");
                        let is_ts = tag_header.contains("lang=\"ts\"") || tag_header.contains("lang='ts'");
                        if is_ts {
                            is_typescript = true;
                        }

                        let close_tag = "</script>";
                        let content_start = abs_tag_end + 1;
                        let (content, content_end) = if let Some(close_idx) = source[content_start..].find(close_tag) {
                            let end = content_start + close_idx;
                            (&source[content_start..end], end + close_tag.len())
                        } else {
                            (&source[content_start..], source_len)
                        };

                        let start_line = source[..abs_tag_start].lines().count().max(1);
                        let end_line = source[..content_end].lines().count().max(start_line);

                        let mut attributes = HashMap::new();
                        if is_setup { attributes.insert("setup".to_string(), "true".to_string()); }
                        if is_ts { attributes.insert("lang".to_string(), "ts".to_string()); }

                        blocks.push(SfcBlock {
                            kind: if is_setup { SfcBlockKind::ScriptSetup } else { SfcBlockKind::Script },
                            attributes,
                            content: content.to_string(),
                            start_line,
                            end_line,
                            start_byte: abs_tag_start,
                            end_byte: content_end,
                        });

                        combined_script.push_str(content);
                        combined_script.push('\n');

                        pos = content_end;
                        continue;
                    } else if tag_name == "template" {
                        let close_tag = "</template>";
                        let content_start = abs_tag_end + 1;
                        let (content, content_end) = if let Some(close_idx) = source[content_start..].find(close_tag) {
                            let end = content_start + close_idx;
                            (&source[content_start..end], end + close_tag.len())
                        } else {
                            (&source[content_start..], source_len)
                        };

                        let start_line = source[..abs_tag_start].lines().count().max(1);
                        let end_line = source[..content_end].lines().count().max(start_line);

                        blocks.push(SfcBlock {
                            kind: SfcBlockKind::Template,
                            attributes: HashMap::new(),
                            content: content.to_string(),
                            start_line,
                            end_line,
                            start_byte: abs_tag_start,
                            end_byte: content_end,
                        });

                        pos = content_end;
                        continue;
                    } else if tag_name == "style" {
                        let is_scoped = tag_header.contains("scoped");
                        let close_tag = "</style>";
                        let content_start = abs_tag_end + 1;
                        let (content, content_end) = if let Some(close_idx) = source[content_start..].find(close_tag) {
                            let end = content_start + close_idx;
                            (&source[content_start..end], end + close_tag.len())
                        } else {
                            (&source[content_start..], source_len)
                        };

                        let start_line = source[..abs_tag_start].lines().count().max(1);
                        let end_line = source[..content_end].lines().count().max(start_line);

                        let mut attributes = HashMap::new();
                        if is_scoped { attributes.insert("scoped".to_string(), "true".to_string()); }

                        blocks.push(SfcBlock {
                            kind: SfcBlockKind::Style,
                            attributes,
                            content: content.to_string(),
                            start_line,
                            end_line,
                            start_byte: abs_tag_start,
                            end_byte: content_end,
                        });

                        pos = content_end;
                        continue;
                    }
                }
                pos = abs_tag_start + 1;
            } else {
                break;
            }
        }

        Self {
            blocks,
            combined_script,
            is_typescript,
        }
    }

    /// Parses a Svelte component (.svelte).
    pub fn parse_svelte(source: &str) -> Self {
        let mut blocks = Vec::new();
        let mut is_typescript = false;
        let mut combined_script = String::new();

        let mut markup_ranges = Vec::new();
        let mut last_markup_start = 0;
        let mut pos = 0;
        let source_len = source.len();

        while pos < source_len {
            if let Some(tag_start) = source[pos..].find('<') {
                let abs_tag_start = pos + tag_start;
                if let Some(tag_end) = source[abs_tag_start..].find('>') {
                    let abs_tag_end = abs_tag_start + tag_end;
                    let tag_header = &source[abs_tag_start + 1..abs_tag_end];
                    let tag_name = tag_header.split_whitespace().next().unwrap_or("").trim_start_matches('/');

                    if tag_name == "script" {
                        if abs_tag_start > last_markup_start {
                            let markup_str = &source[last_markup_start..abs_tag_start];
                            if !markup_str.trim().is_empty() {
                                markup_ranges.push((last_markup_start, abs_tag_start));
                            }
                        }

                        let is_ts = tag_header.contains("lang=\"ts\"") || tag_header.contains("lang='ts'");
                        if is_ts {
                            is_typescript = true;
                        }
                        let is_module = tag_header.contains("context=\"module\"");

                        let close_tag = "</script>";
                        let content_start = abs_tag_end + 1;
                        let (content, content_end) = if let Some(close_idx) = source[content_start..].find(close_tag) {
                            let end = content_start + close_idx;
                            (&source[content_start..end], end + close_tag.len())
                        } else {
                            (&source[content_start..], source_len)
                        };

                        let start_line = source[..abs_tag_start].lines().count().max(1);
                        let end_line = source[..content_end].lines().count().max(start_line);

                        let mut attributes = HashMap::new();
                        if is_ts { attributes.insert("lang".to_string(), "ts".to_string()); }
                        if is_module { attributes.insert("context".to_string(), "module".to_string()); }

                        blocks.push(SfcBlock {
                            kind: SfcBlockKind::Script,
                            attributes,
                            content: content.to_string(),
                            start_line,
                            end_line,
                            start_byte: abs_tag_start,
                            end_byte: content_end,
                        });

                        combined_script.push_str(content);
                        combined_script.push('\n');

                        last_markup_start = content_end;
                        pos = content_end;
                        continue;
                    } else if tag_name == "style" {
                        if abs_tag_start > last_markup_start {
                            let markup_str = &source[last_markup_start..abs_tag_start];
                            if !markup_str.trim().is_empty() {
                                markup_ranges.push((last_markup_start, abs_tag_start));
                            }
                        }

                        let close_tag = "</style>";
                        let content_start = abs_tag_end + 1;
                        let (content, content_end) = if let Some(close_idx) = source[content_start..].find(close_tag) {
                            let end = content_start + close_idx;
                            (&source[content_start..end], end + close_tag.len())
                        } else {
                            (&source[content_start..], source_len)
                        };

                        let start_line = source[..abs_tag_start].lines().count().max(1);
                        let end_line = source[..content_end].lines().count().max(start_line);

                        blocks.push(SfcBlock {
                            kind: SfcBlockKind::Style,
                            attributes: HashMap::new(),
                            content: content.to_string(),
                            start_line,
                            end_line,
                            start_byte: abs_tag_start,
                            end_byte: content_end,
                        });

                        last_markup_start = content_end;
                        pos = content_end;
                        continue;
                    }
                }
                pos = abs_tag_start + 1;
            } else {
                break;
            }
        }

        if last_markup_start < source_len {
            let markup_str = &source[last_markup_start..];
            if !markup_str.trim().is_empty() {
                markup_ranges.push((last_markup_start, source_len));
            }
        }

        for (m_start, m_end) in markup_ranges {
            let content = &source[m_start..m_end];
            let start_line = source[..m_start].lines().count().max(1);
            let end_line = source[..m_end].lines().count().max(start_line);
            blocks.push(SfcBlock {
                kind: SfcBlockKind::Markup,
                attributes: HashMap::new(),
                content: content.to_string(),
                start_line,
                end_line,
                start_byte: m_start,
                end_byte: m_end,
            });
        }

        Self {
            blocks,
            combined_script,
            is_typescript,
        }
    }

    /// Parses an Astro component (.astro).
    pub fn parse_astro(source: &str) -> Self {
        let mut blocks = Vec::new();
        let mut combined_script = String::new();

        let trimmed = source.trim_start();
        if trimmed.starts_with("---") {
            let fence_start = source.find("---").unwrap();
            let after_first_fence = fence_start + 3;
            if let Some(second_fence) = source[after_first_fence..].find("---") {
                let content_start = after_first_fence;
                let content_end = after_first_fence + second_fence;
                let fence_end = content_end + 3;

                let content = &source[content_start..content_end];
                let start_line = source[..fence_start].lines().count().max(1);
                let end_line = source[..fence_end].lines().count().max(start_line);

                blocks.push(SfcBlock {
                    kind: SfcBlockKind::Frontmatter,
                    attributes: HashMap::new(),
                    content: content.to_string(),
                    start_line,
                    end_line,
                    start_byte: fence_start,
                    end_byte: fence_end,
                });

                combined_script.push_str(content);
                combined_script.push('\n');

                let markup_content = &source[fence_end..];
                if !markup_content.trim().is_empty() {
                    let m_start_line = end_line + 1;
                    let m_end_line = source.lines().count().max(m_start_line);
                    blocks.push(SfcBlock {
                        kind: SfcBlockKind::Markup,
                        attributes: HashMap::new(),
                        content: markup_content.to_string(),
                        start_line: m_start_line,
                        end_line: m_end_line,
                        start_byte: fence_end,
                        end_byte: source.len(),
                    });
                }
            } else {
                blocks.push(SfcBlock {
                    kind: SfcBlockKind::Markup,
                    attributes: HashMap::new(),
                    content: source.to_string(),
                    start_line: 1,
                    end_line: source.lines().count().max(1),
                    start_byte: 0,
                    end_byte: source.len(),
                });
            }
        } else {
            blocks.push(SfcBlock {
                kind: SfcBlockKind::Markup,
                attributes: HashMap::new(),
                content: source.to_string(),
                start_line: 1,
                end_line: source.lines().count().max(1),
                start_byte: 0,
                end_byte: source.len(),
            });
        }

        Self {
            blocks,
            combined_script,
            is_typescript: true, // Astro frontmatter is TypeScript by default
        }
    }

    /// Collapses non-script sections into compact summaries.
    pub fn collapse_summaries(&self) -> Vec<String> {
        let mut summaries = Vec::new();
        for block in &self.blocks {
            match block.kind {
                SfcBlockKind::Template => {
                    let line_count = block.content.lines().count();
                    let tags = extract_html_tag_summary(&block.content);
                    summaries.push(format!("<!-- <template> ({line_count} lines collapsed: {tags}) -->"));
                }
                SfcBlockKind::Style => {
                    let line_count = block.content.lines().count();
                    let scoped_attr = if block.attributes.contains_key("scoped") { " scoped" } else { "" };
                    summaries.push(format!("/* <style{scoped_attr}> ({line_count} lines collapsed) */"));
                }
                SfcBlockKind::Markup => {
                    let line_count = block.content.lines().count();
                    let tags = extract_html_tag_summary(&block.content);
                    summaries.push(format!("<!-- Template Markup ({line_count} lines collapsed: {tags}) -->"));
                }
                _ => {}
            }
        }
        summaries
    }
}

fn extract_html_tag_summary(content: &str) -> String {
    let mut tags = Vec::new();
    for word in content.split('<') {
        let tag = word.split_whitespace().next().unwrap_or("").trim_matches(['>', '/']);
        if !tag.is_empty() && !tag.starts_with('!') && !tag.starts_with('/') && !tags.contains(&tag.to_string()) {
            tags.push(tag.to_string());
            if tags.len() >= 5 {
                break;
            }
        }
    }
    if tags.is_empty() {
        "html markup".to_string()
    } else {
        tags.join(", ")
    }
}
