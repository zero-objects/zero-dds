# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-coap-bridge` crate.

### Spec References

- **RFC 7252** (CoAP): §3 (Message Format), §3.1 (Option Format with Delta Encoding + Extended Length), §4 (Reliability + Congestion Control), §5 (Request/Response incl. §5.3 Matching, §5.6 Caching, §5.7 Proxying, §5.8 Method Properties, §5.10 Option Number Registry), §6 (URI Scheme), §8 (Multicast), §9 (DTLS Mode), §12.1 (Code Registry).
- **RFC 7641** (Observe): Observe option, notification numbering, reordering detection.
- **RFC 7959** (Block-Wise Transfer): Block1/Block2 options, reassembly.
- **RFC 6690** (CoRE Link Format): Resource discovery via `link-format` string.

### Public API

**Message model (`message` module):**
- `CoapMessage { version, msg_type, token, code, message_id, options, payload }`.
- `MessageType::{Confirmable, NonConfirmable, Acknowledgement, Reset}`.
- `CoapCode` with all method codes (`Get`, `Post`, `Put`, `Delete`, `Fetch`, `Patch`, `iPatch`) + response codes (2.xx / 4.xx / 5.xx).

**Wire codec (`codec` module):**
- `encode(&CoapMessage) -> Result<Vec<u8>, CodecError>`.
- `decode(&[u8]) -> Result<CoapMessage, CodecError>`.
- `CodecError`.

**Options (`option` module):**
- `CoapOption { number, value }`.
- `OptionNumber` (Critical/Elective + Unsafe + NoCache-Key bits).
- `OptionValue::{Empty, Opaque(Vec<u8>), String(String), UInt(u64)}`.

**Block-Wise (`blockwise` module):**
- `BlockOption::{Block1, Block2}`.
- `BlockValue { num, m_bit, szx }`.
- `BlockReassembler` + `BlockError`.

**CoRE-Link (`core_link` module):**
- `CoreLink { target, attributes }`.
- `encode_links(&[CoreLink]) -> String` / `decode_links(&str) -> Result<Vec<CoreLink>, _>`.

**Observe (`observe` module):**
- `OBSERVE_OPTION_NUMBER = 6` (RFC 7641).
- `ObserverEntry`, `ObserveRegistry::{register, deregister, notify_iter, observers_for_resource}`.

**Reliability (`reliability` module):**
- `ACK_TIMEOUT_MS = 2000`, `MAX_RETRANSMIT = 4` (RFC 7252 §4.8).
- `PendingConfirmable`, `ReliabilityTracker::{schedule, on_ack, on_rst, tick}`.
- `TickOutput::{Retransmit, Timeout, Idle}`.

**Bridge (`bridge` module):**
- `CoapDdsBridge::{new, handle_request}`.
- `BridgeOp::{Read, Write, Dispose, Subscribe, Unsubscribe}`.
- `BridgeError`, `map_method(CoapCode) -> Option<DdsOp>`, `parse_dds_path(&str) -> Result<...>`.

**Method properties (`method_props` module):**
- `is_safe`, `is_idempotent` per RFC 7252 §5.8.

**Multicast (`multicast` module):**
- All-CoAP-Nodes (`224.0.1.187` / `[FF0X::FD]`).
- Multicast response suppression logic.

**Caching/Proxying (`caching_proxy` module):**
- ETag/Max-Age cache layer with RFC 7252 §5.6 freshness check.

**DTLS (`dtls` module):**
- DTLS mode marker (NoSec / PreSharedKey / RawPublicKey / Certificate).

**URI (`uri` module):**
- `coap://` and `coaps://` URI parser per RFC 7252 §6.

### Implementation

`encode`/`decode` operate on `Vec<u8>` and implement the §3.1 delta encoding of the options exactly: delta/length nibbles are 4-bit fields, values 13/14 signal 1-/2-byte extended length, value 15 is reserved. The payload marker `0xFF` separates header+options from the payload.

`ReliabilityTracker::tick` produces `Retransmit` outputs with exponential backoff (`ACK_TIMEOUT_MS * 2^retry_count`) up to `MAX_RETRANSMIT`, then `Timeout`. RFC 7252 §4.2 mandates this exact algorithm; deviating values are only allowed by reconfiguring the constants (tests lock the §4.8 default values).

`BlockReassembler` holds incoming Block2 responses in a `BTreeMap<u32 /* num */, Vec<u8>>` and returns `Some(reassembled_payload)` as soon as the last block (`m_bit == false`) arrives and all previous blocks are present contiguously.

`CoreLink` parses link-format strings per RFC 6690 §2 (backslash escape, quoted-string, multi-attributes).

`CoapDdsBridge` maps methods:
- `GET /<topic>/<key>` → `read-instance-by-key`.
- `PUT|POST /<topic>` (with payload as CDR) → `write` new instance.
- `DELETE /<topic>/<key>` → `dispose-instance`.
- `GET /<topic> + Observe=0` → subscriber registration.

`#![forbid(unsafe_code)]` is set. `extern crate alloc;`.

### Architecture

- **Layer:** 5 (Bridges).
- **Dependencies (in):** none (substrate crate). Only `core` + `alloc`.
- **Dependents (out):** (planned) DDS endpoint layer / constrained-device mapping.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format: fixed by RFC 7252 / 7641 / 7959 / 6690.
- Error discriminants: stable; new discriminants are major-additive.

### Added — Daemon Wireup

- Cross-cutting daemon runtime: the `daemon` feature enables
  Prometheus metrics (§8.2), Catalog/Healthz/Metrics admin endpoint
  (§5.2), signal watcher for graceful shutdown (§9.2), and
  OTLP span exporter (§8.3).
- Bridge security: auth-token option + topic ACL via
  `zerodds-bridge-security` (Bridge-Spec §7.2/§7.3). DTLS (§7.1)
  via separate ADR — the daemon reports a clear signal when
  `--tls-cert/--tls-key` is set without a DTLS acceptor.
- Block-Wise-Transfer module (`blockwise.rs`) +
  cross-vendor interop module.
- DDS-QoS → CoAP behavior translation in `qos_translation`.
