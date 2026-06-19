# `zerodds-websocket-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-websocket-bridge/badge.svg)](https://docs.rs/zerodds-websocket-bridge)

WebSocket (RFC 6455) complete stack set: base framing protocol
(§5.2 + §5.3), opening handshake (§4) with `Sec-WebSocket-Accept`
SHA1 computation, extension + subprotocol negotiation (§9), close-
frame status-code semantics (§7.4), permessage-deflate compression
(RFC 7692), URI parser (`ws://` / `wss://`), streaming UTF-8
validator (§8.1), and a WebSocket↔DDS topic bridge. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| RFC 6455 (WebSocket) | §3 (URI), §4 (Opening Handshake), §5.2 (Base Framing Protocol), §5.3 (Client-to-Server Masking), §6.1 (Send Algorithm), §6.2 (Receive Algorithm), §7.4 (Status Codes), §8.1 (UTF-8 Handling), §9 (Extensions + Subprotocols) |
| RFC 7692 | permessage-deflate Compression Extension |

## What's inside

- **`Frame` / `Opcode`** + **`encode` / `decode`** — wire codec
  (§5.2) including payload length encoding (7-bit / 7+16-bit / 7+64-
  bit, each minimal).
- **`apply_mask` / `MaskingKeyProvider`** — XOR masking (§5.3); two
  providers: `InsecureSplitmixProvider` for tests/no_std builds,
  `ClosureMaskingKeyProvider` for a caller CSPRNG.
- **`compute_accept` / `parse_client_request` /
  `build_server_response` / `render_server_response`** — opening
  handshake (§4) with the `258EAFA5-E914-47DA-95CA-C5AB0DC85B11` GUID
  + SHA-1 + Base64.
- **`parse_extensions` / `parse_subprotocols` / `select_subprotocol`**
  (`negotiation` module) — §9 extension / subprotocol negotiation.
- **`PermessageDeflateParams` / `parse_offer` / `render_accept` /
  `append_tail` / `strip_tail`** — RFC 7692 permessage-deflate.
- **`CloseCode` / `ClosePayload` / `validate_wire_status_code`** —
  §7.4 status-code semantics with forbidden-on-wire checking
  (1004/1005/1006/1015 are not wire-permissible).
- **`StreamingValidator` / `validate_utf8`** — §8.1 text-frame UTF-8
  validator (rejects surrogates / overlong encodings).
- **`WebSocketUri` / `parse_websocket_uri`** — `ws://` and `wss://`
  URI parser per §3.
- **`SubscriptionRegistry` / `parse_op` / `render_notification`** —
  WebSocket↔DDS topic bridge (subscribe/unsubscribe via text
  frames, notifications as text-frame JSON).

## Layer position

Layer 5 — bridges. Substrate for browser↔DDS endpoint mapping (web
UIs, realtime dashboards, DDS web gateway).

## Quickstart

```rust
use zerodds_websocket_bridge::compute_accept;

// RFC 6455 §1.3: Sec-WebSocket-Accept example.
let accept = compute_accept("dGhlIHNhbXBsZSBub25jZQ==");
assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
```

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` impls. |
| `alloc` | ✅ (via std) | `Vec` / `String`. |

`no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1`. Public API + wire format (RFC 6455 / RFC 7692) +
error discriminants are RC1-stable; breaking changes require a
major bump.

## Tests

```bash
cargo test -p zerodds-websocket-bridge
```

155 tests green (150 unit + 4 fuzz-smoke + 1 doc).

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/release/rc1-reviews/websocket-bridge.md`](../../docs/release/rc1-reviews/websocket-bridge.md) — RC1 review.
