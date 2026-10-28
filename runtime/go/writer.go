// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Putting bytes on the wire.
//
// The Go counterpart of crates/wire/src/writer.rs and of
// runtime/python/zerodds_wire/writer.py — a further implementation of
// the same clauses, not a binding to either. Where two columns disagree
// on an octet, one of them has misread DDS-XTypes 1.3, and that is the
// point of having both.
//
// Alignment is relative to the start of the stream, not to the start of
// the value: XCDR aligns each primitive to its own size counted from the
// encapsulation, which is why a long after a char sits at 4 and not at
// 1. XTypes 1.3 §7.4.1.1.
//
// The operations take the walk's argument order — WriteEnum(bitBound,
// value), WriteBitset(anchor, value) — because this runtime was written
// against the shared codec walk rather than adapted to it afterwards;
// the Python column has to name its keyword arguments for the same
// calls.

package zdwire

import (
	"fmt"
	"math"
	"unicode/utf8"
)

// DHeader is a DHEADER waiting for its length. At below zero is the
// delimiter a @final aggregate does not write; see NoDheader.
type DHeader struct {
	At int
}

// NoDheader is the DHEADER a value that takes no delimiter does not
// write. §7.4.3.5.3 gives the delimiter to @appendable and @mutable and
// not to @final, and the shared walk declares one DHeader per value and
// finishes it either way; FinishDheader ignores a header with At below
// zero.
func NoDheader() DHeader { return DHeader{At: -1} }

// EmHeader is an EMHEADER waiting for its NEXTINT, when it has one.
// NextInt is -1 where the length code carries the length itself.
type EmHeader struct {
	NextInt int
	// Word is where the header word itself sits, so a short length code
	// can be patched into it once the value's length is known.
	Word int
	// LengthCode is the code the header was written with, so that
	// FinishEmheader can hold the member's value to the length the code
	// declares. Codes 0 to 3 are lengths; anything else declares nothing
	// of its own.
	LengthCode int
}

// Pid is a parameter header waiting for its length.
type Pid struct {
	At          int
	Extended    bool
	SavedOrigin int
	// Role is which length convention FinishPid patches by.
	Role ParameterRole
	// Pid and MustUnderstand are kept so a short header can be
	// rewritten as an extended one when the value turns out not to fit
	// sixteen bits.
	Pid            int
	MustUnderstand bool
}

// Writer accumulates the bytes of one sample.
type Writer struct {
	out        []byte
	endianness Endianness
	// origin is where alignment is counted from. Zero for a sample; a
	// PL_CDR1 parameter value moves it, because a parameter is its own
	// encapsulated block. XTypes 1.3 §7.4.1.2.
	origin   int
	maxAlign int
	plCdr1   PlCdr1Framing
	// ShortLengthCodes says whether an LC4 header whose member turns
	// out to be 1, 2, 4 or 8 bytes is rewritten with a short length
	// code, which is what Fast DDS does.
	ShortLengthCodes bool
}

// NewWriter builds a writer in `endianness` whose alignment is capped
// at `maxAlign` — four for XCDR2, eight for XCDR1. XTypes 1.3 §7.4.1:
// ALIGN(N) aligns to MIN(N, XCDR.maxalign).
func NewWriter(endianness Endianness, maxAlign int, plCdr1 PlCdr1Framing, shortLengthCodes bool) *Writer {
	if maxAlign < 1 {
		maxAlign = 1
	}
	return &Writer{
		endianness:       endianness,
		maxAlign:         maxAlign,
		plCdr1:           plCdr1,
		ShortLengthCodes: shortLengthCodes,
	}
}

// Position answers how many bytes have been written.
func (w *Writer) Position() int { return len(w.out) }

// Finish answers the bytes written, as a copy.
func (w *Writer) Finish() []byte {
	out := make([]byte, len(w.out))
	copy(out, w.out)
	return out
}

// alignTo pads to `align`, counted from the stream's origin.
func (w *Writer) alignTo(align int) {
	if align > w.maxAlign {
		align = w.maxAlign
	}
	if align <= 1 {
		return
	}
	offset := len(w.out) - w.origin
	padding := (align - offset%align) % align
	for i := 0; i < padding; i++ {
		w.out = append(w.out, 0)
	}
}

// writeRaw appends `width` bytes of `unsigned` in the stream's order.
func (w *Writer) writeRaw(unsigned uint64, width int) {
	if w.endianness.IsLittle() {
		for i := 0; i < width; i++ {
			w.out = append(w.out, byte(unsigned>>(8*i)))
		}
		return
	}
	for i := width - 1; i >= 0; i-- {
		w.out = append(w.out, byte(unsigned>>(8*i)))
	}
}

// writeInt aligns to the value's own width, then writes it.
func (w *Writer) writeInt(unsigned uint64, width int) {
	w.alignTo(width)
	w.writeRaw(unsigned, width)
}

// -- primitives ----------------------------------------------------------

// WriteBool writes a boolean: one byte, 0 or 1.
func (w *Writer) WriteBool(v bool) {
	if v {
		w.out = append(w.out, 1)
	} else {
		w.out = append(w.out, 0)
	}
}

