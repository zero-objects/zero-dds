# `zerodds-mqtt-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-mqtt-bridge-1.0.md`

Implementation:

- `crates/mqtt-bridge/` — DDS↔MQTT-Bridge.

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

### §2 Broker-Schemata mqtt/mqtts/ws/wss

**Spec:** §2 — `--broker` nimmt `mqtt://`, `mqtts://`, `ws://`, `wss://`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (`parse_broker_url`,
`BrokerUrl`), `crates/mqtt-bridge/src/daemon/ws_transport.rs` (MQTT 5.0 §6
über RFC 6455), `crates/mqtt-bridge/src/daemon/client.rs::connect_url`,
`crates/websocket-bridge/src/handshake.rs` (`render_client_request`,
`validate_server_response`).

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::{parse_broker_url_reads_the_websocket_schemes, parse_broker_url_refuses_what_it_would_not_use}`,
`crates/websocket-bridge/src/handshake.rs::tests::the_client_side_validates_the_server_response`;
gegen Mosquitto 2.0.21 (Linux-Testhost)
`crates/mqtt-bridge/tests/websocket_broker.rs::{ws_broker_url_carries_mqtt_both_ways_through_mosquitto, wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate, a_ws_url_on_a_plain_listener_fails_the_handshake}`
(beide Richtungen, eine Nachricht über 64 KiB, nicht vertrautes Zertifikat
und `ws://` auf einem reinen MQTT-Listener abgelehnt).

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`log_level`/`mqtt`/`topics`/`acl`/
`metrics`; ENV-Substitution `${VAR}` und `${VAR:-default}`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs`,
`crates/config/src/parse.rs` (the shared parser),
`crates/mqtt-bridge/src/daemon/mod.rs`.

**Tests:** `crates/mqtt-bridge/tests/daemon_e2e.rs::make_test_config`
(Config-Construction). Inline-Tests in `crates/mqtt-bridge/src/daemon/config.rs::tests` decken
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
encode/decode), `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus DDS → MQTT

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `mqtt_topic`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (slug helper),
`crates/mqtt-bridge/src/topic_filter.rs`.

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::slug_handles_double_colon`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::slug_replaces_unsafe_chars`;
die Filter-Seite in `crates/mqtt-bridge/src/topic_filter.rs::tests`
(`validate_simple_filter`, `topic_name_with_wildcard_rejected`,
`match_exact_topic`, `match_plus_wildcard`, `match_hash_wildcard`);
`crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.

**Status:** done

### §5.2 Catalog-Retain auf $zerodds/<client_id>/catalog

**Spec:** §5.2 — Catalog-Retain JSON mit topics-Liste.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (catalog-publish
hook), `crates/mqtt-bridge/src/daemon/runtime_common.rs` (Catalog-Retain
via Cluster-A-Wireup).

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::catalog_render_json_contains_topic_fields`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::admin_endpoint_serves_paths`;
dass ein `$`-Topic nur über einen expliziten Filter erreichbar ist, prüft
`crates/mqtt-bridge/src/topic_filter.rs::tests::dollar_topic_matched_by_explicit_filter`
und `::dollar_topic_not_matched_by_root_wildcard`; der Retain-Pfad selbst
`crates/mqtt-bridge/tests/broker_server_e2e.rs::retained_message_delivered_to_late_subscriber`.

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
`crates/mqtt-bridge/src/daemon/qos_translation.rs::tests`. Die QoS-1/2-
Abläufe mit dem Broker (MQTT 5.0 §4.3, Receive Maximum §4.9):
`crates/mqtt-bridge/tests/mosquitto_qos.rs::inbound_qos_1_and_2_are_acknowledged_and_all_messages_arrive`
(vorher kamen 20 von 60 QoS-1-Nachrichten an, gemessen),
`::outbound_qos_1_and_2_complete_and_leave_nothing_pending` (vorher leitete
Mosquitto 10 von 30 bestätigten QoS-1-Nachrichten nicht weiter, gemessen).

**Status:** done

## §7 Security

### §7.1 TLS mqtts:// + ALPN

**Spec:** §7.1 — `mqtts://`-Mode per `mqtt.tls.enabled`; ALPN `["mqtt"]`;
SIGHUP-Cert-Rotation.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (TLS-Hook),
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::tls_connect_to_broker_succeeds_with_valid_cert`;
die Zertifikats-Rotation in
`crates/bridge-security/src/connection.rs::tests::rotating_config_reload_swaps_inner_arc`
und `::rotating_config_reload_with_bad_path_keeps_old`; ALPN siehe §7.1a.

**Status:** done
### §7.1a ALPN-Aushandlung

**Spec:** §7.1 — der TLS-Handshake bietet ALPN an und der ausgehandelte
Protokoll-Name wird geprüft.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (`ALPN_MQTT` für
`mqtts://`, `ALPN_HTTP_1_1` unter `wss://`, `open_stream`).

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::tls_connect_offers_the_mqtt_alpn_protocol`
(der Broker liest `mqtt` als ausgehandeltes Protokoll zurück; ein Broker
nur mit anderem Protokoll lehnt ab);
`crates/mqtt-bridge/tests/websocket_broker.rs::wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate`
(`http/1.1` unter `wss://` gegen Mosquitto).

**Status:** done

### §7.2 SASL/MQTT-Auth-Modes

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

### §7.3 ACL Daemon-Side

**Spec:** §7.3 — `acl.default_deny` + `rules` mit `subject`/
`allow_publish`/`allow_subscribe`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (ACL-Felder),
`crates/mqtt-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::acl_skip_subscribe_when_subject_not_in_read_list`,
`crates/mqtt-bridge/tests/security_e2e.rs::ctx_from_config_builds_security_context_with_acl`;
`crates/mqtt-bridge/src/daemon/security.rs::tests::mqtt_acl_publish_check`;
`crates/mqtt-bridge/tests/example_yaml_loadback.rs::example_yaml_acl_reaches_the_daemon`.

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** —

