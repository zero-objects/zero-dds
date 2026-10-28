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
`::a_broker_without_the_sasl_layer_is_refused_by_name` (Exit 5),
`::plain_without_tls_is_refused_towards_a_remote_broker` (Exit 1).

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`amqp`/`topics`/`acl`/`metrics`/`catalog`;
ENV-Substitution `${VAR}` und `${VAR:-default}`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`DaemonConfig::from_config`, `read_topic`),
`crates/config/src/parse.rs` (gemeinsamer Parser, flache Flow-Maps).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::{the_spec_example_loads, what_the_daemon_would_not_honour_is_refused, the_shipped_example_reads_as_it_says, values_the_shared_parser_reads_reach_the_daemon}`;
`crates/config/tests/layering.rs::{a_flat_flow_map_is_a_map, a_hash_inside_quotes_is_part_of_the_value, environment_references_expand_before_the_parse}`.

**Status:** done

## §4 AMQP-Wire-Protocol

### §4.1 Connection-Open mit Properties + Capabilities

**Spec:** §4.1 — `OPEN` mit container-id/hostname/max-frame-size/
channel-max/idle-time-out/properties/desired-+offered-capabilities.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`connect`, `open_body`), `crates/amqp-endpoint/src/bridge_daemon.rs::run`.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::dds_samples_arrive_in_the_queue_as_pass_through_messages`
(RabbitMQ nimmt das `open` an), `crates/amqp-endpoint/tests/broker_tls_sasl.rs` (Test-Server liest das `open`).

**Status:** done

### §4.2 Session + Link-Setup ATTACH

**Spec:** §4.2 — `ATTACH` pro Richtung mit name/source/target,
settle-modes, properties, desired-capabilities; eine Session pro Link.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`BrokerWriter::begin`, `attach`), `crates/amqp-endpoint/src/bridge_daemon.rs` (`link_spec`,
`LinkState`, `begin_all`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(RabbitMQ beendet die Session der abgewiesenen Adresse, die anderen laufen
weiter), `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::after_the_broker_closes_the_connection_the_bridge_reconnects`.

**Status:** done

### §4.3 TRANSFER-Frame DDS→AMQP

**Spec:** §4.3 — Message nach DDS-AMQP 1.0 §8: Header, Properties
(`dds:*`-ApplicationProperties), Pass-Through-Body mit
Encapsulation-Header; mehrteilig über `max-frame-size`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`encode_message`, `drain`), `crates/amqp-endpoint/src/broker.rs` (`BrokerWriter::transfer`),
`crates/amqp-endpoint/src/properties.rs`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::{a_sample_round_trips_through_the_message_form, an_xcdr1_sample_keeps_its_header_and_says_so}`;
`crates/amqp-endpoint/src/broker.rs::tests::a_message_above_the_frame_size_is_split_and_reassembled`;
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::{dds_samples_arrive_in_the_queue_as_pass_through_messages, a_sample_larger_than_the_frame_size_crosses_in_both_directions}`.

**Status:** done

### §4.4 TRANSFER-Frame AMQP→DDS + DISPOSITION

**Spec:** §4.4 — Receiver-Link mit `FLOW`-Credit, Body→DDS-Sample,
DISPOSITION `accepted`/`rejected`/`released`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`deliver`, `decode_message`, `split_encapsulation`),
`crates/amqp-endpoint/src/broker.rs` (`BrokerReader::next_event`, Reassemblierung).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_message_published_by_pika_reaches_a_dds_reader_and_a_bad_body_does_not`
(accepted und rejected, Queue danach leer),
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_sample_larger_than_the_frame_size_crosses_in_both_directions`
(Schleifensperre und mehrteiliger Empfang).

**Status:** done

### §4.5 Disposition-Mapping accepted/rejected/released/modified

**Spec:** §4.5 — Outcome des Brokers auf unbestätigte Deliveries.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs::on_inbound` (`Inbound::Disposition`), `crates/amqp-endpoint/src/broker.rs` (`decode_outcome`).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::outcomes_round_trip_through_their_described_form`;
`accepted` gegen RabbitMQ in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::dds_samples_arrive_in_the_queue_as_pass_through_messages`.
`released`/`modified`/`rejected` vom Broker sind nicht gegen einen Broker
gemessen.

**Status:** partial — `released`/`modified`/`rejected` vom Broker sind nicht gegen einen Broker gemessen.
## §5 Topic-Mapping

### §5.1 Address

**Spec:** §5.1 — `amqp_address` pro Topic, Broker-spezifisch.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs::read_topic`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::what_the_daemon_would_not_honour_is_refused`;
RabbitMQ-v2-Adressen (`/queues/…`) in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`.

**Status:** done

### §5.2 Catalog

