//! Empirical Challenger Stress Tests for Milestone 1:
//! Smart Traversal, ctxcutignore hierarchy, default blacklist pruning,
//! binary detection boundary conditions, max file sizes, and fast stats performance/accuracy.

use ctxcut_core::model::SupportedLanguage;
use ctxcut_core::traversal::{
    estimate_sliced_tokens, is_binary_file, ProjectWalker, TraversalConfig, DEFAULT_IGNORED_DIRS,
};
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

/// Test 1: Complex Nested Ignore Hierarchy (.gitignore + .ctxcutignore + directory pruning)
#[test]
fn test_complex_nested_ignore_hierarchies_and_overrides() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create a .git marker directory so gitignore engine recognizes the repository
    fs::create_dir_all(root.join(".git")).unwrap();

    // 1. Root .gitignore
    fs::write(
        root.join(".gitignore"),
        "git_ignored_dir/\n*.gitignored\nbuild_artifact.rs\n",
    )
    .unwrap();

    // 2. Root .ctxcutignore
    fs::write(
        root.join(".ctxcutignore"),
        "ctxcut_ignored_dir/\n*.generated.ts\nlegacy/\n",
    )
    .unwrap();

    // 3. Root valid file
    fs::write(root.join("root_main.rs"), "fn main() {}").unwrap();

    // 4. Root files matching ignore rules
    fs::write(root.join("test.gitignored"), "git ignored").unwrap();
    fs::write(root.join("build_artifact.rs"), "fn artifact() {}").unwrap();
    fs::write(root.join("types.generated.ts"), "export type A = string;").unwrap();

    let git_dir = root.join("git_ignored_dir");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(git_dir.join("inside_git.rs"), "fn inside() {}").unwrap();

    let ctxcut_dir = root.join("ctxcut_ignored_dir");
    fs::create_dir_all(&ctxcut_dir).unwrap();
    fs::write(ctxcut_dir.join("inside_ctxcut.ts"), "export const x = 1;").unwrap();

    // 5. Nested subdirectory with its own .gitignore and .ctxcutignore
    let sub = root.join("packages").join("frontend");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("App.tsx"), "export function App() {}").unwrap();

    fs::write(sub.join(".gitignore"), "nested_git/\n*.ngit\n").unwrap();
    fs::write(sub.join(".ctxcutignore"), "nested_ctxcut/\n*.nctxcut\n").unwrap();

    let nested_git = sub.join("nested_git");
    fs::create_dir_all(&nested_git).unwrap();
    fs::write(nested_git.join("mod.rs"), "fn mod_git() {}").unwrap();

    let nested_ctxcut = sub.join("nested_ctxcut");
    fs::create_dir_all(&nested_ctxcut).unwrap();
    fs::write(nested_ctxcut.join("mod.ts"), "export const mod_ctxcut = 1;").unwrap();

    fs::write(sub.join("file.ngit"), "content").unwrap();
    fs::write(sub.join("file.nctxcut"), "content").unwrap();

    // Act 1: Default config (respect both .gitignore and .ctxcutignore)
    let config_both = TraversalConfig::default();
    let files_both = ProjectWalker::collect_files(root, &config_both);
    let names: Vec<String> = files_both
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert_eq!(
        files_both.len(),
        2,
        "Only root_main.rs and App.tsx should be collected. Got: {names:?}"
    );
    assert!(names.contains(&"root_main.rs".to_string()));
    assert!(names.contains(&"App.tsx".to_string()));

    // Act 2: Disable ctxcutignore (respect only .gitignore)
    let config_no_ctxcut = TraversalConfig::default().with_respect_ctxcutignore(false);
    let files_no_ctxcut = ProjectWalker::collect_files(root, &config_no_ctxcut);
    let names_no_ctxcut: Vec<String> = files_no_ctxcut
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(
        names_no_ctxcut.contains(&"types.generated.ts".to_string()),
        "Disabling ctxcutignore must include types.generated.ts"
    );
    assert!(
        names_no_ctxcut.contains(&"inside_ctxcut.ts".to_string()),
        "Disabling ctxcutignore must include inside_ctxcut.ts"
    );
    assert!(
        !names_no_ctxcut.contains(&"inside_git.rs".to_string()),
        "Gitignore must still be honored"
    );

    // Act 3: Disable gitignore (respect only .ctxcutignore)
    let config_no_git = TraversalConfig::default().with_respect_gitignore(false);
    let files_no_git = ProjectWalker::collect_files(root, &config_no_git);
    let names_no_git: Vec<String> = files_no_git
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(
        names_no_git.contains(&"inside_git.rs".to_string()),
        "Disabling gitignore must include inside_git.rs"
    );
    assert!(
        names_no_git.contains(&"build_artifact.rs".to_string()),
        "Disabling gitignore must include build_artifact.rs"
    );
    assert!(
        !names_no_git.contains(&"types.generated.ts".to_string()),
        ".ctxcutignore must still be honored"
    );
}

