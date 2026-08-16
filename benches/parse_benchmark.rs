//! Criterion benchmark for Tree-sitter AST parsing latency per language.
//!
//! Measures parse latency and throughput across TypeScript, Python, Go, and Rust
//! for small (500 LOC), medium (2,000 LOC), and large (10,000 LOC) source files.

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use tree_sitter::Parser;

/// Generates synthetic TypeScript source code of approximately `target_lines` LOC.
fn generate_typescript_source(target_lines: usize) -> String {
    let mut code = String::with_capacity(target_lines * 40);
    code.push_str("import { Request, Response } from 'express';\n");
    code.push_str("import { DbClient } from './db';\n\n");

    let items = target_lines / 20;
    for i in 0..items {
        code.push_str(&format!(
            "export interface UserRecord_{i} {{\n\
             \x20   id: string;\n\
             \x20   index: number;\n\
             \x20   username: string;\n\
             \x20   email: string;\n\
             \x20   isActive: boolean;\n\
             \x20   metadata: Record<string, unknown>;\n\
             }}\n\n\
             export async function handleUserAction_{i}(\n\
             \x20   req: Request,\n\
             \x20   res: Response,\n\
             \x20   db: DbClient\n\
             ): Promise<UserRecord_{i}> {{\n\
             \x20   const userId = req.params.id;\n\
             \x20   const record = await db.query<UserRecord_{i}>(userId);\n\
             \x20   if (!record) {{\n\
             \x20       throw new Error(`Record not found: ${{userId}}`);\n\
             \x20   }}\n\
             \x20   return record;\n\
             }}\n\n"
        ));
    }
    code
}

/// Generates synthetic Python source code of approximately `target_lines` LOC.
fn generate_python_source(target_lines: usize) -> String {
    let mut code = String::with_capacity(target_lines * 40);
    code.push_str("from typing import List, Optional, Dict, Any\n");
    code.push_str("from pydantic import BaseModel, Field\n\n");

    let items = target_lines / 18;
    for i in 0..items {
        code.push_str(&format!(
            "class ItemModel_{i}(BaseModel):\n\
             \x20   item_id: str = Field(..., description='Unique ID')\n\
             \x20   seq_no: int\n\
             \x20   payload: Dict[str, Any]\n\
             \x20   tags: List[str]\n\
             \x20   is_active: bool = True\n\n\
             async def process_item_{i}(\n\
             \x20   item: ItemModel_{i},\n\
             \x20   db_session: Any,\n\
             \x20   dry_run: bool = False\n\
             ) -> Optional[ItemModel_{i}]:\n\
             \x20   if not item.is_active:\n\
             \x20       return None\n\
             \x20   validated = await db_session.validate(item.item_id)\n\
             \x20   return validated\n\n"
        ));
    }
    code
}

/// Generates synthetic Go source code of approximately `target_lines` LOC.
fn generate_go_source(target_lines: usize) -> String {
    let mut code = String::with_capacity(target_lines * 40);
    code.push_str("package service\n\n");
    code.push_str("import (\n\t\"context\"\n\t\"fmt\"\n)\n\n");

    let items = target_lines / 18;
    for i in 0..items {
        code.push_str(&format!(
            "type SessionData_{i} struct {{\n\
             \x20   SessionID string `json:\"session_id\"`\n\
             \x20   UserID    int64  `json:\"user_id\"`\n\
             \x20   Token     string `json:\"token\"`\n\
             \x20   Valid     bool   `json:\"valid\"`\n\
             }}\n\n\
             func (s *SessionData_{i}) Authenticate(ctx context.Context, key string) (bool, error) {{\n\
             \x20   if !s.Valid {{\n\
             \x20       return false, fmt.Errorf(\"session %s is invalid\", s.SessionID)\n\
             \x20   }}\n\
             \x20   return s.Token == key, nil\n\
             }}\n\n"
        ));
    }
    code
}

/// Generates synthetic Rust source code of approximately `target_lines` LOC.
fn generate_rust_source(target_lines: usize) -> String {
    let mut code = String::with_capacity(target_lines * 40);
    code.push_str("use std::collections::HashMap;\n");
    code.push_str("use std::sync::Arc;\n\n");

    let items = target_lines / 20;
    for i in 0..items {
        code.push_str(&format!(
            "#[derive(Debug, Clone, PartialEq)]\n\
             pub struct InventoryItem_{i} {{\n\
             \x20   pub sku: String,\n\
             \x20   pub count: u64,\n\
             \x20   pub tags: Vec<String>,\n\
             \x20   pub attributes: HashMap<String, String>,\n\
             }}\n\n\
             impl InventoryItem_{i} {{\n\
             \x20   pub fn update_stock(&mut self, delta: i64) -> Result<u64, String> {{\n\
             \x20       if delta < 0 && self.count < (-delta as u64) {{\n\
             \x20           return Err(\"Insufficient stock\".into());\n\
             \x20       }}\n\
             \x20       self.count = (self.count as i64 + delta) as u64;\n\
             \x20       Ok(self.count)\n\
             \x20   }}\n\
             }}\n\n"
        ));
    }
    code
}

fn bench_tree_sitter_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_sitter_ast_parse");
    let loc_scales = [500, 2000, 10000];

    // Benchmark TypeScript
    for &loc in &loc_scales {
        let ts_source = generate_typescript_source(loc);
        group.throughput(Throughput::Bytes(ts_source.len() as u64));

        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        parser.set_language(&lang).expect("Error loading TypeScript grammar");

        group.bench_with_input(
            BenchmarkId::new("typescript", format!("{}_loc", loc)),
            &ts_source,
            |b, source| {
                b.iter(|| {
                    let tree = parser.parse(black_box(source), None);
                    black_box(tree).expect("Parse failure");
                });
            },
        );
    }

    // Benchmark Python
    for &loc in &loc_scales {
        let py_source = generate_python_source(loc);
        group.throughput(Throughput::Bytes(py_source.len() as u64));

        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
        parser.set_language(&lang).expect("Error loading Python grammar");

        group.bench_with_input(
            BenchmarkId::new("python", format!("{}_loc", loc)),
            &py_source,
            |b, source| {
                b.iter(|| {
                    let tree = parser.parse(black_box(source), None);
                    black_box(tree).expect("Parse failure");
                });
            },
        );
    }

    // Benchmark Go
    for &loc in &loc_scales {
        let go_source = generate_go_source(loc);
        group.throughput(Throughput::Bytes(go_source.len() as u64));

        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
        parser.set_language(&lang).expect("Error loading Go grammar");

        group.bench_with_input(
            BenchmarkId::new("go", format!("{}_loc", loc)),
            &go_source,
            |b, source| {
                b.iter(|| {
                    let tree = parser.parse(black_box(source), None);
                    black_box(tree).expect("Parse failure");
                });
            },
        );
    }

    // Benchmark Rust
    for &loc in &loc_scales {
        let rs_source = generate_rust_source(loc);
        group.throughput(Throughput::Bytes(rs_source.len() as u64));

        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&lang).expect("Error loading Rust grammar");

        group.bench_with_input(
            BenchmarkId::new("rust", format!("{}_loc", loc)),
            &rs_source,
            |b, source| {
                b.iter(|| {
                    let tree = parser.parse(black_box(source), None);
                    black_box(tree).expect("Parse failure");
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_tree_sitter_parsing);
criterion_main!(benches);
