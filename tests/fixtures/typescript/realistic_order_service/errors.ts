/**
 * Domain errors for the realistic order service.
 */

export abstract class OrderDomainError extends Error {
    public abstract readonly errorCode: string;
    public readonly timestamp: number;

    protected constructor(message: string, public readonly context?: Record<string, unknown>) {
        super(message);
        this.name = this.constructor.name;
        this.timestamp = Date.now();
        Object.setPrototypeOf(this, new.target.prototype);
    }
}

export class InsufficientInventoryError extends OrderDomainError {
    public readonly errorCode = "INSUFFICIENT_INVENTORY";

    constructor(
        public readonly sku: string,
        public readonly requestedQuantity: number,
        public readonly availableQuantity: number
    ) {
        super(`Insufficient inventory for SKU '${sku}': requested ${requestedQuantity}, available ${availableQuantity}`, {
            sku,
            requestedQuantity,
            availableQuantity,
        });
    }
}

export class PaymentDeclinedError extends OrderDomainError {
    public readonly errorCode = "PAYMENT_DECLINED";

    constructor(
        public readonly transactionId: string,
        public readonly declineCode: string,
        public readonly declineReason: string
    ) {
        super(`Payment declined (code: ${declineCode}): ${declineReason}`, {
            transactionId,
            declineCode,
            declineReason,
        });
    }
}

export class OrderNotFoundError extends OrderDomainError {
    public readonly errorCode = "ORDER_NOT_FOUND";

    constructor(public readonly orderId: string) {
        super(`Order with ID '${orderId}' was not found in the repository`, { orderId });
    }
}

export class InvalidRefundStateError extends OrderDomainError {
    public readonly errorCode = "INVALID_REFUND_STATE";

    constructor(public readonly orderId: string, public readonly currentStatus: string) {
        super(`Cannot process refund for order '${orderId}' with status '${currentStatus}'`, {
            orderId,
            currentStatus,
        });
    }
}

export class TaxCalculationError extends OrderDomainError {
    public readonly errorCode = "TAX_CALCULATION_FAILURE";

    constructor(public readonly destinationZip: string, public readonly originalError: string) {
        super(`Failed to calculate sales tax for destination '${destinationZip}': ${originalError}`, {
            destinationZip,
            originalError,
        });
    }
}