/// Test 2: Deeply Nested Built-in Blacklisted Directory Pruning
#[test]
fn test_deep_nested_default_directory_blacklist_pruning() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Verify all 19 default blacklist directories at different nesting levels
    let nesting_levels = ["", "services/api", "packages/core/subpkg", "a/b/c/d/e"];

    for (i, dir_name) in DEFAULT_IGNORED_DIRS.iter().enumerate() {
        let parent_prefix = nesting_levels[i % nesting_levels.len()];
        let full_dir = if parent_prefix.is_empty() {
            root.join(dir_name)
        } else {
            root.join(parent_prefix).join(dir_name)
        };

        fs::create_dir_all(&full_dir).unwrap();
        fs::write(
            full_dir.join("blacklisted_code.rs"),
            "fn blacklisted() { panic!(); }",
        )
        .unwrap();
        fs::write(
            full_dir.join("sub_code.ts"),
            "export const blacklisted = true;",
        )
        .unwrap();
    }

    // Place legitimate source files alongside
    let valid_locations = [
        root.join("main.rs"),
        root.join("services/api/handler.rs"),
        root.join("packages/core/subpkg/lib.ts"),
        root.join("a/b/c/d/e/leaf.py"),
    ];

    for path in &valid_locations {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "# Valid source code\n").unwrap();
    }

    let config = TraversalConfig::default();
    let files = ProjectWalker::collect_files(root, &config);

    assert_eq!(
        files.len(),
        valid_locations.len(),
        "Expected exactly {} valid files, but got {}. Files: {:?}",
        valid_locations.len(),
        files.len(),
        files
    );

    for path in &valid_locations {
        assert!(
            files.contains(path),
            "Expected collected files to contain {}",
            path.display()
        );
    }
}

/// Test 3: Blacklisted File Patterns, Lockfiles, Minified JS, and Case Variations
#[test]
fn test_default_file_blacklist_comprehensive() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let blacklisted_files = [
        "package-lock.json",
        "Cargo.lock",
        "yarn.lock",
        "pnpm-lock.yaml",
        "poetry.lock",
        "custom.lock",
        "bundle.min.js",
        "vendor.bundle.js",
        "main.js.map",
        "module.wasm",
        "cached.pyc",
    ];

    for file_name in &blacklisted_files {
        fs::write(root.join(file_name), "raw content").unwrap();
    }

    // Also test case-insensitive matching for extensions (e.g. .WASM, .PYC, .MAP)
    let case_variants = ["DATA.WASM", "MODULE.PYC", "APP.MAP", "TEST.LOCK"];
    for file_name in &case_variants {
        fs::write(root.join(file_name), "variant content").unwrap();
    }

    // Legitimate files with similar but valid names
    let legitimate_files = [
        "lock_manager.rs",
        "min_finder.ts",
        "wasm_loader.rs",
        "pyc_parser.py",
        "bundle_builder.go",
    ];
    for file_name in &legitimate_files {
        fs::write(root.join(file_name), "// Valid source").unwrap();
    }

    let config = TraversalConfig::default();
    let files = ProjectWalker::collect_files(root, &config);
    let names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    for file_name in &legitimate_files {
        assert!(
            names.iter().any(|n| n == *file_name),
            "Legitimate file {file_name} must be collected"
        );
    }

    for file_name in &blacklisted_files {
        assert!(
            !names.iter().any(|n| n == *file_name),
            "Blacklisted file {file_name} must NOT be collected"
        );
    }

    for file_name in &case_variants {
        assert!(
            !names.iter().any(|n| n == *file_name),
            "Case variant {file_name} must NOT be collected"
        );
    }
}

