# `zerodds-amqp-bridge-daemon` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-amqp-bridge-daemon-1.0.md`

Implementation:

- `crates/amqp-endpoint/` — AMQP-Bridge-Daemon-Endpoint.

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-amqp-bridged CLI

**Spec:** §2 — Optionen `--config`/`--broker`/`--container-id`/
`--domain`/`--sasl-mechanism`/`--user`/`--password`/`--tls-*`/`--topic`/
`--log-level`/`--metrics`/`--version`/`--help`; Exit-Codes 0/1/2/3/4/5/6.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::version_flag_emits_one_line_and_exits_zero`,
`::missing_broker_url_fails_with_exit_1`,
`::daemon_with_no_broker_fails_with_exit_2`,
`::daemon_opens_and_attaches_link`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`amqp`/`topics`/`acl`/`metrics`;
ENV-Substitution `${VAR}` und `${VAR:-default}`.

**Repo:** `crates/amqp-endpoint/src/config_xml.rs` (Loader-Plumbing),
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` (CLI-Config-
Mapping), `crates/amqp-endpoint/src/mapping.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `config_xml.rs`,
`mapping.rs`.

**Status:** done

## §4 AMQP-Wire-Protocol

### §4.1 Connection-Open mit Properties + Capabilities

**Spec:** §4.1 — `OPEN`-Performative mit container-id/hostname/
max-frame-size/channel-max/idle-time-out/properties (`zerodds_version`,
`zerodds_role`)/desired-+offered-capabilities `["AMQP_DDS_BRIDGE"]`.

**Repo:** `crates/amqp-endpoint/src/session.rs`,
`crates/amqp-endpoint/src/properties.rs`,
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_opens_and_attaches_link`.

**Status:** done

### §4.2 Session + Link-Setup ATTACH

**Spec:** §4.2 — `ATTACH role=sender|receiver` pro Topic-Direction,
name/source/target, snd-/rcv-settle-mode, properties (`zerodds_topic`/
`zerodds_type`), desired-capabilities `["dds.cdr2"]`.

**Repo:** `crates/amqp-endpoint/src/link.rs`,
`crates/amqp-endpoint/src/session.rs`,
`crates/amqp-endpoint/src/properties.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_opens_and_attaches_link`.

**Status:** done

### §4.3 TRANSFER-Frame DDS→AMQP

**Spec:** §4.3 — `TRANSFER` mit delivery-id/-tag/message-format/settled
+ Header (durable/priority/ttl)/Properties (message-id/subject/
content-type)/ApplicationProperties (`zerodds_*`)/BodySection mit
`[0x00,0x07,0x00,0x00]` + CDR-Bytes.

**Repo:** `crates/amqp-endpoint/src/link.rs` (transfer encoder),
`crates/amqp-endpoint/src/dds_bridge.rs`,
`crates/amqp-endpoint/src/properties.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `link.rs`,
`dds_bridge.rs`; `bridge_e2e.rs::daemon_opens_and_attaches_link`.

**Status:** done

### §4.4 TRANSFER-Frame AMQP→DDS + DISPOSITION

**Spec:** §4.4 — Receiver-Link mit `FLOW`-Credit, Decoder
Body-Section→CDR→DDS-Sample; DISPOSITION `accepted`/`rejected`/
`released`.

**Repo:** `crates/amqp-endpoint/src/link.rs`,
`crates/amqp-endpoint/src/routing.rs`,
`crates/amqp-endpoint/src/dds_bridge.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `link.rs`, `routing.rs`.

**Status:** done

### §4.5 Disposition-Mapping accepted/rejected/released/modified

**Spec:** §4.5 — AMQP-Outcome → DDS-Behavior; modified retry mit
delivery-count.

**Repo:** `crates/amqp-endpoint/src/link.rs`,
`crates/amqp-endpoint/src/rpc_correlation.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` deckt Outcome-Map.

**Status:** done

## §5 Topic-Mapping

### §5.1 Address-Default Broker-spezifisch

**Spec:** §5.1 — RabbitMQ/ActiveMQ/Qpid-Dispatch-Defaults; Override per
`amqp_address`.

