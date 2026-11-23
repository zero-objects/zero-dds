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

**Status:** n/a (informative) — Zuordnung der Levels; die Anforderungen sind die Items §1a und §2 bis §12.

### §1a L6 Multi-Tenant

**Spec:** §1 L6, §1.1 — mehrere DomainParticipants je Daemon; eine Mandantenbindung je AMQP-Verbindung.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`TenantConfig`, `DaemonConfig::of_tenant`), `crates/amqp-endpoint/src/bridge_daemon.rs` (`run_with_reload`, `Participant`, `admin_catalog`), `crates/amqp-endpoint/src/bridge_daemon/reload.rs` (`reload_all`, `reloaded_part`), `crates/amqp-endpoint/src/daemon_runtime.rs` (`BridgeMetrics::for_tenant`, Label `tenant`; `CatalogTopic::tenant`), `crates/amqp-endpoint/src/bridge_daemon/log.rs` (`set_tenant`, `line`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_1_l6_the_tenants_are_read, amqpbridge_1_l6_a_tenant_that_cannot_be_told_apart_is_refused, amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file}`;
`crates/amqp-endpoint/src/bridge_daemon/log.rs::tests::amqpbridge_1_l6_a_tenants_log_lines_carry_the_tenant`;
`crates/amqp-endpoint/src/bridge_daemon/reload.rs::tests::{amqpbridge_1_l6_a_reload_reaches_each_tenant_from_its_own_entry, amqpbridge_1_l6_a_tenant_removed_at_reload_is_refused_for_it_alone}`;
`crates/amqp-endpoint/src/bridge_daemon.rs::tests::amqpbridge_1_l6_the_admin_catalog_lists_the_tenants_topics`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_1_l6_each_tenant_has_its_participant_and_its_connection, amqpbridge_1_l6_a_refused_tenant_stops_the_daemon}`;
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_1_l6_each_tenant_has_its_participant_and_its_connection_at_rabbitmq`.

**Status:** done — jeder Mandant hat einen Teilnehmer auf seiner Domain und eine eigene Verbindung (Container-Id,
`open.hostname`, SASL-Identität, TLS-Client-Zertifikat); ein Sample seiner Domain geht nur über seine Verbindung,
die Metriken tragen das Label `tenant`, die Logzeilen das Feld `tenant`, `/catalog` des Admin-Endpunkts führt die
Topics jedes Mandanten mit dem Feld `tenant`. Ein doppelter Name oder eine doppelte Container-Id wird mit dem Pfad
des zweiten Eintrags (`tenants[i].name`, `tenants[i].amqp.container_id`), Datei und Zeile abgewiesen. SIGHUP lädt
jeden Mandanten aus dem gleichnamigen Eintrag; ein entfernter Mandant bekommt `reload_refused` und läuft weiter.
Ein Mandant, den der Broker abweist, beendet den Daemon mit Exit 5. Gegen RabbitMQ 4.0.5 gemessen: zwei Benutzer, zwei Verbindungen, je ein Sample in
der Queue des eigenen Mandanten.

## §2 CLI-Surface

### §2 zerodds-amqp-bridged CLI

**Spec:** §2 — Optionen `--config`/`--broker`/`--container-id`/
`--domain`/`--sasl-mechanism`/`--user`/`--password`/`--tls-*`/`--topic`/
`--log-level`/`--metrics`/`--version`/`--help`; Exit-Codes 0/1/2/3/4/5/6.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`; `crates/amqp-endpoint/src/bridge_daemon.rs`
(`run_with_reload`, `is_refusal`, `connect_exit_code`, `default_container_id`);
`crates/dcps/src/participant.rs::host_name`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::amqpbridge_2_version_flag_emits_one_line_and_exits_zero`,
`::amqpbridge_2_missing_broker_url_fails_with_exit_1`,
`::amqpbridge_2_plain_without_tls_is_refused_towards_a_remote_broker` (Exit 1),
`::amqpbridge_2_daemon_with_no_broker_fails_with_exit_2` (Exit 2 mit `--once`),
`::amqpbridge_2_a_participant_that_cannot_start_is_exit_3`,
`::amqpbridge_2_a_tls_handshake_that_fails_is_exit_4` (Zertifikat einer fremden CA, ohne `--once`, ein Versuch),
`::amqpbridge_2_a_refused_sasl_exchange_is_exit_5_without_once`,
`::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name` (Exit 5),
`::amqpbridge_2_a_refused_open_is_exit_6` (`close` auf das `open`),
`::amqpbridge_2_topic_flag_adds_a_mapping_and_sigterm_exits_0` (SIGTERM: DETACH, END, CLOSE, Exit 0),
`::amqpbridge_2_set_and_check_config_print_the_origin`,
`::amqpbridge_2_check_config_masks_the_cbs_secret` (`amqp.cbs.secret` aus Datei und `--set` als `***`, `amqp.cbs.ttl_s` sichtbar),
`::amqpbridge_2_help_names_the_environment_of_both_secrets`;
`crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::amqpbridge_2_the_default_container_id_names_the_host`, `crates/amqp-endpoint/src/bridge_daemon.rs::tests::amqpbridge_2_a_refusal_is_told_from_a_broken_connection`;
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_2_an_unknown_vhost_is_exit_6` (RabbitMQ setzt die Verbindung nach dem `open` zurück).

**Status:** done — Exit 0 bis 6 mit Tests, grün auf dem Linux-Testhost. Eine TCP-Störung wird ohne `--once`
nach dem Back-off wiederholt; TLS, SASL und `open` beenden den Daemon beim ersten Connect mit 4, 5, 6.
Ohne `--container-id` ist die `container-id` `zerodds-bridge-<host>`, der Host-Name aus `gethostname`.

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`amqp`/`topics`/`acl`/`metrics`/`catalog`;
ENV-Substitution `${VAR}` und `${VAR:-default}`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`DaemonConfig::from_config`, `read_topic`),
`crates/config/src/parse.rs` (gemeinsamer Parser, flache Flow-Maps).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file, amqpbridge_3_what_the_daemon_would_not_honour_is_refused, amqpbridge_3_the_shipped_example_reads_as_it_says, amqpbridge_3_values_the_shared_parser_reads_reach_the_daemon, amqpbridge_3_a_type_file_the_topic_would_not_read_is_refused}`;
`crates/config/tests/layering.rs::{a_flat_flow_map_is_a_map, a_hash_inside_quotes_is_part_of_the_value, environment_references_expand_before_the_parse}`;
`sender.rcv_settle_mode: 1` (Part 2 §2.8.3, `second`): `crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_3_mode_second_settles_after_the_outcome_as_sender, amqpbridge_3_mode_second_settles_after_the_outcome_as_receiver}`,
`crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_3_a_disposition_in_mode_second_leaves_the_delivery_unsettled`.

**Status:** done — der Test liest den YAML-Block von §3 aus der Datei der Spezifikation (drei Topics, `keyed`,
`type_file`, ACL-Regel, `catalog.address`, `metrics.listen`, `${AMQP_USER}`/`${AMQP_PASSWORD}`).
`rcv_settle_mode: 1` ist in beiden Richtungen gebaut und gegen die Gegenseite im Test geprüft; kein Broker der Matrix ist damit gemessen.

## §4 AMQP-Wire-Protocol

### §4.1 Connection-Open mit Properties + Capabilities

**Spec:** §4.1 — `OPEN` mit container-id/hostname/max-frame-size/
channel-max/idle-time-out/properties/desired-+offered-capabilities.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`connect`, `open_body`), `crates/amqp-endpoint/src/bridge_daemon.rs::run`.

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_4_1_the_open_carries_the_fields_of_the_spec` (zehn Felder, idle-time-out 0 fehlt,
Arrays der Capabilities, Properties mit Schlüsseln vom Typ symbol);
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_4_1_the_daemon_opens_with_its_container_id_and_bridge_properties` (über `run`,
Werte aus der Konfiguration, `hostname` = Host des Brokers);
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_1_rabbitmq_lists_the_bridge_properties_of_the_open`
(`rabbitmqctl list_connections protocol client_properties`: `{1, 0}` und beide Properties; RabbitMQ 4.0.5
kennt für `list_connections` keinen Schlüssel `container_id`).

