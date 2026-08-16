/**
 * Production-grade OrderService microservice implementation.
 */

import {
    Customer,
    Order,
    OrderCreationRequest,
    OrderItem,
    OrderStatus,
    PaymentMethod,
    PaymentTransaction,
    RefundReason,
    RefundResponse,
    RefundResult,
    TaxCalculationResult,
} from "./models";
import {
    InsufficientInventoryError,
    InvalidRefundStateError,
    OrderNotFoundError,
    PaymentDeclinedError,
} from "./errors";
import { EmailNotifier, InventoryGateway, StripeGateway, TaxJarGateway } from "./gateways";

export interface IOrderRepository {
    findById(orderId: string): Promise<Order | null>;
    save(order: Order): Promise<Order>;
    updateStatus(orderId: string, status: OrderStatus): Promise<void>;
}

export class OrderService {
    constructor(
        private readonly repository: IOrderRepository,
        private readonly stripeGateway: StripeGateway,
        private readonly taxJarGateway: TaxJarGateway,
        private readonly inventoryGateway: InventoryGateway,
        private readonly emailNotifier: EmailNotifier
    ) {}

    public async processOrder(request: OrderCreationRequest, customer: Customer): Promise<Order> {
        for (const reqItem of request.items) {
            const hasStock = await this.inventoryGateway.checkAvailability(reqItem.sku, reqItem.quantity);
            if (!hasStock) {
                throw new InsufficientInventoryError(reqItem.sku, reqItem.quantity, 0);
            }
        }

        const items: OrderItem[] = request.items.map((it, idx) => ({
            id: `item_${idx + 1}`,
            sku: it.sku,
            title: `Product ${it.sku}`,
            unitPriceCents: 2500,
            quantity: it.quantity,
            discountCents: 0,
            taxCents: 0,
            totalCents: 2500 * it.quantity,
        }));

        const shippingFeeCents = customer.isVip ? 0 : 500;
        const taxResult = await this.calculateTax(customer, items, shippingFeeCents);
        const subtotalCents = items.reduce((sum, item) => sum + item.totalCents, 0);
        const totalAmountCents = subtotalCents + taxResult.taxAmountCents + shippingFeeCents;

        for (const reqItem of request.items) {
            await this.inventoryGateway.reserveStock(reqItem.sku, reqItem.quantity);
        }

        let paymentTransaction: PaymentTransaction;
        try {
            paymentTransaction = await this.stripeGateway.chargeCard(
                totalAmountCents,
                "USD",
                request.paymentToken,
                { customerId: customer.id }
            );
        } catch (err: unknown) {
            for (const reqItem of request.items) {
                await this.inventoryGateway.releaseStock(reqItem.sku, reqItem.quantity);
            }
            throw err;
        }

        const newOrder: Order = {
            id: `ord_${Date.now()}`,
            orderNumber: `ORD-${Math.floor(100000 + Math.random() * 900000)}`,
            customerId: customer.id,
            customer,
            items,
            status: OrderStatus.PAID,
            subtotalCents,
            taxCents: taxResult.taxAmountCents,
            shippingFeeCents,
            discountTotalCents: 0,
            totalAmountCents,
            currency: "USD",
            transactions: [paymentTransaction],
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        };

        const savedOrder = await this.repository.save(newOrder);
        await this.emailNotifier.sendOrderConfirmation(customer, savedOrder.orderNumber, savedOrder.totalAmountCents);

        return savedOrder;
    }

    public async processRefund(orderId: string, reason: RefundReason, amountCents?: number): Promise<RefundResponse> {
        const order = await this.repository.findById(orderId);
        if (!order) {
            throw new OrderNotFoundError(orderId);
        }

        if (order.status !== OrderStatus.PAID && order.status !== OrderStatus.DELIVERED && order.status !== OrderStatus.PARTIALLY_REFUNDED) {
            throw new InvalidRefundStateError(orderId, order.status);
        }

        const successfulCharge = order.transactions.find(
            (tx) => tx.status === "SUCCEEDED" && tx.amountCents > 0
        );

        if (!successfulCharge) {
            throw new PaymentDeclinedError("none", "NO_PRIOR_CHARGE", "No successful charge found on order to refund");
        }

        const refundTargetAmount = amountCents ?? order.totalAmountCents;
        const stripeRefund = await this.stripeGateway.executeRefund(
            successfulCharge.transactionId,
            refundTargetAmount,
            reason
        );

        const isFullRefund = refundTargetAmount >= order.totalAmountCents;
        const newStatus = isFullRefund ? OrderStatus.REFUNDED : OrderStatus.PARTIALLY_REFUNDED;

        order.status = newStatus;
        order.updatedAt = new Date().toISOString();
        order.transactions.push({
            transactionId: stripeRefund.refundId,
            gateway: "Stripe",
            paymentMethod: successfulCharge.paymentMethod,
            amountCents: -refundTargetAmount,
            currency: order.currency,
            status: "REFUNDED",
            createdAt: new Date().toISOString(),
            metadata: { reason, refundId: stripeRefund.refundId },
        });

        await this.repository.save(order);

        for (const item of order.items) {
            await this.inventoryGateway.releaseStock(item.sku, item.quantity);
        }

        await this.emailNotifier.sendRefundNotification(order.customer, order.orderNumber, refundTargetAmount);

        return {
            refundId: stripeRefund.refundId,
            orderId: order.id,
            result: RefundResult.SUCCESS,
            refundedAmountCents: refundTargetAmount,
            remainingOrderBalanceCents: Math.max(0, order.totalAmountCents - refundTargetAmount),
            reason,
            processedAt: new Date().toISOString(),
        };
    }

    public async cancelOrder(orderId: string, customerId: string): Promise<Order> {
        const order = await this.repository.findById(orderId);
        if (!order) {
            throw new OrderNotFoundError(orderId);
        }

        if (order.customerId !== customerId) {
            throw new Error("Unauthorized to cancel this order");
        }

        if (order.status !== OrderStatus.PENDING_PAYMENT && order.status !== OrderStatus.PAID) {
            throw new InvalidRefundStateError(orderId, order.status);
        }

        if (order.status === OrderStatus.PAID) {
            await this.processRefund(orderId, RefundReason.CUSTOMER_REQUEST);
        }

        order.status = OrderStatus.CANCELLED;
        order.updatedAt = new Date().toISOString();
        return this.repository.save(order);
    }

    public async calculateTax(
        customer: Customer,
        items: OrderItem[],
        shippingFeeCents: number
    ): Promise<TaxCalculationResult> {
        return this.taxJarGateway.calculateSalesTax(customer.shippingAddress, items, shippingFeeCents);
    }
}
