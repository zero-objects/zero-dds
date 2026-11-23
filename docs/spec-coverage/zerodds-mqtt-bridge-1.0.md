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

**Status:** n/a (informative) — Zuordnung der Levels; die Anforderungen sind die Items §1a (L6), §1b (L1: MQTT 3.1.1 und Keep Alive) und §2 bis §12.

### §1a L6 Multi-Tenant

**Spec:** §1 L6 — mehrere DomainParticipants je Daemon; eine
Mandantenbindung je MQTT-Client-ID; §3 `tenants`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs::{TenantConfig, DaemonConfig::of_tenant, read_tenant, check_tenants}`,
`crates/mqtt-bridge/src/daemon/server.rs::{start_with_reload, start_core, TenantHandle, reload, settings_that_need_a_restart}`.

**Tests:** `crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_1_l6_each_tenant_has_its_participant_and_its_client_id, mqttbridge_1_l6_a_reload_applies_the_acl_of_a_tenant}`,
`crates/mqtt-bridge/tests/enhanced_auth.rs::mqttbridge_1_l6_a_tenant_presents_its_password_as_the_token`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_1_l6_the_tenants, mqttbridge_1_l6_a_tenant_authenticates_in_the_mode_of_the_top_level}`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_example_of_the_spec_loads`,
`crates/mqtt-bridge/src/daemon/server.rs::tests::mqttbridge_9_2_a_reload_names_the_settings_that_need_a_restart`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Ein
Mandant ist ein DomainParticipant auf seiner Domäne (GUID-Präfix aus seiner
Client-ID) und eine eigene Broker-Verbindung unter seiner `mqtt.client_id`,
mit eigenen Topics und eigener ACL. Er authentifiziert sich im Modus der
obersten Ebene: unter `password` und SCRAM-SHA-256 ersetzen sein
Benutzername und sein Passwort die der obersten Ebene; wo die oberste Ebene
`auth.bearer_token` vorlegt (`password` ohne `auth.password`, OAUTHBEARER,
eine andere Methode), ist sein Passwort das Token. Ein Benutzername unter
einer Token-Methode und Werte, die `none` oder `mtls` nicht nutzen,
verhindern den Start mit `tenants[<name>].mqtt` in der Meldung. Vorher
lehnte das Laden einen Mandanten mit eigenem `mqtt.password` unter
`enhanced` mit OAUTHBEARER ab (exit 1, gemessen), weil `of_tenant` das
Passwort in das Token und zugleich nach `mqtt.password` schrieb; jetzt
trägt das CONNECT des Mandanten gegen den Test-Broker sein Token. Geprüft gegen den
Broker-Server der Krate: beide Verbindungen stehen unter ihren Client-IDs,
eine Nachricht auf dem MQTT-Topic des Mandanten erreicht nur dessen
Teilnehmer, eine auf dem Topic der obersten Ebene nur deren Teilnehmer (beide
mit einem Topic gleichen DDS-Namens), ein Sample des Mandanten-Teilnehmers
geht auf dem Topic des Mandanten hinaus. Zwei Mandanten eines Namens und eine
Client-ID für zwei Verbindungen verhindern den Start. Ein SIGHUP übernimmt die
ACL jedes Mandanten sofort (nach `default_deny: true` ohne Regel gehen die
Samples des Mandanten nicht mehr hinaus, die der obersten Ebene schon) und
nennt eine andere Änderung an `tenants` als Grund für einen Neustart. Gegen einen fremden Broker ist die Mandantenbindung nicht
eigens geprüft. Die Log-Zeilen eines Mandanten tragen `tenant <name>:`; die
Metriken (§8.2) tragen ihn im Label `dds_topic` (`<tenant>/<topic>`).

### §1b L1 MQTT 3.1.1 und Keep Alive

**Spec:** §1 L1 — der Daemon spricht MQTT 5 und MQTT 3.1.1 als
Kompatibilitätsmodus (`mqtt.protocol_version`, §3); er hält die Verbindung mit
PINGREQ (MQTT 5.0 und 3.1.1 §3.1.2.10).

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs::{connect_body, MqttClient::wait_connack, MqttClient::keep_alive_tick, ClientError::is_auth_refusal}`,
`crates/mqtt-bridge/src/daemon/server.rs::start_core` (der Keep Alive in der
Empfangsschleife), `crates/mqtt-bridge/src/version.rs` und die `_v`-Codecs in
`crates/mqtt-bridge/src/control_packets.rs` und `crates/mqtt-bridge/src/codec.rs`.

**Tests:** `crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_1_l1_the_connect_of_the_3_1_1_mode, mqttbridge_1_l1_an_idle_client_pings_and_a_missing_pingresp_ends_the_connection, mqttbridge_1_l1_a_pingresp_keeps_the_connection, mqttbridge_4_2_the_properties_of_an_inbound_publish_reach_the_event}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_1_l1_the_daemon_speaks_mqtt_3_1_1, mqttbridge_1_l1_an_idle_daemon_stays_connected_past_one_and_a_half_keep_alives, mqttbridge_2_exit_5_on_connack_0x87, mqttbridge_4_4_under_3_1_1_a_dispose_is_not_published_as_a_sample}`;
gegen Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_qos.rs::{mqttbridge_1_l1_the_daemon_speaks_mqtt_3_1_1_to_mosquitto, mqttbridge_1_l1_an_idle_daemon_stays_connected_past_one_and_a_half_keep_alives}`
(`#[ignore]`, Job `integration-bridges-vendor`).

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Unter
`mqtt.protocol_version: 3.1.1` sendet der Daemon CONNECT mit Protokoll-Level 4
und PUBLISH, Acks und DISCONNECT ohne Properties; ein Dispose oder
Unregister wird dann nicht veröffentlicht (§4.4). Mosquitto führt ihn als
3.1.1-Client (`p2`), Nachrichten gehen in beide Richtungen, auch mit
3.1.1-Clients von Mosquitto. Die CONNACK-Codes 4 und 5 sind Exit 5 (§2). In
beiden Versionen sendet der Daemon PINGREQ, wenn er für die Keep-Alive-Zeit
nichts gesendet hat; vorher sendete er keines, und ein Broker durfte eine
stille Verbindung nach 1,5 Keep-Alive-Zeiten schließen. Mosquitto (Keep Alive
2 s, 5,5 s gewartet) und der Broker-Server der Krate (1 s, 3,5 s gewartet)
halten die Verbindung des Daemons; einen stillen Client gleicher
Keep-Alive-Zeit trennt der Broker-Server. Kommt die PINGRESP nicht innerhalb
der halben Keep-Alive-Zeit, schließt der Daemon die Verbindung; ein neuer
Verbindungsaufbau danach (Spec §9.3) fehlt noch.

## §2 CLI-Surface

### §2 zerodds-mqtt-bridged CLI

**Spec:** §2 — Optionen `--config`/`--broker`/`--client-id`/`--domain`/
`--username`/`--password`/`--tls-*`/`--topic`/`--log-level`/`--metrics`/
`--version`/`--help`; Exit-Codes 0/1/2/3/4/5.

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
Exit 0 nach SIGTERM gegen Mosquitto 2.0.21
`crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_9_2_sigterm_disconnects_and_exits_0`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Jede
Option ist `--set` ihres Schlüssels. Durch das Binary gegen den Broker-Server
der Krate erreichen `--broker`, `--client-id`, `--username` und `--password`
den CONNECT, `--topic` den SUBSCRIBE (eine Nachricht auf dem Topic erreicht
DDS) und `--metrics` den Admin-Endpunkt, der sie zählt; `--domain`,
`--log-level` und `--tls-*` stehen in der Ausgabe von `--check-config`. Die
Exit-Codes durch das Binary: 1 (ein Wert außerhalb seines Bereichs,
`mqtts://` ohne TLS-Abschnitt, `verify_hostname: false`, `mqtt.tls.cert_file`
ohne `key_file` und umgekehrt; ein `mqtt.session_dir`, den der Daemon nicht
anlegen oder schreiben kann, oder ein Session State, den ein anderer Prozess
hält; durch das Binary geprüft ist ein Verzeichnis unter einer Datei), 2 (kein Broker; der Start versucht es nach den
Wartezeiten von `mqtt.reconnect` erneut, kürzt die Wartezeit, die über
`mqtt.connect_timeout_secs` hinausginge, und versucht es zu diesem Zeitpunkt
ein letztes Mal; vorher endete er, sobald die nächste Wartezeit über die
Frist ging; mit den Standardwerten gegen einen geschlossenen Port endet er
jetzt nach 30,3 s mit den Wartezeiten 0,5, 1, 2, 4, 8 und 14,5 s, vorher nach
15,8 s (beides durch das Binary auf dem Linux-Testrechner gemessen); ein Broker,
der den TLS-Handshake nicht beantwortet oder die Verbindung darin schließt,
wird ebenso erneut versucht, vorher Exit 4 ohne zweiten Versuch; ein Versuch
wartet höchstens 10 s auf die TCP-Verbindung je Adresse, auch bei einem
Hostnamen, und 5 s auf jede Antwort; eine CONNACK-Ablehnung 0x85), 3 (Domäne
300 hat keine Ports), 4 (ein nicht vertrautes Broker-Zertifikat; vorher Exit
2, denn rustls führte den Handshake erst beim Schreiben des CONNECT aus;
TLS-Material, das nicht lädt, etwa ein fehlendes `mqtt.tls.ca_file`; nur ein
TLS-Fehler von rustls ist Exit 4, ein Timeout oder ein Verbindungsabbruch im
Handshake nicht), 5
(CONNACK 0x86, 0x87, unter MQTT 3.1.1 Code 5), 0 (SIGTERM, auch während der
Verbindungsversuche). Die Standard-Client-ID ist `zerodds-bridge-<host>`
(vorher `zerodds-bridge`). Die Regel für Exit 5 (0x86, 0x87, 0x8A, 0x8C;
3.1.1: 4, 5) steht in §2 und §4.1 gleich; vorher nannte §2 0x86/0x87 und §4.1
jede Ablehnung ab 0x80. Ein Konfigurationsfehler eines Auth-Modus ist Exit 1
(vorher Exit 4). Die Wirkung von `--log-level` auf die Ausgabe gehört zu §8.1
(offen).

### §2 Broker-Schemata mqtt/mqtts/ws/wss

