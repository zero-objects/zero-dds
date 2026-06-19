# `zerodds-grpc-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-grpc-bridge/badge.svg)](https://docs.rs/zerodds-grpc-bridge)

gRPC-over-HTTP/2 + gRPC-Web wire codec: length-prefixed message
(LPM), path parsing (`/<service>/<method>`), `grpc-timeout` header,
`grpc-status` codes, custom-metadata encoding with the `-bin`-suffix
convention, gRPC-Web trailer frames, and a server skeleton for
caller-configured HTTP/2 listeners. Sits on
[`zerodds-http2`](../http2) (RFC 9113) +
[`zerodds-hpack`](../hpack) (RFC 7541). `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| gRPC HTTP/2 Protocol | Length-prefixed message + `:path` + `grpc-timeout` + `grpc-status` + custom metadata + `-bin` suffix |
| gRPC-Web Specification | Trailer frame (LPM with compressed-flag MSB=1) + content types (`application/grpc-web` / `application/grpc-web+proto` / `application/grpc-web-text` / JSON) |

## What's inside

- **`encode_message` / `decode_message`** — gRPC LPM (compressed
  flag 1 byte + message length 4-byte BE + bytes).
- **`parse_path`** — `/<service>/<method>` from `:path`.
- **`encode_timeout` / `decode_timeout` / `TimeoutUnit`** —
  `grpc-timeout` with units H/M/S/m/u/n.
- **`Status`** — all 17 gRPC status codes (0 OK ... 16
  UNAUTHENTICATED).
- **`encode_header_value` / `decode_header_value` / `is_binary_header`
  / `BIN_SUFFIX` / `encode_base64` / `decode_base64`** — custom
  metadata with `-bin` suffix → Base64.
- **`request_headers` / `response_headers` / `content_types`** —
  standard header sets for request/response/gRPC-Web/JSON.
- **`GrpcServer` / `GrpcRequest` / `GrpcResponse`** — server
  skeleton to wire with caller-configured HTTP/2 listeners.

## Layer position

Layer 5 — Bridges. Sits on `zerodds-http2` (RFC 9113 framing +
stream state + flow control) and `zerodds-hpack` (RFC 7541 header
compression). Consumers configure their own TCP/TLS listener and
delegate the HTTP/2 connection lifecycle to `zerodds-http2`.

## Quickstart

```rust
use zerodds_grpc_bridge::{decode_message, encode_message};

let msg = b"hello-grpc";
let wire = encode_message(msg, false).expect("encode");
let (flag, payload, consumed) = decode_message(&wire).expect("decode");
assert_eq!(flag, 0);
assert_eq!(payload, msg);
assert_eq!(consumed, wire.len());
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` impls. |
| `alloc` | ✅ (via std) | `Vec` / `String`. |

`no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1`. Public API + wire format (gRPC HTTP/2 + gRPC-Web) +
error discriminants are RC1-stable; breaking changes require a
major bump.

## Tests

```bash
cargo test -p zerodds-grpc-bridge
```

60 tests passing (54 unit + 5 fuzz-smoke + 1 doc).

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/release/rc1-reviews/grpc-bridge.md`](../../docs/release/rc1-reviews/grpc-bridge.md) — RC1 review.
- [`zerodds-http2`](../http2) — RFC 9113 HTTP/2 framing substrate.
- [`zerodds-hpack`](../hpack) — RFC 7541 HPACK substrate.
