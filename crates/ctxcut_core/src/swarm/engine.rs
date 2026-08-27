//! Swarm partition engine executing workspace graph building, community clustering,
//! boundary stub synthesis, write authority tagging, and context packaging.

use super::budget::SwarmBudgetEngine;
use super::clustering::CommunityClusterer;
use super::graph::WorkspaceGraphBuilder;
use super::mock::MockContractGenerator;
use super::stubs::BoundaryStubGenerator;
use crate::error::Result;
use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, SupportedLanguage, TokenStats};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Packaged context bundle for a single autonomous agent in a multi-agent swarm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmAgentPack {
    /// Agent identifier (e.g. `"agent_0"`, `"agent_1"`).
    pub agent_id: String,
    /// Cluster name derived from dominant module or feature (e.g. `"auth_cluster"`, `"billing_cluster"`).
    pub cluster_name: String,
    /// Internal symbols with write authority for this agent.
    pub internal_symbols: Vec<ExtractedSymbol>,
    /// External boundary signature stubs (Read-Only immutable contracts).
    pub boundary_stubs: Vec<CallSignatureStub>,
    /// Hoisted boundary type definitions (Read-Only immutable contracts).
    pub boundary_types: Vec<ExtractedType>,
    /// Synthesized mock test contracts enabling isolated unit testing.
    pub mock_contracts: String,
    /// Token reduction and line statistics for this agent bundle.
    pub token_stats: TokenStats,
}

impl SwarmAgentPack {
    /// Renders the complete synthesized source code bundle with write authority and immutable contract tags.
    pub fn to_annotated_code(&self) -> String {
        let mut out = String::new();
        let lang = self
            .internal_symbols
            .first()
            .and_then(|s| SupportedLanguage::from_str_loose(&s.language))
            .unwrap_or(SupportedLanguage::TypeScript);

        // Header
        out.push_str(&format!(
            "// =========================================================================\n\
             // SWARM AGENT CONTEXT BUNDLE: {} ({})\n\
             // =========================================================================\n\
             // Write Authority: {} symbols | Boundary Contracts: {} stubs, {} types\n\n",
            self.agent_id,
            self.cluster_name,
            self.internal_symbols.len(),
            self.boundary_stubs.len(),
            self.boundary_types.len()
        ));

        // 1. Boundary Type Contracts (Read-Only)
        if !self.boundary_types.is_empty() {
            out.push_str(
                "// -------------------------------------------------------------------------\n\
                 // BOUNDARY TYPE CONTRACTS (Read-Only)\n\
                 // -------------------------------------------------------------------------\n\n",
            );
            for ty in &self.boundary_types {
                out.push_str(&format!(
                    "// IMMUTABLE_CONTRACT: external (Read-Only)\n{}\n\n",
                    ty.definition.trim()
                ));
            }
        }

        // 2. Boundary Signature Stubs (Read-Only)
        if !self.boundary_stubs.is_empty() {
            out.push_str(
                "// -------------------------------------------------------------------------\n\
                 // BOUNDARY SIGNATURE STUBS (Read-Only)\n\
                 // -------------------------------------------------------------------------\n\n",
            );
            for stub in &self.boundary_stubs {
                out.push_str(&format!(
                    "// IMMUTABLE_CONTRACT: external (Read-Only)\n{}\n\n",
                    stub.signature.trim()
                ));
            }
        }

        // 3. Mock Contracts (Local Testing)
        if !self.mock_contracts.trim().is_empty() {
            out.push_str(
                "// -------------------------------------------------------------------------\n\
                 // MOCK TEST CONTRACTS (For Local Agent Unit Tests)\n\
                 // -------------------------------------------------------------------------\n\n",
            );
            out.push_str(self.mock_contracts.trim());
            out.push_str("\n\n");
        }

        // 4. Internal Symbols (Write Authority)
        if !self.internal_symbols.is_empty() {
            out.push_str(&format!(
                "// -------------------------------------------------------------------------\n\
                 // INTERNAL SYMBOLS (Write Authority: {})\n\
                 // -------------------------------------------------------------------------\n\n",
                self.agent_id
            ));
            for sym in &self.internal_symbols {
                let write_tag =
                    BoundaryStubGenerator::format_write_authority_tag(&self.agent_id, lang);
                out.push_str(&format!("{write_tag}\n"));
                let body_trimmed = sym.body.trim();
                if let Some(doc) = &sym.doc_comment {
                    if !body_trimmed.contains(doc.trim()) {
                        out.push_str(&format!("{doc}\n"));
                    }
                }
                out.push_str(&format!("{body_trimmed}\n\n"));
            }
        }

        out
    }

    /// Renders a high-density Markdown summary for this agent pack.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "### Agent `{}` — `{}`\n\n",
            self.agent_id, self.cluster_name
        ));
        out.push_str(&format!(
            "- **Write Authority**: {} internal symbols\n\
             - **Boundary Contracts**: {} stubs, {} types\n\
             - **Tokens**: `{}` (Raw: `{}`, Savings: `{:.2}%`)\n\n",
            self.internal_symbols.len(),
            self.boundary_stubs.len(),
            self.boundary_types.len(),
            self.token_stats.sliced_tokens,
            self.token_stats.raw_file_tokens,
            self.token_stats.savings_percentage
        ));

        if !self.internal_symbols.is_empty() {
            out.push_str("#### Internal Symbols (Write Authority):\n");
            for sym in &self.internal_symbols {
                out.push_str(&format!(
                    "- `{}` (`{}:{}`)\n",
                    sym.name, sym.file_path, sym.start_line
                ));
            }
            out.push('\n');
        }

        if !self.boundary_stubs.is_empty() {
            out.push_str("#### Boundary Contract Stubs (Read-Only):\n");
            for stub in &self.boundary_stubs {
                out.push_str(&format!("- `{}` (`{}`)\n", stub.name, stub.signature));
            }
            out.push('\n');
        }

        out
    }

    /// Formats the agent pack as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the agent pack as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Swarm partition manifest describing the global partitioning across all agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmPartitionManifest {
    /// Total number of agent packs generated.
    pub total_agents: usize,
    /// Total number of symbols partitioned across all packs.
    pub total_symbols: usize,
    /// Total number of boundary contracts (stubs + types) generated across cluster cuts.
    pub boundary_contracts_count: usize,
    /// Detailed agent context packs.
    pub packs: Vec<SwarmAgentPack>,
}

