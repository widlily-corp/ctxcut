//! Tier 2: Boundary & Corner Cases - Unicode Identifiers & Non-ASCII Paths (`test_unicode_paths.rs`)
//!
//! Verifies byte-offset safety and multi-byte UTF-8 character boundary safety when slicing
//! symbols with Cyrillic, CJK, accents, and emoji identifiers, as well as paths containing spaces and Unicode.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

/// Test 1: Slicing functions and types named with Cyrillic UTF-8 identifiers.
///
/// Arrange: TypeScript file with function `обчислити_податок` and interface `Користувач`.
/// Act: Run `ctxcut slice <path>:обчислити_податок`.
/// Assert: Extracts Cyrillic function body and inlines `Користувач` without byte-slicing panic.
#[test]
fn test_cyrillic_identifiers_and_types() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let ts_code = r#"
export interface Користувач {
    ідентифікатор: string;
    ім_я: string;
    ставка_податку: number;
}

export function обчислити_податок(платник: Користувач, дохід: number): number {
    const сума = дохід * платник.ставка_податку;
    return Math.round(сума * 100) / 100;
}
"#;
    let file_path = temp_dir.path().join("податки.ts");
    fs::write(&file_path, ts_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:обчислити_податок", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command failed on Cyrillic symbols");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("обчислити_податок"), "Must extract Cyrillic function name");
    assert!(stdout.contains("Користувач"), "Must hoist Cyrillic interface");
    assert!(stdout.contains("ставка_податку"), "Must preserve Cyrillic field names");
}

/// Test 2: Slicing functions and models with CJK (Chinese / Japanese / Korean) identifiers in Python.
///
/// Arrange: Python file with function `计算折扣金额` and class `订单数据`.
/// Act: Run `ctxcut slice <path>:计算折扣金额`.
/// Assert: Extracts CJK function body and inlines `订单数据` without UTF-8 boundary errors.
#[test]
fn test_cjk_identifiers_in_python() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let py_code = r#"
class 订单数据:
    def __init__(self, 订单号: str, 原始金额: float, 折扣率: float):
        self.订单号 = 订单号
        self.原始金额 = 原始金额
        self.折扣率 = 折扣率

def 计算折扣金额(订单: 订单数据) -> float:
    """计算实际应付金额"""
    实际支付 = 订单.原始金额 * (1.0 - 订单.折扣率)
    return round(实际支付, 2)
"#;
    let file_path = temp_dir.path().join("订单系统.py");
    fs::write(&file_path, py_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:计算折扣金额", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command failed on CJK symbols");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("计算折扣金额"));
    assert!(stdout.contains("订单数据"));
    assert!(stdout.contains("实际支付"));
}

/// Test 3: Slicing files located in directories with spaces and Unicode characters.
///
/// Arrange: File inside path `my project 🚀/тестова папка/nested folder with spaces/service.ts`.
/// Act: Run `ctxcut slice <path_with_spaces>:calculateTotal`.
/// Assert: Successfully resolves path and extracts function.
#[test]
fn test_paths_with_spaces_and_unicode() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let nested_dir = temp_dir.path().join("my project 🚀").join("папка з пробілами");
    fs::create_dir_all(&nested_dir).unwrap();

    let file_path = nested_dir.join("order service file.ts");
    let code = r#"
export interface Item { price: number; }
export function calculateTotal(items: Item[]): number {
    return items.reduce((sum, item) => sum + item.price, 0);
}
"#;
    fs::write(&file_path, code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:calculateTotal", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command failed on path with spaces and Unicode");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("calculateTotal"));
    assert!(stdout.contains("interface Item"));
}

/// Test 4: Accented Latin characters (Spanish / French / Portuguese) in Go symbols.
///
/// Arrange: Go file with identifiers containing accented vowels (`CalcularDepreciación`).
/// Act: Run `ctxcut slice <path>:CalcularTotal`.
/// Assert: Extracts Go function without character slicing corruption.
#[test]
fn test_accented_latin_characters() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let go_code = r#"
package facturación

type Factura struct {
    Número string
    Monto  float64
}

func CalcularTotal(f Factura, impuesto float64) float64 {
    return f.Monto * (1.0 + impuesto)
}
"#;
    let file_path = temp_dir.path().join("facturación.go");
    fs::write(&file_path, go_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:CalcularTotal", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command failed on accented Go code");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("CalcularTotal"));
    assert!(stdout.contains("Factura"));
}

/// Test 5: Multi-byte Emoji in comments and string literals alongside target function.
///
/// Arrange: Source code containing diverse multi-byte emojis (🦀, ⚡, 🚀, 💻).
/// Act: Run `ctxcut slice <path>:emojiHelper`.
/// Assert: Preserves emoji character offsets without string index panics.
#[test]
fn test_emoji_in_source_offsets() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let ts_code = r#"
// 🚀 Rocket Launcher Service 🦀⚡
export function emojiHelper(): string {
    const greeting = "Hello 🌍! Status: ✅ Running smoothly 🏎️💨";
    return greeting;
}
"#;
    let file_path = temp_dir.path().join("emoji.ts");
    fs::write(&file_path, ts_code).unwrap();

    // Act
    let runner = CliRunner::new();
    let target = format!("{}:emojiHelper", file_path.to_str().unwrap());
    let output = runner.run(&["slice", &target]).expect("Command failed on emoji code");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("emojiHelper"));
    assert!(stdout.contains("Hello 🌍!"));
}
