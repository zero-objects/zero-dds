// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// What can go wrong on the wire.
//
// The counterpart of runtime/python/zerodds_wire/error.py, as the values
// this runtime panics with. A generated codec has no error channel — the
// shared walk emits straight-line calls — so a malformed stream or a
// value outside its type unwinds to the harness, which recovers and
// turns the failure into the exit code the verdict table gives it.
// A refusal about the *type* does not panic; see Reader.ReaderRefuse.

package zdwire

import "fmt"

// WireError is the interface every wire failure of this runtime
// satisfies, so a harness can tell one from an unrelated panic.
type WireError interface {
	error
	wireError()
}

// Truncated says the stream ended before the value did.
type Truncated struct {
	At        int
	Needed    int
	Available int
}

func (e Truncated) Error() string {
	return fmt.Sprintf("at %d: %d bytes needed, %d available", e.At, e.Needed, e.Available)
}

func (Truncated) wireError() {}

// ImpossibleLength says a length field claims more than the stream
// holds. Separate from Truncated: a length that cannot be true is a
// malformed stream, not a short one, and allocating for it is how a
// reader turns a bad message into an out-of-memory.
type ImpossibleLength struct {
	At        int
	Claimed   int
	Available int
}

func (e ImpossibleLength) Error() string {
	return fmt.Sprintf("at %d: a length of %d with %d bytes left", e.At, e.Claimed, e.Available)
}

func (ImpossibleLength) wireError() {}

// NotUtf8 says a string held bytes that are not UTF-8.
type NotUtf8 struct {
	At int
}

func (e NotUtf8) Error() string {
	return fmt.Sprintf("at %d: the string is not UTF-8", e.At)
}

func (NotUtf8) wireError() {}

// OutOfRange says a value is outside what its type admits. Raised by
// the writer as well as the reader: the generated Go holds a member in
// its wire width already, but a policy name, a decimal string or a code
// point can still be outside what the operation accepts.
type OutOfRange struct {
	At       int
	Expected string
}

func (e OutOfRange) Error() string {
	return fmt.Sprintf("at %d: expected %s", e.At, e.Expected)
}

func (OutOfRange) wireError() {}
