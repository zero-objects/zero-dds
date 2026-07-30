// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Alignment-tracking buffer reader/writer for XCDR.
//!
//! XCDR data is alignment-sensitive: a `u32` field must start at a
//! 4-byte boundary, a `u64` at an 8-byte boundary, etc. The encoder
//! inserts the necessary padding bytes (value 0) before each write;
//! the decoder skips them.
//!
//! Alignment is computed relative to the **stream start** (offset 0),
//! not relative to the current position inside a nested member. This
//! matches OMG XTypes §7.4.1: "All elements are aligned to a multiple
//! of their alignment requirement, relative to the beginning of the
//! encapsulation".
//!
//! Deliberate architectural choice: only the dynamic Vec-based writer
//! (alloc feature). A slice-based writer for no_std-without-alloc is
//! not implemented — `alloc` is a transitive mandatory dependency via
//! `zerodds-foundation` anyway.

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::endianness::Endianness;
use crate::error::DecodeError;
#[cfg(feature = "alloc")]
use crate::error::EncodeError;

// ============================================================================
// BufferWriter
// ============================================================================

/// Write buffer with alignment tracking and configurable endianness.
///
/// Phase 0 uses `Vec<u8>` as backing — a growing alloc feature.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct BufferWriter {
    bytes: Vec<u8>,
    endianness: Endianness,
    /// Maximum alignment that [`Self::align`] caps at. XCDR1
    /// (PLAIN_CDR) = 8 (default), XCDR2 (PLAIN_CDR2) = 4.
    /// OMG XTypes 1.3 §7.4.1.1.1: XCDR2 limits alignment to
    /// `min(sizeof, 4)` — 64-bit primitives (double/i64/u64) are aligned
    /// to **4**, NOT 8. Wrong alignment breaks byte-wire interop with
    /// Cyclone/RTI/FastDDS (whose XCDR2 caps correctly).
    max_alignment: usize,
    /// Virtual alignment origin: [`Self::align`] aligns to
    /// `(position + align_origin)` instead of just `position`. Default 0.
    ///
    /// Needed for GIOP 1.2 (CORBA §15.4.2/§15.4.4): the request/reply body
    /// is encoded into a sub-stream that is assembled **separately** from
    /// the 12-byte GIOP header — but the CDR alignment origin is the
    /// message start (byte 0 of the header). `align_origin = 12` restores
    /// continuous alignment, otherwise the unconditionally 8-aligned GIOP
    /// 1.2 body lands at absolute offset ≡4 mod 8 → interop break with
    /// omniORB/TAO. Affects only 8-byte alignments; ≤4-byte stays identical
    /// (12 is divisible by 4), hence no effect on XCDR2/RTPS (origin 0).
    align_origin: usize,
}

/// Default max alignment for XCDR1 (PLAIN_CDR): 64-bit to 8.
const XCDR1_MAX_ALIGNMENT: usize = 8;
/// Max alignment for XCDR2 (PLAIN_CDR2): 64-bit to 4 (§7.4.1.1.1).
pub(crate) const XCDR2_MAX_ALIGNMENT: usize = 4;

#[cfg(feature = "alloc")]
impl BufferWriter {
    /// Creates an empty writer with the given endianness
    /// (XCDR1 alignment, cap 8 — default + backward-compat).
    #[must_use]
    pub fn new(endianness: Endianness) -> Self {
        Self {
            bytes: Vec::new(),
            endianness,
            max_alignment: XCDR1_MAX_ALIGNMENT,
            align_origin: 0,
        }
    }

