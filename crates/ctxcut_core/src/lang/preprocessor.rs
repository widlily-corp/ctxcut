//! Preprocessor abstractions, condition analysis, and context detection traits.
//!
//! Provides data structures and traits for tracking active `#if`, `#elif`, `#else`
//! conditional compilation directives across languages (specifically C# and C/C++),
//! enabling language adapters to extract contextual tags such as `// [Active under: UNITY_EDITOR]`.

use crate::error::Result;
use crate::model::ExtractedSymbol;
use std::collections::HashSet;
use std::ops::Range;
use tree_sitter::Node;

/// Preprocessor directive branch type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreprocessorBranchKind {
    /// `#if <condition>`
    If,
    /// `#elif <condition>`
    Elif,
    /// `#else`
    Else,
}

impl PreprocessorBranchKind {
    /// Returns the directive keyword associated with this branch kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::If => "#if",
            Self::Elif => "#elif",
            Self::Else => "#else",
        }
    }
}

/// Represents an active preprocessor context for a code slice, symbol, or AST node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorContext {
    /// Effective compound condition (e.g. `UNITY_EDITOR`, `!UNITY_SERVER`, `UNITY_EDITOR && DEBUG`).
    pub condition: String,
    /// All enclosing condition strings ordered from outermost to innermost.
    pub enclosing_conditions: Vec<String>,
    /// The branch kind of the innermost block.
    pub branch_kind: PreprocessorBranchKind,
    /// Preformatted contextual tag comment, e.g. `// [Active under: UNITY_EDITOR]`.
    pub tag: String,
}

impl PreprocessorContext {
    /// Constructs a new `PreprocessorContext` with formatted tag.
    pub fn new(
        condition: impl Into<String>,
        enclosing_conditions: Vec<String>,
        branch_kind: PreprocessorBranchKind,
    ) -> Self {
        let condition = condition.into();
        let tag = format_preprocessor_tag(&condition);
        Self {
            condition,
            enclosing_conditions,
            branch_kind,
            tag,
        }
    }

    /// Returns the contextual comment tag string (e.g. `// [Active under: UNITY_EDITOR]`).
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Annotates a verbatim source slice by prepending the active context tag.
    pub fn annotate_slice(&self, source_slice: &str) -> String {
        let trimmed = source_slice.trim();
        if trimmed.starts_with("// [Active under:") {
            source_slice.to_string()
        } else {
            format!("{}\n{}", self.tag, source_slice)
        }
    }

    /// Evaluates whether this context is active given a set of defined compilation symbols.
    pub fn is_active_under(&self, defined_symbols: &HashSet<String>) -> bool {
        evaluate_condition_expression(&self.condition, defined_symbols)
    }
}

/// Metadata describing a single conditional preprocessor block in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorBlock {
    /// Kind of branch (`#if`, `#elif`, `#else`).
    pub kind: PreprocessorBranchKind,
    /// The local condition expression for this branch.
    pub condition: String,
    /// The effective compound condition when taking parent blocks into account.
    pub effective_condition: String,
    /// Byte range spanned by this conditional block's body in the source code.
    pub byte_range: Range<usize>,
    /// 1-based start line of this block.
    pub start_line: usize,
    /// 1-based end line of this block.
    pub end_line: usize,
    /// Enclosing conditions from root to this block.
    pub enclosing_conditions: Vec<String>,
}

