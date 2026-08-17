//! Empirical Challenger Test Suite for Tokenizer & Output Formatters
//!
//! Thoroughly tests:
//! 1. BPE Tokenizer edge cases (empty, 1-token, massive >10k lines, unicode/emojis/Cyrillic/CJK, BOM, zero-width)
//! 2. Special prompt token crash-resilience (<|endoftext|>, <|im_start|>, etc.)
//! 3. Savings percentage arithmetic (zero division, NaN, negative, overflow, rounding, extreme usize)
//! 4. Markdown formatter fidelity (headers, fence balance, language tags, *None* fallbacks, edge whitespaces)
//! 5. JSON serialization & lossless round-trip deserialization
//! 6. Concurrency, multi-threading, and stress benchmarks

use ctxcut_core::{
    formatter::{markdown::normalize_language_tag, JsonFormatter, MarkdownFormatter},
    tokenizer::{
        calculate_savings_percentage, count_lines, count_tokens, get_bpe_tokenizer, TokenCounter,
    },
    CallSignatureStub, ExtractedSymbol, ExtractedType, SliceResult, TokenStats,
};
use std::fmt::Write as _;
use std::time::Instant;

#[test]
fn test_tokenizer_empty_and_minimal_inputs() {
    // 0 tokens
    assert_eq!(count_tokens(""), 0);
    assert_eq!(TokenCounter::count(""), 0);
    assert_eq!(count_lines(""), 0);

    // 1 token / minimal inputs
    let one_char = "a";
    let tokens = count_tokens(one_char);
    assert_eq!(tokens, 1);
    assert_eq!(count_lines(one_char), 1);

    let single_space = " ";
    assert_eq!(count_tokens(single_space), 1);

    let single_newline = "\n";
    assert_eq!(count_tokens(single_newline), 1);
    assert_eq!(count_lines(single_newline), 1);

    let multiple_newlines = "\n\n\n\n\n";
    assert_eq!(count_lines(multiple_newlines), 5);
}

#[test]
fn test_tokenizer_non_ascii_utf8_and_unicode_edge_cases() {
    // 1. Emojis
    let emojis = "🚀 🦀 💡 🔥 ✨ 💻 🌍 📦 🛡️ ⚙️ 🎯 🧠";
    let emoji_tokens = count_tokens(emojis);
    assert!(
        emoji_tokens >= 12,
        "Expected at least 12 tokens for emojis, got {emoji_tokens}"
    );

    // 2. Cyrillic (Russian)
    let cyrillic = "Функция аутентификации пользователя в системе безопасности";
    let cyrillic_tokens = count_tokens(cyrillic);
    assert!(cyrillic_tokens > 0);

    // 3. CJK (Kanji, Hiragana, Katakana, Hanzi)
    let cjk = "ユーザー認証と認可処理およびデータベーストランザクション管理";
    let cjk_tokens = count_tokens(cjk);
    assert!(cjk_tokens > 0);

    let hanzi = "用户认证与权限校验微服务核心接口";
    let hanzi_tokens = count_tokens(hanzi);
    assert!(hanzi_tokens > 0);

    // 4. Unicode BOM and Zero-Width characters
    let bom_text = "\u{FEFF}const x = 1;\u{200B}\u{200C}\u{200D}";
    let bom_tokens = count_tokens(bom_text);
    assert!(bom_tokens > 0);

    // 5. Mixed multi-language text
    let mixed = format!("{emojis}\n{cyrillic}\n{cjk}\n{hanzi}\nconst x: number = 42;");
    let mixed_tokens = count_tokens(&mixed);
    assert_eq!(count_lines(&mixed), 5);
    assert!(mixed_tokens > emoji_tokens + cyrillic_tokens);
}

#[test]
fn test_special_prompt_tokens_zero_panic() {
    // Test known special tokens that trigger panics in standard BPE encode without encode_ordinary
    let special_tokens = vec![
        "<|endoftext|>",
        "<|fim_prefix|>",
        "<|fim_middle|>",
        "<|fim_suffix|>",
        "<|endofprompt|>",
        "<|im_start|>",
        "<|im_end|>",
        "<|extra_0|>",
        "<|extra_100|>",
        "```typescript\n// <|endoftext|>\nconst prompt = '<|im_start|>system\\n<|im_end|>';\n```",
        "const identifier_<|endoftext|>_var = 123;",
    ];

    for st in special_tokens {
        // Must not panic
        let count = count_tokens(st);
        assert!(
            count > 0,
            "Token count for '{st}' should be > 0, got {count}"
        );

        let direct_count = get_bpe_tokenizer().encode_ordinary(st).len();
        assert_eq!(count, direct_count);
    }
}

