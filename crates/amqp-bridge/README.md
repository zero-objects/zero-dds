# `zerodds-amqp-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-amqp-bridge/badge.svg)](https://docs.rs/zerodds-amqp-bridge)

OASIS AMQP 1.0 Wire-Codec — pure-Rust `no_std + alloc`,
`forbid(unsafe_code)`. Implementiert das vollstaendige AMQP-1.0
Type-System (Primitive + Compound), das Frame-Format (`amqp-1.0-
transport` §2.3), alle 9 Performatives (`open` / `begin` / `attach` /
`flow` / `transfer` / `disposition` / `detach` / `end` / `close`),
alle 7 Message-Sections (Header / Delivery-Annotations / Message-
Annotations / Properties / Application-Properties / Body / Footer)
und den DDS-AMQP-1.0 Codec-/Codec-Lite-Profile-Marker. Safety
classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OASIS AMQP 1.0 (Types) | §1.6 (Primitive Types), §1.7 (Restricted Types), §3 (Variable-Width-Encodings) |
| OASIS AMQP 1.0 (Transport) | §2.3 (Frame-Format), §2.7 (Performatives), §6 (Connections / Sessions / Links) |
| OASIS AMQP 1.0 (Messaging) | §3 (Message-Format), §3.2 (Section-Reihenfolge) |
| OMG DDS-AMQP 1.0 (formal/2024-08-01) | §2.3 (Codec-Profile), §2.4 (Codec-Lite-Profile), §6.1 (Direct-Embed-Topology), §7 (Type-System-Mapping), §8 (Message-Section-Mapping) |

## Was ist drin

- **`AmqpValue` / `FormatCode`** — Variant-Modell + alle Format-Codes
  (Primitive + Compound).
- **`AmqpExtValue`** — erweitertes Variant-Modell mit `decimal32`/`64`/
  `128` und alle `int`-Tail-Typen.
- **`encode_*` / `decode_*`** — Primitiv-Encoder/Decoder pro Typ.
- **`decode_value`** — universaler Decoder ueber alle Format-Codes.
- **`FrameHeader` / `FrameType` / `encode_frame_header` /
  `decode_frame_header`** — 4-Byte SIZE BE + DOFF + TYPE + CHANNEL BE
  + Extended-Header.
- **`open` / `begin` / ... / `close`** — Performative-Builder.
- **`encode_performative` / `decode_performative`** — Round-Trip-Codec.
- **`MessageSection`** — alle 9 Section-Typen.
- **`validate_section_sequence`** — §3.2-Reihenfolge-Pruefung.
- **`codec_profile::{CodecProfile, active_profile, is_codec_lite_value, is_codec_lite_section}`** — DDS-AMQP-1.0 §2.4 Codec-Lite-Marker.

## Schichten-Position

Layer 5 — Bridges. Substrat fuer:

- [`zerodds-amqp-endpoint`](../amqp-endpoint) — DDS-AMQP-1.0 Endpoint-
  Layer (Direct-Embed-Topology, Connection-/Session-/Link-Lifecycle).

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

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error`-Impls. |
| `alloc` | ✅ (via std) | `Vec` / `String`. Crate ist `no_std`-fahig: `default-features = false, features = ["alloc"]`. |
| `codec-lite` | ❌ | DDS-AMQP-1.0 §2.4 Codec-Lite-Profile-Marker (Conformance-Claim, kein Code-Pfad-Effekt). |

## Stabilitaet

`1.0.0-rc.1` ist die initiale Release-Materialisierung. Public-API,
Wire-Format (OASIS AMQP 1.0) und Fehler-Diskriminanten sind RC1-
stabil; Breaking-Changes erfordern Major-Bump.

## Tests

```bash
cargo test -p zerodds-amqp-bridge
```

188 Tests grün:
- 82 Unit-Tests in src/ (Type-System + Frame + Performatives + Sections + Extended-Types + Codec-Profile).
- 90 Boundary-Decoder-Tests in `tests/boundary_decoders.rs` (Mutation-Survival-Reduction).
- 8 Property-Tests in `tests/proptest_roundtrip.rs` (Roundtrip-Invarianten).
- 8 Fuzz-Smoke-Tests in `tests/fuzz_smoke.rs` (Pseudo-Random-Bytes-Stream-Decoder, kein Panic).

Coverage-guided Fuzzing via `cargo-fuzz` siehe `fuzz/README.md` (nightly opt-in).

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/amqp-bridge.md`](../../docs/release/rc1-reviews/amqp-bridge.md) — RC1-Review.
- [`zerodds-amqp-endpoint`](../amqp-endpoint) — DDS-AMQP-1.0 Endpoint-Layer.