    /// Creates a writer with pre-allocated capacity (XCDR1 cap 8).
    #[must_use]
    pub fn with_capacity(endianness: Endianness, cap: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(cap),
            endianness,
            max_alignment: XCDR1_MAX_ALIGNMENT,
            align_origin: 0,
        }
    }

    /// Builder: sets the virtual [`Self::align`] origin (default 0).
    /// For GIOP 1.2 = `12` (header size), see the `align_origin` field doc.
    #[must_use]
    pub fn with_align_origin(mut self, origin: usize) -> Self {
        self.align_origin = origin;
        self
    }

    /// Builder: sets the alignment cap (XCDR1=8, XCDR2=4). Inherited by
    /// nested member writers (see `struct_enc`).
    #[must_use]
    pub fn with_max_alignment(mut self, max_alignment: usize) -> Self {
        debug_assert!(
            max_alignment.is_power_of_two(),
            "max_alignment must be a power of two"
        );
        self.max_alignment = max_alignment;
        self
    }

    /// Builder: switches to XCDR2 alignment (cap 4, §7.4.1.1.1).
    #[must_use]
    pub fn xcdr2(self) -> Self {
        self.with_max_alignment(XCDR2_MAX_ALIGNMENT)
    }

    /// The current alignment cap (8 = XCDR1, 4 = XCDR2).
    #[must_use]
    pub fn max_alignment(&self) -> usize {
        self.max_alignment
    }

    /// Returns the current endianness.
    #[must_use]
    pub fn endianness(&self) -> Endianness {
        self.endianness
    }

    /// Current position (== number of bytes written so far).
    #[must_use]
    pub fn position(&self) -> usize {
        self.bytes.len()
    }

    /// Consumes the writer and returns the written buffer.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Read-only view of the buffer written so far.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Inserts null padding until the current position is a multiple
    /// of `alignment`. Alignment must be a power of two
    /// (1, 2, 4, 8); other values are undefined in XCDR.
    pub fn align(&mut self, alignment: usize) {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        // XCDR2 (§7.4.1.1.1) caps the alignment at `max_alignment` (4):
        // a u64/double aligns to 4, not 8. XCDR1 has cap 8
        // (= no-op for all XCDR types).
        let effective = alignment.min(self.max_alignment);
        let pos = self.bytes.len() + self.align_origin;
        let pad = padding_for(pos, effective);
        for _ in 0..pad {
            self.bytes.push(0);
        }
    }

    /// Writes raw bytes without alignment.
    ///
    /// # Errors
    /// Never returns an error with `Vec`-based backing — the signature
    /// is symmetric to the slice-based writer (Phase 1).
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), EncodeError> {
        self.bytes.extend_from_slice(data);
        Ok(())
    }

    /// Writes a single byte (1-byte alignment).
    ///
    /// # Errors
    /// As `write_bytes`.
    pub fn write_u8(&mut self, value: u8) -> Result<(), EncodeError> {
        self.bytes.push(value);
        Ok(())
    }

    /// Aligns + writes `u16`.
    ///
    /// # Errors
    /// As `write_bytes`.
    pub fn write_u16(&mut self, value: u16) -> Result<(), EncodeError> {
        self.align(2);
        self.write_bytes(&self.endianness.write_u16(value))
    }

    /// Aligns + writes `u32`.
    ///
    /// # Errors
    /// As `write_bytes`.
    pub fn write_u32(&mut self, value: u32) -> Result<(), EncodeError> {
        self.align(4);
        self.write_bytes(&self.endianness.write_u32(value))
    }

    /// Overwrites the 4 bytes at `pos` with `value` in the writer's endianness.
    ///
    /// For backpatching a length prefix (a DHEADER / EMHEADER) once the body
    /// that follows it has been written, so the body can be emitted directly
    /// into this writer instead of into a throwaway sub-writer that is then
    /// copied back. `pos` must be a 4-byte slot already written (e.g. a
    /// `write_u32(0)` placeholder).
    ///
    /// # Errors
    /// `ValueOutOfRange` if `pos + 4` is past the current end.
    pub fn patch_u32_at(&mut self, pos: usize, value: u32) -> Result<(), EncodeError> {
        let end = pos
            .checked_add(4)
            .filter(|&e| e <= self.bytes.len())
            .ok_or(EncodeError::ValueOutOfRange {
                message: "patch_u32_at position out of bounds",
            })?;
        self.bytes[pos..end].copy_from_slice(&self.endianness.write_u32(value));
        Ok(())
    }

    /// Aligns + writes `u64`.
    ///
    /// # Errors
    /// As `write_bytes`.
    pub fn write_u64(&mut self, value: u64) -> Result<(), EncodeError> {
        self.align(8);
        self.write_bytes(&self.endianness.write_u64(value))
    }

    /// Writes a CDR string (§9.3.2.7): 4-byte length (incl.
    /// null terminator) + UTF-8 bytes + 1 null byte. No trailing
    /// padding — the next field aligns itself.
    ///
    /// # Errors
    /// As `write_bytes`.
    pub fn write_string(&mut self, s: &str) -> Result<(), EncodeError> {
        let bytes = s.as_bytes();
        // Length = s.len() + 1 for the null terminator
        let len = u32::try_from(bytes.len().saturating_add(1)).map_err(|_| {
            EncodeError::ValueOutOfRange {
                message: "CDR string length exceeds u32::MAX",
            }
        })?;
        self.write_u32(len)?;
        self.write_bytes(bytes)?;
        self.write_u8(0)
    }
}