**Repo:** `crates/amqp-endpoint/src/mapping.rs`,
`crates/amqp-endpoint/src/routing.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `mapping.rs`.

**Status:** done

### §5.2 Catalog auf zerodds.bridge.<container>.catalog

**Spec:** §5.2 — Catalog-Address mit topics-Liste.

**Repo:** `crates/amqp-endpoint/src/management.rs`,
`crates/amqp-endpoint/src/daemon_runtime.rs` (Catalog-Address Cluster-
A-Wireup),
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs` (Catalog-
Address-Subscriber via Cluster-A-Wireup).

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → AMQP-Behavior Map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/
Liveliness/Partition Map; settled-mode-Auto-Derivation.

**Repo:** `crates/amqp-endpoint/src/mapping.rs`,
`crates/amqp-endpoint/src/dds_bridge.rs`,
`crates/amqp-endpoint/src/qos_translation.rs` (Cluster-A QoS-Map
Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition).

**Tests:** Inline `#[cfg(test)] mod tests` in `mapping.rs` und
`qos_translation.rs`; QoS-Matrix-Coverage in
`crates/amqp-endpoint/src/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS amqps:// + ALPN

**Spec:** §7.1 — `amqps://` per `amqp.tls.enabled`, ALPN `["amqp"]`.

**Repo:** `crates/amqp-endpoint/src/security.rs`,
`crates/amqp-endpoint/src/bridge_security.rs`,
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/amqp-endpoint/tests/security_e2e.rs` (amqps + ALPN-
Verify + Cert-Rotation via Cluster-B-Foundation).

**Status:** done

### §7.2 SASL PLAIN/SCRAM/EXTERNAL/ANONYMOUS/XOAUTH2

**Spec:** §7.2 — SASL-Mechanism-Auswahl pro Config.

**Repo:** `crates/amqp-endpoint/src/sasl.rs`,
`crates/amqp-endpoint/src/bridge_security.rs`,
`crates/bridge-security/src/auth.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `sasl.rs` (PLAIN +
ANONYMOUS); `crates/amqp-endpoint/tests/security_e2e.rs` (SCRAM-SHA-
256/512 + EXTERNAL + XOAUTH2 via Cluster-B-Wireup).

**Status:** done

### §7.3 ACL Daemon-Side

**Spec:** §7.3 — Subject = SASL-Identity oder TLS-DN; Filter vor
TRANSFER.

**Repo:** `crates/amqp-endpoint/src/security.rs`,
`crates/amqp-endpoint/src/bridge_security.rs`,
`crates/bridge-security/src/acl.rs`,
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/security_e2e.rs` (ACL-
Enforcement gegen Subject-Matrix).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs` (log-level Args).

**Status:** done

### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + 13 Counter/Gauge-Familien.

**Repo:** `crates/amqp-endpoint/src/metrics.rs`,
`crates/amqp-endpoint/src/daemon_runtime.rs` (Cluster-A Counter/Gauge-
Familien Wireup),
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `metrics.rs`;
`crates/amqp-endpoint/tests/bridge_e2e.rs` (`/metrics`-Endpoint via
Cluster-A-Wireup).

**Status:** done

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert
TRANSFER-Roundtrip-Span.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (OTLP-Init via
`zerodds-observability-otlp`),
`crates/amqp-endpoint/src/link.rs` (Span-Emit pro TRANSFER).

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs` (Daemon-Spawn
mit `OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → TLS → DCPS → Reader/Writer → AMQP-Open + SASL
+ TLS → SESSION + ATTACH → Signal-Handler.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`,
`crates/amqp-endpoint/src/session.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_opens_and_attaches_link`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, DETACH/END/CLOSE; SIGHUP
TLS+ACL-Reload.

**Repo:** `crates/amqp-endpoint/src/session.rs`,
`crates/amqp-endpoint/src/link.rs`,
`crates/amqp-endpoint/src/daemon_runtime.rs` (SIGTERM/SIGINT/SIGHUP
via Cluster-A-Signal-Handler),
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs` (Daemon-Stop),
`crates/amqp-endpoint/tests/security_e2e.rs` (SIGHUP-Reload TLS+ACL).

