//! Criterion benchmark for AST node location and symbol body extraction throughput.
//!
//! Measures latency and throughput for locating symbol AST nodes by name (functions,
//! methods, classes, structs) and extracting exact AST node bodies across languages.

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

fn find_symbol_by_query<'a>(
    root: Node<'a>,
    source: &'a [u8],
    query: &Query,
    target_name: &str,
) -> Option<(usize, usize)> {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, root, source);

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let captured_text = &source[capture.node.byte_range()];
            if captured_text == target_name.as_bytes() {
                // Return parent function node byte range
                if let Some(parent) = capture.node.parent() {
                    return Some((parent.start_byte(), parent.end_byte()));
                }
            }
        }
    }
    None
}

/// Generates a TypeScript monolith with multiple functions.
fn generate_ts_suite(count: usize) -> (String, Vec<String>) {
    let mut code = String::new();
    let mut names = Vec::new();

    for i in 0..count {
        let name = format!("executeTask_{i}");
        code.push_str(&format!(
            "export function {name}(id: string, retries: number): boolean {{\n\
             \x20   console.log(`Executing ${{id}} with retries ${{retries}}`);\n\
             \x20   if (retries <= 0) return false;\n\
             \x20   return true;\n\
             }}\n\n"
        ));
        names.push(name);
    }
    (code, names)
}

/// Generates a Rust monolith with multiple functions.
fn generate_rust_suite(count: usize) -> (String, Vec<String>) {
    let mut code = String::new();
    let mut names = Vec::new();

    for i in 0..count {
        let name = format!("process_record_{i}");
        code.push_str(&format!(
            "pub fn {name}(key: &str, count: u32) -> Result<bool, &'static str> {{\n\
             \x20   if key.is_empty() {{\n\
             \x20       return Err(\"Key is empty\");\n\
             \x20   }}\n\
             \x20   Ok(count > 0)\n\
             }}\n\n"
        ));
        names.push(name);
    }
    (code, names)
}

fn bench_ast_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("ast_symbol_extraction");

    // TypeScript Extraction Benchmark
    {
        let (ts_code, ts_symbols) = generate_ts_suite(200);
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        parser.set_language(&lang).expect("Error loading TypeScript grammar");
        let tree = parser.parse(&ts_code, None).expect("Parse error");
        let query = Query::new(&lang, "(function_declaration name: (identifier) @name)")
            .expect("Query compilation failed");

        let target_first = &ts_symbols[0];
        let target_mid = &ts_symbols[ts_symbols.len() / 2];
        let target_last = &ts_symbols[ts_symbols.len() - 1];

        group.throughput(Throughput::Bytes(ts_code.len() as u64));

        for &(pos, target) in &[
            ("first", target_first),
            ("middle", target_mid),
            ("last", target_last),
        ] {
            group.bench_with_input(
                BenchmarkId::new("ts_extract_location", pos),
                target,
                |b, target_name| {
                    b.iter(|| {
                        let res = find_symbol_by_query(
                            tree.root_node(),
                            ts_code.as_bytes(),
                            &query,
                            black_box(target_name),
                        );
                        black_box(res).expect("Symbol must be found");
                    });
                },
            );
        }
    }

    // Rust Extraction Benchmark
    {
        let (rs_code, rs_symbols) = generate_rust_suite(200);
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&lang).expect("Error loading Rust grammar");
        let tree = parser.parse(&rs_code, None).expect("Parse error");
        let query = Query::new(&lang, "(function_item name: (identifier) @name)")
            .expect("Query compilation failed");

        let target_first = &rs_symbols[0];
        let target_mid = &rs_symbols[rs_symbols.len() / 2];
        let target_last = &rs_symbols[rs_symbols.len() - 1];

        group.throughput(Throughput::Bytes(rs_code.len() as u64));

        for &(pos, target) in &[
            ("first", target_first),
            ("middle", target_mid),
            ("last", target_last),
        ] {
            group.bench_with_input(
                BenchmarkId::new("rust_extract_location", pos),
                target,
                |b, target_name| {
                    b.iter(|| {
                        let res = find_symbol_by_query(
                            tree.root_node(),
                            rs_code.as_bytes(),
                            &query,
                            black_box(target_name),
                        );
                        black_box(res).expect("Symbol must be found");
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_ast_extraction);
criterion_main!(benches);
