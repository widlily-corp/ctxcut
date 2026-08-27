//! ORM & Database/API Schema Stitching Subsystem.
//!
//! Provides automated schema discovery, AST query detection, and model/DDL hoisting for:
//! - Prisma (`schema.prisma`)
//! - Drizzle ORM (`pgTable`, `mysqlTable`, `sqliteTable`)
//! - TypeORM (`@Entity`)
//! - Raw SQL queries with migration DDLs (`migrations/*.sql`, `schema.sql`)
//! - Protocol Buffers (`.proto`) gRPC services and messages
//! - GraphQL SDL (`.graphql`, `.gql`) queries, mutations, and types

pub mod drizzle;
pub mod graphql;
pub mod prisma;
pub mod proto;
pub mod sql_migrations;
pub mod typeorm;

pub use drizzle::{DrizzleStitcher, DrizzleTableDef, ParsedDrizzleSchema};
pub use graphql::{GqlFieldDef, GqlTypeDef, GraphqlStitcher, ParsedGqlSchema};
pub use prisma::{ParsedPrismaSchema, PrismaEnumDef, PrismaModelDef, PrismaStitcher};
pub use proto::{
    ParsedProtoFile, ProtoEnumDef, ProtoMessageDef, ProtoRpcDef, ProtoServiceDef, ProtoStitcher,
};
pub use sql_migrations::{
    SqlColumnDef, SqlEnumDef, SqlMigrationStitcher, SqlSchemaSnapshot, SqlTableDef,
};
pub use typeorm::{ParsedTypeOrmSchema, TypeOrmEntityDef, TypeOrmStitcher};

use crate::error::Result;
use crate::model::{CallSignatureStub, ExtractedType};
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::Node;

/// Orchestrator for schema and data contract stitching across polyglot workspaces.
#[derive(Debug, Default, Clone)]
pub struct SchemaStitcher {
    prisma: PrismaStitcher,
    drizzle: DrizzleStitcher,
    typeorm: TypeOrmStitcher,
    sql_migrations: SqlMigrationStitcher,
    proto: ProtoStitcher,
    graphql: GraphqlStitcher,
}

impl SchemaStitcher {
    /// Creates a new `SchemaStitcher` instance with all schema providers enabled.
    pub fn new() -> Self {
        Self {
            prisma: PrismaStitcher::new(),
            drizzle: DrizzleStitcher::new(),
            typeorm: TypeOrmStitcher::new(),
            sql_migrations: SqlMigrationStitcher::new(),
            proto: ProtoStitcher::new(),
            graphql: GraphqlStitcher::new(),
        }
    }

    /// Automatically stitches database and API schema definitions for a given source snippet and file context.
    pub fn stitch_schemas(
        &self,
        workspace_root: &Path,
        current_file: &Path,
        source: &str,
    ) -> Result<Vec<ExtractedType>> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();

        // 1. Prisma schemas
        for ty in self.prisma.stitch(workspace_root, current_file, source) {
            if seen.insert(ty.name.clone()) {
                results.push(ty);
            }
        }

        // 2. Drizzle tables
        for ty in self.drizzle.stitch(workspace_root, current_file, source) {
            if seen.insert(ty.name.clone()) {
                results.push(ty);
            }
        }

        // 3. TypeORM entities
        for ty in self.typeorm.stitch(workspace_root, current_file, source) {
            if seen.insert(ty.name.clone()) {
                results.push(ty);
            }
        }

        // 4. SQL migrations & raw SQL
        for ty in self
            .sql_migrations
            .stitch(workspace_root, current_file, source)
        {
            if seen.insert(ty.name.clone()) {
                results.push(ty);
            }
        }

        // 5. Protobuf IDL definitions
        for ty in self.proto.stitch(workspace_root, current_file, source) {
            if seen.insert(ty.name.clone()) {
                results.push(ty);
            }
        }

        // 6. GraphQL SDL schemas
        for ty in self.graphql.stitch(workspace_root, current_file, source) {
            if seen.insert(ty.name.clone()) {
                results.push(ty);
            }
        }

