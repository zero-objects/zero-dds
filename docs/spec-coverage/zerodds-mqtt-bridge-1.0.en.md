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

**Status:** n/a (informative) — assignment of the levels; the requirements are the items §1a (L6), §1b (L1: MQTT 3.1.1 and Keep Alive) and §2 to §12.

### §1a L6 Multi-tenant

**Spec:** §1 L6 — several DomainParticipants per daemon; one tenant binding
per MQTT client ID; §3 `tenants`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs::{TenantConfig, DaemonConfig::of_tenant, read_tenant, check_tenants}`,
`crates/mqtt-bridge/src/daemon/server.rs::{start_with_reload, start_core, TenantHandle, reload, settings_that_need_a_restart}`.

**Tests:** `crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_1_l6_each_tenant_has_its_participant_and_its_client_id, mqttbridge_1_l6_a_reload_applies_the_acl_of_a_tenant}`,
`crates/mqtt-bridge/tests/enhanced_auth.rs::mqttbridge_1_l6_a_tenant_presents_its_password_as_the_token`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_1_l6_the_tenants, mqttbridge_1_l6_a_tenant_authenticates_in_the_mode_of_the_top_level}`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_example_of_the_spec_loads`,
`crates/mqtt-bridge/src/daemon/server.rs::tests::mqttbridge_9_2_a_reload_names_the_settings_that_need_a_restart`.

**Status:** done — the tests ran green on the Linux test host. A tenant is a
DomainParticipant on its domain (GUID prefix from its client ID) and a broker
connection of its own under its `mqtt.client_id`, with its own topics and its
own ACL. It authenticates in the mode of the top level: under `password`
and SCRAM-SHA-256 its user name and password replace the top level's; where
the top level presents `auth.bearer_token` (`password` without
`auth.password`, OAUTHBEARER, another method), its password is the token. A
user name under a token method and values that `none` or `mtls` do not use
prevent the start with `tenants[<name>].mqtt` in the message. Before, the
load refused a tenant with its own `mqtt.password` under `enhanced` with
OAUTHBEARER (exit 1, measured), because `of_tenant` wrote the password into
the token and into `mqtt.password` as well; now the tenant's CONNECT carries
its token against the test broker. Checked against the crate's broker server: both connections stand
under their client IDs, a message on the tenant's MQTT topic reaches only its
participant, one on the top level's topic only the top level's participant
(both with a topic of the same DDS name), a sample of the tenant's
participant goes out on the tenant's topic. Two tenants of one name and one
client ID for two connections prevent the start. A SIGHUP takes over each
tenant's ACL at once (after `default_deny: true` without a rule the tenant's
samples no longer go out, the top level's do) and names any other change of
`tenants` as needing a restart.
Against a foreign broker the tenant binding is not checked separately. A
tenant's log lines carry `tenant <name>:`; the metrics (§8.2) carry it in
the label `dds_topic` (`<tenant>/<topic>`).

### §1b L1 MQTT 3.1.1 and Keep Alive

**Spec:** §1 L1 — the daemon speaks MQTT 5 and MQTT 3.1.1 as a compatibility
mode (`mqtt.protocol_version`, §3); it keeps the connection alive with
PINGREQ (MQTT 5.0 and 3.1.1 §3.1.2.10).

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs::{connect_body, MqttClient::wait_connack, MqttClient::keep_alive_tick, ClientError::is_auth_refusal}`,
`crates/mqtt-bridge/src/daemon/server.rs::start_core` (the Keep Alive in the
receive loop), `crates/mqtt-bridge/src/version.rs` and the `_v` codecs in
`crates/mqtt-bridge/src/control_packets.rs` and `crates/mqtt-bridge/src/codec.rs`.

**Tests:** `crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_1_l1_the_connect_of_the_3_1_1_mode, mqttbridge_1_l1_an_idle_client_pings_and_a_missing_pingresp_ends_the_connection, mqttbridge_1_l1_a_pingresp_keeps_the_connection, mqttbridge_4_2_the_properties_of_an_inbound_publish_reach_the_event}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_1_l1_the_daemon_speaks_mqtt_3_1_1, mqttbridge_1_l1_an_idle_daemon_stays_connected_past_one_and_a_half_keep_alives, mqttbridge_2_exit_5_on_connack_0x87, mqttbridge_4_4_under_3_1_1_a_dispose_is_not_published_as_a_sample}`;
against Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_qos.rs::{mqttbridge_1_l1_the_daemon_speaks_mqtt_3_1_1_to_mosquitto, mqttbridge_1_l1_an_idle_daemon_stays_connected_past_one_and_a_half_keep_alives}`
(`#[ignore]`, job `integration-bridges-vendor`).

**Status:** done — the tests ran green on the Linux test host. Under
`mqtt.protocol_version: 3.1.1` the daemon sends CONNECT with protocol level 4
and PUBLISH, acks and DISCONNECT without properties; a dispose or an
unregister is then not published (§4.4). Mosquitto lists it as a
3.1.1 client (`p2`), messages go both ways, also with Mosquitto's 3.1.1
clients. The CONNACK codes 4 and 5 are exit 5 (§2). In both versions the
daemon sends PINGREQ when it has sent nothing for the Keep Alive; before, it
sent none, and a broker could close a silent connection after 1.5 Keep Alive
periods. Mosquitto (Keep Alive 2 s, 5.5 s waited) and the crate's broker
server (1 s, 3.5 s waited) keep the daemon's connection; the broker server
drops a silent client of the same Keep Alive. When the PINGRESP does not
come within half the Keep Alive, the daemon closes the connection; a new
connection after that (spec §9.3) is still missing.

## §2 CLI surface

### §2 zerodds-mqtt-bridged CLI

**Spec:** §2 — options
`--config`/`--broker`/`--client-id`/`--domain`/`--username`/`--password`/`--tls-*`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4/5.

**Repo:** `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`,
`crates/mqtt-bridge/src/daemon/cli.rs` (`CliArgs::sets`),
`crates/mqtt-bridge/src/daemon/config.rs::default_client_id`,
`crates/mqtt-bridge/src/daemon/server.rs::{map_client_err, security_error, ServerError}`,
`crates/mqtt-bridge/src/daemon/client.rs::{open_stream, handshake_error, connect_tcp, connect_with_backoff, ClientError::is_auth_refusal}`,
`crates/dcps/src/participant.rs::host_name`.

**Tests:** `crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_2_exit_1_on_a_config_error, mqttbridge_2_exit_2_when_no_broker_listens, mqttbridge_2_exit_2_when_the_tls_handshake_gets_no_answer, mqttbridge_2_exit_3_when_the_participant_cannot_start, mqttbridge_2_exit_4_on_an_untrusted_broker_certificate, mqttbridge_2_exit_4_when_the_tls_material_does_not_load, mqttbridge_2_exit_5_on_connack_0x87, mqttbridge_2_every_flag_reaches_the_connect_and_the_subscribe, mqttbridge_9_1_sigterm_during_the_connect_retries_exits_0}`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_2_the_default_client_id_names_the_host`,
`crates/mqtt-bridge/src/daemon/server.rs::tests::mqttbridge_2_the_exit_class_of_a_failed_connect`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_2_which_connack_refusal_is_an_auth_error, mqttbridge_9_1_a_broker_that_comes_up_before_the_connect_timeout_is_reached, mqttbridge_9_1_a_tls_handshake_without_an_answer_is_tried_again}`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_new_keys_refuse_what_they_do_not_accept`,
`crates/mqtt-bridge/src/daemon/cli.rs::tests::{every_convenience_flag_becomes_a_set, equals_form, domain_int, unknown_flag_rejected, help_short_form}`,
`crates/mqtt-bridge/tests/contract_dcps.rs::{a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file, the_daemon_environment_name_sits_beneath_the_daemon_file, the_file_the_config_flag_names_must_exist}`;
exit 0 after SIGTERM against Mosquitto 2.0.21
`crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_9_2_sigterm_disconnects_and_exits_0`.

**Status:** done — the tests ran green on the Linux test host. Each option is
`--set` of its key. Through the binary against the crate's broker server,
`--broker`, `--client-id`, `--username` and `--password` reach the CONNECT,
`--topic` the SUBSCRIBE (a message on the topic reaches DDS) and `--metrics`
the admin endpoint, which counts it; `--domain`, `--log-level` and `--tls-*`
appear in the output of `--check-config`. The exit codes through the binary:
1 (a value outside its range, `mqtts://` without a TLS section,
`verify_hostname: false`, `mqtt.tls.cert_file` without `key_file` and the
other way round; a `mqtt.session_dir` the daemon cannot create or write, or
a Session State another process holds; through the binary with a directory
below a file), 2 (no broker; the start tries again after the waits of
`mqtt.reconnect`, shortens the wait that would pass
`mqtt.connect_timeout_secs` and makes a last attempt at that time; before, it
ended as soon as the next wait would pass the time; with the defaults against
a closed port it now ends after 30.3 s with the waits 0.5, 1, 2, 4, 8 and
14.5 s, before after 15.8 s (both measured through the binary on the Linux
test host); a broker that does not answer the TLS handshake or
closes the connection during it is tried again too, before exit 4 without a
second attempt; an attempt waits at most 10 s for the TCP connection of each
address, also for a host name, and 5 s for each answer; a CONNACK refusal
0x85), 3 (domain 300 has no ports), 4 (an untrusted broker certificate;
before, exit 2, since rustls ran the handshake only when the CONNECT was
written; TLS material that does not load, such as a missing
`mqtt.tls.ca_file`; only a TLS error of rustls is exit 4, a timeout or a
closed connection in the handshake is not), 5 (CONNACK 0x86, 0x87, under MQTT 3.1.1 code 5), 0
(SIGTERM, also during the connect attempts). The default client ID is
`zerodds-bridge-<host>` (before, `zerodds-bridge`). The rule for exit 5
(0x86, 0x87, 0x8A, 0x8C; 3.1.1: 4, 5) reads the same in §2 and §4.1; before,
§2 named 0x86/0x87 and §4.1 every refusal from 0x80. A configuration error
of an auth mode is exit 1 (before, exit 4). The effect of `--log-level` on
the output belongs to §8.1 (open).

### §2 Broker schemes mqtt/mqtts/ws/wss

