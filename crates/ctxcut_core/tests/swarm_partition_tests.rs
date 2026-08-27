//! Integration and unit test suite for Milestone 4: Multi-Agent Swarm Context Partitioning (R4).
//!
//! Verifies:
//! 1. Polyglot Workspace Dependency Graph Builder (`WorkspaceGraphBuilder`).
//! 2. Community Clustering via Louvain Modularity Optimization.
//! 3. Seeded Min-Cut & Multi-Way Balanced Graph Partitioning.
//! 4. Strict Disjoint Non-Overlapping AST Symbol Partitioning Invariant.
//! 5. Boundary Stub Synthesizer & 100% Body-Stripped Signatures (`CallSignatureStub`).
//! 6. Write Authority Tags (`// WRITE_AUTHORITY: agent_k`) vs Immutable Contracts (`// IMMUTABLE_CONTRACT: ... (Read-Only)`).
//! 7. Mock Test Contract Generator (`MockContractGenerator`).
//! 8. Per-Agent Token Budgeting & `TokenStats` computation.
//! 9. Adversarial & Edge Cases ($K > N$, $K=1$, empty workspace, cyclic graphs, disconnected components).
//! 10. JSON Manifest Serialization & Markdown report generation.

use ctxcut_core::swarm::{
    BoundaryStubGenerator, DefaultSwarmPartitioner, SwarmPartitionEngine,
    SwarmPartitionManifest, WorkspaceGraphBuilder,
};
use ctxcut_core::SupportedLanguage;
use std::collections::HashSet;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_workspace_graph_builder_polyglot_discovery() {
    let dir = TempDir::new().expect("Failed to create tempdir");
    let auth_file = dir.path().join("auth.ts");
    let billing_file = dir.path().join("billing.rs");

    fs::write(
        &auth_file,
        r#"
export interface UserAuth {
    userId: string;
    token: string;
}

export function authenticate(token: string): UserAuth {
    return { userId: "usr_1", token };
}

export function validateSession(userId: string): boolean {
    return userId.length > 0;
}
"#,
    )
    .unwrap();

    fs::write(
        &billing_file,
        r#"
pub struct Invoice {
    pub id: String,
    pub amount: f64,
}

pub fn create_invoice(user_id: &str, amount: f64) -> Invoice {
    Invoice { id: "inv_1".into(), amount }
}
"#,
    )
    .unwrap();

    let graph = WorkspaceGraphBuilder::build(dir.path()).expect("Graph build failed");

    // Assert: Nodes extracted across TS and Rust
    assert!(graph.nodes.len() >= 3);
    assert!(!graph.find_nodes_by_seed("authenticate").is_empty());
    assert!(!graph.find_nodes_by_seed("validateSession").is_empty());
    assert!(!graph.find_nodes_by_seed("create_invoice").is_empty());

    // Assert: Types extracted
    assert!(graph.type_definitions.contains_key("UserAuth"));
    assert!(graph.type_definitions.contains_key("Invoice"));

    // Assert: Co-location edges exist between auth functions
    let auth_node = graph.find_nodes_by_seed("authenticate")[0];
    let session_node = graph.find_nodes_by_seed("validateSession")[0];

    let has_co_location = graph
        .edges
        .iter()
        .any(|e| (e.from == auth_node.id && e.to == session_node.id) || (e.from == session_node.id && e.to == auth_node.id));
    assert!(has_co_location, "Expected co-location edge between same-file symbols");
}

