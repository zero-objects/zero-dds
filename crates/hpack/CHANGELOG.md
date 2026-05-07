# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-hpack`-Crate.

### Spec-Referenzen

- **RFC 7541** §2.3 (Indexing-Tables), §4 (Dynamic-Table-Management), §5.1 (Integer-Representation), §5.2 (String-Literal-Representation), §6.1 (Indexed-Header-Field), §6.2.1 (Literal-Header-Field-with-Incremental-Indexing), §6.2.2 (Literal-Header-Field-without-Indexing), §6.2.3 (Literal-Header-Field-Never-Indexed), §6.3 (Dynamic-Table-Size-Update), Appendix A (Static-Table, 61 Eintraege), Appendix B (Static-Huffman-Code).

### Public-API

**Encoder/Decoder:**

- `Encoder::{new, with_max_size, table, table_mut, encode}`, `Encoder::use_huffman` (Field).
- `EncoderError` (reserviert; aktuell unused — RFC 7541 §6 erlaubt Encoder-Fehlerfreiheit, weil jeder Header-Set codierbar ist).
- `Decoder::{new, with_max_size, table, table_mut, decode}`.
- `DecoderError::{InvalidIndex, Integer, String, Truncated}` mit `Display` + `From<IntegerError>` + `From<StringError>` + `std::error::Error` (Feature `std`).

**Table-Modell:**

- `STATIC_TABLE: [StaticTableEntry; 61]` — Appendix A.
- `StaticTableEntry { name, value }` (`&'static str` Felder).
- `HeaderField { name, value }` (`String` Felder), `HeaderField::size` (Spec §4.1: 32 + name.len + value.len).
- `Table::{new, default, add, get, find, size, max_size, set_max_size, len, is_empty}` — Combined-Lookup + FIFO-Eviction.

**Primitiv-Codec:**

- `encode_integer(value: u64, prefix_bits: u8, out_byte_prefix_bits: u8) -> Vec<u8>` (§5.1).
- `decode_integer(input: &[u8], prefix_bits: u8) -> Result<(u64, usize), IntegerError>`.
- `IntegerError::{Truncated, TooLarge}`.
- `encode_string(s: &str, huffman_compress: bool) -> Vec<u8>` (§5.2).
- `decode_string(input: &[u8]) -> Result<(String, usize), StringError>`.
- `decode_bytes(input: &[u8]) -> Result<(Vec<u8>, usize), StringError>` — Octet-Pfad (HPACK erlaubt non-UTF8).
- `StringError::{Integer, Truncated, Huffman, NotUtf8}`.

**Huffman:**

- `huffman::encode(bytes: &[u8]) -> Vec<u8>` (Appendix B).
- `huffman::decode(bytes: &[u8]) -> Result<Vec<u8>, HuffmanError>`.
- `HuffmanError`.

### Implementierung

`Encoder` und `Decoder` halten je eine eigene `Table`, weil Sender und Receiver ihre Dynamic-Tabellen unabhaengig syncen — geteiltes State waere ein Spec-Verstoss (§4.1, „kept independent in encoder and decoder"). Encoder-Strategie: `Table::find` liefert `(index, full_match)`; Voll-Match → 7-Bit-Indexed (§6.1, MSB=1), Name-Only-Match → 6-Bit-Indexed-Name + Value-Literal (§6.2.1), kein Match → 0x40 + Name-Literal + Value-Literal. Beide Literal-Pfade adden in die Dynamic-Table (Incremental-Indexing).

Decoder dispatcht auf den ersten Byte: `0b1xxxxxxx` = Indexed (§6.1), `0b01xxxxxx` = Literal-Incremental (§6.2.1), `0b001xxxxx` = Dynamic-Table-Size-Update (§6.3), `0b0000xxxx` = Literal-without-Indexing (§6.2.2), `0b0001xxxx` = Literal-Never-Indexed (§6.2.3, semantisch identisch zu §6.2.2 fuer den Codec — die „never indexed"-Direktive ist eine Hop-by-Hop-Hint fuer Proxies, semantisch fuer den Codec aequivalent).

Variable-Length-Integer-Decode rejected `shift >= 56` als `TooLarge` (= Continuation > 8 Bytes), weil RFC 7541 §5.1 Implementations explizit erlaubt einen Limit zu enforcen. Truncated-Continuation (kein Folge-Byte mit MSB=0) wird als `Truncated` zurueckgegeben.

Static-Huffman-Code aus Appendix B ist als 257-Eintrags-Konstanten-Tabelle `[(code: u32, bit_length: u8); 257]` materialisiert (Index 256 = EOS, nur fuer Padding). Decoder akzeptiert Trailing-Padding bis zu 7 EOS-Bits. Truncated- oder zu-lange-Padding-Codes werden als `HuffmanError` rejected.

`Table::add` implementiert Spec §4.4 streng: ein Entry der allein die Max-Size ueberschreitet leert die ganze Tabelle (statt nur ihn selbst zu skippen). Eviction passiert FIFO vom Tabel-Ende.

`#![no_std]` + `extern crate alloc;` erlaubt Embedded-Builds; `std`-Feature aktiviert nur die `std::error::Error`-Impls.

### Architektur

- **Layer:** 5 (Bridges).
- **Dependencies (in):** keine (Substrat-Crate). Nur `core` + `alloc`.
- **Dependents (out):** `zerodds-http2` (HEADERS-/CONTINUATION-Frame-Bodies), `zerodds-grpc-bridge` (HTTP/2-Header-Block fuer gRPC-Frames), `zerodds-conformance` (Cross-Vendor-Test-Harness).
- **Feature-Flags:** `std` (default, aktiviert `std::error::Error`-Impls), `alloc` (via std, immer noetig).

### Stabilitaet

- Public-API: RC1-stabil. Module-Pfade `decoder`, `encoder`, `huffman`, `integer`, `string`, `table` sind explizit `pub mod` und Teil des stabilen Surface (Caller darf direkt auf z.B. `huffman::encode` zugreifen ohne Re-Export).
- Wire-Format: durch RFC 7541 fixiert; Aenderung waere Spec-Breaking.
- Fehler-Diskriminanten: stabil; neue Diskriminanten sind Major-additive (Caller pattern-matched moeglicherweise erschoepfend).
