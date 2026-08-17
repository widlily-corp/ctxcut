//! AST symbol, module import, type hoisting, and signature stripper resolvers.

pub mod calls;
pub mod imports;
pub mod symbol;
pub mod types;

use crate::error::Result;
use crate::model::{CallSignatureStub, ExtractedType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tree_sitter::Tree;

pub use calls::{resolve_foreign_signature, SignatureStripper};
pub use imports::{
    has_go_files, normalize_path, resolve_go_specifier, resolve_python_specifier,
    resolve_rust_specifier, resolve_ts_js_specifier, ImportMapping, ImportResolver,
};
pub use symbol::SymbolLocator;
pub use types::{resolve_foreign_types, TypeHoister};

/// Abstract interface for cross-file symbol resolution, import mapping,
/// signature extraction, and type hoisting across language boundaries.
pub trait ForeignSymbolLocator: Send + Sync {
    /// Resolves an import module specifier from `current_file` to a concrete target file on disk.
    ///
    /// # Arguments
    /// * `current_file` - The file containing the import statement.
    /// * `import_spec` - The raw module specifier (e.g. `"./models"`, `"../utils/crypto"`, `"app.schemas"`).
    ///
    /// # Returns
    /// * `Some(PathBuf)` if resolved to an existing file on disk.
    /// * `None` if the specifier represents an external package or unresolvable file.
    fn resolve_import_path(&self, current_file: &Path, import_spec: &str) -> Option<PathBuf>;

    /// Locates a function or method signature in `target_file`, stripping its implementation body
    /// to return a 100% clean signature declaration stub.
    ///
    /// # Arguments
    /// * `target_file` - Path to the file containing the function/method definition.
    /// * `symbol_name` - Identifier of the function/method to locate.
    ///
    /// # Returns
    /// * `Ok(Some(CallSignatureStub))` if found and stripped.
    /// * `Ok(None)` if symbol does not exist in `target_file`.
    /// * `Err(CoreError)` on file I/O or syntax parse errors.
    fn locate_foreign_signature(
        &self,
        target_file: &Path,
        symbol_name: &str,
    ) -> Result<Option<CallSignatureStub>>;

    /// Hoists and extracts definitions for the specified types from `target_file`.
    ///
    /// # Arguments
    /// * `target_file` - Path to the file containing type definitions.
    /// * `type_names` - Slice of type identifiers to extract.
    ///
    /// # Returns
    /// * `Ok(Vec<ExtractedType>)` containing verbatim definitions of matching types.
    /// * `Err(CoreError)` on file I/O or syntax parse errors.
    fn hoist_foreign_types(
        &self,
        target_file: &Path,
        type_names: &[&str],
    ) -> Result<Vec<ExtractedType>>;
}

/// Thread-safe default foreign symbol locator with in-memory AST caching.
#[derive(Default)]
pub struct DefaultForeignSymbolLocator {
    /// In-memory cache: PathBuf -> (SourceContent, ParsedAstTree)
    file_cache: RwLock<HashMap<PathBuf, (String, Tree)>>,
}

impl DefaultForeignSymbolLocator {
    /// Creates a new, empty `DefaultForeignSymbolLocator`.
    pub fn new() -> Self {
        Self {
            file_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Clears the internal file and AST cache.
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.file_cache.write() {
            cache.clear();
        }
    }
}

impl ForeignSymbolLocator for DefaultForeignSymbolLocator {
    fn resolve_import_path(&self, current_file: &Path, import_spec: &str) -> Option<PathBuf> {
        ImportResolver::resolve_module_path(current_file, import_spec)
    }

    fn locate_foreign_signature(
        &self,
        target_file: &Path,
        symbol_name: &str,
    ) -> Result<Option<CallSignatureStub>> {
        resolve_foreign_signature(target_file, symbol_name)
    }

    fn hoist_foreign_types(
        &self,
        target_file: &Path,
        type_names: &[&str],
    ) -> Result<Vec<ExtractedType>> {
        resolve_foreign_types(target_file, type_names)
    }
}
