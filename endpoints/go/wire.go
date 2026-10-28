// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Package zerodds is the native pure-Go endpoint SDK (ADR 0013): a from-scratch
// XCDR wire-core, no cgo, byte-identical to the Rust core and the other SDKs.
// Serialization is by explicit byte order, so the output is independent of the
// host and a big-endian target produces the same wire as an x86-64 host.
package zerodds

import (
	"fmt"
	"math"
)

// Endianness is the wire byte order (the XCDR encapsulation flag, not the host).
type Endianness int

const (
	// Little wire byte order.
	Little Endianness = iota
	// Big wire byte order.
	Big
)

// Encoding is the XCDR version the stream is written in.
//
// It is a constructor argument and has no default. XTypes 1.3 §7.4.1 puts
// MAXALIGN at eight under XCDR1 and four under XCDR2, so a writer that did not
// know which one it was in would be writing XCDR2 and calling it XCDR — which
// is what this SDK did until the alignment cap became this field: `align` read
// a constant four, and every `long long` in an XCDR1 sample landed four octets
// early with nothing in the interface to say so.
type Encoding int

const (
	// Xcdr1 is PLAIN_CDR/PL_CDR — MAXALIGN 8.
	Xcdr1 Encoding = iota
	// Xcdr2 is PLAIN_CDR2/DELIMITED_CDR/PL_CDR2 — MAXALIGN 4.
	Xcdr2
)

// MaxAlign is the alignment cap this encoding imposes. XTypes 1.3 §7.4.1.
func (e Encoding) MaxAlign() int {
	if e == Xcdr1 {
		return 8
	}
	return 4
}

// The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
// occupies bits 28..30 of the EMHEADER and decides how the member's byte length
// is recovered: LC0..LC3 give it from the code alone, LC4..LC7 read it from a
// NEXTINT that follows the header.
const (
	// Lc0 — the member body is one octet.
	Lc0 = 0
	// Lc1 — two octets.
	Lc1 = 1
	// Lc2 — four octets.
	Lc2 = 2
	// Lc3 — eight octets.
	Lc3 = 3
	// Lc4 — a NEXTINT holding the body's byte length follows the EMHEADER.
	Lc4 = 4
	// Lc5 — the body's first four octets are themselves the length, and they
	// are part of the body. NEXTINT is not written a second time.
	Lc5 = 5
	// Lc6 — as LC5, and the length counts four-octet elements.
	Lc6 = 6
	// Lc7 — as LC5, and the length counts eight-octet elements.
	Lc7 = 7
)

// PidListEndID is PID_LIST_END. XTypes 1.3 §7.4.1.2 Table 31.
const PidListEndID = 0x3F02

// PidExtendedID is the PID that introduces the twelve-octet parameter header.
// XTypes 1.3 §7.4.1.2.
const PidExtendedID = 0x3F01

// PidShortMaxID is the highest member id a short parameter header can carry,
// inclusive.
const PidShortMaxID = 0x3F00

// pidFlagMustUnderstand is bit 14 of a short parameter id. XTypes 1.3 §7.4.1.2.
const pidFlagMustUnderstand = 0x4000

// emFlagMustUnderstand is bit 30 of the eMemberHeader inside an extended
// parameter header — not bit 31, which is the impl-extension flag.
const emFlagMustUnderstand = 0x4000_0000

// Writer accumulates XCDR bytes into a growable buffer.
type Writer struct {
	Buf    []byte
	Endian Endianness
	// MaxAlign is the alignment cap, from the Encoding the writer was made
	// with. XTypes 1.3 §7.4.1.
	MaxAlign int
	// origin is the position alignment is measured from. Zero for the stream,
	// and moved to the start of a PL_CDR1 parameter's value for the length of
	// that parameter — §7.4.1.2 aligns a parameter's value against its own
	// block, not against the stream. Without it a `double` in a parameter takes
	// four octets of padding and the header declares twelve where Cyclone, RTI
	// and Fast DDS all declare eight.
	origin int
}

// NewWriter returns a Writer for the given wire byte order and XCDR version.
//
// Both arguments are required. The encoding decides MAXALIGN and nothing in the
// call may leave it implied.
func NewWriter(endian Endianness, encoding Encoding) *Writer {
	return &Writer{Endian: endian, MaxAlign: encoding.MaxAlign()}
}

