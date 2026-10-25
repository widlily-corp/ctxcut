//! Integration tests for Milestone Stage 5 MCP tool enhancements:
//! - get_symbol_slice with `lod` parameter
//! - patch_symbol with AstGraftEngine
//! - verify_patch with validate_graft_in_memory preflight check

use ctxcut_mcp::execute_tool_with_timeout;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_mcp_get_symbol_slice_with_lod() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let cs_file = root.join("Character.cs");
    fs::write(
        &cs_file,
        r#"
using UnityEngine;

namespace Game {
    public class Character : MonoBehaviour {
        [SerializeField] private int _health = 100;

        /// <summary>
        /// Applies damage to character.
        /// </summary>
        public void TakeDamage(int amount) {
            _health -= amount;
        }

        public void Heal(int amount) {
            _health += amount;
        }
    }
}
"#,
    )
    .unwrap();

    // 1. Test LOD 1: class skeleton
    let args_lod1 = json!({
        "path": cs_file.to_string_lossy(),
        "symbol": "TakeDamage",
        "lod": 1
    });

    let (response, _metrics, error_opt, _tokens) =
        execute_tool_with_timeout("get_symbol_slice", &args_lod1, 5000);

    assert!(
        error_opt.is_none(),
        "Expected no error, got: {:?}",
        error_opt
    );
    let text = response["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("// [LOD 1: Collapsed context]") || text.contains("// ... ["),
        "Expected LOD skeleton comments, got:\n{}",
        text
    );
    assert!(
        text.contains("Heal(int amount) { ... }"),
        "Expected skeleton to mention other methods, got:\n{}",
        text
    );

    // 2. Test LOD 0: signature only
    let args_lod0 = json!({
        "path": cs_file.to_string_lossy(),
        "symbol": "TakeDamage",
        "lod": "lod0"
    });

    let (response0, _metrics0, error_opt0, _tokens0) =
        execute_tool_with_timeout("get_symbol_slice", &args_lod0, 5000);

    assert!(
        error_opt0.is_none(),
        "Expected no error, got: {:?}",
        error_opt0
    );
    let text0 = response0["content"][0]["text"].as_str().unwrap();
    assert!(
        text0.contains("public void TakeDamage(int amount);"),
        "Expected signature only in LOD 0, got:\n{}",
        text0
    );
}

#[test]
fn test_mcp_patch_symbol_ast_grafting() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let cs_file = root.join("ScoreManager.cs");
    let original_content = r#"
namespace Game {
    public class ScoreManager {
        private int _score;

        public void AddScore(int delta) {
            _score += delta;
        }
    }
}
"#;
    fs::write(&cs_file, original_content).unwrap();

    let replacement = r#"        public void AddScore(int delta) {
            if (delta > 0) {
                _score += delta;
            }
        }"#;

    let args = json!({
        "path": cs_file.to_string_lossy(),
        "symbol": "AddScore",
        "code": replacement,
        "dry_run": false
    });

    let (response, _metrics, error_opt, _tokens) =
        execute_tool_with_timeout("patch_symbol", &args, 5000);

    assert!(
        error_opt.is_none(),
        "Expected no error, got: {:?}",
        error_opt
    );
    assert!(!response["isError"].as_bool().unwrap_or(false));

    let updated_content = fs::read_to_string(&cs_file).unwrap();
    assert!(
        updated_content.contains("if (delta > 0)"),
        "Expected patched content on disk, got:\n{}",
        updated_content
    );
}

#[test]
fn test_mcp_verify_patch_in_memory_validation() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let cs_file = root.join("Calculator.cs");
    let original_content = r#"
namespace MathUtil {
    public class Calculator {
        public int Multiply(int a, int b) {
            return a * b;
        }
    }
}
"#;
    fs::write(&cs_file, original_content).unwrap();

    // 1. Verify invalid syntax is rejected in preflight
    let broken_code = "public int Multiply(int a, int b) { return a *";
    let args_invalid = json!({
        "path": cs_file.to_string_lossy(),
        "symbol": "Multiply",
        "new_code": broken_code,
        "dry_run": true
    });

    let (response_inv, _metrics, _err, _tokens) =
        execute_tool_with_timeout("verify_patch", &args_invalid, 5000);

    let verify_obj = &response_inv["verify_result"];
    assert!(
        !verify_obj["success"].as_bool().unwrap_or(true),
        "Expected invalid patch to fail verification: {:?}",
        response_inv
    );
    let diagnostics = verify_obj["diagnostics"].as_array().unwrap();
    assert!(
        !diagnostics.is_empty(),
        "Expected diagnostics for syntax error"
    );

    // 2. Verify valid syntax passes
    let valid_code = "        public int Multiply(int a, int b) {\n            return checked(a * b);\n        }";
    let args_valid = json!({
        "path": cs_file.to_string_lossy(),
        "symbol": "Multiply",
        "new_code": valid_code,
        "dry_run": true
    });

    let (response_val, _metrics, _err, _tokens) =
        execute_tool_with_timeout("verify_patch", &args_valid, 5000);

    let verify_val_obj = &response_val["verify_result"];
    assert!(
        verify_val_obj["success"].as_bool().unwrap_or(false),
        "Expected valid patch to pass verification: {:?}",
        response_val
    );
}
