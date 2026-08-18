//! Isolated Test Context Generator and Mock Scaffolder (Milestone 6).
//!
//! Assembles target symbol, parameter/return types, generated mock/spy declarations,
//! AAA unit test templates, and nearby reference fixtures.

pub mod fixture_finder;

pub use fixture_finder::FixtureFinder;

use crate::error::Result;
use crate::model::{SliceOptions, TestContextResult};
use crate::slice::ContextSlicer;
use std::path::Path;

/// Test Context Generator assembling target symbol, mock declarations, and test templates.
pub struct TestContextGenerator;

impl TestContextGenerator {
    /// Generates complete isolated test context for a target symbol.
    pub fn generate(
        file_path: &Path,
        symbol_query: &str,
        framework: Option<&str>,
        opts: &SliceOptions,
    ) -> Result<TestContextResult> {
        let slicer = ContextSlicer::new();
        let slice = slicer.slice_symbol(file_path, symbol_query, opts)?;

        let detected_framework = if let Some(f) = framework {
            f.to_lowercase()
        } else {
            Self::detect_test_framework(file_path, &slice.target_symbol.language)
        };

        let mock_scaffolding = Self::generate_mock_scaffolding(&slice, &detected_framework);
        let test_template = Self::generate_test_template(&slice, &detected_framework);
        let reference_fixtures = FixtureFinder::find_fixtures(file_path);

        Ok(TestContextResult {
            slice,
            test_framework: detected_framework,
            mock_scaffolding,
            test_template,
            reference_fixtures,
        })
    }

    fn detect_test_framework(file_path: &Path, language: &str) -> String {
        match language {
            "rust" => "cargo".to_string(),
            "python" => "pytest".to_string(),
            "go" => "gotest".to_string(),
            "typescript" | "javascript" => {
                // Check parent dirs for package.json or config files
                let mut curr = file_path.parent();
                for _ in 0..3 {
                    if let Some(d) = curr {
                        if d.join("vitest.config.ts").exists()
                            || d.join("vitest.config.js").exists()
                        {
                            return "vitest".to_string();
                        }
                        if d.join("jest.config.ts").exists()
                            || d.join("jest.config.js").exists()
                            || d.join("jest.config.json").exists()
                        {
                            return "jest".to_string();
                        }
                        curr = d.parent();
                    } else {
                        break;
                    }
                }
                "vitest".to_string()
            }
            _ => "generic".to_string(),
        }
    }

    fn generate_mock_scaffolding(slice: &crate::model::SliceResult, framework: &str) -> String {
        if slice.stripped_calls.is_empty() {
            return String::new();
        }

        let mut lines = Vec::new();

        match framework {
            "vitest" | "jest" => {
                let mock_fn = if framework == "vitest" {
                    "vi.fn()"
                } else {
                    "jest.fn()"
                };
                for call in &slice.stripped_calls {
                    let clean_name = call
                        .name
                        .replace(|c: char| !c.is_alphanumeric() && c != '_', "");
                    lines.push(format!("// Mock for {}: {}", call.name, call.signature));
                    lines.push(format!(
                        "export const mock_{clean_name} = {mock_fn}.mockResolvedValue(undefined);"
                    ));
                }
            }
            "pytest" => {
                lines.push("from unittest.mock import MagicMock, AsyncMock, patch".to_string());
                for call in &slice.stripped_calls {
                    let clean_name = call
                        .name
                        .replace(|c: char| !c.is_alphanumeric() && c != '_', "");
                    lines.push(format!("# Mock for {}: {}", call.name, call.signature));
                    lines.push(format!("mock_{clean_name} = MagicMock(return_value=None)"));
                }
            }
            "gotest" => {
                for call in &slice.stripped_calls {
                    lines.push(format!(
                        "// Mock stub for {}: {}",
                        call.name, call.signature
                    ));
                }
            }
            "cargo" => {
                for call in &slice.stripped_calls {
                    lines.push(format!(
                        "// Test stub for {}: {}",
                        call.name, call.signature
                    ));
                }
            }
            _ => {
                for call in &slice.stripped_calls {
                    lines.push(format!("// Mock for {}: {}", call.name, call.signature));
                }
            }
        }

        lines.join("\n")
    }

