//! Django, Django REST Framework (DRF), and FastAPI semantic analyzer.
//!
//! Extracts route schemas, Pydantic models, dependency providers (`Depends(...)`),
//! DRF serializers, Django ORM models, permission classes, and ViewSet contracts.

use crate::error::Result;
use crate::framework::FrameworkAnalyzer;
use crate::model::{CallSignatureStub, ExtractedType, SliceResult};
use crate::parser::{AstUtils, ParserManager};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Node};

use crate::fullstack::model::ServerRouteEndpoint;

/// Framework analyzer for Django, Django REST Framework (DRF), and FastAPI.
#[derive(Debug, Default, Clone)]
pub struct DjangoFastApiAnalyzer;

impl DjangoFastApiAnalyzer {
    /// Creates a new `DjangoFastApiAnalyzer` instance.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all server route endpoints from a FastAPI or Django Python source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();

            // FastAPI route decorators: @app.get("/..."), @router.post("/...")
            if t.starts_with('@') && (t.contains(".get(") || t.contains(".post(") || t.contains(".put(") || t.contains(".delete(") || t.contains(".patch(") || t.contains(".api_route(")) {
                for method in &["get", "post", "put", "delete", "patch"] {
                    let pat = format!(".{method}(");
                    if t.contains(&pat) {
                        if let Some(pos) = t.find(&pat) {
                            let after = &t[pos + pat.len()..];
                            if let Some(route_path) = extract_python_string(after) {
                                let mut handler_name = "handler".to_string();
                                let mut handler_sig = String::new();
                                for next_line in lines.iter().skip(i + 1) {
                                    let nt = next_line.trim();
                                    if nt.starts_with("def ") || nt.starts_with("async def ") {
                                        handler_sig = nt.to_string();
                                        let clean = nt.trim_start_matches("async ").trim_start_matches("def ");
                                        handler_name = clean.split(['(', ':']).next().unwrap_or("handler").trim().to_string();
                                        break;
                                    }
                                }

                                let (req_dto, res_dto) = extract_fastapi_dtos(source, t, &handler_sig, &file_path);

                                routes.push(ServerRouteEndpoint {
                                    framework: "fastapi".to_string(),
                                    http_method: method.to_uppercase(),
                                    route_path,
                                    handler_file: file_path.clone(),
                                    handler_symbol: handler_name,
                                    handler_signature: handler_sig,
                                    request_dto_type: req_dto,
                                    response_dto_type: res_dto,
                                });
                            }
                        }
                    }
                }
            }
        }

        routes
    }
}

