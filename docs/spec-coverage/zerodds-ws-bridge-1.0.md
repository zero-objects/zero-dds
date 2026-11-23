# `zerodds-ws-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-ws-bridge-1.0.md`

Implementation:

- `crates/websocket-bridge/` — DDS↔WebSocket-Bridge.

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1, `zerodds-ws-bridge-1.0.md` — sechs Conformance-Levels
(Wire/DDS/Bridging/Config/Auth/Multi-Tenant); L1–L4 Pflicht, L5–L6
optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Zuordnung der Levels; die Anforderungen sind die Items §1a (L6), §1b (L1, RFC 7692) und §2 bis §12.

### §1a L6 Multi-Tenant

**Spec:** §1 L6 — mehrere DomainParticipants je Daemon; eine
Mandantenbindung je WebSocket-Verbindung; §3 `tenants`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs::{TenantConfig, TenantConfig::binds, tenant_acl_topic, read_topics}`,
`crates/websocket-bridge/src/daemon/server.rs::{Core, Cores::of, build_core, serve_connection, settings_that_need_a_restart}`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::BridgeMetrics::for_tenant`.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::wsbridge_1_l6_a_connection_is_bound_to_the_tenant_of_its_subject`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_1_l6_the_tenants`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_3_the_example_of_the_spec_loads`,
`crates/websocket-bridge/src/daemon/server.rs::tests::wsbridge_9_2_a_reload_names_the_changed_settings_that_need_a_restart`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_1_l6_the_log_line_and_the_span_name_the_tenant_of_a_connection`
(das Binary mit Bearer-Subjekt `alice` in einem Mandanten: `connection_open`
mit `tenant: plant-a`, `ws.connection` mit `ws.tenant`);
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_1_the_binary_logs_json_lines_for_a_frame_exchange`
und `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_3_the_spans_of_a_connection_and_its_frames_reach_a_collector`
prüfen `tenant: ""` und das Fehlen von `ws.tenant` ohne Mandant.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Ein
Mandant ist ein DomainParticipant auf seiner Domäne mit eigenen Topics,
Endpunkten, Router und Katalog. Eine Verbindung, deren JWT-Subjekt ein
Mandant nennt, schreibt in dessen Domäne und bekommt nur dessen Samples
(geprüft mit je einem DDS-Teilnehmer des Tests auf beiden Domänen und dem
Topic `Chat` auf beiden); eine Verbindung ohne Mandant nutzt `domain`. Die
ACL eines Mandanten-Topics steht unter `<Mandant>/<Topic>`, die Metriken
zählen unter demselben Label. Ein Top-Level-Topic mit dem Namen
`<Mandant>/<Topic>` eines Mandanten-Topics verhindert den Start, ebenso ein
zweiter Eintrag eines Topics mit eigener `acl` (vorher überschrieb die
spätere ACL die frühere ohne Fehler). Geprüft mit JWT und Bearer; mTLS-DN
und `mtls:`-Fingerprint binden über dasselbe Subjekt (`TenantConfig::binds`,
Unit-Test), sind aber nicht eigens Ende-zu-Ende geprüft.

### §1b L1 permessage-deflate (RFC 7692)

**Spec:** §1 L1 — der Daemon spricht RFC 6455 und RFC 7692
(permessage-deflate optional); §4.1, §4.5: der Daemon stimmt dem ersten
gültigen `permessage-deflate`-Angebot zu, `limits.max_message_size` gilt nach
der Dekompression.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs::serve_connection`
(`permessage_deflate::accept_offers`, `render_accept`, `Receiver`/`Sender`
mit `with_permessage_deflate`), `crates/websocket-bridge/src/permessage_deflate.rs`
(Aushandlung, `Deflater`/`Inflater` über `flate2` mit `zlib-rs`),
`crates/websocket-bridge/src/connection.rs` (RSV1, Kontextübernahme, Grenze
der dekomprimierten Größe). Die Einzelheiten von RFC 7692 führt
`websocket-rfc-6455.md` §9.

**Tests:** `crates/websocket-bridge/tests/daemon_rfc6455.rs::{ws_9_the_daemon_agrees_to_permessage_deflate_and_compresses_both_directions,
ws_9_the_daemon_declines_an_offer_that_is_invalid}`,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::python_websockets_client_talks_compressed_to_the_daemon_and_closes`,
`crates/websocket-bridge/src/connection.rs::tests::deflate::{rfc7692_6_1_the_frames_of_the_rfc_are_received_as_hello,
rfc7692_6_1_rsv1_on_a_control_or_continuation_frame_fails_with_1002,
rfc7692_7_2_1_context_takeover_makes_the_repeated_message_shorter,
rfc7692_8_a_message_that_inflates_over_the_limit_fails_with_1009}`,
`crates/websocket-bridge/src/permessage_deflate.rs::tests::{rfc7692_7_1_the_response_follows_the_offer,
rfc7692_5_1_an_invalid_offer_is_declined_for_the_next_one}`,
`crates/websocket-bridge/src/permessage_deflate.rs::tests::codec::{rfc7692_7_2_1_hello_is_compressed_to_the_octets_of_the_rfc,
rfc7692_7_2_2_no_context_takeover_starts_each_message_with_an_empty_window}`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner, der
Python-Test mit `websockets` 17.4 (Wochenjob `integration-bridges-vendor`).
Der Daemon handelt die Erweiterung immer aus, wenn der Client sie anbietet;
ein Schlüssel zum Abschalten ist in der Spec nicht vorgesehen.

## §2 CLI-Surface

### §2 zerodds-ws-bridged CLI

**Spec:** §2 — Optionen `--config`/`--set`/`--check-config`/`--listen`
(Default `127.0.0.1:8080`)/`--domain`/`--topic <NAME[:WS_PATH]>`/
`--tls-cert`/`--tls-key`/`--auth-token`/`--log-level`/`--metrics`/
`--version`/`--help`; Exit-Codes 0/1/2/3/4; die Kurzformen sind `--set` ihres
Schlüssels und gewinnen über die Dateien.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/websocket-bridge/src/daemon/cli.rs` (`HELP_TEXT`, `VERSION_TEXT`, `CliArgs::sets`),
`crates/websocket-bridge/src/daemon/config.rs` (`read_topic`: `NAME:WS_PATH` über
`zerodds_config::read::split_topic_shorthand`).

**Tests:** Über das Binary:
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_a_config_error_is_exit_1`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_a_port_in_use_is_exit_2`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_a_participant_that_does_not_start_is_exit_3`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_a_certificate_that_does_not_load_is_exit_4`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_check_config_prints_the_values_and_exits`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_help_and_version`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_the_flags_override_the_file_and_the_topic_flag_names_its_path`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_the_binary_exits_0_on_sigterm`.
Im Code:
`crates/websocket-bridge/src/daemon/cli.rs::tests::wsbridge_2_the_help_names_the_options_the_exit_codes_and_the_default`
(Hilfe gegen den Block von §2 der Spezifikation),
`crates/websocket-bridge/src/daemon/cli.rs::tests::wsbridge_2_every_short_form_is_a_set_of_its_key`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_2_a_bare_topic_entry_names_its_path_behind_a_colon`.

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün.

## §3 Config-File-Format

### §3 YAML/JSON/TOML Config-Loader

**Spec:** §3 — Top-Level `listen`/`domain`/`log_level`/`tls`/`auth`/
`topics`/`metrics`/`limits`/`origin`; ENV-Substitution `${VAR}` und
`${VAR:-default}`. `metrics.path` (Default `/metrics`),
`qos.history.kind` (`keep_last`|`keep_all`), `ws_path` als absoluter Pfad,
nicht der eines Meta-Endpoints, je Topic ein eigener. `limits.max_frame_size` (mindestens 125) und
`limits.max_message_size` (mindestens 1) in Oktetten, Default je 16 MiB;
`origin.allowed` als Liste von Origins `scheme://host[:port]`, `null` oder
`*`, `origin.required` nur mit Liste; ein Wert außerhalb davon verhindert
den Start, auch ein Eintrag mit `*` in einem Host (`https://*.example.org`).

**Repo:** `crates/websocket-bridge/src/daemon/config.rs`,
`crates/websocket-bridge/src/daemon/mod.rs` (DaemonConfig).