**Spec:** §2 — `--broker` nimmt `mqtt://`, `mqtts://`, `ws://`, `wss://`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (`parse_broker_url`,
`BrokerUrl`), `crates/mqtt-bridge/src/daemon/ws_transport.rs` (MQTT 5.0 §6
über RFC 6455), `crates/mqtt-bridge/src/daemon/client.rs::connect_url`,
`crates/websocket-bridge/src/handshake.rs` (`render_client_request`,
`validate_server_response`).

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_2_parse_broker_url_reads_the_websocket_schemes, mqttbridge_2_parse_broker_url_refuses_what_it_would_not_use}`,
`crates/websocket-bridge/src/handshake.rs::tests::mqttbridge_2_the_client_side_validates_the_server_response`;
gegen Mosquitto 2.0.21 (Linux-Testhost)
`crates/mqtt-bridge/tests/websocket_broker.rs::{mqtt5_6_ws_broker_url_carries_mqtt_both_ways_through_mosquitto, mqtt5_6_wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate, mqtt5_6_a_ws_url_on_a_plain_listener_fails_the_handshake}`
(beide Richtungen, eine Nachricht über 64 KiB, nicht vertrautes Zertifikat
und `ws://` auf einem reinen MQTT-Listener abgelehnt).

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`log_level`/`mqtt`/`auth`/`topics`/
`tenants`/`acl`/`metrics`; ENV-Substitution `${VAR}` und `${VAR:-default}`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (`KEYS`,
`DaemonConfig::from_config`, `ReconnectConfig`, `TopicConfig::{effective_mqtt_qos, effective_retain, history_depth}`),
`crates/mqtt-bridge/src/daemon/history.rs` (`HistoryQueue`),
`crates/mqtt-bridge/src/daemon/config.rs::default_session_dir` (`mqtt.session_dir`),
`crates/config/src/parse.rs` (the shared parser),
`crates/mqtt-bridge/src/daemon/runtime_common.rs::serve_admin_endpoints`
(`metrics.path`), `crates/mqtt-bridge/src/daemon/server.rs::register_topic`, `crates/mqtt-bridge/src/daemon/pump.rs::Pump`
(`qos.history`), `crates/mqtt-bridge/src/daemon/client.rs::{connect_body, alpn_offer, connect_with_backoff}`.

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_3_the_example_of_the_spec_loads, mqttbridge_3_an_absent_mqtt_qos_is_none, mqttbridge_3_the_defaults_of_the_keys, mqttbridge_3_the_new_keys_refuse_what_they_do_not_accept, mqttbridge_3_the_reconnect_delays_grow_by_the_factor_up_to_the_maximum, what_the_old_parser_dropped_is_refused_with_its_line, mqttbridge_3_the_own_topic_prefix_is_read_and_reaches_the_tenants, mqttbridge_3_data_representation_names_the_representation_of_the_topic}`
(das Beispiel aus `docs/specs/zerodds-mqtt-bridge-1.0.md` §3, jeder Schlüssel
mit seinem Wert),
`crates/mqtt-bridge/src/daemon/qos_translation.rs::tests::mqttbridge_6_data_representation_is_the_offer_of_both_endpoints`,
`crates/mqtt-bridge/tests/daemon_e2e.rs::{mqttbridge_3_data_representation_xcdr2_matches_an_xcdr2_reader_before_the_first_publish, mqttbridge_3_data_representation_xcdr2_reader_matches_no_xcdr1_writer}`,
`crates/mqtt-bridge/src/daemon/status_watch.rs::tests::mqttbridge_6_the_reports_go_under_the_own_topic_prefix`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::mqttbridge_3_the_metrics_are_served_on_metrics_path`,
`crates/mqtt-bridge/tests/security_e2e.rs::mqttbridge_3_the_alpn_list_of_the_configuration_is_offered`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_9_1_an_unreachable_broker_is_retried_until_the_connect_timeout, mqttbridge_9_1_a_broker_that_comes_up_before_the_connect_timeout_is_reached}`,
`crates/mqtt-bridge/src/daemon/history.rs::tests::{mqttbridge_6_keep_last_keeps_the_newest_n_of_each_instance, mqttbridge_6_keep_all_drops_nothing, mqttbridge_6_a_sample_put_back_is_the_oldest_or_dropped}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_6_keep_last_on_an_out_topic_publishes_the_newest_n, mqttbridge_6_keep_all_on_an_out_topic_drops_nothing}`,
`crates/mqtt-bridge/tests/example_yaml_loadback.rs::{mqttbridge_3_example_yaml_parses_without_error, mqttbridge_3_example_yaml_key_fields_match_intent}`
(die ausgelieferte Beispiel-Konfiguration),
`crates/config/tests/layering.rs::environment_references_expand_before_the_parse`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Das
Beispiel der Spezifikation lädt; vorher lehnte der Parser
`mqtt.session_expiry_interval_secs`, `mqtt.max_packet_size`,
`mqtt.receive_maximum`, `mqtt.reconnect.*`, `mqtt.tls.verify_hostname`,
`mqtt.tls.alpn`, `qos.history.kind` und `metrics.path` ab. Jeder Schlüssel
wirkt: `mqtt.session_dir` (Standard `/var/lib/zerodds/mqtt-bridge`, nicht
leer) als Ort des Session State einer Sitzung, die ihre Verbindung
überdauert (§4.1), die vier CONNECT-Schlüssel in §4.1, `mqtt.reconnect` und
`mqtt.connect_timeout_secs` beim Verbindungsaufbau des Starts (§9.1; die
Wartezeit, die über die Frist hinausginge, wird auf sie gekürzt, der letzte
Versuch beginnt dort), `mqtt.tls.alpn` im TLS-Handshake, `metrics.path` am
Admin-Endpunkt, `qos.history` am DataWriter eines Topics (`in`, `bidir`) und
an den Samples, die der Daemon für den Broker hält, solange dieser nichts
bestätigt (`out`, `bidir`; KEEP_LAST N je Instanz, ältere werden verworfen
und in `zerodds_mqtt_bridge_dds_samples_dropped_total` gezählt, KEEP_ALL verwirft
keines, auch nicht nach den 10 s, die ein PUBLISH auf einen freien Platz
wartet; vorher wirkte `qos.history` auf einem `out`-Topic nicht, und nach
10 s ohne Bestätigung ging ein Sample verloren); `mqtt.tls.verify_hostname: false`
verhindert den Start mit Begründung, denn der Name wird immer geprüft. Ein
fehlendes `mqtt_qos` oder `retain` ist `None` und wird abgeleitet (§6); ein
gesetzter Wert bleibt, auch `mqtt_qos: 0` auf einem zuverlässigen Topic
(vorher wurde daraus 1). §3 nennt jetzt `auth`, `mqtt.tls.server_name`,
`mqtt.protocol_version`, `mqtt.topic_alias_maximum`,
`mqtt.connect_timeout_secs` und `tenants`. Ein Wiederverbinden mit
`mqtt.reconnect` nach einem Abbruch und die Samples, die währenddessen
ankommen, gehören zu Spec §9.3. Zwei Schlüssel kamen aus der Broker-Matrix
von §10a: `mqtt.own_topic_prefix` (Default `$zerodds`) trägt den Katalog,
die Berichte und den Will (HiveMQ 4.57.0 lehnt ein Will-Topic, das mit `$`
beginnt, mit CONNACK 0x82 ab); der Topic-Schlüssel `data_representation`
(`xcdr1`, `xcdr2`) ist die eine Repräsentation, die beide DDS-Endpunkte des
Topics ab ihrer Erzeugung verwenden, ein PUBLISH der anderen XCDR-Version
wird abgelehnt (ohne ihn passten die XCDR2-Reader von Cyclone DDS, Fast DDS
und RTI Connext zu keinem Writer des Daemons). Auf einem `out`- oder
`bidir`-Topic nimmt der DataReader dann nur diese Repräsentation an: ein
DDS-Writer, der nur die andere anbietet, passt zu keinem Reader des Daemons
(DDS-XTypes 1.3 §7.6.3.1.2). Die Beispiel-Konfiguration setzte den
Schlüssel auf ihrem `out`-Topic und schloss damit XCDR1-Writer aus; jetzt
steht er auf dem `in`-Topic `Command`, und der Kommentar, §3 der Spezifikation
und die Manpage nennen die Wirkung auf den Reader.

## §4 MQTT-Wire-Protocol

### §4.1 CONNECT mit MQTT-5-Properties

**Spec:** §4.1 — CONNECT mit Session-Expiry/Receive-Maximum/Max-Packet-
Size/Topic-Alias-Max/Request-Response-Information/Request-Problem-Information,
User-Properties `zerodds_version`/`zerodds_role`, optional
Authentication-Method/Authentication-Data; CONNACK-Ablehnung der Zugangsdaten
→ Exit 5; was CONNECT und CONNACK ankündigen, gilt.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs::{connect_body, MqttClient::wait_connack, MqttClient::publish, MqttClient::resolve_topic, MqttClient::take_buffered_packet, MqttClient::establish, MqttClient::resume, MqttClient::read_event, malformed, BrokerLimits}`,
`crates/mqtt-bridge/src/daemon/session.rs` (`SessionState`, die Datei in `mqtt.session_dir`; `append_whole`: ein Eintrag, den die Platte nur zum Teil nimmt, wird wieder abgeschnitten),
`crates/mqtt-bridge/src/property_set.rs` (Properties nach Table 2-4),
`crates/mqtt-bridge/src/daemon/server.rs::start_core` (die Grenzen des
CONNACK je Topic im Log); Authentication Method und Data unter
`auth.mode: enhanced` siehe §7.2 (`MqttClient::send_connect`,
`MqttClient::wait_connack`).

**Tests:** `crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_4_1_the_connect_carries_the_properties_of_the_table, mqttbridge_4_1_the_server_keep_alive_replaces_the_configured_one, mqttbridge_4_1_the_limits_of_the_connack_hold_for_the_publish, mqttbridge_4_1_an_inbound_topic_alias_is_resolved, mqttbridge_4_1_a_packet_above_the_maximum_packet_size_is_0x95, mqttbridge_4_1_qos_2_beyond_our_receive_maximum_is_0x93, mqttbridge_4_1_a_resumed_session_resends_its_state, mqttbridge_4_1_the_resend_holds_the_receive_maximum, mqttbridge_4_1_session_present_without_state_begins_a_new_session, mqttbridge_4_1_session_present_after_clean_start_is_a_protocol_error, mqttbridge_4_1_a_packet_that_does_not_decode_ends_the_connection_with_0x81}`,
`crates/mqtt-bridge/src/daemon/session.rs::tests::{mqttbridge_4_1_the_session_state_outlives_the_client, mqttbridge_4_1_session_present_0_and_clean_start_discard_the_state, mqttbridge_4_1_a_cut_off_or_foreign_session_file, mqttbridge_4_1_one_process_holds_a_session_state, mqttbridge_4_1_the_session_file_is_rewritten_with_the_live_entries, mqttbridge_4_1_a_record_the_disk_takes_in_part_is_cut_off_again}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_4_1_the_broker_reads_each_connect_property, mqttbridge_4_1_an_inbound_topic_alias_is_resolved, mqttbridge_4_1_the_maximum_qos_of_the_connack_holds_for_the_publish, mqttbridge_4_1_a_restarted_daemon_resends_what_the_broker_did_not_acknowledge}`;
`crates/mqtt-bridge/tests/enhanced_auth.rs::{mqttbridge_7_2_enhanced_connect_carries_method_and_data, mqttbridge_7_2_an_auth_continue_is_answered_before_connack}`;
gegen Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_4_1_the_session_survives_a_reconnect_with_clean_start_false`
(`#[ignore]`, Job `integration-bridges-vendor`).

**Status:** done — grün auf dem Linux-Testrechner. Authentication Method
und Authentication Data trägt der CONNECT unter `auth.mode: enhanced`
(§7.2), mit dem Austausch von AUTH bis zum CONNACK. Vorher trug der CONNECT keine
Property, und `clean_start: false` hatte ohne Session Expiry Interval keine
Wirkung. Jetzt trägt er Session Expiry Interval, Receive Maximum, Maximum
Packet Size, Topic Alias Maximum (`mqtt.topic_alias_maximum`, Standard 100),
Request Response Information 0, Request Problem Information 1 und die beiden
User-Properties (Oktette nach MQTT 5.0 §3.1.2.11 geprüft). Gegen Mosquitto
übersteht die Sitzung mit `clean_start: false` und Session Expiry Interval 300
die Trennung (Session Present 1, drei QoS-1-Nachrichten aus der Pause kommen
an), mit Intervall 0 nicht. Was der CONNECT ankündigt, gilt für den Broker:
ein Paket über Maximum Packet Size beendet die Verbindung mit DISCONNECT 0x95,
ein Topic Alias über dem Maximum mit 0x94, Aliase darunter werden aufgelöst,
mehr unfreigegebene QoS-2-Nachrichten als Receive Maximum enden mit 0x93,
ein Paket des Brokers, das sich nicht dekodieren lässt (ein PUBLISH mit
abgeschnittenem Topic Name, der reservierte Pakettyp 0, CONNACK, PUBACK,
PUBREC, PUBREL, PUBCOMP, SUBACK, AUTH, Remaining Length), mit 0x81 Malformed
Packet (MQTT 5.0 §4.13.1); vorher endete es als `ClientError::Codec` ohne
DISCONNECT. Was
der CONNACK ankündigt, gilt für den Daemon: Maximum QoS, Retain Available und
Maximum Packet Size verhindern das PUBLISH eines Samples (Log-Zeile,
Fehlerzähler, die Verbindung bleibt); Server Keep Alive ersetzt
`keep_alive_secs`; Session Present und Assigned Client Identifier werden
gelesen. Session State (MQTT 5.0 §4.1, [MQTT-3.1.2-23]): bei
`clean_start: false` mit Session Expiry Interval über 0 (3.1.1: immer) hält
der Daemon seine QoS-1/2-PUBLISH-Pakete ohne Abschluss durch den Broker und
die QoS-2-PUBLISH-Pakete des Brokers vor dessen PUBREL in einer Datei je
Client-ID in `mqtt.session_dir`, geschrieben vor dem Paket; nach einem
Neustart sendet er bei Session Present 1 die PUBLISH-Pakete mit DUP 1 und die
PUBREL-Pakete mit ihren Packet Identifiern in ihrer Reihenfolge erneut,
innerhalb des Receive Maximum des Brokers, und liefert ein vor dem Neustart
bestätigtes QoS-2-PUBLISH nicht noch einmal nach DDS; bei Session Present 0
verwirft er den Zustand; Session Present 1 ohne Zustand schließt die
Verbindung und beginnt eine neue Sitzung mit Clean Start 1
([MQTT-3.2.2-4], unter 3.1.1 danach ein dritter CONNECT mit Clean Session
0); Session Present 1 auf Clean Start 1 ist ein Protokollfehler (0x82). Durch
das Binary nicht ausgeführt; gegen den eigenen Broker im Test über zwei
Starts des Daemons im Testprozess. Vorher begann jeder Start bei Packet
Identifier 1 ohne Zustand, nichts wurde erneut gesendet, und ein QoS-2-Ablauf
des vorigen Prozesses, der nach PUBREC abbrach, hätte den Identifier beim
Broker offen gelassen (abgeleitet, nicht gegen einen Broker ausgeführt). Die
Datei wird nicht auf die Platte synchronisiert: ein Absturz des Prozesses
behält sie, ein Absturz des Betriebssystems kann die letzten Einträge
verlieren. Ein Eintrag, den die Platte nur zum Teil nimmt (volle Platte),
wird wieder abgeschnitten; gelingt das nicht, wird die Datei vor dem
nächsten Eintrag neu geschrieben. Vorher folgte der nächste Eintrag dem
abgebrochenen, und die Datei las sich danach nicht mehr richtig (im Test mit
einer Platte, die nach einer Anzahl Oktette scheitert; ein echter
Schreibfehler der Platte wurde nicht erzeugt). Exit 5 siehe §2.

### §4.2 PUBLISH mit Content-Type + User-Properties

**Spec:** §4.2 — PUBLISH mit `Payload Format Indicator=0`, Content-Type
`application/x-dds-cdr2` (XCDR1: `application/x-dds-cdr`), User-Properties
`zerodds_type`/`zerodds_topic`/`zerodds_flags`/`zerodds_key_hash`/
`zerodds_source_ts_ns`, Message Expiry Interval aus der Lifespan; der
Encapsulation-Header des Samples + CDR; MQTT→DDS mit Quell-Zeitstempel und
Key-Hash.

**Repo:** `crates/mqtt-bridge/src/daemon/pump.rs` (`publish_properties`,
`inbound_sample`, `content_type_of`, `timestamp_ns`, `message_expiry_of`),
`crates/mqtt-bridge/src/daemon/server.rs::start_core` (Pumpe und
Empfangsschleife), `crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::publish, InboundEvent}`.

**Tests:** `crates/mqtt-bridge/src/daemon/pump.rs::tests::{mqttbridge_4_2_the_properties_of_a_sample, mqttbridge_4_2_what_a_sample_does_not_have_is_left_out, mqttbridge_4_2_the_source_timestamp_round_trips_through_nanoseconds, mqttbridge_4_2_an_inbound_publish_and_what_is_refused}`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_4_2_the_publish_carries_its_properties, mqttbridge_4_2_the_properties_of_an_inbound_publish_reach_the_event}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_4_2_a_dds_sample_is_published_with_content_type_and_user_properties, mqttbridge_4_2_an_inbound_source_timestamp_is_the_dds_source_timestamp}`,
`crates/mqtt-bridge/tests/daemon_e2e.rs::xtypes_7_6_3_1_2_a_publish_carries_the_header_the_sample_came_under`;
gegen Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_qos.rs::{mqttbridge_4_2_mosquitto_sub_prints_the_user_properties, mqttbridge_4_2_mosquitto_pub_user_properties_reach_dds}`
(`#[ignore]`, Job `integration-bridges-vendor`).

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Vorher
trug das PUBLISH keine Property, und ein eingehendes PUBLISH verlor Zeitstempel
und Key-Hash. Jetzt trägt es Payload Format Indicator 0, den Content Type der
Repräsentation, `zerodds_type`, `zerodds_topic`, `zerodds_flags` (`0x00` für
ein Sample), `zerodds_key_hash` und `zerodds_source_ts_ns`, wenn das Sample sie
hat, und den Message Expiry Interval seiner Lifespan (Sekunden, gerundet,
mindestens 1); `mosquitto_sub` gibt sie aus. Eingehend werden Key-Hash und
Quell-Zeitstempel der DDS-Schreibvorgang (mit `mosquitto_pub -D` geprüft); ein
Payload Format Indicator 1, ein fremder Content Type oder einer zur anderen
XCDR-Version des Headers wird mit Log-Zeile und Fehlerzähler abgelehnt. Der
Header ist der, unter dem das Sample kam (Spec §4.2 nennt das jetzt; vorher
stand dort fest `00 07 00 00`). Unter MQTT 3.1.1 gibt es keine Properties
(§1b). `zerodds_flags` eines Dispose und `zerodds_op` gehören zu §4.4.

