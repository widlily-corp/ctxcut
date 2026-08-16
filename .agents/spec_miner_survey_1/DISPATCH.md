## 2026-08-16T06:02:21Z

You are the AST & Multi-Language Spec Miner for ctxcut.
Your working directory: C:\Users\Widlily\Documents\projects\ctxcut\.agents\spec_miner_survey_1
User requirements file: C:\Users\Widlily\Documents\projects\ctxcut\ORIGINAL_REQUEST.md
Specification file: C:\Users\Widlily\Documents\projects\ctxcut\SPECIFICATION.md

Your task:
1. Read ORIGINAL_REQUEST.md and SPECIFICATION.md.
2. Investigate tree-sitter AST nodes, grammar crates (e.g. `tree-sitter-typescript`, `tree-sitter-python`, `tree-sitter-go`, `tree-sitter-rust`, `tree-sitter-javascript`), queries, and extraction mechanics across the 4 supported languages:
   - TypeScript/JavaScript: functions, arrow functions, methods, classes, interfaces, type aliases, enums, imports, call expressions.
   - Python: def functions, async def, methods, classes, TypedDict, dataclasses, TypeAlias, imports, calls.
   - Go: func, methods (receiver), struct types, interface types, type aliases, imports, function calls.
   - Rust: fn, async fn, pub fn, impl methods, trait definitions, struct/enum definitions, type aliases, use statements, macro/function calls.
3. Detail the exact AST traversal algorithm for:
   - Locating target symbol (by file and symbol name, e.g. path/to/file.ts:symbolName).
   - Extracting the full body of the target symbol.
   - Identifying referenced types, interfaces, enums, structs in signature and body, and hoisting/inlining their definitions.
   - Identifying external function/method calls and stripping bodies (signature-only stubs).
   - Formatting prompt-optimized Markdown output with token reduction metrics.
4. Document potential edge cases (nested types, generics, multi-file imports, anonymous callbacks, operator overloading).
5. Produce a comprehensive report in `C:\Users\Widlily\Documents\projects\ctxcut\.agents\spec_miner_survey_1\handoff.md`.
6. Send a message to parent when completed.
