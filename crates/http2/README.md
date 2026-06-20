# `zerodds-http2`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-http2/badge.svg)](https://docs.rs/zerodds-http2)

HTTP/2 (RFC 9113) wire codec: 9-byte frame header + all 10 frame
types (`DATA` / `HEADERS` / `PRIORITY` / `RST_STREAM` / `SETTINGS` /
`PUSH_PROMISE` / `PING` / `GOAWAY` / `WINDOW_UPDATE` /
`CONTINUATION`), connection preface, SETTINGS codec with defaults,
stream state machine (§5.1), and connection + stream flow control
(§5.2 + §6.9). `no_std + alloc`, `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

> RFC 9113 superseded RFC 7540 and keeps the wire format with
> identical §-numbers. This crate follows the 9113 revision.

## Spec mapping

| Spec | Section |
|------|-----------|
| RFC 9113 (HTTP/2) | §3.4 (connection preface), §4 (frame layer), §5.1 (stream state machine), §5.2 (flow control), §6.1-§6.10 (frame types), §6.5 (SETTINGS), §6.9 (`WINDOW_UPDATE`), §7 (error codes) |

## What's inside

- **`Frame` / `FrameHeader` / `FrameType` / `Flags`** — frame-layer
  model (§4) including the length/type/flags/stream-id header and a
  zero-copy payload slice.
- **`encode_frame` / `decode_frame`** — codec with a `max_frame_size`
  bound check (`SETTINGS_MAX_FRAME_SIZE` conformance).
- **`CLIENT_PREFACE` / `check_preface`** — 24-byte connection
  preface (§3.4).
- **`Settings` / `Setting` / `SettingId`** — all six standard
  settings (`HEADER_TABLE_SIZE`, `ENABLE_PUSH`, `MAX_CONCURRENT_STREAMS`,
  `INITIAL_WINDOW_SIZE`, `MAX_FRAME_SIZE`, `MAX_HEADER_LIST_SIZE`).
- **`StreamId` / `StreamState`** — stream state machine with all
  §5.1 transitions.
- **`FlowControl`** — connection + stream window tracking,
  round-trip window-update codec, overflow rejection (§5.2).
- **`ErrorCode` / `Http2Error`** — all standard error codes (§7).

## Layer position

Layer 5 — Bridges. Substrate for:

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

Verify the connection preface:

```rust,no_run
use zerodds_http2::{CLIENT_PREFACE, check_preface};

assert_eq!(CLIENT_PREFACE.len(), 24);
let consumed = check_preface(CLIENT_PREFACE).expect("preface");
assert_eq!(consumed, 24);
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` for all error types. |
| `alloc` | ✅ (via std) | `Vec` / `String`. |

`no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1` is the initial release materialization. The public API,
the wire format (RFC 9113), and error discriminants are RC1-stable;
breaking changes require a major bump.

## Tests

```bash
cargo test -p zerodds-http2
```

45 unit tests + 1 doc test: frame codec (9, including round-trip + R-bit
stripping + buffer bound), flow control (10), error codes (3),
connection preface (5), settings (8), stream state transitions (10).

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`zerodds-hpack`](../hpack) — RFC 7541 HEADERS frame-body codec.
- [`zerodds-grpc-bridge`](../grpc-bridge) — gRPC consumer.