// ============================================================================
// BufferReader
// ============================================================================

/// Read buffer with alignment tracking.
#[derive(Debug, Clone)]
pub struct BufferReader<'a> {
    bytes: &'a [u8],
    pos: usize,
    endianness: Endianness,
    /// Alignment cap analogous to [`BufferWriter`] — MUST match the
    /// encode side, otherwise the reader skips wrong padding. XCDR1=8,
    /// XCDR2=4.
    max_alignment: usize,
    /// Virtual alignment origin, mirroring [`BufferWriter`]:
    /// [`Self::align`] skips padding until `(pos + align_origin)` is
    /// aligned. GIOP 1.2 body = 12, otherwise 0. See
    /// `BufferWriter::align_origin`.
    align_origin: usize,
}

impl<'a> BufferReader<'a> {
    /// Constructs a reader over the given slice (XCDR1 cap 8).
    #[must_use]
    pub fn new(bytes: &'a [u8], endianness: Endianness) -> Self {
        Self {
            bytes,
            pos: 0,
            endianness,
            max_alignment: XCDR1_MAX_ALIGNMENT,
            align_origin: 0,
        }
    }

    /// Builder: sets the virtual [`Self::align`] origin (default 0).
    /// For GIOP 1.2 = `12` (header size), MUST match the encode side.
    #[must_use]
    pub fn with_align_origin(mut self, origin: usize) -> Self {
        self.align_origin = origin;
        self
    }

    /// Builder: sets the alignment cap (XCDR1=8, XCDR2=4). Inherited by
    /// nested member readers.
    #[must_use]
    pub fn with_max_alignment(mut self, max_alignment: usize) -> Self {
        debug_assert!(
            max_alignment.is_power_of_two(),
            "max_alignment must be a power of two"
        );
        self.max_alignment = max_alignment;
        self
    }

    /// Builder: switches to XCDR2 alignment (cap 4, §7.4.1.1.1).
    #[must_use]
    pub fn xcdr2(self) -> Self {
        self.with_max_alignment(XCDR2_MAX_ALIGNMENT)
    }

    /// The current alignment cap (8 = XCDR1, 4 = XCDR2).
    #[must_use]
    pub fn max_alignment(&self) -> usize {
        self.max_alignment
    }

    /// Current endianness.
    #[must_use]
    pub fn endianness(&self) -> Endianness {
        self.endianness
    }