/// Test 4: Binary File Detection Boundary Conditions & UTF-8 Splitting
#[test]
fn test_binary_detection_boundary_conditions() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Null byte at index 0 (first byte)
    let bin_start = root.join("null_start.dat");
    let mut data_start = vec![0u8; 100];
    let test_msg = b"hello world test content here";
    data_start[1..=test_msg.len()].copy_from_slice(test_msg);
    fs::write(&bin_start, &data_start).unwrap();
    assert!(is_binary_file(&bin_start));

    // 2. Null byte at index 512 (middle of 1024 buffer)
    let bin_mid = root.join("null_mid.dat");
    let mut data_mid = vec![b'a'; 1024];
    data_mid[512] = 0;
    fs::write(&bin_mid, &data_mid).unwrap();
    assert!(is_binary_file(&bin_mid));

    // 3. Null byte at index 1023 (last byte of 1024 buffer)
    let bin_end = root.join("null_end.dat");
    let mut data_end = vec![b'a'; 1024];
    data_end[1023] = 0;
    fs::write(&bin_end, &data_end).unwrap();
    assert!(is_binary_file(&bin_end));

    // 4. Invalid UTF-8 bytes (raw binary 0xFF, 0xFE, 0xFD)
    let raw_bin = root.join("raw_binary.bin");
    fs::write(&raw_bin, [0xFF, 0xFE, 0xFD, 0xFC, 0xFB]).unwrap();
    assert!(is_binary_file(&raw_bin));

    // 5. UTF-8 multi-byte sequence split across 1024 buffer boundary:
    // 1022 ASCII bytes + 4-byte UTF-8 emoji '🚀' ([0xF0, 0x9F, 0x99, 0x80])
    // The first 2 bytes [0xF0, 0x9F] fall within 1024 bytes buffer, the rest outside.
    let utf8_split = root.join("utf8_split.ts");
    let mut split_content = "a".repeat(1022);
    split_content.push('🚀'); // takes 4 bytes: indices 1022, 1023, 1024, 1025
    split_content.push_str("\nexport const x = 1;");
    fs::write(&utf8_split, &split_content).unwrap();

    // Must NOT be detected as binary because incomplete UTF-8 at boundary is handled gracefully
    assert!(
        !is_binary_file(&utf8_split),
        "UTF-8 multi-byte split across 1024 boundary must not be classified as binary"
    );

    // 6. UTF-8 with BOM and Unicode characters (Cyrillic, CJK, Emoji)
    let unicode_file = root.join("unicode.rs");
    let mut bom_unicode = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
    bom_unicode
        .extend_from_slice("pub fn привет_мир() -> &'static str { \"世界 🌟\" }\n".as_bytes());
    fs::write(&unicode_file, &bom_unicode).unwrap();
    assert!(
        !is_binary_file(&unicode_file),
        "Valid UTF-8 with BOM and Unicode must not be classified as binary"
    );

    // 7. Single byte files
    let single_ascii = root.join("single_ascii.txt");
    fs::write(&single_ascii, b"x").unwrap();
    assert!(!is_binary_file(&single_ascii));

    let single_null = root.join("single_null.txt");
    fs::write(&single_null, b"\0").unwrap();
    assert!(is_binary_file(&single_null));

    // Verify ProjectWalker skips all binary files
    let config = TraversalConfig::default();
    let collected = ProjectWalker::collect_files(root, &config);
    let collected_names: Vec<String> = collected
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(collected_names.contains(&"utf8_split.ts".to_string()));
    assert!(collected_names.contains(&"unicode.rs".to_string()));
    assert!(collected_names.contains(&"single_ascii.txt".to_string()));
    assert!(!collected_names.contains(&"null_start.dat".to_string()));
    assert!(!collected_names.contains(&"null_mid.dat".to_string()));
    assert!(!collected_names.contains(&"null_end.dat".to_string()));
    assert!(!collected_names.contains(&"raw_binary.bin".to_string()));
    assert!(!collected_names.contains(&"single_null.txt".to_string()));
}