### §4.3 SUBSCRIBE mit Subscription-Identifier

**Spec:** §4.3 — SUBSCRIBE pro `direction=in|bidir` mit Subscription-
Identifier, QoS aus DDS abgeleitet, NoLocal=1.

**Repo:** `crates/mqtt-bridge/src/daemon/topic_map.rs::InboundRoutes` (ein
Abonnement und ein Subscription Identifier je Topic-Filter, ab 1 in der
Reihenfolge der Konfiguration; die Zuordnung eines PUBLISH zu den Topics
über seine Identifier und den Filter), `crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::subscribe, SubscribeEntry, SubscribeOutcome, subscription_options}`
(ein SUBSCRIBE je Filter mit Property 0x0B, Optionen QoS | No Local, das
Warten auf jedes SUBACK; ohne Identifier bei Subscription Identifier
Available 0, kein Wildcard-Filter bei Wildcard Subscription Available 0;
unter MQTT 3.1.1 nur die QoS), `crates/mqtt-bridge/src/daemon/server.rs::subscribe_routes`, `crates/mqtt-bridge/src/daemon/pump.rs::{EchoGuard, Pump}`
(Ablehnung im SUBACK als Log-Zeile und Fehlerzähler; der DataReader eines
bidir-Topics veröffentlicht die Samples des eigenen DataWriters nicht; unter
3.1.1 wird die Kopie des eigenen PUBLISH nicht in DDS geschrieben: der
Eintrag im `EchoGuard` entsteht vor dem Senden unter der Sperre des
Clients und wird bei einem fehlgeschlagenen PUBLISH wieder entfernt).

**Tests:** `crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_4_3_the_subscribe_carries_identifier_and_no_local, mqttbridge_4_3_what_the_broker_or_the_protocol_does_not_have_is_not_sent}`,
`crates/mqtt-bridge/src/daemon/topic_map.rs::tests::{mqttbridge_4_3_one_subscription_and_identifier_per_filter, mqttbridge_4_3_the_identifier_selects_the_topic}`,
`crates/mqtt-bridge/src/daemon/pump.rs::tests::mqttbridge_4_3_the_echo_record_comes_before_the_publish_and_goes_when_it_fails`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_4_3_each_filter_is_subscribed_with_its_identifier_and_no_local, mqttbridge_4_3_a_refused_subscription_is_logged, mqttbridge_4_3_a_bidir_topic_does_not_send_its_own_messages_back, mqttbridge_4_3_under_3_1_1_the_copy_of_its_own_publish_is_not_written}`;
gegen Mosquitto `crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_4_3_a_bidir_topic_does_not_get_its_own_publish_back`.

**Status:** done — grün auf dem Linux-Testrechner, die Mosquitto-Probe gegen
Mosquitto 2.0.21. Unter 3.1.1 wurde der Eintrag im `EchoGuard` erst nach dem
Freigeben der Sperre gemacht; las die Empfangsschleife die Kopie des Brokers
dazwischen, schrieb sie sie ein zweites Mal in DDS (durch Lesen gefunden,
der Wettlauf ist nicht nachgestellt). Die Spec widersprach sich bei der QoS (§4.3 RELIABLE→2,
§6 Vorgabe 1); §4.3 verweist jetzt auf die Ableitung von §6. Vorher sendete
der Daemon ein SUBSCRIBE für alle Filter ohne Identifier und ohne No Local,
las kein SUBACK und ordnete eingehende Nachrichten nur über den genauen
Namen oder ein Textpräfix vor `/#` zu.

### §4.4 zerodds_op Control-Property

**Spec:** §4.4 — User-Property `zerodds_op` Werte `sample`/`dispose`/
`unregister`/`register`; Default `sample`.

**Repo:** `crates/mqtt-bridge/src/daemon/pump.rs::{Op, op_of, lifecycle_message, lifecycle_properties, inbound_message, InboundMessage}`
(DDS → MQTT: Dispose `dispose` 0x01, Unregister `unregister` 0x02, beides
`dispose` 0x03, Key-Hash, Header `00 06 00 00` und Key-Holder; MQTT → DDS:
die vier Werte, `zerodds_flags` passend zur Operation),
`crates/mqtt-bridge/src/daemon/pump.rs::Pump::take`, `crates/mqtt-bridge/src/daemon/server.rs::write_inbound`
(`UserSample::Lifecycle` in die History der Instanz, mit QoS und
Retain-Flag des Topics; unter MQTT 3.1.1 nicht veröffentlicht, die erste
Änderung eines Topics geloggt, jede als Fehler gezählt, die Instanz verlässt
an ihrer Stelle in der History den Replay-Puffer; eingehend
`DcpsRuntime::write_user_lifecycle`, `register` schreibt nichts).

**Tests:** `crates/mqtt-bridge/src/daemon/pump.rs::tests::{mqttbridge_4_4_a_dispose_marker_is_op_dispose_with_the_key_hash, mqttbridge_4_4_a_missing_op_is_sample, mqttbridge_4_4_an_unknown_op_is_refused}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_4_4_a_dds_dispose_reaches_mqtt_and_an_mqtt_dispose_reaches_a_dds_reader`
(ein zweiter Participant des Tests über RTPS: sein Dispose und Unregister
kommen als PUBLISH mit `zerodds_op` an, ein PUBLISH mit `dispose` kommt bei
seinem Reader als NOT_ALIVE_DISPOSED an),
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_4_4_under_3_1_1_a_dispose_is_not_published_as_a_sample`
(unter 3.1.1 kommen beim Abonnenten nur die zwei Samples an, Dispose und
Unregister zählen zwei Fehler).

**Status:** done — grün auf dem Linux-Testrechner. Unter MQTT 3.1.1 gibt es
keine Properties: ein Dispose oder Unregister wird nicht veröffentlicht und
gezählt (Spec §4.4, §1 L1); vorher ging es als PUBLISH mit `00 06 00 00`
und Key-Holder hinaus, das jeder Empfänger als Sample las und das auf einem
retained Topic das letzte Sample ersetzte. Vorher verwarf die Pumpe
jedes `UserSample::Lifecycle`, eingehende User-Properties wurden nicht
gelesen; die zitierten Tests `properties.rs::tests` gab es für `zerodds_op`
nicht. Ein Dispose an einen Reader eines anderen Herstellers gehört zu §10.

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus DDS → MQTT

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `mqtt_topic`.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs::{default_mqtt_slug, check_mqtt_topic}`
(der Slug; `mqtt_topic` eines `out`- oder `bidir`-Topics ist ein Topic-Name
ohne `+` und `#`, der eines `in`-Topics ein gültiger Topic-Filter, sonst ein
Konfigurationsfehler mit Datei und Zeile; `server::start` prüft auch eine im
Code gebaute Konfiguration), `crates/mqtt-bridge/src/daemon/topic_map.rs::InboundRoutes::route`
(eingehend über `crate::topic_filter::matches`, Ebene für Ebene),
`packaging/linux/configs/mqtt-bridged.yaml.example` (das `out`-Topic ohne
Wildcard, ein `in`-Topic mit Filter).

**Tests:** `crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_5_1_slug_handles_double_colon, mqttbridge_5_1_slug_replaces_unsafe_chars, mqttbridge_5_1_slug_of_three_levels, mqttbridge_5_1_an_out_topic_with_a_wildcard_is_refused, mqttbridge_5_1_an_in_topic_may_be_a_filter}`,
`crates/mqtt-bridge/src/daemon/topic_map.rs::tests::{mqttbridge_5_1_a_plus_filter_maps_its_topics, mqttbridge_5_1_a_hash_filter_stops_at_the_level_separator}`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_5_1_a_filter_topic_takes_every_topic_it_matches`,
`crates/mqtt-bridge/tests/example_yaml_loadback.rs::mqttbridge_11_example_yaml_topics_reach_the_daemon`;
die Filter-Seite in `crates/mqtt-bridge/src/topic_filter.rs::tests`
(`mqtt311_3_8_validate_simple_filter`, `mqttbridge_5_1_topic_name_with_wildcard_rejected`,
`mqttbridge_5_1_match_exact_topic`, `mqttbridge_5_1_match_plus_wildcard`, `mqttbridge_5_1_match_hash_wildcard`).

**Status:** done — grün auf dem Linux-Testrechner. Vorher wurde
`mqtt_topic` nicht geprüft: das ausgelieferte Beispiel veröffentlichte auf
`factory/+/telemetry`, was MQTT 5.0 [MQTT-3.3.2-2] verbietet; eingehend nahm
`sensors/#` auch `sensorsX/a`, `+` wurde nicht ausgewertet.

### §5.2 Catalog-Retain auf $zerodds/<client_id>/catalog

**Spec:** §5.2 — Catalog-Retain JSON mit topics-Liste.

**Repo:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::{CatalogSnapshot::render_json, catalog_topic}`
(das JSON der Spec: `topics` mit `dds_name`, `dds_type`, `mqtt_topic`, `qos`
mit `reliability` und `durability`, `schema_url` nur wenn gesetzt; dasselbe
Dokument unter `/catalog` des Admin-Endpunkts),
`crates/mqtt-bridge/src/daemon/server.rs::{publish_catalog, clear_catalog, Session::track_catalog, Core::drain}`
(nach dem CONNACK retained, QoS 1, Payload Format Indicator 1, Content Type
`application/json` auf `$zerodds/<client_id>/catalog`, je Tenant mit seinen
Topics; nach einem Reconnect unter einer neu zugewiesenen Client-ID wird der
Catalog der ID davor gelöscht; beim Shutdown ein retained PUBLISH ohne
Payload vor dem DISCONNECT),
`crates/mqtt-bridge/src/daemon/config.rs::TopicConfig::schema_url` (Schlüssel
`topics[].schema_url`, Spec §3, man5, Beispielkonfiguration).

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::{mqttbridge_5_2_the_catalog_is_the_json_of_the_spec, mqttbridge_5_2_admin_endpoint_serves_paths}`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_example_of_the_spec_loads`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_5_2_the_catalog_is_retained_and_cleared_at_the_shutdown, mqttbridge_5_2_a_tenant_publishes_the_catalog_of_its_topics_and_clears_it, mqttbridge_5_2_a_new_assigned_identifier_clears_the_catalog_of_the_one_before}`
(der Catalog des Mandanten unter seiner Client-ID mit nur seinen Topics,
beim Shutdown beide gelöscht; mit leerer `mqtt.client_id` und einem
Reconnect unter einer neuen zugewiesenen ID steht die Bridge einmal im
Catalog, unter der neuen ID, und eine verpasste Deadline wird unter der
neuen ID gemeldet); gegen Mosquitto `crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_5_2_a_late_subscriber_gets_the_retained_catalog`.

**Status:** done — grün auf dem Linux-Testrechner; Mosquitto 2.0.21 nimmt
das PUBLISH auf `$zerodds/…` an und hält es retained (gemessen; MQTT 5.0
§4.7.2 überlässt `$`-Topics dem Server). EMQX und HiveMQ nicht gemessen
(§10a). Vorher blieb nach einem Reconnect unter einer neu zugewiesenen
Client-ID der Catalog der ID davor retained stehen, und die Meldungen von
§6 gingen weiter unter der ID der ersten Verbindung hinaus. Vorher
veröffentlichte der Daemon keinen Catalog beim Broker; das
JSON unter `/catalog` wich von §5.2 ab (`service`, `version`, `direction`,
`mqtt_qos`, History, kein `schema_url`).

## §6 QoS-Translation

### §6 DDS-QoS → MQTT-Behavior + Auto-Derivation

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/
Liveliness/Partition Map; Auto-Derivation `mqtt_qos` aus Reliability,
`retain` aus Durability.

**Repo:** `crates/mqtt-bridge/src/daemon/qos_translation.rs::{mqtt_qos, retain, durability, replayed, reader_config, writer_config, lifespan_nanos, message_expiry}`
(die Zeilen der Tabelle; `register_topic` und das SUBSCRIBE nutzen sie),
`crates/mqtt-bridge/src/daemon/config.rs::TopicConfig` (Schlüssel
`qos.lifespan_ms`, `qos.deadline_ms`, `qos.liveliness.{kind, lease_ms}`,
`qos.partition`; `mqtt_qos` und `retain` bleiben, wenn gesetzt),
`crates/mqtt-bridge/src/daemon/history.rs::HistoryQueue` (KEEP_LAST/KEEP_ALL
der Samples, die auf den Broker oder die Verbindung warten),
`crates/mqtt-bridge/src/daemon/replay.rs::ReplayBuffer` (TRANSIENT im
Speicher, PERSISTENT auch in `<client id>.replay` in `mqtt.session_dir`;
das neueste hinausgegangene Sample je Instanz, nach jedem CONNACK erneut
veröffentlicht, abgelaufene nicht; eine Datei, die in ihrem letzten
Eintrag endet, gibt die Einträge davor mit der Warnung
`replay_file_truncated` und wird neu geschrieben; ein Eintrag, den die
Platte nur zum Teil nimmt, wird wieder abgeschnitten),
`crates/mqtt-bridge/src/daemon/client.rs::MqttClient::publish_until` (der
Message Expiry Interval ist die Restzeit beim Senden, nach dem Warten auf
einen Sendeplatz; abgelaufen `ClientError::Expired`, nicht gesendet), `crates/mqtt-bridge/src/daemon/status_watch.rs::StatusWatch`
(verpasste Deadline von DataReader und DataWriter als retained PUBLISH auf
`$zerodds/<client_id>/deadline_missed/<slug>`, verlorene Liveliness eines
DDS-Writers als PUBLISH auf `$zerodds/<client_id>/liveliness_lost/<slug>`),
`crates/mqtt-bridge/src/daemon/client.rs::{connect_body, liveliness_topic}`
(der Will des CONNECT auf `$zerodds/<client_id>/liveliness_lost`),
`crates/mqtt-bridge/src/daemon/pump.rs::{Pump, message_expiry_of}` (Sekunden,
gerundet ohne Überlauf, höchstens 4294967295), `crates/mqtt-bridge/src/daemon/server.rs::{Session::replay_to, reconnect}`.