        Ok(results)
    }

    /// Compatibility interface matching the Milestone 3 specification blueprint.
    pub fn stitch_schemas_with_ast(
        &self,
        workspace_root: &Path,
        _ast_root: Node<'_>,
        source: &str,
        _calls: &[CallSignatureStub],
    ) -> Result<Vec<ExtractedType>> {
        self.stitch_schemas(workspace_root, workspace_root, source)
    }
}

/// Single extracted schema entity (table, model, entity, GraphQL type, Proto message/service) for indexing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaEntityRecord {
    /// Schema kind: `"sql_table"`, `"sql_enum"`, `"prisma_model"`, `"prisma_enum"`, `"drizzle_table"`, `"typeorm_entity"`, `"graphql_type"`, `"proto_message"`, `"proto_service"`.
    pub schema_kind: String,
    /// Entity identifier name (e.g. `"User"`, `"products"`, `"UserService"`).
    pub entity_name: String,
    /// Database table name if different or applicable (e.g. `"users"`, `"products"`).
    pub table_name: Option<String>,
    /// Verbatim or reconstructed DDL/definition block.
    pub definition: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
}

/// Extracts all schema entities (SQL tables, Prisma models, Drizzle tables, TypeORM entities, GraphQL types, Proto messages) from a source file.
pub fn extract_schema_entities(path: &Path, source: &str) -> Vec<SchemaEntityRecord> {
    let mut results = Vec::new();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "sql" => {
            let stitcher = SqlMigrationStitcher::new();
            let mut snapshot = SqlSchemaSnapshot::default();
            stitcher.parse_migration_file(source, path, &mut snapshot);
            for (t_name, t_def) in snapshot.tables {
                results.push(SchemaEntityRecord {
                    schema_kind: "sql_table".to_string(),
                    entity_name: t_def.name.clone(),
                    table_name: Some(t_name),
                    definition: t_def.ddl,
                    start_line: 1,
                    end_line: source.lines().count().max(1),
                });
            }
            for (_e_name, e_def) in snapshot.enums {
                results.push(SchemaEntityRecord {
                    schema_kind: "sql_enum".to_string(),
                    entity_name: e_def.name.clone(),
                    table_name: None,
                    definition: e_def.ddl,
                    start_line: 1,
                    end_line: source.lines().count().max(1),
                });
            }
        }
        "prisma" => {
            let stitcher = PrismaStitcher::new();
            let parsed = stitcher.parse_schema(source, path);
            for (_m_name, m_def) in parsed.models {
                results.push(SchemaEntityRecord {
                    schema_kind: "prisma_model".to_string(),
                    entity_name: m_def.name.clone(),
                    table_name: Some(m_def.name.to_lowercase()),
                    definition: m_def.definition,
                    start_line: 1,
                    end_line: source.lines().count().max(1),
                });
            }
            for (_e_name, e_def) in parsed.enums {
                results.push(SchemaEntityRecord {
                    schema_kind: "prisma_enum".to_string(),
                    entity_name: e_def.name.clone(),
                    table_name: None,
                    definition: e_def.definition,
                    start_line: 1,
                    end_line: source.lines().count().max(1),
                });
            }
        }
        "graphql" | "gql" => {
            let stitcher = GraphqlStitcher::new();
            let parsed = stitcher.parse_schema(source, path);
            for (_t_name, t_def) in parsed.types {
                results.push(SchemaEntityRecord {
                    schema_kind: "graphql_type".to_string(),
                    entity_name: t_def.name.clone(),
                    table_name: None,
                    definition: t_def.definition,
                    start_line: 1,
                    end_line: source.lines().count().max(1),
                });
            }
        }
        "proto" => {
            let stitcher = ProtoStitcher::new();
            let parsed = stitcher.parse_proto(source, path);
            for (_m_name, m_def) in parsed.messages {
                results.push(SchemaEntityRecord {
                    schema_kind: "proto_message".to_string(),
                    entity_name: m_def.name.clone(),
                    table_name: None,
                    definition: m_def.definition,
                    start_line: 1,
                    end_line: source.lines().count().max(1),
                });
            }
            for (_s_name, s_def) in parsed.services {
                results.push(SchemaEntityRecord {
                    schema_kind: "proto_service".to_string(),
                    entity_name: s_def.name.clone(),
                    table_name: None,
                    definition: s_def.definition,
                    start_line: 1,
                    end_line: source.lines().count().max(1),
                });
            }
        }
        "ts" | "tsx" | "js" | "jsx" => {
            if source.contains("pgTable") || source.contains("mysqlTable") || source.contains("sqliteTable") {
                let stitcher = DrizzleStitcher::new();
                let parsed = stitcher.parse_schema(source, path);
                for (_t_name, t_def) in parsed.tables {
                    results.push(SchemaEntityRecord {
                        schema_kind: "drizzle_table".to_string(),
                        entity_name: t_def.variable_name.clone(),
                        table_name: Some(t_def.table_name),
                        definition: t_def.definition,
                        start_line: 1,
                        end_line: source.lines().count().max(1),
                    });
                }
            }
            if source.contains("@Entity") {
                let stitcher = TypeOrmStitcher::new();
                let parsed = stitcher.parse_entities(source, path);
                for (_e_name, e_def) in parsed.entities {
                    results.push(SchemaEntityRecord {
                        schema_kind: "typeorm_entity".to_string(),
                        entity_name: e_def.class_name.clone(),
                        table_name: Some(e_def.table_name),
                        definition: e_def.definition,
                        start_line: 1,
                        end_line: source.lines().count().max(1),
                    });
                }
            }
        }
        _ => {}
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_schema_stitcher_polyglot_orchestration() {
        let temp_dir = TempDir::new().unwrap();

        // 1. Prisma schema
        let prisma_path = temp_dir.path().join("schema.prisma");
        fs::write(&prisma_path, "model User { id Int @id, name String }\n").unwrap();

        // 2. Drizzle schema
        let drizzle_path = temp_dir.path().join("schema.ts");
        fs::write(
            &drizzle_path,
            "export const products = pgTable('products', { id: serial('id').primaryKey() });\n",
        )
        .unwrap();

        // 3. SQL migration
        let mig_dir = temp_dir.path().join("migrations");
        fs::create_dir_all(&mig_dir).unwrap();
        fs::write(
            mig_dir.join("001.sql"),
            "CREATE TABLE orders (id INT, total DECIMAL);\n",
        )
        .unwrap();

        let stitcher = SchemaStitcher::new();

        // Test Prisma call
        let source_prisma = "export function getUser(prisma: any, id: number) { return prisma.user.findUnique({ where: { id } }); }";
        let stitched_prisma = stitcher
            .stitch_schemas(
                temp_dir.path(),
                &temp_dir.path().join("src/service.ts"),
                source_prisma,
            )
            .unwrap();
        assert!(stitched_prisma
            .iter()
            .any(|t| t.name == "User" && t.kind == "prisma_model"));

        // Test Drizzle call
        let source_drizzle =
            "export function getProducts(db: any) { return db.select().from(products); }";
        let stitched_drizzle = stitcher
            .stitch_schemas(
                temp_dir.path(),
                &temp_dir.path().join("src/service.ts"),
                source_drizzle,
            )
            .unwrap();
        assert!(stitched_drizzle
            .iter()
            .any(|t| t.name == "products" && t.kind == "drizzle_table"));

        // Test SQL query call
        let source_sql = "export function getOrders() { return 'SELECT * FROM orders'; }";
        let stitched_sql = stitcher
            .stitch_schemas(
                temp_dir.path(),
                &temp_dir.path().join("src/service.ts"),
                source_sql,
            )
            .unwrap();
        assert!(stitched_sql
            .iter()
            .any(|t| t.name == "orders" && t.kind == "sql_table"));
    }
}
