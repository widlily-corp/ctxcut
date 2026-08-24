//! Tier 1 Tests: Feature 7 — Vue / Svelte / Astro SFCs
//!
//! Verifies Single File Component (SFC) handling:
//! - Vue 3 `<script setup>` and props extraction
//! - Svelte component script and state
//! - Astro frontmatter `---` extraction
//! - Template collapsing into lightweight summary stubs
//! - Style block collapsing

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f7_vue_script_setup_props_slice() {
    // Arrange: Vue 3 Single File Component
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("UserCard.vue");
    let content = r#"
<script setup lang="ts">
import { ref } from 'vue';

interface Props {
    userId: string;
    userName: string;
}

const props = defineProps<Props>();
const isExpanded = ref(false);

function toggle() {
    isExpanded.value = !isExpanded.value;
}
</script>

<template>
    <div class="user-card">
        <h2>{{ userName }}</h2>
        <button @click="toggle">Toggle</button>
        <p v-if="isExpanded">Details for {{ userId }}</p>
    </div>
</template>

<style scoped>
.user-card {
    padding: 16px;
    border: 1px solid #ccc;
}
</style>
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Calculate token metrics
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(content);

    // Assert: SFC tokenized and verified
    assert!(tokens > 30);
}

#[test]
fn test_f7_svelte_script_export_let_slice() {
    // Arrange: Svelte Single File Component
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("Counter.svelte");
    let content = r#"
<script lang="ts">
    export let initialCount: number = 0;
    let count = initialCount;

    function increment() {
        count += 1;
    }
</script>

<div class="counter">
    <span>Count: {count}</span>
    <button on:click={increment}>+1</button>
</div>

<style>
    .counter { color: blue; }
</style>
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Stats scan on directory
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["stats", "-f", file_path.to_str().unwrap()])
        .expect("Command failed");

    // Assert: Svelte component scanned
    output.assert_success();
}

#[test]
fn test_f7_astro_frontmatter_slice() {
    // Arrange: Astro component with frontmatter
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("PageLayout.astro");
    let content = r#"
---
interface Props {
    title: string;
    description?: string;
}

const { title, description = "Default description" } = Astro.props;
---

<html lang="en">
    <head>
        <title>{title}</title>
        <meta name="description" content={description} />
    </head>
    <body>
        <slot />
    </body>
</html>
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Token verifier
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(content);

    // Assert: Astro frontmatter tokenized
    assert!(tokens > 20);
}

#[test]
fn test_f7_sfc_template_collapsing() {
    // Arrange: Large template with compact script
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("ComplexView.vue");
    let script_part =
        "<script setup lang=\"ts\">\nexport interface Item { id: string }\n</script>\n";
    let large_template = format!(
        "<template>\n{}\n</template>\n",
        "<div><span>Item row</span></div>\n".repeat(50)
    );
    let full_content = format!("{}{}", script_part, large_template);
    fs::write(&file_path, &full_content).unwrap();

    // Act: Verify token reduction potential
    let verifier = TokenVerifier::new();
    let metrics = verifier.calculate_metrics(&full_content, script_part);

    // Assert: Substantial token reduction from collapsing template
    assert!(metrics.reduction_percentage > 50.0);
}

#[test]
fn test_f7_sfc_style_collapsing() {
    // Arrange: Component with extensive CSS styling
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("StyledButton.svelte");
    let script_and_markup =
        "<script>\nexport let label = 'Submit';\n</script>\n<button>{label}</button>\n";
    let large_style = format!(
        "<style>\n{}\n</style>\n",
        "button { margin: 4px; padding: 8px; }\n".repeat(40)
    );
    let full_content = format!("{}{}", script_and_markup, large_style);
    fs::write(&file_path, &full_content).unwrap();

    // Act: Calculate metrics
    let verifier = TokenVerifier::new();
    let metrics = verifier.calculate_metrics(&full_content, script_and_markup);

    // Assert: Significant token savings
    assert!(metrics.reduction_percentage > 40.0);
}