**Tests:** `crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_3_the_example_of_the_spec_loads`
(liest den YAML-Block aus §3 der Spezifikation, prüft seine Werte und dass er
jeden Schlüssel von `KEYS` nennt),
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_3_the_history_kind_and_the_metrics_path_are_read`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_4_2_a_ws_path_that_cannot_be_served_is_refused`;
das ausgelieferte Beispiel
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_3_example_yaml_parses_without_error`,
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_3_example_yaml_key_fields_match_intent`;
Man-Page gegen Beispiel `tools/traceability/tests/bridge_config_manpages.rs`. Die Schlüssel
`limits.*` und `origin.*`:
`crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_4_the_limits_are_read_and_default_to_16_mib`,
`crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_4_a_limit_that_cannot_be_served_is_refused_with_its_line`,
`crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_2_the_origin_keys_are_read`,
`crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_2_an_origin_setting_that_cannot_be_served_is_refused_with_its_line`,
`crates/websocket-bridge/src/origin.rs::tests::ws_10_2_a_list_entry_with_a_pattern_in_its_host_is_refused`. Über `--set`:
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs::tests::ws_10_the_limits_and_the_origins_are_set_with_set`.

**Status:** done

## §4 WebSocket-Wire-Protocol

### §4.1 RFC 6455 Handshake + zerodds-ws-bridge-1.0 Subprotocol

**Spec:** §4.1 — RFC-6455-Upgrade; Subprotocol-Header
`Sec-WebSocket-Protocol: zerodds-ws-bridge-1.0`, jeder Name ein Token
(RFC 6455 §4.1, `token` nach RFC 7230 §3.2.6); Auth-Header per
Bearer- und JWT-Mode; 401 mit `WWW-Authenticate: Bearer`, wenn die
Credentials niemanden authentifizieren, 403 mit `insufficient_scope`, wenn ein
geprüfter JWT den Claim aus `auth.jwt.required_claim` nicht trägt (RFC 6750
§3, §3.1). Ein ungültiger Handshake bekommt
seinen HTTP-Status (405 mit `Allow: GET`, 426 mit `Sec-WebSocket-Version: 13`,
sonst 400, auch für einen Request-Head über 65536 Oktette). Ein Request-Head,
der nicht binnen 10 s nach dem TCP-Connect vollständig ist, bekommt 408. Mit einer Liste `origin.allowed` bekommt ein `Origin` außerhalb
der Liste 403, vor der Authentifizierung; ohne `Origin` wird der Handshake
angenommen, mit `origin.required` abgewiesen. `permessage-deflate` (RFC 7692):
der Daemon stimmt dem ersten gültigen Angebot zu und komprimiert die
Nachrichten der Verbindung.

**Repo:** `crates/websocket-bridge/src/handshake.rs`,
`crates/websocket-bridge/src/negotiation.rs`,
`crates/websocket-bridge/src/permessage_deflate.rs`,
`crates/websocket-bridge/src/daemon/server.rs` (Upgrade-Path),
`crates/websocket-bridge/src/daemon/security.rs` (`authenticate_ws`,
`render_refusal`), `crates/bridge-security/src/auth.rs`
(`AuthMode::validate_scoped`, `Refusal`, `MissingClaim`).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_handshake_roundtrip_succeeds`,
`::wsbridge_2_rejects_non_upgrade_request`,
`crates/websocket-bridge/src/daemon/server.rs::tests::rfc_6455_4_1_the_subprotocols_are_tokens`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::wsbridge_4_1_a_raw_tcp_client_gets_101_and_the_accept_hash`;
die Python-Bibliothek `websockets` (eigene Prüfung der Subprotocol-Namen)
im Szenario `tests/integration/tests/bridges.rs::websocket_bridge_json_text_frames_against_python_websockets`.
Bis zu dieser Änderung hießen die Subprotocols `zerodds-ws-bridge/1.0` und
`…/1.0+json`; `websockets` lehnte das als „invalid subprotocol“ ab (`/` ist
ein Trennzeichen); der `WebSocket`-Konstruktor des WHATWG-Standards wirft
für einen solchen Namen ebenso (gelesen, nicht gemessen). HTTP-Status des
ungültigen Handshakes:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_1_the_daemon_answers_an_invalid_handshake_with_its_http_status`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_2_the_daemon_answers_an_unsupported_version_with_426_and_names_13`.
Länge und Frist des Request-Heads (`MAX_HANDSHAKE_SIZE`, `HANDSHAKE_TIMEOUT`):
`crates/websocket-bridge/tests/daemon_rfc6455.rs::wsbridge_4_1_a_request_head_over_64_kib_is_answered_with_400`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::wsbridge_4_1_a_handshake_that_does_not_arrive_in_time_gets_408`.
Vorher schloss der Daemon einen Head über 64 KiB ohne Antwort, ohne Log-Zeile,
ohne Fehlerzähler und ohne Span, und ein Client, der nach dem Connect nichts
sendete, hielt seinen Thread bis zum Shutdown.
`permessage-deflate`:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_9_the_daemon_agrees_to_permessage_deflate_and_compresses_both_directions`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_9_the_daemon_declines_an_offer_that_is_invalid`; mit Python
`websockets` als Client
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::python_websockets_client_talks_compressed_to_the_daemon_and_closes`.
Prüfung des `Origin`:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_answers_an_origin_it_does_not_expect_with_403`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_accepts_an_origin_it_expects_and_a_client_without_one`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_refuses_a_handshake_without_origin_when_one_is_required`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_without_a_list_the_daemon_accepts_every_origin`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_checks_the_origin_before_the_credentials`,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::ws_10_2_the_daemon_checks_the_origin_python_websockets_sends`
(Python `websockets` als Client, wöchentlicher Job `integration-bridges-vendor`).
401 und 403:
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_4_1_a_token_without_the_required_claim_is_403`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_a_jwt_is_checked_and_its_subject_meets_the_acl`,
`crates/websocket-bridge/src/daemon/security.rs::tests::wsbridge_4_1_the_refusal_is_401_or_403_with_its_challenge`,
`crates/bridge-security/src/auth.rs::tests::rfc_6750_3_1_a_verified_token_without_the_required_claim_is_told_apart`.
Bis zu dieser Änderung bekam ein geprüfter JWT ohne den Claim 401.

**Status:** done

### §4.2 Pfad-Routing /topics/<slug> + Meta-Endpoints

**Spec:** §4.2 — `/topics/<slug>` Default-Pfad; `ws_path` ersetzt ihn (das
Topic ist nur dort erreichbar); `/topics/__catalog__` über WebSocket als
erstes Frame und auf ein einfaches HTTP-GET, mit den Topics, die der Client
lesen oder schreiben darf; `/healthz` auf dem WebSocket-Listener, ohne
Credentials, auch ohne Metrics-Endpoint; die Metriken unter `metrics.path`
am Admin-Endpoint.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (`serve_connection`,
`plain_get_path`, `origin_field`), `crates/websocket-bridge/src/daemon/runtime_common.rs`
(`CatalogSnapshot::render_json_where`, `health_response`,
`serve_admin_endpoints`), `crates/websocket-bridge/src/daemon/config.rs` (`TopicConfig::path`,
`CATALOG_PATH`, `HEALTH_PATH`, `metrics.path`).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_2_healthz_answers_on_the_ws_listener_without_metrics`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_2_the_metrics_path_is_the_configured_one`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_2_ws_path_replaces_the_default_path`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_2_the_catalog_path_serves_the_catalog_over_ws_and_http`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_4_2_the_catalog_names_the_topics_its_client_may_use`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_2_the_catalog_over_http_checks_the_origin`,
`crates/websocket-bridge/src/daemon/server.rs::tests::wsbridge_4_2_a_plain_get_is_told_from_an_opening_handshake`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_4_2_a_ws_path_that_cannot_be_served_is_refused`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_admin_endpoint_serves_catalog_and_healthz_and_metrics`.

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün.

### §4.3 Frame-Format Binary "ZDB1" + JSON-Mode

**Spec:** §4.3 — Binary-Magic `0x5A 0x44 0x42 0x31`, Flags,
4-Byte-Encap-Header `[0x00,0x07,0x00,0x00]`, CDR-Payload; Text-Frame
JSON-Mode optional. Flag-Bit 1 auf einem Sample, das sein Writer in einem
Coherent Set schrieb; ein Binary-Frame eines Clients mit gesetztem Flag-Bit
bekommt einen Fehler 400. Auf einem Topic mit `idl_file` bekommt ein
Binary-Frame unter XCDR1 oder einem anderen XCDR2-Format als dem des Typs
einen Fehler 400. Die Samples zweier Clients, die gleichzeitig auf dasselbe
Topic schreiben, gehen jeweils unter ihrem eigenen Header hinaus.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs`
(`binary_sample_frame`, `split_binary_sample`, `json_sample_frames`,
`dds_sample_of`, `handle_binary_sample`),
`crates/websocket-bridge/src/daemon/router.rs` (`DdsSample::coherent_set`),
`crates/websocket-bridge/src/daemon/json_frames.rs` (JSON-Textrahmen, Typ
aus `idl_file`), `crates/websocket-bridge/src/codec.rs` (RFC-6455-Codec).

