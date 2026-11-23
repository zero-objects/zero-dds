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

**Status:** n/a (informative) — assignment of the levels; the requirements are the items §1a and §2 to §12.

### §1a L6 multi-tenant

**Spec:** §1 L6, §1.1 — several DomainParticipants per daemon; one tenant binding per AMQP connection.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`TenantConfig`, `DaemonConfig::of_tenant`), `crates/amqp-endpoint/src/bridge_daemon.rs` (`run_with_reload`, `Participant`, `admin_catalog`), `crates/amqp-endpoint/src/bridge_daemon/reload.rs` (`reload_all`, `reloaded_part`), `crates/amqp-endpoint/src/daemon_runtime.rs` (`BridgeMetrics::for_tenant`, label `tenant`; `CatalogTopic::tenant`), `crates/amqp-endpoint/src/bridge_daemon/log.rs` (`set_tenant`, `line`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_1_l6_the_tenants_are_read, amqpbridge_1_l6_a_tenant_that_cannot_be_told_apart_is_refused, amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file}`;
`crates/amqp-endpoint/src/bridge_daemon/log.rs::tests::amqpbridge_1_l6_a_tenants_log_lines_carry_the_tenant`;
`crates/amqp-endpoint/src/bridge_daemon/reload.rs::tests::{amqpbridge_1_l6_a_reload_reaches_each_tenant_from_its_own_entry, amqpbridge_1_l6_a_tenant_removed_at_reload_is_refused_for_it_alone}`;
`crates/amqp-endpoint/src/bridge_daemon.rs::tests::amqpbridge_1_l6_the_admin_catalog_lists_the_tenants_topics`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_1_l6_each_tenant_has_its_participant_and_its_connection, amqpbridge_1_l6_a_refused_tenant_stops_the_daemon}`;
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_1_l6_each_tenant_has_its_participant_and_its_connection_at_rabbitmq`.

**Status:** done — each tenant has a participant on its domain and a connection of its own (container id,
`open.hostname`, SASL identity, TLS client certificate); a sample of its domain goes out on its connection only,
the metrics carry the label `tenant`, the log lines the field `tenant`, the admin endpoint's `/catalog` lists each
tenant's topics with the field `tenant`. A duplicate name or container id is refused with the path of the second
entry (`tenants[i].name`, `tenants[i].amqp.container_id`), file and line. SIGHUP reloads each tenant from the entry
of the same name; a removed tenant gets `reload_refused` and keeps running. A tenant the broker refuses stops the
daemon with exit 5. Measured against RabbitMQ 4.0.5: two users, two connections, one sample each in the queue of
its own tenant.

## §2 CLI surface

### §2 zerodds-amqp-bridged CLI

**Spec:** §2 — options
`--config`/`--broker`/`--container-id`/`--domain`/`--sasl-mechanism`/`--user`/`--password`/`--tls-*`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4/5/6.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs`; `crates/amqp-endpoint/src/bridge_daemon.rs`
(`run_with_reload`, `is_refusal`, `connect_exit_code`, `default_container_id`);
`crates/dcps/src/participant.rs::host_name`.

**Tests:** `crates/amqp-endpoint/tests/bridge_e2e.rs::amqpbridge_2_version_flag_emits_one_line_and_exits_zero`,
`::amqpbridge_2_missing_broker_url_fails_with_exit_1`,
`::amqpbridge_2_plain_without_tls_is_refused_towards_a_remote_broker` (exit 1),
`::amqpbridge_2_daemon_with_no_broker_fails_with_exit_2` (exit 2 with `--once`),
`::amqpbridge_2_a_participant_that_cannot_start_is_exit_3`,
`::amqpbridge_2_a_tls_handshake_that_fails_is_exit_4` (certificate of another CA, without `--once`, one attempt),
`::amqpbridge_2_a_refused_sasl_exchange_is_exit_5_without_once`,
`::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name` (exit 5),
`::amqpbridge_2_a_refused_open_is_exit_6` (`close` in answer to the `open`),
`::amqpbridge_2_topic_flag_adds_a_mapping_and_sigterm_exits_0` (SIGTERM: DETACH, END, CLOSE, exit 0),
`::amqpbridge_2_set_and_check_config_print_the_origin`,
`::amqpbridge_2_check_config_masks_the_cbs_secret` (`amqp.cbs.secret` from a file and from `--set` as `***`, `amqp.cbs.ttl_s` shown),
`::amqpbridge_2_help_names_the_environment_of_both_secrets`;
`crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::amqpbridge_2_the_default_container_id_names_the_host`, `crates/amqp-endpoint/src/bridge_daemon.rs::tests::amqpbridge_2_a_refusal_is_told_from_a_broken_connection`;
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_2_an_unknown_vhost_is_exit_6` (RabbitMQ resets the connection after the `open`).

**Status:** done — exit 0 to 6 with tests, green on the Linux test host. A TCP failure is retried after the
back-off without `--once`; TLS, SASL and `open` end the daemon with 4, 5, 6 at the first connect. Without
`--container-id` the `container-id` is `zerodds-bridge-<host>`, the host name from `gethostname`.

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`amqp`/`topics`/`acl`/`metrics`/`catalog`;
ENV substitution `${VAR}` and `${VAR:-default}`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`DaemonConfig::from_config`, `read_topic`),
`crates/config/src/parse.rs` (the shared parser, flat flow maps).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file, amqpbridge_3_what_the_daemon_would_not_honour_is_refused, amqpbridge_3_the_shipped_example_reads_as_it_says, amqpbridge_3_values_the_shared_parser_reads_reach_the_daemon, amqpbridge_3_a_type_file_the_topic_would_not_read_is_refused}`;
`crates/config/tests/layering.rs::{a_flat_flow_map_is_a_map, a_hash_inside_quotes_is_part_of_the_value, environment_references_expand_before_the_parse}`;
`sender.rcv_settle_mode: 1` (Part 2 §2.8.3, `second`): `crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_3_mode_second_settles_after_the_outcome_as_sender, amqpbridge_3_mode_second_settles_after_the_outcome_as_receiver}`,
`crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_3_a_disposition_in_mode_second_leaves_the_delivery_unsettled`.

**Status:** done — the test reads the YAML block of §3 from the specification file itself (three topics,
`keyed`, `type_file`, the ACL rule, `catalog.address`, `metrics.listen`, `${AMQP_USER}`/`${AMQP_PASSWORD}`).
`rcv_settle_mode: 1` is built in both directions and checked against the broker side of the test; no broker
of the matrix is measured with it.

## §4 AMQP wire protocol

### §4.1 Connection open with properties + capabilities

