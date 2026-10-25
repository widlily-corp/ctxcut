//! Unity and GameDev C# static semantic analyzer.
//!
//! Provides zero GC allocation heuristics (0 GC Alloc) and C# event declaration / invocation tracking
//! for game performance optimization and LLM context slicing.

use crate::error::Result;
use crate::framework::FrameworkAnalyzer;
use crate::model::SliceResult;
use crate::parser::AstUtils;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::Node;

use super::semantic_bridge::{EventMetadata, GcAllocReport, SemanticMetadataHeader};

/// Known value types / structs that allocate on the stack rather than triggering GC heap allocations.
const KNOWN_STRUCT_NAMES: &[&str] = &[
    // C# built-in primitive value types
    "int",
    "int32",
    "uint",
    "uint32",
    "long",
    "int64",
    "ulong",
    "uint64",
    "short",
    "int16",
    "ushort",
    "uint16",
    "byte",
    "sbyte",
    "float",
    "single",
    "double",
    "decimal",
    "bool",
    "boolean",
    "char",
    "nint",
    "nuint",
    "half",
    "intptr",
    "uintptr",
    "guid",
    "datetime",
    "timespan",
    "datetimeoffset",
    "cancellationtoken",
    "valuetuple",
    "keyvaluepair",
    // Unity built-in structs
    "vector2",
    "vector3",
    "vector4",
    "vector2int",
    "vector3int",
    "quaternion",
    "matrix4x4",
    "color",
    "color32",
    "ray",
    "ray2d",
    "raycasthit",
    "raycasthit2d",
    "bounds",
    "boundsint",
    "rect",
    "rectint",
    "rectoffset",
    "plane",
    "pose",
    "layermask",
    "hash128",
    "touch",
    "keyframe",
    "nativearray",
    "nativeslice",
    "jobhandle",
];

/// Known LINQ method names that allocate enumerators/delegates on the GC heap.
const LINQ_METHODS: &[&str] = &[
    "Where",
    "Select",
    "ToList",
    "ToArray",
    "OrderBy",
    "OrderByDescending",
    "GroupBy",
    "Any",
    "Count",
    "FirstOrDefault",
    "LastOrDefault",
    "First",
    "Last",
    "SelectMany",
    "Distinct",
    "Aggregate",
    "All",
    "Sum",
    "Average",
    "Min",
    "Max",
    "ToDictionary",
];

/// Information regarding a detected C# event declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedEventDecl {
    /// Name of the declared event, e.g. `"OnHealthChanged"`.
    pub name: String,
    /// Delegate handler type, e.g. `"Action"`, `"Action<int>"`, `"EventHandler"`, `"UnityAction"`.
    pub handler_type: String,
    /// 1-based line number where the event is declared.
    pub line: usize,
}

/// Information regarding an invoked C# event within a method or snippet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedEventInvocation {
    /// Name of the event being invoked, e.g. `"OnHealthChanged"`.
    pub event_name: String,
    /// The exact invocation expression, e.g. `"OnHealthChanged?.Invoke()"`.
    pub expression: String,
    /// 1-based line number where the invocation occurred.
    pub line: usize,
}

/// Information regarding an event subscription, e.g. `combat.OnHealthChanged += HandleHealth;`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedEventSubscription {
    /// Name of the subscribed event.
    pub event_name: String,
    /// File or component name containing the subscriber.
    pub subscriber_file: String,
    /// 1-based line number of the subscription.
    pub line: usize,
}

/// Unity and C# game engine semantic analyzer.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnityAnalyzer;

impl UnityAnalyzer {
    /// Creates a new `UnityAnalyzer`.
    pub fn new() -> Self {
        Self
    }

    /// Performs zero GC allocation heuristic analysis on the given method source code.
    pub fn analyze_allocations(method_code: &str, method_name: Option<&str>) -> GcAllocReport {
        analyze_allocations(method_code, method_name)
    }

    /// Detects C# event declarations in source code.
    pub fn detect_event_declarations(source: &str) -> Vec<DetectedEventDecl> {
        detect_event_declarations(source)
    }

    /// Detects C# event invocations (e.g. `OnHealthChanged?.Invoke()`) in source code.
    pub fn detect_event_invocations(source: &str) -> Vec<DetectedEventInvocation> {
        detect_event_invocations(source)
    }

    /// Detects event subscriptions in source code.
    pub fn detect_event_subscribers(
        source: &str,
        file_name: &str,
    ) -> Vec<DetectedEventSubscription> {
        detect_event_subscribers(source, file_name)
    }

