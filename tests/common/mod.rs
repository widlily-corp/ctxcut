//! Common test utilities and support infrastructure for ctxcut E2E tests.
//!
//! Provides isolated Git sandboxes, automated BPE token reduction verifier,
//! CLI/MCP test runners, clipboard mocking, and cross-platform snapshot normalization.

pub mod clipboard;
pub mod git_sandbox;
pub mod runner;
pub mod snapshot;
pub mod token_verifier;

// Authoritative Re-exports
pub use clipboard::ClipboardMock;
pub use git_sandbox::GitSandbox;
pub use runner::{CliRunner, CommandOutput, McpClient, McpRunner, TestRunner};
pub use snapshot::NormalizedSnapshot;
pub use token_verifier::{TokenMetrics, TokenVerifier};
