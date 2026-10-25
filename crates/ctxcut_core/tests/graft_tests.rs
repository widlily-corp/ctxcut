//! Integration tests for AST Syntax Grafting & In-Memory Pre-Flight Verification.

use ctxcut_core::error::CoreError;
use ctxcut_core::model::SupportedLanguage;
use ctxcut_core::patch::formatting::LineEnding;
use ctxcut_core::patch::validate::{
    graft::{AstGraftEngine, IndentStyle},
    GraftValidationReport, SyntaxValidator,
};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use tree_sitter::Language;

// ============================================================================
// 1. CRLF Preservation on Windows (.cs files)
// ============================================================================

#[test]
fn test_csharp_crlf_preservation_windows() {
    // Windows C# file with strict CRLF line endings
    let original = "using System;\r\n\r\npublic class CharacterController\r\n{\r\n    public void Jump()\r\n    {\r\n        Console.WriteLine(\"Old jump\");\r\n    }\r\n}\r\n";

    // Replacement code provided with Unix LF newlines
    let replacement = "public void Jump()\n{\n    Console.WriteLine(\"New high jump\");\n    ApplyForce(50.0f);\n}";

    let result = AstGraftEngine::graft_symbol(
        original,
        SupportedLanguage::CSharp,
        &PathBuf::from("CharacterController.cs"),
        "Jump",
        replacement,
    )
    .expect("Syntax grafting should succeed for Jump method");

    assert_eq!(result.line_ending, LineEnding::Crlf);
    assert!(result.full_patched_source.contains("\r\n"));

    // Ensure NO lone LF (\n without preceding \r) exists anywhere in the grafted file
    let stripped_crlf = result.full_patched_source.replace("\r\n", "");
    assert!(
        !stripped_crlf.contains('\n'),
        "Grafted output contains isolated '\\n' characters without '\\r': {:?}",
        stripped_crlf
    );

    // Verify syntax validity in-memory
    let cs_lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
    let report = SyntaxValidator::validate_graft_in_memory(
        original,
        &result.full_patched_source,
        &cs_lang,
        &PathBuf::from("CharacterController.cs"),
    )
    .expect("Pre-flight validation should run cleanly");

    assert!(
        report.is_valid(),
        "Patched source should be syntactically valid"
    );
    assert_eq!(report.error_count(), 0);
    assert!(result.full_patched_source.contains("ApplyForce(50.0f);"));
}

// ============================================================================
// 2. Tabs vs Spaces Preservation
// ============================================================================

#[test]
fn test_tab_indentation_preservation() {
    // Source file strictly indented with tabs
    let original = "public class MathService\n{\n\tpublic int Compute(int x)\n\t{\n\t\treturn x * 2;\n\t}\n\n\tpublic int Other()\n\t{\n\t\treturn 0;\n\t}\n}\n";

    // Space-indented replacement from an LLM
    let replacement =
        "public int Compute(int x)\n{\n    int doubled = x * 2;\n    return doubled + 1;\n}";

    let result = AstGraftEngine::graft_symbol(
        original,
        SupportedLanguage::CSharp,
        &PathBuf::from("MathService.cs"),
        "Compute",
        replacement,
    )
    .expect("Syntax grafting should succeed");

    assert_eq!(result.indent_style, IndentStyle::Tabs);
    assert_eq!(result.base_indentation, "\t");

    // Verify that replacement lines are converted to tabs
    assert!(result
        .full_patched_source
        .contains("\t\tint doubled = x * 2;"));
    assert!(result
        .full_patched_source
        .contains("\t\treturn doubled + 1;"));
    assert!(result.full_patched_source.contains("\tpublic int Other()"));

    // Verify syntax in-memory
    let cs_lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
    let report = SyntaxValidator::validate_graft_in_memory(
        original,
        &result.full_patched_source,
        &cs_lang,
        &PathBuf::from("MathService.cs"),
    )
    .expect("Validation report should generate");

    assert!(report.is_valid());
}

