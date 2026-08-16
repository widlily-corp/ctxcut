//! Import and module resolver for TypeScript and JavaScript ASTs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tree_sitter::Node;
use crate::parser::AstUtils;

/// Represents an imported symbol mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportMapping {
    /// Local identifier name as used in the file.
    pub local_name: String,
    /// Original exported name in the foreign module.
    pub imported_name: String,
    /// Raw module specifier (e.g. `./types`, `../utils/crypto`).
    pub specifier: String,
}

/// Resolves module imports and finds candidate target files on disk.
pub struct ImportResolver;

impl ImportResolver {
    /// Extracts all import mappings from a file's root AST.
    pub fn extract_imports(root: Node<'_>, source: &str) -> HashMap<String, ImportMapping> {
        let mut map = HashMap::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "import_statement" {
                let specifier = child
                    .child_by_field_name("source")
                    .map(|s| AstUtils::node_text(s, source).trim_matches(['\'', '"', '`']))
                    .unwrap_or("");

                if specifier.is_empty() {
                    continue;
                }

                // Check import clause
                if let Some(clause) = child.child_by_field_name("import_clause") {
                    // Default import: import Foo from './foo'
                    if let Some(first_child) = clause.named_child(0) {
                        if first_child.kind() == "identifier" {
                            let name = AstUtils::node_text(first_child, source).to_string();
                            map.insert(
                                name.clone(),
                                ImportMapping {
                                    local_name: name.clone(),
                                    imported_name: "default".to_string(),
                                    specifier: specifier.to_string(),
                                },
                            );
                        }
                    }

                    // Named imports: import { A, B as C } from './foo'
                    for named in AstUtils::find_descendants_by_kind(clause, "import_specifier") {
                        let name_node = named.child_by_field_name("name");
                        let alias_node = named.child_by_field_name("alias");

                        if let Some(name_n) = name_node {
                            let orig_name = AstUtils::node_text(name_n, source).to_string();
                            let local_name = if let Some(alias_n) = alias_node {
                                AstUtils::node_text(alias_n, source).to_string()
                            } else {
                                orig_name.clone()
                            };

                            map.insert(
                                local_name.clone(),
                                ImportMapping {
                                    local_name,
                                    imported_name: orig_name,
                                    specifier: specifier.to_string(),
                                },
                            );
                        }
                    }

                    // Namespace import: import * as Ns from './foo'
                    for ns in AstUtils::find_descendants_by_kind(clause, "namespace_import") {
                        if let Some(id) = ns.named_child(0) {
                            let ns_name = AstUtils::node_text(id, source).to_string();
                            map.insert(
                                ns_name.clone(),
                                ImportMapping {
                                    local_name: ns_name.clone(),
                                    imported_name: "*".to_string(),
                                    specifier: specifier.to_string(),
                                },
                            );
                        }
                    }
                }
            }
        }

        map
    }

    /// Resolves a module specifier to an existing file path on disk.
    pub fn resolve_module_path(from_file: &Path, specifier: &str) -> Option<PathBuf> {
        // Only relative and absolute file paths are resolved locally
        if !specifier.starts_with('.') && !specifier.starts_with('/') && !specifier.starts_with('\\') {
            return None;
        }

        let parent_dir = from_file.parent().unwrap_or_else(|| Path::new("."));
        let base_path = parent_dir.join(specifier);

        // 1. Direct path exists
        if base_path.is_file() {
            return Some(base_path);
        }

        // 2. Candidate file extensions
        let extensions = ["ts", "tsx", "d.ts", "js", "jsx", "mjs", "cjs"];
        for ext in &extensions {
            let candidate = base_path.with_extension(ext);
            if candidate.is_file() {
                return Some(candidate);
            }
            // In case specifier had dots like `foo.service`
            let candidate_str = format!("{}.{}", base_path.display(), ext);
            let candidate_path = PathBuf::from(candidate_str);
            if candidate_path.is_file() {
                return Some(candidate_path);
            }
        }

        // 3. Directory index resolution
        if base_path.is_dir() {
            for ext in &extensions {
                let candidate = base_path.join(format!("index.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }

        None
    }

    /// Extracts barrel re-exports from a file (e.g. `export * from './foo'`, `export { Bar } from './bar'`).
    pub fn extract_reexports(root: Node<'_>, source: &str) -> Vec<(Option<String>, String)> {
        let mut reexports = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "export_statement" {
                let specifier = child
                    .child_by_field_name("source")
                    .map(|s| AstUtils::node_text(s, source).trim_matches(['\'', '"', '`']))
                    .unwrap_or("");

                if specifier.is_empty() {
                    continue;
                }

                // Check for wildcard `export * from './sub'`
                let has_star = child.named_children(&mut child.walk()).any(|c| c.kind() == "asterisk");
                if has_star || child.child_by_field_name("declaration").is_none() && AstUtils::find_descendants_by_kind(child, "export_specifier").is_empty() {
                    reexports.push((None, specifier.to_string()));
                }

                // Check for named re-exports: `export { A, B as C } from './sub'`
                for spec in AstUtils::find_descendants_by_kind(child, "export_specifier") {
                    if let Some(name_node) = spec.child_by_field_name("name") {
                        let orig_name = AstUtils::node_text(name_node, source).to_string();
                        let exported_name = if let Some(alias_node) = spec.child_by_field_name("alias") {
                            AstUtils::node_text(alias_node, source).to_string()
                        } else {
                            orig_name
                        };
                        reexports.push((Some(exported_name), specifier.to_string()));
                    }
                }
            }
        }

        reexports
    }
}