// WriteU8 writes an octet.
func (w *Writer) WriteU8(v uint8) { w.out = append(w.out, v) }

// WriteI8 writes an int8.
func (w *Writer) WriteI8(v int8) { w.out = append(w.out, byte(v)) }

// WriteChar writes a char: one octet, the code point's low byte — which
// is the whole code point, because the generated member holds one octet.
func (w *Writer) WriteChar(v uint8) { w.out = append(w.out, v) }

// WriteI16 writes a short.
func (w *Writer) WriteI16(v int16) { w.writeInt(uint64(uint16(v)), 2) }

// WriteU16 writes an unsigned short.
func (w *Writer) WriteU16(v uint16) { w.writeInt(uint64(v), 2) }

// WriteI32 writes a long.
func (w *Writer) WriteI32(v int32) { w.writeInt(uint64(uint32(v)), 4) }

// WriteU32 writes an unsigned long.
func (w *Writer) WriteU32(v uint32) { w.writeInt(uint64(v), 4) }

// WriteI64 writes a long long.
func (w *Writer) WriteI64(v int64) { w.writeInt(uint64(v), 8) }

// WriteU64 writes an unsigned long long.
func (w *Writer) WriteU64(v uint64) { w.writeInt(v, 8) }

// WriteF32 writes a float.
func (w *Writer) WriteF32(v float32) { w.writeInt(uint64(math.Float32bits(v)), 4) }

// WriteF64 writes a double.
func (w *Writer) WriteF64(v float64) { w.writeInt(math.Float64bits(v), 8) }

// WriteF128 writes a long double: sixteen bytes of IEEE 754 binary128,
// aligned to eight (capped by MAXALIGN). The value arrives as the
// decimal text the generated member holds — Go has no binary128 and a
// float64 member would lose sixty significand bits before any codec ran
// — and is correctly rounded here, once. The two byte orders are the
// whole sixteen bytes reversed; CORBA 3.3 part 2 §9.3.1.3, Figure 9.2.
func (w *Writer) WriteF128(v string) {
	bytes := f128FromDecimal(v, len(w.out))
	w.alignTo(8)
	if w.endianness.IsLittle() {
		w.out = append(w.out, bytes[:]...)
		return
	}
	for i := 15; i >= 0; i-- {
		w.out = append(w.out, bytes[i])
	}
}

// -- characters and strings ----------------------------------------------

// WriteWchar writes a wchar, at `width` octets and in the shape
// `framing` does. Two octets under XTypes 1.3 Table 31; four under RTI
// Connext's XCDR1, TAO's, and rosidl_typesupport_fastrtps_c's.
// "giop-length-prefixed" is CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form:
// the octet count first, then the code unit's octets, all aligned to
// one.
func (w *Writer) WriteWchar(v uint16, width int, framing string, order string) {
	if width != 2 && width != 4 {
		panic(OutOfRange{At: len(w.out), Expected: "a wchar width of 2 or 4"})
	}
	if framing == "giop-length-prefixed" {
		w.out = append(w.out, byte(width))
		// A scalar cannot carry a byte-order mark, so CORBA's default
		// applies and every order but "stream" means big-endian here.
		big := order != "stream" || !w.endianness.IsLittle()
		if big {
			for i := width - 1; i >= 0; i-- {
				w.out = append(w.out, byte(uint32(v)>>(8*i)))
			}
		} else {
			for i := 0; i < width; i++ {
				w.out = append(w.out, byte(uint32(v)>>(8*i)))
			}
		}
		return
	}
	if framing != "fixed" {
		panic(OutOfRange{At: len(w.out), Expected: "a wchar framing, not `" + framing + "`"})
	}
	if width == 4 {
		w.WriteU32(uint32(v))
		return
	}
	w.WriteU16(v)
}

// WriteString writes a string: a four-byte length including the
// terminator, the UTF-8 bytes, then a NUL. XTypes 1.3 §7.4.1.1. The
// length counts bytes, not characters.
func (w *Writer) WriteString(v string) {
	w.WriteU32(uint32(len(v)) + 1)
	w.out = append(w.out, v...)
	w.out = append(w.out, 0)
}

// WriteStringBounded writes a bounded string: the same bytes
// WriteString writes. The bound is a promise about the source and not
// a difference on the wire, so this is the only place it can be
// enforced — refused rather than cut at a character boundary, which
// would put a sample on the wire nobody wrote.
func (w *Writer) WriteStringBounded(v string, bound int) {
	if len(v) > bound {
		panic(OutOfRange{At: len(w.out), Expected: "at most " + itoa(bound) +
			" octets in a bounded string, and this one has " + itoa(len(v))})
	}
	w.WriteString(v)
}

// utf16UnitsOf answers the UTF-16 code units of `v`, without a BOM. A
// byte sequence that is not UTF-8 is refused: writing its replacement
// character would put a string on the wire the caller never held.
func utf16UnitsOf(v string, at int) []uint16 {
	units := make([]uint16, 0, len(v))
	for i := 0; i < len(v); {
		r, size := utf8.DecodeRuneInString(v[i:])
		if r == utf8.RuneError && size <= 1 {
			panic(OutOfRange{At: at, Expected: "a UTF-8 wstring"})
		}
		if r < 0x10000 {
			units = append(units, uint16(r))
		} else {
			offset := r - 0x10000
			units = append(units, uint16(0xD800+(offset>>10)), uint16(0xDC00+(offset&0x3FF)))
		}
		i += size
	}
	return units
}

