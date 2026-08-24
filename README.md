# ⚡ ctxcut

> **AST-accurate contextual code slicer, surgical patcher, test context generator, persistent indexer, query engine & impact tracer for LLMs & AI coding agents. Zero token bloat.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Model Context Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io)
[![Tests Passing](https://img.shields.io/badge/Tests-705%2B%20Passing-brightgreen.svg)]()
[![Version](https://img.shields.io/badge/Version-2.0.0-purple.svg)]()

---

## 🎯 The Problem: Context Obesity in LLMs & AI Agents

When feeding source code to modern LLMs (Claude 3.5 Sonnet, GPT-4o, Gemini 1.5 Pro, DeepSeek V3), developers and AI coding agents face painful compromises:

1. **Full-file dumping (Repomix / gitingest):** Ingests thousands of lines of unrelated helper code, imports, and boilerplate. This burns token budgets, triggers *Lost-in-the-Middle* reasoning degradation, and causes MCP timeouts on large repositories.
2. **Naive RAG / Text Splitting:** Chunks code every 500 characters, breaking functions across boundaries, dropping parameter types, and destroying caller contracts.
3. **Manual Multi-Step Querying:** Agents spend dozens of tool calls chasing down imported interfaces, DTOs, enums, serializers, and external dependencies across multiple files.
4. **Fragile String Replacements:** LLM-generated line edits fail due to minor whitespace, comment, or indentation mismatches.
5. **Opaque Multi-Hop Call Chains:** Agents cannot trace execution paths from API routes down through service layers and database queries without reading entire repositories.

---

## 💡 The Solution: `ctxcut v2.0` (The Titan Core Architecture)

`ctxcut` is an **AST-accurate, thread-safe surgical engine** engineered in Rust for agentic software engineering:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SOURCE CODE / REPOSITORY                          │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                ┌──────────────────────▼──────────────────────┐
                │ 1. Smart Traversal & Persistent Index       │
                │    (.gitignore, SQLite WAL .ctxcut/index.db)│
                └──────────────────────┬──────────────────────┘
                                       │
        ┌──────────────────────────────┼──────────────────────────────┐
        │                              │                              │
┌───────▼────────────────┐   ┌─────────▼──────────────┐   ┌───────────▼────────────┐
│ 2. Deep Graph & Flow   │   │ 3. Polyglot & SFC Lang │   │ 4. ORM & Schema Stitch │
│  • Upstream `callers`  │   │  • TS, JS, Python, Go  │   │  • Prisma models & DDL │
│  • `trace` execution   │   │  • Rust, C/C++, C#/.NET│   │  • Drizzle & TypeORM   │
│  • Implementor Hoist   │   │  • Java, Kotlin        │   │  • SQL Migrations DDL  │
│  • Multi-symbol batch  │   │  • Vue, Svelte, Astro  │   │  • Proto & GraphQL SDL │
└───────┬────────────────┘   └─────────┬──────────────┘   └───────────┬────────────┘
        │                              │                              │
        └──────────────────────────────┼──────────────────────────────┘
                                       │
                ┌──────────────────────▼──────────────────────┐
                │ 5. Adaptive Token Budgeting                 │
                │    (5-level progressive semantic degradation│
                └──────────────────────┬──────────────────────┘
                                       │
        ┌──────────────────────────────┼──────────────────────────────┐
        │                              │                              │
┌───────▼────────────────┐   ┌─────────▼──────────────┐   ┌───────────▼────────────┐
│ 6. Verification Guard  │   │ 7. Semantic AST Diff   │   │ 8. Structural Query    │
│  • AST Syntax check    │   │  • Signature/type delta│   │  • Tree-sitter S-expr  │
│  • Compiler dry-run    │   │  • Token ROI metrics   │   │  • AST Presets         │
│  • RAII auto-rollback  │   │  • Refactor & Rename   │   │  • Interactive TUI     │
└───────┬────────────────┘   └─────────┬──────────────┘   └───────────┬────────────┘
        │                              │                              │
        └──────────────────────────────┴──────────────────────────────┘
                                       │
         ┌─────────────────────────────▼─────────────────────────────┐
         │              DELIVERY INTERFACES & PLATFORMS              │
         │  • Unified CLI (`slice`, `callers`, `trace`, `query`, ...)│
         │  • Model Context Protocol (STDIO JSON-RPC 2.0 Server)     │
         │  • Interactive Ratatui Terminal UI Dashboard (`tui`)      │
         │  • IDE Auto-Config (Antigravity, Cursor, Claude, VSCode)  │
         └───────────────────────────────────────────────────────────┘
```

- 🚀 **1. Persistent SQLite Cache (`.ctxcut/index.db`):** Sub-5ms symbol queries with SHA256/mtime cache invalidation and WAL concurrency.
- 🔍 **2. Reverse Impact Analysis (`ctxcut callers`):** Scans the entire workspace to identify all upstream call-sites and consumers of a target function or method.
- ⚡ **3. Execution Path Tracer (`ctxcut trace`):** Automatically traces multi-hop execution flows from entry points (routes, CLI commands) down to service and database layers within a strict token budget.
- 🧱 **4. Concrete Implementor Hoisting:** Discovers and hoists concrete struct/class implementations for Rust traits (`impl Trait`), Go interfaces (structural duck typing), TypeScript (`implements`), and Python (`Protocol`).
- 🌐 **5. 10 Core Languages + SFCs:** Full AST support for TypeScript/JS, Python, Go, Rust, C/C++, C#/.NET, Java, Kotlin, and Single File Components (**Vue `<script setup>`**, **Svelte**, **Astro**).
- 🗄️ **6. ORM & Database Schema Stitching:** Automatically identifies ORM calls and stitches models from **Prisma**, **Drizzle**, **TypeORM**, **SQL migration DDLs** (`migrations/*.sql`), **Protocol Buffers** (`.proto`), and **GraphQL SDL**.
- 🛡️ **7. Verification Guard & Auto-Rollback (`ctxcut verify-patch`):** Executes in-memory typecheck dry-runs (`cargo check`, `tsc --noEmit`, `mypy`) with RAII auto-rollback on error.
- 🔬 **8. Semantic AST Diffing (`ctxcut semantic-diff`):** Structural AST diffs highlighting changes in signatures, fields, and types with token ROI measurements.
- ✏️ **9. AST Refactoring & Rename (`ctxcut refactor rename`):** AST-accurate multi-file symbol renaming across dependencies without touching unrelated substring matches.
- 🎯 **10. Structural AST Query Engine (`ctxcut query`):** Structural pattern matching via Tree-sitter S-expressions or built-in presets (`functions`, `types`, `routes`, `calls`, `classes`).
- 📊 **11. Interactive Terminal UI Dashboard (`ctxcut tui`):** High-density Ratatui dashboard for AST context inspection, telemetry visualizers, and token ROI KPIs.

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

### 1. Reverse Impact Slicing (`callers`) & Execution Tracing (`trace`)

```bash
# Find all upstream consumers and callers of a function
ctxcut callers ./src/services/auth.ts:validateToken --budget 1500

# Trace execution pathway from an API endpoint down to database sinks
ctxcut trace "POST /api/v1/orders" --budget 2000
```

---

### 2. Single & Multi-Symbol Slicing with Implementor Hoisting

```bash
# Slice a single function with hoisted cross-file types and 1,500 token budget
ctxcut slice ./src/services/order.ts:processRefund --depth 1 --budget 1500 --clip

# Slice multiple functions in one file with unified type deduplication
ctxcut slice ./src/services/order.ts:processRefund,cancelOrder --budget 2000
```

#### Output Example:
````markdown
### Context Slice: `processRefund`
*Language: `typescript` | Lines: `32` (was `520`) | Tokens: `490` (was `4,600`) | Savings: `89.3%`*

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

#### 2. Hoisted Types & ORM Schema Contracts
```typescript
export enum OrderStatus { PENDING = 'PENDING', COMPLETED = 'COMPLETED', REFUNDED = 'REFUNDED' }
export type RefundReason = 'FRAUD' | 'CUSTOMER_REQUEST' | 'DEFECTIVE';
export interface RefundResult { success: boolean; transactionId: string; }

// Stitched from schema.prisma
model Order {
  id          String   @id @default(uuid())
  chargeId    String
  totalAmount Decimal
  status      OrderStatus
}
```

#### 3. Concrete Implementors
```typescript
// from src/gateways/stripe.ts
export class StripePaymentGateway implements PaymentGateway {
  async refund(params: { chargeId: string; amount: number }): Promise<{ id: string; status: string }>;
}
```

#### 4. Cross-File Dependencies & Signatures (Body Stripped)
```typescript
// from src/repositories/orderRepo.ts
findById(id: string): Promise<Order | null>;
markRefunded(id: string, txId: string): Promise<RefundResult>;
```
````

---

### 3. Structural AST Query & Presets

Search across codebases using built-in presets or custom Tree-sitter query expressions:

```bash
# Query all exported functions in Rust files
ctxcut query --preset functions --lang rust --limit 10

# Query API route definitions across repository
ctxcut query --preset routes

# Custom Tree-sitter pattern matching
ctxcut query "(function_declaration name: (identifier) @fn)" --lang typescript
```

---

### 4. Verification Guard & Auto-Rollback Patching

Apply code changes safely with in-memory compiler checks:

```bash
# Dry-run patch with automatic typecheck verification
ctxcut verify-patch ./src/services/order.ts:processRefund --code "export async function processRefund(id: string) {}" --typecheck-cmd "npm run typecheck" --dry-run

# Surgical AST patch
ctxcut patch ./src/services/order.ts:processRefund --file ./new_refund.ts
```

---

### 5. AST-Accurate Symbol Renaming

Rename symbols across files without touching false-positive string matches:

```bash
# Preview workspace-wide symbol renaming
ctxcut refactor rename ./src/services/order.ts:processRefund --to executeRefund --dry-run
```

---

### 6. Interactive Terminal UI (TUI) Dashboard

```bash
# Launch interactive context studio and telemetry visualizer
ctxcut tui
```

---

## 🛠️ Complete CLI Subcommand Reference

The `ctxcut` CLI provides 20 dedicated subcommands:

| Subcommand | Description | Key Arguments & Flags | Example |
| :--- | :--- | :--- | :--- |
| `slice` | Extracts minimal AST context slice for target symbol(s) | `<target>` (`path:symbol` or `path:sym1,sym2`)<br>`--budget <N>`: Token budget limit<br>`--depth <N>`: Type hoisting depth (default: 1)<br>`--no-types`: Disable type hoisting<br>`--no-calls`: Disable signature stripping<br>`--clip`: Copy to clipboard<br>`-o, --output <PATH>`: Save to file<br>`--format <markdown\|json>` | `ctxcut slice src/calc.ts:add,multiply --budget 1000` |
| `callers` | Upstream reverse caller impact analysis across workspace | `<target>` (`symbol` or `path:symbol`)<br>`--budget <N>`: Token budget limit<br>`--limit <N>`: Maximum callers to return<br>`--clip`: Copy to clipboard<br>`--format <markdown\|json>` | `ctxcut callers AuthService.validateToken` |
| `trace` | End-to-end execution flow tracer from entry to database | `<entry>` (`POST /api/v1/orders`, `main`)<br>`--budget <N>`: Token budget (default: 1500)<br>`--depth <N>`: Max call hops (default: 8)<br>`--clip`: Copy to clipboard<br>`--format <markdown\|json>` | `ctxcut trace "POST /api/v1/checkout"` |
| `query` | Searches workspace using Tree-sitter queries or presets | `[<pattern>]`: Tree-sitter S-expression<br>`--preset <functions\|types\|routes\|calls\|classes>`<br>`--lang <LANG>`: Language filter<br>`--limit <N>`: Max results | `ctxcut query --preset routes`<br>`ctxcut query --preset functions --lang rust` |
| `verify-patch` | Verifies patch using AST validation & typecheckers with auto-rollback | `<target>` (`path:symbol`)<br>`-c, --code <CODE>`: Replacement code<br>`-f, --file <PATH>`: Replacement file<br>`--typecheck-cmd <CMD>`: Custom checker<br>`--dry-run`: Preview changes | `ctxcut verify-patch src/calc.rs:add --code "..." --dry-run` |
| `semantic-diff` | Token-efficient structural AST diff with ROI metrics | `[<path>]`: Root path<br>`--staged`: Inspect staged changes only<br>`--budget <N>`: Budget limit<br>`--format <markdown\|json>` | `ctxcut semantic-diff --staged` |
| `refactor` | AST-guided multi-file symbol refactoring & renaming | `rename <target> --to <NEW_NAME>`<br>`--dry-run`: Preview renames | `ctxcut refactor rename UserService:findById --to getUserById` |
| `index` | Manages persistent SQLite index (`.ctxcut/index.db`) | `--clear`: Rebuild index from scratch<br>`--stats`: Display index statistics | `ctxcut index`<br>`ctxcut index --stats` |
| `tui` / `dashboard` | Interactive Terminal UI Context Studio & Telemetry | `--refresh <MS>`: Polling interval | `ctxcut tui` |
| `diff` | Extracts AST slices for all functions modified in Git diff | `--staged`: Staged changes only<br>`--budget <N>`: Budget limit<br>`--clip`: Copy to clipboard | `ctxcut diff --staged` |
| `route` | Resolves web framework route handler to controller slice | `<method>` (GET, POST, PUT, DELETE)<br>`<path>` (e.g. `/api/v1/users`)<br>`--budget <N>`: Budget limit | `ctxcut route POST /auth/login` |
| `patch` | Surgically replaces a function or class in source code | `<target>` (`path:symbol`)<br>`-c, --code <CODE>`: Replacement code<br>`-f, --file <PATH>`: Replacement file<br>`--dry-run`: Preview unified diff | `ctxcut patch src/app.ts:init --code "..." --dry-run` |
| `test-context` | Generates isolated test bundle with mock scaffolding | `<target>` (`path:symbol`)<br>`--framework <vitest\|jest\|pytest\|cargo\|gotest>`<br>`--budget <N>`: Budget limit | `ctxcut test-context src/math.rs:sqrt --framework cargo` |
| `stats` | Analyzes repository/file token savings and statistics | `[<path>]`: File or directory path<br>`-f, --fast`: Fast shallow estimation<br>`--history`: View lifetime telemetry | `ctxcut stats . --fast` |
| `metrics` | Displays lifetime token savings & ROI dashboard | `--format <text\|json>` | `ctxcut metrics` |
| `overview` | High-level workspace symbol indexing & architectural outline | `[<path>]`: Workspace root<br>`--depth <N>`: Directory depth<br>`--budget <N>`: Budget limit | `ctxcut overview . --depth 2` |
| `setup-mcp` | Automatically configures IDEs to use ctxcut as MCP server | `--ide <antigravity\|claude\|cursor\|vscode\|all>`<br>`--workspace`: Project config | `ctxcut setup-mcp --ide antigravity` |
| `init` | Alias for `setup-mcp` to initialize ctxcut in IDE config | Same options as `setup-mcp` | `ctxcut init --ide cursor` |
| `upgrade` | Check for updates and self-upgrade ctxcut | `--check`: Check without installing | `ctxcut upgrade` |
| `mcp` | Launches Model Context Protocol (MCP) server over STDIO | `--log-file <PATH>`: JSONL logging | `ctxcut mcp` |

---

## 🤖 Model Context Protocol (MCP) Integration

### Automated IDE Configuration

```bash
# Configure all detected IDEs in one command
ctxcut setup-mcp
```

### Complete MCP Tools Suite (10 Tools)

| Tool Name | Key Parameters | Description |
| :--- | :--- | :--- |
| `get_symbol_slice` | `path` (req), `symbol` (req, single/batch), `depth` (opt), `budget` (opt), `no_types` (opt), `no_calls` (opt) | Extracts AST slice with hoisted types, implementors, stitched schemas, and call signatures. |
| `get_impact_slice` | `symbol` (req), `path` (opt), `root_dir` (opt), `budget` (opt), `limit` (opt) | Reverse impact analysis tracing all upstream call sites of a symbol across the workspace. |
| `get_trace_slice` | `entry` (req), `root_dir` (opt), `depth` (opt, def 8), `budget` (opt, def 1500) | End-to-end execution pathway tracing from entry points down to database and service sinks. |
| `get_diff_slice` | `path` (opt), `staged` (opt), `budget` (opt) | Extracts contextual AST slices for all functions modified in git working tree or staged changes. |
| `get_workspace_overview` | `path` (opt), `depth` (opt), `budget` (opt) | High-speed symbol outline of workspace files without reading full bodies (90–95% savings). |
| `get_route_slice` | `method` (req), `path` (req), `root_dir` (opt), `budget` (opt) | Resolves web API route handler, controllers, DTO schemas, and middleware chains. |
| `get_test_context` | `path` (req), `symbol` (req), `framework` (opt), `budget` (opt) | Generates isolated test bundle with parameter/return types, mock signatures, and fixtures. |
| `patch_symbol` | `path` (req), `symbol` (req), `code` (req), `dry_run` (opt) | Surgically replaces a function/class in source code with AST boundary alignment & syntax checks. |
| `analyze_token_stats` | `path` (req), `fast` (opt) | Calculates repository or file token savings and optimization statistics with `.gitignore` compliance. |
| `get_metrics` | `format` (opt), `clear` (opt) | Inspects cumulative lifetime token reduction telemetry, dollar ROI analytics, and language distributions. |

---

## 🌐 Supported Language Ecosystems & SFCs

| Language / Framework | Extensions | AST Grammar | Specialized Capabilities |
| :--- | :--- | :--- | :--- |
| **TypeScript / TSX** | `.ts`, `.tsx`, `.mts`, `.cts` | `tree-sitter-typescript` | Generics, barrel re-exports, decorators, JSX branch collapsing, type aliases |
| **JavaScript / JSX** | `.js`, `.jsx`, `.mjs`, `.cjs` | `tree-sitter-javascript` | CommonJS `require()`, ES6 module imports, JSX stubs, prototype methods |
| **Python** | `.py`, `.pyi` | `tree-sitter-python` | PEP 695 generics, Pydantic v1/v2, Django models, `Protocol` inheritance |
| **Go** | `.go` | `tree-sitter-go` | Pointer/value receivers, structural duck-type implementors, sibling packages |
| **Rust** | `.rs` | `tree-sitter-rust` | `impl Trait for Struct`, associated types, lifetimes, `where` clauses |
| **C / C++** | `.c`, `.h`, `.cpp`, `.hpp`, `.cc` | `tree-sitter-c`, `cpp` | `template<...>`, struct/class methods, header inclusions, macro directive stripping |
| **C# / .NET** | `.cs` | `tree-sitter-c-sharp` | ASP.NET Core `[ApiController]`, records, structs, interfaces, namespace hoisting |
| **Java** | `.java` | `tree-sitter-java` | Spring `@RestController`, JPA entities, wildcard generics, interface implementations |
| **Kotlin** | `.kt`, `.kts` | `tree-sitter-kotlin` | Extension functions, data classes, reified type parameters, companion objects |
| **Vue SFC** | `.vue` | `sfc/vue` parser | `<script setup>` & `<script>` isolation, props extraction, template/style compaction |
| **Svelte SFC** | `.svelte` | `sfc/svelte` parser | Svelte 5 runes (`$props`), `<script>` block isolation, reactive state hoisting |
| **Astro SFC** | `.astro` | `sfc/astro` parser | Frontmatter fence `---` component script extraction, client directives |

---

## 🗄️ ORM & Database Schema Stitching

| Provider | Schema Files | Automatic Trigger & Hoisting Behavior |
| :--- | :--- | :--- |
| **Prisma** | `schema.prisma` | Detected upon `prisma.<model>.<method>` calls; extracts `model ModelName { ... }` and relations |
| **Drizzle ORM** | `schema.ts`, `schema.js` | Detected upon `db.select().from(table)`; extracts `pgTable`, `mysqlTable`, `sqliteTable` |
| **TypeORM** | `*.entity.ts`, `*Entity.ts` | Detected upon `@Entity()` repositories; extracts entity classes, columns, and relations |
| **SQL Migrations** | `migrations/*.sql`, `schema.sql` | Detected upon `sqlx::query!`, `db.query("SELECT ...")`; extracts `CREATE TABLE <name>` DDL |
| **Protocol Buffers** | `*.proto` | Detected upon gRPC service handlers; extracts `message` and `service` RPC declarations |
| **GraphQL SDL** | `*.graphql`, `*.gql` | Detected upon GraphQL resolvers/queries; extracts `type`, `input`, `query`, `mutation` |

---

## 🧪 Quality, Performance & Test Verification

`ctxcut v2.0` is verified against an extensive 5-tier test matrix:

| Test Suite | Scope & Coverage | Tests | Status |
| :--- | :--- | :---: | :--- |
| **Tier 1** | Feature Coverage across F1..F15 (Callers, Trace, SFCs, ORMs, Verify, Index, Query) | **298** | ✅ 100% Pass |
| **Tier 2** | Boundary & Corner Cases (Cycles, empty/large files, syntax fault recovery, Unicode paths) | **250** | ✅ 100% Pass |
| **Tier 3** | Pairwise Cross-Feature Combinations (Callers+Trace, SFC+Patch, ORM+Diff, Index+Query) | **74** | ✅ 100% Pass |
| **Tier 4** | Real-World Polyglot Microservices (E-Commerce, Auth, Billing, Inventory, Trace workflows) | **63** | ✅ 100% Pass |
| **Tier 5** | Telemetry, Ratatui Dashboard, IDE Setup, Adversarial Stress & Concurrency | **20** | ✅ 100% Pass |
| **Unit & Benches** | Language Adapters, AST Patcher, Schema Stitchers, Criterion benchmarks | **35+** | ✅ 100% Pass |
| **Total** | **705+ Tests Verified** | **705+** | **✅ 100% Pass** |

---

## 📄 License

MIT © [widlily-corp](https://github.com/widlily-corp)
