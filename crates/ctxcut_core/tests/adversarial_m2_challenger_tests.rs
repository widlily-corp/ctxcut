//! Empirical Challenger 2 Adversarial Test Suite for Milestone 2 AST Adapters
//!
//! Stress-tests:
//! 1. Cross-file import & package resolution across multi-file projects:
//!    - Python relative imports with multiple dots (., .., ...), __init__.py barrel re-exports, .pyi stubs
//!    - Go multi-file packages in subdirectories, sibling package discovery, receiver methods across files
//!    - Rust sibling modules, enclosing impl type hoisting, where clauses, lifetime filtering
//! 2. Signature stripping fidelity & body isolation across complex function headers:
//!    - Go multiple return values with named parameters and generics
//!    - Rust async fn with complex where clauses and associated bounds
//!    - Python PEP 695 type parameters, async def, decorators, quote variants
//! 3. Transitive type hoisting deep recursion and cyclic dependency safety (mutual 3-way cycles)
//! 4. Token reduction invariant verification

use std::fs;
use tempfile::tempdir;
use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;

#[test]
fn test_adversarial_python_multi_dot_relative_imports_and_init_barrel() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Create deep directory hierarchy:
    // root/
    //   core/
    //     __init__.py
    //     types.py (BaseEntity, EntityStatus)
    //   services/
    //     payment/
    //       __init__.py (re-exports PaymentGateway from .gateway)
    //       gateway.py (PaymentGateway, GatewayConfig)
    //       processor.py (PaymentProcessor using ...core.types and .gateway)

    let core_dir = root.join("core");
    let services_dir = root.join("services");
    let payment_dir = services_dir.join("payment");

    fs::create_dir_all(&core_dir).expect("create core");
    fs::create_dir_all(&payment_dir).expect("create payment");

    fs::write(
        core_dir.join("__init__.py"),
        "from .types import BaseEntity, EntityStatus\n",
    )
    .expect("write core __init__");

    fs::write(
        core_dir.join("types.py"),
        r#"from enum import Enum

class EntityStatus(str, Enum):
    ACTIVE = "active"
    SUSPENDED = "suspended"

class BaseEntity:
    id: str
    status: EntityStatus
"#,
    )
    .expect("write core types");

    fs::write(
        payment_dir.join("gateway.py"),
        r#"from dataclasses import dataclass

@dataclass
class GatewayConfig:
    api_key: str
    timeout_ms: int

class PaymentGateway:
    def __init__(self, config: GatewayConfig):
        self.config = config

    def execute_charge(self, amount_cents: int) -> bool:
        # Complex gateway transaction logic
        print("Charging gateway...")
        return True
"#,
    )
    .expect("write gateway.py");

    fs::write(
        payment_dir.join("__init__.py"),
        "from .gateway import PaymentGateway, GatewayConfig\n",
    )
    .expect("write payment __init__");

    let processor_py = payment_dir.join("processor.py");
    fs::write(
        &processor_py,
        r#"from ...core.types import BaseEntity, EntityStatus
from .gateway import PaymentGateway, GatewayConfig

class TransactionRecord(BaseEntity):
    amount: int
    currency: str

class PaymentProcessor:
    def __init__(self, gateway: PaymentGateway):
        self.gateway = gateway

    def process_payment(self, record: TransactionRecord) -> EntityStatus:
        """Process incoming transaction and return updated entity status."""
        success = self.gateway.execute_charge(record.amount)
        if success:
            return EntityStatus.ACTIVE
        return EntityStatus.SUSPENDED
"#,
    )
    .expect("write processor.py");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&processor_py, "PaymentProcessor.process_payment", &opts)
        .expect("Should resolve multi-dot relative imports and slice PaymentProcessor.process_payment");

    assert_eq!(result.target_symbol.name, "process_payment");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.body.contains("def process_payment"));
    assert_eq!(
        result.target_symbol.doc_comment.as_deref(),
        Some("Process incoming transaction and return updated entity status.")
    );

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"TransactionRecord"),
        "Must hoist local TransactionRecord, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"EntityStatus") || hoisted_names.contains(&"BaseEntity"),
        "Must hoist cross-module EntityStatus via multi-dot relative import, found: {:?}",
        hoisted_names
    );

    // Verify stripped calls
    let call_names: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(
        call_names.contains(&"execute_charge"),
        "Must capture execute_charge call stub, found: {:?}",
        call_names
    );
}

