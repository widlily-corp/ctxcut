/**
 * Data models, interfaces, and enums for the realistic order service.
 */

export enum OrderStatus {
    DRAFT = "DRAFT",
    PENDING_PAYMENT = "PENDING_PAYMENT",
    PAID = "PAID",
    PROCESSING = "PROCESSING",
    SHIPPED = "SHIPPED",
    DELIVERED = "DELIVERED",
    CANCELLED = "CANCELLED",
    REFUNDED = "REFUNDED",
    PARTIALLY_REFUNDED = "PARTIALLY_REFUNDED",
}

export enum RefundReason {
    CUSTOMER_REQUEST = "CUSTOMER_REQUEST",
    DEFECTIVE_PRODUCT = "DEFECTIVE_PRODUCT",
    ITEM_NOT_RECEIVED = "ITEM_NOT_RECEIVED",
    FRAUDULENT_ORDER = "FRAUDULENT_ORDER",
    OUT_OF_STOCK = "OUT_OF_STOCK",
}

export enum PaymentMethod {
    CREDIT_CARD = "CREDIT_CARD",
    DEBIT_CARD = "DEBIT_CARD",
    PAYPAL = "PAYPAL",
    APPLE_PAY = "APPLE_PAY",
    BANK_TRANSFER = "BANK_TRANSFER",
}

export enum RefundResult {
    SUCCESS = "SUCCESS",
    PARTIAL = "PARTIAL",
    DECLINED = "DECLINED",
    PENDING_MANUAL_REVIEW = "PENDING_MANUAL_REVIEW",
}

export interface Address {
    streetLine1: string;
    streetLine2?: string;
    city: string;
    stateOrProvince: string;
    postalCode: string;
    countryCode: string;
}

export interface Customer {
    id: string;
    email: string;
    fullName: string;
    phoneNumber?: string;
    shippingAddress: Address;
    billingAddress: Address;
    isVip: boolean;
}

export interface OrderItem {
    id: string;
    sku: string;
    title: string;
    unitPriceCents: number;
    quantity: number;
    discountCents: number;
    taxCents: number;
    totalCents: number;
}

export interface PaymentTransaction {
    transactionId: string;
    gateway: string;
    paymentMethod: PaymentMethod;
    amountCents: number;
    currency: string;
    status: "SUCCEEDED" | "PENDING" | "FAILED" | "REFUNDED";
    createdAt: string;
    metadata: Record<string, string>;
}

export interface Order {
    id: string;
    orderNumber: string;
    customerId: string;
    customer: Customer;
    items: OrderItem[];
    status: OrderStatus;
    subtotalCents: number;
    taxCents: number;
    shippingFeeCents: number;
    discountTotalCents: number;
    totalAmountCents: number;
    currency: string;
    transactions: PaymentTransaction[];
    createdAt: string;
    updatedAt: string;
}

export interface OrderCreationRequest {
    customerId: string;
    items: Array<{ sku: string; quantity: number }>;
    paymentMethod: PaymentMethod;
    paymentToken: string;
    shippingMethodId: string;
    promoCode?: string;
}

export interface TaxCalculationResult {
    taxableAmountCents: number;
    rate: number;
    taxAmountCents: number;
    breakdown: Array<{
        jurisdiction: string;
        rate: number;
        amountCents: number;
    }>;
}

export interface RefundResponse {
    refundId: string;
    orderId: string;
    result: RefundResult;
    refundedAmountCents: number;
    remainingOrderBalanceCents: number;
    reason: RefundReason;
    processedAt: string;
}
