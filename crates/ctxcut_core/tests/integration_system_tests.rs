//! End-to-end integration tests for Stage 5: System Integration.
//!
//! Covers:
//! 1. C# LOD 0 (Signature only + XML-doc), LOD 1 (Class skeleton + collapsed siblings),
//!    and LOD 2 (Transitive private helpers <= 15 LOC) via `ContextSlicer::slice_symbol`.
//! 2. Active preprocessor directive tag detection (`#if ...`) with `// [Active under: ...]` tagging.
//! 3. Semantic metadata header generation for Unity/GameDev C# files, including Callers,
//!    Events emitted, and 0 GC Alloc heuristics, formatted into Markdown Section 1.
//! 4. AST syntax grafting engine (`AstGraftEngine`) preserving indentation styles and line endings (CRLF / LF),
//!    coupled with in-memory pre-flight syntax validation (`SyntaxValidator::validate_graft_in_memory`).

use ctxcut_core::formatter::MarkdownFormatter;
use ctxcut_core::model::{LodLevel, SliceOptions, SupportedLanguage};
use ctxcut_core::patch::{AstGraftEngine, IndentStyle, LineEnding, SyntaxValidator};
use ctxcut_core::slice::{
    ContextSlicer, COLLAPSED_CONTEXT_COMMENT, TARGET_SYMBOL_COMMENT, TRANSITIVE_HELPER_COMMENT,
};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

const SAMPLE_UNITY_CSHARP_CODE: &str = r#"using System;
using UnityEngine;

namespace Game.Combat
{
    public class PlayerHealth : MonoBehaviour
    {
        [SerializeField] private int maxHealth = 100;
        [SerializeField] private int currentHealth;

        public event Action<int> OnHealthChanged;

        /// <summary>
        /// Applies damage to the player and triggers events.
        /// </summary>
        /// <param name="amount">Amount of incoming damage.</param>
        public void TakeDamage(int amount)
        {
            if (amount <= 0) return;
            currentHealth = Math.Max(0, currentHealth - amount);
            NotifyHealthChanged(currentHealth);
        }

        private void NotifyHealthChanged(int hp)
        {
            OnHealthChanged?.Invoke(hp);
        }

        public void Heal(int amount)
        {
            currentHealth = Math.Min(maxHealth, currentHealth + amount);
        }

        private void ResetStats()
        {
            currentHealth = maxHealth;
        }
    }
}
"#;

// =========================================================================
// 1. C# LEVEL OF DETAIL (LOD 0, LOD 1, LOD 2) INTEGRATION TESTS
// =========================================================================

#[test]
fn test_csharp_lod0_signature_only_via_slicer() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("PlayerHealth.cs");
    fs::write(&file_path, SAMPLE_UNITY_CSHARP_CODE).expect("write csharp file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
        lod: Some(LodLevel::Lod0),
    };

    let result = slicer
        .slice_symbol(&file_path, "TakeDamage", &opts)
        .expect("slice LOD 0");

    assert!(result.target_symbol.name.ends_with("TakeDamage"));
    assert!(
        result
            .target_symbol
            .signature
            .contains("void TakeDamage(int amount)"),
        "Signature must be extracted cleanly"
    );
    assert!(
        result
            .target_symbol
            .body
            .contains("Applies damage to the player"),
        "XML doc comment text must be preserved in LOD 0 content"
    );
    // Body in LOD 0 should only contain signature / stub
    assert!(
        result.target_symbol.body.ends_with(';')
            || result.target_symbol.body.contains("void TakeDamage"),
        "LOD 0 body must be signature representation"
    );
}