**Spec:** §5.2 — Catalog (JSON) pro Verbindung auf eigener Adresse.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`catalog_message`, `DaemonConfig::catalog_json`,
`catalog_address`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(`pika` liest den Catalog aus `/queues/zd_bridge_catalog`).

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → AMQP-Behavior Map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/
Liveliness/Partition.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`read_topic`: BEST_EFFORT → settled; `encode_message`:
durable, ttl; `OutLink::cap`), `crates/amqp-endpoint/src/broker.rs::BrokerWriter::heartbeat`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::the_spec_example_loads` (BEST_EFFORT → settled);
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(LIFESPAN 60 s → `expiration` 60000).

**Offen:** Deadline-Annotation, Partition-Filter; KEEP_ALL verwirft über
65 536 wartenden Samples das älteste statt Backpressure.

**Status:** partial — Deadline-Annotation und Partition-Filter fehlen; KEEP_ALL verwirft über 65 536 wartenden Samples das älteste.
## §7 Security

### §7.1 TLS amqps:// + ALPN

**Spec:** §7.1 — `amqps://` per `amqp.tls.enabled`, ALPN `["amqp"]`.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`Wire::Tls`, `ALPN_AMQP`), `crates/amqp-endpoint/src/bridge_daemon.rs::tls_config`.

**Tests:** `crates/amqp-endpoint/tests/broker_tls_sasl.rs::external_authenticates_with_the_tls_client_certificate_and_alpn_amqp`
(der Server liest `amqp` als ausgehandeltes Protokoll).

**Status:** done

### §7.1a ALPN-Aushandlung `["amqp"]`

**Spec:** §7.1 — der TLS-Handshake bietet ALPN `["amqp"]` an.

**Repo:** `crates/amqp-endpoint/src/broker.rs::connect`.

**Tests:** wie §7.1.

**Status:** done

### §7.2 SASL PLAIN/SCRAM/EXTERNAL/ANONYMOUS/XOAUTH2

**Spec:** §7.2 — Mechanismus pro Config; PLAIN nur über TLS, ANONYMOUS nur
gegen Loopback.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`sasl`, `scram_client_final`), `crates/amqp-endpoint/src/bridge_daemon.rs` (`DaemonConfig::sasl`,
Prüfungen in `run`).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::scram_client_final_matches_the_rfc_7677_example`;
`crates/amqp-endpoint/tests/broker_tls_sasl.rs::{scram_sha_256_authenticates_and_a_wrong_password_is_refused, external_authenticates_with_the_tls_client_certificate_and_alpn_amqp, a_mechanism_the_broker_does_not_offer_is_refused_by_name}`;
PLAIN gegen RabbitMQ in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`;
`crates/amqp-endpoint/tests/bridge_e2e.rs::{plain_without_tls_is_refused_towards_a_remote_broker, a_broker_without_the_sasl_layer_is_refused_by_name}`.
XOAUTH2 ist nicht gegen einen Broker gemessen; SCRAM-SHA-512 ist nicht gebaut.

**Status:** partial — XOAUTH2 nicht gegen einen Broker gemessen; SCRAM-SHA-512 nicht gebaut.
### §7.3 ACL Daemon-Side

**Spec:** §7.3 — Subject = SASL-Identity; Filter vor TRANSFER.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`allowed`, `acl.rules`, `acl.default_deny`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::the_shipped_example_reads_as_it_says` (Regeln
gelesen). Kein Test prüft, dass eine verweigerte Richtung keine Daten
bewegt.

**Status:** partial — kein Test prüft, dass eine verweigerte Richtung keine Daten bewegt.
## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log auf stdout mit timestamp/level/event/… und
`--log-level`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`log`, `event_level`, `LOG_LEVEL`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::every_event_has_a_level_and_the_threshold_orders_them`.

**Offen:** die Zeilen gehen auf stderr, ohne `timestamp`, `container_id`,
`delivery_id`, `latency_us`.

**Status:** partial — Zeilen auf stderr, ohne `timestamp`, `container_id`, `delivery_id`, `latency_us`.
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — 13 Familien `zerodds_amqp_bridge_*` mit Labels.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (`BridgeMetrics`),
`crates/amqp-endpoint/src/bridge_daemon.rs` (Zählstellen).

