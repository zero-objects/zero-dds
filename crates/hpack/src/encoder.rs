// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HPACK Encoder — RFC 7541 §6.
//!
//! Vier Header-Field-Repraesentationen:
//!
//! * `Indexed Header Field` (§6.1) — Bit 7 = 1.
//! * `Literal Header Field with Incremental Indexing` (§6.2.1) — Bit
//!   7..6 = 01.
//! * `Literal Header Field without Indexing` (§6.2.2) — Bit 7..4 = 0000.
//! * `Literal Header Field never Indexed` (§6.2.3) — Bit 7..4 = 0001.

use alloc::vec::Vec;

use crate::integer::encode_integer;
use crate::string::encode_string;
use crate::table::{HeaderField, Table};

/// Encoder-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderError {
    /// Reserviert fuer kuenftige Erweiterungen — aktuell unused.
    Reserved,
}

/// HPACK-Encoder mit eigener Dynamic-Table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Encoder {
    table: Table,
    /// Wenn `true`, werden String-Literals Huffman-komprimiert (Spec
    /// §5.2 erlaubt beides).
    pub use_huffman: bool,
}

impl Encoder {
    /// Konstruktor mit Default-Max-Size 4096.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Konstruktor mit konfigurierter Max-Size.
    #[must_use]
    pub fn with_max_size(max: usize) -> Self {
        Self {
            table: Table::new(max),
            use_huffman: false,
        }
    }

    /// Reference auf die Dynamic-Table.
    #[must_use]
    pub fn table(&self) -> &Table {
        &self.table
    }

    /// Mut-Reference (Caller passt Max-Size an).
    pub fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }

    /// Encode eine Header-Liste in einen Output-Buffer.
    ///
    /// Strategie:
    /// * Voll-Match → `Indexed Header Field` (Spec §6.1).
    /// * Name-Only-Match → `Literal with Incremental Indexing,
    ///   indexed name`.
    /// * Kein Match → `Literal with Incremental Indexing, new name`.
    #[must_use]
    pub fn encode(&mut self, headers: &[HeaderField]) -> Vec<u8> {
        let mut out = Vec::new();
        for h in headers {
            match self.table.find(&h.name, &h.value) {
                Some((index, true)) => {
                    // §6.1: 1xxx_xxxx — 7-Bit-Index.
                    let buf = encode_integer(index as u64, 7, 0x80);
                    out.extend_from_slice(&buf);
                }
                Some((index, false)) => {
                    // §6.2.1: 01xx_xxxx — 6-Bit-Index, dann Value-String.
                    let buf = encode_integer(index as u64, 6, 0x40);
                    out.extend_from_slice(&buf);
                    out.extend_from_slice(&encode_string(&h.value, self.use_huffman));
                    self.table.add(h.clone());
                }
                None => {
                    // §6.2.1: 0100_0000 + Name-Literal + Value-Literal.
                    out.push(0x40);
                    out.extend_from_slice(&encode_string(&h.name, self.use_huffman));
                    out.extend_from_slice(&encode_string(&h.value, self.use_huffman));
                    self.table.add(h.clone());
                }
            }
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn hf(n: &str, v: &str) -> HeaderField {
        HeaderField {
            name: n.into(),
            value: v.into(),
        }
    }

    #[test]
    fn full_static_match_is_single_byte() {
        let mut e = Encoder::new();
        let buf = e.encode(&[hf(":method", "GET")]);
        assert_eq!(buf, alloc::vec![0x82]); // index 2 with high-bit set
    }

    #[test]
    fn name_only_static_emits_literal_value() {
        let mut e = Encoder::new();
        let buf = e.encode(&[hf(":method", "PATCH")]);
        // 0x42 (01_000010) = literal incremental, indexed name=2 (or 3)
        assert!(buf[0] & 0xc0 == 0x40);
        // Should add to dynamic table.
        assert_eq!(e.table().len(), 1);
    }

    #[test]
    fn unknown_header_emits_literal_name_and_value() {
        let mut e = Encoder::new();
        let buf = e.encode(&[hf("custom", "value")]);
        assert_eq!(buf[0], 0x40); // literal incremental, new name
        assert_eq!(e.table().len(), 1);
    }

    #[test]
    fn second_encode_uses_dynamic_table_match() {
        let mut e = Encoder::new();
        let _ = e.encode(&[hf("custom", "value")]);
        let buf = e.encode(&[hf("custom", "value")]);
        // Should be Indexed → high bit set.
        assert_eq!(buf[0] & 0x80, 0x80);
    }

    #[test]
    fn huffman_flag_compresses_literal_strings() {
        let mut e = Encoder::with_max_size(4096);
        e.use_huffman = true;
        let buf = e.encode(&[hf("custom", "value")]);
        // Length-prefix bytes have H-flag set on at least name + value.
        // Literal-name byte at offset 1.
        assert!(buf[1] & 0x80 == 0x80, "name string should have H-flag");
    }

    #[test]
    fn multiple_headers_encoded_sequentially() {
        let mut e = Encoder::new();
        let buf = e.encode(&[hf(":method", "GET"), hf(":scheme", "https")]);
        assert_eq!(buf, alloc::vec![0x82, 0x87]); // indices 2 and 7
    }

    #[test]
    fn encoder_default_uses_no_huffman() {
        let e = Encoder::new();
        assert!(!e.use_huffman);
    }
}
