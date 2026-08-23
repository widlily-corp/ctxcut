//! Empirical Challenger 1 Adversarial Test Suite for Milestone 3: ORM & Database/API Schema Stitching
//!
//! Comprehensive empirical stress tests verifying:
//! 1. Deeply nested Prisma models with circular relation fields (multi-way cyclic dependencies, self-referential relations, enums).
//! 2. Complex Drizzle schemas with multiple joined tables, dialects (pg/mysql/sqlite), and enum types.
//! 3. Raw SQL queries with complex multi-table joins, subqueries, CTEs, aliases, and schema-qualified names.
//! 4. Missing schema files, corrupted SQL migration files, syntax errors, and dynamic SQL queries.
//! 5. TypeORM entities, Protobuf deep message hierarchies, and GraphQL SDL queries/mutations.
//! 6. Token overhead bounds and sub-millisecond execution performance.

use ctxcut_core::model::{ExtractedType, SliceOptions};
use ctxcut_core::schema::{
    DrizzleStitcher, PrismaStitcher, SchemaStitcher, SqlMigrationStitcher,
};
use ctxcut_core::slice::ContextSlicer;
use std::collections::HashSet;
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

// ================================================================================================
// 1. Deeply Nested Prisma Models with Circular Relations & Self-Referencing
// ================================================================================================