**Spec:** §4.1 — `OPEN` with container-id/hostname/max-frame-size/
channel-max/idle-time-out/properties/desired + offered capabilities.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`connect`, `open_body`), `crates/amqp-endpoint/src/bridge_daemon.rs::run`.

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_4_1_the_open_carries_the_fields_of_the_spec` (ten fields, no idle-time-out for 0,
the capability arrays, properties with symbol keys);
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_4_1_the_daemon_opens_with_its_container_id_and_bridge_properties` (through `run`,
values from the configuration, `hostname` = the broker host);
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_1_rabbitmq_lists_the_bridge_properties_of_the_open`
(`rabbitmqctl list_connections protocol client_properties`: `{1, 0}` and both properties; RabbitMQ 4.0.5
has no key `container_id` for `list_connections`).

**Status:** done

### §4.2 Session + link setup ATTACH

**Spec:** §4.2 — `ATTACH` per direction with name/source/target,
settle modes, properties, desired capabilities; one session per link.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`BrokerWriter::begin`, `attach`), `crates/amqp-endpoint/src/bridge_daemon/link_state.rs` (`link_spec`,
`LinkState`, `begin_all`, `on_link_frame`).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_4_2_the_attach_carries_name_role_node_settle_modes_properties_and_capability`;
`crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::amqpbridge_4_2_a_link_is_named_container_topic_role`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_4_2_every_link_has_a_session_of_its_own_channel` (a `bidir` topic and the catalog: three `begin` on three channels, one `attach` each),
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_9_3_a_session_the_broker_ends_is_begun_again_after_the_delay`;
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_2_a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(RabbitMQ ends the session of the refused address, the others keep running).

**Status:** done

### §4.3 TRANSFER frame DDS→AMQP

**Spec:** §4.3 — a message per DDS-AMQP 1.0 §8: header, properties,
`dds:*` application properties, a Pass-Through body with the encapsulation
header; multi-frame above `max-frame-size`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/message.rs` (`encode_message`), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`sample_messages`, `drain`), `crates/amqp-endpoint/src/broker.rs` (`BrokerWriter::transfer`),
`crates/amqp-endpoint/src/properties.rs`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/message.rs::tests::{amqpbridge_4_3_a_sample_round_trips_through_the_message_form, amqpbridge_4_3_an_xcdr1_sample_keeps_its_header_and_says_so, amqpbridge_4_3_the_header_and_properties_are_those_of_the_spec}`;
`crates/amqp-endpoint/src/broker.rs::tests::amqpbridge_4_3_a_message_above_the_frame_size_is_split_and_reassembled`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_4_3_a_dispose_carries_the_writer_and_sequence_in_its_message_id, amqpbridge_4_3_a_dispose_from_amqp_is_not_sent_back}`;
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::{amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages, amqpbridge_4_3_a_sample_larger_than_the_frame_size_crosses_in_both_directions}`.

**Status:** done — header (durable, priority 4, ttl, first-acquirer), properties (message-id, to, subject,
content-type, creation-time, absolute-expiry-time, group-id for a keyed topic only) and `dds:*` checked field
by field. A dispose or unregister carries the GUID and sequence number of its writer; one the bridge wrote
from AMQP is not sent back (the test fails without the check of the GUID prefix, checked once).

### §4.4 TRANSFER frame AMQP→DDS + DISPOSITION

**Spec:** §4.4 — receiver link with `FLOW` credit, body → DDS sample,
DISPOSITION `accepted`/`rejected`/`released`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (`deliver`, `settle_inbound`), `crates/amqp-endpoint/src/bridge_daemon/message.rs` (`decode_message`, `split_encapsulation`),
`crates/amqp-endpoint/src/broker.rs` (`BrokerReader::next_event`, reassembly).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_4_a_message_from_pika_reaches_a_reader_and_a_bad_body_does_not`
(accepted and rejected, the queue empty afterwards),
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_a_sample_larger_than_the_frame_size_crosses_in_both_directions`
(loop prevention and multi-frame receive);
`crates/amqp-endpoint/src/bridge_daemon/disposition.rs::tests::amqpbridge_4_4_a_failed_dds_write_releases_the_delivery` (a writer with TRANSIENT_LOCAL and
KEEP_ALL at max_samples 1: the second message is `released`);
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_4_4_credit_is_topped_up_at_half` (credit 4: no `flow` after the first transfer, a
`flow` with delivery-count 2 and credit 4 after the second).

**Status:** done — `released` against a real DDS writer at its RESOURCE_LIMITS; that a broker delivers the
released message again is not measured (behaviour of the broker).

### §4.5 Disposition mapping accepted/rejected/released/modified

**Spec:** §4.5 — the broker's outcome of unsettled deliveries.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/disposition.rs::on_disposition`, `crates/amqp-endpoint/src/bridge_daemon/message.rs::redelivered`,
`crates/amqp-endpoint/src/terminus.rs` (`decode_outcome`, `modified_annotations`), `crates/amqp-endpoint/src/broker.rs` (`Inbound::Disposition::annotations`).

**Tests:** `crates/amqp-endpoint/src/broker.rs::tests::{amqp10_3_4_the_outcomes_are_their_described_lists, amqpbridge_4_5_the_reader_hands_on_the_annotations_of_modified}`;
`crates/amqp-endpoint/src/bridge_daemon/disposition.rs::tests::{amqpbridge_4_5_released_deliveries_go_out_again_in_order_not_first_acquired, amqpbridge_4_5_modified_delivery_failed_raises_the_delivery_count, amqpbridge_4_5_modified_undeliverable_here_is_not_sent_again, amqpbridge_4_5_modified_annotations_are_combined_with_the_message}`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_4_5_a_released_delivery_is_sent_again_and_dropped_after_five, amqpbridge_4_5_a_rejected_delivery_is_not_sent_again, amqpbridge_4_5_modified_counts_annotates_and_respects_undeliverable_here}`;
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::{amqpbridge_4_5_rabbitmq_releases_an_unroutable_message, amqpbridge_4_5_rabbitmq_refuses_a_message_for_a_full_queue}`;
`accepted` in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages`.

**Status:** done — `released` goes out again with `first-acquirer` false, `modified` also with the
`delivery-count` raised (delivery-failed) and the message-annotations combined; `undeliverable-here` and the
fifth attempt discard, count and log. RabbitMQ 4.0.5 answers `released` for a message that reaches no queue and
`rejected` without an error for a full queue with `reject-publish` (measured). `modified` from a real broker is
not measured, only against the scripted broker; that belongs to the broker matrix (§10a).

## §5 Topic mapping

### §5.1 Address

**Spec:** §5.1 — `amqp_address` per topic, broker-specific.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::read_topic`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_5_1_a_topic_without_amqp_address_is_refused, amqpbridge_5_1_the_topic_flag_form_is_dds_equals_address}`;
RabbitMQ v2 addresses (`/queues/…`) in `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`;
`crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::amqpbridge_5_1_artemis_a_topic_address_reaches_every_subscriber` (Artemis 2.44.0, `topic://`);
`crates/amqp-endpoint/tests/bridge_solace_e2e.rs::amqpbridge_5_1_solace_a_topic_address_reaches_every_subscriber` (Solace PubSub+ Standard 10.26.0.9219, `topic://`; green on the Linux test host, the CI job has not run, §10).

**Status:** done — `amqp_address` is mandatory, as a map and as `--topic`; the message names the entry. The
address forms of ActiveMQ Artemis (`<address>`, `queue://<name>`, `topic://<name>`), Solace PubSub+ (`<queue>`, `queue://<name>`, `topic://<name>`) and Qpid Dispatch belong to the broker matrix (§10a).

### §5.2 Catalog

