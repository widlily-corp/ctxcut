package fixtures

// Node represents a doubly linked circular list element.
type Node struct {
	ID    string
	Value interface{}
	Next  *Node
	Prev  *Node
}

// GraphNode represents a vertex in an adjacency graph with bidirectional edge references.
type GraphNode struct {
	ID          string
	Label       string
	Outgoing    []*GraphEdge
	Incoming    []*GraphEdge
	SelfPointer *GraphNode
}

// GraphEdge connects two GraphNodes with weight and properties.
type GraphEdge struct {
	ID     string
	Source *GraphNode
	Target *GraphNode
	Weight float64
}

// Scope represents an AST lexical scope with parent and child relationships.
type Scope struct {
	ScopeID     string
	ParentScope *Scope
	Children    []*Scope
	Symbols     map[string]*Symbol
}

// Symbol represents a variable or type binding in a Scope.
type Symbol struct {
	Name            string
	TypeSignature   string
	DeclaredInScope *Scope
	References      []*Scope
}

// BuildSampleDoublyLinkedList creates a sample cyclic 2-node structure.
func BuildSampleDoublyLinkedList() *Node {
	head := &Node{ID: "node_1", Value: "first"}
	tail := &Node{ID: "node_2", Value: "second"}

	head.Next = tail
	head.Prev = tail

	tail.Next = head
	tail.Prev = head

	return head
}

// ConnectGraphNodes links two GraphNode instances with a directed edge.
func ConnectGraphNodes(src, dst *GraphNode, edgeID string, weight float64) *GraphEdge {
	edge := &GraphEdge{
		ID:     edgeID,
		Source: src,
		Target: dst,
		Weight: weight,
	}
	src.Outgoing = append(src.Outgoing, edge)
	dst.Incoming = append(dst.Incoming, edge)
	return edge
}
