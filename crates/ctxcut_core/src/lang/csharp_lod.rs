//! C# Level of Detail (LOD) and AST-based class skeleton extraction engine.
//!
//! Uses `tree-sitter-c-sharp` to parse C# source code, extract class declarations,
//! preserve fields and serialize attributes, collapse non-target sibling methods,
//! and transitively expand small private helper methods for LOD 2 slices.

use crate::error::{CoreError, Result};
use crate::parser::{AstUtils, ParserManager};
use crate::slice::lod::{LodLevel, LodSliceResult};
use crate::slice::skeleton::{
    ClassMember, ClassSkeleton, SkeletonEngine, SkeletonField, SkeletonMethod, MAX_HELPER_LINES,
};
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::{Language, Node, Tree};

/// C# Level-of-Detail and skeleton extraction engine.
#[derive(Debug, Default, Clone, Copy)]
pub struct CSharpLodEngine;

impl CSharpLodEngine {
    /// Returns the tree-sitter Language definition for C#.
    pub fn tree_sitter_language() -> Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    /// Parses C# source code into a tree-sitter syntax tree.
    pub fn parse_csharp(source: &str) -> Result<Tree> {
        let lang = Self::tree_sitter_language();
        ParserManager::parse_source(source, &lang, Path::new("source.cs"))
    }

    /// Slices C# source code according to the requested LOD level and target symbol.
    pub fn slice_source(
        source: &str,
        symbol_query: &str,
        lod_level: LodLevel,
    ) -> Result<LodSliceResult> {
        let tree = Self::parse_csharp(source)?;
        let root = tree.root_node();
        let (container_query, member_query) = parse_query(symbol_query);

        // 1. Locate target method (or symbol) and enclosing class
        let (target_method_node, class_node, class_name) =
            find_target_method_and_class(root, source, container_query, member_query)?;

        // 2. Handle LOD 0: Signature only with XML-doc / doc comments
        if lod_level.is_signature_only() {
            return build_lod0_result(target_method_node, source, member_query, class_name);
        }

        // 3. Extract class skeleton for LOD 1 & LOD 2
        let skeleton = extract_class_skeleton(class_node, source)?;

        // 4. For LOD 2: Analyze invocations inside target method to find direct private helpers
        let (expanded_helpers, collapsed_methods) = if lod_level.includes_transitive_helpers() {
            resolve_lod2_helpers(target_method_node, &skeleton, source)
        } else {
            // In LOD 1, all sibling methods are collapsed
            let mut collapsed = Vec::new();
            for method in skeleton.methods() {
                if method.name != member_query {
                    collapsed.push(method.name.clone());
                }
            }
            (Vec::new(), collapsed)
        };

        // 5. Render skeleton according to LOD rules
        let content = skeleton.render(member_query, lod_level, &expanded_helpers);

        Ok(LodSliceResult {
            lod_level,
            target_symbol: member_query.to_string(),
            class_name: Some(class_name),
            content,
            expanded_helpers,
            collapsed_methods,
        })
    }

    /// Slices C# source code according to the requested LOD level and target symbol.
    pub fn slice_lod(
        source: &str,
        symbol_query: &str,
        lod_level: LodLevel,
    ) -> Result<LodSliceResult> {
        Self::slice_source(source, symbol_query, lod_level)
    }
}

impl SkeletonEngine for CSharpLodEngine {
    fn generate_slice(
        &self,
        source: &str,
        symbol_query: &str,
        lod: LodLevel,
    ) -> Result<LodSliceResult> {
        Self::slice_source(source, symbol_query, lod)
    }
}

/// Convenience function to slice C# source code with a given LOD level.
pub fn slice_csharp_lod(
    source: &str,
    symbol_query: &str,
    lod_level: LodLevel,
) -> Result<LodSliceResult> {
    CSharpLodEngine::slice_source(source, symbol_query, lod_level)
}

