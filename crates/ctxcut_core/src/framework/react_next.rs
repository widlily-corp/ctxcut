//! React & Next.js Framework Analyzer and JSX Branch Collapser.
//!
//! Extracts Component Props interfaces, generic constraints, referenced custom hooks,
//! and collapses deep secondary presentation JSX branches into compact, informative stubs.

use crate::error::Result;
use crate::framework::FrameworkAnalyzer;
use crate::model::{CallSignatureStub, SliceOptions, SliceResult};
use crate::parser::AstUtils;
use crate::resolver::{SignatureStripper, TypeHoister};
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::Node;

/// React and Next.js semantic intelligence analyzer and JSX branch collapser.
#[derive(Debug, Clone)]
pub struct ReactNextAnalyzer {
    /// Maximum JSX depth to expand before collapsing secondary child branches (default: 2).
    pub max_jsx_depth: usize,
    /// Minimum lines of JSX child block to trigger branch collapsing (default: 3).
    pub min_collapse_lines: usize,
}

impl Default for ReactNextAnalyzer {
    fn default() -> Self {
        Self {
            max_jsx_depth: 2,
            min_collapse_lines: 3,
        }
    }
}

impl ReactNextAnalyzer {
    /// Creates a new `ReactNextAnalyzer` with default collapsing thresholds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `ReactNextAnalyzer` with custom depth and line count thresholds.
    pub fn with_thresholds(max_jsx_depth: usize, min_collapse_lines: usize) -> Self {
        Self {
            max_jsx_depth,
            min_collapse_lines,
        }
    }
}

impl FrameworkAnalyzer for ReactNextAnalyzer {
    fn name(&self) -> &'static str {
        "react_next"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "tsx" || ext == "jsx" {
            return true;
        }

        if ext == "ts" || ext == "js" {
            if source.contains("'use client'")
                || source.contains("\"use client\"")
                || source.contains("'use server'")
                || source.contains("\"use server\"")
                || source.contains("from 'react'")
                || source.contains("from \"react\"")
                || source.contains("from 'next/")
                || source.contains("from \"next/")
            {
                return true;
            }

            let path_str = path.to_string_lossy();
            if path_str.contains("app/")
                || path_str.contains("pages/")
                || path_str.contains("components/")
            {
                return true;
            }
        }

        false
    }

    fn enhance_slice(
        &self,
        target_node: Node<'_>,
        source: &str,
        path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let tree_sitter_lang = tree_sitter_typescript::LANGUAGE_TSX.into();
        let mut root = target_node;
        while let Some(parent) = root.parent() {
            root = parent;
        }

        // 1. Extract and hoist Props interface / generic constraint types
        if let Some(_props_type_name) = self.extract_props_type_name(target_node, source) {
            let opts = SliceOptions {
                depth: 2,
                include_types: true,
                include_calls: true,
            };
            if let Ok(mut hoisted) =
                TypeHoister::hoist_types(target_node, root, source, path, &opts, &tree_sitter_lang)
            {
                for ty in hoisted.drain(..) {
                    if !slice.hoisted_types.iter().any(|t| t.name == ty.name) {
                        slice.hoisted_types.push(ty);
                    }
                }
            }
        }

        // 2. Extract referenced custom hooks
        let custom_hooks =
            self.extract_custom_hooks(target_node, root, source, path, &tree_sitter_lang);
        for hook_stub in custom_hooks {
            if !slice
                .stripped_calls
                .iter()
                .any(|c| c.name == hook_stub.name)
            {
                slice.stripped_calls.push(hook_stub);
            }
        }

        // 3. Collapse secondary JSX branches in component body
        if let Some(collapsed_body) = self.collapse_jsx_branches(source, target_node) {
            slice.target_symbol.body = collapsed_body;
        }

        Ok(())
    }
}