**Tests:** `crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_3_a_sample_frame_is_topic_op_key_hash_data_and_time`,
`crates/websocket-bridge/src/daemon/json_frames.rs::tests::xtypes_7_4_2_a_client_sample_is_the_delimited_body_of_its_type`,
`crates/websocket-bridge/src/daemon/pump.rs::tests::xtypes_7_6_3_1_2_a_binary_frame_carries_the_identifier_the_sample_came_under`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`,
`tests/integration/tests/bridges.rs::websocket_bridge_against_python_websockets`,
`tests/integration/tests/bridges.rs::websocket_bridge_json_text_frames_against_python_websockets`,
`crates/websocket-bridge/tests/fuzz_smoke.rs`. Flag-Bit 1:
`crates/websocket-bridge/src/daemon/pump.rs::tests::wsbridge_4_3_a_sample_of_a_coherent_set_carries_bit_1`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_3_a_coherent_set_reaches_a_binary_client_with_bit_1`
(ein Writer im Participant des Daemons schreibt zwei Samples in einem Coherent
Set und eines danach; der Binary-Client bekommt die Flags `02 02 00`; sein
Frame mit Bit 1 bekommt einen Fehler 400). Vorher setzte der Daemon Bit 1 nie.
Representation des DataWriters: Core::hold_writer hält den Writer des Topics
vom Setzen des Headers bis zum Write;
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_3_a_binary_frame_in_a_format_the_typed_writer_does_not_offer_is_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_3_samples_of_concurrent_clients_go_out_under_their_own_header`
(ein Binary-Client schreibt big endian, ein JSON-Client little endian, je 200
Samples gleichzeitig; ein dritter Client prüft Header gegen Body). Ohne die
Sperre schlug dieser Test auf dem Linux-Testrechner in 2 von 3 Läufen fehl.
Vorher stellte ein Binary-Frame unter XCDR1 den Header eines typisierten
Writers auf XCDR1, obwohl er nur XCDR2 anbietet, und Fehler der drei Setter
wurden verworfen.

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün.

### §4.4 Control-Messages subscribe/unsubscribe/ping/pong/error

**Spec:** §4.4 — JSON-Control-Messages bidir + RFC-6455 PING/PONG. Die
Control-Messages sind Text-Frames; eine Binary-Nachricht ohne `ZDB1` bekommt
einen Fehler 400, auch wenn sie UTF-8-JSON enthält. Ein DDS-Write, der
fehlschlägt, bekommt das Fehler-Frame 500, für ein JSON-Sample-Frame und ein
Binary-Frame.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/daemon/json_frames.rs` (`parse_client`,
`subscribed`, `error`, `pong`), `crates/websocket-bridge/src/daemon/server.rs`
(`handle_inbound_frame`).

**Tests:** `crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_4_the_control_messages`,
`crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_4_a_message_of_another_form_is_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::direction_in_writes_into_dds_and_out_reads_from_it`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_4_a_control_message_in_a_binary_frame_is_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_4_a_dds_write_that_fails_is_an_error_frame_500`
(der Test entfernt den DataWriter des Daemons; Binary-Frame und JSON-Sample
bekommen je `{"op":"error","code":500}`, der Zähler errors_total steigt um 2). Vorher
bekam ein Binary-Frame, dessen Write fehlschlug, kein Fehler-Frame, und eine
Binary-Nachricht mit `{"op":"ping"}` wurde mit `pong` beantwortet.

**Status:** done — die Tests liefen auf dem Linux-Testrechner grün.

### §4.5 Ungültige Daten und Schließen

**Spec:** §4.5 — der Daemon prüft jedes Frame eines Clients (RFC 6455
§10.7). Ungültige Daten beenden die Verbindung mit dem Close-Frame ihres
Status-Codes: 1002 (unmaskiertes Frame, reserviertes Bit, reservierter
Opcode, nicht kürzeste Länge, fragmentiertes oder zu langes Control-Frame,
gebrochene Fragmentfolge, ungültiger Close-Body), 1007 (Text oder
Close-Grund ist kein UTF-8, mit dem Oktett, das ihn ungültig macht), 1009
(Frame über `limits.max_frame_size` oder Nachricht über
`limits.max_message_size`, Default je 16 MiB, das Frame ab seinem Header). Eine
fragmentierte Nachricht wird zusammengesetzt. Ein Close des Clients wird mit
einem Close desselben Status-Codes beantwortet. Nach jedem Close-Frame, das
der Daemon sendet (die Antwort, das Close für ungültige Daten, das Close 1001
beim Herunterfahren, §9.2), beendet er seine Senderichtung, über TLS mit
`close_notify` vor dem FIN, und liest höchstens 500 ms, bis der Client
schließt.

**Repo:** `crates/websocket-bridge/src/connection.rs` (`Receiver`, `Sender`, `Failure`),
`crates/websocket-bridge/src/daemon/server.rs` (`serve_connection`, `WsOut`).

**Tests:** `crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_1_the_daemon_closes_an_unmasked_frame_with_1002`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_2_the_daemon_closes_a_reserved_bit_a_reserved_opcode_and_a_wrong_length_with_1002`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_4_the_daemon_reassembles_a_fragmented_message`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_4_the_daemon_closes_a_broken_fragment_sequence_with_1002`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_5_1_the_daemon_answers_a_close_with_a_close_of_the_same_status`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_5_5_1_the_daemon_closes_an_invalid_close_body_with_1002_or_1007`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_8_2_the_daemon_closes_malformed_utf8_in_a_text_frame_with_1007`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_closes_a_frame_over_its_limit_with_1009_from_the_header`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_closes_a_message_of_many_frames_over_its_limit_with_1009`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_1_the_daemon_closes_the_connection_when_the_client_does_not`
(beide Seiten der 500 ms: nach 250 ms hält der Daemon die Verbindung und
nimmt Oktette an, nach 1 s ist sie beendet; nach einem Close des Clients und
nach einem unmaskierten Frame),
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_1_the_daemon_reads_on_behind_the_close_of_its_shutdown`
(dasselbe nach dem Close 1001),
`crates/websocket-bridge/tests/security_e2e.rs::ws_7_1_the_daemon_ends_a_tls_session_with_close_notify_behind_its_close_frame`
(über TLS: Close-Frame, der Alert close_notify, FIN). Die Grenzen aus der
Konfiguration:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_holds_the_frame_limit_of_its_configuration`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_holds_the_message_limit_of_its_configuration`.

**Status:** done

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus Topic-Name → URL

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `ws_path`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (slug helper),
`crates/websocket-bridge/src/daemon/router.rs`.

**Tests:** `crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_5_1_slug_strips_double_colon`,
`::wsbridge_5_1_slug_replaces_unsafe_chars`;
`crates/websocket-bridge/src/uri.rs::tests` (14 Tests) prüft die URL-Form;
das Pfad-Routing `crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_5_1_dispatch_to_subscribed_connection`
und `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

### §5.2 Type-Discovery Catalog-Frame + /schema/<slug>

**Spec:** §5.2 — der Catalog-Frame `{"op":"catalog","topics":[…]}` mit
`name`, `type`, `direction`, `ws_path`, `qos` (`reliability`, `durability`,
`history` mit `kind` und `depth`) und `schema_url` für ein Topic mit
`idl_file`; als erster Text-Frame auf `/topics/__catalog__` und mit
`catalog_on_connect: true` auf jeder Verbindung; nur die Topics, die der
Client lesen oder schreiben darf.

