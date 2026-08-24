//! Drizzle ORM schema parser and table stitcher.

use crate::model::ExtractedType;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Extracted Drizzle table definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrizzleTableDef {
    /// TypeScript variable name (e.g. `users`, `carts`).
    pub variable_name: String,
    /// Database table name (e.g. `users`, `user_accounts`).
    pub table_name: String,
    /// Path to the defining schema file.
    pub file_path: PathBuf,
    /// Verbatim table definition snippet.
    pub definition: String,
}

/// Parsed Drizzle schema definitions.
#[derive(Debug, Default, Clone)]
pub struct ParsedDrizzleSchema {
    /// Tables indexed by variable name and table name (lowercased).
    pub tables: HashMap<String, DrizzleTableDef>,
}

/// Stitcher for Drizzle ORM table schemas (`pgTable`, `mysqlTable`, `sqliteTable`).
#[derive(Debug, Default, Clone)]
pub struct DrizzleStitcher;

impl DrizzleStitcher {
    /// Creates a new `DrizzleStitcher`.
    pub fn new() -> Self {
        Self
    }

    /// Discovers candidate Drizzle schema files in the workspace.
    pub fn discover_schema_files(
        &self,
        workspace_root: &Path,
        current_file: &Path,
    ) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Ascend from current_file parent up to workspace_root
        let mut curr = current_file.parent();
        while let Some(dir) = curr {
            let standard_paths = [
                dir.join("schema.ts"),
                dir.join("schema.js"),
                dir.join("db").join("schema.ts"),
                dir.join("db").join("schema.js"),
                dir.join("server").join("db").join("schema.ts"),
                dir.join("src").join("db").join("schema.ts"),
                dir.join("src").join("schema.ts"),
            ];
            for p in standard_paths {
                if p.is_file() {
                    candidates.push(p);
                }
            }

            if dir == workspace_root {
                break;
            }
            curr = dir.parent();
        }

        // 2. Also check workspace_root standard paths
        let root_paths = [
            workspace_root.join("schema.ts"),
            workspace_root.join("db").join("schema.ts"),
            workspace_root.join("server").join("db").join("schema.ts"),
            workspace_root.join("src").join("db").join("schema.ts"),
            workspace_root.join("src").join("schema.ts"),
            workspace_root.join("drizzle").join("schema.ts"),
        ];
        for p in root_paths {
            if p.is_file() {
                candidates.push(p);
            }
        }

        // 3. Fallback: Scan for any file containing drizzle table definitions if none found
        if candidates.is_empty() {
            collect_drizzle_files(workspace_root, &mut candidates);
        }

        let mut seen = HashSet::new();
        candidates.retain(|p| seen.insert(p.clone()));

        // Sort by proximity
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

    /// Parses Drizzle table definitions from a source file.
    pub fn parse_schema(&self, content: &str, file_path: &Path) -> ParsedDrizzleSchema {
        let mut schema = ParsedDrizzleSchema::default();

        let table_creators = ["pgTable", "mysqlTable", "sqliteTable"];
        let mut search_from = 0;

        while search_from < content.len() {
            let mut earliest_pos = None;
            let mut matched_creator = "";

            for creator in &table_creators {
                if let Some(pos) = content[search_from..].find(creator) {
                    let abs_pos = search_from + pos;
                    if earliest_pos.map_or(true, |p| abs_pos < p) {
                        earliest_pos = Some(abs_pos);
                        matched_creator = creator;
                    }
                }
            }

            let pos = match earliest_pos {
                Some(p) => p,
                None => break,
            };

            let line_start = content[..pos].rfind('\n').map_or(0, |p| p + 1);
            let before_creator = &content[line_start..pos];

            let var_name = extract_variable_name(before_creator);

            let creator_end = pos + matched_creator.len();
            let after_creator = &content[creator_end..];

            if let Some(open_paren) = after_creator.find('(') {
                let args_str = &after_creator[open_paren + 1..];
                let table_name = extract_string_arg(args_str);

                let start_byte = line_start;
                let end_byte = find_statement_end(content, pos);

                let definition = content[start_byte..end_byte].trim().to_string();

                if let (Some(var), Some(tbl)) = (var_name, table_name) {
                    let table_def = DrizzleTableDef {
                        variable_name: var.clone(),
                        table_name: tbl.clone(),
                        file_path: file_path.to_path_buf(),
                        definition,
                    };
                    schema.tables.insert(var.to_lowercase(), table_def.clone());
                    schema.tables.insert(tbl.to_lowercase(), table_def);
                }
            }

            search_from = pos + matched_creator.len() + 1;
        }

        schema
    }

