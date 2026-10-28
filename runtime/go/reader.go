// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Taking bytes off the wire.
//
// The Go counterpart of crates/wire/src/reader.rs and of
// runtime/python/zerodds_wire/reader.py, and the mirror of writer.go
// clause for clause. A malformed stream panics with a WireError naming
// what was expected and where; a refusal about the *type* sets the
// status and the generated decoder walks on to the end of the sample,
// which is what lets one test at the top of the caller decide what to
// do with it.

package zdwire

import (
	"fmt"
	"math"
	"unicode/utf8"
)

// ReaderMark is everything a two-pass read has to put back: the cursor,
// the stream origin and the status. The status is part of it
// deliberately — a refusal made during a counting pass is a refusal
// about bytes that are about to be read again.
type ReaderMark struct {
	At     int
	Origin int
	Status Status
}

// EmHeaderRead is an EMHEADER as read. NextInt is -1 where the length
// code carries no NEXTINT. Extent is how many octets the member
// occupies after the header word, the NEXTINT included where there is
// one — what a reader skipping an unknown member advances by.
// XTypes 1.3 §7.4.3.4.2.
type EmHeaderRead struct {
	MemberId       int
	LengthCode     int
	MustUnderstand bool
	NextInt        int
	Extent         int
}

// PidRead is a parameter header as read.
type PidRead struct {
	Pid            int
	Length         int
	End            bool
	MustUnderstand bool
	// ImplExtension says the writer set FLAG_IMPL_EXTENSION: the
	// parameter is the writing implementation's own. §7.4.1.2.2.
	ImplExtension bool
	// Reserved says the short-form id is inside §7.4.1.2.2's reserved
	// range, which no member id occupies.
	Reserved bool
	// Ignore is PID_IGNORE, which every consumer shall ignore — with
	// no exception for the must-understand bit.
	Ignore bool
	Role   ParameterRole
	// SavedOrigin is the stream origin in force before this parameter's
	// value block opened, for FinishPidRead to put back.
	SavedOrigin int
	// ValueAt is where the value starts.
	ValueAt int
}

// IsUserMember answers whether this parameter can be a member of the
// reading type: false for a vendor-private parameter and for a reserved
// id. §7.4.1.2.2, §7.2.2.4.4.4.6.
func (p PidRead) IsUserMember() bool {
	return !p.ImplExtension && !p.Reserved && !p.Ignore
}

// Reader reads one sample's bytes.
type Reader struct {
	data           []byte
	at             int
	endianness     Endianness
	maxAlign       int
	origin         int
	pidFlags       string
	highestShortID int
	status         Status
}

// NewReader builds a reader over `data`, capped at `maxAlign` — four
// for XCDR2, eight for XCDR1. `pidFlags` is the writer's framing, which
// the reader needs to tell a vendor-private parameter from a user
// member under "impl-extension-members"; `highestShortID` is where the
// short form's reserved range starts.
func NewReader(data []byte, endianness Endianness, maxAlign int, pidFlags string, highestShortID int) *Reader {
	if maxAlign < 1 {
		maxAlign = 1
	}
	return &Reader{
		data:           data,
		endianness:     endianness,
		maxAlign:       maxAlign,
		pidFlags:       pidFlags,
		highestShortID: highestShortID,
		status:         StatusOk,
	}
}

// -- the status a generated decoder reads --------------------------------

// ReaderStatus answers how the read is going.
func (r *Reader) ReaderStatus() Status { return r.status }

// ReaderRefuse refuses the stream from outside the runtime. The first
// refusal sticks: everything read after it is read at a position no
// writer need have put anything at, and a second refusal would describe
// that rather than the fault. StatusOk refuses nothing.
func (r *Reader) ReaderRefuse(refusal Status) {
	if refusal != StatusOk && r.status == StatusOk {
		r.status = refusal
	}
}

// ReaderClearDiscard takes back a StatusDiscarded, and only that one:
// the site whose @try_construct is USE_DEFAULT beside an explicit
// @default asks for DISCARD and clears it here. A truncated stream is
// not a narrowing and stays.
func (r *Reader) ReaderClearDiscard() bool {
	if r.status == StatusDiscarded {
		r.status = StatusOk
		return true
	}
	return false
}

