package fixtures

// ValidHeaderFunc is well-formed before any syntax breaks.
func ValidHeaderFunc(a, b int) int {
	return a + b
}

// BrokenBraceFunc has unclosed brackets and malformed control flow.
func BrokenBraceFunc(items []string) string {
	for _, it := range items {
		if len(it) > 0 {
			return it
	// Missing closing braces for if and for

// TargetFuncWithSurroundingErrors is an intact function surrounded by syntax errors.
func TargetFuncWithSurroundingErrors(val int) int {
	return val*val + 100
}

// MalformedStruct has missing field types and invalid syntax tokens.
type MalformedStruct struct {
	FieldA string
	FieldB 
	FieldC [][]int
	invalid token %%%% here
