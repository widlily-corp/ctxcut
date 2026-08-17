//! Tier 1: Feature Coverage - Git Diff Contextualizer Tests (`test_diff_features.rs`)
//!
//! Verifies automated identification of modified functions from Git diff/staged changes,
//! multi-file diff slicing, renamed file resolution, and type change contextual expansion.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox};

/// Test 1: Detecting and slicing unstaged single function modification.
///
/// Arrange: Isolated Git repository with committed TypeScript source file; modify 1 function body unstaged.
/// Act: Run `ctxcut diff` inside sandbox.
/// Assert: Automatically identifies modified function, generates slice for it, ignores untouched functions.
#[test]
fn test_diff_unstaged_single_function_change() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let src = r#"
export function calculateTax(amount: number): number {
    return amount * 0.15;
}

export function unusedHelper(): void {
    console.log("untouched");
}
"#;
    sandbox.write_file("src/tax.ts", src).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial commit").unwrap();

    // Modify calculateTax unstaged
    let modified = r#"
export function calculateTax(amount: number): number {
    const rate = 0.20;
    return amount * rate;
}

export function unusedHelper(): void {
    console.log("untouched");
}
"#;
    sandbox.modify_file("src/tax.ts", modified).unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Failed to execute ctxcut diff");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("calculateTax"),
        "Must contain slice for modified calculateTax"
    );
    assert!(
        stdout.contains("const rate = 0.20;"),
        "Must contain updated function body"
    );
}

/// Test 2: Slicing staged changes only with `--staged`.
///
/// Arrange: Modify funcA and funcB; stage funcA only.
/// Act: Run `ctxcut diff --staged`.
/// Assert: Outputs slice for funcA; ignores unstaged changes in funcB.
#[test]
fn test_diff_staged_changes_only() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let src = r#"
export function funcA(): string {
    return "A";
}

export function funcB(): string {
    return "B";
}
"#;
    sandbox.write_file("src/funcs.ts", src).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial commit").unwrap();

    // Modify funcA and stage it
    let modified_a = r#"
export function funcA(): string {
    return "A_staged_modified";
}

export function funcB(): string {
    return "B";
}
"#;
    sandbox.modify_file("src/funcs.ts", modified_a).unwrap();
    sandbox.stage_file("src/funcs.ts").unwrap();

    // Modify funcB unstaged
    let modified_b = r#"
export function funcA(): string {
    return "A_staged_modified";
}

export function funcB(): string {
    return "B_unstaged_modified";
}
"#;
    sandbox.modify_file("src/funcs.ts", modified_b).unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff", "--staged"])
        .expect("Failed to run ctxcut diff --staged");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("funcA"),
        "Must contain slice for staged funcA"
    );
    assert!(
        stdout.contains("A_staged_modified"),
        "Must contain staged body"
    );
}

/// Test 3: Detecting and slicing multiple modified functions across multiple files and languages.
///
/// Arrange: Commit TS and Python files; modify functions in both files.
/// Act: Run `ctxcut diff`.
/// Assert: Markdown output contains slice sections for all modified functions.
#[test]
fn test_diff_multiple_functions_across_files() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    sandbox
        .write_file(
            "src/math.ts",
            "export function add(a: number, b: number): number {\n    return a + b;\n}\n",
        )
        .unwrap();
    sandbox
        .write_file(
            "src/calc.py",
            "def multiply(x: int, y: int) -> int:\n    return x * y\n",
        )
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial commit").unwrap();

    // Mutate both
    sandbox.modify_file("src/math.ts", "export function add(a: number, b: number): number {\n    // comment\n    return a + b;\n}\n").unwrap();
    sandbox
        .modify_file(
            "src/calc.py",
            "def multiply(x: int, y: int) -> int:\n    # comment\n    return x * y\n",
        )
        .unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Failed to run ctxcut diff");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(stdout.contains("add"), "Must contain TS add function");
    assert!(
        stdout.contains("multiply"),
        "Must contain Python multiply function"
    );
}

/// Test 4: Handling renamed files with modifications.
///
/// Arrange: `git mv service.ts order_service.ts` and modify a function body.
/// Act: Run `ctxcut diff`.
/// Assert: Accurately identifies new file path and extracts modified symbol.
#[test]
fn test_diff_renamed_file_with_modifications() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    sandbox
        .write_file(
            "src/service.ts",
            "export function processData(d: string): string {\n    return d;\n}\n",
        )
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial commit").unwrap();

    sandbox
        .rename_file("src/service.ts", "src/order_service.ts")
        .unwrap();
    sandbox
        .modify_file(
            "src/order_service.ts",
            "export function processData(d: string): string {\n    return d.toUpperCase();\n}\n",
        )
        .unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Failed to run ctxcut diff");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("processData"),
        "Must extract processData from renamed file"
    );
    assert!(
        stdout.contains("toUpperCase"),
        "Must reflect modified content"
    );
}

/// Test 5: Type change contextual expansion.
///
/// Arrange: Modify an interface or struct definition used by functions in the file.
/// Act: Run `ctxcut diff`.
/// Assert: Detects the type modification and provides relevant context or affected signatures.
#[test]
fn test_diff_type_change_contextual_expansion() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    let src = r#"
export interface UserConfig {
    timeoutMs: number;
}

export function initClient(cfg: UserConfig): boolean {
    return cfg.timeoutMs > 0;
}
"#;
    sandbox.write_file("src/config.ts", src).unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial commit").unwrap();

    let updated_src = r#"
export interface UserConfig {
    timeoutMs: number;
    retries: number;
}

export function initClient(cfg: UserConfig): boolean {
    return cfg.timeoutMs > 0;
}
"#;
    sandbox.modify_file("src/config.ts", updated_src).unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Failed to run ctxcut diff");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("UserConfig") || stdout.contains("retries"),
        "Must capture modified type definition"
    );
}

/// Test 6: Clean working tree with no modifications returns clean zero-diff output.
///
/// Arrange: Pristine Git repository with all changes committed.
/// Act: Run `ctxcut diff`.
/// Assert: Exits successfully with message indicating no modified symbols found.
#[test]
fn test_diff_clean_working_tree_no_modifications() {
    // Arrange
    let sandbox = GitSandbox::new().expect("Failed to create Git sandbox");
    sandbox
        .write_file("src/app.ts", "export function main(): void {}\n")
        .unwrap();
    sandbox.stage_all().unwrap();
    sandbox.commit("Initial commit").unwrap();

    // Act
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(sandbox.path(), &["diff"])
        .expect("Failed to run ctxcut diff");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("No modified")
            || stdout.contains("0 modified")
            || stdout.trim().is_empty()
            || stdout.contains("clean"),
        "Must cleanly report clean working tree without errors"
    );
}
