# `zerodds-ws-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-ws-bridge-1.0.md`

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1, `zerodds-ws-bridge-1.0.md` — sechs Conformance-Levels
(Wire/DDS/Bridging/Config/Auth/Multi-Tenant); L1–L4 Pflicht, L5–L6
optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-ws-bridged CLI

**Spec:** §2 — Optionen `--config`/`--listen`/`--domain`/`--topic`/
`--tls-cert`/`--tls-key`/`--auth-token`/`--log-level`/`--metrics`/
`--version`/`--help`; Exit-Codes 0/1/2/3/4.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/websocket-bridge/src/daemon/cli.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`,
`::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

## §3 Config-File-Format

### §3 YAML/JSON/TOML Config-Loader

**Spec:** §3 — Top-Level `listen`/`domain`/`log_level`/`tls`/`auth`/
`topics`/`metrics`; ENV-Substitution `${VAR}` und `${VAR:-default}`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs`,
`crates/websocket-bridge/src/daemon/mod.rs` (DaemonConfig).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::make_test_config`
(Config-Construction). Loader-Round-Trip-Coverage in
`crates/websocket-bridge/src/daemon/config.rs::tests`.

**Status:** done

## §4 WebSocket-Wire-Protocol

### §4.1 RFC 6455 Handshake + zerodds-ws-bridge/1.0 Subprotocol

**Spec:** §4.1 — RFC-6455-Upgrade; Subprotocol-Header
`Sec-WebSocket-Protocol: zerodds-ws-bridge/1.0`; Auth-Header per
Bearer-Mode; 401/403 bei Auth-Failure.

**Repo:** `crates/websocket-bridge/src/handshake.rs`,
`crates/websocket-bridge/src/negotiation.rs`,
`crates/websocket-bridge/src/daemon/server.rs` (Upgrade-Path).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`.

**Status:** done

### §4.2 Pfad-Routing /topics/<slug> + Meta-Endpoints

**Spec:** §4.2 — `/topics/<slug>` Default-Pfad; `ws_path` Override;
`/topics/__catalog__`/`/healthz`/`/metrics` Meta-Endpoints.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/daemon/server.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(Topic-Pfad-Routing).

**Status:** done

### §4.3 Frame-Format Binary "ZDB1" + JSON-Mode

**Spec:** §4.3 — Binary-Magic `0x5A 0x44 0x42 0x31`, Flags,
4-Byte-Encap-Header `[0x00,0x07,0x00,0x00]`, CDR-Payload; Text-Frame
JSON-Mode optional.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (frame
encode/decode), `crates/websocket-bridge/src/codec.rs` (RFC-6455
codec).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`,
`crates/websocket-bridge/tests/fuzz_smoke.rs`.

**Status:** done

### §4.4 Control-Messages subscribe/unsubscribe/ping/pong/error

**Spec:** §4.4 — JSON-Control-Messages bidir + RFC-6455 PING/PONG.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/dds_bridge.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus Topic-Name → URL

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `ws_path`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (slug helper),
`crates/websocket-bridge/src/daemon/router.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (Pfad-Routing
deckt slug-Generation).

**Status:** done

### §5.2 Type-Discovery Catalog-Frame + /schema/<slug>

**Spec:** §5.2 — Catalog-Frame + `/schema/<slug>` IDL-Endpoint.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs` (catalog +
schema endpoints).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (Catalog +
`/healthz` Endpoints durch Cluster-A-Wireup).

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → WS-Behavior Map

**Spec:** §6 — Reliability/Durability/History/Deadline/Liveliness/
Partition-Mapping.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (QoS-Pump);
`crates/websocket-bridge/src/daemon/qos_translation.rs` (Reliability/
Durability/History/Deadline/Liveliness/Partition-Map);
`crates/websocket-bridge/src/daemon/config.rs` (QoS-Felder).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(reliable Roundtrip); Cluster-A QoS-Matrix in
`crates/websocket-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS wss:// + Cert-Rotation

**Spec:** §7.1 — `wss://`-Mode per `tls.enabled`, SIGHUP-Cert-Rotation.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (TLS-Hook),
`crates/websocket-bridge/src/daemon/security.rs` (rustls-Wireup),
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs` (TLS-
Handshake + SIGHUP-Cert-Rotation via Cluster-B-Foundation).

**Status:** done

### §7.2 Auth-Modes none/bearer/jwt/mtls

**Spec:** §7.2 — vier Auth-Modes über Header bzw. Cert.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs`,
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/auth.rs` (none/bearer/jwt/mtls).

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs` (Bearer +
JWT + mTLS Roundtrip).