// ReaderPosition answers where the reader is.
func (r *Reader) ReaderPosition() int { return r.at }

// ReaderRemaining answers how many bytes are left.
func (r *Reader) ReaderRemaining() int {
	if r.at > len(r.data) {
		return 0
	}
	return len(r.data) - r.at
}

// ReaderSkipTo moves the cursor forward to `target`, which is never
// backwards: a framing word pointing behind the cursor means the reader
// has already read past what it framed, and that is a refusal about the
// stream's shape and not a seek. Past the end panics, because there is
// no position there to carry on from.
func (r *Reader) ReaderSkipTo(target int) {
	if r.status != StatusOk {
		return
	}
	if target < r.at {
		r.ReaderRefuse(StatusOutOfRange)
		return
	}
	if target > len(r.data) {
		panic(Truncated{At: r.at, Needed: target - r.at, Available: r.ReaderRemaining()})
	}
	r.at = target
}

// ReaderSave answers the cursor, the origin and the status, to be put
// back later.
func (r *Reader) ReaderSave() ReaderMark {
	return ReaderMark{At: r.at, Origin: r.origin, Status: r.status}
}

// ReaderRestore puts back what ReaderSave took.
func (r *Reader) ReaderRestore(mark ReaderMark) {
	r.at = mark.At
	r.origin = mark.Origin
	r.status = mark.Status
}

// ReaderBeginPackedElement moves the alignment origin to the current
// position and answers the origin it displaced, for
// ReaderFinishPackedElement. The read-side half of BeginPackedElement;
// see it for the axis and the recorded rows.
func (r *Reader) ReaderBeginPackedElement() int {
	saved := r.origin
	r.origin = r.at
	return saved
}

// ReaderFinishPackedElement restores the origin
// ReaderBeginPackedElement displaced.
func (r *Reader) ReaderFinishPackedElement(saved int) {
	r.origin = saved
}

// -- the cursor ----------------------------------------------------------

// alignTo skips padding to `align`, counted from the stream's origin.
func (r *Reader) alignTo(align int) {
	if align > r.maxAlign {
		align = r.maxAlign
	}
	if align <= 1 {
		return
	}
	offset := r.at - r.origin
	r.at += (align - offset%align) % align
}

// take answers the next `count` bytes, panicking where the stream ends
// inside them.
func (r *Reader) take(count int) []byte {
	end := r.at + count
	if end > len(r.data) {
		panic(Truncated{At: r.at, Needed: count, Available: r.ReaderRemaining()})
	}
	chunk := r.data[r.at:end]
	r.at = end
	return chunk
}

// readUnsigned aligns to `width` and reads that many bytes in the
// stream's order.
func (r *Reader) readUnsigned(width int) uint64 {
	r.alignTo(width)
	raw := r.take(width)
	var value uint64
	if r.endianness.IsLittle() {
		for i := width - 1; i >= 0; i-- {
			value = value<<8 | uint64(raw[i])
		}
	} else {
		for i := 0; i < width; i++ {
			value = value<<8 | uint64(raw[i])
		}
	}
	return value
}

// -- primitives ----------------------------------------------------------

// ReadBool reads a boolean. A byte that is neither 0 nor 1 is a stream
// that disagrees with its own type, and is refused rather than coerced.
func (r *Reader) ReadBool() bool {
	at := r.at
	value := r.take(1)[0]
	if value == 0 {
		return false
	}
	if value == 1 {
		return true
	}
	panic(OutOfRange{At: at, Expected: "a boolean of 0 or 1"})
}

// ReadU8 reads an octet.
func (r *Reader) ReadU8() uint8 { return r.take(1)[0] }

// ReadI8 reads an int8.
func (r *Reader) ReadI8() int8 { return int8(r.take(1)[0]) }

// ReadChar reads a char: one octet.
func (r *Reader) ReadChar() uint8 { return r.take(1)[0] }

// ReadI16 reads a short.
func (r *Reader) ReadI16() int16 { return int16(r.readUnsigned(2)) }

// ReadU16 reads an unsigned short.
func (r *Reader) ReadU16() uint16 { return uint16(r.readUnsigned(2)) }

// ReadI32 reads a long.
func (r *Reader) ReadI32() int32 { return int32(r.readUnsigned(4)) }