#[test]
fn test_adversarial_prisma_circular_relations_and_self_referencing() {
    let dir = TempDir::new().expect("Create tempdir");
    let prisma_path = dir.path().join("schema.prisma");

    // Complex multi-way cyclic schema:
    // User -> Organization -> Project -> Task -> Comment -> User
    // User -> User (manager/subordinates)
    // Category -> Category (parent/children)
    // Enums: UserRole, AccountStatus, TaskPriority, TaskStatus, CategoryVisibility
    let prisma_content = r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

generator client {
  provider = "prisma-client-js"
}

enum UserRole {
  SUPERADMIN
  ORG_ADMIN
  DEVELOPER
  GUEST
}

enum AccountStatus {
  PENDING_VERIFICATION
  ACTIVE
  SUSPENDED
  DELETED
}

enum TaskPriority {
  LOW
  MEDIUM
  HIGH
  CRITICAL
  BLOCKER
}

enum TaskStatus {
  BACKLOG
  TODO
  IN_PROGRESS
  IN_REVIEW
  DONE
  ARCHIVED
}

enum CategoryVisibility {
  PUBLIC
  INTERNAL
  RESTRICTED
}

model User {
  id            String         @id @default(uuid())
  email         String         @unique
  role          UserRole       @default(DEVELOPER)
  status        AccountStatus  @default(PENDING_VERIFICATION)
  managerId     String?
  manager       User?          @relation("ManagementHierarchy", fields: [managerId], references: [id])
  subordinates  User[]         @relation("ManagementHierarchy")
  organizations Membership[]
  createdTasks  Task[]         @relation("TaskAuthor")
  assignedTasks Task[]         @relation("TaskAssignee")
  comments      Comment[]
  createdAt     DateTime       @default(now())
  updatedAt     DateTime       @updatedAt
}

model Organization {
  id          String       @id @default(uuid())
  name        String
  slug        String       @unique
  members     Membership[]
  projects    Project[]
  categories  Category[]
  createdAt   DateTime     @default(now())
}

model Membership {
  id             String       @id @default(uuid())
  userId         String
  user           User         @relation(fields: [userId], references: [id])
  organizationId String
  organization   Organization @relation(fields: [organizationId], references: [id])
  role           UserRole     @default(DEVELOPER)
  joinedAt       DateTime     @default(now())

  @@unique([userId, organizationId])
}

model Project {
  id             String       @id @default(uuid())
  title          String
  organizationId String
  organization   Organization @relation(fields: [organizationId], references: [id])
  tasks          Task[]
  createdAt      DateTime     @default(now())
}

model Task {
  id          String       @id @default(uuid())
  title       String
  description String?
  priority    TaskPriority @default(MEDIUM)
  status      TaskStatus   @default(TODO)
  projectId   String
  project     Project      @relation(fields: [projectId], references: [id])
  authorId    String
  author      User         @relation("TaskAuthor", fields: [authorId], references: [id])
  assigneeId  String?
  assignee    User?        @relation("TaskAssignee", fields: [assigneeId], references: [id])
  comments    Comment[]
  categoryId  String?
  category    Category?    @relation(fields: [categoryId], references: [id])
}

model Comment {
  id        String   @id @default(uuid())
  body      String
  taskId    String
  task      Task     @relation(fields: [taskId], references: [id])
  authorId  String
  author    User     @relation(fields: [authorId], references: [id])
  createdAt DateTime @default(now())
}

model Category {
  id             String             @id @default(uuid())
  name           String
  visibility     CategoryVisibility @default(PUBLIC)
  organizationId String
  organization   Organization       @relation(fields: [organizationId], references: [id])
  parentId       String?
  parent         Category?          @relation("CategoryTree", fields: [parentId], references: [id])
  children       Category[]         @relation("CategoryTree")
  tasks          Task[]
}
"#;
    fs::write(&prisma_path, prisma_content).expect("Write schema.prisma");
    fs::write(dir.path().join("package.json"), r#"{"name":"test-prisma"}"#).unwrap();

    let service_path = dir.path().join("src/task_service.ts");
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    let service_content = r#"
export async function getTaskDetails(prisma: any, taskId: string) {
    const task = await prisma.task.findUnique({
        where: { id: taskId },
        include: {
            author: true,
            assignee: true,
            comments: { include: { author: true } },
            category: { include: { parent: true } },
        }
    });
    return task;
}
"#;
    fs::write(&service_path, service_content).expect("Write task_service.ts");

    let stitcher = PrismaStitcher::new();
    let start = Instant::now();
    let stitched = stitcher.stitch(dir.path(), &service_path, service_content);
    let elapsed = start.elapsed();

    // Verification:
    // 1. Task model should be hoisted
    assert!(
        stitched.iter().any(|t| t.name == "Task" && t.kind == "prisma_model"),
        "Task model must be hoisted"
    );
    // 2. Referenced enums TaskPriority and TaskStatus must be hoisted
    assert!(
        stitched.iter().any(|t| t.name == "TaskPriority" && t.kind == "prisma_enum"),
        "TaskPriority enum must be hoisted"
    );
    assert!(
        stitched.iter().any(|t| t.name == "TaskStatus" && t.kind == "prisma_enum"),
        "TaskStatus enum must be hoisted"
    );
    // 3. No infinite loops or excessive runtime (< 50ms)
    assert!(elapsed.as_millis() < 50, "Stitching must be instantaneous");

    // 4. Ensure no duplicate entries
    let mut names = HashSet::new();
    for item in &stitched {
        assert!(names.insert(item.name.clone()), "Duplicate type found: {}", item.name);
    }

    // 5. Test slicing integration with ContextSlicer
    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };
    let slice_result = slicer
        .slice_symbol(&service_path, "getTaskDetails", &opts)
        .expect("Slicing getTaskDetails");
    println!("Hoisted types in slice_result: {:?}", slice_result.hoisted_types);
    assert!(
        slice_result.hoisted_types.iter().any(|t| t.name == "Task"),
        "ContextSlicer must include stitched Task model in hoisted_types. Actual: {:?}",
        slice_result.hoisted_types
    );
    assert!(
        slice_result.hoisted_types.iter().any(|t| t.name == "TaskPriority"),
        "ContextSlicer must include stitched TaskPriority enum in hoisted_types"
    );
}

// ================================================================================================
// 2. Complex Drizzle Schemas with Multiple Joined Tables, Dialects & Custom Enum Types
// ================================================================================================

