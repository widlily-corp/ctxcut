//! GraphQL SDL (`.graphql` / `.gql`) schema parser and resolver stitcher.

use crate::model::ExtractedType;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Extracted GraphQL field definition inside a type/query/mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GqlFieldDef {
    /// Field name (e.g. `getProduct`, `createProduct`).
    pub name: String,
    /// Unwrapped return type name (e.g. `Product`).
    pub return_type: String,
    /// Referenced argument type names (e.g. `CreateProductInput`).
    pub arg_types: Vec<String>,
    /// Verbatim field signature (e.g. `getProduct(id: ID!): Product`).
    pub signature: String,
}

/// Extracted GraphQL type definition (`type`, `input`, `enum`, `interface`, `union`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GqlTypeDef {
    /// Type name (e.g. `Product`, `Query`, `CreateProductInput`).
    pub name: String,
    /// GraphQL type kind: `"type"`, `"input"`, `"enum"`, `"interface"`, `"union"`.
    pub kind: String,
    /// Path to the defining `.graphql` file.
    pub file_path: PathBuf,
    /// Field definitions (for object, interface, input types).
    pub fields: Vec<GqlFieldDef>,
    /// Referenced type names across fields/union members.
    pub referenced_types: Vec<String>,
    /// Verbatim type block definition.
    pub definition: String,
}

/// Parsed GraphQL schema representation.
#[derive(Debug, Default, Clone)]
pub struct ParsedGqlSchema {
    /// Types indexed by lowercase type name.
    pub types: HashMap<String, GqlTypeDef>,
}

/// Stitcher for GraphQL SDL definitions and resolvers.
#[derive(Debug, Default, Clone)]
pub struct GraphqlStitcher;

impl GraphqlStitcher {
    /// Creates a new `GraphqlStitcher`.
    pub fn new() -> Self {
        Self
    }

    /// Discovers `.graphql` and `.gql` schema files in the workspace.
    pub fn discover_graphql_files(
        &self,
        workspace_root: &Path,
        current_file: &Path,
    ) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Check current directory parent
        if let Some(dir) = current_file.parent() {
            collect_graphql_files(dir, &mut candidates, 2);
        }

        // 2. Check workspace standard schema directories
        let schema_dirs = [
            workspace_root.join("graphql"),
            workspace_root.join("schema"),
            workspace_root.join("schemas"),
            workspace_root.join("api").join("graphql"),
            workspace_root.join("src").join("graphql"),
            workspace_root.join("src").join("schema"),
        ];
        for dir in &schema_dirs {
            if dir.is_dir() {
                collect_graphql_files(dir, &mut candidates, 3);
            }
        }

        for sname in &["schema.graphql", "schema.gql"] {
            let p = workspace_root.join(sname);
            if p.is_file() {
                candidates.push(p);
            }
        }

        // 3. Fallback: Scan workspace
        if candidates.is_empty() {
            collect_graphql_files(workspace_root, &mut candidates, 5);
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

    /// Parses a GraphQL SDL schema content into structured types and fields.
    pub fn parse_schema(&self, content: &str, file_path: &Path) -> ParsedGqlSchema {
        let mut schema = ParsedGqlSchema::default();
        let lines: Vec<&str> = content.lines().collect();
        let mut idx = 0;

        while idx < lines.len() {
            let line = lines[idx].trim();
            if line.starts_with('#') || line.is_empty() {
                idx += 1;
                continue;
            }

            let type_keywords = ["type ", "input ", "enum ", "interface ", "extend type "];
            let mut matched_kw = None;
            for kw in &type_keywords {
                if line.starts_with(kw) {
                    matched_kw = Some(*kw);
                    break;
                }
            }

            if let Some(kw) = matched_kw {
                if line.contains('{') {
                    let type_name = extract_type_name(line, kw);
                    let (block_lines, next_idx) = extract_balanced_block(&lines, idx);
                    idx = next_idx + 1;

                    if let Some(name) = type_name {
                        let definition = block_lines.join("\n").trim().to_string();
                        let fields = parse_gql_fields(&block_lines);
                        let referenced_types = collect_field_referenced_types(&fields);

                        let kind_str = match kw {
                            "input " => "graphql_input",
                            "enum " => "graphql_enum",
                            "interface " => "graphql_interface",
                            _ => "graphql_type",
                        };

                        let type_def = GqlTypeDef {
                            name: name.clone(),
                            kind: kind_str.to_string(),
                            file_path: file_path.to_path_buf(),
                            fields,
                            referenced_types,
                            definition,
                        };

                        schema.types.insert(name.to_lowercase(), type_def);
                    }
                    continue;
                }
            }

            idx += 1;
        }

        schema
    }

    /// Matches source resolvers and symbol references against GraphQL schemas and returns `ExtractedType` entries.
    pub fn stitch(
        &self,
        workspace_root: &Path,
        current_file: &Path,
        source: &str,
    ) -> Vec<ExtractedType> {
        let schema_files = self.discover_graphql_files(workspace_root, current_file);
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

            // 1. Check Query, Mutation, and Subscription field resolver matching
            for op_type_name in &["query", "mutation", "subscription"] {
                if let Some(op_type) = parsed.types.get(*op_type_name) {
                    for field in &op_type.fields {
                        let variants = generate_name_variants(&field.name);
                        let is_matched = variants.iter().any(|v| source.contains(v));

                        if is_matched {
                            let field_type_name = format!("{}.{}", op_type.name, field.name);
                            if seen_types.insert(field_type_name.clone()) {
                                extracted.push(ExtractedType {
                                    name: field_type_name,
                                    kind: if *op_type_name == "mutation" {
                                        "graphql_mutation".to_string()
                                    } else {
                                        "graphql_query".to_string()
                                    },
                                    file_path: op_type.file_path.to_string_lossy().to_string(),
                                    definition: field.signature.clone(),
                                });
                            }

                            hoist_gql_type_recursive(
                                &field.return_type,
                                &parsed,
                                &mut extracted,
                                &mut seen_types,
                                0,
                            );

                            for arg_type in &field.arg_types {
                                hoist_gql_type_recursive(
                                    arg_type,
                                    &parsed,
                                    &mut extracted,
                                    &mut seen_types,
                                    0,
                                );
                            }
                        }
                    }
                }
            }

            // 2. Check direct type name matching in source
            for (t_key, type_def) in &parsed.types {
                if !matches!(t_key.as_str(), "query" | "mutation" | "subscription")
                    && source.contains(&type_def.name)
                {
                    hoist_gql_type_recursive(
                        &type_def.name,
                        &parsed,
                        &mut extracted,
                        &mut seen_types,
                        0,
                    );
                }
            }
        }

        extracted
    }
}