**Tests:** `crates/mqtt-bridge/src/daemon/qos_translation.rs::tests::{mqttbridge_6_reliability_gives_the_mqtt_qos, mqttbridge_6_durability_gives_retain_and_the_replay, mqttbridge_6_lifespan_is_the_message_expiry_interval, mqttbridge_6_a_lifespan_at_the_bounds_gives_no_wrong_interval, mqttbridge_6_deadline_liveliness_and_partition_reach_the_endpoints}`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_6_the_time_left_of_the_longest_lifespan_is_the_largest_interval`,
`crates/mqtt-bridge/src/daemon/replay.rs::tests::{mqttbridge_6_transient_keeps_the_newest_sample_of_each_instance, mqttbridge_6_persistent_samples_are_read_back_after_a_restart, mqttbridge_6_an_expired_sample_is_not_replayed, mqttbridge_6_a_cut_off_last_record_leaves_the_records_before_it, mqttbridge_6_a_torn_replay_file_is_written_again_before_the_next_record}`,
`crates/mqtt-bridge/src/daemon/status_watch.rs::tests::mqttbridge_6_the_deadline_report`,
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_6_the_connect_carries_the_liveliness_will`,
`crates/mqtt-bridge/src/daemon/history.rs::tests::{mqttbridge_6_keep_last_keeps_the_newest_n_of_each_instance, mqttbridge_6_keep_all_drops_nothing, mqttbridge_6_a_sample_put_back_is_the_oldest_or_dropped}`,
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_an_absent_mqtt_qos_is_none`,
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_6_an_explicit_mqtt_qos_0_stays_0_on_a_reliable_topic, mqttbridge_6_lifespan_is_the_message_expiry_interval, mqttbridge_6_a_sample_that_waited_goes_out_with_the_time_left_or_not_at_all, mqttbridge_6_the_samples_of_an_outage_arrive_once_and_in_order, mqttbridge_6_a_sample_of_another_partition_is_not_published, mqttbridge_6_a_missed_deadline_is_a_retained_publish, mqttbridge_6_a_lost_writer_liveliness_is_published, mqttbridge_6_the_broker_publishes_the_will_when_the_daemon_is_killed, mqttbridge_6_transient_samples_are_replayed_after_the_broker_restarts, mqttbridge_6_persistent_samples_are_replayed_after_a_daemon_restart, mqttbridge_6_a_replay_file_cut_off_in_its_last_record_does_not_prevent_the_start, mqttbridge_6_keep_last_on_an_out_topic_publishes_the_newest_n, mqttbridge_6_keep_all_on_an_out_topic_drops_nothing}`;
die QoS-1/2-Abläufe mit Mosquitto (MQTT 5.0 §4.3, Receive Maximum §4.9):
`crates/mqtt-bridge/tests/mosquitto_qos.rs::{mqttbridge_6_inbound_qos_1_and_2_are_acknowledged_and_all_messages_arrive, mqttbridge_6_outbound_qos_1_and_2_complete_and_leave_nothing_pending, mqttbridge_6_mosquitto_publishes_the_will_when_the_daemon_is_killed}`.

**Status:** done — die elf Zeilen sind gebaut, ihre Tests liefen grün auf
dem Linux-Testrechner, die Mosquitto-Tests gegen Mosquitto 2.0.21. Eine
Replay-Datei, die in ihrem letzten Eintrag endet (der Daemon endete beim
Schreiben), verhinderte vorher jeden weiteren Start mit Exit 1, bis sie von
Hand entfernt wurde; jetzt startet der Daemon mit den Einträgen davor
(Test durch den Daemon im Testprozess). Ein Eintrag, den die Platte nur zum
Teil nahm, blieb vorher stehen, und die folgenden Einträge hingen dahinter;
ein echter Schreibfehler der Platte wurde nicht erzeugt. Ein
Sample, das im Daemon wartet (History, Verbindungsverlust), geht mit der
Restzeit seiner Lifespan hinaus (4 s Lifespan, 2,2 s gewartet: Interval 2),
eines mit abgelaufener Lifespan nicht, gezählt in
`dds_samples_dropped_total`; vorher stand der Interval beim Eintreffen fest.
Der Replay-Puffer nimmt ein Sample erst, wenn es hinausgegangen ist: die
während eines Broker-Ausfalls eintreffenden Samples kommen nach dem
Reconnect je einmal und in ihrer Reihenfolge an (gemessen: 0 aus dem
Puffer, dann 1, 2, 3); vorher ging das neueste wartende zweimal hinaus, das
erste Mal vor den älteren. Der
Message Expiry Interval eines eingehenden PUBLISH erreicht DDS nicht (DDS
kennt keine Lifespan je Sample, Spec §6 sagt es); die DataWriter-Lifespan
kommt aus `qos.lifespan_ms`. Vorher waren nur Reliability und das Retain von
TRANSIENT_LOCAL gebaut: `qos_translation.rs` hatte keinen Aufrufer und
widersprach §6 (KEEP_ALL→QoS 2), es gab keinen Replay-Puffer, keine
Schlüssel für Lifespan, Deadline, Liveliness und Partition, keinen Will.
Eine Lifespan im letzten halben Sekundenbereich unter `u64::MAX` ns
(`qos.lifespan_ms` 18446744073709551615, auch die Restzeit eines gerade
eingetroffenen Samples) ließ die Rundung überlaufen: im Debug-Build eine
Panik (gemessen), im Release-Build nach der Wrapping-Semantik ein Interval
von 1 s (abgeleitet, kein Release-Build gelaufen). Jetzt wird ohne
Überlauf gerundet, und ein `qos.lifespan_ms` ab 2^31 - 1 s ist wie die
LIFESPAN des DataWriters unendlich: kein Message Expiry Interval.

## §7 Security

### §7.1 TLS mqtts:// + ALPN

**Spec:** §7.1 — `mqtts://`-Mode per `mqtt.tls.enabled`; ALPN `["mqtt"]`;
SIGHUP-Cert-Rotation.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::{LiveTls, Session::connect, reload, reconnect}`
(die TLS-Einstellungen in Kraft — Client-Konfiguration aus CA, Zertifikat
und Schlüssel, `server_name`, `alpn`; ein SIGHUP ersetzt sie nach einem
erfolgreichen `crates/mqtt-bridge/src/daemon/security.rs::client_tls_from_config`, jede danach beginnende Verbindung
nutzt sie, die stehende behält ihre; TLS-Material, das nicht lädt, lehnt
den Reload ab), `crates/mqtt-bridge/src/daemon/client.rs::{open_stream, alpn_offer}`,
`crates/mqtt-bridge/src/daemon/config.rs` (`mqtt.tls.alpn`;
`mqtt.tls.verify_hostname: false` und `mqtt.tls.enabled: true` mit
`mqtt://`/`ws://` verhindern den Start), `crates/mqtt-bridge/src/daemon/security.rs`.

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::{mqttbridge_7_1_tls_connect_to_broker_succeeds_with_valid_cert, mqttbridge_7_1_a_sighup_puts_the_new_client_certificate_into_the_next_connection, mqttbridge_7_1_a_failed_reload_keeps_the_old_certificate, mqttbridge_7_1_a_sighup_puts_the_new_server_name_and_alpn_into_the_next_connection, mqttbridge_7_1_a_sighup_reads_the_ca_file_again, mqttbridge_3_the_alpn_list_of_the_configuration_is_offered}`
(ein TLS-Broker im Test hält das Client-Zertifikat jeder Verbindung und
Servername und ALPN-Angebot jedes ClientHello fest, ohne
Sitzungswiederaufnahme: nach SIGHUP und dem Schließen der ersten Verbindung
kommt die zweite mit dem neuen Zertifikat, nach einem Reload mit fehlendem
Schlüssel mit dem alten; nach einem Reload mit `server_name:
broker.example` und `alpn: [x-amzn-mqtt-ca, mqtt]` trägt das nächste
ClientHello beides; wechselt der Broker auf ein Zertifikat, dem die CA des
Starts nicht traut, scheitern die Reconnects, bis die CA-Datei das neue
nennt und ein SIGHUP sie neu liest), `crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_new_keys_refuse_what_they_do_not_accept`
(`verify_hostname: false`, `tls.enabled` mit `mqtt://`); ALPN siehe §7.1a.

**Status:** done — grün auf dem Linux-Testrechner. Dass ein Reload auch
`server_name` und `alpn` übernimmt und `ca_file` neu liest, hatte vorher
keinen Test. Vorher baute `reload`
die neue TLS-Konfiguration und verwarf sie, es gab keine neue Verbindung,
die sie hätte nutzen können; zitiert waren Tests von
`bridge-security::RotatingTlsConfig`, einem Server-Acceptor, den dieser
Daemon nicht nutzt. `mqtt.tls.verify_hostname: false` wird abgelehnt statt
einen Verifier ohne Namensprüfung zu bauen (Entscheidung aus dem vorigen
Arbeitspaket, Spec §3); ein anderer Name geht über `mqtt.tls.server_name`.

### §7.1a ALPN-Aushandlung

**Spec:** §7.1 — der TLS-Handshake bietet ALPN an und der ausgehandelte
Protokoll-Name wird geprüft.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs` (`ALPN_MQTT` für
`mqtts://`, `ALPN_HTTP_1_1` unter `wss://`, `open_stream`).

**Tests:** `crates/mqtt-bridge/tests/security_e2e.rs::mqttbridge_7_1_tls_connect_offers_the_mqtt_alpn_protocol`
(der Broker liest `mqtt` als ausgehandeltes Protokoll zurück; ein Broker
nur mit anderem Protokoll lehnt ab);
`crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate`
(`http/1.1` unter `wss://` gegen Mosquitto).

**Status:** done

### §7.2 SASL/MQTT-Auth-Modes

**Spec:** §7.2 — none/password/mtls/enhanced (SCRAM/OAUTHBEARER/JWT).

**Repo:** `crates/mqtt-bridge/src/daemon/auth.rs::{AuthKind, Credentials, check, is_loopback_host, Exchange}`
(der Modus aus `auth.mode` oder aus dem Konfigurierten, die Zugangsdaten des
CONNECT je Modus, die Prüfung beim Laden und beim Start, der Austausch je
Methode), `crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::handshake, MqttClient::send_connect, MqttClient::wait_connack, MqttClient::reauthenticate, MqttClient::on_auth, ClientError::Auth}`
(Authentication Method und Data im CONNECT, AUTH 0x18 bis zum CONNACK,
[MQTT-4.12.0-5]/[MQTT-4.12.0-6], AUTH 0x19), `crates/mqtt-bridge/src/daemon/server.rs::{LiveAuth, same_credentials, same_auth_way, reload}`
(ein SIGHUP mit gleichem Modus und gleicher Methode ersetzt die
Zugangsdaten für die nächste Verbindung; nur geänderte Zugangsdaten lassen
eine Verbindung mit enhanced sich neu authentifizieren; ein anderer Modus
behält die alten und wird als Neustart genannt), `crates/mqtt-bridge/src/daemon/config.rs`
(`auth.mode` mit `password` und den Namen `bearer`, `sasl`, `sasl_plain`,
`auth.enhanced.method`; `auth.sasl_users` entfernt),
`crates/bridge-security/src/client_auth.rs::{ScramClient, oauthbearer_initial_response, jwt_subject_unverified}`
(SCRAM-SHA-256 nach RFC 5802/7677 auf dem Krypto-Backend `ring`/`aws-lc-rs`,
SASLprep nach RFC 4013, OAUTHBEARER nach RFC 7628).

**Tests:** `crates/bridge-security/src/client_auth.rs::tests::{rfc7677_3_the_exchange_of_the_rfc, rfc5802_3_a_wrong_server_signature_and_an_error_fail, rfc5802_5_1_what_a_server_first_message_must_hold, rfc5802_5_1_the_user_name_is_prepared_and_escaped, rfc7628_3_1_the_initial_client_response}`;
`crates/mqtt-bridge/src/daemon/auth.rs::tests::{mqttbridge_7_2_none_is_refused_for_a_broker_that_is_not_loopback, mqttbridge_7_2_the_mode_follows_from_what_is_configured, mqttbridge_7_2_password_puts_user_name_and_password_into_the_connect, mqttbridge_7_2_mtls_sends_no_user_name_and_no_password, mqttbridge_7_2_enhanced_needs_the_input_of_its_method, mqttbridge_7_2_the_exchange_of_each_method}`;
`crates/mqtt-bridge/tests/enhanced_auth.rs::{mqttbridge_7_2_enhanced_connect_carries_method_and_data, mqttbridge_7_2_an_auth_continue_is_answered_before_connack, mqttbridge_7_2_a_server_signature_that_does_not_verify_ends_the_connection, mqttbridge_7_2_oauthbearer_answers_an_error_challenge_with_0x01, mqttbridge_7_2_an_auth_or_method_the_connect_did_not_ask_for_is_a_protocol_error, mqttbridge_7_2_a_scram_re_authentication_answers_the_continue, mqttbridge_7_2_a_reload_re_authenticates_with_the_new_token, mqttbridge_7_2_a_reload_that_keeps_the_credentials_does_not_re_authenticate}`
(ein Test-Broker mit der Serverseite von SCRAM-SHA-256 nach RFC 5802 §3 und
OAUTHBEARER);
der Reload: `crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_7_2_a_reload_gives_the_next_connect_the_new_password`
(unter `password` trägt das CONNECT nach dem Ende der stehenden Verbindung
das neue Passwort), `crates/mqtt-bridge/src/daemon/server.rs::tests::{mqttbridge_7_2_a_reload_takes_new_credentials_of_the_same_way_only, mqttbridge_9_2_a_reload_names_the_settings_that_need_a_restart}`
(ein Reload nur der ACL oder von `auth.bearer_subject` beginnt keine neue
Generation, neue Zugangsdaten schon, das Subjekt folgt dem neuen
Benutzernamen; ein anderer `auth.mode` oder eine andere
`auth.enhanced.method` behält Zugangsdaten und Subjekt und steht in der
Neustart-Liste);
`crates/mqtt-bridge/tests/security_e2e.rs::{mqttbridge_7_2_auth_bearer_emits_password_in_connect, mqttbridge_7_2_auth_sasl_plain_emits_user_and_password, mqttbridge_7_2_the_bearer_token_is_the_password_without_a_user_name, mqttbridge_7_2_auth_username_and_password_win_over_the_mqtt_keys, mqttbridge_7_2_mtls_sends_no_user_name_and_no_password}`;
`crates/mqtt-bridge/tests/contract_bridge_security.rs::{mqttbridge_7_2_the_scram_client_runs_the_exchange_of_rfc_7677, mqttbridge_7_2_the_oauthbearer_response_is_the_one_of_rfc_7628}`;
gegen Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_auth.rs::{mqttbridge_7_2_a_wrong_password_is_exit_5_against_mosquitto, mqttbridge_7_2_mtls_and_the_dn_subject_against_mosquitto}`
(`#[ignore]`, Job `integration-bridges-vendor`);
gegen EMQX 5.8.6 `crates/mqtt-bridge/tests/emqx_auth.rs::{mqttbridge_7_2_scram_sha_256_and_a_jwt_connect_to_emqx, mqttbridge_7_2_emqx_refuses_a_wrong_scram_password_and_a_forged_jwt}`
(der SCRAM-Authenticator von EMQX mit eingebauter Datenbank, SHA-256, 4096
Iterationen, und sein JWT-Authenticator mit HMAC SHA-256 und dem Token aus
dem Passwort des CONNECT, eingerichtet über die REST-API von EMQX; ein
Daemon unter `enhanced` mit SCRAM-SHA-256 und einer unter `password` mit
dem JWT verbinden sich, ein Sample des ersten erreicht DDS über den
zweiten; EMQX lehnt einen falschen SCRAM-Beweis und ein JWT eines anderen
Schlüssels ab; `#[ignore]`, Job `integration-bridges-vendor` mit EMQX als
Service `emqx`).