func (w *Writer) align(a int) {
	if a > w.MaxAlign {
		a = w.MaxAlign
	}
	for ((len(w.Buf) - w.origin) % a) != 0 {
		w.Buf = append(w.Buf, 0)
	}
}

// putLE writes logical-little-endian bytes at the given alignment, reversed for
// a big-endian wire.
func (w *Writer) putLE(a int, le []byte) {
	w.align(a)
	if w.Endian == Big {
		for i := len(le) - 1; i >= 0; i-- {
			w.Buf = append(w.Buf, le[i])
		}
	} else {
		w.Buf = append(w.Buf, le...)
	}
}

// PutU8 appends one byte (no alignment).
func (w *Writer) PutU8(v byte) { w.Buf = append(w.Buf, v) }

// PutBool appends a boolean as the one octet XTypes 1.3 §7.4.1.1.1 Table 30
// gives it: 0 or 1, never any other pattern.
func (w *Writer) PutBool(v bool) {
	if v {
		w.PutU8(1)
	} else {
		w.PutU8(0)
	}
}

// PutU16 appends a 2-aligned uint16.
func (w *Writer) PutU16(v uint16) { w.putLE(2, []byte{byte(v), byte(v >> 8)}) }

// PutU32 appends a 4-aligned uint32.
func (w *Writer) PutU32(v uint32) {
	w.putLE(4, []byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24)})
}

// PutU64 appends a uint64 at its natural eight-octet alignment, which the
// writer's MaxAlign then caps: eight under XCDR1, four under XCDR2. XTypes 1.3
// §7.4.1.
func (w *Writer) PutU64(v uint64) {
	le := make([]byte, 8)
	for i := 0; i < 8; i++ {
		le[i] = byte(v >> (8 * i))
	}
	w.putLE(8, le)
}

// PutF32 appends a float32 as its IEEE-754 bit pattern.
func (w *Writer) PutF32(v float32) { w.PutU32(math.Float32bits(v)) }

// PutF64 appends a float64 as its IEEE-754 bit pattern, at the same alignment
// as PutU64. `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2.
func (w *Writer) PutF64(v float64) { w.PutU64(math.Float64bits(v)) }

// PutBytes appends raw bytes (no alignment).
func (w *Writer) PutBytes(b []byte) { w.Buf = append(w.Buf, b...) }

// PutString appends a CDR string: u32 length (incl. NUL) + bytes + one NUL.
func (w *Writer) PutString(s string) {
	w.PutU32(uint32(len(s) + 1))
	w.PutBytes([]byte(s))
	w.PutU8(0)
}

// PutSeqU8 appends a sequence<octet>: u32 length + raw bytes.
func (w *Writer) PutSeqU8(b []byte) {
	w.PutU32(uint32(len(b)))
	w.PutBytes(b)
}

// PutChar appends an IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30
// gives it.
//
// A code point above U+00FF is refused and not truncated. `š` is U+0161 and its
// low octet is 0x61, which is `a`: a truncating writer produces a frame of the
// right length holding a different, plausible character, and nothing downstream
// can tell. The Rust core refuses `primitive-char-above-latin1` in the same
// words.
func (w *Writer) PutChar(codePoint uint32) {
	if codePoint > 0xFF {
		panic(fmt.Sprintf("expected a character below U+0100, found U+%04X", codePoint))
	}
	w.PutU8(byte(codePoint))
}

// PutWchar appends an IDL `wchar`: one two-octet code unit, 2-aligned, in the
// stream's byte order.
//
// The four policy axes a wide character carries are answered here and not by
// the caller: the unit is two octets wide, it goes out in the stream's byte
// order, there is no GIOP length octet before it, and no byte-order mark.
//
// A code point above U+FFFF is refused. The refusal is width-independent — a
// four-octet form would zero-extend a code *unit*, not a code point — which is
// how the Rust core refuses `primitive-wchar-above-bmp`.
func (w *Writer) PutWchar(codePoint uint32) {
	if codePoint > 0xFFFF {
		panic(fmt.Sprintf("expected a BMP character, found U+%04X", codePoint))
	}
	w.PutU16(uint16(codePoint))
}

