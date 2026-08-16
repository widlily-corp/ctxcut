/**
 * Malformed TypeScript syntax fixture for testing error recovery and AST resilience.
 */

export interface ValidHeaderInterface {
    id: string;
    validProperty: boolean;
}

export function brokenFunctionOne(a: number, b: number {
    const sum = a + b;
    if (sum > 10) {
        return { result: sum, status: "high" 
    // Missing closing brace for object and if block

export function intactTargetFunction(x: number, y: number): number {
    return x * y + 42;
}

export interface PartiallyCorruptedInterface {
    fieldA: string;
    fieldB: number
    fieldC: Array<string; // missing closing bracket on generic
    fieldD: {
        nestedKey: boolean;

export function brokenFunctionTwo(data: unknown) {
    const item = data as any
    return item.map((val) => {
        val.trim(
        // Missing parenthesis and closing blocks
