// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The wire-core operations this SDK grew so that a caller can write every
// construct XTypes 1.3 defines: the 64-bit float, the XCDR1 alignment cap, the
// delimiter, the member header in all eight length codes, the PL_CDR1 parameter
// list, the wide character and string, the packed decimal and the bitset
// anchor.
//
// The expected octets are not this file's opinion. Every multi-member case is
// the byte string `tests/interop-results/<case>/zerodds-rust/<encoding>.<endian>.bin`
// already holds, named in the test, so a change here that agreed with itself and
// not with the reference column fails.
//
// Each operation is exercised at the ends of its range and not only in the
// middle, because that is where the last four defects in these bindings were.

package zerodds

import (
	"bytes"
	"math"
	"testing"
)

func hex(t *testing.T, w *Writer) string {
	t.Helper()
	const digits = "0123456789abcdef"
	out := make([]byte, 0, len(w.Buf)*2)
	for _, b := range w.Buf {
		out = append(out, digits[b>>4], digits[b&0x0F])
	}
	return string(out)
}

// --- the alignment cap the encoding sets ---------------------------------

func TestXcdr1AlignsEightAndXcdr2AlignsFour(t *testing.T) {
	// XTypes 1.3 §7.4.1. One octet, then a `long long`: under XCDR1 the
	// 64-bit value aligns to 8 and takes seven octets of padding; under XCDR2
	// the cap is 4 and it takes three.
	one := NewWriter(Little, Xcdr1)
	one.PutU8(0xAB)
	one.PutU64(0x0102030405060708)
	if got, want := hex(t, one), "ab000000000000000807060504030201"; got != want {
		t.Fatalf("XCDR1: got %s want %s", got, want)
	}
	two := NewWriter(Little, Xcdr2)
	two.PutU8(0xAB)
	two.PutU64(0x0102030405060708)
	if got, want := hex(t, two), "ab0000000807060504030201"; got != want {
		t.Fatalf("XCDR2: got %s want %s", got, want)
	}
	// The cap is the encoding's and not a constant: a writer made for XCDR2
	// must not be able to reach eight.
	if NewWriter(Big, Xcdr1).MaxAlign != 8 || NewWriter(Big, Xcdr2).MaxAlign != 4 {
		t.Fatal("the encoding does not decide MaxAlign")
	}
}

// --- the 64-bit float ----------------------------------------------------

func TestPutF64WritesTheBitPattern(t *testing.T) {
	// π as `tests/interop-corpus/struct-mutable` carries it, and the two ends
	// of the exponent range either side of it.
	for _, tc := range []struct {
		value float64
		want  string
	}{
		{3.141592653589793, "182d4454fb210940"},
		{0, "0000000000000000"},
		// `math.Copysign(0, -1)` and not `-0`: Go's untyped constant `-0`
		// has the value `+0`, so the row that stood here tested zero a
		// second time and never reached negative zero at all. IEEE 754
		// gives it its own bit pattern and §7.4.1.1.1 puts the pattern on
		// the wire, so a writer that normalised it would lose a value the
		// sample carried — which is what this row is here to catch.
		{math.Copysign(0, -1), "0000000000000080"},
		{1.7976931348623157e308, "ffffffffffffef7f"}, // MAX
		{5e-324, "0100000000000000"},                 // the smallest subnormal
	} {
		w := NewWriter(Little, Xcdr2)
		w.PutF64(tc.value)
		if got := hex(t, w); got != tc.want {
			t.Errorf("PutF64(%v): got %s want %s", tc.value, got, tc.want)
		}
	}
}

// --- DHEADER -------------------------------------------------------------

