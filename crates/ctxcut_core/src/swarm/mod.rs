//! Multi-agent swarm AST context partitioning, community clustering, and boundary contract synthesis (R4).
//!
//! Divides a repository or feature scope into $K$ isolated, non-overlapping AST clusters
//! with stripped boundary interfaces, write authority annotations, mock test contracts, and token budgeting.

pub mod budget;
pub mod clustering;
pub mod engine;
pub mod graph;
pub mod mock;
pub mod stubs;

pub use budget::SwarmBudgetEngine;
pub use clustering::CommunityClusterer;
pub use engine::{
    DefaultSwarmPartitioner, SwarmAgentPack, SwarmPartitionEngine, SwarmPartitionManifest,
};
pub use graph::{EdgeKind, GraphEdge, GraphNode, WorkspaceGraph, WorkspaceGraphBuilder};
pub use mock::MockContractGenerator;
pub use stubs::BoundaryStubGenerator;

/// Type alias for `DefaultSwarmPartitioner` matching alternative naming conventions.
pub type DefaultSwarmPartitionEngine = DefaultSwarmPartitioner;
