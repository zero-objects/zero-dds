// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HPACK Decoder — RFC 7541 §6.

use alloc::vec::Vec;

use crate::integer::{IntegerError, decode_integer};
use crate::string::{StringError, decode_string};
use crate::table::{HeaderField, Table};

/// The SETTINGS_HEADER_TABLE_SIZE an HTTP/2 endpoint starts with (RFC 9113
/// §6.5.2): the limit of a dynamic table size update until the decoder's
/// side announces another.
pub const DEFAULT_TABLE_SIZE_LIMIT: usize = 4096;

/// Decoder error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecoderError {
    /// Index is 0 or beyond the table range.
    InvalidIndex(u64),
    /// Integer decode error.
    Integer(IntegerError),
    /// String decode error.
    String(StringError),
    /// Buffer is truncated.
    Truncated,
    /// RFC 7541 §6.3: a dynamic table size update above the limit the
    /// protocol sets (in HTTP/2 the SETTINGS_HEADER_TABLE_SIZE of the
    /// decoder's side, RFC 7541 §4.2), a decoding error.
    TableSizeAboveLimit {
        /// The size the update names.
        size: u64,
        /// The limit.
        limit: usize,
    },
    /// RFC 7541 §4.2: a dynamic table size update after the first header
    /// field representation of the block.
    TableSizeUpdateNotAtStart,
    /// The decoded header list exceeds the limit of
    /// [`Decoder::set_max_header_list_size`] (its size counted as RFC 9113
    /// §6.5.2 counts it: name, value and 32 octets per field). The block
    /// was decoded to its end, so the dynamic table is in step with the
    /// encoder's, but the fields are not returned: no field beyond the
    /// limit was held.
    HeaderListTooLarge {
        /// The limit in octets.
        limit: usize,
    },
}

impl core::fmt::Display for DecoderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidIndex(i) => write!(f, "invalid index {i}"),
            Self::Integer(e) => write!(f, "integer: {e}"),
            Self::String(e) => write!(f, "string: {e}"),
            Self::Truncated => f.write_str("input truncated"),
            Self::TableSizeAboveLimit { size, limit } => {
                write!(
                    f,
                    "dynamic table size update to {size} above the limit {limit}"
                )
            }
            Self::TableSizeUpdateNotAtStart => {
                f.write_str("dynamic table size update after a header field")
            }
            Self::HeaderListTooLarge { limit } => {
                write!(f, "header list above {limit} octets")
            }
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

/// HPACK decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoder {
    table: Table,
    /// RFC 7541 §6.3: the largest size a dynamic table size update may
    /// name.
    table_size_limit: usize,
    /// The largest decoded header list [`Decoder::decode`] returns; `None`
    /// is no limit.
    max_header_list_size: Option<usize>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self {
            table: Table::default(),
            table_size_limit: DEFAULT_TABLE_SIZE_LIMIT,
            max_header_list_size: None,
        }
    }
}

/// The header list being decoded: the fields returned and their size, or
/// none once the size passed the limit.
struct HeaderList {
    fields: Vec<HeaderField>,
    size: usize,
    limit: Option<usize>,
    over: bool,
}

impl HeaderList {
    /// Whether a field of `size` octets (RFC 9113 §6.5.2) is still held;
    /// past the limit the fields so far are dropped and none is held.
    fn admits(&mut self, size: usize) -> bool {
        if self.over {
            return false;
        }
        let Some(limit) = self.limit else {
            return true;
        };
        self.size = self.size.saturating_add(size);
        if self.size > limit {
            self.over = true;
            self.fields = Vec::new();
            return false;
        }
        true
    }
}

impl Decoder {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructor with a max size: the dynamic table's size and the limit
    /// of a dynamic table size update (RFC 7541 §6.3).
    #[must_use]
    pub fn with_max_size(max: usize) -> Self {
        Self {
            table: Table::new(max),
            table_size_limit: max,
            max_header_list_size: None,
        }
    }

    /// Reference to the dynamic table.
    #[must_use]
    pub fn table(&self) -> &Table {
        &self.table
    }

