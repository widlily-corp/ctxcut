#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_self,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

use crate::error::Result;
use crate::framework::FrameworkAnalyzer;
use crate::model::{CallSignatureStub, ExtractedType, SliceResult};
use crate::parser::AstUtils;
use crate::fullstack::model::ServerRouteEndpoint;
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

    /// Extracts all server route endpoints from a C# ASP.NET Core source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        let lines: Vec<&str> = source.lines().collect();
        let mut controller_base_route = String::new();
        let mut controller_name = String::new();

        // 1. Scan controller class name and base [Route("...")]
        for line in &lines {
            let t = line.trim();
            if t.starts_with("[Route(\"") {
                if let Some(pos) = t.find("[Route(\"") {
                    let after = &t[pos + 8..];
                    if let Some(end) = after.find('"') {
                        controller_base_route = after[..end].to_string();
                    }
                }
            }
            if t.contains("class ") && t.contains("Controller") {
                if let Some(pos) = t.find("class ") {
                    let after = &t[pos + 6..];
                    let name = after.split([' ', ':', '{']).next().unwrap_or("").trim();
                    if name.ends_with("Controller") {
                        controller_name = name.trim_end_matches("Controller").to_string();
                    }
                }
            }
        }

        let base_prefix = if !controller_base_route.is_empty() {
            let mut p = controller_base_route.replace("[controller]", &controller_name.to_lowercase());
            if !p.starts_with('/') {
                p = format!("/{p}");
            }
            p
        } else if !controller_name.is_empty() {
            format!("/api/{}", controller_name.to_lowercase())
        } else {
            String::new()
        };

        // 2. Scan action methods: [HttpGet], [HttpPost], etc.
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            for method in &["HttpGet", "HttpPost", "HttpPut", "HttpDelete", "HttpPatch"] {
                let http_m = method.trim_start_matches("Http").to_uppercase();
                if t.contains(&format!("[{method}")) {
                    let sub_path = if t.contains(&format!("[{method}(\"")) {
                        let pat = format!("[{method}(\"");
                        let pos = t.find(&pat).unwrap();
                        let after = &t[pos + pat.len()..];
                        let end = after.find('"').unwrap_or(0);
                        let sp = &after[..end];
                        if sp.starts_with('/') {
                            sp.to_string()
                        } else {
                            format!("/{sp}")
                        }
                    } else {
                        String::new()
                    };

                    let full_path = format!("{}{}", base_prefix.trim_end_matches('/'), sub_path);
                    let final_path = if full_path.is_empty() { "/".to_string() } else { full_path };

                    // Next line or subsequent lines contain method signature
                    let mut handler_name = "Action".to_string();
                    let mut handler_sig = String::new();
                    for next_line in lines.iter().skip(i + 1) {
                        let nt = next_line.trim();
                        if nt.starts_with("public ") {
                            handler_sig = nt.to_string();
                            let after_public = nt.trim_start_matches("public ");
                            if let Some(paren) = after_public.find('(') {
                                let before_paren = &after_public[..paren];
                                handler_name = before_paren.split_whitespace().last().unwrap_or("Action").to_string();
                            }
                            break;
                        }
                    }

                    let (req_dto, res_dto) = extract_aspnet_action_dtos(source, &handler_sig, &file_path);

                    routes.push(ServerRouteEndpoint {
                        framework: "aspnetcore".to_string(),
                        http_method: http_m,
                        route_path: final_path,
                        handler_file: file_path.clone(),
                        handler_symbol: handler_name,
                        handler_signature: handler_sig,
                        request_dto_type: req_dto,
                        response_dto_type: res_dto,
                    });
                }
            }

            // 3. Minimal APIs: app.MapGet("/...", ...), app.MapPost(...)
            for method in &["MapGet", "MapPost", "MapPut", "MapDelete", "MapPatch"] {
                let http_m = method.trim_start_matches("Map").to_uppercase();
                let pat = format!(".{method}(\"");
                if t.contains(&pat) {
                    if let Some(pos) = t.find(&pat) {
                        let after = &t[pos + pat.len()..];
                        if let Some(end) = after.find('"') {
                            let path_str = &after[..end];
                            let (req_dto, res_dto) = extract_aspnet_action_dtos(source, t, &file_path);
                            routes.push(ServerRouteEndpoint {
                                framework: "aspnetcore".to_string(),
                                http_method: http_m.clone(),
                                route_path: path_str.to_string(),
                                handler_file: file_path.clone(),
                                handler_symbol: format!("MinimalApi_{http_m}"),
                                handler_signature: t.to_string(),
                                request_dto_type: req_dto,
                                response_dto_type: res_dto,
                            });
                        }
                    }
                }
            }
        }

        // Deduplicate
        let mut unique = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for r in routes {
            let key = (r.http_method.clone(), r.route_path.clone(), r.handler_symbol.clone());
            if seen.insert(key) {
                unique.push(r);
            }
        }

        unique
    }
}

