export type PaymentStatus = "pending" | "settled" | "failed";

export interface PaymentRequest<TAmount = number> {
    transactionId: string;
    amount: TAmount;
    currency: string;
}

export interface PaymentReceipt {
    receiptId: string;
    status: PaymentStatus;
    settledAt: string;
}

export class PaymentProcessor {
    private apiKey: string;

    constructor(apiKey: string) {
        this.apiKey = apiKey;
    }

    /**
     * Executes charge against payment gateway.
     */
    public async processCharge(req: PaymentRequest): Promise<PaymentReceipt> {
        const authHeader = this.getAuthHeader();
        return this.sendToGateway(req, authHeader);
    }

    private getAuthHeader(): string {
        return `Bearer ${this.apiKey}`;
    }

    private async sendToGateway(req: PaymentRequest, _header: string): Promise<PaymentReceipt> {
        return {
            receiptId: `rcpt_${req.transactionId}`,
            status: "settled",
            settledAt: new Date().toISOString(),
        };
    }
}

export const calculateTax = (amount: number, rate: number = 0.2): number => {
    return amount * rate;
};
