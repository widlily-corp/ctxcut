//! Mock contract synthesizer for isolated agent unit testing.
//!
//! Generates typed mock implementations and test doubles for external boundary dependencies,
//! enabling concurrent agents to execute unit tests without spinning up external services.

use crate::model::{CallSignatureStub, ExtractedType, SupportedLanguage};

/// Synthesizes mock test contracts for an agent pack.
pub struct MockContractGenerator;

impl MockContractGenerator {
    /// Generates mock test double definitions for all given boundary stubs and types.
    pub fn generate_mocks(
        agent_id: &str,
        boundary_stubs: &[CallSignatureStub],
        boundary_types: &[ExtractedType],
        lang: SupportedLanguage,
    ) -> String {
        if boundary_stubs.is_empty() && boundary_types.is_empty() {
            return match lang {
                SupportedLanguage::Python => {
                    format!("# No external boundary dependencies detected for {agent_id}.")
                }
                _ => format!("// No external boundary dependencies detected for {agent_id}."),
            };
        }

        match lang {
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => {
                generate_ts_mocks(agent_id, boundary_stubs, boundary_types)
            }
            SupportedLanguage::Rust => {
                generate_rust_mocks(agent_id, boundary_stubs, boundary_types)
            }
            SupportedLanguage::Python => {
                generate_python_mocks(agent_id, boundary_stubs, boundary_types)
            }
            SupportedLanguage::Go => {
                generate_go_mocks(agent_id, boundary_stubs, boundary_types)
            }
            SupportedLanguage::CSharp => {
                generate_csharp_mocks(agent_id, boundary_stubs, boundary_types)
            }
            SupportedLanguage::Java | SupportedLanguage::Kotlin => {
                generate_java_mocks(agent_id, boundary_stubs, boundary_types)
            }
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                generate_c_cpp_mocks(agent_id, boundary_stubs, boundary_types)
            }
        }
    }
}

fn generate_ts_mocks(
    agent_id: &str,
    stubs: &[CallSignatureStub],
    types: &[ExtractedType],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// =========================================================================\n\
         // MOCK TEST CONTRACTS ({agent_id} Isolated Agent Testing)\n\
         // =========================================================================\n\
         // Auto-generated test doubles for external cluster cut dependencies.\n\n"
    ));

    if !types.is_empty() {
        out.push_str("// Mock Type Fixtures\n");
        for ty in types {
            out.push_str(&format!(
                "export const mock{}: Partial<{}> = {{}};\n",
                ty.name, ty.name
            ));
        }
        out.push('\n');
    }

    if !stubs.is_empty() {
        out.push_str("export const MockExternalContracts = {\n");
        for stub in stubs {
            let fn_name = &stub.name;
            out.push_str(&format!(
                "    {fn_name}: (...args: any[]): any => ({{\n\
                 \x20       __mock: true,\n\
                 \x20       status: 'mock_success',\n\
                 \x20       timestamp: Date.now(),\n\
                 \x20   }}),\n"
            ));
        }
        out.push_str("};\n");
    }

    out
}

fn generate_rust_mocks(
    agent_id: &str,
    stubs: &[CallSignatureStub],
    _types: &[ExtractedType],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// =========================================================================\n\
         // MOCK TEST CONTRACTS ({agent_id} Isolated Agent Testing)\n\
         // =========================================================================\n\
         // Auto-generated test doubles for external cluster cut dependencies.\n\n\
         #[cfg(test)]\n\
         pub mod mock_contracts {{\n\
         \x20   use super::*;\n\n"
    ));

    for stub in stubs {
        let fn_name = &stub.name;
        out.push_str(&format!(
            "    pub fn mock_{fn_name}() -> bool {{\n\
             \x20       true\n\
             \x20   }}\n\n"
        ));
    }

    out.push_str("}\n");
    out
}

fn generate_python_mocks(
    agent_id: &str,
    stubs: &[CallSignatureStub],
    _types: &[ExtractedType],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# =========================================================================\n\
         # MOCK TEST CONTRACTS ({agent_id} Isolated Agent Testing)\n\
         # =========================================================================\n\
         # Auto-generated test doubles for external cluster cut dependencies.\n\n\
         class MockExternalContracts:\n\
         \x20   \"\"\"Mock doubles allowing {agent_id} to test in complete isolation.\"\"\"\n"
    ));

    if stubs.is_empty() {
        out.push_str("    pass\n");
    } else {
        for stub in stubs {
            let fn_name = &stub.name;
            out.push_str(&format!(
                "    @staticmethod\n\
                 \x20   def {fn_name}(*args, **kwargs):\n\
                 \x20       return {{\"status\": \"mock_success\", \"args\": args}}\n\n"
            ));
        }
    }

    out
}

fn generate_go_mocks(
    agent_id: &str,
    stubs: &[CallSignatureStub],
    _types: &[ExtractedType],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// =========================================================================\n\
         // MOCK TEST CONTRACTS ({agent_id} Isolated Agent Testing)\n\
         // =========================================================================\n\
         // Auto-generated test doubles for external cluster cut dependencies.\n\n\
         type MockExternalContracts struct{{}}\n\n"
    ));

    for stub in stubs {
        let fn_name = &stub.name;
        out.push_str(&format!(
            "func (m *MockExternalContracts) Mock_{fn_name}() bool {{\n\
             \x20   return true\n\
             }}\n\n"
        ));
    }

    out
}

fn generate_csharp_mocks(
    agent_id: &str,
    stubs: &[CallSignatureStub],
    _types: &[ExtractedType],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// =========================================================================\n\
         // MOCK TEST CONTRACTS ({agent_id} Isolated Agent Testing)\n\
         // =========================================================================\n\
         public static class MockExternalContracts {{\n"
    ));

    for stub in stubs {
        let fn_name = &stub.name;
        out.push_str(&format!(
            "    public static object Mock_{fn_name}() => new {{ Status = \"mock_ok\" }};\n"
        ));
    }

    out.push_str("}\n");
    out
}

fn generate_java_mocks(
    agent_id: &str,
    stubs: &[CallSignatureStub],
    _types: &[ExtractedType],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// =========================================================================\n\
         // MOCK TEST CONTRACTS ({agent_id} Isolated Agent Testing)\n\
         // =========================================================================\n\
         public class MockExternalContracts {{\n"
    ));

    for stub in stubs {
        let fn_name = &stub.name;
        out.push_str(&format!(
            "    public static Object mock_{fn_name}() {{ return true; }}\n"
        ));
    }

    out.push_str("}\n");
    out
}

fn generate_c_cpp_mocks(
    agent_id: &str,
    stubs: &[CallSignatureStub],
    _types: &[ExtractedType],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// =========================================================================\n\
         // MOCK TEST CONTRACTS ({agent_id} Isolated Agent Testing)\n\
         // =========================================================================\n"
    ));

    for stub in stubs {
        let fn_name = &stub.name;
        out.push_str(&format!(
            "inline int mock_{fn_name}() {{ return 1; }}\n"
        ));
    }

    out
}
