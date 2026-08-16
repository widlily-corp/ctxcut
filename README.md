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
- ✂️ **Signature-Only Stubs**: Strips 100% of the body of called dependencies, leaving only clean signatures (`body stripping`).

**Result:** **80–90% token reduction** with 100% semantic and type fidelity.

---

## 🚀 Quick Start

### 1. Slice a function directly to your clipboard
```bash
ctxcut slice ./src/orders.ts:processRefund --clip
```

### 2. Output
```markdown
# Context Slice: `processRefund` (from `src/services/orders.ts:84`)

### 1. Target Function (Full Body)
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

### 2. Required Types & Enums (Extracted)
```typescript
// from src/types/orders.ts
export enum OrderStatus { PENDING = 'PENDING', COMPLETED = 'COMPLETED', REFUNDED = 'REFUNDED' }
export type RefundReason = 'FRAUD' | 'CUSTOMER_REQUEST' | 'DEFECTIVE';
export interface RefundResult { success: boolean; transactionId: string; }
```

### 3. External Dependencies (Signatures Only)
```typescript
// from src/repos/orderRepo.ts
findById(id: string): Promise<Order | null>;
markRefunded(id: string, txId: string): Promise<RefundResult>;

// from src/gateways/payment.ts
refund(params: { chargeId: string; amount: number }): Promise<{ id: string; status: string }>;
```
```

---

## 🤖 MCP Server Integration (Cursor / Claude / Antigravity)

Add `ctxcut` to your `mcpServers` configuration:

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

## 🛠️ CLI Commands

| Command | Description |
| :--- | :--- |
| `ctxcut slice <path:symbol> [--clip]` | Extract contextual slice for target symbol(s) |
| `ctxcut diff [--staged] [--clip]` | Slice only functions modified in git diff |
| `ctxcut stats <path>` | Calculate token savings for repository |
| `ctxcut route <METHOD> <PATH>` | Extract web route handler and DTO schemas |
| `ctxcut mcp` | Launch Model Context Protocol server over STDIO |

---

## 📄 License

MIT © [widlily-corp](https://github.com/widlily-corp)