#[test]
fn test_space_indentation_preservation() {
    // Source file strictly indented with 4 spaces
    let original = "public class MathService\n{\n    public int Compute(int x)\n    {\n        return x * 2;\n    }\n}\n";

    // Tab-indented replacement
    let replacement = "public int Compute(int x)\n{\n\tint doubled = x * 2;\n\treturn doubled;\n}";

    let result = AstGraftEngine::graft_symbol(
        original,
        SupportedLanguage::CSharp,
        &PathBuf::from("MathService.cs"),
        "Compute",
        replacement,
    )
    .expect("Syntax grafting should succeed");

    assert_eq!(result.indent_style, IndentStyle::Spaces(4));
    assert_eq!(result.base_indentation, "    ");

    // Verify tabs are converted to 4 spaces, so relative indent becomes 8 spaces
    assert!(result
        .full_patched_source
        .contains("        int doubled = x * 2;"));
    assert!(result
        .full_patched_source
        .contains("        return doubled;"));
    // Ensure no tabs are injected into the space-indented file
    assert!(!result.full_patched_source.contains('\t'));

    let cs_lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
    let report = SyntaxValidator::validate_graft_in_memory(
        original,
        &result.full_patched_source,
        &cs_lang,
        &PathBuf::from("MathService.cs"),
    )
    .expect("Validation report should generate");

    assert!(report.is_valid());
}

// ============================================================================
// 3. In-Memory Pre-Flight Verification & Zero Disk Writes on Error
// ============================================================================

#[test]
fn test_syntax_error_caught_in_memory_no_disk_writes() {
    let dir = tempdir().expect("Temporary directory should be created");
    let file_path = dir.path().join("PlayerService.cs");

    let original_disk_content = "public class PlayerService\n{\n    public void SaveData()\n    {\n        // Clean implementation\n    }\n}\n";
    fs::write(&file_path, original_disk_content).expect("Initial file should be written");

    // Intentionally broken C# replacement syntax
    let broken_replacement = "public void SaveData( {\n    broken syntax !!!\n";

    // Perform graft and in-memory pre-flight validation
    let graft_res = AstGraftEngine::graft_symbol(
        original_disk_content,
        SupportedLanguage::CSharp,
        &file_path,
        "SaveData",
        broken_replacement,
    )
    .expect("Graft engine should calculate AST splice in memory");

    let cs_lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
    let report: GraftValidationReport = SyntaxValidator::validate_graft_in_memory(
        original_disk_content,
        &graft_res.full_patched_source,
        &cs_lang,
        &file_path,
    )
    .expect("Pre-flight validator should return structured report without panic");

    // 1. Report must indicate syntax error
    assert!(!report.is_valid(), "Report must flag invalid syntax");
    assert!(report.error_count() > 0, "Errors vector must not be empty");

    let first_err = &report.errors[0];
    assert!(first_err.line > 0, "Line number must be positive");
    assert!(first_err.column > 0, "Column number must be positive");
    assert!(!first_err.kind.is_empty(), "Error kind must be described");
    assert!(
        !first_err.snippet.is_empty(),
        "Contextual error snippet must be populated"
    );

    // 2. to_result() must convert to CoreError::SyntaxValidationError
    let err = report.to_result().unwrap_err();
    match &err {
        CoreError::SyntaxValidationError { path, errors } => {
            assert_eq!(path, &file_path);
            assert!(!errors.is_empty());
        }
        _ => panic!("Expected SyntaxValidationError, got {:?}", err),
    }

    // 3. CRITICAL: The file on disk was NEVER modified!
    let disk_content_after = fs::read_to_string(&file_path).expect("File must exist on disk");
    assert_eq!(
        disk_content_after, original_disk_content,
        "Disk file content must remain completely unchanged upon syntax validation rejection"
    );
}

// ============================================================================
// 4. Unity C# Class: [SerializeField] Fields and Neighbor Methods Preserved
// ============================================================================

