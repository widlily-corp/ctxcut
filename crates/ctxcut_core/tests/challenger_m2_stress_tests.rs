//! Challenger Milestone 2 deep adversarial, stress, and boundary test suite.
//! Tests Python, Go, and Rust AST slicing engines under hostile, pathological, and edge-case conditions.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tempfile::tempdir;
use ctxcut_core::error::CoreError;
use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;

#[test]
fn test_python_deep_nested_and_multiline_decorators() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("decorators_service.py");
    let code = r#"
from typing import Annotated, Optional
from pydantic import BaseModel, Field

class RequestPayload(BaseModel):
    user_id: str
    metadata: dict[str, str]

class ResponseResult(BaseModel):
    success: bool
    data: Optional[RequestPayload] = None

@app.post(
    "/api/v2/secure/transaction",
    response_model=ResponseResult,
    tags=["transactions", "v2"],
    summary="Multi-line decorated async endpoint with complex arguments"
)
@rate_limiter(
    limit=100,
    window=60
)
@auth_required(role="admin")
async def execute_secure_transaction(
    payload: RequestPayload,
    dry_run: bool = False,
) -> ResponseResult:
    """Execute high security financial transaction with validation.
    
    Args:
        payload: The request payload containing user and meta.
        dry_run: Simulation flag.
    """
    result = await payment_gateway.process_charge(payload.user_id)
    return ResponseResult(success=True, data=payload)
"#;
    fs::write(&file_path, code).expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&file_path, "execute_secure_transaction", &opts)
        .expect("Should slice execute_secure_transaction");

    assert_eq!(result.target_symbol.name, "execute_secure_transaction");
    assert_eq!(result.target_symbol.kind, "function");
    assert!(result.target_symbol.body.contains("@app.post("));
    assert!(result.target_symbol.body.contains("@rate_limiter("));
    assert!(result.target_symbol.body.contains("@auth_required(role=\"admin\")"));
    assert!(result.target_symbol.signature.contains("async def execute_secure_transaction"));
    
    let doc = result.target_symbol.doc_comment.expect("Docstring should be extracted");
    assert!(doc.contains("Execute high security financial transaction"));

    let hoisted: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted.contains(&"RequestPayload"), "Must hoist RequestPayload: {:?}", hoisted);
    assert!(hoisted.contains(&"ResponseResult"), "Must hoist ResponseResult: {:?}", hoisted);

    // Call stripping check
    let calls: Vec<&str> = result.stripped_calls.iter().map(|c| c.name.as_str()).collect();
    assert!(calls.contains(&"process_charge"), "Must strip process_charge call: {:?}", calls);
}

#[test]
fn test_python_pep695_generics_and_protocol_classes() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("protocols.py");
    let code = r#"
from typing import Protocol, Generic, Optional

class Repository[T](Protocol):
    def get(self, id: str) -> Optional[T]:
        ...
    def save(self, entity: T) -> bool:
        ...

type EntityMap[T] = dict[str, T]

class UserEntity:
    id: str
    name: str

class UserStore[T]:
    def __init__(self, repo: Repository[UserEntity]):
        self.repo = repo

    def retrieve_user(self, user_id: str) -> Optional[UserEntity]:
        """Fetch user by strong typed ID."""
        return self.repo.get(user_id)
"#;
    fs::write(&file_path, code).expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&file_path, "UserStore.retrieve_user", &opts)
        .expect("Should slice UserStore.retrieve_user");

    assert_eq!(result.target_symbol.name, "retrieve_user");
    assert_eq!(result.target_symbol.kind, "method");
    
    let hoisted: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted.contains(&"UserEntity"), "Must hoist UserEntity: {:?}", hoisted);
}

#[test]
fn test_python_10_node_circular_type_ring() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("circular_ring.py");
    let code = r"
from typing import Optional

class NodeA:
    next: Optional[NodeB] = None
class NodeB:
    next: Optional[NodeC] = None
class NodeC:
    next: Optional[NodeD] = None
class NodeD:
    next: Optional[NodeE] = None
class NodeE:
    next: Optional[NodeF] = None
class NodeF:
    next: Optional[NodeG] = None
class NodeG:
    next: Optional[NodeH] = None
class NodeH:
    next: Optional[NodeI] = None
class NodeI:
    next: Optional[NodeJ] = None
class NodeJ:
    next: Optional[NodeA] = None