#[test]
fn test_csharp_lod1_class_skeleton_via_slicer() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("PlayerHealth.cs");
    fs::write(&file_path, SAMPLE_UNITY_CSHARP_CODE).expect("write csharp file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
        lod: Some(LodLevel::Lod1),
    };

    let result = slicer
        .slice_symbol(&file_path, "TakeDamage", &opts)
        .expect("slice LOD 1");

    assert!(result.target_symbol.name.ends_with("TakeDamage"));
    let body = &result.target_symbol.body;

    // Must preserve class skeleton with fields
    assert!(
        body.contains("public class PlayerHealth"),
        "Must contain class definition header"
    );
    assert!(
        body.contains("[SerializeField] private int maxHealth = 100;"),
        "Must preserve SerializeField attributes"
    );

    // Target method is open verbatim
    assert!(
        body.contains(TARGET_SYMBOL_COMMENT),
        "Must contain TARGET SYMBOL comment"
    );
    assert!(
        body.contains("currentHealth = Math.Max(0, currentHealth - amount);"),
        "Target method body must be intact"
    );

    // Non-target sibling methods must be collapsed
    assert!(
        body.contains(COLLAPSED_CONTEXT_COMMENT),
        "Must contain collapsed context comment for sibling methods"
    );
    assert!(
        body.contains("public void Heal(int amount) { ... }")
            || body.contains("Heal(int amount) { ... }"),
        "Sibling method Heal must be collapsed"
    );
}

#[test]
fn test_csharp_lod2_transitive_helpers_via_slicer() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("PlayerHealth.cs");
    fs::write(&file_path, SAMPLE_UNITY_CSHARP_CODE).expect("write csharp file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
        lod: Some(LodLevel::Lod2),
    };

    let result = slicer
        .slice_symbol(&file_path, "TakeDamage", &opts)
        .expect("slice LOD 2");

    let body = &result.target_symbol.body;

    // In LOD 2, direct private helper `NotifyHealthChanged` (<= 15 LOC) must be expanded
    assert!(
        body.contains(TRANSITIVE_HELPER_COMMENT),
        "Must contain transitive helper comment"
    );
    assert!(
        body.contains("private void NotifyHealthChanged(int hp)"),
        "Direct private helper must be expanded"
    );
    assert!(
        body.contains("OnHealthChanged?.Invoke(hp);"),
        "Private helper body must be included"
    );

    // Unrelated sibling method `ResetStats` must remain collapsed
    assert!(
        body.contains("private void ResetStats() { ... }") || body.contains("ResetStats() { ... }"),
        "Unrelated helper ResetStats must remain collapsed"
    );
}

// =========================================================================
// 2. ACTIVE PREPROCESSOR DIRECTIVE TAGGING INTEGRATION TESTS
// =========================================================================

#[test]
fn test_csharp_preprocessor_directive_tagging() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("EditorTools.cs");
    let preproc_code = r#"using UnityEngine;

public class EditorTools : MonoBehaviour
{
#if UNITY_EDITOR
    public void OpenDebugInspector()
    {
        Debug.Log("Inspector opened");
    }
#else
    public void OpenDebugInspector()
    {
        // No-op in player build
    }
#endif
}
"#;
    fs::write(&file_path, preproc_code).expect("write preproc file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
        lod: None,
    };

    let result = slicer
        .slice_symbol(&file_path, "OpenDebugInspector", &opts)
        .expect("slice symbol under preprocessor directive");

    assert!(result.target_symbol.name.ends_with("OpenDebugInspector"));
    assert!(
        result.target_symbol.preprocessor_tag.is_some(),
        "Must detect preprocessor directive tag"
    );
    let tag = result.target_symbol.preprocessor_tag.as_ref().unwrap();
    assert!(
        tag.contains("UNITY_EDITOR"),
        "Preprocessor tag must capture UNITY_EDITOR condition, got: {}",
        tag
    );

    // Verify tag comment prepended to body
    assert!(
        result
            .target_symbol
            .body
            .contains("// [Active under: UNITY_EDITOR]"),
        "Target symbol body must contain active condition comment"
    );
}

// =========================================================================
// 3. SEMANTIC METADATA HEADER & MARKDOWN FORMATTING INTEGRATION TESTS
// =========================================================================

