export interface LeafPayload {
    code: string;
    level: number;
}

export function executeLeafAction(payload: LeafPayload): boolean {
    return payload.level > 0;
}
