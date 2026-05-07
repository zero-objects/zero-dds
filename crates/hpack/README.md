# `zerodds-hpack`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-hpack/badge.svg)](https://docs.rs/zerodds-hpack)

HPACK (RFC 7541) Header-Compression-Codec fuer HTTP/2: Variable-
Length-Integer, String-Literals (mit/ohne Huffman, Appendix B),
Static-Table (61 Eintraege, Appendix A), Dynamic-Table mit
SETTINGS_HEADER_TABLE_SIZE-Lifecycle und alle vier Header-Field-
Repraesentationen aus §6. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| RFC 7541 (HPACK) | §2.3 (Indexing-Tables), §4 (Dynamic-Table-Management), §5.1 (Integer), §5.2 (String-Literals), §6.1 (Indexed-Header), §6.2.1 (Literal-with-Indexing), §6.2.2 (Literal-without-Indexing), §6.2.3 (Literal-Never-Indexed), §6.3 (Dynamic-Table-Size-Update), Appendix A (Static-Table), Appendix B (Huffman) |

## Was ist drin

- **`Encoder` / `Decoder`** — High-Level-Codec mit eigener Dynamic-
  Table und Indexing-Strategie (Voll-Match → indexed; Name-Only →
  literal-with-indexing-indexed-name; sonst → literal-with-indexing-
  new-name).
- **`Table` / `HeaderField`** — Combined-Lookup ueber Static + Dynamic
  (Index 1..=61 = Static, 62..N = Dynamic), Eviction per Spec §4.4
  (Single-Entry-Too-Large clears die Tabelle).
- **`STATIC_TABLE` / `StaticTableEntry`** — die 61 Appendix-A-Eintraege
  als `&'static`-Konstante.
- **`encode_integer` / `decode_integer`** — Variable-Length-Integer
  mit konfigurierbarer Prefix-Bit-Position (§5.1).
- **`encode_string` / `decode_string`** — String-Literal mit optional
  Huffman-Compression (§5.2).
- **`huffman::encode` / `huffman::decode`** — Static-Huffman-Code aus
  Appendix B mit EOS-Padding-Detection.

## Schichten-Position

Layer 5 — Bridges. Substrat fuer:

- [`zerodds-http2`](../http2) — RFC 9113 Framing + Stream-State-
  Machine.
- [`zerodds-grpc-bridge`](../grpc-bridge) — gRPC-over-HTTP/2 +
  gRPC-Web Length-Prefixed-Message-Codec.

## Quickstart

```rust
use zerodds_hpack::{Decoder, Encoder, HeaderField};

let mut encoder = Encoder::new();
let mut decoder = Decoder::new();

let headers = vec![
    HeaderField { name: ":method".into(), value: "GET".into() },
    HeaderField { name: ":scheme".into(), value: "https".into() },
    HeaderField { name: "custom-key".into(), value: "custom-value".into() },
];

let wire = encoder.encode(&headers);
let decoded = decoder.decode(&wire).expect("roundtrip");
assert_eq!(decoded, headers);
```

Huffman-Compression aktivieren:

```rust
use zerodds_hpack::Encoder;

let mut encoder = Encoder::new();
encoder.use_huffman = true;
```

Dynamic-Table-Size konfigurieren (z.B. wenn HTTP/2-Peer
`SETTINGS_HEADER_TABLE_SIZE` schickt):

```rust
use zerodds_hpack::Decoder;

let mut decoder = Decoder::with_max_size(8192);
decoder.table_mut().set_max_size(4096);
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` fuer alle Fehler-Typen. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `VecDeque`. |

Crate ist `no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1` ist die initiale Release-Materialisierung. Public-API,
Wire-Format (RFC 7541) und Fehler-Diskriminanten sind RC1-stabil;
Breaking-Changes erfordern Major-Bump.

## Tests

```bash
cargo test -p zerodds-hpack
```

49 Unit-Tests: Integer-Coding (8, davon 3 RFC-7541-Appendix-C-1-
Vektoren), String-Coding (7, inkl. Huffman-Roundtrip), Huffman (7),
Table-Management (14), Encoder (7), Decoder (8 inkl. RFC-7541-Appendix-
C-2.1-Vektor + Dynamic-Table-Size-Update + Invalid-Index-Rejection).

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/hpack.md`](../../docs/release/rc1-reviews/hpack.md) — RC1-Review.
- [`zerodds-http2`](../http2) — HTTP/2-Framing-Konsument.
- [`zerodds-grpc-bridge`](../grpc-bridge) — gRPC-Konsument.