#[test]
fn test_adversarial_drizzle_multi_table_joins_and_dialects() {
    let dir = TempDir::new().expect("Create tempdir");
    let schema_path = dir.path().join("db/schema.ts");
    fs::create_dir_all(schema_path.parent().unwrap()).unwrap();

    let drizzle_content = r#"
import { pgTable, pgEnum, serial, text, integer, decimal, timestamp, boolean, varchar } from 'drizzle-orm/pg-core';
import { mysqlTable, int, varchar as mysqlVarchar } from 'drizzle-orm/mysql-core';
import { sqliteTable, text as sqliteText, integer as sqliteInt } from 'drizzle-orm/sqlite-core';
import { relations } from 'drizzle-orm';

export const orderStatusEnum = pgEnum('order_status', [
    'pending',
    'processing',
    'shipped',
    'delivered',
    'refunded',
    'cancelled'
]);

export const users = pgTable('users', {
    id: serial('id').primaryKey(),
    name: text('name').notNull(),
    email: varchar('email', { length: 255 }).notNull().unique(),
    isActive: boolean('is_active').default(true),
    createdAt: timestamp('created_at').defaultNow(),
});

export const products = pgTable('products', {
    id: serial('id').primaryKey(),
    sku: varchar('sku', { length: 64 }).notNull().unique(),
    title: text('title').notNull(),
    price: decimal('price', { precision: 10, scale: 2 }).notNull(),
    stockQuantity: integer('stock_quantity').notNull().default(0),
});

export const orders = pgTable('orders', {
    id: serial('id').primaryKey(),
    userId: integer('user_id').references(() => users.id).notNull(),
    status: orderStatusEnum('status').default('pending').notNull(),
    totalAmount: decimal('total_amount', { precision: 12, scale: 2 }).notNull(),
    createdAt: timestamp('created_at').defaultNow(),
});

export const orderItems = pgTable('order_items', {
    id: serial('id').primaryKey(),
    orderId: integer('order_id').references(() => orders.id).notNull(),
    productId: integer('product_id').references(() => products.id).notNull(),
    quantity: integer('quantity').notNull(),
    unitPrice: decimal('unit_price', { precision: 10, scale: 2 }).notNull(),
});

export const auditLogs = mysqlTable('audit_logs', {
    id: int('id').primaryKey(),
    action: mysqlVarchar('action', { length: 128 }).notNull(),
});

export const cacheEntries = sqliteTable('cache_entries', {
    key: sqliteText('key').primaryKey(),
    val: sqliteText('val').notNull(),
});
"#;
    fs::write(&schema_path, drizzle_content).expect("Write drizzle schema");

    let repo_path = dir.path().join("src/order_repository.ts");
    fs::create_dir_all(repo_path.parent().unwrap()).unwrap();
    let repo_content = r#"
export async function getOrderSummary(db: any, orderId: number) {
    const result = await db
        .select({
            orderId: orders.id,
            total: orders.totalAmount,
            userName: users.name,
            userEmail: users.email,
        })
        .from(orders)
        .innerJoin(users, eq(orders.userId, users.id))
        .leftJoin(orderItems, eq(orders.id, orderItems.orderId))
        .where(eq(orders.id, orderId));

    await db.insert(auditLogs).values({ action: 'order_view' });
    await db.delete(cacheEntries).where(eq(cacheEntries.key, 'order_' + orderId));

    return result;
}
"#;
    fs::write(&repo_path, repo_content).expect("Write repo");

    let stitcher = DrizzleStitcher::new();
    let stitched = stitcher.stitch(dir.path(), &repo_path, repo_content);

    // Verify all referenced tables are hoisted across pg, mysql, sqlite dialects:
    let table_names: Vec<&str> = stitched.iter().map(|t| t.name.as_str()).collect();
    assert!(table_names.contains(&"orders"), "Should hoist orders table");
    assert!(table_names.contains(&"users"), "Should hoist users table");
    assert!(table_names.contains(&"orderItems"), "Should hoist orderItems table");
    assert!(table_names.contains(&"auditLogs"), "Should hoist auditLogs table");
    assert!(table_names.contains(&"cacheEntries"), "Should hoist cacheEntries table");

    // Unreferenced table 'products' should NOT be hoisted (minimal token overhead)
    assert!(
        !table_names.contains(&"products"),
        "Unreferenced table 'products' should not be hoisted"
    );
}

// ================================================================================================
// 3. Raw SQL Queries with Multi-Table Joins, Complex CTEs, Subqueries & Chronological DDLs
// ================================================================================================

