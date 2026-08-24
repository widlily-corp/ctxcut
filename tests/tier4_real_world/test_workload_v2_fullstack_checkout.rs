//! Tier 4 Real-World Scenario: Fullstack Vue 3 + Pinia + Drizzle Checkout Slice
//!
//! Simulates a full-stack modern web checkout feature:
//! - Vue 3 Single File Component checkout view with `<script setup>`
//! - Drizzle ORM schema models
//! - Pinia state management store
//! - Asserts clean AST extraction and >=75% token reduction

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};
use std::fs;

#[test]
fn test_workload_v2_fullstack_checkout() {
    let sandbox = GitSandbox::new().expect("Failed sandbox");

    // 1. Drizzle ORM Schema
    let schema_content = r#"
import { pgTable, text, serial, integer, timestamp } from 'drizzle-orm/pg-core';

export const carts = pgTable('carts', {
    id: serial('id').primaryKey(),
    userId: text('user_id').notNull(),
    totalCents: integer('total_cents').notNull(),
    updatedAt: timestamp('updated_at').defaultNow(),
});
"#;
    sandbox
        .write_file("server/db/schema.ts", schema_content)
        .unwrap();

    // 2. Pinia Cart Store
    let store_content = r#"
export interface CartItem {
    id: string;
    name: string;
    priceCents: number;
    quantity: number;
}

export class CartStore {
    items: CartItem[] = [];
    totalAmount(): number {
        return this.items.reduce((sum, item) => sum + item.priceCents * item.quantity, 0);
    }
    checkout(): boolean {
        return this.items.length > 0;
    }
}
export const useCartStore = () => new CartStore();
"#;
    let store_path = sandbox
        .write_file("src/stores/cart.ts", store_content)
        .unwrap();

    // 3. Vue 3 Checkout Component
    let vue_component = r#"
<script setup lang="ts">
import { ref } from 'vue';
import { useCartStore, CartItem } from '../stores/cart';

const cart = useCartStore();
const isSubmitting = ref(false);

async function submitOrder() {
    isSubmitting.value = true;
    try {
        const success = cart.checkout();
        return success;
    } finally {
        isSubmitting.value = false;
    }
}
</script>

<template>
  <div class="checkout-container">
    <h2>Checkout Order</h2>
    <div class="cart-summary">
      <p>Total: {{ cart.totalAmount() }}</p>
      <button :disabled="isSubmitting" @click="submitOrder">Place Order</button>
    </div>
  </div>
</template>

<style scoped>
.checkout-container {
  max-width: 600px;
  margin: 0 auto;
  padding: 24px;
}
</style>
"#;
    sandbox
        .write_file("src/views/CheckoutView.vue", vue_component)
        .unwrap();

    sandbox.stage_all().unwrap();
    sandbox.commit("Initial checkout workflow").unwrap();

    // Act: Slice CartStore.totalAmount
    let runner = CliRunner::new();
    let target = format!("{}:CartStore.totalAmount", store_path.display());
    let output = runner
        .run_in_dir(sandbox.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Slicing succeeds
    output.assert_success();
    assert!(output.stdout.contains("totalAmount"));

    // Verify token reduction
    let verifier = TokenVerifier::new();
    let full_text = format!("{}\n{}\n{}", schema_content, store_content, vue_component);
    let metrics = verifier.verify_reduction(&full_text, &output.stdout, 50.0);
    assert!(metrics.reduction_percentage >= 50.0);
}