// ReadU32 reads an unsigned long.
func (r *Reader) ReadU32() uint32 { return uint32(r.readUnsigned(4)) }

// ReadI64 reads a long long.
func (r *Reader) ReadI64() int64 { return int64(r.readUnsigned(8)) }

// ReadU64 reads an unsigned long long.
func (r *Reader) ReadU64() uint64 { return r.readUnsigned(8) }

// ReadF32 reads a float.
func (r *Reader) ReadF32() float32 {
	return math.Float32frombits(uint32(r.readUnsigned(4)))
}

// ReadF64 reads a double.
func (r *Reader) ReadF64() float64 {
	return math.Float64frombits(r.readUnsigned(8))
}

// ReadF128 reads a long double: all sixteen bytes, as the exact decimal
// the generated member holds. The counterpart of Writer.WriteF128.
func (r *Reader) ReadF128() string {
	r.alignTo(8)
	raw := r.take(16)
	var bytes [16]byte
	if r.endianness.IsLittle() {
		copy(bytes[:], raw)
	} else {
		for i := 0; i < 16; i++ {
			bytes[i] = raw[15-i]
		}
	}
	return f128ToExactDecimal(bytes)
}

// ReadWchar reads a wchar, at `width` octets and in `framing`'s shape.
// The four-byte form zero-extends one UTF-16 code unit; a value that
// uses the high bits is refused rather than narrowed. A lone surrogate
// is not a code point and is refused too.
func (r *Reader) ReadWchar(width int, framing string, order string) uint16 {
	at := r.at
	if width != 2 && width != 4 {
		panic(OutOfRange{At: at, Expected: "a wchar width of 2 or 4"})
	}
	var value uint32
	switch framing {
	case "giop-length-prefixed":
		declared := int(r.ReadU8())
		if declared != width {
			panic(OutOfRange{At: at, Expected: "a GIOP 1.2 wchar of " + itoa(width) + " octets, the configured transmission code set, and not " + itoa(declared)})
		}
		big := order != "stream" || !r.endianness.IsLittle()
		raw := r.take(width)
		if big {
			for i := 0; i < width; i++ {
				value = value<<8 | uint32(raw[i])
			}
		} else {
			for i := width - 1; i >= 0; i-- {
				value = value<<8 | uint32(raw[i])
			}
		}
	case "fixed":
		if width == 4 {
			value = r.ReadU32()
			if value > 0xFFFF {
				panic(OutOfRange{At: at, Expected: "a four-byte wchar holding one UTF-16 code unit"})
			}
		} else {
			value = uint32(r.ReadU16())
		}
	default:
		panic(OutOfRange{At: at, Expected: "a wchar framing, not `" + framing + "`"})
	}
	if value >= 0xD800 && value <= 0xDFFF {
		panic(OutOfRange{At: at, Expected: "a Unicode code point"})
	}
	return uint16(value)
}

// -- strings -------------------------------------------------------------

// ReadString reads a string: a length including the terminator, the
// bytes, the NUL.
func (r *Reader) ReadString() string {
	at := r.at
	length := int(r.ReadU32())
	if length == 0 || length > r.ReaderRemaining()+1 {
		panic(ImpossibleLength{At: at, Claimed: length, Available: r.ReaderRemaining()})
	}
	text := r.take(length - 1)
	if !utf8.Valid(text) {
		panic(NotUtf8{At: at})
	}
	r.ReadU8()
	return string(text)
}

// ReadStringAlloc is ReadString under the name the shared walk calls it
// by: a Go string owns its storage and there is nothing for the caller
// to decide, so the two names are one method.
func (r *Reader) ReadStringAlloc() string { return r.ReadString() }

// ReadU16Little reads a 16-bit value little-endian whatever the
// stream's order is.
func (r *Reader) ReadU16Little() uint16 {
	r.alignTo(2)
	raw := r.take(2)
	return uint16(raw[0]) | uint16(raw[1])<<8
}

// ReadU16Big reads a 16-bit value big-endian whatever the stream's
// order is.
func (r *Reader) ReadU16Big() uint16 {
	r.alignTo(2)
	raw := r.take(2)
	return uint16(raw[0])<<8 | uint16(raw[1])
}