**Spec:** §2 — `--broker` takes `mqtt://`, `mqtts://`, `ws://`, `wss://`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (`parse_broker_url`,
`BrokerUrl`), `crates/mqtt-bridge/src/daemon/ws_transport.rs` (MQTT 5.0 §6
over RFC 6455), `crates/mqtt-bridge/src/daemon/client.rs::connect_url`,
`crates/websocket-bridge/src/handshake.rs` (`render_client_request`,
`validate_server_response`).

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_2_parse_broker_url_reads_the_websocket_schemes, mqttbridge_2_parse_broker_url_refuses_what_it_would_not_use}`,
`crates/websocket-bridge/src/handshake.rs::tests::mqttbridge_2_the_client_side_validates_the_server_response`;
against Mosquitto 2.0.21 (Linux test host)
`crates/mqtt-bridge/tests/websocket_broker.rs::{mqtt5_6_ws_broker_url_carries_mqtt_both_ways_through_mosquitto, mqtt5_6_wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate, mqtt5_6_a_ws_url_on_a_plain_listener_fails_the_handshake}`
(both directions, one message above 64 KiB, an untrusted certificate and
`ws://` on a plain MQTT listener refused).

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`log_level`/`mqtt`/`auth`/`topics`/
`tenants`/`acl`/`metrics`; ENV substitution `${VAR}` and `${VAR:-default}`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (`KEYS`,
`DaemonConfig::from_config`, `ReconnectConfig`, `TopicConfig::{effective_mqtt_qos, effective_retain, history_depth}`),
`crates/mqtt-bridge/src/daemon/history.rs` (`HistoryQueue`),
`crates/mqtt-bridge/src/daemon/config.rs::default_session_dir` (`mqtt.session_dir`),
`crates/config/src/parse.rs` (the shared parser),
`crates/mqtt-bridge/src/daemon/runtime_common.rs::serve_admin_endpoints`
(`metrics.path`), `crates/mqtt-bridge/src/daemon/server.rs::register_topic`, `crates/mqtt-bridge/src/daemon/pump.rs::Pump`
(`qos.history`), `crates/mqtt-bridge/src/daemon/client.rs::{connect_body, alpn_offer, connect_with_backoff}`.

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_3_the_example_of_the_spec_loads, mqttbridge_3_an_absent_mqtt_qos_is_none, mqttbridge_3_the_defaults_of_the_keys, mqttbridge_3_the_new_keys_refuse_what_they_do_not_accept, mqttbridge_3_the_reconnect_delays_grow_by_the_factor_up_to_the_maximum, what_the_old_parser_dropped_is_refused_with_its_line, mqttbridge_3_the_own_topic_prefix_is_read_and_reaches_the_tenants, mqttbridge_3_data_representation_names_the_representation_of_the_topic}`
(the example of `docs/specs/zerodds-mqtt-bridge-1.0.md` §3, each key with its
value),
`crates/mqtt-bridge/src/daemon/qos_translation.rs::tests::mqttbridge_6_data_representation_is_the_offer_of_both_endpoints`,
`crates/mqtt-bridge/tests/daemon_e2e.rs::{mqttbridge_3_data_representation_xcdr2_matches_an_xcdr2_reader_before_the_first_publish, mqttbridge_3_data_representation_xcdr2_reader_matches_no_xcdr1_writer}`,
`crates/mqtt-bridge/src/daemon/status_watch.rs::tests::mqttbridge_6_the_reports_go_under_the_own_topic_prefix`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::mqttbridge_3_the_metrics_are_served_on_metrics_path`,
`crates/mqtt-bridge/tests/security_e2e.rs::mqttbridge_3_the_alpn_list_of_the_configuration_is_offered`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_9_1_an_unreachable_broker_is_retried_until_the_connect_timeout, mqttbridge_9_1_a_broker_that_comes_up_before_the_connect_timeout_is_reached}`,
`crates/mqtt-bridge/src/daemon/history.rs::tests::{mqttbridge_6_keep_last_keeps_the_newest_n_of_each_instance, mqttbridge_6_keep_all_drops_nothing, mqttbridge_6_a_sample_put_back_is_the_oldest_or_dropped}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_6_keep_last_on_an_out_topic_publishes_the_newest_n, mqttbridge_6_keep_all_on_an_out_topic_drops_nothing}`,
`crates/mqtt-bridge/tests/example_yaml_loadback.rs::{mqttbridge_3_example_yaml_parses_without_error, mqttbridge_3_example_yaml_key_fields_match_intent}`
(the shipped example configuration),
`crates/config/tests/layering.rs::environment_references_expand_before_the_parse`.

**Status:** done — the tests ran green on the Linux test host. The example
of the specification loads; before, the parser refused
`mqtt.session_expiry_interval_secs`, `mqtt.max_packet_size`,
`mqtt.receive_maximum`, `mqtt.reconnect.*`, `mqtt.tls.verify_hostname`,
`mqtt.tls.alpn`, `qos.history.kind` and `metrics.path`. Each key takes
effect: `mqtt.session_dir` (default `/var/lib/zerodds/mqtt-bridge`, not
empty) as the place of the Session State of a session that outlives its
connection (§4.1), the four CONNECT keys in §4.1, `mqtt.reconnect` and
`mqtt.connect_timeout_secs` in the connect of the start (§9.1; the wait
that would pass the time is shortened to it, the last attempt starts there),
`mqtt.tls.alpn` in the TLS handshake, `metrics.path` at the admin endpoint,
`qos.history` at a topic's DataWriter (`in`, `bidir`) and at the samples the
daemon holds for the broker while it acknowledges nothing (`out`, `bidir`;
KEEP_LAST N per instance, older ones are dropped and counted in
`zerodds_mqtt_bridge_dds_samples_dropped_total`, KEEP_ALL drops none, also not after
the 10 s a PUBLISH waits for a free slot; before, `qos.history` had no effect
on an `out` topic, and a sample was lost after 10 s without an
acknowledgement); `mqtt.tls.verify_hostname: false`
prevents the start with a reason, since the name is always checked. An
absent `mqtt_qos` or `retain` is `None` and derived (§6); a value that is set
stays, also `mqtt_qos: 0` on a reliable topic (before, it became 1). §3 now
names `auth`, `mqtt.tls.server_name`, `mqtt.protocol_version`,
`mqtt.topic_alias_maximum`, `mqtt.connect_timeout_secs` and `tenants`. A
reconnect with `mqtt.reconnect` after a lost connection and the samples that
arrive meanwhile belong to spec §9.3. Two keys came from the broker matrix
of §10a: `mqtt.own_topic_prefix` (default `$zerodds`) holds the catalog,
the reports and the will (HiveMQ 4.57.0 refuses a will topic beginning with
`$` with CONNACK 0x82); the topic key `data_representation` (`xcdr1`,
`xcdr2`) is the one representation both DDS endpoints of the topic use from
their creation, a PUBLISH of the other XCDR version is refused (without
it, the XCDR2 readers of Cyclone DDS, Fast DDS and RTI Connext matched no
writer of the daemon). On an `out` or `bidir` topic the DataReader then
accepts that representation alone: a DDS writer that offers only the other
one matches no reader of the daemon (DDS-XTypes 1.3 §7.6.3.1.2). The
example configuration set the key on its `out` topic and so shut out XCDR1
writers; it is now on the `in` topic `Command`, and the comment, §3 of the
specification and the man page name the effect on the reader.

## §4 MQTT wire protocol

### §4.1 CONNECT with MQTT-5 properties

**Spec:** §4.1 — CONNECT with
Session-Expiry/Receive-Maximum/Max-Packet-Size/Topic-Alias-Max/Request-Response-Information/Request-Problem-Information,
user properties `zerodds_version`/`zerodds_role`, optional
Authentication-Method/Authentication-Data; a CONNACK refusal of the
credentials → exit 5; what CONNECT and CONNACK announce holds.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs::{connect_body, MqttClient::wait_connack, MqttClient::publish, MqttClient::resolve_topic, MqttClient::take_buffered_packet, MqttClient::establish, MqttClient::resume, MqttClient::read_event, malformed, BrokerLimits}`,
`crates/mqtt-bridge/src/daemon/session.rs` (`SessionState`, the file in `mqtt.session_dir`; `append_whole`: a record the disk takes only in part is cut off again),
`crates/mqtt-bridge/src/property_set.rs` (properties by Table 2-4),
`crates/mqtt-bridge/src/daemon/server.rs::start_core` (the limits of the
CONNACK per topic in the log); Authentication Method and Data under
`auth.mode: enhanced` see §7.2 (`MqttClient::send_connect`,
`MqttClient::wait_connack`).

**Tests:** `crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_4_1_the_connect_carries_the_properties_of_the_table, mqttbridge_4_1_the_server_keep_alive_replaces_the_configured_one, mqttbridge_4_1_the_limits_of_the_connack_hold_for_the_publish, mqttbridge_4_1_an_inbound_topic_alias_is_resolved, mqttbridge_4_1_a_packet_above_the_maximum_packet_size_is_0x95, mqttbridge_4_1_qos_2_beyond_our_receive_maximum_is_0x93, mqttbridge_4_1_a_resumed_session_resends_its_state, mqttbridge_4_1_the_resend_holds_the_receive_maximum, mqttbridge_4_1_session_present_without_state_begins_a_new_session, mqttbridge_4_1_session_present_after_clean_start_is_a_protocol_error, mqttbridge_4_1_a_packet_that_does_not_decode_ends_the_connection_with_0x81}`,
`crates/mqtt-bridge/src/daemon/session.rs::tests::{mqttbridge_4_1_the_session_state_outlives_the_client, mqttbridge_4_1_session_present_0_and_clean_start_discard_the_state, mqttbridge_4_1_a_cut_off_or_foreign_session_file, mqttbridge_4_1_one_process_holds_a_session_state, mqttbridge_4_1_the_session_file_is_rewritten_with_the_live_entries, mqttbridge_4_1_a_record_the_disk_takes_in_part_is_cut_off_again}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_4_1_the_broker_reads_each_connect_property, mqttbridge_4_1_an_inbound_topic_alias_is_resolved, mqttbridge_4_1_the_maximum_qos_of_the_connack_holds_for_the_publish, mqttbridge_4_1_a_restarted_daemon_resends_what_the_broker_did_not_acknowledge}`;
`crates/mqtt-bridge/tests/enhanced_auth.rs::{mqttbridge_7_2_enhanced_connect_carries_method_and_data, mqttbridge_7_2_an_auth_continue_is_answered_before_connack}`;
against Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_4_1_the_session_survives_a_reconnect_with_clean_start_false`
(`#[ignore]`, job `integration-bridges-vendor`).

**Status:** done — green on the Linux test host. The CONNECT carries
Authentication Method and Authentication Data under `auth.mode: enhanced`
(§7.2), with the exchange of AUTH up to the CONNACK. Before,
the CONNECT carried no property, and `clean_start: false` had no effect
without a Session Expiry Interval. Now it carries Session Expiry Interval,
Receive Maximum, Maximum Packet Size, Topic Alias Maximum
(`mqtt.topic_alias_maximum`, default 100), Request Response Information 0,
Request Problem Information 1 and the two user properties (octets checked by
MQTT 5.0 §3.1.2.11). Against Mosquitto the session with `clean_start: false`
and Session Expiry Interval 300 survives the disconnect (Session Present 1,
three QoS 1 messages from the gap arrive), with interval 0 it does not. What
the CONNECT announces holds for the broker: a packet above Maximum Packet
Size ends the connection with DISCONNECT 0x95, a Topic Alias above the
maximum with 0x94, aliases below it are resolved, more unreleased QoS 2
messages than Receive Maximum end with 0x93, a packet of the broker that
does not decode (a PUBLISH with a Topic Name cut short, the reserved packet
type 0, CONNACK, PUBACK, PUBREC, PUBREL, PUBCOMP, SUBACK, AUTH, Remaining
Length) with 0x81 Malformed Packet (MQTT 5.0 §4.13.1); before, it ended as
`ClientError::Codec` without a DISCONNECT. What the CONNACK announces holds
for the daemon: Maximum QoS, Retain Available and Maximum Packet Size prevent
the PUBLISH of a sample (log line, error counter, the connection stays);
Server Keep Alive replaces `keep_alive_secs`; Session Present and Assigned
Client Identifier are read. Session State (MQTT 5.0 §4.1, [MQTT-3.1.2-23]):
under `clean_start: false` with a Session Expiry Interval above 0 (3.1.1:
always) the daemon keeps its QoS 1/2 PUBLISH packets the broker has not
completed and the broker's QoS 2 PUBLISH packets before their PUBREL in a
file per client ID in `mqtt.session_dir`, written before the packet; after a
restart, on Session Present 1 it resends the PUBLISH packets with DUP 1 and
the PUBREL packets with their packet identifiers in their order, within the
broker's Receive Maximum, and does not deliver a QoS 2 PUBLISH answered
before the restart into DDS again; on Session Present 0 it discards the
state; Session Present 1 without state closes the connection and begins a
new session with Clean Start 1 ([MQTT-3.2.2-4], under 3.1.1 then a third
CONNECT with Clean Session 0); Session Present 1 to Clean Start 1 is a
protocol error (0x82). Not run through the binary; against the test's own
broker over two starts of the daemon in the test process. Before, every start
began at packet identifier 1 with no state, nothing was resent, and a QoS 2
flow of the previous process that stopped after PUBREC would have left its
identifier open at the broker (derived, not run against a broker). The file
is not synced to the disk: a crash of the process keeps it, a crash of the
operating system can lose the last records. A record the disk takes only in
part (a full disk) is cut off again; when that fails, the file is written
again before the next record. Before, the next record followed the broken
one, and the file no longer read back correctly (in the test with a disk
that fails after a number of octets; a real write error of a disk was not
produced). Exit 5: see §2.

### §4.2 PUBLISH with Content-Type + user properties

**Spec:** §4.2 — PUBLISH with `Payload Format Indicator=0`, Content-Type
`application/x-dds-cdr2` (XCDR1: `application/x-dds-cdr`), user properties
`zerodds_type`/`zerodds_topic`/`zerodds_flags`/`zerodds_key_hash`/`zerodds_source_ts_ns`,
Message Expiry Interval from the lifespan; the sample's encapsulation header
+ CDR; MQTT→DDS with source timestamp and key hash.

**Repo:** `crates/mqtt-bridge/src/daemon/pump.rs` (`publish_properties`,
`inbound_sample`, `content_type_of`, `timestamp_ns`, `message_expiry_of`),
`crates/mqtt-bridge/src/daemon/server.rs::start_core` (pump and receive
loop), `crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::publish, InboundEvent}`.