def traverse_ring(start: NodeA) -> NodeJ:
    curr = start
    return NodeJ()
";
    fs::write(&file_path, code).expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 10,
        include_types: true,
        include_calls: true,
    };

    let start = Instant::now();
    let result = slicer
        .slice_symbol(&file_path, "traverse_ring", &opts)
        .expect("Circular ring should resolve without infinite loop");
    let elapsed = start.elapsed();

    println!("Python 10-node circular ring traversal took: {:?}", elapsed);
    assert_eq!(result.target_symbol.name, "traverse_ring");
    assert!(result.hoisted_types.len() >= 2, "Must hoist at least NodeA and NodeJ");
}

#[test]
fn test_go_generic_methods_and_custom_constraints() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("generics.go");
    let code = r"
package stream

type Constraint interface {
    ~int | ~string | ~float64
}

type StreamElement[T Constraint] struct {
    Value T
    Index int
}

type FilterFunc[T Constraint] func(elem StreamElement[T]) bool

type DataPipeline[T Constraint, R any] struct {
    source []StreamElement[T]
}

func NewDataPipeline[T Constraint, R any](items []StreamElement[T]) *DataPipeline[T, R] {
    return &DataPipeline[T, R]{source: items}
}

func (p *DataPipeline[T, R]) Transform(filter FilterFunc[T]) ([]StreamElement[T], error) {
    var out []StreamElement[T]
    for _, item := range p.source {
        if filter(item) {
            out = append(out, item)
        }
    }
    return out, nil
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
        .slice_symbol(&file_path, "DataPipeline.Transform", &opts)
        .expect("Should slice generic method DataPipeline.Transform");

    assert_eq!(result.target_symbol.name, "Transform");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.signature.contains("func (p *DataPipeline[T, R]) Transform"));

    let hoisted: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted.contains(&"StreamElement"), "Must hoist StreamElement: {:?}", hoisted);
    assert!(hoisted.contains(&"FilterFunc"), "Must hoist FilterFunc: {:?}", hoisted);
    // Generics T and R should NOT be hoisted as standalone types
    assert!(!hoisted.contains(&"T"), "Generic T must not be hoisted: {:?}", hoisted);
    assert!(!hoisted.contains(&"R"), "Generic R must not be hoisted: {:?}", hoisted);
}

#[test]
fn test_go_interface_embedded_specifications_and_stubs() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("interfaces.go");
    let code = r"
package store

type Reader interface {
    Read(p []byte) (n int, err error)
}

type Writer interface {
    Write(p []byte) (n int, err error)
}

type ReadWriter interface {
    Reader
    Writer
    Flush() error
}

type AuditLogger struct {
    writer ReadWriter
}

func (a *AuditLogger) LogEvent(message string) error {
    b := []byte(message)
    _, err := a.writer.Write(b)
    if err != nil {
        return err
    }
    return a.writer.Flush()
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
        .slice_symbol(&file_path, "AuditLogger.LogEvent", &opts)
        .expect("Should slice AuditLogger.LogEvent");

    assert_eq!(result.target_symbol.name, "LogEvent");
    let hoisted: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted.contains(&"ReadWriter"), "Must hoist ReadWriter: {:?}", hoisted);
}

#[test]
fn test_rust_async_trait_generic_impl_and_where_clauses() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("async_service.rs");
    let code = r#"
use std::fmt::Debug;
use std::future::Future;

pub trait Serializer<T> {
    fn serialize(&self, item: &T) -> Vec<u8>;
}

pub struct JsonConfig {
    pub pretty: bool,
}

pub struct GatewayClient<S> {
    serializer: S,
    config: JsonConfig,
}

impl<S> GatewayClient<S> {
    pub fn new(serializer: S, config: JsonConfig) -> Self {
        Self { serializer, config }
    }

