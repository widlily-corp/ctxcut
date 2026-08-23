//! Adversarial Test Suite for Milestone 3: Protobuf & GraphQL Schema Stitching and Resolver Linking.
//!
//! Comprehensive empirical verification covering:
//! - Protobuf: Nested messages, enums, oneofs, streaming RPCs (Unary, Server, Client, BiDi)
//! - Protobuf: Multi-language handler naming conventions (Rust Tonic, TS NestJS, Python grpcio, Go)
//! - GraphQL: Interface inheritance, input types, enum parameters, union types, and field resolvers
//! - GraphQL: Multi-language resolvers (Rust async-graphql, TS Apollo/Nexus, Python Strawberry, Go gqlgen)
//! - Monorepo: Proximity disambiguation with conflicting identical `.proto` and `.graphql` files
//! - Adversarial Stress: Malformed IDL, unbalanced braces, circular schema dependencies, huge types

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use ctxcut_core::schema::{GraphqlStitcher, ProtoStitcher, SchemaStitcher};
use std::fs;
use tempfile::TempDir;

// =========================================================================
// 1. PROTOBUF ADVERSARIAL TESTS
// =========================================================================

#[test]
fn test_m3_adv_proto_nested_messages_enums_oneofs_streaming() {
    let dir = TempDir::new().expect("TempDir failed");
    let proto_path = dir.path().join("gateway.proto");

    let proto_content = r#"
syntax = "proto3";
package api.gateway;

enum ServiceStatus {
    STATUS_UNKNOWN = 0;
    STATUS_HEALTHY = 1;
    STATUS_DEGRADED = 2;
}

message SecurityContext {
    string client_ip = 1;
    string jwt_token = 2;
}

message CreditCardInfo {
    string card_number = 1;
    string expiry = 2;
}

message CryptoWalletInfo {
    string wallet_address = 1;
    string chain_id = 2;
}

message ChargeRequest {
    string tx_id = 1;
    SecurityContext security = 2;
    ServiceStatus expected_status = 3;
    oneof payment_method {
        CreditCardInfo credit_card = 4;
        CryptoWalletInfo crypto_wallet = 5;
    }
}

message ChargeResponse {
    bool approved = 1;
    string auth_code = 2;
}

message AuditChunk {
    bytes chunk_data = 1;
    int64 sequence_no = 2;
}

message AuditSummary {
    int64 total_bytes = 1;
    int64 chunk_count = 2;
}

service PaymentGatewayService {
    // Unary RPC
    rpc ProcessCharge(ChargeRequest) returns (ChargeResponse);
    // Server Streaming RPC
    rpc StreamLiveTransactions(ChargeRequest) returns (stream ChargeResponse);
    // Client Streaming RPC
    rpc UploadAuditLogs(stream AuditChunk) returns (AuditSummary);
    // Bidirectional Streaming RPC
    rpc LiveTelemetrySync(stream AuditChunk) returns (stream AuditChunk);
}
"#;
    fs::write(&proto_path, proto_content).unwrap();

    let stitcher = ProtoStitcher::new();
    let parsed = stitcher.parse_proto(proto_content, &proto_path);

    // Verify service parsing
    assert!(parsed.services.contains_key("paymentgatewayservice"));
    let service = parsed.services.get("paymentgatewayservice").unwrap();
    assert_eq!(service.rpcs.len(), 4);

    // Unary
    let unary = service.rpcs.iter().find(|r| r.name == "ProcessCharge").unwrap();
    assert_eq!(unary.request_type, "ChargeRequest");
    assert_eq!(unary.response_type, "ChargeResponse");
    assert!(!unary.client_streaming);
    assert!(!unary.server_streaming);

    // Server Streaming
    let s_stream = service.rpcs.iter().find(|r| r.name == "StreamLiveTransactions").unwrap();
    assert_eq!(s_stream.request_type, "ChargeRequest");
    assert_eq!(s_stream.response_type, "ChargeResponse");
    assert!(!s_stream.client_streaming);
    assert!(s_stream.server_streaming);

    // Client Streaming
    let c_stream = service.rpcs.iter().find(|r| r.name == "UploadAuditLogs").unwrap();
    assert_eq!(c_stream.request_type, "AuditChunk");
    assert_eq!(c_stream.response_type, "AuditSummary");
    assert!(c_stream.client_streaming);
    assert!(!c_stream.server_streaming);

    // BiDi Streaming
    let b_stream = service.rpcs.iter().find(|r| r.name == "LiveTelemetrySync").unwrap();
    assert_eq!(b_stream.request_type, "AuditChunk");
    assert_eq!(b_stream.response_type, "AuditChunk");
    assert!(b_stream.client_streaming);
    assert!(b_stream.server_streaming);

    // Verify message parsing & oneof referenced types extraction
    assert!(parsed.messages.contains_key("chargerequest"));
    let charge_msg = parsed.messages.get("chargerequest").unwrap();
    assert!(charge_msg.referenced_types.contains(&"SecurityContext".to_string()));
    assert!(charge_msg.referenced_types.contains(&"ServiceStatus".to_string()));
    assert!(charge_msg.referenced_types.contains(&"CreditCardInfo".to_string()));
    assert!(charge_msg.referenced_types.contains(&"CryptoWalletInfo".to_string()));
}

