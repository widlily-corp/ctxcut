export interface User {
    id: string;
    email: string;
    role: UserRole;
    createdAt: Date;
}

export enum UserRole {
    ADMIN = "ADMIN",
    USER = "USER",
    GUEST = "GUEST",
}

export interface CreateUserDto {
    email: string;
    passwordPlain: string;
    role?: UserRole;
}
