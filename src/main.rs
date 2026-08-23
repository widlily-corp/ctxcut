//! `ctxcut` binary entry point.
//!
//! Provides unified CLI routing to subcommands and STDIO Model Context Protocol (MCP) server.

use anyhow::Result;

fn main() -> Result<()> {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            ctxcut_cli::run_cli_handler(|opts| {
                ctxcut_mcp::run_mcp_server(ctxcut_mcp::McpServerOptions {
                    log_file: opts.log_file,
                    ..Default::default()
                })
            })
        })?
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e))
}
