//! Markdown formatter for generating prompt-optimized context slices.

use crate::model::SliceResult;

/// Formats AST slice results into prompt-optimized Markdown documents for LLMs.
pub struct MarkdownFormatter;

impl MarkdownFormatter {
    /// Formats a single `SliceResult` into prompt-optimized Markdown.
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

    /// Formats a batch of `SliceResult` items into a combined Markdown document.
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
    if lang.eq_ignore_ascii_case("typescript") || lang.eq_ignore_ascii_case("ts") {
        "typescript"
    } else if lang.eq_ignore_ascii_case("tsx") {
        "tsx"
    } else if lang.eq_ignore_ascii_case("javascript") || lang.eq_ignore_ascii_case("js") {
        "javascript"
    } else if lang.eq_ignore_ascii_case("jsx") {
        "jsx"
    } else if lang.eq_ignore_ascii_case("python") || lang.eq_ignore_ascii_case("py") {
        "python"
    } else if lang.eq_ignore_ascii_case("go") || lang.eq_ignore_ascii_case("golang") {
        "go"
    } else if lang.eq_ignore_ascii_case("rust") || lang.eq_ignore_ascii_case("rs") {
        "rust"
    } else {
        lang
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, TokenStats};

    #[test]
    fn test_markdown_format_full() {
        let result = SliceResult {
            target_symbol: ExtractedSymbol {
                name: "login".to_string(),
                kind: "function".to_string(),
                file_path: "src/auth.ts".to_string(),
                start_line: 10,
                end_line: 25,
                doc_comment: Some("/** Logs in user */".to_string()),
                signature: "export async function login(dto: LoginDto): Promise<Token>".to_string(),
                body: "export async function login(dto: LoginDto): Promise<Token> {\n    return auth(dto);\n}".to_string(),
                language: "typescript".to_string(),
            },
            hoisted_types: vec![
                ExtractedType {
                    name: "LoginDto".to_string(),
                    kind: "interface".to_string(),
                    file_path: "src/types.ts".to_string(),
                    definition: "export interface LoginDto {\n    email: string;\n}".to_string(),
                }
            ],
            stripped_calls: vec![
                CallSignatureStub {
                    name: "auth".to_string(),
                    receiver: None,
                    file_path: Some("src/utils.ts".to_string()),
                    signature: "export function auth(dto: LoginDto): Token;".to_string(),
                }
            ],
            stats: TokenStats::calculate(200, 50, 40, 15),
        };

        let md = MarkdownFormatter::format(&result);
        assert!(md.contains("### Context Slice: `src/auth.ts:login`"));
        assert!(md.contains("#### 1. Target Implementation (Full Body)"));
        assert!(md.contains("#### 2. Hoisted Types & Data Contracts"));
        assert!(md.contains("#### 3. External Dependencies & Signatures (Body Stripped)"));
        assert!(md.contains("```typescript"));
        assert!(md.contains("export interface LoginDto"));
        assert!(md.contains("export function auth(dto: LoginDto): Token;"));
    }

    #[test]
    fn test_markdown_format_empty_deps() {
        let result = SliceResult {
            target_symbol: ExtractedSymbol {
                name: "add".to_string(),
                kind: "function".to_string(),
                file_path: "src/math.ts".to_string(),
                start_line: 1,
                end_line: 3,
                doc_comment: None,
                signature: "export function add(a: number, b: number): number".to_string(),
                body: "export function add(a: number, b: number): number {\n    return a + b;\n}".to_string(),
                language: "typescript".to_string(),
            },
            hoisted_types: vec![],
            stripped_calls: vec![],
            stats: TokenStats::calculate(50, 20, 10, 8),
        };

        let md = MarkdownFormatter::format(&result);
        assert!(md.contains("*None*"));
    }
}
