import { DeepModel, computeScore } from "./index";

export function evaluateModel(model: DeepModel): number {
    return computeScore(model);
}
