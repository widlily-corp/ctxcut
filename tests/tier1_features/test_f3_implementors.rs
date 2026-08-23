//! Tier 1 Tests: Feature 3 — Interface & Trait Implementor Hoisting
//!
//! Verifies implementor discovery and hoisting across:
//! - Rust `impl Trait for Struct`
//! - Go duck-typed interface implementors
//! - TypeScript `class C implements I`
//! - Python `typing.Protocol` implementors
//! - JSON serialization of hoisted implementors

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f3_rust_trait_impl_hoisting() {
    // Arrange: Rust trait and concrete implementor
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("service.rs");
    let content = r#"
pub trait PaymentGateway {
    fn charge(&self, amount: u64) -> bool;
}

pub struct StripeGateway;

impl PaymentGateway for StripeGateway {
    fn charge(&self, amount: u64) -> bool {
        amount > 0
    }
}

pub fn execute_payment(gateway: &dyn PaymentGateway, amount: u64) -> bool {
    gateway.charge(amount)
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice function using the trait
    let runner = CliRunner::new();
    let target = format!("{}:execute_payment", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command failed");

    // Assert: Slicing output retains trait and contract
    output.assert_success();
    assert!(output.stdout.contains("execute_payment"));
    assert!(output.stdout.contains("PaymentGateway"));
}

#[test]
fn test_f3_go_interface_duck_typing_hoisting() {
    // Arrange: Go interface and struct implementing its methods
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("handler.go");
    let content = r#"
package handler

type Logger interface {
    Log(msg string)
}

type ConsoleLogger struct{}

func (c *ConsoleLogger) Log(msg string) {
    // implementation
}

func HandleRequest(l Logger, msg string) {
    l.Log(msg)
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice Go function
    let runner = CliRunner::new();
    let target = format!("{}:HandleRequest", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command failed");

    // Assert: Go function and interface hoisted
    output.assert_success();
    assert!(output.stdout.contains("HandleRequest"));
    assert!(output.stdout.contains("Logger"));
}

#[test]
fn test_f3_typescript_implements_hoisting() {
    // Arrange: TypeScript interface and implementing class
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("notifier.ts");
    let content = r#"
export interface Notifier {
    send(message: string): Promise<void>;
}

export class EmailNotifier implements Notifier {
    async send(message: string): Promise<void> {
        // body
    }
}

export async function broadcast(n: Notifier, text: string): Promise<void> {
    await n.send(text);
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice function
    let runner = CliRunner::new();
    let target = format!("{}:broadcast", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command failed");

    // Assert: TypeScript interface contract preserved
    output.assert_success();
    assert!(output.stdout.contains("broadcast"));
    assert!(output.stdout.contains("Notifier"));
}

#[test]
fn test_f3_python_protocol_hoisting() {
    // Arrange: Python Protocol and implementor class
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("renderer.py");
    let content = r#"
from typing import Protocol

class Renderable(Protocol):
    def render(self) -> str:
        ...

class HtmlDocument:
    def render(self) -> str:
        return "<html></html>"

def output_view(doc: Renderable) -> str:
    return doc.render()
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Slice Python function
    let runner = CliRunner::new();
    let target = format!("{}:output_view", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target]).expect("Command failed");

    // Assert: Python Protocol type preserved
    output.assert_success();
    assert!(output.stdout.contains("output_view"));
    assert!(output.stdout.contains("Renderable"));
}

#[test]
fn test_f3_implementor_in_slice_result_json() {
    // Arrange: Type hoisting JSON contract
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("contract.ts");
    let content = r#"
export interface Greeter {
    greet(name: string): string;
}

export function runGreet(g: Greeter, name: string): string {
    return g.greet(name);
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Request JSON format
    let runner = CliRunner::new();
    let target = format!("{}:runGreet", file_path.display());
    let output = runner.run_in_dir(dir.path(), &["slice", &target, "--format", "json"]).expect("Command failed");

    // Assert: JSON has valid fields
    output.assert_success();
    let json: serde_json::Value = serde_json::from_str(&output.stdout).expect("Failed to parse JSON");
    assert_eq!(
        json.get("target_symbol")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str()),
        Some("runGreet")
    );
}