**Status:** done — grün auf dem Linux-Testrechner, die Mosquitto-Tests
dort mit Mosquitto 2.0.21, die EMQX-Tests gegen EMQX 5.8.6 im Container.
Gegen EMQX laufen AUTH, CONNACK, die Properties 0x15/0x16 und die
SCRAM-Nachrichten auf der Brokerseite durch den Code von EMQX, nicht durch
den Codec der Krate; ein symmetrischer Fehler im Codec (die Kennung von
Authentication Data in Kodierer und Dekodierer gleich verschoben) besteht
die Tests gegen den Test-Broker und scheitert gegen EMQX (Mutation,
gemessen). Ohne fremden Broker bleiben: OAUTHBEARER und eine andere Methode
mit dem Token als Authentication Data (EMQX 5.8.6 CE hat dafür keinen
Authenticator, seine API antwortet `unsupported_mechanism`; Mosquitto hat
keine eingebaute Enhanced Authentication) und die Neu-Authentifizierung:
EMQX 5.8.6 nimmt AUTH 0x19 an, beantwortet es mit AUTH 0x18 und dem
server-first-message, prüft den client-final-message mit Erfolg, schickt
dann aber kein AUTH 0x00 und schließt die Verbindung fünf Sekunden später
(`session_stepdown_request_timeout` in `emqx_channel:post_process_connect`
im Log von EMQX); danach lehnte es neue Verbindungen derselben Client-ID
eine Zeit lang mit CONNACK 0x85 ab (gemessen). Diese Teile sind nur gegen
den Test-Broker geprüft. Vorher nahm `auth.mode` nur `none`, `bearer`,
`sasl`, `sasl_plain` und `mtls` an: `none` war zu jedem Broker erlaubt,
`mtls` schickte `mqtt.username` und `mqtt.password` trotzdem mit, `enhanced`
gab es nicht (kein Authentication Method/Data, kein AUTH im Daemon, ein AUTH
des Brokers wurde stillschweigend verworfen), und die zitierten Tests von
`authenticate_mqtt` prüften eine Serverfunktion, die der Daemon nie aufrief;
sie ist mit `auth.sasl_users` entfernt. Ein Reload mit gleichem Modus
schickte außerdem AUTH 0x19 auch dann, wenn sich nur die ACL geändert
hatte.

### §7.3 ACL Daemon-Side

**Spec:** §7.3 — `acl.default_deny` + `rules` mit `subject`/
`allow_publish`/`allow_subscribe`.

**Repo:** `crates/mqtt-bridge/src/daemon/acl.rs::{DaemonAcl, AclRule, Subject, LiveAcl, filter_covers, filters_overlap, subject_of, certificate_subject}`
(Regeln je Subjekt mit MQTT-Topic-Filtern; `may_publish` am MQTT-Topic des
ausgehenden PUBLISH, `may_subscribe` am Filter des SUBSCRIBE,
`may_receive` am Topic eines PUBLISH des Brokers; das Subjekt aus dem DN
des Client-Zertifikats, dem Benutzernamen, mit dem sich der Daemon
anmeldet (`crates/mqtt-bridge/src/daemon/auth.rs::Credentials::user_name`: der des CONNECT unter
`password`, der SCRAM-Benutzer unter `enhanced`), dem `sub` des JWT,
ersetzt durch `auth.bearer_subject`), `crates/mqtt-bridge/src/daemon/server.rs::{Session::after_connack, Session::replay_to, reload}`, `crates/mqtt-bridge/src/daemon/pump.rs::Pump::may_publish`
(Ablehnung als Log-Zeile `acl_denied` und in
`zerodds_mqtt_bridge_acl_denials_total{reason}`; ein Reload ersetzt Regeln
und Subjekt), `crates/mqtt-bridge/src/daemon/config.rs::read_acl` (Filter
geprüft).

**Tests:** `crates/mqtt-bridge/src/daemon/acl.rs::tests::{mqttbridge_7_3_a_rule_with_a_filter_covers_its_topics, mqttbridge_7_3_the_star_subject_is_everyone, mqttbridge_7_3_default_deny_closes_the_rest, mqttbridge_7_3_publish_and_subscribe_are_separate_rights, mqttbridge_7_3_a_filter_covers_and_overlaps_as_mqtt_matches, mqttbridge_7_3_the_subject_is_the_certificate_dn_then_the_user_name_then_the_jwt_sub}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_7_3_the_rules_of_the_spec_let_alice_publish_chat_and_nothing_else`
(die Regeln des Beispiels von §7.3, Benutzer `alice`: `chat/general` und
`sensors/s1/public/temp` kommen am Broker an, `news/today` nicht; das
SUBSCRIBE auf `chat/+` geht nicht hinaus; ein PUBLISH auf ein öffentliches
Sensor-Topic erreicht DDS);
`crates/mqtt-bridge/tests/enhanced_auth.rs::mqttbridge_7_3_a_rule_for_the_scram_user_grants_its_topic`
(unter SCRAM-SHA-256 ist der SCRAM-Benutzer das Subjekt: seine Regel gibt
`scram/a` frei, `other/b` wird unter `default_deny: true` abgelehnt und
gezählt);
`crates/mqtt-bridge/tests/daemon_e2e.rs::mqttbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_samples`;
`crates/mqtt-bridge/tests/example_yaml_loadback.rs::mqttbridge_7_3_example_yaml_acl_reaches_the_daemon`;
`crates/mqtt-bridge/tests/contract_bridge_security.rs::{mqttbridge_7_3_the_subject_of_a_certificate_is_its_rfc_4514_dn_and_its_fingerprint, mqttbridge_7_3_the_sub_of_a_jwt_is_its_subject}`;
gegen Mosquitto 2.0.21 `crates/mqtt-bridge/tests/mosquitto_auth.rs::mqttbridge_7_2_mtls_and_the_dn_subject_against_mosquitto`
(Subjekt `CN=bridge-01` aus dem Client-Zertifikat; `#[ignore]`, Job
`integration-bridges-vendor`).

**Status:** done — grün auf dem Linux-Testrechner. Vorher kehrte der
Daemon die Regeln in eine Tabelle mit dem Filtertext als Schlüssel um und
fragte sie mit dem DDS-Topic-Namen ab (exakter Vergleich): eine Regel
`chat/+` griff nie auf `Chat::Message`. Das ausgehende PUBLISH wurde gegen
`allow_subscribe` geprüft, das Subjekt war `auth.bearer_subject` oder die
Konstante `zerodds-mqtt-bridge`, eine Ablehnung zählte nur
`errors_total`. Die zitierten Tests bestanden, weil ihre Schlüssel
DDS-Namen waren. Außerdem hatte der Daemon
unter `enhanced` mit SCRAM-SHA-256 kein Subjekt (es galten nur die Regeln
von `*`): `subject_of` nahm den Benutzernamen des CONNECT, den SCRAM nicht
schickt.

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log auf stdout, ein Objekt pro Zeile, Felder
timestamp/level/event/mqtt_topic/dds_topic/bytes/mqtt_qos/latency_us/connect_state;
`log_level` (`--log-level`) als Schwelle, ein SIGHUP-Reload übernimmt einen
geänderten Wert; die Tabelle der Events mit Level und Feldern.

**Repo:** `crates/mqtt-bridge/src/daemon/log.rs` (`Level`, `set_level`,
`Event::field/number/integer/flag/tenant/reason_code/latency/render/emit`,
`rfc3339`), `crates/mqtt-bridge/src/daemon/server.rs` (die Events von
Start, Verbindung, Verbindungsverlust und Reconnect mit `connect_state`,
Subscriptions, Katalog, Replay, ACL, Reload und Shutdown; `publish_in` mit
`latency_us` vom Lesen des PUBLISH bis zur Rückkehr des DDS-Writes),
`crates/mqtt-bridge/src/daemon/pump.rs::Pump` (`publish_out` mit
`latency_us` von der Entnahme des Samples aus dem DataReader bis zum Ende
des Socket-Writes; `samples_waiting`, `sample_expired`, `publish_failed`),
`crates/mqtt-bridge/src/daemon/client.rs` und
`crates/mqtt-bridge/src/daemon/session.rs` (Sitzung, Re-Authentifizierung,
Ablehnungen des Brokers mit `client_id`; auf der Verbindung eines
Mandanten mit `tenant` aus `DaemonConfig::tenant`, das
`crates/mqtt-bridge/src/daemon/config.rs::DaemonConfig::of_tenant` setzt;
vorher fehlte `tenant` auf diesen Zeilen),
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs` (`running`,
`shutdown`, `stopped`, `start_failed` auf stdout und als Text auf stderr;
als Windows-Dienst in die Log-Datei von §11);
die Tabelle der Events in der Spec §8.1, `man/man1/zerodds-mqtt-bridged.1`
und `man/de/man1/zerodds-mqtt-bridged.1`. Vorher schrieb der Daemon
Freitext mit `eprintln!` auf stderr, und `log_level` wurde gelesen und von
keinem Code-Pfad benutzt.

**Tests:** `crates/mqtt-bridge/src/daemon/log.rs::tests::{mqttbridge_8_1_a_line_is_one_json_object_with_timestamp_level_and_event, mqttbridge_8_1_text_is_escaped, mqttbridge_8_1_the_timestamp_is_rfc_3339_utc, mqttbridge_8_1_an_unset_field_is_left_out, mqttbridge_8_1_a_line_below_log_level_is_not_written}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_1_the_binary_logs_a_publish_with_the_fields_of_the_spec`
(das Binary mit `log_level: debug` gegen den Broker-Server des Crates und
einen DDS-Teilnehmer des Tests: jede Zeile ist JSON mit Zeitstempel nach
RFC 3339, `level` und `event`; `connect` mit `connect_state` connected,
`publish_out` und `publish_in` mit `mqtt_topic`, `dds_topic`, `bytes`,
`mqtt_qos`, `latency_us`; nach SIGTERM `disconnect` mit `connect_state`
disconnected und `reason_code` 0x00, dann `stopped`; stderr bleibt leer);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_1_log_level_warn_writes_no_info_line_and_a_reload_applies_debug`
(mit `warn` schreiben Verbindung, Subscription und eine Nachricht keine
Zeile, die abgelehnte Subscription ihre `warn`-Zeile; nach dem Umschreiben
der Datei auf `debug` und SIGHUP folgen `reload` und `publish_in`);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_4_3_a_refused_subscription_is_logged`
(`subscription_refused` mit Filter und `reason_code` 0x87, `subscribed` mit
1 von 2); `crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_1_the_client_lines_of_a_tenant_carry_its_name`
(das Binary mit einem Mandanten gegen einen Broker des Tests, der eine
Sitzung ohne Zustand fortsetzt und jedes Katalog-PUBLISH mit PUBACK 0x87
ablehnt: `session_unknown` und `publish_refused_by_broker` des Mandanten
tragen `tenant` a, die der obersten Ebene kein `tenant`; ohne `.tenant()`
an `publish_refused_by_broker` scheitert der Test, gemessen mit einer
Mutation); `crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_1_l6_the_tenants`
(`of_tenant` setzt `tenant`); `tests/integration/tests/bridges.rs::mqtt_bridge_against_mosquitto`
wartet auf die Zeile `"event":"subscribed"`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Die
Log-Datei des Windows-Dienstes steht unter §11 (unter Windows nicht
gelaufen).
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + `metrics.*`-Config; die Familien
`zerodds_mqtt_bridge_*` mit ihren Labels.

**Repo:** `crates/mqtt-bridge/src/daemon/metrics.rs::{BridgeMetrics, family, FAMILIES, acl_reason, reason_code_label}`
(die 13 Familien der Liste von §8.2 mit `dds_topic`, `mqtt_qos`,
`direction`, `reason`, `reason_code`; die Reihen eines Topics ab dem Start
mit 0; `<tenant>/<topic>` für die Topics eines Tenants),
`crates/mqtt-bridge/src/daemon/server.rs` (gezählt am Verbindungsaufbau,
an jedem PUBLISH in beide Richtungen, an den Samples von DataReader und
DataWriter, an den Ablehnungen des ACL, am Ende einer Verbindung mit dem
Reason Code des DISCONNECT oder `network`; `inflight_messages` aus den
offenen QoS-1/2-Abläufen jeder Verbindung),
`crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::pending_releases, MqttClient::ended_by}`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::serve_admin_endpoints`
(`metrics.path`).