/// Common trait for language-specific preprocessor context detectors.
pub trait PreprocessorContextDetector: Send + Sync {
    /// Determines the active preprocessor context for a given AST node.
    fn detect_context_for_node(&self, node: Node<'_>, source: &str) -> Option<PreprocessorContext>;

    /// Determines the active preprocessor context for an arbitrary byte range in source code.
    fn detect_context_for_range(
        &self,
        range: Range<usize>,
        source: &str,
    ) -> Option<PreprocessorContext>;

    /// Formats a contextual comment tag for a given condition string.
    fn format_tag(&self, condition: &str) -> String {
        format_preprocessor_tag(condition)
    }

    /// Combines an outer enclosing condition with an inner nested condition.
    fn combine_conditions(&self, outer: &str, inner: &str) -> String {
        combine_condition_strings(outer, inner)
    }

    /// Inverts a condition expression for `#else` branches.
    fn invert_condition(&self, condition: &str) -> String {
        invert_condition_string(condition)
    }

    /// Annotates an `ExtractedSymbol` with preprocessor context if active.
    fn annotate_symbol(&self, symbol: &mut ExtractedSymbol, context: &PreprocessorContext) {
        if !symbol.body.starts_with("// [Active under:") {
            symbol.body = format!("{}\n{}", context.tag, symbol.body);
        }
    }

    /// Locates an AST symbol with preprocessor context attached.
    fn locate_symbol_with_context<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &std::path::Path,
    ) -> Result<(ExtractedSymbol, Node<'a>, Option<PreprocessorContext>)>;
}

/// Formats a condition string into a standard context tag comment.
///
/// Example: `format_preprocessor_tag("UNITY_EDITOR")` -> `"// [Active under: UNITY_EDITOR]"`
pub fn format_preprocessor_tag(condition: &str) -> String {
    format!("// [Active under: {}]", condition.trim())
}

/// Combines an outer condition with an inner condition using boolean conjunction (`&&`).
///
/// Handles operator precedence by wrapping disjunctive expressions in parentheses.
pub fn combine_condition_strings(outer: &str, inner: &str) -> String {
    let o = outer.trim();
    let i = inner.trim();

    if o.is_empty() {
        return i.to_string();
    }
    if i.is_empty() {
        return o.to_string();
    }

    let norm_o = if o.contains("||") && !is_parenthesized(o) {
        format!("({o})")
    } else {
        o.to_string()
    };

    let norm_i = if i.contains("||") && !is_parenthesized(i) {
        format!("({i})")
    } else {
        i.to_string()
    };

    format!("{norm_o} && {norm_i}")
}

/// Inverts a condition expression for `#else` branches.
///
/// Simplifies double negations:
/// - `!UNITY_SERVER` -> `UNITY_SERVER`
/// - `!(UNITY_SERVER)` -> `UNITY_SERVER`
/// - `UNITY_EDITOR` -> `!UNITY_EDITOR`
/// - `UNITY_EDITOR || DEBUG` -> `!(UNITY_EDITOR || DEBUG)`
pub fn invert_condition_string(condition: &str) -> String {
    let trimmed = condition.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Direct negation prefix check
    if let Some(stripped) = trimmed.strip_prefix('!') {
        let inner = stripped.trim();
        if is_parenthesized(inner) {
            let unparenthesized = &inner[1..inner.len() - 1];
            if is_parentheses_balanced(unparenthesized) {
                return unparenthesized.trim().to_string();
            }
        }
        if !inner.contains(' ') && !inner.contains('&') && !inner.contains('|') {
            return inner.to_string();
        }
    }

    // !(...) pattern check
    if trimmed.starts_with("!(") && trimmed.ends_with(')') {
        let inner = &trimmed[2..trimmed.len() - 1];
        if is_parentheses_balanced(inner) {
            return inner.trim().to_string();
        }
    }

    // Simple single identifier
    if !trimmed.contains(' ')
        && !trimmed.contains('&')
        && !trimmed.contains('|')
        && !trimmed.contains('(')
    {
        return format!("!{trimmed}");
    }

    // Already parenthesized compound expression
    if is_parenthesized(trimmed) {
        return format!("!{trimmed}");
    }

    // General compound expression
    format!("!({trimmed})")
}

/// Checks if an expression string is fully enclosed in matching outer parentheses.
fn is_parenthesized(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') {
        return false;
    }
    let mut depth = 0;
    for (idx, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && idx < s.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// Checks if parentheses within a substring are properly balanced.
fn is_parentheses_balanced(s: &str) -> bool {
    let mut depth = 0;
    for ch in s.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    depth == 0
}

/// Evaluates a boolean preprocessor expression against a set of defined symbols.
#[allow(clippy::implicit_hasher)]
pub fn evaluate_condition_expression(expr: &str, defined: &HashSet<String>) -> bool {
    let tokens = tokenize_condition(expr);
    let mut parser = ConditionParser::new(&tokens, defined);
    parser.parse_or_expr()
}

fn tokenize_condition(expr: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {
                chars.next();
            }
            '(' | ')' => {
                tokens.push(ch.to_string());
                chars.next();
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push("!=".to_string());
                } else {
                    tokens.push("!".to_string());
                }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push("&&".to_string());
                } else {
                    tokens.push("&".to_string());
                }
            }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push("||".to_string());
                } else {
                    tokens.push("|".to_string());
                }
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push("==".to_string());
                } else {
                    tokens.push("=".to_string());
                }
            }
            _ => {
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !ident.is_empty() {
                    tokens.push(ident);
                } else {
                    chars.next();
                }
            }
        }
    }

    tokens
}

struct ConditionParser<'a> {
    tokens: &'a [String],
    pos: usize,
    defined: &'a HashSet<String>,
}

impl<'a> ConditionParser<'a> {
    fn new(tokens: &'a [String], defined: &'a HashSet<String>) -> Self {
        Self {
            tokens,
            pos: 0,
            defined,
        }
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|s| s.as_str())
    }

    fn next_token(&mut self) -> Option<&'a str> {
        if self.pos < self.tokens.len() {
            let res = self.tokens[self.pos].as_str();
            self.pos += 1;
            Some(res)
        } else {
            None
        }
    }

    fn parse_or_expr(&mut self) -> bool {
        let mut left = self.parse_and_expr();
        while let Some(tok) = self.peek() {
            if tok == "||" {
                self.next_token();
                let right = self.parse_and_expr();
                left = left || right;
            } else {
                break;
            }
        }
        left
    }

    fn parse_and_expr(&mut self) -> bool {
        let mut left = self.parse_unary_expr();
        while let Some(tok) = self.peek() {
            if tok == "&&" {
                self.next_token();
                let right = self.parse_unary_expr();
                left = left && right;
            } else {
                break;
            }
        }
        left
    }

    fn parse_unary_expr(&mut self) -> bool {
        if let Some("!") = self.peek() {
            self.next_token();
            !self.parse_unary_expr()
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> bool {
        match self.next_token() {
            Some("(") => {
                let res = self.parse_or_expr();
                if let Some(")") = self.peek() {
                    self.next_token();
                }
                res
            }
            Some("true") => true,
            Some("false") => false,
            Some(ident) => self.defined.contains(ident),
            None => false,
        }
    }
}