**Tests:** `crates/mqtt-bridge/src/daemon/pump.rs::tests::{mqttbridge_4_2_the_properties_of_a_sample, mqttbridge_4_2_what_a_sample_does_not_have_is_left_out, mqttbridge_4_2_the_source_timestamp_round_trips_through_nanoseconds, mqttbridge_4_2_an_inbound_publish_and_what_is_refused}`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_4_2_the_publish_carries_its_properties, mqttbridge_4_2_the_properties_of_an_inbound_publish_reach_the_event}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_4_2_a_dds_sample_is_published_with_content_type_and_user_properties, mqttbridge_4_2_an_inbound_source_timestamp_is_the_dds_source_timestamp}`,
`crates/mqtt-bridge/tests/daemon_e2e.rs::xtypes_7_6_3_1_2_a_publish_carries_the_header_the_sample_came_under`;
against Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_qos.rs::{mqttbridge_4_2_mosquitto_sub_prints_the_user_properties, mqttbridge_4_2_mosquitto_pub_user_properties_reach_dds}`
(`#[ignore]`, job `integration-bridges-vendor`).

**Status:** done — the tests ran green on the Linux test host. Before, the
PUBLISH carried no property, and an inbound PUBLISH lost timestamp and key
hash. Now it carries Payload Format Indicator 0, the Content Type of the
representation, `zerodds_type`, `zerodds_topic`, `zerodds_flags` (`0x00` for
a sample), `zerodds_key_hash` and `zerodds_source_ts_ns` when the sample has
them, and the Message Expiry Interval of its lifespan (seconds, rounded, at
least 1); `mosquitto_sub` prints them. Inbound, key hash and source timestamp
go to the DDS write (checked with `mosquitto_pub -D`); a Payload Format
Indicator 1, a foreign Content Type or one of the other XCDR version than the
header is refused with a log line and the error counter. The header is the
one the sample came under (spec §4.2 now says so; before, it fixed
`00 07 00 00`). Under MQTT 3.1.1 there are no properties (§1b).
`zerodds_flags` of a dispose and `zerodds_op` belong to §4.4.

### §4.3 SUBSCRIBE with Subscription-Identifier

**Spec:** §4.3 — SUBSCRIBE per `direction=in|bidir` with a
Subscription-Identifier, QoS derived from DDS, NoLocal=1.

**Repo:** `crates/mqtt-bridge/src/daemon/topic_map.rs::InboundRoutes` (one
subscription and one Subscription Identifier per topic filter, from 1 in
the order of the configuration; the topics of a PUBLISH by its identifiers
and the filter), `crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::subscribe, SubscribeEntry, SubscribeOutcome, subscription_options}`
(one SUBSCRIBE per filter with property 0x0B, options QoS | No Local, the
wait for each SUBACK; no identifier under Subscription Identifier Available
0, no wildcard filter under Wildcard Subscription Available 0; under MQTT
3.1.1 the QoS alone), `crates/mqtt-bridge/src/daemon/server.rs::subscribe_routes`, `crates/mqtt-bridge/src/daemon/pump.rs::{EchoGuard, Pump}`
(a refusal in the SUBACK as a log line and an error count; the DataReader
of a bidir topic does not publish the samples of the bridge's own
DataWriter; under 3.1.1 the copy of the bridge's own PUBLISH is not written
into DDS: the record in the `EchoGuard` is made before the send under the
client's lock and is taken out again when the PUBLISH fails).

**Tests:** `crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_4_3_the_subscribe_carries_identifier_and_no_local, mqttbridge_4_3_what_the_broker_or_the_protocol_does_not_have_is_not_sent}`,
`crates/mqtt-bridge/src/daemon/topic_map.rs::tests::{mqttbridge_4_3_one_subscription_and_identifier_per_filter, mqttbridge_4_3_the_identifier_selects_the_topic}`,
`crates/mqtt-bridge/src/daemon/pump.rs::tests::mqttbridge_4_3_the_echo_record_comes_before_the_publish_and_goes_when_it_fails`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_4_3_each_filter_is_subscribed_with_its_identifier_and_no_local, mqttbridge_4_3_a_refused_subscription_is_logged, mqttbridge_4_3_a_bidir_topic_does_not_send_its_own_messages_back, mqttbridge_4_3_under_3_1_1_the_copy_of_its_own_publish_is_not_written}`;
against Mosquitto `crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_4_3_a_bidir_topic_does_not_get_its_own_publish_back`.

**Status:** done — green on the Linux test host, the Mosquitto probe
against Mosquitto 2.0.21. Under 3.1.1 the record in the `EchoGuard` used
to be made after the lock was released; when the receive loop read the
broker's copy in between, it wrote it into DDS a second time (found by
reading; the race is not reproduced). The spec contradicted itself on the QoS (§4.3
RELIABLE→2, §6 default 1); §4.3 now refers to the derivation of §6. Before,
the daemon sent one SUBSCRIBE for all filters without identifier and
without No Local, read no SUBACK and mapped inbound messages only by the
exact name or a text prefix before `/#`.

### §4.4 zerodds_op control property

**Spec:** §4.4 — user property `zerodds_op` values
`sample`/`dispose`/`unregister`/`register`; default `sample`.

**Repo:** `crates/mqtt-bridge/src/daemon/pump.rs::{Op, op_of, lifecycle_message, lifecycle_properties, inbound_message, InboundMessage}`
(DDS → MQTT: dispose `dispose` 0x01, unregister `unregister` 0x02, both
`dispose` 0x03, the key hash, the header `00 06 00 00` and the key holder;
MQTT → DDS: the four values, `zerodds_flags` agreeing with the operation),
`crates/mqtt-bridge/src/daemon/pump.rs::Pump::take`, `crates/mqtt-bridge/src/daemon/server.rs::write_inbound`
(`UserSample::Lifecycle` into the history of its instance, with the QoS and
the retain flag of the topic; under MQTT 3.1.1 not published, the first
change of a topic logged, each counted as an error, the instance leaves the
replay buffer in its turn in the history; inbound
`DcpsRuntime::write_user_lifecycle`, `register` writes nothing).

**Tests:** `crates/mqtt-bridge/src/daemon/pump.rs::tests::{mqttbridge_4_4_a_dispose_marker_is_op_dispose_with_the_key_hash, mqttbridge_4_4_a_missing_op_is_sample, mqttbridge_4_4_an_unknown_op_is_refused}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_4_4_a_dds_dispose_reaches_mqtt_and_an_mqtt_dispose_reaches_a_dds_reader`
(a second participant of the test over RTPS: its dispose and unregister
arrive as PUBLISH packets with `zerodds_op`, a PUBLISH with `dispose`
reaches its reader as NOT_ALIVE_DISPOSED),
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_4_4_under_3_1_1_a_dispose_is_not_published_as_a_sample`
(under 3.1.1 the subscriber gets the two samples alone, the dispose and the
unregister count two errors).

**Status:** done — green on the Linux test host. MQTT 3.1.1 has no
properties: a dispose or an unregister is not published and is counted
(spec §4.4, §1 L1); before, it went out as a PUBLISH with `00 06 00 00` and
the key holder, which every receiver read as a sample and which replaced
the last sample on a retained topic. Before, the pump dropped
every `UserSample::Lifecycle` and inbound user properties were not read;
the cited `properties.rs::tests` had no test of `zerodds_op`. A dispose to
a reader of another vendor belongs to §10.

## §5 Topic mapping

### §5.1 Slug algorithm DDS → MQTT

**Spec:** §5.1 — lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; override per
`mqtt_topic`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs::{default_mqtt_slug, check_mqtt_topic}`
(the slug; the `mqtt_topic` of an `out` or `bidir` topic is a topic name
without `+` and `#`, the one of an `in` topic a valid topic filter, else a
configuration error with file and line; `server::start` also checks a
configuration built in code), `crates/mqtt-bridge/src/daemon/topic_map.rs::InboundRoutes::route`
(inbound through `crate::topic_filter::matches`, level by level),
`packaging/linux/configs/mqtt-bridged.yaml.example` (the `out` topic
without a wildcard, an `in` topic with a filter).

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_5_1_slug_handles_double_colon, mqttbridge_5_1_slug_replaces_unsafe_chars, mqttbridge_5_1_slug_of_three_levels, mqttbridge_5_1_an_out_topic_with_a_wildcard_is_refused, mqttbridge_5_1_an_in_topic_may_be_a_filter}`,
`crates/mqtt-bridge/src/daemon/topic_map.rs::tests::{mqttbridge_5_1_a_plus_filter_maps_its_topics, mqttbridge_5_1_a_hash_filter_stops_at_the_level_separator}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_5_1_a_filter_topic_takes_every_topic_it_matches`,
`crates/mqtt-bridge/tests/example_yaml_loadback.rs::mqttbridge_11_example_yaml_topics_reach_the_daemon`;
the filter side in `crates/mqtt-bridge/src/topic_filter.rs::tests`
(`mqtt311_3_8_validate_simple_filter`, `mqttbridge_5_1_topic_name_with_wildcard_rejected`,
`mqttbridge_5_1_match_exact_topic`, `mqttbridge_5_1_match_plus_wildcard`, `mqttbridge_5_1_match_hash_wildcard`).

**Status:** done — green on the Linux test host. Before, `mqtt_topic` was
not checked: the shipped example published on `factory/+/telemetry`, which
MQTT 5.0 [MQTT-3.3.2-2] forbids; inbound, `sensors/#` also took
`sensorsX/a`, and `+` was not matched.

### §5.2 Catalog retain on $zerodds/<client_id>/catalog

**Spec:** §5.2 — catalog retain JSON with a topics list.

**Repo:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::{CatalogSnapshot::render_json, catalog_topic}`
(the JSON of the spec: `topics` with `dds_name`, `dds_type`, `mqtt_topic`,
`qos` with `reliability` and `durability`, `schema_url` only when set; the
same document on `/catalog` of the admin endpoint),
`crates/mqtt-bridge/src/daemon/server.rs::{publish_catalog, clear_catalog, Session::track_catalog, Core::drain}`
(after the CONNACK, retained, QoS 1, Payload Format Indicator 1, Content
Type `application/json` on `$zerodds/<client_id>/catalog`, per tenant with
its topics; after a reconnect under a newly assigned client identifier the
catalog of the identifier before is cleared; at the shutdown a retained
PUBLISH without payload before the DISCONNECT), `crates/mqtt-bridge/src/daemon/config.rs::TopicConfig::schema_url`
(key `topics[].schema_url`, spec §3, man5, example configuration).

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::{mqttbridge_5_2_the_catalog_is_the_json_of_the_spec, mqttbridge_5_2_admin_endpoint_serves_paths}`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_example_of_the_spec_loads`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_5_2_the_catalog_is_retained_and_cleared_at_the_shutdown, mqttbridge_5_2_a_tenant_publishes_the_catalog_of_its_topics_and_clears_it, mqttbridge_5_2_a_new_assigned_identifier_clears_the_catalog_of_the_one_before}`
(the tenant's catalog under its client identifier with its topics alone,
both cleared at the shutdown; with an empty `mqtt.client_id` and a
reconnect under a newly assigned identifier the bridge is listed once,
under the new identifier, and a missed deadline is reported under the new
one); against Mosquitto `crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_5_2_a_late_subscriber_gets_the_retained_catalog`.

**Status:** done — green on the Linux test host; Mosquitto 2.0.21 takes
the PUBLISH on `$zerodds/…` and keeps it retained (measured; MQTT 5.0
§4.7.2 leaves `$` topics to the server). EMQX and HiveMQ not measured
(§10a). Before, after a reconnect under a newly assigned client identifier
the catalog of the identifier before stayed retained, and the reports of §6
kept going out under the identifier of the first connection. Before, the
daemon published no catalog to the broker; the JSON on
`/catalog` differed from §5.2 (`service`, `version`, `direction`,
`mqtt_qos`, history, no `schema_url`).

## §6 QoS translation

### §6 DDS QoS → MQTT behaviour + auto-derivation

**Spec:** §6 — reliability/durability/history/lifespan/deadline/
liveliness/partition map; auto-derivation of `mqtt_qos` from reliability,
`retain` from durability.

