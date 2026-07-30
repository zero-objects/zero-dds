// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func goldenDir() string {
	if d := os.Getenv("GOLDEN_DIR"); d != "" {
		return d
	}
	return "build"
}

// fixture is the @final SensorReading vector — MUST match endpoints/golden-gen.
func fixture(w *Writer) {
	w.PutU32(0xA1B2C3D4)
	w.PutU16(0x1234)
	w.PutU8(0x5A)
	w.PutF32(3.5)
	w.PutU64(0x0102030405060708)
	w.PutString("bay-12")
	w.PutSeqU8([]byte{0xDE, 0xAD, 0xBE, 0xEF})
}

func TestByteIdentity(t *testing.T) {
	cases := []struct {
		endian Endianness
		file   string
	}{
		{Little, "golden_le.bin"},
		{Big, "golden_be.bin"},
	}
	for _, tc := range cases {
		w := NewWriter(tc.endian)
		fixture(w)
		golden, err := os.ReadFile(filepath.Join(goldenDir(), tc.file))
		if err != nil {
			t.Fatalf("%s: read golden: %v", tc.file, err)
		}
		if !bytes.Equal(w.Buf, golden) {
			t.Fatalf("%s: not byte-identical (got %d bytes, want %d)",
				tc.file, len(w.Buf), len(golden))
		}
		t.Logf("%s: %d bytes byte-identical to Rust golden", tc.file, len(w.Buf))
	}
}

// frame builds an XRCE frame with an arbitrary submessage id + declared body
// length, for exercising the reader's direction + validation logic.
func frame(id byte, body []byte, declaredLen int) []byte {
	out := []byte{0x80, 0x01, 1, 0, id, 0x03, byte(declaredLen), byte(declaredLen >> 8)}
	return append(out, body...)
}

func TestXrceReadFrameDirection(t *testing.T) {
	// Hub -> endpoint direction is DATA (0x09); the reader must accept it.
	data := []byte{0xAA, 0xBB, 0xCC}
	if body, ok := XrceReadFrame(frame(xrceSMData, data, len(data))); !ok || !bytes.Equal(body, data) {
		t.Fatalf("DATA/0x09 not accepted: ok=%v body=%v", ok, body)
	}
	// Loopback WRITE_DATA (0x07) still accepted.
	wd := []byte{0x01, 0x02}
	if body, ok := XrceReadFrame(frame(xrceSMWriteData, wd, len(wd))); !ok || !bytes.Equal(body, wd) {
		t.Fatalf("WRITE_DATA/0x07 not accepted: ok=%v body=%v", ok, body)
	}
}

func TestXrceReadFrameNegativeVectors(t *testing.T) {
	wd := []byte{0x01, 0x02}
	// Unknown submessage id -> reject.
	if _, ok := XrceReadFrame(frame(0x0A, wd, len(wd))); ok {
		t.Fatal("unknown submessage id must be rejected")
	}
	// Truncated header (fewer than 8 bytes) -> reject.
	for n := 0; n < 8; n++ {
		if _, ok := XrceReadFrame(make([]byte, n)); ok {
			t.Fatalf("%d-byte frame must be rejected", n)
		}
	}
	// Declared length past the datagram -> reject.
	if _, ok := XrceReadFrame(frame(xrceSMData, wd, 100)); ok {
		t.Fatal("over-long declared length must be rejected")
	}
	// Trailing bytes past the declared length must not leak into the body.
	if body, ok := XrceReadFrame(frame(xrceSMData, []byte{1, 2, 3, 4}, 2)); !ok || !bytes.Equal(body, []byte{1, 2}) {
		t.Fatalf("body must be bounded by declared length: ok=%v body=%v", ok, body)
	}
}

func TestXrceWriteFrameRejectsOversizeSample(t *testing.T) {
	// A sample larger than the 16-bit submessage_length field must be refused
	// (nil), not have its length silently truncated (matches the C SDK).
	if XrceWriteFrame(XrceSessionNoKey, XrceStreamBestEffort, 1, make([]byte, 0x10000)) != nil {
		t.Fatal("sample > 0xFFFF must be refused")
	}
	// The largest still-encodable sample is accepted.
	if XrceWriteFrame(XrceSessionNoKey, XrceStreamBestEffort, 1, make([]byte, 0xFFFF)) == nil {
		t.Fatal("0xFFFF sample must still encode")
	}
}
