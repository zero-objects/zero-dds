# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-websocket-bridge`-Crate.

### Spec-Referenzen

- **RFC 6455** (WebSocket): §3 (URI), §4 (Opening-Handshake), §5.2 (Base-Framing-Protocol), §5.3 (Client-to-Server-Masking), §6 (Send/Receive-Algorithm), §7.4 (Status-Codes), §8.1 (UTF-8-Handling), §9 (Extensions + Subprotocols).
- **RFC 7692** (permessage-deflate): Compression-Extension fuer WebSocket.

### Public-API

**Frame/Codec (`frame` + `codec` + `masking`):**
- `Frame { fin, rsv1, rsv2, rsv3, opcode, mask, payload }`, `Opcode::{Continuation, Text, Binary, Close, Ping, Pong}`.
- `encode(&Frame, &mut Vec<u8>)`, `decode(&[u8]) -> Result<(Frame, usize), CodecError>`, `CodecError`.
- `apply_mask(payload, mask_key)`, `generate_masking_key`, `MaskingKeyProvider` (Trait), `InsecureSplitmixProvider`, `ClosureMaskingKeyProvider`.

**Handshake (`handshake`-Modul):**
- `WEBSOCKET_GUID`, `WEBSOCKET_VERSION`.
- `ClientHandshake`, `ServerHandshake`, `HandshakeError`.
- `compute_accept`, `parse_client_request`, `build_server_response`, `render_server_response`.

**Negotiation (`negotiation`-Modul):**
- `ExtensionOffer`, `parse_extensions`, `parse_subprotocols`, `select_subprotocol`.
- `SUBPROTOCOL_HEADER`, `EXTENSIONS_HEADER`.

**Close (`close`-Modul):**
- `CloseCode`, `ClosePayload`, `StatusCodeRange`.
- `encode_close_payload`, `decode_close_payload`.
- `classify_status_code`, `is_forbidden_on_wire`, `validate_wire_status_code`.

**permessage-deflate (`permessage_deflate`-Modul):**
- `PermessageDeflateParams { server_no_context_takeover, client_no_context_takeover, server_max_window_bits, client_max_window_bits }`.
- `parse_offer`, `render_accept`, `NegotiationError`.
- `append_tail`, `strip_tail`, `DEFLATE_TAIL` (`[0x00, 0x00, 0xFF, 0xFF]`).

**URI (`uri`-Modul):**
- `WebSocketUri`, `parse_websocket_uri`, `default_port`, `is_local_loopback`, `resource_name`, `UriError`.

**UTF-8 (`utf8`-Modul):**
- `StreamingValidator`, `validate as validate_utf8`, `Utf8Error`.

**DDS-Bridge (`dds_bridge`-Modul):**
- `BridgeOp::{Subscribe, Unsubscribe, Publish}`, `BridgeError`.
- `Notification`, `SubscriptionRegistry`.
- `parse_op`, `render_notification`.

**Message (`message`-Modul):**
- §6.1 / §6.2 Send- / Receive-Algorithmus mit Fragmentation-Reassembly.

### Implementierung

`encode`/`decode` macht das WebSocket-Wire-Format §5.2 exakt, inklusive der Spec-Anforderung "Payload-Length MUST be encoded in the minimum number of bytes" (Decoder rejected non-minimal Encoding wenn z.B. ein 16-bit-Length verwendet wird obwohl 7-bit reichen wuerde).

`compute_accept` realisiert §4.2.2-Schritt-5: SHA1(`client_key` + WEBSOCKET_GUID), Base64-encoded. WebSocket-GUID `258EAFA5-E914-47DA-95CA-C5AB0DC85B11` ist als Konstante exposed.

`StreamingValidator` ist ein incremental UTF-8-Validator, der byte-fuer-byte fuettern kann (fuer Text-Frames wo Payload ueber mehrere Frames fragmentiert sein kann); rejected Surrogates (D800-DFFF), Overlong-Encodings, und Code-Points > 0x10FFFF.

`PermessageDeflateParams` parst und rendert nur die Negotiation-Header — die eigentliche Compression delegiert der Caller an `flate2` oder eine andere zlib-Bridge (no_std-kompatibel; permessage-deflate selbst ist in der Crate Spec-konform negotiated).

`#![forbid(unsafe_code)]` ist gesetzt. `extern crate alloc;`.

### Architektur

- **Layer:** 5 (Bridges).
- **Dependencies (in):** keine (Substrat-Crate). Nur `core` + `alloc`.
- **Dependents (out):** (vorgesehen) DDS-Web-Gateway / Browser-Endpoint-Layer.
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format: durch RFC 6455 / RFC 7692 fixiert.
- Fehler-Diskriminanten: stabil; neue Diskriminanten sind Major-additive.

### Added — Daemon-Wireup

- Cross-Cutting Daemon-Runtime: `daemon`-Feature aktiviert
  Prometheus-Metrics (§8.2), Catalog/Healthz/Metrics-Admin-Endpoint
  (§5.2), Signal-Watcher fuer Graceful-Shutdown (§9.2), und
  OTLP-Span-Exporter (§8.3).
- Bridge-Security: TLS-Acceptor (rustls 0.23 ServerConnection-Wrapping
  von TcpStream) + Auth-Modes + Topic-ACL via `zerodds-bridge-security`
  Substrat (Bridge-Spec §7.1/§7.2/§7.3); SIGHUP-Hook fuer
  TLS-Cert-Hot-Reload via `RotatingTlsConfig`.
- Cross-Vendor-Interop: `cross_vendor.rs`-Modul fuer Konformitaets-
  Tests gegen externe WebSocket-Implementationen.
- DDS-QoS → WebSocket-Behavior-Translation in `qos_translation`
  (Bridge-Spec §6).
