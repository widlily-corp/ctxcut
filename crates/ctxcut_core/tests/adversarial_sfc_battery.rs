//! Empirical Challenger 2 Adversarial Battery: Vue, Svelte, and Astro SFCs
//!
//! Exhaustive empirical stress testing for:
//! 1. Vue 3 SFCs: `<script setup lang="ts">`, `defineProps`, `defineEmits`, dual script blocks, template/style collapsing
//! 2. Svelte SFCs: `<script lang="ts">`, `export let`, Svelte 5 runes (`$props()`, `$state()`, `$derived()`, `$effect()`), module context scripts, markup/style collapsing
//! 3. Astro SFCs: frontmatter fences `--- ... ---`, `Astro.props`, `interface Props`, client directives, template/style collapsing
//! 4. Quantitative Token Reduction: verifying >50-80% token savings on realistic SFC components
//! 5. Adversarial Edge Cases: empty SFCs, no-script SFCs, unclosed tags/fences, nested tag strings, cross-file imports between SFCs and TS files

use ctxcut_core::lang::sfc::{SfcBlockKind, SfcDocument};
use ctxcut_core::lang::LanguageRegistry;
use ctxcut_core::model::{SliceOptions, SupportedLanguage};
use ctxcut_core::parser::ParserManager;
use ctxcut_core::slice::ContextSlicer;
use ctxcut_core::tokenizer::count_tokens;
use std::fs;
use tempfile::tempdir;

// =========================================================================
// 1. VUE 3 SFC ADVERSARIAL BATTERY
// =========================================================================

#[test]
fn test_adversarial_vue_dual_scripts_setup_and_module_types() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let vue_file = root.join("UserProfile.vue");
    let vue_src = r#"<script lang="ts">
export interface UserMetadata {
    roles: string[];
    permissions: Record<string, boolean>;
}

export const DEFAULT_AVATAR = "https://cdn.example.com/avatar.png";
</script>

<script setup lang="ts">
import { ref, computed } from 'vue';

export interface UserDto {
    id: string;
    username: string;
    email: string;
    meta: UserMetadata;
}

const props = defineProps<{
    user: UserDto;
    isAdmin?: boolean;
}>();

const emit = defineEmits<{
    (e: 'update:user', user: UserDto): void;
    (e: 'delete', id: string): void;
}>();

const isEditing = ref(false);

export function formatUserHeader(u: UserDto): string {
    return `${u.username.toUpperCase()} (${u.email})`;
}
</script>

<template>
    <div class="user-profile-container">
        <header class="profile-header">
            <h2>{{ formatUserHeader(user) }}</h2>
            <img :src="DEFAULT_AVATAR" alt="Avatar" />
        </header>
        <main class="profile-content">
            <p v-if="isAdmin" class="badge">ADMINISTRATOR</p>
            <div v-for="role in user.meta.roles" :key="role" class="role-chip">
                {{ role }}
            </div>
            <button @click="emit('delete', user.id)">Delete User</button>
        </main>
    </div>
</template>

<style scoped>
.user-profile-container {
    padding: 24px;
    background: #1a1a1a;
    color: #f5f5f5;
    border-radius: 8px;
}
.role-chip {
    display: inline-block;
    padding: 4px 8px;
    background: #333;
    margin-right: 4px;
}
</style>
"#;
    fs::write(&vue_file, vue_src).expect("write vue file");

    // 1. SfcDocument segmentation
    let doc = SfcDocument::parse_vue(vue_src);
    assert_eq!(
        doc.blocks.len(),
        4,
        "Must detect 2 script blocks, 1 template, 1 style"
    );
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Script));
    assert!(doc
        .blocks
        .iter()
        .any(|b| b.kind == SfcBlockKind::ScriptSetup));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Template));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Style));
    assert!(doc.is_typescript);

    // Combined script must contain contents of both scripts
    assert!(doc.combined_script.contains("UserMetadata"));
    assert!(doc.combined_script.contains("formatUserHeader"));

    // 2. Tree-sitter AST symbol location & slicing
    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&vue_file, "formatUserHeader", &opts)
        .expect("Should slice formatUserHeader from Vue SFC");

    assert_eq!(slice.target_symbol.name, "formatUserHeader");
    assert_eq!(slice.target_symbol.language, "vue");
    assert!(slice.target_symbol.body.contains("formatUserHeader"));

    // Type hoisting must find UserDto
    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"UserDto"),
        "Must hoist UserDto type from Vue script, found: {:?}",
        hoisted
    );

    // 3. DefineProps symbol lookup
    let adapter = LanguageRegistry::for_language(SupportedLanguage::Vue).unwrap();
    let ts_lang = adapter.tree_sitter_language(&vue_file);
    let tree = ParserManager::parse_source(vue_src, &ts_lang, &vue_file).unwrap();
    let (props_sym, _) = adapter
        .locate_symbol(tree.root_node(), vue_src, "defineProps", &vue_file)
        .unwrap();
    assert_eq!(props_sym.name, "defineProps");
    assert!(props_sym.body.contains("defineProps"));
}

