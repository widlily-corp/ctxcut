//! Comprehensive unit tests for C# preprocessor directive analysis and AST slicing.

pub use ctxcut_core::error;
pub use ctxcut_core::model;
pub use ctxcut_core::parser;
pub use ctxcut_core::CoreError;
pub use ctxcut_core::Result;

/// Preprocessor test modules for ctxcut_core.
pub mod lang {
    pub use ctxcut_core::lang::*;
    /// C# preprocessor implementation.
    #[path = "../../src/lang/csharp_preproc.rs"]
    pub mod csharp_preproc;
    /// Core preprocessor abstractions.
    #[path = "../../src/lang/preprocessor.rs"]
    pub mod preprocessor;
}

use lang::csharp_preproc::CSharpPreprocessorDetector;
use lang::preprocessor::{
    combine_condition_strings, evaluate_condition_expression, format_preprocessor_tag,
    invert_condition_string, PreprocessorBranchKind, PreprocessorContextDetector,
};
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::Parser;

fn parse_csharp(code: &str) -> (tree_sitter::Tree, tree_sitter::Language) {
    let mut parser = Parser::new();
    let lang = tree_sitter_c_sharp::LANGUAGE.into();
    parser.set_language(&lang).expect("set_language");
    let tree = parser.parse(code, None).expect("parse_source");
    (tree, lang)
}

#[test]
fn test_preprocessor_condition_utilities() {
    assert_eq!(invert_condition_string("!UNITY_SERVER"), "UNITY_SERVER");
    assert_eq!(invert_condition_string("!(UNITY_SERVER)"), "UNITY_SERVER");
    assert_eq!(invert_condition_string("UNITY_EDITOR"), "!UNITY_EDITOR");
    assert_eq!(
        invert_condition_string("UNITY_EDITOR || DEBUG"),
        "!(UNITY_EDITOR || DEBUG)"
    );

    assert_eq!(
        combine_condition_strings("UNITY_EDITOR", "DEBUG"),
        "UNITY_EDITOR && DEBUG"
    );
    assert_eq!(
        combine_condition_strings("UNITY_EDITOR || TEST", "DEBUG"),
        "(UNITY_EDITOR || TEST) && DEBUG"
    );
    assert_eq!(
        combine_condition_strings("UNITY_EDITOR", "DEBUG || DEV"),
        "UNITY_EDITOR && (DEBUG || DEV)"
    );

    assert_eq!(
        format_preprocessor_tag("UNITY_EDITOR"),
        "// [Active under: UNITY_EDITOR]"
    );
}

#[test]
fn test_condition_boolean_evaluation() {
    let mut defined = HashSet::new();
    defined.insert("UNITY_EDITOR".to_string());
    defined.insert("DEBUG".to_string());

    assert!(evaluate_condition_expression("UNITY_EDITOR", &defined));
    assert!(evaluate_condition_expression(
        "UNITY_EDITOR && DEBUG",
        &defined
    ));
    assert!(evaluate_condition_expression(
        "UNITY_EDITOR || RELEASE",
        &defined
    ));
    assert!(!evaluate_condition_expression("!UNITY_EDITOR", &defined));
    assert!(evaluate_condition_expression("!RELEASE", &defined));
    assert!(evaluate_condition_expression(
        "RELEASE || (DEBUG && !SERVER)",
        &defined
    ));
}