**Tests:** `crates/mqtt-bridge/src/daemon/metrics.rs::tests::{mqttbridge_8_2_every_family_of_the_spec_is_rendered_with_its_labels, mqttbridge_8_2_register_is_idempotent, mqttbridge_8_2_a_topic_is_counted_under_its_name_and_its_tenant, mqttbridge_8_2_a_disconnect_counts_its_reason_code, mqttbridge_8_2_inflight_follows_the_counts_of_each_connection, mqttbridge_8_2_an_acl_denial_counts_its_reason}`
(die Liste der Familien wird aus dem Codeblock von §8.2 in
`docs/specs/zerodds-mqtt-bridge-1.0.md` gelesen);
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::{mqttbridge_3_the_metrics_are_served_on_metrics_path, mqttbridge_5_2_admin_endpoint_serves_paths}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_2_every_flag_reaches_the_connect_and_the_subscribe, mqttbridge_7_3_the_rules_of_the_spec_let_alice_publish_chat_and_nothing_else, mqttbridge_6_keep_last_on_an_out_topic_publishes_the_newest_n}`
(das Binary mit `--metrics` liefert
`zerodds_mqtt_bridge_dds_samples_published_total{dds_topic="Flag::T"} 1`
über HTTP; die Ablehnungen des ACL; die verworfenen Samples);
die Zählung im Daemon: `crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_8_2_a_refused_and_an_accepted_connect_are_two_attempts_and_one_success, mqttbridge_8_2_each_publish_counts_by_topic_qos_and_octets, mqttbridge_8_2_inflight_counts_the_flows_that_have_not_ended, mqttbridge_8_2_a_lost_connection_counts_under_its_reason_code}`
(ein Broker, der das erste CONNECT mit 0x88 ablehnt: 2 Versuche, 1 Erfolg;
ein PUBLISH je Richtung unter Topic, QoS und Payload-Länge; `out` 1, solange
der Broker das PUBACK zurückhält, dann 0; `in` 1 nach einem QoS-2-PUBLISH
des Brokers bis zum PUBREL; DISCONNECT 0x8E als `0x8e`, ein geschlossener
Socket als `network`).

**Status:** done — grün auf dem Linux-Testrechner. Vorher gab es keine der
Familien von §8.2 unter ihrem Namen: der Daemon führte
`zerodds_mqtt_frames_in_total` und neun weitere Zähler mit dem Präfix
`zerodds_mqtt_` ohne Labels; `connect_successes`, `inflight_messages`,
`acl_denials` und `broker_disconnects` fehlten. Die Spec nennt jetzt auch
`dds_samples_dropped_total{dds_topic}` und `errors_total`, die §3, §4.1 und
§6 zählen lassen. Außerdem prüften nur
Unit-Tests der Struktur `BridgeMetrics` die Familien `connect_attempts`,
`connect_successes`, `publish_in`, `publish_out`, `bytes_in`, `bytes_out`,
`inflight_messages` und `broker_disconnects`; jetzt lässt das Entfernen
oder Verfälschen jeder dieser Zählstellen in `crates/mqtt-bridge/src/daemon/server.rs` einen der vier
Daemon-Tests scheitern (8 Mutationen auf dem Linux-Testrechner, alle
gefangen).

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert Span-Emission;
ohne die Variable entsteht kein Span.

**Repo:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::{otlp_config_from_env, otlp_config_from_endpoint, spawn_otlp_flush_loop}`,
`crates/mqtt-bridge/src/daemon/server.rs::start_with_reload` (der Exporter
entsteht vor den Kernen und nur mit der Variable, die Log-Zeile
`otlp_active`; vorher entstand er nach den Kernen, und kein Code-Pfad rief
`add_span` auf: der Flush-Thread schickte leere Puffer),
`crates/mqtt-bridge/src/daemon/trace.rs::Tracer` (ohne Exporter aus),
`crates/mqtt-bridge/src/daemon/server.rs::Session::replay_to` (ein PUBLISH
des Replay-Puffers ist ein Span seines DDS-Topics ab dem Replay; vorher
bekam es keinen); die Spec §8.3 mit der Tabelle der Spans und den
PUBLISH-Paketen ohne Span (Katalog, Berichte, Wiederholungen einer
fortgesetzten Sitzung), `man/man1/zerodds-mqtt-bridged.1`,
`man/de/man1/zerodds-mqtt-bridged.1`.

**Tests:** `crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::mqttbridge_8_3_otlp_config_parses_endpoint`;
`crates/mqtt-bridge/src/daemon/trace.rs::tests::mqttbridge_8_3_a_tracer_without_exporter_is_off`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_3_the_binary_sends_a_span_per_publish_roundtrip_only_with_the_variable`
(das Binary ohne die Variable: keine Zeile `otlp_active`, der Collector des
Tests bekommt nichts; mit ihr: `otlp_active` und die Spans von §8.3a);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_3_a_replayed_publish_is_a_span_of_its_topic`
(der Broker startet ohne seine retained Nachrichten neu; der Collector
bekommt ein zweites `mqtt.publish` der Art client des transienten Topics
mit DDS-Topic, Packet Identifier und Status ok, begonnen nach dem Start des
neuen Brokers; ohne den Aufruf in `replay_to` scheitert der Test, gemessen
mit einer Mutation).

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.
## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config-Parse → Signal-Handler → TLS → DCPS →
Reader/Writer → MQTT-Connect mit Reconnect-Backoff → SUBSCRIBE.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::{start_with_reload, start_core, Session::connect, Session::after_connack}`
(Signal-Watcher vor dem Verbindungsaufbau, TLS und Auth-Prüfung, dann je
Kern Teilnehmer, Endpunkte, Verbindung, SUBSCRIBE),
`crates/mqtt-bridge/src/daemon/client.rs::connect_with_backoff` (die
Wartezeiten von `mqtt.reconnect` bis `mqtt.connect_timeout_secs`),
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`.

**Tests:** `crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_9_1_the_order_is_participant_endpoints_connect_subscribe, mqttbridge_9_1_the_daemon_connects_once_the_broker_comes_up, mqttbridge_9_1_sigterm_during_the_connect_retries_exits_0}`
(solange der Broker den CONNACK zurückhält, hat ein Teilnehmer des Tests
Writer und Reader der Bridge entdeckt, vor dem CONNACK geht nichts hinaus,
das erste Paket danach ist das SUBSCRIBE; ein Broker, der eine Sekunde nach
dem Binary startet, wird erreicht; SIGTERM während der Versuche ist Exit 0);
`crates/mqtt-bridge/src/daemon/client.rs::tests::{mqttbridge_9_1_an_unreachable_broker_is_retried_until_the_connect_timeout, mqttbridge_9_1_a_broker_that_comes_up_before_the_connect_timeout_is_reached, mqttbridge_9_1_a_tls_handshake_without_an_answer_is_tried_again, mqttbridge_9_1_a_connect_timeout_beyond_the_clock_retries_without_end}`;
`crates/mqtt-bridge/tests/daemon_e2e.rs::mqttbridge_2_daemon_connects_and_subscribes`.

**Status:** done — grün auf dem Linux-Testrechner. Der Verbindungsaufbau
mit Backoff und der Signal-Watcher vor ihm kamen aus einem früheren
Arbeitspaket (Spec §9.1); der zitierte Test prüfte nur CONNECT und
SUBSCRIBE, weder die Reihenfolge noch die Wiederholung. Ein
`mqtt.connect_timeout_secs`, der zur aktuellen Zeit addiert über die Uhr
hinausgeht (die Konfiguration nimmt jeden u64-Wert, etwa
18446744073709551615), versucht ohne Ende wie 0; vorher endete der Start
mit einer Panik in `connect_with_backoff` (gemessen).

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, DISCONNECT 0x00, Cleanup;
SIGHUP TLS+ACL-Reload.

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::DaemonHandle::shutdown`
(Pumps stoppen, die Acks des Brokers bis `mqtt.drain_timeout_secs`
abwarten, ein Wert jenseits der Uhr ohne Ende, DISCONNECT 0x00), `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`
(wartet auf das Stop-Flag; der Prozess lief vorher 3 s nach SIGTERM weiter,
gemessen), `::start_with_reload`, `::reload` (ACL; entzogene Rechte gelten
sofort; die TLS-Einstellungen ab der nächsten Verbindung, §7.1; die
Zugangsdaten von §7.2 bei gleichem Modus und gleicher Methode, mit
Neu-Authentifizierung nur bei geänderten Zugangsdaten; ein anderer
`auth.mode` oder eine andere `auth.enhanced.method` steht in der
Neustart-Liste).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_9_2_sigterm_disconnects_and_exits_0`,
`::mqttbridge_9_2_the_shutdown_waits_for_the_acks_then_disconnects` (Mosquitto 2.0.21,
Linux-Testhost); `crates/mqtt-bridge/tests/daemon_e2e.rs::mqttbridge_9_2_a_reload_that_withdraws_the_read_right_stops_the_samples`;
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_9_2_backoff_connect_aborts_when_stop_set`;
der TLS-Reload: `crates/mqtt-bridge/tests/security_e2e.rs::mqttbridge_7_1_a_sighup_puts_the_new_client_certificate_into_the_next_connection`;
der Reload der Zugangsdaten: `crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_7_2_a_reload_gives_the_next_connect_the_new_password`,
`crates/mqtt-bridge/tests/enhanced_auth.rs::mqttbridge_7_2_a_reload_that_keeps_the_credentials_does_not_re_authenticate`,
`crates/mqtt-bridge/src/daemon/server.rs::tests::{mqttbridge_7_2_a_reload_takes_new_credentials_of_the_same_way_only, mqttbridge_9_2_a_reload_names_the_settings_that_need_a_restart}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_2_a_drain_timeout_beyond_the_clock_waits_for_the_acks`
(`mqtt.drain_timeout_secs` 18446744073709551615: das Herunterfahren endet
nach dem PUBACK, das der Broker des Tests 1,5 s zurückhält).

**Status:** done — grün auf dem Linux-Testrechner. Ein
`mqtt.drain_timeout_secs` jenseits der Uhr ließ `DaemonHandle::shutdown`
vorher mit einer Panik enden (gemessen mit einer Mutation); jetzt wartet
es ohne Ende auf die Acks.

### §9.3 Reconnect

**Spec:** §9.3 — nach einem Verbindungsverlust (DISCONNECT des Brokers,
geschlossener oder fehlerhafter Socket, fehlendes PINGRESP) verbindet sich
der Daemon ohne Ende neu: der erste Versuch sofort, jeder weitere nach den
Wartezeiten von `mqtt.reconnect` (`initial_delay_ms`, dann `factor` mal die
letzte, höchstens `max_delay_ms`), mit den TLS-Einstellungen und
Zugangsdaten, die gelten; eine Ablehnung des TLS-Handshakes oder der
Zugangsdaten wird nach der nächsten Wartezeit wiederholt; nach DISCONNECT
0x8E Session taken over erst nach `max_delay_ms`. Währenddessen warten die
Samples nach der History ihres Topics, `/healthz` antwortet 503. Nach dem
CONNACK: SUBSCRIBE, Katalog, Replay-Puffer; bei `clean_start=false` die
Sitzung (Subscriptions, Inflight-Queue).

**Repo:** `crates/mqtt-bridge/src/daemon/server.rs::{reconnect, Session::attempt, Session::after_connack, Health}`
(eine Folge von Wartezeiten für alle Fehler: nicht erreichbarer Broker,
Ablehnung von TLS oder Zugangsdaten, gescheiterte Subscription; vorher lief
nach einer Ablehnung eine zweite Backoff-Folge in `connect_with_backoff`,
die bei jedem Versuch neu bei `initial_delay_ms` begann; 0x8E wartet
`max_delay_ms`), `crates/mqtt-bridge/src/daemon/pump.rs::Pump`
(`connected` falsch: die Samples warten in der History),
`crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::keep_alive_tick, MqttClient::release, MqttClient::ended_by, MqttClient::wait_for_a_send_slot}`
(das Ende der Verbindung beendet das Warten auf einen Sendeplatz mit
`ClientError::Io`),
`crates/mqtt-bridge/src/daemon/session.rs` (die Sitzung bei
`clean_start=false`, auch über einen Neustart in `mqtt.session_dir`);
die Spec §9.3, `man/man1/zerodds-mqtt-bridged.1`,
`man/de/man1/zerodds-mqtt-bridged.1`. Der Abschnitt hatte vorher keinen
Eintrag in diesem Dokument.

**Tests:** `crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_after_a_broker_restart_the_daemon_subscribes_and_publishes_its_catalog_again`
(der Broker geht weg und kommt auf seinem Port ohne Subscriptions und
retained Nachrichten wieder: `/healthz` 503, dann 200; eine Nachricht auf
dem Topic erreicht DDS wieder, der neue Broker hält den Katalog);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_the_waits_between_the_attempts_grow_to_the_maximum`
(das Binary: die erste Zeile `connect_failed` weniger als 200 ms nach der
Zeile `disconnect` bei `initial_delay_ms` 400, also ohne Wartezeit vor dem
ersten Versuch; `connect_state` reconnecting und `next_attempt_ms` 400,
800, 1600, 1600, 1600 bei 400/2/1600, die Versuche mindestens so weit
auseinander; der zurückgekehrte Broker wird erreicht; wartet der erste
Versuch `initial_delay_ms`, scheitert der Test, gemessen mit einer
Mutation);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_a_refused_reconnect_is_tried_again_after_the_next_wait`
(zwei CONNACK 0x86, die beim Start Exit 5 wären, im Abstand von 200 und
400 ms wiederholt, der dritte CONNECT wird angenommen);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_a_missing_pingresp_ends_the_connection_and_the_daemon_connects_again`
(Keep Alive 1 s, kein PINGRESP: neue Verbindung nach mindestens 1,4 s,
gezählt unter `network`; die zweite Verbindung bekommt PINGRESP und
bleibt);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_a_session_taken_over_is_tried_again_after_the_longest_wait`
(nach 0x8E mindestens 1,4 s bei `max_delay_ms` 1500, nach 0x89 unter 1 s);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_9_3_after_a_lost_connection_the_session_resends_what_the_broker_did_not_acknowledge`
(der Daemon im Testprozess mit `clean_start=false` und Session Expiry
Interval 300 s; der Broker des Tests hält das PUBACK eines QoS-1-PUBLISH
des Daemons zurück und schließt den Socket ohne DISCONNECT; der Daemon
verbindet sich ohne Neustart mit Clean Start 0 neu, und nach Session
Present 1 geht das PUBLISH mit DUP 1, demselben Packet Identifier, Topic
und Payload erneut hinaus; das PUBACK darauf schließt den Fluss in der
Sitzungsdatei ab);
`crates/mqtt-bridge/tests/bridge_e2e.rs::{mqttbridge_6_transient_samples_are_replayed_after_the_broker_restarts, mqttbridge_6_the_samples_of_an_outage_arrive_once_and_in_order, mqttbridge_7_2_a_reload_gives_the_next_connect_the_new_password, mqttbridge_5_2_a_new_assigned_identifier_clears_the_catalog_of_the_one_before, mqttbridge_8_2_a_lost_connection_counts_under_its_reason_code, mqttbridge_4_1_a_restarted_daemon_resends_what_the_broker_did_not_acknowledge}`;
`crates/mqtt-bridge/tests/security_e2e.rs::{mqttbridge_7_1_a_sighup_puts_the_new_client_certificate_into_the_next_connection, mqttbridge_7_1_a_sighup_reads_the_ca_file_again}`
(abgelehnte TLS-Handshakes werden wiederholt, bis die neue CA gilt);
`crates/mqtt-bridge/tests/mosquitto_qos.rs::mqttbridge_4_1_the_session_survives_a_reconnect_with_clean_start_false`
(Mosquitto 2.0.21, `#[ignore]`, Job `integration-bridges-vendor`);
`crates/mqtt-bridge/src/daemon/config.rs::tests::mqttbridge_3_the_reconnect_delays_grow_by_the_factor_up_to_the_maximum`;
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_9_3_the_end_of_the_connection_ends_the_wait_for_a_send_slot`
(Receive Maximum 1, der Broker schließt, während ein QoS-1-PUBLISH auf
einen Sendeplatz wartet: zwei PUBLISH enden in weniger als 5 s mit einem
I/O-Fehler, ein Ereignis des Endes bleibt für die Empfangsschleife);
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_9_3_a_malformed_ack_ends_the_wait_for_a_send_slot_and_the_connection`
(ein PUBACK, das sich nicht dekodieren lässt, während ein QoS-1-PUBLISH auf
einen Sendeplatz wartet: DISCONNECT 0x81 und das Schließen, das wartende
PUBLISH endet mit einem I/O-Fehler und geht nicht hinaus, ein Ereignis des
Endes bleibt für die Empfangsschleife).

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Ein
QoS-1/2-PUBLISH, das auf einen Sendeplatz wartete, bemerkte das Ende der
Verbindung vorher nicht: es las 10 s lang den geschlossenen Socket, hielt
je Durchlauf ein Ereignis `Disconnected` (8066134 in einer Messung des
Reviews) und endete mit `ClientError::Busy`, das die Pumpe ohne Pause
wiederholte. Jetzt endet es sofort mit `ClientError::Io`; die Pumpe legt
das Sample zurück, und die Empfangsschleife verbindet sich neu. Ein Paket
des Brokers, das sich beim Warten nicht dekodieren ließ, endete vorher als
`ClientError::Codec`: die Pumpe verwarf das Sample, und die Verbindung
blieb offen. Jetzt beendet es die Verbindung mit DISCONNECT 0x81 wie in der
Empfangsschleife; jeder andere Fehler beim Warten schließt die Verbindung,
wie die Empfangsschleife es tut (dieser Zweig hat keinen eigenen Test: ein
Fehler der Sitzungsdatei oder des Schreibens während des Wartens ließ sich
im Test nicht erzeugen).

