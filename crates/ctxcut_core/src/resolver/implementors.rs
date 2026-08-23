//! Polyglot trait, interface, and protocol implementor discovery and hoisting.

use crate::error::Result;
use crate::lang::LanguageRegistry;
use crate::model::{ExtractedImplementor, ExtractedSymbol, ExtractedType, SupportedLanguage};
use crate::parser::ParserManager;
use crate::resolver::imports::ImportResolver;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Tree;

/// Engine for locating concrete implementors of traits, interfaces, and protocols across files and workspaces.
pub struct ImplementorHoister;

impl ImplementorHoister {
    /// Discovers implementors for all hoisted traits/interfaces within the current slice context.
    pub fn hoist_implementors_for_slice(
        _workspace_root: &Path,
        current_file: &Path,
        target_symbol: &ExtractedSymbol,
        hoisted_types: &[ExtractedType],
        lang: SupportedLanguage,
    ) -> Result<Vec<ExtractedImplementor>> {
        let mut trait_candidates = HashSet::new();

        // 1. Check if target symbol is a trait/interface
        if target_symbol.kind == "interface"
            || target_symbol.kind == "trait"
            || is_python_protocol(&target_symbol.body)
            || target_symbol.body.contains("interface ")
            || target_symbol.body.contains("interface{")
        {
            trait_candidates.insert(target_symbol.name.clone());
        }

        // 2. Check all hoisted types
        for ty in hoisted_types {
            if ty.kind == "interface"
                || ty.kind == "trait"
                || is_python_protocol(&ty.definition)
                || ty.definition.contains("interface ")
                || ty.definition.contains("interface{")
            {
                trait_candidates.insert(ty.name.clone());
            }
        }

        if trait_candidates.is_empty() {
            return Ok(Vec::new());
        }

        let mut discovered = Vec::new();
        let mut seen_keys = HashSet::new();
        let mut file_cache: HashMap<PathBuf, (String, Tree)> = HashMap::new();

        // 1. Check current file
        if let Ok(source) = fs::read_to_string(current_file) {
            if let Ok(adapter) = LanguageRegistry::for_language(lang) {
                let ts_lang = adapter.tree_sitter_language(current_file);
                if let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, current_file) {
                    for trait_name in &trait_candidates {
                        if let Ok(local) = adapter.find_implementors(
                            tree.root_node(),
                            &source,
                            trait_name,
                            current_file,
                        ) {
                            for imp in local {
                                if seen_keys
                                    .insert((imp.implementor_name.clone(), imp.file_path.clone()))
                                {
                                    discovered.push(imp);
                                }
                            }
                        }
                    }
                    file_cache.insert(current_file.to_path_buf(), (source, tree));
                }
            }
        }

