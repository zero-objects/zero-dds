# `zerodds-websocket-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-websocket-bridge/badge.svg)](https://docs.rs/zerodds-websocket-bridge)

WebSocket (RFC 6455) komplettes Stack-Set: Base-Framing-Protocol
(§5.2 + §5.3), Opening-Handshake (§4) mit `Sec-WebSocket-Accept`-
SHA1-Berechnung, Extension- + Subprotocol-Negotiation (§9), Close-
Frame-Status-Code-Semantik (§7.4), permessage-deflate Compression
(RFC 7692), URI-Parser (`ws://` / `wss://`), Streaming-UTF-8-
Validator (§8.1), und WebSocket↔DDS-Topic-Bridge. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| RFC 6455 (WebSocket) | §3 (URI), §4 (Opening Handshake), §5.2 (Base Framing Protocol), §5.3 (Client-to-Server Masking), §6.1 (Send Algorithm), §6.2 (Receive Algorithm), §7.4 (Status Codes), §8.1 (UTF-8 Handling), §9 (Extensions + Subprotocols) |
| RFC 7692 | permessage-deflate Compression Extension |

## Was ist drin

- **`Frame` / `Opcode`** + **`encode` / `decode`** — Wire-Codec
  (§5.2) inklusive Payload-Length-Encoding (7-bit / 7+16-bit / 7+64-
  bit, jeweils minimal).
- **`apply_mask` / `MaskingKeyProvider`** — XOR-Masking (§5.3); zwei
  Provider: `InsecureSplitmixProvider` fuer Tests/no_std-Builds,
  `ClosureMaskingKeyProvider` fuer Caller-CSPRNG.
- **`compute_accept` / `parse_client_request` /
  `build_server_response` / `render_server_response`** — Opening-
  Handshake (§4) mit `258EAFA5-E914-47DA-95CA-C5AB0DC85B11`-GUID
  + SHA-1 + Base64.
- **`parse_extensions` / `parse_subprotocols` / `select_subprotocol`**
  (`negotiation`-Modul) — §9 Extension- / Subprotocol-Negotiation.
- **`PermessageDeflateParams` / `parse_offer` / `render_accept` /
  `append_tail` / `strip_tail`** — RFC 7692 permessage-deflate.
- **`CloseCode` / `ClosePayload` / `validate_wire_status_code`** —
  §7.4 Status-Code-Semantik mit Forbidden-on-Wire-Pruefung
  (1004/1005/1006/1015 sind nicht-wire-zulaessig).
- **`StreamingValidator` / `validate_utf8`** — §8.1 Text-Frame-UTF-8-
  Validator (rejects Surrogates / Overlong-Encodings).
- **`WebSocketUri` / `parse_websocket_uri`** — `ws://` und `wss://`
  URI-Parser nach §3.
- **`SubscriptionRegistry` / `parse_op` / `render_notification`** —
  WebSocket↔DDS-Topic-Bridge (Subscribe/Unsubscribe ueber Text-
  Frames, Notifications als Text-Frame-JSON).

## Schichten-Position

Layer 5 — Bridges. Substrat fuer Browser↔DDS-Endpoint-Mapping (Web-
UIs, Realtime-Dashboards, DDS-Web-Gateway).

## Quickstart

```rust
use zerodds_websocket_bridge::compute_accept;

// RFC 6455 §1.3: Sec-WebSocket-Accept-Beispiel.
let accept = compute_accept("dGhlIHNhbXBsZSBub25jZQ==");
assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
```

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error`-Impls. |
| `alloc` | ✅ (via std) | `Vec` / `String`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1`. Public-API + Wire-Format (RFC 6455 / RFC 7692) +
Fehler-Diskriminanten sind RC1-stabil; Breaking-Changes erfordern
Major-Bump.

## Tests

```bash
cargo test -p zerodds-websocket-bridge
```

155 Tests grün (150 unit + 4 fuzz-smoke + 1 doc).

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/websocket-bridge.md`](../../docs/release/rc1-reviews/websocket-bridge.md) — RC1-Review.