**Repo:** `crates/websocket-bridge/src/daemon/runtime_common.rs`
(`CatalogSnapshot::render_json_where`: `history` als Objekt, `schema_url`),
`crates/websocket-bridge/src/daemon/config.rs` (Schlüssel
`catalog_on_connect`), `crates/websocket-bridge/src/daemon/server.rs`
(`serve_connection`: der Frame auf jeder Verbindung mit dem Schlüssel;
`settings_that_need_a_restart`), `man/man5/zerodds-ws-bridged.yaml.5`,
`packaging/linux/configs/ws-bridged.yaml.example`, `docs/doc-map/config.md`.
Vorher nannte das Item `router.rs` und einen Catalog-Frame, den keine
Verbindung außer der des Catalog-Pfads bekam, ohne `schema_url`.

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_the_catalog_frame_names_topics_qos_and_schema_url`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_catalog_render_json_contains_topic_fields`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_5_2_catalog_on_connect_is_read`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_a_connection_gets_the_catalog_frame_when_configured`
(mit dem Schlüssel auf einem Topic-Pfad und auf `/` der Catalog als erster
Frame, auf dem Pfad eines Topics ohne Typ vor dem Fehler-Frame 415, ohne
den Schlüssel nicht),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_2_the_catalog_path_serves_the_catalog_over_ws_and_http`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_4_2_the_catalog_names_the_topics_its_client_may_use`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_admin_endpoint_serves_catalog_and_healthz_and_metrics`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_admin_endpoint_serves_catalog_metrics_healthz`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.

### §5.2a `/schema/<slug>`-IDL-Endpoint

**Spec:** §5.2 — ein HTTP-GET von `/schema/<slug>` am WebSocket-Listener
liefert die IDL des Topics (oder die XML-Typdarstellung); `<slug>` ist der
Slug von §5.1 des Topic-Namens.

**Repo:** `crates/websocket-bridge/src/daemon/schema.rs` (`slug`,
`schema_url`, `route`, `expand_includes`: die IDL von `idl_file` mit den
`#include`-Dateien an ihrer Stelle; `xml_type_representation`: die
XML-Typdarstellung nach DDS-XTypes 1.3 §7.3.2 und Annex A, jeder benutzte
Typ vor seiner ersten Verwendung, in seinen `<module>`s),
`crates/websocket-bridge/src/daemon/json_frames.rs` (`TopicType::idl`,
`TopicType::dynamic_type`), `crates/websocket-bridge/src/daemon/server.rs`
(`schema_response`: 200 als `text/plain` oder `application/xml`, 404 für
einen Slug ohne Topic oder ein Topic ohne `idl_file`, 403 ohne Lese- oder
Schreibrecht, 501 für einen Typ ohne XML-Form; Origin und Credentials wie
für einen Topic-Pfad), `crates/websocket-bridge/src/daemon/config.rs`
(zwei Topics mit `idl_file` und einem Slug verhindern den Start).

**Tests:** `crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_the_schema_path_maps_a_slug_to_its_topic`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_the_includes_are_in_place`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_an_include_of_no_file_stays`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_an_include_cycle_is_an_error`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_a_header_with_pragma_once_is_in_place_once`
(ein Header mit `#pragma once`, den zwei Dateien einbinden, steht einmal im
Dokument, ohne die Pragma-Zeile; ein Header ohne die Zeile bei jedem
`#include`),
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_an_include_line_is_recognised`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_types_nested_deeper_than_64_have_no_xml_form`
(ein Typ, der tiefer als 64 Ebenen verschachtelt ist, hat keine XML-Form: ein Fehler),
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_the_xml_form_is_the_type_of_the_idl`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_the_xml_form_of_a_union_a_bitmask_and_a_mutable_struct`
(die XML-Form, gelesen vom DDS-XML-Frontend `zerodds-xml-compose`, ist
derselbe Typ wie der des IDL-Frontends: `equals_ignoring_annotations`;
beide Frontends sind eigener Code, keine fremde Implementierung),
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_a_collection_of_collections_has_no_xml_form`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_5_2_two_topics_of_one_schema_slug_are_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_the_schema_endpoint_returns_the_idl_of_the_topic`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_5_2_the_schema_needs_the_credentials_and_a_grant`;
mit einem fremden IDL-Compiler
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_5_2_cyclone_idlc_compiles_the_served_idl`
(die ausgelieferte IDL eines Topics, dessen `idl_file` eine zweite Datei
einbindet, übersetzt Cyclone DDS `idlc` in einem Verzeichnis ohne diese
Datei; wöchentlicher Job `integration-bridges-vendor`, Szenario
`websocket-cyclone-idlc`).

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner (der
Cyclone-Test mit `--features cross-vendor-tests --run-ignored all`). Nicht
geprüft: die XML-Form mit einem fremden Werkzeug (`rtiddsgen` ist auf dem
Testrechner nicht vorhanden).

---

## §6 QoS-Translation

### §6 DDS-QoS → WS-Behavior Map

**Spec:** §6 — RELIABLE: bei voller Sendewarteschlange
(`max_send_queue_per_connection`) wartet das Topic auf den Client, kein
Sample geht für einen verbundenen Client verloren; ein Client, der
`ping_interval_ms` lang keinen Frame annimmt, wird geschlossen; BEST_EFFORT:
das älteste BEST_EFFORT-Sample der vollen Warteschlange wird verworfen, bei
nur RELIABLE-Samples das neue, jeder Verlust wird gezählt; Durability außer VOLATILE: die letzten N Samples
je Instanz (KEEP_LAST) oder alle (KEEP_ALL), höchstens `max_burst_samples`,
gehen an eine Verbindung beim Abonnieren vor den Live-Samples; Deadline
(`qos.deadline_ms`): `{"op":"deadline_missed"}` je verpasster Periode;
Liveliness: Lease der DDS-Endpoints (`qos.liveliness_lease_ms`), WebSocket-Ping
alle `ping_interval_ms`, ohne Pong Close 1011; Partition (`qos.partition`)
an DataReader und DataWriter.

**Repo:** `crates/websocket-bridge/src/daemon/qos_translation.rs` (`topic_qos`, `behavior_for`,
`dds_qos_to_ws_behavior`: die eine Abbildung),
`crates/websocket-bridge/src/daemon/router.rs` (`ConnQueue`, `Router::route`, `Route::deliver`,
`Router::subscribe` mit Burst, `Router::deadlines_missed`),
`crates/websocket-bridge/src/daemon/server.rs` (Pump, Deadline-Thread, Ping im Writer-Thread,
`register_topic_endpoints`), `crates/websocket-bridge/src/daemon/config.rs` (Schlüssel),
`crates/websocket-bridge/src/daemon/json_frames.rs` (`deadline_missed`).