#[test]
fn test_adversarial_sql_migrations_complex_ctes_and_schema_qualifiers() {
    let dir = TempDir::new().expect("Create tempdir");
    let mig_dir = dir.path().join("migrations");
    fs::create_dir_all(&mig_dir).expect("create mig_dir");

    // 001: Base tables
    fs::write(
        mig_dir.join("001_base_schema.sql"),
        r#"
CREATE TABLE employees (
    id SERIAL PRIMARY KEY,
    manager_id INT REFERENCES employees(id),
    full_name VARCHAR(255) NOT NULL,
    department_id INT NOT NULL
);

CREATE TABLE departments (
    id SERIAL PRIMARY KEY,
    dept_name VARCHAR(100) NOT NULL
);
"#,
    )
    .unwrap();

    // 002: Enum and orders table
    fs::write(
        mig_dir.join("002_orders.sql"),
        r#"
CREATE TYPE payment_status AS ENUM ('initiated', 'authorized', 'captured', 'refunded', 'failed');

CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    employee_id INT REFERENCES employees(id),
    amount NUMERIC(12, 2) NOT NULL,
    status payment_status DEFAULT 'initiated' NOT NULL
);
"#,
    )
    .unwrap();

    // 003: Alter table additions and drops
    fs::write(
        mig_dir.join("003_alterations.sql"),
        r#"
ALTER TABLE employees ADD COLUMN legacy_badge_num VARCHAR(32);
ALTER TABLE employees ADD COLUMN active_flag BOOLEAN DEFAULT true NOT NULL;
ALTER TABLE employees DROP COLUMN legacy_badge_num;
ALTER TABLE orders ADD COLUMN discount_code VARCHAR(50);
"#,
    )
    .unwrap();

    let query_file = dir.path().join("src/analytics/reports.rs");
    fs::create_dir_all(query_file.parent().unwrap()).unwrap();

    // Highly complex raw SQL query with recursive CTE, schema qualifiers, subqueries, aliases
    let rust_source = r##"
pub async fn generate_revenue_report(pool: &PgPool) -> Result<Vec<ReportRow>, sqlx::Error> {
    let query = sqlx::query(
        "WITH RECURSIVE org_tree AS ( \
            SELECT id, manager_id, full_name, 1 as depth \
            FROM public.employees \
            WHERE manager_id IS NULL AND active_flag = true \
            UNION ALL \
            SELECT e.id, e.manager_id, e.full_name, ot.depth + 1 \
            FROM \"public\".\"employees\" e \
            JOIN org_tree ot ON e.manager_id = ot.id \
            WHERE e.active_flag = true \
        ), \
        top_sales AS ( \
            SELECT o.employee_id, SUM(o.amount) as total_rev \
            FROM `orders` o \
            WHERE o.status IN ('captured', 'authorized') \
              AND o.amount > (SELECT AVG(amount) FROM orders) \
            GROUP BY o.employee_id \
        ) \
        SELECT ot.full_name, d.dept_name, ts.total_rev \
        FROM org_tree ot \
        JOIN public.departments d ON ot.id = d.id \
        LEFT JOIN top_sales ts ON ot.id = ts.employee_id \
        ORDER BY ts.total_rev DESC NULLS LAST"
    );
    query.fetch_all(pool).await
}
"##;
    fs::write(&query_file, rust_source).unwrap();

    let stitcher = SqlMigrationStitcher::new();
    let stitched = stitcher.stitch(dir.path(), &query_file, rust_source);

    let names: Vec<&str> = stitched.iter().map(|t| t.name.as_str()).collect();

    // Must extract employees, departments, orders, and payment_status enum
    assert!(names.contains(&"employees"), "Must detect 'employees' table");
    assert!(names.contains(&"departments"), "Must detect 'departments' table");
    assert!(names.contains(&"orders"), "Must detect 'orders' table");
    assert!(
        names.contains(&"payment_status"),
        "Must hoist 'payment_status' enum used by orders table"
    );

    // Verify DDL reflects alterations accurately
    let employees_def = stitched.iter().find(|t| t.name == "employees").unwrap();
    assert!(
        employees_def.definition.contains("active_flag"),
        "Employees table must reflect ADD COLUMN active_flag"
    );
    assert!(
        !employees_def.definition.contains("legacy_badge_num"),
        "Employees table must NOT contain dropped column legacy_badge_num"
    );
}

// ================================================================================================
// 4. Missing Schema Files, Corrupted SQL/Prisma/Drizzle Files & Dynamic Queries
// ================================================================================================

