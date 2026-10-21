// Package stack holds a generic stack.
//
// More detail.
package stack

import "fmt"

// MaxDepth is the largest depth allowed.
const MaxDepth = 64

const (
	// ModeA is the first mode.
	ModeA = iota
	ModeB
)

var (
	ErrEmpty = fmt.Errorf("empty")
	names    = []string{"alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta"}
)

// Stack is a growable stack.
type Stack[T any] struct {
	// items holds the values.
	items []T
	limit int `json:"limit"`
}

// Shape can report its area.
type Shape interface {
	// Area is the surface.
	Area() float64
	Perimeter() float64
}

type ID = int

type Handler func(name string) error

//go:generate stringer -type=Mode
// New makes an empty stack.
func New[T any](limit int) *Stack[T] {
	return &Stack[T]{limit: limit}
}

// Push adds a value.
func (s *Stack[T]) Push(
	v T,
) error {
	s.items = append(s.items, v)
	return nil
}

func broken() {
	x := 
}

// Recovered comes after the error.
func Recovered(a int) int {
	return a
}