func TestDHeaderCountsTheBodyAndNotItself(t *testing.T) {
	// XTypes 1.3 §7.4.3.5. Three octets of content, so the DHEADER reads 3
	// and the four octets it occupies are not in it.
	w := NewWriter(Little, Xcdr2)
	mark := w.DHeaderBegin()
	w.PutU8(1)
	w.PutU8(2)
	w.PutU8(3)
	w.DHeaderEnd(mark)
	if got, want := hex(t, w), "03000000010203"; got != want {
		t.Fatalf("got %s want %s", got, want)
	}
	// An empty body is zero and not the four octets of the header.
	e := NewWriter(Big, Xcdr2)
	e.DHeaderEnd(e.DHeaderBegin())
	if got, want := hex(t, e), "00000000"; got != want {
		t.Fatalf("empty: got %s want %s", got, want)
	}
}

// --- the member header, in every length code ----------------------------

func TestStructMutableXcdr2MatchesTheRustColumn(t *testing.T) {
	// tests/interop-results/struct-mutable/zerodds-rust/plain-cdr2.le.bin —
	// `@mutable struct Sample { long a; double b; short c; }` with
	// a = -1234567, b = π, c = -2. LC2, LC3 and LC1: the length is in the
	// code and no NEXTINT is written for any of them.
	w := NewWriter(Little, Xcdr2)
	d := w.DHeaderBegin()
	m := w.EmHeaderBegin(0, false, Lc2)
	w.PutU32(0xFFED2979)
	w.EmHeaderEnd(m, Lc2)
	m = w.EmHeaderBegin(1, false, Lc3)
	w.PutF64(3.141592653589793)
	w.EmHeaderEnd(m, Lc3)
	m = w.EmHeaderBegin(2, false, Lc1)
	w.PutU16(0xFFFE)
	w.EmHeaderEnd(m, Lc1)
	w.DHeaderEnd(d)
	want := "1a000000" + "00000020" + "7929edff" +
		"01000030" + "182d4454fb210940" + "02000010" + "feff"
	if got := hex(t, w); got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

func TestLc4WritesANextIntAndLc5ReusesTheBodysOwnWord(t *testing.T) {
	// §7.4.3.4.2. LC4 reserves a NEXTINT of its own; LC5 does not, because
	// the member's leading word already is the length and writing it twice
	// puts every following member at the wrong offset.
	four := NewWriter(Little, Xcdr2)
	m := four.EmHeaderBegin(2, false, Lc4)
	four.PutU16(0xFFFF)
	four.PutU16(0x0002)
	four.EmHeaderEnd(m, Lc4)
	if got, want := hex(t, four), "02000040"+"04000000"+"ffff0200"; got != want {
		t.Fatalf("LC4: got %s want %s", got, want)
	}
	five := NewWriter(Little, Xcdr2)
	m = five.EmHeaderBegin(0, false, Lc5)
	five.PutString("abc")
	five.EmHeaderEnd(m, Lc5)
	if got, want := hex(t, five), "00000050"+"04000000"+"61626300"; got != want {
		t.Fatalf("LC5: got %s want %s", got, want)
	}
}

func TestTheMustUnderstandFlagIsBitThirtyOne(t *testing.T) {
	w := NewWriter(Little, Xcdr2)
	m := w.EmHeaderBegin(0, true, Lc2)
	w.PutU32(0)
	w.EmHeaderEnd(m, Lc2)
	// 0x80000000 | (2 << 28) | 0 = 0xA0000000.
	if got, want := hex(t, w), "000000a0"+"00000000"; got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

func TestAMemberIdPastTwentyEightBitsIsRefused(t *testing.T) {
	// The top of the range: 0x0FFFFFFF fits and 0x10000000 does not. A
	// truncating header would put a different member's id on the wire.
	NewWriter(Little, Xcdr2).EmHeaderBegin(0x0FFF_FFFF, false, Lc2)
	defer func() {
		if recover() == nil {
			t.Fatal("a 29-bit member id was accepted")
		}
	}()
	NewWriter(Little, Xcdr2).EmHeaderBegin(0x1000_0000, false, Lc2)
}

func TestALengthCodeThatDisagreesWithTheBodyIsRefused(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Fatal("LC2 accepted a body that was not four octets")
		}
	}()
	w := NewWriter(Little, Xcdr2)
	m := w.EmHeaderBegin(0, false, Lc2)
	w.PutU16(1)
	w.EmHeaderEnd(m, Lc2)
}

