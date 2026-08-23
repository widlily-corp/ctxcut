//! Adaptive Token Budgeting and Progressive Semantic Degradation Engine (Milestone 4).
//!
//! Provides deterministic 5-level semantic degradation to compress AST slices
//! into strict token constraints (`--budget <N>`) while maximizing preserved semantic value.

use crate::error::Result;
use crate::formatter::MarkdownFormatter;
use crate::model::SliceResult;
use crate::tokenizer::count_tokens;
use serde::{Deserialize, Serialize};

/// Report summarizing progressive semantic degradation actions and token savings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradationReport {
    /// Initial token count of the slice before compression.
    pub initial_tokens: usize,
    /// Final token count of the slice after compression.
    pub final_tokens: usize,
    /// Requested token budget.
    pub target_budget: usize,
    /// Degradation level reached (0 to 5).
    pub degradation_level: u8,
    /// Summary of compression actions performed.
    pub actions_taken: Vec<String>,
}

/// Adaptive budget compressor implementing progressive 5-level semantic degradation.
pub struct BudgetCompressor;

impl BudgetCompressor {
    /// Compresses a `SliceResult` in-place to fit within `budget_tokens` using progressive 5-level degradation.
    pub fn compress_slice(
        slice: &mut SliceResult,
        budget_tokens: usize,
    ) -> Result<DegradationReport> {
        let initial_md = MarkdownFormatter::format(slice);
        let initial_tokens = count_tokens(&initial_md);

        let mut report = DegradationReport {
            initial_tokens,
            final_tokens: initial_tokens,
            target_budget: budget_tokens,
            degradation_level: 0,
            actions_taken: Vec::new(),
        };

        if initial_tokens <= budget_tokens {
            return Ok(report);
        }

        // =========================================================================
        // Level 1: Strip Docstrings & Verbose Comments
        // =========================================================================
        report.degradation_level = 1;
        report
            .actions_taken
            .push("Level 1: Stripped target docstrings and type comments".to_string());

        slice.target_symbol.doc_comment = None;

        for ty in &mut slice.hoisted_types {
            ty.definition = strip_doc_comments_from_str(&ty.definition);
        }

        for imp in &mut slice.hoisted_implementors {
            imp.definition = strip_doc_comments_from_str(&imp.definition);
        }

        for call in &mut slice.stripped_calls {
            call.signature = strip_doc_comments_from_str(&call.signature);
        }

        let l1_md = MarkdownFormatter::format(slice);
        let l1_tokens = count_tokens(&l1_md);
        report.final_tokens = l1_tokens;
        if l1_tokens <= budget_tokens {
            return Ok(report);
        }

        // =========================================================================
        // Level 2: Minify / Prune Hoisted Types and Implementors
        // =========================================================================
        report.degradation_level = 2;
        report
            .actions_taken
            .push("Level 2: Minified hoisted type definitions to compact signatures".to_string());

        for ty in &mut slice.hoisted_types {
            ty.definition = minify_type_definition(&ty.definition, &ty.kind);
        }

        for imp in &mut slice.hoisted_implementors {
            imp.definition = minify_type_definition(&imp.definition, &imp.kind);
        }

        let l2_md = MarkdownFormatter::format(slice);
        let l2_tokens = count_tokens(&l2_md);
        report.final_tokens = l2_tokens;
        if l2_tokens <= budget_tokens {
            return Ok(report);
        }

        // If still overflowing, drop secondary hoisted types and implementors (>2)
        let mut pruned_l2 = false;
        if slice.hoisted_types.len() > 2 {
            let removed = slice.hoisted_types.len() - 2;
            slice.hoisted_types.truncate(2);
            report.actions_taken.push(format!(
                "Level 2b: Pruned {removed} secondary hoisted types"
            ));
            pruned_l2 = true;
        }

        if slice.hoisted_implementors.len() > 2 {
            let removed = slice.hoisted_implementors.len() - 2;
            slice.hoisted_implementors.truncate(2);
            report.actions_taken.push(format!(
                "Level 2c: Pruned {removed} secondary hoisted implementors"
            ));
            pruned_l2 = true;
        }

        if pruned_l2 {
            let l2b_md = MarkdownFormatter::format(slice);
            let l2b_tokens = count_tokens(&l2b_md);
            report.final_tokens = l2b_tokens;
            if l2b_tokens <= budget_tokens {
                return Ok(report);
            }
        }

        // =========================================================================
        // Level 3: Prune / Minify Stripped Calls and Implementors
        // =========================================================================
        report.degradation_level = 3;
        report
            .actions_taken
            .push("Level 3: Minified external call signatures to single-line stubs".to_string());

        for call in &mut slice.stripped_calls {
            call.signature = call
                .signature
                .lines()
                .next()
                .unwrap_or(&call.signature)
                .trim()
                .to_string();
        }

        if slice.hoisted_implementors.len() > 1 {
            let removed = slice.hoisted_implementors.len() - 1;
            slice.hoisted_implementors.truncate(1);
            report.actions_taken.push(format!(
                "Level 3b: Pruned {removed} secondary hoisted implementors"
            ));
        }

        let l3_md = MarkdownFormatter::format(slice);
        let l3_tokens = count_tokens(&l3_md);
        report.final_tokens = l3_tokens;
        if l3_tokens <= budget_tokens {
            return Ok(report);
        }

        // Prune stripped calls to top 2 if still exceeding
        if slice.stripped_calls.len() > 2 {
            let removed = slice.stripped_calls.len() - 2;
            slice.stripped_calls.truncate(2);
            report.actions_taken.push(format!(
                "Level 3c: Pruned {removed} secondary external dependency stubs"
            ));

            let l3b_md = MarkdownFormatter::format(slice);
            let l3b_tokens = count_tokens(&l3b_md);
            report.final_tokens = l3b_tokens;
            if l3b_tokens <= budget_tokens {
                return Ok(report);
            }
        }

        // =========================================================================
        // Level 4: Target Symbol Body Folding (Collapse Inner Blocks / Loops)
        // =========================================================================
        report.degradation_level = 4;
        report
            .actions_taken
            .push("Level 4: Folded secondary inner blocks in target symbol body".to_string());

        slice.target_symbol.body = fold_symbol_body(&slice.target_symbol.body);

        let l4_md = MarkdownFormatter::format(slice);
        let l4_tokens = count_tokens(&l4_md);
        report.final_tokens = l4_tokens;
        if l4_tokens <= budget_tokens {
            return Ok(report);
        }

        // =========================================================================
        // Level 5: Target Symbol Signature-Only Stub
        // =========================================================================
        report.degradation_level = 5;
        report
            .actions_taken
            .push("Level 5: Collapsed target symbol to signature-only stub".to_string());

        slice.hoisted_types.clear();
        slice.hoisted_implementors.clear();
        slice.stripped_calls.clear();

        let sig = &slice.target_symbol.signature;
        let lang = &slice.target_symbol.language;
        slice.target_symbol.body = if lang == "python" {
            format!("{sig}:\n    # Implementation collapsed to fit token budget ({budget_tokens} tokens)\n    ...")
        } else if lang == "rust" || lang == "typescript" || lang == "javascript" || lang == "go" {
            format!("{sig} {{\n    // Implementation collapsed to fit token budget ({budget_tokens} tokens)\n    ...\n}}")
        } else {
            format!("{sig} ...")
        };

        let l5_md = MarkdownFormatter::format(slice);
        report.final_tokens = count_tokens(&l5_md);

        Ok(report)
    }
}

