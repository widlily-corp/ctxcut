//! Tree-sitter parser manager and AST traversal helper utilities.

use std::path::Path;
use tree_sitter::{Language, Node, Parser, Tree};
use crate::error::{CoreError, Result};

/// Parser manager and AST parsing helpers.
pub struct ParserManager;

impl ParserManager {
    /// Creates and configures a new Tree-sitter parser for the given language.
    pub fn create_parser(language: &Language) -> Result<Parser> {
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .map_err(|e| CoreError::ParseError {
                path: Path::new("<unknown>").to_path_buf(),
                message: format!("Failed to set parser language: {e}"),
            })?;
        Ok(parser)
    }

    /// Parses the provided source string using the given language.
    pub fn parse_source(source: &str, language: &Language, file_path: &Path) -> Result<Tree> {
        let mut parser = Self::create_parser(language)?;
        parser
            .parse(source, None)
            .ok_or_else(|| CoreError::ParseError {
                path: file_path.to_path_buf(),
                message: "Tree-sitter parser returned None".to_string(),
            })
    }
}

/// Helper functions for Tree-sitter AST nodes and text extraction.
pub struct AstUtils;

impl AstUtils {
    /// Extracts the UTF-8 text slice of a node from the source string.
    pub fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
        let start = node.start_byte();
        let end = node.end_byte();
        if start <= end && end <= source.len() {
            &source[start..end]
        } else {
            ""
        }
    }

    /// Finds the first named child node with the specified kind.
    pub fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == kind {
                return Some(child);
            }
        }
        None
    }

    /// Finds all named children with the specified kind.
    pub fn find_children_by_kind<'a>(node: Node<'a>, kind: &str) -> Vec<Node<'a>> {
        let mut result = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == kind {
                result.push(child);
            }
        }
        result
    }

    /// Recursively traverses descendant nodes to find all nodes with the specified kind.
    pub fn find_descendants_by_kind<'a>(node: Node<'a>, kind: &str) -> Vec<Node<'a>> {
        let mut result = Vec::new();
        Self::collect_descendants_by_kind(node, kind, &mut result);
        result
    }

    fn collect_descendants_by_kind<'a>(node: Node<'a>, kind: &str, acc: &mut Vec<Node<'a>>) {
        if node.kind() == kind {
            acc.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_descendants_by_kind(child, kind, acc);
        }
    }

    /// Extracts preceding doc comment (e.g. `/** ... */` or `// ...`) directly attached to a node.
    pub fn extract_doc_comment(node: Node<'_>, source: &str) -> Option<String> {
        let mut prev = node.prev_named_sibling();
        let mut comments = Vec::new();

        while let Some(sibling) = prev {
            if sibling.kind() == "comment" {
                // Check that between sibling and node/previous comment there is only whitespace
                let sibling_end = sibling.end_byte();
                let next_start = if let Some(last) = comments.last() {
                    let last_node: Node<'_> = *last;
                    last_node.start_byte()
                } else {
                    node.start_byte()
                };

                if sibling_end <= next_start {
                    let gap = &source[sibling_end..next_start];
                    if gap.chars().all(|c| c.is_whitespace()) {
                        comments.push(sibling);
                        prev = sibling.prev_named_sibling();
                        continue;
                    }
                }
            }
            break;
        }

        if comments.is_empty() {
            None
        } else {
            comments.reverse();
            let mut doc = String::new();
            for (i, c_node) in comments.iter().enumerate() {
                if i > 0 {
                    doc.push('\n');
                }
                doc.push_str(Self::node_text(*c_node, source).trim());
            }
            Some(doc)
        }
    }
}
