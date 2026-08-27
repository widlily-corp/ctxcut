#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_self,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

use crate::error::Result;
use crate::framework::FrameworkAnalyzer;
use crate::fullstack::model::ServerRouteEndpoint;
use crate::model::{CallSignatureStub, ExtractedType, SliceOptions, SliceResult};
use crate::parser::AstUtils;
use crate::resolver::TypeHoister;
use std::path::Path;
use tree_sitter::Node;

/// Express.js route and middleware semantic analyzer.
#[derive(Debug, Default, Clone)]
pub struct ExpressAnalyzer;

impl ExpressAnalyzer {
    /// Creates a new `ExpressAnalyzer` instance.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all server route endpoints from an Express.js source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        for line in source.lines() {
            let t = line.trim();
            for method in &["get", "post", "put", "delete", "patch"] {
                let pat = format!(".{method}(");
                if t.contains(&pat) && (t.contains("'/") || t.contains("\"/") || t.contains("`/")) {
                    if let Some(pos) = t.find(&pat) {
                        let after = &t[pos + pat.len()..];
                        if let Some(path_str) = extract_path_from_args(after) {
                            let handler_name = extract_express_handler_name(after);
                            let sig = format!("app.{method}(\"{path_str}\", {handler_name})");
                            routes.push(ServerRouteEndpoint {
                                framework: "express".to_string(),
                                http_method: method.to_uppercase(),
                                route_path: path_str,
                                handler_file: file_path.clone(),
                                handler_symbol: handler_name,
                                handler_signature: sig,
                                request_dto_type: None,
                                response_dto_type: None,
                            });
                        }
                    }
                }
            }
        }

        routes
    }
}

impl FrameworkAnalyzer for ExpressAnalyzer {
    fn name(&self) -> &'static str {
        "express"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(ext.as_str(), "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs") {
            return false;
        }