// PutWstring appends an IDL `wstring` from its UTF-16 code units.
//
// The length word counts **octets** and not code units, there is no terminator,
// there is no byte-order mark, and each unit goes out two octets wide in the
// stream's own byte order. Those are the four axes, and they are the binding's
// answer — the caller hands over units and nothing else.
func (w *Writer) PutWstring(units []uint16) {
	w.PutU32(uint32(len(units) * 2))
	for _, unit := range units {
		w.PutU16(unit)
	}
}

// PutBitset appends a `bitset` as its anchor word: the holder the declared bits
// pack into, written at the anchor's own width and therefore at the anchor's own
// alignment — eight under XCDR1 and four under XCDR2 for an eight-octet anchor.
// XTypes 1.3 §7.3.1.4 and §7.4.1.
//
// `bits` is the packed holder and `anchor` its width in octets, both properties
// of the declared type. What this call decides is the pair the type does not:
// which write the holder goes out through, and what that write aligns to.
func (w *Writer) PutBitset(bits uint64, anchor int) {
	switch anchor {
	case 1:
		w.PutU8(byte(bits))
	case 2:
		w.PutU16(uint16(bits))
	case 4:
		w.PutU32(uint32(bits))
	case 8:
		w.PutU64(bits)
	default:
		panic(fmt.Sprintf("a bitset anchor is one, two, four or eight octets, not %d", anchor))
	}
}

// PutFixed appends an IDL `fixed<digits,scale>` as packed BCD.
// CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.
//
// `value` is the decimal as the sample wrote it — an optional sign, an integer
// part and an optional fraction. Everything after that is this call's:
//
//   - the integer part is left-padded and the fraction right-padded to exactly
//     `digits` decimal digits, with the point implied `scale` from the right;
//     the scale itself is not on the wire,
//   - a leading zero nibble is prepended when `digits + 1` is odd, so digits and
//     sign pair up,
//   - two nibbles to the octet, most significant digit first, high nibble first,
//   - the last nibble is the sign: 0xD negative, 0xC otherwise. A negative zero
//     goes out positive, because there is one zero to write,
//   - alignment is one, and no multi-octet integer is written, so the byte order
//     never enters.
//
// The octet count is therefore ceil((digits + 1) / 2), whatever the value.
func (w *Writer) PutFixed(value string, digits, scale int) {
	w.PutBytes(fixedOctets(value, digits, scale))
}

// fixedOctets is the packing PutFixed writes, apart so it can be tested at the
// edges of its range without a Writer.
func fixedOctets(value string, digits, scale int) []byte {
	if digits < 1 || digits > 31 {
		panic(fmt.Sprintf("fixed<%d,%d>: a digit count is 1..31", digits, scale))
	}
	if scale < 0 || scale > digits {
		panic(fmt.Sprintf("fixed<%d,%d>: a scale is 0..digits", digits, scale))
	}
	text := value
	negative := false
	if len(text) > 0 && (text[0] == '+' || text[0] == '-') {
		negative = text[0] == '-'
		text = text[1:]
	}
	integral, fraction := text, ""
	if at := indexByte(text, '.'); at >= 0 {
		integral, fraction = text[:at], text[at+1:]
	}
	if integral == "" && fraction == "" {
		panic(fmt.Sprintf("fixed<%d,%d>: `%s` has no digits", digits, scale, value))
	}
	for _, part := range []string{integral, fraction} {
		for i := 0; i < len(part); i++ {
			if part[i] < '0' || part[i] > '9' {
				panic(fmt.Sprintf("fixed<%d,%d>: `%s` is not a decimal", digits, scale, value))
			}
		}
	}
	// Leading zeros carry no value, so they are dropped before the width is
	// checked: `000000.001` is one significant integer digit, not six.
	trimmed := 0
	for trimmed < len(integral)-0 && trimmed < len(integral) && integral[trimmed] == '0' {
		trimmed++
	}
	integral = integral[trimmed:]
	if len(integral) > digits-scale {
		panic(fmt.Sprintf("fixed<%d,%d>: `%s` needs %d integer digits and the type has %d",
			digits, scale, value, len(integral), digits-scale))
	}
	// A trailing zero in the fraction is padding either way, so it is dropped
	// before the width is checked and re-added below.
	for len(fraction) > scale && fraction[len(fraction)-1] == '0' {
		fraction = fraction[:len(fraction)-1]
	}
	if len(fraction) > scale {
		panic(fmt.Sprintf("fixed<%d,%d>: `%s` needs %d fraction digits and the type has %d",
			digits, scale, value, len(fraction), scale))
	}
	nibbles := make([]byte, 0, digits+2)
	if (digits+1)%2 == 1 {
		nibbles = append(nibbles, 0)
	}
	for i := 0; i < digits-scale-len(integral); i++ {
		nibbles = append(nibbles, 0)
	}
	for i := 0; i < len(integral); i++ {
		nibbles = append(nibbles, integral[i]-'0')
	}
	for i := 0; i < len(fraction); i++ {
		nibbles = append(nibbles, fraction[i]-'0')
	}
	for i := 0; i < scale-len(fraction); i++ {
		nibbles = append(nibbles, 0)
	}
	nonZero := false
	for _, n := range nibbles {
		if n != 0 {
			nonZero = true
		}
	}
	if negative && nonZero {
		nibbles = append(nibbles, 0x0D)
	} else {
		nibbles = append(nibbles, 0x0C)
	}
	out := make([]byte, 0, len(nibbles)/2)
	for i := 0; i+1 < len(nibbles); i += 2 {
		out = append(out, nibbles[i]<<4|nibbles[i+1])
	}
	return out
}