**Status:** done

### §4.2 Session + Link-Setup ATTACH

**Spec:** §4.2 — `ATTACH` pro Richtung mit name/source/target,
settle-modes, properties, desired-capabilities; eine Session pro Link.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`BrokerWriter::begin`, `attach`), `crates/amqp-endpoint/src/bridge_daemon/link_state.rs` (`link_spec`,
`LinkState`, `begin_all`, `on_link_frame`).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_4_2_the_attach_carries_name_role_node_settle_modes_properties_and_capability`;
`crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::amqpbridge_4_2_a_link_is_named_container_topic_role`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_4_2_every_link_has_a_session_of_its_own_channel` (Topic mit `bidir` und Catalog: drei `begin` auf drei Kanälen, je ein `attach`),
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_9_3_a_session_the_broker_ends_is_begun_again_after_the_delay`;
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_2_a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(RabbitMQ beendet die Session der abgewiesenen Adresse, die anderen laufen weiter).

**Status:** done

### §4.3 TRANSFER-Frame DDS→AMQP

**Spec:** §4.3 — Message nach DDS-AMQP 1.0 §8: Header, Properties
(`dds:*`-ApplicationProperties), Pass-Through-Body mit
Encapsulation-Header; mehrteilig über `max-frame-size`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/message.rs` (`encode_message`), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`sample_messages`, `drain`), `crates/amqp-endpoint/src/broker.rs` (`BrokerWriter::transfer`),
`crates/amqp-endpoint/src/properties.rs`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/message.rs::tests::{amqpbridge_4_3_a_sample_round_trips_through_the_message_form, amqpbridge_4_3_an_xcdr1_sample_keeps_its_header_and_says_so, amqpbridge_4_3_the_header_and_properties_are_those_of_the_spec}`;
`crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_4_3_a_message_above_the_frame_size_is_split_and_reassembled`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_4_3_a_dispose_carries_the_writer_and_sequence_in_its_message_id, amqpbridge_4_3_a_dispose_from_amqp_is_not_sent_back}`;
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::{amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages, amqpbridge_4_3_a_sample_larger_than_the_frame_size_crosses_in_both_directions}`.

**Status:** done — Header (durable, priority 4, ttl, first-acquirer), Properties (message-id, to, subject,
content-type, creation-time, absolute-expiry-time, group-id nur bei keyed) und `dds:*` Feld für Feld geprüft.
Ein Dispose oder Unregister trägt GUID und Sequenznummer des Writers; einer, den die Bridge selbst aus AMQP
geschrieben hat, geht nicht zurück (der Test schlägt ohne die Prüfung des GUID-Präfixes fehl, einmal geprüft).

### §4.4 TRANSFER-Frame AMQP→DDS + DISPOSITION

**Spec:** §4.4 — Receiver-Link mit `FLOW`-Credit, Body→DDS-Sample,
DISPOSITION `accepted`/`rejected`/`released`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (`deliver`, `settle_inbound`), `crates/amqp-endpoint/src/bridge_daemon/message.rs` (`decode_message`, `split_encapsulation`),
`crates/amqp-endpoint/src/broker.rs` (`BrokerReader::next_event`, Reassemblierung).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_4_a_message_from_pika_reaches_a_reader_and_a_bad_body_does_not`
(accepted und rejected, Queue danach leer),
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_a_sample_larger_than_the_frame_size_crosses_in_both_directions`
(Schleifensperre und mehrteiliger Empfang);
`crates/amqp-endpoint/src/bridge_daemon/disposition.rs::tests::amqpbridge_4_4_a_failed_dds_write_releases_the_delivery` (Writer mit TRANSIENT_LOCAL und
KEEP_ALL an max_samples 1: die zweite Nachricht ist `released`);
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_4_4_credit_is_topped_up_at_half` (Credit 4: kein `flow` nach dem ersten Transfer,
`flow` mit delivery-count 2 und Credit 4 nach dem zweiten).

**Status:** done — `released` gegen einen echten DDS-Writer an seinen RESOURCE_LIMITS; dass ein Broker die
freigegebene Nachricht erneut zustellt, ist nicht gemessen (Verhalten des Brokers).

### §4.5 Disposition-Mapping accepted/rejected/released/modified

**Spec:** §4.5 — Outcome des Brokers auf unbestätigte Deliveries.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/disposition.rs::on_disposition`, `crates/amqp-endpoint/src/bridge_daemon/message.rs::redelivered`,
`crates/amqp-endpoint/src/terminus.rs` (`decode_outcome`, `modified_annotations`), `crates/amqp-endpoint/src/broker.rs` (`Inbound::Disposition::annotations`).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::{amqp10_3_4_the_outcomes_are_their_described_lists, amqpbridge_4_5_the_reader_hands_on_the_annotations_of_modified}`;
`crates/amqp-endpoint/src/bridge_daemon/disposition.rs::tests::{amqpbridge_4_5_released_deliveries_go_out_again_in_order_not_first_acquired, amqpbridge_4_5_modified_delivery_failed_raises_the_delivery_count, amqpbridge_4_5_modified_undeliverable_here_is_not_sent_again, amqpbridge_4_5_modified_annotations_are_combined_with_the_message}`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_4_5_a_released_delivery_is_sent_again_and_dropped_after_five, amqpbridge_4_5_a_rejected_delivery_is_not_sent_again, amqpbridge_4_5_modified_counts_annotates_and_respects_undeliverable_here}`;
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::{amqpbridge_4_5_rabbitmq_releases_an_unroutable_message, amqpbridge_4_5_rabbitmq_refuses_a_message_for_a_full_queue}`;
`accepted` in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages`.

**Status:** done — `released` geht erneut mit `first-acquirer` false, `modified` zusätzlich mit erhöhtem
`delivery-count` (delivery-failed) und zusammengeführten message-annotations; `undeliverable-here` und der
fünfte Versuch verwerfen, zählen und protokollieren. RabbitMQ 4.0.5 gibt `released` für eine Nachricht ohne
Ziel-Queue und `rejected` ohne Fehler für eine volle Queue mit `reject-publish` (gemessen). `modified` von einem
echten Broker ist nicht gemessen, nur gegen den geskripteten Broker; das gehört zur Broker-Matrix (§10a).

## §5 Topic-Mapping

### §5.1 Address

**Spec:** §5.1 — `amqp_address` pro Topic, Broker-spezifisch.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::read_topic`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_5_1_a_topic_without_amqp_address_is_refused, amqpbridge_5_1_the_topic_flag_form_is_dds_equals_address}`;
RabbitMQ-v2-Adressen (`/queues/…`) in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`;
`crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::amqpbridge_5_1_artemis_a_topic_address_reaches_every_subscriber` (Artemis 2.44.0, `topic://`);
`crates/amqp-endpoint/tests/bridge_solace_e2e.rs::amqpbridge_5_1_solace_a_topic_address_reaches_every_subscriber` (Solace PubSub+ Standard 10.26.0.9219, `topic://`; Linux-Testhost grün, der CI-Job ist nicht gelaufen, §10).

**Status:** done — `amqp_address` ist Pflicht, als Map und als `--topic`; die Meldung nennt den Eintrag. Die
Adressformen von ActiveMQ Artemis (`<address>`, `queue://<name>`, `topic://<name>`), Solace PubSub+ (`<queue>`, `queue://<name>`, `topic://<name>`) und Qpid Dispatch gehören zur Broker-Matrix (§10a).

