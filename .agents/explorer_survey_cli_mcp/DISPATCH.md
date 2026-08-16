## 2026-08-16T06:02:21Z

You are the CLI, MCP & System Architecture Explorer for ctxcut.
Your working directory: C:\Users\Widlily\Documents\projects\ctxcut\.agents\explorer_survey_cli_mcp
User requirements file: C:\Users\Widlily\Documents\projects\ctxcut\ORIGINAL_REQUEST.md
Specification file: C:\Users\Widlily\Documents\projects\ctxcut\SPECIFICATION.md

Your task:
1. Read ORIGINAL_REQUEST.md and SPECIFICATION.md.
2. Design the Rust workspace crate architecture:
   - `crates/ctxcut_core`: Pure AST parsing, dependency graph traversal, slicing engine, markdown formatter, token calculator.
   - `crates/ctxcut_cli`: `clap` (derive) CLI interface, `colored` formatting, `arboard` clipboard integration, git diff integration (`git2` or git CLI), route resolver for web frameworks (Express, FastAPI, Actix, Gin, etc.).
   - `crates/ctxcut_mcp`: MCP (Model Context Protocol) stdio JSON-RPC server exposing `get_symbol_slice`, `get_diff_slice`, `analyze_token_stats`.
   - Root binary crate linking CLI and MCP.
3. Analyze command specifications:
   - `ctxcut slice <path:symbol> [--clip] [-o <file>]` (single & multi-symbol).
   - `ctxcut diff [--staged] [--clip]`.
   - `ctxcut stats <path>`.
   - `ctxcut route <METHOD> <PATH>`.
   - `ctxcut mcp`.
4. Analyze dependency selection (compatible versions, no heavy unnecessary deps, sub-10ms performance, zero unsafe outside tree-sitter C bindings).
5. Produce a comprehensive architecture report in `C:\Users\Widlily\Documents\projects\ctxcut\.agents\explorer_survey_cli_mcp\handoff.md`.
6. Send a message to parent when completed.