        // 2. Scan sibling files in the same directory ONCE
        if let Some(parent_dir) = current_file.parent() {
            if let Ok(entries) = fs::read_dir(parent_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file()
                        && p != current_file
                        && SupportedLanguage::from_path(&p) == Some(lang)
                    {
                        if let Ok(src) = fs::read_to_string(&p) {
                            let is_duck_typed = lang == SupportedLanguage::Go;
                            let has_candidate = has_implementor_candidate_any(&src, lang, &trait_candidates);
                            if has_candidate {
                                if let Some((cached_src, tree)) =
                                    get_or_load_file(&p, lang, &mut file_cache)
                                {
                                    if let Ok(adapter) = LanguageRegistry::for_language(lang) {
                                        for trait_name in &trait_candidates {
                                            if is_duck_typed || cached_src.contains(trait_name) {
                                                if let Ok(sib) = adapter.find_implementors(
                                                    tree.root_node(),
                                                    cached_src,
                                                    trait_name,
                                                    &p,
                                                ) {
                                                    for imp in sib {
                                                        if seen_keys.insert((
                                                            imp.implementor_name.clone(),
                                                            imp.file_path.clone(),
                                                        )) {
                                                            discovered.push(imp);
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
        }

        // 3. Scan imported files in current file ONCE
        if let Some((source, tree)) = file_cache.get(current_file) {
            let imports = ImportResolver::extract_imports(tree.root_node(), source);
            for imp in imports.values() {
                if let Some(resolved_path) =
                    ImportResolver::resolve_module_path(current_file, &imp.specifier)
                {
                    if resolved_path != current_file
                        && !file_cache.contains_key(&resolved_path)
                        && SupportedLanguage::from_path(&resolved_path) == Some(lang)
                    {
                        if let Ok(src) = fs::read_to_string(&resolved_path) {
                            let is_duck_typed = lang == SupportedLanguage::Go;
                            let has_candidate = has_implementor_candidate_any(&src, lang, &trait_candidates);
                            if has_candidate {
                                if let Some((cached_src, tree)) =
                                    get_or_load_file(&resolved_path, lang, &mut file_cache)
                                {
                                    if let Ok(adapter) = LanguageRegistry::for_language(lang) {
                                        for trait_name in &trait_candidates {
                                            if is_duck_typed || cached_src.contains(trait_name) {
                                                if let Ok(imp_res) = adapter.find_implementors(
                                                    tree.root_node(),
                                                    cached_src,
                                                    trait_name,
                                                    &resolved_path,
                                                ) {
                                                    for imp in imp_res {
                                                        if seen_keys.insert((
                                                            imp.implementor_name.clone(),
                                                            imp.file_path.clone(),
                                                        )) {
                                                            discovered.push(imp);
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
        }

        Ok(discovered)
    }

    /// Finds all concrete implementors of a given trait/interface in local file, sibling files, and workspace.
    pub fn find_implementors(
        _workspace_root: &Path,
        current_file: &Path,
        interface_name: &str,
        lang: SupportedLanguage,
    ) -> Result<Vec<ExtractedImplementor>> {
        let mut results = Vec::new();
        let mut file_cache: HashMap<PathBuf, (String, Tree)> = HashMap::new();

        // A. Check current file
        if let Ok(source) = fs::read_to_string(current_file) {
            let adapter = LanguageRegistry::for_language(lang)?;
            let ts_lang = adapter.tree_sitter_language(current_file);
            if let Ok(tree) = ParserManager::parse_source(&source, &ts_lang, current_file) {
                let mut local = adapter.find_implementors(
                    tree.root_node(),
                    &source,
                    interface_name,
                    current_file,
                )?;
                results.append(&mut local);
                file_cache.insert(current_file.to_path_buf(), (source, tree));
            }
        }

        // B. Check sibling files in the same directory
        if let Some(parent_dir) = current_file.parent() {
            if let Ok(entries) = fs::read_dir(parent_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file()
                        && p != current_file
                        && SupportedLanguage::from_path(&p) == Some(lang)
                    {
                        if let Ok(src) = fs::read_to_string(&p) {
                            let has_candidate = has_implementor_candidate_single(&src, lang, interface_name);
                            if !has_candidate {
                                continue;
                            }
                            if let Some((src, tree)) = get_or_load_file(&p, lang, &mut file_cache) {
                                let adapter = LanguageRegistry::for_language(lang)?;
                                let mut sib = adapter.find_implementors(
                                    tree.root_node(),
                                    src,
                                    interface_name,
                                    &p,
                                )?;
                                results.append(&mut sib);
                            }
                        }
                    }
                }
            }
        }

        // C. Check imported files in current file
        if let Some((source, tree)) = file_cache.get(current_file) {
            let imports = ImportResolver::extract_imports(tree.root_node(), source);
            for imp in imports.values() {
                if let Some(resolved_path) =
                    ImportResolver::resolve_module_path(current_file, &imp.specifier)
                {
                    if resolved_path != current_file
                        && !file_cache.contains_key(&resolved_path)
                        && SupportedLanguage::from_path(&resolved_path) == Some(lang)
                    {
                        if let Ok(src) = fs::read_to_string(&resolved_path) {
                            let has_candidate = has_implementor_candidate_single(&src, lang, interface_name);
                            if has_candidate {
                                if let Some((src, tree)) =
                                    get_or_load_file(&resolved_path, lang, &mut file_cache)
                                {
                                    if let Ok(adapter) = LanguageRegistry::for_language(lang) {
                                        if let Ok(mut imp_res) = adapter.find_implementors(
                                            tree.root_node(),
                                            src,
                                            interface_name,
                                            &resolved_path,
                                        ) {
                                            results.append(&mut imp_res);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }
}

fn get_or_load_file<'a>(
    path: &Path,
    lang: SupportedLanguage,
    cache: &'a mut HashMap<PathBuf, (String, Tree)>,
) -> Option<(&'a str, &'a Tree)> {
    if !cache.contains_key(path) {
        let source = fs::read_to_string(path).ok()?;
        let adapter = LanguageRegistry::for_language(lang).ok()?;
        let ts_lang = adapter.tree_sitter_language(path);
        let tree = ParserManager::parse_source(&source, &ts_lang, path).ok()?;
        cache.insert(path.to_path_buf(), (source, tree));
    }
    cache.get(path).map(|(s, t)| (s.as_str(), t))
}

fn is_python_protocol(text: &str) -> bool {
    text.contains("Protocol") || text.contains("typing.Protocol")
}

fn has_implementor_candidate_any(src: &str, lang: SupportedLanguage, candidates: &HashSet<String>) -> bool {
    let is_duck_typed = lang == SupportedLanguage::Go;
    match lang {
        SupportedLanguage::Rust => {
            src.contains("impl") && src.contains("for") && candidates.iter().any(|t| src.contains(t))
        }
        SupportedLanguage::TypeScript
        | SupportedLanguage::JavaScript
        | SupportedLanguage::Vue
        | SupportedLanguage::Svelte
        | SupportedLanguage::Astro => {
            src.contains("implements") && candidates.iter().any(|t| src.contains(t))
        }
        SupportedLanguage::Python => {
            src.contains("class") && (candidates.iter().any(|t| src.contains(t)) || is_python_protocol(src))
        }
        SupportedLanguage::Go => {
            (src.contains("func (") || src.contains("func(")) && (candidates.iter().any(|t| src.contains(t)) || is_duck_typed)
        }
        SupportedLanguage::C => false,
        SupportedLanguage::Cpp => {
            (src.contains("class") || src.contains("struct")) && candidates.iter().any(|t| src.contains(t))
        }
        SupportedLanguage::CSharp => {
            (src.contains("class") || src.contains("record")) && candidates.iter().any(|t| src.contains(t))
        }
        SupportedLanguage::Java => {
            (src.contains("class") || src.contains("record")) && (src.contains("implements") || src.contains("extends")) && candidates.iter().any(|t| src.contains(t))
        }
        SupportedLanguage::Kotlin => {
            (src.contains("class") || src.contains("object")) && src.contains(':') && candidates.iter().any(|t| src.contains(t))
        }
    }
}

fn has_implementor_candidate_single(src: &str, lang: SupportedLanguage, target: &str) -> bool {
    let is_duck_typed = lang == SupportedLanguage::Go;
    match lang {
        SupportedLanguage::Rust => {
            src.contains("impl") && src.contains("for") && src.contains(target)
        }
        SupportedLanguage::TypeScript
        | SupportedLanguage::JavaScript
        | SupportedLanguage::Vue
        | SupportedLanguage::Svelte
        | SupportedLanguage::Astro => {
            src.contains("implements") && src.contains(target)
        }
        SupportedLanguage::Python => {
            src.contains("class") && (src.contains(target) || is_python_protocol(src))
        }
        SupportedLanguage::Go => {
            (src.contains("func (") || src.contains("func(")) && (src.contains(target) || is_duck_typed)
        }
        SupportedLanguage::C => false,
        SupportedLanguage::Cpp => {
            (src.contains("class") || src.contains("struct")) && src.contains(target)
        }
        SupportedLanguage::CSharp => {
            (src.contains("class") || src.contains("record")) && src.contains(target)
        }
        SupportedLanguage::Java => {
            (src.contains("class") || src.contains("record")) && (src.contains("implements") || src.contains("extends")) && src.contains(target)
        }
        SupportedLanguage::Kotlin => {
            (src.contains("class") || src.contains("object")) && src.contains(':') && src.contains(target)
        }
    }
}