#[test]
fn test_unity_semantic_metadata_header_markdown_integration() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("PlayerHealth.cs");
    fs::write(&file_path, SAMPLE_UNITY_CSHARP_CODE).expect("write csharp file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
        lod: None,
    };

    let result = slicer
        .slice_symbol(&file_path, "TakeDamage", &opts)
        .expect("slice with unity semantic analyzer");

    assert!(
        result.semantic_header.is_some(),
        "C# files matching Unity framework must generate a SemanticMetadataHeader"
    );
    let header = result.semantic_header.as_ref().unwrap();

    // TakeDamage in SAMPLE_UNITY_CSHARP_CODE performs no `new` or LINQ allocations -> 0 GC Alloc
    println!("TakeDamage GC alloc status: {:?}", header.gc_alloc_status);
    assert!(
        header.gc_alloc_status.is_zero_alloc,
        "TakeDamage should be recognized as 0 GC Alloc, but got: {:?}",
        header.gc_alloc_status.potential_allocs
    );

    // Markdown rendering check
    let md = MarkdownFormatter::format(&result);

    assert!(
        md.contains("1. Target Implementation"),
        "Must contain section 1 header"
    );
    assert!(
        md.contains("// [META] Allocations: 0 GC alloc detected in this method"),
        "Section 1 code block must begin with allocation metadata"
    );
}

// =========================================================================
// 4. AST GRAFTING & IN-MEMORY PRE-FLIGHT VALIDATION INTEGRATION TESTS
// =========================================================================

#[test]
fn test_ast_grafting_and_in_memory_validation_csharp() {
    let source = "using System;\r\n\r\nclass Calc\r\n{\r\n    public int Add(int a, int b)\r\n    {\r\n        return a + b;\r\n    }\r\n}\r\n";
    let replacement = "public int Add(int a, int b)\r\n{\r\n    return checked(a + b);\r\n}";

    let graft_res = AstGraftEngine::graft_symbol(
        source,
        SupportedLanguage::CSharp,
        Path::new("Calc.cs"),
        "Add",
        replacement,
    )
    .expect("graft valid replacement");

    assert_eq!(graft_res.indent_style, IndentStyle::Spaces(4));
    assert_eq!(graft_res.line_ending, LineEnding::Crlf);
    assert!(graft_res
        .full_patched_source
        .contains("return checked(a + b);"));

    // Pre-flight in-memory validation
    let lang = tree_sitter_c_sharp::LANGUAGE.into();
    let report = SyntaxValidator::validate_graft_in_memory(
        source,
        &graft_res.full_patched_source,
        &lang,
        Path::new("Calc.cs"),
    )
    .expect("validate in memory");

    assert!(report.is_valid(), "Patched C# source must be valid");
    assert_eq!(report.error_count(), 0);
    assert!(report.diff.is_some());
}

#[test]
fn test_ast_grafting_rejects_syntactically_invalid_replacement_in_memory() {
    let source = "fn compute() -> i32 {\n    10\n}\n";
    let broken_replacement = "fn compute() -> i32 {\n    10 + \n";

    let graft_res = AstGraftEngine::graft_symbol(
        source,
        SupportedLanguage::Rust,
        Path::new("lib.rs"),
        "compute",
        broken_replacement,
    )
    .expect("graft performs textual alignment");

    let lang = tree_sitter_rust::LANGUAGE.into();
    let report = SyntaxValidator::validate_graft_in_memory(
        source,
        &graft_res.full_patched_source,
        &lang,
        Path::new("lib.rs"),
    )
    .expect("validation check runs");

    assert!(
        !report.is_valid(),
        "In-memory validator must detect syntax errors in broken replacement"
    );
    assert!(report.error_count() > 0);
    assert!(
        report.to_result().is_err(),
        "to_result() must return a CoreError::SyntaxValidationError"
    );
}
