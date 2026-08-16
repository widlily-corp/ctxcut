## 2026-08-16T06:04:29Z

You are test_writer_tm2_common_benches, responsible for creating the test support utilities in `tests/common/` and the Criterion benchmark suite in `benches/` for ctxcut.
Your working directory: C:\Users\Widlily\Documents\projects\ctxcut\.agents\test_writer_tm2_common_benches
Your parent conversation ID: 745dbab3-0710-4117-87f3-ec04335926a3 (E2E Testing Orchestrator)
Project root: C:\Users\Widlily\Documents\projects\ctxcut

Read these authoritative specification and architecture documents first:
- User requirements: C:\Users\Widlily\Documents\projects\ctxcut\ORIGINAL_REQUEST.md
- Master architecture: C:\Users\Widlily\Documents\projects\ctxcut\PROJECT.md
- Test infrastructure: C:\Users\Widlily\Documents\projects\ctxcut\TEST_INFRA.md
- Testing survey report: C:\Users\Widlily\Documents\projects\ctxcut\.agents\explorer_survey_test\handoff.md

Write ownership: You EXCLUSIVELY own creating all files in `tests/common/` and `benches/`:
1. `tests/common/`:
   - `mod.rs`: Re-exports `TokenVerifier`, `GitSandbox`, `TestRunner`, `ClipboardMock`, `NormalizedSnapshot`
   - `token_verifier.rs`: Automated `tiktoken-rs` BPE (cl100k_base) token counter asserting >=80-90% token reduction with exact metrics calculation.
   - `git_sandbox.rs`: Automated isolated temporary git repo creator (using tempfile and git2 or std::process::Command) for testing `ctxcut diff` and `--staged` with file modification helpers.
   - `runner.rs`: CLI test runner (invoking `ctxcut` binary via assert_cmd / std::process::Command) and MCP test runner (STDIO JSON-RPC client helper for sending requests and parsing responses).

2. `benches/`:
   - `parse_benchmark.rs`: Criterion benchmark for Tree-sitter AST parse latency per language (500, 2000, 10000 LOC).
   - `extraction_benchmark.rs`: Criterion benchmark for AST node location and symbol body extraction throughput.
   - `hoisting_benchmark.rs`: Criterion benchmark for scope walk and type dependency resolution.
   - `e2e_slice_benchmark.rs`: Criterion benchmark for full end-to-end slice generation pipeline (file read -> parse -> hoist -> strip -> markdown format) verifying <10ms SLA.
