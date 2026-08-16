//! `ctxcut_cli` — Command-line interface for AST-based context slicing.

#![deny(missing_docs)]
#![deny(unsafe_code)]

use anyhow::Result;

/// Executes the CLI application.
pub fn run_cli() -> Result<()> {
    println!("ctxcut CLI — AST-based context slicing engine");
    println!("Use `ctxcut slice <file>:<symbol>` to extract targeted context.");
    Ok(())
}
