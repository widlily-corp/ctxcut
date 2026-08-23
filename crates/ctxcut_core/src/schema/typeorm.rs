//! TypeORM entity parser and schema stitcher.

use crate::model::ExtractedType;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Extracted TypeORM entity definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeOrmEntityDef {
    /// TypeScript class name (e.g. `Order`, `User`).
    pub class_name: String,
    /// Database table name if specified in `@Entity('orders')`, or derived from class name.
    pub table_name: String,
    /// Path to the defining entity file.
    pub file_path: PathBuf,
    /// Verbatim class definition snippet.
    pub definition: String,
}

/// Parsed TypeORM entities.
#[derive(Debug, Default, Clone)]
pub struct ParsedTypeOrmSchema {
    /// Entities indexed by class name and table name (lowercased).
    pub entities: HashMap<String, TypeOrmEntityDef>,
}

/// Stitcher for TypeORM entity schemas (`@Entity`).
#[derive(Debug, Default, Clone)]
pub struct TypeOrmStitcher;

impl TypeOrmStitcher {
    /// Creates a new `TypeOrmStitcher`.
    pub fn new() -> Self {
        Self
    }

    /// Discovers candidate TypeORM entity files in the workspace.
    pub fn discover_entity_files(&self, workspace_root: &Path, current_file: &Path) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Check current directory and sibling files
        if let Some(dir) = current_file.parent() {
            collect_typeorm_files(dir, &mut candidates, 1);
        }

        // 2. Check workspace standard entity directories
        let entity_dirs = [
            workspace_root.join("src").join("entities"),
            workspace_root.join("src").join("entity"),
            workspace_root.join("src").join("models"),
            workspace_root.join("entities"),
            workspace_root.join("entity"),
            workspace_root.join("models"),
        ];
        for dir in &entity_dirs {
            if dir.is_dir() {
                collect_typeorm_files(dir, &mut candidates, 3);
            }
        }

        // 3. Fallback: Scan workspace
        if candidates.is_empty() {
            collect_typeorm_files(workspace_root, &mut candidates, 4);
        }

        let mut seen = HashSet::new();
        candidates.retain(|p| seen.insert(p.clone()));

        // Sort by proximity to current_file
        let target_dir = current_file.parent().unwrap_or(workspace_root);
        candidates.sort_by_key(|p| {
            let common_prefix_len = p
                .parent()
                .map(|parent| common_path_prefix_len(target_dir, parent))
                .unwrap_or(0);
            std::cmp::Reverse(common_prefix_len)
        });

        candidates
    }

    /// Parses TypeORM entity class definitions from source text.
    pub fn parse_entities(&self, content: &str, file_path: &Path) -> ParsedTypeOrmSchema {
        let mut schema = ParsedTypeOrmSchema::default();

        let mut search_from = 0;
        while let Some(pos) = content[search_from..].find("@Entity") {
            let abs_pos = search_from + pos;
            let after_entity = &content[abs_pos + 7..];

            let table_name_opt = extract_entity_table_name(after_entity);

            if let Some(class_pos) = after_entity.find("class ") {
                let class_start = abs_pos + 7 + class_pos;
                let class_line_start = abs_pos;
                let after_class = &content[class_start + 6..];
                let class_name: String = after_class
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();

                if !class_name.is_empty() {
                    let end_pos = find_class_end(content, class_start);
                    let definition = content[class_line_start..end_pos].trim().to_string();

                    let table_name = table_name_opt.unwrap_or_else(|| class_name.to_lowercase());

                    let entity_def = TypeOrmEntityDef {
                        class_name: class_name.clone(),
                        table_name: table_name.clone(),
                        file_path: file_path.to_path_buf(),
                        definition,
                    };

                    schema.entities.insert(class_name.to_lowercase(), entity_def.clone());
                    schema.entities.insert(table_name.to_lowercase(), entity_def);
                }
            }

            search_from = abs_pos + 7;
        }

        schema
    }

    /// Detects referenced TypeORM entities in source code.
    pub fn detect_entity_references(&self, source: &str) -> HashSet<String> {
        let mut references = HashSet::new();

        let repo_patterns = [
            "getRepository(",
            "InjectRepository(",
            "Repository<",
            "getCustomRepository(",
        ];
        for pattern in repo_patterns {
            let mut search = 0;
            while let Some(pos) = source[search..].find(pattern) {
                let start = search + pos + pattern.len();
                let rest = &source[start..];
                let ident: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !ident.is_empty() {
                    references.insert(ident.to_lowercase());
                }
                search = start;
            }
        }

        references
    }

    /// Matches detected references against discovered TypeORM entities and returns `ExtractedType` definitions.
    pub fn stitch(
        &self,
        workspace_root: &Path,
        current_file: &Path,
        source: &str,
    ) -> Vec<ExtractedType> {
        let detected = self.detect_entity_references(source);
        if detected.is_empty() && !is_typeorm_context(source) {
            return Vec::new();
        }

        let entity_files = self.discover_entity_files(workspace_root, current_file);
        if entity_files.is_empty() {
            return Vec::new();
        }

        let mut extracted = Vec::new();
        let mut seen_types = HashSet::new();

        for entity_path in &entity_files {
            let content = match fs::read_to_string(entity_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let parsed = self.parse_entities(&content, entity_path);

            for entity_def in parsed.entities.values() {
                let matches_detected = detected.contains(&entity_def.class_name.to_lowercase())
                    || detected.contains(&entity_def.table_name.to_lowercase());
                let matches_exact_in_source = source.contains(&entity_def.class_name);

                if (matches_detected || (matches_exact_in_source && is_typeorm_context(source)))
                    && seen_types.insert(entity_def.class_name.clone())
                {
                    extracted.push(ExtractedType {
                        name: entity_def.class_name.clone(),
                        kind: "typeorm_entity".to_string(),
                        file_path: entity_def.file_path.to_string_lossy().to_string(),
                        definition: entity_def.definition.clone(),
                    });
                }
            }
        }

        extracted
    }
}