    pub async fn dispatch_async<'a, T, R>(
        &'a self,
        payload: &'a T,
    ) -> Result<R, String>
    where
        S: Serializer<T> + Sync,
        T: Debug + Send + 'a,
        R: Default + Send + 'static,
    {
        let raw = self.serializer.serialize(payload);
        if raw.is_empty() {
            return Err("Empty payload".to_string());
        }
        Ok(R::default())
    }
}
"#;
    fs::write(&file_path, code).expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&file_path, "GatewayClient::dispatch_async", &opts)
        .expect("Should slice GatewayClient::dispatch_async");

    assert_eq!(result.target_symbol.name, "dispatch_async");
    assert_eq!(result.target_symbol.kind, "method");
    assert!(result.target_symbol.signature.contains("pub async fn dispatch_async<'a, T, R>"));
    assert!(result.target_symbol.signature.contains("where"));

    let hoisted: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted.contains(&"GatewayClient"), "Must hoist enclosing GatewayClient struct: {:?}", hoisted);
    assert!(hoisted.contains(&"Serializer"), "Must hoist Serializer trait: {:?}", hoisted);
    assert!(!hoisted.contains(&"T"), "Generic T must not be hoisted: {:?}", hoisted);
    assert!(!hoisted.contains(&"S"), "Generic S must not be hoisted: {:?}", hoisted);
    assert!(!hoisted.contains(&"R"), "Generic R must not be hoisted: {:?}", hoisted);
}

#[test]
fn test_rust_mutually_recursive_enums_and_boxes() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("ast_eval.rs");
    let code = r"
pub enum Expression {
    Literal(i64),
    Variable(String),
    BinaryOp {
        op: String,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    StatementList(Vec<Statement>),
}

pub struct Statement {
    pub expr: Expression,
    pub line: usize,
}

pub fn evaluate_ast(root: Expression) -> i64 {
    match root {
        Expression::Literal(val) => val,
        Expression::Variable(_) => 0,
        Expression::BinaryOp { left, right, .. } => evaluate_ast(*left) + evaluate_ast(*right),
        Expression::StatementList(stmts) => stmts.into_iter().map(|s| evaluate_ast(s.expr)).sum(),
    }
}
";
    fs::write(&file_path, code).expect("write");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
    };

    let result = slicer
        .slice_symbol(&file_path, "evaluate_ast", &opts)
        .expect("Mutually recursive Expression and Statement must slice cleanly");

    assert_eq!(result.target_symbol.name, "evaluate_ast");
    let hoisted: Vec<&str> = result.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted.contains(&"Expression"), "Must hoist Expression: {:?}", hoisted);
    assert!(hoisted.contains(&"Statement"), "Must hoist Statement: {:?}", hoisted);
}