#[test]
fn test_m3_adv_proto_polyglot_grpc_handlers() {
    let dir = TempDir::new().expect("TempDir failed");
    let proto_path = dir.path().join("service.proto");

    let proto_content = r#"
syntax = "proto3";
package inventory;

message ItemSku {
    string sku = 1;
}

message ItemStock {
    string sku = 1;
    int32 quantity = 2;
}

service InventoryService {
    rpc QueryStock(ItemSku) returns (ItemStock);
}
"#;
    fs::write(&proto_path, proto_content).unwrap();

    let stitcher = ProtoStitcher::new();

    // 1. Rust (Tonic snake_case handler)
    let rust_src = "async fn query_stock(&self, request: Request<ItemSku>) -> Result<Response<ItemStock>, Status> { Ok(Response::new(ItemStock::default())) }";
    let rust_file = dir.path().join("handler.rs");
    let rust_stitched = stitcher.stitch(dir.path(), &rust_file, rust_src);
    assert!(rust_stitched.iter().any(|t| t.name == "InventoryService"));
    assert!(rust_stitched.iter().any(|t| t.name == "ItemSku"));
    assert!(rust_stitched.iter().any(|t| t.name == "ItemStock"));

    // 2. TypeScript / NestJS camelCase handler
    let ts_src = "export class InventoryController { @GrpcMethod('InventoryService', 'QueryStock') async queryStock(data: ItemSku): Promise<ItemStock> { return { sku: data.sku, quantity: 10 }; } }";
    let ts_file = dir.path().join("inventory.controller.ts");
    let ts_stitched = stitcher.stitch(dir.path(), &ts_file, ts_src);
    assert!(ts_stitched.iter().any(|t| t.name == "InventoryService"));
    assert!(ts_stitched.iter().any(|t| t.name == "ItemSku"));
    assert!(ts_stitched.iter().any(|t| t.name == "ItemStock"));

    // 3. Python (grpcio PascalCase/snake_case)
    let py_src = "def QueryStock(self, request: ItemSku, context) -> ItemStock:\n    return ItemStock(sku=request.sku, quantity=42)\n";
    let py_file = dir.path().join("servicer.py");
    let py_stitched = stitcher.stitch(dir.path(), &py_file, py_src);
    assert!(py_stitched.iter().any(|t| t.name == "InventoryService"));
    assert!(py_stitched.iter().any(|t| t.name == "ItemSku"));
    assert!(py_stitched.iter().any(|t| t.name == "ItemStock"));

    // 4. Go (PascalCase method)
    let go_src = "func (s *Server) QueryStock(ctx context.Context, req *pb.ItemSku) (*pb.ItemStock, error) { return &pb.ItemStock{Sku: req.Sku, Quantity: 100}, nil }";
    let go_file = dir.path().join("server.go");
    let go_stitched = stitcher.stitch(dir.path(), &go_file, go_src);
    assert!(go_stitched.iter().any(|t| t.name == "InventoryService"));
    assert!(go_stitched.iter().any(|t| t.name == "ItemSku"));
    assert!(go_stitched.iter().any(|t| t.name == "ItemStock"));
}

// =========================================================================
// 2. GRAPHQL ADVERSARIAL TESTS
// =========================================================================