    /// Current position in the stream.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Remaining bytes until the end.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    /// Skips padding until the next `alignment` boundary.
    ///
    /// # Errors
    /// `UnexpectedEof` when not enough bytes remain for the padding.
    pub fn align(&mut self, alignment: usize) -> Result<(), DecodeError> {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be power of two"
        );
        // XCDR2 cap (§7.4.1.1.1), mirroring the BufferWriter.
        let alignment = alignment.min(self.max_alignment);
        let pad = padding_for(self.pos + self.align_origin, alignment);
        if self.remaining() < pad {
            return Err(DecodeError::UnexpectedEof {
                needed: pad,
                offset: self.pos,
            });
        }
        self.pos += pad;
        Ok(())
    }

    /// Reads exactly `n` bytes as a slice (without alignment).
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining() < n {
            return Err(DecodeError::UnexpectedEof {
                needed: n,
                offset: self.pos,
            });
        }
        let slice = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    /// Reads a single byte.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_u8(&mut self) -> Result<u8, DecodeError> {
        let slice = self.read_bytes(1)?;
        Ok(slice[0])
    }

    /// Aligns + reads `u16`.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_u16(&mut self) -> Result<u16, DecodeError> {
        self.align(2)?;
        let slice = self.read_bytes(2)?;
        let mut buf = [0u8; 2];
        buf.copy_from_slice(slice);
        Ok(self.endianness.read_u16(buf))
    }

    /// Aligns + reads `u32`.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_u32(&mut self) -> Result<u32, DecodeError> {
        self.align(4)?;
        let slice = self.read_bytes(4)?;
        let mut buf = [0u8; 4];
        buf.copy_from_slice(slice);
        Ok(self.endianness.read_u32(buf))
    }

    /// Reads a `u32` at the current position **without** consuming it and
    /// **without** alignment. Used for the XTypes 1.3 §7.4.3.4.2 LC5/6/7
    /// EMHEADER length-code optimization: there the member's own leading
    /// length word (a DHEADER / string length) doubles as the NEXTINT, so
    /// the decoder must inspect it but leave it in place as the first 4
    /// bytes of the member body.
    ///
    /// # Errors
    /// `UnexpectedEof` when fewer than 4 bytes remain.
    pub fn peek_u32(&self) -> Result<u32, DecodeError> {
        if self.remaining() < 4 {
            return Err(DecodeError::UnexpectedEof {
                needed: 4,
                offset: self.pos,
            });
        }
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&self.bytes[self.pos..self.pos + 4]);
        Ok(self.endianness.read_u32(buf))
    }

    /// Aligns + reads `u64`.
    ///
    /// # Errors
    /// `UnexpectedEof`.
    pub fn read_u64(&mut self) -> Result<u64, DecodeError> {
        self.align(8)?;
        let slice = self.read_bytes(8)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(slice);
        Ok(self.endianness.read_u64(buf))
    }

    /// Reads a CDR string (§9.3.2.7): 4-byte length (incl. null
    /// terminator) + UTF-8 bytes + 1 null byte. Returns the string
    /// **without** the terminator.
    ///
    /// # Errors
    /// `UnexpectedEof` for too-short data; `InvalidData` for non-UTF-8
    /// bytes, a missing null terminator, or `length == 0`.
    #[cfg(feature = "alloc")]
    pub fn read_string(&mut self) -> Result<alloc::string::String, DecodeError> {
        use alloc::string::String;
        let start = self.pos;
        let len = self.read_u32()? as usize;
        if len == 0 {
            return Err(DecodeError::InvalidString {
                offset: start,
                reason: "length must be > 0 (null terminator required)",
            });
        }
        let raw = self.read_bytes(len)?;
        // last byte must be null terminator
        if raw[len - 1] != 0 {
            return Err(DecodeError::InvalidString {
                offset: start,
                reason: "missing null terminator",
            });
        }
        String::from_utf8(raw[..len - 1].to_vec())
            .map_err(|_| DecodeError::InvalidUtf8 { offset: start + 4 })
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Computes the number of padding bytes to push `pos` to the next
/// multiple-of-`alignment` boundary.
#[must_use]
pub fn padding_for(pos: usize, alignment: usize) -> usize {
    let mask = alignment - 1;
    (alignment - (pos & mask)) & mask
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::vec;

    #[test]
    fn padding_for_zero_position_is_zero() {
        assert_eq!(padding_for(0, 4), 0);
        assert_eq!(padding_for(0, 8), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn xcdr2_caps_u64_alignment_at_4_not_8() {
        // OMG XTypes 1.3 §7.4.1.1.1: XCDR2 caps alignment at 4 —
        // a u64 after a u8 aligns to offset 4 (3 pad), NOT 8.
        // Cyclone/RTI/FastDDS do it this way; without the cap the
        // byte-wire comparison breaks (capture V-3/V-8).
        let mut w = BufferWriter::new(Endianness::Little).xcdr2();
        w.write_u8(1).unwrap();
        w.write_u64(0x4242_4242_4242_4242).unwrap();
        assert_eq!(w.position(), 4 + 8, "u64 at offset 4, not 8");
        assert_eq!(&w.as_bytes()[1..4], &[0, 0, 0], "exactly 3 pad bytes");

        // Reader mirrored: u8 then u64 @ offset 4.
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little).xcdr2();
        assert_eq!(r.read_u8().unwrap(), 1);
        assert_eq!(r.read_u64().unwrap(), 0x4242_4242_4242_4242);
        assert_eq!(r.position(), 12);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn xcdr1_default_keeps_u64_alignment_at_8() {
        // Default (XCDR1, PLAIN_CDR): u64 still 8-aligned —
        // backward-compat, no regression for the XCDR1 wire.
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u8(1).unwrap();
        w.write_u64(7).unwrap();
        assert_eq!(w.position(), 8 + 8, "XCDR1: u64 at offset 8");
    }

    #[test]
    fn padding_for_already_aligned_is_zero() {
        assert_eq!(padding_for(8, 4), 0);
        assert_eq!(padding_for(16, 8), 0);
    }

    #[test]
    fn padding_for_one_byte_to_4_align_is_three() {
        assert_eq!(padding_for(1, 4), 3);
    }

    #[test]
    fn padding_for_three_bytes_to_8_align_is_five() {
        assert_eq!(padding_for(3, 8), 5);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_writes_u8_without_padding() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u8(0xAB).unwrap();
        assert_eq!(w.as_bytes(), &[0xAB]);
        assert_eq!(w.position(), 1);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_aligns_u32_after_u8() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u8(0xAB).unwrap();
        w.write_u32(0xDEAD_BEEF).unwrap();
        // 1 byte 0xAB + 3 bytes padding + 4 bytes LE u32
        assert_eq!(w.as_bytes(), &[0xAB, 0, 0, 0, 0xEF, 0xBE, 0xAD, 0xDE]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_aligns_u64_after_u8() {
        let mut w = BufferWriter::new(Endianness::Big);
        w.write_u8(0x01).unwrap();
        w.write_u64(0x0203_0405_0607_0809).unwrap();
        // 1 byte + 7 bytes padding + 8 bytes BE u64
        assert_eq!(
            w.as_bytes(),
            &[0x01, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 6, 7, 8, 9]
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_with_capacity_preserves_endianness() {
        let w = BufferWriter::with_capacity(Endianness::Big, 64);
        assert_eq!(w.endianness(), Endianness::Big);
        assert_eq!(w.position(), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_into_bytes_returns_full_buffer() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u32(0xCAFE_BABE).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(bytes, vec![0xBE, 0xBA, 0xFE, 0xCA]);
    }

    #[test]
    fn reader_reads_u8() {
        let bytes = [0xAB, 0xCD];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_u8().unwrap(), 0xAB);
        assert_eq!(r.position(), 1);
    }

    #[test]
    fn reader_aligns_before_u32() {
        let bytes = [0xAB, 0, 0, 0, 0xEF, 0xBE, 0xAD, 0xDE];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_u8().unwrap(), 0xAB);
        assert_eq!(r.read_u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn reader_aligns_before_u64_be() {
        let bytes = [0x01, 0, 0, 0, 0, 0, 0, 0, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(r.read_u8().unwrap(), 0x01);
        assert_eq!(r.read_u64().unwrap(), 0x0203_0405_0607_0809);
    }

    #[test]
    fn reader_unexpected_eof_on_short_read() {
        let bytes = [0u8; 2];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let res = r.read_u32();
        // u32 requires 4 bytes from position 0; only 2 available.
        match res {
            Err(DecodeError::UnexpectedEof {
                needed: 4,
                offset: 0,
            }) => {}
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn reader_eof_on_align_overflow() {
        let bytes = [0u8; 1];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let _ = r.read_u8().unwrap();
        let res = r.align(8);
        assert!(matches!(res, Err(DecodeError::UnexpectedEof { .. })));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn writer_reader_roundtrip_mixed_primitives() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_u8(1).unwrap();
        w.write_u16(0x1234).unwrap();
        w.write_u32(0x5678_9ABC).unwrap();
        w.write_u64(0x0102_0304_0506_0708).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_u8().unwrap(), 1);
        assert_eq!(r.read_u16().unwrap(), 0x1234);
        assert_eq!(r.read_u32().unwrap(), 0x5678_9ABC);
        assert_eq!(r.read_u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(r.remaining(), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn write_read_string_roundtrip_ascii() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_string("ChatterTopic").unwrap();
        let bytes = w.into_bytes();
        // length=13 (12 chars + null), 12 bytes ascii, 1 null
        assert_eq!(&bytes[0..4], &[13, 0, 0, 0]);
        assert_eq!(&bytes[4..16], b"ChatterTopic");
        assert_eq!(bytes[16], 0);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_string().unwrap(), "ChatterTopic");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn write_read_string_empty() {
        // "": length=1 (just null), 0 chars, 1 null
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_string("").unwrap();
        let bytes = w.into_bytes();
        assert_eq!(&bytes[..], &[1, 0, 0, 0, 0]);
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_string().unwrap(), "");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn write_read_string_utf8_multibyte() {
        let mut w = BufferWriter::new(Endianness::Little);
        w.write_string("Zähler").unwrap(); // ä = 2 bytes UTF-8
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_string().unwrap(), "Zähler");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn read_string_rejects_length_zero() {
        let bytes = [0u8, 0, 0, 0];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(matches!(
            r.read_string(),
            Err(DecodeError::InvalidString { .. })
        ));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn read_string_rejects_missing_null_terminator() {
        // length=4, but only ASCII without null terminator
        let bytes = [4u8, 0, 0, 0, b'A', b'B', b'C', b'D'];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(matches!(
            r.read_string(),
            Err(DecodeError::InvalidString { .. })
        ));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn read_string_rejects_invalid_utf8() {
        // length=3, bytes = 0xFF, 0xFE, null -> invalid UTF-8
        let bytes = [3u8, 0, 0, 0, 0xFF, 0xFE, 0];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert!(matches!(
            r.read_string(),
            Err(DecodeError::InvalidUtf8 { .. })
        ));
    }

    // ---- Mutation killers for BufferReader ----

    /// Catches mutation `endianness -> Default::default()`. The reader
    /// must return the endianness from the constructor, not the default.
    /// Default::default() is Little — we test with Big.
    #[test]
    fn reader_endianness_getter_returns_construction_value() {
        let r_be = BufferReader::new(&[0u8; 4], Endianness::Big);
        assert_eq!(r_be.endianness(), Endianness::Big);
        let r_le = BufferReader::new(&[0u8; 4], Endianness::Little);
        assert_eq!(r_le.endianness(), Endianness::Little);
    }

    /// Catches mutation `< -> <=` in `align`. align() must NOT error
    /// when `remaining()` has exactly `pad` bytes.
    #[test]
    fn reader_align_succeeds_when_remaining_equals_pad() {
        // pos=1, align=4 -> pad=3, remaining must be >= 3.
        // Exactly 4-byte buffer: read_u8 over 1 byte, then align(4)
        // needs 3 pad bytes. After read_u8 the buffer still has 3 bytes
        // -> remaining == pad (3). Original `<`: 3<3 false, no error.
        // Mutation `<=`: 3<=3 true, error.
        let bytes = [0xAA, 0, 0, 0];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        r.read_u8().unwrap();
        assert!(r.align(4).is_ok());
        assert_eq!(r.position(), 4);
    }

    /// Catches mutation `+= -> *=` on `self.pos += pad` in align().
    /// align() must advance pos FORWARD.
    #[test]
    fn reader_align_advances_position_strictly() {
        let bytes = [0xAA, 0, 0, 0, 1, 2, 3, 4];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        r.read_u8().unwrap();
        assert_eq!(r.position(), 1);
        r.align(4).unwrap();
        // pos *= pad = 1*3 = 3 with the mutation; pos = 1+3 = 4 without it.
        assert_eq!(r.position(), 4);
    }

    /// Catches mutation `+= -> *=` on `self.pos += n` in read_bytes().
    /// read_bytes() must advance pos forward by exactly n.
    #[test]
    fn reader_read_bytes_advances_position_strictly() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let _ = r.read_bytes(3).unwrap();
        // pos *= n = 0*3 = 0 with the mutation; pos = 0+3 = 3 without it.
        assert_eq!(r.position(), 3);
        let _ = r.read_bytes(2).unwrap();
        // pos *= 2 = 3*2 = 6 with the mutation; pos = 3+2 = 5 without it.
        assert_eq!(r.position(), 5);
    }

    /// Catches mutation `read_u16 -> Ok(0)`. read_u16 must actually
    /// read the bytes, not just return 0.
    #[test]
    fn reader_read_u16_returns_actual_bytes_not_zero() {
        let bytes = [0x12, 0x34];
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        assert_eq!(r.read_u16().unwrap(), 0x3412);
        let mut r_be = BufferReader::new(&bytes, Endianness::Big);
        assert_eq!(r_be.read_u16().unwrap(), 0x1234);
    }

    /// Catches mutation `+ -> *` on `start + 4` in the read_string
    /// error-offset computation.
    ///
    /// Spec: offset points to the start of the UTF-8 data = start + 4
    /// (after the 4-byte length prefix). `start` is set BEFORE the
    /// `read_u32`/alignment; with pos=1 at the start, start=1 and
    /// offset=5. Mutation `*` would yield 1*4=4.
    #[test]
    fn reader_read_string_invalid_utf8_offset_is_start_plus_four() {
        // A preceding u8 shifts start to 1 (not 0) so that `+` vs
        // `*` differentiate: 1+4=5 vs 1*4=4.
        let mut bytes = vec![0xAB]; // pos=0
        // pad to 4: 3 zero bytes
        bytes.extend_from_slice(&[0, 0, 0]);
        // length=3 LE (incl. null-terminator slot)
        bytes.extend_from_slice(&3u32.to_le_bytes());
        // 3 bytes, last is NUL — the first two are invalid UTF-8.
        bytes.extend_from_slice(&[0xFF, 0xFE, 0]);

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        r.read_u8().unwrap();
        let err = r.read_string().unwrap_err();
        // `start = self.pos` BEFORE read_u32. After read_u8, pos=1.
        // So start=1, start+4=5. Mutation `*`: 1*4=4. The difference suffices.
        match err {
            DecodeError::InvalidUtf8 { offset } => assert_eq!(offset, 5),
            other => panic!("expected InvalidUtf8, got {other:?}"),
        }
    }

    /// Second variant with start=2: 2+4=6 vs 2*4=8 — two-sided
    /// differentiation (above start=1: +1, here start=2: +4).
    #[test]
    fn reader_read_string_invalid_utf8_offset_with_start_two() {
        let mut bytes = vec![0xAB, 0xCD]; // pos=0..2
        // pad to 4: 2 zero bytes
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&[0xFF, 0xFE, 0]);

        let mut r = BufferReader::new(&bytes, Endianness::Little);
        r.read_u8().unwrap();
        r.read_u8().unwrap();
        let err = r.read_string().unwrap_err();
        match err {
            DecodeError::InvalidUtf8 { offset } => assert_eq!(offset, 6),
            other => panic!("expected InvalidUtf8, got {other:?}"),
        }
    }
}