#[test]
fn test_adversarial_vue_external_import_type_hoisting_and_call_stripping() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Sibling api.ts
    let api_file = root.join("auth_service.ts");
    fs::write(
        &api_file,
        r#"export interface SessionToken {
    token: string;
    expiresAt: number;
}

export async function loginUser(email: string): Promise<SessionToken> {
    return { token: "abc", expiresAt: 123456789 };
}
"#,
    )
    .expect("write api.ts");

    let vue_file = root.join("LoginView.vue");
    let vue_src = r#"<script setup lang="ts">
import { ref } from 'vue';
import { loginUser, SessionToken } from './auth_service';

const email = ref('');
const session = ref<SessionToken | null>(null);

export async function submitLogin(): Promise<boolean> {
    const res = await loginUser(email.value);
    session.value = res;
    return true;
}
</script>

<template>
    <form @submit.prevent="submitLogin">
        <input v-model="email" type="email" placeholder="Enter email" />
        <button type="submit">Sign In</button>
    </form>
</template>
"#;
    fs::write(&vue_file, vue_src).expect("write vue file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&vue_file, "submitLogin", &opts)
        .expect("Should slice submitLogin from Vue SFC with external import");

    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"SessionToken"),
        "Must hoist SessionToken from external auth_service.ts, found: {:?}",
        hoisted
    );

    let calls: Vec<&str> = slice
        .stripped_calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(
        calls.contains(&"loginUser"),
        "Must strip loginUser call from external auth_service.ts, found: {:?}",
        calls
    );
}

// =========================================================================
// 2. SVELTE SFC ADVERSARIAL BATTERY (INCL. SVELTE 5 RUNES & MODULE CONTEXT)
// =========================================================================

#[test]
fn test_adversarial_svelte_runes_and_module_context() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let svelte_file = root.join("RunesCounter.svelte");
    let svelte_src = r#"<script context="module" lang="ts">
export interface CounterConfig {
    step: number;
    maxLimit: number;
}

export const DEFAULT_CONFIG: CounterConfig = { step: 1, maxLimit: 100 };
</script>

<script lang="ts">
interface Props {
    initial?: number;
    config?: CounterConfig;
}

let { initial = 0, config = DEFAULT_CONFIG }: Props = $props();

let count = $state(initial);
let doubled = $derived(count * 2);

export function handleIncrement(): void {
    if (count + config.step <= config.maxLimit) {
        count += config.step;
    }
}

export function resetCount(): void {
    count = initial;
}
</script>

<div class="counter-box">
    <p>Count: {count} (Doubled: {doubled})</p>
    <button on:click={handleIncrement}>Increment</button>
    <button on:click={resetCount}>Reset</button>
</div>

<style>
.counter-box {
    padding: 1rem;
    border: 1px solid #ff3e00;
}
</style>
"#;
    fs::write(&svelte_file, svelte_src).expect("write svelte file");

    // 1. SfcDocument segmentation
    let doc = SfcDocument::parse_svelte(svelte_src);
    assert!(doc.blocks.len() >= 3);
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Script));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Markup));
    assert!(doc.blocks.iter().any(|b| b.kind == SfcBlockKind::Style));
    assert!(doc.is_typescript);

    // 2. Slicing function from Svelte component
    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&svelte_file, "handleIncrement", &opts)
        .expect("Should slice handleIncrement from Svelte SFC");

    assert_eq!(slice.target_symbol.name, "handleIncrement");
    assert_eq!(slice.target_symbol.language, "svelte");
    assert!(slice.target_symbol.body.contains("handleIncrement"));

    // 3. Adapter symbol listing includes runes variables & module exports
    let adapter = LanguageRegistry::for_language(SupportedLanguage::Svelte).unwrap();
    let ts_lang = adapter.tree_sitter_language(&svelte_file);
    let tree = ParserManager::parse_source(svelte_src, &ts_lang, &svelte_file).unwrap();
    let symbols = adapter.list_symbols(tree.root_node(), svelte_src);
    assert!(symbols.contains(&"handleIncrement".to_string()));
    assert!(symbols.contains(&"resetCount".to_string()));
}

