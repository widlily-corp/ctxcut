//! C# Preprocessor analysis, AST traversal across conditional compilation blocks,
//! and `PreprocessorContextDetector` implementation for C# / .NET.
//!
//! Handles `#if`, `#elif`, `#else`, `#endif`, `#define`, and `#undef` directives,
//! providing equal accessibility to symbols inside conditional compilation branches
//! and emitting contextual tags (e.g. `// [Active under: UNITY_EDITOR]`).

use crate::error::{CoreError, Result};
use crate::lang::csharp::CSharpAdapter;
use crate::lang::preprocessor::{
    combine_condition_strings, invert_condition_string, PreprocessorBlock, PreprocessorBranchKind,
    PreprocessorContext, PreprocessorContextDetector,
};
use crate::lang::LanguageAdapter;
use crate::model::{
    CallSignatureStub, ExtractedImplementor, ExtractedSymbol, ExtractedType, SliceOptions,
    SupportedLanguage,
};
use crate::parser::{AstUtils, ParserManager};
use std::collections::HashSet;
use std::ops::Range;
use std::path::Path;
use tree_sitter::{Language, Node};

/// Detector and symbol locator aware of C# preprocessor conditional compilation blocks.
#[derive(Debug, Clone, Default)]
pub struct CSharpPreprocessorDetector {
    /// Symbols explicitly defined with `#define` in the compilation unit.
    defined_symbols: HashSet<String>,
    /// Pre-indexed conditional preprocessor blocks.
    blocks: Vec<PreprocessorBlock>,
}

impl CSharpPreprocessorDetector {
    /// Creates an empty `CSharpPreprocessorDetector`.
    pub fn new() -> Self {
        Self {
            defined_symbols: HashSet::new(),
            blocks: Vec::new(),
        }
    }

    /// Constructs a detector from verbatim C# source code by parsing and indexing AST preprocessor nodes.
    pub fn from_source(source: &str) -> Self {
        let lang: Language = tree_sitter_c_sharp::LANGUAGE.into();
        let path = Path::new("source.cs");
        if let Ok(tree) = ParserManager::parse_source(source, &lang, path) {
            Self::from_ast(tree.root_node(), source)
        } else {
            Self::new()
        }
    }

    /// Constructs a detector from an already parsed C# AST root node.
    pub fn from_ast(root: Node<'_>, source: &str) -> Self {
        let mut defined = HashSet::new();
        for def_node in AstUtils::find_descendants_by_kind(root, "preproc_define") {
            if let Some(arg) = AstUtils::find_child_by_kind(def_node, "preproc_arg") {
                let name = AstUtils::node_text(arg, source).trim();
                if !name.is_empty() {
                    defined.insert(name.to_string());
                }
            }
        }

        let mut blocks = Vec::new();
        collect_preprocessor_blocks(root, source, &mut blocks, Vec::new());
        // Sort blocks by start_byte ascending, and by length descending
        blocks.sort_by(|a, b| {
            a.byte_range.start.cmp(&b.byte_range.start).then_with(|| {
                (b.byte_range.end - b.byte_range.start)
                    .cmp(&(a.byte_range.end - a.byte_range.start))
            })
        });

        Self {
            defined_symbols: defined,
            blocks,
        }
    }

    /// Returns whether a given identifier symbol was defined via `#define`.
    pub fn is_symbol_defined(&self, symbol: &str) -> bool {
        self.defined_symbols.contains(symbol)
    }

    /// Returns a reference to all symbols defined via `#define`.
    pub fn defined_symbols(&self) -> &HashSet<String> {
        &self.defined_symbols
    }

    /// Returns all indexed preprocessor blocks.
    pub fn blocks(&self) -> &[PreprocessorBlock] {
        &self.blocks
    }