/// Splits a query like `"PlayerHealth.TakeDamage"` into `(Some("PlayerHealth"), "TakeDamage")`.
fn parse_query(query: &str) -> (Option<&str>, &str) {
    if let Some((container, member)) = query.split_once('.') {
        (Some(container.trim()), member.trim())
    } else {
        (None, query.trim())
    }
}

/// Locates target method and its enclosing class node.
fn find_target_method_and_class<'a>(
    root: Node<'a>,
    source: &'a str,
    container_query: Option<&str>,
    member_query: &str,
) -> Result<(Node<'a>, Node<'a>, String)> {
    let classes = AstUtils::find_descendants_by_kind(root, "class_declaration");

    // Try finding within specified container or any class
    for class_node in classes {
        let name_node = class_node.child_by_field_name("name");
        let class_name = name_node
            .map(|n| AstUtils::node_text(n, source))
            .unwrap_or_default();

        if let Some(target_container) = container_query {
            if class_name != target_container {
                continue;
            }
        }

        // Search methods in class
        let methods = AstUtils::find_descendants_by_kind(class_node, "method_declaration");
        for method_node in methods {
            if let Some(m_name) = method_node.child_by_field_name("name") {
                if AstUtils::node_text(m_name, source) == member_query {
                    return Ok((method_node, class_node, class_name.to_string()));
                }
            }
        }

        // Search constructors if needed
        let ctors = AstUtils::find_descendants_by_kind(class_node, "constructor_declaration");
        for ctor_node in ctors {
            if let Some(c_name) = ctor_node.child_by_field_name("name") {
                if AstUtils::node_text(c_name, source) == member_query {
                    return Ok((ctor_node, class_node, class_name.to_string()));
                }
            }
        }
    }

    Err(CoreError::SymbolNotFound {
        symbol: match container_query {
            Some(c) => format!("{c}.{member_query}"),
            None => member_query.to_string(),
        },
        path: Path::new("source.cs").to_path_buf(),
        available_symbols: AstUtils::find_descendants_by_kind(root, "method_declaration")
            .into_iter()
            .filter_map(|m| m.child_by_field_name("name"))
            .map(|n| AstUtils::node_text(n, source).to_string())
            .collect(),
    })
}

/// Builds LOD 0 slice containing only method signature and XML-doc / doc comments.
fn build_lod0_result(
    method_node: Node<'_>,
    source: &str,
    member_name: &str,
    class_name: String,
) -> Result<LodSliceResult> {
    let doc_comment = AstUtils::extract_doc_comment(method_node, source);
    let raw_signature = extract_method_signature(method_node, source);
    let mut signature = normalize_signature(&raw_signature);

    if !signature.ends_with(';') && !signature.ends_with('}') {
        signature.push(';');
    }

    let mut content = String::new();
    if let Some(doc) = &doc_comment {
        content.push_str(doc.trim());
        content.push('\n');
    }
    content.push_str(&signature);

    Ok(LodSliceResult {
        lod_level: LodLevel::Lod0,
        target_symbol: member_name.to_string(),
        class_name: Some(class_name),
        content,
        expanded_helpers: Vec::new(),
        collapsed_methods: Vec::new(),
    })
}

