//! ASP.NET Core framework semantic analyzer for C# web APIs and controllers.

use crate::error::Result;
use crate::framework::FrameworkAnalyzer;
use crate::model::{CallSignatureStub, ExtractedType, SliceResult};
use crate::parser::AstUtils;
use std::path::Path;
use tree_sitter::Node;

/// ASP.NET Core controller, routing, DTO, and DI dependency analyzer.
#[derive(Debug, Default, Clone, Copy)]
pub struct AspNetCoreAnalyzer;

impl AspNetCoreAnalyzer {
    /// Creates a new `AspNetCoreAnalyzer`.
    pub fn new() -> Self {
        Self
    }
}

impl FrameworkAnalyzer for AspNetCoreAnalyzer {
    fn name(&self) -> &'static str {
        "aspnetcore"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "cs" && !path.as_os_str().is_empty() {
            return false;
        }

        source.contains("Microsoft.AspNetCore")
            || source.contains("[ApiController]")
            || source.contains("ControllerBase")
            || source.contains(": Controller")
            || source.contains("[Route(")
            || source.contains("[HttpGet")
            || source.contains("[HttpPost")
            || source.contains("[HttpPut")
            || source.contains("[HttpDelete")
            || source.contains("[HttpPatch]")
    }

    fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let mut root = target_node;
        while let Some(parent) = root.parent() {
            root = parent;
        }

        let mut dto_names = Vec::new();
        let mut di_dependencies = Vec::new();

        let search_text = format!("{}\n{}", source, slice.target_symbol.body);

        // 1. Scan for DI constructor parameters in controller classes
        let constructors = AstUtils::find_descendants_by_kind(root, "constructor_declaration");
        for ctor in constructors {
            if let Some(params) = ctor.child_by_field_name("parameters") {
                for param in params.named_children(&mut params.walk()) {
                    if param.kind() == "parameter" {
                        let p_type = param.child_by_field_name("type");
                        let p_name = param.child_by_field_name("name");
                        if let (Some(t_node), Some(n_node)) = (p_type, p_name) {
                            let type_str = AstUtils::node_text(t_node, source).trim();
                            let name_str = AstUtils::node_text(n_node, source).trim();
                            if !is_builtin_aspnet_type(type_str) {
                                di_dependencies.push((type_str.to_string(), name_str.to_string()));
                            }
                        }
                    }
                }
            }
        }

        // 2. Scan for [FromBody], [FromQuery], [FromRoute] action parameters
        for line in search_text.lines() {
            let trimmed = line.trim();
            if trimmed.contains("[FromBody]") || trimmed.contains("[FromQuery]") || trimmed.contains("[FromRoute]") {
                for tag in &["[FromBody]", "[FromQuery]", "[FromRoute]"] {
                    if let Some(pos) = trimmed.find(tag) {
                        let after = &trimmed[pos + tag.len()..].trim();
                        let parts: Vec<&str> = after.split_whitespace().collect();
                        if let Some(type_candidate) = parts.first() {
                            let clean = type_candidate.trim_matches(['(', ')', ',', ';']).trim();
                            if !is_builtin_aspnet_type(clean) && !dto_names.contains(&clean.to_string()) {
                                dto_names.push(clean.to_string());
                            }
                        }
                    }
                }
            }
        }

        // 3. Scan for ActionResult<T> / Task<ActionResult<T>> generic return type
        for line in search_text.lines() {
            if line.contains("ActionResult<") || line.contains("Task<") {
                if let Some(start) = line.find('<') {
                    if let Some(end) = line.rfind('>') {
                        let mut inner = &line[start + 1..end];
                        while let Some(inner_start) = inner.find('<') {
                            if let Some(inner_end) = inner.rfind('>') {
                                inner = &inner[inner_start + 1..inner_end];
                            } else {
                                break;
                            }
                        }
                        let inner_clean = inner.split(',').next().unwrap_or(inner).trim();
                        if !is_builtin_aspnet_type(inner_clean) && !dto_names.contains(&inner_clean.to_string()) {
                            dto_names.push(inner_clean.to_string());
                        }
                    }
                }
            }
        }

        // 4. Add DI constructor dependency stubs
        for (dep_type, dep_name) in di_dependencies {
            let stub_name = format!("DI: {dep_type}");
            if !slice.stripped_calls.iter().any(|c| c.name == stub_name) {
                slice.stripped_calls.push(CallSignatureStub {
                    name: stub_name,
                    receiver: Some(dep_name),
                    file_path: Some(path.to_string_lossy().to_string()),
                    signature: format!("// Injected Service: readonly {dep_type}"),
                });
            }
        }

        // 5. Hoist local DTO definitions if found in AST
        for dto in dto_names {
            if !slice.hoisted_types.iter().any(|t| t.name == dto) {
                if let Some(ty) = find_local_csharp_type(root, source, &dto, path) {
                    slice.hoisted_types.push(ty);
                }
            }
        }

        Ok(())
    }
}

fn is_builtin_aspnet_type(name: &str) -> bool {
    matches!(
        name,
        "int" | "string" | "bool" | "double" | "float" | "decimal" | "byte" | "long"
            | "void" | "object" | "dynamic" | "var" | "Guid" | "DateTime" | "DateTimeOffset"
            | "Task" | "ValueTask" | "IActionResult" | "ActionResult" | "ILogger" | "IConfiguration"
            | "CancellationToken" | "HttpContext" | "HttpRequest" | "HttpResponse"
            | "List" | "IList" | "IEnumerable" | "Dictionary" | "IDictionary"
    )
}

fn find_local_csharp_type(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let classes = AstUtils::find_descendants_by_kind(root, "class_declaration");
    let records = AstUtils::find_descendants_by_kind(root, "record_declaration");
    let interfaces = AstUtils::find_descendants_by_kind(root, "interface_declaration");
    let structs = AstUtils::find_descendants_by_kind(root, "struct_declaration");

    for node in classes {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "class".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    for node in records {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "record".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    for node in interfaces {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "interface".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    for node in structs {
        if let Some(name_node) = node.child_by_field_name("name") {
            if AstUtils::node_text(name_node, source) == target_name {
                return Some(ExtractedType {
                    name: target_name.to_string(),
                    kind: "struct".to_string(),
                    file_path: file_path.to_string_lossy().to_string(),
                    definition: AstUtils::node_text(node, source).to_string(),
                });
            }
        }
    }

    None
}