**Repo:** `crates/mqtt-bridge/src/daemon/qos_translation.rs::{mqtt_qos, retain, durability, replayed, reader_config, writer_config, lifespan_nanos, message_expiry}`
(the rows of the table; `register_topic` and the SUBSCRIBE use them),
`crates/mqtt-bridge/src/daemon/config.rs::TopicConfig` (keys
`qos.lifespan_ms`, `qos.deadline_ms`, `qos.liveliness.{kind, lease_ms}`,
`qos.partition`; `mqtt_qos` and `retain` stay when set),
`crates/mqtt-bridge/src/daemon/history.rs::HistoryQueue` (KEEP_LAST/KEEP_ALL
of the samples that wait for the broker or the connection),
`crates/mqtt-bridge/src/daemon/replay.rs::ReplayBuffer` (TRANSIENT in
memory, PERSISTENT also in `<client id>.replay` in `mqtt.session_dir`; the
newest sample of each instance that went out, published again after each
CONNACK, the expired ones not),
`crates/mqtt-bridge/src/daemon/client.rs::MqttClient::publish_until` (the
Message Expiry Interval is the time left at the send, after the wait for a
send slot; expired `ClientError::Expired`, not sent), `crates/mqtt-bridge/src/daemon/status_watch.rs::StatusWatch`
(a missed deadline of the DataReader and the DataWriter as a retained
PUBLISH on `$zerodds/<client_id>/deadline_missed/<slug>`, a lost liveliness
of a DDS writer as a PUBLISH on `$zerodds/<client_id>/liveliness_lost/<slug>`),
`crates/mqtt-bridge/src/daemon/client.rs::{connect_body, liveliness_topic}`
(the will of the CONNECT on `$zerodds/<client_id>/liveliness_lost`),
`crates/mqtt-bridge/src/daemon/pump.rs::{Pump, message_expiry_of}` (seconds,
rounded without an overflow, at most 4294967295), `crates/mqtt-bridge/src/daemon/server.rs::{Session::replay_to, reconnect}`.

**Tests:** `crates/mqtt-bridge/src/daemon/qos_translation.rs::tests::{mqttbridge_6_reliability_gives_the_mqtt_qos, mqttbridge_6_durability_gives_retain_and_the_replay, mqttbridge_6_lifespan_is_the_message_expiry_interval, mqttbridge_6_a_lifespan_at_the_bounds_gives_no_wrong_interval, mqttbridge_6_deadline_liveliness_and_partition_reach_the_endpoints}`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_6_the_time_left_of_the_longest_lifespan_is_the_largest_interval`,
`crates/mqtt-bridge/src/daemon/replay.rs::tests::{mqttbridge_6_transient_keeps_the_newest_sample_of_each_instance, mqttbridge_6_persistent_samples_are_read_back_after_a_restart, mqttbridge_6_an_expired_sample_is_not_replayed, mqttbridge_6_a_cut_off_last_record_leaves_the_records_before_it, mqttbridge_6_a_torn_replay_file_is_written_again_before_the_next_record}`,
`crates/mqtt-bridge/src/daemon/status_watch.rs::tests::mqttbridge_6_the_deadline_report`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_6_the_connect_carries_the_liveliness_will`,
`crates/mqtt-bridge/src/daemon/history.rs::tests::{mqttbridge_6_keep_last_keeps_the_newest_n_of_each_instance, mqttbridge_6_keep_all_drops_nothing, mqttbridge_6_a_sample_put_back_is_the_oldest_or_dropped}`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_an_absent_mqtt_qos_is_none`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_6_an_explicit_mqtt_qos_0_stays_0_on_a_reliable_topic, mqttbridge_6_lifespan_is_the_message_expiry_interval, mqttbridge_6_a_sample_that_waited_goes_out_with_the_time_left_or_not_at_all, mqttbridge_6_the_samples_of_an_outage_arrive_once_and_in_order, mqttbridge_6_a_sample_of_another_partition_is_not_published, mqttbridge_6_a_missed_deadline_is_a_retained_publish, mqttbridge_6_a_lost_writer_liveliness_is_published, mqttbridge_6_the_broker_publishes_the_will_when_the_daemon_is_killed, mqttbridge_6_transient_samples_are_replayed_after_the_broker_restarts, mqttbridge_6_persistent_samples_are_replayed_after_a_daemon_restart, mqttbridge_6_a_replay_file_cut_off_in_its_last_record_does_not_prevent_the_start, mqttbridge_6_keep_last_on_an_out_topic_publishes_the_newest_n, mqttbridge_6_keep_all_on_an_out_topic_drops_nothing}`;
the QoS 1/2 flows with Mosquitto (MQTT 5.0 §4.3, Receive Maximum §4.9):
`crates/mqtt-bridge/tests/mosquitto_qos.rs::{mqttbridge_6_inbound_qos_1_and_2_are_acknowledged_and_all_messages_arrive, mqttbridge_6_outbound_qos_1_and_2_complete_and_leave_nothing_pending, mqttbridge_6_mosquitto_publishes_the_will_when_the_daemon_is_killed}`.

**Status:** done — the eleven rows are built, their tests were green on the
Linux test host, the Mosquitto tests against Mosquitto 2.0.21. A replay
file that ends inside its last record (the daemon ended while writing it)
prevented every later start with exit 1 before, until it was removed by
hand; now the daemon starts with the records before it, with the warning
`replay_file_truncated`, and writes the file again (test through the daemon
in the test process). A record the disk took only in part stayed before,
and the next records followed it; a record the disk takes only in part is
now cut off again. A real write error of a disk was not produced. A sample
that waits in the daemon (history, lost connection) goes out with what is
left of its lifespan (4 s lifespan, 2.2 s waited: interval 2), one whose
lifespan has passed does not, counted in `dds_samples_dropped_total`;
before, the interval was fixed at the arrival. The replay buffer takes a
sample once it went out: the samples that arrive during a broker outage
arrive after the reconnect each once and in their order (measured: 0 from
the buffer, then 1, 2, 3); before, the newest waiting one went out twice,
the first time before the older ones. The Message
Expiry Interval of an inbound PUBLISH does not reach DDS (DDS has no
lifespan per sample, spec §6 says so); the DataWriter's lifespan comes from
`qos.lifespan_ms`. Before, only reliability and the retain of
TRANSIENT_LOCAL were built: `qos_translation.rs` had no caller and
contradicted §6 (KEEP_ALL→QoS 2), there was no replay buffer, no key for
lifespan, deadline, liveliness and partition, no will. A lifespan within
the last half second below `u64::MAX` ns (`qos.lifespan_ms`
18446744073709551615, also the time left of a sample that just arrived)
made the rounding overflow: a panic in a debug build (measured), by the
wrapping semantics an interval of 1 s in a release build (derived, no
release build was run). Now the rounding has no overflow, and a
`qos.lifespan_ms` of 2^31 - 1 s or more is infinite, as the DataWriter's
LIFESPAN is: no Message Expiry Interval.

## §7 Security

### §7.1 TLS mqtts:// + ALPN

**Spec:** §7.1 — `mqtts://` mode via `mqtt.tls.enabled`; ALPN `["mqtt"]`;
SIGHUP cert rotation.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::{LiveTls, Session::connect, reload, reconnect}`
(the TLS settings in force — the client configuration from CA,
certificate and key, `server_name`, `alpn`; a SIGHUP replaces them after a
successful `crates/mqtt-bridge/src/daemon/security.rs::client_tls_from_config`, each connection that begins after it
uses them, the one that stands keeps its own; TLS material that does not
load refuses the reload), `crates/mqtt-bridge/src/daemon/client.rs::{open_stream, alpn_offer}`,
`crates/mqtt-bridge/src/daemon/config.rs` (`mqtt.tls.alpn`;
`mqtt.tls.verify_hostname: false` and `mqtt.tls.enabled: true` with
`mqtt://`/`ws://` prevent the start), `crates/mqtt-bridge/src/daemon/security.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::{mqttbridge_7_1_tls_connect_to_broker_succeeds_with_valid_cert, mqttbridge_7_1_a_sighup_puts_the_new_client_certificate_into_the_next_connection, mqttbridge_7_1_a_failed_reload_keeps_the_old_certificate, mqttbridge_7_1_a_sighup_puts_the_new_server_name_and_alpn_into_the_next_connection, mqttbridge_7_1_a_sighup_reads_the_ca_file_again, mqttbridge_3_the_alpn_list_of_the_configuration_is_offered}`
(a TLS broker in the test keeps the client certificate of each connection
and the server name and ALPN offer of each ClientHello, without session
resumption: after SIGHUP and the close of the first connection the second
comes with the new certificate, after a reload with a missing key with the
old one; after a reload with `server_name: broker.example` and
`alpn: [x-amzn-mqtt-ca, mqtt]` the next ClientHello carries both; when the
broker moves to a certificate the CA of the start does not trust, the
reconnects fail until the CA file names the new one and a SIGHUP reads it
again), `crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_new_keys_refuse_what_they_do_not_accept`
(`verify_hostname: false`, `tls.enabled` with `mqtt://`); ALPN see §7.1a.

**Status:** done — green on the Linux test host. That a reload also takes
`server_name` and `alpn` and reads `ca_file` again had no test before.
Before, `reload` built the
new TLS configuration and dropped it, and there was no new connection that
could have used it; the cited tests were those of
`bridge-security::RotatingTlsConfig`, a server-side acceptor this daemon
does not use. `mqtt.tls.verify_hostname: false` is refused instead of a
verifier without the name check (a decision of the previous work package,
spec §3); another name is reached with `mqtt.tls.server_name`.

### §7.1a ALPN negotiation

**Spec:** §7.1 — the TLS handshake offers ALPN and the negotiated protocol
name is checked.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (`ALPN_MQTT` for
`mqtts://`, `ALPN_HTTP_1_1` below `wss://`, `open_stream`).

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::mqttbridge_7_1_tls_connect_offers_the_mqtt_alpn_protocol`
(the broker reads back `mqtt` as the agreed protocol; a broker with only
another protocol refuses);
`crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate`
(`http/1.1` below `wss://` against Mosquitto).

**Status:** done

### §7.2 SASL/MQTT auth modes

**Spec:** §7.2 — none/password/mtls/enhanced (SCRAM/OAUTHBEARER/JWT).

**Repo:** `crates/mqtt-bridge/src/daemon/auth.rs::{AuthKind, Credentials, check, is_loopback_host, Exchange}`
(the mode from `auth.mode` or from what is configured, the credentials of
the CONNECT per mode, the check at load and at start, the exchange per
method), `crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::handshake, MqttClient::send_connect, MqttClient::wait_connack, MqttClient::reauthenticate, MqttClient::on_auth, ClientError::Auth}`
(Authentication Method and Data in the CONNECT, AUTH 0x18 up to the
CONNACK, [MQTT-4.12.0-5]/[MQTT-4.12.0-6], AUTH 0x19), `crates/mqtt-bridge/src/daemon/server.rs::{LiveAuth, same_credentials, same_auth_way, reload}`
(a SIGHUP with the same mode and method replaces the credentials for the
next connection; only changed credentials make a connection with enhanced
authentication re-authenticate; another mode keeps the old ones and is named
as needing a restart), `crates/mqtt-bridge/src/daemon/config.rs`
(`auth.mode` with `password` and the names `bearer`, `sasl`, `sasl_plain`,
`auth.enhanced.method`; `auth.sasl_users` removed),
`crates/bridge-security/src/client_auth.rs::{ScramClient, oauthbearer_initial_response, jwt_subject_unverified}`
(SCRAM-SHA-256 by RFC 5802/7677 on the crypto backend `ring`/`aws-lc-rs`,
SASLprep by RFC 4013, OAUTHBEARER by RFC 7628).