/// Extracts a `ClassSkeleton` structure from a tree-sitter `class_declaration` node.
pub fn extract_class_skeleton<'a>(class_node: Node<'a>, source: &'a str) -> Result<ClassSkeleton> {
    let name_node =
        class_node
            .child_by_field_name("name")
            .ok_or_else(|| CoreError::ParseError {
                path: Path::new("source.cs").to_path_buf(),
                message: "Missing class name".to_string(),
            })?;
    let class_name = AstUtils::node_text(name_node, source).to_string();

    let body_node = class_node
        .child_by_field_name("body")
        .or_else(|| AstUtils::find_child_by_kind(class_node, "declaration_list"))
        .ok_or_else(|| CoreError::ParseError {
            path: Path::new("source.cs").to_path_buf(),
            message: format!("Missing class body declaration list in {class_name}"),
        })?;

    // Class header from start of class node up to body opening brace
    let header_raw = &source[class_node.start_byte()..body_node.start_byte()];
    let header = header_raw.trim().to_string();

    let doc_comment = AstUtils::extract_doc_comment(class_node, source);

    let mut members = Vec::new();
    let mut cursor = body_node.walk();

    for child in body_node.children(&mut cursor) {
        match child.kind() {
            "field_declaration" => {
                let full_text = AstUtils::node_text(child, source).to_string();
                let attributes = AstUtils::find_children_by_kind(child, "attribute_list")
                    .into_iter()
                    .map(|a| AstUtils::node_text(a, source).to_string())
                    .collect();

                let modifiers = AstUtils::find_children_by_kind(child, "modifier")
                    .into_iter()
                    .map(|m| AstUtils::node_text(m, source).to_string())
                    .collect();

                let field_type = child
                    .child_by_field_name("type")
                    .map(|t| AstUtils::node_text(t, source).to_string())
                    .unwrap_or_default();

                let name = AstUtils::find_descendants_by_kind(child, "variable_declarator")
                    .first()
                    .and_then(|vd| vd.child_by_field_name("name"))
                    .map(|n| AstUtils::node_text(n, source).to_string())
                    .unwrap_or_default();

                members.push(ClassMember::Field(SkeletonField {
                    name,
                    field_type,
                    full_text,
                    attributes,
                    modifiers,
                }));
            }
            "method_declaration" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source).to_string())
                    .unwrap_or_default();

                let return_type = child
                    .child_by_field_name("type")
                    .map(|t| AstUtils::node_text(t, source).to_string())
                    .unwrap_or_default();

                let raw_sig = extract_method_signature(child, source);
                let signature = normalize_signature(&raw_sig);

                let full_text = AstUtils::node_text(child, source).to_string();
                let body_text = child
                    .child_by_field_name("body")
                    .map(|b| AstUtils::node_text(b, source).to_string())
                    .unwrap_or_default();

                let total_lines = child.end_position().row - child.start_position().row + 1;
                let body_lines = if let Some(b) = child.child_by_field_name("body") {
                    b.end_position().row - b.start_position().row + 1
                } else {
                    total_lines
                };

                let is_private = check_is_private(child, source);
                let doc_comment = AstUtils::extract_doc_comment(child, source);
                let attributes = AstUtils::find_children_by_kind(child, "attribute_list")
                    .into_iter()
                    .map(|a| AstUtils::node_text(a, source).to_string())
                    .collect();

                members.push(ClassMember::Method(SkeletonMethod {
                    name,
                    return_type,
                    signature,
                    full_text,
                    body_text,
                    line_count: total_lines,
                    body_line_count: body_lines,
                    is_private,
                    doc_comment,
                    attributes,
                }));
            }
            "property_declaration" | "constructor_declaration" => {
                let full_text = AstUtils::node_text(child, source).to_string();
                members.push(ClassMember::Other(full_text));
            }
            _ => {}
        }
    }

    Ok(ClassSkeleton {
        name: class_name,
        header,
        doc_comment,
        attributes: Vec::new(),
        members,
    })
}

/// Resolves direct private helpers called inside the target method for LOD 2.
fn resolve_lod2_helpers(
    target_method_node: Node<'_>,
    skeleton: &ClassSkeleton,
    source: &str,
) -> (Vec<String>, Vec<String>) {
    let target_name = target_method_node
        .child_by_field_name("name")
        .map(|n| AstUtils::node_text(n, source))
        .unwrap_or_default();

    // 1. Collect direct invocations inside the target method body
    let mut direct_callees = HashSet::new();
    let invocations =
        AstUtils::find_descendants_by_kind(target_method_node, "invocation_expression");

    for inv in invocations {
        if let Some(callee) = extract_callee_name(inv, source) {
            direct_callees.insert(callee.to_string());
        }
    }

    let mut expanded_helpers = Vec::new();
    let mut collapsed_methods = Vec::new();

    for method in skeleton.methods() {
        if method.name == target_name {
            continue;
        }

        // Direct private helper check:
        // Must be called directly, defined in the same class, and private
        let is_called = direct_callees.contains(&method.name);
        if is_called && method.is_private {
            // Helper line count <= 15 check
            let loc = method.body_line_count.min(method.line_count);
            if loc <= MAX_HELPER_LINES {
                expanded_helpers.push(method.name.clone());
                continue;
            }
        }

        collapsed_methods.push(method.name.clone());
    }

    (expanded_helpers, collapsed_methods)
}

