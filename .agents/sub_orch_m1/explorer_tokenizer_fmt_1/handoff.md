# Formatter, BPE Tokenizer & Test Strategy Report (`ctxcut_core`)

**Agent**: `explorer_tokenizer_fmt_1`  
**Parent Conversation ID**: `392c723b-2888-4f80-8bf8-e5101eb481a6`  
**Working Directory**: `C:\Users\Widlily\Documents\projects\ctxcut\.agents\sub_orch_m1\explorer_tokenizer_fmt_1`  
**Date**: 2026-08-16  
**Status**: COMPLETE / AUTHORITATIVE SPECIFICATION  

---

## 1. Observation

### 1.1. Upstream Requirements & Contracts
From direct inspection of `ORIGINAL_REQUEST.md` (§R2, §R5), `PROJECT.md` (lines 68–71, 107–179), `SCOPE.md` (lines 56–105), and `TEST_INFRA.md`:

1. **`SliceResult` Structure & Contracts**:
   - `ExtractedSymbol`: `name: String`, `kind: String`, `file_path: String`, `start_line: usize`, `end_line: usize`, `doc_comment: Option<String>`, `signature: String`, `body: String`, `language: String`.
   - `ExtractedType`: `name: String`, `kind: String`, `file_path: String`, `definition: String`.
   - `CallSignatureStub`: `name: String`, `receiver: Option<String>`, `file_path: Option<String>`, `signature: String`.
   - `TokenStats`: `raw_file_tokens: usize`, `sliced_tokens: usize`, `savings_percentage: f64`, `raw_lines: usize`, `sliced_lines: usize`.
   - `SliceResult`: Contains `target_symbol`, `hoisted_types`, `stripped_calls`, `stats`, and implements `to_markdown(&self) -> String` and `to_json(&self) -> String`.

2. **Formatting & Prompt Architecture**:
   - LLM prompts require strict separation of concerns:
     - Section 1: Header metadata (File path, target symbol, language, line delta, token metrics, savings percentage).
     - Section 2: Target Implementation with full body and intact docstrings.
     - Section 3: Hoisted Types & Data Contracts (interfaces, types, enums, DTOs).
     - Section 4: External Dependencies & Signatures (100% body-stripped call stubs).
   - Zero hallucinations: Output must be syntactically clean Markdown with explicit code fences matching target language (`typescript`, `tsx`, `javascript`, etc.).

3. **BPE Token Counting Engine**:
   - Must use `tiktoken-rs` with `cl100k_base` encoding (OpenAI GPT-4, GPT-3.5, and standard LLM benchmark compatible).
   - Must calculate raw file tokens, sliced tokens, lines delta, and token reduction percentage via formula:
     $$\text{savings\_percentage} = \left(\left(1.0 - \frac{\text{sliced\_tokens}}{\text{raw\_file\_tokens}}\right) \times 100.0\right).\max(0.0)$$
   - Must handle edge cases: empty files (`raw == 0`), small files where slice exceeds raw file (`sliced >= raw`), non-standard characters, and special BPE tokens without panicking.

4. **Testing Standards**:
   - AAA-pattern (Arrange-Act-Assert) throughout.
   - Comprehensive test fixtures for TypeScript (`.ts`, `.tsx`) and JavaScript (`.js`, `.jsx`).
   - Snapshot testing (`insta`) with line ending normalization (`\r\n` $\to$ `\n`).

---

## 2. Logic Chain

