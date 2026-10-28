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

### §2 Broker schemes mqtt/mqtts/ws/wss

**Spec:** §2 — `--broker` takes `mqtt://`, `mqtts://`, `ws://`, `wss://`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (`parse_broker_url`,
`BrokerUrl`), `crates/mqtt-bridge/src/daemon/ws_transport.rs` (MQTT 5.0 §6
over RFC 6455), `crates/mqtt-bridge/src/daemon/client.rs::connect_url`,
`crates/websocket-bridge/src/handshake.rs` (`render_client_request`,
`validate_server_response`).

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::{parse_broker_url_reads_the_websocket_schemes, parse_broker_url_refuses_what_it_would_not_use}`,
`crates/websocket-bridge/src/handshake.rs::tests::the_client_side_validates_the_server_response`;
against Mosquitto 2.0.21 (Linux test host)
`crates/mqtt-bridge/tests/websocket_broker.rs::{ws_broker_url_carries_mqtt_both_ways_through_mosquitto, wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate, a_ws_url_on_a_plain_listener_fails_the_handshake}`
(both directions, one message above 64 KiB, an untrusted certificate and
`ws://` on a plain MQTT listener refused).

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`log_level`/`mqtt`/`topics`/`acl`/`metrics`;
ENV substitution `${VAR}` and `${VAR:-default}`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs`,
`crates/config/src/parse.rs` (the shared parser), `crates/mqtt-bridge/src/daemon/mod.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::make_test_config`
(config construction). Inline tests in `crates/mqtt-bridge/src/daemon/config.rs::tests` cover the YAML
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
encode/decode), `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

## §5 Topic mapping

### §5.1 Slug algorithm DDS → MQTT

**Spec:** §5.1 — lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; override per
`mqtt_topic`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (slug helper),
`crates/mqtt-bridge/src/topic_filter.rs`.

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::slug_handles_double_colon`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::slug_replaces_unsafe_chars`;
the filter side in `crates/mqtt-bridge/src/topic_filter.rs::tests`
(`validate_simple_filter`, `topic_name_with_wildcard_rejected`,
`match_exact_topic`, `match_plus_wildcard`, `match_hash_wildcard`);
`crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

### §5.2 Catalog retain on $zerodds/<client_id>/catalog

**Spec:** §5.2 — catalog retain JSON with a topics list.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (catalog-publish hook),
`crates/mqtt-bridge/src/daemon/runtime_common.rs` (catalog retain via
cluster-A wire-up).

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::catalog_render_json_contains_topic_fields`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::admin_endpoint_serves_paths`;
that a `$` topic is reachable only through an explicit filter is covered by
`crates/mqtt-bridge/src/topic_filter.rs::tests::dollar_topic_matched_by_explicit_filter`
and `::dollar_topic_not_matched_by_root_wildcard`; the retain path itself by
`crates/mqtt-bridge/tests/broker_server_e2e.rs::retained_message_delivered_to_late_subscriber`.

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
`crates/mqtt-bridge/src/daemon/qos_translation.rs::tests`. The QoS 1/2
flows with the broker (MQTT 5.0 §4.3, Receive Maximum §4.9):
`crates/mqtt-bridge/tests/mosquitto_qos.rs::inbound_qos_1_and_2_are_acknowledged_and_all_messages_arrive`
(20 of 60 QoS 1 messages arrived before, measured),
`::outbound_qos_1_and_2_complete_and_leave_nothing_pending` (Mosquitto did
not pass on 10 of 30 acknowledged QoS 1 messages before, measured).

**Status:** done

## §7 Security

### §7.1 TLS mqtts:// + ALPN

**Spec:** §7.1 — `mqtts://` mode per `mqtt.tls.enabled`; ALPN `["mqtt"]`;
SIGHUP cert rotation.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (TLS hook),
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::tls_connect_to_broker_succeeds_with_valid_cert`;
certificate rotation in
`crates/bridge-security/src/connection.rs::tests::rotating_config_reload_swaps_inner_arc`
and `::rotating_config_reload_with_bad_path_keeps_old`; ALPN see §7.1a.

**Status:** done
### §7.1a ALPN negotiation

**Spec:** §7.1 — the TLS handshake offers ALPN and the negotiated protocol
name is checked.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (`ALPN_MQTT` for
`mqtts://`, `ALPN_HTTP_1_1` below `wss://`, `open_stream`).

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::tls_connect_offers_the_mqtt_alpn_protocol`
(the broker reads back `mqtt` as the agreed protocol; a broker with only
another protocol refuses);
`crates/mqtt-bridge/tests/websocket_broker.rs::wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate`
(`http/1.1` below `wss://` against Mosquitto).

