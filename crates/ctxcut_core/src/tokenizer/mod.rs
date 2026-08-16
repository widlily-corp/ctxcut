//! BPE Token Counting Engine using `tiktoken-rs` with OpenAI `cl100k_base` encoding.

use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;
use crate::model::TokenStats;

static BPE_TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

/// Returns a shared reference to the global `cl100k_base` BPE tokenizer instance.
/// Initialized lazily upon first access.
pub fn get_bpe_tokenizer() -> &'static CoreBPE {
    BPE_TOKENIZER.get_or_init(|| {
        tiktoken_rs::cl100k_base()
            .expect("Fatal: Failed to initialize tiktoken cl100k_base tokenizer")
    })
}

/// Token counter utility struct.
pub struct TokenCounter;

impl TokenCounter {
    /// Counts the exact BPE tokens in a UTF-8 string using `cl100k_base`.
    /// Uses `encode_ordinary` to avoid panicking on special token sequences.
    pub fn count(text: &str) -> usize {
        count_tokens(text)
    }

    /// Computes full `TokenStats` comparing raw source code against generated slice markdown.
    pub fn stats(raw_source: &str, sliced_markdown: &str) -> TokenStats {
        compute_stats(raw_source, sliced_markdown)
    }
}

/// Counts the exact BPE tokens in a UTF-8 string using `cl100k_base`.
/// Uses `encode_ordinary` to prevent panicking on special token sequences like `<|endoftext|>`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens_empty() {
        assert_eq!(count_tokens(""), 0);
    }

    #[test]
    fn test_count_tokens_code() {
        let code = "export async function login(dto: LoginDto): Promise<Token> { return token; }";
        let tokens = count_tokens(code);
        assert!(tokens > 5, "Tokens count was {tokens}");
    }

    #[test]
    fn test_savings_calc_normal() {
        let pct = calculate_savings_percentage(1000, 150);
        assert_eq!(pct, 85.0);
    }

    #[test]
    fn test_savings_calc_zero_raw() {
        let pct = calculate_savings_percentage(0, 50);
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn test_savings_calc_overflow() {
        let pct = calculate_savings_percentage(10, 50);
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn test_special_tokens_safety() {
        let text = "let msg = '<|endoftext|>'; let start = '<|im_start|>';";
        let count = count_tokens(text);
        assert!(count > 0);
    }

    #[test]
    fn test_multithreaded_singleton() {
        use std::thread;
        let mut handles = Vec::new();
        for _ in 0..8 {
            let handle = thread::spawn(|| {
                let code = "const a: number = 42;";
                let count = count_tokens(code);
                assert!(count > 0);
            });
            handles.push(handle);
        }
        for h in handles {
            h.join().unwrap();
        }
    }
}
