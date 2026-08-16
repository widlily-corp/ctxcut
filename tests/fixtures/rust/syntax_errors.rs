//! Rust syntax error fixture for compiler and tree-sitter error recovery tests.

/// Intact header function before any syntax faults.
pub fn valid_header_function(x: i32, y: i32) -> i32 {
    x + y
}

/// Function with unclosed blocks and missing braces.
pub fn broken_braces_fn(items: Vec<String>) -> Vec<String> {
    for item in items {
        if item.len() > 0 {
            println!("Item: {}", item;
    // Missing parenthesis, semicolon, and 2 closing braces

/// Intact target function embedded inside a syntactically invalid file.
pub fn target_intact_function<'a>(input: &'a str) -> &'a str {
    input.trim()
}

/// Unbalanced macro invocation.
macro_broken!(
    struct MalformedStruct {
        field_a: i32,
        invalid lifetime syntax '123_invalid,
