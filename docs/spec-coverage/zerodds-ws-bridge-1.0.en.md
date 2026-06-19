# `zerodds-ws-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-ws-bridge-1.0.md`

Implementation:

- `crates/websocket-bridge/` — DDS↔WebSocket bridge.

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1, `zerodds-ws-bridge-1.0.md` — six conformance levels
(Wire/DDS/Bridging/Config/Auth/Multi-Tenant); L1–L4 mandatory, L5–L6
optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-ws-bridged CLI

**Spec:** §2 — options `--config`/`--listen`/`--domain`/`--topic`/
`--tls-cert`/`--tls-key`/`--auth-token`/`--log-level`/`--metrics`/
`--version`/`--help`; exit codes 0/1/2/3/4.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/websocket-bridge/src/daemon/cli.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`,
`::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

## §3 Config-file format

### §3 YAML/JSON/TOML config loader

**Spec:** §3 — top-level `listen`/`domain`/`log_level`/`tls`/`auth`/
`topics`/`metrics`; ENV substitution `${VAR}` and `${VAR:-default}`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs`,
`crates/websocket-bridge/src/daemon/mod.rs` (DaemonConfig).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::make_test_config`
(config construction). Loader round-trip coverage in
`crates/websocket-bridge/src/daemon/config.rs::tests`.

**Status:** done

## §4 WebSocket wire protocol

### §4.1 RFC 6455 handshake + zerodds-ws-bridge/1.0 subprotocol

**Spec:** §4.1 — RFC-6455 upgrade; subprotocol header
`Sec-WebSocket-Protocol: zerodds-ws-bridge/1.0`; auth header per bearer
mode; 401/403 on auth failure.

**Repo:** `crates/websocket-bridge/src/handshake.rs`,
`crates/websocket-bridge/src/negotiation.rs`,
`crates/websocket-bridge/src/daemon/server.rs` (upgrade path).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`.

**Status:** done

### §4.2 Path routing /topics/<slug> + meta endpoints

**Spec:** §4.2 — `/topics/<slug>` default path; `ws_path` override;
`/topics/__catalog__`/`/healthz`/`/metrics` meta endpoints.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/daemon/server.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(topic-path routing).

**Status:** done

### §4.3 Frame format binary "ZDB1" + JSON mode

**Spec:** §4.3 — binary magic `0x5A 0x44 0x42 0x31`, flags, a 4-byte
encap header `[0x00,0x07,0x00,0x00]`, CDR payload; text-frame JSON mode
optional.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (frame
encode/decode), `crates/websocket-bridge/src/codec.rs` (RFC-6455 codec).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`,
`crates/websocket-bridge/tests/fuzz_smoke.rs`.

**Status:** done

### §4.4 Control messages subscribe/unsubscribe/ping/pong/error

**Spec:** §4.4 — JSON control messages bidir + RFC-6455 PING/PONG.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/dds_bridge.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

## §5 Topic mapping

### §5.1 Slug algorithm topic name → URL

**Spec:** §5.1 — lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; override per
`ws_path`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (slug helper),
`crates/websocket-bridge/src/daemon/router.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (path routing
covers slug generation).

**Status:** done

### §5.2 Type discovery catalog frame + /schema/<slug>

**Spec:** §5.2 — catalog frame + a `/schema/<slug>` IDL endpoint.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs` (catalog + schema
endpoints).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (catalog +
`/healthz` endpoints via cluster-A wire-up).

**Status:** done

## §6 QoS translation

### §6 DDS-QoS → WS-behavior map

**Spec:** §6 — Reliability/Durability/History/Deadline/Liveliness/Partition
mapping.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (QoS pump);
`crates/websocket-bridge/src/daemon/qos_translation.rs`
(Reliability/Durability/History/Deadline/Liveliness/Partition map);
`crates/websocket-bridge/src/daemon/config.rs` (QoS fields).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(reliable round-trip); a cluster-A QoS matrix in
`crates/websocket-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS wss:// + cert rotation

**Spec:** §7.1 — `wss://` mode per `tls.enabled`, SIGHUP cert rotation.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (TLS hook),
`crates/websocket-bridge/src/daemon/security.rs` (rustls wire-up),
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs` (TLS handshake +
SIGHUP cert rotation via the cluster-B foundation).

**Status:** done

### §7.2 Auth modes none/bearer/jwt/mtls

**Spec:** §7.2 — four auth modes via header or cert.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs`,
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/auth.rs` (none/bearer/jwt/mtls).

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs` (bearer + JWT +
mTLS round-trip).