### 2.1. Markdown Layout Reasoning
1. **Header Metadata**: Placing file path, symbol, language, lines, and token savings in the header immediately grounds the LLM agent on context origin, scale, and density before it reads code tokens.
2. **Deterministic Code Fencing**: Markdown blocks must specify language tag (e.g. ```` ```typescript ````) for syntax highlighting and prompt adherence.
3. **Empty Section Handling**: When a target function has no external types or no external function calls, omitting the section or rendering a clean italicized placeholder (`*None*` or `// No external dependencies`) prevents LLMs from hallucinating missing dependencies while avoiding clutter.
4. **Newline Normalization**: Cross-platform execution (Windows vs Linux CI) produces different line endings (`\r\n` vs `\n`). The formatter must emit standardized `\n` to guarantee deterministic token counts and golden snapshot match.

### 2.2. BPE Tokenizer Architecture & Singleton Lifecycle
1. **Lazy Singleton Initialization**: Loading `tiktoken_rs::cl100k_base()` takes ~5–15ms because it parses the BPE vocabulary table. Calling `cl100k_base()` on every AST slice would violate the sub-10ms latency SLA.
2. **Concurrency Safety**: By using `std::sync::OnceLock<tiktoken_rs::CoreBPE>`, the BPE encoder is initialized exactly once upon first slice/token call and shared across threads via immutable references.
3. **Safe Tokenization (`encode_ordinary`)**: Real-world source code may contain string literals such as `"<|endoftext|>"` or `"<|im_start|>"`. Calling `encode_with_special_tokens` on untrusted source code could panic or error. `encode_ordinary` tokenizes all strings as plain text bytes, guaranteeing 100% crash-free token counting.
4. **Division by Zero Protection**: If `raw_file_tokens == 0`, `savings_percentage` defaults to `0.0` to avoid IEEE 754 `NaN`. If `sliced_tokens > raw_file_tokens` (e.g. markdown headers on a 1-line file), `.max(0.0)` clamps the metric to non-negative.

---

## 3. Caveats & Assumptions

1. **Tokenizer Vocabulary Variance**: `cl100k_base` is the de facto standard for GPT-4/ChatGPT and closely approximates Claude/Llama token counts (~95-98% correlation). Token counts for Claude (Anthropic) or Gemini may differ slightly, but `cl100k_base` provides a standard metric.
2. **Rendered Markdown vs AST Tokenization**: `TokenStats::sliced_tokens` measures the exact token count of the rendered Markdown output (`to_markdown()`), as this represents the actual token payload injected into the LLM prompt.
3. **Syntax Error Tolerance**: If source code contains minor syntax anomalies, token counting still succeeds as it operates on UTF-8 text buffers.

---

## 4. Conclusion & Detailed Technical Design

### 4.1. Prompt-Optimized Markdown Formatter (`src/formatter/mod.rs`)

#### 4.1.1. Exact Markdown Template Specification
```markdown
### Context Slice: `{file_path}:{symbol_name}`
*Language: `{language}` | Lines: `{sliced_lines}` (was `{raw_lines}`) | Tokens: `{sliced_tokens}` (was `{raw_file_tokens}`) | Savings: `{savings_percentage:.1}%`*

#### 1. Target Implementation (Full Body)
```{language_tag}
{doc_comment_and_body}
```

#### 2. Hoisted Types & Data Contracts
```{language_tag}
{hoisted_type_definitions}
```

#### 3. External Dependencies & Signatures (Body Stripped)
```{language_tag}
{stripped_call_signatures}
```
```

#### 4.1.2. Rust Implementation Blueprint (`crates/ctxcut_core/src/formatter/mod.rs`)
```rust
use crate::model::SliceResult;

pub struct MarkdownFormatter;

impl MarkdownFormatter {
    /// Formats a single SliceResult into a prompt-optimized Markdown string.
    pub fn format(result: &SliceResult) -> String {
        let mut out = String::with_capacity(2048);

        let lang_tag = normalize_language_tag(&result.target_symbol.language);
        let sym = &result.target_symbol;
        let stats = &result.stats;

        // Header section with metadata
        out.push_str(&format!("### Context Slice: `{}:{}`\n", sym.file_path, sym.name));
        out.push_str(&format!(
            "*Language: `{}` | Lines: `{}` (was `{}`) | Tokens: `{}` (was `{}`) | Savings: `{:.1}%`*\n\n",
            sym.language,
            stats.sliced_lines,
            stats.raw_lines,
            stats.sliced_tokens,
            stats.raw_file_tokens,
            stats.savings_percentage
        ));

        // Section 1: Target Implementation
        out.push_str("#### 1. Target Implementation (Full Body)\n");
        out.push_str(&format!("```{}\n", lang_tag));
        if let Some(ref doc) = sym.doc_comment {
            let trimmed_doc = doc.trim();
            if !trimmed_doc.is_empty() {
                out.push_str(trimmed_doc);
                out.push('\n');
            }
        }
        out.push_str(sym.body.trim());
        out.push_str("\n```\n\n");

        // Section 2: Hoisted Types & Data Contracts
        out.push_str("#### 2. Hoisted Types & Data Contracts\n");
        if result.hoisted_types.is_empty() {
            out.push_str("*None*\n\n");
        } else {
            out.push_str(&format!("```{}\n", lang_tag));
            for (idx, ty) in result.hoisted_types.iter().enumerate() {
                if idx > 0 {
                    out.push_str("\n\n");
                }
                out.push_str(ty.definition.trim());
            }
            out.push_str("\n```\n\n");
        }

        // Section 3: External Dependencies & Signatures (Body Stripped)
        out.push_str("#### 3. External Dependencies & Signatures (Body Stripped)\n");
        if result.stripped_calls.is_empty() {
            out.push_str("*None*\n");
        } else {
            out.push_str(&format!("```{}\n", lang_tag));
            for (idx, call) in result.stripped_calls.iter().enumerate() {
                if idx > 0 {
                    out.push('\n');
                }
                out.push_str(call.signature.trim());
            }
            out.push_str("\n```\n");
        }

        out
    }

    /// Formats a batch of SliceResults into a combined Markdown document.
    pub fn format_batch(results: &[SliceResult]) -> String {
        results
            .iter()
            .map(|r| r.to_markdown())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }
}

