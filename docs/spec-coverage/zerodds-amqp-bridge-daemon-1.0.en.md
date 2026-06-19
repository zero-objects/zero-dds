# `zerodds-amqp-bridge-daemon` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-amqp-bridge-daemon-1.0.md`

Implementation:

- `crates/amqp-endpoint/` — AMQP bridge daemon endpoint.

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-amqp-bridged CLI

**Spec:** §2 — options
`--config`/`--broker`/`--container-id`/`--domain`/`--sasl-mechanism`/`--user`/`--password`/`--tls-*`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4/5/6.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::version_flag_emits_one_line_and_exits_zero`,
`::missing_broker_url_fails_with_exit_1`,
`::daemon_with_no_broker_fails_with_exit_2`,
`::daemon_opens_and_attaches_link`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`amqp`/`topics`/`acl`/`metrics`; ENV
substitution `${VAR}` and `${VAR:-default}`.

**Repo:** `crates/amqp-endpoint/src/config_xml.rs` (loader plumbing),
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` (CLI-config
mapping), `crates/amqp-endpoint/src/mapping.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `config_xml.rs`, `mapping.rs`.

**Status:** done

## §4 AMQP wire protocol

### §4.1 Connection-Open with properties + capabilities

**Spec:** §4.1 — `OPEN` performative with
container-id/hostname/max-frame-size/channel-max/idle-time-out/properties
(`zerodds_version`, `zerodds_role`)/desired-+offered-capabilities
`["AMQP_DDS_BRIDGE"]`.

**Repo:** `crates/amqp-endpoint/src/session.rs`,
`crates/amqp-endpoint/src/properties.rs`,
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_opens_and_attaches_link`.

**Status:** done

### §4.2 Session + link setup ATTACH

**Spec:** §4.2 — `ATTACH role=sender|receiver` per topic direction,
name/source/target, snd-/rcv-settle-mode, properties
(`zerodds_topic`/`zerodds_type`), desired-capabilities `["dds.cdr2"]`.

**Repo:** `crates/amqp-endpoint/src/link.rs`,
`crates/amqp-endpoint/src/session.rs`,
`crates/amqp-endpoint/src/properties.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_opens_and_attaches_link`.

**Status:** done

### §4.3 TRANSFER frame DDS→AMQP

**Spec:** §4.3 — `TRANSFER` with delivery-id/-tag/message-format/settled +
Header (durable/priority/ttl)/Properties (message-id/subject/content-type)/ApplicationProperties
(`zerodds_*`)/BodySection with `[0x00,0x07,0x00,0x00]` + CDR bytes.

**Repo:** `crates/amqp-endpoint/src/link.rs` (transfer encoder),
`crates/amqp-endpoint/src/dds_bridge.rs`,
`crates/amqp-endpoint/src/properties.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `link.rs`, `dds_bridge.rs`;
`bridge_e2e.rs::daemon_opens_and_attaches_link`.

**Status:** done

### §4.4 TRANSFER frame AMQP→DDS + DISPOSITION

**Spec:** §4.4 — receiver link with `FLOW` credit, decoder body
section→CDR→DDS sample; DISPOSITION `accepted`/`rejected`/`released`.

**Repo:** `crates/amqp-endpoint/src/link.rs`,
`crates/amqp-endpoint/src/routing.rs`,
`crates/amqp-endpoint/src/dds_bridge.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `link.rs`, `routing.rs`.

**Status:** done

### §4.5 Disposition mapping accepted/rejected/released/modified

**Spec:** §4.5 — AMQP outcome → DDS behavior; modified retry with
delivery-count.

**Repo:** `crates/amqp-endpoint/src/link.rs`,
`crates/amqp-endpoint/src/rpc_correlation.rs`.

**Tests:** inline `#[cfg(test)] mod tests` covers the outcome map.

**Status:** done

## §5 Topic mapping

### §5.1 Address default broker-specific

