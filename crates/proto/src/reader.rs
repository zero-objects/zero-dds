// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Minimal Protocol Buffers wire-format reader — the subset needed to walk a
//! `FileDescriptorSet` (descriptor.proto). Pure-`core`, no dependencies.
//!
//! Protobuf encodes each field as a varint tag `(field_number << 3) |
//! wire_type` followed by a payload whose shape depends on the wire type.

/// Protobuf wire types (the low 3 bits of a field tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireType {
    /// Base-128 varint (`int32/64`, `uint32/64`, `sint*`, `bool`, `enum`).
    Varint,
    /// Fixed 8-byte little-endian (`fixed64`, `sfixed64`, `double`).
    I64,
    /// Length-delimited: a varint length then that many bytes (`string`,
    /// `bytes`, embedded messages, packed repeated).
    Len,
    /// Fixed 4-byte little-endian (`fixed32`, `sfixed32`, `float`).
    I32,
}

impl WireType {
    /// Map the low-3-bit wire-type code. Groups (3, 4) are not supported and
    /// return `None`.
    #[must_use]
    pub fn from_bits(v: u32) -> Option<Self> {
        Some(match v {
            0 => Self::Varint,
            1 => Self::I64,
            2 => Self::Len,
            5 => Self::I32,
            _ => return None,
        })
    }
}

/// A cursor over a protobuf-encoded byte slice. Every accessor returns `None`
/// on a truncated or malformed input rather than panicking.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Wrap a byte slice.
    #[must_use]
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// `true` once every byte has been consumed.
    #[must_use]
    pub fn at_end(&self) -> bool {
        self.pos >= self.buf.len()
    }

    /// Read a base-128 varint (max 10 bytes, i.e. 64 bits). `None` on
    /// truncation or overlong encoding.
    pub fn read_varint(&mut self) -> Option<u64> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            let byte = *self.buf.get(self.pos)?;
            self.pos += 1;
            result |= u64::from(byte & 0x7f).checked_shl(shift)?;
            if byte & 0x80 == 0 {
                return Some(result);
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
    }

    /// Read a field tag, returning `(field_number, wire_type)`.
    pub fn read_tag(&mut self) -> Option<(u32, WireType)> {
        let tag = self.read_varint()?;
        let field = u32::try_from(tag >> 3).ok()?;
        if field == 0 {
            return None;
        }
        let wt = WireType::from_bits((tag & 0x7) as u32)?;
        Some((field, wt))
    }

    /// Read a length-delimited payload (a varint length then that many bytes).
    pub fn read_len_delimited(&mut self) -> Option<&'a [u8]> {
        let len = usize::try_from(self.read_varint()?).ok()?;
        let end = self.pos.checked_add(len)?;
        let slice = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    /// Read a fixed 4-byte little-endian value.
    pub fn read_i32(&mut self) -> Option<u32> {
        let end = self.pos.checked_add(4)?;
        let bytes = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a fixed 8-byte little-endian value.
    pub fn read_i64(&mut self) -> Option<u64> {
        let end = self.pos.checked_add(8)?;
        let bytes = self.buf.get(self.pos..end)?;
        self.pos = end;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(bytes);
        Some(u64::from_le_bytes(arr))
    }

    /// Skip the payload of a field with the given wire type (for fields the
    /// descriptor walker does not care about).
    pub fn skip(&mut self, wt: WireType) -> Option<()> {
        match wt {
            WireType::Varint => {
                self.read_varint()?;
            }
            WireType::I64 => {
                self.read_i64()?;
            }
            WireType::Len => {
                self.read_len_delimited()?;
            }
            WireType::I32 => {
                self.read_i32()?;
            }
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn varint_single_and_multi_byte() {
        assert_eq!(Reader::new(&[0x00]).read_varint(), Some(0));
        assert_eq!(Reader::new(&[0x7f]).read_varint(), Some(127));
        // 300 = 0b1_0010_1100 -> 0xAC 0x02
        assert_eq!(Reader::new(&[0xac, 0x02]).read_varint(), Some(300));
        // truncated (continuation bit set, no next byte)
        assert_eq!(Reader::new(&[0x80]).read_varint(), None);
    }

    #[test]
    fn tag_splits_field_and_wire_type() {
        // 0x0A = (1 << 3) | 2 -> field 1, Len
        assert_eq!(Reader::new(&[0x0a]).read_tag(), Some((1, WireType::Len)));
        // 0x18 = (3 << 3) | 0 -> field 3, Varint
        assert_eq!(Reader::new(&[0x18]).read_tag(), Some((3, WireType::Varint)));
        // wire type 3 (group start) is unsupported
        assert_eq!(Reader::new(&[0x0b]).read_tag(), None);
    }

    #[test]
    fn len_delimited_reads_exactly() {
        // len 3, "abc"
        let mut r = Reader::new(&[0x03, b'a', b'b', b'c']);
        assert_eq!(r.read_len_delimited(), Some(&b"abc"[..]));
        assert!(r.at_end());
        // len exceeds buffer
        assert_eq!(Reader::new(&[0x05, b'a']).read_len_delimited(), None);
    }

    #[test]
    fn skip_advances_past_each_wire_type() {
        // varint field then a len field; skip the varint, read the len.
        let mut r = Reader::new(&[0x08, 0x96, 0x01, 0x12, 0x02, b'h', b'i']);
        let (f, wt) = r.read_tag().unwrap();
        assert_eq!((f, wt), (1, WireType::Varint));
        r.skip(wt).unwrap();
        let (f2, wt2) = r.read_tag().unwrap();
        assert_eq!((f2, wt2), (2, WireType::Len));
        assert_eq!(r.read_len_delimited(), Some(&b"hi"[..]));
    }
}