**Tests:** `crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_best_effort_drops_on_a_full_queue`,
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_reliable_blocks_on_a_full_queue`,
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_transient_local_bursts_the_last_n_on_subscribe`,
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_a_missed_deadline_is_sent_once_per_period`,
`crates/websocket-bridge/src/daemon/qos_translation.rs::tests::wsbridge_6_the_topic_keys_are_the_qos_and_the_behavior`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_6_the_qos_keys_are_read`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_a_missed_deadline_is_a_deadline_missed_frame`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_a_silent_client_is_closed_after_a_missed_pong`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_transient_local_bursts_the_last_n_on_connect`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_a_sample_of_another_partition_does_not_reach_the_client`,
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_a_best_effort_sample_never_drops_a_reliable_one`,
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_a_keyed_sample_without_a_key_hash_is_not_merged`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_the_samples_of_a_client_go_out_in_the_partition`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_the_deadline_and_the_lease_reach_the_dds_endpoints`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_a_best_effort_drop_is_counted`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_a_client_that_takes_nothing_is_closed_and_the_topic_goes_on`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_6_deadline_missed_passes_the_read_acl`,
`crates/websocket-bridge/src/daemon/server.rs::tests::wsbridge_9_2_a_reload_names_the_changed_settings_that_need_a_restart`.
Grenze der Control-Messages und der Bursts in der Warteschlange
(`MAX_QUEUED_CONTROLS`, ein Burst je Topic, Close 1008 beim Überlauf):
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_a_newer_burst_of_a_topic_replaces_the_waiting_one`,
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_control_messages_over_the_bound_overflow_the_queue`;
das Close 1008 des Writer-Threads hat keinen Ende-zu-Ende-Test.
Vorher wurde `crates/websocket-bridge/src/daemon/qos_translation.rs` nirgends verwendet, die Warteschlange war
unbegrenzt, und Deadline, Liveliness, Partition und der Burst fehlten. Bis
zu dieser Änderung hatten die Control-Einträge der Warteschlange keine
Grenze: ein Client, der nicht las und subscribe/unsubscribe wiederholte,
legte je subscribe einen Burst ab.

**Status:** partial — die Tests liefen auf dem Linux-Testrechner grün.
Offen: Der Daemon berechnet den KeyHash nicht aus dem Payload. Die Reader
eines Topics mit Schlüssel bekommen keine Typunterstützung für den Schlüssel
(dafür fehlt in `zerodds-dcps` das Gegenstück zu
`set_user_writer_key_support` für Reader, und der Daemon müsste aus der
`idl_file` eine `SerializedTypeSupport` bauen). Ein Sample ohne
`PID_KEY_HASH` auf einem solchen Topic nennt deshalb keine Instanz: Es geht
an die Clients, wird aber nicht für spätere Abonnenten gehalten und startet
keine Deadline; es wird in `zerodds_ws_bridge_errors_total` gezählt und
einmal je Topic protokolliert (Spezifikation §6). Ein Topic ohne `idl_file`
gilt als Topic ohne Schlüssel. Jeder Verlust zählt in
`zerodds_ws_bridge_send_queue_drops_total{connection_id}` (§8.2).

## §7 Security

### §7.1 TLS wss:// + Cert-Rotation

**Spec:** §7.1 — `wss://`-Mode per `tls.enabled`, SIGHUP-Cert-Rotation: der
Daemon liest Zertifikat und Schlüssel neu, neue Sessions bekommen das neue
Zertifikat, bestehende Verbindungen behalten ihre Session.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (TLS-Hook),
`crates/websocket-bridge/src/daemon/security.rs` (rustls-Wireup),
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::ws_10_6_tls_plain_handshake_succeeds_then_data_flows`
(Frames in beide Richtungen über die TLS-Session),
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_sighup_cert_reload_keeps_daemon_running_with_new_cert`
(nach dem Reload: neue Session mit dem neuen Zertifikat, die alte Verbindung
tauscht weiter Frames, ein Client, der nur dem alten Zertifikat traut,
scheitert am Handshake),
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_the_binary_reloads_the_certificate_on_sighup`
(dasselbe mit SIGHUP an das Binary);
`crates/websocket-bridge/src/daemon/security.rs::tests::ws_10_6_ctx_from_config_tls_enabled_without_paths_rejected`;
`crates/bridge-security/src/connection.rs::tests::mqttbridge_7_1_rotating_config_reload_swaps_inner_arc`.

**Status:** done

### §7.2 Auth-Modes none/bearer/jwt/mtls

**Spec:** §7.2 — vier Auth-Modes über Header bzw. Cert.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs`,
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/auth.rs` (none/bearer/jwt/mtls);
`jwt` mit dem Block `auth.jwt` aus §3 (`public_key`, `audience`,
`required_claim`, dazu `issuer`) über `SecurityConfig::apply_jwt`. Vorher
ließ sich `jwt` wählen, aber kein Schlüssel angeben, und der Start endete als
Bind-Fehler (Exit 2, gemessen). Das mTLS-Subject ist
der Subject-DN des Zertifikats (RFC 4514); eine ACL-Regel darf stattdessen
den Fingerprint `mtls:<SHA-256>` nennen. `jwt` prüft RS256 und ES256
(`JwtKey`, `jwt_key_from_pem`: ein RSA-Schlüssel für RS256, ein
P-256-`PUBLIC KEY` für ES256, Signatur `R || S` mit 64 Oktetten); das `alg`
des Tokens muss zum Schlüssel passen. Vorher nahm `crates/bridge-security/src/auth.rs` nur RS256 an.
`none` startet nur auf einer Loopback-Adresse
(`crates/websocket-bridge/src/daemon/config.rs::listen_is_loopback`, `DaemonConfig::check_auth`); auf `0.0.0.0`
ist es ein Konfigurationsfehler (Exit 1), `--check-config` meldet die
Konfiguration ungültig (Exit 1), ein SIGHUP-Reload auf `none` wird
verworfen. Der Reload prüft gegen die Adresse, die der Daemon bedient
(`crates/websocket-bridge/src/daemon/server.rs::reload` mit `listen` der Startkonfiguration), denn `listen`
wartet auf einen Neustart. Vorher startete `none` auf jeder Adresse, und ein
Reload, der `listen` auf `127.0.0.1` und `mode` auf `none` setzte, schaltete
`none` auf dem weiter gebundenen `0.0.0.0` ein. `bearer` ohne Token oder mit
leerem Token (ein nicht gesetztes `${VAR}`) ist ein Konfigurationsfehler;
vorher startete der Daemon damit.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_a_jwt_is_checked_and_its_subject_meets_the_acl`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_jwt_without_a_key_and_mtls_without_a_client_ca_are_config_errors`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_auth_missing_bearer_yields_401`, `crates/websocket-bridge/tests/security_e2e.rs::ws_10_7_auth_invalid_bearer_yields_401`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_auth_valid_bearer_then_acl_allows_subscribed_topic`;
`crates/websocket-bridge/src/daemon/security.rs::tests::wsbridge_7_2_ws_extracts_bearer_from_headers_case_insensitive`,
`::wsbridge_7_2_ws_missing_authorization_header_returns_missing`,
`::wsbridge_7_2_ws_none_auth_yields_anonymous_without_header`,
`::wsbridge_7_2_ctx_from_config_with_bearer_tokens`,
`::wsbridge_7_2_none_needs_a_loopback_listen_address`,
`::wsbridge_7_2_bearer_needs_a_token`;
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_check_config_refuses_none_off_loopback`;
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_an_es256_jwt_is_checked`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_none_is_refused_off_loopback`
(auch der Reload, der `listen` auf `127.0.0.1` verlegt: weiter 401),
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_a_client_certificate_subject_meets_the_acl`;
`crates/bridge-security/src/auth.rs::tests::jwt_es256_rfc_7515_a_3_example_verifies`,
`::jwt_es256_is_verified`, `::jwt_alg_must_match_the_key_type`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.

### §7.3 Per-Topic-ACL read/write

**Spec:** §7.3 — `acl.read`/`acl.write` Listen mit `*`/exact/group-Match.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (ACL-Felder),
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs` (Match-Engine).

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_auth_valid_bearer_then_acl_allows_subscribed_topic`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_3_auth_valid_bearer_but_acl_denies_forbidden_topic`;
`crates/websocket-bridge/src/daemon/security.rs::tests::wsbridge_7_3_ws_acl_check_allows_listed_user`;
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_7_3_example_yaml_topic_acl_reaches_the_daemon`;
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_3_a_jwt_group_meets_a_group_rule`
(`*group:engineers*` gegen den `groups`-Claim eines JWT durch den Daemon: 101 mit der Gruppe, 403 ohne);
`crates/bridge-security/src/acl.rs::tests::group_wildcard_allows_members`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log auf stdout, ein Objekt pro Zeile, Felder
timestamp/level/event/connection_id/topic/bytes/peer/latency_us; `log_level`
(`--log-level`) als Schwelle, ein SIGHUP-Reload übernimmt einen geänderten
Wert; die Tabelle der Events mit Level und Feldern.

**Repo:** `crates/websocket-bridge/src/daemon/log.rs` (`Level`, `set_level`,
`Event::field/number/integer/latency/render/emit`, `rfc3339`),
`crates/websocket-bridge/src/daemon/server.rs` (`ConnLog`, `log_frame_in`,
`WsOut::message_on` für `frame_out`, die Events von Start, Verbindungen,
Ablehnungen, Reload und Shutdown; `peer` aus `TcpStream::peer_addr` im
Accept-Thread), `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`
(`running`, `stopped`, `start_failed` auf stdout und als Text auf stderr).
Vorher schrieb der Daemon Freitext mit `eprintln!` auf stderr, und
`log_level` wurde gelesen und von keinem Code-Pfad benutzt.

