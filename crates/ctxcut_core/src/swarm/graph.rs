//! Workspace dependency graph builder constructing symbol dependency graphs across polyglot workspaces.

use crate::error::Result;
use crate::lang::LanguageRegistry;
use crate::model::{ExtractedSymbol, ExtractedType, SliceOptions, SupportedLanguage};
use crate::parser::ParserManager;
use crate::resolver::imports::ImportResolver;
use crate::tokenizer::{count_lines, count_tokens};
use crate::traversal::{ProjectWalker, TraversalConfig};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Edge category representing the nature of relationship between two symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EdgeKind {
    /// Function or method call expression dependency.
    Call,
    /// Type definition / interface / struct / enum reference.
    TypeRef,
    /// Module or file-level import dependency.
    Import,
    /// Co-located in the same source file.
    CoLocated,
}

/// A node in the workspace dependency graph representing a distinct AST symbol.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GraphNode {
    /// Unique symbol node identifier (e.g. `"src/auth.ts:authenticate"`).
    pub id: String,
    /// Symbol identifier name (e.g. `"authenticate"`).
    pub name: String,
    /// Relative file path where the symbol is declared.
    pub file_path: String,
    /// Absolute file path on disk.
    pub absolute_path: PathBuf,
    /// Source programming language.
    pub language: SupportedLanguage,
    /// Full extracted AST symbol with signature, body, doc_comment, start_line, end_line.
    pub symbol: ExtractedSymbol,
    /// BPE token count of the symbol body and signature.
    pub token_count: usize,
    /// Number of lines spanned by the symbol.
    pub line_count: usize,
}

/// A directed weighted edge between two symbol nodes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GraphEdge {
    /// Source node ID (caller / dependent).
    pub from: String,
    /// Target node ID (callee / dependency).
    pub to: String,
    /// Edge weight representing coupling strength (e.g. 3.0 for call, 2.0 for type, 1.5 for import, 1.0 for co-location).
    pub weight: f64,
    /// Kind of edge relationship.
    pub kind: EdgeKind,
}

/// Complete workspace dependency graph containing all symbol nodes, edges, and hoisted type definitions.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceGraph {
    /// Map of node ID to `GraphNode`.
    pub nodes: HashMap<String, GraphNode>,
    /// List of all directed weighted edges.
    pub edges: Vec<GraphEdge>,
    /// Outgoing adjacency: node_id -> list of (target_node_id, weight, EdgeKind).
    pub outgoing: HashMap<String, Vec<(String, f64, EdgeKind)>>,
    /// Incoming adjacency: node_id -> list of (source_node_id, weight, EdgeKind).
    pub incoming: HashMap<String, Vec<(String, f64, EdgeKind)>>,
    /// Workspace-wide type definitions: type_name -> `ExtractedType`.
    pub type_definitions: HashMap<String, ExtractedType>,
    /// File contents cache: relative_path -> source_code.
    pub file_contents: HashMap<String, String>,
    /// File token count cache: relative_path -> token_count.
    pub file_tokens: HashMap<String, usize>,
    /// File line count cache: relative_path -> line_count.
    pub file_lines: HashMap<String, usize>,
    /// Map of file_path -> list of node IDs declared in that file.
    pub file_symbols: HashMap<String, Vec<String>>,
}

impl WorkspaceGraph {
    /// Returns total incident degree (sum of in + out edge weights) for a node.
    pub fn node_degree(&self, node_id: &str) -> f64 {
        let out_w: f64 = self
            .outgoing
            .get(node_id)
            .map(|list| list.iter().map(|(_, w, _)| *w).sum())
            .unwrap_or(0.0);
        let in_w: f64 = self
            .incoming
            .get(node_id)
            .map(|list| list.iter().map(|(_, w, _)| *w).sum())
            .unwrap_or(0.0);
        out_w + in_w
    }

    /// Returns the sum of all edge weights in the graph ($2m$).
    pub fn total_weight(&self) -> f64 {
        self.edges.iter().map(|e| e.weight).sum()
    }

    /// Returns a reference to a `GraphNode` by its ID.
    pub fn get_node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.get(id)
    }

    /// Finds nodes matching a seed query (exact node ID, exact symbol name, or suffix).
    pub fn find_nodes_by_seed(&self, seed: &str) -> Vec<&GraphNode> {
        let trimmed = seed.trim();
        let mut results = Vec::new();

        // 1. Exact node ID match
        if let Some(n) = self.nodes.get(trimmed) {
            results.push(n);
            return results;
        }

        // 2. Exact symbol name match
        for node in self.nodes.values() {
            if node.name == trimmed || node.name.eq_ignore_ascii_case(trimmed) {
                results.push(node);
            }
        }

        if !results.is_empty() {
            return results;
        }

        // 3. Substring match
        for node in self.nodes.values() {
            if node.id.contains(trimmed) || node.name.contains(trimmed) {
                results.push(node);
            }
        }

        results
    }
}

