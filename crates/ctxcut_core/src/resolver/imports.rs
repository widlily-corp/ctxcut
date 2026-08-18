//! Import and module resolver supporting TypeScript, JavaScript, Python, Go, and Rust ASTs.

use crate::model::SupportedLanguage;
use crate::parser::AstUtils;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Represents an imported symbol mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportMapping {
    /// Local identifier name as used in the file.
    pub local_name: String,
    /// Original exported name in the foreign module.
    pub imported_name: String,
    /// Raw module specifier (e.g. `./types`, `../utils/crypto`, `app.models`, `crate::models`).
    pub specifier: String,
}

/// Resolves module imports and finds candidate target files on disk across supported languages.
pub struct ImportResolver;

impl ImportResolver {
    /// Extracts all import mappings from a file's root AST.
    pub fn extract_imports(root: Node<'_>, source: &str) -> HashMap<String, ImportMapping> {
        let mut map = HashMap::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "import_statement" {
                let specifier = child
                    .child_by_field_name("source")
                    .map(|s| AstUtils::node_text(s, source).trim_matches(['\'', '"', '`']))
                    .or_else(|| {
                        AstUtils::find_descendants_by_kind(child, "string")
                            .first()
                            .map(|s| AstUtils::node_text(*s, source).trim_matches(['\'', '"', '`']))
                    })
                    .unwrap_or("");

                if specifier.is_empty() {
                    continue;
                }

                // Default import: import Foo from './foo'
                for clause in AstUtils::find_children_by_kind(child, "import_clause") {
                    if let Some(first_child) = clause.named_child(0) {
                        if first_child.kind() == "identifier" {
                            let name = AstUtils::node_text(first_child, source).to_string();
                            map.insert(
                                name.clone(),
                                ImportMapping {
                                    local_name: name,
                                    imported_name: "default".to_string(),
                                    specifier: specifier.to_string(),
                                },
                            );
                        }
                    }
                }

                // Named imports: import { A, B as C } from './foo'
                for named in AstUtils::find_descendants_by_kind(child, "import_specifier") {
                    let name_node = named
                        .child_by_field_name("name")
                        .or_else(|| named.named_child(0));
                    let alias_node = named.child_by_field_name("alias").or_else(|| {
                        if named.named_child_count() > 1 {
                            named.named_child(1)
                        } else {
                            None
                        }
                    });

                    if let Some(name_n) = name_node {
                        let orig_name = AstUtils::node_text(name_n, source).to_string();
                        let local_name = if let Some(alias_n) = alias_node {
                            AstUtils::node_text(alias_n, source).to_string()
                        } else {
                            orig_name.clone()
                        };

                        map.insert(
                            local_name.clone(),
                            ImportMapping {
                                local_name,
                                imported_name: orig_name,
                                specifier: specifier.to_string(),
                            },
                        );
                    }
                }

                // Namespace import: import * as Ns from './foo'
                for ns in AstUtils::find_descendants_by_kind(child, "namespace_import") {
                    if let Some(id) = ns.named_child(0) {
                        let ns_name = AstUtils::node_text(id, source).to_string();
                        map.insert(
                            ns_name.clone(),
                            ImportMapping {
                                local_name: ns_name,
                                imported_name: "*".to_string(),
                                specifier: specifier.to_string(),
                            },
                        );
                    }
                }
            } else if child.kind() == "lexical_declaration"
                || child.kind() == "variable_declaration"
            {
                // CommonJS require: const { foo } = require('./foo') or const bar = require('./bar')
                for declarator in AstUtils::find_children_by_kind(child, "variable_declarator") {
                    if let Some(val) = declarator.child_by_field_name("value") {
                        if val.kind() == "call_expression" {
                            if let Some(fn_node) = val.child_by_field_name("function") {
                                if AstUtils::node_text(fn_node, source) == "require" {
                                    if let Some(args) = val.child_by_field_name("arguments") {
                                        if let Some(first_arg) = args.named_child(0) {
                                            let specifier = AstUtils::node_text(first_arg, source)
                                                .trim_matches(['\'', '"', '`']);
                                            if !specifier.is_empty() {
                                                if let Some(name_node) =
                                                    declarator.child_by_field_name("name")
                                                {
                                                    if name_node.kind() == "object_pattern" {
                                                        for pattern_child in name_node
                                                            .named_children(&mut name_node.walk())
                                                        {
                                                            if pattern_child.kind() == "shorthand_property_identifier_pattern" || pattern_child.kind() == "identifier" {
                                                                let name = AstUtils::node_text(pattern_child, source).to_string();
                                                                map.insert(
                                                                    name.clone(),
                                                                    ImportMapping {
                                                                        local_name: name.clone(),
                                                                        imported_name: name,
                                                                        specifier: specifier.to_string(),
                                                                    },
                                                                );
                                                            } else if pattern_child.kind() == "pair_pattern" {
                                                                if let (Some(key), Some(val)) = (pattern_child.child_by_field_name("key"), pattern_child.child_by_field_name("value")) {
                                                                    let imported_name = AstUtils::node_text(key, source).to_string();
                                                                    let local_name = AstUtils::node_text(val, source).to_string();
                                                                    map.insert(
                                                                        local_name.clone(),
                                                                        ImportMapping {
                                                                            local_name,
                                                                            imported_name,
                                                                            specifier: specifier.to_string(),
                                                                        },
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    } else if name_node.kind() == "identifier" {
                                                        let name =
                                                            AstUtils::node_text(name_node, source)
                                                                .to_string();
                                                        map.insert(
                                                            name.clone(),
                                                            ImportMapping {
                                                                local_name: name,
                                                                imported_name: "default"
                                                                    .to_string(),
                                                                specifier: specifier.to_string(),
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        map
    }

    /// Resolves a module specifier to an existing file or directory path on disk across any supported language.
    pub fn resolve_module_path(from_file: &Path, specifier: &str) -> Option<PathBuf> {
        let lang = SupportedLanguage::from_path(from_file);
        match lang {
            Some(SupportedLanguage::TypeScript | SupportedLanguage::JavaScript) => {
                resolve_ts_js_specifier(from_file, specifier)
            }
            Some(SupportedLanguage::Python) => resolve_python_specifier(from_file, specifier),
            Some(SupportedLanguage::Go) => resolve_go_specifier(from_file, specifier),
            Some(SupportedLanguage::Rust) => resolve_rust_specifier(from_file, specifier),
            None => resolve_ts_js_specifier(from_file, specifier)
                .or_else(|| resolve_python_specifier(from_file, specifier))
                .or_else(|| resolve_go_specifier(from_file, specifier))
                .or_else(|| resolve_rust_specifier(from_file, specifier)),
        }
    }

    /// Extracts barrel re-exports from a file (e.g. `export * from './foo'`, `export { Bar } from './bar'`).
    pub fn extract_reexports(root: Node<'_>, source: &str) -> Vec<(Option<String>, String)> {
        let mut reexports = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "export_statement" {
                let specifier = child
                    .child_by_field_name("source")
                    .map(|s| AstUtils::node_text(s, source).trim_matches(['\'', '"', '`']))
                    .or_else(|| {
                        AstUtils::find_descendants_by_kind(child, "string")
                            .first()
                            .map(|s| AstUtils::node_text(*s, source).trim_matches(['\'', '"', '`']))
                    })
                    .unwrap_or("");

                if specifier.is_empty() {
                    continue;
                }

                // Check for wildcard `export * from './sub'`
                let has_star = child
                    .children(&mut child.walk())
                    .any(|c| c.kind() == "*" || c.kind() == "asterisk");
                let has_no_specs =
                    AstUtils::find_descendants_by_kind(child, "export_specifier").is_empty();
                if has_star || (child.child_by_field_name("declaration").is_none() && has_no_specs)
                {
                    reexports.push((None, specifier.to_string()));
                }

                // Check for named re-exports: `export { A, B as C } from './sub'`
                for spec in AstUtils::find_descendants_by_kind(child, "export_specifier") {
                    let name_node = spec
                        .child_by_field_name("name")
                        .or_else(|| spec.named_child(0));
                    let alias_node = spec.child_by_field_name("alias").or_else(|| {
                        if spec.named_child_count() > 1 {
                            spec.named_child(1)
                        } else {
                            None
                        }
                    });

                    if let Some(name_n) = name_node {
                        let orig_name = AstUtils::node_text(name_n, source).to_string();
                        let exported_name = if let Some(alias_n) = alias_node {
                            AstUtils::node_text(alias_n, source).to_string()
                        } else {
                            orig_name
                        };
                        reexports.push((Some(exported_name), specifier.to_string()));
                    }
                }
            }
        }

        reexports
    }
}

/// Resolves TypeScript and JavaScript module specifiers to target file paths on disk.
pub fn resolve_ts_js_specifier(from_file: &Path, specifier: &str) -> Option<PathBuf> {
    let parent_dir = from_file.parent().unwrap_or_else(|| Path::new("."));

    let raw_target = if specifier.starts_with("@/") || specifier.starts_with("~/") {
        let root = find_project_root(from_file)?;
        let remainder = &specifier[2..];
        let src_candidate = root.join("src").join(remainder);
        if src_candidate.exists() {
            src_candidate
        } else {
            root.join(remainder)
        }
    } else if specifier.starts_with('.')
        || specifier.starts_with('/')
        || specifier.starts_with('\\')
    {
        parent_dir.join(specifier)
    } else {
        return None;
    };

    let base_path = normalize_path(&raw_target);

    // 1. Direct path exists as file
    if base_path.is_file() {
        return Some(base_path);
    }

    // 2. Candidate file extensions
    let extensions = ["ts", "tsx", "d.ts", "js", "jsx", "mjs", "cjs"];

    // Handle ESM .js -> .ts/.tsx mapping
    let check_base = if let Some(stem) = base_path.to_str() {
        if stem.ends_with(".js") {
            PathBuf::from(stem.trim_end_matches(".js"))
        } else {
            base_path.clone()
        }
    } else {
        base_path.clone()
    };

    for ext in &extensions {
        let candidate = check_base.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
        let candidate_str = format!("{}.{}", check_base.display(), ext);
        let candidate_path = PathBuf::from(candidate_str);
        if candidate_path.is_file() {
            return Some(candidate_path);
        }
    }

    // 3. Directory index resolution
    let target_dir = if base_path.is_dir() {
        Some(&base_path)
    } else if check_base.is_dir() {
        Some(&check_base)
    } else {
        None
    };

    if let Some(dir) = target_dir {
        for ext in &extensions {
            let candidate = dir.join(format!("index.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Resolves Python module specifiers (relative dots and absolute module names) to target files on disk.
pub fn resolve_python_specifier(from_file: &Path, specifier: &str) -> Option<PathBuf> {
    let current_dir = from_file.parent().unwrap_or_else(|| Path::new("."));
    let trimmed = specifier.trim();
    if trimmed.is_empty() {
        return None;
    }

    let dots = trimmed.chars().take_while(|c| *c == '.').count();
    let mod_name = trimmed[dots..].trim();

    if dots > 0 {
        // Relative import: level 1 = ., level 2 = .., etc.
        let mut base = current_dir;
        for _ in 1..dots {
            base = base.parent()?;
        }

        if mod_name.is_empty() {
            return check_python_candidate(base);
        }

        let parts: Vec<&str> = mod_name.split('.').filter(|s| !s.is_empty()).collect();
        let mut p = base.to_path_buf();
        for part in parts {
            p.push(part);
        }
        check_python_candidate(&p)
    } else {
        // Absolute import
        let parts: Vec<&str> = trimmed.split('.').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return None;
        }

        // 1. Relative to current directory
        let mut rel = current_dir.to_path_buf();
        for part in &parts {
            rel.push(part);
        }
        if let Some(cand) = check_python_candidate(&rel) {
            return Some(cand);
        }

        // 2. Search ancestor directories as sys.path roots
        let mut curr = current_dir;
        while let Some(parent) = curr.parent() {
            let mut p = parent.to_path_buf();
            for part in &parts {
                p.push(part);
            }
            if let Some(cand) = check_python_candidate(&p) {
                return Some(cand);
            }

            for sub in &["src", "lib", "app", "backend"] {
                let sub_dir = parent.join(sub);
                if sub_dir.is_dir() {
                    let mut sp = sub_dir;
                    for part in &parts {
                        sp.push(part);
                    }
                    if let Some(cand) = check_python_candidate(&sp) {
                        return Some(cand);
                    }
                }
            }

            curr = parent;
        }

        None
    }
}

/// Checks candidate Python file variants (.py, .pyi, __init__.py, __init__.pyi).
fn check_python_candidate(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let py = path.with_extension("py");
    if py.is_file() {
        return Some(py);
    }
    let pyi = path.with_extension("pyi");
    if pyi.is_file() {
        return Some(pyi);
    }
    let init_py = path.join("__init__.py");
    if init_py.is_file() {
        return Some(init_py);
    }
    let init_pyi = path.join("__init__.pyi");
    if init_pyi.is_file() {
        return Some(init_pyi);
    }
    None
}

/// Resolves Go module and package specifiers to directories or files on disk.
pub fn resolve_go_specifier(from_file: &Path, specifier: &str) -> Option<PathBuf> {
    let current_dir = from_file.parent().unwrap_or_else(|| Path::new("."));
    let trimmed = specifier.trim_matches(['"', '\'']);

    // Direct single file in same directory
    if trimmed.ends_with(".go") {
        let direct = current_dir.join(trimmed);
        if direct.is_file() {
            return Some(direct);
        }
    }

    // 1. Relative subpackage
    if trimmed.starts_with("./") || trimmed.starts_with("../") {
        let target = normalize_path(&current_dir.join(trimmed));
        if target.is_dir() && has_go_files(&target) {
            return Some(target);
        }
        if target.is_file() {
            return Some(target);
        }
    }

    // 2. Module-rooted path using go.mod
    if let Some((go_mod_dir, module_name)) = find_go_module(from_file) {
        if trimmed == module_name {
            return Some(go_mod_dir);
        }
        if let Some(rel) = trimmed.strip_prefix(&module_name) {
            let rel_clean = rel.trim_start_matches(['/', '\\']);
            let target = go_mod_dir.join(rel_clean);
            if target.is_dir() && has_go_files(&target) {
                return Some(target);
            }
        }
    }

    // 3. Fallback: Search ancestor directories for matching package folder
    let segments: Vec<&str> = trimmed.split('/').collect();
    if let Some(last_seg) = segments.last() {
        let mut curr = current_dir;
        while let Some(parent) = curr.parent() {
            let cand = parent.join(last_seg);
            if cand.is_dir() && has_go_files(&cand) {
                return Some(cand);
            }
            curr = parent;
        }
    }

    // Sibling directory check if trimmed matches directory name
    let sibling_cand = current_dir.join(trimmed);
    if sibling_cand.is_dir() && has_go_files(&sibling_cand) {
        return Some(sibling_cand);
    }

    None
}

/// Checks if a directory contains any `.go` source files.
pub fn has_go_files(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("go") {
                return true;
            }
        }
    }
    false
}

/// Finds the nearest `go.mod` and extracts its module name.
fn find_go_module(start_path: &Path) -> Option<(PathBuf, String)> {
    let mut curr = if start_path.is_dir() {
        start_path
    } else {
        start_path.parent()?
    };

    loop {
        let go_mod = curr.join("go.mod");
        if go_mod.is_file() {
            if let Ok(content) = fs::read_to_string(&go_mod) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if let Some(mod_name) = trimmed.strip_prefix("module ") {
                        return Some((curr.to_path_buf(), mod_name.trim().to_string()));
                    }
                }
            }
            return Some((curr.to_path_buf(), String::new()));
        }
        curr = curr.parent()?;
    }
}

/// Resolves Rust module declarations (`mod foo;`) and `use crate::...` paths to target files.
pub fn resolve_rust_specifier(from_file: &Path, specifier: &str) -> Option<PathBuf> {
    let current_dir = from_file.parent().unwrap_or_else(|| Path::new("."));
    let file_stem = from_file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let clean_spec = specifier
        .trim()
        .trim_start_matches("pub ")
        .trim_start_matches("use ")
        .trim_end_matches(';')
        .trim();

    // 1. Direct mod declaration (e.g. `mod models;` or `models`)
    if !clean_spec.contains("::") {
        let mod_name = clean_spec;

        // A. Direct sibling foo.rs
        let sibling_rs = current_dir.join(format!("{mod_name}.rs"));
        if sibling_rs.is_file() {
            return Some(sibling_rs);
        }

        // B. Directory foo/mod.rs
        let dir_mod_rs = current_dir.join(mod_name).join("mod.rs");
        if dir_mod_rs.is_file() {
            return Some(dir_mod_rs);
        }

        // C. Nested module under file_stem/foo.rs (Rust 2018 edition)
        if file_stem != "lib" && file_stem != "main" && file_stem != "mod" {
            let nested_rs = current_dir.join(file_stem).join(format!("{mod_name}.rs"));
            if nested_rs.is_file() {
                return Some(nested_rs);
            }
            let nested_mod_rs = current_dir.join(file_stem).join(mod_name).join("mod.rs");
            if nested_mod_rs.is_file() {
                return Some(nested_mod_rs);
            }
        }
    }

    // 2. Crate-relative path `use crate::models::user;`
    if let Some(crate_path) = clean_spec.strip_prefix("crate::") {
        let crate_root = find_rust_crate_root(from_file)?;
        let segments: Vec<&str> = crate_path.split("::").collect();
        let mut curr_dir = crate_root;

        for (i, seg) in segments.iter().enumerate() {
            let is_last = i == segments.len() - 1;
            let file_cand = curr_dir.join(format!("{seg}.rs"));
            let mod_cand = curr_dir.join(seg).join("mod.rs");

            if file_cand.is_file() {
                if is_last {
                    return Some(file_cand);
                }
                curr_dir = curr_dir.join(seg);
            } else if mod_cand.is_file() {
                if is_last {
                    return Some(mod_cand);
                }
                curr_dir = curr_dir.join(seg);
            } else if curr_dir.join(seg).is_dir() {
                curr_dir = curr_dir.join(seg);
            } else if is_last {
                let parent_file = curr_dir.with_extension("rs");
                if parent_file.is_file() {
                    return Some(parent_file);
                }
                let parent_mod = curr_dir.join("mod.rs");
                if parent_mod.is_file() {
                    return Some(parent_mod);
                }
            }
        }
    }

    // 3. Super-relative path `use super::foo;`
    if let Some(super_path) = clean_spec.strip_prefix("super::") {
        if let Some(parent_dir) = current_dir.parent() {
            let segs: Vec<&str> = super_path.split("::").collect();
            if let Some(first) = segs.first() {
                let cand_file = parent_dir.join(format!("{first}.rs"));
                if cand_file.is_file() {
                    return Some(cand_file);
                }
                let cand_mod = parent_dir.join(first).join("mod.rs");
                if cand_mod.is_file() {
                    return Some(cand_mod);
                }
            }
        }
    }

    // 4. Sibling file fallback
    let sibling_file = current_dir.join(format!("{clean_spec}.rs"));
    if sibling_file.is_file() {
        return Some(sibling_file);
    }

    None
}

/// Finds the root source directory for a Rust crate (e.g. `src/` or directory containing `lib.rs` / `main.rs`).
fn find_rust_crate_root(from_file: &Path) -> Option<PathBuf> {
    let mut curr = from_file.parent()?;
    while let Some(parent) = curr.parent() {
        if curr.join("Cargo.toml").is_file() {
            let src = curr.join("src");
            if src.is_dir() {
                return Some(src);
            }
            return Some(curr.to_path_buf());
        }
        if curr.join("lib.rs").is_file() || curr.join("main.rs").is_file() {
            return Some(curr.to_path_buf());
        }
        curr = parent;
    }
    None
}

/// Finds the project root directory (containing `package.json`, `tsconfig.json`, `Cargo.toml`, or `.git`).
fn find_project_root(from_file: &Path) -> Option<PathBuf> {
    let mut curr = from_file.parent()?;
    loop {
        if curr.join("package.json").is_file()
            || curr.join("tsconfig.json").is_file()
            || curr.join("Cargo.toml").is_file()
            || curr.join(".git").is_dir()
        {
            return Some(curr.to_path_buf());
        }
        curr = curr.parent()?;
    }
}

/// Normalizes a path by removing redundant `.` and `..` segments.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => match components.last() {
                Some(std::path::Component::Normal(_)) => {
                    components.pop();
                }
                Some(std::path::Component::ParentDir) | None => {
                    components.push(comp);
                }
                _ => {}
            },
            c => components.push(c),
        }
    }
    components.into_iter().collect()
}