fn extract_entity_table_name(after_entity: &str) -> Option<String> {
    let trimmed = after_entity.trim_start();
    if !trimmed.starts_with('(') {
        return None;
    }
    let rest = &trimmed[1..];
    let close_paren = rest.find(')')?;
    let inner = rest[..close_paren].trim();
    let quote = inner.chars().next()?;
    if quote == '\'' || quote == '"' || quote == '`' {
        let after_quote = &inner[1..];
        let end_quote = after_quote.find(quote)?;
        Some(after_quote[..end_quote].to_string())
    } else {
        None
    }
}

fn find_class_end(source: &str, class_start: usize) -> usize {
    let mut brace_count = 0;
    let mut started = false;
    let bytes = source.as_bytes();
    let mut i = class_start;

    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                brace_count += 1;
                started = true;
            }
            b'}' => {
                brace_count -= 1;
                if started && brace_count == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    source.len()
}

fn is_typeorm_context(source: &str) -> bool {
    source.contains("typeorm")
        || source.contains("@Entity")
        || source.contains("getRepository")
        || source.contains("Repository<")
        || source.contains("DataSource")
        || source.contains("EntityManager")
}

fn collect_typeorm_files(dir: &Path, results: &mut Vec<PathBuf>, max_depth: usize) {
    if max_depth == 0 {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !matches!(
                    name,
                    "node_modules"
                        | "target"
                        | ".git"
                        | ".agents"
                        | ".ctxcut"
                        | "dist"
                        | "build"
                        | "vendor"
                        | ".cache"
                        | ".next"
                ) {
                    collect_typeorm_files(&p, results, max_depth - 1);
                }
            } else if p.is_file() {
                if let Some(ext) = p.extension() {
                    if ext == "ts" || ext == "js" {
                        if let Ok(content) = fs::read_to_string(&p) {
                            if content.contains("@Entity") {
                                results.push(p);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn common_path_prefix_len(p1: &Path, p2: &Path) -> usize {
    let c1: Vec<_> = p1.components().collect();
    let c2: Vec<_> = p2.components().collect();
    c1.iter().zip(c2.iter()).take_while(|(a, b)| a == b).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_typeorm_entities() {
        let content = r#"
import { Entity, PrimaryGeneratedColumn, Column, CreateDateColumn } from 'typeorm';

@Entity('orders')
export class Order {
    @PrimaryGeneratedColumn()
    id!: number;

    @Column('decimal')
    totalAmount!: number;

    @CreateDateColumn()
    createdAt!: Date;
}
"#;
        let stitcher = TypeOrmStitcher::new();
        let parsed = stitcher.parse_entities(content, Path::new("Order.ts"));

        assert!(parsed.entities.contains_key("order"));
        assert!(parsed.entities.contains_key("orders"));
        let order = parsed.entities.get("order").unwrap();
        assert_eq!(order.class_name, "Order");
        assert_eq!(order.table_name, "orders");
        assert!(order.definition.contains("@Entity('orders')"));
    }

    #[test]
    fn test_stitch_typeorm_repository() {
        let temp_dir = TempDir::new().unwrap();
        let entity_path = temp_dir.path().join("Order.ts");
        let content = r#"
@Entity('orders')
export class Order {
    id!: number;
}
"#;
        fs::write(&entity_path, content).unwrap();

        let source = "export function getRepo(ds: DataSource) { return ds.getRepository(Order); }";
        let stitcher = TypeOrmStitcher::new();
        let file_path = temp_dir.path().join("service.ts");

        let stitched = stitcher.stitch(temp_dir.path(), &file_path, source);
        assert_eq!(stitched.len(), 1);
        assert_eq!(stitched[0].name, "Order");
        assert_eq!(stitched[0].kind, "typeorm_entity");
    }
}