#[test]
fn test_adversarial_svelte_legacy_export_let_props_and_type_hoisting() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let svelte_file = root.join("LegacyButton.svelte");
    let svelte_src = r#"<script lang="ts">
export interface ButtonTheme {
    primaryColor: string;
    textColor: string;
}

export let label: string = "Submit";
export let theme: ButtonTheme;
export let disabled: boolean = false;

export function handleClick(event: MouseEvent) {
    if (!disabled) {
        console.log("Clicked " + label);
    }
}
</script>

<button style="background: {theme.primaryColor}; color: {theme.textColor}" on:click={handleClick} {disabled}>
    {label}
</button>

<style>
button {
    padding: 8px 16px;
    border-radius: 4px;
}
</style>
"#;
    fs::write(&svelte_file, svelte_src).expect("write svelte file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // Slice handleClick
    let slice = slicer
        .slice_symbol(&svelte_file, "handleClick", &opts)
        .expect("Should slice handleClick from Svelte SFC");

    assert_eq!(slice.target_symbol.name, "handleClick");

    // Locate export let property
    let adapter = LanguageRegistry::for_language(SupportedLanguage::Svelte).unwrap();
    let ts_lang = adapter.tree_sitter_language(&svelte_file);
    let tree = ParserManager::parse_source(svelte_src, &ts_lang, &svelte_file).unwrap();

    let (label_sym, _) = adapter
        .locate_symbol(tree.root_node(), svelte_src, "label", &svelte_file)
        .unwrap();
    assert_eq!(label_sym.name, "label");
    assert_eq!(label_sym.kind, "property");
    assert!(label_sym.signature.contains("export let label"));

    let (theme_sym, _) = adapter
        .locate_symbol(tree.root_node(), svelte_src, "theme", &svelte_file)
        .unwrap();
    assert_eq!(theme_sym.name, "theme");
    assert!(theme_sym.signature.contains("export let theme"));
}

#[test]
fn test_adversarial_svelte_external_import_type_hoisting() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let model_file = root.join("user_types.ts");
    fs::write(
        &model_file,
        r#"export interface SvelteUser {
    id: string;
    displayName: string;
}
"#,
    )
    .expect("write user_types.ts");

    let svelte_file = root.join("UserBadge.svelte");
    let svelte_src = r#"<script lang="ts">
import { SvelteUser } from './user_types';

export let user: SvelteUser;

export function getUserDisplayName(u: SvelteUser): string {
    return u.displayName;
}
</script>

<span>{getUserDisplayName(user)}</span>
"#;
    fs::write(&svelte_file, svelte_src).expect("write svelte file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&svelte_file, "getUserDisplayName", &opts)
        .expect("Should slice getUserDisplayName from Svelte SFC");

    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    println!("Svelte external import hoisted types: {:?}", hoisted);
    assert!(
        hoisted.contains(&"SvelteUser"),
        "Must hoist SvelteUser from user_types.ts, found: {:?}",
        hoisted
    );
}

#[test]
fn test_adversarial_astro_frontmatter_fences_and_type_slicing() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Sibling data loader
    let data_file = root.join("content_loader.ts");
    fs::write(
        &data_file,
        r#"export interface BlogPost {
    slug: string;
    title: string;
    pubDate: string;
    author: string;
}

export async function fetchPostBySlug(slug: string): Promise<BlogPost> {
    return { slug, title: "Astro Deep Dive", pubDate: "2026-08-23", author: "Widlily" };
}
"#,
    )
    .expect("write data loader");

    let astro_file = root.join("PostLayout.astro");
    let astro_src = r#"---
import { fetchPostBySlug, BlogPost } from './content_loader';

export interface Props {
    postSlug: string;
    showAuthor?: boolean;
}

const { postSlug, showAuthor = true } = Astro.props;
const post: BlogPost = await fetchPostBySlug(postSlug);

export function buildPageTitle(p: BlogPost): string {
    return `${p.title} | My Dev Blog`;
}
---

