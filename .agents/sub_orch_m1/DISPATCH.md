## 2026-08-16T06:04:01Z

You are the Sub-Orchestrator for Milestone 1 of ctxcut: "Workspace Foundation & Core AST Engine (TS/JS)".
Your working directory: C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1
Project root: C:\Users\Widlily\Documents\projects\ctxcut
Your parent conversation ID: 7f6a6784-239e-411e-bbac-1e1b7d4a94cf
User requirements file: C:\Users\Widlily\Documents\projects\ctxcut\ORIGINAL_REQUEST.md
Master architecture document: C:\Users\Widlily\Documents\projects\ctxcut\PROJECT.md
Survey reports:
- C:\Users\Widlily\Documents\projects\ctxcut\.agents\spec_miner_survey_1\handoff.md
- C:\Users\Widlily\Documents\projects\ctxcut\.agents\explorer_survey_cli_mcp\handoff.md

Your scope (Milestone 1):
1. Setup Cargo workspace root manifest `Cargo.toml`, `clippy.toml`, `rustfmt.toml`.
2. Create `crates/ctxcut_core` with pure AST parsing engine, dependency graph traversal, slicing engine, markdown/JSON formatter, and BPE token counter (tiktoken-rs).
3. Implement Tree-Sitter parsing and symbol extraction for TypeScript / JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`):
   - Symbol locator: locate function declarations, arrow functions, class methods, types, interfaces, enums by name or range.
   - Type hoister: traverse scope and AST to inline referenced types, interfaces, DTOs, type aliases, and enums from local and imported files.
   - Signature stripper: strip 100% of bodies from called external functions/methods, preserving exact signatures.
   - Formatter: prompt-optimized Markdown generation and JSON output.
   - BPE Token counter: calculate raw tokens vs slice tokens and savings percentage using `tiktoken-rs`.
4. Ensure 0 compiler warnings on `cargo clippy --all-targets -- -D warnings` and 100% unit test pass rate.