        source.contains("express")
            || source.contains("Router")
            || (source.contains("Request") && source.contains("Response"))
            || source.contains(".get(")
            || source.contains(".post(")
            || source.contains(".put(")
            || source.contains(".delete(")
            || source.contains(".use(")
    }

    fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let tree_sitter_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let mut root = target_node;
        while let Some(parent) = root.parent() {
            root = parent;
        }

        let target_name = slice.target_symbol.name.as_str();

        // 1. Scan for Express route call expressions in the file
        let calls = AstUtils::find_descendants_by_kind(root, "call_expression");
        let mut middleware_names = Vec::new();
        let mut dto_type_names = Vec::new();

        for call in calls {
            if let Some(func) = call.child_by_field_name("function") {
                let func_t = AstUtils::node_text(func, source);
                if is_express_route_method(func_t) {
                    if let Some(args) = call.child_by_field_name("arguments") {
                        let arg_nodes: Vec<Node<'_>> =
                            args.named_children(&mut args.walk()).collect();

                        // Check if this route call contains our target symbol or inline handler
                        let mut contains_target = false;
                        let mut target_index = 0;

                        let mut handler_idx = None;
                        for (idx, arg) in arg_nodes.iter().enumerate() {
                            let arg_t = AstUtils::node_text(*arg, source);
                            if !target_name.is_empty()
                                && (arg_t == target_name
                                    || arg_t.contains(&format!("function {target_name}"))
                                    || arg_t.contains(target_name))
                            {
                                handler_idx = Some(idx);
                                break;
                            }
                            if arg.start_byte() <= target_node.start_byte()
                                && target_node.end_byte() <= arg.end_byte()
                            {
                                handler_idx = Some(idx);
                                break;
                            }
                        }

                        if handler_idx.is_none()
                            && (call == target_node || call.parent() == Some(target_node))
                        {
                            handler_idx = arg_nodes.iter().rposition(|arg| {
                                arg.kind() == "function_expression"
                                    || arg.kind() == "arrow_function"
                                    || arg.kind() == "function_declaration"
                            });
                        }

                        if let Some(idx) = handler_idx {
                            contains_target = true;
                            target_index = idx;
                        }

                        if contains_target {
                            extract_express_param_dtos(call, source, &mut dto_type_names);
                            // Collect middleware args before target handler (skip arg 0 if string route path)
                            for arg in arg_nodes.iter().take(target_index) {
                                if arg.kind() == "string" || arg.kind() == "template_string" {
                                    continue;
                                }

                                if arg.kind() == "identifier" {
                                    let mid_name = AstUtils::node_text(*arg, source);
                                    if !middleware_names.contains(&mid_name.to_string()) {
                                        middleware_names.push(mid_name.to_string());
                                    }
                                } else if arg.kind() == "call_expression" {
                                    if let Some(c_func) = arg.child_by_field_name("function") {
                                        let factory_name = AstUtils::node_text(c_func, source);
                                        let full_call_text = AstUtils::node_text(*arg, source);
                                        if !middleware_names.contains(&full_call_text.to_string()) {
                                            middleware_names.push(full_call_text.to_string());
                                        }

                                        // Check if arguments to validation middleware are DTO identifiers (e.g. validate(CheckoutDTO))
                                        if let Some(c_args) = arg.child_by_field_name("arguments") {
                                            for c_arg in c_args.named_children(&mut c_args.walk()) {
                                                if c_arg.kind() == "identifier" {
                                                    let cand = AstUtils::node_text(c_arg, source);
                                                    if (cand.ends_with("DTO")
                                                        || cand.ends_with("Dto")
                                                        || cand.ends_with("Schema")
                                                        || cand.ends_with("Request"))
                                                        && !dto_type_names
                                                            .contains(&cand.to_string())
                                                    {
                                                        dto_type_names.push(cand.to_string());
                                                    }
                                                }
                                            }
                                        }
                                        let _ = factory_name;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Parse generic Request/Response parameters on target_node
        extract_express_param_dtos(target_node, source, &mut dto_type_names);

        // 3. Hoist DTO types
        let opts = SliceOptions {
            depth: 2,
            include_types: true,
            include_calls: true,
            budget: None,
        };

        for dto_name in &dto_type_names {
            if let Ok(mut hoisted) =
                TypeHoister::hoist_types(target_node, root, source, path, &opts, &tree_sitter_lang)
            {
                for ty in hoisted.drain(..) {
                    if !slice.hoisted_types.iter().any(|t| t.name == ty.name) {
                        slice.hoisted_types.push(ty);
                    }
                }
            }

            // Fallback direct AST lookup for DTO if not already in slice.hoisted_types
            if !slice.hoisted_types.iter().any(|t| t.name == *dto_name) {
                if let Some(ty) = find_local_type_or_interface(root, source, dto_name, path) {
                    slice.hoisted_types.push(ty);
                }
            }
        }

        // 4. Add middleware stubs
        for mid in middleware_names {
            if !slice.stripped_calls.iter().any(|c| c.name == mid) {
                slice.stripped_calls.push(CallSignatureStub {
                    name: mid.clone(),
                    receiver: None,
                    file_path: Some(path.to_string_lossy().to_string()),
                    signature: format!("// Middleware: {mid}"),
                });
            }
        }

        Ok(())
    }
}

/// NestJS controller, guard, and DTO analyzer.
#[derive(Debug, Default, Clone, Copy)]
pub struct NestJsAnalyzer;

impl NestJsAnalyzer {
    /// Creates a new `NestJsAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all server route endpoints from a NestJS source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        let lines: Vec<&str> = source.lines().collect();
        let mut controller_prefix = String::new();

        // 1. Controller prefix: @Controller('users') or @Controller('/api/users')
        for line in &lines {
            let t = line.trim();
            if t.starts_with("@Controller(") {
                if let Some(p) = extract_path_or_string(t) {
                    let clean = p.trim_matches('/');
                    controller_prefix = if clean.is_empty() { String::new() } else { format!("/{clean}") };
                }
            }
        }

        // 2. Methods: @Get(':id'), @Post(), @Put(':id'), @Delete(':id'), @Patch(':id')
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            for method in &["Get", "Post", "Put", "Delete", "Patch"] {
                let pat = format!("@{method}(");
                let pat_empty = format!("@{method}()");
                if t.starts_with(&pat) || t.starts_with(&pat_empty) {
                    let sub = extract_path_or_string(t).unwrap_or_default();
                    let clean_sub = sub.trim_matches('/');
                    let full_path = if clean_sub.is_empty() {
                        if controller_prefix.is_empty() { "/".to_string() } else { controller_prefix.clone() }
                    } else if controller_prefix.is_empty() {
                        format!("/{clean_sub}")
                    } else {
                        format!("{controller_prefix}/{clean_sub}")
                    };

                    // Handler name from next line
                    let mut handler_name = "handler".to_string();
                    let mut handler_sig = String::new();
                    for next_line in lines.iter().skip(i + 1) {
                        let nt = next_line.trim();
                        if !nt.starts_with('@') && !nt.is_empty() {
                            handler_sig = nt.to_string();
                            let clean = nt.trim_start_matches("async ").trim_start_matches("public ");
                            if let Some(paren) = clean.find('(') {
                                handler_name = clean[..paren].trim().to_string();
                            }
                            break;
                        }
                    }

                    routes.push(ServerRouteEndpoint {
                        framework: "nestjs".to_string(),
                        http_method: method.to_uppercase(),
                        route_path: full_path,
                        handler_file: file_path.clone(),
                        handler_symbol: handler_name,
                        handler_signature: handler_sig,
                        request_dto_type: None,
                        response_dto_type: None,
                    });
                }
            }
        }

        routes
    }
}

impl FrameworkAnalyzer for NestJsAnalyzer {
    fn name(&self) -> &'static str {
        "nestjs"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "ts" && ext != "js" && !path.as_os_str().is_empty() {
            return false;
        }

        source.contains("@nestjs")
            || source.contains("@Controller")
            || source.contains("@Injectable")
            || source.contains("@UseGuards")
            || source.contains("@UseInterceptors")
            || source.contains("@UsePipes")
            || source.contains("@UseFilters")
            || source.contains("@Body(")
            || source.contains("@Body()")
            || source.contains("@Param(")
            || source.contains("@Query(")
    }

    fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let tree_sitter_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let mut root = target_node;
        while let Some(parent) = root.parent() {
            root = parent;
        }

        let mut guard_names: Vec<String> = Vec::new();
        let mut dto_names: Vec<String> = Vec::new();

        // Helper to extract guard, interceptor, pipe, and filter names from a decorator node
        let collect_from_decorator = |dec: Node<'_>, guards: &mut Vec<String>| {
            let dec_text = AstUtils::node_text(dec, source);
            if dec_text.contains("UseGuards")
                || dec_text.contains("UseInterceptors")
                || dec_text.contains("UsePipes")
                || dec_text.contains("UseFilters")
            {
                for id in AstUtils::find_descendants_by_kind(dec, "identifier") {
                    let name = AstUtils::node_text(id, source);
                    if !matches!(
                        name,
                        "UseGuards" | "UseInterceptors" | "UsePipes" | "UseFilters"
                    ) && !guards.contains(&name.to_string())
                    {
                        guards.push(name.to_string());
                    }
                }
            }
        };

        // 1. Inspect class-level decorators if target is inside a class
        let class_node = find_enclosing_class(target_node);
        if let Some(cls) = class_node {
            for dec in AstUtils::find_descendants_by_kind(cls, "decorator") {
                // Only collect class-level decorators (direct children of class or before class body)
                if dec.end_byte()
                    <= cls
                        .child_by_field_name("body")
                        .map_or(cls.end_byte(), |b| b.start_byte())
                {
                    collect_from_decorator(dec, &mut guard_names);
                }
            }
            if let Some(parent) = cls.parent() {
                for dec in AstUtils::find_children_by_kind(parent, "decorator") {
                    collect_from_decorator(dec, &mut guard_names);
                }
            }
            let mut prev = cls.prev_named_sibling();
            while let Some(sibling) = prev {
                if sibling.kind() == "decorator" {
                    collect_from_decorator(sibling, &mut guard_names);
                    prev = sibling.prev_named_sibling();
                } else {
                    break;
                }
            }
        }

        // 2. Inspect method-level decorators (on target_node, descendants, parent, and preceding siblings)
        for dec in AstUtils::find_descendants_by_kind(target_node, "decorator") {
            collect_from_decorator(dec, &mut guard_names);
        }
        if let Some(parent) = target_node.parent() {
            if parent.kind() == "decorated_definition" {
                for dec in AstUtils::find_children_by_kind(parent, "decorator") {
                    collect_from_decorator(dec, &mut guard_names);
                }
            }
        }
        let mut prev = target_node.prev_named_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "decorator" {
                collect_from_decorator(sibling, &mut guard_names);
                prev = sibling.prev_named_sibling();
            } else {
                break;
            }
        }

        // 3. Inspect method parameters for parameter decorators (@Body, @Query, @Param)
        if let Some(params) = target_node.child_by_field_name("parameters") {
            for param in params.named_children(&mut params.walk()) {
                if let Some(type_ann) = param.child_by_field_name("type") {
                    let type_t = AstUtils::node_text(type_ann, source)
                        .trim_start_matches(':')
                        .trim();
                    let base_type = type_t.split('<').next().unwrap_or(type_t).trim();
                    if !is_builtin_ts_type(base_type) && !dto_names.contains(&base_type.to_string())
                    {
                        dto_names.push(base_type.to_string());
                    }
                }
            }
        }

        // 4. Inspect return type (e.g. Promise<UserResponseDto>)
        if let Some(ret_ann) = target_node.child_by_field_name("return_type") {
            let ret_t = AstUtils::node_text(ret_ann, source)
                .trim_start_matches(':')
                .trim();
            if let Some(inner) = unwrap_generic_type(ret_t) {
                if !is_builtin_ts_type(&inner) && !dto_names.contains(&inner) {
                    dto_names.push(inner);
                }
            } else if !is_builtin_ts_type(ret_t) && !dto_names.contains(&ret_t.to_string()) {
                dto_names.push(ret_t.to_string());
            }
        }

        // 5. Hoist DTOs
        let opts = SliceOptions {
            depth: 2,
            include_types: true,
            include_calls: true,
            budget: None,
        };

        for dto in &dto_names {
            if let Ok(mut hoisted) =
                TypeHoister::hoist_types(target_node, root, source, path, &opts, &tree_sitter_lang)
            {
                for ty in hoisted.drain(..) {
                    if !slice.hoisted_types.iter().any(|t| t.name == ty.name) {
                        slice.hoisted_types.push(ty);
                    }
                }
            }

            if !slice.hoisted_types.iter().any(|t| t.name == *dto) {
                if let Some(ty) = find_local_type_or_interface(root, source, dto, path) {
                    slice.hoisted_types.push(ty);
                }
            }
        }

        // 6. Add guard & interceptor stubs to stripped calls
        for guard in guard_names {
            if !slice.stripped_calls.iter().any(|c| c.name == guard) {
                slice.stripped_calls.push(CallSignatureStub {
                    name: guard.clone(),
                    receiver: None,
                    file_path: Some(path.to_string_lossy().to_string()),
                    signature: format!("// Guard / Interceptor: {guard}"),
                });
            }
        }

        Ok(())
    }
}