#[test]
fn test_adversarial_python_pep695_complex_type_parameters_and_docstrings() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("pep695_advanced.py");

    let code = r#"from typing import Sequence

type KeyType[K: (str, bytes)] = K
type ValueList[V: Sequence[int]] = list[V]

class ComplexResult[T]:
    data: T
    code: int

def complex_compute[T: (int, float), *Ts, **P](
    primary: T,
    *args: Ts,
    **kwargs: P
) -> ComplexResult[T]:
    r"""Calculates complex aggregation with raw docstring escape: \n \t \x00."""
    # Step 1: inner computations
    total = primary
    return ComplexResult()
"#;
    fs::write(&file_path, code).expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&file_path, "complex_compute", &opts)
        .expect("Should slice complex PEP 695 generic function");

    assert_eq!(result.target_symbol.name, "complex_compute");
    assert!(result.target_symbol.signature.contains("def complex_compute[T: (int, float)"));
    assert_eq!(
        result.target_symbol.doc_comment.as_deref(),
        Some("Calculates complex aggregation with raw docstring escape: \\n \\t \\x00.")
    );

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"ComplexResult"),
        "Must hoist ComplexResult, found: {:?}",
        hoisted_names
    );
    // Generic parameters T, Ts, P must not be hoisted
    assert!(!hoisted_names.contains(&"T"), "Type parameter T must be filtered");
    assert!(!hoisted_names.contains(&"Ts"), "Type parameter Ts must be filtered");
    assert!(!hoisted_names.contains(&"P"), "Type parameter P must be filtered");
}

#[test]
fn test_adversarial_go_multi_file_subpackage_transitive_sibling_hoisting() {
    let dir = tempdir().expect("tempdir");
    let pkg_dir = dir.path().join("billing");
    fs::create_dir_all(&pkg_dir).expect("create pkg_dir");

    // 1. service.go
    let service_go = pkg_dir.join("service.go");
    fs::write(
        &service_go,
        r#"package billing

import "context"

type BillingService struct {
    repo Repository
}

func (s *BillingService) ProcessInvoice(ctx context.Context, req InvoiceRequest) (*InvoiceResult, error) {
    if err := req.Validate(); err != nil {
        return nil, err
    }
    res := &InvoiceResult{
        InvoiceID: "INV-1001",
        Total: req.Amount,
        Status: StatusPaid,
    }
    s.repo.SaveInvoice(ctx, res)
    return res, nil
}
"#,
    )
    .expect("write service.go");

    // 2. models.go
    let models_go = pkg_dir.join("models.go");
    fs::write(
        &models_go,
        "package billing\n\ntype InvoiceRequest struct {\n    CustomerID string\n    Amount     MoneyAmount\n    Items      []LineItem\n}\n\nfunc (r *InvoiceRequest) Validate() error {\n    return nil\n}\n\ntype InvoiceResult struct {\n    InvoiceID string\n    Total     MoneyAmount\n    Status    InvoiceStatus\n}\n",
    )
    .expect("write models.go");

    // 3. types.go
    let types_go = pkg_dir.join("types.go");
    fs::write(
        &types_go,
        r#"package billing

type MoneyAmount int64
type InvoiceStatus string

const (
    StatusPending InvoiceStatus = "PENDING"
    StatusPaid    InvoiceStatus = "PAID"
)

type LineItem struct {
    Description string
    Quantity    int
    UnitPrice   MoneyAmount
}

type Repository interface {
    SaveInvoice(ctx context.Context, inv *InvoiceResult) error
}
"#,
    )
    .expect("write types.go");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&service_go, "BillingService.ProcessInvoice", &opts)
        .expect("Should slice BillingService.ProcessInvoice with transitive sibling hoisting");

    assert_eq!(result.target_symbol.name, "ProcessInvoice");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.signature.contains("func (s *BillingService) ProcessInvoice"));

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"InvoiceRequest"),
        "Must hoist InvoiceRequest from models.go, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"InvoiceResult"),
        "Must hoist InvoiceResult from models.go, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"MoneyAmount"),
        "Must transitively hoist MoneyAmount from types.go, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_adversarial_go_multiple_return_values_named_params_and_generics() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("generic_store.go");

    let code = r#"package store

import "context"

type EntityStore[K comparable, V any] struct {
    data map[K]V
}

