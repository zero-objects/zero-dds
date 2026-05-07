# `zerodds-coap-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-coap-bridge/badge.svg)](https://docs.rs/zerodds-coap-bridge)

CoAP (Constrained Application Protocol, RFC 7252) komplettes Stack-
Set: Wire-Codec, Options + Delta-Encoding, Reliability mit Retransmit-
Tracker (§4), Request/Response-Matching (§5.3), Block-Wise-Transfer
(RFC 7959), Resource-Discovery im CoRE-Link-Format (RFC 6690),
Observer-Pattern (RFC 7641), Multicast-Operation (§8), Caching +
Proxying (§5.6 + §5.7), DTLS-Mode-Marker (§9), und einen
Bidirectional-CoAP↔DDS-Topic-Bridge. `no_std + alloc`,
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| RFC 7252 (CoAP) | §3 (Message-Format), §3.1 (Option-Format), §4 (Reliability), §5 (Request/Response), §5.6 (Caching), §5.7 (Proxying), §5.8 (Method-Properties), §5.10 (Option-Number-Registry), §6 (URI-Scheme), §8 (Multicast), §9 (DTLS), §12.1 (Code-Registry) |
| RFC 7641 | Observing Resources |
| RFC 7959 | Block-Wise Transfer |
| RFC 6690 | CoRE-Link-Format (Resource-Discovery) |

## Was ist drin

- **`CoapMessage` / `CoapCode` / `MessageType`** — Message-Modell (§3 +
  §12.1).
- **`encode` / `decode`** — Wire-Codec inklusive Options-Delta-
  Encoding und Payload-Marker `0xFF`.
- **`CoapOption` / `OptionNumber` / `OptionValue`** — Options mit
  allen registrierten Numbers (Critical/Elective/Unsafe/NoCache-Key-
  Bits per §5.10).
- **`BlockOption` / `BlockValue` / `BlockReassembler`** — Block-Wise-
  Transfer (RFC 7959) mit Block1/Block2-Optionen.
- **`CoreLink` / `encode_links` / `decode_links`** — Resource-
  Discovery via `link-format`-String (RFC 6690).
- **`OBSERVE_OPTION_NUMBER` / `ObserveRegistry` / `ObserverEntry`** —
  RFC-7641 Observer-State + Notification-Numbering.
- **`PendingConfirmable` / `ReliabilityTracker` / `TickOutput` /
  `ACK_TIMEOUT_MS` / `MAX_RETRANSMIT`** — §4 Retransmit-Logic.
- **`CoapDdsBridge` / `BridgeOp` / `BridgeError` / `map_method` /
  `parse_dds_path`** — CoAP↔DDS-Topic-Mapping (GET → read-by-key,
  PUT/POST → write, DELETE → dispose, Observe → Subscriber).

## Schichten-Position

Layer 5 — Bridges. Substrat fuer DDS↔IoT-Endpoint-Mapping (constrained
Devices, Multicast-Diskovery, Observer-Pattern fuer DDS-Live-Updates).

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

## Feature-Flags

| Feature | Default | Zweck |
|---------|---------|-------|
| `std` | ✅ | `std::error::Error`-Impls. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `BTreeMap`. |

`no_std`-fahig: `default-features = false, features = ["alloc"]`.

## Stabilitaet

`1.0.0-rc.1` ist die initiale Release-Materialisierung. Public-API,
Wire-Format (RFC 7252) und Fehler-Diskriminanten sind RC1-stabil;
Breaking-Changes erfordern Major-Bump.

## Tests

```bash
cargo test -p zerodds-coap-bridge
```

145 Tests grün:
- 141 Unit-Tests (Codec, Options, Reliability, Block-Wise, Observe,
  CoRE-Link, Matching, Multicast, Method-Properties, URI, DTLS-Mode,
  Caching/Proxying, Bridge).
- 3 Fuzz-Smoke-Tests (Pseudo-Random-Bytes-Stream, kein Panic).
- 1 Doc-Test.

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/release/rc1-reviews/coap-bridge.md`](../../docs/release/rc1-reviews/coap-bridge.md) — RC1-Review.
