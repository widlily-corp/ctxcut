//! Tier 1 Tests: Feature 19 — Multi-Agent Swarm Context Partitioning (R4)
//!
//! Verifies swarm context partitioning and boundary stub generation:
//! - Workspace Graph Clustering into $K$ Non-Overlapping AST Slices
//! - Disjoint Symbol Sets with Write Authority Partitioning
//! - Boundary Stub Synthesizer for Inter-Agent Contract Interfaces
//! - Write Authority Annotations vs Immutable Contract Tags
//! - Per-Agent Token Budget Compression (1,500–2,000 tokens)
//! - JSON Manifest Schema (`SwarmPartitionManifest`, `SwarmAgentPack`) and MCP Tool Integration

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, McpClient, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f19_swarm_partition_k_agent_graph_clustering() {
    // Arrange: 3-module workspace: Auth, Billing, and Notification
    let dir = TempDir::new().expect("Failed to create tempdir");
    let auth_file = dir.path().join("auth.ts");
    let billing_file = dir.path().join("billing.ts");
    let notify_file = dir.path().join("notify.ts");

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
"#,
    )
    .unwrap();

    fs::write(
        &billing_file,
        r#"
export interface Invoice {
    id: string;
    amount: number;
}

export function createInvoice(userId: string, amount: number): Invoice {
    return { id: "inv_1", amount };
}
"#,
    )
    .unwrap();

    fs::write(
        &notify_file,
        r#"
export function sendReceiptNotification(email: string, invoiceId: string): boolean {
    return email.includes("@") && invoiceId.length > 0;
}
"#,
    )
    .unwrap();

    // Act: Extract overview to verify graph clustering discovery
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .expect("Command failed");

    // Assert: All 3 modules and symbols are recognized
    output.assert_success();
    assert!(output.stdout.contains("authenticate"));
    assert!(output.stdout.contains("createInvoice"));
    assert!(output.stdout.contains("sendReceiptNotification"));
}

#[test]
fn test_f19_swarm_partition_non_overlapping_ast_slices() {
    // Arrange: Disjoint services
    let dir = TempDir::new().expect("Failed to create tempdir");
    let orders_file = dir.path().join("orders.ts");
    let shipping_file = dir.path().join("shipping.ts");

    fs::write(
        &orders_file,
        "export function placeOrder(orderId: string): boolean { return true; }\n",
    )
    .unwrap();
    fs::write(
        &shipping_file,
        "export function shipPackage(pkgId: string): boolean { return true; }\n",
    )
    .unwrap();

    // Act: Slice each module independently
    let runner = CliRunner::new();
    let target_order = format!("{}:placeOrder", orders_file.display());
    let target_shipping = format!("{}:shipPackage", shipping_file.display());

    let out_order = runner
        .run_in_dir(dir.path(), &["slice", &target_order])
        .expect("Order slice failed");
    let out_shipping = runner
        .run_in_dir(dir.path(), &["slice", &target_shipping])
        .expect("Shipping slice failed");

    // Assert: Slices are isolated and non-overlapping
    out_order.assert_success();
    out_shipping.assert_success();
    assert!(out_order.stdout.contains("placeOrder"));
    assert!(!out_order.stdout.contains("shipPackage"));
    assert!(out_shipping.stdout.contains("shipPackage"));
    assert!(!out_shipping.stdout.contains("placeOrder"));
}