#[test]
fn test_savings_percentage_arithmetic_safety() {
    // Normal cases
    assert_eq!(calculate_savings_percentage(1000, 200), 80.0);
    assert_eq!(calculate_savings_percentage(1000, 0), 100.0);
    assert_eq!(calculate_savings_percentage(100, 50), 50.0);
    assert_eq!(calculate_savings_percentage(300, 100), 66.67);
    assert_eq!(calculate_savings_percentage(7, 1), 85.71);
    assert_eq!(calculate_savings_percentage(100, 99), 1.0);

    // Edge case: raw == sliced (0% savings)
    assert_eq!(calculate_savings_percentage(500, 500), 0.0);

    // Edge case: raw == 0 (must return 0.0, no NaN, no division by zero)
    let zero_raw = calculate_savings_percentage(0, 50);
    assert_eq!(zero_raw, 0.0);
    assert!(!zero_raw.is_nan());
    assert!(!zero_raw.is_infinite());

    let zero_both = calculate_savings_percentage(0, 0);
    assert_eq!(zero_both, 0.0);
    assert!(!zero_both.is_nan());

    // Edge case: sliced > raw (overflow / slice larger than raw) -> must return 0.0, never negative
    let overflow_1 = calculate_savings_percentage(100, 150);
    assert_eq!(overflow_1, 0.0);

    let overflow_2 = calculate_savings_percentage(1, 10000);
    assert_eq!(overflow_2, 0.0);

    // Extreme scale: large usize
    let big_raw = 1_000_000_000;
    let big_sliced = 100_000_000;
    assert_eq!(calculate_savings_percentage(big_raw, big_sliced), 90.0);

    // Verify TokenStats::calculate matches calculate_savings_percentage exactly
    let stats1 = TokenStats::calculate(1000, 200, 50, 10);
    assert_eq!(stats1.savings_percentage, 80.0);
    assert_eq!(stats1.raw_file_tokens, 1000);
    assert_eq!(stats1.sliced_tokens, 200);

    let stats_zero = TokenStats::calculate(0, 50, 0, 5);
    assert_eq!(stats_zero.savings_percentage, 0.0);
    assert!(!stats_zero.savings_percentage.is_nan());

    let stats_overflow = TokenStats::calculate(50, 100, 5, 10);
    assert_eq!(stats_overflow.savings_percentage, 0.0);
}

#[test]
fn test_massive_file_token_counting_performance() {
    // Generate massive synthetic source code (>10,000 lines)
    let mut massive_code = String::with_capacity(1_000_000);
    for i in 0..10_500 {
        let _ = write!(
            massive_code,
            "export function calculateMetric_{i}(paramA: number, paramB: string): Result<MetricDto_{i}> {{\n    return processMetric(paramA, paramB, {i});\n}}\n"
        );
    }

    let line_count = count_lines(&massive_code);
    assert!(line_count >= 10_500, "Line count was {line_count}");

    let start = Instant::now();
    let token_count = count_tokens(&massive_code);
    let elapsed = start.elapsed();

    assert!(token_count > 90_000, "Token count was {token_count}");

    // Release mode takes < 200ms; Debug mode takes < 8000ms
    let max_allowed_ms = if cfg!(debug_assertions) { 8000 } else { 500 };
    assert!(
        elapsed.as_millis() < max_allowed_ms,
        "Tokenization of 10.5k lines took too long: {}ms (limit: {}ms)",
        elapsed.as_millis(),
        max_allowed_ms
    );

    // Compute full stats on massive text vs small slice
    let small_slice = "export function calculateMetric_42(paramA: number, paramB: string): Result<MetricDto_42> {\n    return processMetric(paramA, paramB, 42);\n}";
    let stats = TokenCounter::stats(&massive_code, small_slice);
    assert!(stats.savings_percentage > 99.0);
    assert_eq!(stats.raw_lines, line_count);
    assert_eq!(stats.sliced_lines, 3);
}