func indexByte(s string, c byte) int {
	for i := 0; i < len(s); i++ {
		if s[i] == c {
			return i
		}
	}
	return -1
}

// --- back-patched framing -----------------------------------------------
//
// A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same shape:
// a slot reserved before the body and filled in once the body's length is
// known. `patchU32` is the one place that writes into an already-written
// position, and it honours the wire byte order like every other write.

func (w *Writer) patchU32(at int, v uint32) {
	le := [4]byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24)}
	if w.Endian == Big {
		for i := 0; i < 4; i++ {
			w.Buf[at+i] = le[3-i]
		}
		return
	}
	for i := 0; i < 4; i++ {
		w.Buf[at+i] = le[i]
	}
}

func (w *Writer) patchU16(at int, v uint16) {
	le := [2]byte{byte(v), byte(v >> 8)}
	if w.Endian == Big {
		w.Buf[at] = le[1]
		w.Buf[at+1] = le[0]
		return
	}
	w.Buf[at] = le[0]
	w.Buf[at+1] = le[1]
}

// DHeaderBegin reserves a 4-aligned DHEADER slot and returns the position the
// body starts at, to hand back to DHeaderEnd. XTypes 1.3 §7.4.3.5.
func (w *Writer) DHeaderBegin() int {
	w.PutU32(0)
	return len(w.Buf)
}

// DHeaderEnd back-patches the DHEADER with the byte length written since
// bodyStart. The DHEADER itself is not counted — §7.4.3.5 measures the content
// that follows it.
func (w *Writer) DHeaderEnd(bodyStart int) {
	w.patchU32(bodyStart-4, uint32(len(w.Buf)-bodyStart))
}

// EmHeaderBegin writes the EMHEADER for one member of a @mutable aggregate and
// returns the body-start position. XTypes 1.3 §7.4.3.4.2:
//
//	EMHEADER = (must_understand << 31) | (length_code << 28) | member_id
//
// with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
// EmHeaderEnd fills it in; for LC0..LC3 the code alone gives the length and
// nothing follows; for LC5..LC7 the body's own leading four octets are the
// length, so no separate NEXTINT is written and EmHeaderEnd has nothing to
// patch.
func (w *Writer) EmHeaderBegin(id uint32, mustUnderstand bool, lengthCode int) int {
	if id > 0x0FFF_FFFF {
		panic(fmt.Sprintf("member id %d does not fit the 28 bits of an EMHEADER", id))
	}
	if lengthCode < Lc0 || lengthCode > Lc7 {
		panic(fmt.Sprintf("length code %d is not one of LC0..LC7", lengthCode))
	}
	header := uint32(lengthCode)<<28 | id
	if mustUnderstand {
		header |= 1 << 31
	}
	w.PutU32(header)
	if lengthCode == Lc4 {
		w.PutU32(0)
	}
	return len(w.Buf)
}

