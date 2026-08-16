package fixtures

import (
	"errors"
	"fmt"
	"math"
	"strings"
)

// AddNumbers returns the sum of two integers.
func AddNumbers(a int, b int) int {
	return a + b
}

// FormatUserName formats a first and last name with an optional title prefix.
func FormatUserName(firstName, lastName string, prefix string) string {
	fullName := strings.TrimSpace(fmt.Sprintf("%s %s", strings.TrimSpace(firstName), strings.TrimSpace(lastName)))
	if strings.TrimSpace(prefix) != "" {
		return fmt.Sprintf("%s %s", strings.TrimSpace(prefix), fullName)
	}
	return fullName
}

// DivideWithRemainder demonstrates multiple return values and named returns.
func DivideWithRemainder(numerator, denominator int) (quotient int, remainder int, err error) {
	if denominator == 0 {
		return 0, 0, errors.New("division by zero")
	}
	quotient = numerator / denominator
	remainder = numerator % denominator
	return quotient, remainder, nil
}

// CalculateDiscount computes the discounted price given a base price and percentage.
func CalculateDiscount(price float64, percentage float64) (float64, error) {
	if price < 0 {
		return 0, errors.New("price cannot be negative")
	}
	if percentage < 0 || percentage > 100 {
		return 0, errors.New("percentage must be between 0 and 100")
	}
	discount := (price * percentage) / 100.0
	rounded := math.Round((price-discount)*100) / 100
	return rounded, nil
}

// ClampFloat restricts a value to a given minimum and maximum boundary.
func ClampFloat(val, min, max float64) float64 {
	if val < min {
		return min
	}
	if val > max {
		return max
	}
	return val
}
