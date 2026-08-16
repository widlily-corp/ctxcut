/**
 * Express router fixtures with route registrations, middleware, DTOs, and handlers.
 */

export interface Request<P = Record<string, string>, ResBody = unknown, ReqBody = unknown, ReqQuery = Record<string, string>> {
    params: P;
    body: ReqBody;
    query: ReqQuery;
    headers: Record<string, string | string[] | undefined>;
    userId?: string;
}

export interface Response<ResBody = unknown> {
    status(code: number): this;
    json(body: ResBody): this;
    send(body?: unknown): this;
}

export type NextFunction = (err?: unknown) => void;
export type RequestHandler<P = Record<string, string>, ResBody = unknown, ReqBody = unknown, ReqQuery = Record<string, string>> = (
    req: Request<P, ResBody, ReqBody, ReqQuery>,
    res: Response<ResBody>,
    next: NextFunction
) => void | Promise<void>;

export interface Router {
    get(path: string, ...handlers: RequestHandler[]): this;
    post(path: string, ...handlers: RequestHandler[]): this;
    put(path: string, ...handlers: RequestHandler[]): this;
    delete(path: string, ...handlers: RequestHandler[]): this;
}

export interface CheckoutItemDTO {
    sku: string;
    quantity: number;
    unitPriceCents: number;
}

export interface CheckoutRequestDTO {
    customerId: string;
    items: CheckoutItemDTO[];
    shippingAddress: {
        street: string;
        city: string;
        state: string;
        postalCode: string;
        country: string;
    };
    paymentToken: string;
    idempotencyKey: string;
}

export interface CheckoutResponseDTO {
    orderId: string;
    status: "CONFIRMED" | "REQUIRES_ACTION" | "FAILED";
    totalAmountCents: number;
    currency: string;
    estimatedDeliveryDate: string;
    createdAt: string;
}

export interface UserProfileUpdateDTO {
    displayName?: string;
    email?: string;
    locale?: string;
}

export interface UserProfileResponseDTO {
    userId: string;
    displayName: string;
    email: string;
    locale: string;
    updatedAt: string;
}

export function validate<T>(schemaValidator: (data: unknown) => { success: boolean; data?: T; errors?: string[] }): RequestHandler {
    return (req, res, next) => {
        const result = schemaValidator(req.body);
        if (!result.success) {
            res.status(400).json({ error: "VALIDATION_FAILED", details: result.errors });
            return;
        }
        req.body = result.data;
        next();
    };
}

export function authenticate(req: Request, res: Response, next: NextFunction): void {
    const authHeader = req.headers["authorization"];
    if (!authHeader || typeof authHeader !== "string" || !authHeader.startsWith("Bearer ")) {
        res.status(401).json({ error: "UNAUTHORIZED", message: "Missing or invalid bearer token" });
        return;
    }
    const token = authHeader.substring(7);
    req.userId = `user_from_${token.slice(0, 8)}`;
    next();
}

export const CheckoutSchema = (data: unknown) => {
    if (typeof data !== "object" || data === null) {
        return { success: false, errors: ["Body must be an object"] };
    }
    const candidate = data as Partial<CheckoutRequestDTO>;
    if (!candidate.customerId || typeof candidate.customerId !== "string") {
        return { success: false, errors: ["customerId is required"] };
    }
    if (!Array.isArray(candidate.items) || candidate.items.length === 0) {
        return { success: false, errors: ["items must be a non-empty array"] };
    }
    return { success: true, data: data as CheckoutRequestDTO };
};

export async function handleCheckout(
    req: Request<Record<string, string>, CheckoutResponseDTO | { error: string; details?: unknown }, CheckoutRequestDTO>,
    res: Response<CheckoutResponseDTO | { error: string; details?: unknown }>
): Promise<void> {
    const payload = req.body;
    const totalCents = payload.items.reduce((sum, item) => sum + item.unitPriceCents * item.quantity, 0);

    const response: CheckoutResponseDTO = {
        orderId: `ord_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`,
        status: "CONFIRMED",
        totalAmountCents: totalCents,
        currency: "USD",
        estimatedDeliveryDate: new Date(Date.now() + 86400000 * 3).toISOString(),
        createdAt: new Date().toISOString(),
    };

    res.status(201).json(response);
}

export async function handleUserProfile(
    req: Request<{ userId: string }, UserProfileResponseDTO | { error: string }, UserProfileUpdateDTO>,
    res: Response<UserProfileResponseDTO | { error: string }>
): Promise<void> {
    const userId = req.params.userId;
    const body = req.body;

    res.status(200).json({
        userId,
        displayName: body.displayName ?? "Anonymous User",
        email: body.email ?? `${userId}@example.com`,
        locale: body.locale ?? "en_US",
        updatedAt: new Date().toISOString(),
    });
}

export function handleHealthCheck(_req: Request, res: Response<{ status: string; uptime: number }>): void {
    res.status(200).json({ status: "healthy", uptime: process.uptime() });
}

export function createRouter(routerImpl: Router): Router {
    routerImpl.get("/api/v1/health", handleHealthCheck as RequestHandler);
    routerImpl.post("/api/v1/checkout", authenticate as RequestHandler, validate(CheckoutSchema) as RequestHandler, handleCheckout as RequestHandler);
    routerImpl.put("/api/v1/users/:userId", authenticate as RequestHandler, handleUserProfile as RequestHandler);
    return routerImpl;
}
