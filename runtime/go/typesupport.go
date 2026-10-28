// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// What a generated codec and its harness name besides the two streams.
//
// The Go counterpart of runtime/python/zerodds_wire/typesupport.py plus
// the helpers this column's shape asks for and no other's does: Ptr,
// because an @optional member is a *T and Go cannot take the address of
// a call's result, and the checked character constructors, because a
// code point outside the member's width is the sample's refusal and has
// to happen at run time — where the sibling columns' runtimes raise it
// — rather than at generation time.

package zdwire

import (
	"fmt"
	"math"
	"strconv"
	"unicode/utf8"
)

// Ptr answers a pointer to a copy of `v`: what a generated decoder
// assigns an @optional member with, and what a generated sample builds
// one from. Go has no address-of on an rvalue, and a temporary per site
// would need a name no two sites in one scope could share.
func Ptr[T any](v T) *T { return &v }

// Strcmp compares two strings the way C11 §7.24.4 does — as unsigned
// octets — and answers the sign. Go's own string order is exactly that
// order, so this is the operator with C's name on it; it exists because
// the shared walk emits `strcmp` where a map's keys sort by code units.
func Strcmp(a string, b string) int {
	if a < b {
		return -1
	}
	if a > b {
		return 1
	}
	return 0
}

// Utf8Octets answers how many octets `text` is, which is what a
// string<N> bound counts: string<5> is char[6] in the C mapping of
// every measured DDS implementation.
func Utf8Octets(text string) int { return len(text) }

// Utf16Units answers how many UTF-16 code units `text` is, which is
// what a wstring<N> bound counts. A scalar above the basic plane is
// two.
func Utf16Units(text string) int {
	units := 0
	for _, r := range text {
		if r > 0xFFFF {
			units += 2
		} else {
			units++
		}
	}
	return units
}

// TrimUtf8 cuts `text` to at most `octets` bytes, never inside a
// character. Table 11: the order of the characters is preserved, so
// this is a prefix; a character that would straddle the bound is left
// out whole.
func TrimUtf8(text string, octets int) string {
	used := 0
	end := 0
	for i := 0; i < len(text); {
		_, size := utf8.DecodeRuneInString(text[i:])
		if used+size > octets {
			break
		}
		used += size
		i += size
		end = i
	}
	return text[:end]
}

// TrimUtf16 cuts `text` to at most `units` UTF-16 code units.
func TrimUtf16(text string, units int) string {
	used := 0
	end := 0
	for i, r := range text {
		cost := 1
		if r > 0xFFFF {
			cost = 2
		}
		if used+cost > units {
			break
		}
		used += cost
		end = i + utf8.RuneLen(r)
	}
	return text[:end]
}

// CheckedChar answers `point` as the one octet an IDL char is, refusing
// a code point above U+00FF rather than truncating it: 0xE4 is a
// different character from U+0161, and a sample outside its type is the
// sample's refusal — the same one Writer.write_char raises in the
// Python column.
func CheckedChar(point uint32) uint8 {
	if point > 0xFF {
		panic(OutOfRange{At: 0, Expected: "a character below U+0100"})
	}
	return uint8(point)
}

// CheckedWchar answers `point` as the one UTF-16 code unit an IDL wchar
// is, refusing a code point outside the BMP. §7.2.2.2.1.2.5.
func CheckedWchar(point uint32) uint16 {
	if point > 0xFFFF {
		panic(OutOfRange{At: 0, Expected: "a BMP character"})
	}
	return uint16(point)
}

// NaN answers the quiet NaN, which Go has no literal for.
func NaN() float64 { return math.NaN() }

// Inf answers the infinity of `sign`'s sign.
func Inf(sign int) float64 { return math.Inf(sign) }

// NegZero answers -0.0, which a Go constant cannot spell: an untyped
// -0.0 is +0, and WriteF64 puts a different octet on the wire for the
// two.
func NegZero() float64 { return math.Copysign(0, -1) }

// Itoa spells an int in decimal, for the paths a generated printer
// builds without importing strconv into every probe.
func Itoa(v int) string { return strconv.Itoa(v) }

// FormatI64 spells a 64-bit signed integer in decimal: a JSON number
// is a double and loses the low bits past 2^53, so both 64-bit widths
// travel as strings — for every value and not only the large ones.
func FormatI64(v int64) string { return strconv.FormatInt(v, 10) }

// FormatU64 spells a 64-bit unsigned integer in decimal.
func FormatU64(v uint64) string { return strconv.FormatUint(v, 10) }

// JSONNumberOK answers whether a float has a JSON form at all: NaN and
// the two infinities are not JSON, and a document carrying one would
// fail in the caller's canonicaliser with the path lost.
func JSONNumberOK(v float64) bool { return !math.IsNaN(v) && !math.IsInf(v, 0) }

// PanicText spells a recovered panic value, for the harness that turns
// a WireError into the sample's refusal.
func PanicText(v any) string {
	if err, ok := v.(error); ok {
		return err.Error()
	}
	return fmt.Sprint(v)
}
