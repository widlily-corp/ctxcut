//! Import and module resolver for TypeScript and JavaScript ASTs.

use crate::parser::AstUtils;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

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
                    .or_else(|| {
                        AstUtils::find_descendants_by_kind(child, "string")
                            .first()
                            .map(|s| AstUtils::node_text(*s, source).trim_matches(['\'', '"', '`']))
                    })
                    .unwrap_or("");

                if specifier.is_empty() {
                    continue;
                }

                // Default import: import Foo from './foo'
                for clause in AstUtils::find_children_by_kind(child, "import_clause") {
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
                }

                // Named imports: import { A, B as C } from './foo'
                for named in AstUtils::find_descendants_by_kind(child, "import_specifier") {
                    let name_node = named
                        .child_by_field_name("name")
                        .or_else(|| named.named_child(0));
                    let alias_node = named.child_by_field_name("alias").or_else(|| {
                        if named.named_child_count() > 1 {
                            named.named_child(1)
                        } else {
                            None
                        }
                    });

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
                for ns in AstUtils::find_descendants_by_kind(child, "namespace_import") {
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
            } else if child.kind() == "lexical_declaration"
                || child.kind() == "variable_declaration"
            {
                // CommonJS require: const { foo } = require('./foo') or const bar = require('./bar')
                for declarator in AstUtils::find_children_by_kind(child, "variable_declarator") {
                    if let Some(val) = declarator.child_by_field_name("value") {
                        if val.kind() == "call_expression" {
                            if let Some(fn_node) = val.child_by_field_name("function") {
                                if AstUtils::node_text(fn_node, source) == "require" {
                                    if let Some(args) = val.child_by_field_name("arguments") {
                                        if let Some(first_arg) = args.named_child(0) {
                                            let specifier = AstUtils::node_text(first_arg, source)
                                                .trim_matches(['\'', '"', '`']);
                                            if !specifier.is_empty() {
                                                if let Some(name_node) =
                                                    declarator.child_by_field_name("name")
                                                {
                                                    if name_node.kind() == "object_pattern" {
                                                        for pattern_child in name_node
                                                            .named_children(&mut name_node.walk())
                                                        {
                                                            if pattern_child.kind() == "shorthand_property_identifier_pattern" || pattern_child.kind() == "identifier" {
                                                                let name = AstUtils::node_text(pattern_child, source).to_string();
                                                                map.insert(
                                                                    name.clone(),
                                                                    ImportMapping {
                                                                        local_name: name.clone(),
                                                                        imported_name: name,
                                                                        specifier: specifier.to_string(),
                                                                    },
                                                                );
                                                            } else if pattern_child.kind() == "pair_pattern" {
                                                                if let (Some(key), Some(val)) = (pattern_child.child_by_field_name("key"), pattern_child.child_by_field_name("value")) {
                                                                    let imported_name = AstUtils::node_text(key, source).to_string();
                                                                    let local_name = AstUtils::node_text(val, source).to_string();
                                                                    map.insert(
                                                                        local_name.clone(),
                                                                        ImportMapping {
                                                                            local_name,
                                                                            imported_name,
                                                                            specifier: specifier.to_string(),
                                                                        },
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    } else if name_node.kind() == "identifier" {
                                                        let name =
                                                            AstUtils::node_text(name_node, source)
                                                                .to_string();
                                                        map.insert(
                                                            name.clone(),
                                                            ImportMapping {
                                                                local_name: name.clone(),
                                                                imported_name: "default"
                                                                    .to_string(),
                                                                specifier: specifier.to_string(),
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        map
    }

    /// Resolves a module specifier to an existing file path on disk.
    pub fn resolve_module_path(from_file: &Path, specifier: &str) -> Option<PathBuf> {
        if !specifier.starts_with('.')
            && !specifier.starts_with('/')
            && !specifier.starts_with('\\')
        {
            return None;
        }

        let parent_dir = from_file.parent().unwrap_or_else(|| Path::new("."));
        let joined = parent_dir.join(specifier);
        let base_path = normalize_path(&joined);

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
                    .or_else(|| {
                        AstUtils::find_descendants_by_kind(child, "string")
                            .first()
                            .map(|s| AstUtils::node_text(*s, source).trim_matches(['\'', '"', '`']))
                    })
                    .unwrap_or("");

                if specifier.is_empty() {
                    continue;
                }

                // Check for wildcard `export * from './sub'`
                let has_star = child
                    .children(&mut child.walk())
                    .any(|c| c.kind() == "*" || c.kind() == "asterisk");
                let has_no_specs =
                    AstUtils::find_descendants_by_kind(child, "export_specifier").is_empty();
                if has_star || (child.child_by_field_name("declaration").is_none() && has_no_specs)
                {
                    reexports.push((None, specifier.to_string()));
                }

                // Check for named re-exports: `export { A, B as C } from './sub'`
                for spec in AstUtils::find_descendants_by_kind(child, "export_specifier") {
                    let name_node = spec
                        .child_by_field_name("name")
                        .or_else(|| spec.named_child(0));
                    let alias_node = spec.child_by_field_name("alias").or_else(|| {
                        if spec.named_child_count() > 1 {
                            spec.named_child(1)
                        } else {
                            None
                        }
                    });

                    if let Some(name_n) = name_node {
                        let orig_name = AstUtils::node_text(name_n, source).to_string();
                        let exported_name = if let Some(alias_n) = alias_node {
                            AstUtils::node_text(alias_n, source).to_string()
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

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                components.pop();
            }
            c => components.push(c),
        }
    }
    components.into_iter().collect()
}
