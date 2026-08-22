//! Integration tests for multi-symbol batch slicing with unified deduplication.

use ctxcut_core::{ContextSlicer, SliceOptions};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_batch_slicing_with_type_deduplication() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let file_path = root.join("orders.ts");
    fs::write(
        &file_path,
        r#"
export interface OrderPayload {
    orderId: string;
    totalAmount: number;
    currency: string;
}

export interface Customer {
    id: string;
    name: string;
}

export function createOrder(customer: Customer, payload: OrderPayload): string {
    validateOrder(payload);
    return "created_" + payload.orderId;
}

export function cancelOrder(orderId: string, payload: OrderPayload): boolean {
    notifyUser(orderId);
    return true;
}

function validateOrder(p: OrderPayload): boolean {
    return p.totalAmount > 0;
}

function notifyUser(id: string): void {
    console.log("Notified", id);
}
"#,
    )
    .unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let batch = slicer
        .slice_batch(&file_path, &["createOrder", "cancelOrder"], &opts)
        .unwrap();

    assert_eq!(batch.target_symbols.len(), 2);
    assert_eq!(batch.target_symbols[0].name, "createOrder");
    assert_eq!(batch.target_symbols[1].name, "cancelOrder");

    // Check that OrderPayload is deduplicated (only once in hoisted_types)
    let order_payload_count = batch
        .hoisted_types
        .iter()
        .filter(|t| t.name == "OrderPayload")
        .count();
    assert_eq!(order_payload_count, 1);

    let md = batch.to_markdown();
    assert!(md.contains("### Context Slice:"));
    assert!(md.contains("createOrder, cancelOrder"));
    assert!(md.contains("#### 1. Target Implementation (Full Body)"));
    assert!(md.contains("function createOrder"));
    assert!(md.contains("function cancelOrder"));
    assert!(md.contains("#### 2. Hoisted Types & Data Contracts"));
    assert!(md.contains("OrderPayload"));
    assert!(md.contains("Customer"));

    // Ensure OrderPayload definition only appears once in the markdown
    let payload_occurrences = md.matches("interface OrderPayload").count();
    assert_eq!(payload_occurrences, 1);
}
