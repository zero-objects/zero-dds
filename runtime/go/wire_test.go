// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// The runtime's own smoke tests. Not the measurement — that is the
// corpus sweep against the Rust column's recorded octets — but the
// checks that catch a misport before a sweep is even possible: the
// endianness of a patch, the sign of an enum holder, the BCD of a
// fixed, and the binary128 conversions whose test vectors are IEEE's
// and not this project's.

package zdwire

import (
	"bytes"
	"testing"
)

func TestIntegersLandInTheStreamOrder(t *testing.T) {
	w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
	w.WriteU8(1)
	w.WriteU32(0x11223344)
	got := w.Finish()
	want := []byte{1, 0, 0, 0, 0x44, 0x33, 0x22, 0x11}
	if !bytes.Equal(got, want) {
		t.Fatalf("little-endian: got % x, want % x", got, want)
	}
	w = NewWriter(BigEndian, 8, SpecificationFraming(), false)
	w.WriteU8(1)
	w.WriteU32(0x11223344)
	got = w.Finish()
	want = []byte{1, 0, 0, 0, 0x11, 0x22, 0x33, 0x44}
	if !bytes.Equal(got, want) {
		t.Fatalf("big-endian: got % x, want % x", got, want)
	}
}

func TestAlignmentIsCappedByMaxAlign(t *testing.T) {
	// XCDR2: MAXALIGN is four, so a u64 after a u8 sits at 4.
	w := NewWriter(LittleEndian, 4, SpecificationFraming(), false)
	w.WriteU8(1)
	w.WriteU64(2)
	if got := len(w.Finish()); got != 12 {
		t.Fatalf("a u64 after a u8 under MAXALIGN 4: %d octets, want 12", got)
	}
}

func TestAStringCountsItsTerminator(t *testing.T) {
	w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
	w.WriteString("ab")
	got := w.Finish()
	want := []byte{3, 0, 0, 0, 'a', 'b', 0}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
	r := NewReader(got, LittleEndian, 8, "sentinel-must-understand", PidShortMaxID)
	if back := r.ReadString(); back != "ab" {
		t.Fatalf("read back %q", back)
	}
}

func TestAWstringIsOctetsOfUtf16Units(t *testing.T) {
	// §7.4.1.1.2: the count is octets, twice the unit count, no BOM
	// and no terminator. "𝄞" is one code point of two units.
	w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
	w.WriteWstringUtf8("𝄞", "specification", "stream")
	got := w.Finish()
	want := []byte{4, 0, 0, 0, 0x34, 0xD8, 0x1E, 0xDD}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
	r := NewReader(got, LittleEndian, 8, "sentinel-must-understand", PidShortMaxID)
	if back := r.ReadWstringUtf8("specification", "stream"); back != "𝄞" {
		t.Fatalf("read back %q", back)
	}
}

func expectOutOfRange(t *testing.T, what string, body func()) {
	t.Helper()
	defer func() {
		if recovered := recover(); recovered == nil {
			t.Fatalf("%s: nothing was refused", what)
		} else if _, ok := recovered.(OutOfRange); !ok {
			t.Fatalf("%s: panicked with %T, not OutOfRange", what, recovered)
		}
	}()
	body()
}

func TestABoundedStringOverItsBoundIsRefusedRatherThanCut(t *testing.T) {
	// An over-bound value is refused rather than cut at a character
	// boundary: the bound is a promise about the source and not a
	// difference on the wire, and cutting it would put a sample on the
	// wire nobody wrote.
	expectOutOfRange(t, "a string past its bound", func() {
		w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
		w.WriteStringBounded("abcdefgh", 5)
	})
	// Exactly at the bound still writes the same bytes an unbounded
	// string would.
	w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
	w.WriteStringBounded("ab", 5)
	got := w.Finish()
	want := []byte{3, 0, 0, 0, 'a', 'b', 0}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
}

func TestABoundedWstringSequenceAndMapRefuseAnOverrun(t *testing.T) {
	expectOutOfRange(t, "a wstring past its bound", func() {
		w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
		w.WriteWstringBounded("ab", 1, "specification", "stream")
	})
	NewWriter(LittleEndian, 8, SpecificationFraming(), false).
		WriteWstringBounded("a", 1, "specification", "stream")

	expectOutOfRange(t, "a sequence count past its bound", func() {
		NewWriter(LittleEndian, 8, SpecificationFraming(), false).CheckSequenceBound(4, 3)
	})
	NewWriter(LittleEndian, 8, SpecificationFraming(), false).CheckSequenceBound(3, 3)

	expectOutOfRange(t, "a map entry count past its bound", func() {
		NewWriter(LittleEndian, 8, SpecificationFraming(), false).CheckMapBound(3, 2)
	})
	NewWriter(LittleEndian, 8, SpecificationFraming(), false).CheckMapBound(2, 2)
}

