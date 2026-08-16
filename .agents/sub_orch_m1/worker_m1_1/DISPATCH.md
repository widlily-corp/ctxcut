## 2026-08-16T11:06:13+05:00

You are the Implementation Worker for Milestone 1 of ctxcut: "Workspace Foundation & Core AST Engine (TS/JS)".
Working directory: C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\worker_m1_1
Project root: C:\Users\Widlily\Documents\projects\ctxcut
User requirements: C:\Users\Widlily\Documents\projects\ctxcut\ORIGINAL_REQUEST.md
Master architecture: C:\Users\Widlily\Documents\projects\ctxcut\PROJECT.md
Milestone scope: C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\SCOPE.md

Explorer handoffs:
- Architecture & Workspace: C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\explorer_workspace_1\handoff.md
- TS/JS AST Queries & Resolvers: C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\explorer_ast_ts_1\handoff.md
- Formatter & BPE Tokenizer: C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\explorer_tokenizer_fmt_1\handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your Tasks:
1. Workspace root setup:
   - Create root Cargo.toml with [workspace] containing members ["crates/ctxcut_core", "crates/ctxcut_cli", "crates/ctxcut_mcp"], [workspace.package], [workspace.dependencies], and [workspace.lints].
   - Create clippy.toml and rustfmt.toml.
   - Create root binary src/main.rs (minimal runner routing to CLI/MCP or printing help).
   - Create minimal stubs for crates/ctxcut_cli (Cargo.toml and src/lib.rs with pub fn run_cli() -> Result<(), anyhow::Error>) and crates/ctxcut_mcp (Cargo.toml and src/lib.rs with pub fn run_mcp_server() -> Result<(), anyhow::Error>) so the entire workspace compiles seamlessly.

2. Implement crates/ctxcut_core:
   - Cargo.toml: configure all required dependencies.
   - src/lib.rs: exports ContextSlicer, SliceOptions, SliceResult, ExtractedSymbol, ExtractedType, CallSignatureStub, TokenStats, SupportedLanguage, CoreError, and modules.
   - src/error.rs: complete CoreError enum with thiserror.
   - src/model.rs: complete data structures conforming strictly to PROJECT.md and SCOPE.md interface contracts.
   - src/lang/mod.rs & src/lang/typescript.rs: LanguageAdapter trait and TypeScript/TSX/JavaScript grammar integration using Tree-Sitter 0.24. Add stub adapters for python.rs, go.rs, rust_lang.rs for future milestones.
   - src/parser/mod.rs: Tree-sitter parser manager, parse error checking, and AST traversal helper utilities.
   - src/resolver/mod.rs, symbol.rs, imports.rs, types.rs, calls.rs: complete implementation.
   - src/slice/mod.rs: ContextSlicer implementing slice_symbol and slice_symbols.
   - src/formatter/mod.rs: MarkdownFormatter and JsonFormatter.
   - src/tokenizer/mod.rs: BPE token counter using tiktoken-rs cl100k_base singleton.

3. Testing & Validation:
   - Create TypeScript/JavaScript test fixtures.
   - Implement extensive unit and integration tests.
   - Run cargo test --workspace.
   - Run cargo clippy --workspace --all-targets -- -D warnings.

4. Report:
   - Write comprehensive report to handoff.md.
   - Send completion message to parent.
