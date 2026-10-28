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
`::a_broker_without_the_sasl_layer_is_refused_by_name` (exit 5),
`::plain_without_tls_is_refused_towards_a_remote_broker` (exit 1).

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`amqp`/`topics`/`acl`/`metrics`/`catalog`;
ENV substitution `${VAR}` and `${VAR:-default}`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`DaemonConfig::from_config`, `read_topic`),
`crates/config/src/parse.rs` (the shared parser, flat flow maps).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::{the_spec_example_loads, what_the_daemon_would_not_honour_is_refused, the_shipped_example_reads_as_it_says, values_the_shared_parser_reads_reach_the_daemon}`;
`crates/config/tests/layering.rs::{a_flat_flow_map_is_a_map, a_hash_inside_quotes_is_part_of_the_value, environment_references_expand_before_the_parse}`.

**Status:** done

## §4 AMQP wire protocol

### §4.1 Connection open with properties + capabilities

**Spec:** §4.1 — `OPEN` with container-id/hostname/max-frame-size/
channel-max/idle-time-out/properties/desired + offered capabilities.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`connect`, `open_body`), `crates/amqp-endpoint/src/bridge_daemon.rs::run`.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::dds_samples_arrive_in_the_queue_as_pass_through_messages`
(RabbitMQ accepts the `open`), `crates/amqp-endpoint/tests/broker_tls_sasl.rs` (the test server reads the `open`).

**Status:** done

### §4.2 Session + link setup ATTACH

**Spec:** §4.2 — `ATTACH` per direction with name/source/target,
settle modes, properties, desired capabilities; one session per link.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`BrokerWriter::begin`, `attach`), `crates/amqp-endpoint/src/bridge_daemon.rs` (`link_spec`,
`LinkState`, `begin_all`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(RabbitMQ ends the session of the refused address, the others keep
running), `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::after_the_broker_closes_the_connection_the_bridge_reconnects`.

**Status:** done

### §4.3 TRANSFER frame DDS→AMQP

**Spec:** §4.3 — a message per DDS-AMQP 1.0 §8: header, properties,
`dds:*` application properties, a Pass-Through body with the encapsulation
header; multi-frame above `max-frame-size`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`encode_message`, `drain`), `crates/amqp-endpoint/src/broker.rs` (`BrokerWriter::transfer`),
`crates/amqp-endpoint/src/properties.rs`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::{a_sample_round_trips_through_the_message_form, an_xcdr1_sample_keeps_its_header_and_says_so}`;
`crates/amqp-endpoint/src/broker.rs::tests::a_message_above_the_frame_size_is_split_and_reassembled`;
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::{dds_samples_arrive_in_the_queue_as_pass_through_messages, a_sample_larger_than_the_frame_size_crosses_in_both_directions}`.

**Status:** done

### §4.4 TRANSFER frame AMQP→DDS + DISPOSITION

**Spec:** §4.4 — receiver link with `FLOW` credit, body → DDS sample,
DISPOSITION `accepted`/`rejected`/`released`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`deliver`, `decode_message`, `split_encapsulation`),
`crates/amqp-endpoint/src/broker.rs` (`BrokerReader::next_event`, reassembly).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_message_published_by_pika_reaches_a_dds_reader_and_a_bad_body_does_not`
(accepted and rejected, the queue empty afterwards),
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_sample_larger_than_the_frame_size_crosses_in_both_directions`
(loop prevention and multi-frame receive).

**Status:** done

### §4.5 Disposition mapping accepted/rejected/released/modified

**Spec:** §4.5 — the broker's outcome of unsettled deliveries.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs::on_inbound` (`Inbound::Disposition`), `crates/amqp-endpoint/src/broker.rs` (`decode_outcome`).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::outcomes_round_trip_through_their_described_form`;
`accepted` against RabbitMQ in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::dds_samples_arrive_in_the_queue_as_pass_through_messages`.
`released`/`modified`/`rejected` from a broker are not measured.

**Status:** partial — `released`/`modified`/`rejected` from a broker are not measured.
## §5 Topic mapping

### §5.1 Address

**Spec:** §5.1 — `amqp_address` per topic, broker-specific.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs::read_topic`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::what_the_daemon_would_not_honour_is_refused`;
RabbitMQ v2 addresses (`/queues/…`) in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`.

**Status:** done

### §5.2 Catalog

**Spec:** §5.2 — a catalog (JSON) per connection on its own address.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`catalog_message`, `DaemonConfig::catalog_json`,
`catalog_address`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(`pika` reads the catalog from `/queues/zd_bridge_catalog`).

