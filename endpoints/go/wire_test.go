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
		w := NewWriter(tc.endian, Xcdr2)
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

// frame builds an XRCE frame with an arbitrary submessage id + declared
// submessage length, for exercising the reader's direction + validation logic.
//
// declaredLen is the whole submessage_length — the BaseObjectRequest plus the
// sample — because that is the field the reader validates. A WRITE_DATA / DATA
// submessage carries a BaseObjectRequest (request_id then object_id, each two
// octets most significant first, DDS-XRCE 1.0 §7.7.8) in front of the sample,
// and the flags are 0x01. This helper wrote neither, so every frame it built
// was one the reader is right to reject.
func frame(id byte, body []byte, declaredLen int) []byte {
	out := []byte{
		0x80, 0x01, 1, 0,
		id, xrceWriteFlags, byte(declaredLen), byte(declaredLen >> 8),
		0x00, 0x01, 0x00, 0x10, // BaseObjectRequest: request_id=1, object_id=0x0010
	}
	return append(out, body...)
}

// sampleFrame is the common case: a frame whose declared length is exactly the
// BaseObjectRequest plus this body.
func sampleFrame(id byte, body []byte) []byte {
	return frame(id, body, XrceBaseSize+len(body))
}

func TestXrceReadFrameDirection(t *testing.T) {
	// Hub -> endpoint direction is DATA (0x09); the reader must accept it.
	data := []byte{0xAA, 0xBB, 0xCC}
	if body, ok := XrceReadFrame(sampleFrame(xrceSMData, data)); !ok || !bytes.Equal(body, data) {
		t.Fatalf("DATA/0x09 not accepted: ok=%v body=%v", ok, body)
	}
	// Loopback WRITE_DATA (0x07) still accepted.
	wd := []byte{0x01, 0x02}
	if body, ok := XrceReadFrame(sampleFrame(xrceSMWriteData, wd)); !ok || !bytes.Equal(body, wd) {
		t.Fatalf("WRITE_DATA/0x07 not accepted: ok=%v body=%v", ok, body)
	}
	// The BaseObjectRequest reaches the caller that asks for it.
	if rq, obj, _, ok := XrceReadFrameWithBase(sampleFrame(xrceSMData, data)); !ok || rq != 1 || obj != 0x0010 {
		t.Fatalf("BaseObjectRequest not read back: ok=%v request=%#x object=%#x", ok, rq, obj)
	}
}

func TestXrceReadFrameNegativeVectors(t *testing.T) {
	wd := []byte{0x01, 0x02}
	// Unknown submessage id -> reject.
	if _, ok := XrceReadFrame(sampleFrame(0x0A, wd)); ok {
		t.Fatal("unknown submessage id must be rejected")
	}
	// Truncated header — anything shorter than the header plus the
	// BaseObjectRequest cannot carry one -> reject.
	for n := 0; n < 8+XrceBaseSize; n++ {
		if _, ok := XrceReadFrame(make([]byte, n)); ok {
			t.Fatalf("%d-byte frame must be rejected", n)
		}
	}
	// Declared length past the datagram -> reject.
	if _, ok := XrceReadFrame(frame(xrceSMData, wd, 100)); ok {
		t.Fatal("over-long declared length must be rejected")
	}
	// A declared length too short to hold the BaseObjectRequest -> reject.
	if _, ok := XrceReadFrame(frame(xrceSMData, wd, XrceBaseSize-1)); ok {
		t.Fatal("declared length below the BaseObjectRequest must be rejected")
	}
	// Trailing bytes past the declared length must not leak into the body.
	if body, ok := XrceReadFrame(frame(xrceSMData, []byte{1, 2, 3, 4}, XrceBaseSize+2)); !ok || !bytes.Equal(body, []byte{1, 2}) {
		t.Fatalf("body must be bounded by declared length: ok=%v body=%v", ok, body)
	}
}

func TestXrceWriteFrameRejectsOversizeSample(t *testing.T) {
	// submessage_length covers the BaseObjectRequest as well as the sample, so
	// the largest encodable sample is 0xFFFF - XrceBaseSize. Anything above
	// that must be refused (nil), not have its length silently truncated while
	// the full payload is appended (matches the C SDK).
	const largest = 0xFFFF - XrceBaseSize
	obj := XrceObjectID(0x001, uint16(ObjkDataWriter))
	if XrceWriteFrame(XrceSessionNoKey, XrceStreamBestEffort, 1, obj, make([]byte, largest+1)) != nil {
		t.Fatalf("sample of %d must be refused", largest+1)
	}
	frame := XrceWriteFrame(XrceSessionNoKey, XrceStreamBestEffort, 1, obj, make([]byte, largest))
	if frame == nil {
		t.Fatalf("sample of %d must still encode", largest)
	}
	// The declared length is what the reader will believe, so it is checked
	// rather than assumed: a truncated length here is the silent corruption
	// this test exists to catch.
	if got := int(frame[6]) | int(frame[7])<<8; got != 0xFFFF {
		t.Fatalf("declared submessage_length %d, want %d", got, 0xFFFF)
	}
}

// countingTransport records what reached the link, so a refused sample can be
// held to having reached nothing.
type countingTransport struct{ delivered int }

func (t *countingTransport) Deliver(frame []byte) error { t.delivered++; return nil }
func (t *countingTransport) Receive(buf []byte) (int, bool, error) {
	return 0, true, nil
}

// The builder's nil was representable and both writers handed it to Deliver
// unread: an empty datagram went out and Write returned nil. The refusal now
// stops at the caller, and the sequence is not spent on a sample that was
// never written.
func TestWriteRefusesTheSampleTheBuilderRefused(t *testing.T) {
	for _, w := range []struct {
		name  string
		write func(Transport) func([]byte) error
	}{
		{"Client", func(tr Transport) func([]byte) error {
			return NewClient(tr, XrceObjectID(0x001, uint16(ObjkDataWriter))).Write
		}},
		{"AsyncWriter", func(tr Transport) func([]byte) error {
			return NewAsyncWriter(tr, XrceObjectID(0x001, uint16(ObjkDataWriter))).Write
		}},
	} {
		tr := &countingTransport{}
		write := w.write(tr)
		if err := write(make([]byte, 0x10000)); err == nil {
			t.Fatalf("%s.Write: a sample above 0xFFFF must report", w.name)
		}
		if tr.delivered != 0 {
			t.Fatalf("%s.Write: delivered %d frames for a refused sample", w.name, tr.delivered)
		}
		if err := write(make([]byte, 4)); err != nil {
			t.Fatalf("%s.Write: %v", w.name, err)
		}
		if tr.delivered != 1 {
			t.Fatalf("%s.Write: delivered %d", w.name, tr.delivered)
		}
	}
}
