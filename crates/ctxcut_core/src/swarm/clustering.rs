//! Community clustering and graph partitioning engine for multi-agent swarm context partitioning.
//!
//! Provides:
//! - Louvain-style modularity optimization
//! - Seeded min-cut / multi-way balanced partitioning
//! - Connected components clustering with token-balancing heuristics
//! - Strict disjoint non-overlapping partition guarantee

use super::graph::WorkspaceGraph;
use std::collections::{HashMap, HashSet};

/// Community clustering engine that partitions the workspace dependency graph into $K$ non-overlapping clusters.
pub struct CommunityClusterer;

impl CommunityClusterer {
    /// Partitions the given `WorkspaceGraph` into $K$ disjoint, non-overlapping symbol clusters.
    pub fn cluster(
        graph: &WorkspaceGraph,
        agents_count: usize,
        seed_symbols: &[String],
    ) -> Vec<Vec<String>> {
        let total_nodes = graph.nodes.len();
        if total_nodes == 0 {
            return Vec::new();
        }

        let mut node_ids: Vec<String> = graph.nodes.keys().cloned().collect();
        node_ids.sort();

        if agents_count <= 1 || total_nodes == 1 {
            return vec![node_ids];
        }

        let k = agents_count.min(total_nodes);

        if !seed_symbols.is_empty() {
            seeded_min_cut_partition(graph, k, seed_symbols, &node_ids)
        } else {
            louvain_modularity_partition(graph, k, &node_ids)
        }
    }
}

/// Symmetrized edge weight lookup between node `u` and node `v`.
fn edge_weight(graph: &WorkspaceGraph, u: &str, v: &str) -> f64 {
    let mut w = 0.0;
    if let Some(list) = graph.outgoing.get(u) {
        for (target, weight, _) in list {
            if target == v {
                w += *weight;
            }
        }
    }
    if let Some(list) = graph.outgoing.get(v) {
        for (target, weight, _) in list {
            if target == u {
                w += *weight;
            }
        }
    }
    w
}