// WriteWstringUtf8 writes a wstring, in the form `policy` and `order`
// name. The value arrives as UTF-8 — a Go string — and the conversion
// to UTF-16 code units happens here, which is what makes the
// §7.4.1.1.2 surrogate rule this runtime's own reading rather than a
// copy of the Rust column's answer.
func (w *Writer) WriteWstringUtf8(v string, policy string, order string) {
	cell, known := wstringCellOf(policy)
	if !known {
		panic(OutOfRange{At: len(w.out), Expected: "a wstring length policy, not `" + policy + "`"})
	}
	if !wstringOrderKnown(order) {
		panic(OutOfRange{At: len(w.out), Expected: "a wstring unit order, not `" + order + "`"})
	}
	units := utf16UnitsOf(v, len(w.out))
	if cell.countsCharacters && cell.width == 2 {
		// A character count over two-octet units has no way to carry a
		// code point outside the BMP: Fast CDR writes its low sixteen
		// bits and a different character arrives. A vendor's length
		// convention is a policy; its data loss is not.
		for index, unit := range units {
			if unit >= 0xD800 && unit <= 0xDFFF {
				panic(OutOfRange{At: index, Expected: "a code point inside the BMP: a character count over two-octet units has no way to carry one outside it"})
			}
		}
	}
	if cell.terminated {
		// CORBA §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form: a NUL
		// code unit at the end, inside whatever the length counts.
		units = append(units, 0)
	}
	// An empty wstring carries no mark: a mark on one would be a
	// two-octet string that decodes to nothing, and no peer writes it.
	if order == "byte-order-mark" && len(units) > 0 {
		units = append([]uint16{byteOrderMark}, units...)
	}
	if cell.countsCharacters {
		w.WriteU32(uint32(len(units)))
	} else {
		w.WriteU32(uint32(len(units) * cell.width))
	}
	for _, unit := range units {
		w.writeWstringUnit(unit, cell.width, order)
	}
}

// WriteWstringBounded writes a bounded wstring, as WriteStringBounded
// is to WriteString. The bound counts UTF-16 code units — not
// characters and not octets, whatever policy puts on the wire.
func (w *Writer) WriteWstringBounded(v string, bound int, policy string, order string) {
	units := utf16UnitsOf(v, len(w.out))
	if len(units) > bound {
		panic(OutOfRange{At: len(w.out), Expected: "at most " + itoa(bound) +
			" UTF-16 code units in a bounded wstring, and this one has " + itoa(len(units))})
	}
	w.WriteWstringUtf8(v, policy, order)
}

// WriteWstringUtf8Octets writes a wstring whose characters go out as
// their own UTF-8 octets.
//
// WstringCodeUnits::Utf8 is the measured departure of one product's
// generated serialiser: XTypes 1.3 Table 31 gives a wstring's code unit
// as Char16 and §7.4.1.1.2 makes the length "twice the number of
// characters", so the clause has no UTF-8 form at all. The declared
// length is the UTF-8 octet count plus the terminator the policy asks
// for. wstring-member is `04 00 00 00 61 62 63 00` for "abc": three
// octets and a NUL, and the four declares both.
func (w *Writer) WriteWstringUtf8Octets(v string, policy string) {
	cell, known := wstringCellOf(policy)
	if !known {
		panic(OutOfRange{At: len(w.out), Expected: "a wstring length policy, not `" + policy + "`"})
	}
	// The same malformed-UTF-8 refusal WriteWstringUtf8 makes: a Go
	// string can hold arbitrary bytes, and writing them would put a
	// sequence on the wire no UTF-8 reader accepts.
	_ = utf16UnitsOf(v, len(w.out))
	bytes := []byte(v)
	length := len(bytes)
	if cell.terminated {
		length++
	}
	if int64(length) > int64(0xFFFFFFFF) {
		panic(OutOfRange{At: len(w.out), Expected: "a wstring length the UInt32 field can hold"})
	}
	w.WriteU32(uint32(length))
	w.out = append(w.out, bytes...)
	if cell.terminated {
		w.out = append(w.out, 0)
	}
}

// WriteWstringUtf8OctetsBounded writes a bounded UTF-8-octet wstring, as
// WriteWstringBounded is to WriteWstringUtf8. The bound still counts
// UTF-16 code units, which is what a wstring<N> bound means to every
// peer, while the wire carries the UTF-8 octets.
func (w *Writer) WriteWstringUtf8OctetsBounded(v string, bound int, policy string) {
	units := utf16UnitsOf(v, len(w.out))
	if len(units) > bound {
		panic(OutOfRange{At: len(w.out), Expected: "at most " + itoa(bound) +
			" UTF-16 code units in a bounded wstring, and this one has " + itoa(len(units))})
	}
	w.WriteWstringUtf8Octets(v, policy)
}