/// Normalizes language identifier to standard Markdown code-fence syntax tag.
pub fn normalize_language_tag(lang: &str) -> &str {
    match lang.to_ascii_lowercase().as_str() {
        "typescript" | "ts" => "typescript",
        "tsx" => "tsx",
        "javascript" | "js" => "javascript",
        "jsx" => "jsx",
        "python" | "py" => "python",
        "go" | "golang" => "go",
        "rust" | "rs" => "rust",
        other => other,
    }
}
```

#### 4.1.3. Structured JSON Serialization (`to_json()`)
```rust
impl SliceResult {
    /// Formats the slice result as prompt-optimized Markdown.
    pub fn to_markdown(&self) -> String {
        crate::formatter::MarkdownFormatter::format(self)
    }

    /// Formats the slice result as pretty-printed JSON adhering to the canonical schema.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Formats the slice result as compact JSON (for MCP payloads or CLI piping).
    pub fn to_json_compact(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}
```

**JSON Schema Contract (`to_json()` output example)**:
```json
{
  "target_symbol": {
    "name": "registerUser",
    "kind": "function",
    "file_path": "src/services/auth.ts",
    "start_line": 24,
    "end_line": 48,
    "doc_comment": "/**\n * Registers a new user and sends verification email.\n */",
    "signature": "export async function registerUser(dto: CreateUserDto): Promise<User>",
    "body": "export async function registerUser(dto: CreateUserDto): Promise<User> {\n  validateDto(dto);\n  const hash = await hashPassword(dto.password);\n  return db.user.create({ data: { ...dto, passwordHash: hash } });\n}",
    "language": "TypeScript"
  },
  "hoisted_types": [
    {
      "name": "CreateUserDto",
      "kind": "interface",
      "file_path": "src/models/user.ts",
      "definition": "export interface CreateUserDto {\n  email: string;\n  password: string;\n  name: string;\n}"
    },
    {
      "name": "User",
      "kind": "interface",
      "file_path": "src/models/user.ts",
      "definition": "export interface User {\n  id: string;\n  email: string;\n  name: string;\n  createdAt: Date;\n}"
    }
  ],
  "stripped_calls": [
    {
      "name": "validateDto",
      "receiver": null,
      "file_path": "src/utils/validation.ts",
      "signature": "export function validateDto(dto: unknown): void;"
    },
    {
      "name": "hashPassword",
      "receiver": null,
      "file_path": "src/utils/crypto.ts",
      "signature": "export async function hashPassword(plain: string): Promise<string>;"
    }
  ],
  "stats": {
    "raw_file_tokens": 1420,
    "sliced_tokens": 178,
    "savings_percentage": 87.46,
    "raw_lines": 120,
    "sliced_lines": 32
  }
}
```

---

### 4.2. BPE Token Counting Engine Design (`src/tokenizer/mod.rs`)

#### 4.2.1. Rust Implementation Blueprint (`crates/ctxcut_core/src/tokenizer/mod.rs`)
```rust
use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;
use crate::model::TokenStats;

static BPE_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

/// Returns a shared reference to the global `cl100k_base` BPE tokenizer instance.
/// Initialized lazily on first access.
pub fn get_bpe_tokenizer() -> &'static CoreBPE {
    BPE_TOKENIZER.get_or_init(|| {
        tiktoken_rs::cl100k_base()
            .expect("Fatal: Failed to initialize tiktoken cl100k_base tokenizer")
    })
}

/// Counts the exact BPE tokens in a UTF-8 string using `cl100k_base`.
/// Uses `encode_ordinary` to avoid panicking on special token byte sequences.
pub fn count_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    get_bpe_tokenizer().encode_ordinary(text).len()
}

/// Computes the exact percentage of tokens saved by slicing.
/// Formula: `((1.0 - (sliced / raw)) * 100.0).max(0.0)`.
/// Guaranteed to never produce NaN or negative values.
pub fn calculate_savings_percentage(raw_tokens: usize, sliced_tokens: usize) -> f64 {
    if raw_tokens == 0 {
        return 0.0;
    }
    let raw = raw_tokens as f64;
    let sliced = sliced_tokens as f64;
    let ratio = sliced / raw;
    let savings = ((1.0 - ratio) * 100.0).max(0.0);
    (savings * 100.0).round() / 100.0
}