#[test]
fn test_method_inside_if_unity_editor() {
    let code = r#"
namespace Game.Editor {
    public class ToolWindow {
#if UNITY_EDITOR
        public void DrawInspector() {
            EditorGUILayout.LabelField("Inspector");
        }
#endif
    }
}
"#;
    let (tree, _) = parse_csharp(code);
    let root = tree.root_node();
    let detector = CSharpPreprocessorDetector::from_ast(root, code);
    let path = Path::new("ToolWindow.cs");

    let (symbol, node) = detector
        .locate_symbol(root, code, "ToolWindow.DrawInspector", path)
        .expect("locate DrawInspector");

    assert_eq!(symbol.name, "ToolWindow.DrawInspector");
    assert_eq!(symbol.kind, "method");
    assert!(
        symbol.body.contains("// [Active under: UNITY_EDITOR]"),
        "Symbol body should be tagged with active under comment, got: {}",
        symbol.body
    );

    let ctx = detector
        .detect_context_for_node(node, code)
        .expect("context for node");
    assert_eq!(ctx.condition, "UNITY_EDITOR");
    assert_eq!(ctx.branch_kind, PreprocessorBranchKind::If);
    assert_eq!(ctx.tag(), "// [Active under: UNITY_EDITOR]");
}

#[test]
fn test_method_inside_else_branch() {
    let code = r#"
public class NetworkManager {
#if !UNITY_SERVER
    public void StartClient() {
        ConnectToHost();
    }
#else
    public void StartDedicatedServer() {
        ListenOnPort(7777);
    }
#endif
}
"#;
    let (tree, _) = parse_csharp(code);
    let root = tree.root_node();
    let detector = CSharpPreprocessorDetector::from_ast(root, code);
    let path = Path::new("NetworkManager.cs");

    // Check client method in #if branch
    let (client_sym, client_node) = detector
        .locate_symbol(root, code, "NetworkManager.StartClient", path)
        .expect("locate StartClient");
    assert_eq!(client_sym.name, "NetworkManager.StartClient");
    assert!(client_sym.body.contains("// [Active under: !UNITY_SERVER]"));

    let client_ctx = detector
        .detect_context_for_node(client_node, code)
        .expect("context for client_node");
    assert_eq!(client_ctx.condition, "!UNITY_SERVER");
    assert_eq!(client_ctx.branch_kind, PreprocessorBranchKind::If);
    assert_eq!(client_ctx.tag(), "// [Active under: !UNITY_SERVER]");

    // Check server method in #else branch
    let (server_sym, server_node) = detector
        .locate_symbol(root, code, "NetworkManager.StartDedicatedServer", path)
        .expect("locate StartDedicatedServer");
    assert_eq!(server_sym.name, "NetworkManager.StartDedicatedServer");
    assert!(
        server_sym.body.contains("// [Active under: UNITY_SERVER]"),
        "Symbol body should contain tag for UNITY_SERVER, got: {}",
        server_sym.body
    );

    let server_ctx = detector
        .detect_context_for_node(server_node, code)
        .expect("context for server_node");
    assert_eq!(server_ctx.condition, "UNITY_SERVER");
    assert_eq!(server_ctx.branch_kind, PreprocessorBranchKind::Else);
    assert_eq!(server_ctx.tag(), "// [Active under: UNITY_SERVER]");
}