**Tests:** `crates/websocket-bridge/src/daemon/log.rs::tests::wsbridge_8_1_a_line_is_a_json_object_with_the_spec_fields`,
`crates/websocket-bridge/src/daemon/log.rs::tests::wsbridge_8_1_the_log_level_is_the_threshold`,
`crates/websocket-bridge/src/daemon/log.rs::tests::rfc3339_5_6_the_utc_date_and_time`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_1_the_binary_logs_json_lines_for_a_frame_exchange`
(das Binary mit `log_level: debug`: jede Zeile ist JSON; `connection_open`,
`frame_in` mit `connection_id`, `topic`, `bytes`, `peer`, `latency_us`,
`frame_out` der Antwort, `frame_refused` mit Grund, `connection_close`; mit
`log_level: error` schreibt derselbe Austausch keine Zeile);
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_1_a_sighup_reload_applies_a_changed_log_level`
(das Binary startet mit `error`, ein Frame-Austausch schreibt keine Zeile;
nach dem Umschreiben der Datei auf `debug` und SIGHUP schreibt dieselbe
Verbindung `frame_in` und `frame_out`; nach dem Umschreiben auf `warn` und
SIGHUP wieder keine Zeile);
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_the_binary_reloads_the_certificate_on_sighup`
liest `running` und `reload` aus dem Log.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `metrics.*`-Config; die zehn Familien
`zerodds_ws_bridge_*` mit ihren Labels, dazu `errors_total`.

**Repo:** `crates/websocket-bridge/src/daemon/runtime_common.rs`
(`BridgeMetrics`, `family`, `auth_reason`: `frame_in`/`frame_out` unter
`{topic, direction}`, `send_queue_drop` unter `{connection_id}`,
`dds_received`/`dds_published` unter `{topic}`, `auth_failure` unter
`{reason}`), `crates/websocket-bridge/src/daemon/server.rs` (Zählstellen:
`WsOut::message_on`, `handle_inbound_frame`, `handle_binary_sample`, die
Pumpen, Auth und ACL), `crates/websocket-bridge/src/daemon/router.rs`
(`Delivery::dropped_on`), `crates/websocket-bridge/src/daemon/config.rs`
(`metrics.*`). Vorher trug der Code neun Familien `zerodds_ws_*` ohne
Labels; `send_queue_drops_total{connection_id}` und `auth_failures_total`
fehlten. Ein Sample-Frame mit einem nicht konfigurierten `topic` zählt unter
`topic=""` (`BridgeMetrics::by_topic`); vorher legte jeder neue Name eines
Clients zwei Serien an. `connections_total` zählt im Accept-Thread, also
auch eine Verbindung mit gescheitertem TLS-Handshake; vorher erst nach dem
TLS-Handshake. Eine ACL-Ablehnung zählt nur in `auth_failures_total`, ein
Fehler-Frame von §4.4 einmal in `errors_total`; vorher zählte eine
ACL-Ablehnung eines Sample-Frames auch in `errors_total` und ein
JSON-Fehler-Frame dort doppelt.

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_2_metrics_register_is_idempotent`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_2_the_ten_families_carry_their_labels`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_admin_endpoint_serves_catalog_and_healthz_and_metrics`;
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_2_a_frame_each_way_counts_under_its_topic`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_2_a_sample_frame_of_an_unknown_topic_counts_under_no_topic`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_admin_endpoint_serves_catalog_metrics_healthz`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_a_best_effort_drop_is_counted`
(die Drops unter `connection_id="1"`, aus dem Prometheus-Text des Admin-Endpoints);
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_8_2_a_refused_token_counts_as_auth_failure`
(ACL-Ablehnung von `subscribe`, JSON- und Binär-Sample-Frame, `errors_total` 0),
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_8_2_a_failed_tls_handshake_counts_as_an_accepted_connection`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_4_1_a_token_without_the_required_claim_is_403`;
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_a_best_effort_sample_never_drops_a_reliable_one`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Die
Spec nennt die Bedeutung von `direction` (die `direction` des Topics in der
Konfiguration), die Werte von `reason` und `errors_total`.

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT`-ENV aktiviert
Span-Emission per `zerodds-observability-otlp`: ein Trace je Verbindung mit
der Wurzel `ws.connection`, je Nachricht `ws.frame.in` (mit `dds.write`) und
`ws.frame.out`; ein abgelehnter Opening Handshake als `ws.connection` mit
Status error.

**Repo:** `crates/websocket-bridge/src/daemon/trace.rs` (`Tracer`,
`ConnectionSpan`, die Spans und ihre Attribute),
`crates/websocket-bridge/src/daemon/server.rs` (`serve_connection`: die
Wurzel ab dem Opening Handshake, `trace_frame_in`, `WsOut::message_on`,
die Zeiten von `dds.write` in `handle_inbound_frame` und
`handle_binary_sample`; `DaemonHandle::shutdown` sendet nach der letzten
Verbindung noch einmal),
`crates/websocket-bridge/src/daemon/runtime_common.rs` (Endpoint aus der
Umgebung, Flush alle 5 s, der Stop wird alle 100 ms geprüft).

**Tests:** `crates/websocket-bridge/src/daemon/trace.rs::tests::{wsbridge_8_3_a_frame_span_is_a_child_of_its_connection_span,
wsbridge_8_3_a_frame_span_names_topic_type_and_octets,
wsbridge_8_3_a_control_message_has_no_dds_write,
wsbridge_8_3_a_refused_message_is_an_error_span,
wsbridge_8_3_an_outgoing_message_is_a_child_of_its_connection,
wsbridge_8_3_the_connection_span_is_the_root,
wsbridge_8_3_an_end_before_the_start_is_clamped,
wsbridge_8_3_a_tracer_without_exporter_is_off,
wsbridge_8_3_span_ids_differ}`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_3_the_spans_of_a_connection_and_its_frames_reach_a_collector`
(auch eine Anfrage, die kein UTF-8 ist, als 400 mit Status error, eine
Verbindung, die der Client zurücksetzt, mit Status error und dem Grund des
Lesefehlers),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_3_a_connection_that_fails_on_a_write_timeout_is_an_error_span`,
`crates/websocket-bridge/src/daemon/server.rs::tests::wsbridge_8_3_a_failed_write_fails_the_connection_span`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_3_otlp_config_from_endpoint_parses_http_url`,
`::wsbridge_8_3_otlp_config_from_endpoint_parses_bare_host_port`,
`::wsbridge_8_3_otlp_config_from_endpoint_handles_https_and_trailing_slash`,
`::wsbridge_8_3_otlp_config_from_endpoint_falls_back_to_default_port`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner. Der
End-to-End-Test startet das Binary mit einem OTLP-Collector im Test und
prüft Wurzel, Kinder, Eltern-Span-IDs, Trace-ID und Attribute im
`POST /v1/traces`. Jeder abgelehnte Opening Handshake ist ein
`ws.connection` mit Status error, auch die 400 einer Anfrage, die kein
UTF-8 ist. Eine Verbindung, die an einem Socket-Fehler scheitert (ein
Lesefehler, ein Schreibvorgang, den der Client nicht innerhalb von
`ping_interval_ms` annimmt, ein Ping, der nicht geschrieben werden kann),
hat Status error mit dem Grund; ein Schreibversuch nach dem Close des
Daemons zählt nicht als Fehler. Scheitert das Schreiben der 101-Antwort,
endet der Root-Span mit Status error. §8.3a ist hierin aufgegangen.

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config-Parse → TLS → DCPS → Reader/Writer-Register →
WS-Bind → Signal-Handler.

