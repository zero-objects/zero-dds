// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HPACK Decoder — RFC 7541 §6.

use alloc::vec::Vec;

use crate::integer::{IntegerError, decode_integer};
use crate::string::{StringError, decode_string};
use crate::table::{HeaderField, Table};

/// Decoder-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecoderError {
    /// Index ist 0 oder ueber Table-Range.
    InvalidIndex(u64),
    /// Integer-Decode-Fehler.
    Integer(IntegerError),
    /// String-Decode-Fehler.
    String(StringError),
    /// Buffer ist truncated.
    Truncated,
}

impl core::fmt::Display for DecoderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidIndex(i) => write!(f, "invalid index {i}"),
            Self::Integer(e) => write!(f, "integer: {e}"),
            Self::String(e) => write!(f, "string: {e}"),
            Self::Truncated => f.write_str("input truncated"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecoderError {}

impl From<IntegerError> for DecoderError {
    fn from(e: IntegerError) -> Self {
        Self::Integer(e)
    }
}

impl From<StringError> for DecoderError {
    fn from(e: StringError) -> Self {
        Self::String(e)
    }
}

/// HPACK-Decoder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Decoder {
    table: Table,
}

impl Decoder {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Konstruktor mit Max-Size.
    #[must_use]
    pub fn with_max_size(max: usize) -> Self {
        Self {
            table: Table::new(max),
        }
    }

    /// Reference auf die Dynamic-Table.
    #[must_use]
    pub fn table(&self) -> &Table {
        &self.table
    }

    /// Mut-Reference auf die Dynamic-Table (z.B. fuer Max-Size-Update).
    pub fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }

    /// Decode einen kompletten Header-Block.
    ///
    /// # Errors
    /// Siehe [`DecoderError`].
    pub fn decode(&mut self, mut input: &[u8]) -> Result<Vec<HeaderField>, DecoderError> {
        let mut out = Vec::new();
        while !input.is_empty() {
            let first = input[0];
            if first & 0x80 != 0 {
                // §6.1 Indexed Header Field.
                let (index, consumed) = decode_integer(input, 7)?;
                input = &input[consumed..];
                if index == 0 {
                    return Err(DecoderError::InvalidIndex(0));
                }
                let h = self
                    .table
                    .get(index as usize)
                    .ok_or(DecoderError::InvalidIndex(index))?;
                out.push(h);
            } else if first & 0xc0 == 0x40 {
                // §6.2.1 Literal with Incremental Indexing.
                let (index, consumed) = decode_integer(input, 6)?;
                input = &input[consumed..];
                let name = if index == 0 {
                    let (s, c) = decode_string(input)?;
                    input = &input[c..];
                    s
                } else {
                    self.table
                        .get(index as usize)
                        .ok_or(DecoderError::InvalidIndex(index))?
                        .name
                };
                let (value, c) = decode_string(input)?;
                input = &input[c..];
                let h = HeaderField { name, value };
                self.table.add(h.clone());
                out.push(h);
            } else if first & 0xe0 == 0x20 {
                // §6.3 Dynamic Table Size Update.
                let (new_size, consumed) = decode_integer(input, 5)?;
                input = &input[consumed..];
                self.table.set_max_size(new_size as usize);
            } else if first & 0xf0 == 0x00 || first & 0xf0 == 0x10 {
                // §6.2.2 Literal without indexing OR §6.2.3 never indexed.
                let prefix_bits = 4u8;
                let (index, consumed) = decode_integer(input, prefix_bits)?;
                input = &input[consumed..];
                let name = if index == 0 {
                    let (s, c) = decode_string(input)?;
                    input = &input[c..];
                    s
                } else {
                    self.table
                        .get(index as usize)
                        .ok_or(DecoderError::InvalidIndex(index))?
                        .name
                };
                let (value, c) = decode_string(input)?;
                input = &input[c..];
                out.push(HeaderField { name, value });
            } else {
                return Err(DecoderError::Truncated);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    fn hf(n: &str, v: &str) -> HeaderField {
        HeaderField {
            name: n.into(),
            value: v.into(),
        }
    }

    #[test]
    fn round_trip_static_match() {
        let mut e = Encoder::new();
        let mut d = Decoder::new();
        let headers = alloc::vec![hf(":method", "GET"), hf(":scheme", "https")];
        let buf = e.encode(&headers);
        let decoded = d.decode(&buf).unwrap();
        assert_eq!(decoded, headers);
    }

    #[test]
    fn round_trip_literal_with_indexing() {
        let mut e = Encoder::new();
        let mut d = Decoder::new();
        let headers = alloc::vec![hf("custom-key", "custom-value")];
        let buf = e.encode(&headers);
        let decoded = d.decode(&buf).unwrap();
        assert_eq!(decoded, headers);
        // Both should have synced dynamic tables.
        assert_eq!(d.table().len(), 1);
    }

    #[test]
    fn round_trip_indexed_match_after_first() {
        let mut e = Encoder::new();
        let mut d = Decoder::new();
        let headers = alloc::vec![hf("foo", "bar")];
        let _ = d.decode(&e.encode(&headers)).unwrap();
        // Encode again — encoder finds it in table → indexed.
        let buf = e.encode(&headers);
        let decoded = d.decode(&buf).unwrap();
        assert_eq!(decoded, headers);
    }

    #[test]
    fn round_trip_huffman_strings() {
        let mut e = Encoder::with_max_size(4096);
        e.use_huffman = true;
        let mut d = Decoder::with_max_size(4096);
        let headers = alloc::vec![hf("custom-key", "custom-value")];
        let buf = e.encode(&headers);
        let decoded = d.decode(&buf).unwrap();
        assert_eq!(decoded, headers);
    }

    #[test]
    fn invalid_index_zero_rejected() {
        let mut d = Decoder::new();
        // 0x80 = Indexed with index 0 → invalid.
        let buf = alloc::vec![0x80];
        assert!(matches!(d.decode(&buf), Err(DecoderError::InvalidIndex(0))));
    }

    #[test]
    fn dynamic_table_size_update_applied() {
        let mut d = Decoder::new();
        // 0x20 = size update with prefix 5 bits → 0 size.
        let buf = alloc::vec![0x20];
        d.decode(&buf).unwrap();
        assert_eq!(d.table().max_size(), 0);
    }

    #[test]
    fn rfc7541_c2_1_literal_with_indexing() {
        // Spec §C.2.1 — "custom-key: custom-header"
        // Expected encoding (literal name + value, no Huffman):
        //   400a 6375 7374 6f6d 2d6b 6579 0d63 7573 746f 6d2d 6865 6164 6572
        let mut d = Decoder::new();
        let buf = alloc::vec![
            0x40, 0x0a, b'c', b'u', b's', b't', b'o', b'm', b'-', b'k', b'e', b'y', 0x0d, b'c',
            b'u', b's', b't', b'o', b'm', b'-', b'h', b'e', b'a', b'd', b'e', b'r',
        ];
        let decoded = d.decode(&buf).unwrap();
        assert_eq!(decoded, alloc::vec![hf("custom-key", "custom-header")]);
    }

    #[test]
    fn literal_without_indexing_does_not_grow_table() {
        let mut d = Decoder::new();
        // 0x00 = Literal w/o indexing, new name.
        // followed by name and value strings.
        let buf = alloc::vec![0x00, 0x03, b'a', b'b', b'c', 0x03, b'1', b'2', b'3',];
        let decoded = d.decode(&buf).unwrap();
        assert_eq!(decoded, alloc::vec![hf("abc", "123")]);
        assert!(d.table().is_empty());
    }
}
