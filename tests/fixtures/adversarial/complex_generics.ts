/**
 * Adversarial fixture 1: Complex TypeScript Generics, Mapped Types, Conditional Types.
 */

export interface Entity<TId extends string | number = string> {
    id: TId;
    version: number;
}

export interface DomainMeta {
    createdAt: Date;
    tenantId: string;
}

export interface RepoConfig<T extends Record<K, V>, K extends string = string, V = unknown> {
    registry: T;
    defaultKey: K;
    defaultValue: V;
    meta: DomainMeta;
}

export type ConditionalUnwrap<T> = T extends Promise<infer U> ? U : T extends Array<infer E> ? E : T;

export type MappedRecord<T extends Record<string, unknown>> = {
    [K in keyof T as `get_${string & K}`]: () => T[K];
};

export class AdvancedRepository<
    TEntity extends Entity<TId>,
    TId extends string | number,
    TConfig extends RepoConfig<Record<string, TEntity>, string, TEntity>
> {
    private config: TConfig;

    constructor(config: TConfig) {
        this.config = config;
    }

    /**
     * Finds entity by ID and unwraps conditional types.
     */
    public async findAndUnwrap<U extends ConditionalUnwrap<TEntity>>(
        id: TId,
        fallback: U
    ): Promise<TEntity | U> {
        if (!id) {
            return fallback;
        }
        return this.config.defaultValue;
    }
}