    /// Builds a `SemanticMetadataHeader` for a method based on its callers, events, and GC allocation status.
    pub fn build_semantic_header(
        method_name: Option<&str>,
        method_code: &str,
        callers: Vec<String>,
        known_subscribers: &[(String, String)],
    ) -> SemanticMetadataHeader {
        build_semantic_header(method_name, method_code, callers, known_subscribers)
    }
}

impl FrameworkAnalyzer for UnityAnalyzer {
    fn name(&self) -> &'static str {
        "unity"
    }

    fn matches_framework(&self, path: &Path, source: &str) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !ext.eq_ignore_ascii_case("cs") {
            return false;
        }

        source.contains("UnityEngine")
            || source.contains("MonoBehaviour")
            || source.contains("ScriptableObject")
            || source.contains("UnityAction")
            || source.contains("UnityEvent")
            || source.contains("Update()")
            || source.contains("FixedUpdate()")
            || source.contains("StartCoroutine(")
    }

    fn enhance_slice(
        &self,
        _target_node: Node<'_>,
        _source: &str,
        _path: &Path,
        slice: &mut SliceResult,
    ) -> Result<()> {
        let method_name = slice.target_symbol.name.as_str();
        let method_body = &slice.target_symbol.body;

        if slice.semantic_header.is_none() {
            let header = build_semantic_header(Some(method_name), method_body, Vec::new(), &[]);
            slice.semantic_header = Some(header);
        }
        Ok(())
    }
}

/// Checks if a type identifier corresponds to a primitive or known struct (value type).
fn is_known_struct(type_name: &str, custom_structs: &HashSet<String>) -> bool {
    let clean = type_name
        .trim()
        .trim_end_matches('?')
        .trim_end_matches("[]");

    let lower = clean.to_ascii_lowercase();

    if KNOWN_STRUCT_NAMES.contains(&lower.as_str()) {
        return true;
    }

    custom_structs.contains(clean) || custom_structs.contains(&lower)
}

/// Scans source code for any local `struct <Name>` definitions.
fn extract_local_struct_names(source: &str) -> HashSet<String> {
    let mut structs = HashSet::new();
    for line in source.lines() {
        let t = line.trim();
        if t.contains("struct ") {
            if let Some(pos) = t.find("struct ") {
                let after = &t[pos + 7..];
                let name = after
                    .split([' ', ':', '{', '<', ';', '(', '\r', '\n'])
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    structs.insert(name.to_string());
                    structs.insert(name.to_ascii_lowercase());
                }
            }
        }
    }
    structs
}

/// Checks whether a method is a per-frame or high-frequency game engine tick method.
fn is_frame_method(method_name: Option<&str>, method_code: &str) -> bool {
    if let Some(name) = method_name {
        let n = name.to_ascii_lowercase();
        if n == "update"
            || n == "fixedupdate"
            || n == "lateupdate"
            || n == "ongui"
            || n == "tick"
            || n.contains("update")
        {
            return true;
        }
    }

    let code_lower = method_code.to_ascii_lowercase();
    code_lower.contains("void update(")
        || code_lower.contains("void fixedupdate(")
        || code_lower.contains("void lateupdate(")
        || code_lower.contains("void ongui(")
}

/// Performs zero GC allocation heuristic analysis on the given method source code.
///
/// Detects:
/// 1. `new ` heap allocations (excluding known structs like `Vector3`, `Color`, `int`, etc.).
/// 2. LINQ invocations: `.Where(`, `.Select(`, `.ToList(`, `.ToArray(`, `.OrderBy(`, `.GroupBy(`, `.Any(`, `.Count(`.
/// 3. Boxing & string allocations (`+` concatenation, `$""` interpolation, or `string.Format(` in frame methods `Update`/`FixedUpdate`, and `(object)` casting).
/// 4. Coroutine allocations (`StartCoroutine`).
pub fn analyze_allocations(method_code: &str, method_name: Option<&str>) -> GcAllocReport {
    let mut potential_allocs = Vec::new();
    let mut seen = HashSet::new();

    let custom_structs = extract_local_struct_names(method_code);
    let frame_method = is_frame_method(method_name, method_code);

    // 1. Check for `new ` allocations using AST and fallback scanning
    scan_new_allocations(
        method_code,
        &custom_structs,
        &mut potential_allocs,
        &mut seen,
    );

    // 2. Check for LINQ method invocations
    scan_linq_allocations(method_code, &mut potential_allocs, &mut seen);

    // 3. Check for boxing and string allocations
    scan_boxing_and_strings(method_code, frame_method, &mut potential_allocs, &mut seen);

    // 4. Check for Coroutine allocations (StartCoroutine)
    scan_coroutine_allocations(method_code, &mut potential_allocs, &mut seen);

    GcAllocReport::with_allocs(potential_allocs)
}