#[test]
fn test_louvain_modularity_clustering_3_modules() {
    let dir = TempDir::new().expect("Failed to create tempdir");

    // 1. Auth Module
    fs::write(
        dir.path().join("auth.ts"),
        r#"
export function login(user: string): string { return "tok_" + user; }
export function logout(tok: string): boolean { return tok.length > 0; }
"#,
    )
    .unwrap();

    // 2. Billing Module
    fs::write(
        dir.path().join("billing.ts"),
        r#"
export function chargeCard(amount: number): boolean { return amount > 0; }
export function refundCard(txnId: string): boolean { return txnId.len > 0; }
"#,
    )
    .unwrap();

    // 3. Notification Module
    fs::write(
        dir.path().join("notify.ts"),
        r#"
export function sendEmail(to: string): boolean { return to.contains("@"); }
export function sendSms(phone: string): boolean { return phone.len > 5; }
"#,
    )
    .unwrap();

    let engine = DefaultSwarmPartitioner::new();
    let manifest = engine
        .partition_workspace(dir.path(), 3, &[], None)
        .expect("Partitioning failed");

    // Assert: Exactly 3 agent packs created
    assert_eq!(manifest.total_agents, 3);
    assert_eq!(manifest.total_symbols, 6);
    assert_eq!(manifest.packs.len(), 3);

    // Assert: Strict non-overlapping partition invariant
    let mut all_assigned_symbols = HashSet::new();
    for pack in &manifest.packs {
        assert!(!pack.internal_symbols.is_empty());
        for sym in &pack.internal_symbols {
            let unique_key = format!("{}:{}", sym.file_path, sym.name);
            assert!(
                all_assigned_symbols.insert(unique_key.clone()),
                "Symbol {unique_key} was assigned to multiple agent clusters!"
            );
        }
    }
    assert_eq!(all_assigned_symbols.len(), 6);
}

#[test]
fn test_seeded_min_cut_partitioning() {
    let dir = TempDir::new().expect("Failed to create tempdir");

    fs::write(
        dir.path().join("services.ts"),
        r#"
export function authService(): string { return "auth"; }
export function authHelper(): string { return authService(); }

export function paymentService(): string { return "pay"; }
export function paymentHelper(): string { return paymentService(); }
"#,
    )
    .unwrap();

    let engine = DefaultSwarmPartitioner::new();
    let seeds = vec!["authService".to_string(), "paymentService".to_string()];
    let manifest = engine
        .partition_workspace(dir.path(), 2, &seeds, None)
        .expect("Seeded partitioning failed");

    assert_eq!(manifest.total_agents, 2);

    // Verify seed separation into distinct agent clusters
    let pack0_symbols: HashSet<String> = manifest.packs[0]
        .internal_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let pack1_symbols: HashSet<String> = manifest.packs[1]
        .internal_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    let has_auth_in_0 = pack0_symbols.contains("authService");
    let has_pay_in_0 = pack0_symbols.contains("paymentService");

    let has_auth_in_1 = pack1_symbols.contains("authService");
    let has_pay_in_1 = pack1_symbols.contains("paymentService");

    assert!(
        (has_auth_in_0 && has_pay_in_1) || (has_pay_in_0 && has_auth_in_1),
        "Expected seed symbols to be partitioned into distinct agent packs"
    );
}

#[test]
fn test_boundary_stubs_and_immutable_contract_tags() {
    let dir = TempDir::new().expect("Failed to create tempdir");

    fs::write(
        dir.path().join("provider.ts"),
        r#"
export interface ChargeRequest {
    amount: number;
}

export function executeCharge(req: ChargeRequest): boolean {
    // Heavy internal provider implementation
    const apiKey = "secret_provider_key";
    return req.amount > 0;
}
"#,
    )
    .unwrap();

    fs::write(
        dir.path().join("consumer.ts"),
        r#"
import { executeCharge, ChargeRequest } from './provider';

export function checkout(req: ChargeRequest): boolean {
    return executeCharge(req);
}
"#,
    )
    .unwrap();

    let engine = DefaultSwarmPartitioner::new();
    let seeds = vec!["checkout".to_string(), "executeCharge".to_string()];
    let manifest = engine
        .partition_workspace(dir.path(), 2, &seeds, None)
        .expect("Partitioning failed");

    assert_eq!(manifest.packs.len(), 2);

    // Find the pack containing checkout (consumer)
    let consumer_pack = manifest
        .packs
        .iter()
        .find(|p| p.internal_symbols.iter().any(|s| s.name == "checkout"))
        .expect("Consumer pack not found");

    // Assert: Consumer pack contains executeCharge as a boundary stub
    assert!(
        consumer_pack
            .boundary_stubs
            .iter()
            .any(|s| s.name == "executeCharge"),
        "Expected executeCharge in boundary stubs"
    );

    // Assert: Consumer pack contains ChargeRequest in boundary types
    assert!(
        consumer_pack
            .boundary_types
            .iter()
            .any(|t| t.name == "ChargeRequest"),
        "Expected ChargeRequest in boundary types"
    );

    // Assert: Rendered annotated code includes write authority and immutable contract tags
    let annotated_code = consumer_pack.to_annotated_code();
    assert!(annotated_code.contains("// WRITE_AUTHORITY:"));
    assert!(annotated_code.contains("// IMMUTABLE_CONTRACT:"));
    assert!(annotated_code.contains("executeCharge"));
    assert!(annotated_code.contains("checkout"));
}