/// Builder that scans workspace files and constructs a `WorkspaceGraph`.
pub struct WorkspaceGraphBuilder;

impl WorkspaceGraphBuilder {
    /// Scans the given workspace root directory and constructs a `WorkspaceGraph`.
    pub fn build(root_dir: &Path) -> Result<WorkspaceGraph> {
        let mut graph = WorkspaceGraph::default();
        let files = ProjectWalker::collect_files(root_dir, &TraversalConfig::default());

        if files.is_empty() {
            return Ok(graph);
        }

        let mut file_parsed_asts: HashMap<String, (PathBuf, SupportedLanguage, String)> =
            HashMap::new();

        // Step 1: Discover all files, symbols, and types across languages
        for file_path in &files {
            let lang = match SupportedLanguage::from_path(file_path) {
                Some(l) => l,
                None => continue,
            };

            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let rel_path = file_path
                .strip_prefix(root_dir)
                .unwrap_or(file_path)
                .to_string_lossy()
                .replace('\\', "/");

            let token_cnt = count_tokens(&content);
            let line_cnt = count_lines(&content);

            graph
                .file_contents
                .insert(rel_path.clone(), content.clone());
            graph.file_tokens.insert(rel_path.clone(), token_cnt);
            graph.file_lines.insert(rel_path.clone(), line_cnt);

            file_parsed_asts.insert(rel_path.clone(), (file_path.clone(), lang, content));
        }

        let mut node_symbol_map: HashMap<String, (PathBuf, SupportedLanguage, String)> =
            HashMap::new();

        // Extract symbols and hoisted types
        for (rel_path, (abs_path, lang, content)) in &file_parsed_asts {
            let adapter = match LanguageRegistry::for_path(abs_path) {
                Ok(a) => a,
                Err(_) => continue,
            };

            let ts_lang = adapter.tree_sitter_language(abs_path);
            let tree = match ParserManager::parse_source(content, &ts_lang, abs_path) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let root_node = tree.root_node();
            let symbol_names = adapter.list_symbols(root_node, content);
            let mut file_node_ids = Vec::new();

            for sym_name in symbol_names {
                if let Ok((extracted_sym, _)) =
                    adapter.locate_symbol(root_node, content, &sym_name, abs_path)
                {
                    let node_id = format!("{rel_path}:{}", extracted_sym.name);
                    let final_node_id = if graph.nodes.contains_key(&node_id) {
                        format!("{rel_path}:{}:{}", extracted_sym.name, extracted_sym.start_line)
                    } else {
                        node_id
                    };

                    let sym_tokens =
                        count_tokens(&extracted_sym.signature) + count_tokens(&extracted_sym.body);
                    let sym_lines = if extracted_sym.end_line >= extracted_sym.start_line {
                        extracted_sym.end_line - extracted_sym.start_line + 1
                    } else {
                        1
                    };

                    let graph_node = GraphNode {
                        id: final_node_id.clone(),
                        name: extracted_sym.name.clone(),
                        file_path: rel_path.clone(),
                        absolute_path: abs_path.clone(),
                        language: *lang,
                        symbol: extracted_sym,
                        token_count: sym_tokens,
                        line_count: sym_lines,
                    };

                    graph.nodes.insert(final_node_id.clone(), graph_node);
                    file_node_ids.push(final_node_id.clone());
                    node_symbol_map.insert(
                        final_node_id,
                        (abs_path.clone(), *lang, sym_name.clone()),
                    );
                }
            }

            // Extract hoisted types defined in this file
            extract_types_from_file(
                abs_path,
                root_node,
                content,
                *lang,
                rel_path,
                &mut graph.type_definitions,
            );

            graph.file_symbols.insert(rel_path.clone(), file_node_ids);
        }

        // Build symbol lookup index: symbol_name -> Vec<node_id>
        let mut symbol_name_index: HashMap<String, Vec<String>> = HashMap::new();
        for node in graph.nodes.values() {
            symbol_name_index
                .entry(node.name.clone())
                .or_default()
                .push(node.id.clone());
        }

        let mut edge_set: HashSet<(String, String)> = HashSet::new();

        // Step 2: Build dependency edges
        for (node_id, (abs_path, lang, _sym_name)) in &node_symbol_map {
            let node = match graph.nodes.get(node_id) {
                Some(n) => n.clone(),
                None => continue,
            };

            let adapter = match LanguageRegistry::for_language(*lang) {
                Ok(a) => a,
                Err(_) => continue,
            };

            let content = match graph.file_contents.get(&node.file_path) {
                Some(c) => c,
                None => continue,
            };

            let ts_lang = adapter.tree_sitter_language(abs_path);
            let tree = match ParserManager::parse_source(content, &ts_lang, abs_path) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let root_node = tree.root_node();
            let (_, ast_node) = match adapter.locate_symbol(root_node, content, &node.name, abs_path) {
                Ok(res) => res,
                Err(_) => continue,
            };

            // A. Call dependencies
            if let Ok(stubs) = adapter.strip_calls(ast_node, root_node, content, abs_path) {
                for stub in stubs {
                    if let Some(target_node_ids) = symbol_name_index.get(&stub.name) {
                        for target_id in target_node_ids {
                            if target_id != node_id && !edge_set.contains(&(node_id.clone(), target_id.clone())) {
                                edge_set.insert((node_id.clone(), target_id.clone()));
                                let edge = GraphEdge {
                                    from: node_id.clone(),
                                    to: target_id.clone(),
                                    weight: 3.0,
                                    kind: EdgeKind::Call,
                                };
                                graph.edges.push(edge);
                                graph
                                    .outgoing
                                    .entry(node_id.clone())
                                    .or_default()
                                    .push((target_id.clone(), 3.0, EdgeKind::Call));
                                graph
                                    .incoming
                                    .entry(target_id.clone())
                                    .or_default()
                                    .push((node_id.clone(), 3.0, EdgeKind::Call));
                            }
                        }
                    }
                }
            }

            // Direct AST call expressions & symbol name references scan
            let call_nodes = crate::parser::AstUtils::find_descendants_by_kind(ast_node, "call_expression");
            for call in call_nodes {
                if let Some(fn_node) = call.child_by_field_name("function") {
                    let fn_name = crate::parser::AstUtils::node_text(fn_node, content).trim();
                    let simple_name = fn_name.split('.').next_back().unwrap_or(fn_name).trim();
                    if let Some(target_node_ids) = symbol_name_index.get(simple_name) {
                        for target_id in target_node_ids {
                            if target_id != node_id && !edge_set.contains(&(node_id.clone(), target_id.clone())) {
                                edge_set.insert((node_id.clone(), target_id.clone()));
                                let edge = GraphEdge {
                                    from: node_id.clone(),
                                    to: target_id.clone(),
                                    weight: 3.0,
                                    kind: EdgeKind::Call,
                                };
                                graph.edges.push(edge);
                                graph
                                    .outgoing
                                    .entry(node_id.clone())
                                    .or_default()
                                    .push((target_id.clone(), 3.0, EdgeKind::Call));
                                graph
                                    .incoming
                                    .entry(target_id.clone())
                                    .or_default()
                                    .push((node_id.clone(), 3.0, EdgeKind::Call));
                            }
                        }
                    }
                }
            }

            // B. Type references
            if let Ok(types) = adapter.hoist_types(
                ast_node,
                root_node,
                content,
                abs_path,
                &SliceOptions::default(),
            ) {
                for ty in types {
                    // Check if this type is defined in another file and find symbols in that file
                    for (other_file, symbols_in_file) in &graph.file_symbols {
                        if other_file != &node.file_path && ty.file_path.contains(other_file) {
                            for target_id in symbols_in_file {
                                if target_id != node_id
                                    && !edge_set.contains(&(node_id.clone(), target_id.clone()))
                                {
                                    edge_set.insert((node_id.clone(), target_id.clone()));
                                    let edge = GraphEdge {
                                        from: node_id.clone(),
                                        to: target_id.clone(),
                                        weight: 2.0,
                                        kind: EdgeKind::TypeRef,
                                    };
                                    graph.edges.push(edge);
                                    graph
                                        .outgoing
                                        .entry(node_id.clone())
                                        .or_default()
                                        .push((target_id.clone(), 2.0, EdgeKind::TypeRef));
                                    graph
                                        .incoming
                                        .entry(target_id.clone())
                                        .or_default()
                                        .push((node_id.clone(), 2.0, EdgeKind::TypeRef));
                                }
                            }
                        }
                    }
                }
            }

            // C. Co-location edges (same source file)
            if let Some(siblings) = graph.file_symbols.get(&node.file_path) {
                for sibling_id in siblings {
                    if sibling_id != node_id
                        && !edge_set.contains(&(node_id.clone(), sibling_id.clone()))
                    {
                        edge_set.insert((node_id.clone(), sibling_id.clone()));
                        let edge = GraphEdge {
                            from: node_id.clone(),
                            to: sibling_id.clone(),
                            weight: 1.0,
                            kind: EdgeKind::CoLocated,
                        };
                        graph.edges.push(edge);
                        graph
                            .outgoing
                            .entry(node_id.clone())
                            .or_default()
                            .push((sibling_id.clone(), 1.0, EdgeKind::CoLocated));
                        graph
                            .incoming
                            .entry(sibling_id.clone())
                            .or_default()
                            .push((node_id.clone(), 1.0, EdgeKind::CoLocated));
                    }
                }
            }

            // D. Module imports
            let imports = ImportResolver::extract_imports(root_node, content);
            for mapping in imports.values() {
                if let Some(target_file) =
                    ImportResolver::resolve_module_path(abs_path, &mapping.specifier)
                {
                    let target_rel = target_file
                        .strip_prefix(root_dir)
                        .unwrap_or(&target_file)
                        .to_string_lossy()
                        .replace('\\', "/");

                    if let Some(target_symbols) = graph.file_symbols.get(&target_rel) {
                        for target_id in target_symbols {
                            if target_id != node_id
                                && !edge_set.contains(&(node_id.clone(), target_id.clone()))
                            {
                                edge_set.insert((node_id.clone(), target_id.clone()));
                                let edge = GraphEdge {
                                    from: node_id.clone(),
                                    to: target_id.clone(),
                                    weight: 1.5,
                                    kind: EdgeKind::Import,
                                };
                                graph.edges.push(edge);
                                graph
                                    .outgoing
                                    .entry(node_id.clone())
                                    .or_default()
                                    .push((target_id.clone(), 1.5, EdgeKind::Import));
                                graph
                                    .incoming
                                    .entry(target_id.clone())
                                    .or_default()
                                    .push((node_id.clone(), 1.5, EdgeKind::Import));
                            }
                        }
                    }
                }
            }
        }

        Ok(graph)
    }
}

