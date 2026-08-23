//! Prisma schema (`.prisma`) parser and ORM model stitcher.

use crate::model::ExtractedType;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Extracted Prisma model definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrismaModelDef {
    /// Model name (e.g. `User`).
    pub name: String,
    /// Path to the defining `.prisma` file.
    pub file_path: PathBuf,
    /// Verbatim model definition block.
    pub definition: String,
    /// Field type names referenced inside the model (for enum/relation lookup).
    pub referenced_types: Vec<String>,
}

/// Extracted Prisma enum definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrismaEnumDef {
    /// Enum name (e.g. `Role`).
    pub name: String,
    /// Path to the defining `.prisma` file.
    pub file_path: PathBuf,
    /// Verbatim enum definition block.
    pub definition: String,
}

/// Parsed representation of a Prisma schema file.
#[derive(Debug, Default, Clone)]
pub struct ParsedPrismaSchema {
    /// Discovered models indexed by lowercase model name.
    pub models: HashMap<String, PrismaModelDef>,
    /// Discovered enums indexed by lowercase enum name.
    pub enums: HashMap<String, PrismaEnumDef>,
}

/// Stitcher for Prisma ORM models and enums.
#[derive(Debug, Default, Clone)]
pub struct PrismaStitcher;

impl PrismaStitcher {
    /// Creates a new `PrismaStitcher`.
    pub fn new() -> Self {
        Self
    }

    /// Discovers candidate `.prisma` files in the workspace, sorted by proximity to `current_file`.
    pub fn discover_schema_files(&self, workspace_root: &Path, current_file: &Path) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Ascend from current_file parent up to workspace_root checking for prisma schemas
        let mut curr = current_file.parent();
        while let Some(dir) = curr {
            let direct_prisma = dir.join("schema.prisma");
            if direct_prisma.is_file() {
                candidates.push(direct_prisma);
            }
            let sub_prisma = dir.join("prisma").join("schema.prisma");
            if sub_prisma.is_file() {
                candidates.push(sub_prisma);
            }

            if dir == workspace_root {
                break;
            }
            curr = dir.parent();
        }

        // 2. Also check workspace_root standard locations
        let root_direct = workspace_root.join("schema.prisma");
        if root_direct.is_file() {
            candidates.push(root_direct);
        }
        let root_prisma = workspace_root.join("prisma").join("schema.prisma");
        if root_prisma.is_file() {
            candidates.push(root_prisma);
        }

        // 3. Fallback scan for any *.prisma file if none found yet
        if candidates.is_empty() {
            collect_prisma_files(workspace_root, &mut candidates);
        }

        // Deduplicate while preserving proximity order
        let mut seen = HashSet::new();
        candidates.retain(|p| seen.insert(p.clone()));

        // Sort by proximity to current file parent
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

    /// Parses a Prisma schema file content into structured models and enums.
    pub fn parse_schema(&self, content: &str, file_path: &Path) -> ParsedPrismaSchema {
        let mut schema = ParsedPrismaSchema::default();
        let lines: Vec<&str> = content.lines().collect();
        let mut idx = 0;

        while idx < lines.len() {
            let line = lines[idx].trim();

            if line.starts_with("model ") && line.contains('{') {
                let model_name = extract_block_name(line, "model");
                let mut brace_count: i64 = count_braces(line);
                let mut block_lines = vec![lines[idx]];

                while brace_count > 0 && idx + 1 < lines.len() {
                    idx += 1;
                    brace_count += count_braces(lines[idx]);
                    block_lines.push(lines[idx]);
                }

                if let Some(name) = model_name {
                    let definition = block_lines.join("\n").trim().to_string();
                    let referenced_types = extract_referenced_types(&block_lines);
                    let model_def = PrismaModelDef {
                        name: name.clone(),
                        file_path: file_path.to_path_buf(),
                        definition,
                        referenced_types,
                    };
                    schema.models.insert(name.to_lowercase(), model_def);
                }
            } else if line.starts_with("enum ") && line.contains('{') {
                let enum_name = extract_block_name(line, "enum");
                let mut brace_count: i64 = count_braces(line);
                let mut block_lines = vec![lines[idx]];

                while brace_count > 0 && idx + 1 < lines.len() {
                    idx += 1;
                    brace_count += count_braces(lines[idx]);
                    block_lines.push(lines[idx]);
                }

                if let Some(name) = enum_name {
                    let definition = block_lines.join("\n").trim().to_string();
                    let enum_def = PrismaEnumDef {
                        name: name.clone(),
                        file_path: file_path.to_path_buf(),
                        definition,
                    };
                    schema.enums.insert(name.to_lowercase(), enum_def);
                }
            }
            idx += 1;
        }

        schema
    }

    /// Detects referenced Prisma model names in source code or AST calls.
    pub fn detect_model_references(&self, source: &str) -> HashSet<String> {
        let mut references = HashSet::new();

        // 1. Prisma client calls: prisma.<model>.<method>, db.<model>.<method>, ctx.prisma.<model>.<method>
        let prefixes = ["prisma.", "this.prisma.", "ctx.prisma.", "db.", "client."];
        for prefix in prefixes {
            let mut search_from = 0;
            while let Some(pos) = source[search_from..].find(prefix) {
                let start = search_from + pos + prefix.len();
                let rest = &source[start..];
                let ident: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();

                if !ident.is_empty() && is_likely_model_identifier(&ident) {
                    references.insert(ident.to_lowercase());
                }
                search_from = start;
            }
        }

        references
    }

