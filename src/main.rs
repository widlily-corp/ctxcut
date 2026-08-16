//! `ctxcut` binary entry point.
//! Routes invocation to CLI commands or MCP server mode.

use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--mcp" || arg == "mcp") {
        ctxcut_mcp::run_mcp_server()
    } else {
        ctxcut_cli::run_cli()
    }
}