// BeginPackedElement moves the alignment origin to the current position
// and answers the origin it displaced, for FinishPackedElement.
//
// ElementPadding::Packed is HDDS's measured departure from §7.4.1, which
// counts every element's alignment from the encapsulation: each
// collection element and each aggregate value counts its own alignment
// from its own first octet instead. array-1d-struct-final is
// `01000000 0200 03000000 0400 77`, the second Cell's long at offset six
// where the clause puts it at eight.
func (w *Writer) BeginPackedElement() int {
	saved := w.origin
	w.origin = len(w.out)
	return saved
}

// FinishPackedElement restores the origin BeginPackedElement displaced.
func (w *Writer) FinishPackedElement(saved int) {
	w.origin = saved
}

// CheckSequenceBound is the count check every bounded sequence makes
// before a byte of it is written. Table 17 makes
// O2.length <= T1.length the condition for constructing a
// sequence<T, N>, and the wire carries no bound to compare against: a
// sender whose count already exceeds its own declaration has written a
// value of a type nobody declared.
func (w *Writer) CheckSequenceBound(actual uint32, bound uint32) {
	if actual > bound {
		panic(OutOfRange{At: len(w.out), Expected: "at most " + itoa(int(bound)) +
			" elements in a bounded sequence, and this one has " + itoa(int(actual))})
	}
}

// CheckMapBound is the count check every bounded map makes before a
// byte of it is written. Its own message and not
// CheckSequenceBound's: the unit is an entry and not an element, per
// XTypes 1.3 §7.4.1.1.4's map-as-a-sequence-of-pairs equivalence.
func (w *Writer) CheckMapBound(actual uint32, bound uint32) {
	if actual > bound {
		panic(OutOfRange{At: len(w.out), Expected: "at most " + itoa(int(bound)) +
			" entries in a bounded map, and this one has " + itoa(int(actual))})
	}
}

// writeWstringUnit writes one wstring code unit, at its width and in
// its order. C3 picks the writer and C4 picks its endianness.
func (w *Writer) writeWstringUnit(unit uint16, width int, order string) {
	little := order == "byte-order-mark" || order == "host-unswapped"
	if width == 4 {
		switch {
		case order == "big-endian-always":
			w.WriteU32Big(uint32(unit))
		case little:
			w.WriteU32Little(uint32(unit))
		default:
			w.WriteU32(uint32(unit))
		}
		return
	}
	switch {
	case order == "big-endian-always":
		w.WriteU16Big(unit)
	case little:
		w.WriteU16Little(unit)
	default:
		w.WriteU16(unit)
	}
}

// WriteU16Little writes a 16-bit value little-endian whatever the
// stream's order is. Aligned to two like any other 16-bit value.
// WriteUnionOverlay is the Cyclone union overlay: the low octets of
// pattern, written as one unsigned integer of that width in the
// stream's byte order. A discriminator carrying a label the
// union-labels dialect axis says selects nothing makes the peer fall
// through to `default:` and serialise its one C union storage as that
// arm — the selected member's octets, reinterpreted. The pattern is
// the uint64 whose little-endian bytes are the selected member's
// little-endian storage octets, zero-extended; the BitPattern*
// functions produce it.
func (w *Writer) WriteUnionOverlay(pattern uint64, octets int64) {
	switch octets {
	case 1:
		w.WriteU8(uint8(pattern))
	case 2:
		w.WriteU16(uint16(pattern))
	case 4:
		w.WriteU32(uint32(pattern))
	case 8:
		w.WriteU64(pattern)
	default:
		panic(OutOfRange{At: len(w.out), Expected: "a union arm of 1, 2, 4 or 8 octets"})
	}
}

// BitPatternInt is the low octets of value's two's-complement bits,
// zero-extended — WriteUnionOverlay's pattern for an integer,
// character, enumeration or bitmask arm. The mask is the function:
// a plain widening of a negative value sign-extends, and -2 in four
// octets has to be 0x00000000FFFFFFFE.
func BitPatternInt(value int64, octets int64) uint64 {
	pattern := uint64(value)
	if octets >= 8 {
		return pattern
	}
	return pattern & (1<<(octets*8) - 1)
}

// BitPatternBool is one octet, 0 or 1 — the boolean overlay pattern.
func BitPatternBool(value bool) uint64 {
	if value {
		return 1
	}
	return 0
}

// BitPatternF32 is the IEEE 754 binary32 bits, zero-extended.
func BitPatternF32(value float32) uint64 {
	return uint64(math.Float32bits(value))
}

// BitPatternF64 is the IEEE 754 binary64 bits.
func BitPatternF64(value float64) uint64 {
	return math.Float64bits(value)
}

func (w *Writer) WriteU16Little(v uint16) {
	w.alignTo(2)
	w.out = append(w.out, byte(v), byte(v>>8))
}

// WriteU16Big writes a 16-bit value big-endian whatever the stream's
// order is: what omniORB and JacORB decode a wstring's code units as.
func (w *Writer) WriteU16Big(v uint16) {
	w.alignTo(2)
	w.out = append(w.out, byte(v>>8), byte(v))
}

// WriteU32Little writes a 32-bit value little-endian whatever the
// stream's order is.
func (w *Writer) WriteU32Little(v uint32) {
	w.alignTo(4)
	w.out = append(w.out, byte(v), byte(v>>8), byte(v>>16), byte(v>>24))
}

