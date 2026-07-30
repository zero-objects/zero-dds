// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Package zerodds is the native pure-Go endpoint SDK (ADR 0013): a from-scratch
// XCDR wire-core, no cgo, byte-identical to the Rust core and the other SDKs.
// Serialization is by explicit byte order, so the output is independent of the
// host and a big-endian target produces the same wire as an x86-64 host.
package zerodds

import "math"

// Endianness is the wire byte order (the XCDR encapsulation flag, not the host).
type Endianness int

const (
	// Little wire byte order.
	Little Endianness = iota
	// Big wire byte order.
	Big
)

const xcdr2MaxAlign = 4

// Writer accumulates XCDR bytes into a growable buffer.
type Writer struct {
	Buf    []byte
	Endian Endianness
}

// NewWriter returns a Writer for the given wire byte order.
func NewWriter(endian Endianness) *Writer { return &Writer{Endian: endian} }

func (w *Writer) align(a int) {
	if a > xcdr2MaxAlign {
		a = xcdr2MaxAlign
	}
	for (len(w.Buf) % a) != 0 {
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

// PutU16 appends a 2-aligned uint16.
func (w *Writer) PutU16(v uint16) { w.putLE(2, []byte{byte(v), byte(v >> 8)}) }

// PutU32 appends a 4-aligned uint32.
func (w *Writer) PutU32(v uint32) {
	w.putLE(4, []byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24)})
}

// PutU64 appends a uint64 (XCDR2: 4-aligned).
func (w *Writer) PutU64(v uint64) {
	le := make([]byte, 8)
	for i := 0; i < 8; i++ {
		le[i] = byte(v >> (8 * i))
	}
	w.putLE(4, le)
}

// PutF32 appends a float32 as its IEEE-754 bit pattern.
func (w *Writer) PutF32(v float32) { w.PutU32(math.Float32bits(v)) }

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

// Reader walks an XCDR byte slice.
type Reader struct {
	Buf    []byte
	Pos    int
	Endian Endianness
}

// NewReader returns a Reader over b for the given wire byte order.
func NewReader(b []byte, endian Endianness) *Reader {
	return &Reader{Buf: b, Endian: endian}
}

func (r *Reader) getLE(a, n int) []byte {
	if a > xcdr2MaxAlign {
		a = xcdr2MaxAlign
	}
	for (r.Pos % a) != 0 {
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