    /// Locates an AST symbol in the C# source, properly descending through `#if`, `#elif`, and `#else`
    /// branches, and attaches the preprocessor context tag comment to the symbol body.
    pub fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        let (sym, node, _) =
            self.locate_symbol_with_context(root, source, symbol_query, file_path)?;
        Ok((sym, node))
    }

    /// Locates all matching occurrences of a symbol across different conditional preprocessor branches.
    pub fn locate_all_symbols<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Vec<(ExtractedSymbol, Node<'a>, Option<PreprocessorContext>)> {
        let (container_query, member_query) = parse_query(symbol_query);
        let mut results = Vec::new();

        if let Some(container_name) = container_query {
            let containers = find_all_containers(root, source, container_name);
            for container_node in containers {
                let mut members = Vec::new();
                collect_members_in_container(container_node, source, member_query, &mut members);
                for member_node in members {
                    let full_name = format!("{container_name}.{member_query}");
                    let mut sym = build_csharp_symbol(member_node, source, file_path, &full_name);
                    let ctx = self.detect_context_for_node(member_node, source);
                    if let Some(ref c) = ctx {
                        self.annotate_symbol(&mut sym, c);
                    }
                    results.push((sym, member_node, ctx));
                }
            }
        } else {
            // Find methods/properties
            let mut matches = Vec::new();
            find_all_members_global(root, source, member_query, &mut matches);
            for node in matches {
                let container_name = find_enclosing_class_name(node, source);
                let full_name = match container_name {
                    Some(ref c) => format!("{c}.{member_query}"),
                    None => member_query.to_string(),
                };
                let mut sym = build_csharp_symbol(node, source, file_path, &full_name);
                let ctx = self.detect_context_for_node(node, source);
                if let Some(ref c) = ctx {
                    self.annotate_symbol(&mut sym, c);
                }
                results.push((sym, node, ctx));
            }

            // Find types
            let mut types = Vec::new();
            find_all_top_level_types(root, source, member_query, &mut types);
            for node in types {
                let mut sym = build_csharp_symbol(node, source, file_path, member_query);
                let ctx = self.detect_context_for_node(node, source);
                if let Some(ref c) = ctx {
                    self.annotate_symbol(&mut sym, c);
                }
                results.push((sym, node, ctx));
            }
        }

        results
    }

    /// Lists all candidate top-level or exported symbol names defined in the AST,
    /// ensuring symbols inside `#if`, `#elif`, and `#else` directives are fully listed.
    pub fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String> {
        let mut symbols = Vec::new();
        collect_symbols_preproc_recursive(root, source, &mut symbols, None);
        symbols.sort();
        symbols.dedup();
        symbols
    }
}

impl PreprocessorContextDetector for CSharpPreprocessorDetector {
    fn detect_context_for_node(&self, node: Node<'_>, source: &str) -> Option<PreprocessorContext> {
        // 1. Walk up the AST parent chain
        let mut condition_stack = Vec::new();
        let mut curr = node.parent();
        let mut prev = node;
        let mut branch_kind = PreprocessorBranchKind::If;

        while let Some(parent) = curr {
            match parent.kind() {
                "preproc_else" => {
                    // Node is inside an #else branch
                    let mut parent_if = parent.parent();
                    while let Some(p) = parent_if {
                        if p.kind() == "preproc_if" || p.kind() == "preproc_elif" {
                            let cond = extract_csharp_condition(p, source);
                            let inverted = invert_condition_string(&cond);
                            if !inverted.is_empty() {
                                condition_stack.push(inverted);
                                branch_kind = PreprocessorBranchKind::Else;
                            }
                            break;
                        }
                        parent_if = p.parent();
                    }
                }
                "preproc_elif" => {
                    // Only if we did not come from alternative
                    if prev.kind() != "preproc_else" && prev.kind() != "preproc_elif" {
                        let cond = extract_csharp_condition(parent, source);
                        if !cond.is_empty() {
                            condition_stack.push(cond);
                            branch_kind = PreprocessorBranchKind::Elif;
                        }
                    }
                }
                "preproc_if" if prev.kind() != "preproc_elif" && prev.kind() != "preproc_else" => {
                    let cond = extract_csharp_condition(parent, source);
                    if !cond.is_empty() {
                        condition_stack.push(cond);
                        branch_kind = PreprocessorBranchKind::If;
                    }
                }
                _ => {}
            }
            prev = parent;
            curr = parent.parent();
        }

        if !condition_stack.is_empty() {
            condition_stack.reverse();
            let effective = condition_stack.iter().fold(String::new(), |acc, cond| {
                if acc.is_empty() {
                    cond.clone()
                } else {
                    combine_condition_strings(&acc, cond)
                }
            });
            return Some(PreprocessorContext::new(
                effective,
                condition_stack,
                branch_kind,
            ));
        }

        // 2. Fallback to indexed byte ranges
        self.detect_context_for_range(node.start_byte()..node.end_byte(), source)
    }

    fn detect_context_for_range(
        &self,
        range: Range<usize>,
        _source: &str,
    ) -> Option<PreprocessorContext> {
        let mut matching_blocks: Vec<&PreprocessorBlock> = self
            .blocks
            .iter()
            .filter(|b| b.byte_range.start <= range.start && b.byte_range.end >= range.end)
            .collect();

        // Sort from outermost to innermost (smallest range is innermost)
        matching_blocks.sort_by_key(|b| b.byte_range.end - b.byte_range.start);

        matching_blocks.first().map(|innermost| {
            PreprocessorContext::new(
                innermost.effective_condition.clone(),
                innermost.enclosing_conditions.clone(),
                innermost.kind,
            )
        })
    }

