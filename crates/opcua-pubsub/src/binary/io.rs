// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Byte-level reader/writer for the OPC-UA binary encoding.
//!
//! Unlike CDR (`zerodds-cdr`), the OPC-UA binary encoding (Part 6
//! §5.2) has **no alignment padding** — every primitive is written
//! back-to-back in little-endian order. The reader/writer here are
//! therefore deliberately simpler than the alignment-tracking CDR
//! `BufferReader`/`BufferWriter`.

use alloc::vec::Vec;

use crate::error::DecodeError;

/// Append-only little-endian byte writer backed by a growable `Vec`.
#[derive(Debug, Default, Clone)]
pub struct UaWriter {
    buf: Vec<u8>,
}

impl UaWriter {
    /// Creates an empty writer.
    #[must_use]
    pub const fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Creates a writer with a pre-reserved capacity.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
    }

    /// Appends a single byte.
    pub fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Appends raw bytes verbatim.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Current length of the encoded buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// `true` if nothing has been written yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Read-only view of the encoded bytes so far.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// Consumes the writer and returns the encoded bytes.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }
}

macro_rules! write_le {
    ($name:ident, $ty:ty) => {
        impl UaWriter {
            #[doc = concat!("Appends a little-endian `", stringify!($ty), "`.")]
            pub fn $name(&mut self, v: $ty) {
                self.buf.extend_from_slice(&v.to_le_bytes());
            }
        }
    };
}

write_le!(write_u16, u16);
write_le!(write_u32, u32);
write_le!(write_u64, u64);
write_le!(write_i16, i16);
write_le!(write_i32, i32);
write_le!(write_i64, i64);
write_le!(write_f32, f32);
write_le!(write_f64, f64);

/// Cursor-based little-endian byte reader over a borrowed slice.
#[derive(Debug, Clone)]
pub struct UaReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> UaReader<'a> {
    /// Creates a reader positioned at the start of `buf`.
    #[must_use]
    pub const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Number of bytes not yet consumed.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// `true` if the cursor reached the end.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Current cursor offset from the start of the buffer.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining() < n {
            return Err(DecodeError::UnexpectedEof {
                needed: n,
                remaining: self.remaining(),
            });
        }
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    /// Reads a single byte.
    pub fn read_u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    /// Borrows `n` raw bytes, advancing the cursor.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        self.take(n)
    }
}

macro_rules! read_le {
    ($name:ident, $ty:ty, $n:expr) => {
        impl<'a> UaReader<'a> {
            #[doc = concat!("Reads a little-endian `", stringify!($ty), "`.")]
            pub fn $name(&mut self) -> Result<$ty, DecodeError> {
                let bytes = self.take($n)?;
                let mut arr = [0u8; $n];
                arr.copy_from_slice(bytes);
                Ok(<$ty>::from_le_bytes(arr))
            }
        }
    };
}

read_le!(read_u16, u16, 2);
read_le!(read_u32, u32, 4);
read_le!(read_u64, u64, 8);
read_le!(read_i16, i16, 2);
read_le!(read_i32, i32, 4);
read_le!(read_i64, i64, 8);
read_le!(read_f32, f32, 4);
read_le!(read_f64, f64, 8);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_roundtrip_is_little_endian() {
        let mut w = UaWriter::new();
        w.write_u32(0x0102_0304);
        // OPC-UA Part 6 §5.2.2.1 — integers are little-endian.
        assert_eq!(w.as_slice(), &[0x04, 0x03, 0x02, 0x01]);
        let mut r = UaReader::new(w.as_slice());
        assert_eq!(r.read_u32().expect("u32"), 0x0102_0304);
        assert!(r.is_empty());
    }

    #[test]
    fn read_past_end_reports_eof() {
        let mut r = UaReader::new(&[0x01, 0x02]);
        let err = r.read_u32().expect_err("should be eof");
        assert_eq!(
            err,
            DecodeError::UnexpectedEof {
                needed: 4,
                remaining: 2
            }
        );
    }

    #[test]
    fn floats_roundtrip() {
        let mut w = UaWriter::new();
        w.write_f64(core::f64::consts::PI);
        w.write_f32(1.5);
        let mut r = UaReader::new(w.as_slice());
        assert!((r.read_f64().expect("f64") - core::f64::consts::PI).abs() < f64::EPSILON);
        assert!((r.read_f32().expect("f32") - 1.5).abs() < f32::EPSILON);
    }
}
