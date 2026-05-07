# `zerodds-grpc-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-grpc-bridge/badge.svg)](https://docs.rs/zerodds-grpc-bridge)

gRPC-over-HTTP/2 + gRPC-Web Wire-Codec: Length-Prefixed-Message
(LPM), Path-Parsing (`/<service>/<method>`), `grpc-timeout`-Header,
`grpc-status`-Codes, Custom-Metadata-Encoding mit `-bin`-Suffix-
Konvention, gRPC-Web-Trailer-Frames, und ein Server-Skeleton fuer
Caller-konfigurierte HTTP/2-Listener. Sitzt auf
[`zerodds-http2`](../http2) (RFC 9113) +
[`zerodds-hpack`](../hpack) (RFC 7541). `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| gRPC HTTP/2 Protocol | Length-Prefixed-Message + `:path` + `grpc-timeout` + `grpc-status` + Custom-Metadata + `-bin`-Suffix |
| gRPC-Web Specification | Trailer-Frame (LPM mit Compressed-Flag-MSB=1) + Content-Types (`application/grpc-web` / `application/grpc-web+proto` / `application/grpc-web-text` / JSON) |

## Was ist drin

- **`encode_message` / `decode_message`** — gRPC LPM (Compressed-
  Flag 1 byte + Message-Length 4-byte BE + Bytes).
- **`parse_path`** — `/<service>/<method>` aus `:path`.
- **`encode_timeout` / `decode_timeout` / `TimeoutUnit`** —
  `grpc-timeout` mit Units H/M/S/m/u/n.
- **`Status`** — alle 17 gRPC Status-Codes (0 OK ... 16
  UNAUTHENTICATED).
- **`encode_header_value` / `decode_header_value` / `is_binary_header`
  / `BIN_SUFFIX` / `encode_base64` / `decode_base64`** — Custom-
  Metadata mit `-bin`-Suffix → Base64.
- **`request_headers` / `response_headers` / `content_types`** —
  Standard-Header-Sets fuer Request/Response/gRPC-Web/JSON.
- **`GrpcServer` / `GrpcRequest` / `GrpcResponse`** — Server-
  Skeleton zum Wiren mit Caller-konfigurierten HTTP/2-Listenern.

## Schichten-Position

Layer 5 — Bridges. Sitzt auf `zerodds-http2` (RFC 9113 Framing +
Stream-State + Flow-Control) und `zerodds-hpack` (RFC 7541 Header-
Compression). Konsumenten konfigurieren ihren eigenen TCP/TLS-
Listener und delegieren HTTP/2-Connection-Lifecycle an
`zerodds-http2`.

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

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error`-Impls. |
| `alloc` | ✅ (via std) | `Vec` / `String`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1`. Public-API + Wire-Format (gRPC HTTP/2 + gRPC-Web) +
Fehler-Diskriminanten sind RC1-stabil; Breaking-Changes erfordern
Major-Bump.

## Tests

```bash
cargo test -p zerodds-grpc-bridge
```

60 Tests grün (54 unit + 5 fuzz-smoke + 1 doc).

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/grpc-bridge.md`](../../docs/release/rc1-reviews/grpc-bridge.md) — RC1-Review.
- [`zerodds-http2`](../http2) — RFC 9113 HTTP/2-Framing-Substrat.
- [`zerodds-hpack`](../hpack) — RFC 7541 HPACK-Substrat.
