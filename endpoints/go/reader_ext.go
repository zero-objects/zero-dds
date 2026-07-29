// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reader primitives beyond GetU32 — the byte-exact inverse of the Writer, so a
// received sample decodes into all of its fields (used by the deeper examples).

package zerodds

import "math"

// GetU8 reads one byte (no alignment).
func (r *Reader) GetU8() byte {
	b := r.Buf[r.Pos]
	r.Pos++
	return b
}

// GetU16 reads a 2-aligned uint16.
func (r *Reader) GetU16() uint16 {
	le := r.getLE(2, 2)
	return uint16(le[0]) | uint16(le[1])<<8
}

// GetU64 reads a uint64 (XCDR2: 4-aligned).
func (r *Reader) GetU64() uint64 {
	le := r.getLE(4, 8)
	var v uint64
	for i := 0; i < 8; i++ {
		v |= uint64(le[i]) << (8 * i)
	}
	return v
}

// GetF32 reads a float32 from its IEEE-754 bit pattern.
func (r *Reader) GetF32() float32 { return math.Float32frombits(r.GetU32()) }

// GetString reads a CDR string: u32 length (incl. NUL) + bytes + NUL.
func (r *Reader) GetString() string {
	n := int(r.GetU32())
	s := string(r.Buf[r.Pos : r.Pos+n-1])
	r.Pos += n
	return s
}

// GetSeqU8 reads a sequence<octet>: u32 length + raw bytes.
func (r *Reader) GetSeqU8() []byte {
	n := int(r.GetU32())
	b := make([]byte, n)
	copy(b, r.Buf[r.Pos:r.Pos+n])
	r.Pos += n
	return b
}