**Tests:** —

**Status:** open — `crates/mqtt-bridge/src/daemon/cli.rs::tests` prüft die
Flag-Zerlegung, aber kein Test prüft die JSON-Log-Form oder die Wirkung von
`--log-level`.
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + `metrics.*`-Config; 12 Counter/Gauge
Familien.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs`,
`crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs` (Counter/Gauge-
Familien Cluster-A-Wireup).

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::metrics_register_idempotent`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::metrics_visible_in_prometheus_render`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::admin_endpoint_serves_paths`.

**Status:** done

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert Span-Emission.

**Repo:** `crates/mqtt-bridge/src/daemon/runtime_common.rs` (OTLP-Init
via `zerodds-observability-otlp`),
`crates/mqtt-bridge/src/daemon/client.rs` (Span-Emit pro PUBLISH-
Roundtrip).

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::otlp_config_parses_endpoint` — der Endpoint aus
`OTEL_EXPORTER_OTLP_ENDPOINT` wird gelesen. Dass pro PUBLISH-Roundtrip ein
Span entsteht, prüft kein Test — siehe §8.3a.

**Status:** partial — die Konfiguration ist geprüft, der Span-Emit nicht; der fehlende
Teil wird als §8.3a `open` geführt.
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

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::DaemonHandle::shutdown`
(Pumps stoppen, die Acks des Brokers bis `mqtt.drain_timeout_secs`
abwarten, DISCONNECT 0x00), `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`
(wartet auf das Stop-Flag; der Prozess lief vorher 3 s nach SIGTERM weiter,
gemessen), `::start_with_reload`, `::reload` (ACL; entzogene Rechte gelten
sofort).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_qos.rs::sigterm_disconnects_and_exits_0`,
`::the_shutdown_waits_for_the_acks_then_disconnects` (Mosquitto 2.0.21,
Linux-Testhost); `crates/mqtt-bridge/tests/daemon_e2e.rs::a_reload_that_withdraws_the_read_right_stops_the_samples`;
`crates/mqtt-bridge/src/daemon/client.rs::tests::backoff_connect_aborts_when_stop_set`.

**Status:** done

### §8.3a PUBLISH-Roundtrip-Span

**Spec:** §8.3 — bei gesetztem `OTEL_EXPORTER_OTLP_ENDPOINT` entsteht pro
PUBLISH-Roundtrip ein Span.

**Repo:** —

**Tests:** —

**Status:** open — der Emit-Teil von §8.3.

## §10 Cross-Vendor

### §10 RTPS-Peer + Mosquitto/EMQX/HiveMQ

**Spec:** §10 — Daemon ist normaler RTPS-Peer; MQTT-Seite gegen
Mosquitto/EMQX/HiveMQ getestet.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`.

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::zerodds_publish_received_by_mosquitto_sub`,
`crates/mqtt-bridge/tests/cross_vendor.rs::zerodds_daemon_connects_to_mosquitto_broker` (beide `#[ignore]`,
Feature `cross-vendor-tests`);
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::zerodds_311_client_receives_from_mosquitto`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mosquitto_311_client_receives_from_zerodds`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mosquitto_v5_clients_through_zerodds_broker`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mosquitto_v311_clients_through_zerodds_broker`;
die RTPS-Seite `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.
EMQX und HiveMQ hat niemand gemessen — siehe §10a.

**Status:** partial — Mosquitto ist gemessen, EMQX und HiveMQ nicht; der fehlende Teil
wird als §10a `open` geführt.
### §10a Broker-Matrix EMQX/HiveMQ

**Spec:** §10 — die MQTT-Seite läuft gegen EMQX und HiveMQ.

**Repo:** —

**Tests:** —

**Status:** open — der Teil von §10 und §12.3 jenseits Mosquitto.
`crates/mqtt-bridge/tests/cross_vendor.rs` kennt nur Mosquitto.

---

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

**Tests:** `crates/mqtt-bridge/tests/example_yaml_loadback.rs::{example_yaml_parses_without_error, example_yaml_key_fields_match_intent, example_yaml_topics_reach_the_daemon, example_yaml_acl_reaches_the_daemon, example_yaml_has_no_unknown_top_level_keys}`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel).

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/mqtt_codec/topic_map/qos_translate/dds_pump/
acl je ≥ 5 Tests.

**Repo:** `crates/mqtt-bridge/src/{daemon/config.rs,codec.rs,control_packets.rs,topic_filter.rs,properties.rs,reason_codes.rs,vbi.rs,packet.rs,data_types.rs,keep_alive.rs,dds_bridge.rs,broker.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul:
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
dazu `crates/mqtt-bridge/tests/fuzz_smoke.rs::fuzz_decode_publish_no_panic`
und die sechs weiteren Fuzz-Smokes.

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

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::zerodds_publish_received_by_mosquitto_sub`,
`crates/mqtt-bridge/tests/cross_vendor.rs::zerodds_daemon_connects_to_mosquitto_broker`;
die DDS-Seite `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.
Die Broker-Matrix jenseits Mosquitto ist nicht gemessen — siehe §10a.

**Status:** partial — Mosquitto und die DDS-Seite sind geprüft, die restliche
Broker-Matrix nicht.
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

20 done / 3 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-mqtt-bridge` — Tests grün, 0 failed.

Offene Punkte: siehe `zerodds-mqtt-bridge-1.0.open.md`. Decision-Records: keine.
