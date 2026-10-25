//! Level of Detail (LOD) architecture for context reduction and symbol slicing.
//!
//! Provides progressive levels of detail for AI coding agent contexts:
//! - [`LodLevel::Lod0`]: Signature only with doc-comments / XML-doc.
//! - [`LodLevel::Lod1`]: Focus target method body fully expanded with enclosing class skeleton.
//! - [`LodLevel::Lod2`]: Transitive slice expanding bodies of direct private helpers <= 15 LOC.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Level of Detail (LOD) specification for slicing and context extraction.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum LodLevel {
    /// LOD 0 (Signature): Only method signature and XML-doc / Doc-comments.
    #[serde(rename = "lod0", alias = "Lod0", alias = "signature", alias = "0")]
    Lod0,

    /// LOD 1 (Skeleton + Focus): Target method body is fully open verbatim,
    /// while parent class is presented as a skeleton:
    /// - All fields (especially `[SerializeField]`, `[Header]`, private/public) visible.
    /// - Signatures of sibling methods collapsed into `{ ... }`.
    /// - Comment `// [LOD 1: Collapsed context]` placed above collapsed methods.
    /// - Comment `// [TARGET SYMBOL: FULL BODY]` placed above the target method.
    #[default]
    #[serde(
        rename = "lod1",
        alias = "Lod1",
        alias = "skeleton",
        alias = "skeleton_focus",
        alias = "1"
    )]
    Lod1,

    /// LOD 2 (Transitive Slice): Target method + expanded bodies of its direct
    /// private helper methods in the same class if their body is <= 15 lines of code.
    #[serde(
        rename = "lod2",
        alias = "Lod2",
        alias = "transitive",
        alias = "transitive_slice",
        alias = "2"
    )]
    Lod2,
}

impl LodLevel {
    /// Returns the canonical string representation for this LOD level.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lod0 => "lod0",
            Self::Lod1 => "lod1",
            Self::Lod2 => "lod2",
        }
    }

    /// Returns `true` if this level only extracts the signature without implementation body.
    pub fn is_signature_only(&self) -> bool {
        matches!(self, Self::Lod0)
    }

    /// Returns `true` if this level generates a class skeleton around the target method.
    pub fn includes_skeleton(&self) -> bool {
        matches!(self, Self::Lod1 | Self::Lod2)
    }

    /// Returns `true` if this level expands direct private helper bodies (<= 15 LOC).
    pub fn includes_transitive_helpers(&self) -> bool {
        matches!(self, Self::Lod2)
    }
}

impl fmt::Display for LodLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for LodLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "lod0" | "0" | "signature" => Ok(Self::Lod0),
            "lod1" | "1" | "skeleton" | "skeleton_focus" => Ok(Self::Lod1),
            "lod2" | "2" | "transitive" | "transitive_slice" => Ok(Self::Lod2),
            other => Err(format!("Unknown LOD level: '{other}'")),
        }
    }
}

/// Slicing result produced by a Level of Detail extraction engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LodSliceResult {
    /// The requested LOD level.
    pub lod_level: LodLevel,
    /// The target symbol name (e.g. `TakeDamage`).
    pub target_symbol: String,
    /// Name of the enclosing class or container, if found.
    pub class_name: Option<String>,
    /// The rendered/generated LOD context string.
    pub content: String,
    /// Names of direct private helper methods expanded in LOD 2.
    pub expanded_helpers: Vec<String>,
    /// Names of sibling methods collapsed in the skeleton.
    pub collapsed_methods: Vec<String>,
}