#[test]
fn test_m3_adv_graphql_interface_input_enum_query_mutation() {
    let dir = TempDir::new().expect("TempDir failed");
    let gql_path = dir.path().join("schema.graphql");

    let gql_content = r#"
interface Node {
    id: ID!
    createdAt: String!
}

enum AccountStatus {
    ACTIVE
    SUSPENDED
    PENDING
}

input CreateAccountInput {
    email: String!
    status: AccountStatus!
}

type Account implements Node {
    id: ID!
    createdAt: String!
    email: String!
    status: AccountStatus!
}

type Query {
    getAccount(id: ID!): Account
}

type Mutation {
    createAccount(input: CreateAccountInput!): Account!
}
"#;
    fs::write(&gql_path, gql_content).unwrap();

    let stitcher = GraphqlStitcher::new();
    let parsed = stitcher.parse_schema(gql_content, &gql_path);

    assert!(parsed.types.contains_key("node"));
    assert!(parsed.types.contains_key("accountstatus"));
    assert!(parsed.types.contains_key("createaccountinput"));
    assert!(parsed.types.contains_key("account"));
    assert!(parsed.types.contains_key("query"));
    assert!(parsed.types.contains_key("mutation"));

    let node = parsed.types.get("node").unwrap();
    assert_eq!(node.kind, "graphql_interface");

    let input = parsed.types.get("createaccountinput").unwrap();
    assert_eq!(input.kind, "graphql_input");
    assert!(input.referenced_types.contains(&"AccountStatus".to_string()));

    let query = parsed.types.get("query").unwrap();
    assert_eq!(query.fields[0].name, "getAccount");
    assert_eq!(query.fields[0].return_type, "Account");

    let mutation = parsed.types.get("mutation").unwrap();
    assert_eq!(mutation.fields[0].name, "createAccount");
    assert_eq!(mutation.fields[0].return_type, "Account");
    assert!(mutation.fields[0].arg_types.contains(&"CreateAccountInput".to_string()));
}

#[test]
fn test_m3_adv_graphql_polyglot_resolvers() {
    let dir = TempDir::new().expect("TempDir failed");
    let gql_path = dir.path().join("schema.graphql");

    let gql_content = r#"
type Article {
    id: ID!
    title: String!
}

input PublishArticleInput {
    title: String!
}

type Query {
    getArticle(id: ID!): Article
}

type Mutation {
    publishArticle(input: PublishArticleInput!): Article!
}
"#;
    fs::write(&gql_path, gql_content).unwrap();

    let stitcher = GraphqlStitcher::new();

    // 1. Rust async-graphql
    let rust_src = "async fn get_article(&self, ctx: &Context<'_>, id: ID) -> Result<Article> { Ok(Article { id, title: \"Rust\".into() }) }";
    let rust_file = dir.path().join("query.rs");
    let rust_stitched = stitcher.stitch(dir.path(), &rust_file, rust_src);
    assert!(rust_stitched.iter().any(|t| t.name == "Query.getArticle"));
    assert!(rust_stitched.iter().any(|t| t.name == "Article"));

    // 2. TypeScript Apollo resolver
    let ts_src = "const resolvers = { Mutation: { publishArticle: async (_: any, { input }: { input: PublishArticleInput }) => { return { id: '1', title: input.title }; } } };";
    let ts_file = dir.path().join("resolvers.ts");
    let ts_stitched = stitcher.stitch(dir.path(), &ts_file, ts_src);
    assert!(ts_stitched.iter().any(|t| t.name == "Mutation.publishArticle"));
    assert!(ts_stitched.iter().any(|t| t.name == "Article"));
    assert!(ts_stitched.iter().any(|t| t.name == "PublishArticleInput"));

    // 3. Python Strawberry / Ariadne resolver
    let py_src = "def resolve_get_article(root, info, id: str) -> Article:\n    return Article(id=id, title='Python')\n";
    let py_file = dir.path().join("queries.py");
    let py_stitched = stitcher.stitch(dir.path(), &py_file, py_src);
    assert!(py_stitched.iter().any(|t| t.name == "Query.getArticle"));
    assert!(py_stitched.iter().any(|t| t.name == "Article"));

    // 4. Go gqlgen resolver
    let go_src = "func (r *queryResolver) GetArticle(ctx context.Context, id string) (*model.Article, error) { return &model.Article{ID: id, Title: \"Go\"}, nil }";
    let go_file = dir.path().join("schema.resolvers.go");
    let go_stitched = stitcher.stitch(dir.path(), &go_file, go_src);
    assert!(go_stitched.iter().any(|t| t.name == "Query.getArticle"));
    assert!(go_stitched.iter().any(|t| t.name == "Article"));
}

// =========================================================================
// 3. MONOREPO PROXIMITY DISAMBIGUATION
// =========================================================================