impl ReactNextAnalyzer {
    /// Collapses secondary JSX presentation branches within a component while preserving custom component subtrees.
    pub fn collapse_jsx_branches(&self, source: &str, node: Node<'_>) -> Option<String> {
        let jsx_roots = find_jsx_return_roots(node);
        if jsx_roots.is_empty() {
            return None;
        }

        let mut replacements: Vec<(usize, usize, String)> = Vec::new();
        for jsx_root in jsx_roots {
            let collapsed_jsx = self.collapse_node(jsx_root, source, 0);
            replacements.push((jsx_root.start_byte(), jsx_root.end_byte(), collapsed_jsx));
        }

        replacements.sort_by_key(|r| std::cmp::Reverse(r.0));
        let node_start = node.start_byte();
        let node_end = node.end_byte();
        if node_start >= node_end || node_end > source.len() {
            return None;
        }

        let mut body = source[node_start..node_end].to_string();
        for (start, end, repl) in replacements {
            let local_start = start.saturating_sub(node_start);
            let local_end = end.saturating_sub(node_start);
            if local_start <= local_end && local_end <= body.len() {
                body.replace_range(local_start..local_end, &repl);
            }
        }

        Some(body)
    }

    /// Extracts the Props type identifier from function parameters or variable type annotations.
    pub fn extract_props_type_name<'a>(&self, node: Node<'a>, source: &'a str) -> Option<String> {
        let unwrapped = AstUtils::unwrap_export(node);

        // A. Function declaration parameters
        let fn_node = if unwrapped.kind() == "function_declaration" {
            Some(unwrapped)
        } else if unwrapped.kind() == "lexical_declaration"
            || unwrapped.kind() == "variable_declaration"
        {
            let declarators = AstUtils::find_children_by_kind(unwrapped, "variable_declarator");
            declarators
                .into_iter()
                .find_map(|d| d.child_by_field_name("value"))
                .filter(|v| v.kind() == "arrow_function" || v.kind() == "function_expression")
        } else if unwrapped.kind() == "variable_declarator" {
            unwrapped
                .child_by_field_name("value")
                .filter(|v| v.kind() == "arrow_function" || v.kind() == "function_expression")
        } else {
            None
        };

        if let Some(f_node) = fn_node {
            if let Some(params) = f_node.child_by_field_name("parameters") {
                if let Some(first_param) = params.named_child(0) {
                    if let Some(type_ann) = first_param.child_by_field_name("type") {
                        let type_text = AstUtils::node_text(type_ann, source)
                            .trim_start_matches(':')
                            .trim();
                        let base = type_text.split('<').next().unwrap_or(type_text).trim();
                        if !is_builtin_or_primitive(base) {
                            return Some(base.to_string());
                        }
                    }
                }
            }
        }

