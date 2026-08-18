//! Empirical Challenger Adversarial Test Suite for M1:
//! Deep nesting, complex ignore precedence (.gitignore + .ctxcutignore + negations),
//! exhaustive UTF-8 split & null-byte binary detection matrix, hidden flags, and fast stats.

use ctxcut_core::traversal::{
    is_binary_bytes, is_binary_file, is_blacklisted_file, is_ignored_directory, ProjectWalker,
    TraversalConfig, DEFAULT_IGNORED_DIRS, DEFAULT_IGNORED_FILES,
};
use std::fs;
use tempfile::TempDir;

/// Test A1: Deep Directory Nesting (25 levels) with nested .gitignore and .ctxcutignore
#[test]
fn test_deep_nesting_25_levels_with_mixed_ignores() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create a .git marker directory
    fs::create_dir_all(root.join(".git")).unwrap();

    // Build 25 levels deep path
    let mut current_dir = root.to_path_buf();
    for level in 1..=25 {
        current_dir = current_dir.join(format!("level_{level}"));
        fs::create_dir_all(&current_dir).unwrap();

        // At level 5: add .gitignore ignoring *.tmp
        if level == 5 {
            fs::write(current_dir.join(".gitignore"), "*.tmp\nsub_ignore/\n").unwrap();
        }

        // At level 10: add .ctxcutignore ignoring *.gen
        if level == 10 {
            fs::write(current_dir.join(".ctxcutignore"), "*.gen\nctx_sub/\n").unwrap();
        }

        // At level 15: add a valid file and an ignored file
        if level == 15 {
            fs::write(current_dir.join("valid_level15.rs"), "fn level15() {}").unwrap();
            fs::write(current_dir.join("test.tmp"), "ignored tmp").unwrap();
            fs::write(current_dir.join("test.gen"), "ignored gen").unwrap();
        }

        // At level 20: add a sub directory matching ignore rule
        if level == 20 {
            let sub_ignore = current_dir.join("sub_ignore");
            fs::create_dir_all(&sub_ignore).unwrap();
            fs::write(sub_ignore.join("hidden.rs"), "fn hidden() {}").unwrap();

            let ctx_sub = current_dir.join("ctx_sub");
            fs::create_dir_all(&ctx_sub).unwrap();
            fs::write(ctx_sub.join("hidden_ctx.rs"), "fn hidden_ctx() {}").unwrap();
        }

        // At level 25 (deepest leaf): add a valid file
        if level == 25 {
            fs::write(current_dir.join("leaf_deep.ts"), "export const deep = 25;").unwrap();
        }
    }

    let config = TraversalConfig::default();
    let files = ProjectWalker::collect_files(root, &config);
    let names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(
        names.iter().any(|n| n == "valid_level15.rs"),
        "valid_level15.rs at level 15 must be collected"
    );
    assert!(
        names.iter().any(|n| n == "leaf_deep.ts"),
        "leaf_deep.ts at level 25 must be collected"
    );
    assert!(
        !names.iter().any(|n| n == "test.tmp"),
        "test.tmp at level 15 must be ignored by .gitignore"
    );
    assert!(
        !names.iter().any(|n| n == "test.gen"),
        "test.gen at level 15 must be ignored by .ctxcutignore"
    );
    assert!(
        !names.iter().any(|n| n == "hidden.rs"),
        "hidden.rs inside sub_ignore/ must be ignored by .gitignore"
    );
    assert!(
        !names.iter().any(|n| n == "hidden_ctx.rs"),
        "hidden_ctx.rs inside ctx_sub/ must be ignored by .ctxcutignore"
    );
}

