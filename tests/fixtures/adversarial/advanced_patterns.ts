/**
 * Adversarial fixture 4: Advanced exports, overloads, namespaces, private identifiers.
 */

export function overloadedFn(x: string): string;
export function overloadedFn(x: number): number;
export function overloadedFn(x: string | number): string | number {
    return x;
}

export abstract class AbstractBaseWorker {
    protected workerId: string;

    constructor(id: string) {
        this.workerId = id;
    }

    abstract runTask(input: string): Promise<boolean>;
}

export class ConcreteWorker extends AbstractBaseWorker {
    #internalCounter: number = 0;

    constructor(id: string) {
        super(id);
    }

    public async runTask(input: string): Promise<boolean> {
        this.#increment();
        return input.length > 0;
    }

    #increment(): void {
        this.#internalCounter += 1;
    }
}

export namespace Utilities {
    export interface UtilityConfig {
        enabled: boolean;
    }

    export function formatUtility(cfg: UtilityConfig): string {
        return cfg.enabled ? "ACTIVE" : "INACTIVE";
    }
}