**Status:** done

## §6 QoS translation

### §6 DDS QoS → AMQP behaviour map

**Spec:** §6 — reliability/durability/history/lifespan/deadline/
liveliness/partition.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`read_topic`: BEST_EFFORT → settled; `encode_message`:
durable, ttl; `OutLink::cap`), `crates/amqp-endpoint/src/broker.rs::BrokerWriter::heartbeat`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::the_spec_example_loads` (BEST_EFFORT → settled);
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(LIFESPAN 60 s → `expiration` 60000).

**Open:** the deadline annotation, the partition filter; KEEP_ALL drops
the oldest sample beyond 65 536 waiting ones instead of back-pressure.

**Status:** partial — the deadline annotation and the partition filter are missing; KEEP_ALL drops the oldest beyond 65 536 waiting samples.
## §7 Security

### §7.1 TLS amqps:// + ALPN

**Spec:** §7.1 — `amqps://` per `amqp.tls.enabled`, ALPN `["amqp"]`.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`Wire::Tls`, `ALPN_AMQP`), `crates/amqp-endpoint/src/bridge_daemon.rs::tls_config`.

**Tests:** `crates/amqp-endpoint/tests/broker_tls_sasl.rs::external_authenticates_with_the_tls_client_certificate_and_alpn_amqp`
(the server reads `amqp` as the agreed protocol).

**Status:** done

### §7.1a ALPN negotiation `["amqp"]`

**Spec:** §7.1 — the TLS handshake offers ALPN `["amqp"]`.

**Repo:** `crates/amqp-endpoint/src/broker.rs::connect`.

**Tests:** as §7.1.

**Status:** done

### §7.2 SASL PLAIN/SCRAM/EXTERNAL/ANONYMOUS/XOAUTH2

**Spec:** §7.2 — the mechanism per config; PLAIN only over TLS, ANONYMOUS
only towards loopback.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`sasl`, `scram_client_final`), `crates/amqp-endpoint/src/bridge_daemon.rs` (`DaemonConfig::sasl`,
the checks in `run`).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::scram_client_final_matches_the_rfc_7677_example`;
`crates/amqp-endpoint/tests/broker_tls_sasl.rs::{scram_sha_256_authenticates_and_a_wrong_password_is_refused, external_authenticates_with_the_tls_client_certificate_and_alpn_amqp, a_mechanism_the_broker_does_not_offer_is_refused_by_name}`;
PLAIN against RabbitMQ in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`;
`crates/amqp-endpoint/tests/bridge_e2e.rs::{plain_without_tls_is_refused_towards_a_remote_broker, a_broker_without_the_sasl_layer_is_refused_by_name}`.
XOAUTH2 is not measured against a broker; SCRAM-SHA-512 is not built.

**Status:** partial — XOAUTH2 not measured against a broker; SCRAM-SHA-512 not built.
### §7.3 ACL daemon side

**Spec:** §7.3 — subject = SASL identity; filter before TRANSFER.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`allowed`, `acl.rules`, `acl.default_deny`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::the_shipped_example_reads_as_it_says` (the rules
are read). No test checks that a denied direction moves no data.

**Status:** partial — no test checks that a denied direction moves no data.
## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log on stdout with timestamp/level/event/… and
`--log-level`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`log`, `event_level`, `LOG_LEVEL`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::every_event_has_a_level_and_the_threshold_orders_them`.

**Open:** the lines go to stderr, without `timestamp`, `container_id`,
`delivery_id`, `latency_us`.

**Status:** partial — lines on stderr, without `timestamp`, `container_id`, `delivery_id`, `latency_us`.
### §8.2 Prometheus metrics

**Spec:** §8.2 — 13 families `zerodds_amqp_bridge_*` with labels.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (`BridgeMetrics`),
`crates/amqp-endpoint/src/bridge_daemon.rs` (the counting sites).

**Tests:** `crates/amqp-endpoint/src/daemon_runtime.rs::tests`,
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::the_metrics_of_section_8_2_count_transfers_dispositions_and_samples`
(Linux test host, RabbitMQ 4.0.5: 3 samples out, one good and one broken message
in; transfers, samples and dispositions per topic,
`rejected{reason="amqp:decode-error"}` 1).

**Status:** done — all 13 families; dispositions count in both
directions, `reason` is the AMQP condition (`amqp:*`, else `other`). The
generic `zerodds_amqp_*` remain beside them.
### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables the TRANSFER
round-trip span.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/amqp-endpoint/src/link.rs` (span
emit per TRANSFER).