fn extract_python_string(s: &str) -> Option<String> {
    for quote in ['\'', '"'] {
        if let Some(first) = s.find(quote) {
            if let Some(second) = s[first + 1..].find(quote) {
                let candidate = &s[first + 1..first + 1 + second];
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn extract_fastapi_dtos(
    source: &str,
    decorator_line: &str,
    handler_sig: &str,
    file_path: &str,
) -> (Option<ExtractedType>, Option<ExtractedType>) {
    let mut req_dto = None;
    let mut res_dto = None;

    if decorator_line.contains("response_model=") {
        if let Some(pos) = decorator_line.find("response_model=") {
            let after = &decorator_line[pos + 15..];
            let name = after.split([',', ')', ' ']).next().unwrap_or("").trim();
            if !name.is_empty() {
                res_dto = find_python_model(source, name, file_path);
            }
        }
    }

    if let Some(paren) = handler_sig.find('(') {
        if let Some(end_paren) = handler_sig.rfind(')') {
            let params = &handler_sig[paren + 1..end_paren];
            for param in params.split(',') {
                if let Some((_p_name, p_type)) = param.split_once(':') {
                    let clean_type = p_type.split('=').next().unwrap_or(p_type).trim();
                    if clean_type.contains("BaseModel") || clean_type.ends_with("Schema") || clean_type.ends_with("Dto") || clean_type.ends_with("Request") || clean_type.ends_with("In") {
                        req_dto = find_python_model(source, clean_type, file_path);
                    }
                }
            }
        }
    }

    (req_dto, res_dto)
}

fn find_python_model(source: &str, name: &str, file_path: &str) -> Option<ExtractedType> {
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with(&format!("class {name}(")) || t.starts_with(&format!("class {name}:")) {
            return Some(ExtractedType {
                name: name.to_string(),
                kind: "class".to_string(),
                file_path: file_path.to_string(),
                definition: t.to_string(),
            });
        }
    }
    None
}

/// Extracted metadata from a FastAPI route handler node.
#[derive(Debug, Default, Clone)]
pub struct FastApiRouteMetadata {
    /// HTTP method (e.g. "get", "post", "put", "delete").
    pub http_method: Option<String>,
    /// Route path (e.g. "/items/").
    pub route_path: Option<String>,
    /// Response schema names.
    pub response_models: Vec<String>,
    /// Parameter schema names.
    pub parameter_schemas: Vec<String>,
    /// Dependency provider names (from `Depends(...)`).
    pub dependencies: Vec<String>,
}

/// Extracted metadata from a Django / DRF ViewSet or APIView class node.
#[derive(Debug, Default, Clone)]
pub struct DjangoViewMetadata {
    /// View or ViewSet class name.
    pub view_name: String,
    /// Serializer class names.
    pub serializers: Vec<String>,
    /// Model class names.
    pub models: Vec<String>,
    /// Permission class names.
    pub permissions: Vec<String>,
    /// Pagination class name.
    pub pagination: Option<String>,
    /// Filter backend names.
    pub filter_backends: Vec<String>,
}

#[derive(Debug, Clone)]
struct FoundPythonEntity {
    kind: String,
    file_path: PathBuf,
    minified_def: String,
    function_sig: String,
    inner_model: Option<String>,
    nested_serializers: Vec<String>,
    related_models: Vec<String>,
    nested_schemas: Vec<String>,
}

impl FrameworkAnalyzer for DjangoFastApiAnalyzer {
    fn name(&self) -> &'static str {
        "django_fastapi"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let is_python = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext, "py" | "pyi"))
            .unwrap_or(false);

        if !is_python {
            return false;
        }

        // FastAPI indicators
        let has_fastapi = source.contains("fastapi")
            || source.contains("APIRouter")
            || source.contains("FastAPI")
            || source.contains("Depends")
            || source.contains("BaseModel")
            || source.contains("response_model=")
            || source.contains("@router.")
            || source.contains("@app.");

        // Django / DRF indicators
        let has_django_drf = source.contains("rest_framework")
            || source.contains("serializers")
            || source.contains("viewsets")
            || source.contains("django.db")
            || source.contains("models.Model")
            || source.contains("APIView")
            || source.contains("permission_classes")
            || source.contains("serializer_class")
            || source.contains("BasePermission")
            || source.contains("GenericAPIView");

        has_fastapi || has_django_drf
    }

    fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let ts_lang: Language = tree_sitter_python::LANGUAGE.into();

        // 1. FastAPI extraction
        if let Some(fastapi_meta) = self.extract_fastapi_route_metadata(target_node, source) {
            self.resolve_and_augment_fastapi(
                &fastapi_meta,
                target_node,
                source,
                path,
                &ts_lang,
                slice,
            )?;
        }

        // 2. Django / DRF extraction
        if let Some(django_meta) = self.extract_django_view_metadata(target_node, source) {
            self.resolve_and_augment_django(
                &django_meta,
                target_node,
                source,
                path,
                &ts_lang,
                slice,
            )?;
        }

        Ok(())
    }
}