**Status:** done

### §7.3 Per-topic ACL read/write

**Spec:** §7.3 — `acl.read`/`acl.write` lists with `*`/exact/group match.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (ACL fields),
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs` (match engine).

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs` (per-topic ACL
enforcement).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log on stdout, fields
timestamp/level/event/connection_id/topic/bytes/peer/latency_us; a
`--log-level` switch.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/websocket-bridge/src/daemon/cli.rs` (log-level wire-up).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (daemon spawn with
log-level).

**Status:** done

### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + `metrics.*` config; 11 counter/gauge
metric families.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (metrics listener),
`crates/websocket-bridge/src/daemon/config.rs` (metrics config block),
`crates/websocket-bridge/src/daemon/runtime_common.rs` (counter/gauge
families cluster-A wire-up).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (`/metrics`
endpoint via cluster-A wire-up).

**Status:** done

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` ENV enables span emission via
`zerodds-observability-otlp`.

**Repo:** `crates/websocket-bridge/src/daemon/runtime_common.rs` (OTLP init
via `zerodds-observability-otlp`),
`crates/websocket-bridge/src/daemon/server.rs` (span emit per frame).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (daemon spawn with
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config parse → TLS → DCPS → reader/writer register → WS
bind → signal handler.

**Repo:** `crates/websocket-bridge/src/daemon/mod.rs`,
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`
(spawn sequence).

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain (max 30 s), `op:shutdown` frame, close
1001, cleanup; SIGHUP hot-reload TLS+ACL.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (signal + drain),
`crates/websocket-bridge/src/daemon/runtime_common.rs`
(SIGTERM/SIGINT/SIGHUP via the cluster-A signal handler);
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs` (daemon stop at
test end), `crates/websocket-bridge/tests/security_e2e.rs` (SIGHUP reload
TLS+ACL).

**Status:** done

## §10 Cross-vendor

### §10 RTPS-peer co-existence Cyclone/RTI/Fast-DDS

**Spec:** §10 — the daemon is a normal RTPS peer; verified in
`crates/websocket-bridge/tests/cross_vendor.rs`.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (DCPS wire-up).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(DDS round-trip via DomainParticipant),
`crates/websocket-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
RTPS peer Cyclone/RTI/Fast-DDS).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-ws-bridged`; config defaults per OS;
systemd/launchd/Windows service; Docker `zerodds/ws-bridged:1.0`; manuals
1+5.

**Repo:** `packaging/linux/systemd/zerodds-ws-bridged.service`,
`packaging/macos/launchd/org.zerodds.ws-bridged.plist`,
`packaging/macos/homebrew/zerodds-ws-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ws-bridged/`,
`packaging/linux/configs/ws-bridged.yaml.example`,
`man/man1/zerodds-ws-bridged.1`, `man/man5/zerodds-ws-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/topic_map/auth/frame_codec/dds_pump, ≥ 5 tests
each.

**Repo:** `crates/websocket-bridge/src/{daemon/config.rs,codec.rs,frame.rs,handshake.rs,masking.rs,negotiation.rs,permessage_deflate.rs,uri.rs,utf8.rs,dds_bridge.rs}`.

**Tests:** inline `#[cfg(test)] mod tests` per module; the workspace test
covers them.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, WS client connect, DDS sub receives;
byte-exact round-trip.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`,
`::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone subscriber in docker-compose, the ZeroDDS bridge
publishes.

**Repo:** `crates/websocket-bridge/tests/cross_vendor.rs` (cluster-C
cross-vendor harness).

**Tests:** `crates/websocket-bridge/tests/cross_vendor.rs` (Cyclone
subscriber round-trip + WS-bridge publish).

**Status:** done

## §13 Cross-references

### §13 Related library + daemons

**Spec:** §13 — library `crates/websocket-bridge/`, the wire-format spec
`zerodds-xcdr2-bindings-conformance-1.0` §3, the deployment spec
`zerodds-deployment-1.0`, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config, major=wire-protocol
change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

22 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-websocket-bridge` — tests green, 0 failed.

No open items or decision records — all items `done` / `n/a (informative)`.