// EmHeaderEnd closes the member EmHeaderBegin opened.
//
// It takes the length code back rather than remembering it, so a caller that
// paired an LC4 begin with a differently-coded end gets a refusal instead of a
// NEXTINT patched over the first four octets of a member body.
func (w *Writer) EmHeaderEnd(bodyStart int, lengthCode int) {
	written := len(w.Buf) - bodyStart
	switch lengthCode {
	case Lc4:
		w.patchU32(bodyStart-4, uint32(written))
	case Lc0, Lc1, Lc2, Lc3:
		// 1, 2, 4, 8 — the code *is* the length, so a body of any other
		// size means the plan and the member disagree. Silence here would
		// put a length on the wire that no octet count backs.
		want := 1 << uint(lengthCode)
		if written != want {
			panic(fmt.Sprintf("LC%d frames %d octets and the member wrote %d",
				lengthCode, want, written))
		}
	case Lc5, Lc6, Lc7:
		if written < 4 {
			panic(fmt.Sprintf("LC%d takes its length from the body's first four octets and the member wrote %d",
				lengthCode, written))
		}
	}
}

// Pid is an open PL_CDR1 parameter header, to be handed back to PidEnd.
type Pid struct {
	// at is the position of the length field.
	at int
	// extended says whether the length field is four octets or two.
	extended bool
	// savedOrigin is the alignment origin to restore when the parameter
	// closes.
	savedOrigin int
}

// PidBegin writes a PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a
// four-octet boundary an `unsigned short` parameter id, then an `unsigned
// short` byte length of the value that follows. Both go out in the stream's
// byte order.
//
// The must-understand flag is bit 14 of the id field. Bit 15 is the
// implementation-extension flag and is not set here.
//
// The parameter's value aligns against the block and not against the stream,
// so the alignment origin moves for the length of the parameter and PidEnd
// restores it.
func (w *Writer) PidBegin(pid uint16, mustUnderstand bool) Pid {
	if pid > PidShortMaxID {
		panic(fmt.Sprintf("member id %d needs the extended parameter header", pid))
	}
	w.align(4)
	id := pid
	if mustUnderstand {
		id |= pidFlagMustUnderstand
	}
	w.putLE(2, []byte{byte(id), byte(id >> 8)})
	at := len(w.Buf)
	w.Buf = append(w.Buf, 0, 0)
	saved := w.origin
	w.origin = len(w.Buf)
	return Pid{at: at, extended: false, savedOrigin: saved}
}

// PidExtendedBegin writes the twelve-octet parameter header XTypes 1.3 §7.4.1.2
// requires for a member id above 0x3F00 or a value longer than 65535 octets:
//
//	[PID_EXTENDED | must-understand : u16][8 : u16][eMemberHeader : u32][length : u32]
//
// The must-understand flag on the marker is not the member's — it is on the
// marker under this framing whatever the member says, and the member's own flag
// is bit 30 of the eMemberHeader. Reading it off the marker instead makes every
// parameter must-understand, which is what Cyclone's wire looks like if you do.
func (w *Writer) PidExtendedBegin(memberID uint32, mustUnderstand bool) Pid {
	if memberID > 0x0FFF_FFFF {
		panic(fmt.Sprintf("member id %d does not fit the 28 bits of an eMemberHeader", memberID))
	}
	w.align(4)
	marker := uint16(PidExtendedID) | pidFlagMustUnderstand
	w.putLE(2, []byte{byte(marker), byte(marker >> 8)})
	w.putLE(2, []byte{8, 0})
	header := memberID
	if mustUnderstand {
		header |= emFlagMustUnderstand
	}
	w.putLE(4, []byte{byte(header), byte(header >> 8), byte(header >> 16), byte(header >> 24)})
	at := len(w.Buf)
	w.Buf = append(w.Buf, 0, 0, 0, 0)
	saved := w.origin
	w.origin = len(w.Buf)
	return Pid{at: at, extended: true, savedOrigin: saved}
}

