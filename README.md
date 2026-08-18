# ⚡ ctxcut

> **AST-powered contextual code slicer, bidirectional patcher & test context generator for LLMs & AI agents. Zero token bloat.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Model Context Protocol](https://img.shields.io/badge/MCP-Compatible-green.svg)](https://modelcontextprotocol.io)
[![Tests Passing](https://img.shields.io/badge/Tests-428%20Passing-brightgreen.svg)]()

---

## 🎯 The Problem: Context Obesity in LLMs & AI Agents

When working with LLMs (Claude, GPT-4, Cursor, Gemini, Codex), feeding source code usually forces painful compromises:
1. **Full-file dumping (Repomix / gitingest):** Injects thousands of lines of irrelevant helper code and vendor files, burning tokens, causing *Lost-in-the-Middle* degradation, and risking MCP timeouts.
2. **Naive RAG / Text Chunks:** Slices code every 500 characters, breaking signatures, types, and caller contracts.
3. **Manual multi-step querying:** Agents spend minutes hunting down cross-file interfaces, DTOs, enums, serializers, and external dependencies across 10 different files.
4. **Fragile String Replacements:** LLM-generated code replacements fail due to whitespace or comment shifts.

## 💡 The Solution: `ctxcut` 2.0 (The 6-Pillar Agent Engine)

`ctxcut` is an **AST-accurate surgical toolkit** engineered in Rust for agentic software engineering:

- 🚀 **1. Smart Traversal & Timeout Guard:** Automatic `.gitignore` & `.ctxcutignore` resolution, vendor/cache blacklisting (`node_modules`, `target`, `.venv`, `.git`), instant `--fast` shallow scan, and robust MCP deadline guards.
- 🕸️ **2. Deep Semantic Multi-File Slicing (`--depth 1`):** Resolves local module imports across TS/JS, Python, Go, and Rust, automatically inlining stripped foreign signatures and types without extra agent queries.
- 🧩 **3. Framework Intelligence:** Specialized extractors for **Django / FastAPI** (Serializers, Schemas, permissions, models, `Depends`), **React / Next.js** (Props interfaces, custom hooks, intelligent JSX branch collapsing), and **Express / NestJS / Spring** (DTOs, middleware chains, Guards).
- 🎯 **4. Adaptive Token Budgeting (`--budget <N>`):** Deterministic 5-level progressive semantic degradation engine that fits complex codebases strictly into token limits.
- 🛠️ **5. Bidirectional AST Patcher (`ctxcut patch`):** Surgical AST node replacement that updates files in place while preserving indentation, line endings (CRLF/LF), and comments with strict syntax validation.
- 🧪 **6. Isolated Test Context Generator (`ctxcut test-context`):** Assembles minimal test packages (target code, input/output contracts, mock/spy signatures, and reference project test fixtures) for AAA-style unit test creation.

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

### From Source (Rust)
```bash
cargo install --git https://github.com/widlily-corp/ctxcut
```

---

## 🚀 Quick Start

### 1. Multi-File Slicing with Adaptive Budget
```bash
# Slice symbol with cross-file dependency resolution and 1,500 token budget
ctxcut slice ./src/services/order.ts:processRefund --depth 1 --budget 1500 --clip
```

### 2. Output
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

### 3. Surgical AST Patching
```bash
# Safely replace a function without touching surrounding whitespace or comments
ctxcut patch ./src/services/order.ts:processRefund --with ./new_refund.ts
```

### 4. Generate Isolated Test Context
```bash
# Generate mock scaffolding and fixture context for unit tests
ctxcut test-context ./src/services/order.ts:processRefund --framework vitest
```

---

## 🛠️ CLI Commands & Flags

| Command | Description | Key Flags |
| :--- | :--- | :--- |
| `ctxcut slice <path:symbol>` | Extract contextual AST slice | `--depth <N>`, `--budget <N>`, `--clip`, `-o <file>` |
| `ctxcut patch <path:symbol> --with <replacement>` | Surgical AST node replacement in source file | `--dry-run`, `--diff` |
| `ctxcut test-context <path:symbol>` | Assemble isolated testing & mock bundle | `--framework <name>`, `--depth <N>` |
| `ctxcut diff` | Slice functions modified in git diff | `--staged`, `--budget <N>`, `--clip` |
| `ctxcut stats <path>` | Calculate token savings for repo/file | `--fast` (instant shallow scan), `--json` |
| `ctxcut metrics` | View lifetime token savings dashboard & ROI | `--clear` |
| `ctxcut route <METHOD> <PATH>` | Extract web route handler and DTO schemas | `--budget <N>` |
| `ctxcut setup-mcp` | Auto-configure MCP server in your IDE | `--ide <ide_name>` |
| `ctxcut mcp` | Launch Model Context Protocol STDIO server | `--log-file <path>`, `--timeout <secs>` |

---

## 🤖 Model Context Protocol (MCP) Integration

### Automatic Setup

One command to auto-detect and configure your IDE:

```bash
# Configure all detected IDEs at once
ctxcut setup-mcp

# Or target a specific IDE
ctxcut setup-mcp --ide antigravity
ctxcut setup-mcp --ide cursor
ctxcut setup-mcp --ide claude
ctxcut setup-mcp --ide vscode
```

Supported IDEs: **Google Antigravity**, **Cursor**, **Claude Desktop**, **VS Code / Cline**, **Roo Code**.

### Complete 6-Pillar MCP Tools Suite

| Tool Name | Parameters | Description |
| :--- | :--- | :--- |
| `get_symbol_slice` | `path`, `symbol`, `depth?`, `budget?` | Extracts AST slice with hoisted types, cross-file dependencies, and framework enrichment |
| `get_diff_slice` | `path?`, `staged?`, `budget?` | Extracts contextual AST slices for all functions modified in git diff |
| `analyze_token_stats` | `path`, `fast?` | High-speed token estimation with `.gitignore` and vendor blacklist support |
| `patch_symbol` | `path`, `symbol`, `replacement`, `dry_run?` | AST-safe surgical code replacement with syntax validation |
| `get_test_context` | `path`, `symbol`, `framework?` | Generates isolated test bundle with signatures, contracts, and mock scaffolding |
| `get_route_slice` | `method`, `route_path`, `budget?` | Slices API route handler, serializers, schemas, and middleware chains |

---

## 🧩 Framework Intelligence

| Ecosystem | Automatic AST Capabilities |
| :--- | :--- |
| **Django & DRF** | Serializers, Model definitions, `permission_classes`, `filter_backends`, `pagination_class` |
| **FastAPI** | Pydantic Request/Response models, `Depends(...)`, `Security(...)`, query/path params |
| **React & Next.js** | Component `Props` interfaces, custom hooks (`useAuth`, `useTableSort`), intelligent JSX branch collapsing |
| **Express & NestJS** | Request/Response DTOs, route controllers, `@UseGuards`, `@UseInterceptors`, middleware chains |
| **Spring Boot** | `@RestController`, `@RequestMapping`, DTO models, Service call stripping |

---

## 📊 Token Savings Telemetry

Every slice operation records metrics to `~/.ctxcut/metrics.jsonl`. View cumulative savings:

```bash
ctxcut metrics
```

```text
📊 ctxcut Lifetime Token Savings
======================================================
Total Requests:       520
Total Tokens Saved:   2,940,000
Estimated Cost Saved: $8.82
Average Compression:  88.2%
======================================================

Language Breakdown:
  TypeScript:  240 requests | 1,320,000 tokens saved
  Python:      142 requests |   810,000 tokens saved
  Go:           84 requests |   490,000 tokens saved
  Rust:         54 requests |   320,000 tokens saved
```

---

## 🌐 Supported Languages

| Language | AST Engine | Features |
| :--- | :--- | :--- |
| **TypeScript / TSX** | tree-sitter-typescript | Generics, barrel re-exports, decorators, JSX branch collapser |
| **JavaScript / JSX** | tree-sitter-javascript | CommonJS `require()`, ES6 module imports, JSX stubs |
| **Python** | tree-sitter-python | PEP 695 generics, Pydantic v1/v2, Django models, async/await decorators |
| **Go** | tree-sitter-go | Pointer/value receivers, sibling package resolution, generics |
| **Rust** | tree-sitter-rust | `impl` blocks, traits, lifetimes, `where` clauses, macro hygiene |

---

## 🏗️ Workspace Architecture

```
ctxcut/
├── crates/
│   ├── ctxcut_core/     # AST parser, traversal, resolver, framework, budget, patch, test_context
│   ├── ctxcut_cli/      # Clap CLI frontend (slice, patch, test-context, diff, stats, metrics)
│   └── ctxcut_mcp/      # Model Context Protocol JSON-RPC STDIO server with timeout safety
├── tests/
│   ├── tier1_features/  # Core language & feature parity tests
│   ├── tier2_boundaries/# Edge cases, syntax errors, unicode
│   ├── tier3_cross_feature/ # MCP chaining, IDE setup, clipboard
│   ├── tier4_real_world/# Microservice & multi-framework workloads
│   └── tier5_adversarial/ # Stress testing, fuzzing, token invariants
├── install.sh           # Linux/macOS one-line installer
├── install.ps1          # Windows PowerShell one-line installer
└── .github/workflows/release.yml  # Multi-platform binary release pipeline
```

---

## 🧪 Quality & Test Verification

| Test Suite | Tests | Status |
| :--- | :--- | :--- |
| Tier 1 — Core Features | 88 | ✅ 100% |
| Tier 2 — Boundaries & Edge Cases | 91 | ✅ 100% |
| Tier 3 — Cross-Feature & MCP | 56 | ✅ 100% |
| Tier 4 — Real-World Multi-Framework Workloads | 36 | ✅ 100% |
| Tier 5 — Adversarial & Stress Suites | 73 | ✅ 100% |
| Unit Tests, Traversal & Patching | 84 | ✅ 100% |
| **Total** | **428** | **✅ 100%** |

---

## 📄 License

MIT © [widlily-corp](https://github.com/widlily-corp)