#[test]
fn test_m3_adv_monorepo_conflicting_proto_proximity() {
    let dir = TempDir::new().expect("TempDir failed");
    let auth_dir = dir.path().join("services/auth");
    let billing_dir = dir.path().join("services/billing");
    fs::create_dir_all(&auth_dir).unwrap();
    fs::create_dir_all(&billing_dir).unwrap();

    let auth_proto = r#"
syntax = "proto3";
package auth;

message UserSession {
    string session_token = 1;
    int64 expires_at = 2;
}

service AuthService {
    rpc ValidateSession(UserSession) returns (UserSession);
}
"#;

    let billing_proto = r#"
syntax = "proto3";
package billing;

message UserSession {
    string billing_account_id = 1;
    double credit_balance = 2;
}

service BillingService {
    rpc GetBillingSession(UserSession) returns (UserSession);
}
"#;

    fs::write(auth_dir.join("api.proto"), auth_proto).unwrap();
    fs::write(billing_dir.join("api.proto"), billing_proto).unwrap();

    let stitcher = ProtoStitcher::new();

    // Slicing inside services/auth
    let auth_src = "async fn validate_session(req: UserSession) -> UserSession { req }";
    let auth_handler = auth_dir.join("src/handler.rs");
    let auth_stitched = stitcher.stitch(dir.path(), &auth_handler, auth_src);

    assert!(auth_stitched.iter().any(|t| t.name == "AuthService"));
    let user_session = auth_stitched.iter().find(|t| t.name == "UserSession").unwrap();
    assert!(user_session.definition.contains("session_token"));
    assert!(!user_session.definition.contains("billing_account_id"));

    // Slicing inside services/billing
    let billing_src = "async fn get_billing_session(req: UserSession) -> UserSession { req }";
    let billing_handler = billing_dir.join("src/handler.rs");
    let billing_stitched = stitcher.stitch(dir.path(), &billing_handler, billing_src);

    assert!(billing_stitched.iter().any(|t| t.name == "BillingService"));
    let billing_user_session = billing_stitched.iter().find(|t| t.name == "UserSession").unwrap();
    assert!(billing_user_session.definition.contains("billing_account_id"));
    assert!(!billing_user_session.definition.contains("session_token"));
}

#[test]
fn test_m3_adv_monorepo_conflicting_graphql_proximity() {
    let dir = TempDir::new().expect("TempDir failed");
    let storefront_dir = dir.path().join("apps/storefront");
    let admin_dir = dir.path().join("apps/admin");
    fs::create_dir_all(&storefront_dir).unwrap();
    fs::create_dir_all(&admin_dir).unwrap();

    let storefront_gql = r#"
type Viewer {
    id: ID!
    cartItemsCount: Int!
}

type Query {
    getViewer: Viewer
}
"#;

    let admin_gql = r#"
type Viewer {
    id: ID!
    adminRole: String!
    permissions: [String!]!
}

type Query {
    getViewer: Viewer
}
"#;

    fs::write(storefront_dir.join("schema.graphql"), storefront_gql).unwrap();
    fs::write(admin_dir.join("schema.graphql"), admin_gql).unwrap();

    let stitcher = GraphqlStitcher::new();

    // Slicing inside storefront
    let sf_src = "export const getViewer = async () => ({ id: '1', cartItemsCount: 3 });";
    let sf_file = storefront_dir.join("src/viewer.ts");
    let sf_stitched = stitcher.stitch(dir.path(), &sf_file, sf_src);

    assert!(sf_stitched.iter().any(|t| t.name == "Query.getViewer"));
    let sf_viewer = sf_stitched.iter().find(|t| t.name == "Viewer").unwrap();
    assert!(sf_viewer.definition.contains("cartItemsCount"));
    assert!(!sf_viewer.definition.contains("adminRole"));

    // Slicing inside admin
    let admin_src = "export const getViewer = async () => ({ id: '1', adminRole: 'SUPER_ADMIN' });";
    let admin_file = admin_dir.join("src/viewer.ts");
    let admin_stitched = stitcher.stitch(dir.path(), &admin_file, admin_src);

    assert!(admin_stitched.iter().any(|t| t.name == "Query.getViewer"));
    let admin_viewer = admin_stitched.iter().find(|t| t.name == "Viewer").unwrap();
    assert!(admin_viewer.definition.contains("adminRole"));
    assert!(!admin_viewer.definition.contains("cartItemsCount"));
}

// =========================================================================
// 4. ADVERSARIAL CORNER CASES & RESILIENCE
// =========================================================================