**Status:** done

### §7.2 SASL/MQTT auth modes

**Spec:** §7.2 — none/password/mtls/enhanced (SCRAM/OAUTHBEARER/JWT).

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`,
`crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/auth.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::auth_sasl_plain_emits_user_and_password`,
`crates/mqtt-bridge/tests/security_e2e.rs::auth_bearer_emits_password_in_connect`,
`crates/mqtt-bridge/tests/security_e2e.rs::outbound_credentials_bearer_yields_password_only`,
`crates/mqtt-bridge/tests/security_e2e.rs::outbound_credentials_sasl_yields_user_and_password`;
`crates/mqtt-bridge/src/daemon/security.rs::tests::mqtt_sasl_plain_accepts_user_pass`,
`::mqtt_sasl_plain_rejects_wrong_password`, `::mqtt_bearer_treats_password_as_token`,
`::mqtt_none_yields_anonymous_without_creds`.

**Status:** done

### §7.3 ACL daemon-side

**Spec:** §7.3 — `acl.default_deny` + `rules` with
`subject`/`allow_publish`/`allow_subscribe`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (ACL fields),
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::acl_skip_subscribe_when_subject_not_in_read_list`,
`crates/mqtt-bridge/tests/security_e2e.rs::ctx_from_config_builds_security_context_with_acl`;
`crates/mqtt-bridge/src/daemon/security.rs::tests::mqtt_acl_publish_check`;
`crates/mqtt-bridge/tests/example_yaml_loadback.rs::example_yaml_acl_reaches_the_daemon`.

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** —

**Tests:** —

**Status:** open — `crates/mqtt-bridge/src/daemon/cli.rs::tests` covers the flag
splitting, but nothing tests the JSON log form or the effect of
`--log-level`.
### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + `metrics.*` config; 12 counter/gauge
families.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs`,
`crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs` (counter/gauge families
cluster-A wire-up).

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::metrics_register_idempotent`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::metrics_visible_in_prometheus_render`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::admin_endpoint_serves_paths`.

**Status:** done

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables span emission.

**Repo:** `crates/mqtt-bridge/src/daemon/runtime_common.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/mqtt-bridge/src/daemon/client.rs`
(span emit per PUBLISH round-trip).

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::otlp_config_parses_endpoint` — the endpoint from
`OTEL_EXPORTER_OTLP_ENDPOINT` is read. Nothing tests that a span is emitted
per PUBLISH roundtrip — see §8.3a.

**Status:** partial — the configuration is covered, the span emit is not; the missing
half is carried as §8.3a `open`.
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

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::DaemonHandle::shutdown`
(the pumps stop, the broker's acks are awaited up to
`mqtt.drain_timeout_secs`, DISCONNECT 0x00),
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs` (waits for the stop
flag; the process was still running 3 s after SIGTERM before, measured),
`::start_with_reload`, `::reload` (ACL; withdrawn rights apply at once).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_qos.rs::sigterm_disconnects_and_exits_0`,
`::the_shutdown_waits_for_the_acks_then_disconnects` (Mosquitto 2.0.21,
the Linux test host); `crates/mqtt-bridge/tests/daemon_e2e.rs::a_reload_that_withdraws_the_read_right_stops_the_samples`;
`crates/mqtt-bridge/src/daemon/client.rs::tests::backoff_connect_aborts_when_stop_set`.

**Status:** done

### §8.3a PUBLISH roundtrip span

**Spec:** §8.3 — with `OTEL_EXPORTER_OTLP_ENDPOINT` set, one span is emitted
per PUBLISH roundtrip.