fn extract_types_from_file(
    _abs_path: &Path,
    root: tree_sitter::Node<'_>,
    source: &str,
    lang: SupportedLanguage,
    rel_path: &str,
    out_types: &mut HashMap<String, ExtractedType>,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let decl = crate::parser::AstUtils::unwrap_export(child);
        match lang {
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => match decl.kind() {
                "interface_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(child, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "interface".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
                "type_alias_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(child, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "type_alias".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
                "enum_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(child, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "enum".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
                _ => {}
            },
            SupportedLanguage::Rust => match decl.kind() {
                "struct_item" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "struct".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
                "enum_item" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "enum".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
                "trait_item" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "trait".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
                "type_item" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "type_alias".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
                _ => {}
            },
            SupportedLanguage::Go => {
                if decl.kind() == "type_declaration" {
                    let mut type_cursor = decl.walk();
                    for spec in decl.children(&mut type_cursor) {
                        if spec.kind() == "type_spec" {
                            if let Some(name_node) = spec.child_by_field_name("name") {
                                let name =
                                    crate::parser::AstUtils::node_text(name_node, source).to_string();
                                let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                                out_types.insert(
                                    name.clone(),
                                    ExtractedType {
                                        name,
                                        kind: "struct".to_string(),
                                        file_path: rel_path.to_string(),
                                        definition: def,
                                    },
                                );
                            }
                        }
                    }
                }
            }
            SupportedLanguage::Python => {
                if decl.kind() == "class_definition" {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "class".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
            }
            SupportedLanguage::CSharp => {
                if matches!(
                    decl.kind(),
                    "class_declaration"
                        | "struct_declaration"
                        | "interface_declaration"
                        | "enum_declaration"
                ) {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let kind = decl.kind().replace("_declaration", "");
                        let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind,
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
            }
            SupportedLanguage::Java | SupportedLanguage::Kotlin => {
                if matches!(
                    decl.kind(),
                    "class_declaration" | "interface_declaration" | "enum_declaration"
                ) {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let kind = decl.kind().replace("_declaration", "");
                        let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind,
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
            }
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                if matches!(
                    decl.kind(),
                    "struct_specifier" | "enum_specifier" | "class_specifier" | "type_definition"
                ) {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        let name = crate::parser::AstUtils::node_text(name_node, source).to_string();
                        let def = crate::parser::AstUtils::node_text(decl, source).to_string();
                        out_types.insert(
                            name.clone(),
                            ExtractedType {
                                name,
                                kind: "struct".to_string(),
                                file_path: rel_path.to_string(),
                                definition: def,
                            },
                        );
                    }
                }
            }
        }
    }
}