**Spec:** §5.1 — RabbitMQ/ActiveMQ/Qpid-Dispatch defaults; override per
`amqp_address`.

**Repo:** `crates/amqp-endpoint/src/mapping.rs`,
`crates/amqp-endpoint/src/routing.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `mapping.rs`.

**Status:** done

### §5.2 Catalog on zerodds.bridge.<container>.catalog

**Spec:** §5.2 — a catalog address with a topics list.

**Repo:** `crates/amqp-endpoint/src/management.rs`,
`crates/amqp-endpoint/src/daemon_runtime.rs` (catalog address cluster-A
wire-up), `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs` (catalog-address
subscriber via cluster-A wire-up).

**Status:** done

## §6 QoS translation

### §6 DDS-QoS → AMQP-behavior map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition
map; settled-mode auto-derivation.

**Repo:** `crates/amqp-endpoint/src/mapping.rs`,
`crates/amqp-endpoint/src/dds_bridge.rs`,
`crates/amqp-endpoint/src/qos_translation.rs` (cluster-A QoS map
Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition).

**Tests:** inline `#[cfg(test)] mod tests` in `mapping.rs` and
`qos_translation.rs`; QoS-matrix coverage in
`crates/amqp-endpoint/src/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS amqps:// + ALPN

**Spec:** §7.1 — `amqps://` per `amqp.tls.enabled`, ALPN `["amqp"]`.

**Repo:** `crates/amqp-endpoint/src/security.rs`,
`crates/amqp-endpoint/src/bridge_security.rs`,
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/amqp-endpoint/tests/security_e2e.rs` (amqps + ALPN verify
+ cert rotation via the cluster-B foundation).

**Status:** done

### §7.2 SASL PLAIN/SCRAM/EXTERNAL/ANONYMOUS/XOAUTH2

**Spec:** §7.2 — SASL mechanism choice per config.

**Repo:** `crates/amqp-endpoint/src/sasl.rs`,
`crates/amqp-endpoint/src/bridge_security.rs`,
`crates/bridge-security/src/auth.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `sasl.rs` (PLAIN +
ANONYMOUS); `crates/amqp-endpoint/tests/security_e2e.rs`
(SCRAM-SHA-256/512 + EXTERNAL + XOAUTH2 via cluster-B wire-up).

**Status:** done

### §7.3 ACL daemon-side

**Spec:** §7.3 — subject = SASL identity or TLS DN; filter before TRANSFER.

**Repo:** `crates/amqp-endpoint/src/security.rs`,
`crates/amqp-endpoint/src/bridge_security.rs`,
`crates/bridge-security/src/acl.rs`,
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/security_e2e.rs` (ACL enforcement
against a subject matrix).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs` (log-level args).

**Status:** done

### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + 13 counter/gauge families.

**Repo:** `crates/amqp-endpoint/src/metrics.rs`,
`crates/amqp-endpoint/src/daemon_runtime.rs` (cluster-A counter/gauge
families wire-up), `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `metrics.rs`;
`crates/amqp-endpoint/tests/bridge_e2e.rs` (`/metrics` endpoint via
cluster-A wire-up).

**Status:** done

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables the TRANSFER
round-trip span.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/amqp-endpoint/src/link.rs` (span
emit per TRANSFER).

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs` (daemon spawn with
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → reader/writer → AMQP-Open + SASL +
TLS → SESSION + ATTACH → signal handler.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`,
`crates/amqp-endpoint/src/session.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_opens_and_attaches_link`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, DETACH/END/CLOSE; SIGHUP TLS+ACL
reload.

