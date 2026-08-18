/**
 * Unit and integration tests for OrderService.
 */

import { OrderService, IOrderRepository } from "./order_service";
import { Customer, Order, OrderCreationRequest, OrderStatus, RefundReason, RefundResult } from "./models";
import { EmailNotifier, InventoryGateway, StripeGateway, TaxJarGateway } from "./gateways";

describe("OrderService", () => {
    let repository: jest.Mocked<IOrderRepository>;
    let stripeGateway: jest.Mocked<StripeGateway>;
    let taxJarGateway: jest.Mocked<TaxJarGateway>;
    let inventoryGateway: jest.Mocked<InventoryGateway>;
    let emailNotifier: jest.Mocked<EmailNotifier>;
    let service: OrderService;

    const mockCustomer: Customer = {
        id: "cust_123",
        email: "buyer@example.com",
        firstName: "Jane",
        lastName: "Doe",
        isVip: false,
        shippingAddress: {
            street: "123 Market St",
            city: "San Francisco",
            state: "CA",
            zipCode: "94105",
            country: "USA",
        },
    };

    beforeEach(() => {
        repository = {
            findById: jest.fn(),
            save: jest.fn().mockImplementation((order) => Promise.resolve(order)),
            updateStatus: jest.fn().mockResolvedValue(undefined),
        } as any;

        stripeGateway = {
            chargeCard: jest.fn().mockResolvedValue({
                transactionId: "ch_mock_123",
                gateway: "Stripe",
                paymentMethod: "CREDIT_CARD",
                amountCents: 5425,
                currency: "USD",
                status: "SUCCEEDED",
                createdAt: new Date().toISOString(),
                metadata: {},
            }),
            executeRefund: jest.fn().mockResolvedValue({
                refundId: "re_mock_456",
                amountCents: 5425,
                status: "succeeded",
            }),
        } as any;

        taxJarGateway = {
            calculateSalesTax: jest.fn().mockResolvedValue({
                taxAmountCents: 425,
                taxRate: 0.085,
                jurisdiction: "CA",
                breakdown: [],
            }),
        } as any;

        inventoryGateway = {
            checkAvailability: jest.fn().mockResolvedValue(true),
            reserveStock: jest.fn().mockResolvedValue(true),
            releaseStock: jest.fn().mockResolvedValue(true),
        } as any;

        emailNotifier = {
            sendOrderConfirmation: jest.fn().mockResolvedValue(true),
            sendRefundNotification: jest.fn().mockResolvedValue(true),
        } as any;

        service = new OrderService(
            repository,
            stripeGateway,
            taxJarGateway,
            inventoryGateway,
            emailNotifier
        );
    });

    it("should process a new order successfully", async () => {
        const request: OrderCreationRequest = {
            customerId: "cust_123",
            items: [{ sku: "SKU-ABC", quantity: 2 }],
            paymentToken: "tok_visa",
        };

        const order = await service.processOrder(request, mockCustomer);

        expect(order.status).toBe(OrderStatus.PAID);
        expect(order.items.length).toBe(1);
        expect(inventoryGateway.reserveStock).toHaveBeenCalledWith("SKU-ABC", 2);
        expect(stripeGateway.chargeCard).toHaveBeenCalled();
        expect(emailNotifier.sendOrderConfirmation).toHaveBeenCalled();
        expect(repository.save).toHaveBeenCalled();
    });

    it("should process a valid refund for paid order", async () => {
        const existingOrder: Order = {
            id: "ord_100",
            orderNumber: "ORD-100",
            customerId: "cust_123",
            customer: mockCustomer,
            items: [{ id: "item_1", sku: "SKU-ABC", title: "Product", unitPriceCents: 2500, quantity: 2, discountCents: 0, taxCents: 0, totalCents: 5000 }],
            status: OrderStatus.PAID,
            subtotalCents: 5000,
            taxCents: 425,
            shippingFeeCents: 500,
            discountTotalCents: 0,
            totalAmountCents: 5925,
            currency: "USD",
            transactions: [
                {
                    transactionId: "ch_123",
                    gateway: "Stripe",
                    paymentMethod: "CREDIT_CARD",
                    amountCents: 5925,
                    currency: "USD",
                    status: "SUCCEEDED",
                    createdAt: new Date().toISOString(),
                    metadata: {},
                },
            ],
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        };

        repository.findById.mockResolvedValue(existingOrder);

        const refund = await service.processRefund("ord_100", RefundReason.CUSTOMER_REQUEST);

        expect(refund.result).toBe(RefundResult.SUCCESS);
        expect(refund.refundedAmountCents).toBe(5925);
        expect(stripeGateway.executeRefund).toHaveBeenCalledWith("ch_123", 5925, RefundReason.CUSTOMER_REQUEST);
        expect(inventoryGateway.releaseStock).toHaveBeenCalledWith("SKU-ABC", 2);
    });
});
