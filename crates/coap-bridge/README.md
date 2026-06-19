# `zerodds-coap-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-coap-bridge/badge.svg)](https://docs.rs/zerodds-coap-bridge)

CoAP (Constrained Application Protocol, RFC 7252) complete stack
set: wire codec, options + delta encoding, reliability with retransmit
tracker (§4), request/response matching (§5.3), block-wise transfer
(RFC 7959), resource discovery in CoRE-Link format (RFC 6690),
observer pattern (RFC 7641), multicast operation (§8), caching +
proxying (§5.6 + §5.7), DTLS mode marker (§9), and a
bidirectional CoAP↔DDS topic bridge. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec Mapping

| Spec | Section |
|------|-----------|
| RFC 7252 (CoAP) | §3 (Message-Format), §3.1 (Option-Format), §4 (Reliability), §5 (Request/Response), §5.6 (Caching), §5.7 (Proxying), §5.8 (Method-Properties), §5.10 (Option-Number-Registry), §6 (URI-Scheme), §8 (Multicast), §9 (DTLS), §12.1 (Code-Registry) |
| RFC 7641 | Observing Resources |
| RFC 7959 | Block-Wise Transfer |
| RFC 6690 | CoRE-Link-Format (Resource-Discovery) |

## What's Inside

- **`CoapMessage` / `CoapCode` / `MessageType`** — message model (§3 +
  §12.1).
- **`encode` / `decode`** — wire codec including options delta
  encoding and payload marker `0xFF`.
- **`CoapOption` / `OptionNumber` / `OptionValue`** — options with
  all registered numbers (Critical/Elective/Unsafe/NoCache-Key
  bits per §5.10).
- **`BlockOption` / `BlockValue` / `BlockReassembler`** — block-wise
  transfer (RFC 7959) with Block1/Block2 options.
- **`CoreLink` / `encode_links` / `decode_links`** — resource
  discovery via `link-format` string (RFC 6690).
- **`OBSERVE_OPTION_NUMBER` / `ObserveRegistry` / `ObserverEntry`** —
  RFC 7641 observer state + notification numbering.
- **`PendingConfirmable` / `ReliabilityTracker` / `TickOutput` /
  `ACK_TIMEOUT_MS` / `MAX_RETRANSMIT`** — §4 retransmit logic.
- **`CoapDdsBridge` / `BridgeOp` / `BridgeError` / `map_method` /
  `parse_dds_path`** — CoAP↔DDS topic mapping (GET → read-by-key,
  PUT/POST → write, DELETE → dispose, Observe → subscriber).

## Layer Position

Layer 5 — Bridges. Substrate for DDS↔IoT endpoint mapping (constrained
devices, multicast discovery, observer pattern for DDS live updates).

## Quickstart

```rust
use zerodds_coap_bridge::{decode, encode, CoapCode, CoapMessage, MessageType};

let msg = CoapMessage {
    version: 1,
    message_type: MessageType::Confirmable,
    token: Vec::new(),
    code: CoapCode::GET,
    message_id: 0xBEEF,
    options: Vec::new(),
    payload: Vec::new(),
};

let wire = encode(&msg).expect("encode");
let decoded = decode(&wire).expect("decode");
assert_eq!(decoded.code, CoapCode::GET);
```

## Feature Flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error` impls. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `BTreeMap`. |

`no_std`-capable: `default-features = false, features = ["alloc"]`.

## Stability

`1.0.0-rc.1` is the initial release materialization. The public API,
wire format (RFC 7252), and error discriminants are RC1-stable;
breaking changes require a major bump.

## Tests

```bash
cargo test -p zerodds-coap-bridge
```

145 tests passing:
- 141 unit tests (codec, options, reliability, block-wise, observe,
  CoRE-Link, matching, multicast, method properties, URI, DTLS mode,
  caching/proxying, bridge).
- 3 fuzz smoke tests (pseudo-random byte stream, no panic).
- 1 doc test.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See Also

- [`docs/release/rc1-reviews/coap-bridge.md`](../../docs/release/rc1-reviews/coap-bridge.md) — RC1 review.