**Repo:** `crates/websocket-bridge/src/daemon/mod.rs`,
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_handshake_roundtrip_succeeds`
(Spawn-Sequence).

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain (max 30 s), `op:shutdown`-Frame, Close
1001, Cleanup; SIGHUP Hot-Reload TLS+ACL.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (signal +
drain), `crates/websocket-bridge/src/daemon/runtime_common.rs` (SIGTERM/
SIGINT/SIGHUP via Cluster-A-Signal-Handler);
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`. Ablauf von
`DaemonHandle::shutdown`: Annahme endet, die Pumpen geben ab, was ihre
DataReader halten, und enden (`draining`); danach `{"op":"shutdown"}` hinter
den Samples jeder Warteschlange (`json_frames::shutdown`), Close 1001, das
Lesen des Close des Clients; alles innerhalb von `shutdown_drain_ms`
(Vorgabe 30000, `crates/websocket-bridge/src/daemon/config.rs`), danach das Stop-Flag und höchstens 2 s für
die übrigen Verbindungen. SIGTERM/SIGINT setzen `shutdown_requested`, nicht
mehr das Stop-Flag. systemd-Unit: `TimeoutStopSec=40`. Vorher schickte der
Daemon keinen `shutdown`-Frame, und das Stop-Flag beendete die Pumpen vor
dem Leeren ihrer Kanäle.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_the_binary_exits_0_on_sigterm`
(das Binary lief vorher 3 s nach SIGTERM weiter, gemessen),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_graceful_shutdown_completes_within_5s`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_sighup_cert_reload_keeps_daemon_running_with_new_cert`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_9_2_sighup_applies_the_reloaded_acl` (ACL und Auth, 403 vor dem Reload, 101 danach),
`crates/websocket-bridge/src/daemon/server.rs::tests::wsbridge_9_2_a_reload_names_the_changed_settings_that_need_a_restart` (die Liste der geänderten Einstellungen, die auf einen Neustart warten: `limits`, `origin`, Topic-Map, TLS-Dateipfade, `listen`, `domain`);
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_9_2_shutdown_broadcasts_to_all`;
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_samples_queued_before_the_signal_arrive_before_the_shutdown_frame`
(300 Samples, die beim Beenden noch im Daemon warten, kommen vollständig und in Reihenfolge vor `{"op":"shutdown"}` an, dann Close 1001),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_the_drain_is_bounded_by_shutdown_drain_ms`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_the_daemon_closes_every_connection_with_1001_when_it_shuts_down`
(16 Verbindungen: `{"op":"shutdown"}`, dann Close 1001),
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_the_binary_closes_every_connection_with_1001_on_sigterm`
(dasselbe über das Binary und SIGTERM, Oktett für Oktett),
`crates/websocket-bridge/tests/security_e2e.rs::ws_7_1_the_daemon_ends_a_tls_session_with_close_notify_behind_its_close_frame`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_9_2_shutdown_drain_ms_is_read`;
die Close-Codes selbst `crates/websocket-bridge/src/close.rs::tests` (25).

**Status:** partial — die Tests liefen grün auf dem Linux-Testrechner.
Offen: Auf Windows setzt nichts das Flag des Beendens (`install_signal_watcher`
ist dort ein leerer Thread; es fehlen der Konsolen-Handler für Ctrl+C und der
Stopp des Windows-Dienstes `ZeroDDSWSBridge`). Der Linux-Testrechner hat kein
Windows-Target, und es gibt keinen Windows-Runner; ungeprüfter Code wurde
nicht geschrieben.

## §10 Cross-Vendor

### §10 RTPS-Peer Co-Existence Cyclone/RTI/Fast-DDS

**Spec:** §10 — Daemon ist normaler RTPS-Peer; verifiziert in
`crates/websocket-bridge/tests/cross_vendor.rs`.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (DCPS-Wire-Up;
`register_topic_endpoints`: der DataWriter eines Topics mit `idl_file`
bietet nur XCDR2 an und trägt von Anfang an den Header der Extensibility des
Typs, `json_frames::TopicType::identifier`),
`crates/websocket-bridge/tests/interop/{reading.idl,cyclone_reading.c}`
(Cyclone-DDS-Teilnehmer, vom Test mit `idlc` und dem C-Compiler gebaut),
`crates/websocket-bridge/tests/interop/fastdds_reading.cpp` (Fast-DDS-3-
Teilnehmer, der Typ über die DynamicTypes-API, XCDR2, vom Test mit dem
C++-Compiler gebaut), der `shape_main` von RTI Connext DDS 7.7.0 aus dem
Release der OMG-Suite `dds-rtps` (`interop/shapes/omg-dds-rtps.idl`,
`ShapeType`, `-x 2`), `tests/integration/provision.sh` (`fastdds-dev`,
`connext-shape-main`: das Release-Asset mit dem sha256 aus
`tests/interop/omg/vendors.tsv` nach `/opt/omg-connext`),
`ci/integration_scenarios.py` (Job `integration-bridges-vendor`, Feature
`cross-vendor-tests`, Szenarien `websocket-cyclone`, `websocket-fastdds`,
`websocket-connext`). Vorher bot der Writer
`[XCDR, XCDR2]` an und schickte XCDR2; ein Cyclone-DDS-Reader nahm die
JSON-Samples eines Clients nicht an (gemessen: gematcht, kein Sample).

**Tests:** `crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_a_ws_publish_reaches_cyclone_and_a_cyclone_sample_a_ws_client`
(Cyclone DDS 0.10.5: ein JSON-`publish` erreicht den Cyclone-Reader als
dieses Sample; ein Sample des Cyclone-Writers erreicht einen JSON-Client als
JSON und einen Binär-Client als ZDB1-Frame; der ZDB1-Frame, vom Client
zurückgeschickt, erreicht den Cyclone-Reader wieder),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`
(DDS-Roundtrip über einen DomainParticipant),
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_fastdds_a_ws_publish_reaches_fastdds_and_a_fastdds_sample_a_ws_client`
(dieselben Wege mit Fast DDS 3.1.2),
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_rti_a_ws_publish_reaches_connext_and_a_connext_sample_a_ws_client`
(dieselben Wege mit RTI Connext DDS 7.7.0 auf `Square`: der Connext-Reader
druckt das Shape des JSON-`publish`, ein Shape des Connext-Writers erreicht
den JSON-Client und als ZDB1-Frame mit `D_CDR2` den Binär-Client, der
ZDB1-Frame erreicht den Connext-Reader wieder mit denselben `x`, `y`).

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner
(`--features cross-vendor-tests --run-ignored all`); der Connext-Test mit
dem `shape_main` aus dem Release-Asset (`CONNEXT_SHAPE_MAIN`).

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-ws-bridged`; Config-Defaults pro OS;
systemd/launchd/Win-Service; Docker `zerodds/ws-bridged:1.0`; Manuals
1+5.