/// Scans for `new ` expressions (heap classes, arrays, or anonymous/implicit allocations).
fn scan_new_allocations(
    source: &str,
    custom_structs: &HashSet<String>,
    potential_allocs: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    // Attempt Tree-sitter parsing if possible
    let ts_lang = tree_sitter_c_sharp::LANGUAGE.into();
    let wrapped = if !source.contains("class ") && !source.contains("struct ") {
        format!("class __CtxCutDummy__ {{ void __Dummy() {{\n{source}\n}} }}")
    } else {
        source.to_string()
    };

    let mut parsed_via_ast = false;
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_lang).is_ok() {
        if let Some(tree) = parser.parse(&wrapped, None) {
            parsed_via_ast = true;
            let root = tree.root_node();

            // Object creations: new HeavyClass(...)
            let object_creations =
                AstUtils::find_descendants_by_kind(root, "object_creation_expression");
            for node in object_creations {
                if let Some(type_node) = node.child_by_field_name("type") {
                    let type_text = AstUtils::node_text(type_node, &wrapped).trim();
                    let base_type = type_text
                        .split(['<', '(', '{', ' ', '['])
                        .next()
                        .unwrap_or(type_text)
                        .trim();

                    if !is_known_struct(base_type, custom_structs) {
                        let desc = format!("'new {base_type}()'");
                        if seen.insert(desc.clone()) {
                            potential_allocs.push(desc);
                        }
                    }
                }
            }

            // Array creations: new int[10], new Vector3[5] -> ALL arrays allocate on the heap!
            let array_creations =
                AstUtils::find_descendants_by_kind(root, "array_creation_expression");
            for node in array_creations {
                let desc = if let Some(type_node) = node.child_by_field_name("type") {
                    let t_text = AstUtils::node_text(type_node, &wrapped).trim();
                    let base_type = t_text.split('[').next().unwrap_or(t_text).trim();
                    format!("'new {base_type}[]'")
                } else {
                    let text = AstUtils::node_text(node, &wrapped).trim();
                    let first_line = text.lines().next().unwrap_or(text);
                    let clean = first_line.split('{').next().unwrap_or(first_line).trim();
                    let base_type = clean
                        .split('[')
                        .next()
                        .unwrap_or(clean)
                        .trim_start_matches("new")
                        .trim();
                    format!("'new {base_type}[]'")
                };
                if seen.insert(desc.clone()) {
                    potential_allocs.push(desc);
                }
            }

            // Implicit object creations: new()
            let implicit_creations =
                AstUtils::find_descendants_by_kind(root, "implicit_object_creation_expression");
            for _ in implicit_creations {
                let desc = "'new()'".to_string();
                if seen.insert(desc.clone()) {
                    potential_allocs.push(desc);
                }
            }
        }
    }

    // Fallback line-by-line scanner if AST didn't capture or for extra precision
    if !parsed_via_ast || potential_allocs.is_empty() {
        for line in source.lines() {
            let t = line.trim();
            if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') {
                continue;
            }

            let mut start_idx = 0;
            while let Some(pos) = t[start_idx..].find("new ") {
                let abs_pos = start_idx + pos;
                let after = &t[abs_pos + 4..].trim_start();

                // Check if it's an array creation: new int[...] or new Vector3[...]
                if let Some(bracket_pos) = after.find('[') {
                    let before_bracket = after[..bracket_pos].trim();
                    let valid_ident = before_bracket
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_');
                    if valid_ident && !before_bracket.is_empty() {
                        let desc = format!("'new {before_bracket}[]'");
                        if seen.insert(desc.clone()) {
                            potential_allocs.push(desc);
                        }
                        start_idx = abs_pos + 4;
                        continue;
                    }
                }

                // Regular object creation: new SomeType(...)
                let type_token = after
                    .split(['(', '<', '{', ';', ' ', '\t', '\r', '\n'])
                    .next()
                    .unwrap_or("")
                    .trim();

                if !type_token.is_empty() {
                    let clean_token = type_token.trim_end_matches('?');
                    if !is_known_struct(clean_token, custom_structs) && clean_token != "()" {
                        let desc = format!("'new {clean_token}()'");
                        if seen.insert(desc.clone()) {
                            potential_allocs.push(desc);
                        }
                    }
                }

                start_idx = abs_pos + 4;
            }
        }
    }
}

