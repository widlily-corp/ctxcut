/**
 * Adversarial fixture 2: Deeply nested class members, static methods, getters/setters, generators.
 */

export interface SystemMetrics {
    cpu: number;
    memoryMb: number;
    activeConnections: number;
}

export class EngineController {
    private static instanceCount: number = 0;
    private _isThrottled: boolean = false;
    private _maxBandwidth: number = 1000;

    constructor(initialBandwidth: number = 1000) {
        this._maxBandwidth = initialBandwidth;
        EngineController.instanceCount += 1;
    }

    /**
     * Factory static method.
     */
    public static createDefault(): EngineController {
        return new EngineController(5000);
    }

    /**
     * Getter for throttled state.
     */
    public get isThrottled(): boolean {
        return this._isThrottled;
    }

    /**
     * Setter for throttled state.
     */
    public set isThrottled(val: boolean) {
        this._isThrottled = val;
    }

    /**
     * Async generator stream for telemetry events.
     */
    public async *streamMetrics(): AsyncGenerator<SystemMetrics, void, unknown> {
        while (!this._isThrottled) {
            yield {
                cpu: Math.random() * 100,
                memoryMb: 512,
                activeConnections: 42,
            };
        }
    }

    public executeCommand(command: string, timeoutMs: number = 3000): boolean {
        if (this._isThrottled) {
            return false;
        }
        return command.length > 0 && timeoutMs > 0;
    }
}