**Tests:** `crates/bridge-security/src/client_auth.rs::tests::{rfc7677_3_the_exchange_of_the_rfc, rfc5802_3_a_wrong_server_signature_and_an_error_fail, rfc5802_5_1_what_a_server_first_message_must_hold, rfc5802_5_1_the_user_name_is_prepared_and_escaped, rfc7628_3_1_the_initial_client_response}`;
`crates/mqtt-bridge/src/daemon/auth.rs::tests::{mqttbridge_7_2_none_is_refused_for_a_broker_that_is_not_loopback, mqttbridge_7_2_the_mode_follows_from_what_is_configured, mqttbridge_7_2_password_puts_user_name_and_password_into_the_connect, mqttbridge_7_2_mtls_sends_no_user_name_and_no_password, mqttbridge_7_2_enhanced_needs_the_input_of_its_method, mqttbridge_7_2_the_exchange_of_each_method}`;
`crates/mqtt-bridge/tests/enhanced_auth.rs::{mqttbridge_7_2_enhanced_connect_carries_method_and_data, mqttbridge_7_2_an_auth_continue_is_answered_before_connack, mqttbridge_7_2_a_server_signature_that_does_not_verify_ends_the_connection, mqttbridge_7_2_oauthbearer_answers_an_error_challenge_with_0x01, mqttbridge_7_2_an_auth_or_method_the_connect_did_not_ask_for_is_a_protocol_error, mqttbridge_7_2_a_scram_re_authentication_answers_the_continue, mqttbridge_7_2_a_reload_re_authenticates_with_the_new_token, mqttbridge_7_2_a_reload_that_keeps_the_credentials_does_not_re_authenticate}`
(a test broker with the server side of SCRAM-SHA-256 by RFC 5802 §3 and of
OAUTHBEARER);
the reload: `crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_7_2_a_reload_gives_the_next_connect_the_new_password`
(under `password` the CONNECT after the end of the standing connection
carries the new password), `crates/mqtt-bridge/src/daemon/server.rs::tests::{mqttbridge_7_2_a_reload_takes_new_credentials_of_the_same_way_only, mqttbridge_9_2_a_reload_names_the_settings_that_need_a_restart}`
(a reload of only the ACL or of `auth.bearer_subject` starts no new
generation, new credentials do, the subject follows the new user name;
another `auth.mode` or `auth.enhanced.method` keeps credentials and subject
and is in the restart list);
`crates/mqtt-bridge/tests/security_e2e.rs::{mqttbridge_7_2_auth_bearer_emits_password_in_connect, mqttbridge_7_2_auth_sasl_plain_emits_user_and_password, mqttbridge_7_2_the_bearer_token_is_the_password_without_a_user_name, mqttbridge_7_2_auth_username_and_password_win_over_the_mqtt_keys, mqttbridge_7_2_mtls_sends_no_user_name_and_no_password}`;
`crates/mqtt-bridge/tests/contract_bridge_security.rs::{mqttbridge_7_2_the_scram_client_runs_the_exchange_of_rfc_7677, mqttbridge_7_2_the_oauthbearer_response_is_the_one_of_rfc_7628}`;
against Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_auth.rs::{mqttbridge_7_2_a_wrong_password_is_exit_5_against_mosquitto, mqttbridge_7_2_mtls_and_the_dn_subject_against_mosquitto}`
(`#[ignore]`, job `integration-bridges-vendor`);
against EMQX 5.8.6 `crates/mqtt-bridge/tests/emqx_auth.rs::{mqttbridge_7_2_scram_sha_256_and_a_jwt_connect_to_emqx, mqttbridge_7_2_emqx_refuses_a_wrong_scram_password_and_a_forged_jwt}`
(EMQX's SCRAM authenticator with its built-in database, SHA-256, 4096
iterations, and its JWT authenticator with HMAC SHA-256 and the token from
the CONNECT password, set up through EMQX's REST API; a daemon under
`enhanced` with SCRAM-SHA-256 and one under `password` with the JWT
connect, a sample of the first reaches DDS through the second; EMQX refuses
a wrong SCRAM proof and a JWT of another key; `#[ignore]`, job
`integration-bridges-vendor` with EMQX as the service `emqx`).

**Status:** done — green on the Linux test host, the Mosquitto tests there
with Mosquitto 2.0.21, the EMQX tests against EMQX 5.8.6 in a container.
Against EMQX, AUTH, CONNACK, the properties 0x15/0x16 and the SCRAM messages
go through EMQX's code on the broker's side, not through the crate's codec;
a symmetric error in the codec (the identifier of Authentication Data moved
alike in encoder and decoder) passes the tests against the test broker and
fails against EMQX (mutation, measured). Without a foreign broker remain:
OAUTHBEARER and another method with the token as Authentication Data (EMQX
5.8.6 CE has no authenticator for them, its API answers
`unsupported_mechanism`; Mosquitto has no built-in enhanced authentication)
and the re-authentication: EMQX 5.8.6 takes AUTH 0x19, answers it with AUTH
0x18 and the server-first-message, checks the client-final-message with
success, but then sends no AUTH 0x00 and closes the connection five seconds
later (`session_stepdown_request_timeout` in
`emqx_channel:post_process_connect` in EMQX's log); after that it refused
new connections of the same client ID with CONNACK 0x85 for a while
(measured). These parts are checked against the test broker only. Before, `auth.mode` took only `none`, `bearer`,
`sasl`, `sasl_plain` and `mtls`: `none` was allowed towards any broker,
`mtls` still sent `mqtt.username` and `mqtt.password`, `enhanced` did not
exist (no Authentication Method/Data, no AUTH in the daemon, an AUTH of the
broker was dropped without a word), and the cited tests of
`authenticate_mqtt` checked a server function the daemon never called; it is
removed together with `auth.sasl_users`. A reload with the same mode also
sent AUTH 0x19 when only the ACL had changed.

### §7.3 ACL daemon-side

**Spec:** §7.3 — `acl.default_deny` + `rules` with
`subject`/`allow_publish`/`allow_subscribe`.

**Repo:** `crates/mqtt-bridge/src/daemon/acl.rs::{DaemonAcl, AclRule, Subject, LiveAcl, filter_covers, filters_overlap, subject_of, certificate_subject}`
(rules per subject with MQTT topic filters; `may_publish` on the MQTT topic
of an outbound PUBLISH, `may_subscribe` on the filter of a SUBSCRIBE,
`may_receive` on the topic of a PUBLISH of the broker; the subject from the
DN of the client certificate, the user name the daemon authenticates with
(`crates/mqtt-bridge/src/daemon/auth.rs::Credentials::user_name`: the CONNECT's under `password`, the
SCRAM user under `enhanced`), the `sub` of the JWT, replaced by
`auth.bearer_subject`), `crates/mqtt-bridge/src/daemon/server.rs::{Session::after_connack, Session::replay_to, reload}`, `crates/mqtt-bridge/src/daemon/pump.rs::Pump::may_publish`
(a refusal as the log line `acl_denied` and in
`zerodds_mqtt_bridge_acl_denials_total{reason}`; a reload replaces rules and
subject), `crates/mqtt-bridge/src/daemon/config.rs::read_acl` (filters
checked).

**Tests:** `crates/mqtt-bridge/src/daemon/acl.rs::tests::{mqttbridge_7_3_a_rule_with_a_filter_covers_its_topics, mqttbridge_7_3_the_star_subject_is_everyone, mqttbridge_7_3_default_deny_closes_the_rest, mqttbridge_7_3_publish_and_subscribe_are_separate_rights, mqttbridge_7_3_a_filter_covers_and_overlaps_as_mqtt_matches, mqttbridge_7_3_the_subject_is_the_certificate_dn_then_the_user_name_then_the_jwt_sub}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_7_3_the_rules_of_the_spec_let_alice_publish_chat_and_nothing_else`
(the rules of the example of §7.3, user `alice`: `chat/general` and
`sensors/s1/public/temp` arrive at the broker, `news/today` does not; the
SUBSCRIBE to `chat/+` does not go out; a PUBLISH on a public sensor topic
reaches DDS);
`crates/mqtt-bridge/tests/enhanced_auth.rs::mqttbridge_7_3_a_rule_for_the_scram_user_grants_its_topic`
(under SCRAM-SHA-256 the SCRAM user is the subject: its rule grants
`scram/a`, `other/b` is refused under `default_deny: true` and counted);
`crates/mqtt-bridge/tests/daemon_e2e.rs::mqttbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_samples`;
`crates/mqtt-bridge/tests/example_yaml_loadback.rs::mqttbridge_7_3_example_yaml_acl_reaches_the_daemon`;
`crates/mqtt-bridge/tests/contract_bridge_security.rs::{mqttbridge_7_3_the_subject_of_a_certificate_is_its_rfc_4514_dn_and_its_fingerprint, mqttbridge_7_3_the_sub_of_a_jwt_is_its_subject}`;
against Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_auth.rs::mqttbridge_7_2_mtls_and_the_dn_subject_against_mosquitto`
(subject `CN=bridge-01` from the client certificate; `#[ignore]`, job
`integration-bridges-vendor`).

**Status:** done — green on the Linux test host. Before, the daemon
inverted the rules into a table keyed by the filter text and asked it with
the DDS topic name (an exact comparison): a rule `chat/+` never applied to
`Chat::Message`. The outbound PUBLISH was checked against
`allow_subscribe`, the subject was `auth.bearer_subject` or the constant
`zerodds-mqtt-bridge`, a refusal only counted `errors_total`. The cited tests
passed because their keys were DDS names. Also, under `enhanced` with
SCRAM-SHA-256 the daemon had no subject (only the rules of `*` applied):
`subject_of` took the user name of the CONNECT, which SCRAM does not send.

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log on stdout, one object per line, fields
timestamp/level/event/mqtt_topic/dds_topic/bytes/mqtt_qos/latency_us/connect_state;
`log_level` (`--log-level`) as the threshold, a SIGHUP reload applies a
changed value; the table of the events with level and fields.

**Repo:** `crates/mqtt-bridge/src/daemon/log.rs` (`Level`, `set_level`,
`Event::field/number/integer/flag/tenant/reason_code/latency/render/emit`,
`rfc3339`), `crates/mqtt-bridge/src/daemon/server.rs` (the events of the
start, the connection, its loss and the reconnect with `connect_state`, the
subscriptions, the catalog, the replay, the ACL, the reload and the
shutdown; `publish_in` with `latency_us` from reading the PUBLISH to the
return of its DDS write), `crates/mqtt-bridge/src/daemon/pump.rs::Pump`
(`publish_out` with `latency_us` from taking the sample from the DataReader
to the end of the socket write; `samples_waiting`, `sample_expired`,
`publish_failed`), `crates/mqtt-bridge/src/daemon/client.rs` and
`crates/mqtt-bridge/src/daemon/session.rs` (the session, the
re-authentication, the broker's refusals with `client_id`; on a tenant's
connection with `tenant` from `DaemonConfig::tenant`, which
`crates/mqtt-bridge/src/daemon/config.rs::DaemonConfig::of_tenant` sets;
before, these lines had no `tenant`),
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs` (`running`,
`shutdown`, `stopped`, `start_failed` on stdout and as text on stderr; as
the Windows service into the log file of §11); the
table of the events in spec §8.1, `man/man1/zerodds-mqtt-bridged.1` and
`man/de/man1/zerodds-mqtt-bridged.1`. Before, the daemon wrote free text
with `eprintln!` on stderr, and `log_level` was read and used by no code
path.

**Tests:** `crates/mqtt-bridge/src/daemon/log.rs::tests::{mqttbridge_8_1_a_line_is_one_json_object_with_timestamp_level_and_event, mqttbridge_8_1_text_is_escaped, mqttbridge_8_1_the_timestamp_is_rfc_3339_utc, mqttbridge_8_1_an_unset_field_is_left_out, mqttbridge_8_1_a_line_below_log_level_is_not_written}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_1_the_binary_logs_a_publish_with_the_fields_of_the_spec`
(the binary with `log_level: debug` against the crate's broker server and a
DDS participant of the test: every line is JSON with an RFC 3339
timestamp, `level` and `event`; `connect` with `connect_state` connected,
`publish_out` and `publish_in` with `mqtt_topic`, `dds_topic`, `bytes`,
`mqtt_qos`, `latency_us`; after SIGTERM `disconnect` with `connect_state`
disconnected and `reason_code` 0x00, then `stopped`; stderr stays empty);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_1_log_level_warn_writes_no_info_line_and_a_reload_applies_debug`
(under `warn` the connection, the subscription and a message write no line,
the refused subscription its `warn` line; after the file says `debug` and
a SIGHUP, `reload` and `publish_in` follow);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_4_3_a_refused_subscription_is_logged`
(`subscription_refused` with the filter and `reason_code` 0x87,
`subscribed` with 1 of 2); `crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_1_the_client_lines_of_a_tenant_carry_its_name`
(the binary with a tenant against a broker of the test that resumes a
session without state and refuses each catalog PUBLISH with PUBACK 0x87:
the tenant's `session_unknown` and `publish_refused_by_broker` carry
`tenant` a, the top level's no `tenant`; without `.tenant()` on
`publish_refused_by_broker` the test fails, measured with a mutation);
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_1_l6_the_tenants`
(`of_tenant` sets `tenant`); `tests/integration/tests/bridges.rs::mqtt_bridge_against_mosquitto`
waits for the line `"event":"subscribed"`.

**Status:** done — the tests ran green on the Linux test host. The log
file of the Windows service is under §11 (not run on Windows).
### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + `metrics.*` config; the families
`zerodds_mqtt_bridge_*` with their labels.