/// Extracts callee method name from an invocation expression.
fn extract_callee_name<'a>(inv: Node<'a>, source: &'a str) -> Option<&'a str> {
    let expr = inv
        .child_by_field_name("expression")
        .or_else(|| inv.named_child(0))?;

    match expr.kind() {
        "identifier" => Some(AstUtils::node_text(expr, source)),
        "generic_name" => expr
            .child_by_field_name("name")
            .map(|n| AstUtils::node_text(n, source)),
        "member_access_expression" => {
            let obj = expr.child_by_field_name("expression")?;
            let obj_text = AstUtils::node_text(obj, source).trim();
            if obj_text == "this" {
                expr.child_by_field_name("name")
                    .map(|n| AstUtils::node_text(n, source))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extracts method signature text up to body block or arrow expression.
fn extract_method_signature(method_node: Node<'_>, source: &str) -> String {
    let raw = AstUtils::node_text(method_node, source);

    // Skip preceding attribute lists to get clean signature (e.g. `private void Awake()`)
    let sig_start = method_node
        .children(&mut method_node.walk())
        .find(|c| c.kind() != "attribute_list" && c.kind() != "comment")
        .map(|c| c.start_byte())
        .unwrap_or(method_node.start_byte());

    let offset_start = sig_start.saturating_sub(method_node.start_byte());

    if let Some(body) = method_node.child_by_field_name("body") {
        let offset_end = body.start_byte().saturating_sub(method_node.start_byte());
        if offset_start < offset_end && offset_end <= raw.len() {
            return raw[offset_start..offset_end].trim().to_string();
        }
    }

    if let Some(arrow) = AstUtils::find_child_by_kind(method_node, "arrow_expression_clause") {
        let offset_end = arrow.start_byte().saturating_sub(method_node.start_byte());
        if offset_start < offset_end && offset_end <= raw.len() {
            return raw[offset_start..offset_end].trim().to_string();
        }
    }

    if let Some(idx) = raw.find('{') {
        raw[offset_start..idx].trim().to_string()
    } else if let Some(idx) = raw.find(';') {
        raw[offset_start..=idx].trim().to_string()
    } else {
        raw.lines().next().unwrap_or(raw).trim().to_string()
    }
}

/// Collapses multi-line signature and irregular whitespace into a single clean line.
fn normalize_signature(sig: &str) -> String {
    let mut normalized = String::new();
    let mut prev_is_space = false;

    for c in sig.trim().chars() {
        if c.is_whitespace() {
            if !prev_is_space {
                normalized.push(' ');
                prev_is_space = true;
            }
        } else {
            normalized.push(c);
            prev_is_space = false;
        }
    }

    normalized
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" ,", ",")
}

/// Checks whether a method has private accessibility (explicit `private` or default in class).
fn check_is_private(method_node: Node<'_>, source: &str) -> bool {
    let mut has_access_mod = false;
    let mut is_priv = false;

    let mut cursor = method_node.walk();
    for child in method_node.children(&mut cursor) {
        if child.kind() == "modifier" {
            let m_text = AstUtils::node_text(child, source).trim();
            match m_text {
                "private" => {
                    has_access_mod = true;
                    is_priv = true;
                }
                "public" | "protected" | "internal" => {
                    has_access_mod = true;
                }
                _ => {}
            }
        }
    }

    if has_access_mod {
        is_priv
    } else {
        // In C# class declarations, methods without access modifiers default to private
        true
    }
}