/// Seeded min-cut / multi-way balanced graph partitioning algorithm.
fn seeded_min_cut_partition(
    graph: &WorkspaceGraph,
    k: usize,
    seed_symbols: &[String],
    all_node_ids: &[String],
) -> Vec<Vec<String>> {
    let mut resolved_seed_nodes: Vec<String> = Vec::new();
    let mut seen_seeds = HashSet::new();

    for seed in seed_symbols {
        let matched = graph.find_nodes_by_seed(seed);
        for node in matched {
            if !seen_seeds.contains(&node.id) {
                seen_seeds.insert(node.id.clone());
                resolved_seed_nodes.push(node.id.clone());
            }
        }
    }

    let mut clusters: Vec<Vec<String>> = vec![Vec::new(); k];
    let mut assigned: HashSet<String> = HashSet::new();

    // 1. Initialize clusters with seed nodes
    let seed_count = resolved_seed_nodes.len();
    for (i, seed_node) in resolved_seed_nodes.iter().enumerate() {
        let cluster_idx = if i < k { i } else { i % k };
        clusters[cluster_idx].push(seed_node.clone());
        assigned.insert(seed_node.clone());
    }

    // 2. If seed count < k, initialize remaining empty clusters with unassigned nodes
    if seed_count < k {
        let mut unassigned_candidates: Vec<String> = all_node_ids
            .iter()
            .filter(|id| !assigned.contains(*id))
            .cloned()
            .collect();

        // Sort by degree descending to pick prominent anchor nodes
        unassigned_candidates.sort_by(|a, b| {
            let deg_a = graph.node_degree(a);
            let deg_b = graph.node_degree(b);
            deg_b.partial_cmp(&deg_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut candidate_iter = unassigned_candidates.into_iter();
        for cluster in clusters.iter_mut().take(k).skip(seed_count) {
            if let Some(candidate) = candidate_iter.next() {
                assigned.insert(candidate.clone());
                cluster.push(candidate);
            }
        }
    }

    let total_tokens: usize = graph.nodes.values().map(|n| n.token_count).sum();
    let target_tokens_per_cluster = (total_tokens / k.max(1)).max(1);

    // 3. Greedily assign remaining unassigned nodes to the best cluster
    let mut unassigned: Vec<String> = all_node_ids
        .iter()
        .filter(|id| !assigned.contains(*id))
        .cloned()
        .collect();

    // Sort unassigned nodes by degree descending
    unassigned.sort_by(|a, b| {
        let deg_a = graph.node_degree(a);
        let deg_b = graph.node_degree(b);
        deg_b.partial_cmp(&deg_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    for node_id in unassigned {
        let node_tokens = graph
            .nodes
            .get(&node_id)
            .map(|n| n.token_count)
            .unwrap_or(10);

        let mut best_cluster = 0;
        let mut best_score = f64::NEG_INFINITY;

        for (c_idx, cluster) in clusters.iter().enumerate() {
            let mut connection_weight = 0.0;
            let mut cluster_tokens = 0;

            for member in cluster {
                connection_weight += edge_weight(graph, &node_id, member);
                if let Some(member_node) = graph.nodes.get(member) {
                    cluster_tokens += member_node.token_count;
                }
            }

            // Penalize clusters that exceed the balanced token target
            let size_ratio = (cluster_tokens + node_tokens) as f64 / target_tokens_per_cluster as f64;
            let size_penalty = if size_ratio > 1.2 {
                (size_ratio - 1.2) * 5.0
            } else {
                0.0
            };

            let score = connection_weight - size_penalty;

            if score > best_score {
                best_score = score;
                best_cluster = c_idx;
            }
        }

        // If no connections found, assign to the smallest cluster
        if best_score <= 0.0 {
            let mut min_size = usize::MAX;
            let mut min_cluster = 0;
            for (c_idx, cluster) in clusters.iter().enumerate() {
                let cluster_tokens: usize = cluster
                    .iter()
                    .filter_map(|m| graph.nodes.get(m).map(|n| n.token_count))
                    .sum();
                if cluster_tokens < min_size {
                    min_size = cluster_tokens;
                    min_cluster = c_idx;
                }
            }
            best_cluster = min_cluster;
        }

        clusters[best_cluster].push(node_id);
    }

    // 4. Refinement pass: Kernighan-Lin style boundary swapping
    let anchored_seeds: HashSet<String> = resolved_seed_nodes.into_iter().collect();
    refine_cluster_boundaries(graph, &mut clusters, target_tokens_per_cluster, &anchored_seeds);

    finalize_clusters(clusters)
}

/// Louvain-style modularity optimization partitioning algorithm.
fn louvain_modularity_partition(
    graph: &WorkspaceGraph,
    k: usize,
    all_node_ids: &[String],
) -> Vec<Vec<String>> {
    let total_nodes = all_node_ids.len();
    if total_nodes <= k {
        let mut result = Vec::new();
        for id in all_node_ids {
            result.push(vec![id.clone()]);
        }
        return result;
    }

    let total_weight = graph.total_weight().max(1.0);
    let two_m = 2.0 * total_weight;

    // Phase 1: Initialize each node in its own community
    let mut node_to_comm: HashMap<String, usize> = HashMap::new();
    let mut comm_to_nodes: HashMap<usize, HashSet<String>> = HashMap::new();
    let mut comm_tot_weight: HashMap<usize, f64> = HashMap::new();

    for (idx, node_id) in all_node_ids.iter().enumerate() {
        node_to_comm.insert(node_id.clone(), idx);
        let mut set = HashSet::new();
        set.insert(node_id.clone());
        comm_to_nodes.insert(idx, set);
        comm_tot_weight.insert(idx, graph.node_degree(node_id));
    }

    // Iterative modularity maximization
    for _ in 0..30 {
        let mut moved = false;

        for node_id in all_node_ids {
            let current_comm = *node_to_comm.get(node_id).unwrap();
            let k_i = graph.node_degree(node_id);

            // Remove node from current community
            comm_tot_weight
                .entry(current_comm)
                .and_modify(|w| *w -= k_i);
            comm_to_nodes
                .entry(current_comm)
                .and_modify(|set| { set.remove(node_id); });

            // Find candidate neighbor communities
            let mut neighbor_comms: HashSet<usize> = HashSet::new();
            if let Some(list) = graph.outgoing.get(node_id) {
                for (target, _, _) in list {
                    if let Some(target_comm) = node_to_comm.get(target) {
                        neighbor_comms.insert(*target_comm);
                    }
                }
            }
            if let Some(list) = graph.incoming.get(node_id) {
                for (source, _, _) in list {
                    if let Some(source_comm) = node_to_comm.get(source) {
                        neighbor_comms.insert(*source_comm);
                    }
                }
            }
            neighbor_comms.insert(current_comm);

            let mut best_comm = current_comm;
            let mut best_delta_q = 0.0;

            for cand_comm in neighbor_comms {
                let k_i_in: f64 = comm_to_nodes
                    .get(&cand_comm)
                    .map(|members| members.iter().map(|m| edge_weight(graph, node_id, m)).sum())
                    .unwrap_or(0.0);

                let sigma_tot = *comm_tot_weight.get(&cand_comm).unwrap_or(&0.0);
                let delta_q = (k_i_in / two_m) - (sigma_tot * k_i / (two_m * two_m));

                if delta_q > best_delta_q {
                    best_delta_q = delta_q;
                    best_comm = cand_comm;
                }
            }

            // Put node into best community
            comm_to_nodes
                .entry(best_comm)
                .or_default()
                .insert(node_id.clone());
            comm_tot_weight
                .entry(best_comm)
                .and_modify(|w| *w += k_i);
            node_to_comm.insert(node_id.clone(), best_comm);

            if best_comm != current_comm {
                moved = true;
            }
        }

        if !moved {
            break;
        }
    }

    // Extract non-empty communities
    let mut initial_clusters: Vec<Vec<String>> = comm_to_nodes
        .into_values()
        .filter(|set| !set.is_empty())
        .map(|set| {
            let mut list: Vec<String> = set.into_iter().collect();
            list.sort();
            list
        })
        .collect();

    // Sort clusters by size descending
    initial_clusters.sort_by_key(|b| std::cmp::Reverse(b.len()));

    // Phase 2: Merge or split to achieve exactly K clusters
    adjust_to_k_clusters(graph, initial_clusters, k)
}

/// Merges or splits clusters until exactly $K$ clusters are formed.
fn adjust_to_k_clusters(
    graph: &WorkspaceGraph,
    mut clusters: Vec<Vec<String>>,
    k: usize,
) -> Vec<Vec<String>> {
    let target_k = k.min(graph.nodes.len());

    // 1. If too many clusters, merge closest pairs
    while clusters.len() > target_k {
        let mut best_pair = (0, 1);
        let mut best_weight = f64::NEG_INFINITY;

        for i in 0..clusters.len() {
            for j in (i + 1)..clusters.len() {
                let mut mutual_w = 0.0;
                for u in &clusters[i] {
                    for v in &clusters[j] {
                        mutual_w += edge_weight(graph, u, v);
                    }
                }

                // If tied at 0 mutual weight, prefer merging smaller clusters
                let token_penalty = (clusters[i].len() + clusters[j].len()) as f64 * 0.01;
                let score = mutual_w - token_penalty;

                if score > best_weight {
                    best_weight = score;
                    best_pair = (i, j);
                }
            }
        }

        let (i, j) = best_pair;
        let merged_nodes = clusters.remove(j);
        clusters[i].extend(merged_nodes);
    }

    // 2. If too few clusters, split largest cluster
    while clusters.len() < target_k {
        let mut max_idx = 0;
        let mut max_len = 0;
        for (i, c) in clusters.iter().enumerate() {
            if c.len() > max_len {
                max_len = c.len();
                max_idx = i;
            }
        }

        if max_len < 2 {
            break;
        }

        let to_split = clusters.remove(max_idx);
        let (c1, c2) = bisect_cluster(graph, to_split);
        clusters.push(c1);
        clusters.push(c2);
    }

    let total_tokens: usize = graph.nodes.values().map(|n| n.token_count).sum();
    let target_tokens_per_cluster = (total_tokens / target_k.max(1)).max(1);

    refine_cluster_boundaries(graph, &mut clusters, target_tokens_per_cluster, &HashSet::new());
    finalize_clusters(clusters)
}

/// Bisects a cluster into two sub-clusters along its weakest cut.
fn bisect_cluster(graph: &WorkspaceGraph, nodes: Vec<String>) -> (Vec<String>, Vec<String>) {
    if nodes.len() <= 1 {
        return (nodes, Vec::new());
    }

    let mut c1 = Vec::new();
    let mut c2 = Vec::new();

    // Pick two seed nodes with lowest mutual connection
    let s1 = nodes[0].clone();
    let mut s2 = nodes[1].clone();
    let mut min_w = f64::INFINITY;

    for node in nodes.iter().skip(1) {
        let w = edge_weight(graph, &s1, node);
        if w < min_w {
            min_w = w;
            s2.clone_from(node);
        }
    }

    c1.push(s1.clone());
    c2.push(s2.clone());

    for node in nodes.into_iter().skip(1) {
        if node == s2 {
            continue;
        }
        let w1 = edge_weight(graph, &node, &s1);
        let w2 = edge_weight(graph, &node, &s2);
        if w1 > w2 {
            c1.push(node);
        } else if w2 > w1 {
            c2.push(node);
        } else if c1.len() <= c2.len() {
            c1.push(node);
        } else {
            c2.push(node);
        }
    }

    (c1, c2)
}

/// Refines cluster boundaries using iterative greedy boundary swapping.
fn refine_cluster_boundaries(
    graph: &WorkspaceGraph,
    clusters: &mut [Vec<String>],
    target_tokens: usize,
    anchored_seeds: &HashSet<String>,
) {
    let k = clusters.len();
    if k <= 1 {
        return;
    }

    for _ in 0..20 {
        let mut moved = false;

        for src_idx in 0..k {
            if clusters[src_idx].len() <= 1 {
                continue;
            }

            let mut node_to_move = None;
            let mut target_cluster = 0;

            for (node_pos, node_id) in clusters[src_idx].iter().enumerate() {
                if anchored_seeds.contains(node_id) {
                    continue;
                }
                let mut current_int_w = 0.0;
                for other in &clusters[src_idx] {
                    if other != node_id {
                        current_int_w += edge_weight(graph, node_id, other);
                    }
                }

                let src_tokens: usize = clusters[src_idx]
                    .iter()
                    .filter_map(|m| graph.nodes.get(m).map(|n| n.token_count))
                    .sum();
                let node_tokens = graph.nodes.get(node_id).map(|n| n.token_count).unwrap_or(10);

                for (dst_idx, dst_cluster) in clusters.iter().enumerate().take(k) {
                    if dst_idx == src_idx {
                        continue;
                    }

                    let mut ext_w = 0.0;
                    let dst_tokens: usize = dst_cluster
                        .iter()
                        .filter_map(|m| graph.nodes.get(m).map(|n| n.token_count))
                        .sum();

                    for other in dst_cluster {
                        ext_w += edge_weight(graph, node_id, other);
                    }

                    let balance_diff = ((dst_tokens + node_tokens) as f64 - src_tokens as f64)
                        / target_tokens.max(1) as f64;
                    let gain = (ext_w - current_int_w) - (balance_diff * 2.0);

                    if gain > 0.5 && dst_tokens + node_tokens <= (target_tokens * 13) / 10 {
                        node_to_move = Some(node_pos);
                        target_cluster = dst_idx;
                        break;
                    }
                }

                if node_to_move.is_some() {
                    break;
                }
            }

            if let Some(pos) = node_to_move {
                let node_id = clusters[src_idx].remove(pos);
                clusters[target_cluster].push(node_id);
                moved = true;
            }
        }

        if !moved {
            break;
        }
    }
}

/// Filters empty clusters, sorts inner nodes, and sorts clusters deterministically.
fn finalize_clusters(clusters: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut valid: Vec<Vec<String>> = clusters
        .into_iter()
        .filter(|c| !c.is_empty())
        .map(|mut c| {
            c.sort();
            c
        })
        .collect();

    valid.sort_by(|a, b| {
        let a_first = a.first().map(|s| s.as_str()).unwrap_or("");
        let b_first = b.first().map(|s| s.as_str()).unwrap_or("");
        a_first.cmp(b_first)
    });

    valid
}