**Spec:** §5.2 — a catalog (JSON) per connection on its own address.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs` (`catalog_message`), `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`DaemonConfig::catalog_json`,
`catalog_address`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_2_a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive`
(`pika` reads the catalog from `/queues/zd_bridge_catalog`).

**Status:** done

## §6 QoS translation

### §6 DDS QoS → AMQP behaviour map

**Spec:** §6 — reliability/durability/history/lifespan/deadline/
liveliness/partition.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`read_topic`: BEST_EFFORT → settled, `qos.deadline_ms`, `qos.liveliness_lease_ms`, `qos.partition`), `TopicMapping::terminus_durable` (terminus durable 1 on the target of the sender link for a durability other than VOLATILE; the source of the receiver link stays 0), `crates/amqp-endpoint/src/bridge_daemon/message.rs` (`encode_message`:
durable, ttl, `dds:partition`; `annotated`, `decode_message`: `dds:partition`), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`enqueue`: `OutLink::cap`; `look_at_deadline`, `writer_partitions`, heartbeat timing), `crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (`deliver`: partition check), `crates/amqp-endpoint/src/bridge_daemon.rs` (`run_with_reload`, `endpoint_qos`: HISTORY, DEADLINE, LIVELINESS, PARTITION of the DDS endpoints), `crates/amqp-endpoint/src/broker.rs::BrokerWriter::heartbeat`.

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file, amqpbridge_6_the_qos_keys_are_read}`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_6_keep_last_holds_n_and_drops_the_oldest, amqpbridge_6_transient_local_is_a_durable_message_to_a_durable_node, amqpbridge_6_a_heartbeat_goes_out_at_half_the_idle_time_out, amqpbridge_6_a_missed_deadline_is_annotated, amqpbridge_6_the_liveliness_lease_reaches_the_dds_endpoints, amqpbridge_6_partitions_reach_the_reader_and_the_message, amqpbridge_6_a_message_of_another_partition_is_rejected, amqpbridge_6_a_keep_all_topic_from_amqp_keeps_every_sample_for_a_late_reader}`;
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::{amqpbridge_4_2_a_refused_link_leaves_the_others_running_and_catalog_and_ttl_arrive, amqpbridge_6_rabbitmq_takes_a_transient_local_sample_as_a_persistent_message}`
(LIFESPAN 60 s → `expiration` 60000; TRANSIENT_LOCAL → delivery mode 2).

**Open:** KEEP_ALL without credit discards the oldest of more than 65,536 waiting samples instead of holding
the DDS writer back. That needs the runtime's DDS reader (`crates/dcps`, `register_user_reader_kind` returns an
unbounded `mpsc` channel) to withhold its ACKNACK while the bridge does not take the samples.

**Status:** partial — reliability, durability (header and terminus), KEEP_LAST, lifespan, the deadline
annotation `x-opt-zerodds-deadline-missed`, liveliness (lease and heartbeat) and partition (endpoints,
`dds:partition`, refusal) built and green on the Linux test host. The lease is checked at the DDS endpoints: a
writer offering 200 ms matches the reader of a 500 ms topic, one offering 2000 ms does not; a reader requesting
1000 ms matches the writer, one requesting 300 ms does not. The terminus default `durable` 1 applies only to the
target of the sender link. The heartbeat used to go out only at the next
250 ms tick (measured: 3 instead of 5 in 1.6 s at an idle time-out of 600 ms); the pump now wakes up when the
heartbeat is due. Open: backpressure for KEEP_ALL.

## §7 Security

### §7.1 TLS amqps:// + ALPN

**Spec:** §7.1 — `amqps://` per `amqp.tls.enabled`, ALPN `["amqp"]`.

**Repo:** `crates/amqp-endpoint/src/broker.rs` (`Wire::Tls`, `ALPN_AMQP`), `crates/amqp-endpoint/src/bridge_daemon.rs::tls_config`.

**Tests:** `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp`
(the server reads `amqp` as the agreed protocol).

**Status:** done

### §7.1a ALPN negotiation `["amqp"]`

**Spec:** §7.1 — the TLS handshake offers ALPN `["amqp"]`.

**Repo:** `crates/amqp-endpoint/src/broker.rs::connect`.

**Tests:** `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp` (the server reads `amqp` as the negotiated protocol).

**Status:** done

### §7.2 SASL PLAIN/SCRAM/EXTERNAL/ANONYMOUS/XOAUTH2

**Spec:** §7.2 — the mechanism per config; PLAIN without TLS and ANONYMOUS only
towards loopback.

**Repo:** `crates/amqp-endpoint/src/broker/sasl.rs` (`exchange`, `initial_response`, `scram_client_final_with`), `crates/amqp-endpoint/src/scram.rs` (`ScramHash`), `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`DaemonConfig::sasl`), `crates/amqp-endpoint/src/bridge_daemon.rs` (`Participant::prepare`).

**Tests:** `crates/amqp-endpoint/src/broker/sasl.rs::tests::{amqpbridge_7_2_scram_client_final_matches_the_rfc_7677_example, amqpbridge_7_2_scram_sha_512_authenticates_and_checks_the_signature, amqpbridge_7_2_xoauth2_sends_the_bearer_token_and_reads_the_error, amqpbridge_12_1_sasl_each_mechanism_has_its_initial_response, amqpbridge_12_1_sasl_scram_checks_the_server_signature}`;
`crates/amqp-endpoint/src/scram.rs::tests::{amqpbridge_7_2_scram_sha_512_proof_and_signature, rfc7677_scram_hash_sha256_is_the_rfc_schedule}`;
`crates/amqp-endpoint/tests/broker_tls_sasl.rs::{amqpbridge_7_2_scram_sha_256_authenticates_and_a_wrong_password_is_refused, amqpbridge_7_1_external_authenticates_with_the_tls_client_certificate_and_alpn_amqp, amqpbridge_7_2_a_mechanism_the_broker_does_not_offer_is_refused_by_name}`;
`crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::{amqpbridge_7_2_artemis_accepts_scram_sha_256_and_512, amqpbridge_7_2_a_wrong_scram_sha_512_password_is_exit_5}`;
`crates/amqp-endpoint/tests/bridge_brokerj_e2e.rs::amqpbridge_7_2_brokerj_accepts_the_xoauth2_token`;
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_7_2_rabbitmq_accepts_external_with_the_client_certificate` (PLAIN against RabbitMQ in the other tests of the file);
`crates/amqp-endpoint/tests/bridge_e2e.rs::{amqpbridge_2_plain_without_tls_is_refused_towards_a_remote_broker, amqpbridge_7_2_anonymous_towards_a_remote_broker_is_refused, amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name}`.
Measured on the Linux test host: Apache ActiveMQ Artemis 2.44.0 (`SCRAMPropertiesLoginModule`, an acceptor with SCRAM-SHA-256 and SCRAM-SHA-512 alone) accepts both, a wrong password is exit code 5;
RabbitMQ 4.0.5 with `rabbitmq_auth_mechanism_ssl` accepts EXTERNAL with the client certificate, a certificate without a user is exit code 5.
Apache Qpid Broker-J 9.2.1 (OAuth2 provider, identity resolver `GitHubUser` against an HTTPS endpoint in the test) accepts the XOAUTH2 token and refuses an unknown token in the SASL stage.
The SHA-512 proof is checked against values from CPython `hashlib`/`hmac`.