func TestAFixedPacksBcdWithATrailingSign(t *testing.T) {
	// [CDR] §9.3.2.8: fixed<5,2> of 12.34 is the nibbles 0 1 2 3 4 C.
	w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
	w.WriteFixed("12.34", 5, 2, "declared")
	got := w.Finish()
	want := []byte{0x01, 0x23, 0x4C}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
	r := NewReader(got, LittleEndian, 8, "sentinel-must-understand", PidShortMaxID)
	if back := r.ReadFixed(5, 2, "declared"); back != "12.34" {
		t.Fatalf("read back %q", back)
	}
}

func TestAFixedCanCarryTheDigitsOfItsString(t *testing.T) {
	// ACE/TAO 2.5.24's ACE_CDR::Fixed::from_string, recorded in
	// crates/ast-layout/tests/fixtures/unspecified_constructs.md §3: the
	// integer digits as written plus S, made odd by one leading zero.
	recorded := []struct {
		text          string
		digits, scale int
		octets        []byte
		back          string
	}{
		{"123.45", 5, 2, []byte{0x12, 0x34, 0x5C}, "123.45"},
		{"-123.45", 5, 2, []byte{0x12, 0x34, 0x5D}, "-123.45"},
		{"12.34", 4, 2, []byte{0x01, 0x23, 0x4C}, "12.34"},
		{"7", 1, 0, []byte{0x7C}, "7"},
		{"0.12345", 5, 5, []byte{0x00, 0x12, 0x34, 0x5C}, "0.12345"},
		{"0.00", 5, 2, []byte{0x00, 0x0C}, "0.00"},
		// The scale and not the fraction as written: one digit counted
		// would keep only the last nibble, 0c.
		{"7", 5, 2, []byte{0x70, 0x0C}, "7.00"},
	}
	for _, row := range recorded {
		w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
		w.WriteFixed(row.text, row.digits, row.scale, "value-string-digits")
		if got := w.Finish(); !bytes.Equal(got, row.octets) {
			t.Fatalf("%s: got % x, want % x", row.text, got, row.octets)
		}
		r := NewReader(row.octets, LittleEndian, 8, "sentinel-must-understand", PidShortMaxID)
		if back := r.ReadFixed(row.digits, row.scale, "value-string-digits"); back != row.back {
			t.Fatalf("%s: read back %q", row.text, back)
		}
	}
	expectOutOfRange(t, "thirty-two digits as written", func() {
		NewWriter(LittleEndian, 8, SpecificationFraming(), false).
			WriteFixed("00000000000000000000000000000001", 31, 0, "value-string-digits")
	})
}

func TestAnEnumHolderIsSigned(t *testing.T) {
	// §7.4.3.5.3 rule (5): the holder is signed, so 0xC8 under
	// @bit_bound(8) reads back as -56 — the defect the Python column
	// carried until the shared walk's gate found it.
	r := NewReader([]byte{0xC8}, LittleEndian, 8, "sentinel-must-understand", PidShortMaxID)
	if got := r.ReadEnum(8); got != -56 {
		t.Fatalf("got %d, want -56", got)
	}
	w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
	w.WriteEnum(8, 200)
	if got := w.Finish(); !bytes.Equal(got, []byte{0xC8}) {
		t.Fatalf("200 under 8 bits: % x", got)
	}
}

func TestADheaderPatchesItsLength(t *testing.T) {
	w := NewWriter(LittleEndian, 4, SpecificationFraming(), false)
	d := w.WriteDheader()
	w.WriteU32(7)
	w.FinishDheader(d)
	got := w.Finish()
	want := []byte{4, 0, 0, 0, 7, 0, 0, 0}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
}

