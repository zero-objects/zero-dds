# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-coap-bridge`-Crate.

### Spec-Referenzen

- **RFC 7252** (CoAP): §3 (Message-Format), §3.1 (Option-Format mit Delta-Encoding + Extended-Length), §4 (Reliability + Congestion-Control), §5 (Request/Response inkl. §5.3 Matching, §5.6 Caching, §5.7 Proxying, §5.8 Method-Properties, §5.10 Option-Number-Registry), §6 (URI-Scheme), §8 (Multicast), §9 (DTLS-Mode), §12.1 (Code-Registry).
- **RFC 7641** (Observe): Observe-Option, Notification-Numbering, Reordering-Detection.
- **RFC 7959** (Block-Wise Transfer): Block1/Block2-Optionen, Reassembly.
- **RFC 6690** (CoRE-Link-Format): Resource-Discovery via `link-format`-String.

### Public-API

**Message-Modell (`message`-Modul):**
- `CoapMessage { version, msg_type, token, code, message_id, options, payload }`.
- `MessageType::{Confirmable, NonConfirmable, Acknowledgement, Reset}`.
- `CoapCode` mit allen Method-Codes (`Get`, `Post`, `Put`, `Delete`, `Fetch`, `Patch`, `iPatch`) + Response-Codes (2.xx / 4.xx / 5.xx).

**Wire-Codec (`codec`-Modul):**
- `encode(&CoapMessage) -> Result<Vec<u8>, CodecError>`.
- `decode(&[u8]) -> Result<CoapMessage, CodecError>`.
- `CodecError`.

**Options (`option`-Modul):**
- `CoapOption { number, value }`.
- `OptionNumber` (Critical/Elective + Unsafe + NoCache-Key-Bits).
- `OptionValue::{Empty, Opaque(Vec<u8>), String(String), UInt(u64)}`.

**Block-Wise (`blockwise`-Modul):**
- `BlockOption::{Block1, Block2}`.
- `BlockValue { num, m_bit, szx }`.
- `BlockReassembler` + `BlockError`.

**CoRE-Link (`core_link`-Modul):**
- `CoreLink { target, attributes }`.
- `encode_links(&[CoreLink]) -> String` / `decode_links(&str) -> Result<Vec<CoreLink>, _>`.

**Observe (`observe`-Modul):**
- `OBSERVE_OPTION_NUMBER = 6` (RFC 7641).
- `ObserverEntry`, `ObserveRegistry::{register, deregister, notify_iter, observers_for_resource}`.

**Reliability (`reliability`-Modul):**
- `ACK_TIMEOUT_MS = 2000`, `MAX_RETRANSMIT = 4` (RFC 7252 §4.8).
- `PendingConfirmable`, `ReliabilityTracker::{schedule, on_ack, on_rst, tick}`.
- `TickOutput::{Retransmit, Timeout, Idle}`.

**Bridge (`bridge`-Modul):**
- `CoapDdsBridge::{new, handle_request}`.
- `BridgeOp::{Read, Write, Dispose, Subscribe, Unsubscribe}`.
- `BridgeError`, `map_method(CoapCode) -> Option<DdsOp>`, `parse_dds_path(&str) -> Result<...>`.

**Method-Properties (`method_props`-Modul):**
- `is_safe`, `is_idempotent` per RFC 7252 §5.8.

**Multicast (`multicast`-Modul):**
- All-CoAP-Nodes (`224.0.1.187` / `[FF0X::FD]`).
- Multicast-Response-Suppression-Logic.

**Caching/Proxying (`caching_proxy`-Modul):**
- ETag/Max-Age-Cache-Layer mit RFC 7252 §5.6 Freshness-Pruefung.

**DTLS (`dtls`-Modul):**
- DTLS-Mode-Marker (NoSec / PreSharedKey / RawPublicKey / Certificate).

**URI (`uri`-Modul):**
- `coap://` und `coaps://` URI-Parser per RFC 7252 §6.

### Implementierung

`encode`/`decode` arbeiten auf `Vec<u8>` und implementieren das §3.1-Delta-Encoding der Options exakt: Delta-/Length-Nibbles sind 4-Bit-Felder, Werte 13/14 signalisieren 1-/2-Byte-Extended-Length, Wert 15 ist reserviert. Payload-Marker `0xFF` separiert Header+Options vom Payload.

`ReliabilityTracker::tick` produziert `Retransmit`-Outputs mit exponentiellem Backoff (`ACK_TIMEOUT_MS * 2^retry_count`) bis `MAX_RETRANSMIT`, danach `Timeout`. RFC 7252 §4.2 verlangt diesen Algorithmus exakt; abweichende Werte sind nur via Re-Konfiguration der Konstanten erlaubt (Tests verriegeln die §4.8-Default-Werte).

`BlockReassembler` haelt eingehende Block2-Antworten in einem `BTreeMap<u32 /* num */, Vec<u8>>` und liefert `Some(reassembled_payload)`, sobald das letzte Block (`m_bit == false`) eintrifft und alle vorigen Blocks zusammenhaengend vorhanden sind.

`CoreLink` parsed link-format-Strings nach RFC 6690 §2 (Backslash-Escape, quoted-string, Multi-Attributes).

`CoapDdsBridge` mappt Methoden:
- `GET /<topic>/<key>` → `read-instance-by-key`.
- `PUT|POST /<topic>` (mit Payload als CDR) → `write` neue Instance.
- `DELETE /<topic>/<key>` → `dispose-instance`.
- `GET /<topic> + Observe=0` → Subscriber-Registrierung.

`#![forbid(unsafe_code)]` ist gesetzt. `extern crate alloc;`.

### Architektur

- **Layer:** 5 (Bridges).
- **Dependencies (in):** keine (Substrat-Crate). Nur `core` + `alloc`.
- **Dependents (out):** (vorgesehen) DDS-Endpoint-Layer / Constrained-Device-Mapping.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format: durch RFC 7252 / 7641 / 7959 / 6690 fixiert.
- Fehler-Diskriminanten: stabil; neue Diskriminanten sind Major-additive.

### Added — Daemon-Wireup

- Cross-Cutting Daemon-Runtime: `daemon`-Feature aktiviert
  Prometheus-Metrics (§8.2), Catalog/Healthz/Metrics-Admin-Endpoint
  (§5.2), Signal-Watcher fuer Graceful-Shutdown (§9.2), und
  OTLP-Span-Exporter (§8.3).
- Bridge-Security: Auth-Token-Option + Topic-ACL via
  `zerodds-bridge-security` (Bridge-Spec §7.2/§7.3). DTLS (§7.1)
  via separates ADR — der Daemon meldet ein klares Signal beim
  Setzen von `--tls-cert/--tls-key` ohne DTLS-Acceptor.
- Block-Wise-Transfer-Modul (`blockwise.rs`) +
  Cross-Vendor-Interop-Modul.
- DDS-QoS → CoAP-Behavior-Translation in `qos_translation`.
