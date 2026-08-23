//! Built-in AST query preset catalog mapping 9 presets across 13 programming languages.

use crate::model::SupportedLanguage;

/// Registry of built-in Tree-sitter AST S-expression query presets.
pub struct PresetRegistry;

impl PresetRegistry {
    /// Resolves a preset query S-expression pattern for the specified language.
    pub fn get_query(preset_name: &str, language: SupportedLanguage) -> Option<&'static str> {
        let normalized = preset_name
            .to_lowercase()
            .replace('-', "_")
            .trim()
            .to_string();

        match normalized.as_str() {
            "functions" | "function" | "fn" => Self::functions_query(language),
            "structs" | "struct" => Self::structs_query(language),
            "classes" | "class" => Self::classes_query(language),
            "interfaces" | "interface" | "traits" | "trait" => Self::interfaces_query(language),
            "enums" | "enum" => Self::enums_query(language),
            "exports" | "export" => Self::exports_query(language),
            "async_fns" | "async_functions" | "async_fn" | "async" => Self::async_fns_query(language),
            "api_routes" | "routes" | "route" | "endpoints" => Self::api_routes_query(language),
            "errors" | "exceptions" | "error" | "exception" => Self::errors_query(language),
            "react_hooks" | "hooks" | "hook" => Self::react_hooks_query(language),
            _ => None,
        }
    }

    fn functions_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some("(function_item name: (identifier) @name) @definition"),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some(
                r#"
                (function_declaration name: (identifier) @name) @definition
                (method_definition name: (property_identifier) @name) @definition
                (variable_declarator name: (identifier) @name value: (arrow_function)) @definition
                "#,
            ),
            SupportedLanguage::Python => Some("(function_definition name: (identifier) @name) @definition"),
            SupportedLanguage::Go => Some(
                r#"
                (function_declaration name: (identifier) @name) @definition
                (method_declaration name: (field_identifier) @name) @definition
                "#,
            ),
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                Some("(function_definition declarator: (function_declarator declarator: (identifier) @name)) @definition")
            }
            SupportedLanguage::CSharp => {
                Some("(method_declaration name: (identifier) @name) @definition")
            }
            SupportedLanguage::Java => {
                Some("(method_declaration name: (identifier) @name) @definition")
            }
            SupportedLanguage::Kotlin => {
                Some("(function_declaration name: (simple_identifier) @name) @definition")
            }
        }
    }

    fn structs_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some("(struct_item name: (type_identifier) @name) @definition"),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some(
                r#"
                (interface_declaration name: (type_identifier) @name) @definition
                (type_alias_declaration name: (type_identifier) @name) @definition
                "#,
            ),
            SupportedLanguage::Python => Some("(class_definition name: (identifier) @name) @definition"),
            SupportedLanguage::Go => Some("(type_spec name: (type_identifier) @name type: (struct_type)) @definition"),
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                Some("(struct_specifier name: (type_identifier) @name) @definition")
            }
            SupportedLanguage::CSharp => Some(
                r#"
                (struct_declaration name: (identifier) @name) @definition
                (record_declaration name: (identifier) @name) @definition
                "#,
            ),
            SupportedLanguage::Java => Some("(record_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Kotlin => Some("(class_declaration (type_identifier) @name) @definition"),
        }
    }

    fn classes_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some("(struct_item name: (type_identifier) @name) @definition"),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some(
                r#"
                (class_declaration name: (type_identifier) @name) @definition
                "#,
            ),
            SupportedLanguage::Python => Some("(class_definition name: (identifier) @name) @definition"),
            SupportedLanguage::Go => Some("(type_spec name: (type_identifier) @name type: (struct_type)) @definition"),
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                Some("(class_specifier name: (type_identifier) @name) @definition")
            }
            SupportedLanguage::CSharp => Some("(class_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Java => Some("(class_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Kotlin => Some("(class_declaration (type_identifier) @name) @definition"),
        }
    }

    fn interfaces_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some("(trait_item name: (type_identifier) @name) @definition"),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some("(interface_declaration name: (type_identifier) @name) @definition"),
            SupportedLanguage::Python => Some("(class_definition name: (identifier) @name) @definition"),
            SupportedLanguage::Go => Some("(type_spec name: (type_identifier) @name type: (interface_type)) @definition"),
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                Some("(class_specifier name: (type_identifier) @name) @definition")
            }
            SupportedLanguage::CSharp => Some("(interface_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Java => Some("(interface_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Kotlin => Some("(class_declaration (type_identifier) @name) @definition"),
        }
    }

    fn enums_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some("(enum_item name: (type_identifier) @name) @definition"),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some("(enum_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Python => Some("(class_definition name: (identifier) @name) @definition"),
            SupportedLanguage::Go => Some("(const_declaration (const_spec name: (identifier) @name)) @definition"),
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                Some("(enum_specifier name: (type_identifier) @name) @definition")
            }
            SupportedLanguage::CSharp => Some("(enum_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Java => Some("(enum_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Kotlin => Some("(class_declaration (type_identifier) @name) @definition"),
        }
    }

    fn exports_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some(
                r#"
                (function_item (visibility_modifier) name: (identifier) @name) @definition
                (struct_item (visibility_modifier) name: (type_identifier) @name) @definition
                (enum_item (visibility_modifier) name: (type_identifier) @name) @definition
                (trait_item (visibility_modifier) name: (type_identifier) @name) @definition
                "#,
            ),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some(
                r#"
                (export_statement declaration: (function_declaration name: (identifier) @name)) @definition
                (export_statement declaration: (class_declaration name: (type_identifier) @name)) @definition
                (export_statement declaration: (interface_declaration name: (type_identifier) @name)) @definition
                (export_statement declaration: (type_alias_declaration name: (type_identifier) @name)) @definition
                (export_statement declaration: (enum_declaration name: (identifier) @name)) @definition
                "#,
            ),
            SupportedLanguage::Python => Some("(function_definition name: (identifier) @name) @definition"),
            SupportedLanguage::Go => Some(
                r#"
                (function_declaration name: (identifier) @name) @definition
                (method_declaration name: (field_identifier) @name) @definition
                "#,
            ),
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                Some("(function_definition declarator: (function_declarator declarator: (identifier) @name)) @definition")
            }
            SupportedLanguage::CSharp => Some("(method_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Java => Some("(method_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Kotlin => Some("(function_declaration name: (simple_identifier) @name) @definition"),
        }
    }

    fn async_fns_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some("(function_item (function_modifiers) name: (identifier) @name) @definition"),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some(
                r#"
                (function_declaration name: (identifier) @name) @definition
                (method_definition name: (property_identifier) @name) @definition
                (variable_declarator name: (identifier) @name value: (arrow_function)) @definition
                "#,
            ),
            SupportedLanguage::Python => Some("(function_definition name: (identifier) @name) @definition"),
            SupportedLanguage::Go => Some("(go_statement) @definition"),
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                Some("(function_definition declarator: (function_declarator declarator: (identifier) @name)) @definition")
            }
            SupportedLanguage::CSharp => Some("(method_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Java => Some("(method_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Kotlin => Some("(function_declaration name: (simple_identifier) @name) @definition"),
        }
    }

    fn api_routes_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some("(attribute_item (attribute (identifier) @name)) @definition"),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some(
                "(call_expression function: (member_expression property: (property_identifier) @method) arguments: (arguments (string) @path)) @definition",
            ),
            SupportedLanguage::Python => Some("(decorated_definition (decorator (call function: (attribute attribute: (identifier) @method) arguments: (argument_list (string) @path))) definition: (function_definition name: (identifier) @name)) @definition"),
            SupportedLanguage::Go => Some("(call_expression function: (selector_expression field: (field_identifier) @method) arguments: (argument_list (interpreted_string_literal) @path)) @definition"),
            SupportedLanguage::C | SupportedLanguage::Cpp => Some("(call_expression function: (field_expression field: (field_identifier) @method)) @definition"),
            SupportedLanguage::CSharp => Some("(attribute name: (identifier) @name) @definition"),
            SupportedLanguage::Java => Some("(annotation name: (identifier) @method) @definition"),
            SupportedLanguage::Kotlin => Some("(call_expression (simple_identifier) @method) @definition"),
        }
    }

    fn errors_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::Rust => Some(
                r#"
                (enum_item name: (type_identifier) @name) @definition
                (struct_item name: (type_identifier) @name) @definition
                "#,
            ),
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some("(class_declaration name: (type_identifier) @name) @definition"),
            SupportedLanguage::Python => Some("(class_definition name: (identifier) @name) @definition"),
            SupportedLanguage::Go => Some("(type_spec name: (type_identifier) @name) @definition"),
            SupportedLanguage::C | SupportedLanguage::Cpp => Some("(class_specifier name: (type_identifier) @name) @definition"),
            SupportedLanguage::CSharp => Some("(class_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Java => Some("(class_declaration name: (identifier) @name) @definition"),
            SupportedLanguage::Kotlin => Some("(class_declaration (type_identifier) @name) @definition"),
        }
    }

    fn react_hooks_query(lang: SupportedLanguage) -> Option<&'static str> {
        match lang {
            SupportedLanguage::TypeScript
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Vue
            | SupportedLanguage::Svelte
            | SupportedLanguage::Astro => Some(
                r#"
                (call_expression function: (identifier) @name) @definition
                "#,
            ),
            _ => None,
        }
    }
}