/// Scans for LINQ method invocations.
fn scan_linq_allocations(
    source: &str,
    potential_allocs: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for method in LINQ_METHODS {
        let pattern = format!(".{method}(");
        let mut idx = 0;
        let mut is_linq = false;
        while let Some(pos) = source[idx..].find(&pattern) {
            let abs_pos = idx + pos;
            let prefix = &source[..abs_pos];
            if (*method == "Max" || *method == "Min")
                && (prefix.ends_with("Math") || prefix.ends_with("Mathf"))
            {
                // Not LINQ, it's System.Math or UnityEngine.Mathf
            } else {
                is_linq = true;
                break;
            }
            idx = abs_pos + pattern.len();
        }
        if is_linq {
            let desc = format!("LINQ '.{method}()'");
            if seen.insert(desc.clone()) {
                potential_allocs.push(desc);
            }
        }
    }
}

/// Scans for boxing operations and per-frame string allocations.
fn scan_boxing_and_strings(
    source: &str,
    frame_method: bool,
    potential_allocs: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    // 1. Explicit boxing: (object)value or (System.Object)value
    if source.contains("(object)") || source.contains("(System.Object)") {
        let desc = "boxing '(object)'".to_string();
        if seen.insert(desc.clone()) {
            potential_allocs.push(desc);
        }
    }

    // 2. High-frequency per-frame string concatenation & interpolation in Update/FixedUpdate
    if frame_method {
        // String interpolation: $""
        if source.contains("$\"") {
            let desc = "string interpolation '$' in frame method".to_string();
            if seen.insert(desc.clone()) {
                potential_allocs.push(desc);
            }
        }

        // String concatenation with '+'
        let has_str_concat = source.lines().any(|line| {
            let t = line.trim();
            !t.starts_with("//") && (t.contains("\" +") || t.contains("+ \""))
        });

        if has_str_concat {
            let desc = "string concatenation '+' in frame method".to_string();
            if seen.insert(desc.clone()) {
                potential_allocs.push(desc);
            }
        }

        // string.Format(...)
        if source.contains("string.Format(") {
            let desc = "string formatting 'string.Format()' in frame method".to_string();
            if seen.insert(desc.clone()) {
                potential_allocs.push(desc);
            }
        }
    }
}

/// Scans for coroutine invocations (StartCoroutine).
fn scan_coroutine_allocations(
    source: &str,
    potential_allocs: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if source.contains("StartCoroutine(") {
        let desc = "coroutine 'StartCoroutine'".to_string();
        if seen.insert(desc.clone()) {
            potential_allocs.push(desc);
        }
    }
}

/// Detects C# event declarations in source code.
///
/// Matches:
/// - `event Action...`
/// - `event EventHandler...`
/// - `event UnityAction...`
/// - `event <CustomDelegate>...`
/// - `UnityEvent<T>...` / `UnityEvent...`
pub fn detect_event_declarations(source: &str) -> Vec<DetectedEventDecl> {
    let mut events = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let t = line.trim();

        if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') {
            continue;
        }

        // 1. C# event keyword: [modifiers] event <Type> <Name>;
        if let Some(pos) = t.find("event ") {
            let after = &t[pos + 6..].trim();
            let decl = after.trim_end_matches(';').trim();

            if let Some(last_space) = decl.rfind([' ', '\t']) {
                let handler_type = decl[..last_space].trim();
                let name = decl[last_space + 1..].trim();

                if !name.is_empty() && !handler_type.is_empty() {
                    events.push(DetectedEventDecl {
                        name: name.to_string(),
                        handler_type: handler_type.to_string(),
                        line: line_num,
                    });
                    continue;
                }
            }
        }

        // 2. UnityEvent declarations: public UnityEvent[<T>] <Name>;
        if t.contains("UnityEvent") && t.ends_with(';') {
            let clean = t.trim_end_matches(';').trim();
            let parts: Vec<&str> = clean.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[parts.len() - 1].trim();
                let ty = parts[parts.len() - 2].trim();
                if ty.starts_with("UnityEvent") {
                    events.push(DetectedEventDecl {
                        name: name.to_string(),
                        handler_type: ty.to_string(),
                        line: line_num,
                    });
                }
            }
        }
    }

    events
}

