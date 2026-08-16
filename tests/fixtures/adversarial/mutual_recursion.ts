/**
 * Adversarial fixture 3: Deep mutual recursion and self-referencing generic AST structures.
 */

export interface NodeA {
    id: string;
    bRef: NodeB;
    payload: string;
}

export interface NodeB {
    id: string;
    cRef: NodeC;
    score: number;
}

export interface NodeC {
    id: string;
    aRef: NodeA;
    active: boolean;
}

export type BinaryTree<T> = {
    value: T;
    left?: BinaryTree<T>;
    right?: BinaryTree<T>;
};

export interface GraphEdge {
    source: GraphVertex;
    target: GraphVertex;
    weight: number;
}

export interface GraphVertex {
    name: string;
    outgoing: GraphEdge[];
    incoming: GraphEdge[];
}

export function processRecursiveGraph(root: NodeA, depthLimit: number): NodeC {
    let currentA = root;
    let currentB = currentA.bRef;
    let currentC = currentB.cRef;

    for (let i = 0; i < depthLimit; i++) {
        currentA = currentC.aRef;
        currentB = currentA.bRef;
        currentC = currentB.cRef;
    }

    return currentC;
}