// WriteU32Big writes a 32-bit value big-endian whatever the stream's
// order is.
func (w *Writer) WriteU32Big(v uint32) {
	w.alignTo(4)
	w.out = append(w.out, byte(v>>24), byte(v>>16), byte(v>>8), byte(v))
}

// -- fixed ---------------------------------------------------------------

// WriteFixed writes a fixed<digits, scale> as packed BCD, [CDR]
// §9.3.2.8 — the only wire definition an IDL fixed has. The value
// arrives as a decimal string because fixed<31, 2> is exactly the case
// a double cannot carry. `count` is "declared" — `digits` nibbles
// whatever the value, omniORB's form and this project's —
// "value-precision", as many digits as the value has and never fewer
// than one, which JacORB writes, or "value-string-digits", the digits of
// the decimal as written made odd by one leading zero, which ACE/TAO
// 2.5.24's ACE_CDR::Fixed::from_string writes, where OpenDDS writes the declared count (0.12345
// under fixed<5,5> is 0012345c; past 31 digits it is refused).
func (w *Writer) WriteFixed(v string, digits int, scale int, count string) {
	nibbles := fixedNibbles(v, digits, scale, count, len(w.out))
	for i := 0; i < len(nibbles); i += 2 {
		w.out = append(w.out, byte(nibbles[i]<<4|nibbles[i+1]))
	}
}

// fixedNibbles answers the BCD nibbles of `text` under
// fixed<digits, scale>: digit nibbles most-significant first, then a
// sign nibble — 0xC for positive or zero, 0xD for negative — with a
// leading zero nibble prepended when the count is odd.
func fixedNibbles(text string, digits int, scale int, count string, at int) []int {
	if digits == 0 {
		panic(OutOfRange{At: at, Expected: "a fixed with at least one digit"})
	}
	// OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and fixed<31, S>
	// is the widest declaration IDL admits. A wider one has no packing
	// defined for it, so it is refused rather than packed into octets no
	// reader can be told the width of.
	if digits > 31 {
		panic(OutOfRange{At: at, Expected: fmt.Sprintf(
			"a fixed of at most 31 digits, not %d", digits)})
	}
	if scale > digits {
		panic(OutOfRange{At: at, Expected: "a scale no larger than the digit count"})
	}
	if count != "declared" && count != "value-precision" && count != "value-string-digits" {
		panic(OutOfRange{At: at, Expected: "a fixed digit count, not `" + count + "`"})
	}
	negative := false
	magnitude := text
	if len(magnitude) > 0 && (magnitude[0] == '-' || magnitude[0] == '+') {
		negative = magnitude[0] == '-'
		magnitude = magnitude[1:]
	}
	whole := magnitude
	fraction := ""
	for i := 0; i < len(magnitude); i++ {
		if magnitude[i] == '.' {
			whole = magnitude[:i]
			fraction = magnitude[i+1:]
			break
		}
	}
	refuse := func() {
		panic(OutOfRange{At: at, Expected: "a decimal that fits fixed<" + itoa(digits) + ", " + itoa(scale) + ">, not `" + text + "`"})
	}
	if whole == "" && fraction == "" {
		refuse()
	}
	for i := 0; i < len(whole); i++ {
		if whole[i] < '0' || whole[i] > '9' {
			refuse()
		}
	}
	for i := 0; i < len(fraction); i++ {
		if fraction[i] < '0' || fraction[i] > '9' {
			refuse()
		}
	}
	asWritten := len(whole) + scale
	integralWidth := digits - scale
	for len(whole) > 0 && whole[0] == '0' {
		whole = whole[1:]
	}
	if len(whole) > integralWidth {
		panic(OutOfRange{At: at, Expected: "a decimal whose integer part fits " + itoa(integralWidth) + " digits, not `" + text + "`"})
	}
	if len(fraction) > scale {
		panic(OutOfRange{At: at, Expected: "a decimal whose fraction fits " + itoa(scale) + " digits, not `" + text + "`"})
	}

	// The full `digits`-wide run first, whatever the count: the value's
	// precision is that run with its leading zeros removed, so both
	// counts are read off one construction.
	every := make([]int, 0, digits)
	for i := 0; i < integralWidth-len(whole); i++ {
		every = append(every, 0)
	}
	for i := 0; i < len(whole); i++ {
		every = append(every, int(whole[i]-'0'))
	}
	for i := 0; i < len(fraction); i++ {
		every = append(every, int(fraction[i]-'0'))
	}
	for i := 0; i < scale-len(fraction); i++ {
		every = append(every, 0)
	}
	significant := 0
	for i, nibble := range every {
		if nibble != 0 {
			significant = len(every) - i
			break
		}
	}
	wire := len(every)
	if count == "value-precision" {
		wire = significant
		if wire < 1 {
			wire = 1
		}
	} else if count == "value-string-digits" {
		// ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
		// zero. Past 31 no reader can be told the field's width.
		wire = asWritten + 1 - asWritten%2
		if wire > 31 {
			panic(OutOfRange{At: at, Expected: "a fixed of at most 31 digits on the wire, not " + itoa(wire) + " for `" + text + "`"})
		}
	}

	nibbles := make([]int, 0, wire+2)
	if (wire+1)%2 == 1 {
		nibbles = append(nibbles, 0)
	}
	// A count wider than P is filled with leading zeros.
	kept := wire
	if kept > len(every) {
		kept = len(every)
	}
	for i := kept; i < wire; i++ {
		nibbles = append(nibbles, 0)
	}
	nibbles = append(nibbles, every[len(every)-kept:]...)
	// A negative zero is written positive, because there is one zero.
	sign := 0x0C
	if negative && significant != 0 {
		sign = 0x0D
	}
	nibbles = append(nibbles, sign)
	return nibbles
}