    fn locate_symbol_with_context<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>, Option<PreprocessorContext>)> {
        let matches = self.locate_all_symbols(root, source, symbol_query, file_path);
        if let Some((sym, node, ctx)) = matches.into_iter().next() {
            Ok((sym, node, ctx))
        } else {
            let available = self.list_symbols(root, source);
            Err(CoreError::SymbolNotFound {
                symbol: symbol_query.to_string(),
                path: file_path.to_path_buf(),
                available_symbols: available,
            })
        }
    }
}

impl LanguageAdapter for CSharpPreprocessorDetector {
    fn language(&self) -> SupportedLanguage {
        SupportedLanguage::CSharp
    }

    fn tree_sitter_language(&self, _path: &Path) -> Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<(ExtractedSymbol, Node<'a>)> {
        self.locate_symbol(root, source, symbol_query, file_path)
    }

    fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String> {
        self.list_symbols(root, source)
    }

    fn hoist_types<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>> {
        CSharpAdapter.hoist_types(target_node, root, source, file_path, opts)
    }

    fn strip_calls<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
    ) -> Result<Vec<CallSignatureStub>> {
        CSharpAdapter.strip_calls(target_node, root, source, file_path)
    }

    fn find_implementors<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        interface_name: &str,
        file_path: &Path,
    ) -> Result<Vec<ExtractedImplementor>> {
        CSharpAdapter.find_implementors(root, source, interface_name, file_path)
    }
}

// =========================================================================
// AST TRAVERSAL & EXTRACTION INTERNALS
// =========================================================================

/// Extracts the condition expression text from a `preproc_if` or `preproc_elif` node.
pub fn extract_csharp_condition<'a>(node: Node<'a>, source: &'a str) -> String {
    if let Some(cond_node) = node.child_by_field_name("condition") {
        let text = AstUtils::node_text(cond_node, source).trim();
        if !text.is_empty() {
            return text.to_string();
        }
    }

    let mut cursor = node.walk();
    let mut after_directive = false;
    for child in node.children(&mut cursor) {
        if after_directive
            && child.is_named()
            && child.kind() != "preproc_elif"
            && child.kind() != "preproc_else"
        {
            return AstUtils::node_text(child, source).trim().to_string();
        }
        if child.kind() == "#if" || child.kind() == "#elif" {
            after_directive = true;
        }
    }

    let raw_line = source[node.start_byte()..].lines().next().unwrap_or("");
    for kw in &["#if", "#elif"] {
        if let Some(pos) = raw_line.find(kw) {
            let rest = &raw_line[pos + kw.len()..];
            let clean = rest.split("//").next().unwrap_or(rest).trim();
            if !clean.is_empty() {
                return clean.to_string();
            }
        }
    }

    String::new()
}

#[allow(clippy::needless_pass_by_value)]
fn collect_preprocessor_blocks(
    node: Node<'_>,
    source: &str,
    blocks: &mut Vec<PreprocessorBlock>,
    enclosing_stack: Vec<String>,
) {
    if node.kind() == "preproc_if" {
        let cond = extract_csharp_condition(node, source);
        let mut if_stack = enclosing_stack.clone();
        if !cond.is_empty() {
            if_stack.push(cond.clone());
        }
        let effective_if = if_stack.join(" && ");

        let mut branch_end = node.end_byte();
        if let Some(alt) = node.child_by_field_name("alternative") {
            branch_end = alt.start_byte();
        } else if let Some(endif) = AstUtils::find_child_by_kind(node, "#endif") {
            branch_end = endif.start_byte();
        }

        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        blocks.push(PreprocessorBlock {
            kind: PreprocessorBranchKind::If,
            condition: cond,
            effective_condition: effective_if,
            byte_range: node.start_byte()..branch_end,
            start_line,
            end_line,
            enclosing_conditions: if_stack.clone(),
        });

        // Traverse children inside the if branch
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "preproc_elif" && child.kind() != "preproc_else" {
                collect_preprocessor_blocks(child, source, blocks, if_stack.clone());
            }
        }

        // Process alternatives: preproc_elif or preproc_else
        if let Some(alt) = node.child_by_field_name("alternative") {
            collect_alternative_blocks(alt, source, blocks, enclosing_stack.clone());
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_preprocessor_blocks(child, source, blocks, enclosing_stack.clone());
    }
}

