//! Token Verifier module for ctxcut E2E test suite.
//!
//! Provides automated OpenAI BPE (`cl100k_base`) token counting via `tiktoken-rs`
//! and strict assertions verifying >=80-90% token reduction with exact metrics.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use tiktoken_rs::CoreBPE;

/// Metrics representing token counts and reduction percentages before and after slicing.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenMetrics {
    /// Token count of the full, un-sliced source file or content.
    pub full_tokens: usize,
    /// Token count of the extracted context slice (Markdown or code).
    pub slice_tokens: usize,
    /// Percentage of tokens saved: `((full - slice) / full) * 100.0`.
    pub reduction_percentage: f64,
    /// Total lines in full source content.
    pub full_lines: usize,
    /// Total lines in sliced content.
    pub slice_lines: usize,
}

/// Automated BPE token counter and reduction verifier.
#[derive(Clone)]
pub struct TokenVerifier {
    bpe: Arc<CoreBPE>,
}

impl Default for TokenVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TokenVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenVerifier")
            .field("encoding", &"cl100k_base")
            .finish()
    }
}

impl TokenVerifier {
    /// Creates a new `TokenVerifier` initialized with the `cl100k_base` BPE tokenizer.
    ///
    /// # Panics
    /// Panics if the `cl100k_base` tokenizer cannot be loaded.
    pub fn new() -> Self {
        let bpe = tiktoken_rs::cl100k_base()
            .expect("Failed to initialize cl100k_base tokenizer for TokenVerifier");
        Self { bpe: Arc::new(bpe) }
    }

    /// Counts the exact number of BPE tokens in the provided text.
    pub fn count_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        self.bpe.encode_with_special_tokens(text).len()
    }

    /// Counts total lines in the provided text.
    pub fn count_lines(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        text.lines().count()
    }

    /// Calculates token and line metrics comparing full text to slice text.
    pub fn calculate_metrics(&self, full_text: &str, slice_text: &str) -> TokenMetrics {
        let full_tokens = self.count_tokens(full_text);
        let slice_tokens = self.count_tokens(slice_text);
        let full_lines = self.count_lines(full_text);
        let slice_lines = self.count_lines(slice_text);

        let reduction_percentage = if full_tokens == 0 {
            0.0
        } else {
            ((full_tokens as f64 - slice_tokens as f64) / full_tokens as f64) * 100.0
        };

        TokenMetrics {
            full_tokens,
            slice_tokens,
            reduction_percentage,
            full_lines,
            slice_lines,
        }
    }

    /// Verifies that the token reduction percentage meets or exceeds `min_expected_reduction_pct`.
    ///
    /// # Panics
    /// Panics with detailed diagnostics if the reduction is less than expected.
    pub fn verify_reduction(
        &self,
        full_text: &str,
        slice_markdown: &str,
        min_expected_reduction_pct: f64,
    ) -> TokenMetrics {
        let metrics = self.calculate_metrics(full_text, slice_markdown);
        assert!(
            metrics.reduction_percentage >= min_expected_reduction_pct,
            "Token reduction assertion failed!\n\
             Expected minimum: {:.2}%\n\
             Actual reduction: {:.2}%\n\
             Full tokens:      {}\n\
             Slice tokens:     {}\n\
             Full lines:       {}\n\
             Slice lines:      {}",
            min_expected_reduction_pct,
            metrics.reduction_percentage,
            metrics.full_tokens,
            metrics.slice_tokens,
            metrics.full_lines,
            metrics.slice_lines
        );
        metrics
    }

    /// Verifies that the token reduction percentage falls within `[min_pct, max_pct]`.
    ///
    /// # Panics
    /// Panics with detailed diagnostics if the reduction is outside the specified range.
    pub fn verify_reduction_range(
        &self,
        full_text: &str,
        slice_markdown: &str,
        min_pct: f64,
        max_pct: f64,
    ) -> TokenMetrics {
        let metrics = self.calculate_metrics(full_text, slice_markdown);
        assert!(
            metrics.reduction_percentage >= min_pct && metrics.reduction_percentage <= max_pct,
            "Token reduction out of expected range!\n\
             Expected range:   {:.2}% - {:.2}%\n\
             Actual reduction: {:.2}%\n\
             Full tokens:      {}\n\
             Slice tokens:     {}",
            min_pct,
            max_pct,
            metrics.reduction_percentage,
            metrics.full_tokens,
            metrics.slice_tokens
        );
        metrics
    }

    /// Reads full content from a file path and verifies token reduction against slice markdown.
    pub fn verify_file_reduction(
        &self,
        file_path: impl AsRef<Path>,
        slice_markdown: &str,
        min_expected_reduction_pct: f64,
    ) -> std::io::Result<TokenMetrics> {
        let full_text = fs::read_to_string(file_path)?;
        Ok(self.verify_reduction(&full_text, slice_markdown, min_expected_reduction_pct))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_verifier_empty_text() {
        let verifier = TokenVerifier::new();
        assert_eq!(verifier.count_tokens(""), 0);
        assert_eq!(verifier.count_lines(""), 0);

        let metrics = verifier.calculate_metrics("", "");
        assert_eq!(metrics.full_tokens, 0);
        assert_eq!(metrics.slice_tokens, 0);
        assert_eq!(metrics.reduction_percentage, 0.0);
    }

    #[test]
    fn test_token_verifier_metrics_calculation() {
        let verifier = TokenVerifier::new();
        let full_source = r#"
            import { DbClient } from './db';
            import { Logger } from './logger';
            import { NotificationService } from './notification';

            export interface UserProfile {
                id: string;
                email: string;
                displayName: string;
                role: 'admin' | 'member';
            }

            export class UserService {
                constructor(
                    private db: DbClient,
                    private logger: Logger,
                    private notifier: NotificationService
                ) {}

                async getUser(id: string): Promise<UserProfile> {
                    this.logger.info(`Fetching user ${id}`);
                    const user = await this.db.users.findUnique({ where: { id } });
                    if (!user) throw new Error('User not found');
                    return user;
                }

                async deleteUser(id: string): Promise<void> {
                    await this.db.users.delete({ where: { id } });
                }

                async listUsers(): Promise<UserProfile[]> {
                    return this.db.users.findMany();
                }
            }
        "#;

        let sliced_markdown = r#"
            # Context Slice: UserService.getUser
            ```typescript
            export interface UserProfile {
                id: string;
                email: string;
                displayName: string;
                role: 'admin' | 'member';
            }

            async getUser(id: string): Promise<UserProfile> {
                this.logger.info(`Fetching user ${id}`);
                const user = await this.db.users.findUnique({ where: { id } });
                if (!user) throw new Error('User not found');
                return user;
            }
            ```
        "#;

        let metrics = verifier.calculate_metrics(full_source, sliced_markdown);
        assert!(metrics.full_tokens > metrics.slice_tokens);
        assert!(metrics.reduction_percentage > 30.0);
        assert_eq!(metrics.full_lines, full_source.lines().count());
        assert_eq!(metrics.slice_lines, sliced_markdown.lines().count());
    }

    #[test]
    fn test_token_verifier_verify_reduction_success() {
        let verifier = TokenVerifier::new();
        let large_full =
            "fn large_function() {\n".to_string() + &"    println!(\"work\");\n".repeat(50) + "}\n";
        let minimal_slice = "fn large_function() {}\n";

        let metrics = verifier.verify_reduction(&large_full, minimal_slice, 80.0);
        assert!(metrics.reduction_percentage >= 80.0);
    }
}