/// Spring Boot controller, routing, and DTO analyzer.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpringAnalyzer;

impl SpringAnalyzer {
    /// Creates a new `SpringAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Extracts all server route endpoints from a Spring Boot source file.
    pub fn extract_routes(&self, path: &Path, source: &str) -> Vec<ServerRouteEndpoint> {
        let mut routes = Vec::new();
        let file_path = path.to_string_lossy().to_string();

        let lines: Vec<&str> = source.lines().collect();
        let mut class_prefix = String::new();

        // 1. Class prefix: @RequestMapping("/api/users")
        for line in &lines {
            let t = line.trim();
            if t.starts_with("@RequestMapping(") || t.starts_with("@RestController(") {
                if let Some(p) = extract_path_or_string(t) {
                    let clean = p.trim_matches('/');
                    class_prefix = if clean.is_empty() { String::new() } else { format!("/{clean}") };
                }
            }
        }

        // 2. Methods: @GetMapping, @PostMapping, @PutMapping, @DeleteMapping, @PatchMapping, @RequestMapping
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            for method in &["GetMapping", "PostMapping", "PutMapping", "DeleteMapping", "PatchMapping"] {
                let http_m = method.trim_end_matches("Mapping").to_uppercase();
                let pat = format!("@{method}");
                if t.starts_with(&pat) {
                    let sub = extract_path_or_string(t).unwrap_or_default();
                    let clean_sub = sub.trim_matches('/');
                    let full_path = if clean_sub.is_empty() {
                        if class_prefix.is_empty() { "/".to_string() } else { class_prefix.clone() }
                    } else if class_prefix.is_empty() {
                        format!("/{clean_sub}")
                    } else {
                        format!("{class_prefix}/{clean_sub}")
                    };

                    let mut handler_name = "handler".to_string();
                    let mut handler_sig = String::new();
                    for next_line in lines.iter().skip(i + 1) {
                        let nt = next_line.trim();
                        if !nt.starts_with('@') && (nt.contains("public ") || nt.contains("private ") || nt.contains("protected ")) {
                            handler_sig = nt.to_string();
                            let clean = nt.split('(').next().unwrap_or(nt);
                            handler_name = clean.split_whitespace().last().unwrap_or("handler").to_string();
                            break;
                        }
                    }

                    routes.push(ServerRouteEndpoint {
                        framework: "spring_boot".to_string(),
                        http_method: http_m,
                        route_path: full_path,
                        handler_file: file_path.clone(),
                        handler_symbol: handler_name,
                        handler_signature: handler_sig,
                        request_dto_type: None,
                        response_dto_type: None,
                    });
                }
            }
        }