#[test]
fn test_nested_preprocessor_directives() {
    let code = r#"
public class DiagnosticsLogger {
#if UNITY_EDITOR
    #if DEBUG
    public void LogVerboseEditor(string msg) {
        Debug.Log($"[VERBOSE] {msg}");
    }
    #else
    public void LogReleaseEditor(string msg) {
        Debug.Log(msg);
    }
    #endif
#endif
}
"#;
    let (tree, _) = parse_csharp(code);
    let root = tree.root_node();
    let detector = CSharpPreprocessorDetector::from_ast(root, code);
    let path = Path::new("DiagnosticsLogger.cs");

    // Test nested #if DEBUG inside #if UNITY_EDITOR
    let (verbose_sym, verbose_node) = detector
        .locate_symbol(root, code, "DiagnosticsLogger.LogVerboseEditor", path)
        .expect("locate LogVerboseEditor");
    assert_eq!(verbose_sym.name, "DiagnosticsLogger.LogVerboseEditor");
    assert!(
        verbose_sym
            .body
            .contains("// [Active under: UNITY_EDITOR && DEBUG]"),
        "Expected compound tag, got: {}",
        verbose_sym.body
    );

    let verbose_ctx = detector
        .detect_context_for_node(verbose_node, code)
        .expect("context for verbose");
    assert_eq!(verbose_ctx.condition, "UNITY_EDITOR && DEBUG");
    assert_eq!(
        verbose_ctx.enclosing_conditions,
        vec!["UNITY_EDITOR".to_string(), "DEBUG".to_string()]
    );
    assert_eq!(
        verbose_ctx.tag(),
        "// [Active under: UNITY_EDITOR && DEBUG]"
    );

    // Test nested #else inside #if UNITY_EDITOR
    let (release_sym, release_node) = detector
        .locate_symbol(root, code, "DiagnosticsLogger.LogReleaseEditor", path)
        .expect("locate LogReleaseEditor");
    assert_eq!(release_sym.name, "DiagnosticsLogger.LogReleaseEditor");
    assert!(
        release_sym
            .body
            .contains("// [Active under: UNITY_EDITOR && !DEBUG]"),
        "Expected compound tag for nested else, got: {}",
        release_sym.body
    );

    let release_ctx = detector
        .detect_context_for_node(release_node, code)
        .expect("context for release");
    assert_eq!(release_ctx.condition, "UNITY_EDITOR && !DEBUG");
    assert_eq!(
        release_ctx.tag(),
        "// [Active under: UNITY_EDITOR && !DEBUG]"
    );
}

#[test]
fn test_equal_accessibility_and_listing_across_branches() {
    let code = r#"
public class PlatformBridge {
#if UNITY_IOS
    public void SendNativeMessage(string payload) {}
#elif UNITY_ANDROID
    public void SendNativeMessage(string payload) {}
#else
    public void SendNativeMessage(string payload) {}
#endif
}
"#;
    let (tree, _) = parse_csharp(code);
    let root = tree.root_node();
    let detector = CSharpPreprocessorDetector::from_ast(root, code);
    let path = Path::new("PlatformBridge.cs");

    let all_matches =
        detector.locate_all_symbols(root, code, "PlatformBridge.SendNativeMessage", path);
    assert_eq!(
        all_matches.len(),
        3,
        "Should find SendNativeMessage in all 3 branches (#if, #elif, #else)"
    );

    let symbols = detector.list_symbols(root, code);
    assert!(symbols.contains(&"PlatformBridge".to_string()));
    assert!(symbols.contains(&"PlatformBridge.SendNativeMessage".to_string()));
}

#[test]
fn test_preprocessor_defines_and_range_lookup() {
    let code = r#"
#define DEBUG
#define UNITY_EDITOR
public class BuildInfo {
#if UNITY_EDITOR || DEVELOPMENT_BUILD
    public static string GetBuildType() => "Editor/Dev";
#endif
}
"#;
    let (tree, _) = parse_csharp(code);
    let root = tree.root_node();
    let detector = CSharpPreprocessorDetector::from_ast(root, code);

    assert!(detector.is_symbol_defined("DEBUG"));
    assert!(detector.is_symbol_defined("UNITY_EDITOR"));
    assert!(!detector.is_symbol_defined("RELEASE"));

    let path = Path::new("BuildInfo.cs");
    let (sym, node) = detector
        .locate_symbol(root, code, "BuildInfo.GetBuildType", path)
        .expect("locate GetBuildType");

    assert!(sym
        .body
        .contains("// [Active under: UNITY_EDITOR || DEVELOPMENT_BUILD]"));

    // Range-based context detection
    let range = node.start_byte()..node.end_byte();
    let range_ctx = detector
        .detect_context_for_range(range, code)
        .expect("range context");
    assert_eq!(range_ctx.condition, "UNITY_EDITOR || DEVELOPMENT_BUILD");
    assert!(range_ctx.is_active_under(detector.defined_symbols()));
}