// --- the PL_CDR1 parameter list -----------------------------------------

func TestStructMutableXcdr1MatchesTheRustColumn(t *testing.T) {
	// tests/interop-results/struct-mutable/zerodds-rust/pl-cdr1.le.bin. The
	// three things this pins: the `double`'s parameter declares 8 and not 12,
	// because its value aligns against its own block; the `short`'s declares
	// 4 and not 2, because the padding that brings the next header back to a
	// four-octet boundary is inside the length; and the sentinel carries the
	// must-understand flag, so it is 0x7F02 and not 0x3F02.
	w := NewWriter(Little, Xcdr1)
	p := w.PidBegin(0, false)
	w.PutU32(0xFFED2979)
	w.PidEnd(p)
	p = w.PidBegin(1, false)
	w.PutF64(3.141592653589793)
	w.PidEnd(p)
	p = w.PidBegin(2, false)
	w.PutU16(0xFFFE)
	w.PidEnd(p)
	w.PidListEnd()
	want := "00000400" + "7929edff" +
		"01000800" + "182d4454fb210940" +
		"02000400" + "feff0000" + "027f0000"
	if got := hex(t, w); got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

func TestStructMutableXcdr1BigEndian(t *testing.T) {
	// The same case's pl-cdr1.be.bin. Both halves of the header go out in the
	// stream's order, so the id is `0000` and the length `0004` — not the
	// octet-swapped pair a per-field constant would give.
	w := NewWriter(Big, Xcdr1)
	p := w.PidBegin(0, false)
	w.PutU32(0xFFED2979)
	w.PidEnd(p)
	p = w.PidBegin(1, false)
	w.PutF64(3.141592653589793)
	w.PidEnd(p)
	p = w.PidBegin(2, false)
	w.PutU16(0xFFFE)
	w.PidEnd(p)
	w.PidListEnd()
	want := "00000004" + "ffed2979" +
		"00010008" + "400921fb54442d18" +
		"00020004" + "fffe0000" + "7f020000"
	if got := hex(t, w); got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

func TestTheExtendedParameterHeaderMatchesTheRustColumn(t *testing.T) {
	// tests/interop-results/annotation-id-sparse/zerodds-rust/pl-cdr1.le.bin.
	// The id 16383 is past 0x3F00 and forces the long form even though the
	// value is four octets. The marker carries the must-understand flag and
	// the member does not, which is the trap: reading the flag off the marker
	// makes every parameter must-understand.
	w := NewWriter(Little, Xcdr1)
	p := w.PidBegin(0, false)
	w.PutU32(0xFFED2979)
	w.PidEnd(p)
	p = w.PidBegin(1, false)
	w.PutU32(0x7FFFFFFF)
	w.PidEnd(p)
	p = w.PidExtendedBegin(16383, false)
	w.PutU32(0xFFFFFFFE)
	w.PidEnd(p)
	p = w.PidExtendedBegin(268435455, false)
	w.PutU32(0x00000001)
	w.PidEnd(p)
	w.PidListEnd()
	want := "00000400" + "7929edff" +
		"01000400" + "ffffff7f" +
		"017f0800" + "ff3f0000" + "04000000" + "feffffff" +
		"017f0800" + "ffffff0f" + "04000000" + "01000000" +
		"027f0000"
	if got := hex(t, w); got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

func TestTheParameterMustUnderstandFlagIsBitFourteen(t *testing.T) {
	// tests/interop-results/annotation-must-understand — the id 0 with the
	// flag reads `0040` in a little-endian stream.
	w := NewWriter(Little, Xcdr1)
	p := w.PidBegin(0, true)
	w.PutU32(0xFFED2979)
	w.PidEnd(p)
	if got, want := hex(t, w), "00400400"+"7929edff"; got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

func TestAnIdPastTheShortRangeIsRefusedByTheShortHeader(t *testing.T) {
	// 0x3F00 is the last id the short form carries, inclusive.
	NewWriter(Little, Xcdr1).PidBegin(0x3F00, false)
	defer func() {
		if recover() == nil {
			t.Fatal("a short header accepted an id past 0x3F00")
		}
	}()
	NewWriter(Little, Xcdr1).PidBegin(0x3F01, false)
}

func TestAnAbsentOptionalParameterKeepsItsHeader(t *testing.T) {
	// tests/interop-results/annotation-optional-absent/zerodds-rust/pl-cdr1.le.bin.
	// The header is written either way and declares zero; dropping it leaves
	// the sample four octets short of what every peer counts on.
	w := NewWriter(Little, Xcdr1)
	w.PutU32(0xFFED2979)
	w.PidEnd(w.PidBegin(1, false))
	w.PutU32(0xFFFFFFFE)
	if got, want := hex(t, w), "7929edff"+"01000000"+"feffffff"; got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

// --- the wide character and string --------------------------------------

func TestWcharIsTwoOctetsInTheStreamsOrder(t *testing.T) {
	// tests/interop-results/primitive-wchar — `ä` is U+00E4 and `ሴ` U+1234.
	le := NewWriter(Little, Xcdr2)
	le.PutWchar(0x00E4)
	le.PutWchar(0x1234)
	if got, want := hex(t, le), "e4003412"; got != want {
		t.Fatalf("le: got %s want %s", got, want)
	}
	be := NewWriter(Big, Xcdr2)
	be.PutWchar(0x00E4)
	be.PutWchar(0x1234)
	if got, want := hex(t, be), "00e41234"; got != want {
		t.Fatalf("be: got %s want %s", got, want)
	}
}

func TestWcharAlignsToTwo(t *testing.T) {
	// tests/interop-results/char-and-wchar — `char c; wchar w;` with 'a' and
	// 'z': one octet of padding before the wide one.
	w := NewWriter(Little, Xcdr2)
	w.PutChar('a')
	w.PutWchar('z')
	if got, want := hex(t, w), "61007a00"; got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

func TestACodePointPastTheBmpHasNoWcharForm(t *testing.T) {
	// U+FFFF is the last one that fits and U+10000 the first that does not.
	// The refusal is the code point's and not the width's: a four-octet form
	// would zero-extend a code *unit*, not a code point.
	NewWriter(Little, Xcdr2).PutWchar(0xFFFF)
	defer func() {
		if recover() == nil {
			t.Fatal("U+10000 was accepted as a wchar")
		}
	}()
	NewWriter(Little, Xcdr2).PutWchar(0x10000)
}

func TestACodePointPastLatin1HasNoCharForm(t *testing.T) {
	// U+0161's low octet is 0x61, which is `a` — a truncating writer produces
	// a frame of the right length holding a plausible, wrong character.
	NewWriter(Little, Xcdr2).PutChar(0xFF)
	defer func() {
		if recover() == nil {
			t.Fatal("U+0161 was accepted as a char")
		}
	}()
	NewWriter(Little, Xcdr2).PutChar(0x0161)
}

func TestWstringLengthCountsOctets(t *testing.T) {
	// tests/interop-results/wstring-unbounded-utf8 — "äöü ሴ Z" is seven code
	// units and the length word reads 14, not 7. No terminator, no
	// byte-order mark.
	w := NewWriter(Little, Xcdr2)
	w.PutWstring([]uint16{0x00E4, 0x00F6, 0x00FC, 0x0020, 0x1234, 0x0020, 0x005A})
	want := "0e000000" + "e400" + "f600" + "fc00" + "2000" + "3412" + "2000" + "5a00"
	if got := hex(t, w); got != want {
		t.Fatalf("got %s want %s", got, want)
	}
	// The empty one is a length of zero and nothing after it.
	e := NewWriter(Little, Xcdr2)
	e.PutWstring(nil)
	if got, want := hex(t, e), "00000000"; got != want {
		t.Fatalf("empty: got %s want %s", got, want)
	}
}

// --- the bitset anchor ---------------------------------------------------

func TestBitsetAnchorsMatchTheRustColumn(t *testing.T) {
	for _, tc := range []struct {
		name   string
		bits   uint64
		anchor int
		endian Endianness
		want   string
	}{
		// bitset-basic: bitfield<3> low = 5, bitfield<5> high = 17.
		{"bitset-basic", 5 | 17<<3, 1, Little, "8d"},
		// bitset-typed-fields: 5, 1, 4001 over 3 + 1 + 12 bits.
		{"bitset-typed-fields", 5 | 1<<3 | 4001<<4, 2, Little, "1dfa"},
		{"bitset-typed-fields be", 5 | 1<<3 | 4001<<4, 2, Big, "fa1d"},
		// bitset-multiword: 20 + 20 + 24 bits into one eight-octet anchor.
		{"bitset-multiword", 0xFFFFF | 0xBC614E<<40, 8, Little, "ffff0f00004e61bc"},
		{"bitset-multiword be", 0xFFFFF | 0xBC614E<<40, 8, Big, "bc614e00000fffff"},
	} {
		w := NewWriter(tc.endian, Xcdr2)
		w.PutBitset(tc.bits, tc.anchor)
		if got := hex(t, w); got != tc.want {
			t.Errorf("%s: got %s want %s", tc.name, got, tc.want)
		}
	}
}

func TestAnEightOctetAnchorTakesTheEncodingsAlignment(t *testing.T) {
	// The one place a bitset differs between the two encodings: the anchor's
	// own alignment, capped at four under XCDR2.
	one := NewWriter(Little, Xcdr1)
	one.PutU8(0xAB)
	one.PutBitset(1, 8)
	if got, want := hex(t, one), "ab00000000000000"+"0100000000000000"; got != want {
		t.Fatalf("XCDR1: got %s want %s", got, want)
	}
	two := NewWriter(Little, Xcdr2)
	two.PutU8(0xAB)
	two.PutBitset(1, 8)
	if got, want := hex(t, two), "ab000000"+"0100000000000000"; got != want {
		t.Fatalf("XCDR2: got %s want %s", got, want)
	}
}

func TestAnAnchorThatIsNotOneTwoFourOrEightIsRefused(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Fatal("a three-octet anchor was accepted")
		}
	}()
	NewWriter(Little, Xcdr2).PutBitset(0, 3)
}

// --- the packed decimal --------------------------------------------------

func TestFixedMatchesTheRustColumn(t *testing.T) {
	// tests/interop-results/fixed-basic — `fixed<9,3>` with "123456.789" and
	// "-000000.001". P + 1 = 10 is even, so there is no leading zero nibble
	// and each value is five octets.
	w := NewWriter(Little, Xcdr2)
	w.PutFixed("123456.789", 9, 3)
	w.PutFixed("-000000.001", 9, 3)
	w.PutU8(0xAB)
	if got, want := hex(t, w), "123456789c"+"000000001d"+"ab"; got != want {
		t.Fatalf("got %s want %s", got, want)
	}
}

func TestFixedIsIndependentOfTheByteOrder(t *testing.T) {
	// No multi-octet integer is written, so nothing consults the stream's
	// order — which is why fixed-basic records the same octets in all four
	// slots.
	le := NewWriter(Little, Xcdr1)
	le.PutFixed("123456.789", 9, 3)
	be := NewWriter(Big, Xcdr2)
	be.PutFixed("123456.789", 9, 3)
	if !bytes.Equal(le.Buf, be.Buf) {
		t.Fatalf("le %x be %x", le.Buf, be.Buf)
	}
}

func TestFixedAtTheEndsOfItsRange(t *testing.T) {
	for _, tc := range []struct {
		value  string
		digits int
		scale  int
		want   string
	}{
		// An odd digit count pairs the sign with a prepended zero nibble.
		{"1", 1, 0, "1c"},
		{"9", 1, 0, "9c"},
		{"-9", 1, 0, "9d"},
		// P + 1 odd -> a leading zero nibble. P = 2 gives two octets.
		{"12", 2, 0, "012c"},
		// scale == digits: the value is all fraction.
		{"0.123", 3, 3, "123c"},
		{"-0.001", 3, 3, "001d"},
		// A negative zero is written positive: there is one zero to write.
		{"-0", 1, 0, "0c"},
		{"-0.000", 3, 3, "000c"},
		// The widest an IDL fixed goes.
		{"1234567890123456789012345678901", 31, 0, "1234567890123456789012345678901c"},
		{"-1234567890123456789012345678901", 31, 0, "1234567890123456789012345678901d"},
		// Padding on both sides at once.
		{"7.5", 9, 3, "000007500c"},
	} {
		w := NewWriter(Little, Xcdr2)
		w.PutFixed(tc.value, tc.digits, tc.scale)
		if got := hex(t, w); got != tc.want {
			t.Errorf("PutFixed(%q, %d, %d): got %s want %s",
				tc.value, tc.digits, tc.scale, got, tc.want)
		}
		if want := (tc.digits + 2) / 2; len(w.Buf) != want {
			t.Errorf("PutFixed(%q, %d, %d): %d octets, want %d",
				tc.value, tc.digits, tc.scale, len(w.Buf), want)
		}
	}
}

func TestAValueWiderThanItsFixedTypeIsRefused(t *testing.T) {
	for _, tc := range []struct {
		value  string
		digits int
		scale  int
	}{
		{"1234567", 6, 0}, // seven integer digits into six
		{"1.2345", 6, 3},  // four fraction digits into three
		{"1.0", 2, 3},     // a scale wider than the digit count
		{"twelve", 4, 0},  // not a decimal
		{"", 4, 0},        // no digits at all
	} {
		func() {
			defer func() {
				if recover() == nil {
					t.Errorf("PutFixed(%q, %d, %d) was accepted",
						tc.value, tc.digits, tc.scale)
				}
			}()
			NewWriter(Little, Xcdr2).PutFixed(tc.value, tc.digits, tc.scale)
		}()
	}
}

// --- the reading direction ----------------------------------------------

func TestEveryWriteReadsBack(t *testing.T) {
	for _, encoding := range []Encoding{Xcdr1, Xcdr2} {
		for _, endian := range []Endianness{Little, Big} {
			w := NewWriter(endian, encoding)
			w.PutU8(0xFF)
			w.PutBool(true)
			w.PutChar(0xFF)
			w.PutU16(0xFFFF)
			w.PutU32(0xFFFFFFFF)
			w.PutU64(0xFFFFFFFFFFFFFFFF)
			w.PutF32(-3.5)
			w.PutF64(3.141592653589793)
			w.PutWchar(0xFFFF)
			w.PutString("grüße")
			w.PutSeqU8([]byte{0xDE, 0xAD})
			w.PutWstring([]uint16{0xD83D, 0xDE00})
			w.PutBitset(0xBC614E00000FFFFF, 8)
			w.PutFixed("-000000.001", 9, 3)

			r := NewReader(w.Buf, endian, encoding)
			if got := r.GetU8(); got != 0xFF {
				t.Fatalf("GetU8: %v", got)
			}
			if !r.GetBool() {
				t.Fatal("GetBool")
			}
			if got := r.GetChar(); got != 0xFF {
				t.Fatalf("GetChar: %v", got)
			}
			if got := r.GetU16(); got != 0xFFFF {
				t.Fatalf("GetU16: %v", got)
			}
			if got := r.GetU32(); got != 0xFFFFFFFF {
				t.Fatalf("GetU32: %v", got)
			}
			if got := r.GetU64(); got != 0xFFFFFFFFFFFFFFFF {
				t.Fatalf("GetU64: %v", got)
			}
			if got := r.GetF32(); got != -3.5 {
				t.Fatalf("GetF32: %v", got)
			}
			if got := r.GetF64(); got != 3.141592653589793 {
				t.Fatalf("GetF64: %v", got)
			}
			if got := r.GetWchar(); got != 0xFFFF {
				t.Fatalf("GetWchar: %v", got)
			}
			if got := r.GetString(); got != "grüße" {
				t.Fatalf("GetString: %q", got)
			}
			if got := r.GetSeqU8(); !bytes.Equal(got, []byte{0xDE, 0xAD}) {
				t.Fatalf("GetSeqU8: %x", got)
			}
			units := r.GetWstring()
			if len(units) != 2 || units[0] != 0xD83D || units[1] != 0xDE00 {
				t.Fatalf("GetWstring: %v", units)
			}
			if got := r.GetBitset(8); got != 0xBC614E00000FFFFF {
				t.Fatalf("GetBitset: %x", got)
			}
			if got := r.GetFixed(9, 3); got != "-000000.001" {
				t.Fatalf("GetFixed: %q", got)
			}
			if r.Pos != len(w.Buf) {
				t.Fatalf("the reader stopped at %d of %d octets", r.Pos, len(w.Buf))
			}
		}
	}
}

func TestTheFramingReadsBack(t *testing.T) {
	w := NewWriter(Little, Xcdr2)
	d := w.DHeaderBegin()
	for _, lc := range []int{Lc0, Lc1, Lc2, Lc3, Lc4} {
		m := w.EmHeaderBegin(uint32(lc), lc == Lc4, lc)
		switch lc {
		case Lc0:
			w.PutU8(1)
		case Lc1:
			w.PutU16(2)
		case Lc2:
			w.PutU32(3)
		case Lc3:
			w.PutU64(4)
		case Lc4:
			w.PutU16(5)
			w.PutU16(6)
		}
		w.EmHeaderEnd(m, lc)
	}
	m := w.EmHeaderBegin(5, false, Lc5)
	w.PutString("abc")
	w.EmHeaderEnd(m, Lc5)
	w.DHeaderEnd(d)

	r := NewReader(w.Buf, Little, Xcdr2)
	body := r.DHeaderRead()
	if int(body) != len(w.Buf)-4 {
		t.Fatalf("the DHEADER reads %d over a body of %d", body, len(w.Buf)-4)
	}
	for _, want := range []struct {
		lc     int
		extent uint32
		mu     bool
	}{
		{Lc0, 1, false}, {Lc1, 2, false}, {Lc2, 4, false}, {Lc3, 8, false},
		{Lc4, 4, true}, {Lc5, 8, false},
	} {
		id, mu, lc, extent := r.EmHeaderRead()
		if lc != want.lc || extent != want.extent || mu != want.mu || id != uint32(want.lc) {
			t.Fatalf("EmHeaderRead: id=%d mu=%v lc=%d extent=%d, want lc=%d extent=%d mu=%v",
				id, mu, lc, extent, want.lc, want.extent, want.mu)
		}
		r.Pos += int(extent)
	}
}

func TestTheParameterListReadsBack(t *testing.T) {
	w := NewWriter(Little, Xcdr1)
	p := w.PidBegin(0, false)
	w.PutU32(0xFFED2979)
	w.PidEnd(p)
	p = w.PidBegin(1, true)
	w.PutF64(3.141592653589793)
	w.PidEnd(p)
	p = w.PidExtendedBegin(16383, true)
	w.PutU16(0xFFFE)
	w.PidEnd(p)
	w.PidListEnd()

	r := NewReader(w.Buf, Little, Xcdr1)
	for _, want := range []struct {
		id     uint32
		mu     bool
		length uint32
	}{{0, false, 4}, {1, true, 8}, {16383, true, 4}} {
		id, mu, length, end := r.PidRead()
		if end || id != want.id || mu != want.mu || length != want.length {
			t.Fatalf("PidRead: id=%d mu=%v len=%d end=%v, want id=%d mu=%v len=%d",
				id, mu, length, end, want.id, want.mu, want.length)
		}
		r.PidSkip(r.Pos, length)
	}
	if _, _, _, end := r.PidRead(); !end {
		t.Fatal("the sentinel did not end the list")
	}
	if r.Pos != len(w.Buf) {
		t.Fatalf("the reader stopped at %d of %d octets", r.Pos, len(w.Buf))
	}
}