<article class="blog-post-article">
    <header>
        <h1>{buildPageTitle(post)}</h1>
        <p class="pub-date">Published on {post.pubDate}</p>
        {showAuthor && <p class="author">By {post.author}</p>}
    </header>
    <section class="post-body">
        <slot />
    </section>
</article>

<style>
.blog-post-article {
    max-width: 800px;
    margin: 0 auto;
    font-family: system-ui, sans-serif;
}
</style>
"#;
    fs::write(&astro_file, astro_src).expect("write astro file");

    // 1. SfcDocument segmentation
    let doc = SfcDocument::parse_astro(astro_src);
    assert_eq!(doc.blocks.len(), 2, "Must segment frontmatter and markup");
    assert_eq!(doc.blocks[0].kind, SfcBlockKind::Frontmatter);
    assert_eq!(doc.blocks[1].kind, SfcBlockKind::Markup);
    assert!(doc.is_typescript);
    assert!(doc.combined_script.contains("fetchPostBySlug"));
    assert!(doc.combined_script.contains("buildPageTitle"));

    // 2. Slice buildPageTitle
    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&astro_file, "buildPageTitle", &opts)
        .expect("Should slice buildPageTitle from Astro frontmatter");

    assert_eq!(slice.target_symbol.name, "buildPageTitle");
    assert_eq!(slice.target_symbol.language, "astro");

    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"BlogPost"),
        "Must hoist BlogPost type from external content_loader.ts, found: {:?}",
        hoisted
    );

    // 3. Astro.props / Props lookup
    let adapter = LanguageRegistry::for_language(SupportedLanguage::Astro).unwrap();
    let ts_lang = adapter.tree_sitter_language(&astro_file);
    let tree = ParserManager::parse_source(astro_src, &ts_lang, &astro_file).unwrap();

    let (props_sym, _) = adapter
        .locate_symbol(tree.root_node(), astro_src, "Props", &astro_file)
        .unwrap();
    assert_eq!(props_sym.name, "Props");
    assert_eq!(props_sym.kind, "interface");
}

// =========================================================================
// 4. QUANTITATIVE TOKEN REDUCTION VERIFICATION (>50-80% SAVINGS)
// =========================================================================

#[test]
fn test_adversarial_sfc_token_reduction_empirical_measurements() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Construct a realistic, production-scale Vue component with large template and style blocks
    let vue_file = root.join("DashboardView.vue");
    let mut template_body = String::new();
    for i in 0..100 {
        template_body.push_str(&format!(
            "        <div class=\"data-row\" id=\"row-{i}\">\n            <span class=\"row-label\">Metric item {i}</span>\n            <span class=\"row-val\">{{{{ metrics[{i}].value }}}}</span>\n            <button class=\"btn-action\" @click=\"inspectItem({i})\">View Details {i}</button>\n        </div>\n"
        ));
    }

    let mut style_body = String::new();
    for i in 0..80 {
        style_body.push_str(&format!(
            ".dashboard-theme-{i} {{\n    background-color: var(--theme-bg-{i});\n    color: var(--theme-fg-{i});\n    padding: {i}px;\n    border: 1px solid rgba(255, 255, 255, 0.1);\n}}\n"
        ));
    }

    let full_vue = format!(
        r#"<script setup lang="ts">
export interface MetricSummary {{
    totalCount: number;
    activePercentage: number;
}}

export function calculateSummary(raw: number[]): MetricSummary {{
    const total = raw.reduce((acc, v) => acc + v, 0);
    return {{ totalCount: raw.len(), activePercentage: total > 0 ? 100.0 : 0.0 }};
}}
</script>

<template>
    <section class="dashboard-grid">
{template_body}
    </section>
</template>

<style scoped>
{style_body}
</style>
"#
    );
    fs::write(&vue_file, &full_vue).expect("write large vue file");

    let raw_tokens = count_tokens(&full_vue);

    let doc = SfcDocument::parse_vue(&full_vue);
    let collapsed_summaries = doc.collapse_summaries();
    let collapsed_view = format!(
        "{}\n\n{}",
        doc.combined_script,
        collapsed_summaries.join("\n")
    );
    let collapsed_tokens = count_tokens(&collapsed_view);

    let token_reduction = 100.0 * (1.0 - (collapsed_tokens as f64 / raw_tokens as f64));
    println!("Vue Full Tokens: {raw_tokens}, Collapsed Tokens: {collapsed_tokens}, Reduction: {token_reduction:.2}%");

    assert!(
        token_reduction > 80.0,
        "Token reduction on large Vue SFC must exceed 80%, achieved: {token_reduction:.2}%"
    );

    // Also verify Slicer result token reduction
    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice_result = slicer
        .slice_symbol(&vue_file, "calculateSummary", &opts)
        .expect("Slice calculateSummary");

    let slice_reduction = slice_result.stats.savings_percentage;
    println!("Vue Slice Stats Reduction: {slice_reduction:.2}%");
    assert!(
        slice_reduction > 80.0,
        "Slice reduction percentage must exceed 80%, achieved: {slice_reduction:.2}%"
    );
}