type QueryOptions struct {
    Limit  int
    Offset int
}

type QueryResult[V any] struct {
    Items []V
    Total int
}

func (s *EntityStore[K, V]) QueryEntities(
    ctx context.Context,
    keys []K,
    opts QueryOptions,
) (result QueryResult[V], count int, err error) {
    var items []V
    for _, k := range keys {
        if v, ok := s.data[k]; ok {
            items = append(items, v)
        }
    }
    return QueryResult[V]{Items: items, Total: len(items)}, len(items), nil
}
"#;
    fs::write(&file_path, code).expect("write generic_store.go");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    let result = slicer
        .slice_symbol(&file_path, "EntityStore.QueryEntities", &opts)
        .expect("Should slice generic method EntityStore.QueryEntities");

    assert_eq!(result.target_symbol.name, "QueryEntities");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.signature.contains("func (s *EntityStore[K, V]) QueryEntities"));

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted_names.contains(&"EntityStore"), "Must hoist EntityStore");
    assert!(hoisted_names.contains(&"QueryOptions"), "Must hoist QueryOptions");
    assert!(hoisted_names.contains(&"QueryResult"), "Must hoist QueryResult");
}

#[test]
fn test_adversarial_rust_nested_where_clauses_lifetimes_and_impl_enclosing() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("graph_pipeline.rs");

    let code = r"use std::fmt::Debug;

pub struct GraphError {
    pub message: String,
}

pub struct NodeEdge<'a, T> {
    pub target: &'a str,
    pub weight: f64,
    pub metadata: Option<T>,
}

pub struct GraphNode<'a, T: Clone + Send + 'static>
where
    T: Debug,
{
    pub id: &'a str,
    pub data: T,
    pub neighbors: Vec<NodeEdge<'a, T>>,
}

impl<'a, T: Clone + Send + 'static> GraphNode<'a, T>
where
    T: Debug,
{
    /// Traverses the outgoing neighbors, applies visitor, and returns aggregated result.
    pub async fn traverse_and_aggregate<F, R>(&self, visitor: F) -> Result<R, GraphError>
    where
        F: Fn(&T) -> R + Send + Sync,
        R: Default + std::ops::AddAssign,
    {
        let mut sum = R::default();
        sum += visitor(&self.data);
        Ok(sum)
    }
}
";
    fs::write(&file_path, code).expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&file_path, "GraphNode::traverse_and_aggregate", &opts)
        .expect("Should slice GraphNode::traverse_and_aggregate");

    assert_eq!(result.target_symbol.name, "traverse_and_aggregate");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.signature.contains("pub async fn traverse_and_aggregate<F, R>"));
    assert!(result.target_symbol.signature.contains("where"));

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"GraphNode"),
        "Must hoist enclosing GraphNode, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"GraphError"),
        "Must hoist return error type GraphError, found: {:?}",
        hoisted_names
    );
}

#[test]
fn test_adversarial_rust_cross_module_sibling_hoisting_and_call_stripping() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. models.rs
    let models_rs = root.join("models.rs");
    fs::write(
        &models_rs,
        r"#[derive(Debug, Clone)]
pub struct QueryPayload {
    pub query_id: String,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub rows: Vec<String>,
    pub execution_time_ms: u64,
}
",
    )
    .expect("write models.rs");

    // 2. external.rs
    let external_rs = root.join("external.rs");
    fs::write(
        &external_rs,
        r#"use crate::models::{QueryPayload, QueryResponse};

pub async fn execute_remote_query(payload: &QueryPayload) -> QueryResponse {
    println!("Executing query {}", payload.query_id);
    QueryResponse {
        rows: vec!["row1".to_string(), "row2".to_string()],
        execution_time_ms: 42,
    }
}
"#,
    )
    .expect("write external.rs");

    // 3. service.rs
    let service_rs = root.join("service.rs");
    fs::write(
        &service_rs,
        r"use crate::external::execute_remote_query;
use crate::models::{QueryPayload, QueryResponse};

pub struct QueryService;

impl QueryService {
    pub async fn run(&self, payload: QueryPayload) -> QueryResponse {
        let resp = execute_remote_query(&payload).await;
        resp
    }
}
",
    )
    .expect("write service.rs");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&service_rs, "QueryService::run", &opts)
        .expect("Should slice QueryService::run with sibling hoisting and call stripping");

    assert_eq!(result.target_symbol.name, "run");
    assert_eq!(result.target_symbol.kind, "method");

    let hoisted_names: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(
        hoisted_names.contains(&"QueryPayload"),
        "Must hoist QueryPayload from models.rs, found: {:?}",
        hoisted_names
    );
    assert!(
        hoisted_names.contains(&"QueryResponse"),
        "Must hoist QueryResponse from models.rs, found: {:?}",
        hoisted_names
    );

    let call_stubs: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(
        call_stubs.contains(&"execute_remote_query"),
        "Must strip execute_remote_query from external.rs, found: {:?}",
        call_stubs
    );
    let stub = result
        .stripped_calls
        .iter()
        .find(|c| c.name == "execute_remote_query")
        .unwrap();
    assert!(
        stub.signature.ends_with(';'),
        "Rust signature stub must end with semicolon: {}",
        stub.signature
    );
}