// PidEnd closes the parameter PidBegin or PidExtendedBegin opened.
//
// The declared length counts the value's octets **and** the padding that brings
// the next header back to a four-octet boundary: a reader skips `length` from
// the end of the header and has to land on the next parameter. The padding
// itself is written after the length is patched, so the two cannot disagree.
func (w *Writer) PidEnd(header Pid) {
	width := 2
	if header.extended {
		width = 4
	}
	length := len(w.Buf) - header.at - width
	// The parameter's own origin is still in force here, so the round-up is
	// measured from the block that the length describes.
	if past := (len(w.Buf) - header.savedOrigin) % 4; past != 0 {
		length += 4 - past
	}
	if header.extended {
		w.patchU32(header.at, uint32(length))
	} else {
		if length > 0xFFFF {
			panic(fmt.Sprintf("a short parameter header describes at most 65535 octets, not %d", length))
		}
		w.patchU16(header.at, uint16(length))
	}
	w.origin = header.savedOrigin
	w.align(4)
}

// PidListEnd writes the sentinel that closes a PL_CDR1 parameter list: the id
// PID_LIST_END with a zero length, four-aligned. XTypes 1.3 §7.4.1.2.
//
// The must-understand flag is set on the sentinel — 0x7F02 on the wire, which
// is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02; the flag is
// not the sentinel's meaning either way, and this is the majority form the Rust
// column records.
func (w *Writer) PidListEnd() {
	w.align(4)
	id := uint16(PidListEndID) | pidFlagMustUnderstand
	w.putLE(2, []byte{byte(id), byte(id >> 8)})
	w.putLE(2, []byte{0, 0})
}

// Reader walks an XCDR byte slice.
type Reader struct {
	Buf    []byte
	Pos    int
	Endian Endianness
	// MaxAlign is the alignment cap, from the Encoding the reader was made
	// with — the same cap the writer used, or the fields land elsewhere.
	MaxAlign int
	// origin mirrors the writer's: a PL_CDR1 parameter's value aligns against
	// its own block.
	origin int
}

// NewReader returns a Reader over b for the given wire byte order and XCDR
// version.
func NewReader(b []byte, endian Endianness, encoding Encoding) *Reader {
	return &Reader{Buf: b, Endian: endian, MaxAlign: encoding.MaxAlign()}
}

func (r *Reader) getLE(a, n int) []byte {
	if a > r.MaxAlign {
		a = r.MaxAlign
	}
	for ((r.Pos - r.origin) % a) != 0 {
		r.Pos++
	}
	out := make([]byte, n)
	copy(out, r.Buf[r.Pos:r.Pos+n])
	r.Pos += n
	if r.Endian == Big {
		for i, j := 0, n-1; i < j; i, j = i+1, j-1 {
			out[i], out[j] = out[j], out[i]
		}
	}
	return out
}

// GetU32 reads a 4-aligned uint32.
func (r *Reader) GetU32() uint32 {
	le := r.getLE(4, 4)
	return uint32(le[0]) | uint32(le[1])<<8 | uint32(le[2])<<16 | uint32(le[3])<<24
}

// DHeaderRead reads a DHEADER and returns the announced body byte length.
func (r *Reader) DHeaderRead() uint32 { return r.GetU32() }

// EmHeaderRead reads a member header and returns the member id, the
// must-understand flag, the length code, and the member body's extent in octets
// measured from the position this call leaves the cursor at.
//
// LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that follows
// the header, and the extent is that word. LC5..LC7 do not consume anything: the
// next four octets are the member's own leading word and belong to the body, so
// they are peeked and the extent is 4 + the word for LC5, 4 + 4×word for LC6 and
// 4 + 8×word for LC7 — the last two count elements, not octets.
func (r *Reader) EmHeaderRead() (id uint32, mustUnderstand bool, lengthCode int, extent uint32) {
	header := r.GetU32()
	id = header & 0x0FFF_FFFF
	mustUnderstand = header&(1<<31) != 0
	lengthCode = int((header >> 28) & 0x7)
	switch lengthCode {
	case Lc0, Lc1, Lc2, Lc3:
		extent = uint32(1) << uint(lengthCode)
	case Lc4:
		extent = r.GetU32()
	default:
		at := r.Pos
		word := r.GetU32()
		r.Pos = at
		switch lengthCode {
		case Lc5:
			extent = 4 + word
		case Lc6:
			extent = 4 + 4*word
		default:
			extent = 4 + 8*word
		}
	}
	return id, mustUnderstand, lengthCode, extent
}

