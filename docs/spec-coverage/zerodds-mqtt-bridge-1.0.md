# `zerodds-mqtt-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-mqtt-bridge-1.0.md`

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-mqtt-bridged CLI

**Spec:** §2 — Optionen `--config`/`--broker`/`--client-id`/`--domain`/
`--username`/`--password`/`--tls-*`/`--topic`/`--log-level`/`--metrics`/
`--version`/`--help`; Exit-Codes 0/1/2/3/4/5.

**Repo:** `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`,
`crates/mqtt-bridge/src/daemon/cli.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`,
`::mqtt_publish_to_daemon_does_not_crash_and_subscribe_arrived`,
`::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`log_level`/`mqtt`/`topics`/`acl`/
`metrics`; ENV-Substitution `${VAR}` und `${VAR:-default}`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/yaml.rs`,
`crates/mqtt-bridge/src/daemon/mod.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::make_test_config`
(Config-Construction). Inline-Tests in `config.rs::tests` decken
YAML-Roundtrip.

**Status:** done

## §4 MQTT-Wire-Protocol

### §4.1 CONNECT mit MQTT-5-Properties

**Spec:** §4.1 — CONNECT mit Session-Expiry/Receive-Maximum/Max-Packet-
Size/Topic-Alias-Max/Authentication-Method/Authentication-Data; CONNACK-
0x80+ → Exit 5.

**Repo:** `crates/mqtt-bridge/src/control_packets.rs`,
`crates/mqtt-bridge/src/properties.rs`,
`crates/mqtt-bridge/src/daemon/client.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`.

**Status:** done

### §4.2 PUBLISH mit Content-Type + User-Properties

**Spec:** §4.2 — PUBLISH mit `Payload Format Indicator=0`, Content-Type
`application/x-dds-cdr2`, User-Properties `zerodds_type`/`zerodds_topic`/
`zerodds_flags`/`zerodds_key_hash`/`zerodds_source_ts_ns`; Encap-Header
`[0x00,0x07,0x00,0x00]` + CDR.

**Repo:** `crates/mqtt-bridge/src/codec.rs`,
`crates/mqtt-bridge/src/properties.rs`,
`crates/mqtt-bridge/src/dds_bridge.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

### §4.3 SUBSCRIBE mit Subscription-Identifier

**Spec:** §4.3 — SUBSCRIBE pro `direction=in|bidir` mit Subscription-
Identifier, QoS aus DDS abgeleitet, NoLocal=1.

**Repo:** `crates/mqtt-bridge/src/control_packets.rs`,
`crates/mqtt-bridge/src/topic_filter.rs`,
`crates/mqtt-bridge/src/daemon/client.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::mqtt_publish_to_daemon_does_not_crash_and_subscribe_arrived`.

**Status:** done

### §4.4 zerodds_op Control-Property

**Spec:** §4.4 — User-Property `zerodds_op` Werte `sample`/`dispose`/
`unregister`/`register`; Default `sample`.

**Repo:** `crates/mqtt-bridge/src/properties.rs`,
`crates/mqtt-bridge/src/dds_bridge.rs`.

**Tests:** `crates/mqtt-bridge/src/properties.rs::tests` (op-property
encode/decode), `daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus DDS → MQTT

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `mqtt_topic`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (slug helper),
`crates/mqtt-bridge/src/topic_filter.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (Topic-Mapping
implizit über Pump).

**Status:** done

### §5.2 Catalog-Retain auf $zerodds/<client_id>/catalog

**Spec:** §5.2 — Catalog-Retain JSON mit topics-Liste.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (catalog-publish
hook), `crates/mqtt-bridge/src/daemon/runtime_common.rs` (Catalog-Retain
via Cluster-A-Wireup).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (Catalog-Retain-
Topic via Cluster-A-Wireup).

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → MQTT-Behavior + Auto-Derivation

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/
Liveliness/Partition Map; Auto-Derivation `mqtt_qos` aus Reliability,
`retain` aus Durability.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (QoS-Felder +
Derivation), `crates/mqtt-bridge/src/dds_bridge.rs`,
`crates/mqtt-bridge/src/daemon/qos_translation.rs` (Cluster-A QoS-Map
Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`
(reliable QoS-1 mapping); QoS-Matrix in
`crates/mqtt-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS mqtts:// + ALPN

**Spec:** §7.1 — `mqtts://`-Mode per `mqtt.tls.enabled`; ALPN `["mqtt"]`;
SIGHUP-Cert-Rotation.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (TLS-Hook),
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs` (mqtts +
ALPN-Verify + Cert-Rotation via Cluster-B-Foundation).

**Status:** done

### §7.2 SASL/MQTT-Auth-Modes

**Spec:** §7.2 — none/password/mtls/enhanced (SCRAM/OAUTHBEARER/JWT).

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`,
`crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/auth.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs` (password + mtls +
JWT-Bearer Roundtrip).

