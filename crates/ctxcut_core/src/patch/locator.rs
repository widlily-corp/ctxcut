//! AST node locator for resolving symbol queries to concrete byte ranges and indentation.

use crate::error::Result;
use crate::lang::LanguageAdapter;
use crate::patch::formatting::detect_node_base_indentation;
use std::path::Path;
use tree_sitter::Node;

/// Represents a successfully located AST node ready for patching.
#[derive(Debug, Clone)]
pub struct LocatedNode<'a> {
    /// Resolved symbol name (e.g., `"AuthService.login"`, `"process_data"`).
    pub symbol_name: String,
    /// Kind of AST node (`"function"`, `"method"`, `"class"`, `"struct"`, etc.).
    pub kind: String,
    /// Underlying Tree-Sitter AST node.
    pub node: Node<'a>,
    /// Byte range `(start_byte, end_byte)` in the source text to replace.
    pub byte_range: (usize, usize),
    /// Leading indentation string of the node's line.
    pub base_indentation: String,
}

/// Locator engine for mapping symbol queries to target AST node byte ranges.
pub struct AstNodeLocator;

impl AstNodeLocator {
    /// Locates the target AST node matching `symbol_query` and resolves its enclosing byte range and indentation.
    pub fn locate<'a>(
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        adapter: &dyn LanguageAdapter,
        file_path: &Path,
    ) -> Result<LocatedNode<'a>> {
        let (extracted_sym, raw_node) =
            adapter.locate_symbol(root, source, symbol_query, file_path)?;

        let target_node = resolve_enclosing_node(raw_node, adapter);
        let byte_range = (target_node.start_byte(), target_node.end_byte());
        let base_indentation = detect_node_base_indentation(source, byte_range.0).to_string();

        let symbol_name = if symbol_query.starts_with('*') {
            symbol_query.trim_start_matches('*').to_string()
        } else if symbol_query.contains('.') {
            symbol_query.to_string()
        } else {
            extracted_sym.name
        };

        Ok(LocatedNode {
            symbol_name,
            kind: extracted_sym.kind,
            node: target_node,
            byte_range,
            base_indentation,
        })
    }
}

/// Adjusts the target node to enclose export or declaration statements where appropriate.
fn resolve_enclosing_node<'a>(node: Node<'a>, adapter: &dyn LanguageAdapter) -> Node<'a> {
    if adapter.language().is_typescript_family() {
        // If the located node is a variable_declarator inside a lexical_declaration/variable_declaration
        if node.kind() == "variable_declarator" {
            if let Some(parent) = node.parent() {
                if matches!(
                    parent.kind(),
                    "lexical_declaration" | "variable_declaration"
                ) {
                    if let Some(grandparent) = parent.parent() {
                        if grandparent.kind() == "export_statement" {
                            return grandparent;
                        }
                    }
                    return parent;
                }
            }
        }

        // If the located node is directly wrapped in an export_statement
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                return parent;
            }
        }
    }

    node
}
