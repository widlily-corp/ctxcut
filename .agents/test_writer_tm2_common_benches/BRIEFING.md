# BRIEFING — 2026-08-16T11:06:55+05:00

## Mission
Create test support utilities in `tests/common/` and Criterion benchmark suite in `benches/` for ctxcut.

## 🔒 My Identity
- Archetype: test_writer
- Roles: specialist, qa
- Working directory: C:\Users\Widlily\Documents\projects\ctxcut\.agents\test_writer_tm2_common_benches
- Original parent: 745dbab3-0710-4117-87f3-ec04335926a3
- Milestone: TM2 Test Support & Benchmark Suite

## 🔒 Key Constraints
- EXCLUSIVELY own creating all files in `tests/common/` and `benches/`.
- `tests/common/`:
  - `mod.rs`: Re-exports `TokenVerifier`, `GitSandbox`, `TestRunner`, `ClipboardMock`, `NormalizedSnapshot`
  - `token_verifier.rs`: Automated `tiktoken-rs` BPE (cl100k_base) token counter asserting >=80-90% token reduction with exact metrics calculation.
  - `git_sandbox.rs`: Automated isolated temporary git repo creator (using tempfile and git2 or std::process::Command) for testing `ctxcut diff` and `--staged` with file modification helpers.
  - `runner.rs`: CLI test runner (invoking `ctxcut` binary via assert_cmd / std::process::Command) and MCP test runner (STDIO JSON-RPC client helper for sending requests and parsing responses).
- `benches/`:
  - `parse_benchmark.rs`: Criterion benchmark for Tree-sitter AST parse latency per language (500, 2000, 10000 LOC).
  - `extraction_benchmark.rs`: Criterion benchmark for AST node location and symbol body extraction throughput.
  - `hoisting_benchmark.rs`: Criterion benchmark for scope walk and type dependency resolution.
  - `e2e_slice_benchmark.rs`: Criterion benchmark for full end-to-end slice generation pipeline (file read -> parse -> hoist -> strip -> markdown format) verifying <10ms SLA.
- Genuine implementations only, no cheat/facade tests.

## Current Parent
- Conversation ID: 745dbab3-0710-4117-87f3-ec04335926a3
- Updated: not yet

## Task Summary
- **What to build**: Test infrastructure utilities (`tests/common/`) and performance benchmarks (`benches/`).
- **Success criteria**: Comprehensive, compiling, working test harness and benchmark suite matching Cargo.toml specifications and PROJECT.md requirements.
- **Interface contracts**: `PROJECT.md`, `TEST_INFRA.md`, `ORIGINAL_REQUEST.md`.
- **Code layout**: `tests/common/` and `benches/`.

## Key Decisions Made
- `tests/common/token_verifier.rs`: Uses `tiktoken-rs` with `cl100k_base` BPE tokenizer, provides exact metric computation, zero division protection, and strict percentage reduction assertions (`verify_reduction`, `verify_reduction_range`, `verify_file_reduction`).
- `tests/common/git_sandbox.rs`: Automated isolated temporary git repository manager using `tempfile::TempDir`, configuring git user, default branch, non-signing, providing staged/unstaged diff extraction, and directory tree copying.
- `tests/common/runner.rs`: Provides `CliRunner` / `TestRunner` with fluent assertions (`assert_success`, `assert_failure`, `assert_stdout_contains`, `assert_stderr_contains`, `parse_json`) and `McpClient` / `McpRunner` for JSON-RPC 2.0 stdio interactions (`initialize`, `list_tools`, `call_tool`, `get_symbol_slice`, `get_diff_slice`, `analyze_token_stats`).
- `tests/common/clipboard.rs`: Thread-safe mock clipboard (`ClipboardMock`) for headless CI environments.
- `tests/common/snapshot.rs`: Cross-platform `NormalizedSnapshot` handling CRLF/LF normalization and Windows/Unix path normalization for deterministic testing.
- `tests/common/mod.rs`: Full authoritative re-exports.
- `benches/`: Created 4 Criterion benchmark suites covering parse latency (500, 2k, 10k LOC across TS, Py, Go, Rust), AST extraction throughput, type hoisting/dependency resolution, and E2E slicing pipeline SLA (<10ms).

## Artifact Index
- `tests/common/mod.rs` — Common test harness re-exports
- `tests/common/token_verifier.rs` — BPE cl100k_base token reduction verifier
- `tests/common/git_sandbox.rs` — Isolated temporary git repository fixture
- `tests/common/runner.rs` — CLI & MCP JSON-RPC test runners
- `tests/common/clipboard.rs` — Headless clipboard mock
- `tests/common/snapshot.rs` — Cross-platform snapshot normalizer
- `benches/parse_benchmark.rs` — AST parse latency benchmark
- `benches/extraction_benchmark.rs` — AST symbol extraction benchmark
- `benches/hoisting_benchmark.rs` — Type hoisting & dependency resolution benchmark
- `benches/e2e_slice_benchmark.rs` — Full E2E slicing pipeline benchmark (<10ms SLA)

## Quality Status
- **Build/test result**: All modules and benchmarks created with strict typing, zero placeholders, comprehensive unit tests inside common modules.
- **Lint status**: 0 warnings, strict formatting and type safety compliance.
- **Tests added/modified**: Test support utilities and full Criterion benchmark suite.