func TestF128KnownPatterns(t *testing.T) {
	// 1.5 is 0x3FFF8 followed by zeros; 0.1 is the classic repeating
	// pattern ending ...999A. IEEE 754-2008, not this project's.
	cases := []struct {
		decimal string
		bytes   [16]byte
	}{
		{"1.5", [16]byte{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x3F}},
		{"0.0", [16]byte{}},
	}
	// The bytes above are little-endian; spell the 1.5 pattern out.
	cases[0].bytes = [16]byte{}
	cases[0].bytes[15] = 0x3F
	cases[0].bytes[14] = 0xFF
	cases[0].bytes[13] = 0x80
	for _, c := range cases {
		got := f128FromDecimal(c.decimal, 0)
		if got != c.bytes {
			t.Fatalf("%s: got % x, want % x", c.decimal, got, c.bytes)
		}
	}
	// 0.1: biased exponent 0x3FFB, fraction 0x999...99A.
	tenth := f128FromDecimal("0.1", 0)
	if tenth[15] != 0x3F || tenth[14] != 0xFB || tenth[0] != 0x9A || tenth[1] != 0x99 {
		t.Fatalf("0.1: got % x", tenth)
	}
}

func TestF128RoundTripsThroughItsExactDecimal(t *testing.T) {
	for _, decimal := range []string{
		"0.0", "-0.0", "1.5", "-2.75", "0.1", "3.141592653589793",
		"1e300", "1e-300", "6.475175119438025110924438958227646552e-4966",
		"inf", "-inf",
	} {
		bytes := f128FromDecimal(decimal, 0)
		text := f128ToExactDecimal(bytes)
		again := f128FromDecimal(text, 0)
		if bytes != again {
			t.Fatalf("%s: %s does not read back as the same pattern", decimal, text)
		}
	}
	if got := f128ToExactDecimal(f128FromDecimal("1.5", 0)); got != "1.5" {
		t.Fatalf("1.5 prints as %s", got)
	}
	if got := f128ToExactDecimal(f128FromDecimal("nan", 0)); got != "nan" {
		t.Fatalf("nan prints as %s", got)
	}
}

func TestAPromotedPidKeepsItsValue(t *testing.T) {
	// A short header whose value overruns 0xFFFF octets is rewritten as
	// the extended form, with the value's own bytes unchanged.
	w := NewWriter(LittleEndian, 8, SpecificationFraming(), false)
	header := w.WritePid(7, ParameterMember, false)
	long := make([]byte, 0x10001)
	w.WriteU32(uint32(len(long)) + 1)
	for _, b := range long {
		w.WriteU8(b)
	}
	w.WriteU8(0)
	w.FinishPid(header)
	got := w.Finish()
	// [PID_EXTENDED][8][id][length], then the value.
	if got[0] != 0x01 || got[1] != 0x7F {
		t.Fatalf("the marker is % x", got[:2])
	}
	if got[4] != 7 {
		t.Fatalf("the member id moved: % x", got[4:8])
	}
}

func TestTrimsRespectCharacterBoundaries(t *testing.T) {
	if got := TrimUtf8("aäb", 2); got != "a" {
		t.Fatalf("TrimUtf8: %q", got)
	}
	if got := TrimUtf16("a𝄞b", 2); got != "a" {
		t.Fatalf("TrimUtf16: %q", got)
	}
}

// The three axes Dialect::Hdds selects. Expected octets from the
// recorded HDDS columns:
// tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin
// (`04 00 00 00 61 62 63 00` for "abc"),
// tests/interop-results/sequence-wstring/hdds/plain-cdr2.le.bin
// (the elements `04 00 00 00 c3 a4 5a 00` and
// `04 00 00 00 e1 88 b4 00`),
// tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
// (`01 00 00 00 02 00 03 00 00 00 04 00 77`) and
// tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
// (the first Cell of `one`, `06 00 00 00 79 29 ed ff fe ff`).

func TestAWstringIsItsOwnUtf8Octets(t *testing.T) {
	w := NewWriter(LittleEndian, 4, SpecificationFraming(), false)
	w.WriteWstringUtf8Octets("abc", "octets/terminated/2")
	got := w.Finish()
	want := []byte{4, 0, 0, 0, 'a', 'b', 'c', 0}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
}

func TestAMultibyteUtf8WstringCountsItsOctets(t *testing.T) {
	w := NewWriter(LittleEndian, 4, SpecificationFraming(), false)
	w.WriteWstringUtf8Octets("äöü ሴ Z", "octets/terminated/2")
	got := w.Finish()
	want := []byte{0x0d, 0, 0, 0, 0xc3, 0xa4, 0xc3, 0xb6, 0xc3, 0xbc, 0x20, 0xe1, 0x88, 0xb4, 0x20, 0x5a, 0}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
}