impl DjangoFastApiAnalyzer {
    /// Extracts FastAPI route metadata from a function or decorated definition node.
    pub fn extract_fastapi_route_metadata(
        &self,
        node: Node<'_>,
        source: &str,
    ) -> Option<FastApiRouteMetadata> {
        let mut meta = FastApiRouteMetadata::default();
        let mut is_fastapi_route = false;

        let dec_parent = if node.kind() == "decorated_definition" {
            Some(node)
        } else {
            node.parent().filter(|p| p.kind() == "decorated_definition")
        };

        if let Some(dec_def) = dec_parent {
            for dec in AstUtils::find_children_by_kind(dec_def, "decorator") {
                if let Some(call) = AstUtils::find_child_by_kind(dec, "call") {
                    if let Some(func) = call.child_by_field_name("function") {
                        let func_text = AstUtils::node_text(func, source);
                        if let Some((_, method)) = func_text.split_once('.') {
                            let method_lower = method.to_lowercase();
                            if matches!(
                                method_lower.as_str(),
                                "get"
                                    | "post"
                                    | "put"
                                    | "delete"
                                    | "patch"
                                    | "options"
                                    | "head"
                                    | "trace"
                                    | "api_route"
                                    | "websocket"
                            ) {
                                is_fastapi_route = true;
                                meta.http_method = Some(method_lower);

                                if let Some(args) = call.child_by_field_name("arguments") {
                                    for arg in args.named_children(&mut args.walk()) {
                                        if arg.kind() == "string" && meta.route_path.is_none() {
                                            meta.route_path = Some(
                                                AstUtils::node_text(arg, source)
                                                    .trim_matches('"')
                                                    .trim_matches('\'')
                                                    .to_string(),
                                            );
                                        } else if arg.kind() == "keyword_argument" {
                                            if let Some(name_n) = arg.child_by_field_name("name") {
                                                let kw_name = AstUtils::node_text(name_n, source);
                                                if kw_name == "response_model" {
                                                    if let Some(val_n) =
                                                        arg.child_by_field_name("value")
                                                    {
                                                        Self::collect_schema_identifiers(
                                                            val_n,
                                                            source,
                                                            &mut meta.response_models,
                                                        );
                                                    }
                                                } else if kw_name == "dependencies" {
                                                    if let Some(val_n) =
                                                        arg.child_by_field_name("value")
                                                    {
                                                        Self::collect_depends_calls(
                                                            val_n,
                                                            source,
                                                            &mut meta.dependencies,
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Inspect function parameters
        let func_node = if node.kind() == "decorated_definition" {
            AstUtils::find_child_by_kind(node, "function_definition")
        } else if node.kind() == "function_definition" {
            Some(node)
        } else {
            None
        };

        if let Some(fn_def) = func_node {
            if let Some(params) = fn_def.child_by_field_name("parameters") {
                for param in params.named_children(&mut params.walk()) {
                    match param.kind() {
                        "typed_parameter" => {
                            if let Some(type_node) = param.child_by_field_name("type") {
                                Self::collect_schema_identifiers(
                                    type_node,
                                    source,
                                    &mut meta.parameter_schemas,
                                );
                                Self::extract_annotated_dependencies(
                                    type_node,
                                    source,
                                    &mut meta.dependencies,
                                    &mut meta.parameter_schemas,
                                );
                            }
                        }
                        "typed_default_parameter" => {
                            if let Some(type_node) = param.child_by_field_name("type") {
                                Self::collect_schema_identifiers(
                                    type_node,
                                    source,
                                    &mut meta.parameter_schemas,
                                );
                                Self::extract_annotated_dependencies(
                                    type_node,
                                    source,
                                    &mut meta.dependencies,
                                    &mut meta.parameter_schemas,
                                );
                            }
                            if let Some(val_node) = param.child_by_field_name("value") {
                                Self::collect_depends_calls(
                                    val_node,
                                    source,
                                    &mut meta.dependencies,
                                );
                            }
                        }
                        "default_parameter" => {
                            if let Some(val_node) = param.child_by_field_name("value") {
                                Self::collect_depends_calls(
                                    val_node,
                                    source,
                                    &mut meta.dependencies,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }

            if let Some(ret_type) = fn_def.child_by_field_name("return_type") {
                Self::collect_schema_identifiers(ret_type, source, &mut meta.response_models);
            }
        }

        if is_fastapi_route || !meta.dependencies.is_empty() || !meta.response_models.is_empty() {
            Some(meta)
        } else {
            None
        }
    }

    /// Extracts Django / DRF ViewSet or APIView metadata.
    pub fn extract_django_view_metadata(
        &self,
        node: Node<'_>,
        source: &str,
    ) -> Option<DjangoViewMetadata> {
        let class_node = if node.kind() == "class_definition" {
            Some(node)
        } else if node.kind() == "decorated_definition" {
            AstUtils::find_child_by_kind(node, "class_definition")
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "block" {
                parent.parent().filter(|p| p.kind() == "class_definition")
            } else {
                None
            }
        } else {
            None
        };

        let class_def = class_node?;
        let superclasses = class_def.child_by_field_name("superclasses");
        let class_name = class_def
            .child_by_field_name("name")
            .map(|n| AstUtils::node_text(n, source))
            .unwrap_or("");

        let mut is_django_view = false;
        if let Some(supers) = superclasses {
            let supers_text = AstUtils::node_text(supers, source);
            if supers_text.contains("ViewSet")
                || supers_text.contains("APIView")
                || supers_text.contains("GenericAPIView")
                || supers_text.contains("ListCreateAPIView")
                || supers_text.contains("RetrieveUpdateDestroyAPIView")
                || supers_text.contains("ListAPIView")
                || supers_text.contains("CreateAPIView")
                || supers_text.contains("RetrieveAPIView")
                || supers_text.contains("UpdateAPIView")
                || supers_text.contains("DestroyAPIView")
                || supers_text.contains("View")
            {
                is_django_view = true;
            }
        }

        let mut meta = DjangoViewMetadata {
            view_name: class_name.to_string(),
            ..Default::default()
        };

        if let Some(body) = class_def.child_by_field_name("body") {
            for stmt in body.named_children(&mut body.walk()) {
                let actual_stmt = if stmt.kind() == "expression_statement" {
                    stmt.named_child(0).unwrap_or(stmt)
                } else {
                    stmt
                };

                if actual_stmt.kind() == "assignment" {
                    if let (Some(left), Some(right)) = (
                        actual_stmt.child_by_field_name("left"),
                        actual_stmt.child_by_field_name("right"),
                    ) {
                        let left_text = AstUtils::node_text(left, source).trim();
                        match left_text {
                            "serializer_class" => {
                                let ser_text = AstUtils::node_text(right, source).trim();
                                let ser_name = ser_text.split('.').next_back().unwrap_or(ser_text);
                                if !ser_name.is_empty()
                                    && !meta.serializers.contains(&ser_name.to_string())
                                {
                                    meta.serializers.push(ser_name.to_string());
                                    is_django_view = true;
                                }
                            }
                            "queryset" => {
                                let qs_text = AstUtils::node_text(right, source).trim();
                                if let Some((model_part, _)) = qs_text.split_once(".objects") {
                                    let model_name = model_part
                                        .split('.')
                                        .next_back()
                                        .unwrap_or(model_part)
                                        .trim();
                                    if !model_name.is_empty()
                                        && !meta.models.contains(&model_name.to_string())
                                    {
                                        meta.models.push(model_name.to_string());
                                        is_django_view = true;
                                    }
                                }
                            }
                            "permission_classes" => {
                                for id in AstUtils::find_descendants_by_kind(right, "identifier") {
                                    let p_name = AstUtils::node_text(id, source);
                                    if !is_builtin_python_type(p_name)
                                        && !meta.permissions.contains(&p_name.to_string())
                                    {
                                        meta.permissions.push(p_name.to_string());
                                        is_django_view = true;
                                    }
                                }
                            }
                            "pagination_class" => {
                                let pag_text = AstUtils::node_text(right, source).trim();
                                meta.pagination = Some(
                                    pag_text
                                        .split('.')
                                        .next_back()
                                        .unwrap_or(pag_text)
                                        .to_string(),
                                );
                            }
                            "filter_backends" => {
                                for id in AstUtils::find_descendants_by_kind(right, "identifier") {
                                    let fb_name = AstUtils::node_text(id, source);
                                    if !meta.filter_backends.contains(&fb_name.to_string()) {
                                        meta.filter_backends.push(fb_name.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else if actual_stmt.kind() == "function_definition"
                    || actual_stmt.kind() == "decorated_definition"
                {
                    let fn_def = if actual_stmt.kind() == "decorated_definition" {
                        // Check @action decorator
                        for dec in AstUtils::find_children_by_kind(actual_stmt, "decorator") {
                            if let Some(call) = AstUtils::find_child_by_kind(dec, "call") {
                                if let Some(args) = call.child_by_field_name("arguments") {
                                    for arg in args.named_children(&mut args.walk()) {
                                        if arg.kind() == "keyword_argument" {
                                            if let (Some(n), Some(v)) = (
                                                arg.child_by_field_name("name"),
                                                arg.child_by_field_name("value"),
                                            ) {
                                                let name_t = AstUtils::node_text(n, source);
                                                if name_t == "serializer_class" {
                                                    let val_t =
                                                        AstUtils::node_text(v, source).trim();
                                                    let ser_n = val_t
                                                        .split('.')
                                                        .next_back()
                                                        .unwrap_or(val_t);
                                                    if !meta
                                                        .serializers
                                                        .contains(&ser_n.to_string())
                                                    {
                                                        meta.serializers.push(ser_n.to_string());
                                                    }
                                                } else if name_t == "permission_classes" {
                                                    for id in AstUtils::find_descendants_by_kind(
                                                        v,
                                                        "identifier",
                                                    ) {
                                                        let p_name =
                                                            AstUtils::node_text(id, source);
                                                        if !is_builtin_python_type(p_name)
                                                            && !meta
                                                                .permissions
                                                                .contains(&p_name.to_string())
                                                        {
                                                            meta.permissions
                                                                .push(p_name.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        AstUtils::find_child_by_kind(actual_stmt, "function_definition")
                    } else {
                        Some(actual_stmt)
                    };

                    if let Some(f) = fn_def {
                        let fn_name = f
                            .child_by_field_name("name")
                            .map(|n| AstUtils::node_text(n, source))
                            .unwrap_or("");
                        if fn_name == "get_serializer_class" {
                            for ret in AstUtils::find_descendants_by_kind(f, "return_statement") {
                                for id in AstUtils::find_descendants_by_kind(ret, "identifier") {
                                    let name = AstUtils::node_text(id, source);
                                    if name.ends_with("Serializer")
                                        && !meta.serializers.contains(&name.to_string())
                                    {
                                        meta.serializers.push(name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if is_django_view {
            Some(meta)
        } else {
            None
        }
    }

    fn collect_schema_identifiers(node: Node<'_>, source: &str, out: &mut Vec<String>) {
        for id in AstUtils::find_descendants_by_kind(node, "identifier") {
            let name = AstUtils::node_text(id, source);
            if !is_builtin_python_type(name)
                && !is_builtin_python_func(name)
                && !matches!(
                    name,
                    "Annotated"
                        | "Depends"
                        | "Security"
                        | "Query"
                        | "Path"
                        | "Header"
                        | "Cookie"
                        | "Body"
                        | "Form"
                        | "File"
                        | "Optional"
                        | "Union"
                        | "List"
                        | "Dict"
                        | "Set"
                        | "Tuple"
                        | "Any"
                        | "None"
                )
                && !out.contains(&name.to_string())
            {
                out.push(name.to_string());
            }
        }
    }

    fn collect_depends_calls(node: Node<'_>, source: &str, out: &mut Vec<String>) {
        for call in AstUtils::find_descendants_by_kind(node, "call") {
            if let Some(func) = call.child_by_field_name("function") {
                let func_name = AstUtils::node_text(func, source);
                let base_name = func_name.split('.').next_back().unwrap_or(func_name);
                if base_name == "Depends" || base_name == "Security" {
                    if let Some(args) = call.child_by_field_name("arguments") {
                        if let Some(first_arg) = args.named_children(&mut args.walk()).next() {
                            let dep_name = AstUtils::node_text(first_arg, source);
                            let clean_name = dep_name.split('.').next_back().unwrap_or(dep_name);
                            if !clean_name.is_empty() && !out.contains(&clean_name.to_string()) {
                                out.push(clean_name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    fn extract_annotated_dependencies(
        node: Node<'_>,
        source: &str,
        deps: &mut Vec<String>,
        _schemas: &mut Vec<String>,
    ) {
        Self::collect_depends_calls(node, source, deps);
    }

    fn resolve_and_augment_fastapi(
        &self,
        meta: &FastApiRouteMetadata,
        _target_node: Node<'_>,
        source: &str,
        file_path: &Path,
        ts_lang: &Language,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let tree = ParserManager::parse_source(source, ts_lang, file_path)?;
        let root = tree.root_node();

        let mut schema_queue: VecDeque<String> = VecDeque::new();
        let mut visited_types: HashSet<String> = HashSet::new();

        for s in &meta.response_models {
            if !visited_types.contains(s) {
                visited_types.insert(s.clone());
                schema_queue.push_back(s.clone());
            }
        }
        for s in &meta.parameter_schemas {
            if !visited_types.contains(s) {
                visited_types.insert(s.clone());
                schema_queue.push_back(s.clone());
            }
        }

        // Hoist schemas recursively
        while let Some(schema_name) = schema_queue.pop_front() {
            if is_builtin_python_type(&schema_name) {
                continue;
            }

            if let Some(entity) =
                self.find_python_class_or_func(root, source, file_path, &schema_name, ts_lang)
            {
                if entity.kind == "class" {
                    let ext_type = ExtractedType {
                        name: schema_name.clone(),
                        kind: "class".to_string(),
                        file_path: entity.file_path.to_string_lossy().to_string(),
                        definition: entity.minified_def,
                    };
                    if !slice.hoisted_types.iter().any(|t| t.name == schema_name) {
                        slice.hoisted_types.push(ext_type);
                    }

                    for n in entity.nested_schemas {
                        if !visited_types.contains(&n) && !is_builtin_python_type(&n) {
                            visited_types.insert(n.clone());
                            schema_queue.push_back(n);
                        }
                    }
                }
            }
        }

        // Extract dependencies to stripped calls
        for dep in &meta.dependencies {
            if slice.stripped_calls.iter().any(|c| c.name == *dep) {
                continue;
            }

            if let Some(entity) =
                self.find_python_class_or_func(root, source, file_path, dep, ts_lang)
            {
                let sig = if entity.kind == "function" {
                    entity.function_sig
                } else {
                    format!("class {dep}(...): ...")
                };

                let stub = CallSignatureStub {
                    name: dep.clone(),
                    receiver: None,
                    file_path: Some(entity.file_path.to_string_lossy().to_string()),
                    signature: sig,
                };
                slice.stripped_calls.push(stub);
            }
        }

        Ok(())
    }

    fn resolve_and_augment_django(
        &self,
        meta: &DjangoViewMetadata,
        _target_node: Node<'_>,
        source: &str,
        file_path: &Path,
        ts_lang: &Language,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let tree = ParserManager::parse_source(source, ts_lang, file_path)?;
        let root = tree.root_node();

        let mut type_queue: VecDeque<(String, String)> = VecDeque::new(); // (name, role: "serializer" | "model" | "permission")
        let mut visited_types: HashSet<String> = HashSet::new();

        for ser in &meta.serializers {
            if !visited_types.contains(ser) {
                visited_types.insert(ser.clone());
                type_queue.push_back((ser.clone(), "serializer".to_string()));
            }
        }
        for m in &meta.models {
            if !visited_types.contains(m) {
                visited_types.insert(m.clone());
                type_queue.push_back((m.clone(), "model".to_string()));
            }
        }
        for p in &meta.permissions {
            if !visited_types.contains(p) {
                visited_types.insert(p.clone());
                type_queue.push_back((p.clone(), "permission".to_string()));
            }
        }

        while let Some((symbol_name, role)) = type_queue.pop_front() {
            if is_builtin_python_type(&symbol_name) {
                continue;
            }

            if let Some(entity) =
                self.find_python_class_or_func(root, source, file_path, &symbol_name, ts_lang)
            {
                if entity.kind == "class" {
                    let ext_type = ExtractedType {
                        name: symbol_name.clone(),
                        kind: "class".to_string(),
                        file_path: entity.file_path.to_string_lossy().to_string(),
                        definition: entity.minified_def,
                    };

                    if !slice.hoisted_types.iter().any(|t| t.name == symbol_name) {
                        slice.hoisted_types.push(ext_type);
                    }

                    // Discover linked entities
                    if role == "serializer" {
                        if let Some(inner_model) = entity.inner_model {
                            if !visited_types.contains(&inner_model) {
                                visited_types.insert(inner_model.clone());
                                type_queue.push_back((inner_model, "model".to_string()));
                            }
                        }
                        for ns in entity.nested_serializers {
                            if !visited_types.contains(&ns) {
                                visited_types.insert(ns.clone());
                                type_queue.push_back((ns, "serializer".to_string()));
                            }
                        }
                    } else if role == "model" {
                        for rel in entity.related_models {
                            if !visited_types.contains(&rel) {
                                visited_types.insert(rel.clone());
                                type_queue.push_back((rel, "model".to_string()));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Strips complex implementation bodies from DRF Serializer, Django Model, or Pydantic Schema.
    pub fn minify_python_class(&self, class_node: Node<'_>, source: &str) -> String {
        let class_name = class_node
            .child_by_field_name("name")
            .map(|n| AstUtils::node_text(n, source))
            .unwrap_or("Unknown");

        let superclasses = class_node
            .child_by_field_name("superclasses")
            .map(|s| AstUtils::node_text(s, source))
            .unwrap_or("");

        let header = if superclasses.is_empty() {
            format!("class {class_name}:")
        } else {
            format!("class {class_name}{superclasses}:")
        };

        let mut lines = Vec::new();
        lines.push(header);

        // Check docstring
        if let Some(body) = class_node.child_by_field_name("body") {
            let mut body_cursor = body.walk();
            let mut children = body.named_children(&mut body_cursor);

            if let Some(first_child) = children.next() {
                if first_child.kind() == "expression_statement" {
                    let first_expr = first_child.named_child(0).unwrap_or(first_child);
                    if first_expr.kind() == "string" {
                        let doc_str = AstUtils::node_text(first_expr, source).trim();
                        for d_line in doc_str.lines() {
                            lines.push(format!("    {d_line}"));
                        }
                    }
                }
            }

            for member in body.named_children(&mut body.walk()) {
                let actual = if member.kind() == "expression_statement" {
                    member.named_child(0).unwrap_or(member)
                } else {
                    member
                };

                match actual.kind() {
                    "assignment" | "type" => {
                        let text = AstUtils::node_text(member, source).trim();
                        lines.push(format!("    {text}"));
                    }
                    "class_definition" => {
                        let inner_name = actual
                            .child_by_field_name("name")
                            .map(|n| AstUtils::node_text(n, source))
                            .unwrap_or("");
                        if inner_name == "Meta" || inner_name == "Config" {
                            let inner_text = AstUtils::node_text(actual, source).trim();
                            for inner_line in inner_text.lines() {
                                lines.push(format!("    {inner_line}"));
                            }
                        }
                    }
                    "function_definition" | "decorated_definition" => {
                        let fn_node = if actual.kind() == "decorated_definition" {
                            let decs = AstUtils::find_children_by_kind(actual, "decorator");
                            for d in decs {
                                lines
                                    .push(format!("    {}", AstUtils::node_text(d, source).trim()));
                            }
                            AstUtils::find_child_by_kind(actual, "function_definition")
                        } else {
                            Some(actual)
                        };

                        if let Some(fn_def) = fn_node {
                            let sig = extract_python_function_signature(fn_def, source);
                            lines.push(format!("    {sig}: ..."));
                        }
                    }
                    _ => {}
                }
            }
        }

        if lines.len() == 1 {
            lines.push("    ...".to_string());
        }

        lines.join("\n")
    }

    fn extract_serializer_meta_model(node: Node<'_>, source: &str) -> Option<String> {
        if let Some(body) = node.child_by_field_name("body") {
            for child in body.named_children(&mut body.walk()) {
                if child.kind() == "class_definition" {
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| AstUtils::node_text(n, source));
                    if name == Some("Meta") {
                        if let Some(meta_body) = child.child_by_field_name("body") {
                            for stmt in meta_body.named_children(&mut meta_body.walk()) {
                                let actual = if stmt.kind() == "expression_statement" {
                                    stmt.named_child(0).unwrap_or(stmt)
                                } else {
                                    stmt
                                };
                                if actual.kind() == "assignment" {
                                    if let (Some(left), Some(right)) = (
                                        actual.child_by_field_name("left"),
                                        actual.child_by_field_name("right"),
                                    ) {
                                        let left_t = AstUtils::node_text(left, source).trim();
                                        if left_t == "model" {
                                            let right_t = AstUtils::node_text(right, source).trim();
                                            let model_n =
                                                right_t.split('.').next_back().unwrap_or(right_t);
                                            return Some(model_n.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_nested_serializers(node: Node<'_>, source: &str) -> Vec<String> {
        let mut serializers = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            for child in body.named_children(&mut body.walk()) {
                let actual = if child.kind() == "expression_statement" {
                    child.named_child(0).unwrap_or(child)
                } else {
                    child
                };
                if actual.kind() == "assignment" {
                    if let Some(right) = actual.child_by_field_name("right") {
                        for call in AstUtils::find_descendants_by_kind(right, "call") {
                            if let Some(func) = call.child_by_field_name("function") {
                                let func_t = AstUtils::node_text(func, source);
                                if func_t.ends_with("Serializer") {
                                    let clean = func_t.split('.').next_back().unwrap_or(func_t);
                                    if !serializers.contains(&clean.to_string()) {
                                        serializers.push(clean.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        serializers
    }

    fn extract_model_related_entities(node: Node<'_>, source: &str) -> Vec<String> {
        let mut related = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            for child in body.named_children(&mut body.walk()) {
                let actual = if child.kind() == "expression_statement" {
                    child.named_child(0).unwrap_or(child)
                } else {
                    child
                };
                if actual.kind() == "assignment" {
                    if let Some(right) = actual.child_by_field_name("right") {
                        if right.kind() == "call" {
                            if let Some(func) = right.child_by_field_name("function") {
                                let func_t = AstUtils::node_text(func, source);
                                if func_t.contains("ForeignKey")
                                    || func_t.contains("ManyToManyField")
                                    || func_t.contains("OneToOneField")
                                {
                                    if let Some(args) = right.child_by_field_name("arguments") {
                                        if let Some(first_arg) = args.named_child(0) {
                                            let arg_t = AstUtils::node_text(first_arg, source)
                                                .trim_matches('\'')
                                                .trim_matches('"');
                                            let model_n = arg_t
                                                .split('.')
                                                .next_back()
                                                .unwrap_or(arg_t)
                                                .trim();
                                            if !model_n.is_empty()
                                                && !is_builtin_python_type(model_n)
                                                && !related.contains(&model_n.to_string())
                                            {
                                                related.push(model_n.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        related
    }

    fn entity_from_node(
        &self,
        actual: Node<'_>,
        src: &str,
        p: &Path,
        _target_name: &str,
    ) -> FoundPythonEntity {
        if actual.kind() == "class_definition" {
            let minified = self.minify_python_class(actual, src);
            let inner_m = Self::extract_serializer_meta_model(actual, src);
            let nested_s = Self::extract_nested_serializers(actual, src);
            let related_m = Self::extract_model_related_entities(actual, src);
            let mut nested_sch = Vec::new();
            Self::collect_schema_identifiers(actual, src, &mut nested_sch);

            FoundPythonEntity {
                kind: "class".to_string(),
                file_path: p.to_path_buf(),
                minified_def: minified,
                function_sig: String::new(),
                inner_model: inner_m,
                nested_serializers: nested_s,
                related_models: related_m,
                nested_schemas: nested_sch,
            }
        } else {
            let sig = extract_python_function_signature(actual, src);
            FoundPythonEntity {
                kind: "function".to_string(),
                file_path: p.to_path_buf(),
                minified_def: String::new(),
                function_sig: sig,
                inner_model: None,
                nested_serializers: Vec::new(),
                related_models: Vec::new(),
                nested_schemas: Vec::new(),
            }
        }
    }

    fn find_python_class_or_func(
        &self,
        root: Node<'_>,
        source: &str,
        file_path: &Path,
        target_name: &str,
        ts_lang: &Language,
    ) -> Option<FoundPythonEntity> {
        // 1. Search local AST
        for child in root.named_children(&mut root.walk()) {
            let actual = if child.kind() == "decorated_definition" {
                AstUtils::find_child_by_kind(child, "class_definition")
                    .or_else(|| AstUtils::find_child_by_kind(child, "function_definition"))
                    .unwrap_or(child)
            } else {
                child
            };

            if actual.kind() == "class_definition" || actual.kind() == "function_definition" {
                if let Some(name_n) = actual.child_by_field_name("name") {
                    if AstUtils::node_text(name_n, source) == target_name {
                        return Some(self.entity_from_node(actual, source, file_path, target_name));
                    }
                }
            }
        }

        // 2. Scan import statements and resolve in referenced files
        let parent_dir = file_path.parent().unwrap_or_else(|| Path::new("."));
        let import_nodes = AstUtils::find_descendants_by_kind(root, "import_from_statement");
        for imp in import_nodes {
            let imp_text = AstUtils::node_text(imp, source);
            if imp_text.contains(target_name) {
                // Extract module name from `from <module> import ...`
                if let Some(mod_node) = imp.child_by_field_name("module_name") {
                    let mod_text = AstUtils::node_text(mod_node, source);
                    let relative_parts: Vec<&str> = mod_text.split('.').collect();
                    let mut candidate = parent_dir.to_path_buf();
                    for part in relative_parts {
                        candidate.push(part);
                    }
                    let candidates = [
                        candidate.with_extension("py"),
                        candidate.join("__init__.py"),
                    ];
                    for cand_path in candidates {
                        if cand_path.is_file() {
                            if let Ok(cand_source) = fs::read_to_string(&cand_path) {
                                if let Ok(cand_tree) =
                                    ParserManager::parse_source(&cand_source, ts_lang, &cand_path)
                                {
                                    for c_child in cand_tree
                                        .root_node()
                                        .named_children(&mut cand_tree.root_node().walk())
                                    {
                                        let c_actual = if c_child.kind() == "decorated_definition" {
                                            AstUtils::find_child_by_kind(
                                                c_child,
                                                "class_definition",
                                            )
                                            .or_else(|| {
                                                AstUtils::find_child_by_kind(
                                                    c_child,
                                                    "function_definition",
                                                )
                                            })
                                            .unwrap_or(c_child)
                                        } else {
                                            c_child
                                        };
                                        if c_actual.kind() == "class_definition"
                                            || c_actual.kind() == "function_definition"
                                        {
                                            if let Some(name_n) =
                                                c_actual.child_by_field_name("name")
                                            {
                                                if AstUtils::node_text(name_n, &cand_source)
                                                    == target_name
                                                {
                                                    return Some(self.entity_from_node(
                                                        c_actual,
                                                        &cand_source,
                                                        &cand_path,
                                                        target_name,
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

fn extract_python_function_signature(node: Node<'_>, source: &str) -> String {
    let name = node
        .child_by_field_name("name")
        .map(|n| AstUtils::node_text(n, source))
        .unwrap_or("func");
    let is_async = node.children(&mut node.walk()).any(|c| c.kind() == "async");
    let prefix = if is_async { "async def" } else { "def" };

    let params = node
        .child_by_field_name("parameters")
        .map(|p| AstUtils::node_text(p, source))
        .unwrap_or("()");

    let ret = node
        .child_by_field_name("return_type")
        .map(|r| format!(" -> {}", AstUtils::node_text(r, source)))
        .unwrap_or_default();

    format!("{prefix} {name}{params}{ret}")
}

fn is_builtin_python_type(name: &str) -> bool {
    matches!(
        name,
        "int"
            | "float"
            | "str"
            | "bool"
            | "bytes"
            | "list"
            | "dict"
            | "set"
            | "tuple"
            | "object"
            | "type"
            | "None"
            | "Any"
            | "Union"
            | "Optional"
            | "List"
            | "Dict"
            | "Set"
            | "Tuple"
            | "Sequence"
            | "Iterable"
            | "Mapping"
            | "Callable"
            | "TypeVar"
            | "Generic"
            | "Annotated"
            | "BaseModel"
            | "Model"
            | "Serializer"
            | "ModelSerializer"
            | "APIView"
            | "ViewSet"
            | "ModelViewSet"
            | "BasePermission"
    )
}

fn is_builtin_python_func(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "len"
            | "range"
            | "enumerate"
            | "zip"
            | "map"
            | "filter"
            | "sum"
            | "min"
            | "max"
            | "abs"
            | "round"
            | "isinstance"
            | "issubclass"
            | "getattr"
            | "setattr"
            | "hasattr"
            | "delattr"
            | "super"
    )
}