### §5.2 Catalog

**Spec:** §5.2 — Catalog (JSON) pro Verbindung auf eigener Adresse.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`catalog_message`), `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`DaemonConfig::catalog_json`,
`catalog_address`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_2_a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(`pika` liest den Catalog aus `/queues/zd_bridge_catalog`).

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → AMQP-Behavior Map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/
Liveliness/Partition.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`read_topic`: BEST_EFFORT → settled, `qos.deadline_ms`, `qos.liveliness_lease_ms`, `qos.partition`), `TopicMapping::terminus_durable` (terminus durable 1 am Target des Sender-Links bei Durability ≠ VOLATILE, die Source des Receiver-Links bleibt 0), `crates/amqp-endpoint/src/bridge_daemon/message.rs` (`encode_message`:
durable, ttl, `dds:partition`; `annotated`, `decode_message`: `dds:partition`), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`enqueue`: `OutLink::cap`; `look_at_deadline`, `writer_partitions`, Herzschlag-Takt), `crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (`deliver`: Partition-Abgleich), `crates/amqp-endpoint/src/bridge_daemon.rs` (`run_with_reload`, `endpoint_qos`: HISTORY, DEADLINE, LIVELINESS, PARTITION der DDS-Endpunkte), `crates/amqp-endpoint/src/broker.rs::BrokerWriter::heartbeat`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file, amqpbridge_6_the_qos_keys_are_read}`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_6_keep_last_holds_n_and_drops_the_oldest, amqpbridge_6_transient_local_is_a_durable_message_to_a_durable_node, amqpbridge_6_a_heartbeat_goes_out_at_half_the_idle_time_out, amqpbridge_6_a_missed_deadline_is_annotated, amqpbridge_6_the_liveliness_lease_reaches_the_dds_endpoints, amqpbridge_6_partitions_reach_the_reader_and_the_message, amqpbridge_6_a_message_of_another_partition_is_rejected, amqpbridge_6_a_keep_all_topic_from_amqp_keeps_every_sample_for_a_late_reader}`;
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::{amqpbridge_4_2_a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive, amqpbridge_6_rabbitmq_takes_a_transient_local_sample_as_a_persistent_message}`
(LIFESPAN 60 s → `expiration` 60000; TRANSIENT_LOCAL → delivery mode 2).

**Offen:** KEEP_ALL ohne Credit verwirft über 65 536 wartenden Samples das älteste, statt den DDS-Writer
zurückzuhalten. Dafür müsste der DDS-Reader der Laufzeit (`crates/dcps`, `register_user_reader_kind` liefert einen
unbegrenzten `mpsc`-Kanal) das ACKNACK zurückhalten, solange die Bridge die Samples nicht abnimmt.

**Status:** partial — Reliability, Durability (Header und Terminus), KEEP_LAST, Lifespan, Deadline-Annotation
`x-opt-zerodds-deadline-missed`, Liveliness (Lease und Herzschlag) und Partition (Endpunkte, `dds:partition`,
Abweisung) gebaut und auf dem Linux-Testhost grün. Die Lease ist an den DDS-Endpunkten geprüft: ein Writer mit
200 ms matcht den Reader eines 500-ms-Topics, einer mit 2000 ms nicht; ein Reader mit 1000 ms matcht den Writer,
einer mit 300 ms nicht. Der Terminus-Default `durable` 1 gilt nur für das Target des Sender-Links. Der Herzschlag ging vorher erst beim nächsten 250-ms-Takt
hinaus (gemessen 3 statt 5 in 1,6 s bei 600 ms idle-time-out); der Pump wacht jetzt zum fälligen Herzschlag auf.
Offen: Backpressure für KEEP_ALL.

## §7 Security

### §7.1 TLS amqps:// + ALPN

**Spec:** §7.1 — `amqps://` per `amqp.tls.enabled`, ALPN `["amqp"]`.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`Wire::Tls`, `ALPN_AMQP`), `crates/amqp-endpoint/src/bridge_daemon.rs::tls_config`.

**Tests:** `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp`
(der Server liest `amqp` als ausgehandeltes Protokoll).

**Status:** done

### §7.1a ALPN-Aushandlung `["amqp"]`

**Spec:** §7.1 — der TLS-Handshake bietet ALPN `["amqp"]` an.

**Repo:** `crates/amqp-endpoint/src/broker.rs::connect`.

**Tests:** `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp` (der Server liest `amqp` als ausgehandeltes Protokoll).

**Status:** done

### §7.2 SASL PLAIN/SCRAM/EXTERNAL/ANONYMOUS/XOAUTH2

**Spec:** §7.2 — Mechanismus pro Config; PLAIN ohne TLS nur gegen Loopback, ANONYMOUS nur
gegen Loopback.

**Repo:** `crates/amqp-endpoint/src/broker/sasl.rs` (`exchange`, `initial_response`, `scram_client_final_with`), `crates/amqp-endpoint/src/scram.rs` (`ScramHash`), `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`DaemonConfig::sasl`), `crates/amqp-endpoint/src/bridge_daemon.rs` (`Participant::prepare`).

**Tests:** `crates/amqp-endpoint/src/broker/sasl.rs::tests::{amqpbridge_7_2_scram_client_final_matches_the_rfc_7677_example, amqpbridge_7_2_scram_sha_512_authenticates_and_checks_the_signature, amqpbridge_7_2_xoauth2_sends_the_bearer_token_and_reads_the_error, amqpbridge_12_1_sasl_each_mechanism_has_its_initial_response, amqpbridge_12_1_sasl_scram_checks_the_server_signature}`;
`crates/amqp-endpoint/src/scram.rs::tests::{amqpbridge_7_2_scram_sha_512_proof_and_signature, rfc7677_scram_hash_sha256_is_the_rfc_schedule}`;
`crates/amqp-endpoint/tests/broker_tls_sasl.rs::{amqpbridge_7_2_scram_sha_256_authenticates_and_a_wrong_password_is_refused, amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp, amqpbridge_7_2_a_mechanism_the_broker_does_not_offer_is_refused_by_name}`;
`crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::{amqpbridge_7_2_artemis_accepts_scram_sha_256_and_512, amqpbridge_7_2_a_wrong_scram_sha_512_password_is_exit_5}`;
`crates/amqp-endpoint/tests/bridge_brokerj_e2e.rs::amqpbridge_7_2_brokerj_accepts_the_xoauth2_token`;
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_7_2_rabbitmq_accepts_external_with_the_client_certificate` (PLAIN gegen RabbitMQ in den übrigen Tests der Datei);
`crates/amqp-endpoint/tests/bridge_e2e.rs::{amqpbridge_2_plain_without_tls_is_refused_towards_a_remote_broker, amqpbridge_7_2_anonymous_towards_a_remote_broker_is_refused, amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name}`.
Gemessen auf dem Linux-Testhost: Apache ActiveMQ Artemis 2.44.0 (`SCRAMPropertiesLoginModule`, Akzeptor nur mit SCRAM-SHA-256 und SCRAM-SHA-512) nimmt beide an, ein falsches Passwort ist Exit-Code 5;
RabbitMQ 4.0.5 mit `rabbitmq_auth_mechanism_ssl` nimmt EXTERNAL mit dem Client-Zertifikat an, ein Zertifikat ohne Benutzer ist Exit-Code 5.
Apache Qpid Broker-J 9.2.1 (OAuth2-Provider, Identitäts-Resolver `GitHubUser` gegen einen HTTPS-Endpunkt im Test) nimmt das XOAUTH2-Token an und lehnt ein unbekanntes Token in der SASL-Stufe ab.
Der SHA-512-Beweis ist gegen Werte aus CPython `hashlib`/`hmac` geprüft.