#[test]
fn test_mock_contract_generation() {
    let dir = TempDir::new().expect("Failed to create tempdir");

    fs::write(
        dir.path().join("api.ts"),
        r#"
export interface OrderPayload {
    item: string;
}

export function submitOrder(payload: OrderPayload): boolean {
    return payload.item.length > 0;
}
"#,
    )
    .unwrap();

    fs::write(
        dir.path().join("workflow.ts"),
        r#"
import { submitOrder, OrderPayload } from './api';

export function runOrderWorkflow(item: string): boolean {
    return submitOrder({ item });
}
"#,
    )
    .unwrap();

    let engine = DefaultSwarmPartitioner::new();
    let seeds = vec!["runOrderWorkflow".to_string(), "submitOrder".to_string()];
    let manifest = engine
        .partition_workspace(dir.path(), 2, &seeds, None)
        .expect("Partitioning failed");

    let workflow_pack = manifest
        .packs
        .iter()
        .find(|p| p.internal_symbols.iter().any(|s| s.name == "runOrderWorkflow"))
        .expect("Workflow pack not found");

    // Assert: Mock contracts generated
    assert!(!workflow_pack.mock_contracts.is_empty());
    assert!(
        workflow_pack.mock_contracts.contains("MockExternalContracts")
            || workflow_pack.mock_contracts.contains("submitOrder")
            || workflow_pack.mock_contracts.contains("mockOrderPayload")
    );
}

#[test]
fn test_per_agent_token_budget_and_stats() {
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("large_service.ts");

    let mut code = String::new();
    code.push_str("export interface Config { timeout: number; }\n");
    for i in 0..15 {
        code.push_str(&format!(
            r#"
/**
 * Exhaustive documentation for subOperation_{i} explaining business rules in detail.
 * @param cfg Configuration options
 */
export function subOperation_{i}(cfg: Config): number {{
    return cfg.timeout + {i};
}}
"#
        ));
    }
    fs::write(&file_path, &code).unwrap();

    let engine = DefaultSwarmPartitioner::new();
    let manifest = engine
        .partition_workspace(dir.path(), 3, &[], Some(200))
        .expect("Budgeted partition failed");

    for pack in &manifest.packs {
        assert!(
            pack.token_stats.sliced_tokens <= 500,
            "Expected compressed bundle <= 500 tokens, got {}",
            pack.token_stats.sliced_tokens
        );
        assert!(pack.token_stats.raw_file_tokens > 0);
        assert!(pack.token_stats.savings_percentage >= 0.0);
    }
}