fn strip_doc_comments_from_str(s: &str) -> String {
    let mut out = Vec::new();
    let mut in_block_comment = false;

    for line in s.lines() {
        let trimmed = line.trim();
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.starts_with("/*") || trimmed.starts_with("/**") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }

        if trimmed.starts_with("//")
            || trimmed.starts_with("///")
            || trimmed.starts_with('#')
            || trimmed.starts_with("\"\"\"")
            || trimmed.starts_with("'''")
        {
            continue;
        }

        out.push(line);
    }

    out.join("\n")
}

fn minify_type_definition(def: &str, kind: &str) -> String {
    let cleaned = strip_doc_comments_from_str(def);
    let lines: Vec<&str> = cleaned
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() <= 3 {
        return lines.join("\n");
    }

    // Keep first and last line (e.g. interface Foo { ... })
    if let (Some(first), Some(last)) = (lines.first(), lines.last()) {
        let field_count = lines.len().saturating_sub(2);
        format!("{first}\n    /* {field_count} fields */\n{last}")
    } else {
        format!("{kind} {{ ... }}")
    }
}

fn fold_symbol_body(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() <= 6 {
        return body.to_string();
    }

    let header_lines = &lines[..2];
    let tail_lines = &lines[lines.len().saturating_sub(2)..];
    let collapsed_count = lines.len().saturating_sub(4);

    let mut result = Vec::new();
    result.extend_from_slice(header_lines);
    result.push("    /* ... internal implementation collapsed to fit budget ... */");
    let _ = collapsed_count;
    result.extend_from_slice(tail_lines);
    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, TokenStats};

    fn make_test_slice() -> SliceResult {
        SliceResult {
            target_symbol: ExtractedSymbol {
                name: "processPayment".to_string(),
                kind: "function".to_string(),
                file_path: "src/payment.ts".to_string(),
                start_line: 1,
                end_line: 30,
                doc_comment: Some("/** Comprehensive payment processor with telemetry and retry. */".to_string()),
                signature: "export async function processPayment(req: PaymentRequest): Promise<PaymentResponse>".to_string(),
                body: r#"export async function processPayment(req: PaymentRequest): Promise<PaymentResponse> {
    const validated = validatePayment(req);
    if (!validated) {
        throw new Error("Invalid payment");
    }
    const charge = await stripe.charges.create({
        amount: req.amount,
        currency: req.currency,
        source: req.token,
    });
    const receipt = await emailService.sendReceipt(req.email, charge.id);
    return {
        id: charge.id,
        status: charge.status,
        receiptSent: !!receipt,
    };
}"#.to_string(),
                language: "typescript".to_string(),
            },
            hoisted_types: vec![
                ExtractedType {
                    name: "PaymentRequest".to_string(),
                    kind: "interface".to_string(),
                    file_path: "src/types.ts".to_string(),
                    definition: "/** Payment request payload */\nexport interface PaymentRequest {\n    amount: number;\n    currency: string;\n    token: string;\n    email: string;\n}".to_string(),
                },
                ExtractedType {
                    name: "PaymentResponse".to_string(),
                    kind: "interface".to_string(),
                    file_path: "src/types.ts".to_string(),
                    definition: "/** Payment response payload */\nexport interface PaymentResponse {\n    id: string;\n    status: string;\n    receiptSent: boolean;\n}".to_string(),
                },
            ],
            hoisted_implementors: Vec::new(),
            stripped_calls: vec![
                CallSignatureStub {
                    name: "validatePayment".to_string(),
                    receiver: None,
                    file_path: Some("src/val.ts".to_string()),
                    signature: "export function validatePayment(req: PaymentRequest): boolean;".to_string(),
                },
                CallSignatureStub {
                    name: "sendReceipt".to_string(),
                    receiver: Some("emailService".to_string()),
                    file_path: Some("src/email.ts".to_string()),
                    signature: "export async function sendReceipt(email: string, id: string): Promise<boolean>;".to_string(),
                },
            ],
            stats: TokenStats::calculate(500, 180, 50, 30),
        }
    }

    #[test]
    fn test_budget_no_degradation_when_within_budget() {
        let mut slice = make_test_slice();
        let report = BudgetCompressor::compress_slice(&mut slice, 10_000).unwrap();
        assert_eq!(report.degradation_level, 0);
        assert!(slice.target_symbol.doc_comment.is_some());
    }

    #[test]
    fn test_budget_level_1_docstring_stripping() {
        let mut slice = make_test_slice();
        let initial_tokens = count_tokens(&slice.to_markdown());
        let report = BudgetCompressor::compress_slice(&mut slice, initial_tokens - 15).unwrap();
        assert!(report.degradation_level >= 1);
        assert!(slice.target_symbol.doc_comment.is_none());
    }

    #[test]
    fn test_budget_level_5_extreme_budget_forces_signature_stub() {
        let mut slice = make_test_slice();
        let report = BudgetCompressor::compress_slice(&mut slice, 35).unwrap();
        assert_eq!(report.degradation_level, 5);
        assert!(slice.hoisted_types.is_empty());
        assert!(slice.hoisted_implementors.is_empty());
        assert!(slice.stripped_calls.is_empty());
        assert!(slice
            .target_symbol
            .body
            .contains("// Implementation collapsed"));
    }

    #[test]
    fn test_budget_hoisted_implementors_progressive_degradation() {
        use crate::model::ExtractedImplementor;

        let mut slice = make_test_slice();
        for i in 1..=4 {
            slice.hoisted_implementors.push(ExtractedImplementor {
                interface_name: "PaymentProcessor".to_string(),
                implementor_name: format!("ProcessorImpl{i}"),
                kind: "ts_class".to_string(),
                file_path: format!("src/impl{i}.ts"),
                definition: format!("/** Doc comment for Impl {i} */\nexport class ProcessorImpl{i} implements PaymentProcessor {{\n    public process() {{}}\n}}"),
            });
        }

        // Test Level 1: docstrings stripped
        let initial_tokens = count_tokens(&slice.to_markdown());
        let report_l1 = BudgetCompressor::compress_slice(&mut slice, initial_tokens - 10).unwrap();
        assert!(report_l1.degradation_level >= 1);
        for imp in &slice.hoisted_implementors {
            assert!(!imp.definition.contains("/**"));
        }

        // Test Level 2: implementors pruned to <= 2
        let report_l2 = BudgetCompressor::compress_slice(&mut slice, 120).unwrap();
        assert!(report_l2.degradation_level >= 2);
        assert!(slice.hoisted_implementors.len() <= 2);

        // Test Level 3: implementors pruned to <= 1
        let report_l3 = BudgetCompressor::compress_slice(&mut slice, 70).unwrap();
        assert!(report_l3.degradation_level >= 3);
        assert!(slice.hoisted_implementors.len() <= 1);

        // Test Level 5: implementors cleared
        let report_l5 = BudgetCompressor::compress_slice(&mut slice, 30).unwrap();
        assert_eq!(report_l5.degradation_level, 5);
        assert!(slice.hoisted_implementors.is_empty());
    }
}
