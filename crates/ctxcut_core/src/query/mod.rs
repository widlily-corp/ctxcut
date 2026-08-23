//! AST Query Engine for polyglot structural queries and preset discovery.

pub mod evaluator;
pub mod model;
pub mod presets;

pub use evaluator::AstQueryEngine;
pub use model::{AstQueryReport, MatchCapture, QueryMatchResult};
pub use presets::PresetRegistry;
