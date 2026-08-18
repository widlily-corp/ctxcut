//! Comprehensive test suite for smart traversal, ignore rules, and fast token estimation.

use ctxcut_core::traversal::{
    estimate_sliced_tokens, is_binary_bytes, is_binary_file, is_blacklisted_file,
    is_ignored_directory, ProjectWalker, TraversalConfig, DEFAULT_IGNORED_DIRS,
    DEFAULT_IGNORED_FILES,
};
use ctxcut_core::SupportedLanguage;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_default_directory_blacklist_pruning() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    for dir in DEFAULT_IGNORED_DIRS {
        let dir_path = root.join(dir);
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join("file.ts"), "export const x = 1;").unwrap();
    }

    let valid_dir = root.join("src");
    fs::create_dir_all(&valid_dir).unwrap();
    fs::write(valid_dir.join("main.ts"), "export const valid = 1;").unwrap();

    let config = TraversalConfig::default();
    let files = ProjectWalker::collect_files(root, &config);

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_name().unwrap(), "main.ts");
}

#[test]
fn test_default_file_blacklist_filtering() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    for f in DEFAULT_IGNORED_FILES {
        let clean_name = f.strip_prefix('*').unwrap_or(f);
        let file_name = if clean_name.starts_with('.') {
            format!("bundle{clean_name}")
        } else {
            clean_name.to_string()
        };
        fs::write(root.join(&file_name), "ignored content").unwrap();
    }

    fs::write(root.join("service.ts"), "export class Service {}").unwrap();

    let config = TraversalConfig::default();
    let files = ProjectWalker::collect_files(root, &config);

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_name().unwrap(), "service.ts");
}

#[test]
fn test_binary_file_detector() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Binary PNG header with null bytes
    let bin_path = root.join("test.bin");
    fs::write(&bin_path, [0x89, 0x50, 0x4E, 0x47, 0x00, 0x00, 0x01]).unwrap();
    assert!(is_binary_file(&bin_path));
    assert!(is_binary_bytes(&[0x89, 0x50, 0x4E, 0x47, 0x00, 0x00]));

    // 2. Pure UTF-8 text with unicode (Cyrillic + Emoji)
    let txt_path = root.join("test.ts");
    fs::write(
        &txt_path,
        "export const greeting = 'Привет мир 🚀 Lightning fast';",
    )
    .unwrap();
    assert!(!is_binary_file(&txt_path));

    // 3. UTF-8 multi-byte character truncated at 1024-byte buffer boundary
    let mut large_text = "a".repeat(1023);
    large_text.push('🚀'); // 4-byte UTF-8 character crossing index 1024
    let truncated_bytes = &large_text.as_bytes()[..1024];
    assert!(!is_binary_bytes(truncated_bytes));
}

#[test]
fn test_ctxcutignore_custom_rules() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    fs::write(
        root.join(".ctxcutignore"),
        "*.generated.ts\nlegacy/\nignore_me.py\n",
    )
    .unwrap();

    fs::write(root.join("api.generated.ts"), "export const a = 1;").unwrap();
    fs::write(root.join("ignore_me.py"), "print('no')").unwrap();

    let legacy = root.join("legacy");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("old.ts"), "export const b = 2;").unwrap();

    let valid_dir = root.join("src");
    fs::create_dir_all(&valid_dir).unwrap();
    fs::write(valid_dir.join("main.ts"), "export const c = 3;").unwrap();

    let config = TraversalConfig::default();
    let files = ProjectWalker::collect_files(root, &config);

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_name().unwrap(), "main.ts");
}

#[test]
fn test_max_file_size_filtering() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let small_file = root.join("small.ts");
    let large_file = root.join("large.ts");

    fs::write(&small_file, "export const x = 1;").unwrap();
    fs::write(&large_file, "a".repeat(5000)).unwrap();

    let config = TraversalConfig::default().with_max_file_size_bytes(1000);
    let files = ProjectWalker::collect_files(root, &config);

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_name().unwrap(), "small.ts");
}