**Repo:** `crates/mqtt-bridge/src/daemon/metrics.rs::{BridgeMetrics, family, FAMILIES, acl_reason, reason_code_label}`
(the 13 families of the list of §8.2 with `dds_topic`, `mqtt_qos`,
`direction`, `reason`, `reason_code`; the series of a topic from the start
with 0; `<tenant>/<topic>` for the topics of a tenant),
`crates/mqtt-bridge/src/daemon/server.rs` (counted at the connect, at each
PUBLISH in both directions, at the samples of DataReader and DataWriter, at
the refusals of the ACL, at the end of a connection with the Reason Code of
the DISCONNECT or `network`; `inflight_messages` from the open QoS 1/2 flows
of each connection),
`crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::pending_releases, MqttClient::ended_by}`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::serve_admin_endpoints`
(`metrics.path`).

**Tests:** `crates/mqtt-bridge/src/daemon/metrics.rs::tests::{mqttbridge_8_2_every_family_of_the_spec_is_rendered_with_its_labels, mqttbridge_8_2_register_is_idempotent, mqttbridge_8_2_a_topic_is_counted_under_its_name_and_its_tenant, mqttbridge_8_2_a_disconnect_counts_its_reason_code, mqttbridge_8_2_inflight_follows_the_counts_of_each_connection, mqttbridge_8_2_an_acl_denial_counts_its_reason}`
(the list of the families is read from the code block of §8.2 in
`docs/specs/zerodds-mqtt-bridge-1.0.md`);
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::{mqttbridge_3_the_metrics_are_served_on_metrics_path, mqttbridge_5_2_admin_endpoint_serves_paths}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_2_every_flag_reaches_the_connect_and_the_subscribe, mqttbridge_7_3_the_rules_of_the_spec_let_alice_publish_chat_and_nothing_else, mqttbridge_6_keep_last_on_an_out_topic_publishes_the_newest_n}`
(the binary with `--metrics` serves
`zerodds_mqtt_bridge_dds_samples_published_total{dds_topic="Flag::T"} 1`
over HTTP; the refusals of the ACL; the dropped samples);
the counting in the daemon: `crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_8_2_a_refused_and_an_accepted_connect_are_two_attempts_and_one_success, mqttbridge_8_2_each_publish_counts_by_topic_qos_and_octets, mqttbridge_8_2_inflight_counts_the_flows_that_have_not_ended, mqttbridge_8_2_a_lost_connection_counts_under_its_reason_code}`
(a broker that refuses the first CONNECT with 0x88: 2 attempts, 1 success;
one PUBLISH each way under topic, QoS and payload length; `out` 1 while the
broker holds the PUBACK back, then 0; `in` 1 after a QoS 2 PUBLISH of the
broker until the PUBREL; DISCONNECT 0x8E as `0x8e`, a closed socket as
`network`).

**Status:** done — green on the Linux test host. Before, none of the
families of §8.2 existed under its name: the daemon kept
`zerodds_mqtt_frames_in_total` and nine more counters with the prefix
`zerodds_mqtt_` without labels; `connect_successes`, `inflight_messages`,
`acl_denials` and `broker_disconnects` were missing. The spec now also names
`dds_samples_dropped_total{dds_topic}` and `errors_total`, which §3, §4.1 and
§6 count in. Also, only unit tests of the struct `BridgeMetrics` checked the
families `connect_attempts`, `connect_successes`, `publish_in`,
`publish_out`, `bytes_in`, `bytes_out`, `inflight_messages` and
`broker_disconnects`; now removing or falsifying each of these counting
points in `crates/mqtt-bridge/src/daemon/server.rs` fails one of the four daemon tests (8 mutations on the
Linux test host, all caught).

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables span emission;
without the variable no span is made.

**Repo:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::{otlp_config_from_env, otlp_config_from_endpoint, spawn_otlp_flush_loop}`,
`crates/mqtt-bridge/src/daemon/server.rs::start_with_reload` (the exporter
is made before the cores and only with the variable, the log line
`otlp_active`; before, it was made after the cores, and no code path called
`add_span`: the flush thread posted empty buffers),
`crates/mqtt-bridge/src/daemon/trace.rs::Tracer` (off without an
exporter), `crates/mqtt-bridge/src/daemon/server.rs::Session::replay_to`
(a PUBLISH of the replay buffer is a span of its DDS topic from the
replay; before, it had none); spec §8.3 with the table of the spans and the
PUBLISH packets without a span (the catalog, the reports, the resends of a
resumed session), `man/man1/zerodds-mqtt-bridged.1`,
`man/de/man1/zerodds-mqtt-bridged.1`.

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::mqttbridge_8_3_otlp_config_parses_endpoint`;
`crates/mqtt-bridge/src/daemon/trace.rs::tests::mqttbridge_8_3_a_tracer_without_exporter_is_off`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_3_the_binary_sends_a_span_per_publish_roundtrip_only_with_the_variable`
(the binary without the variable: no line `otlp_active`, the test's
collector gets nothing; with it: `otlp_active` and the spans of §8.3a);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_3_a_replayed_publish_is_a_span_of_its_topic`
(the broker restarts without its retained messages; the collector gets a
second `mqtt.publish` of kind client of the transient topic with the DDS
topic, a packet identifier and status ok, begun after the new broker came
up; without the call in `replay_to` the test fails, measured with a
mutation).

**Status:** done — the tests ran green on the Linux test host.
## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config parse → signal handlers → TLS → DCPS →
reader/writer → MQTT connect with reconnect backoff → SUBSCRIBE.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::{start_with_reload, start_core, Session::connect, Session::after_connack}`
(the signal watcher before the connect, TLS and the auth check, then per
core participant, endpoints, connection, SUBSCRIBE),
`crates/mqtt-bridge/src/daemon/client.rs::connect_with_backoff` (the waits
of `mqtt.reconnect` up to `mqtt.connect_timeout_secs`),
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`.

**Tests:** `crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_9_1_the_order_is_participant_endpoints_connect_subscribe, mqttbridge_9_1_the_daemon_connects_once_the_broker_comes_up, mqttbridge_9_1_sigterm_during_the_connect_retries_exits_0}`
(while the broker holds back the CONNACK, a participant of the test has
discovered the bridge's writer and reader, nothing goes out before the
CONNACK, the first packet after it is the SUBSCRIBE; a broker that starts
one second after the binary is reached; SIGTERM during the retries is exit
0);
`crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_9_1_an_unreachable_broker_is_retried_until_the_connect_timeout, mqttbridge_9_1_a_broker_that_comes_up_before_the_connect_timeout_is_reached, mqttbridge_9_1_a_tls_handshake_without_an_answer_is_tried_again, mqttbridge_9_1_a_connect_timeout_beyond_the_clock_retries_without_end}`;
`crates/mqtt-bridge/tests/daemon_e2e.rs::mqttbridge_2_daemon_connects_and_subscribes`.

**Status:** done — green on the Linux test host. The connect with backoff
and the signal watcher before it came from an earlier work package (spec
§9.1); the cited test checked only CONNECT and SUBSCRIBE, neither the order
nor the retry. A `mqtt.connect_timeout_secs` that, added to the current
time, goes beyond the clock (the configuration takes every u64 value, such
as 18446744073709551615) retries without end, as 0 does; before, the start
ended with a panic in `connect_with_backoff` (measured).

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, DISCONNECT 0x00, cleanup; SIGHUP
TLS+ACL reload.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::DaemonHandle::shutdown`
(the pumps stop, the broker's acks are awaited up to
`mqtt.drain_timeout_secs`, a value beyond the clock without end,
DISCONNECT 0x00),
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs` (waits for the stop
flag; the process was still running 3 s after SIGTERM before, measured),
`::start_with_reload`, `::reload` (ACL; withdrawn rights apply at once;
the TLS settings from the next connection, §7.1; the credentials of §7.2
with the same mode and method, with a re-authentication only for changed
credentials; another `auth.mode` or `auth.enhanced.method` is in the restart
list).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_9_2_sigterm_disconnects_and_exits_0`,
`::mqttbridge_9_2_the_shutdown_waits_for_the_acks_then_disconnects` (Mosquitto 2.0.21,
the Linux test host); `crates/mqtt-bridge/tests/daemon_e2e.rs::mqttbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_samples`;
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_9_2_backoff_connect_aborts_when_stop_set`;
the TLS reload: `crates/mqtt-bridge/tests/security_e2e.rs::mqttbridge_7_1_a_sighup_puts_the_new_client_certificate_into_the_next_connection`;
the reload of the credentials: `crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_7_2_a_reload_gives_the_next_connect_the_new_password`,
`crates/mqtt-bridge/tests/enhanced_auth.rs::mqttbridge_7_2_a_reload_that_keeps_the_credentials_does_not_re_authenticate`,
`crates/mqtt-bridge/src/daemon/server.rs::tests::{mqttbridge_7_2_a_reload_takes_new_credentials_of_the_same_way_only, mqttbridge_9_2_a_reload_names_the_settings_that_need_a_restart}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_2_a_drain_timeout_beyond_the_clock_waits_for_the_acks`
(`mqtt.drain_timeout_secs` 18446744073709551615: the shutdown ends after
the PUBACK the test's broker holds back for 1.5 s).

**Status:** done — green on the Linux test host. A
`mqtt.drain_timeout_secs` beyond the clock made `DaemonHandle::shutdown`
end with a panic before (measured with a mutation); now it waits for the
acks without end.

### §9.3 Reconnect

**Spec:** §9.3 — after a lost connection (a DISCONNECT of the broker, a
closed or failed socket, a missing PINGRESP) the daemon connects again
without an end: the first attempt at once, each next one after the waits
of `mqtt.reconnect` (`initial_delay_ms`, then `factor` times the last one,
at most `max_delay_ms`), with the TLS settings and credentials in force; a
refusal of the TLS handshake or of the credentials is tried again after
the next wait; after DISCONNECT 0x8E Session taken over only after
`max_delay_ms`. Meanwhile the samples wait by the history of their topic,
`/healthz` answers 503. After the CONNACK: SUBSCRIBE, catalog, replay
buffer; with `clean_start=false` the session (subscriptions, inflight
queue).

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::{reconnect, Session::attempt, Session::after_connack, Health}`
(one sequence of waits for every failure: a broker that cannot be reached,
a refusal of TLS or of the credentials, a failed subscription; before, a
refusal started a second backoff in `connect_with_backoff` that began at
`initial_delay_ms` at each attempt; 0x8E waits `max_delay_ms`),
`crates/mqtt-bridge/src/daemon/pump.rs::Pump` (`connected` false: the
samples wait in the history),
`crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::keep_alive_tick, MqttClient::release, MqttClient::ended_by, MqttClient::wait_for_a_send_slot}`
(the end of the connection ends the wait for a send slot with
`ClientError::Io`),
`crates/mqtt-bridge/src/daemon/session.rs` (the session with
`clean_start=false`, also across a restart in `mqtt.session_dir`); spec
§9.3, `man/man1/zerodds-mqtt-bridged.1`,
`man/de/man1/zerodds-mqtt-bridged.1`. The section had no item in this
document before.

**Tests:** `crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_after_a_broker_restart_the_daemon_subscribes_and_publishes_its_catalog_again`
(the broker goes away and comes back on its port without subscriptions and
retained messages: `/healthz` 503, then 200; a message on the topic reaches
DDS again, the new broker holds the catalog);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_the_waits_between_the_attempts_grow_to_the_maximum`
(the binary: the first line `connect_failed` less than 200 ms after the
line `disconnect` with `initial_delay_ms` 400, so no wait before the first
attempt; `connect_state` reconnecting and `next_attempt_ms` 400, 800,
1600, 1600, 1600 for 400/2/1600, the attempts at least that far apart; the
broker that is back is reached; a first attempt that waits
`initial_delay_ms` fails the test, measured with a mutation);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_a_refused_reconnect_is_tried_again_after_the_next_wait`
(two CONNACK 0x86, which would be exit 5 at the start, tried again 200 and
400 ms apart, the third CONNECT is let in);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_a_missing_pingresp_ends_the_connection_and_the_daemon_connects_again`
(Keep Alive 1 s, no PINGRESP: a new connection after at least 1.4 s, counted
under `network`; the second connection gets its PINGRESP and stands);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_a_session_taken_over_is_tried_again_after_the_longest_wait`
(after 0x8E at least 1.4 s with `max_delay_ms` 1500, after 0x89 below 1 s);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_after_a_lost_connection_the_session_resends_what_the_broker_did_not_acknowledge`
(the daemon in the test process with `clean_start=false` and a Session
Expiry Interval of 300 s; the broker of the test holds the PUBACK of a QoS
1 PUBLISH of the daemon and closes the socket without DISCONNECT; the
daemon connects again without a restart with Clean Start 0, and after
Session Present 1 the PUBLISH goes out again with DUP 1, the same packet
identifier, topic and payload; the PUBACK of it completes the flow in the
session file);
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_6_transient_samples_are_replayed_after_the_broker_restarts, mqttbridge_6_the_samples_of_an_outage_arrive_once_and_in_order, mqttbridge_7_2_a_reload_gives_the_next_connect_the_new_password, mqttbridge_5_2_a_new_assigned_identifier_clears_the_catalog_of_the_one_before, mqttbridge_8_2_a_lost_connection_counts_under_its_reason_code, mqttbridge_4_1_a_restarted_daemon_resends_what_the_broker_did_not_acknowledge}`;
`crates/mqtt-bridge/tests/security_e2e.rs::{mqttbridge_7_1_a_sighup_puts_the_new_client_certificate_into_the_next_connection, mqttbridge_7_1_a_sighup_reads_the_ca_file_again}`
(refused TLS handshakes are tried again until the new CA applies);
`crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_4_1_the_session_survives_a_reconnect_with_clean_start_false`
(Mosquitto 2.0.21, `#[ignore]`, job `integration-bridges-vendor`);
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_reconnect_delays_grow_by_the_factor_up_to_the_maximum`;
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_9_3_the_end_of_the_connection_ends_the_wait_for_a_send_slot`
(Receive Maximum 1, the broker closes while a QoS 1 PUBLISH waits for a
send slot: two PUBLISH end within 5 s with an I/O error, one event of the
end stays for the inbound loop);
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_9_3_a_malformed_ack_ends_the_wait_for_a_send_slot_and_the_connection`
(a PUBACK that does not decode while a QoS 1 PUBLISH waits for a send
slot: DISCONNECT 0x81 and the close, the waiting PUBLISH ends with an I/O
error and does not go out, one event of the end stays for the inbound
loop).

**Status:** done — the tests ran green on the Linux test host. A QoS 1/2
PUBLISH that waited for a send slot did not notice the end of the
connection before: it read the closed socket for 10 s, held one
`Disconnected` event per pass (8066134 in a measurement of the review) and
ended with `ClientError::Busy`, which the pump repeated without a pause.
Now it ends at once with `ClientError::Io`; the pump puts the sample back,
and the inbound loop connects again. A packet of the broker that did not
decode during the wait ended as `ClientError::Codec` before: the pump
dropped the sample, and the connection stayed open. Now it ends the
connection with DISCONNECT 0x81 as in the inbound loop; any other error
during the wait closes the connection as the inbound loop does (this branch
has no test of its own: an error of the session file or of a write during
the wait could not be produced in the test).

### §8.3a PUBLISH roundtrip span

**Spec:** §8.3 — with `OTEL_EXPORTER_OTLP_ENDPOINT` set, each PUBLISH
roundtrip is a trace with the root `mqtt.publish`; `dds.write` is a child
of a PUBLISH of the broker. The earlier text of the spec named
"`mqtt.publish` + `dds.write` as child spans" for every roundtrip; a
PUBLISH of the daemon has no `dds.write`, because on the way to MQTT the
daemon writes nothing into DDS (a DataWriter of another participant wrote
the sample). Spec §8.3 says so now.

**Repo:** `crates/mqtt-bridge/src/daemon/trace.rs` (`Tracer`,
`PublishOut`, `PublishIn`, `DdsWrite`, `publish_out_span`,
`publish_in_spans`), `crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::trace_publish, MqttClient::set_tracer}`
(the span of a PUBLISH of the daemon ends with the socket write under QoS
0, with the PUBACK or PUBCOMP under QoS 1 and 2; a refusal of the broker
and a connection that ended before the acknowledgement are status error),
`crates/mqtt-bridge/src/daemon/pump.rs::Pump` (the start is taking the
sample from the DataReader),
`crates/mqtt-bridge/src/daemon/server.rs::Session::replay_to` (a PUBLISH
of the replay buffer after a CONNACK: the start is the replay),
`crates/mqtt-bridge/src/daemon/server.rs::start_core`
(a PUBLISH of the broker: `mqtt.publish` from reading it to the end of its
handling, one `dds.write` per DDS topic; without a route or refused by the
ACL the root alone with status error).

**Tests:** `crates/mqtt-bridge/src/daemon/trace.rs::tests::{mqttbridge_8_3_a_publish_of_the_daemon_is_a_root_span_to_its_acknowledgement, mqttbridge_8_3_a_refused_publish_is_status_error, mqttbridge_8_3_a_publish_of_the_broker_has_a_dds_write_child_per_topic, mqttbridge_8_3_a_publish_written_nowhere_is_the_root_alone}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_3_the_binary_sends_a_span_per_publish_roundtrip_only_with_the_variable`
(the binary against a broker of the test that holds the PUBACK 1.5 s and an
OTLP collector of the test: the daemon's QoS 1 PUBLISH is an
`mqtt.publish` of kind client with the topic, the QoS, the packet
identifier and the DDS topic that lasts at least 1.4 s; the broker's
PUBLISH is an `mqtt.publish` of kind server with the child `dds.write` in
its trace; a span that ends with the socket write instead of the PUBACK
fails the test, measured with a mutation);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_3_a_replayed_publish_is_a_span_of_its_topic`.

