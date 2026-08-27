//! Hybrid AST scoring engine combining BM25 lexical relevance, AST degree centrality, and graph proximity.

use crate::model::ExtractedSymbol;
use std::collections::{HashMap, HashSet};

/// Candidate scored symbol with hybrid ranking metrics.
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    /// Extracted symbol AST node.
    pub symbol: ExtractedSymbol,
    /// Raw BM25 lexical score.
    pub bm25_score: f64,
    /// Normalized BM25 relevance score (0.0 to 1.0).
    pub bm25_norm: f64,
    /// AST graph degree centrality (in-degree + out-degree).
    pub degree_centrality: f64,
    /// AST dependency proximity to seed matches (0.0 to 1.0).
    pub proximity: f64,
    /// Final composite hybrid score.
    pub final_score: f64,
}

/// Hybrid AST Ranker.
#[derive(Debug, Clone)]
pub struct HybridAstRanker {
    /// Weight for BM25 lexical relevance (default: 0.65).
    pub bm25_weight: f64,
    /// Weight for AST degree centrality (default: 0.20).
    pub centrality_weight: f64,
    /// Weight for AST graph proximity (default: 0.15).
    pub proximity_weight: f64,
}

impl Default for HybridAstRanker {
    fn default() -> Self {
        Self {
            bm25_weight: 0.65,
            centrality_weight: 0.20,
            proximity_weight: 0.15,
        }
    }
}

impl HybridAstRanker {
    /// Creates a new `HybridAstRanker` with default weights.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ranks candidates combining BM25 scores, call-graph degree centrality, and dependency proximity.
    ///
    /// # Arguments
    /// * `symbols` - List of candidate symbols.
    /// * `bm25_scores` - Map from symbol index to raw BM25 score.
    /// * `caller_counts` - Map from symbol name to count of callers (in-degree).
    /// * `call_dependencies` - Map from symbol name to set of outgoing called symbol names (out-degree).
    /// * `type_dependencies` - Map from symbol name to set of referenced types.
    pub fn rank(
        &self,
        symbols: &[ExtractedSymbol],
        bm25_scores: &HashMap<usize, f64>,
        caller_counts: &HashMap<String, usize>,
        call_dependencies: &HashMap<String, HashSet<String>>,
        type_dependencies: &HashMap<String, HashSet<String>>,
    ) -> Vec<ScoredCandidate> {
        if symbols.is_empty() {
            return Vec::new();
        }

        let max_bm25 = bm25_scores
            .values()
            .copied()
            .fold(0.0f64, |acc, v| acc.max(v));

        // Find top BM25 matches as "seed targets" (BM25 score >= 0.5 * max_bm25)
        let mut seed_names = HashSet::new();
        for (&idx, &score) in bm25_scores {
            if max_bm25 > 0.0 && score >= 0.5 * max_bm25 {
                if let Some(sym) = symbols.get(idx) {
                    seed_names.insert(sym.name.clone());
                }
            }
        }

        let mut candidates = Vec::with_capacity(symbols.len());

        for (idx, sym) in symbols.iter().enumerate() {
            let raw_bm25 = bm25_scores.get(&idx).copied().unwrap_or(0.0);
            let bm25_norm = if max_bm25 > 0.0 {
                (raw_bm25 / max_bm25).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Calculate AST Degree Centrality
            let in_degree = caller_counts.get(&sym.name).copied().unwrap_or(0);
            let out_degree = call_dependencies
                .get(&sym.name)
                .map(|set| set.len())
                .unwrap_or(0);
            let total_degree = in_degree + out_degree;
            let degree_centrality = ((total_degree as f64 + 1.0).ln() / (10.0f64).ln()).clamp(0.0, 1.0);

            // Calculate AST Graph Proximity to Seed Targets
            let proximity = if seed_names.contains(&sym.name) {
                1.0
            } else {
                let mut max_prox = 0.0f64;
                for seed in &seed_names {
                    // Check if sym calls seed or is called by seed
                    if let Some(calls) = call_dependencies.get(seed) {
                        if calls.contains(&sym.name) {
                            max_prox = max_prox.max(0.8);
                        }
                    }
                    if let Some(calls) = call_dependencies.get(&sym.name) {
                        if calls.contains(seed) {
                            max_prox = max_prox.max(0.8);
                        }
                    }
                    // Check if sym references seed type
                    if let Some(types) = type_dependencies.get(seed) {
                        if types.contains(&sym.name) {
                            max_prox = max_prox.max(0.75);
                        }
                    }
                }
                max_prox
            };

            let final_score = self.bm25_weight * bm25_norm
                + self.centrality_weight * degree_centrality
                + self.proximity_weight * proximity;

            candidates.push(ScoredCandidate {
                symbol: sym.clone(),
                bm25_score: raw_bm25,
                bm25_norm,
                degree_centrality,
                proximity,
                final_score,
            });
        }

        // Sort descending by final hybrid score
        candidates.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates
    }
}
