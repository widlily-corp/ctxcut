//! SQL migration DDL crawler, parser, and raw SQL query schema stitcher.

use crate::model::ExtractedType;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Parsed column definition in a SQL table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlColumnDef {
    /// Column name (e.g. `id`, `price_cents`).
    pub name: String,
    /// Data type (e.g. `SERIAL`, `VARCHAR(255)`, `INTEGER`).
    pub data_type: String,
    /// Column constraints (e.g. `PRIMARY KEY`, `NOT NULL`).
    pub constraints: Vec<String>,
}

/// Extracted SQL table definition from migration DDLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlTableDef {
    /// Table name (e.g. `products`, `users`).
    pub name: String,
    /// Path to the migration file where defined or last altered.
    pub file_path: PathBuf,
    /// Columns in the table.
    pub columns: Vec<SqlColumnDef>,
    /// Verbatim or synthesized `CREATE TABLE ...` DDL.
    pub ddl: String,
}

/// Extracted SQL custom enum type (e.g. `CREATE TYPE user_role AS ENUM (...)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlEnumDef {
    /// Enum type name.
    pub name: String,
    /// Path to the migration file.
    pub file_path: PathBuf,
    /// Enum variants.
    pub variants: Vec<String>,
    /// Verbatim `CREATE TYPE ...` DDL.
    pub ddl: String,
}

/// Cumulative SQL schema snapshot constructed from chronological migrations.
#[derive(Debug, Default, Clone)]
pub struct SqlSchemaSnapshot {
    /// Tables indexed by lowercase table name.
    pub tables: HashMap<String, SqlTableDef>,
    /// Custom enum types indexed by lowercase enum name.
    pub enums: HashMap<String, SqlEnumDef>,
}

/// Stitcher for raw SQL queries and database migration DDLs.
#[derive(Debug, Default, Clone)]
pub struct SqlMigrationStitcher;

impl SqlMigrationStitcher {
    /// Creates a new `SqlMigrationStitcher`.
    pub fn new() -> Self {
        Self
    }

    /// Discovers all SQL migration files in the workspace in chronological order.
    pub fn discover_migration_files(&self, workspace_root: &Path, current_file: &Path) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Check directories ascending from current_file to workspace_root
        let mut curr = current_file.parent();
        while let Some(dir) = curr {
            let migration_subdirs = [
                dir.join("migrations"),
                dir.join("db").join("migrations"),
                dir.join("database").join("migrations"),
                dir.join("sql").join("migrations"),
                dir.join("src").join("migrations"),
                dir.join("prisma").join("migrations"),
            ];
            for mdir in &migration_subdirs {
                if mdir.is_dir() {
                    collect_sql_files_recursive(mdir, &mut candidates);
                }
            }

            for sname in &["schema.sql", "init.sql", "tables.sql", "structure.sql", "db.sql"] {
                let sfile = dir.join(sname);
                if sfile.is_file() {
                    candidates.push(sfile);
                }
            }

            if dir == workspace_root {
                break;
            }
            curr = dir.parent();
        }

        // 2. Check workspace root standard locations
        let root_migration_subdirs = [
            workspace_root.join("migrations"),
            workspace_root.join("db").join("migrations"),
            workspace_root.join("database").join("migrations"),
            workspace_root.join("sql").join("migrations"),
            workspace_root.join("src").join("migrations"),
            workspace_root.join("prisma").join("migrations"),
        ];
        for mdir in &root_migration_subdirs {
            if mdir.is_dir() {
                collect_sql_files_recursive(mdir, &mut candidates);
            }
        }

        for sname in &["schema.sql", "init.sql", "tables.sql", "structure.sql", "db.sql"] {
            let sfile = workspace_root.join(sname);
            if sfile.is_file() {
                candidates.push(sfile);
            }
        }

        // 3. Fallback: Search workspace for any .sql files if none found
        if candidates.is_empty() {
            collect_sql_files_recursive(workspace_root, &mut candidates);
        }

