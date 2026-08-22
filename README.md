# ⚡ ctxcut

> **AST-accurate contextual code slicer, surgical patcher, test context generator & workspace indexer for LLMs & AI coding agents. Zero token bloat.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Model Context Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io)
[![Tests Passing](https://img.shields.io/badge/Tests-428%2B%20Passing-brightgreen.svg)]()

---

## 🎯 The Problem: Context Obesity in LLMs & AI Agents

When feeding source code to modern LLMs (Claude 3.5 Sonnet, GPT-4o, Gemini 1.5 Pro, DeepSeek V3), developers and AI coding agents face painful compromises:

1. **Full-file dumping (Repomix / gitingest):** Ingests thousands of lines of unrelated helper code, imports, and boilerplate. This burns token budgets, triggers *Lost-in-the-Middle* reasoning degradation, and causes MCP timeouts on large repositories.
2. **Naive RAG / Text Splitting:** Chunks code every 500 characters, breaking functions across boundaries, dropping parameter types, and destroying caller contracts.
3. **Manual Multi-Step Querying:** Agents spend dozens of tool calls chasing down imported interfaces, DTOs, enums, serializers, and external dependencies across multiple files.
4. **Fragile String Replacements:** LLM-generated line edits fail due to minor whitespace, comment, or indentation mismatches.

---

## 💡 The Solution: `ctxcut` (The 6-Pillar + Expansion Engine)

`ctxcut` is an **AST-accurate, thread-safe surgical engine** engineered in Rust for agentic software engineering:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SOURCE CODE / REPOSITORY                          │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                ┌──────────────────────▼──────────────────────┐
                │ 1. Smart Traversal & Ignore Engine          │
                │    (.gitignore, .ctxcutignore, fast scan)   │
                └──────────────────────┬──────────────────────┘
                                       │
        ┌──────────────────────────────┼──────────────────────────────┐
        │                              │                              │
┌───────▼────────────────┐   ┌─────────▼──────────────┐   ┌───────────▼────────────┐
│ 2. Deep AST Slicing    │   │ 3. Framework Analyzers │   │ 7. Workspace Overview  │
│  • Cross-file imports  │   │  • Django / DRF models │   │  • Body-free indexing  │
│  • Transitive types    │   │  • FastAPI Pydantic    │   │  • Fast symbol outline │
│  • Stripped call stubs │   │  • React/Next JSX stubs│   │  • 90-95% compression  │
│  • Multi-symbol batch  │   │  • Express/Nest DTOs   │   └───────────┬────────────┘
└───────┬────────────────┘   └─────────┬──────────────┘               │
        │                              │                              │
        └──────────────────────────────┼──────────────────────────────┘
                                       │
                ┌──────────────────────▼──────────────────────┐
                │ 4. Adaptive Token Budgeting                 │
                │    (5-level progressive semantic degradation│
                └──────────────────────┬──────────────────────┘
                                       │
        ┌──────────────────────────────┼──────────────────────────────┐
        │                              │                              │
┌───────▼────────────────┐   ┌─────────▼──────────────┐   ┌───────────▼────────────┐
│ 5. Bidirectional Patch │   │ 6. Test Context Gen    │   │ 8. Telemetry & ROI     │
│  • AST node locator    │   │  • AAA test scaffolding│   │  • ~/.ctxcut/metrics   │
│  • Indent preservation │   │  • Mock/spy signatures │   │  • Model tier pricing  │
│  • Pre-write validator │   │  • Workspace fixtures  │   │  • Terminal dashboard  │
└───────┬────────────────┘   └─────────┬──────────────┘   └───────────┬────────────┘
        │                              │                              │
        └──────────────────────────────┴──────────────────────────────┘
                                       │
         ┌─────────────────────────────▼─────────────────────────────┐
         │              DELIVERY INTERFACES & PLATFORMS              │
         │  • Unified CLI (`slice`, `diff`, `route`, `overview`, ...)│
         │  • Model Context Protocol (STDIO JSON-RPC 2.0 Server)     │
         │  • IDE Auto-Config (Antigravity, Cursor, Claude, VSCode)  │
         └───────────────────────────────────────────────────────────┘
```

- 🚀 **1. Smart Traversal & Timeout Guard:** Automatic `.gitignore` & `.ctxcutignore` evaluation, vendor blacklisting (`node_modules`, `target`, `.venv`, `.git`), instant `--fast` shallow scan, and MCP deadline guards.
- 🕸️ **2. Deep Semantic Multi-File Slicing (`--depth 1`):** Resolves local module imports across TypeScript, JavaScript, Python, Go, and Rust, automatically hoisting referenced types and inlining stripped foreign signatures without extra roundtrips.
- 👥 **Multi-Symbol Batch Slicing (`path:sym1,sym2`):** Slices multiple target symbols in a single query with unified type and call stub deduplication.
- 🧩 **3. Framework Intelligence:** Specialized extractors for **Django / DRF** (Serializers, Models, permissions, filter backends), **FastAPI** (Pydantic models, `Depends`, `Security`), **React / Next.js** (Props interfaces, custom hooks, intelligent JSX branch collapsing), and **Express / NestJS / Spring** (DTOs, middleware chains, Guards).
- 🎯 **4. Adaptive Token Budgeting (`--budget <N>`):** Deterministic 5-level progressive semantic compression engine that fits complex codebases strictly into token limits.
- 🛠️ **5. Bidirectional AST Patcher (`ctxcut patch`):** Surgical AST node replacement that updates files in place while preserving indentation, line endings (CRLF/LF), and comments with strict Tree-sitter syntax validation.
- 🧪 **6. Isolated Test Context Generator (`ctxcut test-context`):** Assembles minimal test packages (target code, input/output contracts, mock/spy signatures, and reference project test fixtures) for AAA-style unit test creation.
- 🗺️ **7. Workspace Symbol Overview (`ctxcut overview`):** Generates a token-dense architectural outline of all declarations, interfaces, and routes without ingesting full implementation bodies (90–95% savings).
- 📊 **8. Persistent Telemetry & ROI Dashboard:** Records all operations to append-only `~/.ctxcut/metrics.jsonl` with cost models across economy, baseline, and frontier LLMs.

**Result:** **80–92% token reduction** with 100% semantic, syntactic, and type fidelity.

---

## 📦 Installation

### One-Line Installer (Recommended)

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/widlily-corp/ctxcut/main/install.ps1 | iex
```

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/widlily-corp/ctxcut/main/install.sh | bash
```

### From Source (Rust Cargo)
```bash
cargo install --git https://github.com/widlily-corp/ctxcut
# Or locally from repository clone:
cargo install --path . --force
```

---

## 🚀 Quick Start & Usage Examples

### 1. Single & Multi-Symbol Slicing with Adaptive Budget

```bash
# Slice a single function with hoisted cross-file types and 1,500 token budget
ctxcut slice ./src/services/order.ts:processRefund --depth 1 --budget 1500 --clip

# Slice multiple functions in one file with unified type deduplication
ctxcut slice ./src/services/order.ts:processRefund,cancelOrder --budget 2000
```

#### Output Example:
````markdown
### Context Slice: `processRefund`
*Language: `typescript` | Lines: `28` (was `480`) | Tokens: `450` (was `4,200`) | Savings: `89.3%`*

#### 1. Target Implementation (Full Body)
```typescript
export async function processRefund(orderId: string, reason: RefundReason): Promise<RefundResult> {
  const order = await orderRepo.findById(orderId);
  if (!order || order.status !== OrderStatus.COMPLETED) {
    throw new InvalidOrderStateError(orderId);
  }
  const tx = await paymentGateway.refund({
    chargeId: order.chargeId,
    amount: order.totalAmount,
  });
  return orderRepo.markRefunded(orderId, tx.id);
}
```

#### 2. Hoisted Types & Data Contracts
```typescript
export enum OrderStatus { PENDING = 'PENDING', COMPLETED = 'COMPLETED', REFUNDED = 'REFUNDED' }
export type RefundReason = 'FRAUD' | 'CUSTOMER_REQUEST' | 'DEFECTIVE';
export interface RefundResult { success: boolean; transactionId: string; }
```

#### 3. Cross-File Dependencies & Signatures (Body Stripped)
```typescript
// from src/repositories/orderRepo.ts
findById(id: string): Promise<Order | null>;
markRefunded(id: string, txId: string): Promise<RefundResult>;

// from src/services/paymentGateway.ts
refund(params: { chargeId: string; amount: number }): Promise<{ id: string; status: string }>;
```
````

---

### 2. Workspace Symbol Overview

Generate a high-density architectural outline of all symbols, types, and classes without reading full bodies:

```bash
# Inspect entire workspace up to depth 2
ctxcut overview . --depth 2

# Limit overview to 3,000 tokens
ctxcut overview src/ --budget 3000 --format markdown
```

---

### 3. Surgical AST Patching

Replace an existing function safely without touching surrounding code, comments, or whitespace:

```bash
# Preview changes in unified diff format (dry-run)
ctxcut patch ./src/services/order.ts:processRefund --code "export async function processRefund(orderId: string): Promise<void> {}" --dry-run

# Apply replacement from an external file
ctxcut patch ./src/services/order.ts:processRefund --file ./new_refund.ts
```

---

### 4. Isolated Test Context Generator

Synthesize AAA test templates, parameter types, mock scaffolding, and existing project fixtures:

```bash
# Generate Vitest test context for TypeScript
ctxcut test-context ./src/services/order.ts:processRefund --framework vitest

# Generate Pytest test context for Python
ctxcut test-context ./src/services/auth.py:verify_token --framework pytest
```

---

### 5. Git Diff Slicing & Route Resolution

```bash
# Extract slices for all modified functions in staged git changes
ctxcut diff --staged --budget 2000

# Resolve HTTP route endpoint directly to controller AST and DTO models
ctxcut route GET /api/v1/orders --budget 1500
```

---

### 6. Fast Token Stats & Telemetry Dashboard

```bash
# Instant shallow scan of repository token optimization potential
ctxcut stats . --fast

# Interactive terminal ROI dashboard
ctxcut metrics
```

---

## 🛠️ CLI Subcommand Reference

The `ctxcut` CLI provides 11 dedicated subcommands:

| Subcommand | Description | Key Arguments & Flags | Example |
| :--- | :--- | :--- | :--- |
| `slice` | Extracts minimal AST context slice for target symbol(s) | `<target>` (`path:symbol` or `path:sym1,sym2`)<br>`--budget <N>`: Token budget limit<br>`--depth <N>`: Type hoisting depth (default: 1)<br>`--no-types`: Disable type hoisting<br>`--no-calls`: Disable signature stripping<br>`--clip`: Copy to clipboard<br>`-o, --output <PATH>`: Save to file<br>`--format <markdown\|json>` | `ctxcut slice src/calc.ts:add,multiply --budget 1000` |
| `diff` | Extracts AST slices for all functions modified in Git diff | `--staged`: Inspect staged changes only<br>`--budget <N>`: Token budget limit per slice<br>`--clip`: Copy to clipboard<br>`-o, --output <PATH>`: Save to file<br>`--format <markdown\|json>` | `ctxcut diff --staged` |
| `route` | Resolves web framework route handler to controller slice | `<method>` (GET, POST, PUT, DELETE)<br>`<path>` (e.g. `/api/v1/users`)<br>`--budget <N>`: Token budget limit<br>`--clip`: Copy to clipboard<br>`-o, --output <PATH>`: Save to file<br>`--format <markdown\|json>` | `ctxcut route POST /auth/login` |
| `patch` | Surgically replaces a function, method, or class in source code | `<target>` (`path:symbol`)<br>`-c, --code <CODE>`: Replacement code string<br>`-f, --file <PATH>`: Replacement code file<br>`--dry-run`: Preview unified diff | `ctxcut patch src/app.ts:init --code "..." --dry-run` |
| `test-context` | Generates isolated test bundle with mock scaffolding and fixtures | `<target>` (`path:symbol`)<br>`--framework <vitest\|jest\|pytest\|cargo\|gotest>`<br>`--budget <N>`: Token budget limit<br>`--clip`: Copy to clipboard<br>`-o, --output <PATH>`: Save to file<br>`--format <markdown\|json>` | `ctxcut test-context src/math.rs:sqrt --framework cargo` |
| `stats` | Analyzes repository/file token savings and optimization stats | `[<path>]`: File or directory path<br>`-f, --fast`: Fast shallow estimation scan<br>`--history`: View lifetime telemetry history & ROI<br>`--format <text\|json>` | `ctxcut stats . --fast`<br>`ctxcut stats --history` |
| `metrics` | Displays interactive lifetime token savings & ROI dashboard | `--format <text\|json>` (default: text) | `ctxcut metrics` |
| `overview` | High-level workspace symbol indexing & architectural outline | `[<path>]`: Workspace root directory (default: `.`)<br>`--depth <N>`: Traversal depth limit<br>`--budget <N>`: Token budget limit<br>`--format <text\|json>` | `ctxcut overview . --depth 2` |
| `setup-mcp` | Automatically configures IDEs to use ctxcut as an MCP server | `--ide <antigravity\|claude\|cursor\|vscode\|all>`<br>`--workspace`: Project/workspace config<br>`--workspace-dir <PATH>`: Workspace root path<br>`--custom-path <PATH>`: Custom config file path<br>`--use-absolute-path`: Use absolute binary path<br>`--remove`: Uninstall ctxcut from MCP config<br>`--dry-run`: Preview config changes | `ctxcut setup-mcp --ide antigravity`<br>`ctxcut setup-mcp --ide all --workspace` |
| `init` | Alias for `setup-mcp` to initialize ctxcut in IDE configuration | Same options as `setup-mcp` (except `--remove`) | `ctxcut init --ide cursor` |
| `mcp` | Launches Model Context Protocol (MCP) server over STDIO | `--log-file <PATH>`: Path for structured JSONL logging | `ctxcut mcp` |

---

## 🤖 Model Context Protocol (MCP) Integration

### Automated IDE Configuration

Run one command to automatically discover and configure ctxcut in your installed IDEs:

```bash
# Configure all detected IDEs
ctxcut setup-mcp

# Or target specific environments
ctxcut setup-mcp --ide antigravity
ctxcut setup-mcp --ide cursor
ctxcut setup-mcp --ide claude
ctxcut setup-mcp --ide vscode
```

### Manual Configuration Snippets

#### Google Antigravity / Cursor (`~/.gemini/antigravity/mcp_config.json` or `.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "ctxcut": {
      "command": "ctxcut",
      "args": ["mcp"]
    }
  }
}
```

#### Claude Desktop (`claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "ctxcut": {
      "command": "ctxcut",
      "args": ["mcp"]
    }
  }
}
```

---

### Complete MCP Tools Suite (8 Tools)

| Tool Name | Parameters | Description |
| :--- | :--- | :--- |
| `get_symbol_slice` | `path` (req), `symbol` (req, single or comma-separated), `depth` (opt, def 1), `budget` (opt), `no_types` (opt), `no_calls` (opt), `timeout_ms` (opt) | Extracts AST slice with hoisted types, cross-file dependencies, and framework enrichment. Supports multi-symbol batching (`sym1,sym2`). |
| `get_diff_slice` | `path` (opt), `staged` (opt, def false), `budget` (opt), `timeout_ms` (opt) | Extracts contextual AST slices for all functions modified in git working tree or staged changes. |
| `get_workspace_overview` | `path` (opt), `depth` (opt), `budget` (opt), `timeout_ms` (opt) | Indexes workspace symbols and generates a token-dense architectural outline without reading full file bodies. |
| `get_route_slice` | `method` (req), `path` (req), `root_dir` (opt), `budget` (opt), `timeout_ms` (opt) | Resolves web API route handler, serializers, schemas, and middleware chains (Express, FastAPI, Gin, Axum). |
| `get_test_context` | `path` (req), `symbol` (req), `framework` (opt), `budget` (opt), `timeout_ms` (opt) | Generates isolated test bundle with parameter/return types, mock/spy signatures, and nearby test fixtures. |
| `patch_symbol` | `path` (req), `symbol` (req), `code` (req), `dry_run` (opt, def false), `timeout_ms` (opt) | Surgically replaces a function/class in source code with AST boundary alignment and Tree-sitter syntax validation. |
| `analyze_token_stats` | `path` (req), `fast` (opt, def true for dirs), `timeout_ms` (opt) | Calculates repository or file token savings and optimization statistics with `.gitignore` compliance. |
| `get_metrics` | `format` (opt: "text"\|"json", def "text"), `clear` (opt, def false), `timeout_ms` (opt) | Inspects cumulative lifetime token reduction telemetry, dollar ROI analytics, and language distributions. |

---

## 🧩 Framework Intelligence

| Ecosystem | Automatic AST Capabilities |
| :--- | :--- |
| **Django & DRF** | Serializers, Model definitions, `permission_classes`, `filter_backends`, `pagination_class` |
| **FastAPI** | Pydantic Request/Response models, `Depends(...)`, `Security(...)`, query/path parameters |
| **React & Next.js** | Component `Props` interfaces, custom hooks (`useAuth`, `useTableSort`), intelligent JSX branch collapsing |
| **Express & NestJS** | Request/Response DTOs, route controllers, `@UseGuards`, `@UseInterceptors`, middleware chains |
| **Spring Boot** | `@RestController`, `@RequestMapping`, DTO models, service call stripping |

---

## 🌐 Supported Languages

| Language | Extensions | AST Engine | Capabilities |
| :--- | :--- | :--- | :--- |
| **TypeScript / TSX** | `.ts`, `.tsx`, `.mts`, `.cts` | `tree-sitter-typescript` | Generics, barrel re-exports, decorators, JSX branch collapser, type aliases |
| **JavaScript / JSX** | `.js`, `.jsx`, `.mjs`, `.cjs` | `tree-sitter-javascript` | CommonJS `require()`, ES6 module imports, JSX stubs, prototype methods |
| **Python** | `.py`, `.pyi` | `tree-sitter-python` | PEP 695 generics, Pydantic v1/v2, Django models, async decorators |
| **Go** | `.go` | `tree-sitter-go` | Pointer/value receivers, sibling package resolution, structs, interfaces |
| **Rust** | `.rs` | `tree-sitter-rust` | `impl` blocks, traits, lifetimes, `where` clauses, macro hygiene |

---

## 📊 Token Savings Telemetry & ROI Dashboard

`ctxcut` records every slicing invocation to `~/.ctxcut/metrics.jsonl`. View interactive analytics anytime:

```bash
ctxcut metrics
```

```text
================================================================================
  CTXCUT TELEMETRY & TOKEN SAVINGS DASHBOARD