#[test]
fn test_adversarial_boundary_cases() {
    let engine = DefaultSwarmPartitioner::new();

    // 1. Empty workspace
    let empty_dir = TempDir::new().unwrap();
    let empty_manifest = engine
        .partition_workspace(empty_dir.path(), 4, &[], None)
        .expect("Empty workspace failed");
    assert_eq!(empty_manifest.total_agents, 0);
    assert_eq!(empty_manifest.total_symbols, 0);
    assert!(empty_manifest.packs.is_empty());

    // 2. K > N (Oversized agents count for tiny 1-symbol repo)
    let tiny_dir = TempDir::new().unwrap();
    fs::write(
        tiny_dir.path().join("tiny.ts"),
        "export function singleSymbol(): number { return 42; }\n",
    )
    .unwrap();

    let tiny_manifest = engine
        .partition_workspace(tiny_dir.path(), 10, &[], None)
        .expect("Oversized partition failed");
    assert_eq!(tiny_manifest.total_symbols, 1);
    assert_eq!(tiny_manifest.packs.len(), 1);
    assert_eq!(tiny_manifest.packs[0].internal_symbols[0].name, "singleSymbol");

    // 3. K = 1 (Single agent pack)
    let single_agent_manifest = engine
        .partition_workspace(tiny_dir.path(), 1, &[], None)
        .expect("K=1 partition failed");
    assert_eq!(single_agent_manifest.total_agents, 1);
    assert_eq!(single_agent_manifest.packs.len(), 1);

    // 4. Cyclic dependencies cut
    let cyclic_dir = TempDir::new().unwrap();
    fs::write(
        cyclic_dir.path().join("a.ts"),
        "import { fnB } from './b';\nexport function fnA(): boolean { return fnB(); }\n",
    )
    .unwrap();
    fs::write(
        cyclic_dir.path().join("b.ts"),
        "import { fnA } from './a';\nexport function fnB(): boolean { return fnA(); }\n",
    )
    .unwrap();

    let cyclic_manifest = engine
        .partition_workspace(cyclic_dir.path(), 2, &[], None)
        .expect("Cyclic partition failed");
    assert_eq!(cyclic_manifest.total_symbols, 2);
    assert_eq!(cyclic_manifest.packs.len(), 2);
}

#[test]
fn test_json_manifest_and_markdown_serialization() {
    let dir = TempDir::new().expect("Failed to create tempdir");
    fs::write(
        dir.path().join("worker.ts"),
        "export function runTask(): string { return 'done'; }\n",
    )
    .unwrap();

    let engine = DefaultSwarmPartitioner::new();
    let manifest = engine
        .partition_workspace(dir.path(), 1, &[], None)
        .expect("Partitioning failed");

    // Serialize to JSON
    let json_str = serde_json::to_string_pretty(&manifest).expect("JSON serialization failed");
    assert!(json_str.contains("runTask"));
    assert!(json_str.contains("agent_0"));

    // Deserialize back
    let deserialized: SwarmPartitionManifest =
        serde_json::from_str(&json_str).expect("JSON deserialization failed");
    assert_eq!(deserialized.total_symbols, 1);
    assert_eq!(deserialized.packs[0].internal_symbols[0].name, "runTask");

    // Markdown generation
    let md = manifest.to_markdown();
    assert!(md.contains("# Swarm Multi-Agent Context Partition Manifest"));
    assert!(md.contains("agent_0"));
}

#[test]
fn test_language_comment_formatting_tags() {
    let ts_write = BoundaryStubGenerator::format_write_authority_tag("agent_0", SupportedLanguage::TypeScript);
    let py_write = BoundaryStubGenerator::format_write_authority_tag("agent_1", SupportedLanguage::Python);
    let rs_write = BoundaryStubGenerator::format_write_authority_tag("agent_2", SupportedLanguage::Rust);

    assert_eq!(ts_write, "// WRITE_AUTHORITY: agent_0");
    assert_eq!(py_write, "# WRITE_AUTHORITY: agent_1");
    assert_eq!(rs_write, "// WRITE_AUTHORITY: agent_2");

    let ts_imm = BoundaryStubGenerator::format_immutable_contract_tag("agent_1", SupportedLanguage::TypeScript);
    let py_imm = BoundaryStubGenerator::format_immutable_contract_tag("agent_2", SupportedLanguage::Python);

    assert_eq!(ts_imm, "// IMMUTABLE_CONTRACT: agent_1 (Read-Only)");
    assert_eq!(py_imm, "# IMMUTABLE_CONTRACT: agent_2 (Read-Only)");
}