#[allow(clippy::needless_pass_by_value)]
fn collect_alternative_blocks(
    node: Node<'_>,
    source: &str,
    blocks: &mut Vec<PreprocessorBlock>,
    enclosing_stack: Vec<String>,
) {
    match node.kind() {
        "preproc_elif" => {
            let cond = extract_csharp_condition(node, source);
            let mut elif_stack = enclosing_stack.clone();
            if !cond.is_empty() {
                elif_stack.push(cond.clone());
            }
            let effective_elif = elif_stack.join(" && ");

            let mut branch_end = node.end_byte();
            if let Some(alt) = node.child_by_field_name("alternative") {
                branch_end = alt.start_byte();
            }

            blocks.push(PreprocessorBlock {
                kind: PreprocessorBranchKind::Elif,
                condition: cond,
                effective_condition: effective_elif,
                byte_range: node.start_byte()..branch_end,
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                enclosing_conditions: elif_stack.clone(),
            });

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "preproc_elif" && child.kind() != "preproc_else" {
                    collect_preprocessor_blocks(child, source, blocks, elif_stack.clone());
                }
            }

            if let Some(alt) = node.child_by_field_name("alternative") {
                collect_alternative_blocks(alt, source, blocks, enclosing_stack);
            }
        }
        "preproc_else" => {
            let mut parent_cond = String::new();
            if let Some(p) = node.parent() {
                parent_cond = extract_csharp_condition(p, source);
            }
            let inverted = invert_condition_string(&parent_cond);
            let mut else_stack = enclosing_stack.clone();
            if !inverted.is_empty() {
                else_stack.push(inverted.clone());
            }
            let effective_else = else_stack.join(" && ");

            blocks.push(PreprocessorBlock {
                kind: PreprocessorBranchKind::Else,
                condition: inverted,
                effective_condition: effective_else,
                byte_range: node.start_byte()..node.end_byte(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                enclosing_conditions: else_stack.clone(),
            });

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_preprocessor_blocks(child, source, blocks, else_stack.clone());
            }
        }
        _ => {}
    }
}

fn parse_query(q: &str) -> (Option<&str>, &str) {
    if let Some(idx) = q.rfind('.') {
        (Some(&q[..idx]), &q[idx + 1..])
    } else {
        (None, q)
    }
}

fn find_all_containers<'a>(root: Node<'a>, source: &'a str, container_name: &str) -> Vec<Node<'a>> {
    let mut containers = Vec::new();
    collect_containers_recursive(root, source, container_name, &mut containers);
    containers
}

fn collect_containers_recursive<'a>(
    node: Node<'a>,
    source: &'a str,
    target_name: &str,
    result: &mut Vec<Node<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_declaration"
            | "record_declaration"
            | "record_struct_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == target_name {
                        result.push(child);
                    }
                }
                // Also search nested containers
                collect_containers_recursive(child, source, target_name, result);
            }
            "namespace_declaration"
            | "file_scoped_namespace_declaration"
            | "preproc_if"
            | "preproc_elif"
            | "preproc_else"
            | "declaration_list" => {
                collect_containers_recursive(child, source, target_name, result);
            }
            _ => {}
        }
    }
}

fn collect_members_in_container<'a>(
    container: Node<'a>,
    source: &'a str,
    member_name: &str,
    result: &mut Vec<Node<'a>>,
) {
    if let Some(body) = container
        .child_by_field_name("body")
        .or_else(|| AstUtils::find_child_by_kind(container, "declaration_list"))
    {
        collect_members_in_body(body, source, member_name, result);
    }
}

fn collect_members_in_body<'a>(
    node: Node<'a>,
    source: &'a str,
    member_name: &str,
    result: &mut Vec<Node<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "method_declaration"
            | "constructor_declaration"
            | "property_declaration"
            | "field_declaration"
            | "event_declaration"
            | "indexer_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == member_name {
                        result.push(child);
                    }
                }
            }
            "preproc_if" | "preproc_elif" | "preproc_else" | "declaration_list" => {
                // Descend into preprocessor directives inside the body
                collect_members_in_body(child, source, member_name, result);
            }
            _ => {}
        }
    }
}

fn find_all_members_global<'a>(
    node: Node<'a>,
    source: &'a str,
    member_name: &str,
    result: &mut Vec<Node<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "method_declaration" | "constructor_declaration" | "property_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == member_name {
                        result.push(child);
                    }
                }
            }
            _ => {
                find_all_members_global(child, source, member_name, result);
            }
        }
    }
}