**Status:** done — PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, EXTERNAL, ANONYMOUS and XOAUTH2; SCRAM and EXTERNAL measured against Artemis and RabbitMQ, XOAUTH2 against Broker-J.
### §7.3 ACL daemon side

**Spec:** §7.3 — subject = SASL identity or TLS DN; filter before TRANSFER.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/acl.rs` (`allowed`, `acl_allows`, `subjects_of`, `Live`), `crates/amqp-endpoint/src/bridge_daemon/config.rs` (`acl.rules`, `acl.default_deny`),
`crates/amqp-endpoint/src/bridge_daemon.rs` (`Participant::prepare`, `Participant::run`: no DDS entity and no link for a denied direction),
`crates/amqp-endpoint/src/bridge_daemon/reload.rs::apply_reload` (the subjects of a new certificate), `crates/amqp-endpoint/src/bridge_daemon/disposition.rs::on_transfer`, `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::{pump, drain}` (once the read right is withdrawn, `drain` drops the messages waiting for credit and those the broker released, with `acl_denied`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_read_and_write_are_separate_lists`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_the_wildcard_admits_every_subject`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_default_deny_decides_a_topic_no_rule_names`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_the_configured_rules_decide`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::amqpbridge_12_1_acl_live_rules_follow_a_reload`,
`crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::{amqpbridge_7_3_external_takes_the_certificate_dn, amqpbridge_7_3_the_sasl_user_is_the_subject, amqpbridge_7_3_a_reloaded_certificate_is_the_new_subject}`
(read and write lists apart, `*` for every subject, `default_deny`, the configured rules, reload; with EXTERNAL the subject DN per RFC 4514 and the fingerprint `mtls:<SHA-256>` of the certificate in `amqp.tls.cert_file`, the CN alone does not match; the user of PLAIN, SCRAM, XOAUTH2; a new certificate at reload is the new subject),
`crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::amqpbridge_3_the_shipped_example_reads_as_it_says` (the rules are read);
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::tests::amqpbridge_7_3_dds_pump_a_withdrawn_read_right_drops_the_queue`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_7_3_a_direction_denied_at_start_moves_no_data, amqpbridge_7_3_a_write_right_withdrawn_by_reload_rejects_the_transfer, amqpbridge_7_3_a_read_right_withdrawn_by_reload_stops_the_queued_messages}`
(scripted broker: `read: []` attaches only the receiving link and registers no DDS reader, `write: []` only the sending link without a DDS writer; after the reload `rejected` with `amqp:unauthorized-access`, not written to DDS; once the read right is withdrawn, neither a message waiting for credit nor one the broker released goes out when the broker grants credit);
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_topic`. Linux test host: green.

**Status:** done — the subject is the SASL user, or with EXTERNAL the subject DN and fingerprint of the client certificate; a denied direction has neither a DDS entity nor a link; a read right withdrawn by a reload also holds back the topic's messages already taken from DDS.
## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log on stdout with timestamp/level/event/… and
`--log-level`.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/log.rs` (`line`, `emit`, `log`, `event_level`, `set_participant`, `escape`, `rfc3339`),
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`sent`, `round_trip_out`: `transfer_out`, `disposition`),
`crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (`on_transfer`: `transfer_in`),
`crates/amqp-endpoint/src/bridge_daemon.rs` (`log_event`, `set_log_level`), `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` (the program's lines).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/log.rs::tests::{amqpbridge_8_1_a_line_is_a_json_object_with_the_spec_fields, amqpbridge_8_1_the_log_level_is_the_threshold, amqpbridge_8_1_control_characters_are_escaped, amqpbridge_8_1_every_event_has_a_level_and_the_threshold_orders_them, rfc3339_5_6_the_utc_date_and_time}`;
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::tests::amqpbridge_8_1_key_hash_unavailable_is_logged_once_per_writer_with_its_fields` (`key_hash_unavailable`: level `warn`, `dds_topic`, `writer`, `type_file`, once per writer);
`crates/amqp-endpoint/tests/bridge_e2e.rs::{amqpbridge_8_1_8_3_the_log_lines_on_stdout_and_the_spans_of_a_transfer, amqpbridge_8_1_transfer_out_and_disposition_lines_of_both_settle_modes}`
(the program against the scripted broker, `log_level: debug`: every line on stdout begins with `timestamp`; every line but the program's own (`startup`, `admin_endpoint`, `admin_endpoint_bind_failed`, `signal_watcher_init_failed`) carries `container_id`, `startup` names it as a field;
`attached` with `link_name`, `dds_topic`, `amqp_address`; `transfer_in` with `delivery_id`, `bytes`, `outcome`, `latency_us`;
DDS → AMQP with DDS writers of the test: `transfer_out` with `link_name`, `dds_topic`, `amqp_address`, `delivery_id`, `bytes`, and `disposition` with outcome `accepted` on the unsettled link and `settled` on the link that sends settled, each with `latency_us`;
`link_detached` with `link_name`, `dds_topic`, `amqp_address` and the broker's condition; no log line on stderr).
Linux test host: green.

**Status:** done — lines on stdout with `timestamp`, `level`, `event`, `tenant`, `container_id` and the fields of the event;
`transfer_in`, `transfer_out` and `disposition` at level `debug` with `delivery_id`, `bytes`, `latency_us`.
### §8.2 Prometheus metrics

**Spec:** §8.2 — 13 families `zerodds_amqp_bridge_*` with labels.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (`BridgeMetrics`),
the counting sites in `crates/amqp-endpoint/src/bridge_daemon.rs`,
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`pump`, `drain`, `enqueue`, `topic_metrics`),
`crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (`count_outcome`, `on_transfer`, `settle_inbound`, `on_disposition`, `deliver`),
`crates/amqp-endpoint/src/bridge_daemon/link_state.rs` (`top_up`, `on_link_frame`),
`crates/amqp-endpoint/src/peer.rs::condition::DEFINED`.