        candidates.sort();
        candidates.dedup();
        candidates
    }

    /// Parses all migration files into a cumulative `SqlSchemaSnapshot`.
    pub fn build_schema_snapshot(&self, migration_files: &[PathBuf]) -> SqlSchemaSnapshot {
        let mut snapshot = SqlSchemaSnapshot::default();
        for file in migration_files {
            if let Ok(content) = fs::read_to_string(file) {
                self.parse_migration_file(&content, file, &mut snapshot);
            }
        }
        snapshot
    }

    /// Parses a single SQL migration file content into the snapshot.
    pub fn parse_migration_file(
        &self,
        content: &str,
        file_path: &Path,
        snapshot: &mut SqlSchemaSnapshot,
    ) {
        let statements = split_sql_statements(content);
        for stmt in statements {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }

            let upper = trimmed.to_uppercase();
            if upper.starts_with("CREATE TABLE") {
                if let Some(table) = parse_create_table(trimmed, file_path) {
                    snapshot.tables.insert(table.name.to_lowercase(), table);
                }
            } else if upper.starts_with("CREATE TYPE") && upper.contains("AS ENUM") {
                if let Some(sql_enum) = parse_create_type_enum(trimmed, file_path) {
                    snapshot.enums.insert(sql_enum.name.to_lowercase(), sql_enum);
                }
            } else if upper.starts_with("ALTER TABLE") && upper.contains("ADD") {
                if let Some((tbl_name, col)) = parse_alter_table_add_column(trimmed) {
                    if let Some(table) = snapshot.tables.get_mut(&tbl_name.to_lowercase()) {
                        table.columns.push(col);
                        table.ddl = synthesize_table_ddl(table);
                    }
                }
            } else if upper.starts_with("ALTER TABLE") && upper.contains("DROP") {
                if let Some((tbl_name, col_name)) = parse_alter_table_drop_column(trimmed) {
                    if let Some(table) = snapshot.tables.get_mut(&tbl_name.to_lowercase()) {
                        table.columns.retain(|c| !c.name.eq_ignore_ascii_case(&col_name));
                        table.ddl = synthesize_table_ddl(table);
                    }
                }
            }
        }
    }

    /// Detects referenced database table names from SQL queries in source code.
    pub fn detect_table_references(&self, source: &str) -> HashSet<String> {
        let mut tables = HashSet::new();

        let query_strings = extract_sql_query_strings(source);
        for query in &query_strings {
            let stripped = strip_sql_comments(query);
            extract_table_names_from_sql(&stripped, &mut tables);
        }

        extract_table_names_from_sql(source, &mut tables);

        tables
    }

    /// Matches detected SQL queries against migration schemas and returns `ExtractedType` definitions.
    pub fn stitch(
        &self,
        workspace_root: &Path,
        current_file: &Path,
        source: &str,
    ) -> Vec<ExtractedType> {
        let referenced_tables = self.detect_table_references(source);
        if referenced_tables.is_empty() {
            return Vec::new();
        }

        let migration_files = self.discover_migration_files(workspace_root, current_file);
        if migration_files.is_empty() {
            return Vec::new();
        }

        let snapshot = self.build_schema_snapshot(&migration_files);
        if snapshot.tables.is_empty() && snapshot.enums.is_empty() {
            return Vec::new();
        }

        let mut extracted = Vec::new();
        let mut seen_types = HashSet::new();

        for tbl_name in &referenced_tables {
            if let Some(table_def) = snapshot.tables.get(tbl_name) {
                if seen_types.insert(table_def.name.clone()) {
                    extracted.push(ExtractedType {
                        name: table_def.name.clone(),
                        kind: "sql_table".to_string(),
                        file_path: table_def.file_path.to_string_lossy().to_string(),
                        definition: table_def.ddl.clone(),
                    });

                    // Check if table columns use any custom enum types
                    for col in &table_def.columns {
                        let col_type_lower = col.data_type.to_lowercase();
                        if let Some(enum_def) = snapshot.enums.get(&col_type_lower) {
                            if seen_types.insert(enum_def.name.clone()) {
                                extracted.push(ExtractedType {
                                    name: enum_def.name.clone(),
                                    kind: "sql_enum".to_string(),
                                    file_path: enum_def.file_path.to_string_lossy().to_string(),
                                    definition: enum_def.ddl.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        extracted
    }
}

fn split_sql_statements(content: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_quote = None;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        let next_b = if i + 1 < bytes.len() { Some(bytes[i + 1]) } else { None };

        if in_line_comment {
            current.push(b as char);
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            current.push(b as char);
            if b == b'*' && next_b == Some(b'/') {
                current.push('/');
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if let Some(q) = in_quote {
            current.push(b as char);
            if b == q && (i == 0 || bytes[i - 1] != b'\\') {
                in_quote = None;
            }
            i += 1;
            continue;
        }

        if b == b'-' && next_b == Some(b'-') {
            in_line_comment = true;
            current.push('-');
            current.push('-');
            i += 2;
            continue;
        }
        if b == b'/' && next_b == Some(b'*') {
            in_block_comment = true;
            current.push('/');
            current.push('*');
            i += 2;
            continue;
        }

        match b {
            b'\'' | b'"' | b'`' => {
                in_quote = Some(b);
                current.push(b as char);
            }
            b';' => {
                current.push(';');
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    statements.push(trimmed);
                }
                current.clear();
            }
            _ => {
                current.push(b as char);
            }
        }
        i += 1;
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }

    statements
}

fn parse_create_table(stmt: &str, file_path: &Path) -> Option<SqlTableDef> {
    let upper = stmt.to_uppercase();
    let create_idx = upper.find("CREATE TABLE")?;
    let after_create = stmt[create_idx + 12..].trim_start();

    let after_if = if after_create.to_uppercase().starts_with("IF NOT EXISTS") {
        after_create[13..].trim_start()
    } else {
        after_create
    };

    let paren_idx = after_if.find('(')?;
    let table_name_raw = after_if[..paren_idx].trim();
    let table_name = sanitize_table_ident(table_name_raw);
    if table_name.is_empty() {
        return None;
    }

    let columns = parse_column_definitions(&after_if[paren_idx..]);

    Some(SqlTableDef {
        name: table_name,
        file_path: file_path.to_path_buf(),
        columns,
        ddl: stmt.trim().to_string(),
    })
}

fn parse_create_type_enum(stmt: &str, file_path: &Path) -> Option<SqlEnumDef> {
    let upper = stmt.to_uppercase();
    let type_idx = upper.find("CREATE TYPE")?;
    let after_type = stmt[type_idx + 11..].trim_start();
    let as_idx = after_type.to_uppercase().find("AS ENUM")?;
    let enum_name_raw = after_type[..as_idx].trim();
    let enum_name = sanitize_table_ident(enum_name_raw);

    let paren_start = after_type[as_idx..].find('(')?;
    let after_paren = &after_type[as_idx + paren_start + 1..];
    let paren_end = after_paren.find(')')?;
    let variants_str = &after_paren[..paren_end];

    let variants: Vec<String> = variants_str
        .split(',')
        .map(|v| v.trim().trim_matches(['\'', '"']).to_string())
        .filter(|v| !v.is_empty())
        .collect();

    Some(SqlEnumDef {
        name: enum_name,
        file_path: file_path.to_path_buf(),
        variants,
        ddl: stmt.trim().to_string(),
    })
}

fn parse_alter_table_add_column(stmt: &str) -> Option<(String, SqlColumnDef)> {
    let upper = stmt.to_uppercase();
    let alter_idx = upper.find("ALTER TABLE")?;
    let after_alter = stmt[alter_idx + 11..].trim_start();
    let add_idx = after_alter.to_uppercase().find("ADD")?;
    let table_name_raw = after_alter[..add_idx].trim();
    let table_name = sanitize_table_ident(table_name_raw);

    let after_add = after_alter[add_idx + 3..].trim_start();
    let after_column = if after_add.to_uppercase().starts_with("COLUMN") {
        after_add[6..].trim_start()
    } else {
        after_add
    };

    let col_str = after_column.trim_end_matches(';').trim();
    let parts: Vec<&str> = col_str.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let col_name = sanitize_table_ident(parts[0]);
    let data_type = parts[1].to_string();
    let constraints = parts[2..].iter().map(|s| (*s).to_string()).collect();

    Some((
        table_name,
        SqlColumnDef {
            name: col_name,
            data_type,
            constraints,
        },
    ))
}

fn parse_alter_table_drop_column(stmt: &str) -> Option<(String, String)> {
    let upper = stmt.to_uppercase();
    let alter_idx = upper.find("ALTER TABLE")?;
    let after_alter = stmt[alter_idx + 11..].trim_start();
    let drop_idx = after_alter.to_uppercase().find("DROP")?;
    let table_name_raw = after_alter[..drop_idx].trim();
    let table_name = sanitize_table_ident(table_name_raw);

    let after_drop = after_alter[drop_idx + 4..].trim_start();
    let after_column = if after_drop.to_uppercase().starts_with("COLUMN") {
        after_drop[6..].trim_start()
    } else {
        after_drop
    };

    let col_name = sanitize_table_ident(after_column.trim_end_matches(';').trim());
    Some((table_name, col_name))
}

fn parse_column_definitions(paren_body: &str) -> Vec<SqlColumnDef> {
    let mut columns = Vec::new();
    let trimmed = paren_body.trim_start_matches('(').trim_end_matches([')', ';', '\n', ' ']);

    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in trimmed.chars() {
        match c {
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                let s = current.trim().to_string();
                if !s.is_empty() {
                    parts.push(s);
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }
    let s = current.trim().to_string();
    if !s.is_empty() {
        parts.push(s);
    }

    for part in parts {
        let trimmed_part = part.trim();
        let upper_part = trimmed_part.to_uppercase();
        if upper_part.starts_with("PRIMARY KEY")
            || upper_part.starts_with("FOREIGN KEY")
            || upper_part.starts_with("CONSTRAINT")
            || upper_part.starts_with("UNIQUE")
            || upper_part.starts_with("CHECK")
        {
            continue;
        }

        let words: Vec<&str> = trimmed_part.split_whitespace().collect();
        if words.len() >= 2 {
            let col_name = sanitize_table_ident(words[0]);
            let data_type = words[1].to_string();
            let constraints = words[2..].iter().map(|w| (*w).to_string()).collect();
            if !col_name.is_empty() {
                columns.push(SqlColumnDef {
                    name: col_name,
                    data_type,
                    constraints,
                });
            }
        }
    }

    columns
}

fn synthesize_table_ddl(table: &SqlTableDef) -> String {
    let mut ddl = format!("CREATE TABLE {} (\n", table.name);
    for (i, col) in table.columns.iter().enumerate() {
        let is_last = i + 1 == table.columns.len();
        let constr = if col.constraints.is_empty() {
            String::new()
        } else {
            format!(" {}", col.constraints.join(" "))
        };
        let comma = if is_last { "" } else { "," };
        ddl.push_str(&format!("    {} {}{}{}\n", col.name, col.data_type, constr, comma));
    }
    ddl.push_str(");");
    ddl
}

fn extract_sql_query_strings(source: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let mut search = 0;

    let sql_keywords = ["SELECT", "INSERT INTO", "UPDATE", "DELETE FROM", "CREATE TABLE"];

    while search < source.len() {
        let next_delim = source[search..].find(['`', '"', '\'']);
        let delim_pos = match next_delim {
            Some(p) => search + p,
            None => break,
        };

        let delim_char = source.as_bytes()[delim_pos];
        let after_delim = &source[delim_pos + 1..];

        let mut end_pos = None;
        let bytes = after_delim.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == delim_char && (i == 0 || bytes[i - 1] != b'\\') {
                end_pos = Some(delim_pos + 1 + i);
                break;
            }
            i += 1;
        }

        if let Some(end) = end_pos {
            let str_content = &source[delim_pos + 1..end];
            let upper = str_content.to_uppercase();
            if sql_keywords.iter().any(|kw| upper.contains(kw)) {
                queries.push(str_content.to_string());
            }
            search = end + 1;
        } else {
            break;
        }
    }

    queries
}

fn strip_sql_comments(sql: &str) -> String {
    let mut result = String::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if let Some(pos) = trimmed.find("--") {
            result.push_str(&trimmed[..pos]);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}

fn extract_table_names_from_sql(sql: &str, tables: &mut HashSet<String>) {
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let mut i = 0;

    while i < tokens.len() {
        let tok = tokens[i].to_uppercase();
        let clean_tok = tok.trim_matches(['(', ')', ';']);

        if (clean_tok == "FROM"
            || clean_tok == "JOIN"
            || clean_tok == "INTO"
            || clean_tok == "UPDATE"
            || clean_tok == "TABLE")
            && i + 1 < tokens.len()
        {
            let candidate = tokens[i + 1];
            let sanitized = sanitize_table_ident(candidate);
            if is_valid_table_name(&sanitized) {
                tables.insert(sanitized.to_lowercase());
            }

            let mut next_idx = i + 1;
            while tokens[next_idx].ends_with(',') && next_idx + 1 < tokens.len() {
                next_idx += 1;
                let next_candidate = tokens[next_idx];
                let next_sanitized = sanitize_table_ident(next_candidate);
                if is_valid_table_name(&next_sanitized) {
                    tables.insert(next_sanitized.to_lowercase());
                }
            }
        }
        i += 1;
    }
}

fn sanitize_table_ident(raw: &str) -> String {
    let trimmed = raw.trim_matches([';', '(', ')', ',', '\'', '"', '`']);
    let without_schema = if let Some(dot_pos) = trimmed.rfind('.') {
        &trimmed[dot_pos + 1..]
    } else {
        trimmed
    };

    let first_word = without_schema.split_whitespace().next().unwrap_or(without_schema);

    if first_word.contains("${") || first_word.starts_with('$') || first_word.starts_with('?') || first_word.starts_with('{') {
        return String::new();
    }

    first_word
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

fn is_valid_table_name(ident: &str) -> bool {
    if ident.is_empty() || ident.len() < 2 {
        return false;
    }
    let upper = ident.to_uppercase();
    !matches!(
        upper.as_str(),
        "SELECT"
            | "WHERE"
            | "SET"
            | "VALUES"
            | "GROUP"
            | "ORDER"
            | "HAVING"
            | "LIMIT"
            | "ON"
            | "AS"
            | "AND"
            | "OR"
            | "NOT"
            | "IN"
            | "JOIN"
            | "LEFT"
            | "RIGHT"
            | "INNER"
            | "OUTER"
            | "FULL"
            | "CROSS"
            | "CASE"
            | "WHEN"
            | "THEN"
            | "ELSE"
            | "END"
            | "EXISTS"
            | "TRUE"
            | "FALSE"
            | "NULL"
    )
}

fn collect_sql_files_recursive(dir: &Path, results: &mut Vec<PathBuf>) {
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
                    collect_sql_files_recursive(&p, results);
                }
            } else if p.is_file() {
                if let Some(ext) = p.extension() {
                    if ext == "sql" {
                        results.push(p);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_create_table_and_alter() {
        let content = r#"
CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL
);

ALTER TABLE products ADD COLUMN price_cents INTEGER NOT NULL;
"#;
        let stitcher = SqlMigrationStitcher::new();
        let mut snapshot = SqlSchemaSnapshot::default();
        stitcher.parse_migration_file(content, Path::new("001.sql"), &mut snapshot);

        assert!(snapshot.tables.contains_key("products"));
        let products = snapshot.tables.get("products").unwrap();
        assert_eq!(products.columns.len(), 3);
        assert_eq!(products.columns[0].name, "id");
        assert_eq!(products.columns[1].name, "title");
        assert_eq!(products.columns[2].name, "price_cents");
    }

    #[test]
    fn test_stitch_raw_sql_query_with_joins() {
        let temp_dir = TempDir::new().unwrap();
        let mig_dir = temp_dir.path().join("migrations");
        fs::create_dir_all(&mig_dir).unwrap();

        let mig1 = r#"
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL
);
"#;
        let mig2 = r#"
CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    user_id INT REFERENCES users(id),
    total DECIMAL NOT NULL
);
"#;
        fs::write(mig_dir.join("001_users.sql"), mig1).unwrap();
        fs::write(mig_dir.join("002_orders.sql"), mig2).unwrap();

        let source = "export function fetchOrders() { return 'SELECT users.email, orders.total FROM users JOIN orders ON users.id = orders.user_id'; }";
        let stitcher = SqlMigrationStitcher::new();
        let file_path = temp_dir.path().join("src/db.ts");

        let stitched = stitcher.stitch(temp_dir.path(), &file_path, source);
        assert_eq!(stitched.len(), 2);
        assert!(stitched.iter().any(|t| t.name == "users" && t.kind == "sql_table"));
        assert!(stitched.iter().any(|t| t.name == "orders" && t.kind == "sql_table"));
    }
}
