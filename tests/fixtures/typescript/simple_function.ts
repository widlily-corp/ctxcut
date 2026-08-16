/**
 * Standalone typed helper functions for testing AST extraction of pure functions.
 */

export function addNumbers(a: number, b: number): number {
    return a + b;
}

export function formatUserName(firstName: string, lastName: string, prefix?: string): string {
    const fullName = `${firstName.trim()} ${lastName.trim()}`.trim();
    if (prefix && prefix.trim().length > 0) {
        return `${prefix.trim()} ${fullName}`;
    }
    return fullName;
}

export function calculateDiscount(price: number, percentage: number): number {
    if (price < 0) {
        throw new RangeError("Price cannot be negative");
    }
    if (percentage < 0 || percentage > 100) {
        throw new RangeError("Percentage must be between 0 and 100");
    }
    const discount = (price * percentage) / 100;
    return Math.round((price - discount) * 100) / 100;
}

export function clamp(value: number, min: number, max: number): number {
    if (min > max) {
        throw new Error("Min cannot be greater than Max");
    }
    return Math.min(Math.max(value, min), max);
}

export function isNonEmptyString(input: unknown): input is string {
    return typeof input === "string" && input.trim().length > 0;
}

export function generateSlug(text: string): string {
    return text
        .toLowerCase()
        .trim()
        .replace(/[^\w\s-]/g, "")
        .replace(/[\s_-]+/g, "-")
        .replace(/^-+|-+$/g, "");
}
