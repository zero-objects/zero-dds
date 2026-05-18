# RC1 Review — `zerodds-mqtt-bridge`

> **Layer:** 5 (Bridges) | **Reviewer:** claude | **Public-Strategy:** 🌐 public

## 1 Purpose

OASIS MQTT v5.0 Wire-Codec + Broker + Topic-Filter + Keep-Alive + DDS-Bridge. `no_std + alloc`.

## 2-5 Inhalt

- 12 src-Files (broker, codec, control_packets, data_types, dds_bridge, keep_alive, lib, packet, properties, reason_codes, topic_filter, vbi).
- 1 tests-File (fuzz_smoke.rs).
- **115 Tests gruen** (107 unit + 7 fuzz-smoke + 1 doc).
- Spec: OASIS MQTT 5.0 §1.5 + §2.1 + §2.2.2 + §2.4 + §3 + §4.1 + §4.7.

### Coherence-Audit

| Item-Familie | Spec | External Refs | Klassifikation |
|---|---|---|---|
| Wire-Codec (`encode_*` / `decode_*`) | §1.5 + §2 + §3 | — | OPTIONAL-HOOK (Substrat fuer Caller-Endpoint) |
| `Broker` / `Session` | §3 + §4.1 | — | OPTIONAL-HOOK (In-Memory-Referenz-Broker) |
| `topic_matches` / `validate_*` | §4.7 | — | OPTIONAL-HOOK |
| `KeepAliveTracker` | §3.1.2.10 | — | OPTIONAL-HOOK |
| `MqttDdsBridge` / `TopicMapper` | DDS-Mapping | — | OPTIONAL-HOOK |
| `Property` / `property_data_type` / `ReasonCode` | §2.2.2 + §2.4 | (intern) | CONNECTED |

Akzeptanz: 5 OPTIONAL-HOOKs explizit dokumentiert (Substrat-Crate ohne direkte Caller im Workspace). 0 ❌-Klassen.

## 6 Cleanup

- **Forbidden:** 0 Treffer.
- **Sprint-Marker:** 2 Treffer in `control_packets.rs` (header + line 628 "Phase-B-Cluster-6 (Spec-Cycle 5)") entfernt.
- **TODO/FIXME:** 0 Treffer.
- **lib.rs**: Header korrigiert — vorherige "Was nicht abgedeckt" Sektion war stale (Topic-Filter-Matching + Session-State SIND voll abgedeckt: `topic_filter.rs` + `broker.rs`).

## 7 Cleanup-Actions

1. Cargo.toml: publish=true + Metadata komplett.
2. lib.rs: vollstaendiger RC1-Header mit Public-API-Liste + Quickstart-Doc-Test.
3. SPDX-License-Header auf alle 12 src-Files.
4. Sprint-Marker entfernt (control_packets.rs 2 Stellen).
5. README + CHANGELOG.
6. Mirror unter `github/crates/mqtt-bridge/` (incl. tests/).
7. `website/docs/mqtt-bridge.md`.
8. Tracker: 5.7 mqtt-bridge → ✅.

## 10-12 Gates

- `cargo test`: ✅ 115 tests.
- `cargo clippy --tests -- -D warnings`: ✅.
- `cargo fmt --check`: ✅.
- `cargo doc --no-deps`: ✅.

## 13 Daemon-Wireup-Append

Folgende Items sind nach dem ersten Sign-off in den `daemon`-Feature-
Pfad eingebracht worden (kein Major-Bump, alles innerhalb 1.0.0-rc.1):

- `daemon/runtime_common.rs` + `daemon/qos_translation.rs` +
  `daemon/server.rs` + `daemon/config.rs`.
- TLS-Connector (rustls 0.23 ClientConnection) + SASL-PLAIN + Bearer +
  ACL via `zerodds-bridge-security` voll wired (Bridge-Spec
  §7.1/§7.2/§7.3).
- `cross_vendor.rs`-Modul + `backoff.rs` (Exponential-Backoff fuer
  Broker-Reconnect).
- Tests gruen: 107 unit + 7 fuzz-smoke + 1 doc.

**Crate-Version:** `1.0.0-rc.1` | **Sign-off:** claude