        routes
    }
}

impl FrameworkAnalyzer for SpringAnalyzer {
    fn name(&self) -> &'static str {
        "spring"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(ext.as_str(), "java" | "kt" | "scala" | "groovy")
            && !path.as_os_str().is_empty()
        {
            return false;
        }

        source.contains("org.springframework")
            || source.contains("@RestController")
            || source.contains("@Controller")
            || source.contains("@Service")
            || source.contains("@Repository")
            || source.contains("@RequestMapping")
            || source.contains("@GetMapping")
            || source.contains("@PostMapping")
            || source.contains("@PutMapping")
            || source.contains("@DeleteMapping")
            || source.contains("@RequestBody")
            || source.contains("@PreAuthorize")
            || source.contains("@Secured")
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
        let mut security_stubs = Vec::new();

        // 1. Scan for security annotations on method or class
        let node_text = if target_node.end_byte() > target_node.start_byte()
            && target_node.end_byte() <= source.len()
        {
            AstUtils::node_text(target_node, source)
        } else {
            source
        };
        let search_text = format!("{}\n{}", node_text, slice.target_symbol.body);
        for line in search_text.lines() {
            let trimmed = line.trim();
            if (trimmed.starts_with("@PreAuthorize") || trimmed.starts_with("@Secured"))
                && !security_stubs.contains(&trimmed.to_string())
            {
                security_stubs.push(trimmed.to_string());
            }
        }