**Status:** done — PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, EXTERNAL, ANONYMOUS und XOAUTH2; SCRAM und EXTERNAL gegen Artemis und RabbitMQ, XOAUTH2 gegen Broker-J gemessen.
### §7.3 ACL Daemon-Side

**Spec:** §7.3 — Subject = SASL-Identity oder TLS-DN; Filter vor TRANSFER.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/acl.rs` (`allowed`, `acl_allows`, `subjects_of`, `Live`), `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`acl.rules`, `acl.default_deny`),
`crates/amqp-endpoint/src/bridge_daemon.rs` (`Participant::prepare`, `Participant::run`: keine DDS-Entity und kein Link für eine verweigerte Richtung),
`crates/amqp-endpoint/src/bridge_daemon/reload.rs::apply_reload` (Subjekte eines neuen Zertifikats), `crates/amqp-endpoint/src/bridge_daemon/disposition.rs::on_transfer`, `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::{pump, drain}` (nach dem Entzug des Leserechts verwirft `drain` die auf Credit wartenden und die vom Broker freigegebenen Nachrichten mit `acl_denied`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_read_and_write_are_separate_lists`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_the_wildcard_admits_every_subject`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_default_deny_decides_a_topic_no_rule_names`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_the_configured_rules_decide`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_live_rules_follow_a_reload`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::{amqpbridge_7_3_external_takes_the_certificate_dn, amqpbridge_7_3_the_sasl_user_is_the_subject, amqpbridge_7_3_a_reloaded_certificate_is_the_new_subject}`
(Lese- und Schreibliste getrennt, `*` für jedes Subjekt, `default_deny`, die konfigurierten Regeln, Reload; mit EXTERNAL der Subject-DN nach RFC 4514 und der Fingerabdruck `mtls:<SHA-256>` des Zertifikats aus `amqp.tls.cert_file`, der CN allein genügt nicht; Benutzer von PLAIN, SCRAM, XOAUTH2; ein neues Zertifikat beim Reload ist das neue Subjekt),
`crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::amqpbridge_3_the_shipped_example_reads_as_it_says` (Regeln gelesen);
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::tests::amqpbridge_7_3_dds_pump_a_withdrawn_read_right_drops_the_queue`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_7_3_a_direction_denied_at_start_moves_no_data, amqpbridge_7_3_a_write_right_withdrawn_by_reload_rejects_the_transfer, amqpbridge_7_3_a_read_right_withdrawn_by_reload_stops_the_queued_messages}`
(skriptierter Broker: `read: []` attacht nur den empfangenden Link und meldet keinen DDS-Reader an, `write: []` nur den sendenden ohne DDS-Writer; nach dem Reload `rejected` mit `amqp:unauthorized-access`, nicht nach DDS geschrieben; nach dem Entzug des Leserechts geht weder eine auf Credit wartende noch eine vom Broker freigegebene Nachricht hinaus, als der Broker Credit gibt);
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_topic`. Linux-Testhost: grün.

**Status:** done — Subjekt ist der SASL-Benutzer oder bei EXTERNAL der Subject-DN und Fingerabdruck des Client-Zertifikats; eine verweigerte Richtung hat weder DDS-Entity noch Link; ein per Reload entzogenes Leserecht hält auch die schon aus DDS genommenen Nachrichten des Topics zurück.
## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log auf stdout mit timestamp/level/event/… und
`--log-level`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/log.rs` (`line`, `emit`, `log`, `event_level`, `set_participant`, `escape`, `rfc3339`),
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`sent`, `round_trip_out`: `transfer_out`, `disposition`),
`crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (`on_transfer`: `transfer_in`),
`crates/amqp-endpoint/src/bridge_daemon.rs` (`log_event`, `set_log_level`), `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` (Zeilen des Programms).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/log.rs::tests::{amqpbridge_8_1_a_line_is_a_json_object_with_the_spec_fields, amqpbridge_8_1_the_log_level_is_the_threshold, amqpbridge_8_1_control_characters_are_escaped, amqpbridge_8_1_every_event_has_a_level_and_the_threshold_orders_them, rfc3339_5_6_the_utc_date_and_time}`;
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::tests::amqpbridge_8_1_key_hash_unavailable_is_logged_once_per_writer_with_its_fields` (`key_hash_unavailable`: Stufe `warn`, `dds_topic`, `writer`, `type_file`, einmal je Schreiber);
`crates/amqp-endpoint/tests/bridge_e2e.rs::{amqpbridge_8_1_8_3_the_log_lines_on_stdout_and_the_spans_of_a_transfer, amqpbridge_8_1_transfer_out_and_disposition_lines_of_both_settle_modes}`
(Programm gegen den skriptierten Broker, `log_level: debug`: jede Zeile auf stdout beginnt mit `timestamp`; jede Zeile außer denen des Programms (`startup`, `admin_endpoint`, `admin_endpoint_bind_failed`, `signal_watcher_init_failed`) trägt `container_id`, `startup` nennt sie als Feld;
`attached` mit `link_name`, `dds_topic`, `amqp_address`; `transfer_in` mit `delivery_id`, `bytes`, `outcome`, `latency_us`;
DDS → AMQP mit DDS-Writern des Tests: `transfer_out` mit `link_name`, `dds_topic`, `amqp_address`, `delivery_id`, `bytes`, dazu `disposition` mit `outcome` `accepted` auf dem unsettled Link und `settled` auf dem Link, der settled sendet, je mit `latency_us`;
`link_detached` mit `link_name`, `dds_topic`, `amqp_address` und der Bedingung des Brokers; keine Logzeile auf stderr).
Linux-Testhost: grün.

**Status:** done — Zeilen auf stdout mit `timestamp`, `level`, `event`, `tenant`, `container_id` und den Feldern des Ereignisses;
`transfer_in`, `transfer_out` und `disposition` auf Stufe `debug` mit `delivery_id`, `bytes`, `latency_us`.
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — 13 Familien `zerodds_amqp_bridge_*` mit Labels.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (`BridgeMetrics`),
Zählstellen in `crates/amqp-endpoint/src/bridge_daemon.rs`,
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`pump`, `drain`, `enqueue`, `topic_metrics`),
`crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (`count_outcome`, `on_transfer`, `settle_inbound`, `on_disposition`, `deliver`),
`crates/amqp-endpoint/src/bridge_daemon/link_state.rs` (`top_up`, `on_link_frame`),
`crates/amqp-endpoint/src/peer.rs::condition::DEFINED`.

