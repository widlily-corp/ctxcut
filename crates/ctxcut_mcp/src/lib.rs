//! `ctxcut_mcp` — Model Context Protocol (MCP) server for ctxcut.

#![deny(missing_docs)]
#![deny(unsafe_code)]

use anyhow::Result;

/// Runs the Model Context Protocol (MCP) server over STDIO.
pub fn run_mcp_server() -> Result<()> {
    eprintln!("ctxcut MCP server starting on STDIO...");
    Ok(())
}