// ReadU32Little reads a 32-bit value little-endian whatever the
// stream's order is.
func (r *Reader) ReadU32Little() uint32 {
	r.alignTo(4)
	raw := r.take(4)
	return uint32(raw[0]) | uint32(raw[1])<<8 | uint32(raw[2])<<16 | uint32(raw[3])<<24
}

// ReadU32Big reads a 32-bit value big-endian whatever the stream's
// order is.
func (r *Reader) ReadU32Big() uint32 {
	r.alignTo(4)
	raw := r.take(4)
	return uint32(raw[0])<<24 | uint32(raw[1])<<16 | uint32(raw[2])<<8 | uint32(raw[3])
}

// stripByteOrderMark takes the mark off the front and applies what it
// said. The units arrived little-endian; FEFF leaves them as read, FFFE
// swaps every unit back, and no mark means CORBA's GIOP 1.2 default —
// UTF-16 big-endian — so every unit read little-endian is swapped.
func stripByteOrderMark(units []uint16) []uint16 {
	if len(units) == 0 {
		return units
	}
	if units[0] == byteOrderMark {
		return units[1:]
	}
	rest := units
	if units[0] == swappedByteOrderMark {
		rest = units[1:]
	}
	swapped := make([]uint16, len(rest))
	for i, unit := range rest {
		swapped[i] = unit<<8 | unit>>8
	}
	return swapped
}

// ReadWstringUtf8 reads a wstring, in the form `policy` and `order`
// name, answering UTF-8 — which is what the generated member holds.
// Under "specification" the count is octets, so a count the unit width
// does not divide is a stream no encoder produced and is refused rather
// than rounded down. The form is never inferred from the stream.
func (r *Reader) ReadWstringUtf8(policy string, order string) string {
	at := r.at
	declared := int(r.ReadU32())
	if !wstringOrderKnown(order) {
		panic(OutOfRange{At: at, Expected: "a wstring unit order, not `" + order + "`"})
	}
	cell, known := wstringCellOf(policy)
	if !known {
		panic(OutOfRange{At: at, Expected: "a wstring length policy, not `" + policy + "`"})
	}
	count := declared
	if !cell.countsCharacters {
		if declared%cell.width != 0 {
			panic(ImpossibleLength{At: at, Claimed: declared, Available: r.ReaderRemaining()})
		}
		count = declared / cell.width
	}
	units := r.wstringUnits(at, count, cell.width, order)
	if cell.terminated {
		// The count includes the NUL, so a count of zero is a frame
		// that does not hold its own terminator.
		if declared < 1 {
			panic(ImpossibleLength{At: at, Claimed: declared, Available: r.ReaderRemaining()})
		}
		last := units[len(units)-1]
		units = units[:len(units)-1]
		if last != 0 {
			panic(OutOfRange{At: at, Expected: "a NUL character ending the wstring, as its count includes one"})
		}
		// A NUL inside the text is the narrow string's problem in wide
		// form: every C peer stops at it. Only where a terminator is
		// expected — a cell without one may carry U+0000 as a
		// character.
		for _, unit := range units {
			if unit == 0 {
				panic(OutOfRange{At: at, Expected: "no NUL inside a wstring a C peer would stop at"})
			}
		}
	}
	if order == "byte-order-mark" {
		units = stripByteOrderMark(units)
	}
	out := make([]rune, 0, len(units))
	for i := 0; i < len(units); {
		unit := units[i]
		if unit >= 0xD800 && unit <= 0xDBFF && i+1 < len(units) {
			low := units[i+1]
			if low >= 0xDC00 && low <= 0xDFFF {
				out = append(out, rune(0x10000+(int(unit)-0xD800)<<10+(int(low)-0xDC00)))
				i += 2
				continue
			}
		}
		if unit >= 0xD800 && unit <= 0xDFFF {
			panic(OutOfRange{At: at, Expected: "a UTF-16 sequence with no lone surrogate"})
		}
		out = append(out, rune(unit))
		i++
	}
	return string(out)
}