**Tests:** `crates/amqp-endpoint/src/daemon_runtime.rs::tests::amqpbridge_8_2_the_reason_label_is_the_condition_or_other`
(`amqp:decode-error`, `amqp:transaction:rollback`, `amqp:session:window-violation`, Symbol eines Herstellers, keine Condition, nicht definierte `amqp:`-Symbole),
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_8_2_the_metrics_count_transfers_dispositions_and_samples`
(Linux-Testhost, RabbitMQ 4.0.5: 3 Samples hinaus, eine gute und eine kaputte
Nachricht herein; Transfers, Samples und Dispositionen pro Topic,
`rejected{reason="amqp:decode-error"}` 1).

**Status:** done — alle 13 Familien; Dispositionen zählen in beiden
Richtungen, `reason` ist die AMQP-Condition, wenn AMQP 1.0 sie definiert (Part 2 §2.8.15 bis §2.8.18, Part 4 §4.5.8), sonst `other`, auch ohne Condition.
Daneben bleiben die generischen `zerodds_amqp_*`.
### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert
TRANSFER-Roundtrip-Span.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (`otlp_config_from_env`, `spawn_otlp_flush_loop`),
`crates/amqp-endpoint/src/bridge_daemon/trace.rs` (`Tracer`, `spans`, `record`),
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::round_trip_out`, `crates/amqp-endpoint/src/bridge_daemon/disposition.rs::on_transfer`,
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` (Exporter an `run_with_reload`, letzter Flush beim Ende).

**Tests:** `crates/amqp-endpoint/src/daemon_runtime.rs::tests::amqpbridge_8_3_otlp_config_parses_endpoint`;
`crates/amqp-endpoint/src/bridge_daemon/trace.rs::tests::{amqpbridge_8_3_an_inbound_transfer_is_a_span_with_a_dds_write_child, amqpbridge_8_3_an_outbound_transfer_ends_with_its_disposition, amqpbridge_8_3_a_rejected_transfer_has_status_error, amqpbridge_8_3_a_tracer_without_exporter_is_off, amqpbridge_8_3_span_ids_differ}`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_8_3_the_spans_of_a_transfer_reach_the_collector`
(beide Richtungen, ein OTLP/HTTP-Empfänger im Test: drei `amqp.transfer`, ein `dds.read`, ein `dds.write`, `amqp:decode-error` als Bedingung);
`crates/amqp-endpoint/tests/bridge_e2e.rs::amqpbridge_8_1_8_3_the_log_lines_on_stdout_and_the_spans_of_a_transfer` (das Programm mit
`OTEL_EXPORTER_OTLP_ENDPOINT`). Linux-Testhost: grün. Gegen einen OpenTelemetry Collector nicht gemessen; der Empfänger im Test prüft die Spans im JSON von `zerodds-observability-otlp`.

**Status:** done — pro TRANSFER-Roundtrip ein Trace `amqp.transfer` mit `dds.write` (AMQP → DDS) oder `dds.read` (DDS → AMQP).
## §9 Lifecycle

### §8.3a TRANSFER-Roundtrip-Span

**Spec:** §8.3 — bei gesetztem `OTEL_EXPORTER_OTLP_ENDPOINT` entsteht pro
TRANSFER ein Roundtrip-Span.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/trace.rs`, wie §8.3.

**Tests:** `crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_8_3_the_spans_of_a_transfer_reach_the_collector`;
`crates/amqp-endpoint/tests/bridge_e2e.rs::amqpbridge_8_1_8_3_the_log_lines_on_stdout_and_the_spans_of_a_transfer`. Linux-Testhost: grün.

**Status:** done — der Emit-Teil von §8.3, dort beschrieben.
### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → Signal-Handler → TLS → DCPS → Reader/Writer →
Open + SASL + TLS → Session + Attach.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` (Konfiguration, dann `install_signal_watcher`);
`crates/amqp-endpoint/src/bridge_daemon.rs::Participant::prepare` (TLS, Log `tls_loaded`),
`crates/amqp-endpoint/src/bridge_daemon.rs::Participant::run` (DCPS `participant_started`, Reader und Writer `endpoints_registered`,
`broker::connect` `connected`, `begin_all` und die Attaches `attached`).

**Tests:** `crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_9_1_the_dds_endpoints_stand_before_the_broker_is_contacted`; `crates/amqp-endpoint/tests/bridge_e2e.rs::{amqpbridge_9_1_the_log_lines_follow_the_steps_and_sigint_exits_0, amqpbridge_9_1_tls_material_that_does_not_load_stops_before_dds, amqpbridge_9_1_tls_loaded_comes_between_startup_and_the_participant}`. Linux-Testhost: grün.

**Status:** done — ein zweiter DDS-Teilnehmer matcht Reader und Writer der Bridge, während auf dem Port des Brokers
niemand lauscht; danach folgen SASL-Header und -Init, AMQP-Header und Open, je Link Begin vor Attach. Die Log-Zeilen
des Programms stehen in der Reihenfolge der Schritte: ohne TLS `startup`, `participant_started`, `endpoints_registered`,
`connected`, `attached`; mit TLS-Material, das lädt, steht `tls_loaded` (mit `ca_file`) nach `startup` und vor
`participant_started`, gemessen gegen einen TLS-Server mit einem Zertifikat dieser CA. TLS-Material, das nicht lädt, endet mit Exit 1 ohne
`participant_started` und ohne Verbindung zum Broker. Abweichung vom ursprünglichen Text: Die Signal-Handler stehen
direkt nach der Konfiguration (Spec §9.1 Schritt 1), damit ein Stopp während TLS, DDS-Start, Connect oder Back-off der
Shutdown von §9.2 ist.

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, DETACH/END/CLOSE; SIGHUP
TLS+ACL-Reload.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::pump` (`DaemonConfig::drain_budget`, Vorgabe `DRAIN_BUDGET` 30 s;
`drain_started`, `shutdown_with_unsettled`, `shutdown_with_queued`; mit dem Beginn des Drains `OutLink::stopping`),
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::drain` (nach dem Stopp geht keine Nachricht der Warteschlange
hinaus, welchen Credit der Broker auch gewährt), `crates/amqp-endpoint/src/bridge_daemon.rs::Participant::run` (DETACH/END/CLOSE,
`left_at_stop` für einen Stopp ohne Verbindung); `crates/amqp-endpoint/src/daemon_runtime.rs::install_signal_watcher`
(Unix: SIGTERM, SIGINT, SIGHUP; Windows: Console-Control-Handler über `ctrlc`);
`crates/amqp-endpoint/src/bridge_daemon.rs::run_with_reload` (`Watchers`: die Threads für STOPPING=1, SIGHUP und den
gemeinsamen Stopp der Tenants enden mit dem Aufruf, auf jedem Rückweg; `watch_stopping`), `crates/amqp-endpoint/src/bridge_daemon/reload.rs` (`reload_all`, `apply_reload`;
ACL und das TLS-Material des nächsten Connects je Teilnehmer; entzogene Rechte gelten
sofort, neu erteilte nach einem Neustart).

**Tests:** `crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_9_2_the_drain_waits_for_the_disposition_then_detaches_ends_and_closes, amqpbridge_9_2_the_drain_gives_up_after_its_budget_and_counts_what_is_left, amqpbridge_9_2_a_sample_written_during_the_drain_is_not_sent, amqpbridge_9_2_a_message_waiting_for_credit_is_counted_at_the_stop, amqpbridge_9_2_a_stop_without_a_broker_connection_counts_in_errors_total, amqpbridge_9_2_a_reload_gives_the_next_connect_the_new_certificate, amqpbridge_9_2_the_reload_watcher_ends_with_the_run, amqpbridge_9_2_credit_granted_during_the_drain_sends_no_queued_message, amqpbridge_9_2_a_message_released_during_the_drain_is_not_sent_again}`; `crates/amqp-endpoint/src/bridge_daemon.rs::tests::{amqpbridge_9_2_a_stop_after_the_run_sends_no_stopping, amqpbridge_9_2_a_stop_before_the_end_of_the_run_sends_stopping_once}`; `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::amqpbridge_9_2_a_link_asked_to_drain_after_the_stop_consumes_its_credit`; `crates/amqp-endpoint/tests/bridge_e2e.rs::{amqpbridge_2_topic_flag_adds_a_mapping_and_sigterm_exits_0, amqpbridge_9_1_the_log_lines_follow_the_steps_and_sigint_exits_0, amqpbridge_9_2_a_stop_without_a_broker_connection_counts_the_samples_left, amqpbridge_9_2_the_stop_line_counts_a_message_waiting_for_credit, amqpbridge_9_2_the_stop_line_counts_a_sample_delivered_during_the_drain}`; `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_topic` (RabbitMQ 4.0.5). Linux-Testhost: grün.