// PidRead reads a PL_CDR1 parameter header in either form and returns the
// member id with its flags stripped, the member's must-understand flag, the
// declared byte length, and whether the list ends here.
//
// The extended form is recognised by the marker PID_EXTENDED, and the
// must-understand answer then comes from bit 30 of the eMemberHeader and never
// from the marker — the marker carries the flag under this framing whatever the
// member says.
//
// The value's alignment origin is moved to the position this call leaves, and
// PidSkip restores it.
func (r *Reader) PidRead() (id uint32, mustUnderstand bool, length uint32, listEnd bool) {
	raw := r.GetU16()
	short := raw & 0x3FFF
	if short == PidListEndID {
		r.GetU16()
		return 0, false, 0, true
	}
	if short == PidExtendedID {
		r.GetU16() // the slength, always 8
		header := r.GetU32()
		length = r.GetU32()
		r.origin = r.Pos
		return header & 0x0FFF_FFFF, header&emFlagMustUnderstand != 0, length, false
	}
	length = uint32(r.GetU16())
	r.origin = r.Pos
	return uint32(short), raw&pidFlagMustUnderstand != 0, length, false
}

// PidSkip moves the cursor past a parameter value of `length` octets from
// `bodyStart` and restores the stream's alignment origin.
func (r *Reader) PidSkip(bodyStart int, length uint32) {
	r.Pos = bodyStart + int(length)
	r.origin = 0
	for (r.Pos % 4) != 0 {
		r.Pos++
	}
}

// GetBool reads the one octet a boolean occupies.
func (r *Reader) GetBool() bool { return r.GetU8() != 0 }

// GetF64 reads a float64 from its IEEE-754 bit pattern.
func (r *Reader) GetF64() float64 { return math.Float64frombits(r.GetU64()) }

// GetChar reads the one octet an IDL `char` occupies.
func (r *Reader) GetChar() uint32 { return uint32(r.GetU8()) }

// GetWchar reads one two-octet `wchar` code unit.
func (r *Reader) GetWchar() uint32 { return uint32(r.GetU16()) }

// GetWstring reads a `wstring` back into its UTF-16 code units. The length word
// counts octets, so the unit count is half of it.
func (r *Reader) GetWstring() []uint16 {
	octets := int(r.GetU32())
	units := make([]uint16, 0, octets/2)
	for i := 0; i < octets/2; i++ {
		units = append(units, r.GetU16())
	}
	return units
}

// GetBitset reads a `bitset` anchor of the given width back into its holder
// word.
func (r *Reader) GetBitset(anchor int) uint64 {
	switch anchor {
	case 1:
		return uint64(r.GetU8())
	case 2:
		return uint64(r.GetU16())
	case 4:
		return uint64(r.GetU32())
	case 8:
		return r.GetU64()
	default:
		panic(fmt.Sprintf("a bitset anchor is one, two, four or eight octets, not %d", anchor))
	}
}

// GetFixed reads a `fixed<digits,scale>` back into the decimal it was written
// from: a leading `-` when the sign nibble is 0xD, `digits - scale` integer
// digits, and a `.` with `scale` fraction digits where the scale is not zero.
//
// The scale is not on the wire, so it is an argument here for the same reason it
// is one in PutFixed.
func (r *Reader) GetFixed(digits, scale int) string {
	octets := (digits + 2) / 2
	nibbles := make([]byte, 0, octets*2)
	for i := 0; i < octets; i++ {
		b := r.GetU8()
		nibbles = append(nibbles, b>>4, b&0x0F)
	}
	sign := nibbles[len(nibbles)-1]
	nibbles = nibbles[len(nibbles)-1-digits : len(nibbles)-1]
	out := make([]byte, 0, digits+2)
	if sign == 0x0D {
		out = append(out, '-')
	}
	for i, n := range nibbles {
		if scale != 0 && i == digits-scale {
			out = append(out, '.')
		}
		out = append(out, '0'+n)
	}
	return string(out)
}
