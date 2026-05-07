# `zerodds-http2`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-http2/badge.svg)](https://docs.rs/zerodds-http2)

HTTP/2 (RFC 9113) Wire-Codec: 9-Byte-Frame-Header + alle 10 Frame-
Types (`DATA` / `HEADERS` / `PRIORITY` / `RST_STREAM` / `SETTINGS` /
`PUSH_PROMISE` / `PING` / `GOAWAY` / `WINDOW_UPDATE` /
`CONTINUATION`), Connection-Preface, SETTINGS-Codec mit Defaults,
Stream-State-Machine (§5.1), und Connection + Stream Flow-Control
(§5.2 + §6.9). `no_std + alloc`, `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

> RFC 9113 hat RFC 7540 abgeloest und behaelt das Wire-Format mit
> identischen §-Nummern. Diese Crate folgt dem 9113-Stand.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| RFC 9113 (HTTP/2) | §3.4 (Connection-Preface), §4 (Frame-Layer), §5.1 (Stream-State-Machine), §5.2 (Flow-Control), §6.1-§6.10 (Frame-Types), §6.5 (SETTINGS), §6.9 (`WINDOW_UPDATE`), §7 (Error-Codes) |

## Was ist drin

- **`Frame` / `FrameHeader` / `FrameType` / `Flags`** — Frame-Layer-
  Modell (§4) inkl. Length/Type/Flags/Stream-Id-Header und
  zero-copy-Payload-Slice.
- **`encode_frame` / `decode_frame`** — Codec mit `max_frame_size`-
  Bound-Check (`SETTINGS_MAX_FRAME_SIZE`-Konformitaet).
- **`CLIENT_PREFACE` / `check_preface`** — 24-Byte-Connection-
  Preface (§3.4).
- **`Settings` / `Setting` / `SettingId`** — alle sechs Standard-
  Settings (`HEADER_TABLE_SIZE`, `ENABLE_PUSH`, `MAX_CONCURRENT_STREAMS`,
  `INITIAL_WINDOW_SIZE`, `MAX_FRAME_SIZE`, `MAX_HEADER_LIST_SIZE`).
- **`StreamId` / `StreamState`** — Stream-State-Machine mit allen
  §5.1-Uebergaengen.
- **`FlowControl`** — Connection + Stream Window-Tracking,
  Round-Trip-Window-Update-Codec, Overflow-Rejection (§5.2).
- **`ErrorCode` / `Http2Error`** — alle Standard-Error-Codes (§7).

## Schichten-Position

Layer 5 — Bridges. Substrat fuer:

- [`zerodds-grpc-bridge`](../grpc-bridge) — gRPC-over-HTTP/2 + gRPC-
  Web (HEADERS/CONTINUATION via [`zerodds-hpack`](../hpack)).

## Quickstart

```rust
use zerodds_http2::{FrameHeader, FrameType, Flags, encode_frame, decode_frame};
use zerodds_http2::frame::DEFAULT_MAX_FRAME_SIZE;

// PING-Frame (8-Byte-Opaque-Payload, Stream-ID 0).
let payload = [0u8; 8];
let header = FrameHeader {
    length: 8,
    frame_type: FrameType::Ping,
    flags: Flags(0),
    stream_id: 0,
};

let mut buf = [0u8; 17];
let written = encode_frame(&header, &payload, &mut buf, DEFAULT_MAX_FRAME_SIZE)
    .expect("encode");
assert_eq!(written, 17);

let (decoded, consumed) = decode_frame(&buf, DEFAULT_MAX_FRAME_SIZE).expect("decode");
assert_eq!(consumed, 17);
assert_eq!(decoded.header.frame_type, FrameType::Ping);
```

Connection-Preface verifizieren:

```rust,no_run
use zerodds_http2::{CLIENT_PREFACE, check_preface};

assert_eq!(CLIENT_PREFACE.len(), 24);
let consumed = check_preface(CLIENT_PREFACE).expect("preface");
assert_eq!(consumed, 24);
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` fuer alle Fehler-Typen. |
| `alloc` | ✅ (via std) | `Vec` / `String`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1` ist die initiale Release-Materialisierung. Public-API,
Wire-Format (RFC 9113) und Fehler-Diskriminanten sind RC1-stabil;
Breaking-Changes erfordern Major-Bump.

## Tests

```bash
cargo test -p zerodds-http2
```

45 Unit-Tests + 1 Doc-Test: Frame-Codec (9, inkl. Round-Trip + R-Bit-
Stripping + Buffer-Bound), Flow-Control (10), Error-Codes (3),
Connection-Preface (5), Settings (8), Stream-State-Transitions (10).

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/http2.md`](../../docs/release/rc1-reviews/http2.md) — RC1-Review.
- [`zerodds-hpack`](../hpack) — RFC 7541 HEADERS-Frame-Body-Codec.
- [`zerodds-grpc-bridge`](../grpc-bridge) — gRPC-Konsument.
