/**
 * Mutually recursive and circular interface references for TypeScript.
 */

export interface TreeNode<T = string> {
    id: string;
    value: T;
    parent?: TreeNode<T>;
    children: Array<TreeNode<T>>;
    leftSibling?: TreeNode<T>;
    rightSibling?: TreeNode<T>;
}

export interface GraphNode {
    id: string;
    label: string;
    incomingEdges: Edge[];
    outgoingEdges: Edge[];
    metadata: Record<string, unknown>;
}

export interface Edge {
    id: string;
    source: GraphNode;
    target: GraphNode;
    weight: number;
    bidirectional: boolean;
}

export interface ScopeTree {
    scopeId: string;
    parentScope?: ScopeTree;
    childScopes: ScopeTree[];
    declaredSymbols: Map<string, SymbolBinding>;
}

export interface SymbolBinding {
    name: string;
    typeAnnotation: string;
    declaredInScope: ScopeTree;
    referencingScopes: ScopeTree[];
    shadowsParentSymbol?: SymbolBinding;
}

export function buildSampleGraph(): { root: GraphNode; edge: Edge } {
    const rootNode: GraphNode = {
        id: "node-1",
        label: "Root Node",
        incomingEdges: [],
        outgoingEdges: [],
        metadata: {},
    };

    const targetNode: GraphNode = {
        id: "node-2",
        label: "Child Node",
        incomingEdges: [],
        outgoingEdges: [],
        metadata: {},
    };

    const edge: Edge = {
        id: "edge-1-2",
        source: rootNode,
        target: targetNode,
        weight: 1.0,
        bidirectional: false,
    };

    rootNode.outgoingEdges.push(edge);
    targetNode.incomingEdges.push(edge);

    return { root: rootNode, edge };
}

export function traverseTreeDepthFirst<T>(node: TreeNode<T>, visitor: (item: T) => void): void {
    visitor(node.value);
    for (const child of node.children) {
        traverseTreeDepthFirst(child, visitor);
    }
}

export interface User {
    id: string;
    name: string;
    posts: Post[];
}

export interface Post {
    id: string;
    title: string;
    author: User;
    comments: Comment[];
}

export interface Comment {
    id: string;
    text: string;
    author: User;
    post: Post;
}

export function formatUser(user: User): string {
    return `${user.name} (${user.posts.length} posts)`;
}