#[test]
fn test_csharp_class_serialize_field_and_neighbor_methods_preserved() {
    let original = r#"using UnityEngine;

public class PlayerController : MonoBehaviour
{
    [SerializeField]
    private int _health = 100;

    [SerializeField]
    private float _speed = 5.0f;

    /// <summary>
    /// Executes the player's primary attack move.
    /// </summary>
    [RequireComponent(typeof(Rigidbody))]
    public void Attack()
    {
        Debug.Log("Old attack move");
    }

    public void Defend()
    {
        Debug.Log("Defend move");
    }
}
"#;

    // Replacement code without repeating attributes or XML doc comments
    let replacement = r#"public void Attack()
{
    Debug.Log("Surgically grafted attack move");
    ApplyDamage(25);
}"#;

    let result = AstGraftEngine::graft_symbol(
        original,
        SupportedLanguage::CSharp,
        &PathBuf::from("PlayerController.cs"),
        "Attack",
        replacement,
    )
    .expect("Syntax grafting Attack method should succeed");

    let patched = &result.full_patched_source;

    // 1. [SerializeField] fields are 100% preserved
    assert!(patched.contains("[SerializeField]\n    private int _health = 100;"));
    assert!(patched.contains("[SerializeField]\n    private float _speed = 5.0f;"));

    // 2. XML doc comments and attributes on Attack are preserved
    assert!(patched.contains("/// <summary>"));
    assert!(patched.contains("/// Executes the player's primary attack move."));
    assert!(patched.contains("[RequireComponent(typeof(Rigidbody))]"));

    // 3. Attack body was updated
    assert!(patched.contains("ApplyDamage(25);"));
    assert!(!patched.contains("Old attack move"));

    // 4. Sibling method Defend is completely intact
    assert!(
        patched.contains("public void Defend()\n    {\n        Debug.Log(\"Defend move\");\n    }")
    );

    // 5. Preserved trivia was noted in GraftResult
    assert!(result.preserved_leading_trivia.is_some());

    // 6. Validates cleanly in memory
    let cs_lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
    let report = SyntaxValidator::validate_graft_in_memory(
        original,
        patched,
        &cs_lang,
        &PathBuf::from("PlayerController.cs"),
    )
    .expect("Validation should run");

    assert!(
        report.is_valid(),
        "Patched C# class must be syntactically valid"
    );
}

// ============================================================================
// 5. Attribute & Trivia Update When Replacement Supplies New Attributes
// ============================================================================

#[test]
fn test_csharp_graft_with_attribute_and_doc_update() {
    let original = r#"public class CombatService
{
    /// <summary>
    /// Old documentation.
    /// </summary>
    [OldAttribute]
    public void Strike()
    {
        // old body
    }
}
"#;

    // Replacement supplies its own new attribute and new doc comment
    let replacement = r#"/// <summary>
/// Modern burst-compiled strike.
/// </summary>
[BurstCompile]
public void Strike()
{
    // new high-speed body
}"#;

    let result = AstGraftEngine::graft_symbol(
        original,
        SupportedLanguage::CSharp,
        &PathBuf::from("CombatService.cs"),
        "Strike",
        replacement,
    )
    .expect("Grafting should succeed");

    let patched = &result.full_patched_source;

    // Ensure new attribute replaced old attribute, without duplication
    assert!(patched.contains("[BurstCompile]"));
    assert!(!patched.contains("[OldAttribute]"));

    // Ensure new doc comment replaced old doc comment, without duplication
    assert!(patched.contains("Modern burst-compiled strike."));
    assert!(!patched.contains("Old documentation."));

    let cs_lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
    let report = SyntaxValidator::validate_graft_in_memory(
        original,
        patched,
        &cs_lang,
        &PathBuf::from("CombatService.cs"),
    )
    .expect("Validation should succeed");

    assert!(report.is_valid());
}

// ============================================================================
// 6. Multi-Language Grafting: Rust Function & Trivia Preservation
// ============================================================================

#[test]
fn test_rust_grafting_preserves_attributes_and_comments() {
    let original = r#"pub struct Engine {
    running: bool,
}

impl Engine {
    /// Checks engine status and telemetry.
    #[inline]
    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn stop(&mut self) {
        self.running = false;
    }
}
"#;

    let replacement = r#"pub fn is_running(&self) -> bool {
    let status = self.running;
    status
}"#;

    let result = AstGraftEngine::graft_symbol(
        original,
        SupportedLanguage::Rust,
        &PathBuf::from("engine.rs"),
        "Engine::is_running",
        replacement,
    )
    .expect("Rust grafting should succeed");

    let patched = &result.full_patched_source;

    // Preserves #[inline] and doc comment
    assert!(patched.contains("/// Checks engine status and telemetry."));
    assert!(patched.contains("#[inline]"));
    assert!(patched.contains("let status = self.running;"));
    assert!(patched.contains("pub fn stop(&mut self)"));

    let rs_lang: Language = tree_sitter_rust::LANGUAGE.into();
    let report = SyntaxValidator::validate_graft_in_memory(
        original,
        patched,
        &rs_lang,
        &PathBuf::from("engine.rs"),
    )
    .expect("Validation should succeed");

    assert!(report.is_valid());
}