#[test]
fn test_adversarial_svelte_and_astro_token_reduction() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Svelte with extensive HTML markup and styles
    let svelte_file = root.join("DataTable.svelte");
    let mut markup_body = String::new();
    for i in 0..80 {
        markup_body.push_str(&format!(
            "<tr><td>Row {i}</td><td>Value {i}</td><td><button on:click={{() => selectRow({i})}}>Select</button></td></tr>\n"
        ));
    }
    let full_svelte = format!(
        r#"<script lang="ts">
export interface TableRow {{
    id: number;
    title: string;
}}

export function filterRows(rows: TableRow[], query: string): TableRow[] {{
    return rows.filter(r => r.title.toLowerCase().includes(query.toLowerCase()));
}}
</script>

<table>
    <thead><tr><th>ID</th><th>Title</th><th>Actions</th></tr></thead>
    <tbody>
{markup_body}
    </tbody>
</table>

<style>
table {{ width: 100%; border-collapse: collapse; }}
td, th {{ border: 1px solid #ddd; padding: 8px; }}
</style>
"#
    );
    fs::write(&svelte_file, &full_svelte).expect("write svelte");

    let svelte_raw = count_tokens(&full_svelte);
    let svelte_doc = SfcDocument::parse_svelte(&full_svelte);
    let svelte_collapsed = format!(
        "{}\n\n{}",
        svelte_doc.combined_script,
        svelte_doc.collapse_summaries().join("\n")
    );
    let svelte_collapsed_tokens = count_tokens(&svelte_collapsed);
    let svelte_reduction = 100.0 * (1.0 - (svelte_collapsed_tokens as f64 / svelte_raw as f64));
    println!("Svelte Raw: {svelte_raw}, Collapsed: {svelte_collapsed_tokens}, Reduction: {svelte_reduction:.2}%");

    assert!(
        svelte_reduction > 70.0,
        "Svelte token reduction must exceed 70%, got: {svelte_reduction:.2}%"
    );

    // Astro token reduction
    let astro_file = root.join("Landing.astro");
    let mut astro_markup = String::new();
    for i in 0..60 {
        astro_markup.push_str(&format!(
            "<section id=\"feature-{i}\" class=\"feature-card\"><h3>Feature {i}</h3><p>Description for feature {i} in Astro project</p></section>\n"
        ));
    }
    let full_astro = format!(
        r#"---
export interface PageConfig {{
    seoTitle: string;
    analyticsId: string;
}}

export function initConfig(): PageConfig {{
    return {{ seoTitle: "Home", analyticsId: "UA-12345" }};
}}
---

<main>
{astro_markup}
</main>

<style>
.feature-card {{ margin: 16px; padding: 24px; border-radius: 12px; }}
</style>
"#
    );
    fs::write(&astro_file, &full_astro).expect("write astro");

    let astro_raw = count_tokens(&full_astro);
    let astro_doc = SfcDocument::parse_astro(&full_astro);
    let astro_collapsed = format!(
        "{}\n\n{}",
        astro_doc.combined_script,
        astro_doc.collapse_summaries().join("\n")
    );
    let astro_collapsed_tokens = count_tokens(&astro_collapsed);
    let astro_reduction = 100.0 * (1.0 - (astro_collapsed_tokens as f64 / astro_raw as f64));
    println!("Astro Raw: {astro_raw}, Collapsed: {astro_collapsed_tokens}, Reduction: {astro_reduction:.2}%");

    assert!(
        astro_reduction > 60.0,
        "Astro token reduction must exceed 60%, got: {astro_reduction:.2}%"
    );
}

// =========================================================================
// 5. ADVERSARIAL EDGE CASES & FAULT RESILIENCE
// =========================================================================

#[test]
fn test_adversarial_sfc_edge_cases_and_graceful_degradation() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. Empty SFC files
    let empty_vue = root.join("Empty.vue");
    fs::write(&empty_vue, "").expect("write empty vue");
    let doc = SfcDocument::parse_vue("");
    assert!(doc.blocks.is_empty());
    assert!(doc.combined_script.is_empty());

    let empty_svelte = root.join("Empty.svelte");
    fs::write(&empty_svelte, "   \n\t  \n").expect("write whitespace svelte");
    let doc = SfcDocument::parse_svelte("   \n\t  \n");
    assert!(doc.combined_script.is_empty());

    let empty_astro = root.join("Empty.astro");
    fs::write(&empty_astro, "").expect("write empty astro");
    let doc = SfcDocument::parse_astro("");
    assert!(doc.combined_script.is_empty());

    // 2. SFC with NO script block (only template and style)
    let static_vue = root.join("StaticView.vue");
    fs::write(
        &static_vue,
        "<template><h1>Static Header</h1><p>No JS logic</p></template><style>h1 { color: red; }</style>\n",
    )
    .expect("write static vue");
    let doc = SfcDocument::parse_vue("<template><h1>Static Header</h1><p>No JS logic</p></template><style>h1 { color: red; }</style>\n");
    assert_eq!(doc.blocks.len(), 2);
    assert_eq!(doc.blocks[0].kind, SfcBlockKind::Template);
    assert_eq!(doc.blocks[1].kind, SfcBlockKind::Style);

    // 3. Malformed unclosed tags
    let broken_vue = root.join("Broken.vue");
    fs::write(
        &broken_vue,
        "<script setup lang=\"ts\">\nexport function alive() { return 42; }\n// missing closing script tag\n<template><div><span>unclosed",
    )
    .expect("write broken vue");
    let doc = SfcDocument::parse_vue("<script setup lang=\"ts\">\nexport function alive() { return 42; }\n// missing closing script tag\n<template><div><span>unclosed");
    assert!(doc.combined_script.contains("export function alive"));

    // 4. Astro unclosed frontmatter
    let broken_astro = root.join("Broken.astro");
    fs::write(
        &broken_astro,
        "---\nexport const x = 100;\n// missing second fence\n<h1>Broken</h1>",
    )
    .expect("write broken astro");
    let doc = SfcDocument::parse_astro(
        "---\nexport const x = 100;\n// missing second fence\n<h1>Broken</h1>",
    );
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.blocks[0].kind, SfcBlockKind::Markup);
}