**Status:** done

### §7.3 ACL Daemon-Side

**Spec:** §7.3 — `acl.default_deny` + `rules` mit `subject`/
`allow_publish`/`allow_subscribe`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (ACL-Felder),
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs` (ACL-
Enforcement gegen Subject-Matrix).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`,
`crates/mqtt-bridge/src/daemon/cli.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (Daemon-Spawn mit
log-level-Args).

**Status:** done

### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + `metrics.*`-Config; 12 Counter/Gauge
Familien.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs`,
`crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs` (Counter/Gauge-
Familien Cluster-A-Wireup).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (`/metrics`-Endpoint
via Cluster-A-Wireup).

**Status:** done

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert Span-Emission.

**Repo:** `crates/mqtt-bridge/src/daemon/runtime_common.rs` (OTLP-Init
via `zerodds-observability-otlp`),
`crates/mqtt-bridge/src/daemon/client.rs` (Span-Emit pro PUBLISH-
Roundtrip).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (Daemon-Spawn mit
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config-Parse → TLS → DCPS → Reader/Writer → MQTT-
Connect → SUBSCRIBE → Signal-Handler.

**Repo:** `crates/mqtt-bridge/src/daemon/mod.rs`,
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, DISCONNECT 0x00, Cleanup;
SIGHUP TLS+ACL-Reload.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (drain),
`crates/mqtt-bridge/src/daemon/runtime_common.rs` (SIGTERM/SIGINT/
SIGHUP via Cluster-A-Signal-Handler);
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (Daemon-Stop),
`crates/mqtt-bridge/tests/security_e2e.rs` (SIGHUP-Reload TLS+ACL).

**Status:** done

### §9.3 Reconnect mit Exponential-Backoff

**Spec:** §9.3 — Broker-Disconnect → Backoff `initial_delay_ms`..
`max_delay_ms`; Session-State per `clean_start=false`.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (Reconnect-Loop
mit Backoff via Cluster-C-Wireup),
`crates/mqtt-bridge/src/keep_alive.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (Reconnect-
Sequence gegen Mock-Broker disconnect; Cluster-C-Edge-Case).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + Mosquitto/EMQX/HiveMQ

**Spec:** §10 — Daemon ist normaler RTPS-Peer; MQTT-Seite gegen
Mosquitto/EMQX/HiveMQ getestet.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs` (mock-broker
roundtrip), `crates/mqtt-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor Mosquitto/EMQX/HiveMQ-Matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-mqtt-bridged`; Config-Defaults pro OS;
systemd/launchd/Win-Service; Docker `zerodds/mqtt-bridged:1.0`; Manuals.

**Repo:** `packaging/linux/systemd/zerodds-mqtt-bridged.service`,
`packaging/macos/launchd/org.zerodds.mqtt-bridged.plist`,
`packaging/macos/homebrew/zerodds-mqtt-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/mqtt-bridged/`,
`packaging/linux/configs/mqtt-bridged.yaml.example`,
`man/man1/zerodds-mqtt-bridged.1`,
`man/man5/zerodds-mqtt-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/mqtt_codec/topic_map/qos_translate/dds_pump/
acl je ≥ 5 Tests.

**Repo:** `crates/mqtt-bridge/src/{daemon/config.rs,codec.rs,control_packets.rs,topic_filter.rs,properties.rs,reason_codes.rs,vbi.rs,packet.rs,data_types.rs,keep_alive.rs,dds_bridge.rs,broker.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, Mosquitto via testcontainers, Roundtrip
MQTT↔DDS.

**Repo:** `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`,
`crates/mqtt-bridge/src/broker.rs` (in-test mock-broker).

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::daemon_connects_and_subscribes`,
`::mqtt_publish_to_daemon_does_not_crash_and_subscribe_arrived`,
`::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-DDS-Subscriber im Compose, Broker-Matrix
Mosquitto/EMQX/HiveMQ/Aedes.

**Repo:** `crates/mqtt-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs` (Broker-Matrix
+ Cyclone-DDS-Subscriber).

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + Standards + Daemons

**Spec:** §13 — Library `crates/mqtt-bridge/`, OASIS-MQTT-5-Standard,
Wire-Format, Deployment, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config, Major=Wire-
Protocol-Changes (MQTT-5.x→6).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

23 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-mqtt-bridge` — Tests grün, 0 failed.

Offene Punkte und Decision-Records: siehe `zerodds-mqtt-bridge-1.0.open.md`.