================================================================================
  Total Invocations:       542 requests
  Original Raw Tokens:     3,412,800 tokens
  Sliced Transmitted:        418,200 tokens
  Cumulative Tokens Saved: 2,994,600 tokens
  Average Compression:     87.7% token reduction
--------------------------------------------------------------------------------
  ESTIMATED ECONOMIC SAVINGS:
    • Economy Tier ($0.50 / 1M):  $1.50 USD
    • Standard Tier ($3.00 / 1M): $8.98 USD
    • Frontier Tier ($15.00 / 1M): $44.92 USD
--------------------------------------------------------------------------------
  LANGUAGE BREAKDOWN:
    • TypeScript: 248 queries  │  1,420,100 tokens saved (88.4%)
    • Python:     154 queries  │    845,200 tokens saved (87.1%)
    • Go:          82 queries  │    462,300 tokens saved (86.9%)
    • Rust:        58 queries  │    267,000 tokens saved (88.0%)
================================================================================
```

---

## 🧪 Quality & Test Verification

`ctxcut` is verified with an extensive multi-tier test suite:

| Test Suite | Coverage Area | Status |
| :--- | :--- | :--- |
| **Tier 1** | Traversal, ignore rules, binary detection, fast stats scan | ✅ 100% |
| **Tier 2** | Multi-file imports, transitive type hoisting, signature stripping | ✅ 100% |
| **Tier 3** | Framework extractors, 5-level budget compression, multi-symbol batching | ✅ 100% |
| **Tier 4** | Real-world microservice workloads across TS, Python, Go, Rust | ✅ 100% |
| **Tier 5** | Telemetry, dashboard, IDE setup, timeout safety, adversarial stress | ✅ 100% |
| **Unit & Core** | AST parser, language adapters, patcher validation, fixture finder | ✅ 100% |
| **Total** | **428+ Tests Verified** | **✅ 100% Pass** |

---

## 📄 License

MIT © [widlily-corp](https://github.com/widlily-corp)