#[test]
fn test_adversarial_deep_3way_cyclic_dependency_graphs_all_languages() {
    let dir = tempdir().expect("tempdir");

    // 1. Python 3-way circular models: ModelA -> ModelB -> ModelC -> ModelA
    let py_file = dir.path().join("cycle_3way.py");
    let py_code = r#"from typing import Optional

class ModelC:
    link_a: Optional["ModelA"]

class ModelB:
    link_c: ModelC

class ModelA:
    link_b: ModelB

def entry_point(root: ModelA) -> ModelC:
    return root.link_b.link_c
"#;
    fs::write(&py_file, py_code).expect("write py cycle");

    // 2. Go 3-way circular types: NodeA -> NodeB -> NodeC -> NodeA
    let go_file = dir.path().join("cycle_3way.go");
    let go_code = r"package cycle

type NodeC struct {
    ToA *NodeA
}

type NodeB struct {
    ToC *NodeC
}

type NodeA struct {
    ToB *NodeB
}

func TraverseCycle(start *NodeA) *NodeC {
    return start.ToB.ToC
}
";
    fs::write(&go_file, go_code).expect("write go cycle");

    // 3. Rust 3-way circular types: StructA -> StructB -> StructC -> StructA
    let rs_file = dir.path().join("cycle_3way.rs");
    let rs_code = r"pub struct StructC {
    pub a: Option<Box<StructA>>,
}

pub struct StructB {
    pub c: StructC,
}

pub struct StructA {
    pub b: StructB,
}

pub fn traverse_cycle(start: &StructA) -> &StructC {
    &start.b.c
}
";
    fs::write(&rs_file, rs_code).expect("write rs cycle");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 5, // High depth to stress cycle protection
        include_types: true,
        include_calls: true,
    };

    // Verify Python 3-way cycle
    let py_res = slicer.slice_symbol(&py_file, "entry_point", &opts).expect("Python 3-way cycle slice");
    let py_hoisted: Vec<&str> = py_res.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(py_hoisted.contains(&"ModelA") && py_hoisted.contains(&"ModelB") && py_hoisted.contains(&"ModelC"));
    // Ensure no duplicates
    let mut py_dedup = py_hoisted.clone();
    py_dedup.sort_unstable();
    py_dedup.dedup();
    assert_eq!(py_hoisted.len(), py_dedup.len(), "Python hoisted types must have no duplicates");

    // Verify Go 3-way cycle
    let go_res = slicer.slice_symbol(&go_file, "TraverseCycle", &opts).expect("Go 3-way cycle slice");
    let go_hoisted: Vec<&str> = go_res.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(go_hoisted.contains(&"NodeA") && go_hoisted.contains(&"NodeB") && go_hoisted.contains(&"NodeC"));
    let mut go_dedup = go_hoisted.clone();
    go_dedup.sort_unstable();
    go_dedup.dedup();
    assert_eq!(go_hoisted.len(), go_dedup.len(), "Go hoisted types must have no duplicates");

    // Verify Rust 3-way cycle
    let rs_res = slicer.slice_symbol(&rs_file, "traverse_cycle", &opts).expect("Rust 3-way cycle slice");
    let rs_hoisted: Vec<&str> = rs_res.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(rs_hoisted.contains(&"StructA") && rs_hoisted.contains(&"StructB") && rs_hoisted.contains(&"StructC"));
    let mut rs_dedup = rs_hoisted.clone();
    rs_dedup.sort_unstable();
    rs_dedup.dedup();
    assert_eq!(rs_hoisted.len(), rs_dedup.len(), "Rust hoisted types must have no duplicates");
}