**Tests:** `crates/amqp-endpoint/src/daemon_runtime.rs::tests::amqpbridge_8_2_the_reason_label_is_the_condition_or_other`
(`amqp:decode-error`, `amqp:transaction:rollback`, `amqp:session:window-violation`, a vendor's symbol, no condition, `amqp:` symbols AMQP 1.0 does not define),
`crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_8_2_the_metrics_count_transfers_dispositions_and_samples`
(Linux test host, RabbitMQ 4.0.5: 3 samples out, one good and one broken message
in; transfers, samples and dispositions per topic,
`rejected{reason="amqp:decode-error"}` 1).

**Status:** done — all 13 families; dispositions count in both
directions, `reason` is the AMQP condition when AMQP 1.0 defines it (Part 2 §2.8.15 to §2.8.18, Part 4 §4.5.8), else `other`, also without a condition. The
generic `zerodds_amqp_*` remain beside them.
### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables the TRANSFER
round-trip span.

**Repo:** `crates/amqp-endpoint/src/daemon_runtime.rs` (`otlp_config_from_env`, `spawn_otlp_flush_loop`),
`crates/amqp-endpoint/src/bridge_daemon/trace.rs` (`Tracer`, `spans`, `record`),
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::round_trip_out`, `crates/amqp-endpoint/src/bridge_daemon/disposition.rs::on_transfer`,
`crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` (the exporter passed to `run_with_reload`, a last flush at the end).

**Tests:** `crates/amqp-endpoint/src/daemon_runtime.rs::tests::amqpbridge_8_3_otlp_config_parses_endpoint`;
`crates/amqp-endpoint/src/bridge_daemon/trace.rs::tests::{amqpbridge_8_3_an_inbound_transfer_is_a_span_with_a_dds_write_child, amqpbridge_8_3_an_outbound_transfer_ends_with_its_disposition, amqpbridge_8_3_a_rejected_transfer_has_status_error, amqpbridge_8_3_a_tracer_without_exporter_is_off, amqpbridge_8_3_span_ids_differ}`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_8_3_the_spans_of_a_transfer_reach_the_collector`
(both directions, an OTLP/HTTP receiver in the test: three `amqp.transfer`, one `dds.read`, one `dds.write`, `amqp:decode-error` as the condition);
`crates/amqp-endpoint/tests/bridge_e2e.rs::amqpbridge_8_1_8_3_the_log_lines_on_stdout_and_the_spans_of_a_transfer` (the program with
`OTEL_EXPORTER_OTLP_ENDPOINT`). Linux test host: green. Not measured against an OpenTelemetry Collector; the receiver in the test checks the spans in the JSON of `zerodds-observability-otlp`.

**Status:** done — one trace per TRANSFER round trip, `amqp.transfer` with `dds.write` (AMQP → DDS) or `dds.read` (DDS → AMQP).
## §9 Lifecycle

### §8.3a TRANSFER roundtrip span

**Spec:** §8.3 — with `OTEL_EXPORTER_OTLP_ENDPOINT` set, one roundtrip span
is emitted per TRANSFER.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/trace.rs`, as §8.3.

**Tests:** `crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_8_3_the_spans_of_a_transfer_reach_the_collector`;
`crates/amqp-endpoint/tests/bridge_e2e.rs::amqpbridge_8_1_8_3_the_log_lines_on_stdout_and_the_spans_of_a_transfer`. Linux test host: green.

**Status:** done — the emit half of §8.3, described there.
### §9.1 Startup sequence

**Spec:** §9.1 — config → signal handler → TLS → DCPS → reader/writer →
open + SASL + TLS → session + attach.

**Repo:** `crates/amqp-endpoint/src/bin/zerodds-amqp-bridged.rs` (configuration, then `install_signal_watcher`);
`crates/amqp-endpoint/src/bridge_daemon.rs::Participant::prepare` (TLS, log `tls_loaded`),
`crates/amqp-endpoint/src/bridge_daemon.rs::Participant::run` (DCPS `participant_started`, readers and writers `endpoints_registered`,
`broker::connect` `connected`, `begin_all` and the attaches `attached`).

**Tests:** `crates/amqp-endpoint/tests/bridge_scripted.rs::amqpbridge_9_1_the_dds_endpoints_stand_before_the_broker_is_contacted`; `crates/amqp-endpoint/tests/bridge_e2e.rs::{amqpbridge_9_1_the_log_lines_follow_the_steps_and_sigint_exits_0, amqpbridge_9_1_tls_material_that_does_not_load_stops_before_dds, amqpbridge_9_1_tls_loaded_comes_between_startup_and_the_participant}`. Linux test host: green.

**Status:** done — a second DDS participant matches the bridge's reader and writer while nothing listens on the
broker's port; then SASL header and init, AMQP header and open follow, per link its begin before its attach. The
program's log lines follow the order of the steps: without TLS `startup`, `participant_started`, `endpoints_registered`,
`connected`, `attached`; with TLS material that loads, `tls_loaded` (with `ca_file`) comes after `startup` and before
`participant_started`, measured against a TLS server with a certificate of that CA. TLS material that does not load ends with exit 1, without
`participant_started` and without a connection to the broker. Deviation from the original text: the signal handlers
come right after the configuration (spec §9.1 step 1), so that a stop during TLS, DDS start, connect or back-off is the
shutdown of §9.2.

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain of at most 30 s, DETACH/END/CLOSE; SIGHUP
TLS + ACL reload.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::pump` (`DaemonConfig::drain_budget`, default `DRAIN_BUDGET` 30 s;
`drain_started`, `shutdown_with_unsettled`, `shutdown_with_queued`; `OutLink::stopping` from the start of the drain),
`crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::drain` (after the stop no message of the queue goes out, whatever
credit the broker grants), `crates/amqp-endpoint/src/bridge_daemon.rs::Participant::run` (DETACH/END/CLOSE,
`left_at_stop` for a stop without a connection); `crates/amqp-endpoint/src/daemon_runtime.rs::install_signal_watcher`
(Unix: SIGTERM, SIGINT, SIGHUP; Windows: the console control handler through `ctrlc`);
`crates/amqp-endpoint/src/bridge_daemon.rs::run_with_reload` (`Watchers`: the threads for STOPPING=1, SIGHUP and the
common stop of the tenants end with the call, on every path out of it; `watch_stopping`), `crates/amqp-endpoint/src/bridge_daemon/reload.rs` (`reload_all`, `apply_reload`;
the ACL and the TLS material of the next connect per participant; withdrawn rights apply
at once, new grants after a restart).

**Tests:** `crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_9_2_the_drain_waits_for_the_disposition_then_detaches_ends_and_closes, amqpbridge_9_2_the_drain_gives_up_after_its_budget_and_counts_what_is_left, amqpbridge_9_2_a_sample_written_during_the_drain_is_not_sent, amqpbridge_9_2_a_message_waiting_for_credit_is_counted_at_the_stop, amqpbridge_9_2_a_stop_without_a_broker_connection_counts_in_errors_total, amqpbridge_9_2_a_reload_gives_the_next_connect_the_new_certificate, amqpbridge_9_2_the_reload_watcher_ends_with_the_run, amqpbridge_9_2_credit_granted_during_the_drain_sends_no_queued_message, amqpbridge_9_2_a_message_released_during_the_drain_is_not_sent_again}`; `crates/amqp-endpoint/src/bridge_daemon.rs::tests::{amqpbridge_9_2_a_stop_after_the_run_sends_no_stopping, amqpbridge_9_2_a_stop_before_the_end_of_the_run_sends_stopping_once}`; `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::amqpbridge_9_2_a_link_asked_to_drain_after_the_stop_consumes_its_credit`; `crates/amqp-endpoint/tests/bridge_e2e.rs::{amqpbridge_2_topic_flag_adds_a_mapping_and_sigterm_exits_0, amqpbridge_9_1_the_log_lines_follow_the_steps_and_sigint_exits_0, amqpbridge_9_2_a_stop_without_a_broker_connection_counts_the_samples_left, amqpbridge_9_2_the_stop_line_counts_a_message_waiting_for_credit, amqpbridge_9_2_the_stop_line_counts_a_sample_delivered_during_the_drain}`; `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_topic` (RabbitMQ 4.0.5). Linux test host: green.

**Status:** done — the drain waits for the disposition of the delivery in flight, then sends DETACH, END and CLOSE;
without a disposition it ends after its budget and counts what is left; a sample after the stop and a message without
credit are not sent and are counted (the program's `shutdown_with_queued` line: `count` 1 and `not_taken` 0 for the
message without credit, `count` 0 and `not_taken` 1 for the sample during the drain); credit the broker grants during
the drain sends no message that waited for credit before the stop, and a delivery the broker gives back with
`released` during the drain waits for credit again and is not sent again; both count in `shutdown_with_queued`; a link
the broker asks to drain after the stop has no message available and consumes its credit (AMQP 1.0 Part 2 §2.6.7); the
line `drain_started` names
`in_flight` and `queued` when the drain begins; a stop without a broker
connection (in the back-off) ends at once with exit 0, counts the samples the DDS reader delivered as `not_taken` and
adds one to `errors_total` (`left_at_stop`); SIGTERM and SIGINT end with exit 0; after a reload the next connect shows
the new client certificate; the threads of `run_with_reload` that watch the stop end with the call, also on an error
without a stop: a later reload is not applied, a later stop sends no STOPPING=1. Windows: CTRL_C and CTRL_BREAK start the stop with the drain. Closing the console, logoff
and shutdown set the stop too, but `ctrlc`'s handler returns at once and Windows ends the process before the drain;
spec §9.2 and the man pages say so. The handler is built for `x86_64-pc-windows-gnu` (`cargo check`), not run on a
Windows host: no Windows host.

### §9.3 Reconnect

**Spec:** §9.3 — exponential back-off; unsettled deliveries sent again.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/link_state.rs` (`next_delay`, the requeue in `connection_lost` and `out_link_down`).

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::amqpbridge_9_3_the_delay_grows_by_the_factor_up_to_the_maximum`;
`crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_9_3_unsettled_deliveries_go_out_again_first_and_in_order, amqpbridge_9_3_a_session_the_broker_ends_is_begun_again_after_the_delay}`;
against RabbitMQ 4.0.5 `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_9_3_after_the_broker_closes_the_connection_the_bridge_reconnects`
(`rabbitmqctl close_all_connections`; the sample before and the one after the close arrive).

**Status:** done — three unsettled deliveries go out again after the connection broke, first and in their
order, before a sample written after it; a session the broker ends is begun again on its channel after the
back-off. A restart of RabbitMQ while samples are written is not measured.

## §10 Cross-vendor

### §10 RTPS peer + RabbitMQ/Artemis/Qpid/Solace/ServiceBus

**Spec:** §10 — a plain RTPS peer; the AMQP side against five brokers.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon.rs`, `crates/amqp-endpoint/src/broker.rs`, `crates/amqp-endpoint/src/coexistence.rs`, `crates/amqp-endpoint/src/bridge_daemon/cbs.rs`; the test setup `crates/amqp-endpoint/tests/support/broker_matrix.rs`, `crates/amqp-endpoint/tests/support/proton_peer.py`, `crates/amqp-endpoint/tests/support/artemis.rs`, `crates/amqp-endpoint/tests/support/dispatch.rs`, `crates/amqp-endpoint/tests/support/solace.rs`, `tests/integration/provision.sh` (`artemis`, `dispatch`).

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_4_a_message_from_pika_reaches_a_reader_and_a_bad_body_does_not`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_a_sample_larger_than_the_frame_size_crosses_in_both_directions` (RabbitMQ 4.0.5); `crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::{amqpbridge_10_artemis_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_artemis_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_artemis_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_artemis_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_artemis_a_refused_address_leaves_the_other_links_running}` (Artemis 2.44.0); `crates/amqp-endpoint/tests/bridge_dispatch_e2e.rs::{amqpbridge_10_dispatch_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_dispatch_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_dispatch_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_dispatch_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_dispatch_a_refused_address_leaves_the_other_links_running}` (Qpid Dispatch 1.19.0); `crates/amqp-endpoint/tests/bridge_solace_e2e.rs::{amqpbridge_10_solace_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_solace_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_solace_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_solace_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_solace_a_refused_address_leaves_the_other_links_running}` (Solace PubSub+ Standard 10.26.0.9219); qpid-proton 0.37 as the client at the broker; jobs `integration-bridges-vendor-amqp` and `integration-bridges-vendor-amqp-solace`. The RTPS peer with a writer of another vendor: `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_12_3_a_cyclone_writer_reaches_the_queue` (Cyclone DDS 11.0.1, job `vendor-live-dispose-bridges`). Linux test host: green.

**Open:** Azure Service Bus. The emulator needs its license terms accepted (`ACCEPT_EULA`), which has not been given; there is no Azure namespace. The claims-based security of Azure Service Bus is built (§10a). The job `integration-bridges-vendor-amqp-solace` has not run in CI: Solace comes as a container image only, a service container in the job. Measured on the Linux test host: Solace PubSub+ Standard 10.26.0.9219 starts with 1 GB of `/dev/shm` and a hard `nofile` limit of 1048576; with 64 MB of `/dev/shm` (Docker's default) or a hard limit of 524288 it ends in its platform audit (`POST Violation [022]`). GitLab Runner gives a service container the runner's `shm_size` and no ulimits; the job therefore needs a runner with `shm_size` of at least 1 GiB and a Docker daemon whose default hard `nofile` limit is at least 1048576.

**Status:** partial — RabbitMQ 4.0.5, Artemis 2.44.0, Qpid Dispatch 1.19.0 and Solace PubSub+ Standard 10.26.0.9219 measured on the Linux test host; a Cyclone writer in front of the bridge measured (§12.3); the Solace job has not run in CI (runner configuration); Azure Service Bus not measured (license acceptance for the emulator).

### §10a Broker matrix Artemis/Qpid Dispatch/Solace/Azure ServiceBus

**Spec:** §10 — the AMQP side against Artemis, Qpid Dispatch, Solace and
Azure ServiceBus.

**Repo:** as §10; `put-token` on `$cbs` in `crates/amqp-endpoint/src/bridge_daemon/cbs.rs` (keys `amqp.cbs.*`).

**Tests:** `crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::{amqpbridge_10_artemis_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_artemis_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_artemis_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_artemis_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_artemis_a_refused_address_leaves_the_other_links_running}`; `crates/amqp-endpoint/tests/bridge_dispatch_e2e.rs::{amqpbridge_10_dispatch_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_dispatch_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_dispatch_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_dispatch_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_dispatch_a_refused_address_leaves_the_other_links_running}`; `crates/amqp-endpoint/src/bridge_daemon/cbs.rs::tests::{amqpbridge_10_the_sas_token_matches_the_documented_algorithm, amqpbridge_10_the_put_token_request_names_operation_type_audience_and_reply_to, amqpbridge_10_an_answer_gives_its_correlation_id_and_status, amqpbridge_10_the_audience_of_an_address, amqpbridge_10_percent_encoding_as_quote_plus_and_quote}`; `crates/amqp-endpoint/src/bridge_daemon.rs::tests::amqpbridge_10_cbs_keys_and_the_anonymous_rule`; `crates/amqp-endpoint/tests/bridge_scripted.rs::{amqpbridge_10_cbs_a_token_is_put_before_the_link_and_renewed, amqpbridge_10_cbs_a_refused_token_ends_the_connection_before_the_link, amqpbridge_10_cbs_a_put_token_without_answer_ends_the_connection, amqpbridge_10_cbs_a_renewal_without_answer_ends_the_connection}`; `crates/amqp-endpoint/tests/bridge_artemis_e2e.rs::amqpbridge_5_1_artemis_a_topic_address_reaches_every_subscriber`; `crates/amqp-endpoint/tests/bridge_solace_e2e.rs::{amqpbridge_10_solace_dds_samples_arrive_as_pass_through_messages, amqpbridge_10_solace_a_message_reaches_a_reader_and_a_bad_body_is_rejected, amqpbridge_10_solace_a_multi_frame_sample_crosses_in_both_directions, amqpbridge_10_solace_the_bridge_reconnects_after_the_broker_drops_the_connection, amqpbridge_10_solace_a_refused_address_leaves_the_other_links_running}`, `crates/amqp-endpoint/tests/bridge_solace_e2e.rs::amqpbridge_5_1_solace_a_topic_address_reaches_every_subscriber`. Linux test host: green.

**Open:** Azure Service Bus (emulator or namespace) not measured, for the reason of §10: the license acceptance for the emulator has not been given. The CBS tests run against the broker side in the test, not against Azure. The Solace job has not run in CI (runner configuration, §10).

**Status:** partial — Artemis, Qpid Dispatch and Solace PubSub+ Standard 10.26.0.9219 measured with the five cases of the matrix (Solace: `<queue>` and `queue://<name>` in the matrix, the queues created by the test through SEMP v2; the refused link is the receiver of a queue with `no-access`, since Solace attaches every sender; `topic://<name>` with two receivers, each of which gets every sample; the bridge over TLS with SASL PLAIN; Artemis: `<address>` and `queue://<name>` in the matrix, `topic://<name>` with two receivers, each of which gets every sample; a restart of the broker; Dispatch: the receiver first, the bridge's outcome back through the router; Dispatch 1.19.0 built against CPython 3.11, since it ends with SIGSEGV on Python 3.13). CBS built, not measured against Azure; a `put-token` round not answered within the connect time-out (the first round and a renewal) ends the connection, and the back-off follows.
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

**Tests:** `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::amqpbridge_3_the_shipped_example_reads_as_it_says`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does, every_key_the_daemon_reads_is_in_the_man_page}`
(the manual page and the example file name the same keys; every key the parser reads in its `.only(&[...])`
lists and in `KEYS` is in the manual page: the `catalog` section and `topics[].type_file` were missing from
page and example before).

**Open:** the Windows service. `Install-Services.ps1` registers the console program with `sc.exe create`;
the program has no service control dispatcher (`StartServiceCtrlDispatcher`), and `install_signal_watcher` is an
empty thread outside Unix. A start and a stop by the service control manager are not handled. A Windows host to
build and test on is missing; the weekly Windows runner does not build `zerodds-amqp-endpoint`.

**Status:** partial — Linux, macOS and Docker as above; the Windows service does not answer the service control manager (no Windows host, not built).

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/amqp_codec/link_state/disposition/sasl/
dds_pump/acl, ≥ 5 tests each.

**Repo:** `crates/amqp-endpoint/src/bridge_daemon/config.rs` (config), `crates/amqp-endpoint/src/broker.rs` (amqp_codec: `BrokerWriter`, `BrokerReader`, `FrameReader` on the codec of `crates/amqp-bridge/`), `crates/amqp-endpoint/src/bridge_daemon/link_state.rs` (link_state), `crates/amqp-endpoint/src/bridge_daemon/disposition.rs` (disposition), `crates/amqp-endpoint/src/broker/sasl.rs` (sasl, the client side), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (dds_pump), `crates/amqp-endpoint/src/bridge_daemon/acl.rs` (acl).

**Tests:** config (8): `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::{amqpbridge_3_what_the_daemon_would_not_honour_is_refused, broker_urls, amqpbridge_3_the_shipped_example_reads_as_it_says, amqpbridge_3_values_the_shared_parser_reads_reach_the_daemon, amqpbridge_2_the_default_container_id_names_the_host, amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file, amqpbridge_5_1_a_topic_without_amqp_address_is_refused, amqpbridge_5_1_the_topic_flag_form_is_dds_equals_address}`, amqp_codec (11): `crates/amqp-endpoint/src/broker.rs::tests::{amqp10_3_4_the_outcomes_are_their_described_lists, amqpbridge_4_3_a_message_above_the_frame_size_is_split_and_reassembled, amqp10_4_4_1_a_posted_message_names_the_transaction_on_every_frame, amqpbridge_4_1_the_open_carries_the_fields_of_the_spec, amqpbridge_4_2_the_attach_carries_name_role_node_settle_modes_properties_and_capability, amqpbridge_3_a_disposition_in_mode_second_leaves_the_delivery_unsettled, amqp10_4_2_the_attach_of_a_control_link_has_a_coordinator_as_its_target, amqp10_4_4_dispositions_and_flows_name_the_transaction, amqp10_4_5_the_reader_decodes_coordinator_and_transactional_states, amqp10_4_5_4_the_reader_takes_longer_txn_ids_only_when_told, a_full_session_window_refuses_the_transfer}`, link_state (12): `crates/amqp-endpoint/src/bridge_daemon/link_state.rs::tests::{amqpbridge_12_1_link_state_begin_all_begins_every_link_that_is_down, amqpbridge_12_1_link_state_the_brokers_begin_is_answered_with_the_attach, amqpbridge_12_1_link_state_an_attach_brings_the_link_up_and_a_receiver_grants_credit, amqpbridge_12_1_link_state_an_attach_without_a_node_sends_nothing, amqpbridge_12_1_link_state_credit_follows_part_2_2_6_7, amqpbridge_12_1_link_state_a_detach_is_answered_and_the_session_ended, amqpbridge_12_1_link_state_an_end_takes_the_link_down_until_the_back_off, amqpbridge_12_1_link_state_retry_due_begins_again_after_the_back_off, amqpbridge_12_1_link_state_a_lost_connection_puts_unsettled_deliveries_first, amqpbridge_12_1_link_state_credit_is_granted_again_at_half, amqpbridge_4_2_a_link_is_named_container_topic_role, amqpbridge_9_3_the_delay_grows_by_the_factor_up_to_the_maximum}`, disposition (8): `crates/amqp-endpoint/src/bridge_daemon/disposition.rs::tests::{amqpbridge_12_1_disposition_accepted_ends_the_deliveries_of_the_range, amqpbridge_12_1_disposition_rejected_ends_the_delivery_and_counts_an_error, amqpbridge_12_1_disposition_released_and_modified_send_again_up_to_max_attempts, amqpbridge_12_1_disposition_an_unknown_delivery_changes_nothing, amqpbridge_12_1_disposition_an_unsettled_outcome_is_settled_by_the_daemon, amqpbridge_12_1_disposition_the_brokers_settlement_frees_credit_in_mode_second, amqpbridge_12_1_disposition_an_inbound_outcome_goes_out_settled_or_in_mode_second, amqpbridge_4_4_a_failed_dds_write_releases_the_delivery}`, sasl (8): `crates/amqp-endpoint/src/broker/sasl.rs::tests::{amqpbridge_7_2_scram_client_final_matches_the_rfc_7677_example, amqpbridge_12_1_sasl_each_mechanism_has_its_initial_response, amqpbridge_12_1_sasl_the_configured_mechanism_is_sent_and_accepted, amqpbridge_12_1_sasl_a_mechanism_not_offered_is_refused_before_the_init, amqpbridge_12_1_sasl_an_outcome_other_than_ok_is_a_refusal, amqpbridge_12_1_sasl_a_challenge_for_plain_is_a_protocol_error, amqpbridge_12_1_sasl_a_broker_without_the_layer_is_named, amqpbridge_12_1_sasl_scram_checks_the_server_signature}`, dds_pump (8): `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::tests::{amqpbridge_12_1_dds_pump_a_sample_of_this_bridge_is_not_sent_back, amqpbridge_12_1_dds_pump_a_sample_becomes_one_message_with_its_key_on_a_keyed_topic, amqpbridge_12_1_dds_pump_a_lifecycle_marker_becomes_one_message_per_operation, amqpbridge_12_1_dds_pump_the_end_of_a_coherent_set_has_no_message, amqpbridge_12_1_dds_pump_a_full_queue_drops_its_oldest_message, amqpbridge_12_1_dds_pump_drain_sends_while_credit_lasts, amqpbridge_12_1_dds_pump_a_full_session_window_keeps_the_message, rtps_9_6_4_9_a_dispose_with_unregister_is_two_messages_dispose_first}`, acl (5): `crates/amqp-endpoint/src/bridge_daemon/acl.rs::tests::{amqpbridge_12_1_acl_read_and_write_are_separate_lists, amqpbridge_12_1_acl_the_wildcard_admits_every_subject, amqpbridge_12_1_acl_default_deny_decides_a_topic_no_rule_names, amqpbridge_12_1_acl_the_configured_rules_decide, amqpbridge_12_1_acl_live_rules_follow_a_reload}`. The tests of link_state, disposition and dds_pump run against the frames a
`BrokerWriter` writes to a socket pair, without a broker; those of sasl
against a broker side in the test that answers frame by frame.

**Status:** done — Linux test host: `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` 474 of 474 (18 skipped: the tests marked `#[ignore]`).

