import { LeafPayload, executeLeafAction } from './hop3';

export function runMultiHopAction(payload: LeafPayload): boolean {
    return executeLeafAction(payload);
}