        // B. Variable declarator: const Comp: React.FC<Props> = ...
        if unwrapped.kind() == "lexical_declaration"
            || unwrapped.kind() == "variable_declaration"
            || unwrapped.kind() == "variable_declarator"
        {
            let decls = if unwrapped.kind() == "variable_declarator" {
                vec![unwrapped]
            } else {
                AstUtils::find_children_by_kind(unwrapped, "variable_declarator")
            };

            for decl in decls {
                if let Some(type_ann) = decl.child_by_field_name("type") {
                    let text = AstUtils::node_text(type_ann, source);
                    if let Some(props_name) = extract_generic_type_argument(text) {
                        return Some(props_name);
                    }
                }
                // Also check React.forwardRef<Ref, Props>
                if let Some(val) = decl.child_by_field_name("value") {
                    if val.kind() == "call_expression" {
                        if let Some(type_args) = val.child_by_field_name("type_arguments") {
                            let text = AstUtils::node_text(type_args, source);
                            if let Some(props_name) = extract_second_type_argument(text) {
                                return Some(props_name);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Identifies custom hooks and resolves their signature stubs.
    pub fn extract_custom_hooks<'a>(
        &self,
        node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        tree_sitter_lang: &tree_sitter::Language,
    ) -> Vec<CallSignatureStub> {
        let mut stubs = Vec::new();
        let mut seen = HashSet::new();

        let call_nodes = AstUtils::find_descendants_by_kind(node, "call_expression");
        for call in call_nodes {
            if let Some(fn_node) = call.child_by_field_name("function") {
                let name = if fn_node.kind() == "identifier" {
                    AstUtils::node_text(fn_node, source).to_string()
                } else if fn_node.kind() == "member_expression" {
                    fn_node
                        .child_by_field_name("property")
                        .map(|p| AstUtils::node_text(p, source).to_string())
                        .unwrap_or_default()
                } else {
                    continue;
                };

                if is_custom_hook_name(&name) && !seen.contains(&name) {
                    seen.insert(name.clone());
                    let mut resolved_any = false;
                    if let Ok(mut resolved) = SignatureStripper::strip_calls(
                        node,
                        root,
                        source,
                        file_path,
                        tree_sitter_lang,
                    ) {
                        for stub in resolved.drain(..) {
                            if stub.name == name
                                && !stubs.iter().any(|s: &CallSignatureStub| s.name == name)
                            {
                                stubs.push(stub);
                                resolved_any = true;
                            }
                        }
                    }

                    if !resolved_any && !stubs.iter().any(|s: &CallSignatureStub| s.name == name) {
                        stubs.push(CallSignatureStub {
                            name: name.clone(),
                            receiver: None,
                            file_path: Some(file_path.to_string_lossy().to_string()),
                            signature: format!("export function {name}(...args: any[]): any;"),
                        });
                    }
                }
            }
        }

        stubs
    }

    fn collapse_node<'a>(&self, node: Node<'a>, source: &'a str, depth: usize) -> String {
        match node.kind() {
            "jsx_element" => {
                let open_tag = match node.child_by_field_name("open_tag") {
                    Some(t) => t,
                    None => return AstUtils::node_text(node, source).to_string(),
                };
                let close_tag = node.child_by_field_name("close_tag");
                let tag_name = get_jsx_tag_name(open_tag, source);
                let is_custom = is_pascal_case(&tag_name);

                let line_count = node
                    .end_position()
                    .row
                    .saturating_sub(node.start_position().row)
                    + 1;
                let contains_custom = has_custom_component_descendant(node, source);
                let should_collapse = depth >= self.max_jsx_depth
                    && line_count >= self.min_collapse_lines
                    && !is_custom
                    && !contains_custom;

                let open_text = AstUtils::node_text(open_tag, source);
                if should_collapse {
                    let close_text = close_tag
                        .map(|c| AstUtils::node_text(c, source).to_string())
                        .unwrap_or_else(|| format!("</{tag_name}>"));
                    format!("{open_text}/* {line_count} lines collapsed */{close_text}")
                } else {
                    let mut inner = String::new();
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.id() != open_tag.id()
                            && close_tag.map_or(true, |c| c.id() != child.id())
                        {
                            inner.push_str(&self.collapse_node(child, source, depth + 1));
                        }
                    }
                    let close_text = close_tag
                        .map(|c| AstUtils::node_text(c, source))
                        .unwrap_or("");
                    format!("{open_text}{inner}{close_text}")
                }
            }
            "jsx_self_closing_element" => {
                let tag_name = get_jsx_tag_name(node, source);
                if tag_name == "svg" {
                    "<svg /* icon */ />".to_string()
                } else {
                    AstUtils::node_text(node, source).to_string()
                }
            }
            "jsx_fragment" => {
                let mut inner = String::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    let k = child.kind();
                    if k != "<" && k != ">" && k != "</" {
                        inner.push_str(&self.collapse_node(child, source, depth + 1));
                    }
                }
                format!("<>{inner}</>")
            }
            "jsx_expression" => {
                if let Some(inner) = node.named_child(0) {
                    if inner.kind() == "ternary_expression" {
                        if let (Some(cond), Some(conseq), Some(alt)) = (
                            inner.child_by_field_name("condition"),
                            inner.child_by_field_name("consequence"),
                            inner.child_by_field_name("alternative"),
                        ) {
                            let cond_t = AstUtils::node_text(cond, source);
                            let conseq_t = self.collapse_node(conseq, source, depth + 1);
                            let alt_t = self.collapse_node(alt, source, depth + 1);
                            return format!("{{{cond_t} ? {conseq_t} : {alt_t}}}");
                        }
                    } else if inner.kind() == "binary_expression" {
                        if let (Some(left), Some(right)) = (
                            inner.child_by_field_name("left"),
                            inner.child_by_field_name("right"),
                        ) {
                            let left_t = AstUtils::node_text(left, source);
                            let right_t = self.collapse_node(right, source, depth + 1);
                            return format!("{{{left_t} && {right_t}}}");
                        }
                    }
                }
                AstUtils::node_text(node, source).to_string()
            }
            "parenthesized_expression" => {
                if let Some(inner) = node.named_child(0) {
                    format!("({})", self.collapse_node(inner, source, depth))
                } else {
                    AstUtils::node_text(node, source).to_string()
                }
            }
            _ => AstUtils::node_text(node, source).to_string(),
        }
    }
}

fn has_custom_component_descendant(node: Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_element" || child.kind() == "jsx_self_closing_element" {
            let open_tag = if child.kind() == "jsx_element" {
                child.child_by_field_name("open_tag")
            } else {
                Some(child)
            };
            if let Some(tag) = open_tag {
                let tag_name = get_jsx_tag_name(tag, source);
                if is_pascal_case(&tag_name) {
                    return true;
                }
            }
        }
        if has_custom_component_descendant(child, source) {
            return true;
        }
    }
    false
}