**Tests:** `crates/amqp-endpoint/src/daemon_runtime.rs::tests::otlp_config_parses_endpoint` — the endpoint from
`OTEL_EXPORTER_OTLP_ENDPOINT` is read. Nothing tests that a span is emitted
per TRANSFER — see §8.3a.

**Status:** partial — the configuration is covered, the span emit is not; the
missing half is carried as §8.3a `open`.
## §9 Lifecycle

### §8.3a TRANSFER roundtrip span

**Spec:** §8.3 — with `OTEL_EXPORTER_OTLP_ENDPOINT` set, one roundtrip span
is emitted per TRANSFER.

**Repo:** —

**Tests:** —

**Status:** open — the emit half of §8.3. Only reading the endpoint
configuration is covered.

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → reader/writer → open + SASL + TLS →
session + attach → signal handler.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs::run`, `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs` (every case starts the bridge through `run`).

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain of at most 30 s, DETACH/END/CLOSE; SIGHUP
TLS + ACL reload.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs::pump` (`DRAIN_BUDGET`), `crates/amqp-endpoint/src/bridge_daemon.rs::run` (DETACH/END/CLOSE);
`crates/amqp-endpoint/src/bridge_daemon.rs::run_with_reload`, `::apply_reload`
(the ACL and the TLS material of the next connect; withdrawn rights apply
at once, new grants after a restart).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_reload_that_withdraws_the_read_right_stops_the_topic`
(RabbitMQ 4.0, Linux test host); no test for the drain; the stop through the flag
runs in every case of `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`.

**Status:** partial — no test for the drain.

### §9.3 Reconnect

**Spec:** §9.3 — exponential back-off; unsettled deliveries sent again.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`next_delay`, the requeue in `run` and `out_link_down`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::after_the_broker_closes_the_connection_the_bridge_reconnects`
(`rabbitmqctl close_all_connections`, both samples arrive in order).

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + RabbitMQ/Artemis/Qpid/Solace/ServiceBus

**Spec:** §10 — a plain RTPS peer; the AMQP side against five brokers.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs`, `crates/amqp-endpoint/src/broker.rs`, `crates/amqp-endpoint/src/coexistence.rs`.

**Tests:** against RabbitMQ 4.0.5 (Linux test host) `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`, 5 cases; the bridge
chain in `crates/amqp-endpoint/tests/e2e_multi_bridge_hop.rs`. Artemis,
Qpid Dispatch, Solace, Azure ServiceBus: not measured — see §10a.

**Status:** partial — RabbitMQ 4.0.5 measured, the four other brokers not (§10a).
### §10a Broker matrix Artemis/Qpid Dispatch/Solace/Azure ServiceBus

**Spec:** §10 — the AMQP side against Artemis, Qpid Dispatch, Solace and
Azure ServiceBus.

**Repo:** —

**Tests:** —

**Status:** open — the part of §10 and §12.3 beyond RabbitMQ.
---

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

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::the_shipped_example_reads_as_it_says`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys).

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/amqp_codec/link_state/disposition/sasl/
dds_pump/acl, ≥ 5 tests each.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs`, `crates/amqp-endpoint/src/broker.rs`, the modules under `crates/amqp-endpoint/src/`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests` (8), `crates/amqp-endpoint/src/broker.rs::tests` (7), the library's module tests
(`cargo test -p zerodds-amqp-endpoint --lib`: 248).

**Status:** done

### §12.2 Integration tests

**Spec:** §12.2 — the daemon against RabbitMQ, round trip + DISPOSITION
sequence.

**Repo:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs`, `crates/amqp-endpoint/tests/bridge_e2e.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs` (5, `#[ignore]`, Linux test host), `crates/amqp-endpoint/tests/broker_tls_sasl.rs` (3), `crates/amqp-endpoint/tests/bridge_e2e.rs` (5).

**Status:** done

### §12.3 Multi-vendor

**Spec:** §12.3 — Cyclone DDS subscriber + RabbitMQ + bridge; broker matrix.

**Repo:** —

**Tests:** —

**Open:** a Cyclone subscriber behind the bridge, brokers beyond RabbitMQ.

**Status:** open — a Cyclone subscriber behind the bridge and brokers beyond RabbitMQ are not measured.
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

16 done / 8 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-amqp-endpoint` — tests green, 0 failed.

Open items: see `zerodds-amqp-bridge-daemon-1.0.open.en.md`. Decision records: none.