// ReadWstringUtf8Octets is the inverse of WriteWstringUtf8Octets: the
// declared length is the UTF-8 octet count, the terminator `policy` asks
// for is the last of those octets, and a count the stream cannot hold, a
// missing or an interior NUL, or an ill-formed UTF-8 sequence is
// refused. See WstringCodeUnits::Utf8.
func (r *Reader) ReadWstringUtf8Octets(policy string) string {
	at := r.at
	declared := int(r.ReadU32())
	cell, known := wstringCellOf(policy)
	if !known {
		panic(OutOfRange{At: at, Expected: "a wstring length policy, not `" + policy + "`"})
	}
	if declared > r.ReaderRemaining() {
		panic(ImpossibleLength{At: at, Claimed: declared, Available: r.ReaderRemaining()})
	}
	data := r.take(declared)
	if cell.terminated {
		if declared < 1 || data[declared-1] != 0 {
			panic(OutOfRange{At: at, Expected: "a NUL octet ending the wstring, as its count includes one"})
		}
		data = data[:declared-1]
		// A NUL inside the text is the narrow string's problem in wide
		// form: every C peer stops at it.
		for _, octet := range data {
			if octet == 0 {
				panic(OutOfRange{At: at, Expected: "no NUL inside a wstring a C peer would stop at"})
			}
		}
	}
	if !utf8.Valid(data) {
		panic(NotUtf8{At: at})
	}
	return string(data)
}

// wstringUnits reads `count` wstring characters of `width` bytes, as
// UTF-16 units. A four-byte character above 0xFFFF is refused rather
// than truncated — truncating is the defect this whole policy exists
// because of.
func (r *Reader) wstringUnits(at int, count int, width int, order string) []uint16 {
	if count*width > r.ReaderRemaining() {
		panic(ImpossibleLength{At: at, Claimed: count * width, Available: r.ReaderRemaining()})
	}
	little := order == "byte-order-mark" || order == "host-unswapped"
	units := make([]uint16, 0, count)
	for i := 0; i < count; i++ {
		if width == 4 {
			var wide uint32
			switch {
			case order == "big-endian-always":
				wide = r.ReadU32Big()
			case little:
				wide = r.ReadU32Little()
			default:
				wide = r.ReadU32()
			}
			if wide > 0xFFFF {
				panic(OutOfRange{At: at, Expected: "a four-byte wstring character holding one UTF-16 unit"})
			}
			units = append(units, uint16(wide))
			continue
		}
		switch {
		case order == "big-endian-always":
			units = append(units, r.ReadU16Big())
		case little:
			units = append(units, r.ReadU16Little())
		default:
			units = append(units, r.ReadU16())
		}
	}
	return units
}

// ReadFixed reads a fixed<digits, scale>, as the decimal string the
// writer took: ceil((digits + 1) / 2) bytes of BCD, digits
// most-significant first, sign nibble last. The leading zeros go — the
// write side would never produce them from the same value — and a
// negative zero loses its sign, because there is one zero. Under
// "value-precision" the field's end is read out of the octets: the
// field ends at the first octet whose low nibble is a sign, and one
// that never presents a sign within the sixteen octets of the widest
// fixed IDL admits is refused.
func (r *Reader) ReadFixed(digits int, scale int, count string) string {
	if count != "declared" && count != "value-precision" && count != "value-string-digits" {
		panic(OutOfRange{At: r.at, Expected: "a fixed digit count, not `" + count + "`"})
	}
	// The same bound as the writer's, on this side too: OMG IDL 4.2 rule (42)
	// caps the BCD rule at 31 digits, and a reader asked for a wider field
	// would take octets no declaration can name.
	if digits <= 0 || digits > 31 || scale > digits {
		panic(OutOfRange{At: r.at, Expected: fmt.Sprintf(
			"a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<%d, %d>", digits, scale)})
	}
	nibbles := make([]int, 0, digits+2)
	var sign int
	if count == "value-precision" || count == "value-string-digits" {
		for {
			if len(nibbles) >= 32 {
				panic(OutOfRange{At: r.at, Expected: "a sign nibble within sixteen octets"})
			}
			b := r.take(1)[0]
			nibbles = append(nibbles, int(b>>4), int(b&0x0F))
			if b&0x0F >= 0x0A {
				break
			}
		}
		sign = nibbles[len(nibbles)-1]
		nibbles = nibbles[:len(nibbles)-1]
	} else {
		raw := r.take((digits + 2) / 2)
		for _, b := range raw {
			nibbles = append(nibbles, int(b>>4), int(b&0x0F))
		}
		sign = nibbles[len(nibbles)-1]
		nibbles = nibbles[:len(nibbles)-1]
		if (digits+1)%2 == 1 {
			nibbles = nibbles[1:]
		}
	}
	for _, nibble := range nibbles {
		if nibble > 9 {
			panic(OutOfRange{At: r.at, Expected: "BCD digits"})
		}
	}
	// The point sits `scale` digits from the right: under the value's
	// count the field may carry fewer than `digits`, and one that
	// carries fewer than the scale needs its integer part written as a
	// zero.
	text := make([]byte, 0, len(nibbles)+2)
	for _, nibble := range nibbles {
		text = append(text, byte('0'+nibble))
	}
	for len(text) < scale+1 {
		text = append([]byte{'0'}, text...)
	}
	split := len(text) - scale
	whole := text[:split]
	for len(whole) > 1 && whole[0] == '0' {
		whole = whole[1:]
	}
	fraction := text[split:]
	out := string(whole)
	if scale > 0 {
		out = out + "." + string(fraction)
	}
	zero := string(whole) == "0"
	for _, digit := range fraction {
		if digit != '0' {
			zero = false
		}
	}
	if sign == 0x0D && !zero {
		return "-" + out
	}
	return out
}

