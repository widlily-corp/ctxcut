export interface TreeNode {
    id: string;
    parent?: TreeNode;
    children: TreeChild[];
}

export interface TreeChild {
    node: TreeNode;
    metadata: Record<string, string>;
}

export function findRoot(node: TreeNode): TreeNode {
    let curr: TreeNode = node;
    while (curr.parent) {
        curr = curr.parent;
    }
    return curr;
}