    /// Mutable reference to the dynamic table (e.g. for a max-size update).
    pub fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }

    /// The largest size a dynamic table size update may name (RFC 7541
    /// §6.3).
    #[must_use]
    pub fn table_size_limit(&self) -> usize {
        self.table_size_limit
    }

    /// Sets the limit of a dynamic table size update: in HTTP/2 the
    /// SETTINGS_HEADER_TABLE_SIZE the decoder's side announced (RFC 7541
    /// §4.2).
    pub fn set_table_size_limit(&mut self, limit: usize) {
        self.table_size_limit = limit;
    }

    /// The largest decoded header list [`Decoder::decode`] returns.
    #[must_use]
    pub fn max_header_list_size(&self) -> Option<usize> {
        self.max_header_list_size
    }

    /// Sets the largest decoded header list [`Decoder::decode`] returns, in
    /// octets as RFC 9113 §6.5.2 counts them (in HTTP/2 the
    /// SETTINGS_MAX_HEADER_LIST_SIZE the decoder's side announced); `None`
    /// is no limit.
    pub fn set_max_header_list_size(&mut self, limit: Option<usize>) {
        self.max_header_list_size = limit;
    }

    /// Decode a complete header block.
    ///
    /// # Errors
    /// See [`DecoderError`]. [`DecoderError::HeaderListTooLarge`] comes
    /// after the whole block was decoded, every other error at the place
    /// it was found.
    pub fn decode(&mut self, mut input: &[u8]) -> Result<Vec<HeaderField>, DecoderError> {
        let mut list = HeaderList {
            fields: Vec::new(),
            size: 0,
            limit: self.max_header_list_size,
            over: false,
        };
        let mut field_seen = false;
        while !input.is_empty() {
            let first = input[0];
            if first & 0x80 != 0 {
                // §6.1 Indexed Header Field.
                let (index, consumed) = decode_integer(input, 7)?;
                input = &input[consumed..];
                field_seen = true;
                if index == 0 {
                    return Err(DecoderError::InvalidIndex(0));
                }
                let at = usize::try_from(index).map_err(|_| DecoderError::InvalidIndex(index))?;
                let size = self
                    .table
                    .entry_size(at)
                    .ok_or(DecoderError::InvalidIndex(index))?;
                // Past the limit the entry is not copied.
                if list.admits(size) {
                    let h = self
                        .table
                        .get(at)
                        .ok_or(DecoderError::InvalidIndex(index))?;
                    list.fields.push(h);
                }
            } else if first & 0xc0 == 0x40 {
                // §6.2.1 Literal with Incremental Indexing.
                let (index, consumed) = decode_integer(input, 6)?;
                input = &input[consumed..];
                field_seen = true;
                let name = self.literal_name(index, &mut input)?;
                let (value, c) = decode_string(input)?;
                input = &input[c..];
                let h = HeaderField { name, value };
                if list.admits(h.size()) {
                    list.fields.push(h.clone());
                }
                self.table.add(h);
            } else if first & 0xe0 == 0x20 {
                // §6.3 Dynamic Table Size Update: at the start of the
                // block (§4.2), within the protocol's limit.
                let (new_size, consumed) = decode_integer(input, 5)?;
                input = &input[consumed..];
                if field_seen {
                    return Err(DecoderError::TableSizeUpdateNotAtStart);
                }
                let size = usize::try_from(new_size)
                    .ok()
                    .filter(|s| *s <= self.table_size_limit)
                    .ok_or(DecoderError::TableSizeAboveLimit {
                        size: new_size,
                        limit: self.table_size_limit,
                    })?;
                self.table.set_max_size(size);
            } else if first & 0xf0 == 0x00 || first & 0xf0 == 0x10 {
                // §6.2.2 Literal without indexing OR §6.2.3 never indexed.
                let (index, consumed) = decode_integer(input, 4)?;
                input = &input[consumed..];
                field_seen = true;
                let name = self.literal_name(index, &mut input)?;
                let (value, c) = decode_string(input)?;
                input = &input[c..];
                let h = HeaderField { name, value };
                if list.admits(h.size()) {
                    list.fields.push(h);
                }
            } else {
                return Err(DecoderError::Truncated);
            }
        }
        match (list.over, list.limit) {
            (true, Some(limit)) => Err(DecoderError::HeaderListTooLarge { limit }),
            _ => Ok(list.fields),
        }
    }

    /// The name of a literal representation (§6.2): a string literal for
    /// index 0, else the name of the table entry.
    fn literal_name(
        &self,
        index: u64,
        input: &mut &[u8],
    ) -> Result<alloc::string::String, DecoderError> {
        if index == 0 {
            let (s, c) = decode_string(input)?;
            *input = &input[c..];
            return Ok(s);
        }
        let at = usize::try_from(index).map_err(|_| DecoderError::InvalidIndex(index))?;
        self.table.name(at).ok_or(DecoderError::InvalidIndex(index))
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

    /// RFC 7541 §6.3: a dynamic table size update above the protocol's
    /// limit is a decoding error; one at the limit is applied.
    #[test]
    fn rfc7541_6_3_a_size_update_above_the_limit_is_an_error() {
        let mut d = Decoder::new();
        assert_eq!(d.table_size_limit(), DEFAULT_TABLE_SIZE_LIMIT);
        let above = crate::encode_integer(4097, 5, 0x20);
        assert_eq!(
            d.decode(&above),
            Err(DecoderError::TableSizeAboveLimit {
                size: 4097,
                limit: 4096
            })
        );
        assert_eq!(d.table().max_size(), 4096);
        let huge = crate::encode_integer(1 << 20, 5, 0x20);
        assert!(matches!(
            d.decode(&huge),
            Err(DecoderError::TableSizeAboveLimit { .. })
        ));
        d.decode(&crate::encode_integer(4096, 5, 0x20)).unwrap();
        assert_eq!(d.table().max_size(), 4096);
        let mut zero = Decoder::with_max_size(0);
        assert!(zero.decode(&crate::encode_integer(1, 5, 0x20)).is_err());
        zero.set_table_size_limit(100);
        zero.decode(&crate::encode_integer(100, 5, 0x20)).unwrap();
        assert_eq!(zero.table().max_size(), 100);
    }

    /// RFC 7541 §4.2: size updates stand at the start of a block (two in a
    /// row are allowed, §4.2); one after a header field is an error.
    #[test]
    fn rfc7541_4_2_a_size_update_after_a_field_is_an_error() {
        let mut d = Decoder::new();
        let mut start = alloc::vec![0x20];
        start.extend_from_slice(&crate::encode_integer(4096, 5, 0x20));
        start.push(0x82);
        assert_eq!(d.decode(&start).unwrap(), alloc::vec![hf(":method", "GET")]);
        assert_eq!(
            d.decode(&[0x82, 0x20]),
            Err(DecoderError::TableSizeUpdateNotAtStart)
        );
    }

    /// The block of the review of zerodds-grpc-bridge-1.0: a literal of
    /// 4 000 octets with incremental indexing, then `n` times its index
    /// (0xbe, the first dynamic entry).
    fn amplifying_block(n: usize) -> Vec<u8> {
        let mut b = alloc::vec![0x83, 0x04, 0x01, b'/'];
        b.push(0x40);
        b.extend_from_slice(&crate::encode_string("x", false));
        b.extend_from_slice(&crate::encode_string(&"v".repeat(4000), false));
        b.extend(core::iter::repeat_n(0xbe, n));
        b
    }

    /// RFC 9113 §6.5.2: with a header list limit the decoder returns a list
    /// up to it and refuses a larger one, counting 32 octets per field;
    /// past the limit it decodes the block to the end without copying an
    /// entry, so its dynamic table stays in step for the next block.
    #[test]
    fn rfc9113_6_5_2_a_header_list_above_the_limit_is_refused() {
        // :method POST (32 + 7 + 4), :path / (32 + 5 + 1), x (32 + 1 + 4000).
        let one = 32 + 1 + 4000;
        let base = 43 + 38 + one;
        let mut d = Decoder::new();
        d.set_max_header_list_size(Some(base + one));
        assert_eq!(d.max_header_list_size(), Some(base + one));
        let fields = d.decode(&amplifying_block(1)).unwrap();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[3].value.len(), 4000);
        assert_eq!(
            d.decode(&amplifying_block(60_000)),
            Err(DecoderError::HeaderListTooLarge { limit: base + one })
        );
        // The literal of the refused block is in the table: the next
        // block refers to it.
        assert_eq!(d.table().len(), 1);
        let next = d.decode(&[0xbe]).unwrap();
        assert_eq!(next, alloc::vec![hf("x", &"v".repeat(4000))]);
        // Without a limit the whole list comes back.
        let mut open = Decoder::new();
        assert_eq!(open.decode(&amplifying_block(100)).unwrap().len(), 103);
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