#[test]
fn test_custom_ignored_dirs_and_files() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let custom_dir = root.join("my_build_dir");
    fs::create_dir_all(&custom_dir).unwrap();
    fs::write(custom_dir.join("output.ts"), "export const out = 1;").unwrap();

    let custom_file = root.join("custom.generated.json");
    fs::write(&custom_file, "{}").unwrap();

    let valid_file = root.join("app.ts");
    fs::write(&valid_file, "export const app = true;").unwrap();

    let config = TraversalConfig::default()
        .with_custom_ignored_dirs(vec!["my_build_dir".to_string()])
        .with_custom_ignored_files(vec!["*.generated.json".to_string()]);

    let files = ProjectWalker::collect_files(root, &config);

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_name().unwrap(), "app.ts");
}

#[test]
fn test_fast_stats_report_aggregation() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let ts_source = r"
export interface User {
  id: string;
  name: string;
  email: string;
  roles: string[];
}

export class UserService {
  private users: Map<string, User> = new Map();

  public async getUser(id: string): Promise<User | undefined> {
    return this.users.get(id);
  }

  public async createUser(user: User): Promise<User> {
    this.users.set(user.id, user);
    return user;
  }

  public async deleteUser(id: string): Promise<boolean> {
    return this.users.delete(id);
  }
}
";

    let py_source = r#"
class PaymentProcessor:
    def __init__(self, api_key: str):
        self.api_key = api_key
        self.transactions = []

    def process_payment(self, amount: float, currency: str) -> bool:
        if amount <= 0:
            raise ValueError("Amount must be positive")
        self.transactions.append({"amount": amount, "currency": currency})
        return True

    def refund_payment(self, transaction_id: str) -> bool:
        return True
"#;

    fs::write(root.join("service.ts"), ts_source).unwrap();
    fs::write(root.join("util.py"), py_source).unwrap();

    let report = ProjectWalker::estimate_fast_stats(root, Some(5)).unwrap();

    assert_eq!(report.total_files, 2);
    assert!(report.total_lines >= 30);
    assert!(report.estimated_raw_tokens > 100);
    assert!(report.estimated_sliced_tokens > 0);
    assert!(report.estimated_sliced_tokens < report.estimated_raw_tokens);
    assert!(report.estimated_savings_percentage > 0.0);
    assert_eq!(report.language_breakdown.len(), 2);
}

#[test]
fn test_fast_stats_single_file() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let file = root.join("main.rs");
    fs::write(
        &file,
        "pub fn calculate(a: usize, b: usize) -> usize {\n    a + b\n}\n",
    )
    .unwrap();

    let report = ProjectWalker::estimate_fast_stats(&file, None).unwrap();

    assert_eq!(report.total_files, 1);
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].language.as_deref(), Some("rust"));
    assert!(report.estimated_raw_tokens > 0);
}

#[test]
fn test_estimate_sliced_tokens_power_law_distribution() {
    // 1. Small file (<= 20 tokens)
    assert_eq!(
        estimate_sliced_tokens(10, 2, Some(SupportedLanguage::Rust)),
        10
    );

    // 2. Medium file (500 tokens)
    let medium = estimate_sliced_tokens(500, 50, Some(SupportedLanguage::TypeScript));
    assert!((100..=200).contains(&medium));

    // 3. Large file (3000 tokens)
    let large = estimate_sliced_tokens(3000, 300, Some(SupportedLanguage::Python));
    assert!((200..=350).contains(&large));

    // 4. Zero tokens
    assert_eq!(estimate_sliced_tokens(0, 0, None), 0);
}

#[test]
fn test_blacklist_predicates() {
    assert!(is_ignored_directory("node_modules", &[]));
    assert!(is_ignored_directory(".venv", &[]));
    assert!(is_ignored_directory(
        "custom_cache",
        &["custom_cache".to_string()]
    ));
    assert!(!is_ignored_directory("src", &[]));

    assert!(is_blacklisted_file("Cargo.lock", &[]));
    assert!(is_blacklisted_file("app.min.js", &[]));
    assert!(is_blacklisted_file("data.wasm", &[]));
    assert!(is_blacklisted_file(
        "file.custom",
        &["*.custom".to_string()]
    ));
    assert!(!is_blacklisted_file("index.ts", &[]));
}
