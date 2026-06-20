# `zerodds-hpack`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-hpack/badge.svg)](https://docs.rs/zerodds-hpack)

HPACK (RFC 7541) header-compression codec for HTTP/2: variable-
length integer, string literals (with/without Huffman, Appendix B),
static table (61 entries, Appendix A), dynamic table with
SETTINGS_HEADER_TABLE_SIZE lifecycle and all four header-field
representations from §6. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| RFC 7541 (HPACK) | §2.3 (indexing tables), §4 (dynamic-table management), §5.1 (integer), §5.2 (string literals), §6.1 (indexed header), §6.2.1 (literal with indexing), §6.2.2 (literal without indexing), §6.2.3 (literal never indexed), §6.3 (dynamic-table size update), Appendix A (static table), Appendix B (Huffman) |

## What's inside

- **`Encoder` / `Decoder`** — high-level codec with its own dynamic
  table and indexing strategy (full match → indexed; name-only →
  literal-with-indexing indexed name; otherwise → literal-with-indexing
  new name).
- **`Table` / `HeaderField`** — combined lookup over static + dynamic
  (index 1..=61 = static, 62..N = dynamic), eviction per spec §4.4
  (a single entry too large clears the table).
- **`STATIC_TABLE` / `StaticTableEntry`** — the 61 Appendix A entries
  as a `&'static` constant.
- **`encode_integer` / `decode_integer`** — variable-length integer
  with configurable prefix-bit position (§5.1).
- **`encode_string` / `decode_string`** — string literal with optional
  Huffman compression (§5.2).
- **`huffman::encode` / `huffman::decode`** — static Huffman code from
  Appendix B with EOS-padding detection.

## Layer position

Layer 5 — Bridges. Substrate for:

- [`zerodds-http2`](../http2) — RFC 9113 framing + stream state
  machine.
- [`zerodds-grpc-bridge`](../grpc-bridge) — gRPC-over-HTTP/2 +
  gRPC-Web length-prefixed-message codec.

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

Enable Huffman compression:

```rust
use zerodds_hpack::Encoder;

let mut encoder = Encoder::new();
encoder.use_huffman = true;
```

Configure the dynamic-table size (e.g. when an HTTP/2 peer
sends `SETTINGS_HEADER_TABLE_SIZE`):

```rust
use zerodds_hpack::Decoder;

let mut decoder = Decoder::with_max_size(8192);
decoder.table_mut().set_max_size(4096);
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` for all error types. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `VecDeque`. |

The crate is `no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1` is the initial release materialization. The public API,
wire format (RFC 7541) and error discriminants are RC1-stable;
breaking changes require a major bump.

## Tests

```bash
cargo test -p zerodds-hpack
```

49 unit tests: integer coding (8, of which 3 are RFC 7541 Appendix C.1
vectors), string coding (7, incl. Huffman roundtrip), Huffman (7),
table management (14), encoder (7), decoder (8, incl. RFC 7541 Appendix
C.2.1 vector + dynamic-table size update + invalid-index rejection).

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`zerodds-http2`](../http2) — HTTP/2 framing consumer.
- [`zerodds-grpc-bridge`](../grpc-bridge) — gRPC consumer.
