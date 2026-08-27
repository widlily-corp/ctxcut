//! BM25 multi-field lexical-structural tokenizer for source code and natural language queries.

use std::collections::{HashMap, HashSet};

/// Document fields indexed for BM25 lexical-structural ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    /// Symbol identifier name (e.g. `validateJwtToken`, `ReserveStockResult`).
    Name,
    /// Type signatures, parameter contracts, and return types.
    Signature,
    /// Documentation comments, JSDoc, rustdoc, and docstrings.
    Docstring,
    /// File path and directory components.
    Path,
    /// Implementation body source code.
    Body,
}

impl FieldKind {
    /// Returns string identifier for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Signature => "signature",
            Self::Docstring => "docstring",
            Self::Path => "path",
            Self::Body => "body",
        }
    }

    /// Parses field kind from string identifier.
    pub fn from_field_str(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// BM25F field weight multiplier.
    pub fn weight(&self) -> f64 {
        match self {
            Self::Name => 3.5,
            Self::Signature => 2.0,
            Self::Docstring => 1.5,
            Self::Path => 1.2,
            Self::Body => 0.8,
        }
    }
}

impl std::str::FromStr for FieldKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "name" => Ok(Self::Name),
            "signature" => Ok(Self::Signature),
            "docstring" => Ok(Self::Docstring),
            "path" => Ok(Self::Path),
            "body" => Ok(Self::Body),
            _ => Err(()),
        }
    }
}

/// Tokenized multi-field document for a symbol or file.
#[derive(Debug, Clone, Default)]
pub struct SymbolTokenDocument {
    /// Optional symbol ID in SQLite database.
    pub symbol_id: Option<i64>,
    /// Optional file ID in SQLite database.
    pub file_id: Option<i64>,
    /// Extracted term frequencies per field: FieldKind -> (Term -> Frequency).
    pub field_term_freqs: HashMap<FieldKind, HashMap<String, usize>>,
    /// Total term count in each field.
    pub field_lengths: HashMap<FieldKind, usize>,
    /// Total terms across all fields.
    pub total_terms: usize,
}

impl SymbolTokenDocument {
    /// Creates a new empty symbol token document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds tokens for a given field kind.
    pub fn add_field_tokens(&mut self, field: FieldKind, tokens: &[String]) {
        let entry = self.field_term_freqs.entry(field).or_default();
        let mut count = 0;
        for tok in tokens {
            if tok.trim().is_empty() {
                continue;
            }
            *entry.entry(tok.clone()).or_insert(0) += 1;
            count += 1;
        }
        *self.field_lengths.entry(field).or_insert(0) += count;
        self.total_terms += count;
    }
}

/// Set of standard stop words in English and common programming languages.
fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "has"
            | "he"
            | "in"
            | "is"
            | "it"
            | "its"
            | "of"
            | "on"
            | "that"
            | "the"
            | "to"
            | "was"
            | "were"
            | "will"
            | "with"
            | "then"
            | "else"
            | "this"
            | "self"
            | "true"
            | "false"
            | "null"
            | "nil"
            | "undefined"
            | "var"
            | "let"
            | "const"
            | "function"
            | "fn"
            | "def"
            | "return"
            | "pub"
            | "impl"
            | "struct"
            | "class"
            | "interface"
            | "type"
            | "mut"
            | "ref"
            | "new"
    )
}

/// Tokenizes text splitting on identifiers (camelCase, PascalCase, snake_case, kebab-case),
/// punctuation, and whitespace, filtering stop words and retaining terms.
pub fn tokenize_nl_and_code(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_segment = String::new();

    for ch in input.chars() {
        if ch.is_alphanumeric() {
            current_segment.push(ch);
        } else {
            if !current_segment.is_empty() {
                split_identifier(&current_segment, &mut tokens);
                current_segment.clear();
            }
        }
    }
    if !current_segment.is_empty() {
        split_identifier(&current_segment, &mut tokens);
    }

    tokens
}

