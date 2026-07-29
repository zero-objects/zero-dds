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