**Repo:** —

**Tests:** —

**Status:** open — the emit half of §8.3.

## §10 Cross-vendor

### §10 RTPS peer + Mosquitto/EMQX/HiveMQ

**Spec:** §10 — the daemon is a normal RTPS peer; the MQTT side tested
against Mosquitto/EMQX/HiveMQ.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`.

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::zerodds_publish_received_by_mosquitto_sub`,
`crates/mqtt-bridge/tests/cross_vendor.rs::zerodds_daemon_connects_to_mosquitto_broker` (both `#[ignore]`,
feature `cross-vendor-tests`);
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::zerodds_311_client_receives_from_mosquitto`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mosquitto_311_client_receives_from_zerodds`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mosquitto_v5_clients_through_zerodds_broker`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mosquitto_v311_clients_through_zerodds_broker`;
the RTPS side `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.
EMQX and HiveMQ have not been measured — see §10a.

**Status:** partial — Mosquitto is measured, EMQX and HiveMQ are not; the missing part
is carried as §10a `open`.
### §10a Broker matrix EMQX/HiveMQ

**Spec:** §10 — the MQTT side runs against EMQX and HiveMQ.

**Repo:** —

**Tests:** —

**Status:** open — the part of §10 and §12.3 beyond Mosquitto.
`crates/mqtt-bridge/tests/cross_vendor.rs` knows only Mosquitto.

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
`man/man1/zerodds-mqtt-bridged.1`, `man/man5/zerodds-mqtt-bridged.yaml.5`.

**Tests:** `crates/mqtt-bridge/tests/example_yaml_loadback.rs::{example_yaml_parses_without_error, example_yaml_key_fields_match_intent, example_yaml_topics_reach_the_daemon, example_yaml_acl_reaches_the_daemon, example_yaml_has_no_unknown_top_level_keys}`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys).

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/mqtt_codec/topic_map/qos_translate/dds_pump/acl, ≥ 5
tests each.

**Repo:** `crates/mqtt-bridge/src/{daemon/config.rs,codec.rs,control_packets.rs,topic_filter.rs,properties.rs,reason_codes.rs,vbi.rs,packet.rs,data_types.rs,keep_alive.rs,dds_bridge.rs,broker.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` per module:
`crates/mqtt-bridge/src/codec.rs::tests::publish_qos0_no_packet_id_round_trip`,
`crates/mqtt-bridge/src/control_packets.rs::tests::connect_body_round_trip_minimal`,
`crates/mqtt-bridge/src/topic_filter.rs::tests::match_hash_wildcard` (14),
`crates/mqtt-bridge/src/properties.rs::tests::property_id_round_trip_for_all_named`,
`crates/mqtt-bridge/src/reason_codes.rs::tests::round_trip_all_codes`,
`crates/mqtt-bridge/src/vbi.rs::tests::encode_128_yields_two_bytes_with_continuation`,
`crates/mqtt-bridge/src/packet.rs::tests::packet_type_round_trip_via_bits`,
`crates/mqtt-bridge/src/data_types.rs::tests::two_byte_int_round_trip`,
`crates/mqtt-bridge/src/keep_alive.rs::tests::record_packet_resets_timer` (5),
`crates/mqtt-bridge/src/dds_bridge.rs::tests::qos_1_maps_to_reliable_volatile`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::config_loads_minimal` (7),
`crates/config/tests/layering.rs::environment_references_expand_before_the_parse`;
plus `crates/mqtt-bridge/tests/fuzz_smoke.rs::fuzz_decode_publish_no_panic`
and the six other fuzz smokes.

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

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::zerodds_publish_received_by_mosquitto_sub`,
`crates/mqtt-bridge/tests/cross_vendor.rs::zerodds_daemon_connects_to_mosquitto_broker`;
the DDS side `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.
The broker matrix beyond Mosquitto is not measured — see §10a.

**Status:** partial — Mosquitto and the DDS side are covered, the rest of the broker
matrix is not.
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

20 done / 3 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-mqtt-bridge` — tests green, 0 failed.

Open items: see `zerodds-mqtt-bridge-1.0.open.en.md`. Decision records: none.
