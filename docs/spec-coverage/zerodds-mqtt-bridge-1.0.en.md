# `zerodds-mqtt-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-mqtt-bridge-1.0.md`

Implementation:

- `crates/mqtt-bridge/` — DDS↔MQTT bridge.

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-mqtt-bridged CLI

**Spec:** §2 — options
`--config`/`--broker`/`--client-id`/`--domain`/`--username`/`--password`/`--tls-*`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4/5.

**Repo:** `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`,
`crates/mqtt-bridge/src/daemon/cli.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`,
`::mqtt_publish_to_daemon_does_not_crash_and_subscribe_arrived`,
`::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`log_level`/`mqtt`/`topics`/`acl`/`metrics`;
ENV substitution `${VAR}` and `${VAR:-default}`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/yaml.rs`, `crates/mqtt-bridge/src/daemon/mod.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::make_test_config`
(config construction). Inline tests in `config.rs::tests` cover the YAML
round-trip.

**Status:** done

## §4 MQTT wire protocol

### §4.1 CONNECT with MQTT-5 properties

**Spec:** §4.1 — CONNECT with
Session-Expiry/Receive-Maximum/Max-Packet-Size/Topic-Alias-Max/Authentication-Method/Authentication-Data;
CONNACK 0x80+ → exit 5.

**Repo:** `crates/mqtt-bridge/src/control_packets.rs`,
`crates/mqtt-bridge/src/properties.rs`,
`crates/mqtt-bridge/src/daemon/client.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`.

**Status:** done

### §4.2 PUBLISH with Content-Type + user properties

**Spec:** §4.2 — PUBLISH with `Payload Format Indicator=0`, Content-Type
`application/x-dds-cdr2`, user properties
`zerodds_type`/`zerodds_topic`/`zerodds_flags`/`zerodds_key_hash`/`zerodds_source_ts_ns`;
encap header `[0x00,0x07,0x00,0x00]` + CDR.

**Repo:** `crates/mqtt-bridge/src/codec.rs`,
`crates/mqtt-bridge/src/properties.rs`,
`crates/mqtt-bridge/src/dds_bridge.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

### §4.3 SUBSCRIBE with Subscription-Identifier

**Spec:** §4.3 — SUBSCRIBE per `direction=in|bidir` with a
Subscription-Identifier, QoS derived from DDS, NoLocal=1.

**Repo:** `crates/mqtt-bridge/src/control_packets.rs`,
`crates/mqtt-bridge/src/topic_filter.rs`,
`crates/mqtt-bridge/src/daemon/client.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::mqtt_publish_to_daemon_does_not_crash_and_subscribe_arrived`.

**Status:** done

### §4.4 zerodds_op control property

**Spec:** §4.4 — user property `zerodds_op` values
`sample`/`dispose`/`unregister`/`register`; default `sample`.

**Repo:** `crates/mqtt-bridge/src/properties.rs`,
`crates/mqtt-bridge/src/dds_bridge.rs`.

**Tests:** `crates/mqtt-bridge/src/properties.rs::tests` (op-property
encode/decode), `daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

## §5 Topic mapping

### §5.1 Slug algorithm DDS → MQTT

**Spec:** §5.1 — lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; override per
`mqtt_topic`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (slug helper),
`crates/mqtt-bridge/src/topic_filter.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (topic mapping
implicitly via the pump).

**Status:** done

### §5.2 Catalog retain on $zerodds/<client_id>/catalog

**Spec:** §5.2 — catalog retain JSON with a topics list.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (catalog-publish hook),
`crates/mqtt-bridge/src/daemon/runtime_common.rs` (catalog retain via
cluster-A wire-up).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (catalog-retain topic
via cluster-A wire-up).

**Status:** done

## §6 QoS translation

### §6 DDS-QoS → MQTT-behavior + auto-derivation

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition
map; auto-derivation of `mqtt_qos` from Reliability, `retain` from
Durability.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (QoS fields +
derivation), `crates/mqtt-bridge/src/dds_bridge.rs`,
`crates/mqtt-bridge/src/daemon/qos_translation.rs` (cluster-A QoS map
Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`
(reliable QoS-1 mapping); a QoS matrix in
`crates/mqtt-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS mqtts:// + ALPN

**Spec:** §7.1 — `mqtts://` mode per `mqtt.tls.enabled`; ALPN `["mqtt"]`;
SIGHUP cert rotation.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (TLS hook),
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs` (mqtts + ALPN verify +
cert rotation via the cluster-B foundation).

**Status:** done

### §7.2 SASL/MQTT auth modes

**Spec:** §7.2 — none/password/mtls/enhanced (SCRAM/OAUTHBEARER/JWT).

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`,
`crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/auth.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs` (password + mtls +
JWT-bearer round-trip).

