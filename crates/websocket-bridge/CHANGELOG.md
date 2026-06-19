# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.2] — 2026-05-15

Hotfix for two bugs that made `zerodds-ws-bridged` unusable in
production-like deployments. ZeroCollab Wave 2b had been deferred to
the workaround "YAML instead of CLI" — with rc.2 both paths work.

### Fixed

- **CLI merge:** `bin/zerodds-ws-bridged.rs::run()` previously merged only
  `--listen`, `--domain` and `--log-level` from the parsed `CliArgs`
  into the `DaemonConfig`. `--topic`, `--auth-token`, `--tls-cert`,
  `--tls-key` and `--metrics` stayed no-op. Spec §2 says "the CLI
  overrides file values" — which only held partially. Fix: `apply_cli_overrides`
  extracted as a testable function; `--topic` additive with
  `default_ws_path`/`type_name=name`/`direction="bidir"` (Spec §5);
  `--auth-token` implies `auth_mode="bearer"`; `--tls-cert`/`--tls-key`
  implies `tls_enabled=true`; `--metrics` implies
  `metrics_enabled=true`. Eight unit tests in the bin crate. (GitHub #1, PR #5.)

- **yaml.example did not match the parser schema:** The shipped
  `packaging/linux/configs/ws-bridged.yaml.example` had
  nested top-level keys (`participant:`, `websocket:`,
  `routes:`, `observability:`). The parser expects flat keys
  (`listen/domain/log_level/tls/auth/acl/metrics/topics` per Spec §3)
  and ignored unknown keys silently → the bridge booted
  with defaults. Example rewritten, spec §-ref corrected from §4 (wire) to
  §3 (config file format). New integration test
  `tests/example_yaml_loadback.rs` includes the shipped
  example yaml via `include_str!` and runs it through
  `DaemonConfig::load_from_str` — drift surfaces immediately as a
  test failure instead of only in the field. (GitHub #3, PR #5.)

### Added

- **Parser WARN for unknown top-level keys:**
  `DaemonConfig::load_from_str` no longer ignores unknown keys
  silently, but emits a WARN line on stderr with a
  hint about the expected keys and spec-ref §3. Forward compatibility
  is kept (no `ConfigError`). A unit test freezes the semantics.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-websocket-bridge` crate.

### Spec references

- **RFC 6455** (WebSocket): §3 (URI), §4 (opening handshake), §5.2 (base framing protocol), §5.3 (client-to-server masking), §6 (send/receive algorithm), §7.4 (status codes), §8.1 (UTF-8 handling), §9 (extensions + subprotocols).
- **RFC 7692** (permessage-deflate): compression extension for WebSocket.

### Public-API

**Frame/Codec (`frame` + `codec` + `masking`):**
- `Frame { fin, rsv1, rsv2, rsv3, opcode, mask, payload }`, `Opcode::{Continuation, Text, Binary, Close, Ping, Pong}`.
- `encode(&Frame, &mut Vec<u8>)`, `decode(&[u8]) -> Result<(Frame, usize), CodecError>`, `CodecError`.
- `apply_mask(payload, mask_key)`, `generate_masking_key`, `MaskingKeyProvider` (Trait), `InsecureSplitmixProvider`, `ClosureMaskingKeyProvider`.

**Handshake (`handshake` module):**
- `WEBSOCKET_GUID`, `WEBSOCKET_VERSION`.
- `ClientHandshake`, `ServerHandshake`, `HandshakeError`.
- `compute_accept`, `parse_client_request`, `build_server_response`, `render_server_response`.

**Negotiation (`negotiation` module):**
- `ExtensionOffer`, `parse_extensions`, `parse_subprotocols`, `select_subprotocol`.
- `SUBPROTOCOL_HEADER`, `EXTENSIONS_HEADER`.

**Close (`close` module):**
- `CloseCode`, `ClosePayload`, `StatusCodeRange`.
- `encode_close_payload`, `decode_close_payload`.
- `classify_status_code`, `is_forbidden_on_wire`, `validate_wire_status_code`.

**permessage-deflate (`permessage_deflate` module):**
- `PermessageDeflateParams { server_no_context_takeover, client_no_context_takeover, server_max_window_bits, client_max_window_bits }`.
- `parse_offer`, `render_accept`, `NegotiationError`.
- `append_tail`, `strip_tail`, `DEFLATE_TAIL` (`[0x00, 0x00, 0xFF, 0xFF]`).

**URI (`uri` module):**
- `WebSocketUri`, `parse_websocket_uri`, `default_port`, `is_local_loopback`, `resource_name`, `UriError`.

**UTF-8 (`utf8` module):**
- `StreamingValidator`, `validate as validate_utf8`, `Utf8Error`.

**DDS bridge (`dds_bridge` module):**
- `BridgeOp::{Subscribe, Unsubscribe, Publish}`, `BridgeError`.
- `Notification`, `SubscriptionRegistry`.
- `parse_op`, `render_notification`.

**Message (`message` module):**
- §6.1 / §6.2 send / receive algorithm with fragmentation reassembly.

### Implementation

`encode`/`decode` implements the WebSocket wire format §5.2 exactly, including the spec requirement "Payload-Length MUST be encoded in the minimum number of bytes" (the decoder rejects non-minimal encoding when, e.g., a 16-bit length is used even though 7-bit would suffice).

`compute_accept` realizes §4.2.2 step 5: SHA1(`client_key` + WEBSOCKET_GUID), Base64-encoded. The WebSocket GUID `258EAFA5-E914-47DA-95CA-C5AB0DC85B11` is exposed as a constant.

`StreamingValidator` is an incremental UTF-8 validator that can be fed byte-by-byte (for text frames where the payload may be fragmented across multiple frames); it rejects surrogates (D800-DFFF), overlong encodings, and code points > 0x10FFFF.

`PermessageDeflateParams` only parses and renders the negotiation headers — the actual compression is delegated by the caller to `flate2` or another zlib bridge (no_std-compatible; permessage-deflate itself is negotiated spec-conformantly in the crate).

`#![forbid(unsafe_code)]` is set. `extern crate alloc;`.

### Architecture

- **Layer:** 5 (bridges).
- **Dependencies (in):** none (substrate crate). Only `core` + `alloc`.
- **Dependents (out):** (planned) DDS web gateway / browser endpoint layer.
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format: fixed by RFC 6455 / RFC 7692.
- Error discriminants: stable; new discriminants are major-additive.

### Added — daemon wireup

- Cross-cutting daemon runtime: the `daemon` feature enables
  Prometheus metrics (§8.2), the catalog/healthz/metrics admin endpoint
  (§5.2), a signal watcher for graceful shutdown (§9.2), and the
  OTLP span exporter (§8.3).
- Bridge security: TLS acceptor (rustls 0.23 ServerConnection wrapping
  of TcpStream) + auth modes + topic ACL via the `zerodds-bridge-security`
  substrate (bridge spec §7.1/§7.2/§7.3); SIGHUP hook for
  TLS cert hot-reload via `RotatingTlsConfig`.
- Cross-vendor interop: `cross_vendor.rs` module for conformance
  tests against external WebSocket implementations.
- DDS QoS → WebSocket behavior translation in `qos_translation`
  (bridge spec §6).
