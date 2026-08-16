/**
 * Complex nested generic structures and type hoisting fixtures for TypeScript.
 */

export type Result<T, E = DomainError> =
    | { readonly ok: true; readonly value: T }
    | { readonly ok: false; readonly error: E };

export interface DomainError {
    readonly code: string;
    readonly message: string;
    readonly details?: Readonly<Record<string, unknown>>;
    readonly timestamp: number;
}

export interface UserDTO {
    readonly id: string;
    readonly username: string;
    readonly email: string;
    readonly roles: ReadonlyArray<string>;
    readonly metadata: UserMetadata;
}

export interface UserMetadata {
    readonly createdAt: string;
    readonly lastLoginAt: string | null;
    readonly preferences: UserPreferences;
    readonly tags: ReadonlySet<string>;
}

export interface UserPreferences {
    readonly theme: "light" | "dark" | "system";
    readonly locale: string;
    readonly notifications: NotificationSettings;
}

export interface NotificationSettings {
    readonly email: boolean;
    readonly sms: boolean;
    readonly push: boolean;
    readonly frequency: "instant" | "daily_digest" | "weekly_digest";
}

export interface PaginatedResult<TData> {
    readonly items: ReadonlyArray<TData>;
    readonly total: number;
    readonly page: number;
    readonly pageSize: number;
    readonly hasNextPage: boolean;
}

export interface QueryFilterCriteria<TRecord> {
    readonly filter: Partial<Record<keyof TRecord, unknown>>;
    readonly sort?: {
        readonly field: keyof TRecord;
        readonly direction: "asc" | "desc";
    };
    readonly pagination: {
        readonly limit: number;
        readonly offset: number;
    };
}

export type ComplexUserMapResult = Promise<Result<Map<string, UserDTO>, DomainError>>;
export type PaginatedUserQueryResult = Promise<Result<PaginatedResult<UserDTO>, DomainError>>;

export async function fetchUserMapping(
    userIds: ReadonlyArray<string>,
    fallbackLocale: string
): Promise<Result<Map<string, UserDTO>, DomainError>> {
    if (userIds.length === 0) {
        return {
            ok: false,
            error: {
                code: "EMPTY_ID_LIST",
                message: "User ID array cannot be empty",
                timestamp: Date.now(),
            },
        };
    }

    const userMap = new Map<string, UserDTO>();
    for (const id of userIds) {
        const dto: UserDTO = {
            id,
            username: `user_${id}`,
            email: `user_${id}@example.com`,
            roles: ["standard_user"],
            metadata: {
                createdAt: new Date().toISOString(),
                lastLoginAt: null,
                preferences: {
                    theme: "system",
                    locale: fallbackLocale,
                    notifications: {
                        email: true,
                        sms: false,
                        push: true,
                        frequency: "instant",
                    },
                },
                tags: new Set(["verified"]),
            },
        };
        userMap.set(id, dto);
    }

    return {
        ok: true,
        value: userMap,
    };
}

export async function queryUsersWithFilter(
    criteria: QueryFilterCriteria<UserDTO>
): Promise<Result<PaginatedResult<UserDTO>, DomainError>> {
    if (criteria.pagination.limit <= 0) {
        return {
            ok: false,
            error: {
                code: "INVALID_PAGE_SIZE",
                message: "Pagination limit must be positive",
                details: { requestedLimit: criteria.pagination.limit },
                timestamp: Date.now(),
            },
        };
    }

    const items: UserDTO[] = [];
    return {
        ok: true,
        value: {
            items,
            total: items.length,
            page: Math.floor(criteria.pagination.offset / criteria.pagination.limit) + 1,
            pageSize: criteria.pagination.limit,
            hasNextPage: false,
        },
    };
}

export type UserRole = "admin" | "editor" | "viewer";

export interface UserProfileDTO {
    readonly id: string;
    readonly username: string;
    readonly role: UserRole;
    readonly metadata: UserMetadata;
}

export interface ApiResponse<TData> {
    readonly success: boolean;
    readonly payload: TData;
}

export async function fetchUserProfile<T extends UserProfileDTO>(
    userId: string
): Promise<ApiResponse<T>> {
    const profile = {} as T;
    return {
        success: true,
        payload: profile,
    };
}