// itoa spells a small non-negative int, without importing strconv into
// every message site.
func itoa(v int) string {
	if v == 0 {
		return "0"
	}
	digits := make([]byte, 0, 4)
	for v > 0 {
		digits = append([]byte{byte('0' + v%10)}, digits...)
		v /= 10
	}
	return string(digits)
}

// -- the bit-bounded kinds -----------------------------------------------

// WriteEnum writes an enumerator, as the integer its bit bound calls
// for. XTypes 1.3 §7.4.3.5.3 rule (5): the holder is signed —
// @value(-1) at the default bound goes out as ffffffff. What fits is
// everything expressible in the bound's bits, signed or unsigned:
// -2^(n-1) ..= 2^n - 1. §7.3.1.2.1.5 caps the bound at 1 to 32.
func (w *Writer) WriteEnum(bitBound int, value int64) {
	if bitBound == 0 || bitBound > 32 {
		panic(OutOfRange{At: len(w.out), Expected: "an @bit_bound between 1 and 32, not " + itoa(bitBound)})
	}
	if value < -(int64(1)<<(bitBound-1)) || value > (int64(1)<<bitBound)-1 {
		panic(OutOfRange{At: len(w.out), Expected: "an enumerator a " + itoa(bitBound) + "-bit holder can hold"})
	}
	pattern := uint64(value) & ((uint64(1) << bitBound) - 1)
	switch {
	case bitBound <= 8:
		w.WriteU8(uint8(pattern))
	case bitBound <= 16:
		w.WriteU16(uint16(pattern))
	default:
		w.WriteU32(uint32(pattern))
	}
}

// WriteBitmask writes a bitmask, as the unsigned integer its bit bound
// calls for. Table 33 of XTypes 1.3 §7.4.1.1.3.2 gives the holder.
func (w *Writer) WriteBitmask(bitBound int, value uint64) {
	if bitBound == 0 || bitBound > 64 {
		panic(OutOfRange{At: len(w.out), Expected: "an @bit_bound between 1 and 64, not " + itoa(bitBound)})
	}
	if bitBound < 64 && value >= uint64(1)<<bitBound {
		panic(OutOfRange{At: len(w.out), Expected: "a bitmask below 2^" + itoa(bitBound)})
	}
	switch {
	case bitBound <= 8:
		w.WriteU8(uint8(value))
	case bitBound <= 16:
		w.WriteU16(uint16(value))
	case bitBound <= 32:
		w.WriteU32(uint32(value))
	default:
		w.WriteU64(value)
	}
}

// Bitfield shifts one bitfield into the accumulator a bitset packs
// into. A value wider than the field is refused rather than masked:
// masking would put a different number on the wire than the caller
// asked for, and the caller would never find out.
func (w *Writer) Bitfield(value uint64, width int, position int) uint64 {
	if width <= 0 || width > 64 || position < 0 || position+width > 64 {
		panic(OutOfRange{At: len(w.out), Expected: "a bitfield of " + itoa(width) + " bits at " + itoa(position) + ", inside 64"})
	}
	if width < 64 && value > (uint64(1)<<width)-1 {
		panic(OutOfRange{At: len(w.out), Expected: "a value inside " + itoa(width) + " bits"})
	}
	return value << position
}

// WriteBitset writes a bitset, as the anchor integer its bitfields pack
// into. DDS-XTypes 1.3 has no serialisation rule for a bitset; the
// anchor's width comes from the layout planner.
func (w *Writer) WriteBitset(anchor int, value uint64) {
	switch anchor {
	case 1:
		w.WriteU8(uint8(value))
	case 2:
		w.WriteU16(uint16(value))
	case 4:
		w.WriteU32(uint32(value))
	case 8:
		w.WriteU64(value)
	default:
		panic(OutOfRange{At: len(w.out), Expected: "a bitset anchor of 1, 2, 4 or 8 bytes, not " + itoa(anchor)})
	}
}

// -- framing -------------------------------------------------------------

// WriteDheader writes a DHEADER: a four-byte length the members that
// follow occupy, written as a placeholder and patched when they are
// done.
func (w *Writer) WriteDheader() DHeader {
	w.alignTo(4)
	at := len(w.out)
	w.out = append(w.out, 0, 0, 0, 0)
	return DHeader{At: at}
}

// FinishDheader patches a DHEADER with the length of what followed it.
// A header from NoDheader is not there and is ignored.
func (w *Writer) FinishDheader(header DHeader) {
	if header.At < 0 {
		return
	}
	w.patch(header.At, uint64(len(w.out)-header.At-4), 4)
}