**Status:** done

### §9.3 Reconnect

**Spec:** §9.3 — Exponential-Backoff; Unsettled-Deliveries Re-Attempt.

**Repo:** `crates/amqp-endpoint/src/session.rs`,
`crates/amqp-endpoint/src/backoff.rs` (Cluster-C Reconnect-Backoff),
`crates/amqp-endpoint/src/limits.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_with_no_broker_fails_with_exit_2`
(Connect-Failure); Cluster-C Reconnect-Sequence in
`crates/amqp-endpoint/tests/bridge_e2e.rs` (Mock-Broker disconnect →
Re-Attempt).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + RabbitMQ/Artemis/Qpid/Solace/ServiceBus

**Spec:** §10 — Daemon ist normaler RTPS-Peer; AMQP-Seite gegen
RabbitMQ/Artemis/Qpid-Dispatch/Solace/Azure ServiceBus.

**Repo:** `crates/amqp-endpoint/src/coexistence.rs`.

**Tests:** `crates/amqp-endpoint/tests/e2e_multi_bridge_hop.rs` (Multi-
Hop-Test mit Mock-Broker),
`crates/amqp-endpoint/tests/cross_vendor.rs` (Cluster-C Cross-Vendor
RabbitMQ/Artemis/Qpid/Solace/ServiceBus-Matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-amqp-bridged`; Configs/Services/Docker;
Manuals.

**Repo:** `packaging/linux/systemd/zerodds-amqp-bridged.service`,
`packaging/macos/launchd/org.zerodds.amqp-bridged.plist`,
`packaging/macos/homebrew/zerodds-amqp-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/amqp-bridged/`,
`packaging/linux/configs/amqp-bridged.yaml.example`,
`man/man1/zerodds-amqp-bridged.1`,
`man/man5/zerodds-amqp-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/amqp_codec/link_state/disposition/sasl/
dds_pump/acl je ≥ 5 Tests.

**Repo:** `crates/amqp-endpoint/src/{session.rs,link.rs,routing.rs,sasl.rs,security.rs,mapping.rs,properties.rs,metrics.rs,limits.rs,annex_a.rs,management.rs,rpc_correlation.rs,coexistence.rs,keyhash.rs,errors.rs,dds_bridge.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul; Property-Tests
in `crates/amqp-endpoint/tests/proptest_state_machine.rs`.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, RabbitMQ via testcontainers, Roundtrip
+ DISPOSITION-Sequence.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_opens_and_attaches_link`,
`::daemon_with_no_broker_fails_with_exit_2`,
`::version_flag_emits_one_line_and_exits_zero`,
`::missing_broker_url_fails_with_exit_1`;
`crates/amqp-endpoint/tests/e2e_multi_bridge_hop.rs`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-DDS-Subscriber + RabbitMQ + ZeroDDS-AMQP-
Bridge im Compose; Broker-Matrix RabbitMQ/Artemis/Qpid/Solace/
ServiceBus.

**Repo:** `crates/amqp-endpoint/src/coexistence.rs`,
`crates/amqp-endpoint/tests/cross_vendor.rs` (Cluster-C Cross-Vendor-
Harness).

**Tests:** `crates/amqp-endpoint/tests/e2e_multi_bridge_hop.rs` (Mock-
Broker), `crates/amqp-endpoint/tests/cross_vendor.rs` (RabbitMQ/
Artemis/Qpid/Solace/ServiceBus-Matrix via Cluster-C).

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + OMG-Spec + Standards

**Spec:** §13 — Library `crates/amqp-bridge/` + `crates/amqp-endpoint/`,
OMG-DDS-AMQP-1.0, ISO/IEC 19464:2014, Wire-Format, Deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config, Major=Wire-
Protocol-Change (AMQP-1.0→2).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

24 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-amqp-endpoint` — Tests grün, 0 failed.

Keine offenen Punkte oder Decision-Records — alle Items `done` / `n/a (informative)`.