        // 2. Scan for @RequestBody DTO parameter
        for line in search_text.lines() {
            let trimmed = line.trim();
            if trimmed.contains("@RequestBody") {
                // e.g. @RequestBody CreateOrderRequest request
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                for (i, p) in parts.iter().enumerate() {
                    if *p == "@RequestBody" || *p == "@Valid" {
                        if let Some(next) = parts.get(i + 1) {
                            let clean = next.trim_start_matches('@').trim();
                            if clean != "Valid"
                                && !is_builtin_java_type(clean)
                                && !dto_names.contains(&clean.to_string())
                            {
                                dto_names.push(clean.to_string());
                            }
                        }
                    }
                }
            }
        }

        // 3. Scan for return type DTO (e.g. ResponseEntity<OrderResponse>)
        if let Some(line) = search_text
            .lines()
            .find(|l| l.contains("public ") || l.contains("private ") || l.contains("protected "))
        {
            if let Some(start) = line.find('<') {
                if let Some(end) = line.rfind('>') {
                    let inner = line[start + 1..end].trim();
                    let inner_clean = inner.split(',').next().unwrap_or(inner).trim();
                    if !is_builtin_java_type(inner_clean)
                        && !dto_names.contains(&inner_clean.to_string())
                    {
                        dto_names.push(inner_clean.to_string());
                    }
                }
            }
        }