/// Test 5: Max File Size Boundary Limits (Exact, Off-by-one, Exceeded)
#[test]
fn test_max_file_size_boundary_limits() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let max_limit: u64 = 4096; // 4 KB limit

    // File 1: Size exactly equal to max_limit
    let exact_file = root.join("exact.rs");
    fs::write(&exact_file, vec![b'a'; max_limit as usize]).unwrap();

    // File 2: Size = max_limit - 1
    let below_file = root.join("below.rs");
    fs::write(&below_file, vec![b'b'; (max_limit - 1) as usize]).unwrap();

    // File 3: Size = max_limit + 1
    let above_file = root.join("above.rs");
    fs::write(&above_file, vec![b'c'; (max_limit + 1) as usize]).unwrap();

    // File 4: Huge 100 KB file
    let huge_file = root.join("huge.rs");
    fs::write(&huge_file, vec![b'd'; 100 * 1024]).unwrap();

    let config = TraversalConfig::default().with_max_file_size_bytes(max_limit);
    let files = ProjectWalker::collect_files(root, &config);
    let names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(
        names.contains(&"exact.rs".to_string()),
        "File with size == max_limit must be included"
    );
    assert!(
        names.contains(&"below.rs".to_string()),
        "File with size < max_limit must be included"
    );
    assert!(
        !names.contains(&"above.rs".to_string()),
        "File with size > max_limit must be excluded"
    );
    assert!(
        !names.contains(&"huge.rs".to_string()),
        "File with size >> max_limit must be excluded"
    );
}

/// Test 6: Hidden Files & Directories Filter Flag
#[test]
fn test_hidden_files_and_directories_filtering() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Hidden files
    fs::write(root.join(".env.local"), "SECRET=123").unwrap();
    fs::write(root.join(".config.json"), "{}").unwrap();

    // Hidden directory
    let hidden_dir = root.join(".internal");
    fs::create_dir_all(&hidden_dir).unwrap();
    fs::write(hidden_dir.join("secret_logic.rs"), "fn secret() {}").unwrap();

    // Normal file
    fs::write(root.join("visible.rs"), "fn visible() {}").unwrap();

    // Act 1: include_hidden = false (default)
    let config_default = TraversalConfig::default().with_include_hidden(false);
    let files_default = ProjectWalker::collect_files(root, &config_default);
    assert_eq!(files_default.len(), 1);
    assert_eq!(files_default[0].file_name().unwrap(), "visible.rs");

    // Act 2: include_hidden = true
    let config_hidden = TraversalConfig::default().with_include_hidden(true);
    let files_hidden = ProjectWalker::collect_files(root, &config_hidden);
    let names_hidden: Vec<String> = files_hidden
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(names_hidden.contains(&"visible.rs".to_string()));
    assert!(names_hidden.contains(&".env.local".to_string()));
    assert!(names_hidden.contains(&".config.json".to_string()));
    assert!(names_hidden.contains(&"secret_logic.rs".to_string()));
}