    /// Matches detected references against discovered Prisma schemas and returns `ExtractedType` definitions.
    pub fn stitch(
        &self,
        workspace_root: &Path,
        current_file: &Path,
        source: &str,
    ) -> Vec<ExtractedType> {
        let detected = self.detect_model_references(source);
        if detected.is_empty() && !is_prisma_context(source) {
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

            for (model_key, model_def) in &parsed.models {
                let matches_detected = detected.contains(model_key);
                let matches_exact_in_source = source.contains(&model_def.name);

                if (matches_detected || (matches_exact_in_source && is_prisma_context(source)))
                    && seen_types.insert(model_def.name.clone())
                {
                    extracted.push(ExtractedType {
                        name: model_def.name.clone(),
                        kind: "prisma_model".to_string(),
                        file_path: model_def.file_path.to_string_lossy().to_string(),
                        definition: model_def.definition.clone(),
                    });

                    // Also hoist any referenced enums inside this model
                    for ref_type in &model_def.referenced_types {
                        if let Some(enum_def) = parsed.enums.get(&ref_type.to_lowercase()) {
                            if seen_types.insert(enum_def.name.clone()) {
                                extracted.push(ExtractedType {
                                    name: enum_def.name.clone(),
                                    kind: "prisma_enum".to_string(),
                                    file_path: enum_def.file_path.to_string_lossy().to_string(),
                                    definition: enum_def.definition.clone(),
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

fn extract_block_name(line: &str, keyword: &str) -> Option<String> {
    let stripped = line.strip_prefix(keyword)?.trim_start();
    let name_part = stripped.split('{').next()?.trim();
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

fn extract_referenced_types(lines: &[&str]) -> Vec<String> {
    let mut types = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("@@") || trimmed.starts_with('}') || trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let field_type = parts[1].trim_matches(['?', '[', ']']);
            if !is_scalar_prisma_type(field_type) && !field_type.is_empty() {
                types.push(field_type.to_string());
            }
        }
    }
    types
}

fn is_scalar_prisma_type(ty: &str) -> bool {
    matches!(
        ty,
        "String"
            | "Boolean"
            | "Int"
            | "BigInt"
            | "Float"
            | "Decimal"
            | "DateTime"
            | "Json"
            | "Bytes"
            | "Unsupported"
    )
}

fn is_likely_model_identifier(ident: &str) -> bool {
    !matches!(
        ident,
        "findUnique"
            | "findMany"
            | "findFirst"
            | "create"
            | "createMany"
            | "update"
            | "updateMany"
            | "upsert"
            | "delete"
            | "deleteMany"
            | "count"
            | "aggregate"
            | "groupBy"
            | "$connect"
            | "$disconnect"
            | "$transaction"
            | "$queryRaw"
            | "$executeRaw"
            | "$queryRawUnsafe"
    )
}

fn is_prisma_context(source: &str) -> bool {
    source.contains("prisma") || source.contains("PrismaClient") || source.contains("Prisma.")
}

fn collect_prisma_files(dir: &Path, results: &mut Vec<PathBuf>) {
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
                    collect_prisma_files(&p, results);
                }
            } else if p.is_file() {
                if let Some(ext) = p.extension() {
                    if ext == "prisma" {
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
    fn test_parse_prisma_models_and_enums() {
        let content = r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

enum Role {
  USER
  ADMIN
}

model User {
  id        Int      @id @default(autoincrement())
  email     String   @unique
  name      String?
  role      Role     @default(USER)
  createdAt DateTime @default(now())
}
"#;
        let stitcher = PrismaStitcher::new();
        let parsed = stitcher.parse_schema(content, Path::new("schema.prisma"));

        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.enums.len(), 1);

        let user = parsed.models.get("user").unwrap();
        assert_eq!(user.name, "User");
        assert!(user.definition.contains("model User {"));
        assert!(user.referenced_types.contains(&"Role".to_string()));

        let role = parsed.enums.get("role").unwrap();
        assert_eq!(role.name, "Role");
        assert!(role.definition.contains("enum Role {"));
    }

    #[test]
    fn test_stitch_prisma_model_and_enum_in_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let schema_path = temp_dir.path().join("schema.prisma");
        let prisma_content = r#"
enum Status {
  ACTIVE
  INACTIVE
}

model Account {
  id     Int    @id
  status Status
}
"#;
        fs::write(&schema_path, prisma_content).unwrap();

        let source = "export async function getAcc(prisma: any, id: number) { return prisma.account.findUnique({ where: { id } }); }";
        let stitcher = PrismaStitcher::new();
        let file_path = temp_dir.path().join("src/service.ts");

        let stitched = stitcher.stitch(temp_dir.path(), &file_path, source);
        assert_eq!(stitched.len(), 2);
        assert!(stitched.iter().any(|t| t.name == "Account" && t.kind == "prisma_model"));
        assert!(stitched.iter().any(|t| t.name == "Status" && t.kind == "prisma_enum"));
    }
}