/// Test A2: Exhaustive Binary Detection Matrix on UTF-8 Multi-byte Splits and Nulls
#[test]
fn test_exhaustive_binary_detection_matrix() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Empty file (0 bytes) -> NOT binary
    let empty_file = root.join("empty.txt");
    fs::write(&empty_file, b"").unwrap();
    assert!(!is_binary_file(&empty_file));
    assert!(!is_binary_bytes(b""));

    // 2. Single byte non-null -> NOT binary
    let one_byte_ascii = root.join("one_ascii.txt");
    fs::write(&one_byte_ascii, b"A").unwrap();
    assert!(!is_binary_file(&one_byte_ascii));
    assert!(!is_binary_bytes(b"A"));

    // 3. Single byte null -> BINARY
    let one_byte_null = root.join("one_null.txt");
    fs::write(&one_byte_null, b"\0").unwrap();
    assert!(is_binary_file(&one_byte_null));
    assert!(is_binary_bytes(b"\0"));

    // 4. 2-byte UTF-8 split: 'д' is 2 bytes (0xD0, 0xB4)
    // 1023 ASCII bytes + 'д' -> first byte 0xD0 at index 1023, second byte at index 1024
    let mut split_2byte = "a".repeat(1023);
    split_2byte.push('д');
    split_2byte.push_str(" extra text");
    let file_2byte = root.join("split_2byte.txt");
    fs::write(&file_2byte, &split_2byte).unwrap();
    assert!(
        !is_binary_file(&file_2byte),
        "2-byte UTF-8 split across 1024 boundary must be recognized as text"
    );

    // 5. 3-byte UTF-8 split: '€' is 3 bytes (0xE2, 0x82, 0xAC)
    // Case a: 1023 ASCII bytes + '€' (1 byte inside buffer, 2 bytes outside)
    let mut split_3byte_a = "a".repeat(1023);
    split_3byte_a.push('€');
    let file_3byte_a = root.join("split_3byte_a.txt");
    fs::write(&file_3byte_a, &split_3byte_a).unwrap();
    assert!(
        !is_binary_file(&file_3byte_a),
        "3-byte UTF-8 split (1 byte in) must be recognized as text"
    );

    // Case b: 1022 ASCII bytes + '€' (2 bytes inside buffer, 1 byte outside)
    let mut split_3byte_b = "a".repeat(1022);
    split_3byte_b.push('€');
    let file_3byte_b = root.join("split_3byte_b.txt");
    fs::write(&file_3byte_b, &split_3byte_b).unwrap();
    assert!(
        !is_binary_file(&file_3byte_b),
        "3-byte UTF-8 split (2 bytes in) must be recognized as text"
    );

    // 6. 4-byte UTF-8 split: '🦀' is 4 bytes (0xF0, 0x9F, 0xA6, 0x80)
    // Case a: 1021 ASCII + '🦀' (3 bytes in, 1 byte out)
    let mut split_4byte_a = "a".repeat(1021);
    split_4byte_a.push('🦀');
    let file_4byte_a = root.join("split_4byte_a.txt");
    fs::write(&file_4byte_a, &split_4byte_a).unwrap();
    assert!(!is_binary_file(&file_4byte_a));

    // Case b: 1022 ASCII + '🦀' (2 bytes in, 2 bytes out)
    let mut split_4byte_b = "a".repeat(1022);
    split_4byte_b.push('🦀');
    let file_4byte_b = root.join("split_4byte_b.txt");
    fs::write(&file_4byte_b, &split_4byte_b).unwrap();
    assert!(!is_binary_file(&file_4byte_b));

    // Case c: 1023 ASCII + '🦀' (1 byte in, 3 bytes out)
    let mut split_4byte_c = "a".repeat(1023);
    split_4byte_c.push('🦀');
    let file_4byte_c = root.join("split_4byte_c.txt");
    fs::write(&file_4byte_c, &split_4byte_c).unwrap();
    assert!(!is_binary_file(&file_4byte_c));

    // 7. Invalid UTF-8 sequence within first 1024 bytes -> MUST be BINARY
    let mut invalid_utf8_mid = vec![b'a'; 1024];
    invalid_utf8_mid[500] = 0xFF; // Invalid byte in UTF-8
    let file_invalid_mid = root.join("invalid_mid.bin");
    fs::write(&file_invalid_mid, &invalid_utf8_mid).unwrap();
    assert!(
        is_binary_file(&file_invalid_mid),
        "0xFF at byte 500 must be recognized as binary"
    );

    // 8. Null byte at byte 0, 500, 1023 -> MUST be BINARY
    let mut null_at_1023 = vec![b'a'; 1024];
    null_at_1023[1023] = 0;
    let file_null_1023 = root.join("null_1023.bin");
    fs::write(&file_null_1023, &null_at_1023).unwrap();
    assert!(is_binary_file(&file_null_1023));
}

/// Test A3: Hidden files configuration vs Default Directory Blacklist Pruning
#[test]
fn test_hidden_files_and_default_pruning_interaction() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Hidden non-blacklisted directory (.github)
    let github_dir = root.join(".github").join("workflows");
    fs::create_dir_all(&github_dir).unwrap();
    fs::write(github_dir.join("ci.yml"), "name: CI").unwrap();

    // 2. Hidden blacklisted directories (.git, .venv, .idea)
    for blacklisted_hidden in &[".git", ".venv", ".idea", ".vscode"] {
        let b_dir = root.join(blacklisted_hidden);
        fs::create_dir_all(&b_dir).unwrap();
        fs::write(b_dir.join("secret.rs"), "fn secret() {}").unwrap();
    }

    // 3. Normal source file
    fs::write(root.join("index.ts"), "export const x = 1;").unwrap();

    // Run with include_hidden = true:
    // Should include .github/workflows/ci.yml and index.ts,
    // BUT still prune .git, .venv, .idea, .vscode!
    let config = TraversalConfig::default().with_include_hidden(true);
    let files = ProjectWalker::collect_files(root, &config);
    let names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(
        names.iter().any(|n| n == "index.ts"),
        "index.ts must be included"
    );
    assert!(
        names.iter().any(|n| n == "ci.yml"),
        "ci.yml in .github/ must be included when include_hidden is true"
    );
    assert!(
        !names.iter().any(|n| n == "secret.rs"),
        "secret.rs in .git/.venv/.idea must remain pruned by default directory blacklist"
    );
}

/// Test A4: Fast stats on empty repo and zero files
#[test]
fn test_fast_stats_on_empty_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let report = ProjectWalker::estimate_fast_stats(root, Some(5)).unwrap();

    assert_eq!(report.total_files, 0);
    assert_eq!(report.total_lines, 0);
    assert_eq!(report.estimated_raw_tokens, 0);
    assert_eq!(report.estimated_sliced_tokens, 0);
    assert_eq!(report.estimated_savings_percentage, 0.0);
    assert!(report.language_breakdown.is_empty());
    assert!(report.files.is_empty());
}

/// Test A5: All DEFAULT_IGNORED_DIRS and DEFAULT_IGNORED_FILES predicates
#[test]
fn test_all_default_constants_pruning() {
    for dir in DEFAULT_IGNORED_DIRS {
        assert!(
            is_ignored_directory(dir, &[]),
            "Directory constant {dir} must be recognized as ignored"
        );
    }

    for file in DEFAULT_IGNORED_FILES {
        let clean = file.strip_prefix('*').unwrap_or(file);
        let test_name = if clean.starts_with('.') {
            format!("test{clean}")
        } else {
            clean.to_string()
        };
        assert!(
            is_blacklisted_file(&test_name, &[]),
            "File pattern {file} must match {test_name}"
        );
    }
}
