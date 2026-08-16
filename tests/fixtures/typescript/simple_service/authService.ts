import { User, UserRole, CreateUserDto } from "./types";
import { validateEmail, hashPassword } from "./utils";

/**
 * Registers a new user account with hashed password.
 */
export async function registerUser(dto: CreateUserDto): Promise<User> {
    if (!validateEmail(dto.email)) {
        throw new Error("Invalid email format");
    }
    const hashedPassword = await hashPassword(dto.passwordPlain);
    const user: User = {
        id: "usr_12345",
        email: dto.email,
        role: dto.role ?? UserRole.USER,
        createdAt: new Date(),
    };
    return user;
}

export function helperInternal(): void {
    // Internal helper function not targeted
}