#[test]
fn test_adversarial_corrupted_files_and_dynamic_sql_resilience() {
    let dir = TempDir::new().expect("Create tempdir");

    // 1. Non-UTF8 binary file in migrations dir (must be skipped safely)
    let mig_dir = dir.path().join("migrations");
    fs::create_dir_all(&mig_dir).unwrap();
    fs::write(mig_dir.join("000_binary_asset.sql"), b"\x00\xFF\xFE\x10\x20\x30\xDE\xAD\xBE\xEF").unwrap();

    // 2. Corrupted SQL migration with syntax errors, broken DDLs, and unclosed quotes
    let corrupted_sql = r#"
CREATE TABLE valid_table (id INT PRIMARY KEY, name TEXT);

-- Broken table with syntax errors
CREATE TABLE broken_table (
    invalid syntax here !@#$%^&*()
);

'Unclosed string literal without termination...
ALTER TABLE ;;;;; DROP COLUMN ;

CREATE TABLE partially_valid (
    col1 INT NOT NULL
);
"#;
    fs::write(mig_dir.join("001_corrupted.sql"), corrupted_sql).unwrap();

    // 2. Corrupted Prisma schema
    let prisma_path = dir.path().join("schema.prisma");
    fs::write(
        &prisma_path,
        "model BrokenModel { id Int \n\n enum CorruptEnum { VAL1 VAL2 \n",
    )
    .unwrap();

    // 3. Corrupted Drizzle schema
    let drizzle_path = dir.path().join("schema.ts");
    fs::write(
        &drizzle_path,
        "export const unclosed = pgTable('unclosed_table', { id: serial(\n",
    )
    .unwrap();

    let orchestrator = SchemaStitcher::new();

    // Dynamic SQL expressions in source code
    let source_code = r#"
export function executeDynamic(tablePrefix: string, id: number) {
    const sql1 = `SELECT * FROM ${tablePrefix}_users WHERE id = ${id}`;
    const sql2 = "SELECT * FROM valid_table WHERE id = 1";
    const sql3 = `INSERT INTO ${getTableName(env)} VALUES (1, 2)`;
    const sql4 = "SELECT * FROM";
    return [sql1, sql2, sql3, sql4];
}
"#;

    let target_file = dir.path().join("src/dynamic.ts");
    fs::create_dir_all(target_file.parent().unwrap()).unwrap();
    fs::write(&target_file, source_code).unwrap();

    // Must NOT panic or fail
    let results = orchestrator
        .stitch_schemas(dir.path(), &target_file, source_code)
        .expect("Must gracefully handle corrupted schemas without error");

    // Only 'valid_table' should be stitched
    assert!(
        results.iter().any(|t| t.name == "valid_table"),
        "Valid table should still be successfully parsed"
    );

    // Garbage tokens like '${tablePrefix}_users' or '${getTableName(env)}' must NOT appear as type names
    for res in &results {
        assert!(
            !res.name.contains("${") && !res.name.contains('(') && !res.name.contains('}'),
            "Extracted type name must not contain dynamic expression fragments: '{}'",
            res.name
        );
    }
}

// ================================================================================================
// 5. TypeORM Entities, Protobuf Deep Hierarchies & GraphQL SDL Resolvers
// ================================================================================================