fn find_all_top_level_types<'a>(
    node: Node<'a>,
    source: &'a str,
    type_name: &str,
    result: &mut Vec<Node<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_declaration"
            | "record_declaration"
            | "record_struct_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if AstUtils::node_text(name_node, source) == type_name {
                        result.push(child);
                    }
                }
            }
            "namespace_declaration"
            | "file_scoped_namespace_declaration"
            | "preproc_if"
            | "preproc_elif"
            | "preproc_else" => {
                find_all_top_level_types(child, source, type_name, result);
            }
            _ => {}
        }
    }
}

fn find_enclosing_class_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut current = node.parent();
    while let Some(n) = current {
        if matches!(
            n.kind(),
            "class_declaration"
                | "record_declaration"
                | "struct_declaration"
                | "interface_declaration"
        ) {
            if let Some(name_node) = n.child_by_field_name("name") {
                return Some(AstUtils::node_text(name_node, source).to_string());
            }
        }
        current = n.parent();
    }
    None
}

fn collect_symbols_preproc_recursive(
    node: Node<'_>,
    source: &str,
    symbols: &mut Vec<String>,
    current_container: Option<&str>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_declaration"
            | "record_declaration"
            | "record_struct_declaration"
            | "interface_declaration"
            | "struct_declaration"
            | "enum_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let type_name = AstUtils::node_text(name_node, source).to_string();
                    let full_type = match current_container {
                        Some(c) => format!("{c}.{type_name}"),
                        None => type_name.clone(),
                    };
                    symbols.push(full_type.clone());
                    if let Some(body) = child
                        .child_by_field_name("body")
                        .or_else(|| AstUtils::find_child_by_kind(child, "declaration_list"))
                    {
                        collect_symbols_preproc_recursive(body, source, symbols, Some(&full_type));
                    }
                }
            }
            "method_declaration" | "constructor_declaration" | "property_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let m_name = AstUtils::node_text(name_node, source);
                    let sym = match current_container {
                        Some(c) => format!("{c}.{m_name}"),
                        None => m_name.to_string(),
                    };
                    symbols.push(sym);
                }
            }
            "namespace_declaration"
            | "file_scoped_namespace_declaration"
            | "declaration_list"
            | "preproc_if"
            | "preproc_elif"
            | "preproc_else" => {
                collect_symbols_preproc_recursive(child, source, symbols, current_container);
            }
            _ => {}
        }
    }
}

fn build_csharp_symbol(
    node: Node<'_>,
    source: &str,
    file_path: &Path,
    name: &str,
) -> ExtractedSymbol {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let doc_comment = extract_csharp_doc_comment(node, source);
    let signature = extract_csharp_signature(node, source);
    let body = AstUtils::node_text(node, source).to_string();

    let kind = match node.kind() {
        "class_declaration" => "class",
        "record_declaration" | "record_struct_declaration" => "record",
        "interface_declaration" => "interface",
        "struct_declaration" => "struct",
        "enum_declaration" => "enum",
        "method_declaration" => {
            if name.contains('.') {
                "method"
            } else {
                "function"
            }
        }
        "constructor_declaration" => "constructor",
        "property_declaration" => "property",
        _ => "function",
    };

    ExtractedSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        start_line,
        end_line,
        doc_comment,
        signature,
        body,
        language: "csharp".to_string(),
        preprocessor_tag: None,
    }
}

fn extract_csharp_doc_comment(node: Node<'_>, source: &str) -> Option<String> {
    let mut comments = Vec::new();
    let mut prev = node.prev_named_sibling();

    while let Some(p) = prev {
        if p.kind() == "comment" {
            comments.push(AstUtils::node_text(p, source).trim().to_string());
            prev = p.prev_named_sibling();
        } else {
            break;
        }
    }

    if comments.is_empty() {
        None
    } else {
        comments.reverse();
        Some(comments.join("\n"))
    }
}

fn extract_csharp_signature(node: Node<'_>, source: &str) -> String {
    let raw = AstUtils::node_text(node, source);
    if let Some(body) = node.child_by_field_name("body") {
        let offset = body.start_byte() - node.start_byte();
        return raw[..offset].trim().to_string();
    }
    if let Some(arrow) = AstUtils::find_child_by_kind(node, "arrow_expression_clause") {
        let offset = arrow.start_byte() - node.start_byte();
        return raw[..offset].trim().to_string();
    }
    if let Some(idx) = raw.find('{') {
        raw[..idx].trim().to_string()
    } else if let Some(idx) = raw.find(';') {
        raw[..=idx].trim().to_string()
    } else {
        raw.lines().next().unwrap_or(raw).trim().to_string()
    }
}