**Status:** done

### §7.3 Per-Topic-ACL read/write

**Spec:** §7.3 — `acl.read`/`acl.write` Listen mit `*`/exact/group-Match.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (ACL-Felder),
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs` (Match-Engine).

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs` (Per-Topic-
ACL Enforcement).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log auf stdout, Felder timestamp/level/event/
connection_id/topic/bytes/peer/latency_us; `--log-level`-Switch.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/websocket-bridge/src/daemon/cli.rs` (log-level Wire-Up).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (Daemon-Spawn
mit log-level).

**Status:** done

### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + `metrics.*`-Config; 11 Counter/Gauge
Metric-Familien.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (metrics
listener), `crates/websocket-bridge/src/daemon/config.rs` (metrics
config block), `crates/websocket-bridge/src/daemon/runtime_common.rs`
(Counter/Gauge-Familien Cluster-A-Wireup).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (`/metrics`-
Endpoint via Cluster-A-Wireup).

**Status:** done

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT`-ENV aktiviert
Span-Emission per `zerodds-observability-otlp`.

**Repo:** `crates/websocket-bridge/src/daemon/runtime_common.rs`
(OTLP-Init via `zerodds-observability-otlp`),
`crates/websocket-bridge/src/daemon/server.rs` (Span-Emit pro Frame).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (Daemon-Spawn
mit `OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config-Parse → TLS → DCPS → Reader/Writer-Register →
WS-Bind → Signal-Handler.

**Repo:** `crates/websocket-bridge/src/daemon/mod.rs`,
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`
(Spawn-Sequence).

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain (max 30 s), `op:shutdown`-Frame, Close
1001, Cleanup; SIGHUP Hot-Reload TLS+ACL.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (signal +
drain), `crates/websocket-bridge/src/daemon/runtime_common.rs` (SIGTERM/
SIGINT/SIGHUP via Cluster-A-Signal-Handler);
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (Daemon-Stop
am Test-Ende), `crates/websocket-bridge/tests/security_e2e.rs` (SIGHUP-
Reload TLS+ACL).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer Co-Existence Cyclone/RTI/Fast-DDS

**Spec:** §10 — Daemon ist normaler RTPS-Peer; verifiziert in
`crates/websocket-bridge/tests/cross_vendor.rs`.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (DCPS-Wire-Up).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(DDS-Roundtrip via DomainParticipant),
`crates/websocket-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor RTPS-Peer Cyclone/RTI/Fast-DDS).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-ws-bridged`; Config-Defaults pro OS;
systemd/launchd/Win-Service; Docker `zerodds/ws-bridged:1.0`; Manuals
1+5.

**Repo:** `packaging/linux/systemd/zerodds-ws-bridged.service`,
`packaging/macos/launchd/org.zerodds.ws-bridged.plist`,
`packaging/macos/homebrew/zerodds-ws-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ws-bridged/`,
`packaging/linux/configs/ws-bridged.yaml.example`,
`man/man1/zerodds-ws-bridged.1`,
`man/man5/zerodds-ws-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/topic_map/auth/frame_codec/dds_pump je ≥ 5
Tests.

**Repo:** `crates/websocket-bridge/src/{daemon/config.rs,codec.rs,frame.rs,handshake.rs,masking.rs,negotiation.rs,permessage_deflate.rs,uri.rs,utf8.rs,dds_bridge.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul; Workspace-Test
deckt sie ab.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, WS-Client connect, DDS-Sub empfängt;
byte-genauer Roundtrip.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`,
`::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-Subscriber im Docker-Compose, ZeroDDS-Bridge
published.

**Repo:** `crates/websocket-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/websocket-bridge/tests/cross_vendor.rs` (Cyclone-
Subscriber-Roundtrip + WS-Bridge-Publish).

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + Daemons

**Spec:** §13 — Library `crates/websocket-bridge/`, Wire-Format-Spec
`zerodds-xcdr2-bindings-conformance-1.0` §3, Deployment-Spec
`zerodds-deployment-1.0`, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config, Major=Wire-
Protocol-Change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

22 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-websocket-bridge` — Tests grün, 0 failed.

Offene Punkte und Decision-Records: siehe `zerodds-ws-bridge-1.0.open.md`.