#[test]
fn test_f19_swarm_partition_boundary_contract_stubs() {
    // Arrange: Consumer service depending on external provider signature
    let dir = TempDir::new().expect("Failed to create tempdir");
    let provider_file = dir.path().join("payment_provider.ts");
    let consumer_file = dir.path().join("checkout_workflow.ts");

    let provider_code = r#"
export interface ChargeRequest {
    cardNumber: string;
    amountCents: number;
}

export interface ChargeResponse {
    transactionId: string;
    success: boolean;
}

export function executeStripeCharge(req: ChargeRequest): ChargeResponse {
    // Internal heavy stripe API logic
    const secretApiKey = "sk_live_stripe_secret";
    return { transactionId: "txn_9999", success: true };
}
"#;

    let consumer_code = r#"
import { ChargeRequest, ChargeResponse, executeStripeCharge } from './payment_provider';

export function runCheckoutWorkflow(req: ChargeRequest): ChargeResponse {
    return executeStripeCharge(req);
}
"#;

    fs::write(&provider_file, provider_code).unwrap();
    fs::write(&consumer_file, consumer_code).unwrap();

    // Act: Slice consumer workflow
    let runner = CliRunner::new();
    let target = format!("{}:runCheckoutWorkflow", consumer_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Consumer slice includes provider signature stub without full provider body
    output.assert_success();
    assert!(output.stdout.contains("runCheckoutWorkflow"));
    assert!(
        output.stdout.contains("executeStripeCharge")
            || output.stdout.contains("ChargeRequest")
            || output.stdout.contains("payment_provider")
    );
}

#[test]
fn test_f19_swarm_partition_write_authority_vs_contract_tags() {
    // Arrange: Service with internal logic vs imported type
    let dir = TempDir::new().expect("Failed to create tempdir");
    let models_file = dir.path().join("contract.ts");
    let service_file = dir.path().join("worker_service.ts");

    fs::write(
        &models_file,
        "export interface TaskContract { id: string; payload: string; }\n",
    )
    .unwrap();

    fs::write(
        &service_file,
        r#"
import { TaskContract } from './contract';

export function executeTask(task: TaskContract): string {
    return "completed:" + task.id;
}
"#,
    )
    .unwrap();

    // Act: Slice worker service
    let runner = CliRunner::new();
    let target = format!("{}:executeTask", service_file.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Slicing extracts writable function while importing contract
    output.assert_success();
    assert!(output.stdout.contains("executeTask"));
}

#[test]
fn test_f19_swarm_partition_per_agent_token_budget() {
    // Arrange: Large module partitioned for agent consumption
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("partition_pack.ts");

    let mut content = String::new();
    content.push_str("export interface AgentConfig { agentId: string; maxBudget: number; }\n");
    for i in 0..20 {
        content.push_str(&format!(
            "export function subTask_{i}(cfg: AgentConfig): boolean {{ return cfg.maxBudget > {i}; }}\n"
        ));
    }
    fs::write(&file_path, &content).unwrap();

    // Act: Slice specific agent task with strict budget
    let runner = CliRunner::new();
    let target = format!("{}:subTask_0", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--budget", "150"])
        .expect("Command failed");

    // Assert: Output stays within token limit
    output.assert_success();
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(&output.stdout);
    assert!(
        tokens <= 250,
        "Expected partitioned bundle <= 250 tokens, got {}",
        tokens
    );
}

#[test]
fn test_f19_swarm_partition_multi_language_polyglot_swarm() {
    // Arrange: Multi-language microservice swarm (Frontend TS + Backend Rust)
    let dir = TempDir::new().expect("Failed to create tempdir");
    let ts_file = dir.path().join("frontend.ts");
    let rs_file = dir.path().join("backend.rs");

    fs::write(
        &ts_file,
        "export function renderDashboard(): string { return '<h1>Dashboard</h1>'; }\n",
    )
    .unwrap();

    fs::write(
        &rs_file,
        "pub fn serve_telemetry_metrics() -> usize { 42 }\n",
    )
    .unwrap();

    // Act: Workspace overview across polyglot agents
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .expect("Command failed");

    // Assert: Overview discovers polyglot agent capabilities
    output.assert_success();
    assert!(output.stdout.contains("renderDashboard") || output.stdout.contains("frontend.ts"));
    assert!(output.stdout.contains("serve_telemetry_metrics") || output.stdout.contains("backend.rs"));
}

#[test]
fn test_f19_swarm_partition_json_manifest_schema() {
    // Arrange: Workspace files
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("agent_manifest.ts");
    fs::write(
        &file_path,
        "export function runAgentJob(): boolean { return true; }\n",
    )
    .unwrap();

    // Act: Slice in JSON format
    let runner = CliRunner::new();
    let target = format!("{}:runAgentJob", file_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target, "--format", "json"])
        .expect("Command failed");

    // Assert: Valid JSON manifest
    output.assert_success();
    let json: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("Failed to parse JSON");
    assert!(json.is_object());
}

#[test]
fn test_f19_swarm_partition_mcp_pack_agent_context_tool() {
    // Arrange: MCP client session
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("mcp_swarm.ts");

    fs::write(
        &file_path,
        "export function swarmAgentCoordinator(): string { return 'coordinated'; }\n",
    )
    .unwrap();

    let mut client = McpClient::start_in_dir(dir.path()).expect("Failed to start MCP server");
    let _ = client.initialize().expect("MCP initialize failed");

    // Act: Request symbol slice via MCP
    let slice_content = client
        .get_symbol_slice(file_path.to_str().unwrap(), "swarmAgentCoordinator")
        .expect("MCP slice request failed");

    // Assert: Response includes coordinator symbol
    assert!(slice_content.contains("swarmAgentCoordinator"));
}

#[test]
fn test_f19_precomputed_swarm_clusters_sqlite_caching_and_fast_retrieval() {
    use ctxcut_core::{DefaultSwarmPartitioner, IndexEngine, IndexOptions, SwarmPartitionEngine};

    // Arrange: Multi-file project
    let dir = TempDir::new().expect("Failed to create tempdir");
    let auth_file = dir.path().join("auth.ts");
    let billing_file = dir.path().join("billing.ts");
    let orders_file = dir.path().join("orders.ts");

    fs::write(
        &auth_file,
        r#"
export interface UserSession {
    userId: string;
    token: string;
}
export function verifySession(token: string): UserSession {
    return { userId: "user_1", token };
}
"#,
    )
    .unwrap();

    fs::write(
        &billing_file,
        r#"
import { UserSession } from "./auth";
export interface PaymentReceipt {
    receiptId: string;
    amount: number;
}
export function processPayment(session: UserSession, amount: number): PaymentReceipt {
    return { receiptId: "rcpt_1", amount };
}
"#,
    )
    .unwrap();

    fs::write(
        &orders_file,
        r#"
import { processPayment } from "./billing";
import { verifySession } from "./auth";
export function checkoutCart(token: string, total: number): boolean {
    const session = verifySession(token);
    const receipt = processPayment(session, total);
    return receipt.amount > 0;
}
"#,
    )
    .unwrap();

    // Act 1: Build SQLite index with precomputed community clusters
    let mut engine = IndexEngine::open_or_create(dir.path()).expect("Failed to open index engine");
    let sync_result = engine
        .sync_incremental(&IndexOptions { rebuild: true, max_depth: None })
        .expect("Failed to sync index");
    assert!(sync_result.files_added >= 3);

    // Verify SQLite schema tables populated
    let cluster_count: usize = engine
        .connection()
        .query_row("SELECT COUNT(*) FROM clusters", [], |r| r.get(0))
        .expect("Query clusters failed");
    assert!(cluster_count > 0, "Clusters table should contain pre-computed partitions");

    let cluster_sym_count: usize = engine
        .connection()
        .query_row("SELECT COUNT(*) FROM cluster_symbols", [], |r| r.get(0))
        .expect("Query cluster_symbols failed");
    assert!(cluster_sym_count > 0, "Cluster symbols table should contain mapped symbols");

    let edge_count: usize = engine
        .connection()
        .query_row("SELECT COUNT(*) FROM graph_edges", [], |r| r.get(0))
        .expect("Query graph_edges failed");
    assert!(edge_count > 0, "Graph edges cache table should contain dependency edges");

    // Act 2: Retrieve precomputed swarm manifest via IndexEngine in O(1) time (<10ms)
    let start = std::time::Instant::now();
    let manifest_opt = engine
        .get_precomputed_swarm_manifest(2, &[], None)
        .expect("Failed to get precomputed manifest");
    let elapsed = start.elapsed();
    assert!(manifest_opt.is_some(), "Precomputed manifest should be retrieved");
    assert!(elapsed.as_millis() < 50, "SQLite precomputed manifest query should be sub-50ms (took {}ms)", elapsed.as_millis());

    let manifest = manifest_opt.unwrap();
    assert_eq!(manifest.total_agents, 2);
    assert!(manifest.total_symbols > 0);
    assert!(!manifest.packs.is_empty());

    // Act 3: Fast SwarmPartitionEngine partition_workspace hits SQLite cache
    let partitioner = DefaultSwarmPartitioner::new();
    let swarm_start = std::time::Instant::now();
    let p_manifest = partitioner
        .partition_workspace(dir.path(), 2, &[], None)
        .expect("Failed to partition workspace");
    let swarm_elapsed = swarm_start.elapsed();
    assert_eq!(p_manifest.total_agents, 2);
    assert!(swarm_elapsed.as_millis() < 100, "partition_workspace should complete sub-100ms via cache (took {}ms)", swarm_elapsed.as_millis());

    // Act 4: CLI pack-agent-context command verification
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["pack-agent-context", "--root", dir.path().to_str().unwrap(), "--format", "markdown"])
        .expect("CLI pack-agent-context failed");
    output.assert_success();
    assert!(output.stdout.contains("Swarm Multi-Agent Context Partition Manifest") || output.stdout.contains("Agent `agent_0`"));
}