**Repo:** `packaging/linux/systemd/zerodds-ws-bridged.service`,
`packaging/macos/launchd/org.zerodds.ws-bridged.plist`,
`packaging/macos/homebrew/zerodds-ws-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ws-bridged/` (das Image trägt eine eigene Konfiguration,
`ws-bridged.yaml`: `0.0.0.0:8080`, `bearer` mit dem Token aus
`WS_BRIDGE_TOKEN`; vorher kopierte es das Beispiel mit `127.0.0.1` und
`none`, das über einen veröffentlichten Port nicht erreichbar war),
`packaging/docker/docker-compose.yml` (verweist auf diese Datei statt auf das
fehlende `conf/ws-bridged.yaml`),
`packaging/linux/configs/ws-bridged.yaml.example`,
`man/man1/zerodds-ws-bridged.1`,
`man/man5/zerodds-ws-bridged.yaml.5`;
der Windows-Dienst: `crates/dcps/src/cli.rs` (`run_as_service`,
`request_service_stop`, `request_service_reload`, `READY=1` und
`STOPPING=1` von `notify_service_manager` als Dienstzustände),
`crates/dcps/src/cli/windows_service.rs` (`StartServiceCtrlDispatcherW`,
`RegisterServiceCtrlHandlerExW` mit Stop, Shutdown, Paramchange und
Interrogate, `SetServiceStatus` von `SERVICE_START_PENDING` bis
`SERVICE_STOPPED` mit dem Exit-Code, der Konsolen-Handler, wenn der
Prozess aus einer Konsole läuft),
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs` (`main` startet
den Daemon über `run_as_service("ZeroDDSWSBridge", …)`),
`crates/websocket-bridge/src/daemon/runtime_common.rs`
(`install_signal_watcher` außerhalb von Unix: `watch_service_requests`
setzt die Shutdown- und Reload-Flags von SIGTERM und SIGHUP); die Spec §11
und `man/man1/zerodds-ws-bridged.1` (WINDOWS SERVICE) nennen die
Dienstzustände und Controls. Vorher hatte
das Binary keinen Service-Control-Dispatcher, und der Watcher außerhalb von
Unix war ein leerer Thread.

**Tests:** `crates/websocket-bridge/tests/example_yaml_loadback.rs::{wsbridge_3_example_yaml_parses_without_error, wsbridge_3_example_yaml_key_fields_match_intent, wsbridge_7_3_example_yaml_topic_acl_reaches_the_daemon, wsbridge_11_example_yaml_has_no_unknown_top_level_keys}`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_11_the_docker_image_serves_its_published_port_with_a_token`
(die Konfiguration des Images: mit Token gültig, ohne Token abgelehnt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel);
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_11_the_service_requests_set_the_shutdown_and_reload_flags`
(der Watcher von Windows in einem Thread unter Linux: ohne Anfrage wartet
er, er endet mit dem Daemon, eine Reload-Anfrage setzt das Reload-Flag,
eine Stopp-Anfrage das Shutdown-Flag);
`crates/dcps/src/cli.rs::service_tests::{deployment_4_3_the_service_controls_are_stop_reload_and_interrogate, deployment_4_3_the_notify_states_are_service_states, deployment_4_3_the_exit_code_has_its_number, deployment_4_3_the_requests_and_the_run}`
(die Control-Codes, die Zustände, der Exit-Code eines gestoppten Dienstes,
die Anfragen und der Lauf außerhalb des Dienst-Managers);
`crates/dcps/src/cli/windows_service.rs::tests::deployment_4_3_the_control_codes_are_the_win32_constants`
(nur Windows).

**Status:** partial — die Tests liefen grün auf dem Linux-Testrechner; der
Windows-Code wurde dort mit `cargo clippy --target x86_64-pc-windows-gnu -p
zerodds-websocket-bridge --all-targets -- -D warnings` und ebenso für
`zerodds-dcps` (`--lib --tests`) übersetzt, nicht ausgeführt. Es fehlt: (1)
der Lauf des Dienstes `ZeroDDSWSBridge` unter `sc.exe` (Start, `READY=1`
als `SERVICE_RUNNING`, Stopp mit dem Drain, `paramchange` als Reload); er
braucht einen Windows-Rechner mit dem registrierten Dienst, der
Linux-Testrechner ist keiner; der Windows-Job von `zerodds-dcps` (Windows-Runner,
Benutzer SYSTEM) führt die Tests von `cli` aus, darunter den Konsolenweg
von `run_as_service`, und ist auf dieser Änderung noch nicht gelaufen. (2) Kein Job
startet das Image `packaging/docker/ws-bridged/` und spricht es mit einem
Client an; `release.yml` baut es nur beim Release, und auf dem
Messrechner (unprivilegierter LXC-Container) scheitert `docker build` laut `ci/release-packaging-check.yml` (gelesen, nicht gemessen).

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — je ≥ 5 Unit-Tests in den Modulen Konfiguration
(`src/daemon/config.rs`), Topic-Map und Sende-Queues (`src/daemon/router.rs`),
Authentifizierung und ACL (`src/daemon/security.rs`), Frame-Codec
(`src/codec.rs`) und DDS-Pump (`src/daemon/pump.rs`). Vorher nannte die Spec
`src/bin/` als Ort, an dem kein Modul liegt.

**Repo:** `crates/websocket-bridge/src/daemon/pump.rs` (aus `crates/websocket-bridge/src/daemon/server.rs`
herausgelöst: `dds_sample_of`, `binary_sample_frame`, `split_binary_sample`,
`check_client_flags`, `writer_form`, `json_sample_frames`),
`crates/websocket-bridge/src/{codec.rs,daemon/config.rs,daemon/router.rs,daemon/security.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul (Zahl der `#[test]` im
Quelltext):
`crates/websocket-bridge/src/codec.rs::tests` (21),
`crates/websocket-bridge/src/frame.rs::tests` (9),
`crates/websocket-bridge/src/message.rs::tests` (22),
`crates/websocket-bridge/src/masking.rs::tests` (10),
`crates/websocket-bridge/src/handshake.rs::tests` (22),
`crates/websocket-bridge/src/negotiation.rs::tests` (12),
`crates/websocket-bridge/src/permessage_deflate.rs::tests` (23),
`crates/websocket-bridge/src/close.rs::tests` (28),
`crates/websocket-bridge/src/utf8.rs::tests` (18),
`crates/websocket-bridge/src/uri.rs::tests` (14),
`crates/websocket-bridge/src/origin.rs::tests` (10),
`crates/websocket-bridge/src/connection.rs::tests` (35),
`crates/websocket-bridge/src/daemon/config.rs::tests` (22),
`crates/websocket-bridge/src/daemon/router.rs::tests` (11),
`crates/websocket-bridge/src/daemon/security.rs::tests` (11),
`crates/websocket-bridge/src/daemon/pump.rs::tests` (10),
`crates/websocket-bridge/src/daemon/json_frames.rs::tests` (6),
`crates/websocket-bridge/src/daemon/qos_translation.rs::tests` (9).
Die Pump: `crates/websocket-bridge/src/daemon/pump.rs::tests::wsbridge_12_1_an_alive_sample_is_a_frame_with_flags_0`,
`::wsbridge_12_1_a_dispose_carries_bit_0_and_the_key_holder`,
`::wsbridge_12_1_an_unregister_carries_bit_2`,
`::wsbridge_12_1_a_marker_that_is_both_carries_both`,
`::wsbridge_12_1_the_coherent_set_end_is_no_frame`,
`::wsbridge_12_1_a_client_frame_with_any_flag_bit_is_refused`,
`::wsbridge_12_1_a_sample_of_a_topic_without_type_is_the_error_415`
(Sample → Frame für Sample, Dispose, Unregister, beides, Ende des Coherent
Set; die Flags eines Client-Frames). Dazu `crates/websocket-bridge/tests/fuzz_smoke.rs::wsbridge_12_1_fuzz_websocket_decode_no_panic`,
`::wsbridge_12_1_empty_input_no_panic`, `::wsbridge_12_1_single_byte_inputs_no_panic`,
`::wsbridge_12_1_extended_length_overflow_no_oom`.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — der Daemon als Subprozess, ein WS-Client publiziert, ein
DDS-Reader im Testprozess empfängt; byte-genauer Roundtrip. Die Datei ist
`tests/daemon_e2e.rs` (die Spec nannte `tests/bridge_e2e.rs`, die es nicht
gab).

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_12_2_a_ws_publish_reaches_a_dds_reader_byte_exact`
(das Binary als Subprozess, ein eigener `DcpsRuntime` im Testprozess auf
der geliehenen Domain mit Reader und Writer, Discovery zwischen den
Prozessen: der ZDB1-Frame eines Binär-Clients erreicht den Reader mit
denselben CDR-Bytes und dem Header `00 07`; ein Sample des Writers erreicht
den Client als ZDB1-Frame mit denselben Bytes und Header);
die Szenarien der Integrations-Säule mit Python `websockets` als Client
gegen das Binary: `tests/integration/tests/bridges.rs::websocket_bridge_against_python_websockets`,
`tests/integration/tests/bridges.rs::websocket_bridge_json_text_frames_against_python_websockets`.
Vorher standen hier Tests mit dem Daemon im Testprozess und eingespeisten
Reader-Samples ohne WS-Publish, den ein DDS-Reader empfängt.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — der Daemon im Testprozess gegen einen fremden
DDS-Teilnehmer in beiden Richtungen (Cyclone DDS, Fast DDS, RTI Connext
DDS), Feature `cross-vendor-tests`, Job `integration-bridges-vendor`. Vorher
nannte die Spec einen Cyclone-Subscriber in Docker Compose, den es nicht gab.

**Repo:** `crates/websocket-bridge/tests/cross_vendor.rs`,
`crates/websocket-bridge/tests/interop/`, `tests/integration/provision.sh`,
`ci/integration_scenarios.py`.

**Tests:** `crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_a_ws_publish_reaches_cyclone_and_a_cyclone_sample_a_ws_client`,
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_fastdds_a_ws_publish_reaches_fastdds_and_a_fastdds_sample_a_ws_client`,
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_rti_a_ws_publish_reaches_connext_and_a_connext_sample_a_ws_client`.
Der Handshake-Test eines rohen Clients, der hier stand, lief ohne fremden
Teilnehmer gegen den eigenen Daemon; er ist jetzt
`crates/websocket-bridge/tests/daemon_rfc6455.rs::wsbridge_4_1_a_raw_tcp_client_gets_101_and_the_accept_hash`
unter §4.1.

**Status:** done — die Tests liefen grün auf dem Linux-Testrechner.

## §13 Cross-References

### §13 Verwandte Library + Daemons

**Spec:** §13 — Library `crates/websocket-bridge/`, Wire-Format-Spec
`zerodds-xcdr2-bindings-conformance-1.0` §3, Deployment-Spec
`zerodds-deployment-1.0`, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config, Major=Wire-
Protocol-Change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Regel für die Vergabe der Versionsnummer eines Release, keine Eigenschaft des Daemons.

---

## Audit-Status

23 done / 3 partial / 0 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-ws-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (Job `merge-rust-bridge-security`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 4 Tests grün.
* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (Job `merge-rust-websocket-bridge`) — 52 Tests grün.
* `python3 ci/run_integration.py integration-bridges` (Job `integration-bridges`) — 2 Tests grün.

59 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-ws-bridge-1.0.open.md`.
