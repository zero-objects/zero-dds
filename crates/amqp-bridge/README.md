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

Frame-Header Round-Trip:

```rust
use zerodds_amqp_bridge::{FrameHeader, FrameType, encode_frame_header, decode_frame_header};

let header = FrameHeader {
    size: 16,
    doff: 2,
    frame_type: FrameType::Amqp,
    channel: 0,
};
let mut buf = [0u8; 8];
let written = encode_frame_header(&header, &mut buf).expect("encode");
let (decoded, consumed) = decode_frame_header(&buf).expect("decode");
assert_eq!(written, consumed);
assert_eq!(decoded.frame_type, FrameType::Amqp);
```

## Feature-Flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` impls. |
| `alloc` | ✅ (via std) | `Vec` / `String`. The crate is `no_std`-capable: `default-features = false, features = ["alloc"]`. |
| `codec-lite` | ❌ | DDS-AMQP-1.0 §2.4 codec-lite profile marker (conformance claim, no code-path effect). |

## Stability

`1.0.0-rc.1` is the initial release materialization. The public API,
wire format (OASIS AMQP 1.0) and error discriminants are RC1-
stable; breaking changes require a major bump.

## Tests

```bash
cargo test -p zerodds-amqp-bridge
```

188 tests green:
- 82 unit tests in src/ (type system + frame + performatives + sections + extended types + codec profile).
- 90 boundary-decoder tests in `tests/boundary_decoders.rs` (mutation-survival reduction).
- 8 property tests in `tests/proptest_roundtrip.rs` (round-trip invariants).
- 8 fuzz smoke tests in `tests/fuzz_smoke.rs` (pseudo-random byte-stream decoder, no panic).

Coverage-guided fuzzing via `cargo-fuzz`, see `fuzz/README.md` (nightly opt-in).

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`zerodds-amqp-endpoint`](../amqp-endpoint) — DDS-AMQP-1.0 endpoint layer.