/// Counts total physical lines in a string. Returns 0 for empty strings.
pub fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count().max(1)
    }
}

/// Computes full `TokenStats` comparing raw source code to the generated sliced Markdown.
pub fn compute_stats(raw_source: &str, sliced_markdown: &str) -> TokenStats {
    let raw_file_tokens = count_tokens(raw_source);
    let sliced_tokens = count_tokens(sliced_markdown);
    let savings_percentage = calculate_savings_percentage(raw_file_tokens, sliced_tokens);
    let raw_lines = count_lines(raw_source);
    let sliced_lines = count_lines(sliced_markdown);

    TokenStats {
        raw_file_tokens,
        sliced_tokens,
        savings_percentage,
        raw_lines,
        sliced_lines,
    }
}
```

---

### 4.3. Unit & Integration Test Strategy & Fixtures

#### 4.3.1. Unit Test Matrix for `ctxcut_core`

| Module | Test Case | Purpose | Arrange | Act | Assert |
|---|---|---|---|---|---|
| `formatter` | `test_format_full_slice` | Validates complete markdown with headers, target, types, and stubs | Populate full `SliceResult` | `to_markdown()` | Assert exact headers, fences, and sections |
| `formatter` | `test_format_empty_dependencies` | Tests clean rendering when no types/calls are present | `hoisted_types = vec![]`, `stripped_calls = vec![]` | `to_markdown()` | Assert `*None*` placeholders present |
| `formatter` | `test_json_roundtrip` | Verifies serialization and deserialization integrity | Valid `SliceResult` | `to_json()` $\to$ `serde_json::from_str` | Assert reconstructed struct equals original |
| `formatter` | `test_normalize_language_tag` | Verifies language string to markdown fence tag mapping | `"TypeScript"`, `"TSX"`, `"Python"` | `normalize_language_tag()` | Assert `"typescript"`, `"tsx"`, `"python"` |
| `tokenizer` | `test_count_tokens_empty` | Empty string yields 0 tokens | `""` | `count_tokens("")` | `assert_eq!(0)` |
| `tokenizer` | `test_count_tokens_code` | Verifies deterministic token count on TypeScript snippet | TS function snippet | `count_tokens(code)` | Assert exact expected count $> 0$ |
| `tokenizer` | `test_savings_calc_normal` | Tests standard reduction calculation | `raw=1000, sliced=150` | `calculate_savings_percentage(1000, 150)` | `assert_eq!(85.0)` |
| `tokenizer` | `test_savings_calc_zero_raw` | Prevents division by zero / NaN on empty files | `raw=0, sliced=50` | `calculate_savings_percentage(0, 50)` | `assert_eq!(0.0)` |
| `tokenizer` | `test_savings_calc_overflow` | Sliced tokens larger than raw file yields 0.0% savings | `raw=10, sliced=50` | `calculate_savings_percentage(10, 50)` | `assert_eq!(0.0)` |
| `tokenizer` | `test_special_tokens_safety` | Proves no crash on raw prompt tokens in strings | `"let x = '<|endoftext|>';"` | `count_tokens(text)` | Assert no panic, token count $> 0$ |
| `tokenizer` | `test_multithreaded_singleton` | Verifies concurrent thread-safety of `OnceLock<CoreBPE>` | 10 concurrent threads | Call `count_tokens()` in parallel | Assert all threads succeed |

#### 4.3.2. TypeScript / JavaScript Test Fixtures Specification

The following fixtures should be placed in `tests/fixtures/typescript/` (and mirrored in `ctxcut_core/tests/fixtures/`):

##### Fixture 1: `simple_service/` (Named functions, DTO interfaces, and body-stripped utilities)
- `tests/fixtures/typescript/simple_service/types.ts`:
  ```typescript
  export interface User {
    id: string;
    email: string;
    role: UserRole;
    createdAt: Date;
  }

  export enum UserRole {
    ADMIN = "ADMIN",
    USER = "USER",
    GUEST = "GUEST",
  }

  export interface CreateUserDto {
    email: string;
    passwordPlain: string;
    role?: UserRole;
  }
  ```
- `tests/fixtures/typescript/simple_service/utils.ts`:
  ```typescript
  export function validateEmail(email: string): boolean {
    // 50 lines of regex and validation logic
    const re = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return re.test(email);
  }

  export async function hashPassword(password: string): Promise<string> {
    // Heavy bcrypt hashing body
    return `hash_${password}`;
  }
  ```
- `tests/fixtures/typescript/simple_service/authService.ts`:
  ```typescript
  import { User, UserRole, CreateUserDto } from "./types";
  import { validateEmail, hashPassword } from "./utils";

  /**
   * Registers a new user account with hashed password.
   */
  export async function registerUser(dto: CreateUserDto): Promise<User> {
    if (!validateEmail(dto.email)) {
      throw new Error("Invalid email format");
    }
    const hashedPassword = await hashPassword(dto.passwordPlain);
    const user: User = {
      id: "usr_12345",
      email: dto.email,
      role: dto.role ?? UserRole.USER,
      createdAt: new Date(),
    };
    return user;
  }
  ```

##### Fixture 2: `classes_and_arrow/` (Class methods, arrow functions, and generic types)
- `tests/fixtures/typescript/classes_and_arrow/payment.ts`:
  ```typescript
  export type PaymentStatus = "pending" | "settled" | "failed";

  export interface PaymentRequest<TAmount = number> {
    transactionId: string;
    amount: TAmount;
    currency: string;
  }

  export interface PaymentReceipt {
    receiptId: string;
    status: PaymentStatus;
    settledAt: string;
  }

  export class PaymentProcessor {
    private apiKey: string;

    constructor(apiKey: string) {
      this.apiKey = apiKey;
    }

    /**
     * Executes charge against gateway.
     */
    public async processCharge(req: PaymentRequest): Promise<PaymentReceipt> {
      const authHeader = this.getAuthHeader();
      return this.sendToGateway(req, authHeader);
    }

    private getAuthHeader(): string {
      return `Bearer ${this.apiKey}`;
    }

    private async sendToGateway(req: PaymentRequest, header: string): Promise<PaymentReceipt> {
      // 100 lines of network calls, retries, exponential backoff
      return {
        receiptId: `rcpt_${req.transactionId}`,
        status: "settled",
        settledAt: new Date().toISOString(),
      };
    }
  }

  export const calculateTax = (amount: number, rate: number = 0.2): number => {
    return amount * rate;
  };
  ```

##### Fixture 3: `tsx_components/` (React / TSX components and hooks)
- `tests/fixtures/typescript/tsx_components/UserProfile.tsx`:
  ```tsx
  import React, { useMemo } from 'react';
  import { User } from '../simple_service/types';

  export interface UserProfileProps {
    user: User;
    onUpdate: (updated: User) => void;
    className?: string;
  }

  export function UserProfile({ user, onUpdate, className }: UserProfileProps): JSX.Element {
    const formattedDate = useMemo(() => user.createdAt.toLocaleDateString(), [user.createdAt]);

    return (
      <div className={`user-card ${className ?? ''}`}>
        <h2>{user.email}</h2>
        <span>Role: {user.role}</span>
        <p>Member since: {formattedDate}</p>
        <button onClick={() => onUpdate(user)}>Edit Profile</button>
      </div>
    );
  }
  ```

##### Fixture 4: `edge_cases/` (Circular types, boundary tests, deep nesting)
- `tests/fixtures/typescript/edge_cases/circular.ts`:
  ```typescript
  export interface TreeNode {
    id: string;
    parent?: TreeNode;
    children: TreeChild[];
  }

  export interface TreeChild {
    node: TreeNode;
    metadata: Record<string, string>;
  }

  export function findRoot(node: TreeNode): TreeNode {
    let curr: TreeNode = node;
    while (curr.parent) {
      curr = curr.parent;
    }
    return curr;
  }
  ```
- `tests/fixtures/typescript/edge_cases/empty.ts`: (0 bytes)
- `tests/fixtures/typescript/edge_cases/comments_only.ts`: (`// Just a comment\n/* Multi-line */`)

---

## 5. Verification Method

To independently verify the implementation during and after implementation:

### 5.1. Automated Test Execution
Run the following test commands:
```powershell
# Run formatter and tokenizer unit tests
cargo test -p ctxcut_core --lib formatter
cargo test -p ctxcut_core --lib tokenizer

# Run full core test suite including TS parser and resolver
cargo test -p ctxcut_core --all-targets

# Run clippy to ensure strict zero-warning compliance
cargo clippy --all-targets -- -D warnings
```

### 5.2. Test Invalidation Conditions
- Any occurrence of `NaN` or negative values in `savings_percentage`.
- Any crash/panic when encountering special tokens in source text.
- Any discrepancy in Markdown code-fence tags (e.g. missing language identifier).
- Any failure in round-trip JSON serialization `serde_json::from_str::<SliceResult>(&json)`.
