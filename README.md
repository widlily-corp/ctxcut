# ⚡ ctxcut

> **AST-powered contextual code slicer for LLM prompts & AI agents. Zero token bloat.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Model Context Protocol](https://img.shields.io/badge/MCP-Compatible-green.svg)](https://modelcontextprotocol.io)

---

## 🎯 The Problem: Context Obesity in LLMs

When working with LLMs (Claude, GPT-4, Cursor, Codex), providing code context usually forces painful compromises:
1. **Full-file dumping (Repomix / gitingest):** Injects thousands of lines of irrelevant helper code, burning tokens and causing *Lost-in-the-Middle* degradation.
2. **Naive RAG / Text Chunks:** Slices code every 500 characters, breaking signatures, types, and caller contracts.
3. **Manual copy-pasting:** Devs spend minutes hunting down interfaces, enums, and dependencies across 5 different files.

## 💡 The Solution: `ctxcut`

`ctxcut` acts as an **AST-accurate surgical scalpel**:
- 🎯 **Full Body** for the exact target function/class.
- 📦 **Inlined Types & Enums**: Extracts definitions of all referenced DTOs and interfaces.
- ✂️ **Signature-Only Stubs**: Strips 100% of the body of called dependencies, leaving only clean signatures.

**Result:** **80–90% token reduction** with 100% semantic and type fidelity.

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

### 1. Slice a function directly to your clipboard
```bash
ctxcut slice ./src/orders.ts:processRefund --clip
```

### 2. Output
````markdown
### Context Slice: `processRefund`
*Language: `typescript` | Lines: `24` (was `340`) | Tokens: `420` (was `3400`) | Savings: `87.6%`*

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

#### 3. External Dependencies & Signatures (Body Stripped)
```typescript
findById(id: string): Promise<Order | null>;
markRefunded(id: string, txId: string): Promise<RefundResult>;
refund(params: { chargeId: string; amount: number }): Promise<{ id: string; status: string }>;
```
````

---

## 🛠️ CLI Commands

| Command | Description |
| :--- | :--- |
| `ctxcut slice <path:symbol> [--clip] [-o file]` | Extract contextual slice for target symbol(s) |
| `ctxcut slice <path:sym1,sym2>` | Batch-slice multiple symbols at once |
| `ctxcut diff [--staged] [--clip]` | Slice only functions modified in git diff |
| `ctxcut stats <path>` | Calculate token savings for repository or file |
| `ctxcut metrics` | View lifetime token savings dashboard and ROI analytics |
| `ctxcut route <METHOD> <PATH>` | Extract web route handler and DTO schemas |
| `ctxcut setup-mcp [--ide <ide>]` | Auto-configure MCP server in your IDE |
| `ctxcut mcp [--log-file <path>]` | Launch MCP server over STDIO |

---

## 🤖 MCP Server Integration

### Automatic Setup (Recommended)

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

### Manual Setup

Add to your IDE's `mcp_config.json`:

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

### MCP Tools

| Tool | Description |
| :--- | :--- |
| `get_symbol_slice` | Extract AST slice for a specific function, method, or class |
| `get_diff_slice` | Extract slices for all functions modified in git diff |
| `analyze_token_stats` | Calculate token savings and context optimization metrics |

---

## 📊 Token Savings Telemetry

Every slice operation records metrics to `~/.ctxcut/metrics.jsonl`. View your cumulative savings:

```bash
ctxcut metrics
```

```text
📊 ctxcut Lifetime Token Savings
======================================================
Total Requests:       412
Total Tokens Saved:   2,180,000
Estimated Cost Saved: $6.54
Average Compression:  86.4%
======================================================

Language Breakdown:
  TypeScript:  182 requests | 920,000 tokens saved
  Python:      104 requests | 580,000 tokens saved
  Go:           78 requests | 410,000 tokens saved
  Rust:         48 requests | 270,000 tokens saved
```

---

## 🌐 Supported Languages

| Language | AST Parser | Features |
| :--- | :--- | :--- |
| **TypeScript / TSX** | tree-sitter-typescript | Generics, barrel re-exports, decorators |
| **JavaScript / JSX** | tree-sitter-javascript | CommonJS `require()`, ES6 imports |
| **Python** | tree-sitter-python | PEP 695 generics, Pydantic models, async/await, decorators |
| **Go** | tree-sitter-go | Pointer/value receivers, sibling package files, generics |
| **Rust** | tree-sitter-rust | `impl` blocks, traits, lifetimes, `where` clauses |

---

## 🏗️ Architecture

```
ctxcut/
├── crates/
│   ├── ctxcut_core/     # AST parsing engine, type resolver, formatter, telemetry
│   ├── ctxcut_cli/      # CLI interface (clap), diff, stats, metrics, setup-mcp
│   └── ctxcut_mcp/      # JSON-RPC 2.0 STDIO MCP server with file logging
├── tests/
│   ├── tier1_features/  # Core language & feature parity tests
│   ├── tier2_boundaries/# Edge cases, syntax errors, unicode
│   ├── tier3_cross_feature/ # MCP chaining, IDE setup, clipboard
│   └── tier4_real_world/# Microservice workload simulations
├── install.sh           # Linux/macOS one-line installer
├── install.ps1          # Windows PowerShell one-line installer
└── .github/workflows/release.yml  # CI/CD multi-platform release pipeline
```

---

## 🧪 Test Coverage

| Suite | Tests | Status |
| :--- | :--- | :--- |
| Tier 1 — Core Features | 88 | ✅ 100% |
| Tier 2 — Boundaries & Edge Cases | 91 | ✅ 100% |
| Tier 3 — Cross-Feature & MCP | 56 | ✅ 100% |
| Tier 4 — Real-World Workloads | 36 | ✅ 100% |
| Adversarial & Stress Suites | 73 | ✅ 100% |
| Unit Tests & Telemetry | 84 | ✅ 100% |
| **Total** | **428** | **✅ 100%** |

---

## 📄 License

MIT © [widlily-corp](https://github.com/widlily-corp)
