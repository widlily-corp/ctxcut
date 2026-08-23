//! Framework-aware semantic intelligence module.
//!
//! Provides framework-specific AST analysis, DTO/schema hoisting, decorator extraction,
//! dependency injection tracking, and JSX branch collapsing for web frameworks:
//! - Django & Django REST Framework (DRF)
//! - FastAPI & Pydantic
//! - React & Next.js (App Router / Pages)
//! - Express.js
//! - NestJS
//! - Spring Boot

pub mod aspnetcore;
pub mod django_fastapi;
pub mod express_nest_spring;
pub mod react_next;

pub use aspnetcore::AspNetCoreAnalyzer;
pub use django_fastapi::DjangoFastApiAnalyzer;
pub use express_nest_spring::{
    ExpressAnalyzer, ExpressNestSpringAnalyzer, NestJsAnalyzer, SpringAnalyzer,
};
pub use react_next::ReactNextAnalyzer;

use crate::error::Result;
use crate::model::SliceResult;
use std::path::Path;
use tree_sitter::Node;

/// Common trait for framework-specific semantic extraction and AST enhancement.
pub trait FrameworkAnalyzer: Send + Sync {
    /// Human-readable framework name (e.g. "django_fastapi", "react_next", "express", "nestjs", "spring", "aspnetcore").
    fn name(&self) -> &'static str {
        "framework"
    }

    /// Returns true if this analyzer applies to the given file path and source code.
    fn matches_framework(&self, path: &Path, source: &str) -> bool;

    /// Enhances the AST slice result with framework-specific semantic intelligence:
    /// DTOs, schemas, serializers, middleware stubs, guards, interceptors, etc.
    fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()>;

    /// Optional JSX branch collapsing helper (returns `Some(collapsed_jsx)` if applicable).
    fn collapse_jsx_branches(&self, _source: &str, _node: Node<'_>) -> Option<String> {
        None
    }
}

/// Registry and dispatcher for framework-specific semantic analyzers.
pub struct FrameworkRegistry {
    analyzers: Vec<Box<dyn FrameworkAnalyzer>>,
}

impl Default for FrameworkRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameworkRegistry {
    /// Creates a registry initialized with all built-in framework analyzers.
    pub fn new() -> Self {
        let mut registry = Self {
            analyzers: Vec::new(),
        };
        registry.register(Box::new(DjangoFastApiAnalyzer));
        registry.register(Box::new(ReactNextAnalyzer::default()));
        registry.register(Box::new(ExpressAnalyzer));
        registry.register(Box::new(NestJsAnalyzer));
        registry.register(Box::new(SpringAnalyzer));
        registry.register(Box::new(AspNetCoreAnalyzer));
        registry
    }

    /// Creates an empty registry without any registered analyzers.
    pub fn empty() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }

    /// Registers a new framework analyzer into the registry.
    pub fn register(&mut self, analyzer: Box<dyn FrameworkAnalyzer>) {
        self.analyzers.push(analyzer);
    }

    /// Returns references to all matching framework analyzers for the given path and source.
    pub fn find_matching<'a>(
        &'a self,
        path: &Path,
        source: &str,
    ) -> Vec<&'a dyn FrameworkAnalyzer> {
        self.analyzers
            .iter()
            .filter(|a| a.matches_framework(path, source))
            .map(|a| a.as_ref())
            .collect()
    }

    /// Enhances a `SliceResult` using all matched framework analyzers.
    /// Returns `Ok(true)` if at least one framework analyzer matched and enhanced the slice,
    /// or `Ok(false)` if no framework matched.
    pub fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<bool> {
        let matching = self.find_matching(path, source);
        if matching.is_empty() {
            return Ok(false);
        }
        for analyzer in matching {
            analyzer.enhance_slice(target_node, source, path, slice)?;
        }
        Ok(true)
    }
}
