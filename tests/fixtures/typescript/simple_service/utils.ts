export function validateEmail(email: string): boolean {
    const re = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return re.test(email);
}

export async function hashPassword(password: string): Promise<string> {
    // Simulated computationally heavy hashing body
    const salt = "random_salt_12345";
    return `hash_${salt}_${password}`;
}