    /// Detects referenced Drizzle table variables in source code.
    pub fn detect_table_references(&self, source: &str) -> HashSet<String> {
        let mut references = HashSet::new();

        // 1. `.from(table)`
        let mut search = 0;
        while let Some(pos) = source[search..].find(".from(") {
            let start = search + pos + 6;
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

        // 2. `db.insert(table)`, `db.update(table)`, `db.delete(table)`
        let methods = [".insert(", ".update(", ".delete(", "db.query."];
        for method in methods {
            let mut search_m = 0;
            while let Some(pos) = source[search_m..].find(method) {
                let start = search_m + pos + method.len();
                let rest = &source[start..];
                let ident: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !ident.is_empty() {
                    references.insert(ident.to_lowercase());
                }
                search_m = start;
            }
        }

        references
    }

    /// Matches detected references against discovered Drizzle schemas and returns `ExtractedType` definitions.
    pub fn stitch(
        &self,
        workspace_root: &Path,
        current_file: &Path,
        source: &str,
    ) -> Vec<ExtractedType> {
        let detected = self.detect_table_references(source);
        if detected.is_empty() && !is_drizzle_context(source) {
            return Vec::new();
        }

        let schema_files = self.discover_schema_files(workspace_root, current_file);
        if schema_files.is_empty() {
            return Vec::new();
        }

        let mut extracted = Vec::new();
        let mut seen_types = HashSet::new();

        for schema_path in &schema_files {
            let content = match fs::read_to_string(schema_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let parsed = self.parse_schema(&content, schema_path);

            for table_def in parsed.tables.values() {
                let matches_detected = detected.contains(&table_def.variable_name.to_lowercase())
                    || detected.contains(&table_def.table_name.to_lowercase());
                let matches_exact_in_source = source.contains(&table_def.variable_name)
                    || source.contains(&table_def.table_name);

                if (matches_detected || (matches_exact_in_source && is_drizzle_context(source)))
                    && seen_types.insert(table_def.variable_name.clone())
                {
                    extracted.push(ExtractedType {
                        name: table_def.variable_name.clone(),
                        kind: "drizzle_table".to_string(),
                        file_path: table_def.file_path.to_string_lossy().to_string(),
                        definition: table_def.definition.clone(),
                    });
                }
            }
        }

        extracted
    }
}

fn extract_variable_name(before: &str) -> Option<String> {
    let equals_idx = before.rfind('=')?;
    let left = before[..equals_idx].trim();
    let ident: String = left
        .split_whitespace()
        .last()?
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if ident.is_empty() {
        None
    } else {
        Some(ident)
    }
}

fn extract_string_arg(args_str: &str) -> Option<String> {
    let trimmed = args_str.trim_start();
    let quote_char = trimmed.chars().next()?;
    if quote_char != '\'' && quote_char != '"' && quote_char != '`' {
        return None;
    }
    let rest = &trimmed[1..];
    let end_quote = rest.find(quote_char)?;
    Some(rest[..end_quote].to_string())
}

fn find_statement_end(source: &str, start_pos: usize) -> usize {
    let mut paren_count = 0;
    let mut brace_count = 0;
    let mut in_quote = None;
    let bytes = source.as_bytes();
    let mut i = start_pos;

    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_quote {
            if b == q && (i == 0 || bytes[i - 1] != b'\\') {
                in_quote = None;
            }
        } else {
            match b {
                b'\'' | b'"' | b'`' => in_quote = Some(b),
                b'(' => paren_count += 1,
                b')' => {
                    paren_count -= 1;
                    if paren_count == 0 && brace_count == 0 {
                        let mut j = i + 1;
                        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                            j += 1;
                        }
                        if j < bytes.len() && bytes[j] == b';' {
                            return j + 1;
                        }
                        return i + 1;
                    }
                }
                b'{' => brace_count += 1,
                b'}' => {
                    brace_count -= 1;
                    if paren_count == 0 && brace_count == 0 {
                        let mut j = i + 1;
                        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                            j += 1;
                        }
                        if j < bytes.len() && bytes[j] == b';' {
                            return j + 1;
                        }
                        return i + 1;
                    }
                }
                b';' if paren_count == 0 && brace_count == 0 => return i + 1,
                _ => {}
            }
        }
        i += 1;
    }
    source.len()
}

fn is_drizzle_context(source: &str) -> bool {
    source.contains("drizzle")
        || source.contains("pgTable")
        || source.contains("mysqlTable")
        || source.contains("sqliteTable")
        || source.contains(".from(")
}

fn collect_drizzle_files(dir: &Path, results: &mut Vec<PathBuf>) {
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
                    collect_drizzle_files(&p, results);
                }
            } else if p.is_file() {
                if let Some(ext) = p.extension() {
                    if (ext == "ts" || ext == "js") && !p.to_string_lossy().contains(".d.ts") {
                        if let Ok(content) = fs::read_to_string(&p) {
                            if content.contains("pgTable")
                                || content.contains("mysqlTable")
                                || content.contains("sqliteTable")
                            {
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
    fn test_parse_drizzle_tables() {
        let content = r#"
import { pgTable, serial, text, timestamp } from 'drizzle-orm/pg-core';

export const users = pgTable('users', {
    id: serial('id').primaryKey(),
    name: text('name').notNull(),
    email: text('email').notNull().unique(),
    createdAt: timestamp('created_at').defaultNow(),
});
"#;
        let stitcher = DrizzleStitcher::new();
        let parsed = stitcher.parse_schema(content, Path::new("schema.ts"));

        assert!(parsed.tables.contains_key("users"));
        let users = parsed.tables.get("users").unwrap();
        assert_eq!(users.variable_name, "users");
        assert_eq!(users.table_name, "users");
        assert!(users.definition.contains("pgTable('users'"));
    }

    #[test]
    fn test_stitch_drizzle_query() {
        let temp_dir = TempDir::new().unwrap();
        let schema_path = temp_dir.path().join("schema.ts");
        let content = r#"
export const products = pgTable('products', {
    id: serial('id').primaryKey(),
    title: text('title').notNull(),
});
"#;
        fs::write(&schema_path, content).unwrap();

        let source = "export function getProducts(db: any) { return db.select().from(products); }";
        let stitcher = DrizzleStitcher::new();
        let file_path = temp_dir.path().join("src/repo.ts");

        let stitched = stitcher.stitch(temp_dir.path(), &file_path, source);
        assert_eq!(stitched.len(), 1);
        assert_eq!(stitched[0].name, "products");
        assert_eq!(stitched[0].kind, "drizzle_table");
    }
}