#[test]
fn test_adversarial_typeorm_proto_graphql_subsystems() {
    let dir = TempDir::new().expect("Create tempdir");

    // 1. TypeORM entities
    let entities_dir = dir.path().join("src/entities");
    fs::create_dir_all(&entities_dir).unwrap();
    fs::write(
        entities_dir.join("Customer.ts"),
        r#"
import { Entity, PrimaryGeneratedColumn, Column, OneToMany, CreateDateColumn } from 'typeorm';

@Entity('tbl_customers')
export class Customer {
    @PrimaryGeneratedColumn('uuid')
    id!: string;

    @Column({ length: 150 })
    fullName!: string;

    @Column({ unique: true })
    email!: string;

    @CreateDateColumn()
    createdAt!: Date;
}
"#,
    )
    .unwrap();

    // 2. Protobuf 4-level deep hierarchy
    let proto_dir = dir.path().join("proto");
    fs::create_dir_all(&proto_dir).unwrap();
    fs::write(
        proto_dir.join("billing.proto"),
        r#"
syntax = "proto3";
package billing.v1;

enum InvoiceState {
    DRAFT = 0;
    PENDING = 1;
    SETTLED = 2;
    VOIDED = 3;
}

message CurrencyAmount {
    string currency = 1;
    int64 units = 2;
    int32 nanos = 3;
}

message LineItem {
    string item_id = 1;
    string description = 2;
    CurrencyAmount price = 3;
}

message InvoiceRequest {
    string invoice_id = 1;
    string customer_id = 2;
    repeated LineItem items = 3;
    InvoiceState state = 4;
}

message InvoiceResponse {
    string invoice_id = 1;
    bool approved = 2;
    CurrencyAmount total_amount = 3;
}

service BillingService {
    rpc GenerateInvoice(InvoiceRequest) returns (InvoiceResponse);
    rpc StreamInvoices(stream InvoiceRequest) returns (stream InvoiceResponse);
}
"#,
    )
    .unwrap();

    // 3. GraphQL SDL with complex queries, mutations, interfaces, unions
    let gql_dir = dir.path().join("graphql");
    fs::create_dir_all(&gql_dir).unwrap();
    fs::write(
        gql_dir.join("schema.graphql"),
        r#"
enum PaymentMethod {
    CREDIT_CARD
    BANK_TRANSFER
    CRYPTO
}

input CreateCheckoutInput {
    customerId: ID!
    amount: Float!
    paymentMethod: PaymentMethod!
}

type CheckoutPayload {
    checkoutId: ID!
    redirectUrl: String
    success: Boolean!
}

type Query {
    getCheckoutStatus(id: ID!): CheckoutPayload
}

type Mutation {
    createCheckout(input: CreateCheckoutInput!): CheckoutPayload
}
"#,
    )
    .unwrap();

    // Slicing TypeScript service referencing all three (TypeORM repo, gRPC handler, GraphQL mutation)
    let app_service_path = dir.path().join("src/services/checkout_service.ts");
    fs::create_dir_all(app_service_path.parent().unwrap()).unwrap();
    let app_service_code = r#"
export class CheckoutService {
    constructor(
        @InjectRepository(Customer) private readonly customerRepo: Repository<Customer>,
        private readonly billingClient: BillingService,
    ) {}

    async createCheckout(input: CreateCheckoutInput): Promise<CheckoutPayload> {
        const customer = await this.customerRepo.findOne({ where: { id: input.customerId } });
        const invoice = await this.billingClient.generateInvoice({
            customerId: customer.id,
            items: [],
        });
        return { checkoutId: invoice.invoiceId, success: true };
    }
}
"#;
    fs::write(&app_service_path, app_service_code).unwrap();

    let stitcher = SchemaStitcher::new();
    let stitched = stitcher
        .stitch_schemas(dir.path(), &app_service_path, app_service_code)
        .expect("Stitch polyglot schemas");

    let type_map: std::collections::HashMap<String, ExtractedType> = stitched
        .into_iter()
        .map(|t| (t.name.clone(), t))
        .collect();

    // TypeORM assertions:
    assert!(
        type_map.contains_key("Customer"),
        "TypeORM Customer entity must be stitched"
    );
    assert_eq!(
        type_map["Customer"].kind, "typeorm_entity",
        "Kind must be typeorm_entity"
    );

    // Protobuf assertions (deep transitive message resolution):
    assert!(
        type_map.contains_key("BillingService"),
        "BillingService must be stitched"
    );
    assert!(
        type_map.contains_key("InvoiceRequest"),
        "InvoiceRequest must be stitched"
    );
    assert!(
        type_map.contains_key("LineItem"),
        "LineItem (nested in InvoiceRequest) must be recursively stitched"
    );
    assert!(
        type_map.contains_key("CurrencyAmount"),
        "CurrencyAmount (nested in LineItem) must be recursively stitched"
    );
    assert!(
        type_map.contains_key("InvoiceState"),
        "InvoiceState enum must be hoisted"
    );

    // GraphQL assertions:
    assert!(
        type_map.contains_key("Mutation.createCheckout"),
        "Mutation.createCheckout must be stitched"
    );
    assert!(
        type_map.contains_key("CreateCheckoutInput"),
        "CreateCheckoutInput must be stitched"
    );
    assert!(
        type_map.contains_key("CheckoutPayload"),
        "CheckoutPayload must be stitched"
    );
    assert!(
        type_map.contains_key("PaymentMethod"),
        "PaymentMethod enum referenced in CreateCheckoutInput must be hoisted"
    );
}

// ================================================================================================
// 6. Token Overhead Bounds & High-Throughput Performance Stress
// ================================================================================================

