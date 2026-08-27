//! Boundary stub generator and contract synthesizer for inter-agent cluster cuts.
//!
//! Synthesizes 100% body-stripped signatures (`CallSignatureStub`) and hoisted types (`ExtractedType`)
//! for dependencies crossing cluster cut boundaries, tagging them with immutable contract annotations.

use super::graph::{EdgeKind, WorkspaceGraph};
use crate::model::{CallSignatureStub, ExtractedType, SupportedLanguage};
use std::collections::{HashMap, HashSet};

/// Synthesizes boundary stubs and hoisted types for an agent's cluster cut.
pub struct BoundaryStubGenerator;

impl BoundaryStubGenerator {
    /// Extracts boundary stubs and types for external dependencies referenced by symbols in `cluster_nodes`.
    pub fn synthesize_boundaries(
        graph: &WorkspaceGraph,
        cluster_nodes: &[String],
        node_to_agent: &HashMap<String, String>,
    ) -> (Vec<CallSignatureStub>, Vec<ExtractedType>) {
        let cluster_set: HashSet<&str> = cluster_nodes.iter().map(|s| s.as_str()).collect();
        let mut stubs = Vec::new();
        let mut types = Vec::new();

        let mut seen_stub_names: HashSet<String> = HashSet::new();
        let mut seen_type_names: HashSet<String> = HashSet::new();

        for node_id in cluster_nodes {
            if let Some(outgoing_edges) = graph.outgoing.get(node_id) {
                for (target_id, _, kind) in outgoing_edges {
                    // Only process cross-cluster cut edges
                    if !cluster_set.contains(target_id.as_str()) {
                        if let Some(target_node) = graph.nodes.get(target_id) {
                            match kind {
                                EdgeKind::Call => {
                                    if !seen_stub_names.contains(&target_node.name) {
                                        seen_stub_names.insert(target_node.name.clone());
                                        let stub = create_signature_stub(target_node);
                                        stubs.push(stub);
                                    }
                                }
                                EdgeKind::TypeRef | EdgeKind::Import => {
                                    // Check if target symbol is a type definition
                                    if let Some(ty) = graph.type_definitions.get(&target_node.name) {
                                        if !seen_type_names.contains(&ty.name) {
                                            seen_type_names.insert(ty.name.clone());
                                            types.push(ty.clone());
                                        }
                                    }
                                }
                                EdgeKind::CoLocated => {}
                            }
                        }
                    }
                }
            }
        }

        // Also check if any internal symbols reference types declared in external files
        for node_id in cluster_nodes {
            if let Some(node) = graph.nodes.get(node_id) {
                let sig_or_body = format!("{} {}", node.symbol.signature, node.symbol.body);
                for (ty_name, ty) in &graph.type_definitions {
                    if ty.file_path != node.file_path && !seen_type_names.contains(ty_name) {
                        let is_referenced = sig_or_body.contains(ty_name);
                        if is_referenced {
                            seen_type_names.insert(ty_name.clone());
                            types.push(ty.clone());
                        }
                    }
                }

                // Also check if internal symbol calls or references external symbols
                for (ext_id, ext_node) in &graph.nodes {
                    if !cluster_set.contains(ext_id.as_str())
                        && !seen_stub_names.contains(&ext_node.name)
                        && sig_or_body.contains(&ext_node.name)
                    {
                        seen_stub_names.insert(ext_node.name.clone());
                        let stub = create_signature_stub(ext_node);
                        stubs.push(stub);
                    }
                }
            }
        }

        // Sort for determinism
        stubs.sort_by(|a, b| a.name.cmp(&b.name));
        types.sort_by(|a, b| a.name.cmp(&b.name));

        let _ = node_to_agent;
        (stubs, types)
    }

    /// Formats a write authority header comment for an internal symbol.
    pub fn format_write_authority_tag(agent_id: &str, lang: SupportedLanguage) -> String {
        match lang {
            SupportedLanguage::Python => format!("# WRITE_AUTHORITY: {agent_id}"),
            _ => format!("// WRITE_AUTHORITY: {agent_id}"),
        }
    }

    /// Formats an immutable contract header comment for a boundary stub.
    pub fn format_immutable_contract_tag(source_agent_id: &str, lang: SupportedLanguage) -> String {
        match lang {
            SupportedLanguage::Python => {
                format!("# IMMUTABLE_CONTRACT: {source_agent_id} (Read-Only)")
            }
            _ => format!("// IMMUTABLE_CONTRACT: {source_agent_id} (Read-Only)"),
        }
    }
}

/// Helper that converts a `GraphNode` into a body-stripped `CallSignatureStub`.
fn create_signature_stub(node: &super::graph::GraphNode) -> CallSignatureStub {
    let raw_sig = &node.symbol.signature;
    let clean_sig = format_stripped_signature(raw_sig, node.language);

    let receiver = if node.symbol.name.contains('.') {
        node.symbol.name.split('.').next().map(|s| s.to_string())
    } else {
        None
    };

    CallSignatureStub {
        name: node.symbol.name.clone(),
        receiver,
        file_path: Some(node.file_path.clone()),
        signature: clean_sig,
    }
}

/// Formats a pure signature declaration with stripped body.
fn format_stripped_signature(raw_sig: &str, lang: SupportedLanguage) -> String {
    let trimmed = raw_sig.trim();
    match lang {
        SupportedLanguage::TypeScript
        | SupportedLanguage::JavaScript
        | SupportedLanguage::Vue
        | SupportedLanguage::Svelte
        | SupportedLanguage::Astro => {
            if trimmed.ends_with(';') {
                trimmed.to_string()
            } else if trimmed.ends_with('{') {
                format!("{};", trimmed.trim_end_matches('{').trim())
            } else {
                format!("{trimmed};")
            }
        }
        SupportedLanguage::Rust => {
            if trimmed.ends_with(';') {
                trimmed.to_string()
            } else if trimmed.ends_with('{') {
                format!("{};", trimmed.trim_end_matches('{').trim())
            } else {
                format!("{trimmed};")
            }
        }
        SupportedLanguage::Go => {
            if trimmed.ends_with('{') {
                trimmed.trim_end_matches('{').trim().to_string()
            } else {
                trimmed.to_string()
            }
        }
        SupportedLanguage::Python => {
            if trimmed.ends_with(':') {
                format!("{trimmed} ...")
            } else {
                format!("{trimmed}: ...")
            }
        }
        SupportedLanguage::C | SupportedLanguage::Cpp | SupportedLanguage::CSharp | SupportedLanguage::Java | SupportedLanguage::Kotlin => {
            if trimmed.ends_with(';') {
                trimmed.to_string()
            } else {
                format!("{trimmed};")
            }
        }
    }
}