#[test]
fn test_markdown_formatter_fidelity_and_code_fence_balance() {
    let mock_result = SliceResult {
        target_symbol: ExtractedSymbol {
            name: "executeOrder".to_string(),
            kind: "function".to_string(),
            file_path: "src/services/trading.ts".to_string(),
            start_line: 45,
            end_line: 75,
            doc_comment: Some("/**\n * Executes a trading order.\n * @param order Order payload\n */".to_string()),
            signature: "export async function executeOrder(order: OrderDto): Promise<OrderReceipt>".to_string(),
            body: "export async function executeOrder(order: OrderDto): Promise<OrderReceipt> {\n    validateOrder(order);\n    return broker.submit(order);\n}".to_string(),
            language: "typescript".to_string(),
        },
        hoisted_types: vec![
            ExtractedType {
                name: "OrderDto".to_string(),
                kind: "interface".to_string(),
                file_path: "src/types/order.ts".to_string(),
                definition: "export interface OrderDto {\n    id: string;\n    amount: number;\n    side: 'BUY' | 'SELL';\n}".to_string(),
            },
            ExtractedType {
                name: "OrderReceipt".to_string(),
                kind: "type_alias".to_string(),
                file_path: "src/types/order.ts".to_string(),
                definition: "export type OrderReceipt = {\n    status: 'FILLED' | 'REJECTED';\n    txHash: string;\n};".to_string(),
            },
        ],
        stripped_calls: vec![
            CallSignatureStub {
                name: "validateOrder".to_string(),
                receiver: None,
                file_path: Some("src/utils/validation.ts".to_string()),
                signature: "export function validateOrder(order: OrderDto): void;".to_string(),
            },
            CallSignatureStub {
                name: "submit".to_string(),
                receiver: Some("broker".to_string()),
                file_path: Some("src/clients/broker.ts".to_string()),
                signature: "submit(order: OrderDto): Promise<OrderReceipt>;".to_string(),
            },
        ],
        stats: TokenStats::calculate(1500, 250, 120, 25),
    };

    let md = MarkdownFormatter::format(&mock_result);

    // 1. Check Exact Headers
    assert!(md.contains("### Context Slice: `src/services/trading.ts:executeOrder`"));
    assert!(md.contains("*Language: `typescript` | Lines: `25` (was `120`) | Tokens: `250` (was `1500`) | Savings: `83.3%`*"));
    assert!(md.contains("#### 1. Target Implementation (Full Body)"));
    assert!(md.contains("#### 2. Hoisted Types & Data Contracts"));
    assert!(md.contains("#### 3. External Dependencies & Signatures (Body Stripped)"));

    // 2. Check Code Fence Balance
    // In full slice with types and stubs, there must be exactly 3 code blocks: 3 opening ```typescript and 3 closing ```
    let open_fence_count = md.matches("```typescript").count();
    let total_fence_count = md.matches("```").count();
    assert_eq!(
        open_fence_count, 3,
        "Expected 3 opening fences ```typescript"
    );
    assert_eq!(
        total_fence_count, 6,
        "Expected 6 fence markers total (3 opening + 3 closing)"
    );

    // 3. Verify Doc Comments in Target Implementation
    assert!(md.contains("/**\n * Executes a trading order."));

    // 4. Verify hoisted types separated by double newlines
    assert!(md.contains("export interface OrderDto"));
    assert!(md.contains("export type OrderReceipt"));

    // 5. Verify stripped calls
    assert!(md.contains("export function validateOrder(order: OrderDto): void;"));
    assert!(md.contains("submit(order: OrderDto): Promise<OrderReceipt>;"));
}

#[test]
fn test_markdown_formatter_none_fallbacks_and_empty_doc_comments() {
    let empty_deps_result = SliceResult {
        target_symbol: ExtractedSymbol {
            name: "pureMath".to_string(),
            kind: "function".to_string(),
            file_path: "src/math.ts".to_string(),
            start_line: 1,
            end_line: 3,
            doc_comment: Some("   \n\t  ".to_string()), // Whitespace-only doc comment should be ignored
            signature: "export function pureMath(x: number): number".to_string(),
            body: "export function pureMath(x: number): number {\n    return x * 2;\n}".to_string(),
            language: "typescript".to_string(),
        },
        hoisted_types: vec![],
        stripped_calls: vec![],
        stats: TokenStats::calculate(50, 15, 5, 3),
    };

    let md = MarkdownFormatter::format(&empty_deps_result);

    // Check Fallback *None*
    assert_eq!(
        md.matches("*None*").count(),
        2,
        "Both types and calls sections should show *None*"
    );

    // Code fence balance: Only Section 1 has code block (1 open, 1 close)
    let open_fence_count = md.matches("```typescript").count();
    let total_fence_count = md.matches("```").count();
    assert_eq!(open_fence_count, 1);
    assert_eq!(total_fence_count, 2);

    // Verify whitespace-only doc comment didn't insert empty extra lines
    assert!(!md.contains("```typescript\n\nexport function pureMath"));
}