**Status:** done — the tests ran green on the Linux test host.

## §10 Cross-vendor

### §10 RTPS peer + Mosquitto/EMQX/HiveMQ

**Spec:** §10 — the daemon is a normal RTPS peer; the MQTT side tested
against Mosquitto/EMQX/HiveMQ.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`,
`crates/mqtt-bridge/src/daemon/server.rs::write_inbound` (the
representation of a topic's DataWriter),
`crates/mqtt-bridge/src/daemon/qos_translation.rs::{reader_config, writer_config}`
(`data_representation`), `crates/mqtt-bridge/tests/interop/{cyclone_reading.c, fastdds_reading.cpp, reading.idl}`.

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_12_3_mosquitto_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_10_fastdds_carries_mqtt_to_dds_and_dds_to_mqtt, mqttbridge_10_connext_carries_mqtt_to_dds_and_dds_to_mqtt}`
(the daemon between the clients of Mosquitto and a Cyclone DDS 0.10.5
participant under XCDR1 and XCDR2, a Fast DDS 3.1 participant and RTI
Connext DDS 7.7.0 `shape_main` under XCDR2, each over RTPS both ways: once
the daemon's writer has found the foreign reader and that reader the writer
(the participant reports its match), one PUBLISH of `mosquitto_pub` reaches
the foreign reader as the sample; a sample of the foreign writer reaches `mosquitto_sub` as the encapsulation
header and the CDR bytes, and those octets, published again, reach the
foreign reader as the sample written; all `#[ignore]`, job
`integration-bridges-vendor`);
`crates/mqtt-bridge/tests/daemon_e2e.rs::{mqttbridge_3_data_representation_xcdr2_matches_an_xcdr2_reader_before_the_first_publish, mqttbridge_3_data_representation_xcdr2_reader_matches_no_xcdr1_writer}`;
the MQTT side against EMQX, HiveMQ and Aedes in §10a; further Mosquitto
tests `crates/mqtt-bridge/tests/mosquitto_qos.rs::{mqttbridge_6_inbound_qos_1_and_2_are_acknowledged_and_all_messages_arrive, mqttbridge_6_outbound_qos_1_and_2_complete_and_leave_nothing_pending}`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::{mqtt5_4_zerodds_311_client_receives_from_mosquitto, mqtt5_4_mosquitto_311_client_receives_from_zerodds}`;
the ZeroDDS participant over RTPS with the binary and Mosquitto
`tests/integration/tests/bridges.rs::mqtt_bridge_against_mosquitto`.

**Status:** done — the tests ran green on the Linux test host. Before, the
item cited for the RTPS side a test that injects the sample into the
daemon's reader and sends nothing over RTPS. The vendor tests found that
the XCDR2 readers of Cyclone DDS, Fast DDS and RTI Connext did not match
the daemon's DataWriter, which offered XCDR1 until the first PUBLISH
(measured); the topic key `data_representation` (§3) fixes the
representation from the creation on. Before, the test repeated the PUBLISH
every second for up to 15 s until a sample arrived, so a dropped PUBLISH
went unnoticed. Now one PUBLISH goes out once both sides have matched, and
it must arrive: a mutation that drops the daemon's first DDS write fails the
three tests (measured).

### §10a Broker matrix EMQX/HiveMQ

**Spec:** §10 — the MQTT side runs against EMQX and HiveMQ.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (`mqtt.own_topic_prefix`),
`crates/mqtt-bridge/src/daemon/runtime_common.rs::catalog_topic`,
`crates/mqtt-bridge/src/daemon/client.rs::liveliness_topic`,
`crates/mqtt-bridge/src/daemon/status_watch.rs::StatusWatch`,
`ci/integration-weekly.yml` (the services `emqx-matrix`, `hivemq`, `aedes`
of the job `integration-bridges-vendor`).

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_10_emqx_carries_qos_0_1_2_retained_messages_and_the_properties, mqttbridge_10_hivemq_carries_qos_0_1_2_retained_messages_and_the_properties, mqttbridge_10_aedes_carries_qos_0_1_2_and_retained_messages, mqttbridge_10_mosquitto_carries_qos_0_1_2_retained_messages_and_the_properties}`
(the samples of a ZeroDDS participant reach `mosquitto_sub` with the QoS of
their topic, 0, 1 and 2, under MQTT 5 with Content Type, Payload Format
Indicator 0, the user properties of §4.2 and the Message Expiry Interval of
the writer's lifespan: 28 to 30 for a 30 s lifespan, as the time left the
broker passes on, none for an infinite lifespan; the messages of `mosquitto_pub`
at QoS 0, 1 and 2 reach DDS once each, under MQTT 5 with the key hash and
the source timestamp of their user properties; a retained message of the
broker reaches DDS at the subscription; the sample of a TRANSIENT_LOCAL
topic stays retained on the broker for a later subscriber),
`crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_9_3_emqx_keeps_the_session_and_publishes_the_will, mqttbridge_9_3_hivemq_keeps_the_session_and_publishes_the_will, mqttbridge_9_3_aedes_keeps_the_session_and_publishes_the_will, mqttbridge_9_3_mosquitto_keeps_the_session_and_publishes_the_will}`
(the daemon's connection with `clean_start: false` ends without DISCONNECT
through a forwarder of the test; the broker publishes the will and queues
the QoS 1 messages of the outage, which reach DDS once each and in order
after the reconnect),
`crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_5_2_emqx_keeps_the_retained_catalog, mqttbridge_5_2_aedes_keeps_the_retained_catalog, mqttbridge_5_2_mosquitto_keeps_the_retained_catalog, mqttbridge_5_2_hivemq_refuses_the_dollar_prefix_and_keeps_the_catalog_under_another}`,
`crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_12_3_emqx_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_hivemq_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_aedes_carries_mqtt_to_cyclone_and_cyclone_to_mqtt}`
(all `#[ignore]`, job `integration-bridges-vendor`; EMQX 5.8.6, HiveMQ
4.57.0 and Aedes 1.0.2 as containers);
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_6_the_connect_carries_the_liveliness_will`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::mqttbridge_5_2_the_catalog_is_the_json_of_the_spec`.

**Status:** done — the tests ran green on the Linux test host, the brokers
as containers. Measured: HiveMQ 4.57.0 answers a CONNECT whose will topic
begins with `$` with CONNACK 0x82 and a PUBLISH on such a topic with
DISCONNECT 0x82 (MQTT 5.0 §4.7.2 leaves those topics to the server), so the
daemon could not connect to HiveMQ; `mqtt.own_topic_prefix` (§3) moves the
catalog, the reports and the will. Aedes 1.0.2 refuses a CONNECT of
protocol level 5; the daemon runs against it under
`mqtt.protocol_version: 3.1.1` (§1 L1), without the properties of §4.2.