**Status:** done — der Drain wartet auf die Disposition der laufenden Zustellung und sendet dann DETACH, END und CLOSE;
ohne Disposition endet er nach seinem Budget und zählt den Rest; ein Sample nach dem Stopp und eine Nachricht ohne Credit
gehen nicht hinaus und werden gezählt (Zeile `shutdown_with_queued` des Programms: `count` 1 und `not_taken` 0 für die
Nachricht ohne Credit, `count` 0 und `not_taken` 1 für das Sample während des Drains); Credit, den der Broker
während des Drains gewährt, sendet keine Nachricht, die vor dem Stopp auf Credit wartete, und eine Zustellung, die der
Broker während des Drains mit `released` zurückgibt, wartet wieder auf Credit und wird nicht erneut gesendet; beide
zählen in `shutdown_with_queued`; ein Link, den der Broker nach dem Stopp leeren lässt (`drain`), hat keine Nachricht
verfügbar und verbraucht seinen Credit (AMQP 1.0 Part 2 §2.6.7); die Zeile `drain_started` nennt
beim Beginn des Drains `in_flight` und `queued`; ein Stopp ohne Broker-Verbindung
(im Back-off) endet sofort mit Exit 0, zählt die vom DDS-Reader gelieferten Samples als `not_taken` und erhöht
`errors_total` um eins (`left_at_stop`); SIGTERM und SIGINT enden mit Exit 0; nach einem Reload zeigt der nächste
Connect das neue Client-Zertifikat; die Threads von `run_with_reload`, die den Stopp beobachten, enden mit dem
Aufruf, auch bei einem Fehler ohne Stopp: ein späterer Reload wird nicht angewandt, ein späterer Stopp sendet kein
STOPPING=1. Windows: CTRL_C und CTRL_BREAK lösen den Stopp mit Drain aus. Schließen der Konsole,
Abmeldung und Herunterfahren setzen den Stopp ebenfalls, doch der Handler von `ctrlc` kehrt sofort zurück und Windows
beendet den Prozess vor dem Drain; Spec §9.2 und die Manpages sagen das so. Der Handler ist für
`x86_64-pc-windows-gnu` gebaut (`cargo check`), nicht auf einem Windows-Host ausgeführt: kein Windows-Host.

### §9.3 Reconnect

**Spec:** §9.3 — Exponential-Backoff; unbestätigte Deliveries erneut.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/link_state.rs` (`next_delay`, Requeue in `connection_lost` und `out_link_down`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::amqpbridge_9_3_the_delay_grows_by_the_factor_up_to_the_maximum`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_9_3_unsettled_deliveries_go_out_again_first_and_in_order, amqpbridge_9_3_a_session_the_broker_ends_is_begun_again_after_the_delay}`;
gegen RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_9_3_after_the_broker_closes_the_connection_the_bridge_reconnects`
(`rabbitmqctl close_all_connections`; das Sample vor und das nach dem Schließen kommen an).

**Status:** done — drei unbestätigte Deliveries gehen nach dem Verbindungsabbruch zuerst und in Reihenfolge
erneut hinaus, vor einem danach geschriebenen Sample; eine vom Broker beendete Session wird nach dem Back-off
auf ihrem Kanal neu begonnen. Ein Neustart von RabbitMQ während des Schreibens ist nicht gemessen.

## §10 Cross-Vendor

### §10 RTPS-Peer + RabbitMQ/Artemis/Qpid/Solace/ServiceBus

