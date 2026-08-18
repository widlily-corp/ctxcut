/**
 * TypeScript class indented with 4 spaces.
 */

export class FourSpaceCalculator {
    private baseValue: number;

    constructor(baseValue: number = 0) {
        this.baseValue = baseValue;
    }

    public add(amount: number): number {
        this.baseValue += amount;
        return this.baseValue;
    }

    public multiply(factor: number): number {
        this.baseValue *= factor;
        return this.baseValue;
    }

    public reset(): void {
        this.baseValue = 0;
    }
}