#[test]
fn test_stress_multithreaded_concurrent_slicing() {
    let slicer = Arc::new(ContextSlicer::new());
    let mut handles = Vec::new();

    for i in 0..20 {
        let slicer_clone = Arc::clone(&slicer);
        let handle = thread::spawn(move || {
            let opts = SliceOptions::default();
            let py_path = Path::new("../../tests/fixtures/python/realistic_payment_service/payment_service.py");
            let go_path = Path::new("../../tests/fixtures/go/realistic_auth_service/service.go");
            let rs_path = Path::new("../../tests/fixtures/rust/realistic_inventory_service/inventory.rs");

            let r1 = slicer_clone.slice_symbol(py_path, "PaymentProcessor.execute_charge", &opts);
            assert!(r1.is_ok(), "Thread {} Py slice failed", i);

            let r2 = slicer_clone.slice_symbol(go_path, "AuthService.AuthenticateUser", &opts);
            assert!(r2.is_ok(), "Thread {} Go slice failed", i);

            let r3 = slicer_clone.slice_symbol(rs_path, "InventoryService::reserve_stock", &opts);
            assert!(r3.is_ok(), "Thread {} Rust slice failed", i);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Concurrent thread panicked!");
    }
}

#[test]
fn test_hostile_syntax_resilience_and_partial_recovery() {
    let dir = tempdir().expect("tempdir");
    let opts = SliceOptions::default();
    let slicer = ContextSlicer::new();

    // Hostile Python: top function valid, syntax error in bottom function
    let py_path = dir.path().join("broken.py");
    let py_code = r"
def first_valid_function(x: int) -> int:
    return x * 2

def broken_syntax_function(:
    if True
        broken = [1, 2, 3
";
    fs::write(&py_path, py_code).expect("write");
    let r1 = slicer.slice_symbol(&py_path, "first_valid_function", &opts);
    assert!(r1.is_ok(), "Tree-sitter error recovery should slice first valid python function");

    // Hostile Go: broken struct, invalid tokens
    let go_path = dir.path().join("broken.go");
    let go_code = r#"
package broken

func TopValidFunc(a int) int {
    return a + 10
}

type Corrupted struct {
    broken ??? ===
}

func BottomValidFunc(b string) string {
    return b + " suffix"
}
"#;
    fs::write(&go_path, go_code).expect("write");
    let g1 = slicer.slice_symbol(&go_path, "TopValidFunc", &opts);
    assert!(g1.is_ok(), "Tree-sitter error recovery should slice TopValidFunc in Go");
    let g2 = slicer.slice_symbol(&go_path, "BottomValidFunc", &opts);
    assert!(g2.is_ok(), "Tree-sitter error recovery should slice BottomValidFunc in Go");

    // Hostile Rust: broken macro, missing semicolons, unbalanced braces
    let rs_path = dir.path().join("broken.rs");
    let rs_code = r#"
pub fn top_valid_rs(x: u32) -> u32 {
    x * 42
}

pub struct BrokenStruct {
    invalid %% $$$
}

pub fn bottom_valid_rs(msg: &str) -> String {
    format!("Msg: {}", msg)
}
"#;
    fs::write(&rs_path, rs_code).expect("write");
    let rs1 = slicer.slice_symbol(&rs_path, "top_valid_rs", &opts);
    assert!(rs1.is_ok(), "Tree-sitter error recovery should slice top_valid_rs in Rust");
    let rs2 = slicer.slice_symbol(&rs_path, "bottom_valid_rs", &opts);
    assert!(rs2.is_ok(), "Tree-sitter error recovery should slice bottom_valid_rs in Rust");
}

#[test]
fn test_missing_symbol_available_list_fidelity() {
    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    let py_path = Path::new("../../tests/fixtures/python/fastapi_routes.py");
    let err_py = slicer.slice_symbol(py_path, "non_existent_handler", &opts).unwrap_err();
    match err_py {
        CoreError::SymbolNotFound { available_symbols, .. } => {
            assert!(available_symbols.contains(&"get_user_profile".to_string()));
            assert!(available_symbols.contains(&"create_item".to_string()));
        }
        _ => panic!("Expected SymbolNotFound"),
    }

    let go_path = Path::new("../../tests/fixtures/go/structs_interfaces.go");
    let err_go = slicer.slice_symbol(go_path, "NonExistentMethod", &opts).unwrap_err();
    match err_go {
        CoreError::SymbolNotFound { available_symbols, .. } => {
            assert!(available_symbols.contains(&"Service.Execute".to_string()));
        }
        _ => panic!("Expected SymbolNotFound"),
    }

    let rs_path = Path::new("../../tests/fixtures/rust/traits_generics_lifetimes.rs");
    let err_rs = slicer.slice_symbol(rs_path, "NonExistentRustFn", &opts).unwrap_err();
    match err_rs {
        CoreError::SymbolNotFound { available_symbols, .. } => {
            assert!(available_symbols.contains(&"process_batch".to_string()));
        }
        _ => panic!("Expected SymbolNotFound"),
    }
}

#[test]
fn test_adversarial_token_reduction_empirical_measurements() {
    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    // 1. Python large file
    let py_path = Path::new("../../tests/fixtures/python/large_file.py");
    let py_res = slicer.slice_symbol(py_path, "analytics_module_fn_001", &opts).expect("py slice");
    println!(
        "PYTHON Large File Reduction: Raw = {} tokens, Sliced = {} tokens, Savings = {:.2}%",
        py_res.stats.raw_file_tokens, py_res.stats.sliced_tokens, py_res.stats.savings_percentage
    );
    assert!(py_res.stats.savings_percentage >= 85.0);

    // 2. Go large file
    let go_path = Path::new("../../tests/fixtures/go/large_file.go");
    let go_res = slicer.slice_symbol(go_path, "ComputeGoClusterMetric_001", &opts).expect("go slice");
    println!(
        "GO Large File Reduction: Raw = {} tokens, Sliced = {} tokens, Savings = {:.2}%",
        go_res.stats.raw_file_tokens, go_res.stats.sliced_tokens, go_res.stats.savings_percentage
    );
    assert!(go_res.stats.savings_percentage >= 85.0);

    // 3. Rust large file
    let rs_path = Path::new("../../tests/fixtures/rust/large_file.rs");
    let rs_res = slicer.slice_symbol(rs_path, "compute_rust_engine_fn_001", &opts).expect("rs slice");
    println!(
        "RUST Large File Reduction: Raw = {} tokens, Sliced = {} tokens, Savings = {:.2}%",
        rs_res.stats.raw_file_tokens, rs_res.stats.sliced_tokens, rs_res.stats.savings_percentage
    );
    assert!(rs_res.stats.savings_percentage >= 85.0);
}
