//! JSON formatter for producing structured context slice payloads.

use crate::model::SliceResult;
use serde_json::Result;

/// Formats AST slice results into structured JSON.
pub struct JsonFormatter;

impl JsonFormatter {
    /// Formats a single `SliceResult` into pretty-printed JSON.
    pub fn format_pretty(result: &SliceResult) -> Result<String> {
        serde_json::to_string_pretty(result)
    }

    /// Formats a single `SliceResult` into compact JSON.
    pub fn format_compact(result: &SliceResult) -> Result<String> {
        serde_json::to_string(result)
    }

    /// Formats a batch of `SliceResult` items into pretty-printed JSON array.
    pub fn format_batch(results: &[SliceResult]) -> Result<String> {
        serde_json::to_string_pretty(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CallSignatureStub, ExtractedSymbol, ExtractedType, TokenStats};

    #[test]
    fn test_json_roundtrip() {
        let result = SliceResult {
            target_symbol: ExtractedSymbol {
                name: "test".to_string(),
                kind: "function".to_string(),
                file_path: "test.ts".to_string(),
                start_line: 1,
                end_line: 5,
                doc_comment: None,
                signature: "function test(): void".to_string(),
                body: "function test() {}".to_string(),
                language: "typescript".to_string(),
            },
            hoisted_types: vec![ExtractedType {
                name: "T".to_string(),
                kind: "type_alias".to_string(),
                file_path: "test.ts".to_string(),
                definition: "type T = string;".to_string(),
            }],
            stripped_calls: vec![CallSignatureStub {
                name: "call".to_string(),
                receiver: None,
                file_path: None,
                signature: "function call(): void;".to_string(),
            }],
            stats: TokenStats::calculate(100, 20, 20, 10),
        };

        let json = JsonFormatter::format_pretty(&result).expect("Failed to serialize to JSON");
        let deserialized: SliceResult = serde_json::from_str(&json).expect("Failed to deserialize JSON");
        assert_eq!(result, deserialized);
    }
}