        // 4. Add security guard stubs
        for sec in security_stubs {
            if !slice.stripped_calls.iter().any(|c| c.name == sec) {
                slice.stripped_calls.push(CallSignatureStub {
                    name: sec.clone(),
                    receiver: None,
                    file_path: Some(path.to_string_lossy().to_string()),
                    signature: format!("// Security: {sec}"),
                });
            }
        }

        // 5. Hoist local DTO definitions if present in source
        for dto in dto_names {
            if !slice.hoisted_types.iter().any(|t| t.name == dto) {
                if let Some(ty) = find_local_type_or_interface(root, source, &dto, path) {
                    slice.hoisted_types.push(ty);
                }
            }
        }

        Ok(())
    }
}

/// Composite Express, NestJS, and Spring Boot analyzer.
#[derive(Debug, Default, Clone)]
pub struct ExpressNestSpringAnalyzer {
    express: ExpressAnalyzer,
    nestjs: NestJsAnalyzer,
    spring: SpringAnalyzer,
}

impl ExpressNestSpringAnalyzer {
    /// Creates a new `ExpressNestSpringAnalyzer` aggregating all three sub-analyzers.
    pub fn new() -> Self {
        Self {
            express: ExpressAnalyzer::new(),
            nestjs: NestJsAnalyzer::new(),
            spring: SpringAnalyzer::new(),
        }
    }
}

impl FrameworkAnalyzer for ExpressNestSpringAnalyzer {
    fn name(&self) -> &'static str {
        "express_nest_spring"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        self.express.matches_framework(path, source)
            || self.nestjs.matches_framework(path, source)
            || self.spring.matches_framework(path, source)
    }

    fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()> {
        if self.express.matches_framework(path, source) {
            self.express
                .enhance_slice(target_node, source, path, slice)?;
        }
        if self.nestjs.matches_framework(path, source) {
            self.nestjs
                .enhance_slice(target_node, source, path, slice)?;
        }
        if self.spring.matches_framework(path, source) {
            self.spring
                .enhance_slice(target_node, source, path, slice)?;
        }
        Ok(())
    }
}

fn is_express_route_method(func_text: &str) -> bool {
    let lower = func_text.to_lowercase();
    lower.ends_with(".get")
        || lower.ends_with(".post")
        || lower.ends_with(".put")
        || lower.ends_with(".delete")
        || lower.ends_with(".patch")
        || lower.ends_with(".all")
        || lower.ends_with(".use")
}