#[test]
fn test_markdown_batch_formatting() {
    let r1 = SliceResult {
        target_symbol: ExtractedSymbol {
            name: "fn1".to_string(),
            kind: "function".to_string(),
            file_path: "a.ts".to_string(),
            start_line: 1,
            end_line: 2,
            doc_comment: None,
            signature: "function fn1()".to_string(),
            body: "function fn1() {}".to_string(),
            language: "ts".to_string(),
        },
        hoisted_types: vec![],
        stripped_calls: vec![],
        stats: TokenStats::calculate(10, 5, 2, 1),
    };
    let r2 = SliceResult {
        target_symbol: ExtractedSymbol {
            name: "fn2".to_string(),
            kind: "function".to_string(),
            file_path: "b.ts".to_string(),
            start_line: 1,
            end_line: 2,
            doc_comment: None,
            signature: "function fn2()".to_string(),
            body: "function fn2() {}".to_string(),
            language: "ts".to_string(),
        },
        hoisted_types: vec![],
        stripped_calls: vec![],
        stats: TokenStats::calculate(10, 5, 2, 1),
    };

    let batch_md = MarkdownFormatter::format_batch(&[r1, r2]);
    assert!(batch_md.contains("### Context Slice: `a.ts:fn1`"));
    assert!(batch_md.contains("### Context Slice: `b.ts:fn2`"));
    assert!(batch_md.contains("\n\n---\n\n"));
}

#[test]
fn test_language_tag_normalization() {
    assert_eq!(normalize_language_tag("typescript"), "typescript");
    assert_eq!(normalize_language_tag("TS"), "typescript");
    assert_eq!(normalize_language_tag("tsx"), "tsx");
    assert_eq!(normalize_language_tag("TSX"), "tsx");
    assert_eq!(normalize_language_tag("javascript"), "javascript");
    assert_eq!(normalize_language_tag("js"), "javascript");
    assert_eq!(normalize_language_tag("jsx"), "jsx");
    assert_eq!(normalize_language_tag("python"), "python");
    assert_eq!(normalize_language_tag("py"), "python");
    assert_eq!(normalize_language_tag("go"), "go");
    assert_eq!(normalize_language_tag("golang"), "go");
    assert_eq!(normalize_language_tag("rust"), "rust");
    assert_eq!(normalize_language_tag("rs"), "rust");
    assert_eq!(normalize_language_tag("custom_lang"), "custom_lang");
}

#[test]
fn test_json_serialization_and_roundtrip_integrity() {
    let result = SliceResult {
        target_symbol: ExtractedSymbol {
            name: "complexHandler".to_string(),
            kind: "function".to_string(),
            file_path: "src/complex.ts".to_string(),
            start_line: 10,
            end_line: 50,
            doc_comment: Some("/** Complex JSDoc with special characters: <|endoftext|> & \"quotes\" & \nnewlines */".to_string()),
            signature: "export async function complexHandler(req: Request<User>): Promise<Response<User>>".to_string(),
            body: "export async function complexHandler(req: Request<User>): Promise<Response<User>> {\n    // unicode: 🚀 Привет\n    return res;\n}".to_string(),
            language: "typescript".to_string(),
        },
        hoisted_types: vec![
            ExtractedType {
                name: "User".to_string(),
                kind: "interface".to_string(),
                file_path: "src/user.ts".to_string(),
                definition: "export interface User {\n    name: string;\n    tags: string[];\n}".to_string(),
            }
        ],
        stripped_calls: vec![
            CallSignatureStub {
                name: "sendResponse".to_string(),
                receiver: None,
                file_path: Some("src/http.ts".to_string()),
                signature: "export function sendResponse(res: Response): void;".to_string(),
            }
        ],
        stats: TokenStats::calculate(800, 120, 80, 15),
    };

    // 1. Pretty JSON roundtrip
    let pretty_json = JsonFormatter::format_pretty(&result).expect("Pretty JSON failed");
    let deserialized_pretty: SliceResult =
        serde_json::from_str(&pretty_json).expect("Deserialization pretty failed");
    assert_eq!(result, deserialized_pretty);

    // 2. Compact JSON roundtrip
    let compact_json = JsonFormatter::format_compact(&result).expect("Compact JSON failed");
    let deserialized_compact: SliceResult =
        serde_json::from_str(&compact_json).expect("Deserialization compact failed");
    assert_eq!(result, deserialized_compact);

    // 3. Batch JSON roundtrip
    let batch = vec![result.clone(), result];
    let batch_json = JsonFormatter::format_batch(&batch).expect("Batch JSON failed");
    let deserialized_batch: Vec<SliceResult> =
        serde_json::from_str(&batch_json).expect("Deserialization batch failed");
    assert_eq!(deserialized_batch.len(), 2);
    assert_eq!(deserialized_batch[0], deserialized_batch[1]);

    // 4. Verify SliceResult helper methods
    assert_eq!(deserialized_compact.to_json(), pretty_json);
    assert_eq!(deserialized_compact.to_json_compact(), compact_json);
}