    fn generate_test_template(slice: &crate::model::SliceResult, framework: &str) -> String {
        let sym_name = &slice.target_symbol.name;
        let clean_sym = sym_name.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");

        match framework {
            "vitest" | "jest" => {
                let import_src = framework;
                let mock_reset = if framework == "vitest" {
                    "vi.clearAllMocks();"
                } else {
                    "jest.clearAllMocks();"
                };
                format!(
                    r"import {{ describe, it, expect, beforeEach }} from '{import_src}';

describe('{sym_name}', () => {{
    beforeEach(() => {{
        {mock_reset}
    }});

    it('should execute successfully with valid inputs (AAA Pattern)', async () => {{
        // 1. Arrange: prepare inputs and mock returns
        // TODO: instantiate test inputs from hoisted types

        // 2. Act: invoke target symbol
        const result = await {clean_sym}();

        // 3. Assert: verify results and mock invocations
        expect(result).toBeDefined();
    }});

    it('should handle edge cases and error conditions properly', async () => {{
        // 1. Arrange: set up error or empty inputs

        // 2. Act & Assert: verify expected error handling
    }});
}};"
                )
            }
            "pytest" => {
                format!(
                    r#"import pytest

def test_{clean_sym}_success():
    """Test {sym_name} happy path (AAA Pattern)."""
    # 1. Arrange
    # TODO: instantiate test data and configure mocks

    # 2. Act
    result = {clean_sym}()

    # 3. Assert
    assert result is not None

def test_{clean_sym}_edge_cases():
    """Test {sym_name} boundary conditions."""
    # 1. Arrange & Act & Assert
    pass"#
                )
            }
            "cargo" => {
                format!(
                    r"#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_{clean_sym}_success() {{
        // 1. Arrange: setup inputs from hoisted contracts

        // 2. Act: execute target symbol
        let result = {clean_sym}();

        // 3. Assert: verify invariants
        assert!(result.is_ok());
    }}

    #[test]
    fn test_{clean_sym}_boundary_conditions() {{
        // Arrange & Act & Assert
    }}
}}"
                )
            }
            "gotest" => {
                format!(
                    r#"package main

import (
    "testing"
)

func Test_{clean_sym}_Success(t *testing.T) {{
    // 1. Arrange: prepare test inputs

    // 2. Act: invoke target function
    result := {clean_sym}()

    // 3. Assert: verify outputs
    if result == nil {{
        t.Fatalf("expected non-nil result from {sym_name}")
    }}
}}"#
                )
            }
            _ => {
                format!("// Test template for {sym_name} using {framework}\n// Arrange -> Act -> Assert")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_test_context_generator_end_to_end() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("service.ts");
        let source = r#"
export interface UserDTO {
    id: string;
    email: string;
}

export function sendWelcomeEmail(email: string): boolean {
    return true;
}

export async function registerUser(dto: UserDTO): Promise<{ success: boolean }> {
    sendWelcomeEmail(dto.email);
    return { success: true };
}
"#;
        fs::write(&file_path, source).unwrap();

        let opts = SliceOptions::default();
        let res = TestContextGenerator::generate(&file_path, "registerUser", Some("vitest"), &opts)
            .unwrap();

        assert_eq!(res.slice.target_symbol.name, "registerUser");
        assert_eq!(res.test_framework, "vitest");
        assert!(res.mock_scaffolding.contains("mock_sendWelcomeEmail"));
        assert!(res.test_template.contains("describe('registerUser'"));

        let md = res.to_markdown();
        assert!(md.contains("# Test Context: `registerUser` (vitest)"));
        assert!(md.contains("## 1. Target Symbol"));
        assert!(md.contains("## 2. Generated Mock Scaffolding"));
        assert!(md.contains("## 3. Unit Test Template"));
    }
}
