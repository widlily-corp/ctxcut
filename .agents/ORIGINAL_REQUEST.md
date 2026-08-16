# Original User Request

## 2026-08-16T06:01:42Z

Build `ctxcut`, a lightning-fast Rust CLI and MCP server that performs AST-based dependency slicing on source code to extract minimal, self-contained context (target function body, inlined type definitions, stripped signature-only stubs for external calls) for LLM prompts and AI coding agents.

Working directory: C:\Users\Widlily\Documents\projects\ctxcut
Integrity mode: development

## Requirements

### R1. Multi-Language AST Parsing Engine (Rust + Tree-Sitter)
- Implement AST parsing and symbol resolution using `tree-sitter` for TypeScript/JavaScript, Python, Go, and Rust.
- Support locating target symbols (functions, methods, classes, types) by file and symbol name (e.g. `path/to/file.ts:symbolName`).
- Target parse and traversal execution time must be sub-10ms for files under 2,000 LOC.

### R2. Dependency Graph Traversal & AST Context Slicing
- Traverse the target symbol's AST to extract:
  1. Full body of the target symbol.
  2. Complete definitions of referenced types, interfaces, DTOs, type aliases, and enums used in the function signature and body (type hoisting / inlining).
  3. Signatures only (parameter types and return types) of external called functions/methods, stripping out function bodies entirely (`body stripping`).
- Format output as clean, prompt-optimized Markdown containing:
  - Target Function (Full Body)
  - Required Types & Enums (Extracted)
  - External Dependencies (Signatures Only)
  - Token reduction metrics / metadata.

### R3. High-Performance Terminal CLI & Clipboard Integration
- Implement an intuitive CLI interface using `clap` (derive API) and `colored`:
  - `ctxcut slice <path:symbol> [--clip] [-o <file>]`: Extract context slice for one or more comma-separated symbols. Direct copy to system clipboard via `arboard`.
  - `ctxcut diff [--staged] [--clip]`: Automatically identify modified functions in Git diff/staged changes and build contextual slices for them.
  - `ctxcut stats <path>`: Scan repository and calculate potential token savings compared to full-file inclusion.
  - `ctxcut route <METHOD> <PATH>`: Resolve web framework route handlers (Express/FastAPI/Actix/etc.) and extract handler with DTOs and validation schemas.

### R4. Model Context Protocol (MCP) Server
- Implement a JSON-RPC / STDIO Model Context Protocol (MCP) server:
  - Expose tools: `get_symbol_slice`, `get_diff_slice`, `analyze_token_stats`.
  - Seamlessly integrate with AI agents (Cursor, Claude Code, Antigravity) allowing agents to request surgical AST slices dynamically.

### R5. Comprehensive Test Fixtures & Benchmarks
- Include comprehensive unit and integration test fixtures for TypeScript, Python, Go, and Rust demonstrating 80-90%+ token reduction while maintaining 100% semantic correctness of types and signatures.
- Add criterion benchmarks for parsing speed and AST extraction throughput.

## Acceptance Criteria

### Core AST & Slicing Engine
- [ ] Successfully parses TypeScript, Python, Go, and Rust source files using `tree-sitter`.
- [ ] Accurately extracts target function AST node without modifying its body.
- [ ] Correctly resolves and inlines referenced interfaces/types/enums from local and imported files.
- [ ] Strips method/function bodies from called dependencies while preserving exact signatures.
- [ ] Slicing execution completes in under 10ms for standard project files.

### CLI & Workflows
- [ ] `ctxcut slice <file:symbol>` outputs formatted Markdown to stdout, file (`-o`), or system clipboard (`--clip`).
- [ ] `ctxcut slice <file:sym1,sym2>` handles multiple symbols across files.
- [ ] `ctxcut diff` parses `git diff` / `git diff --cached` and outputs slices for all modified functions.
- [ ] `ctxcut stats` produces human-readable token savings estimation report.

### MCP Server
- [ ] `ctxcut mcp` starts stdio MCP server adhering to MCP specification.
- [ ] Exposes `get_symbol_slice` and responds with valid JSON-RPC schema and markdown payload.

### Quality & Architecture
- [ ] 0 compiler warnings on `cargo check` / `cargo clippy`.
- [ ] 100% pass rate across all automated unit & integration test suites.
- [ ] Clean modular architecture: `crates/ctxcut_core`, `crates/ctxcut_cli`, `crates/ctxcut_mcp` (or unified structured modules).