**Repo:** `crates/amqp-endpoint/src/session.rs`,
`crates/amqp-endpoint/src/link.rs`,
`crates/amqp-endpoint/src/daemon_runtime.rs` (SIGTERM/SIGINT/SIGHUP via the
cluster-A signal handler),
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs` (daemon stop),
`crates/amqp-endpoint/tests/security_e2e.rs` (SIGHUP reload TLS+ACL).

**Status:** done

### §9.3 Reconnect

**Spec:** §9.3 — exponential backoff; unsettled deliveries re-attempt.

**Repo:** `crates/amqp-endpoint/src/session.rs`,
`crates/amqp-endpoint/src/backoff.rs` (cluster-C reconnect backoff),
`crates/amqp-endpoint/src/limits.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_with_no_broker_fails_with_exit_2`
(connect failure); a cluster-C reconnect sequence in
`crates/amqp-endpoint/tests/bridge_e2e.rs` (mock-broker disconnect →
re-attempt).

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + RabbitMQ/Artemis/Qpid/Solace/ServiceBus

**Spec:** §10 — the daemon is a normal RTPS peer; the AMQP side against
RabbitMQ/Artemis/Qpid-Dispatch/Solace/Azure ServiceBus.

**Repo:** `crates/amqp-endpoint/src/coexistence.rs`.

**Tests:** `crates/amqp-endpoint/tests/e2e_multi_bridge_hop.rs` (multi-hop
test with a mock broker), `crates/amqp-endpoint/tests/cross_vendor.rs`
(cluster-C cross-vendor RabbitMQ/Artemis/Qpid/Solace/ServiceBus matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-amqp-bridged`; configs/services/Docker;
manuals.

**Repo:** `packaging/linux/systemd/zerodds-amqp-bridged.service`,
`packaging/macos/launchd/org.zerodds.amqp-bridged.plist`,
`packaging/macos/homebrew/zerodds-amqp-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/amqp-bridged/`,
`packaging/linux/configs/amqp-bridged.yaml.example`,
`man/man1/zerodds-amqp-bridged.1`, `man/man5/zerodds-amqp-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/amqp_codec/link_state/disposition/sasl/dds_pump/acl,
≥ 5 tests each.

**Repo:** `crates/amqp-endpoint/src/{session.rs,link.rs,routing.rs,sasl.rs,security.rs,mapping.rs,properties.rs,metrics.rs,limits.rs,annex_a.rs,management.rs,rpc_correlation.rs,coexistence.rs,keyhash.rs,errors.rs,dds_bridge.rs}`.

**Tests:** inline `#[cfg(test)] mod tests` per module; property tests in
`crates/amqp-endpoint/tests/proptest_state_machine.rs`.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, RabbitMQ via testcontainers, round-trip
+ DISPOSITION sequence.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::daemon_opens_and_attaches_link`,
`::daemon_with_no_broker_fails_with_exit_2`,
`::version_flag_emits_one_line_and_exits_zero`,
`::missing_broker_url_fails_with_exit_1`;
`crates/amqp-endpoint/tests/e2e_multi_bridge_hop.rs`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone-DDS subscriber + RabbitMQ + the ZeroDDS AMQP
bridge in compose; broker matrix RabbitMQ/Artemis/Qpid/Solace/ServiceBus.

**Repo:** `crates/amqp-endpoint/src/coexistence.rs`,
`crates/amqp-endpoint/tests/cross_vendor.rs` (cluster-C cross-vendor
harness).

**Tests:** `crates/amqp-endpoint/tests/e2e_multi_bridge_hop.rs` (mock
broker), `crates/amqp-endpoint/tests/cross_vendor.rs`
(RabbitMQ/Artemis/Qpid/Solace/ServiceBus matrix via cluster-C).

**Status:** done

## §13 Cross-references

### §13 Related library + OMG spec + standards

**Spec:** §13 — library `crates/amqp-bridge/` + `crates/amqp-endpoint/`,
OMG DDS-AMQP-1.0, ISO/IEC 19464:2014, wire format, deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config, major=wire-protocol
change (AMQP-1.0→2).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

24 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-amqp-endpoint` — tests green, 0 failed.

No open items or decision records — all items `done` / `n/a (informative)`.
