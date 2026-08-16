//! Circular references, recursive enum ASTs, and shared ownership graphs.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex, RwLock};

/// Self-referencing enum AST for mathematical expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Value(i64),
    Variable(String),
    Unary {
        op: String,
        expr: Box<Expr>,
    },
    Binary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
}

impl Expr {
    /// Evaluates simple constant expressions recursively.
    pub fn eval_constant(&self) -> Option<i64> {
        match self {
            Expr::Value(v) => Some(*v),
            Expr::Unary { op, expr } if op == "-" => expr.eval_constant().map(|v| -v),
            Expr::Binary { op, left, right } => {
                let l = left.eval_constant()?;
                let r = right.eval_constant()?;
                match op.as_str() {
                    "+" => Some(l + r),
                    "-" => Some(l - r),
                    "*" => Some(l * r),
                    "/" if r != 0 => Some(l / r),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// Doubly linked cyclic graph node using Rc and RefCell.
#[derive(Debug)]
pub struct GraphNode {
    pub id: String,
    pub value: i64,
    pub edges: Vec<Rc<RefCell<GraphNode>>>,
    pub parent: Option<Weak<RefCell<GraphNode>>>,
}

impl GraphNode {
    pub fn new(id: impl Into<String>, value: i64) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            id: id.into(),
            value,
            edges: Vec::new(),
            parent: None,
        }))
    }

    pub fn add_edge(from: &Rc<RefCell<GraphNode>>, to: &Rc<RefCell<GraphNode>>) {
        from.borrow_mut().edges.push(Rc::clone(to));
        to.borrow_mut().parent = Some(Rc::downgrade(from));
    }
}

/// Thread-safe concurrent mesh node with Arc and RwLock.
#[derive(Debug)]
pub struct ConcurrentMeshNode {
    pub uuid: String,
    pub peers: RwLock<Vec<Arc<ConcurrentMeshNode>>>,
    pub state_lock: Mutex<i32>,
}

impl ConcurrentMeshNode {
    pub fn new(uuid: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            uuid: uuid.into(),
            peers: RwLock::new(Vec::new()),
            state_lock: Mutex::new(0),
        })
    }

    pub fn connect(a: &Arc<Self>, b: &Arc<Self>) {
        a.peers.write().unwrap().push(Arc::clone(b));
        b.peers.write().unwrap().push(Arc::clone(a));
    }
}