**Tests:** `crates/amqp-endpoint/src/daemon_runtime.rs::tests`,
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::the_metrics_of_section_8_2_count_transfers_dispositions_and_samples`
(Linux-Testhost, RabbitMQ 4.0.5: 3 Samples hinaus, eine gute und eine kaputte
Nachricht herein; Transfers, Samples und Dispositionen pro Topic,
`rejected{reason="amqp:decode-error"}` 1).

**Status:** done — alle 13 Familien; Dispositionen zählen in beiden
Richtungen, `reason` ist die AMQP-Condition (`amqp:*`, sonst `other`).
Daneben bleiben die generischen `zerodds_amqp_*`.
### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert
TRANSFER-Roundtrip-Span.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (OTLP-Init via
`zerodds-observability-otlp`),
`crates/amqp-endpoint/src/link.rs` (Span-Emit pro TRANSFER).

**Tests:** `crates/amqp-endpoint/src/daemon_runtime.rs::tests::otlp_config_parses_endpoint` — der Endpoint aus
`OTEL_EXPORTER_OTLP_ENDPOINT` wird gelesen. Dass pro TRANSFER ein Span
entsteht, prüft kein Test — siehe §8.3a.

**Status:** partial — die Konfiguration ist geprüft, der Span-Emit nicht; der
fehlende Teil wird als §8.3a `open` geführt.
## §9 Lifecycle

### §8.3a TRANSFER-Roundtrip-Span

**Spec:** §8.3 — bei gesetztem `OTEL_EXPORTER_OTLP_ENDPOINT` entsteht pro
TRANSFER ein Roundtrip-Span.

**Repo:** —

**Tests:** —

**Status:** open — der Emit-Teil von §8.3. Geprüft ist nur, dass die
Endpoint-Konfiguration gelesen wird.

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → TLS → DCPS → Reader/Writer → Open + SASL + TLS →
Session + Attach → Signal-Handler.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs::run`, `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs` (alle Fälle starten die Bridge über `run`).

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, DETACH/END/CLOSE; SIGHUP
TLS+ACL-Reload.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs::pump` (`DRAIN_BUDGET`), `crates/amqp-endpoint/src/bridge_daemon.rs::run` (DETACH/END/CLOSE);
`crates/amqp-endpoint/src/bridge_daemon.rs::run_with_reload`, `::apply_reload`
(ACL und das TLS-Material des nächsten Connects; entzogene Rechte gelten
sofort, neu erteilte nach einem Neustart).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::a_reload_that_withdraws_the_read_right_stops_the_topic`
(RabbitMQ 4.0, Linux-Testhost); kein Test für den Drain; der Stopp über das Flag
läuft in jedem Fall von `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`.

**Status:** partial — kein Test für den Drain.

### §9.3 Reconnect

**Spec:** §9.3 — Exponential-Backoff; unbestätigte Deliveries erneut.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`next_delay`, Requeue in `run` und `out_link_down`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::after_the_broker_closes_the_connection_the_bridge_reconnects`
(`rabbitmqctl close_all_connections`, beide Samples kommen in Reihenfolge an).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + RabbitMQ/Artemis/Qpid/Solace/ServiceBus

**Spec:** §10 — normaler RTPS-Peer; AMQP-Seite gegen fünf Broker.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs`, `crates/amqp-endpoint/src/broker.rs`, `crates/amqp-endpoint/src/coexistence.rs`.

**Tests:** gegen RabbitMQ 4.0.5 (Linux-Testhost) `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`, 5 Fälle; die Bridge-Kette
in `crates/amqp-endpoint/tests/e2e_multi_bridge_hop.rs`. Artemis,
Qpid-Dispatch, Solace, Azure ServiceBus: nicht gemessen — siehe §10a.

**Status:** partial — RabbitMQ 4.0.5 gemessen, die vier weiteren Broker nicht (§10a).
### §10a Broker-Matrix Artemis/Qpid-Dispatch/Solace/Azure ServiceBus

**Spec:** §10 — die AMQP-Seite gegen Artemis, Qpid-Dispatch, Solace und
Azure ServiceBus.

**Repo:** —

**Tests:** —

**Status:** open — der Teil von §10 und §12.3 jenseits RabbitMQ.
---

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

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests::the_shipped_example_reads_as_it_says`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel).

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/amqp_codec/link_state/disposition/sasl/
dds_pump/acl je ≥ 5 Tests.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs`, `crates/amqp-endpoint/src/broker.rs`, die Module unter `crates/amqp-endpoint/src/`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon.rs::tests` (8), `crates/amqp-endpoint/src/broker.rs::tests` (7), die Modultests der Bibliothek
(`cargo test -p zerodds-amqp-endpoint --lib`: 248).

**Status:** done

### §12.2 Integration-Tests

**Spec:** §12.2 — Daemon gegen RabbitMQ, Roundtrip + DISPOSITION-Folge.

**Repo:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs`, `crates/amqp-endpoint/tests/bridge_e2e.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs` (5, `#[ignore]`, Linux-Testhost), `crates/amqp-endpoint/tests/broker_tls_sasl.rs` (3), `crates/amqp-endpoint/tests/bridge_e2e.rs` (5).

**Status:** done

### §12.3 Multi-Vendor

**Spec:** §12.3 — Cyclone-DDS-Subscriber + RabbitMQ + Bridge; Broker-Matrix.

**Repo:** —

**Tests:** —

**Offen:** Cyclone-Subscriber hinter der Bridge, Broker jenseits RabbitMQ.

**Status:** open — Cyclone-Subscriber hinter der Bridge und Broker jenseits RabbitMQ nicht gemessen.
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

16 done / 8 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-amqp-endpoint` — Tests grün, 0 failed.

Offene Punkte: siehe `zerodds-amqp-bridge-daemon-1.0.open.md`. Decision-Records: keine.