/// Detects C# event invocations in source code.
///
/// Matches:
/// - `OnHealthChanged?.Invoke()`
/// - `OnDamageTaken?.Invoke(10)`
/// - `OnClicked.Invoke()`
/// - `OnReset?.Invoke(this, EventArgs.Empty)`
pub fn detect_event_invocations(source: &str) -> Vec<DetectedEventInvocation> {
    let mut invocations = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let t = line.trim();

        if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') {
            continue;
        }

        // Find `?.Invoke(` or `.Invoke(`
        let mut search_idx = 0;
        while let Some(invoke_pos) = t[search_idx..].find("Invoke(") {
            let abs_pos = search_idx + invoke_pos;
            let before = &t[..abs_pos].trim_end();

            let (event_name, is_cond) = if let Some(stripped) = before.strip_suffix("?.") {
                let ev = stripped.trim_end();
                let name = ev
                    .split([' ', '\t', ';', '(', '{', '}', '=', '!'])
                    .next_back()
                    .unwrap_or("")
                    .trim();
                (name, true)
            } else if let Some(stripped) = before.strip_suffix('.') {
                let ev = stripped.trim_end();
                let name = ev
                    .split([' ', '\t', ';', '(', '{', '}', '=', '!'])
                    .next_back()
                    .unwrap_or("")
                    .trim();
                (name, false)
            } else {
                ("", false)
            };

            if !event_name.is_empty() {
                // Find closing parenthesis
                let after_paren = &t[abs_pos + 7..];
                let expr = if let Some(close_pos) = after_paren.find(')') {
                    let args = &after_paren[..close_pos];
                    if is_cond {
                        format!("{event_name}?.Invoke({args})")
                    } else {
                        format!("{event_name}.Invoke({args})")
                    }
                } else if is_cond {
                    format!("{event_name}?.Invoke()")
                } else {
                    format!("{event_name}.Invoke()")
                };

                invocations.push(DetectedEventInvocation {
                    event_name: event_name.to_string(),
                    expression: expr,
                    line: line_num,
                });
            }

            search_idx = abs_pos + 7;
        }
    }

    invocations
}

/// Detects event subscriptions in source code, e.g. `combat.OnHealthChanged += HandleHealth;`.
pub fn detect_event_subscribers(source: &str, file_name: &str) -> Vec<DetectedEventSubscription> {
    let mut subscribers = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let t = line.trim();

        if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') {
            continue;
        }

        // Event addition: `<event> += ...`
        if t.contains("+=") {
            let parts: Vec<&str> = t.split("+=").collect();
            if parts.len() >= 2 {
                let left = parts[0].trim();
                let event_name = left.split('.').next_back().unwrap_or(left).trim();

                if !event_name.is_empty()
                    && (event_name.starts_with("On")
                        || event_name.ends_with("Event")
                        || event_name.ends_with("Action"))
                {
                    subscribers.push(DetectedEventSubscription {
                        event_name: event_name.to_string(),
                        subscriber_file: file_name.to_string(),
                        line: line_num,
                    });
                }
            }
        }

        // UnityEvent AddListener: `<event>.AddListener(...)`
        if t.contains(".AddListener(") {
            if let Some(pos) = t.find(".AddListener(") {
                let left = t[..pos].trim();
                let event_name = left.split('.').next_back().unwrap_or(left).trim();
                if !event_name.is_empty() {
                    subscribers.push(DetectedEventSubscription {
                        event_name: event_name.to_string(),
                        subscriber_file: file_name.to_string(),
                        line: line_num,
                    });
                }
            }
        }
    }

    subscribers
}

/// Builds a `SemanticMetadataHeader` for a method based on its callers, emitted events, and GC allocation status.
pub fn build_semantic_header(
    method_name: Option<&str>,
    method_code: &str,
    callers: Vec<String>,
    known_subscribers: &[(String, String)],
) -> SemanticMetadataHeader {
    let gc_report = analyze_allocations(method_code, method_name);
    let invocations = detect_event_invocations(method_code);

    let mut events = Vec::new();
    let mut seen_events = HashSet::new();

    for inv in invocations {
        if seen_events.insert(inv.event_name.clone()) {
            let subs: Vec<String> = known_subscribers
                .iter()
                .filter(|(ev, _)| *ev == inv.event_name)
                .map(|(_, sub)| sub.clone())
                .collect();

            events.push(EventMetadata::new(inv.event_name, subs));
        }
    }

    SemanticMetadataHeader::new(callers, events, gc_report)
}