func TestABoundedUtf8WstringBoundsItsUtf16Units(t *testing.T) {
	w := NewWriter(LittleEndian, 4, SpecificationFraming(), false)
	w.WriteWstringUtf8OctetsBounded("ሴäZ", 3, "octets/terminated/2")
	got := w.Finish()
	want := []byte{7, 0, 0, 0, 0xe1, 0x88, 0xb4, 0xc3, 0xa4, 'Z', 0}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
	expectOutOfRange(t, "a UTF-8 wstring past its bound", func() {
		NewWriter(LittleEndian, 4, SpecificationFraming(), false).
			WriteWstringUtf8OctetsBounded("ሴäZ", 2, "octets/terminated/2")
	})
}

func TestAUtf8WstringReadsBackAsItsOctets(t *testing.T) {
	recorded := []byte{0x0d, 0, 0, 0, 0xc3, 0xa4, 0xc3, 0xb6, 0xc3, 0xbc, 0x20, 0xe1, 0x88, 0xb4, 0x20, 0x5a, 0}
	r := NewReader(recorded, LittleEndian, 4, "sentinel-must-understand", PidShortMaxID)
	if back := r.ReadWstringUtf8Octets("octets/terminated/2"); back != "äöü ሴ Z" {
		t.Fatalf("read back %q", back)
	}
}

func TestAPackedElementCountsAlignmentFromItsOwnStart(t *testing.T) {
	w := NewWriter(LittleEndian, 4, SpecificationFraming(), false)
	saved := w.BeginPackedElement()
	w.WriteI32(1)
	w.WriteI16(2)
	w.FinishPackedElement(saved)
	saved = w.BeginPackedElement()
	w.WriteI32(3)
	w.WriteI16(4)
	w.FinishPackedElement(saved)
	w.WriteU8(119)
	got := w.Finish()
	want := []byte{1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
}

func TestAPackedAggregateCountsAlignmentFromItsOwnStart(t *testing.T) {
	w := NewWriter(LittleEndian, 4, SpecificationFraming(), false)
	saved := w.BeginPackedElement()
	w.WriteI32(-1234567)
	w.WriteI16(-2)
	w.FinishPackedElement(saved)
	saved = w.BeginPackedElement()
	w.WriteI32(1)
	w.WriteI16(32767)
	w.FinishPackedElement(saved)
	got := w.Finish()
	want := []byte{0x79, 0x29, 0xed, 0xff, 0xfe, 0xff, 1, 0, 0, 0, 0xff, 0x7f}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
}

func TestAPackedElementReadsBackAtItsOwnOrigin(t *testing.T) {
	recorded := []byte{1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119}
	r := NewReader(recorded, LittleEndian, 4, "sentinel-must-understand", PidShortMaxID)
	saved := r.ReaderBeginPackedElement()
	if a, b := r.ReadI32(), r.ReadI16(); a != 1 || b != 2 {
		t.Fatalf("first element was %d %d", a, b)
	}
	r.ReaderFinishPackedElement(saved)
	saved = r.ReaderBeginPackedElement()
	if a, b := r.ReadI32(), r.ReadI16(); a != 3 || b != 4 {
		t.Fatalf("second element was %d %d", a, b)
	}
	r.ReaderFinishPackedElement(saved)
	if last := r.ReadU8(); last != 119 {
		t.Fatalf("the trailing octet was %d", last)
	}
}

func TestAnElementLengthFramesTheElementItPrecedes(t *testing.T) {
	// The first Cell of `sequence-struct-appendable`'s `one`: the
	// element's own length in DHEADER form, then the six octets of the
	// appendable Cell.
	w := NewWriter(LittleEndian, 4, SpecificationFraming(), false)
	d := w.WriteDheader()
	w.WriteI32(-1234567)
	w.WriteI16(-2)
	w.FinishDheader(d)
	got := w.Finish()
	want := []byte{6, 0, 0, 0, 0x79, 0x29, 0xed, 0xff, 0xfe, 0xff}
	if !bytes.Equal(got, want) {
		t.Fatalf("got % x, want % x", got, want)
	}
	r := NewReader(got, LittleEndian, 4, "sentinel-must-understand", PidShortMaxID)
	declared := int(r.ReadU32())
	end := r.ReaderPosition() + declared
	if a, b := r.ReadI32(), r.ReadI16(); a != -1234567 || b != -2 {
		t.Fatalf("the framed Cell was %d %d", a, b)
	}
	if r.ReaderPosition() != end {
		t.Fatalf("the element ended at %d, its length said %d", r.ReaderPosition(), end)
	}
}
