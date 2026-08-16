export interface DeepModel {
    uuid: string;
    score: number;
}

export function computeScore(m: DeepModel): number {
    return m.score * 10;
}