fn hoist_gql_type_recursive(
    type_name: &str,
    parsed: &ParsedGqlSchema,
    extracted: &mut Vec<ExtractedType>,
    seen_types: &mut HashSet<String>,
    depth: usize,
) {
    if depth > 3 {
        return;
    }

    let clean_name = unwrap_gql_type_name(type_name);
    if clean_name.is_empty() || is_builtin_gql_scalar(&clean_name) {
        return;
    }

    let key = clean_name.to_lowercase();
    if let Some(type_def) = parsed.types.get(&key) {
        if seen_types.insert(type_def.name.clone()) {
            extracted.push(ExtractedType {
                name: type_def.name.clone(),
                kind: type_def.kind.clone(),
                file_path: type_def.file_path.to_string_lossy().to_string(),
                definition: type_def.definition.clone(),
            });

            for nested_ref in &type_def.referenced_types {
                hoist_gql_type_recursive(nested_ref, parsed, extracted, seen_types, depth + 1);
            }
        }
    }
}

fn extract_type_name(line: &str, keyword: &str) -> Option<String> {
    let stripped = line.strip_prefix(keyword)?.trim_start();
    let name_part = stripped
        .split('{')
        .next()?
        .split("implements")
        .next()?
        .trim();
    let name: String = name_part
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn count_braces(line: &str) -> i64 {
    let mut diff: i64 = 0;
    for c in line.chars() {
        if c == '{' {
            diff += 1;
        } else if c == '}' {
            diff -= 1;
        }
    }
    diff
}

fn extract_balanced_block<'a>(lines: &[&'a str], start_idx: usize) -> (Vec<&'a str>, usize) {
    let mut block = vec![lines[start_idx]];
    let mut brace_count: i64 = count_braces(lines[start_idx]);
    let mut idx = start_idx;

    while brace_count > 0 && idx + 1 < lines.len() {
        idx += 1;
        brace_count += count_braces(lines[idx]);
        block.push(lines[idx]);
    }

    (block, idx)
}

fn parse_gql_fields(lines: &[&str]) -> Vec<GqlFieldDef> {
    let mut fields = Vec::new();
    let joined = lines.join("\n");
    let inner = if let Some(open) = joined.find('{') {
        let after = &joined[open + 1..];
        if let Some(close) = after.rfind('}') {
            &after[..close]
        } else {
            after
        }
    } else {
        &joined
    };

    for line in inner.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#')
            || trimmed.starts_with("type ")
            || trimmed.starts_with("input ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with('}')
            || trimmed.is_empty()
        {
            continue;
        }

        for field_part in trimmed.split([';', ',']) {
            let part_trimmed = field_part.trim();
            if part_trimmed.is_empty() || part_trimmed.starts_with('#') {
                continue;
            }

            if let Some(colon_idx) = part_trimmed.rfind(':') {
                let left = part_trimmed[..colon_idx].trim();
                let right = part_trimmed[colon_idx + 1..].trim();

                let return_type = unwrap_gql_type_name(right);

                let (field_name, arg_types) = if let Some(open_paren) = left.find('(') {
                    let fname = left[..open_paren].trim().to_string();
                    let args_part = &left[open_paren + 1..left.rfind(')').unwrap_or(left.len())];
                    let args = extract_arg_types(args_part);
                    (fname, args)
                } else {
                    (left.to_string(), Vec::new())
                };

                if !field_name.is_empty() && !return_type.is_empty() {
                    fields.push(GqlFieldDef {
                        name: field_name,
                        return_type,
                        arg_types,
                        signature: part_trimmed.to_string(),
                    });
                }
            }
        }
    }
    fields
}

