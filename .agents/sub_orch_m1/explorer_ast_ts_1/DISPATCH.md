## 2026-08-16T06:04:25Z
You are an Explorer for ctxcut Milestone 1 (TS/JS AST Queries & Resolver Engine).
Working directory: C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\explorer_ast_ts_1
Project root: C:\Users\Widlily\Documents\projects\ctxcut
User requirements: C:\Users\Widlily\Documents\projects\ctxcut\ORIGINAL_REQUEST.md
Master architecture: C:\Users\Widlily\Documents\projects\ctxcut\PROJECT.md
Milestone scope: C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\SCOPE.md
Survey handoff: C:\Users\Widlily\Documents\projects\ctxcut\.agents\spec_miner_survey_1\handoff.md

Your task:
1. Read ORIGINAL_REQUEST.md, PROJECT.md, SCOPE.md, and spec_miner_survey_1 handoff.
2. Formulate the exact Tree-sitter queries, node kind matching, and AST extraction rules for TypeScript (.ts, .tsx) and JavaScript (.js, .jsx):
   - Symbol Locator: named functions, generator functions, const/let arrow functions, class declarations, method definitions (instance/static, getters, setters, constructors), interface declarations, type aliases, enums.
   - Type Hoister: extracting referenced user types/interfaces/enums from signatures and bodies, excluding built-in primitives and generics; local scope search and imported file resolution (`import { X } from './path'`).
   - Signature Stripper: identifying call expressions, locating their definitions (local or imported), generating body-stripped signatures (`export function foo(...): Ret;`).
3. Detail error handling (e.g. `SymbolNotFound`, `UnsupportedLanguage`, `ParseError`, `FileNotFound`).
4. Write your comprehensive analysis and exact code structure plan to `C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\explorer_ast_ts_1\handoff.md`.
5. Send a completion message back to parent.