// -- the trims a bound reaches for ---------------------------------------

// TrimUtf8 cuts `text` to at most `octets` bytes, never inside a
// character. Reached as a method because the shared walk emits the trim
// as a call on the reader.
func (r *Reader) TrimUtf8(text string, octets int) string { return TrimUtf8(text, octets) }

// TrimUtf16 cuts `text` to at most `units` UTF-16 code units. A scalar
// above the basic plane costs two units and is left out whole.
// §7.2.2.7, Table 11.
func (r *Reader) TrimUtf16(text string, units int) string { return TrimUtf16(text, units) }

// -- the bit-bounded kinds -----------------------------------------------

// ReadEnum reads an enumerator's holder, as the signed integer
// §7.4.3.5.3 makes it — the holder's width and not the bound's, so
// @bit_bound(3) is carried in an Int8 and the sign bit is bit 7.
func (r *Reader) ReadEnum(bitBound int) int64 {
	if bitBound == 0 || bitBound > 32 {
		panic(OutOfRange{At: r.at, Expected: "an @bit_bound between 1 and 32, not " + itoa(bitBound)})
	}
	switch {
	case bitBound <= 8:
		return int64(int8(r.ReadU8()))
	case bitBound <= 16:
		return int64(int16(r.ReadU16()))
	default:
		return int64(int32(r.ReadU32()))
	}
}

// ReadBitmask reads a bitmask's holder. Table 33 of XTypes 1.3
// §7.4.1.1.3.2.
func (r *Reader) ReadBitmask(bitBound int) uint64 {
	if bitBound == 0 || bitBound > 64 {
		panic(OutOfRange{At: r.at, Expected: "an @bit_bound between 1 and 64, not " + itoa(bitBound)})
	}
	switch {
	case bitBound <= 8:
		return uint64(r.ReadU8())
	case bitBound <= 16:
		return uint64(r.ReadU16())
	case bitBound <= 32:
		return uint64(r.ReadU32())
	default:
		return r.ReadU64()
	}
}

// ReadBitset reads a bitset's anchor integer, of the width the planner
// sized.
func (r *Reader) ReadBitset(anchor int) uint64 {
	switch anchor {
	case 1:
		return uint64(r.ReadU8())
	case 2:
		return uint64(r.ReadU16())
	case 4:
		return uint64(r.ReadU32())
	case 8:
		return r.ReadU64()
	}
	panic(OutOfRange{At: r.at, Expected: "a bitset anchor of 1, 2, 4 or 8 bytes, not " + itoa(anchor)})
}

// -- framing -------------------------------------------------------------

// ReadDheader reads a DHEADER: the length the members that follow
// occupy, refusing one the stream cannot hold.
func (r *Reader) ReadDheader() uint32 {
	at := r.at
	length := r.ReadU32()
	if int(length) > r.ReaderRemaining() {
		panic(ImpossibleLength{At: at, Claimed: int(length), Available: r.ReaderRemaining()})
	}
	return length
}

