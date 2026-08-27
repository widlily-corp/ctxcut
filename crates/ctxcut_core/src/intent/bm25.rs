//! BM25 and BM25F lexical ranking algorithm for code and documentation tokens.

use super::tokenizer::{FieldKind, SymbolTokenDocument};
use std::collections::HashMap;

/// Parameters for BM25 ranking.
#[derive(Debug, Clone, Copy)]
pub struct Bm25Params {
    /// Term frequency saturation parameter (default: 1.2).
    pub k1: f64,
    /// Length normalization parameter (default: 0.75).
    pub b: f64,
}

impl Default for Bm25Params {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

/// Computes Okapi BM25 Inverse Document Frequency (IDF) score.
///
/// # Arguments
/// * `total_docs` - Total number of documents $N$ in corpus.
/// * `doc_freq` - Number of documents $n$ containing the term.
#[inline]
pub fn compute_idf(total_docs: usize, doc_freq: usize) -> f64 {
    if total_docs == 0 || doc_freq == 0 {
        return 0.0;
    }
    let n = doc_freq as f64;
    let total = total_docs as f64;
    // Standard Lucene / BM25 IDF: ln(1 + (N - n + 0.5) / (n + 0.5))
    let val = ((total - n + 0.5) / (n + 0.5)) + 1.0;
    val.ln().max(0.0)
}

/// Inverted posting item for a term occurrence in a document field.
#[derive(Debug, Clone)]
pub struct Posting {
    /// Document index or ID.
    pub doc_id: usize,
    /// Field kind.
    pub field: FieldKind,
    /// Frequency of the term in this field.
    pub term_freq: usize,
    /// Total length of the field.
    pub field_length: usize,
}

/// High-performance in-memory BM25 index.
#[derive(Debug, Clone, Default)]
pub struct Bm25Index {
    /// BM25 tuning parameters.
    pub params: Bm25Params,
    /// Inverted index: term -> list of postings.
    pub postings: HashMap<String, Vec<Posting>>,
    /// Document frequency per term: term -> number of unique documents containing term.
    pub doc_freqs: HashMap<String, usize>,
    /// Field length averages: FieldKind -> average length across all documents.
    pub avg_field_lengths: HashMap<FieldKind, f64>,
    /// Total number of documents indexed.
    pub total_docs: usize,
    /// Total document lengths: doc_id -> total terms across all fields.
    pub doc_lengths: Vec<usize>,
}

impl Bm25Index {
    /// Creates a new empty `Bm25Index`.
    pub fn new(params: Bm25Params) -> Self {
        Self {
            params,
            postings: HashMap::new(),
            doc_freqs: HashMap::new(),
            avg_field_lengths: HashMap::new(),
            total_docs: 0,
            doc_lengths: Vec::new(),
        }
    }

    /// Builds index from a slice of token documents.
    pub fn build_from_documents(docs: &[SymbolTokenDocument], params: Bm25Params) -> Self {
        let mut index = Self::new(params);
        index.total_docs = docs.len();
        index.doc_lengths.resize(docs.len(), 0);

        let mut total_field_lengths: HashMap<FieldKind, usize> = HashMap::new();

        for (doc_id, doc) in docs.iter().enumerate() {
            index.doc_lengths[doc_id] = doc.total_terms;

            let mut doc_seen_terms = HashMap::new();

            for (&field, terms) in &doc.field_term_freqs {
                let field_len = doc.field_lengths.get(&field).copied().unwrap_or(0);
                *total_field_lengths.entry(field).or_insert(0) += field_len;

                for (term, &freq) in terms {
                    index.postings.entry(term.clone()).or_default().push(Posting {
                        doc_id,
                        field,
                        term_freq: freq,
                        field_length: field_len,
                    });
                    *doc_seen_terms.entry(term.clone()).or_insert(0) += 1;
                }
            }

            for term in doc_seen_terms.keys() {
                *index.doc_freqs.entry(term.clone()).or_insert(0) += 1;
            }
        }

        if index.total_docs > 0 {
            for (field, total_len) in total_field_lengths {
                let avg = total_len as f64 / index.total_docs as f64;
                index.avg_field_lengths.insert(field, avg.max(1.0));
            }
        }

        index
    }

    /// Calculates BM25 score for a specific document against query terms.
    pub fn score_document(&self, doc_id: usize, query_terms: &[String]) -> f64 {
        if self.total_docs == 0 || doc_id >= self.total_docs {
            return 0.0;
        }

        let mut total_score = 0.0;

        for term in query_terms {
            let Some(postings) = self.postings.get(term) else {
                continue;
            };
            let doc_freq = self.doc_freqs.get(term).copied().unwrap_or(0);
            if doc_freq == 0 {
                continue;
            }

            let idf = compute_idf(self.total_docs, doc_freq);

            // Compute field-weighted normalized TF: \tilde{tf}
            let mut weighted_tf = 0.0;
            for p in postings {
                if p.doc_id == doc_id {
                    let avg_len = self
                        .avg_field_lengths
                        .get(&p.field)
                        .copied()
                        .unwrap_or(1.0);
                    let field_len = p.field_length as f64;
                    let norm = 1.0 - self.params.b + self.params.b * (field_len / avg_len);
                    let tf_norm = if norm > 0.0 {
                        p.term_freq as f64 / norm
                    } else {
                        p.term_freq as f64
                    };
                    weighted_tf += p.field.weight() * tf_norm;
                }
            }

            if weighted_tf > 0.0 {
                let num = weighted_tf * (self.params.k1 + 1.0);
                let denom = weighted_tf + self.params.k1;
                total_score += idf * (num / denom);
            }
        }

        total_score
    }

    /// Ranks all documents in index against query terms, returning sorted (doc_id, score) pairs.
    pub fn rank(&self, query_terms: &[String]) -> Vec<(usize, f64)> {
        let mut scores = Vec::with_capacity(self.total_docs);
        for doc_id in 0..self.total_docs {
            let score = self.score_document(doc_id, query_terms);
            if score > 0.0 {
                scores.push((doc_id, score));
            }
        }
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::tokenizer::extract_symbol_tokens;

    #[test]
    fn test_bm25_scoring_prioritizes_exact_symbol_match() {
        let doc1 = extract_symbol_tokens(
            "validateJwtToken",
            "export function validateJwtToken(token: string): JwtTokenPayload | null",
            Some("Validates incoming JWT bearer tokens."),
            "auth/jwt.ts",
            "if (!token) return null;",
        );

        let doc2 = extract_symbol_tokens(
            "hashPassword",
            "export function hashPassword(plain: string): string",
            Some("Hashes passwords."),
            "auth/crypto.ts",
            "return sha256(plain);",
        );

        let docs = vec![doc1, doc2];
        let index = Bm25Index::build_from_documents(&docs, Bm25Params::default());

        let query = vec!["validate".to_string(), "jwt".to_string(), "token".to_string()];
        let ranks = index.rank(&query);

        assert!(!ranks.is_empty());
        assert_eq!(ranks[0].0, 0, "validateJwtToken should rank highest");
        assert!(ranks[0].1 > 0.0);
    }
}