/// Test 7: Empirical Fast Token Estimation Performance & Accuracy on 200-File Repository
#[test]
fn test_fast_stats_performance_and_mathematical_invariants() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create a synthetic repository with 200 diverse source files across 5 languages
    // Each file has realistic content (~300-500 tokens)
    let languages = [
        ("rs", SupportedLanguage::Rust),
        ("ts", SupportedLanguage::TypeScript),
        ("js", SupportedLanguage::JavaScript),
        ("py", SupportedLanguage::Python),
        ("go", SupportedLanguage::Go),
    ];

    for i in 0..200 {
        let (ext, lang) = languages[i % languages.len()];
        let dir_index = i / 20;
        let sub_dir = root.join(format!("module_{dir_index}"));
        fs::create_dir_all(&sub_dir).unwrap();

        let file_path = sub_dir.join(format!("file_{i}.{ext}"));
        let code = match lang {
            SupportedLanguage::Rust => format!(
                r"
pub struct ServiceRecord{i} {{
    pub id: u64,
    pub title: String,
    pub payload: Vec<u8>,
    pub active: bool,
}}

impl ServiceRecord{i} {{
    pub fn new(id: u64, title: String) -> Self {{
        Self {{
            id,
            title,
            payload: Vec::new(),
            active: true,
        }}
    }}

    pub fn compute_hash(&self) -> u64 {{
        self.id.wrapping_mul(31).wrapping_add({i})
    }}

    pub fn validate(&self) -> bool {{
        !self.title.is_empty() && self.active
    }}
}}
"
            ),
            SupportedLanguage::TypeScript => format!(
                r#"
export interface UserPayload{i} {{
  id: string;
  name: string;
  permissions: string[];
  lastLogin: Date;
}}

export class UserManager{i} {{
  private cache = new Map<string, UserPayload{i}>();

  public async fetchUser(id: string): Promise<UserPayload{i} | undefined> {{
    if (this.cache.has(id)) {{
      return this.cache.get(id);
    }}
    const user: UserPayload{i} = {{
      id,
      name: "User_" + id,
      permissions: ["read", "write"],
      lastLogin: new Date(),
    }};
    this.cache.set(id, user);
    return user;
  }}

  public clear(): void {{
    this.cache.clear();
  }}
}}
"#
            ),
            SupportedLanguage::JavaScript => format!(
                r"
class InventoryWorker{i} {{
  constructor(config) {{
    this.config = config;
    this.items = [];
  }}

  addItem(item) {{
    this.items.push(item);
    return this.items.length;
  }}

  processAll() {{
    return this.items.map(item => ({{
      id: item.id,
      processed: true,
      workerId: {i}
    }}));
  }}
}}

module.exports = {{ InventoryWorker{i} }};
"
            ),
            SupportedLanguage::Python => format!(
                r#"
class BillingService{i}:
    def __init__(self, tenant_id: str):
        self.tenant_id = tenant_id
        self.invoices = []

    def create_invoice(self, amount: float, description: str) -> dict:
        invoice = {{
            "id": f"inv_{i}_{{len(self.invoices)}}",
            "amount": amount,
            "description": description,
            "status": "pending"
        }}
        self.invoices.append(invoice)
        return invoice

    def total_revenue(self) -> float:
        return sum(inv["amount"] for inv in self.invoices)
"#
            ),
            SupportedLanguage::Go => format!(
                r#"
package module

import (
    "fmt"
    "time"
)

type AuditLog{i} struct {{
    Id        int64     `json:"id"`
    Action    string    `json:"action"`
    Timestamp time.Time `json:"timestamp"`
}}

type AuditService{i} struct {{
    Logs []AuditLog{i}
}}

func (s *AuditService{i}) Record(action string) AuditLog{i} {{
    log := AuditLog{i}{{
        Id:        int64(len(s.Logs) + {i}),
        Action:    action,
        Timestamp: time.Now(),
    }}
    s.Logs = append(s.Logs, log)
    return log
}}
"#
            ),
            _ => String::new(),
        };
        fs::write(&file_path, code).unwrap();
    }

    // Benchmark execution time
    let start = Instant::now();
    let report = ProjectWalker::estimate_fast_stats(root, Some(10)).unwrap();
    let elapsed = start.elapsed();

    // Invariant 1: Speed — 200 files should scan rapidly (< 2000ms in debug, < 50ms in release)
    assert!(
        elapsed.as_millis() < 2500,
        "Fast stats scan took too long: {} ms for 200 files",
        elapsed.as_millis()
    );

    // Invariant 2: Totals accuracy
    assert_eq!(
        report.total_files, 200,
        "Report should have analyzed exactly 200 files"
    );
    assert!(
        report.total_lines > 2000,
        "Total lines should exceed 2000. Got: {}",
        report.total_lines
    );
    assert!(
        report.estimated_raw_tokens > 10000,
        "Raw tokens should exceed 10000. Got: {}",
        report.estimated_raw_tokens
    );

    // Invariant 3: Reduction bounds
    assert!(
        report.estimated_sliced_tokens > 0,
        "Sliced tokens must be positive"
    );
    assert!(
        report.estimated_sliced_tokens < report.estimated_raw_tokens,
        "Sliced tokens ({}) must be strictly less than raw tokens ({})",
        report.estimated_sliced_tokens,
        report.estimated_raw_tokens
    );
    assert!(
        report.estimated_savings_percentage > 0.0 && report.estimated_savings_percentage < 100.0,
        "Savings percentage ({:.2}%) must be in (0.0, 100.0)",
        report.estimated_savings_percentage
    );

    // Invariant 4: Language Breakdown distribution
    assert_eq!(
        report.language_breakdown.len(),
        5,
        "Must contain breakdown for all 5 supported languages"
    );
    let mut total_breakdown_files = 0;
    for item in &report.language_breakdown {
        assert_eq!(
            item.file_count, 40,
            "Language {} should have 40 files",
            item.language
        );
        total_breakdown_files += item.file_count;
    }
    assert_eq!(total_breakdown_files, 200);
}