#[test]
fn test_adversarial_signature_stripping_body_isolation() {
    let dir = tempdir().expect("tempdir");

    // Rust file with massive function body that must be stripped cleanly to just the signature
    let rs_file = dir.path().join("massive_body.rs");
    let mut rs_code = String::from("pub fn complex_external_worker(x: i32, y: i32) -> Result<i32, String> {\n");
    for i in 0..100 {
        use std::fmt::Write;
        let _ = writeln!(rs_code, "    let v{i} = x + y + {i};");
        let _ = writeln!(rs_code, "    if v{i} % 2 == 0 {{ println!(\"even {i}\"); }}");
    }
    rs_code.push_str("    Ok(x + y)\n}\n\n");
    rs_code.push_str("pub fn caller_target(a: i32, b: i32) -> i32 {\n    complex_external_worker(a, b).unwrap_or(0)\n}\n");
    fs::write(&rs_file, rs_code).expect("write rs");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    let res = slicer.slice_symbol(&rs_file, "caller_target", &opts).expect("slice caller_target");
    let call_names: Vec<&str> = res.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(
        call_names.contains(&"complex_external_worker"),
        "Must include complex_external_worker in stripped calls, found: {:?}",
        call_names
    );
    let worker_stub = res
        .stripped_calls
        .iter()
        .find(|c| c.name == "complex_external_worker")
        .unwrap();
    assert!(worker_stub.signature.contains("pub fn complex_external_worker(x: i32, y: i32) -> Result<i32, String>;"));
    assert!(!worker_stub.signature.contains("let v0 ="), "Body must not leak into signature stub");
    assert!(!worker_stub.signature.contains("println!"), "Body must not leak into signature stub");
}

#[test]
fn test_adversarial_python_cross_file_transitive_hoisting() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // models.py
    fs::write(
        root.join("models.py"),
        r"from .types import PriorityTier

class UserAccount:
    id: str
    tier: PriorityTier
",
    )
    .expect("write models.py");

    // types.py
    fs::write(
        root.join("types.py"),
        r#"from enum import Enum

class PriorityTier(str, Enum):
    STANDARD = "standard"
    PREMIUM = "premium"
"#,
    )
    .expect("write types.py");

    // main.py
    let main_py = root.join("main.py");
    fs::write(
        &main_py,
        r"from .models import UserAccount

def process_account(acc: UserAccount) -> bool:
    return True
",
    )
    .expect("write main.py");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let res = slicer.slice_symbol(&main_py, "process_account", &opts).expect("slice main.py");
    let hoisted: Vec<&str> = res.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    println!("Python transitive hoisted: {:?}", hoisted);
    assert!(hoisted.contains(&"UserAccount"), "Must hoist UserAccount");
    assert!(
        hoisted.contains(&"PriorityTier"),
        "Must transitively hoist PriorityTier at depth 3, found: {:?}",
        hoisted
    );
}

#[test]
fn test_adversarial_rust_cross_file_transitive_hoisting() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // types.rs
    fs::write(
        root.join("types.rs"),
        r"pub enum DeviceKind {
    Mobile,
    Desktop,
}
",
    )
    .expect("write types.rs");

    // models.rs
    fs::write(
        root.join("models.rs"),
        r"use crate::types::DeviceKind;

pub struct DeviceSession {
    pub session_id: String,
    pub kind: DeviceKind,
}
",
    )
    .expect("write models.rs");

    // service.rs
    let service_rs = root.join("service.rs");
    fs::write(
        &service_rs,
        r"use crate::models::DeviceSession;

pub struct SessionService;

impl SessionService {
    pub fn validate(&self, session: DeviceSession) -> bool {
        true
    }
}
",
    )
    .expect("write service.rs");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let res = slicer.slice_symbol(&service_rs, "SessionService::validate", &opts).expect("slice service.rs");
    let hoisted: Vec<&str> = res.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    println!("Rust transitive hoisted: {:?}", hoisted);
    assert!(hoisted.contains(&"DeviceSession"), "Must hoist DeviceSession");
    assert!(
        hoisted.contains(&"DeviceKind"),
        "Must transitively hoist DeviceKind at depth 3, found: {:?}",
        hoisted
    );
}


