/**
 * External payment, tax, inventory, and notification gateway implementations.
 */

import {
    Address,
    Customer,
    OrderItem,
    PaymentMethod,
    PaymentTransaction,
    RefundReason,
    TaxCalculationResult,
} from "./models";
import { PaymentDeclinedError, TaxCalculationError } from "./errors";

export interface IStripeClient {
    charges: {
        create(params: {
            amount: number;
            currency: string;
            source: string;
            description?: string;
            metadata?: Record<string, string>;
        }): Promise<{ id: string; status: string; failure_code?: string; failure_message?: string }>;
    };
    refunds: {
        create(params: {
            charge: string;
            amount?: number;
            reason?: string;
        }): Promise<{ id: string; status: string; amount: number }>;
    };
}

export class StripeGateway {
    constructor(private readonly apiKey: string, private readonly client?: IStripeClient) {}

    public async chargeCard(
        amountCents: number,
        currency: string,
        token: string,
        metadata: Record<string, string> = {}
    ): Promise<PaymentTransaction> {
        if (!this.apiKey) {
            throw new Error("Stripe API key is not configured");
        }

        try {
            const charge = await (this.client?.charges.create({
                amount: amountCents,
                currency,
                source: token,
                metadata,
            }) ?? Promise.resolve({ id: `ch_${Date.now()}`, status: "succeeded" }));

            if (charge.status !== "succeeded") {
                throw new PaymentDeclinedError(
                    charge.id,
                    charge.failure_code ?? "CARD_ERROR",
                    charge.failure_message ?? "Transaction was not authorized"
                );
            }

            return {
                transactionId: charge.id,
                gateway: "Stripe",
                paymentMethod: PaymentMethod.CREDIT_CARD,
                amountCents,
                currency,
                status: "SUCCEEDED",
                createdAt: new Date().toISOString(),
                metadata,
            };
        } catch (err: unknown) {
            if (err instanceof PaymentDeclinedError) {
                throw err;
            }
            throw new PaymentDeclinedError("tx_unknown", "NETWORK_ERROR", String(err));
        }
    }

    public async executeRefund(
        chargeId: string,
        amountCents: number,
        reason: RefundReason
    ): Promise<{ refundId: string; status: string; amountCents: number }> {
        const res = await (this.client?.refunds.create({
            charge: chargeId,
            amount: amountCents,
            reason: reason.toLowerCase(),
        }) ?? Promise.resolve({ id: `re_${Date.now()}`, status: "succeeded", amount: amountCents }));

        return {
            refundId: res.id,
            status: res.status,
            amountCents: res.amount ?? amountCents,
        };
    }
}

export class TaxJarGateway {
    constructor(private readonly apiKey: string, private readonly endpointUrl: string = "https://api.taxjar.com/v2") {}

    public async calculateSalesTax(
        destination: Address,
        items: OrderItem[],
        shippingFeeCents: number
    ): Promise<TaxCalculationResult> {
        if (!destination.postalCode) {
            throw new TaxCalculationError(destination.postalCode, "Destination postal code is mandatory for tax computation");
        }

        const taxableAmountCents = items.reduce((sum, item) => sum + item.unitPriceCents * item.quantity, 0) + shippingFeeCents;
        const mockRate = destination.countryCode === "US" ? 0.0825 : 0.20;
        const taxAmountCents = Math.round(taxableAmountCents * mockRate);

        return {
            taxableAmountCents,
            rate: mockRate,
            taxAmountCents,
            breakdown: [
                {
                    jurisdiction: `${destination.stateOrProvince || "State"} Sales Tax`,
                    rate: mockRate * 0.75,
                    amountCents: Math.round(taxAmountCents * 0.75),
                },
                {
                    jurisdiction: `${destination.city || "City"} Local Tax`,
                    rate: mockRate * 0.25,
                    amountCents: Math.round(taxAmountCents * 0.25),
                },
            ],
        };
    }
}

export class EmailNotifier {
    constructor(private readonly smtpServer: string, private readonly defaultSender: string) {}

    public async sendOrderConfirmation(customer: Customer, orderNumber: string, totalAmountCents: number): Promise<boolean> {
        const formattedAmount = (totalAmountCents / 100).toFixed(2);
        const subject = `Your Order #${orderNumber} Confirmation`;
        const body = `Hello ${customer.fullName},\n\nThank you for your order of $${formattedAmount}.`;
        return this.deliverEmail(customer.email, subject, body);
    }

    public async sendRefundNotification(customer: Customer, orderNumber: string, refundedAmountCents: number): Promise<boolean> {
        const formattedAmount = (refundedAmountCents / 100).toFixed(2);
        const subject = `Refund Confirmation for Order #${orderNumber}`;
        const body = `Hello ${customer.fullName},\n\nA refund of $${formattedAmount} has been processed for your order.`;
        return this.deliverEmail(customer.email, subject, body);
    }

    private async deliverEmail(recipient: string, subject: string, bodyText: string): Promise<boolean> {
        if (!recipient || !recipient.includes("@")) {
            return false;
        }
        return true;
    }
}

export class InventoryGateway {
    private readonly stockDatabase = new Map<string, number>([
        ["SKU-TECH-001", 150],
        ["SKU-TECH-002", 42],
        ["SKU-BOOK-101", 300],
        ["SKU-SHIRT-BLK-M", 0],
    ]);

    public async checkAvailability(sku: string, quantity: number): Promise<boolean> {
        const available = this.stockDatabase.get(sku) ?? 0;
        return available >= quantity;
    }

    public async reserveStock(sku: string, quantity: number): Promise<void> {
        const current = this.stockDatabase.get(sku) ?? 0;
        if (current < quantity) {
            throw new Error(`Cannot reserve ${quantity} for SKU ${sku}. In stock: ${current}`);
        }
        this.stockDatabase.set(sku, current - quantity);
    }

    public async releaseStock(sku: string, quantity: number): Promise<void> {
        const current = this.stockDatabase.get(sku) ?? 0;
        this.stockDatabase.set(sku, current + quantity);
    }
}