### §8.3a PUBLISH-Roundtrip-Span

**Spec:** §8.3 — bei gesetztem `OTEL_EXPORTER_OTLP_ENDPOINT` entsteht pro
PUBLISH-Roundtrip ein Trace mit der Wurzel `mqtt.publish`; `dds.write` ist
Kind eines PUBLISH des Brokers. Die frühere Fassung der Spec nannte
„`mqtt.publish` + `dds.write` als Kind“ für jeden Roundtrip; ein PUBLISH
des Daemons hat kein `dds.write`, weil der Daemon auf dem Weg nach MQTT
nichts in DDS schreibt (das Sample schrieb ein DataWriter eines anderen
Teilnehmers). Die Spec §8.3 sagt das jetzt ausdrücklich.

**Repo:** `crates/mqtt-bridge/src/daemon/trace.rs` (`Tracer`,
`PublishOut`, `PublishIn`, `DdsWrite`, `publish_out_span`,
`publish_in_spans`), `crates/mqtt-bridge/src/daemon/client.rs::{MqttClient::trace_publish, MqttClient::set_tracer}`
(der Span eines PUBLISH des Daemons endet mit dem Socket-Write bei QoS 0,
mit PUBACK oder PUBCOMP bei QoS 1 und 2; eine Ablehnung des Brokers und
eine vor der Bestätigung beendete Verbindung sind Status error),
`crates/mqtt-bridge/src/daemon/pump.rs::Pump` (der Start ist die Entnahme
des Samples aus dem DataReader),
`crates/mqtt-bridge/src/daemon/server.rs::Session::replay_to` (ein PUBLISH
des Replay-Puffers nach einem CONNACK: der Start ist der Replay),
`crates/mqtt-bridge/src/daemon/server.rs::start_core` (ein PUBLISH des
Brokers: `mqtt.publish` vom Lesen bis zum Ende der Behandlung, ein
`dds.write` je DDS-Topic; ohne Route oder vom ACL abgelehnt die Wurzel
allein mit Status error).

**Tests:** `crates/mqtt-bridge/src/daemon/trace.rs::tests::{mqttbridge_8_3_a_publish_of_the_daemon_is_a_root_span_to_its_acknowledgement, mqttbridge_8_3_a_refused_publish_is_status_error, mqttbridge_8_3_a_publish_of_the_broker_has_a_dds_write_child_per_topic, mqttbridge_8_3_a_publish_written_nowhere_is_the_root_alone}`;
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_3_the_binary_sends_a_span_per_publish_roundtrip_only_with_the_variable`
(das Binary gegen einen Broker des Tests, der das PUBACK 1,5 s zurückhält,
und einen OTLP-Collector des Tests: das QoS-1-PUBLISH des Daemons ist ein
`mqtt.publish` der Art client mit Topic, QoS, Packet Identifier und
DDS-Topic, das mindestens 1,4 s dauert; das PUBLISH des Brokers ist ein
`mqtt.publish` der Art server mit dem Kind `dds.write` in seinem Trace;
endet der Span beim Socket-Write statt beim PUBACK, scheitert der Test,
gemessen mit einer Mutation);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_3_a_replayed_publish_is_a_span_of_its_topic`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.

## §10 Cross-Vendor

### §10 RTPS-Peer + Mosquitto/EMQX/HiveMQ

**Spec:** §10 — Daemon ist normaler RTPS-Peer; MQTT-Seite gegen
Mosquitto/EMQX/HiveMQ getestet.