// WriteEmheader writes an XCDR2 EMHEADER: the member id, its length
// code and the must-understand flag in one 32-bit word. XTypes 1.3
// §7.4.3.4.2. LC4 is followed by a NEXTINT placeholder; LC5 to LC7
// reuse the member's own leading word as the NEXTINT, so none is
// reserved for them.
func (w *Writer) WriteEmheader(memberID int, lengthCode int, mustUnderstand bool) EmHeader {
	// §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and §7.4.3.4.2
	// leaves them the low 28 bits of the word. Masking a larger one writes
	// a well-formed header for `id & 0x0FFFFFFF`: the peer reads a member
	// the caller did not name, and nothing at either end says so.
	if memberID < 0 || memberID > eMemberIDMask {
		panic(OutOfRange{At: len(w.out), Expected: fmt.Sprintf(
			"a member id inside the 28 bits of an EMHEADER, not %d", memberID)})
	}
	w.alignTo(4)
	word := uint32(lengthCode&0x7)<<28 | uint32(memberID)&eMemberIDMask
	if mustUnderstand {
		word |= 1 << 31
	}
	w.WriteU32(word)
	wordAt := len(w.out) - 4
	if lengthCode == 4 {
		at := len(w.out)
		w.out = append(w.out, 0, 0, 0, 0)
		return EmHeader{NextInt: at, Word: wordAt, LengthCode: lengthCode}
	}
	return EmHeader{NextInt: -1, Word: wordAt, LengthCode: lengthCode}
}

// FinishEmheader patches an EMHEADER's NEXTINT with the length of what
// followed. Under ShortLengthCodes an LC4 header whose member turned
// out to be 1, 2, 4 or 8 bytes long instead loses its NEXTINT and
// states the length in the code, which is what Fast DDS does.
func (w *Writer) FinishEmheader(header EmHeader) {
	if header.NextInt < 0 {
		// Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
		// octets, §7.4.3.4.2. A value of another size misdeclares it and
		// the reader's skip lands inside data, so the mismatch is refused
		// here rather than written. Codes 5 to 7 declare nothing of their
		// own: the member's leading length word is the NEXTINT.
		if header.LengthCode >= 0 && header.LengthCode <= 3 && header.Word+4 <= len(w.out) {
			declared := 1 << uint(header.LengthCode)
			body := len(w.out) - header.Word - 4
			if body != declared {
				panic(OutOfRange{At: len(w.out), Expected: fmt.Sprintf(
					"a member of %d octets under length code %d, not %d",
					declared, header.LengthCode, body)})
			}
		}
		return
	}
	length := len(w.out) - header.NextInt - 4
	if w.ShortLengthCodes && (length == 1 || length == 2 || length == 4 || length == 8) {
		w.shortenEmheader(header, length)
		return
	}
	w.patch(header.NextInt, uint64(length), 4)
}

// shortenEmheader removes an LC4 header's NEXTINT and raises its length
// code. Safe only under XCDR2, where MAXALIGN is four: the value began
// at a four-aligned offset with the NEXTINT in front of it and begins
// at a four-aligned offset without it.
func (w *Writer) shortenEmheader(header EmHeader, length int) {
	var code uint32
	switch length {
	case 1:
		code = 0
	case 2:
		code = 1
	case 4:
		code = 2
	default:
		code = 3
	}
	w.out = append(w.out[:header.NextInt], w.out[header.NextInt+4:]...)
	previous := w.readPatched(header.Word)
	w.patch(header.Word, uint64(previous&^uint32(0x7<<28)|code<<28), 4)
}

// readPatched reads the 32-bit word at `at` back, in the stream's
// order.
func (w *Writer) readPatched(at int) uint32 {
	var value uint32
	if w.endianness.IsLittle() {
		for i := 3; i >= 0; i-- {
			value = value<<8 | uint32(w.out[at+i])
		}
	} else {
		for i := 0; i < 4; i++ {
			value = value<<8 | uint32(w.out[at+i])
		}
	}
	return value
}

// WritePid writes a PL_CDR1 parameter header, short form:
// [id:u16][length:u16]. `mustUnderstand` is bit 14 of the id;
// §7.4.1.2.2 sets it if and only if the member declares it, and the
// framing may put flags on every id regardless.
func (w *Writer) WritePid(pid int, role ParameterRole, mustUnderstand bool) Pid {
	// §7.4.1.2.2 gives the short form the ids 0 to HighestShortID
	// inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
	// itself. A short header with a reserved id collides with the
	// protocol's own parameters, so such an id takes the extended form or
	// none.
	if pid < 0 || pid > w.plCdr1.HighestShortID {
		panic(OutOfRange{At: len(w.out), Expected: fmt.Sprintf(
			"a parameter id the short header carries, not %d — the extended header takes it",
			pid)})
	}
	w.alignTo(4)
	if mustUnderstand && w.plCdr1.MemberMustUnderstand {
		pid |= PidMustUnderstand
	}
	pid |= pidFlagsAt(w.plCdr1.PidFlags, "member")
	bare := pid &^ (PidMustUnderstand | PidImplExtension)
	w.WriteU16(uint16(pid))
	at := len(w.out)
	w.out = append(w.out, 0, 0)
	return Pid{
		At:             at,
		Extended:       false,
		SavedOrigin:    w.beginParameterValue(),
		Role:           role,
		Pid:            bare,
		MustUnderstand: mustUnderstand,
	}
}

