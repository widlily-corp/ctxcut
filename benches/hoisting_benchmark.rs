//! Criterion benchmark for type hoisting and dependency resolution.
//!
//! Measures performance of AST scope walking, transitive type dependency resolution,
//! and signature stripping on complex real-world type graphs across languages.

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::{HashMap, HashSet};

/// Mock representation of an AST symbol for benchmarking graph traversal.
#[derive(Debug, Clone)]
struct MockAstType {
    name: String,
    definition: String,
    referenced_types: Vec<String>,
}

/// In-memory type graph for benchmarking dependency resolution algorithms.
struct MockTypeResolver {
    types: HashMap<String, MockAstType>,
}

impl MockTypeResolver {
    fn new(count: usize, fanout: usize) -> Self {
        let mut types = HashMap::with_capacity(count);
        for i in 0..count {
            let name = format!("Type_{i}");
            let mut referenced_types = Vec::new();
            for f in 1..=fanout {
                if i + f < count {
                    referenced_types.push(format!("Type_{}", i + f));
                }
            }
            types.insert(
                name.clone(),
                MockAstType {
                    name: name.clone(),
                    definition: format!("export interface {} {{ field: string; }}", name),
                    referenced_types,
                },
            );
        }
        Self { types }
    }

    /// Recursively hoists referenced types up to `max_depth`.
    fn hoist_types(&self, root_types: &[&str], max_depth: usize) -> Vec<MockAstType> {
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        let mut current_level: Vec<String> = root_types.iter().map(|s| s.to_string()).collect();

        for _ in 0..max_depth {
            if current_level.is_empty() {
                break;
            }
            let mut next_level = Vec::new();
            for type_name in current_level {
                if visited.insert(type_name.clone()) {
                    if let Some(ast_type) = self.types.get(&type_name) {
                        result.push(ast_type.clone());
                        for child in &ast_type.referenced_types {
                            if !visited.contains(child) {
                                next_level.push(child.clone());
                            }
                        }
                    }
                }
            }
            current_level = next_level;
        }

        result
    }
}

fn bench_hoisting_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("type_hoisting_resolution");

    let graph_sizes = [50, 200, 1000];
    let depths = [1, 2, 3, 5];

    for &size in &graph_sizes {
        let resolver = MockTypeResolver::new(size, 3);
        group.throughput(Throughput::Elements(size as u64));

        for &depth in &depths {
            let start_nodes = ["Type_0", "Type_1", "Type_2"];
            group.bench_with_input(
                BenchmarkId::new(format!("size_{size}"), format!("depth_{depth}")),
                &depth,
                |b, &d| {
                    b.iter(|| {
                        let hoisted = resolver.hoist_types(black_box(&start_nodes), black_box(d));
                        black_box(hoisted);
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_hoisting_resolution);
criterion_main!(benches);