**Spec:** §10 — normaler RTPS-Peer; AMQP-Seite gegen fünf Broker.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs`, `crates/amqp-endpoint/src/broker.rs`, `crates/amqp-endpoint/src/coexistence.rs`, `crates/amqp-endpoint/src/bridge_daemon/cbs.rs`; der Testaufbau `crates/amqp-endpoint/tests/support/broker_matrix.rs`, `crates/amqp-endpoint/tests/support/proton_peer.py`, `crates/amqp-endpoint/tests/support/artemis.rs`, `crates/amqp-endpoint/tests/support/dispatch.rs`, `crates/amqp-endpoint/tests/support/solace.rs`, `tests/integration/provision.sh` (`artemis`, `dispatch`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_4_a_message_from_pika_reaches_a_reader_and_a_bad_body_does_not`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_a_sample_larger_than_the_frame_size_crosses_in_both_directions` (RabbitMQ 4.0.5); `crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::{amqpbridge_10_artemis_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_artemis_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_artemis_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_artemis_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_artemis_a_refused_address_leaves_the_other_links_running}` (Artemis 2.44.0); `crates/amqp-endpoint/tests/bridge_dispatch_e2e.rs::{amqpbridge_10_dispatch_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_dispatch_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_dispatch_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_dispatch_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_dispatch_a_refused_address_leaves_the_other_links_running}` (Qpid Dispatch 1.19.0); `crates/amqp-endpoint/tests/bridge_solace_e2e.rs::{amqpbridge_10_solace_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_solace_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_solace_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_solace_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_solace_a_refused_address_leaves_the_other_links_running}` (Solace PubSub+ Standard 10.26.0.9219); qpid-proton 0.37 als Client am Broker; Jobs `integration-bridges-vendor-amqp` und `integration-bridges-vendor-amqp-solace`. Der RTPS-Peer mit einem Schreiber eines anderen Herstellers: `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_12_3_a_cyclone_writer_reaches_the_queue` (Cyclone DDS 11.0.1, Job `vendor-live-dispose-bridges`). Linux-Testhost: grün.

**Offen:** Azure Service Bus. Der Emulator verlangt die Annahme seiner Lizenzbedingungen (`ACCEPT_EULA`), die nicht erteilt ist; ein Azure-Namespace ist nicht vorhanden. Die Claims-based Security für Azure Service Bus ist gebaut (§10a). Der Job `integration-bridges-vendor-amqp-solace` ist in CI nicht gelaufen: Solace gibt es nur als Container-Image, im Job ein Service-Container. Gemessen auf dem Linux-Testhost: Solace PubSub+ Standard 10.26.0.9219 startet mit 1 GB `/dev/shm` und einem harten `nofile`-Limit von 1048576; mit 64 MB `/dev/shm` (Vorgabe von Docker) oder einem harten Limit von 524288 endet es in seiner Plattformprüfung (`POST Violation [022]`). GitLab Runner gibt einem Service-Container das `shm_size` des Runners und keine ulimits; der Job braucht daher einen Runner mit `shm_size` von mindestens 1 GiB und einen Docker-Daemon, dessen voreingestelltes hartes `nofile`-Limit mindestens 1048576 beträgt.

**Status:** partial — RabbitMQ 4.0.5, Artemis 2.44.0, Qpid Dispatch 1.19.0 und Solace PubSub+ Standard 10.26.0.9219 auf dem Linux-Testhost gemessen; ein Cyclone-Schreiber vor der Bridge gemessen (§12.3); der Solace-Job in CI nicht gelaufen (Runner-Konfiguration); Azure Service Bus nicht gemessen (Lizenzannahme für den Emulator).

### §10a Broker-Matrix Artemis/Qpid-Dispatch/Solace/Azure ServiceBus

**Spec:** §10 — die AMQP-Seite gegen Artemis, Qpid-Dispatch, Solace und
Azure ServiceBus.

**Repo:** wie §10; `put-token` auf `$cbs` in `crates/amqp-endpoint/src/bridge_daemon/cbs.rs` (Schlüssel `amqp.cbs.*`).

**Tests:** `crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::{amqpbridge_10_artemis_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_artemis_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_artemis_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_artemis_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_artemis_a_refused_address_leaves_the_other_links_running}`; `crates/amqp-endpoint/tests/bridge_dispatch_e2e.rs::{amqpbridge_10_dispatch_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_dispatch_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_dispatch_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_dispatch_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_dispatch_a_refused_address_leaves_the_other_links_running}`; `crates/amqp-endpoint/src/bridge_daemon/cbs.rs::tests::{amqpbridge_10_the_sas_token_matches_the_documented_algorithm, amqpbridge_10_the_put_token_request_names_operation_type_audience_and_reply_to, amqpbridge_10_an_answer_gives_its_correlation_id_and_status, amqpbridge_10_the_audience_of_an_address, amqpbridge_10_percent_encoding_as_quote_plus_and_quote}`; `crates/amqp-endpoint/src/bridge_daemon.rs::tests::amqpbridge_10_cbs_keys_and_the_anonymous_rule`; `crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_10_cbs_a_token_is_put_before_the_link_and_renewed, amqpbridge_10_cbs_a_refused_token_ends_the_connection_before_the_link, amqpbridge_10_cbs_a_put_token_without_answer_ends_the_connection, amqpbridge_10_cbs_a_renewal_without_answer_ends_the_connection}`; `crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::amqpbridge_5_1_artemis_a_topic_address_reaches_every_subscriber`; `crates/amqp-endpoint/tests/bridge_solace_e2e.rs::{amqpbridge_10_solace_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_solace_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_solace_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_solace_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_solace_a_refused_address_leaves_the_other_links_running}`, `crates/amqp-endpoint/tests/bridge_solace_e2e.rs::amqpbridge_5_1_solace_a_topic_address_reaches_every_subscriber`. Linux-Testhost: grün.

**Offen:** Azure Service Bus (Emulator oder Namespace) nicht gemessen, Grund wie §10: die Lizenzannahme für den Emulator ist nicht erteilt. Die CBS-Tests laufen gegen die Broker-Seite im Test, nicht gegen Azure. Der Solace-Job ist in CI nicht gelaufen (Runner-Konfiguration, §10).

**Status:** partial — Artemis, Qpid Dispatch und Solace PubSub+ Standard 10.26.0.9219 mit den fünf Fällen der Matrix gemessen (Solace: `<queue>` und `queue://<name>` in der Matrix, die Queues legt der Test über SEMP v2 an; abgelehnt wird der Empfänger einer Queue mit `no-access`, weil Solace jeden Sender annimmt; `topic://<name>` mit zwei Empfängern, von denen jeder jedes Sample erhält; die Bridge über TLS mit SASL PLAIN; Artemis: `<address>` und `queue://<name>` in der Matrix, `topic://<name>` mit zwei Empfängern, von denen jeder jedes Sample erhält; ein Neustart des Brokers; Dispatch: der Empfänger zuerst, das Ergebnis der Bridge über den Router zurück; Dispatch 1.19.0 gegen CPython 3.11 gebaut, weil es unter Python 3.13 mit SIGSEGV endet). CBS gebaut, gegen Azure nicht gemessen; eine Runde von `put-token`, die nicht binnen des Connect-Time-outs beantwortet ist (erste Runde und Erneuerung), beendet die Verbindung, der Back-off folgt.
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

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::amqpbridge_3_the_shipped_example_reads_as_it_says`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does, every_key_the_daemon_reads_is_in_the_man_page}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel; jeder Schlüssel, den der Parser in seinen
`.only(&[...])`-Listen und in `KEYS` liest, steht in der Handbuchseite: der Abschnitt `catalog` und
`topics[].type_file` fehlten vorher in Seite und Beispiel).

**Offen:** der Windows-Dienst. `Install-Services.ps1` registriert das Konsolenprogramm mit `sc.exe create`;
das Programm hat keinen Service Control Dispatcher (`StartServiceCtrlDispatcher`), und
`install_signal_watcher` ist außerhalb von Unix ein leerer Thread. Start und Stopp durch den Dienststeuerungs-Manager
werden nicht behandelt. Es fehlt ein Windows-Host zum Bauen und Testen; der wöchentliche Windows-Runner baut
`zerodds-amqp-endpoint` nicht.

**Status:** partial — Linux, macOS und Docker wie oben; der Windows-Dienst reagiert nicht auf den Dienststeuerungs-Manager (kein Windows-Host, nicht gebaut).

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/amqp_codec/link_state/disposition/sasl/
dds_pump/acl je ≥ 5 Tests.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs` (config), `crates/amqp-endpoint/src/broker.rs` (amqp_codec: `BrokerWriter`, `BrokerReader`, `FrameReader` über dem Codec von `crates/amqp-bridge/`), `crates/amqp-endpoint/src/bridge_daemon/link_state.rs` (link_state), `crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (disposition), `crates/amqp-endpoint/src/broker/sasl.rs` (sasl, die Client-Seite), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (dds_pump), `crates/amqp-endpoint/src/bridge_daemon/acl.rs` (acl).

**Tests:** config (8): `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_3_what_the_daemon_would_not_honour_is_refused, broker_urls, amqpbridge_3_the_shipped_example_reads_as_it_says, amqpbridge_3_values_the_shared_parser_reads_reach_the_daemon, amqpbridge_2_the_default_container_id_names_the_host, amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file, amqpbridge_5_1_a_topic_without_amqp_address_is_refused, amqpbridge_5_1_the_topic_flag_form_is_dds_equals_address}`, amqp_codec (11): `crates/amqp-endpoint/src/broker.rs::tests::{amqp10_3_4_the_outcomes_are_their_described_lists, amqpbridge_4_3_a_message_above_the_frame_size_is_split_and_reassembled, amqp10_4_4_1_a_posted_message_names_the_transaction_on_every_frame, amqpbridge_4_1_the_open_carries_the_fields_of_the_spec, amqpbridge_4_2_the_attach_carries_name_role_node_settle_modes_properties_and_capability, amqpbridge_3_a_disposition_in_mode_second_leaves_the_delivery_unsettled, amqp10_4_2_the_attach_of_a_control_link_has_a_coordinator_as_its_target, amqp10_4_4_dispositions_and_flows_name_the_transaction, amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states, amqp10_4_5_4_the_reader_takes_longer_txn_ids_only_when_told, a_full_session_window_refuses_the_transfer}`, link_state (12): `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::{amqpbridge_12_1_link_state_begin_all_begins_every_link_that_is_down, amqpbridge_12_1_link_state_the_brokers_begin_is_answered_with_the_attach, amqpbridge_12_1_link_state_an_attach_brings_the_link_up_and_a_receiver_grants_credit, amqpbridge_12_1_link_state_an_attach_without_a_node_sends_nothing, amqpbridge_12_1_link_state_credit_follows_part_2_2_6_7, amqpbridge_12_1_link_state_a_detach_is_answered_and_the_session_ended, amqpbridge_12_1_link_state_an_end_takes_the_link_down_until_the_back_off, amqpbridge_12_1_link_state_retry_due_begins_again_after_the_back_off, amqpbridge_12_1_link_state_a_lost_connection_puts_unsettled_deliveries_first, amqpbridge_12_1_link_state_credit_is_granted_again_at_half, amqpbridge_4_2_a_link_is_named_container_topic_role, amqpbridge_9_3_the_delay_grows_by_the_factor_up_to_the_maximum}`, disposition (8): `crates/amqp-endpoint/src/bridge_daemon/disposition.rs::tests::{amqpbridge_12_1_disposition_accepted_ends_the_deliveries_of_the_range, amqpbridge_12_1_disposition_rejected_ends_the_delivery_and_counts_an_error, amqpbridge_12_1_disposition_released_and_modified_send_again_up_to_max_attempts, amqpbridge_12_1_disposition_an_unknown_delivery_changes_nothing, amqpbridge_12_1_disposition_an_unsettled_outcome_is_settled_by_the_daemon, amqpbridge_12_1_disposition_the_brokers_settlement_frees_credit_in_mode_second, amqpbridge_12_1_disposition_an_inbound_outcome_goes_out_settled_or_in_mode_second, amqpbridge_4_4_a_failed_dds_write_releases_the_delivery}`, sasl (8): `crates/amqp-endpoint/src/broker/sasl.rs::tests::{amqpbridge_7_2_scram_client_final_matches_the_rfc_7677_example, amqpbridge_12_1_sasl_each_mechanism_has_its_initial_response, amqpbridge_12_1_sasl_the_configured_mechanism_is_sent_and_accepted, amqpbridge_12_1_sasl_a_mechanism_not_offered_is_refused_before_the_init, amqpbridge_12_1_sasl_an_outcome_other_than_ok_is_a_refusal, amqpbridge_12_1_sasl_a_challenge_for_plain_is_a_protocol_error, amqpbridge_12_1_sasl_a_broker_without_the_layer_is_named, amqpbridge_12_1_sasl_scram_checks_the_server_signature}`, dds_pump (8): `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::tests::{amqpbridge_12_1_dds_pump_a_sample_of_this_bridge_is_not_sent_back, amqpbridge_12_1_dds_pump_a_sample_becomes_one_message_with_its_key_on_a_keyed_topic, amqpbridge_12_1_dds_pump_a_lifecycle_marker_becomes_one_message_per_operation, amqpbridge_12_1_dds_pump_the_end_of_a_coherent_set_has_no_message, amqpbridge_12_1_dds_pump_a_full_queue_drops_its_oldest_message, amqpbridge_12_1_dds_pump_drain_sends_while_credit_lasts, amqpbridge_12_1_dds_pump_a_full_session_window_keeps_the_message, rtps_9_6_4_9_a_dispose_with_unregister_is_two_messages_dispose_first}`, acl (5): `crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::{amqpbridge_12_1_acl_read_and_write_are_separate_lists, amqpbridge_12_1_acl_the_wildcard_admits_every_subject, amqpbridge_12_1_acl_default_deny_decides_a_topic_no_rule_names, amqpbridge_12_1_acl_the_configured_rules_decide, amqpbridge_12_1_acl_live_rules_follow_a_reload}`. Die Tests von link_state, disposition und dds_pump laufen gegen die Rahmen, die ein
`BrokerWriter` auf ein Socket-Paar schreibt, ohne Broker; die von sasl
gegen eine Broker-Seite im Test, die Rahmen für Rahmen antwortet.

**Status:** done — Linux-Testhost: `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` 474 von 474 (18 übersprungen: die Tests mit `#[ignore]`).