// ReadEmheader reads an EMHEADER, unpacked into its parts and the
// NEXTINT it carries. Codes 5 to 7 report the NEXTINT they read, but
// the reader is put back before it: that word is the member's own
// leading length and the member's decoder has to see it. §7.4.3.4.2.
func (r *Reader) ReadEmheader() EmHeaderRead {
	word := r.ReadU32()
	header := EmHeaderRead{
		MemberId:       int(word & eMemberIDMask),
		LengthCode:     int(word >> 28 & 0x7),
		MustUnderstand: word>>31 != 0,
		NextInt:        -1,
	}
	if header.LengthCode == 4 {
		header.NextInt = int(r.ReadU32())
	} else if header.LengthCode >= 5 {
		at := r.at
		header.NextInt = int(r.ReadU32())
		r.at = at
	}
	header.Extent = emExtent(header)
	return header
}

// emExtent answers how many octets a member occupies after its header
// word: codes 0 to 3 state a size outright; code 4's NEXTINT is the
// length; codes 5 to 7 span their NEXTINT word plus the length it
// states, times one, four or eight.
func emExtent(header EmHeaderRead) int {
	if header.LengthCode <= 3 {
		return 1 << header.LengthCode
	}
	if header.NextInt < 0 {
		return 0
	}
	if header.LengthCode == 4 {
		return header.NextInt
	}
	scale := 1
	switch header.LengthCode {
	case 6:
		scale = 4
	case 7:
		scale = 8
	}
	return 4 + header.NextInt*scale
}

// readPidHeader reads a PL_CDR1 parameter header, in whichever form it
// was written, without opening the value block.
func (r *Reader) readPidHeader(role ParameterRole) PidRead {
	r.alignTo(4)
	pid := int(r.ReadU16())
	length := int(r.ReadU16())
	if pid&pidIDMask == PidExtended&pidIDMask {
		// The four flag bits are not part of the id: §7.4.1.2.1 splits
		// the eMemberHeader into FLAG_1 to FLAG_4 and a 28-bit member
		// id, and FLAG_2 is the member's must-understand.
		memberHeader := r.ReadU32()
		wideLength := int(r.ReadU32())
		return PidRead{
			Pid:            int(memberHeader & eMemberIDMask),
			Length:         wideLength,
			End:            false,
			MustUnderstand: memberHeader&eFlagMustUnderstand != 0,
			ImplExtension:  memberHeader&eFlagImplExtension != 0,
			Role:           role,
		}
	}
	identifier := pid & pidIDMask
	end := identifier == PidListEnd&pidIDMask
	// Under a framing that puts FLAG_IMPL_EXTENSION on every member id
	// the bit carries no information and is reported absent, so a
	// member does not go down the unknown-parameter path.
	implExtension := pid&PidImplExtension != 0 &&
		pidFlagsAt(r.pidFlags, "member")&PidImplExtension == 0
	return PidRead{
		Pid:            identifier,
		Length:         length,
		End:            end,
		MustUnderstand: pid&PidMustUnderstand != 0,
		ImplExtension:  implExtension,
		Reserved:       !end && identifier > r.highestShortID,
		Ignore:         identifier == PidIgnore,
		Role:           role,
	}
}

// ReadPidAs reads a parameter header and opens the value block it
// frames: §7.4.1.2 makes a parameter's value its own stream block, so a
// member's internal alignment is counted from the first octet after the
// header and not from the start of the message. The previous origin
// travels on the header, so FinishPidRead can put it back without a
// stack — parameters do not nest.
func (r *Reader) ReadPidAs(role ParameterRole) PidRead {
	header := r.readPidHeader(role)
	header.SavedOrigin = r.origin
	header.ValueAt = r.at
	r.origin = r.at
	return header
}

// FinishPidRead ends a parameter's value and steps to the next header:
// the origin goes back, and the padding to four is stepped against the
// enclosing stream. Nothing follows PID_LIST_END, and a reader that has
// already refused does not move.
func (r *Reader) FinishPidRead(header PidRead) {
	r.origin = header.SavedOrigin
	if r.status != StatusOk || header.End {
		return
	}
	r.alignTo(4)
}