### §12.2 Integration tests

**Spec:** §12.2 — the daemon against RabbitMQ, round trip + DISPOSITION
sequence.

**Repo:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs`, `crates/amqp-endpoint/tests/bridge_e2e.rs`.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_3_dds_samples_arrive_in_the_queue_as_pass_through_messages`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_4_4_a_message_from_pika_reaches_a_reader_and_a_bad_body_does_not`, `crates/amqp-endpoint/tests/broker_tls_sasl.rs::amqpbridge_7_2_scram_sha_256_authenticates_and_a_wrong_password_is_refused`, `crates/amqp-endpoint/tests/bridge_e2e.rs::amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name` (the RabbitMQ cases are `#[ignore]`, Linux test host).

**Status:** done

### §12.3 Multi-vendor

**Spec:** §12.3 — Cyclone DDS subscriber + RabbitMQ + bridge; broker matrix.

**Repo:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs` (a Cyclone reader and a Cyclone writer through Cyclone's Python binding, qpid-proton at the broker); the key of an instance from a writer without `PID_KEY_HASH`: `crates/amqp-endpoint/src/bridge_daemon.rs` (`type_file` for the reader too), `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs` (`note_unknown_key_hash`, `sample_messages`), `crates/dcps/src/runtime.rs` (`DcpsRuntime::set_user_reader_key_support`, `UserSample::Lifecycle::key_hash_unavailable`); the broker matrix as §10a.

**Tests:** `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::ddsamqp_7_7_2_an_amqp_dispose_reaches_a_cyclone_reader`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_12_3_a_cyclone_writer_reaches_the_queue`, `crates/amqp-endpoint/tests/bridge_rabbitmq_e2e.rs::amqpbridge_12_3_a_cyclone_writer_without_type_file_gets_the_nil_handle` (Cyclone DDS 11.0.1, job `vendor-live-dispose-bridges`); `crates/amqp-endpoint/src/bridge_daemon/dds_pump.rs::tests::{amqpbridge_12_1_dds_pump_a_keyed_sample_without_key_hash_is_told_once_per_writer, amqpbridge_12_1_dds_pump_a_dispose_without_key_hash_goes_out_with_the_nil_handle, amqpbridge_8_1_key_hash_unavailable_is_logged_once_per_writer_with_its_fields}`; `crates/dcps/src/runtime.rs::tests::{set_user_reader_key_support_reads_the_keys_of_an_unannounced_type, rtps_8_7_10_a_lifecycle_marker_without_a_computable_key_hash_says_so}`; `crates/amqp-endpoint/src/bridge_daemon/config.rs::tests::amqpbridge_3_a_type_file_the_topic_would_not_read_is_refused`; the broker matrix with the tests of §10a.

