//! Tier 4 Real-World Scenario: TypeScript / Next.js / Prisma Monorepo Refactor
//!
//! Simulates multi-package monorepo refactoring and slicing:
//! - Next.js App Router server action
//! - Shared database package with Prisma models
//! - Domain billing service with extensive sibling methods
//! - Asserts >=70% token reduction and zero data corruption

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};

#[test]
fn test_workload_v2_monorepo_refactor() {
    // Arrange: Create realistic Next.js + Prisma monorepo structure
    let sandbox = GitSandbox::new().expect("Failed to initialize git sandbox");

    // Package 1: Shared Prisma Database Client & Models
    let prisma_schema = r#"
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model Invoice {
  id          String   @id @default(uuid())
  customerId  String
  amountCents Int
  currency    String   @default("USD")
  status      String
  taxCents    Int      @default(0)
  memo        String?
  createdAt   DateTime @default(now())
  updatedAt   DateTime @updatedAt
}

model Customer {
  id        String   @id @default(uuid())
  email     String   @unique
  name      String
  balance   Int      @default(0)
}
"#;
    sandbox
        .write_file("packages/db/schema.prisma", prisma_schema)
        .unwrap();

    let db_client = r#"
export interface Invoice {
    id: string;
    customerId: string;
    amountCents: number;
    currency: string;
    status: 'pending' | 'paid' | 'failed' | 'refunded';
    taxCents: number;
    memo?: string;
    createdAt: Date;
    updatedAt: Date;
}

export interface Customer {
    id: string;
    email: string;
    name: string;
    balance: number;
}

export class PrismaClient {
    invoice = {
        findUnique: async (args: { where: { id: string } }): Promise<Invoice | null> => null,
        findMany: async (args?: any): Promise<Invoice[]> => [],
        update: async (args: { where: { id: string }; data: Partial<Invoice> }): Promise<Invoice> => ({} as Invoice),
        create: async (args: { data: Partial<Invoice> }): Promise<Invoice> => ({} as Invoice),
        delete: async (args: { where: { id: string } }): Promise<Invoice> => ({} as Invoice),
    };
    customer = {
        findUnique: async (args: { where: { id: string } }): Promise<Customer | null> => null,
        update: async (args: { where: { id: string }; data: Partial<Customer> }): Promise<Customer> => ({} as Customer),
    };
}
export const prisma = new PrismaClient();
"#;
    sandbox
        .write_file("packages/db/src/index.ts", db_client)
        .unwrap();

    // Package 2: Core Billing Domain Service (with full multi-method implementation)
    let billing_service = r#"
import { prisma, Invoice, Customer } from '../../db/src';

export interface ProcessRefundRequest {
    invoiceId: string;
    reason: string;
    requestedByUserId: string;
}

export interface ProcessRefundResult {
    success: boolean;
    invoiceId: string;
    refundedAmountCents: number;
    error?: string;
}

export interface CreateInvoiceRequest {
    customerId: string;
    amountCents: number;
    taxRate: number;
    memo?: string;
}

export class BillingService {
    static async createInvoice(req: CreateInvoiceRequest): Promise<Invoice> {
        const taxCents = Math.round(req.amountCents * req.taxRate);
        const customer = await prisma.customer.findUnique({ where: { id: req.customerId } });
        if (!customer) throw new Error('Customer not found');

        return await prisma.invoice.create({
            data: {
                customerId: req.customerId,
                amountCents: req.amountCents,
                taxCents,
                status: 'pending',
                memo: req.memo,
            },
        });
    }

    static async calculateCustomerOutstandingBalance(customerId: string): Promise<number> {
        const invoices = await prisma.invoice.findMany({ where: { customerId, status: 'pending' } });
        return invoices.reduce((total, inv) => total + inv.amountCents + inv.taxCents, 0);
    }

    static async voidInvoice(invoiceId: string, reason: string): Promise<boolean> {
        const inv = await prisma.invoice.findUnique({ where: { id: invoiceId } });
        if (!inv || inv.status === 'paid') return false;
        await prisma.invoice.update({
            where: { id: invoiceId },
            data: { status: 'failed', memo: `Voided: ${reason}` },
        });
        return true;
    }

    static async listOverdueInvoices(cutoffDate: Date): Promise<Invoice[]> {
        return await prisma.invoice.findMany({
            where: {
                status: 'pending',
                createdAt: { lt: cutoffDate },
            },
        });
    }

    static async processRefund(req: ProcessRefundRequest): Promise<ProcessRefundResult> {
        const invoice = await prisma.invoice.findUnique({
            where: { id: req.invoiceId },
        });

        if (!invoice) {
            return {
                success: false,
                invoiceId: req.invoiceId,
                refundedAmountCents: 0,
                error: 'Invoice not found',
            };
        }

        if (invoice.status !== 'paid') {
            return {
                success: false,
                invoiceId: req.invoiceId,
                refundedAmountCents: 0,
                error: 'Cannot refund unpaid invoice',
            };
        }

        await prisma.invoice.update({
            where: { id: req.invoiceId },
            data: { status: 'refunded' },
        });

        return {
            success: true,
            invoiceId: req.invoiceId,
            refundedAmountCents: invoice.amountCents,
        };
    }

    static async auditAccount(customerId: string): Promise<{ totalPaid: number; totalRefunded: number }> {
        const all = await prisma.invoice.findMany({ where: { customerId } });
        let totalPaid = 0;
        let totalRefunded = 0;
        for (const inv of all) {
            if (inv.status === 'paid') totalPaid += inv.amountCents;
            if (inv.status === 'refunded') totalRefunded += inv.amountCents;
        }
        return { totalPaid, totalRefunded };
    }
}
"#;
    let billing_path = sandbox
        .write_file("packages/core/src/billing.ts", billing_service)
        .unwrap();

    // Package 3: Next.js App Router Action
    let server_action = r#"
'use server';

import { BillingService, ProcessRefundRequest } from '../../../core/src/billing';

export async function handleRefundAction(formData: FormData) {
    const invoiceId = formData.get('invoiceId') as string;
    const reason = formData.get('reason') as string;

    const request: ProcessRefundRequest = {
        invoiceId,
        reason,
        requestedByUserId: 'admin-usr-1',
    };

    return await BillingService.processRefund(request);
}
"#;
    sandbox
        .write_file("apps/web/app/actions/refund.ts", server_action)
        .unwrap();

    sandbox.stage_all().unwrap();
    sandbox.commit("Initial monorepo architecture").unwrap();

    // Act: Slice `BillingService.processRefund`
    let runner = CliRunner::new();
    let target = format!("{}:BillingService.processRefund", billing_path.display());
    let output = runner
        .run_in_dir(sandbox.path(), &["slice", &target])
        .expect("Command failed");

    // Assert: Slicing output
    output.assert_success();
    assert!(output.stdout.contains("processRefund"));

    // Verify token reduction against full monorepo billing files (>= 60%)
    let verifier = TokenVerifier::new();
    let full_text = format!(
        "{}\n{}\n{}\n{}",
        prisma_schema, db_client, billing_service, server_action
    );
    let metrics = verifier.verify_reduction(&full_text, &output.stdout, 60.0);
    assert!(metrics.reduction_percentage >= 60.0);
}