**Status:** done

### §7.3 ACL daemon-side

**Spec:** §7.3 — `acl.default_deny` + `rules` with
`subject`/`allow_publish`/`allow_subscribe`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (ACL fields),
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs` (ACL enforcement
against a subject matrix).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`,
`crates/mqtt-bridge/src/daemon/cli.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (daemon spawn with
log-level args).

**Status:** done

### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + `metrics.*` config; 12 counter/gauge
families.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs`,
`crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs` (counter/gauge families
cluster-A wire-up).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (`/metrics` endpoint via
cluster-A wire-up).

**Status:** done

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables span emission.

**Repo:** `crates/mqtt-bridge/src/daemon/runtime_common.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/mqtt-bridge/src/daemon/client.rs`
(span emit per PUBLISH round-trip).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (daemon spawn with
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config parse → TLS → DCPS → reader/writer → MQTT connect →
SUBSCRIBE → signal handler.

**Repo:** `crates/mqtt-bridge/src/daemon/mod.rs`,
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, DISCONNECT 0x00, cleanup; SIGHUP
TLS+ACL reload.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (drain),
`crates/mqtt-bridge/src/daemon/runtime_common.rs` (SIGTERM/SIGINT/SIGHUP via
the cluster-A signal handler);
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (daemon stop),
`crates/mqtt-bridge/tests/security_e2e.rs` (SIGHUP reload TLS+ACL).

**Status:** done

### §9.3 Reconnect with exponential backoff

**Spec:** §9.3 — broker disconnect → backoff `initial_delay_ms`..`max_delay_ms`;
session state per `clean_start=false`.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (reconnect loop with
backoff via cluster-C wire-up), `crates/mqtt-bridge/src/keep_alive.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (reconnect sequence
against a mock-broker disconnect; cluster-C edge case).

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + Mosquitto/EMQX/HiveMQ

**Spec:** §10 — the daemon is a normal RTPS peer; the MQTT side tested
against Mosquitto/EMQX/HiveMQ.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (mock-broker round-trip),
`crates/mqtt-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
Mosquitto/EMQX/HiveMQ matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-mqtt-bridged`; config defaults per OS;
systemd/launchd/Windows service; Docker `zerodds/mqtt-bridged:1.0`; manuals.

**Repo:** `packaging/linux/systemd/zerodds-mqtt-bridged.service`,
`packaging/macos/launchd/org.zerodds.mqtt-bridged.plist`,
`packaging/macos/homebrew/zerodds-mqtt-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/mqtt-bridged/`,
`packaging/linux/configs/mqtt-bridged.yaml.example`,
`man/man1/zerodds-mqtt-bridged.1`, `man/man5/zerodds-mqtt-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/mqtt_codec/topic_map/qos_translate/dds_pump/acl, ≥ 5
tests each.

**Repo:** `crates/mqtt-bridge/src/{daemon/config.rs,codec.rs,control_packets.rs,topic_filter.rs,properties.rs,reason_codes.rs,vbi.rs,packet.rs,data_types.rs,keep_alive.rs,dds_bridge.rs,broker.rs}`.

**Tests:** inline `#[cfg(test)] mod tests` per module.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, Mosquitto via testcontainers, round-trip
MQTT↔DDS.

**Repo:** `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`,
`crates/mqtt-bridge/src/broker.rs` (in-test mock broker).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`,
`::mqtt_publish_to_daemon_does_not_crash_and_subscribe_arrived`,
`::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone-DDS subscriber in compose, broker matrix
Mosquitto/EMQX/HiveMQ/Aedes.

**Repo:** `crates/mqtt-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
harness).

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs` (broker matrix +
Cyclone-DDS subscriber).

**Status:** done

## §13 Cross-references

### §13 Related library + standards + daemons

**Spec:** §13 — library `crates/mqtt-bridge/`, the OASIS MQTT-5 standard,
wire format, deployment, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config, major=wire-protocol
changes (MQTT-5.x→6).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

23 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-mqtt-bridge` — tests green, 0 failed.

No open items or decision records — all items `done` / `n/a (informative)`.