**Open:** Azure Service Bus in the broker matrix (§10a).

**Status:** partial — measured on the Linux test host: a Cyclone reader behind the bridge (pika and qpid-proton → RabbitMQ → bridge → Cyclone, instances 7 and 8 and the dispose of 7) and a Cyclone writer in front of the bridge (Cyclone → bridge → RabbitMQ → qpid-proton: two samples in the order written with the content type of their encapsulation header, Cyclone's payload, `durable` under TRANSIENT_LOCAL, `dds:instance-handle` as the KeyHash, `group-id` from that KeyHash; the dispose of 7 with `dds:operation` `dispose`, the KeyHash of 7 and its `group-id`). The run showed: Cyclone 11.0.1 sends no `PID_KEY_HASH`, and the bridge gave `dds:instance-handle` as zeros. Now the reader computes the KeyHash from `type_file`. Without `type_file`, or when the body or the key does not decode as the type, such a sample and a dispose or unregister of such a writer go out with the nil handle (zeros, no `group-id`); `key_hash_unavailable` is logged and counted in `errors_total` once per writer. For the marker dcps reports this through `UserSample::Lifecycle::key_hash_unavailable`; before, such a dispose went out with the first 16 octets of the `serializedKey` as `dds:instance-handle`, without a log line. Measured against Cyclone without `type_file`: the two samples and the dispose of 7 with 16 zeros as `dds:instance-handle` and no `group-id`, `errors_total` 1; with `type_file` 0. The unit test of dds_pump checks the log line itself. The broker matrix is measured without Azure Service Bus (§10a).

## §13 Cross-references

### §13 Related library + OMG spec + standards

**Spec:** §13 — library `crates/amqp-bridge/` + `crates/amqp-endpoint/`,
OMG DDS-AMQP-1.0, ISO/IEC 19464:2014, wire format, deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config, major=wire-protocol
change (AMQP-1.0→2).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — rule for assigning the version number of a release, not a property of the daemon.

---

## Audit status

23 done / 5 partial / 0 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-amqp-bridge-daemon-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` (job `merge-rust-amqp-endpoint`) — 23 tests green.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (job `merge-rust-config`) — 3 tests green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 4 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor-amqp` (job `integration-bridges-vendor-amqp`) — 7 tests green.
* job `vendor-live-dispose-bridges` — 2 tests green.

39 cited tests green, 0 not green.

Open items: `zerodds-amqp-bridge-daemon-1.0.open.en.md`.