// WritePidExtended writes a PL_CDR1 parameter header, extended form:
// [PID_EXTENDED][8][id:u32][length:u32]. XTypes 1.3 §7.4.1.2. The
// member's must-understand is FLAG_2 of the eMemberHeader and not the
// marker's bit above it.
func (w *Writer) WritePidExtended(pid int, role ParameterRole, mustUnderstand bool) Pid {
	// The same 28 bits as an EMHEADER's, here in the eMemberHeader of
	// §7.4.1.2.1. Masking aliases the parameter onto another member
	// exactly as it does there.
	if pid < 0 || pid > eMemberIDMask {
		panic(OutOfRange{At: len(w.out), Expected: fmt.Sprintf(
			"a member id inside the 28 bits of an eMemberHeader, not %d", pid)})
	}
	w.alignTo(4)
	marker := PidExtended | pidFlagsAt(w.plCdr1.PidFlags, "extended-marker")
	w.WriteU16(uint16(marker))
	w.WriteU16(8)
	memberHeader := uint32(pid) & eMemberIDMask
	if mustUnderstand && w.plCdr1.MemberMustUnderstand {
		memberHeader |= eFlagMustUnderstand
	}
	w.WriteU32(memberHeader)
	at := len(w.out)
	w.out = append(w.out, 0, 0, 0, 0)
	return Pid{
		At:          at,
		Extended:    true,
		SavedOrigin: w.beginParameterValue(),
		Role:        role,
	}
}

// beginParameterValue starts a parameter's value, returning the origin
// it replaces: each parameter is its own CDR-encapsulated block, so an
// eight-byte member at the head of one is already aligned. §7.4.1.2.
func (w *Writer) beginParameterValue() int {
	saved := w.origin
	w.origin = len(w.out)
	return saved
}

// FinishPid patches a parameter header with its value's length. The
// declared length covers the value's trailing padding or stops at the
// value, depending on the framing; the padding itself starts the next
// parameter at a four-byte offset measured from the enclosing stream.
func (w *Writer) FinishPid(header Pid) {
	width := 2
	if header.Extended {
		width = 4
	}
	length := len(w.out) - header.At - width
	if w.plCdr1.padsDeclaredLength(header.Role) {
		past := (len(w.out) - header.SavedOrigin) % 4
		if past != 0 {
			length += 4 - past
		}
	}
	if !header.Extended && length > PidShortMaxLen {
		w.promotePid(header, length)
	} else {
		w.patch(header.At, uint64(length), width)
	}
	w.origin = header.SavedOrigin
	// Under Fast DDS's framing the parameter writes no padding of its
	// own — the next header aligns itself, and a final parameter ends
	// the stream where its value does.
	if w.plCdr1.PadAfterValue {
		w.alignTo(4)
	}
}

// promotePid rewrites a short parameter header as the extended form.
// §7.4.1.2.2's ssize is sixteen bits and which values overrun it is a
// property of the sample, so the fallback lives here where the length
// is known. Eight bytes go in between the header and the value;
// XCDR1's MAXALIGN is eight, so every offset inside the value keeps its
// remainder.
func (w *Writer) promotePid(header Pid, length int) {
	start := header.At - 2
	inserted := make([]byte, 0, len(w.out)+8)
	inserted = append(inserted, w.out[:header.At+2]...)
	inserted = append(inserted, 0, 0, 0, 0, 0, 0, 0, 0)
	inserted = append(inserted, w.out[header.At+2:]...)
	w.out = inserted
	marker := PidExtended | pidFlagsAt(w.plCdr1.PidFlags, "extended-marker")
	w.patch(start, uint64(marker), 2)
	w.patch(start+2, 8, 2)
	memberHeader := uint32(header.Pid) & eMemberIDMask
	if header.MustUnderstand && w.plCdr1.MemberMustUnderstand {
		memberHeader |= eFlagMustUnderstand
	}
	w.patch(start+4, uint64(memberHeader), 4)
	w.patch(start+8, uint64(length), 4)
}

// WriteSentinel writes the PID_LIST_END sentinel a PL_CDR1 parameter
// list ends with. The flags come from the framing asked at the
// "sentinel" position: PID_LIST_END is not a member and has no
// declaration of its own.
func (w *Writer) WriteSentinel() {
	w.alignTo(4)
	sentinel := PidListEnd | pidFlagsAt(w.plCdr1.PidFlags, "sentinel")
	w.WriteU16(uint16(sentinel))
	w.WriteU16(0)
}

// patch writes `value` at `at` with `width`, in the stream's order.
func (w *Writer) patch(at int, value uint64, width int) {
	if value >= uint64(1)<<(width*8) {
		panic(OutOfRange{At: at, Expected: "a " + itoa(width) + "-byte length"})
	}
	if w.endianness.IsLittle() {
		for i := 0; i < width; i++ {
			w.out[at+i] = byte(value >> (8 * i))
		}
		return
	}
	for i := 0; i < width; i++ {
		w.out[at+width-1-i] = byte(value >> (8 * i))
	}
}