impl SwarmPartitionManifest {
    /// Renders the complete swarm partition manifest into a high-density Markdown report.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Swarm Multi-Agent Context Partition Manifest\n\n");
        out.push_str(&format!(
            "> **Total Agents**: `{}` | **Total Partitioned Symbols**: `{}` | **Boundary Contracts**: `{}`\n\n",
            self.total_agents, self.total_symbols, self.boundary_contracts_count
        ));

        out.push_str("## Agent Context Bundles\n\n");
        for pack in &self.packs {
            out.push_str(&pack.to_markdown());
        }

        out
    }

    /// Formats the manifest as pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the manifest as compact JSON.
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Primary trait for executing multi-agent swarm AST context partitioning.
pub trait SwarmPartitionEngine: Send + Sync {
    /// Partitions the workspace at `root_dir` into $K$ isolated agent packs.
    fn partition_workspace(
        &self,
        root_dir: &Path,
        agents_count: usize,
        seed_symbols: &[String],
        budget_per_agent: Option<usize>,
    ) -> Result<SwarmPartitionManifest>;
}

/// Default implementation of `SwarmPartitionEngine`.
#[derive(Debug, Clone, Default)]
pub struct DefaultSwarmPartitioner;

impl DefaultSwarmPartitioner {
    /// Creates a new `DefaultSwarmPartitioner`.
    pub fn new() -> Self {
        Self
    }
}

impl SwarmPartitionEngine for DefaultSwarmPartitioner {
    fn partition_workspace(
        &self,
        root_dir: &Path,
        agents_count: usize,
        seed_symbols: &[String],
        budget_per_agent: Option<usize>,
    ) -> Result<SwarmPartitionManifest> {
        let graph = WorkspaceGraphBuilder::build(root_dir)?;
        let total_symbols = graph.nodes.len();

        if total_symbols == 0 {
            return Ok(SwarmPartitionManifest {
                total_agents: 0,
                total_symbols: 0,
                boundary_contracts_count: 0,
                packs: Vec::new(),
            });
        }

        let clusters = CommunityClusterer::cluster(&graph, agents_count, seed_symbols);
        let actual_agents = clusters.len();

        // Build mapping: node_id -> owning_agent_id
        let mut node_to_agent: HashMap<String, String> = HashMap::new();
        for (idx, cluster) in clusters.iter().enumerate() {
            let agent_id = format!("agent_{idx}");
            for node_id in cluster {
                node_to_agent.insert(node_id.clone(), agent_id.clone());
            }
        }

        let mut packs = Vec::new();
        let mut total_boundary_contracts = 0;

        for (idx, cluster_node_ids) in clusters.iter().enumerate() {
            let agent_id = format!("agent_{idx}");
            let cluster_name = derive_cluster_name(&graph, cluster_node_ids, idx);

            let mut internal_symbols = Vec::new();
            for node_id in cluster_node_ids {
                if let Some(node) = graph.nodes.get(node_id) {
                    internal_symbols.push(node.symbol.clone());
                }
            }

            let (boundary_stubs, boundary_types) = BoundaryStubGenerator::synthesize_boundaries(
                &graph,
                cluster_node_ids,
                &node_to_agent,
            );

            total_boundary_contracts += boundary_stubs.len() + boundary_types.len();

            let primary_lang = internal_symbols
                .first()
                .and_then(|s| SupportedLanguage::from_str_loose(&s.language))
                .unwrap_or(SupportedLanguage::TypeScript);

            let mock_contracts = MockContractGenerator::generate_mocks(
                &agent_id,
                &boundary_stubs,
                &boundary_types,
                primary_lang,
            );

            let mut pack = SwarmAgentPack {
                agent_id,
                cluster_name,
                internal_symbols,
                boundary_stubs,
                boundary_types,
                mock_contracts,
                token_stats: TokenStats::calculate(0, 0, 0, 0),
            };

            SwarmBudgetEngine::compute_and_apply_budget(&mut pack, &graph, budget_per_agent);
            packs.push(pack);
        }

        Ok(SwarmPartitionManifest {
            total_agents: actual_agents,
            total_symbols,
            boundary_contracts_count: total_boundary_contracts,
            packs,
        })
    }
}

/// Derives a semantic cluster name based on the dominant file or primary symbol.
fn derive_cluster_name(
    graph: &super::graph::WorkspaceGraph,
    node_ids: &[String],
    cluster_idx: usize,
) -> String {
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    for id in node_ids {
        if let Some(node) = graph.nodes.get(id) {
            let file_stem = Path::new(&node.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("module")
                .to_string();
            *file_counts.entry(file_stem).or_insert(0) += 1;
        }
    }

    if let Some((best_stem, _)) = file_counts.into_iter().max_by_key(|(_, cnt)| *cnt) {
        format!("{best_stem}_cluster")
    } else {
        format!("cluster_{cluster_idx}")
    }
}