/// Splits camelCase, PascalCase, snake_case, and acronyms into sub-tokens.
fn split_identifier(ident: &str, out: &mut Vec<String>) {
    if ident.is_empty() {
        return;
    }

    for segment in ident.split(['_', '-', '.', ':']) {
        if segment.is_empty() {
            continue;
        }

        let raw_lower = segment.to_lowercase();
        let chars: Vec<char> = segment.chars().collect();
        let len = chars.len();

        let mut parts: Vec<String> = Vec::new();
        let mut start = 0;

        for i in 0..len {
            let is_upper = chars[i].is_uppercase();
            let prev_is_lower = i > 0 && chars[i - 1].is_lowercase();
            let next_is_lower = i + 1 < len && chars[i + 1].is_lowercase();
            let prev_is_upper = i > 0 && chars[i - 1].is_uppercase();

            // Boundary conditions:
            // 1. lowerToUpper: e.g. "camelCase" -> split before 'C'
            // 2. acronymToLower: e.g. "JWTToken" -> split before 'T' (between T and o)
            if ((is_upper && prev_is_lower) || (is_upper && prev_is_upper && next_is_lower))
                && i > start
            {
                let part: String = chars[start..i].iter().collect();
                parts.push(part.to_lowercase());
                start = i;
            }
        }

        if start < len {
            let part: String = chars[start..len].iter().collect();
            parts.push(part.to_lowercase());
        }

        // Add whole segment (lowercase) if it has >1 part and is not a stop word
        if parts.len() > 1 && raw_lower.len() >= 2 && !is_stop_word(&raw_lower) {
            out.push(raw_lower);
        }

        // Add each sub-part if >= 2 characters and not a stop word
        for part in parts {
            if part.len() >= 2 && !is_stop_word(&part) {
                out.push(part);
            }
        }
    }
}

/// Extracts multi-field token document for a symbol declaration.
pub fn extract_symbol_tokens(
    name: &str,
    signature: &str,
    doc_comment: Option<&str>,
    file_path: &str,
    body: &str,
) -> SymbolTokenDocument {
    let mut doc = SymbolTokenDocument::new();

    let name_tokens = tokenize_nl_and_code(name);
    doc.add_field_tokens(FieldKind::Name, &name_tokens);

    let sig_tokens = tokenize_nl_and_code(signature);
    doc.add_field_tokens(FieldKind::Signature, &sig_tokens);

    if let Some(doc_str) = doc_comment {
        let doc_tokens = tokenize_nl_and_code(doc_str);
        doc.add_field_tokens(FieldKind::Docstring, &doc_tokens);
    }

    let path_tokens = tokenize_nl_and_code(file_path);
    doc.add_field_tokens(FieldKind::Path, &path_tokens);

    let body_tokens = tokenize_nl_and_code(body);
    doc.add_field_tokens(FieldKind::Body, &body_tokens);

    doc
}

/// Extracts unique search keywords from a user query prompt.
pub fn extract_query_keywords(prompt: &str) -> Vec<String> {
    let tokens = tokenize_nl_and_code(prompt);
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for tok in tokens {
        if seen.insert(tok.clone()) {
            result.push(tok);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_camel_and_snake_case() {
        let mut tokens = Vec::new();
        split_identifier("validateJwtToken", &mut tokens);
        assert!(tokens.contains(&"validate".to_string()));
        assert!(tokens.contains(&"jwt".to_string()));
        assert!(tokens.contains(&"token".to_string()));
        assert!(tokens.contains(&"validatejwttoken".to_string()));

        let mut tokens2 = Vec::new();
        split_identifier("reserve_inventory_stock", &mut tokens2);
        assert!(tokens2.contains(&"reserve".to_string()));
        assert!(tokens2.contains(&"inventory".to_string()));
        assert!(tokens2.contains(&"stock".to_string()));
    }

    #[test]
    fn test_extract_query_keywords() {
        let kw = extract_query_keywords("Find validateJwtToken function and check UserSession expiresAt");
        assert!(kw.contains(&"validate".to_string()));
        assert!(kw.contains(&"jwt".to_string()));
        assert!(kw.contains(&"token".to_string()));
        assert!(kw.contains(&"usersession".to_string()));
        assert!(kw.contains(&"user".to_string()));
        assert!(kw.contains(&"session".to_string()));
        assert!(kw.contains(&"expiresat".to_string()));
    }
}
