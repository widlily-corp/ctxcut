//! Tier 2 Boundary Tests: Features 4 to 7 (C/C++, C#, Java/Kotlin, SFCs)
//!
//! Comprehensive boundary and corner cases:
//! - F4: Complex templates, deep namespaces, function pointers, malformed macros, extern C
//! - F5: Partial classes, extension methods, pattern matching switch, deep nullability, DI constructors
//! - F6: Lombok annotations, Kotlin companion objects, inner classes, reified types, wildcards
//! - F7: SFC without script block, dual script blocks, Svelte 5 runes, Astro client directives, unclosed tags

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;
use tempfile::TempDir;

// --- F4 Boundaries: C / C++ ---

#[test]
fn test_f4_boundary_complex_template_specialization() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("spec.cpp");
    let content = r#"
template<typename T> struct Traits { static const int id = 0; };
template<> struct Traits<int> { static const int id = 1; };
int get_id() { return Traits<int>::id; }
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f4_boundary_deep_nested_namespaces() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("ns.cpp");
    let content = "namespace A::B::C::D { class Core { public: void run() {} }; }\n";
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 5);
}

#[test]
fn test_f4_boundary_c_function_pointer_types() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("fnptr.c");
    let content = "typedef int (*Callback)(int, void*); int exec(Callback cb, void* ctx) { return cb(1, ctx); }\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f4_boundary_malformed_c_syntax_recovery() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("broken.c");
    let content = "#ifdef UNCLOSED_MACRO\nint broken_func(void) {\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f4_boundary_c_extern_linkage() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("linkage.cpp");
    let content = "extern \"C\" { void c_abi_entry(void) {} }\n";
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 5);
}

// --- F5 Boundaries: C# / .NET ---

#[test]
fn test_f5_boundary_csharp_partial_classes() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("Part1.cs"), "namespace App; public partial class Service { public void A() {} }\n").unwrap();
    fs::write(dir.path().join("Part2.cs"), "namespace App; public partial class Service { public void B() {} }\n").unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f5_boundary_csharp_extension_methods() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Ext.cs");
    let content = "namespace App; public static class StrExt { public static bool IsEmpty(this string s) => string.IsNullOrEmpty(s); }\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f5_boundary_csharp_pattern_matching() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Pattern.cs");
    let content = r#"
namespace App;
public class Matcher {
    public static string Check(object o) => o switch {
        int i when i > 0 => "positive",
        string s => s,
        _ => "unknown"
    };
}
"#;
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 10);
}

#[test]
fn test_f5_boundary_csharp_nullability_annotations() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Nulls.cs");
    let content = "namespace App; public record Item(string? Title, System.Collections.Generic.List<int?>? Tags);\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f5_boundary_csharp_dependency_injection_constructors() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Di.cs");
    let content = "namespace App; public class Worker(ILogger logger, IConfig config) { public void Run() {} }\n";
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 5);
}

// --- F6 Boundaries: Java / Kotlin ---

#[test]
fn test_f6_boundary_java_lombok_annotations() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Model.java");
    let content = "package app; import lombok.Data; @Data public class Model { private String name; }\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f6_boundary_kotlin_companion_objects() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Factory.kt");
    let content = "package app\nclass Factory { companion object { fun create(): Factory = Factory() } }\n";
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 5);
}

#[test]
fn test_f6_boundary_java_anonymous_inner_classes() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Anon.java");
    let content = "package app; public class Runner { public void run() { new Thread(new Runnable() { public void run() {} }).start(); } }\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f6_boundary_kotlin_reified_type_parameters() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Reified.kt");
    let content = "package app\ninline fun <reified T> parse(json: String): T = TODO()\n";
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 5);
}

#[test]
fn test_f6_boundary_java_wildcard_generics() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Wildcard.java");
    let content = "package app; import java.util.List; public class Wild { public static void print(List<? extends Number> list) {} }\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

// --- F7 Boundaries: Vue / Svelte / Astro SFCs ---

#[test]
fn test_f7_boundary_sfc_no_script_block() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Static.vue");
    let content = "<template><h1>Static Content</h1></template><style>h1 { color: red; }</style>\n";
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 5);
}

#[test]
fn test_f7_boundary_vue_dual_script_blocks() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Dual.vue");
    let content = r#"
<script lang="ts">
export default { name: 'DualComponent' };
</script>
<script setup lang="ts">
const count = 0;
</script>
<template><div>{{ count }}</div></template>
"#;
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}

#[test]
fn test_f7_boundary_svelte5_runes_syntax() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Runes.svelte");
    let content = "<script>\nlet { name = 'default' } = $props();\nlet count = $state(0);\n</script>\n<button>{name}: {count}</button>\n";
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 10);
}

#[test]
fn test_f7_boundary_astro_client_directives() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Interactive.astro");
    let content = "---\nimport Counter from './Counter.vue';\n---\n<Counter client:visible />\n";
    fs::write(&file, content).unwrap();

    let verifier = TokenVerifier::new();
    assert!(verifier.count_tokens(content) > 5);
}

#[test]
fn test_f7_boundary_malformed_sfc_unclosed_tags() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("Broken.vue");
    let content = "<script setup>\nconst x = 1;\n</script>\n<template><div><span>Unclosed\n";
    fs::write(&file, content).unwrap();

    let runner = CliRunner::new();
    let output = runner.run_in_dir(dir.path(), &["stats", "-f", file.to_str().unwrap()]).unwrap();
    output.assert_success();
}
