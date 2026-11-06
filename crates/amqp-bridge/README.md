# `zerodds-amqp-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-amqp-bridge/badge.svg)](https://docs.rs/zerodds-amqp-bridge)

OASIS AMQP 1.0 wire codec — pure-Rust `no_std + alloc`,
`forbid(unsafe_code)`. Implements the complete AMQP-1.0
type system (primitive + compound), the frame format (`amqp-1.0-
transport` §2.3), all 9 performatives (`open` / `begin` / `attach` /
`flow` / `transfer` / `disposition` / `detach` / `end` / `close`),
all 7 message sections (Header / Delivery-Annotations / Message-
Annotations / Properties / Application-Properties / Body / Footer)
and the DDS-AMQP-1.0 codec / codec-lite profile marker. Safety
classification: **STANDARD**.

## Spec-Mapping

| Spec | Section |
|------|-----------|
| OASIS AMQP 1.0 (Types) | §1.6 (Primitive Types), §1.7 (Restricted Types), §3 (Variable-Width-Encodings) |
| OASIS AMQP 1.0 (Transport) | §2.3 (Frame-Format), §2.7 (Performatives), §6 (Connections / Sessions / Links) |
| OASIS AMQP 1.0 (Messaging) | §3 (Message-Format), §3.2 (Section ordering) |
| OMG DDS-AMQP 1.0 (formal/2024-08-01) | §2.3 (Codec-Profile), §2.4 (Codec-Lite-Profile), §6.1 (Direct-Embed-Topology), §7 (Type-System-Mapping), §8 (Message-Section-Mapping) |

## What's included

- **`AmqpValue` / `FormatCode`** — variant model + all format codes
  (primitive + compound).
- **`AmqpExtValue`** — extended variant model with `decimal32`/`64`/
  `128` and all `int` tail types.
- **`encode_*` / `decode_*`** — primitive encoder/decoder per type.
- **`decode_value`** — universal decoder across all format codes.
- **`FrameHeader` / `FrameType` / `encode_frame_header` /
  `decode_frame_header`** — 4-byte SIZE BE + DOFF + TYPE + CHANNEL BE
  + extended header.
- **`open` / `begin` / ... / `close`** — performative builders.
- **`encode_performative` / `decode_performative`** — round-trip codec.
- **`MessageSection`** — all 9 section types.
- **`validate_section_sequence`** — §3.2 ordering check.
- **`codec_profile::{CodecProfile, active_profile, is_codec_lite_value, is_codec_lite_section}`** — DDS-AMQP-1.0 §2.4 codec-lite marker.

## Layer position

Layer 5 — Bridges. Substrate for:

- [`zerodds-amqp-endpoint`](../amqp-endpoint) — DDS-AMQP-1.0 endpoint
  layer (Direct-Embed-Topology, connection / session / link lifecycle).

## Quickstart

```rust
use zerodds_amqp_bridge::{decode_value, encode_long, encode_string, AmqpValue};

let buf = encode_long(42);
let (v, consumed) = decode_value(&buf).expect("decode");
assert_eq!(v, AmqpValue::Long(42));
assert_eq!(consumed, buf.len());
```

Frame header (OASIS AMQP 1.0 Part 2 §2.3.1):

```rust
use zerodds_amqp_bridge::{FrameHeader, FrameType, encode_frame_header, decode_frame_header};

let header = FrameHeader {
    size: 16,
    doff: 2,
    frame_type: FrameType::Amqp,
    channel: 0,
};
let octets = encode_frame_header(header);
assert_eq!(octets, [0, 0, 0, 16, 2, 0, 0, 0]);
let decoded = decode_frame_header(&octets).expect("decode");
assert_eq!(decoded.frame_type, FrameType::Amqp);
```

## Feature-Flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` impls. |
| `alloc` | ✅ (via std) | `Vec` / `String`. The crate is `no_std`-capable: `default-features = false, features = ["alloc"]`. |
| `codec-lite` | ❌ | DDS-AMQP-1.0 §2.4 codec-lite profile marker (conformance claim, no code-path effect). |

## Stability

`1.0.0-rc.7`, a pre-release. The public API can still change before
1.0.0; the crate's CHANGELOG lists every change.

## Tests

```bash
cargo test -p zerodds-amqp-bridge
```

The codec tests check the octets of OASIS AMQP 1.0 Parts 1 to 3 and 5: each
encoding is written to the octets the specification gives and read back from
them (`src/types.rs`, `src/extended_types.rs`, `src/frame.rs`,
`src/sections.rs`, `src/performatives.rs`). `tests/boundary_decoders.rs` reads
encodings cut at each length, `tests/fuzz_smoke.rs` feeds pseudo-random octets
to the decoders (no panic), `tests/type_mapping.rs` checks the DDS-AMQP 1.0 type
mapping. `tests/cross_language_agreement.rs` compiles the emitted C++, Java and
TypeScript codecs and compares their octets and JSON with the Rust reference; a
missing compiler fails the test, and the Java column runs in the integration
pillar's job `integration-bridges-vendor-amqp`.

Coverage-guided fuzzing via `cargo-fuzz`, see `fuzz/README.md` (nightly opt-in).

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`zerodds-amqp-endpoint`](../amqp-endpoint) — DDS-AMQP-1.0 endpoint layer.