#[test]
fn test_adversarial_cross_sfc_and_typescript_import_chain() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. types.ts
    fs::write(
        root.join("types.ts"),
        r#"export interface NavigationItem {
    id: string;
    path: string;
    label: string;
}
"#,
    )
    .expect("write types.ts");

    // 2. Svelte component importing types.ts
    fs::write(
        root.join("NavItem.svelte"),
        r#"<script lang="ts">
import { NavigationItem } from './types';

export let item: NavigationItem;
export let isActive: boolean = false;
</script>

<a href="{item.path}" class:active="{isActive}">{item.label}</a>
"#,
    )
    .expect("write NavItem.svelte");

    // 3. Vue component importing types.ts
    let nav_bar_vue = root.join("NavBar.vue");
    fs::write(
        &nav_bar_vue,
        r#"<script setup lang="ts">
import { ref } from 'vue';
import { NavigationItem } from './types';

export interface NavBarProps {
    items: NavigationItem[];
    brandTitle: string;
}

const props = defineProps<NavBarProps>();
const activeId = ref(props.items[0]?.id || '');

export function selectItem(item: NavigationItem): void {
    activeId.value = item.id;
}
</script>

<template>
    <nav class="navbar">
        <span class="brand">{{ brandTitle }}</span>
        <ul>
            <li v-for="item in items" :key="item.id" @click="selectItem(item)">
                {{ item.label }}
            </li>
        </ul>
    </nav>
</template>
"#,
    )
    .expect("write NavBar.vue");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&nav_bar_vue, "selectItem", &opts)
        .expect("Should slice selectItem from NavBar.vue with cross-file types.ts hoisting");

    assert_eq!(slice.target_symbol.name, "selectItem");
    let hoisted: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"NavigationItem"),
        "Must hoist NavigationItem from types.ts, found: {:?}",
        hoisted
    );
}
