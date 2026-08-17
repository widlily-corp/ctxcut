//! `ctxcut` binary entry point.
//!
//! Provides unified CLI routing to subcommands and STDIO Model Context Protocol (MCP) server.

use anyhow::Result;

fn main() -> Result<()> {
    ctxcut_cli::run_cli_handler(|opts| {
        ctxcut_mcp::run_mcp_server(ctxcut_mcp::McpServerOptions {
            log_file: opts.log_file,
            ..Default::default()
        })
    })
}
