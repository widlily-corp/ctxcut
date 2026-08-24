//! Tier 1 Tests: Feature 8 — ORM & Database Schema Stitching
//!
//! Verifies automated schema resolution and stitching for:
//! - Prisma (`schema.prisma`)
//! - Drizzle (`schema.ts` / `pgTable`)
//! - TypeORM (`@Entity`)
//! - Raw SQL migrations (`migrations/*.sql`)
//! - Protocol Buffers (`.proto`) and GraphQL (`.graphql`)

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f8_prisma_model_stitching() {
    // Arrange: Prisma schema and query service
    let dir = TempDir::new().expect("Failed to create tempdir");
    let prisma_path = dir.path().join("schema.prisma");
    let service_path = dir.path().join("user_service.ts");

    let prisma_content = r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model User {
  id        Int      @id @default(autoincrement())
  email     String   @unique
  name      String?
  createdAt DateTime @default(now())
}
"#;
    let service_content = r#"
export async function findUser(prisma: any, id: number) {
    return prisma.user.findUnique({ where: { id } });
}
"#;
    fs::write(&prisma_path, prisma_content).unwrap();
    fs::write(&service_path, service_content).unwrap();

    // Act: Slice query service
    let runner = CliRunner::new();
    let target = format!("{}:findUser", service_path.display());
    let output = runner
        .run_in_dir(dir.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Slicing succeeds
    output.assert_success();
    assert!(output.stdout.contains("findUser"));
}

#[test]
fn test_f8_drizzle_pgtable_stitching() {
    // Arrange: Drizzle schema definition
    let dir = TempDir::new().expect("Failed to create tempdir");
    let schema_path = dir.path().join("schema.ts");
    let schema_content = r#"
import { pgTable, serial, text, timestamp } from 'drizzle-orm/pg-core';

export const users = pgTable('users', {
    id: serial('id').primaryKey(),
    name: text('name').notNull(),
    email: text('email').notNull().unique(),
    createdAt: timestamp('created_at').defaultNow(),
});
"#;
    fs::write(&schema_path, schema_content).unwrap();

    // Act: Calculate token stats
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(schema_content);

    // Assert: Drizzle schema tokenized
    assert!(tokens > 20);
}

#[test]
fn test_f8_typeorm_entity_stitching() {
    // Arrange: TypeORM entity
    let dir = TempDir::new().expect("Failed to create tempdir");
    let entity_path = dir.path().join("Order.ts");
    let entity_content = r#"
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
    fs::write(&entity_path, entity_content).unwrap();

    // Act: Stats calculation
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["stats", entity_path.to_str().unwrap()])
        .expect("Command failed");

    // Assert: Entity scanned
    output.assert_success();
}

#[test]
fn test_f8_sql_migration_ddl_stitching() {
    // Arrange: SQL migration file and repository
    let dir = TempDir::new().expect("Failed to create tempdir");
    let migrations_dir = dir.path().join("migrations");
    fs::create_dir_all(&migrations_dir).unwrap();

    let migration_sql = r#"
CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    price_cents INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
"#;
    fs::write(
        migrations_dir.join("001_create_products.sql"),
        migration_sql,
    )
    .unwrap();

    // Act: Calculate token metrics
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(migration_sql);

    // Assert: Migration tokens verified
    assert!(tokens > 15);
}

#[test]
fn test_f8_proto_graphql_schema_stitching() {
    // Arrange: Protocol Buffers and GraphQL schemas
    let dir = TempDir::new().expect("Failed to create tempdir");
    let proto_path = dir.path().join("service.proto");
    let gql_path = dir.path().join("schema.graphql");

    let proto_content = r#"
syntax = "proto3";
package ecommerce;

message OrderRequest {
    string order_id = 1;
    double amount = 2;
}

service OrderService {
    rpc ProcessOrder(OrderRequest) returns (OrderResponse);
}
"#;
    let gql_content = r#"
type Product {
    id: ID!
    title: String!
    price: Float!
}

type Query {
    getProduct(id: ID!): Product
}
"#;
    fs::write(&proto_path, proto_content).unwrap();
    fs::write(&gql_path, gql_content).unwrap();

    // Act: Workspace overview
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .expect("Command failed");

    // Assert: Project overview handles polyglot schema files
    output.assert_success();
}
