# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-hpack` crate.

### Spec references

- **RFC 7541** §2.3 (indexing tables), §4 (dynamic-table management), §5.1 (integer representation), §5.2 (string-literal representation), §6.1 (indexed header field), §6.2.1 (literal header field with incremental indexing), §6.2.2 (literal header field without indexing), §6.2.3 (literal header field never indexed), §6.3 (dynamic-table size update), Appendix A (static table, 61 entries), Appendix B (static Huffman code).

### Public API

**Encoder/Decoder:**

- `Encoder::{new, with_max_size, table, table_mut, encode}`, `Encoder::use_huffman` (field).
- `EncoderError` (reserved; currently unused — RFC 7541 §6 allows the encoder to be error-free because every header set is encodable).
- `Decoder::{new, with_max_size, table, table_mut, decode}`.
- `DecoderError::{InvalidIndex, Integer, String, Truncated}` with `Display` + `From<IntegerError>` + `From<StringError>` + `std::error::Error` (feature `std`).

**Table model:**

- `STATIC_TABLE: [StaticTableEntry; 61]` — Appendix A.
- `StaticTableEntry { name, value }` (`&'static str` fields).
- `HeaderField { name, value }` (`String` fields), `HeaderField::size` (spec §4.1: 32 + name.len + value.len).
- `Table::{new, default, add, get, find, size, max_size, set_max_size, len, is_empty}` — combined lookup + FIFO eviction.

**Primitive codec:**

- `encode_integer(value: u64, prefix_bits: u8, out_byte_prefix_bits: u8) -> Vec<u8>` (§5.1).
- `decode_integer(input: &[u8], prefix_bits: u8) -> Result<(u64, usize), IntegerError>`.
- `IntegerError::{Truncated, TooLarge}`.
- `encode_string(s: &str, huffman_compress: bool) -> Vec<u8>` (§5.2).
- `decode_string(input: &[u8]) -> Result<(String, usize), StringError>`.
- `decode_bytes(input: &[u8]) -> Result<(Vec<u8>, usize), StringError>` — octet path (HPACK allows non-UTF8).
- `StringError::{Integer, Truncated, Huffman, NotUtf8}`.

**Huffman:**

- `huffman::encode(bytes: &[u8]) -> Vec<u8>` (Appendix B).
- `huffman::decode(bytes: &[u8]) -> Result<Vec<u8>, HuffmanError>`.
- `HuffmanError`.

### Implementation

`Encoder` and `Decoder` each hold their own `Table`, because the sender and receiver sync their dynamic tables independently — shared state would be a spec violation (§4.1, "kept independent in encoder and decoder"). Encoder strategy: `Table::find` returns `(index, full_match)`; full match → 7-bit indexed (§6.1, MSB=1), name-only match → 6-bit indexed name + value literal (§6.2.1), no match → 0x40 + name literal + value literal. Both literal paths add to the dynamic table (incremental indexing).

The decoder dispatches on the first byte: `0b1xxxxxxx` = indexed (§6.1), `0b01xxxxxx` = literal incremental (§6.2.1), `0b001xxxxx` = dynamic-table size update (§6.3), `0b0000xxxx` = literal without indexing (§6.2.2), `0b0001xxxx` = literal never indexed (§6.2.3, semantically identical to §6.2.2 for the codec — the "never indexed" directive is a hop-by-hop hint for proxies, semantically equivalent for the codec).

Variable-length integer decode rejects `shift >= 56` as `TooLarge` (= continuation > 8 bytes), because RFC 7541 §5.1 explicitly allows implementations to enforce a limit. A truncated continuation (no following byte with MSB=0) is returned as `Truncated`.

The static Huffman code from Appendix B is materialized as a 257-entry constant table `[(code: u32, bit_length: u8); 257]` (index 256 = EOS, only for padding). The decoder accepts trailing padding of up to 7 EOS bits. Truncated or overlong padding codes are rejected as `HuffmanError`.

`Table::add` implements spec §4.4 strictly: an entry that alone exceeds the max size clears the entire table (instead of just skipping it). Eviction happens FIFO from the end of the table.

`#![no_std]` + `extern crate alloc;` allows embedded builds; the `std` feature only enables the `std::error::Error` impls.

### Architecture

- **Layer:** 5 (Bridges).
- **Dependencies (in):** none (substrate crate). Only `core` + `alloc`.
- **Dependents (out):** `zerodds-http2` (HEADERS/CONTINUATION frame bodies), `zerodds-grpc-bridge` (HTTP/2 header block for gRPC frames), `zerodds-conformance` (cross-vendor test harness).
- **Feature flags:** `std` (default, enables the `std::error::Error` impls), `alloc` (via std, always required).

### Stability

- Public API: RC1-stable. The module paths `decoder`, `encoder`, `huffman`, `integer`, `string`, `table` are explicitly `pub mod` and part of the stable surface (callers may access e.g. `huffman::encode` directly without a re-export).
- Wire format: fixed by RFC 7541; a change would be spec-breaking.
- Error discriminants: stable; new discriminants are major-additive (callers may pattern-match exhaustively).