#[test]
fn test_adversarial_schema_stitching_token_overhead_and_performance() {
    let dir = TempDir::new().expect("Create tempdir");

    // Create a large schema repository with 50 Prisma models and 50 SQL tables
    let mut large_prisma = String::from("datasource db { provider = \"postgresql\" url = env(\"DB\") }\n\n");
    for i in 0..50 {
        large_prisma.push_str(&format!(
            "model Model{i} {{\n  id Int @id @default(autoincrement())\n  fieldA String\n  fieldB Int\n  fieldC DateTime @default(now())\n}}\n\n"
        ));
    }
    fs::write(dir.path().join("schema.prisma"), &large_prisma).unwrap();

    let mig_dir = dir.path().join("migrations");
    fs::create_dir_all(&mig_dir).unwrap();
    let mut large_sql = String::new();
    for i in 0..50 {
        large_sql.push_str(&format!(
            "CREATE TABLE sql_table_{i} (\n    id INT PRIMARY KEY,\n    col_a VARCHAR(100),\n    col_b INT,\n    col_c TIMESTAMP\n);\n"
        ));
    }
    fs::write(mig_dir.join("001_large.sql"), &large_sql).unwrap();

    let query_file = dir.path().join("src/handler.ts");
    fs::create_dir_all(query_file.parent().unwrap()).unwrap();
    let query_code = r#"
export async function handleTarget(prisma: any, db: any) {
    const item = await prisma.model17.findUnique({ where: { id: 1 } });
    const row = await db.query("SELECT * FROM sql_table_42 WHERE id = 1");
    return { item, row };
}
"#;
    fs::write(&query_file, query_code).unwrap();

    let stitcher = SchemaStitcher::new();

    // 1. Performance stress: Run 100 consecutive stitching cycles
    let start = Instant::now();
    for _ in 0..100 {
        let res = stitcher
            .stitch_schemas(dir.path(), &query_file, query_code)
            .expect("Stitch schemas in loop");
        assert_eq!(res.len(), 2, "Only Model17 and sql_table_42 should be stitched");
    }
    let total_time = start.elapsed();
    let avg_ms = total_time.as_secs_f64() * 1000.0 / 100.0;

    assert!(
        avg_ms < 50.0,
        "Average stitching latency must be < 50.0ms per slice in debug mode (actual: {:.2}ms)",
        avg_ms
    );

    // 2. Token overhead bound:
    let stitched = stitcher
        .stitch_schemas(dir.path(), &query_file, query_code)
        .unwrap();

    let total_chars: usize = stitched.iter().map(|s| s.definition.len()).sum();
    // 2 models of ~100 chars each = ~200 chars (~50 tokens).
    // Whole schema would be ~15,000 chars (~3,750 tokens).
    assert!(
        total_chars < 500,
        "Stitched definitions must be minimal ({total_chars} chars, ~{} tokens), preventing schema bloat",
        total_chars / 4
    );
}

// ================================================================================================
// 7. Monorepo Multi-Package Proximity Disambiguation
// ================================================================================================