fn extract_aspnet_action_dtos(
    source: &str,
    sig: &str,
    file_path: &str,
) -> (Option<ExtractedType>, Option<ExtractedType>) {
    let mut req_dto = None;
    let mut res_dto = None;

    if sig.contains("[FromBody]") {
        if let Some(pos) = sig.find("[FromBody]") {
            let after = &sig[pos + 10..].trim();
            let type_name = after.split_whitespace().next().unwrap_or("").trim_matches([',', ')']);
            if !type_name.is_empty() {
                req_dto = find_type_declaration(source, type_name, Path::new(file_path));
            }
        }
    }

    if sig.contains("ActionResult<") {
        if let Some(pos) = sig.find("ActionResult<") {
            let after = &sig[pos + 13..];
            if let Some(end) = after.find('>') {
                let type_name = after[..end].trim();
                if !type_name.is_empty() {
                    res_dto = find_type_declaration(source, type_name, Path::new(file_path));
                }
            }
        }
    }

    (req_dto, res_dto)
}

fn find_type_declaration(source: &str, name: &str, file_path: &Path) -> Option<ExtractedType> {
    for line in source.lines() {
        let t = line.trim();
        if t.contains(&format!("class {name}")) || t.contains(&format!("record {name}")) || t.contains(&format!("struct {name}")) {
            return Some(ExtractedType {
                name: name.to_string(),
                kind: "class".to_string(),
                file_path: file_path.to_string_lossy().to_string(),
                definition: t.to_string(),
            });
        }
    }
    None
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
            if trimmed.contains("[FromBody]")
                || trimmed.contains("[FromQuery]")
                || trimmed.contains("[FromRoute]")
            {
                for tag in &["[FromBody]", "[FromQuery]", "[FromRoute]"] {
                    if let Some(pos) = trimmed.find(tag) {
                        let after = &trimmed[pos + tag.len()..].trim();
                        let parts: Vec<&str> = after.split_whitespace().collect();
                        if let Some(type_candidate) = parts.first() {
                            let clean = type_candidate.trim_matches(['(', ')', ',', ';']).trim();
                            if !is_builtin_aspnet_type(clean)
                                && !dto_names.contains(&clean.to_string())
                            {
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
                        if !is_builtin_aspnet_type(inner_clean)
                            && !dto_names.contains(&inner_clean.to_string())
                        {
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
        "int"
            | "string"
            | "bool"
            | "double"
            | "float"
            | "decimal"
            | "byte"
            | "long"
            | "void"
            | "object"
            | "dynamic"
            | "var"
            | "Guid"
            | "DateTime"
            | "DateTimeOffset"
            | "Task"
            | "ValueTask"
            | "IActionResult"
            | "ActionResult"
            | "ILogger"
            | "IConfiguration"
            | "CancellationToken"
            | "HttpContext"
            | "HttpRequest"
            | "HttpResponse"
            | "List"
            | "IList"
            | "IEnumerable"
            | "Dictionary"
            | "IDictionary"
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