**Repo:** `crates/mqtt-bridge/src/daemon/client.rs`,
`crates/mqtt-bridge/src/daemon/server.rs::write_inbound` (die
Repräsentation des DataWriters eines Topics),
`crates/mqtt-bridge/src/daemon/qos_translation.rs::{reader_config, writer_config}`
(`data_representation`), `crates/mqtt-bridge/tests/interop/{cyclone_reading.c, fastdds_reading.cpp, reading.idl}`.

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_12_3_mosquitto_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_10_fastdds_carries_mqtt_to_dds_and_dds_to_mqtt, mqttbridge_10_connext_carries_mqtt_to_dds_and_dds_to_mqtt}`
(der Daemon zwischen den Clients von Mosquitto und einem Teilnehmer von
Cyclone DDS 0.10.5 unter XCDR1 und XCDR2, einem Teilnehmer von Fast DDS 3.1
und `shape_main` von RTI Connext DDS 7.7.0 unter XCDR2, jeweils über RTPS
in beide Richtungen: sobald der Writer des Daemons den fremden Reader und
dieser den Writer gefunden hat (der Teilnehmer meldet seinen Match), erreicht
ein einzelnes PUBLISH von `mosquitto_pub` den fremden Reader als das Sample;
ein Sample des fremden Writers erreicht
`mosquitto_sub` als Encapsulation-Header und CDR-Bytes, und diese Oktette,
erneut publiziert, erreichen den fremden Reader als das geschriebene
Sample; alle `#[ignore]`, Job `integration-bridges-vendor`);
`crates/mqtt-bridge/tests/daemon_e2e.rs::{mqttbridge_3_data_representation_xcdr2_matches_an_xcdr2_reader_before_the_first_publish, mqttbridge_3_data_representation_xcdr2_reader_matches_no_xcdr1_writer}`;
die MQTT-Seite gegen EMQX, HiveMQ und Aedes in §10a; weitere Tests gegen
Mosquitto `crates/mqtt-bridge/tests/mosquitto_qos.rs::{mqttbridge_6_inbound_qos_1_and_2_are_acknowledged_and_all_messages_arrive, mqttbridge_6_outbound_qos_1_and_2_complete_and_leave_nothing_pending}`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::{mqtt5_4_zerodds_311_client_receives_from_mosquitto, mqtt5_4_mosquitto_311_client_receives_from_zerodds}`;
der ZeroDDS-Teilnehmer über RTPS mit dem Binary und Mosquitto
`tests/integration/tests/bridges.rs::mqtt_bridge_against_mosquitto`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Vorher
zitierte der Eintrag für die RTPS-Seite einen Test, der das Sample in den
Reader des Daemons einspeist und nichts über RTPS sendet. Die Tests mit
fremden Teilnehmern zeigten, dass die XCDR2-Reader von Cyclone DDS, Fast
DDS und RTI Connext nicht zum DataWriter des Daemons passten, der bis zum
ersten PUBLISH XCDR1 anbot (gemessen); der Topic-Schlüssel
`data_representation` (§3) legt die Repräsentation ab der Erzeugung fest.
Vorher wiederholte der Test das PUBLISH bis zu 15 s lang jede Sekunde, bis
ein Sample ankam; ein verworfenes PUBLISH blieb so unbemerkt. Jetzt geht ein
PUBLISH hinaus, wenn beide Seiten gematcht sind, und muss ankommen: eine
Mutation, die den ersten DDS-Write des Daemons verwirft, lässt die drei Tests
scheitern (gemessen).

### §10a Broker-Matrix EMQX/HiveMQ

**Spec:** §10 — die MQTT-Seite läuft gegen EMQX und HiveMQ.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs` (`mqtt.own_topic_prefix`),
`crates/mqtt-bridge/src/daemon/runtime_common.rs::catalog_topic`,
`crates/mqtt-bridge/src/daemon/client.rs::liveliness_topic`,
`crates/mqtt-bridge/src/daemon/status_watch.rs::StatusWatch`,
`ci/integration-weekly.yml` (die Services `emqx-matrix`, `hivemq`, `aedes`
des Jobs `integration-bridges-vendor`).

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_10_emqx_carries_qos_0_1_2_retained_messages_and_the_properties, mqttbridge_10_hivemq_carries_qos_0_1_2_retained_messages_and_the_properties, mqttbridge_10_aedes_carries_qos_0_1_2_and_retained_messages, mqttbridge_10_mosquitto_carries_qos_0_1_2_retained_messages_and_the_properties}`
(die Samples eines ZeroDDS-Teilnehmers erreichen `mosquitto_sub` mit der
QoS ihres Topics, 0, 1 und 2, unter MQTT 5 mit Content Type, Payload Format
Indicator 0, den User Properties von §4.2 und dem Message Expiry Interval
der Lifespan des Writers: 28 bis 30 bei 30 s Lifespan als die Restzeit, die
der Broker weitergibt, keiner bei unendlicher Lifespan; die Nachrichten von
`mosquitto_pub` mit QoS 0, 1 und 2 erreichen DDS je einmal, unter MQTT 5 mit
dem Key-Hash und dem Quellzeitstempel ihrer User Properties; eine
gehaltene Nachricht des Brokers erreicht DDS beim Abonnieren; das Sample
eines TRANSIENT_LOCAL-Topics bleibt auf dem Broker gehalten für einen
späteren Abonnenten),
`crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_9_3_emqx_keeps_the_session_and_publishes_the_will, mqttbridge_9_3_hivemq_keeps_the_session_and_publishes_the_will, mqttbridge_9_3_aedes_keeps_the_session_and_publishes_the_will, mqttbridge_9_3_mosquitto_keeps_the_session_and_publishes_the_will}`
(die Verbindung des Daemons mit `clean_start: false` endet ohne DISCONNECT
über eine Weiterleitung des Tests; der Broker publiziert den Will und hält
die QoS-1-Nachrichten des Ausfalls, die nach dem Wiederverbinden DDS je
einmal und in Reihenfolge erreichen),
`crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_5_2_emqx_keeps_the_retained_catalog, mqttbridge_5_2_aedes_keeps_the_retained_catalog, mqttbridge_5_2_mosquitto_keeps_the_retained_catalog, mqttbridge_5_2_hivemq_refuses_the_dollar_prefix_and_keeps_the_catalog_under_another}`,
`crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_12_3_emqx_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_hivemq_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_aedes_carries_mqtt_to_cyclone_and_cyclone_to_mqtt}`
(alle `#[ignore]`, Job `integration-bridges-vendor`; EMQX 5.8.6, HiveMQ
4.57.0 und Aedes 1.0.2 als Container);
`crates/mqtt-bridge/src/daemon/client.rs::tests::mqttbridge_6_the_connect_carries_the_liveliness_will`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::mqttbridge_5_2_the_catalog_is_the_json_of_the_spec`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner, die
Broker als Container. Gemessen: HiveMQ 4.57.0 beantwortet ein CONNECT,
dessen Will-Topic mit `$` beginnt, mit CONNACK 0x82 und ein PUBLISH auf
einem solchen Topic mit DISCONNECT 0x82 (MQTT 5.0 §4.7.2 überlässt diese
Topics dem Server); der Daemon konnte sich daher nicht mit HiveMQ verbinden.
`mqtt.own_topic_prefix` (§3) verlegt Katalog, Berichte und Will. Aedes 1.0.2
lehnt ein CONNECT mit Protokollstufe 5 ab; der Daemon läuft gegen Aedes
unter `mqtt.protocol_version: 3.1.1` (§1 L1), ohne die Properties von §4.2.

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
`man/man5/zerodds-mqtt-bridged.yaml.5`;
der Windows-Dienst: `crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`
(`main` startet den Daemon über
`zerodds_dcps::cli::run_as_service("ZeroDDSMQTTBridge", …)`),
`crates/dcps/src/cli.rs` (`run_as_service`, `request_service_stop`,
`request_service_reload`, `READY=1` und `STOPPING=1` von
`notify_service_manager` als Dienstzustände),
`crates/dcps/src/cli/windows_service.rs`,
`crates/mqtt-bridge/src/daemon/runtime_common.rs`
(`install_signal_watcher` außerhalb von Unix: `watch_service_requests`
setzt die Flags von SIGTERM und SIGHUP); das Log des Dienstes:
`crates/dcps/src/cli.rs::running_as_service` (gesetzt von
`crates/dcps/src/cli/windows_service.rs::service_main` vor dem Daemon),
`crates/mqtt-bridge/src/daemon/log.rs::{to_file, service_log_path}` und
`daemon_main` des Binarys (als Dienst hängt der Daemon sein Log an
`%PROGRAMDATA%\ZeroDDS\logs\mqtt-bridged.log` an, die Datei von
zerodds-deployment-1.0 §4.1; eine Datei, die sich nicht öffnen lässt,
beendet den Start mit Exit-Code 1; vorher ging das Log des Dienstes auf
ein stdout, das es nicht gibt); die Spec §8.1 und §11 und
`man/man1/zerodds-mqtt-bridged.1` (WINDOWS SERVICE) nennen die
Dienstzustände, Controls und die Log-Datei. Vorher hatte das Binary keinen
Service-Control-Dispatcher, und der Watcher außerhalb von Unix war ein
leerer Thread.

**Tests:** `crates/mqtt-bridge/tests/example_yaml_loadback.rs::{mqttbridge_3_example_yaml_parses_without_error, mqttbridge_3_example_yaml_key_fields_match_intent, mqttbridge_11_example_yaml_topics_reach_the_daemon, mqttbridge_7_3_example_yaml_acl_reaches_the_daemon, mqttbridge_11_example_yaml_has_no_unknown_top_level_keys}`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_11_the_service_units_start_the_binary_with_flags_it_knows`
(die Argumente der systemd-Unit, der launchd-plist, des Homebrew-Dienstes,
des Windows-Skripts und des Docker-Images, mit der Beispiel-Konfiguration,
bestehen `--check-config` mit Exit 0; vom Windows-Skript alle Wörter von
`$binPath` nach dem Programm, vorher nur das erste `--`-Wort; das Image
trägt die Beispiel-Konfiguration);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_11_the_binary_tells_the_service_manager_ready_and_stopping`
(die Unit ist `Type=notify`; das Binary schickt `READY=1` an
`$NOTIFY_SOCKET`, wenn die Broker-Verbindung steht, und `STOPPING=1` nach
SIGTERM);
`crates/mqtt-bridge/src/daemon/runtime_common.rs::tests::mqttbridge_11_the_service_requests_set_the_shutdown_and_reload_flags`
(der Watcher von Windows in einem Thread unter Linux: ohne Anfrage wartet
er, er endet mit dem Daemon, eine Reload-Anfrage setzt das Reload-Flag,
eine Stopp-Anfrage das Shutdown-Flag);
`crates/mqtt-bridge/src/daemon/log.rs::tests::{mqttbridge_11_the_lines_go_to_the_log_file_of_the_service, mqttbridge_11_the_log_file_of_the_service_is_under_programdata}`
(nach `to_file` gehen die Zeilen als JSON in die Datei, ihr Verzeichnis
wird angelegt, ein zweites `to_file` hängt an; der Pfad unter
`%PROGRAMDATA%`, ohne die Variable unter `C:\ProgramData`);
`crates/dcps/src/cli.rs::service_tests::{deployment_4_3_the_service_controls_are_stop_reload_and_interrogate, deployment_4_3_the_notify_states_are_service_states, deployment_4_3_the_exit_code_has_its_number, deployment_4_3_the_requests_and_the_run}`
(ein Lauf aus der Konsole ist kein Dienst: `running_as_service` falsch);
`crates/dcps/src/cli/windows_service.rs::tests::deployment_4_3_the_control_codes_are_the_win32_constants`
(nur Windows);
`tools/traceability/tests/deployment_layout.rs::{every_systemd_unit_carries_the_hardening_set_of_the_clause, every_launchd_plist_carries_the_keys_of_the_clause, windows_service_scripts_register_and_remove_the_same_bridge_set, every_daemon_image_is_cargo_chef_multi_stage_and_runs_non_root}`.

**Status:** partial — die Tests liefen grün auf dem Linux-Testrechner; der
Windows-Code wurde dort mit `cargo clippy --target x86_64-pc-windows-gnu -p
zerodds-mqtt-bridge --all-targets -- -D warnings` und ebenso für
`zerodds-dcps` (`--lib --tests`) übersetzt, nicht ausgeführt. Es fehlt der Lauf des Dienstes `ZeroDDSMQTTBridge` unter
`sc.exe` (Start, `READY=1` als `SERVICE_RUNNING`, Stopp mit dem Drain,
`paramchange` als Reload, das Log in
`%PROGRAMDATA%\ZeroDDS\logs\mqtt-bridged.log`); er braucht einen
Windows-Rechner mit dem registrierten Dienst, der Linux-Testrechner ist
keiner. Es fehlt ein CI-Job, der den Windows-Code dieses Pakets übersetzt
(`install_signal_watcher` unter `cfg(not(unix))`, der Windows-Weg von
`run_as_service`, `running_as_service`): die Windows-Jobs der CI
(`ci/platform_jobs.py`, `CORE` und `WINDOWS_EXTRA`) laufen nur für die
Kern-Crates und `zerodds-foundation`, `zerodds-qos`,
`zerodds-transport-shm`, und das CI-Image (`ci/Dockerfile.rust`) hat das
Target `x86_64-pc-windows-gnu` nicht; ein solcher Job braucht einen der
beiden Wege und einen Lauf, der ihn grün zeigt. Das Image von
`packaging/docker/mqtt-bridged/Dockerfile` wurde auf dem Linux-Testrechner
einmal von Hand gebaut (Docker 26.1.5, Exit 0) und gestartet:
`--check-config` mit der Konfiguration des Images Exit 0; mit
`--network host` und `--broker` gegen Mosquitto 2.0.21 die Zeilen
`connect`, `subscribed`, `catalog_published`, der Katalog auf
`$zerodds/zerodds-mqtt-bridge/catalog` von `mosquitto_sub` gelesen, nach
`docker stop` `catalog_cleared`, `disconnect` mit 0x00 und Exit 0. Kein Job
außer `release.yml` beim Release baut das Image.

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — je ≥ 5 Tests für die Module Konfiguration, MQTT-Codec,
Topic-Map, QoS-Übersetzung, DDS-Pumpe und ACL.

**Repo:** `crates/mqtt-bridge/src/daemon/config.rs`,
`crates/mqtt-bridge/src/codec.rs`, `crates/mqtt-bridge/src/daemon/topic_map.rs`,
`crates/mqtt-bridge/src/daemon/qos_translation.rs` (von
`crates/mqtt-bridge/src/daemon/server.rs::register_topic` für die
Endpunkte und von `crates/mqtt-bridge/src/daemon/pump.rs::Pump` für die
Lebensdauer benutzt), `crates/mqtt-bridge/src/daemon/pump.rs` (die
Abbildung Sample ↔ PUBLISH und jetzt auch der Pumpen-Thread `Pump`,
`Outgoing` und `EchoGuard`, die vorher in `crates/mqtt-bridge/src/daemon/server.rs` standen),
`crates/mqtt-bridge/src/daemon/acl.rs`; die Spec §12.1 nennt die Module
mit ihren Pfaden im Baum. `crates/mqtt-bridge/src/dds_bridge.rs`
(`TopicMapper`, `MqttDdsBridge`) ist die `no_std`-Abbildung der Bibliothek
und wird vom Daemon nicht benutzt; ihre Tests zählen hier nicht.

**Tests:** je Modul (Anzahl der Tests):
`crates/mqtt-bridge/src/daemon/config.rs::tests::{mqttbridge_12_1_config_loads_minimal, mqttbridge_3_the_defaults_of_the_keys, mqttbridge_5_1_an_out_topic_with_a_wildcard_is_refused, mqttbridge_3_the_reconnect_delays_grow_by_the_factor_up_to_the_maximum, mqttbridge_3_the_new_keys_refuse_what_they_do_not_accept}` (21);
`crates/mqtt-bridge/src/codec.rs::tests::{mqtt5_3_3_2_the_publish_of_figure_3_9, mqtt5_3_3_2_2_a_packet_identifier_only_above_qos_0, mqtt5_3_3_invalid_qos_3_rejected_on_encode, mqtt5_3_3_missing_packet_id_at_qos1_rejected, mqtt5_2_1_4_truncated_remaining_length_decode_fails}` (11);
`crates/mqtt-bridge/src/daemon/topic_map.rs::tests::{mqttbridge_5_1_a_plus_filter_maps_its_topics, mqttbridge_5_1_a_hash_filter_stops_at_the_level_separator, mqttbridge_4_3_one_subscription_and_identifier_per_filter, mqttbridge_4_3_the_identifier_selects_the_topic, mqttbridge_4_3_a_subscription_routes_once_it_is_granted, mqttbridge_5_1_two_topics_of_one_filter_each_get_its_publish_once}` (6);
`crates/mqtt-bridge/src/daemon/qos_translation.rs::tests::{mqttbridge_6_reliability_gives_the_mqtt_qos, mqttbridge_6_durability_gives_retain_and_the_replay, mqttbridge_6_lifespan_is_the_message_expiry_interval, mqttbridge_6_deadline_liveliness_and_partition_reach_the_endpoints, mqttbridge_6_a_duration_in_milliseconds_is_seconds_and_fractions}` (5);
`crates/mqtt-bridge/src/daemon/pump.rs::tests::{mqttbridge_4_2_the_properties_of_a_sample, mqttbridge_4_2_what_a_sample_does_not_have_is_left_out, mqttbridge_4_2_the_source_timestamp_round_trips_through_nanoseconds, mqttbridge_4_2_an_inbound_publish_and_what_is_refused, mqttbridge_4_4_a_dispose_marker_is_op_dispose_with_the_key_hash, mqttbridge_4_4_a_missing_op_is_sample, mqttbridge_4_4_an_unknown_op_is_refused, mqttbridge_4_3_the_echo_record_comes_before_the_publish_and_goes_when_it_fails}` (8);
`crates/mqtt-bridge/src/daemon/acl.rs::tests::{mqttbridge_7_3_a_rule_with_a_filter_covers_its_topics, mqttbridge_7_3_the_star_subject_is_everyone, mqttbridge_7_3_default_deny_closes_the_rest, mqttbridge_7_3_publish_and_subscribe_are_separate_rights, mqttbridge_7_3_a_filter_covers_and_overlaps_as_mqtt_matches, mqttbridge_7_3_the_subject_is_the_certificate_dn_then_the_user_name_then_the_jwt_sub}` (6);
dazu `crates/mqtt-bridge/tests/fuzz_smoke.rs::mqttbridge_12_1_fuzz_decode_publish_no_panic`
und die sechs weiteren Fuzz-Smokes.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Vorher
zitierte der Eintrag Tests von Bibliotheksmodulen und von
`dds_bridge.rs`, das der Daemon nicht benutzt; Topic-Map und
QoS-Übersetzung hatten je 4 Tests, die Pumpe stand in `crates/mqtt-bridge/src/daemon/server.rs`.

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — das Binary als Prozess, Mosquitto, Roundtrip MQTT↔DDS
Byte für Byte.

**Repo:** `tests/integration/tests/bridges.rs`,
`crates/mqtt-bridge/src/bin/zerodds-mqtt-bridged.rs`; die Spec §12.2 nennt
`tests/integration/tests/bridges.rs` und Mosquitto als lokalen Prozess
statt testcontainers.

**Tests:** `tests/integration/tests/bridges.rs::mqtt_bridge_against_mosquitto`
(Job `integration-bridges`: das Binary, Mosquitto als Prozess,
`mosquitto_sub` und `mosquitto_pub`, ein DDS-Teilnehmer; beide Richtungen,
die Payload Oktett für Oktett verglichen);
`crates/mqtt-bridge/tests/daemon_e2e.rs::{mqttbridge_2_daemon_connects_and_subscribes, mqttbridge_12_2_an_mqtt_publish_is_written_to_dds, mqttbridge_2_dds_publish_pumps_to_mqtt_broker}`
(der Daemon im Testprozess: ein DDS-Reader des Tests bekommt die Bytes
eines PUBLISH nach dem Encapsulation-Header);
`crates/mqtt-bridge/tests/bridge_e2e.rs::mqttbridge_8_1_the_binary_logs_a_publish_with_the_fields_of_the_spec`
(das Binary gegen den Broker-Server des Crates, beide Richtungen über RTPS).

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Vorher
zitierte der Eintrag nur Tests mit dem Daemon im Testprozess; der Test der
Richtung MQTT → DDS schickte eine Payload ohne Encapsulation-Header, die
der Daemon ablehnt, und prüfte nur, dass nichts abstürzt.

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — ein Teilnehmer von Cyclone DDS über die Bridge,
Broker-Matrix Mosquitto/EMQX/HiveMQ/Aedes.

**Repo:** `crates/mqtt-bridge/tests/cross_vendor.rs`,
`crates/mqtt-bridge/tests/interop/{cyclone_reading.c, reading.idl}`,
`ci/integration_scenarios.py` (Job `integration-bridges-vendor`, Ziel
`cross_vendor`), `ci/integration-weekly.yml` (die Broker als Services).

**Tests:** `crates/mqtt-bridge/tests/cross_vendor.rs::{mqttbridge_12_3_mosquitto_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_emqx_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_hivemq_carries_mqtt_to_cyclone_and_cyclone_to_mqtt, mqttbridge_12_3_aedes_carries_mqtt_to_cyclone_and_cyclone_to_mqtt}`
(ein Teilnehmer von Cyclone DDS 0.10.5, vom Test mit `idlc` und dem
C-Compiler gebaut, über den Daemon auf jedem Broker der Matrix: ein PUBLISH
von `mosquitto_pub` unter `CDR_LE` und unter `CDR2_LE` erreicht den
Cyclone-Reader dieser Repräsentation als das Sample; ein Sample des
Cyclone-Writers erreicht `mosquitto_sub` als Header `00 01 00 00` oder
`00 07 00 00` und die CDR-Bytes, und diese Oktette erreichen den
Cyclone-Reader wieder als das geschriebene Sample); die übrige
Broker-Matrix in §10a.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Die
Spezifikation nannte Docker Compose; der Test startet Mosquitto als Prozess,
EMQX, HiveMQ und Aedes sind Services des Jobs (die Spezifikation sagt das
jetzt). Vorher gab es kein `tests/cross_vendor.rs`, und der Eintrag
zitierte für die DDS-Seite einen Test ohne RTPS.
## §13 Cross-References

### §13 Verwandte Library + Standards + Daemons

**Spec:** §13 — Library `crates/mqtt-bridge/`, OASIS-MQTT-5-Standard,
Wire-Format, Deployment, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config, Major=Wire-
Protocol-Changes (MQTT-5.x→6).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Regel für die Vergabe der Versionsnummer eines Release, keine Eigenschaft des Daemons.

---

## Audit-Status

28 done / 1 partial / 0 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-mqtt-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (Job `merge-rust-bridge-security`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (Job `merge-rust-config`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-mqtt-bridge --all-targets` (Job `merge-rust-mqtt-bridge`) — 51 Tests grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 4 Tests grün.
* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (Job `merge-rust-websocket-bridge`) — 1 Test grün.
* `python3 ci/run_integration.py integration-bridges-vendor` (Job `integration-bridges-vendor`) — 11 Tests grün.

70 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-mqtt-bridge-1.0.open.md`.