fn extract_express_param_dtos(node: Node<'_>, source: &str, out: &mut Vec<String>) {
    if let Some(params) = node.child_by_field_name("parameters") {
        for param in params.named_children(&mut params.walk()) {
            if let Some(type_ann) = param.child_by_field_name("type") {
                let text = AstUtils::node_text(type_ann, source);
                // Parse Request<Params, ResBody, ReqBody, ReqQuery>
                if text.contains("Request<") || text.contains("Response<") {
                    if let Some(start) = text.find('<') {
                        if let Some(end) = text.rfind('>') {
                            let inner = &text[start + 1..end];
                            for part in inner.split(',') {
                                let trimmed = part.trim();
                                for id in trimmed.split(|c: char| !c.is_alphanumeric() && c != '_')
                                {
                                    let id_clean = id.trim();
                                    if !id_clean.is_empty()
                                        && !is_builtin_ts_type(id_clean)
                                        && !out.contains(&id_clean.to_string())
                                    {
                                        out.push(id_clean.to_string());
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

fn find_enclosing_class(node: Node<'_>) -> Option<Node<'_>> {
    let mut curr = node;
    while let Some(parent) = curr.parent() {
        if parent.kind() == "class_declaration" || parent.kind() == "class" {
            return Some(parent);
        }
        curr = parent;
    }
    None
}

#[allow(dead_code)]
fn unwrap_generic_type(type_text: &str) -> Option<String> {
    if let Some(start) = type_text.find('<') {
        if let Some(end) = type_text.rfind('>') {
            let inner = type_text[start + 1..end].trim();
            let first = inner.split(',').next().unwrap_or(inner).trim();
            return Some(first.to_string());
        }
    }
    None
}

fn find_local_type_or_interface(
    root: Node<'_>,
    source: &str,
    target_name: &str,
    file_path: &Path,
) -> Option<ExtractedType> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let unwrapped = AstUtils::unwrap_export(child);
        let kind = unwrapped.kind();

        if kind == "interface_declaration"
            || kind == "type_alias_declaration"
            || kind == "class_declaration"
            || kind == "enum_declaration"
        {
            if let Some(name_node) = unwrapped.child_by_field_name("name") {
                if AstUtils::node_text(name_node, source) == target_name {
                    let k_str = match kind {
                        "interface_declaration" => "interface",
                        "type_alias_declaration" => "type",
                        "class_declaration" => "class",
                        "enum_declaration" => "enum",
                        _ => "type",
                    };
                    return Some(ExtractedType {
                        name: target_name.to_string(),
                        kind: k_str.to_string(),
                        file_path: file_path.to_string_lossy().to_string(),
                        definition: AstUtils::node_text(child, source).to_string(),
                    });
                }
            }
        }
    }
    None
}

fn is_builtin_ts_type(name: &str) -> bool {
    matches!(
        name,
        "string"
            | "number"
            | "boolean"
            | "any"
            | "unknown"
            | "void"
            | "null"
            | "undefined"
            | "never"
            | "object"
            | "symbol"
            | "bigint"
            | "Promise"
            | "Observable"
            | "Array"
            | "Record"
            | "Partial"
            | "Required"
            | "Readonly"
            | "Pick"
            | "Omit"
            | "Exclude"
            | "Extract"
            | "NonNullable"
            | "Request"
            | "Response"
            | "NextFunction"
    )
}

fn is_builtin_java_type(name: &str) -> bool {
    matches!(
        name,
        "String"
            | "Integer"
            | "int"
            | "Long"
            | "long"
            | "Double"
            | "double"
            | "Float"
            | "float"
            | "Boolean"
            | "boolean"
            | "Byte"
            | "byte"
            | "Short"
            | "short"
            | "Object"
            | "List"
            | "Set"
            | "Map"
            | "ResponseEntity"
            | "Optional"
            | "void"
            | "Void"
    )
}

fn extract_path_or_string(line: &str) -> Option<String> {
    for quote in ['\'', '"', '`'] {
        if let Some(first) = line.find(quote) {
            if let Some(second) = line[first + 1..].find(quote) {
                let candidate = &line[first + 1..first + 1 + second];
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn extract_path_from_args(s: &str) -> Option<String> {
    for quote in ['\'', '"', '`'] {
        if let Some(first) = s.find(quote) {
            if let Some(second) = s[first + 1..].find(quote) {
                let candidate = &s[first + 1..first + 1 + second];
                if candidate.starts_with('/') {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    None
}

fn extract_express_handler_name(s: &str) -> String {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 2 {
        let last = parts[parts.len() - 1].trim_matches([' ', ')', ';', '}']).trim();
        let name = last.split(['(', ' ']).next().unwrap_or("handler").trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }
    "handler".to_string()
}

