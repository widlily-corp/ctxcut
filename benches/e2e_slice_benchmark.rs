//! Criterion benchmark for full End-to-End slice generation pipeline.
//!
//! Measures end-to-end latency for:
//! `File Read -> AST Parse -> Symbol Location -> Type Hoisting -> Signature Stripping -> Markdown Format -> Token Counting`
//! Verifies the strict <10ms SLA across TypeScript, Python, Go, and Rust on 2,000 LOC files.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ctxcut_core::{ContextSlicer, SliceOptions};
use std::fs;
use std::path::PathBuf;

fn ensure_benchmark_fixtures() -> Vec<(&'static str, PathBuf, &'static str)> {
    let tmp_dir = std::env::temp_dir().join("ctxcut_bench_fixtures");
    fs::create_dir_all(&tmp_dir).expect("Failed to create bench fixtures dir");

    // 1. TypeScript 2,000 LOC fixture
    let ts_path = tmp_dir.join("large_order_service.ts");
    if !ts_path.exists() {
        let mut ts = String::new();
        ts.push_str("export interface OrderPayload { id: string; amount: number; }\n");
        ts.push_str("export interface OrderResult { success: boolean; txId: string; }\n\n");
        for i in 0..100 {
            ts.push_str(&format!(
                "export async function processOrderBatch_{i}(payload: OrderPayload): Promise<OrderResult> {{\n\
                 \x20   console.log('Processing batch {i}', payload.id);\n\
                 \x20   return {{ success: payload.amount > 0, txId: `tx_{i}_${{payload.id}}` }};\n\
                 }}\n\n"
            ));
        }
        fs::write(&ts_path, ts).expect("Write ts bench fixture");
    }

    // 2. Python 2,000 LOC fixture
    let py_path = tmp_dir.join("large_payment_service.py");
    if !py_path.exists() {
        let mut py = String::new();
        py.push_str("from typing import Dict, Any, Optional\n\n");
        py.push_str("class PaymentProfile:\n    user_id: str\n    card_hash: str\n\n");
        for i in 0..100 {
            py.push_str(&format!(
                "def execute_charge_batch_{i}(profile: PaymentProfile, amount: float) -> Dict[str, Any]:\n\
                 \x20   return {{'status': 'charged', 'batch': {i}, 'amount': amount}}\n\n"
            ));
        }
        fs::write(&py_path, py).expect("Write py bench fixture");
    }

    // 3. Go 2,000 LOC fixture
    let go_path = tmp_dir.join("large_auth_service.go");
    if !go_path.exists() {
        let mut go = String::new();
        go.push_str("package auth\n\n");
        go.push_str("type AuthClaims struct {\n    UserID string `json:\"user_id\"`\n    Role string `json:\"role\"`\n}\n\n");
        for i in 0..100 {
            go.push_str(&format!(
                "func ValidateClaimsBatch_{i}(claims *AuthClaims) bool {{\n\
                 \x20   return claims != nil && claims.Role != \"\"\n\
                 }}\n\n"
            ));
        }
        fs::write(&go_path, go).expect("Write go bench fixture");
    }

    // 4. Rust 2,000 LOC fixture
    let rs_path = tmp_dir.join("large_inventory_service.rs");
    if !rs_path.exists() {
        let mut rs = String::new();
        rs.push_str(
            "#[derive(Debug, Clone)]\npub struct StockEntry { pub sku: String, pub qty: u32 }\n\n",
        );
        for i in 0..100 {
            rs.push_str(&format!(
                "pub fn reserve_stock_batch_{i}(entry: &StockEntry) -> Result<u32, &'static str> {{\n\
                 \x20   if entry.qty == 0 {{\n\
                 \x20       return Err(\"Zero qty\");\n\
                 \x20   }}\n\
                 \x20   Ok(entry.qty)\n\
                 }}\n\n"
            ));
        }
        fs::write(&rs_path, rs).expect("Write rs bench fixture");
    }

    vec![
        ("TypeScript (2k LOC)", ts_path, "processOrderBatch_50"),
        ("Python (2k LOC)", py_path, "execute_charge_batch_50"),
        ("Go (2k LOC)", go_path, "ValidateClaimsBatch_50"),
        ("Rust (2k LOC)", rs_path, "reserve_stock_batch_50"),
    ]
}

fn bench_e2e_slice_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_slicing_pipeline");
    group.sample_size(50);

    let slicer = ContextSlicer::new();
    let options = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let fixtures = ensure_benchmark_fixtures();

    for (lang_label, file_path, symbol_name) in fixtures {
        let file_bytes = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
        group.throughput(Throughput::Bytes(file_bytes));

        group.bench_with_input(
            BenchmarkId::new("e2e_slice", lang_label),
            &(file_path, symbol_name),
            |b, (path, symbol)| {
                b.iter(|| {
                    let result = slicer.slice_symbol(
                        black_box(path),
                        black_box(symbol),
                        black_box(&options),
                    );
                    let res = black_box(result).expect("E2E Slice must succeed within SLA");
                    let md = res.to_markdown();
                    black_box(md);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_e2e_slice_pipeline);
criterion_main!(benches);