fn find_jsx_return_roots(node: Node<'_>) -> Vec<Node<'_>> {
    let mut roots = Vec::new();
    let returns = AstUtils::find_descendants_by_kind(node, "return_statement");
    for ret in returns {
        for child in ret.named_children(&mut ret.walk()) {
            if matches!(
                child.kind(),
                "jsx_element"
                    | "jsx_fragment"
                    | "jsx_self_closing_element"
                    | "parenthesized_expression"
            ) {
                roots.push(child);
            }
        }
    }

    if node.kind() == "variable_declarator" {
        if let Some(val) = node.child_by_field_name("value") {
            if val.kind() == "arrow_function" {
                if let Some(body) = val.child_by_field_name("body") {
                    if matches!(
                        body.kind(),
                        "jsx_element"
                            | "jsx_fragment"
                            | "jsx_self_closing_element"
                            | "parenthesized_expression"
                    ) {
                        roots.push(body);
                    }
                }
            }
        }
    }

    roots
}

fn get_jsx_tag_name<'a>(node: Node<'a>, source: &'a str) -> String {
    if let Some(name_node) = node.child_by_field_name("name") {
        AstUtils::node_text(name_node, source).to_string()
    } else {
        node.named_children(&mut node.walk())
            .find(|c| c.kind() == "identifier" || c.kind() == "nested_identifier")
            .map(|n| AstUtils::node_text(n, source).to_string())
            .unwrap_or_else(|| "div".to_string())
    }
}

fn is_pascal_case(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_uppercase())
}

fn is_custom_hook_name(name: &str) -> bool {
    if !name.starts_with("use") || name.len() <= 3 {
        return false;
    }
    let fourth = name.chars().nth(3).unwrap_or('a');
    if !fourth.is_uppercase() {
        return false;
    }
    !matches!(
        name,
        "useState"
            | "useEffect"
            | "useContext"
            | "useReducer"
            | "useCallback"
            | "useMemo"
            | "useRef"
            | "useImperativeHandle"
            | "useLayoutEffect"
            | "useId"
            | "useTransition"
            | "useDeferredValue"
            | "useSyncExternalStore"
            | "useActionState"
            | "useOptimistic"
    )
}

fn extract_generic_type_argument(type_text: &str) -> Option<String> {
    if let Some(start) = type_text.find('<') {
        if let Some(end) = type_text.rfind('>') {
            let inner = type_text[start + 1..end].trim();
            let first_arg = inner.split(',').next().unwrap_or(inner).trim();
            if !first_arg.is_empty() && !is_builtin_or_primitive(first_arg) {
                return Some(first_arg.to_string());
            }
        }
    }
    None
}

fn extract_second_type_argument(type_text: &str) -> Option<String> {
    if let Some(start) = type_text.find('<') {
        if let Some(end) = type_text.rfind('>') {
            let inner = type_text[start + 1..end].trim();
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() >= 2 {
                let second = parts[1].trim();
                if !second.is_empty() && !is_builtin_or_primitive(second) {
                    return Some(second.to_string());
                }
            }
        }
    }
    None
}

fn is_builtin_or_primitive(name: &str) -> bool {
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
            | "JSX.Element"
            | "React.ReactNode"
            | "ReactNode"
            | "ReactElement"
    )
}