/// Test 8: Power-law Token Estimation Boundary Values
#[test]
fn test_power_law_estimation_scaling_and_language_weights() {
    // 1. Zero tokens -> 0 sliced
    assert_eq!(estimate_sliced_tokens(0, 0, None), 0);

    // 2. Minimal tokens (<= 20) -> equal to raw tokens (no artificial expansion or compression)
    for raw in 1..=20 {
        assert_eq!(
            estimate_sliced_tokens(raw, 1, Some(SupportedLanguage::Rust)),
            raw,
            "For small token count {raw}, sliced tokens must equal raw tokens"
        );
    }

    // 3. Language weight ordering: Rust (1.05) > Go (1.02) > TS (1.00) > Python (0.95)
    let raw = 1000;
    let rust_est = estimate_sliced_tokens(raw, 50, Some(SupportedLanguage::Rust));
    let go_est = estimate_sliced_tokens(raw, 50, Some(SupportedLanguage::Go));
    let ts_est = estimate_sliced_tokens(raw, 50, Some(SupportedLanguage::TypeScript));
    let py_est = estimate_sliced_tokens(raw, 50, Some(SupportedLanguage::Python));

    assert!(
        rust_est >= go_est,
        "Rust estimate ({rust_est}) should be >= Go estimate ({go_est})"
    );
    assert!(
        go_est >= ts_est,
        "Go estimate ({go_est}) should be >= TS estimate ({ts_est})"
    );
    assert!(
        ts_est >= py_est,
        "TS estimate ({ts_est}) should be >= Python estimate ({py_est})"
    );

    // 4. Extreme large file (100,000 tokens) -> substantial sublinear compression
    let extreme_raw = 100_000;
    let extreme_sliced = estimate_sliced_tokens(extreme_raw, 5000, Some(SupportedLanguage::Rust));
    assert!(
        extreme_sliced < 5000,
        "Extreme 100k raw tokens should compress sublinearly to ~2000 tokens, got {extreme_sliced}"
    );
    assert!(
        extreme_sliced >= 15,
        "Sliced estimate must respect clamp floor of 15"
    );
}

/// Test 9: Single-file Walker Target Mode
#[test]
fn test_walker_target_is_single_file() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let single_file = root.join("lone_file.rs");
    fs::write(&single_file, "pub fn standalone() {}").unwrap();

    let config = TraversalConfig::default();
    let files = ProjectWalker::collect_files(&single_file, &config);

    assert_eq!(files.len(), 1);
    assert_eq!(files[0], single_file);

    // Single binary file target should yield empty vec
    let bin_file = root.join("lone_binary.wasm");
    fs::write(&bin_file, [0x00, 0x61, 0x73, 0x6D]).unwrap();
    let bin_files = ProjectWalker::collect_files(&bin_file, &config);
    assert_eq!(bin_files.len(), 0);
}