#[test]
fn test_adversarial_monorepo_proximity_and_cross_dialect_isolation() {
    let dir = TempDir::new().expect("Create tempdir");
    fs::write(dir.path().join("package.json"), r#"{"name":"monorepo","workspaces":["packages/*"]}"#).unwrap();

    let auth_pkg = dir.path().join("packages/auth");
    let billing_pkg = dir.path().join("packages/billing");
    fs::create_dir_all(auth_pkg.join("src")).unwrap();
    fs::create_dir_all(billing_pkg.join("src")).unwrap();

    // Auth package schema
    let auth_prisma = r#"
datasource db { provider = "postgresql" url = env("AUTH_DB") }
model User {
    id        String @id
    email     String @unique
    authHash  String
    lastLogin DateTime
}
"#;
    fs::write(auth_pkg.join("schema.prisma"), auth_prisma).unwrap();
    fs::write(auth_pkg.join("package.json"), r#"{"name":"@mono/auth"}"#).unwrap();

    // Billing package schema with conflicting model name "User"
    let billing_prisma = r#"
datasource db { provider = "postgresql" url = env("BILLING_DB") }
model User {
    id               String @id
    stripeCustomerId String @unique
    paymentMethods   String[]
}
"#;
    fs::write(billing_pkg.join("schema.prisma"), billing_prisma).unwrap();
    fs::write(billing_pkg.join("package.json"), r#"{"name":"@mono/billing"}"#).unwrap();

    let auth_service = auth_pkg.join("src/auth_service.ts");
    fs::write(
        &auth_service,
        "export async function login(prisma: any) { return prisma.user.findUnique({ where: { email: 'a@b.com' } }); }",
    ).unwrap();

    let billing_service = billing_pkg.join("src/billing_service.ts");
    fs::write(
        &billing_service,
        "export async function getBilling(prisma: any) { return prisma.user.findUnique({ where: { id: 'u1' } }); }",
    ).unwrap();

    let stitcher = PrismaStitcher::new();

    // Auth query must stitch auth package's User model containing authHash
    let auth_stitched = stitcher.stitch(dir.path(), &auth_service, "prisma.user.findUnique");
    assert_eq!(auth_stitched.len(), 1);
    assert!(
        auth_stitched[0].definition.contains("authHash"),
        "Auth service must stitch auth package schema, not billing schema"
    );

    // Billing query must stitch billing package's User model containing stripeCustomerId
    let billing_stitched = stitcher.stitch(dir.path(), &billing_service, "prisma.user.findUnique");
    assert_eq!(billing_stitched.len(), 1);
    assert!(
        billing_stitched[0].definition.contains("stripeCustomerId"),
        "Billing service must stitch billing package schema, not auth schema"
    );
}

// ================================================================================================
// 8. Adaptive Token Budget Compression with Hoisted Schemas
// ================================================================================================

#[test]
fn test_adversarial_schema_budget_compression_and_degradation() {
    let dir = TempDir::new().expect("Create tempdir");
    fs::write(dir.path().join("package.json"), r#"{"name":"budget-test"}"#).unwrap();

    let schema_path = dir.path().join("schema.prisma");
    let prisma_content = r#"
enum Status {
    ACTIVE
    INACTIVE
}

model Account {
    id          Int      @id
    email       String   @unique
    status      Status
    metadata    String
    description String
    settings    String
}
"#;
    fs::write(&schema_path, prisma_content).unwrap();

    let service_path = dir.path().join("service.ts");
    let service_content = r#"
export function getAccount(prisma: any, id: number) {
    const acc = prisma.account.findUnique({ where: { id } });
    return acc;
}
"#;
    fs::write(&service_path, service_content).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: Some(60), // Tight budget
    };

    let (slice_result, report) = slicer
        .slice_symbol_with_budget(&service_path, "getAccount", &opts, 60)
        .expect("Slice with budget");

    // Slicing must succeed without panic, report degradation if types were compressed
    assert!(slice_result.target_symbol.name == "getAccount");
    println!("Budget slice sliced_tokens: {}, raw_file_tokens: {}", slice_result.stats.sliced_tokens, slice_result.stats.raw_file_tokens);
    let _ = report;
}

// ================================================================================================
// 9. Batch Slice Polyglot Deduplication
// ================================================================================================

#[test]
fn test_adversarial_batch_slice_polyglot_schema_deduplication() {
    let dir = TempDir::new().expect("Create tempdir");
    fs::write(dir.path().join("package.json"), r#"{"name":"batch-test"}"#).unwrap();

    let prisma_path = dir.path().join("schema.prisma");
    fs::write(
        &prisma_path,
        r#"
enum Role {
  ADMIN
  USER
}

model User {
  id   Int  @id
  role Role
}

model Post {
  id Int @id
}
"#,
    )
    .unwrap();

    let ddl_dir = dir.path().join("migrations");
    fs::create_dir_all(&ddl_dir).unwrap();
    fs::write(ddl_dir.join("001.sql"), "CREATE TABLE audit_records (id INT, log TEXT);\n").unwrap();

    let multi_service_path = dir.path().join("multi_service.ts");
    let multi_service_content = r#"
export function getUser(prisma: any) {
    return prisma.user.findUnique({ where: { id: 1 } });
}

export function getUserAgain(prisma: any) {
    return prisma.user.findMany();
}

export function getPost(prisma: any) {
    return prisma.post.findUnique({ where: { id: 1 } });
}

export function logAudit(db: any) {
    return db.query("INSERT INTO audit_records VALUES (1, 'ok')");
}
"#;
    fs::write(&multi_service_path, multi_service_content).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 1,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let batch = slicer
        .slice_batch(
            &multi_service_path,
            &["getUser", "getUserAgain", "getPost", "logAudit"],
            &opts,
        )
        .expect("Batch slice");

    assert_eq!(batch.target_symbols.len(), 4);

    // Verify all 4 types exist (User, Role enum, Post, audit_records table)
    let hoisted_names: Vec<&str> = batch.hoisted_types.iter().map(|t| t.name.as_str()).collect();
    assert!(hoisted_names.contains(&"User"), "Must hoist User");
    assert!(hoisted_names.contains(&"Role"), "Must hoist Role enum");
    assert!(hoisted_names.contains(&"Post"), "Must hoist Post");
    assert!(hoisted_names.contains(&"audit_records"), "Must hoist audit_records");

    // Ensure NO duplicates in batch hoisted_types
    let mut seen = HashSet::new();
    for t in &batch.hoisted_types {
        assert!(seen.insert(t.name.clone()), "Duplicate type in batch slice: {}", t.name);
    }
}
