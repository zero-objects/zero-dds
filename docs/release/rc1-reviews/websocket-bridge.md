# RC1 Review — `zerodds-websocket-bridge`

> **Layer:** 5 (Bridges) | **Reviewer:** claude | **Public-Strategy:** 🌐 public

## 1 Purpose

WebSocket (RFC 6455) komplettes Stack-Set: Wire + Handshake + Negotiation + Close + permessage-deflate (RFC 7692) + URI + UTF-8-Validator + DDS-Bridge. `no_std + alloc`.

## 2-3 Inhalt

- 12 src-Files (close, codec, dds_bridge, frame, handshake, lib, masking, message, negotiation, permessage_deflate, uri, utf8).
- 1 tests-File (fuzz_smoke.rs).
- **155 Tests gruen** (150 unit + 4 fuzz-smoke + 1 doc).

## 3.4 Coherence-Audit

| Item-Familie | Spec | Klassifikation |
|---|---|---|
| Wire-Codec (`Frame` / `Opcode` / `encode` / `decode`) | RFC 6455 §5.2 | OPTIONAL-HOOK (Substrat) |
| Masking (`apply_mask` / `MaskingKeyProvider`) | §5.3 | OPTIONAL-HOOK |
| Handshake (`compute_accept` / `parse_client_request` / ...) | §4 | OPTIONAL-HOOK |
| Negotiation (`parse_extensions` / `select_subprotocol`) | §9 | OPTIONAL-HOOK |
| Close (`CloseCode` / `validate_wire_status_code`) | §7.4 | OPTIONAL-HOOK |
| permessage-deflate (`PermessageDeflateParams` / `parse_offer`) | RFC 7692 | OPTIONAL-HOOK |
| URI (`WebSocketUri` / `parse_websocket_uri`) | §3 | OPTIONAL-HOOK |
| UTF-8 (`StreamingValidator` / `validate_utf8`) | §8.1 | OPTIONAL-HOOK |
| DDS-Bridge (`SubscriptionRegistry` / `parse_op`) | DDS-Mapping | OPTIONAL-HOOK |

Alle 9 Item-Familien als OPTIONAL-HOOK explizit dokumentiert (Substrat-Crate fuer Caller-konstruierte Browser-/Web-Gateway-Endpoints). 0 ❌.

## 6 Cleanup

- **Forbidden:** 0.
- **Sprint-Marker:** 0 (Crate war pre-Review pristine).
- **TODO/FIXME/Stub:** 0.
- **lib.rs**: Header korrigiert — vorherige "Was nicht abgedeckt: Opening Handshake / Extension-Negotiation / Close-Frame Status-Code-Semantik" Behauptung war stale (alle drei SIND vollstaendig implementiert: `handshake.rs` + `negotiation.rs` + `close.rs`).

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: vollstaendiger RC1-Header + Quickstart-Doc-Test.
3. SPDX-License-Header auf alle 12 src-Files.
4. README + CHANGELOG.
5. Mirror unter `github/crates/websocket-bridge/`.
6. `website/docs/websocket-bridge.md`.
7. Tracker: 5.8 websocket-bridge → ✅.

## 10-12 Gates

- `cargo test`: ✅ 155 tests.
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅.

## 13 Daemon-Wireup-Append

Folgende Items sind nach dem ersten Sign-off in den `daemon`-Feature-
Pfad eingebracht worden (kein Major-Bump, alles innerhalb 1.0.0-rc.1):

- `daemon/runtime_common.rs` + `daemon/qos_translation.rs` +
  `daemon/server.rs` + `daemon/config.rs` + Daemon-Wireup im
  `zerodds-ws-bridged`-Binary.
- TLS-Acceptor (rustls 0.23) + Auth-Modes + ACL via
  `zerodds-bridge-security` voll wired; SIGHUP-Hook fuer
  Cert-Hot-Reload via `RotatingTlsConfig`.
- `cross_vendor.rs`-Modul fuer Konformitaets-Tests gegen externe
  WebSocket-Implementationen.
- E2E-Tests in `tests/daemon_e2e.rs`: graceful_shutdown +
  admin_endpoint + metrics_counters.
- Tests gruen: 150 unit + 4 fuzz-smoke + 4 daemon_e2e + 1 doc.

**Crate-Version:** `1.0.0-rc.1` | **Sign-off:** claude
