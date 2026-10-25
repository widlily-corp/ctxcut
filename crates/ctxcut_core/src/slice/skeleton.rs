//! Class skeleton structures, comments, and formatting engine for context reduction.
//!
//! Provides structural representations of classes, methods, and fields,
//! along with formatting algorithms for collapsed vs. focused members.

use crate::slice::lod::{LodLevel, LodSliceResult};

/// Comment header placed above collapsed sibling methods in LOD skeletons.
pub const COLLAPSED_CONTEXT_COMMENT: &str = "// [LOD 1: Collapsed context]";

/// Comment header placed above the target symbol in LOD skeletons.
pub const TARGET_SYMBOL_COMMENT: &str = "// [TARGET SYMBOL: FULL BODY]";

/// Comment header placed above expanded transitive helpers in LOD 2 skeletons.
pub const TRANSITIVE_HELPER_COMMENT: &str = "// [LOD 2: Transitive helper]";

/// Maximum lines of code (inclusive) for a direct private helper to be expanded in LOD 2.
pub const MAX_HELPER_LINES: usize = 15;

/// Field or property member inside a class skeleton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonField {
    /// Name of the field identifier.
    pub name: String,
    /// Type text of the field.
    pub field_type: String,
    /// Full verbatim source text of field declaration (e.g. `[SerializeField] private int maxHealth = 100;`).
    pub full_text: String,
    /// Attribute list texts (e.g. `[SerializeField]`, `[Header("Stats")]`).
    pub attributes: Vec<String>,
    /// Modifiers (e.g. `private`, `public`, `protected`, `readonly`, `static`).
    pub modifiers: Vec<String>,
}

/// Method member inside a class skeleton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonMethod {
    /// Name of the method.
    pub name: String,
    /// Return type of the method (e.g. `void`, `int`, `Task<T>`).
    pub return_type: String,
    /// Collapsed signature (e.g. `private void Awake()`).
    pub signature: String,
    /// Full verbatim text of the method as written in the source code.
    pub full_text: String,
    /// Inner body text (e.g. `{ ... }`).
    pub body_text: String,
    /// Total line count of the method.
    pub line_count: usize,
    /// Line count of only the method body.
    pub body_line_count: usize,
    /// Whether the method is private (explicitly or by default without public/protected/internal).
    pub is_private: bool,
    /// Doc comment (XML-doc or standard comments) if present.
    pub doc_comment: Option<String>,
    /// Attributes attached to method (e.g. `[ContextMenu("...")]`).
    pub attributes: Vec<String>,
}

/// A member element of a class declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassMember {
    /// Field declaration (with attributes).
    Field(SkeletonField),
    /// Method declaration.
    Method(SkeletonMethod),
    /// Other member (e.g. property, constructor, event) preserved verbatim.
    Other(String),
}

/// Extracted class skeleton representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSkeleton {
    /// Name of the class (e.g. `PlayerHealth`).
    pub name: String,
    /// Class declaration header before body (e.g. `public class PlayerHealth : MonoBehaviour, IDamageable`).
    pub header: String,
    /// Doc comment for the class itself, if any.
    pub doc_comment: Option<String>,
    /// Class-level attributes (e.g. `[RequireComponent(typeof(Rigidbody))]`).
    pub attributes: Vec<String>,
    /// Ordered class members (fields, methods, etc.).
    pub members: Vec<ClassMember>,
}

impl ClassSkeleton {
    /// Returns an iterator over all fields in this skeleton.
    pub fn fields(&self) -> impl Iterator<Item = &SkeletonField> {
        self.members.iter().filter_map(|m| match m {
            ClassMember::Field(f) => Some(f),
            _ => None,
        })
    }

    /// Returns an iterator over all methods in this skeleton.
    pub fn methods(&self) -> impl Iterator<Item = &SkeletonMethod> {
        self.members.iter().filter_map(|m| match m {
            ClassMember::Method(m) => Some(m),
            _ => None,
        })
    }

    /// Renders the class skeleton according to the specified LOD level and target symbol.
    pub fn render(
        &self,
        target_symbol_name: &str,
        lod_level: LodLevel,
        expanded_helpers: &[String],
    ) -> String {
        let mut out = String::new();

        // Optional class doc comment
        if let Some(doc) = &self.doc_comment {
            for line in doc.lines() {
                out.push_str(line.trim());
                out.push('\n');
            }
        }

        // Optional class attributes
        for attr in &self.attributes {
            out.push_str(attr.trim());
            out.push('\n');
        }

        out.push_str(&self.header);
        out.push_str("\n{\n");

        let mut first = true;
        for member in &self.members {
            if !first {
                out.push('\n');
            }
            first = false;

            match member {
                ClassMember::Field(field) => {
                    for line in field.full_text.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            out.push_str("    ");
                            out.push_str(trimmed);
                            out.push('\n');
                        }
                    }
                }
                ClassMember::Method(method) => {
                    let is_target = method.name == target_symbol_name;
                    let is_expanded_helper = lod_level == LodLevel::Lod2
                        && expanded_helpers.iter().any(|h| h == &method.name);

                    if is_target {
                        out.push_str("    ");
                        out.push_str(TARGET_SYMBOL_COMMENT);
                        out.push('\n');

                        if let Some(doc) = &method.doc_comment {
                            for d_line in doc.lines() {
                                out.push_str("    ");
                                out.push_str(d_line.trim());
                                out.push('\n');
                            }
                        }

                        let reindented = reindent_lines(&method.full_text, "    ");
                        out.push_str(&reindented);
                        if !reindented.ends_with('\n') {
                            out.push('\n');
                        }
                    } else if is_expanded_helper {
                        out.push_str("    ");
                        out.push_str(TRANSITIVE_HELPER_COMMENT);
                        out.push('\n');

                        if let Some(doc) = &method.doc_comment {
                            for d_line in doc.lines() {
                                out.push_str("    ");
                                out.push_str(d_line.trim());
                                out.push('\n');
                            }
                        }

                        let reindented = reindent_lines(&method.full_text, "    ");
                        out.push_str(&reindented);
                        if !reindented.ends_with('\n') {
                            out.push('\n');
                        }
                    } else {
                        out.push_str("    ");
                        out.push_str(COLLAPSED_CONTEXT_COMMENT);
                        out.push('\n');
                        out.push_str("    ");
                        out.push_str(&method.signature);
                        out.push_str(" { ... }\n");
                    }
                }
                ClassMember::Other(raw) => {
                    let reindented = reindent_lines(raw, "    ");
                    out.push_str(&reindented);
                    if !reindented.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
        }

        out.push('}');
        out
    }
}

/// Helper function to normalize indentation of a multi-line block of code.
pub fn reindent_lines(text: &str, indent_prefix: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    // Determine common leading whitespace across non-empty lines
    let min_indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|c| *c == ' ' || *c == '\t').count())
        .min()
        .unwrap_or(0);

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        if line.trim().is_empty() {
            // Keep empty lines clean without trailing spaces
            continue;
        }
        let stripped = if line.len() >= min_indent {
            &line[min_indent..]
        } else {
            line.trim_start()
        };
        result.push_str(indent_prefix);
        result.push_str(stripped);
    }
    result
}

/// Generic skeleton engine trait for polyglot skeleton extraction.
pub trait SkeletonEngine: Send + Sync {
    /// Generates a skeleton slice result for the given source and symbol query.
    fn generate_slice(
        &self,
        source: &str,
        symbol_query: &str,
        lod: LodLevel,
    ) -> crate::error::Result<LodSliceResult>;
}