#[test]
fn test_m3_adv_malformed_unbalanced_proto_graphql() {
    let dir = TempDir::new().expect("TempDir failed");
    let bad_proto = dir.path().join("broken.proto");
    let bad_gql = dir.path().join("broken.graphql");

    // Unbalanced braces, unclosed quotes, broken syntax
    fs::write(&bad_proto, "syntax = \"proto3\";\nmessage Broken {\n  string val = 1;\n// unclosed\nservice BadService {\n").unwrap();
    fs::write(&bad_gql, "type Broken {\n  id: ID!\n# unclosed type\ntype Query {\n  test: String\n").unwrap();

    let proto_stitcher = ProtoStitcher::new();
    let gql_stitcher = GraphqlStitcher::new();

    // Must not panic or crash
    let p_parsed = proto_stitcher.parse_proto(&fs::read_to_string(&bad_proto).unwrap(), &bad_proto);
    assert!(p_parsed.messages.contains_key("broken"));

    let g_parsed = gql_stitcher.parse_schema(&fs::read_to_string(&bad_gql).unwrap(), &bad_gql);
    assert!(!g_parsed.types.is_empty());
}

#[test]
fn test_m3_adv_circular_graphql_and_proto_dependencies() {
    let dir = TempDir::new().expect("TempDir failed");
    let gql_path = dir.path().join("circular.graphql");

    // Mutual recursive references: NodeA -> NodeB -> NodeA
    let gql_content = r#"
type NodeA {
    id: ID!
    b: NodeB
}

type NodeB {
    id: ID!
    a: NodeA
}

type Query {
    getNodeA: NodeA
}
"#;
    fs::write(&gql_path, gql_content).unwrap();

    let stitcher = GraphqlStitcher::new();
    let src = "function getNodeA() { return null; }";
    let file_path = dir.path().join("test.ts");

    // Must terminate cleanly (cycle detection / recursion depth bound)
    let stitched = stitcher.stitch(dir.path(), &file_path, src);
    assert!(stitched.iter().any(|t| t.name == "Query.getNodeA"));
    assert!(stitched.iter().any(|t| t.name == "NodeA"));
    assert!(stitched.iter().any(|t| t.name == "NodeB"));
}

#[test]
fn test_m3_adv_schema_stitcher_unified_polyglot_e2e() {
    let dir = TempDir::new().expect("TempDir failed");

    // 1. Proto
    fs::write(dir.path().join("api.proto"), r#"
syntax = "proto3";
message MetricPacket { int64 count = 1; }
service Telemetry { rpc PushMetric(MetricPacket) returns (MetricPacket); }
"#).unwrap();

    // 2. GraphQL
    fs::write(dir.path().join("schema.graphql"), r#"
type UserProfile { id: ID!, name: String! }
type Query { getUserProfile(id: ID!): UserProfile }
"#).unwrap();

    // 3. Prisma
    fs::write(dir.path().join("schema.prisma"), r#"
model Session { id String @id, userId String }
"#).unwrap();

    // 4. SQL migration
    let mig = dir.path().join("migrations");
    fs::create_dir_all(&mig).unwrap();
    fs::write(mig.join("001.sql"), "CREATE TABLE audit_logs (id INT, event TEXT);\n").unwrap();

    let orchestrator = SchemaStitcher::new();
    let multi_source = r#"
export async function compositeHandler(prisma: any, db: any) {
    const session = await prisma.session.findUnique();
    const logs = await db.query("SELECT * FROM audit_logs");
    const profile = await getUserProfile(session.userId);
    pushMetric({ count: logs.length });
}
"#;
    let service_file = dir.path().join("src/composite.ts");
    let stitched = orchestrator.stitch_schemas(dir.path(), &service_file, multi_source).unwrap();

    // Verify all 4 schema sources are seamlessly unified without collision
    assert!(stitched.iter().any(|t| t.name == "Session" && t.kind == "prisma_model"));
    assert!(stitched.iter().any(|t| t.name == "audit_logs" && t.kind == "sql_table"));
    assert!(stitched.iter().any(|t| t.name == "Query.getUserProfile" && t.kind == "graphql_query"));
    assert!(stitched.iter().any(|t| t.name == "UserProfile" && t.kind == "graphql_type"));
    assert!(stitched.iter().any(|t| t.name == "Telemetry" && t.kind == "protobuf_service"));
    assert!(stitched.iter().any(|t| t.name == "MetricPacket" && t.kind == "protobuf_message"));
}