---

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
`man/man1/zerodds-mqtt-bridged.1`, `man/man5/zerodds-mqtt-bridged.yaml.5`;
the Windows service: `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`
(`main` starts the daemon through
`zerodds_dcps::cli::run_as_service("ZeroDDSMQTTBridge", …)`),
`crates/dcps/src/cli.rs` (`run_as_service`, `request_service_stop`,
`request_service_reload`, `READY=1` and `STOPPING=1` of
`notify_service_manager` as service states),
`crates/dcps/src/cli/windows_service.rs`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs`
(`install_signal_watcher` off Unix: `watch_service_requests` sets the flags
of SIGTERM and SIGHUP); the log of the service:
`crates/dcps/src/cli.rs::running_as_service` (set by
`crates/dcps/src/cli/windows_service.rs::service_main` before the daemon),
`crates/mqtt-bridge/src/daemon/log.rs::{to_file, service_log_path}` and
the binary's `daemon_main` (as the service the daemon appends its log to
`%PROGRAMDATA%\ZeroDDS\logs\mqtt-bridged.log`, the file of
zerodds-deployment-1.0 §4.1; a file that cannot be opened ends the start
with exit code 1; before, the service's log went to a stdout that does not
exist); spec §8.1 and §11 and `man/man1/zerodds-mqtt-bridged.1` (WINDOWS
SERVICE) name the service states, the controls and the log file. Before,
the binary had no service control dispatcher, and the watcher off Unix was
an empty thread.

**Tests:** `crates/mqtt-bridge/tests/example_yaml_loadback.rs::{mqttbridge_3_example_yaml_parses_without_error, mqttbridge_3_example_yaml_key_fields_match_intent, mqttbridge_11_example_yaml_topics_reach_the_daemon, mqttbridge_7_3_example_yaml_acl_reaches_the_daemon, mqttbridge_11_example_yaml_has_no_unknown_top_level_keys}`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_11_the_service_units_start_the_binary_with_flags_it_knows`
(the arguments of the systemd unit, the launchd plist, the Homebrew
service, the Windows script and the Docker image, with the example
configuration, pass `--check-config` with exit 0; of the Windows script
every word of `$binPath` after the program, before only the first `--`
word; the image carries the example configuration);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_11_the_binary_tells_the_service_manager_ready_and_stopping`
(the unit is `Type=notify`; the binary sends `READY=1` to `$NOTIFY_SOCKET`
once the broker connection stands and `STOPPING=1` after SIGTERM);
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::mqttbridge_11_the_service_requests_set_the_shutdown_and_reload_flags`
(the Windows watcher in a thread under Linux: without a request it waits,
it ends with the daemon, a reload request sets the reload flag, a stop
request the shutdown flag);
`crates/mqtt-bridge/src/daemon/log.rs::tests::{mqttbridge_11_the_lines_go_to_the_log_file_of_the_service, mqttbridge_11_the_log_file_of_the_service_is_under_programdata}`
(after `to_file` the lines go into the file as JSON, its directory is
made, a second `to_file` appends; the path under `%PROGRAMDATA%`, without
the variable under `C:\ProgramData`);
`crates/dcps/src/cli.rs::service_tests::{deployment_4_3_the_service_controls_are_stop_reload_and_interrogate, deployment_4_3_the_notify_states_are_service_states, deployment_4_3_the_exit_code_has_its_number, deployment_4_3_the_requests_and_the_run}`
(a console run is no service: `running_as_service` false);
`crates/dcps/src/cli/windows_service.rs::tests::deployment_4_3_the_control_codes_are_the_win32_constants`
(Windows only);
`tools/traceability/tests/deployment_layout.rs::{every_systemd_unit_carries_the_hardening_set_of_the_clause, every_launchd_plist_carries_the_keys_of_the_clause, windows_service_scripts_register_and_remove_the_same_bridge_set, every_daemon_image_is_cargo_chef_multi_stage_and_runs_non_root}`.

**Status:** partial — the tests ran green on the Linux test host; the
Windows code was compiled there with `cargo clippy --target
x86_64-pc-windows-gnu -p zerodds-mqtt-bridge --all-targets -- -D warnings`
and likewise for `zerodds-dcps` (`--lib --tests`), not run. Missing: the run of the service `ZeroDDSMQTTBridge` under
`sc.exe` (start, `READY=1` as `SERVICE_RUNNING`, stop with the drain,
`paramchange` as reload, the log in
`%PROGRAMDATA%\ZeroDDS\logs\mqtt-bridged.log`); it needs a Windows host
with the registered service, and the Linux test host is none. Missing: a
CI job that compiles this package's Windows code (`install_signal_watcher`
under `cfg(not(unix))`, the Windows path of `run_as_service`,
`running_as_service`): the Windows jobs of the CI (`ci/platform_jobs.py`,
`CORE` and `WINDOWS_EXTRA`) run only for the core crates and
`zerodds-foundation`, `zerodds-qos`, `zerodds-transport-shm`, and the CI
image (`ci/Dockerfile.rust`) does not have the target
`x86_64-pc-windows-gnu`; such a job needs one of the two ways and a run
that shows it green. The image of
`packaging/docker/mqtt-bridged/Dockerfile` was built once by hand on the
Linux test host (Docker 26.1.5, exit 0) and started: `--check-config` with
the image's configuration exit 0; with `--network host` and `--broker`
against Mosquitto 2.0.21 the lines `connect`, `subscribed`,
`catalog_published`, the catalog on `$zerodds/zerodds-mqtt-bridge/catalog`
read by `mosquitto_sub`, after `docker stop` `catalog_cleared`,
`disconnect` with 0x00 and exit 0. No job but `release.yml` at a release
builds the image.

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — ≥ 5 tests each for the modules configuration, MQTT
codec, topic map, QoS translation, DDS pump and ACL.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/codec.rs`, `crates/mqtt-bridge/src/daemon/topic_map.rs`,
`crates/mqtt-bridge/src/daemon/qos_translation.rs` (used by
`crates/mqtt-bridge/src/daemon/server.rs::register_topic` for the endpoints
and by `crates/mqtt-bridge/src/daemon/pump.rs::Pump` for the lifespan),
`crates/mqtt-bridge/src/daemon/pump.rs` (the mapping sample ↔ PUBLISH and
now also the pump thread `Pump`, `Outgoing` and `EchoGuard`, which were in
`crates/mqtt-bridge/src/daemon/server.rs`), `crates/mqtt-bridge/src/daemon/acl.rs`; spec §12.1 names the
modules with their paths in the tree. `crates/mqtt-bridge/src/dds_bridge.rs`
(`TopicMapper`, `MqttDdsBridge`) is the `no_std` mapping of the library,
which the daemon does not use; its tests do not count here.

**Tests:** per module (the number of tests):
`crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_12_1_config_loads_minimal, mqttbridge_3_the_defaults_of_the_keys, mqttbridge_5_1_an_out_topic_with_a_wildcard_is_refused, mqttbridge_3_the_reconnect_delays_grow_by_the_factor_up_to_the_maximum, mqttbridge_3_the_new_keys_refuse_what_they_do_not_accept}` (21);
`crates/mqtt-bridge/src/codec.rs::tests::{mqtt5_3_3_2_the_publish_of_figure_3_9, mqtt5_3_3_2_2_a_packet_identifier_only_above_qos_0, mqtt5_3_3_invalid_qos_3_rejected_on_encode, mqtt5_3_3_missing_packet_id_at_qos1_rejected, mqtt5_2_1_4_truncated_remaining_length_decode_fails}` (11);
`crates/mqtt-bridge/src/daemon/topic_map.rs::tests::{mqttbridge_5_1_a_plus_filter_maps_its_topics, mqttbridge_5_1_a_hash_filter_stops_at_the_level_separator, mqttbridge_4_3_one_subscription_and_identifier_per_filter, mqttbridge_4_3_the_identifier_selects_the_topic, mqttbridge_4_3_a_subscription_routes_once_it_is_granted, mqttbridge_5_1_two_topics_of_one_filter_each_get_its_publish_once}` (6);
`crates/mqtt-bridge/src/daemon/qos_translation.rs::tests::{mqttbridge_6_reliability_gives_the_mqtt_qos, mqttbridge_6_durability_gives_retain_and_the_replay, mqttbridge_6_lifespan_is_the_message_expiry_interval, mqttbridge_6_deadline_liveliness_and_partition_reach_the_endpoints, mqttbridge_6_a_duration_in_milliseconds_is_seconds_and_fractions}` (5);
`crates/mqtt-bridge/src/daemon/pump.rs::tests::{mqttbridge_4_2_the_properties_of_a_sample, mqttbridge_4_2_what_a_sample_does_not_have_is_left_out, mqttbridge_4_2_the_source_timestamp_round_trips_through_nanoseconds, mqttbridge_4_2_an_inbound_publish_and_what_is_refused, mqttbridge_4_4_a_dispose_marker_is_op_dispose_with_the_key_hash, mqttbridge_4_4_a_missing_op_is_sample, mqttbridge_4_4_an_unknown_op_is_refused, mqttbridge_4_3_the_echo_record_comes_before_the_publish_and_goes_when_it_fails}` (8);
`crates/mqtt-bridge/src/daemon/acl.rs::tests::{mqttbridge_7_3_a_rule_with_a_filter_covers_its_topics, mqttbridge_7_3_the_star_subject_is_everyone, mqttbridge_7_3_default_deny_closes_the_rest, mqttbridge_7_3_publish_and_subscribe_are_separate_rights, mqttbridge_7_3_a_filter_covers_and_overlaps_as_mqtt_matches, mqttbridge_7_3_the_subject_is_the_certificate_dn_then_the_user_name_then_the_jwt_sub}` (6);
plus `crates/mqtt-bridge/tests/fuzz_smoke.rs::mqttbridge_12_1_fuzz_decode_publish_no_panic`
and the six other fuzz smokes.

**Status:** done — the tests ran green on the Linux test host. Before, the
item cited tests of library modules and of `dds_bridge.rs`, which the
daemon does not use; the topic map and the QoS translation had 4 tests
each, the pump was in `crates/mqtt-bridge/src/daemon/server.rs`.

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — the binary as a process, Mosquitto, a round trip
MQTT↔DDS octet for octet.

**Repo:** `tests/integration/tests/bridges.rs`,
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`; spec §12.2 names
`tests/integration/tests/bridges.rs` and Mosquitto as a local process in
place of testcontainers.

**Tests:** `tests/integration/tests/bridges.rs::mqtt_bridge_against_mosquitto`
(job `integration-bridges`: the binary, Mosquitto as a process,
`mosquitto_sub` and `mosquitto_pub`, a DDS participant; both directions,
the payload compared octet by octet);
`crates/mqtt-bridge/tests/daemon_e2e.rs::{mqttbridge_2_daemon_connects_and_subscribes, mqttbridge_12_2_an_mqtt_publish_is_written_to_dds, mqttbridge_2_dds_publish_pumps_to_mqtt_broker}`
(the daemon in the test process: a DDS reader of the test gets the bytes of
a PUBLISH after the encapsulation header);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_1_the_binary_logs_a_publish_with_the_fields_of_the_spec`
(the binary against the crate's broker server, both directions over RTPS).

**Status:** done — the tests ran green on the Linux test host. Before, the
item cited only tests with the daemon in the test process; the test of the
direction MQTT → DDS sent a payload without an encapsulation header, which
the daemon refuses, and checked only that nothing crashed.

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone DDS participant through the bridge, broker matrix
Mosquitto/EMQX/HiveMQ/Aedes.

**Repo:** `crates/mqtt-bridge/tests/cross_vendor.rs`,
`crates/mqtt-bridge/tests/interop/{cyclone_reading.c, reading.idl}`,
`ci/integration_scenarios.py` (job `integration-bridges-vendor`, target
`cross_vendor`), `ci/integration-weekly.yml` (the brokers as services).

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_12_3_mosquitto_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_emqx_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_hivemq_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_aedes_carries_mqtt_to_cyclone_and_cyclone_to_mqtt}`
(a Cyclone DDS 0.10.5 participant, built by the test with `idlc` and the C
compiler, through the daemon on each broker of the matrix: a PUBLISH of
`mosquitto_pub` under `CDR_LE` and under `CDR2_LE` reaches the Cyclone
reader of that representation as the sample; a sample of the Cyclone
writer reaches `mosquitto_sub` as the header `00 01 00 00` or `00 07 00 00`
and the CDR bytes, and those octets reach the Cyclone reader again as the
sample written); the rest of the broker matrix in §10a.

**Status:** done — the tests ran green on the Linux test host. The
specification named Docker Compose; the test starts Mosquitto as a process,
EMQX, HiveMQ and Aedes are services of the job (the specification says so
now). Before, there was no `tests/cross_vendor.rs`, and the item cited for
the DDS side a test without RTPS.
## §13 Cross-references

### §13 Related library + standards + daemons

**Spec:** §13 — library `crates/mqtt-bridge/`, the OASIS MQTT-5 standard,
wire format, deployment, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config, major=wire-protocol
changes (MQTT-5.x→6).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — rule for assigning the version number of a release, not a property of the daemon.

---

## Audit status

28 done / 1 partial / 0 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-mqtt-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (job `merge-rust-bridge-security`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (job `merge-rust-config`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-mqtt-bridge --all-targets` (job `merge-rust-mqtt-bridge`) — 51 tests green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (job `merge-rust-websocket-bridge`) — 1 test green.
* `python3 ci/run_integration.py integration-bridges-vendor` (job `integration-bridges-vendor`) — 11 tests green.

70 cited tests green, 0 not green.

Open items: `zerodds-mqtt-bridge-1.0.open.en.md`.