fn extract_arg_types(args_str: &str) -> Vec<String> {
    let mut types = Vec::new();
    for part in args_str.split(',') {
        if let Some(colon_idx) = part.find(':') {
            let ty_raw = part[colon_idx + 1..].trim();
            let unwrapped = unwrap_gql_type_name(ty_raw);
            if !unwrapped.is_empty() && !is_builtin_gql_scalar(&unwrapped) {
                types.push(unwrapped);
            }
        }
    }
    types
}

fn collect_field_referenced_types(fields: &[GqlFieldDef]) -> Vec<String> {
    let mut types = Vec::new();
    for field in fields {
        if !is_builtin_gql_scalar(&field.return_type) {
            types.push(field.return_type.clone());
        }
        for arg in &field.arg_types {
            if !is_builtin_gql_scalar(arg) {
                types.push(arg.clone());
            }
        }
    }
    types
}

fn unwrap_gql_type_name(ty: &str) -> String {
    ty.trim()
        .trim_matches(['!', '[', ']', ';', ','])
        .split('@')
        .next()
        .unwrap_or(ty)
        .trim()
        .to_string()
}

fn is_builtin_gql_scalar(ty: &str) -> bool {
    matches!(
        ty,
        "ID" | "String" | "Int" | "Float" | "Boolean" | "Date" | "DateTime" | "JSON"
    )
}

fn generate_name_variants(name: &str) -> Vec<String> {
    let mut variants = Vec::new();
    variants.push(name.to_string());

    // PascalCase
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        let pascal = format!("{}{}", first.to_uppercase(), chars.as_str());
        if !variants.contains(&pascal) {
            variants.push(pascal);
        }
    }

    let mut snake = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                snake.push('_');
            }
            snake.push(c.to_ascii_lowercase());
        } else {
            snake.push(c);
        }
    }
    if !variants.contains(&snake) {
        variants.push(snake.clone());
    }

    let resolve_snake = format!("resolve_{snake}");
    if !variants.contains(&resolve_snake) {
        variants.push(resolve_snake);
    }

    let stripped = name
        .strip_prefix("resolve_")
        .or_else(|| name.strip_prefix("resolve"))
        .or_else(|| name.strip_prefix("get_"))
        .or_else(|| name.strip_prefix("get"))
        .or_else(|| name.strip_prefix("handle_"))
        .or_else(|| name.strip_prefix("handle"));

    if let Some(s) = stripped {
        let mut chars = s.chars();
        if let Some(first) = chars.next() {
            let camel = format!("{}{}", first.to_lowercase(), chars.as_str());
            if !variants.contains(&camel) {
                variants.push(camel);
            }
        }
    }

    variants
}

fn collect_graphql_files(dir: &Path, results: &mut Vec<PathBuf>, max_depth: usize) {
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
                    collect_graphql_files(&p, results, max_depth - 1);
                }
            } else if p.is_file() {
                if let Some(ext) = p.extension() {
                    if ext == "graphql" || ext == "gql" {
                        results.push(p);
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
    fn test_parse_graphql_types_and_queries() {
        let content = r#"
type Product {
    id: ID!
    title: String!
    price: Float!
}

type Query {
    getProduct(id: ID!): Product
}
"#;
        let stitcher = GraphqlStitcher::new();
        let parsed = stitcher.parse_schema(content, Path::new("schema.graphql"));

        assert!(parsed.types.contains_key("product"));
        assert!(parsed.types.contains_key("query"));

        let product = parsed.types.get("product").unwrap();
        assert_eq!(product.name, "Product");
        assert_eq!(product.fields.len(), 3);

        let query = parsed.types.get("query").unwrap();
        assert_eq!(query.fields.len(), 1);
        assert_eq!(query.fields[0].name, "getProduct");
        assert_eq!(query.fields[0].return_type, "Product");
    }

    #[test]
    fn test_stitch_graphql_resolver() {
        let temp_dir = TempDir::new().unwrap();
        let gql_path = temp_dir.path().join("schema.graphql");
        let content = r#"
type Product {
    id: ID!
    title: String!
}

type Query {
    getProduct(id: ID!): Product
}
"#;
        fs::write(&gql_path, content).unwrap();

        let source = "export const resolvers = { Query: { getProduct: async (_: any, { id }: any) => { return { id, title: 'Item' }; } } };";
        let stitcher = GraphqlStitcher::new();
        let file_path = temp_dir.path().join("src/resolvers.ts");

        let stitched = stitcher.stitch(temp_dir.path(), &file_path, source);
        assert!(stitched
            .iter()
            .any(|t| t.name == "Query.getProduct" && t.kind == "graphql_query"));
        assert!(stitched
            .iter()
            .any(|t| t.name == "Product" && t.kind == "graphql_type"));
    }
}