### §12.2 Integration-Tests

**Spec:** §12.2 — Daemon gegen RabbitMQ, Roundtrip + DISPOSITION-Folge.

**Repo:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs`, `crates/amqp-endpoint/tests/bridge_e2e.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_4_a_message_from_pika_reaches_a_reader_and_a_bad_body_does_not`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_2_scram_sha_256_authenticates_and_a_wrong_password_is_refused`, `crates/amqp-endpoint/tests/bridge_e2e.rs::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name` (die RabbitMQ-Fälle sind `#[ignore]`, Linux-Testhost).

**Status:** done

### §12.3 Multi-Vendor

**Spec:** §12.3 — Cyclone-DDS-Subscriber + RabbitMQ + Bridge; Broker-Matrix.

**Repo:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs` (Cyclone-Leser und Cyclone-Schreiber über Cyclones Python-Binding, qpid-proton am Broker); der Schlüssel einer Instanz von einem Schreiber ohne `PID_KEY_HASH`: `crates/amqp-endpoint/src/bridge_daemon.rs` (`type_file` auch für den Leser), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`note_unknown_key_hash`, `sample_messages`), `crates/dcps/src/runtime.rs` (`DcpsRuntime::set_user_reader_key_support`, `UserSample::Lifecycle::key_hash_unavailable`); die Broker-Matrix wie §10a.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::ddsamqp_7_7_2_an_amqp_dispose_reaches_a_cyclone_reader`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_12_3_a_cyclone_writer_reaches_the_queue`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_12_3_a_cyclone_writer_without_type_file_gets_the_nil_handle` (Cyclone DDS 11.0.1, Job `vendor-live-dispose-bridges`); `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::tests::{amqpbridge_12_1_dds_pump_a_keyed_sample_without_key_hash_is_told_once_per_writer, amqpbridge_12_1_dds_pump_a_dispose_without_key_hash_goes_out_with_the_nil_handle, amqpbridge_8_1_key_hash_unavailable_is_logged_once_per_writer_with_its_fields}`; `crates/dcps/src/runtime.rs::tests::{set_user_reader_key_support_reads_the_keys_of_an_unannounced_type, rtps_8_7_10_a_lifecycle_marker_without_a_computable_key_hash_says_so}`; `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::amqpbridge_3_a_type_file_the_topic_would_not_read_is_refused`; die Broker-Matrix mit den Tests von §10a.

**Offen:** Azure Service Bus in der Broker-Matrix (§10a).

**Status:** partial — auf dem Linux-Testhost gemessen: ein Cyclone-Leser hinter der Bridge (pika und qpid-proton → RabbitMQ → Bridge → Cyclone, Instanzen 7 und 8 und der Dispose von 7) und ein Cyclone-Schreiber vor der Bridge (Cyclone → Bridge → RabbitMQ → qpid-proton: zwei Samples in Schreibreihenfolge mit dem Content-Type ihres Encapsulation-Headers, Cyclones Nutzlast, `durable` bei TRANSIENT_LOCAL, `dds:instance-handle` als KeyHash, `group-id` aus diesem KeyHash; der Dispose von 7 mit `dds:operation` `dispose`, dem KeyHash von 7 und dessen `group-id`). Der Lauf zeigte: Cyclone 11.0.1 sendet kein `PID_KEY_HASH`, und die Bridge gab `dds:instance-handle` als Nullen aus. Jetzt berechnet der Leser den KeyHash aus `type_file`. Ohne `type_file`, oder wenn Körper oder Schlüssel nicht als der Typ dekodieren, gehen ein solches Sample und ein Dispose oder Unregister eines solchen Schreibers mit dem Nil-Handle hinaus (Nullen, kein `group-id`); `key_hash_unavailable` wird einmal je Schreiber geloggt und in `errors_total` gezählt. Für den Marker meldet dcps das über `UserSample::Lifecycle::key_hash_unavailable`; vorher ging ein solcher Dispose mit den ersten 16 Oktetten des `serializedKey` als `dds:instance-handle` hinaus, ohne Logzeile. Gegen Cyclone ohne `type_file` gemessen: die beiden Samples und der Dispose von 7 mit 16 Nullen als `dds:instance-handle` und ohne `group-id`, `errors_total` 1; mit `type_file` 0. Die Logzeile selbst prüft der Unit-Test von dds_pump. Die Broker-Matrix ist ohne Azure Service Bus gemessen (§10a).

## §13 Cross-References

### §13 Verwandte Library + OMG-Spec + Standards

**Spec:** §13 — Library `crates/amqp-bridge/` + `crates/amqp-endpoint/`,
OMG-DDS-AMQP-1.0, ISO/IEC 19464:2014, Wire-Format, Deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config, Major=Wire-
Protocol-Change (AMQP-1.0→2).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Regel für die Vergabe der Versionsnummer eines Release, keine Eigenschaft des Daemons.

---

## Audit-Status

23 done / 5 partial / 0 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-amqp-bridge-daemon-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` (Job `merge-rust-amqp-endpoint`) — 23 Tests grün.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (Job `merge-rust-config`) — 3 Tests grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 4 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor-amqp` (Job `integration-bridges-vendor-amqp`) — 7 Tests grün.
* Job `vendor-live-dispose-bridges` — 2 Tests grün.

39 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-amqp-bridge-daemon-1.0.open.md`.
