# `zerodds-grpc-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-grpc-bridge-1.0.md`

Implementation:

- `crates/grpc-bridge/` — DDS↔gRPC-Bridge.

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Zuordnung der Levels; die Anforderungen sind die Items §1a bis §12.

### §1a L6 Multi-Tenant

**Spec:** §1 L6 — mehrere DomainParticipants je Daemon; eine
Mandantenbindung je gRPC-Verbindung.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/` (`TenantConfig`, `DaemonConfig::from_config`, `Participant`, `participant_of`, `serve_tls`, `security_from`, `SpecMetrics::register`).

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_1_l6_a_tls_connection_is_bound_to_the_tenant_of_its_certificate`,
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_1_l6_the_tenants`,
`::grpcbridge_1_l6_a_subject_is_bound_to_one_tenant_at_most`,
`::grpcbridge_1_l6_the_acl_of_a_tenant_topic_carries_its_name`,
`::grpcbridge_1_l6_tenants_need_tls_with_a_client_ca`,
`::grpcbridge_3_every_key_of_the_spec_example_reaches_the_config`,
`crates/grpc-bridge/tests/daemon_cli_e2e.rs::grpcbridge_2_exit_3_when_the_participant_does_not_start`.

**Status:** done — ein Eintrag von `tenants` ist ein DomainParticipant auf seiner Domäne mit eigenen Topics. Eine TLS-Verbindung, deren Client-Zertifikat (Subject-DN oder `mtls:`-Fingerabdruck) ein Mandant nennt, ist für ihre Lebensdauer an ihn gebunden: ihr `Publish` kommt bei einem DDS-Reader auf der Domäne des Mandanten an (im Test geprüft), ihr `Catalog` nennt nur seine Topics, die Topics von `domain` sind ihr `NOT_FOUND`. Eine Verbindung ohne Zertifikat und eine mit einem Zertifikat ohne Mandant sehen die Topics von `domain`. ACL-Einträge und die Labels `dds_topic`/`service` sind `<tenant>/<name>`. Geprüft mit dem Subject-DN; der Fingerabdruck bindet über dieselbe Funktion und ist nur im Unit-Test geprüft. `tenants` ohne `grpc.tls.enabled` und `grpc.tls.client_ca_file` ist ein Konfigurationsfehler mit Datei und Zeile (Exit 1, auch unter `--check-config`), da ohne beide keine Verbindung ein Client-Zertifikat trägt.

## §2 CLI-Surface

### §2 zerodds-grpc-bridged CLI

**Spec:** §2 — Optionen `--config`/`--bind`/`--domain`/`--tls-*`/
`--reflection`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
Exit-Codes 0/1/2/3/4.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/mod.rs` (`run`, `DaemonError`, `log_event!`, `LOG_LEVEL`), `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs` (`main`).

**Tests:** `crates/grpc-bridge/tests/daemon_cli_e2e.rs::grpcbridge_2_version_and_help_exit_0`,
`::grpcbridge_2_exit_1_on_an_unknown_argument_and_an_invalid_file`,
`::grpcbridge_2_exit_2_when_the_port_is_in_use`,
`::grpcbridge_2_exit_3_when_the_participant_does_not_start`,
`::grpcbridge_2_exit_4_when_the_tls_material_does_not_load`,
`::grpcbridge_2_each_short_form_wins_over_the_file`,
`::grpcbridge_2_check_config_refuses_what_the_start_refuses`;
`crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_2_log_level_filters_the_lines`,
`::grpcbridge_9_2_sigterm_sends_goaway_with_the_last_stream_and_exits_0` (Exit 0);
`crates/grpc-bridge/tests/contract_dcps.rs::a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file`,
`::the_usage_text_names_the_configuration_flags`.

**Status:** done — jede Kurzform ist `--set` ihres Dateischlüssels und gewinnt über die Datei (unter `--check-config` mit ihrer Herkunft geprüft). Exit 0 nach `--version`, `--help`, gültigem `--check-config` und SIGTERM; 1 bei unbekanntem Argument, nicht lesbarer Datei, unbekanntem Schlüssel, falschem Typ, Wert außerhalb seines Bereichs, `--tls-cert` ohne `--tls-key` und bei Einstellungen nach §7, die nicht bedient werden können (`auth.mode: bearer` ohne `auth.bearer_token`, `jwt` ohne nutzbaren Schlüssel, `mtls` ohne TLS und Client-CA, `tenants` ohne TLS und Client-CA); `--check-config` lehnt dieselben Fälle mit Exit 1 ab, im Test gegen den Start geprüft; das TLS-Material lädt erst der Start; 2 bei belegtem Port; 3, wenn der Participant (auch der eines Mandanten) nicht startet; 4 bei fehlendem oder unlesbarem TLS-Material. `--log-level` setzt die Schwelle der Ereigniszeilen auf stderr. `--topic` verlangt `DDS=SERVICE` oder `DDS:SERVICE`, wie die Synopsis; der Default-Servicename nach §5.1 gehört zu §5.1.

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`log_level`/`grpc`/`auth`/`topics`/`metrics`/`tenants`;
die Grenzen `grpc.max_concurrent_streams`, `max_recv_message_size`, `max_send_message_size`,
`initial_window_size`, `max_frame_size`, `grpc.keepalive.*`, `grpc.tls.alpn`, `metrics.path`;
ENV-Substitution.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/` (`DaemonConfig::from_config`, `read_topic`,
`handle_connection`: Keepalive, Ablehnungen), `crates/grpc-bridge/src/server.rs` (`ServerLimits`,
`GrpcServer::handshake`, Fenster, `REFUSED_STREAM`, `RESOURCE_EXHAUSTED`, PING-Regeln),
`crates/grpc-bridge/src/daemon_runtime.rs::serve_admin_endpoints_at` (`metrics.path`).

**Tests:** `crates/grpc-bridge/src/bridge_daemon/config.rs::tests`: `grpcbridge_3_the_example_of_the_spec_loads`,
`grpcbridge_3_every_key_of_the_spec_example_reaches_the_config`, `grpcbridge_3_a_value_outside_its_range_is_refused`,
`grpcbridge_3_config_default_has_default_bind`,
`grpcbridge_3_parse_minimal_grpc_config`, `grpcbridge_3_parse_topics_array`, `grpcbridge_3_parse_invalid_yaml_returns_err`,
`grpcbridge_3_example_yaml_parses_without_error`, `grpcbridge_3_example_yaml_key_fields_match_intent`,
`grpcbridge_3_example_yaml_topics_reach_the_daemon`, `grpcbridge_3_example_yaml_has_no_unknown_top_level_keys`,
`grpcbridge_3_unknown_top_level_key_does_not_abort_the_parse`, `grpcbridge_3_env_substitution_applies_before_the_parse`,
`grpcbridge_3_keep_all_reaches_the_writer_and_takes_no_depth`;
`crates/grpc-bridge/src/server/tests.rs`: `grpcbridge_3_the_handshake_announces_the_limits`,
`grpcbridge_3_a_stream_above_max_concurrent_streams_is_refused`, `grpcbridge_3_a_message_above_max_recv_message_size_is_refused`,
`grpcbridge_3_a_message_above_max_send_message_size_is_resource_exhausted`,
`grpcbridge_3_the_receive_windows_are_kept_at_initial_window_size`,
`grpcbridge_3_data_beyond_a_stream_window_resets_the_stream`,
`grpcbridge_3_a_small_stream_window_applies_after_the_settings_ack`,
`grpcbridge_3_padded_data_is_taken_without_its_padding`, `grpcbridge_3_a_frame_above_max_frame_size_is_refused`,
`grpcbridge_3_pings_without_a_call_follow_permit_without_calls`, `grpcbridge_3_a_keepalive_ping_and_its_ack`;
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_3_metrics_path_is_served`;
`crates/grpc-bridge/tests/daemon_limits_e2e.rs::grpcbridge_3_the_limits_of_the_file_reach_the_wire`,
`::grpcbridge_3_max_recv_message_size_answers_resource_exhausted`,
`::grpcbridge_3_max_concurrent_streams_refuses_the_stream_above`,
`::grpcbridge_3_max_send_message_size_ends_the_stream_with_resource_exhausted`,
`::grpcbridge_3_keepalive_pings_and_closes_without_an_ack`, `::grpcbridge_3_keepalive_with_acks_keeps_the_connection`,
`::grpcbridge_3_pings_without_a_call_follow_permit_without_calls`;
`crates/config/tests/layering.rs::a_hash_inside_quotes_is_part_of_the_value`;
`tools/traceability/tests/bridge_config_manpages.rs::every_example_key_is_documented_in_the_man_page`.

**Status:** done — das Beispiel der Spec lädt, jeder seiner Schlüssel kommt mit seinem Wert an. Die Grenzen wirken auf der Leitung (gegen den Daemon gemessen): die SETTINGS nennen `max_concurrent_streams`, `initial_window_size` und `max_frame_size`, ein WINDOW_UPDATE öffnet das Verbindungsfenster, DATA über dem Fenster eines Streams bekommt `RST_STREAM(FLOW_CONTROL_ERROR)` (im Unit-Test), ein Stream-Fenster unter 65 535 gilt erst nach dem ACK der SETTINGS (RFC 9113 §6.9.3, im Unit-Test), die Auffüllung von PADDED DATA gehört nicht zur Anfrage (im Unit-Test), ein Stream über der Grenze bekommt `RST_STREAM(REFUSED_STREAM)`, eine Nachricht über `max_recv_message_size` oder `max_send_message_size` endet mit `RESOURCE_EXHAUSTED`, der Daemon sendet nach `time_secs` ein PING und nach `timeout_secs` ohne ACK GOAWAY, PINGs ohne Aufruf enden nach dem dritten Strike in `GOAWAY(ENHANCE_YOUR_CALM)`, mit `permit_without_calls: true` bleibt eine Verbindung ohne Aufruf über die 5 s hinaus offen (6,5 s beobachtet), `metrics.path` liefert die Prometheus-Seite. `qos.history` mit `keep_all` ergibt KEEP_ALL (`usize::MAX`), `keep_last` ohne Tiefe lässt den Default der Laufzeit; `keep_all` mit `depth` verhindert den Start, die Meldung nennt `history.depth` (Unit-Test). `tls.alpn` nimmt `h2` und `http/1.1` (Default beide, `h2` zuerst); eine leere Liste, ein doppelter Eintrag, ein anderes Protokoll und eine Liste ohne `h2` verhindern den Start (Unit-Test). Gemessen nur gegen den Client der Tests in diesem Repository; die Flusskontrolle beim Senden (Fenster des Clients) ist unter §4.1 gebaut und geprüft.

## §4 gRPC-Wire-Protocol

### §4.1 HTTP/2-Setup mit ALPN h2

**Spec:** §4.1 — HTTP/2-Server, ALPN `h2`, Cleartext-h2c nur Dev-Mode
(`bind=127.0.0.1`); SETTINGS-Frame, GOAWAY bei Shutdown.

**Repo:** `crates/grpc-bridge/src/server.rs` (Frames, Flusskontrolle in beide
Richtungen, CONTINUATION, Verbindungs- und Stream-Fehler),
`crates/grpc-bridge/src/bridge_daemon/` (`transport_settings`,
`bind_is_loopback`, die ALPN-Prüfung in `serve_tls`, GOAWAY beim Herunterfahren),
`crates/hpack/src/decoder.rs` (Grenze der dekodierten Header-Liste und des
Dynamic Table Size Update).

**Tests:** `crates/grpc-bridge/src/server/tests.rs`:
`grpcbridge_3_the_handshake_announces_the_limits`,
`grpcbridge_4_1_a_message_above_max_frame_size_is_split`,
`grpcbridge_4_1_data_waits_for_the_clients_window`,
`grpcbridge_4_1_a_header_block_in_continuation_frames_is_read`,
`grpcbridge_4_1_padded_headers_and_data_are_read`,
`grpcbridge_4_1_unknown_frames_are_ignored_and_push_promise_is_refused`,
`grpcbridge_4_1_a_large_header_block_goes_out_in_continuation_frames`,
`grpcbridge_4_1_a_header_block_above_1_mib_is_enhance_your_calm`,
`grpcbridge_4_1_a_headers_frame_above_1_mib_is_enhance_your_calm`,
`grpcbridge_4_1_a_header_list_above_the_limit_is_resource_exhausted`,
`grpcbridge_4_1_a_table_size_update_above_4096_is_compression_error`,
`grpcbridge_3_pings_without_a_call_follow_permit_without_calls`,
`grpcbridge_3_the_receive_windows_are_kept_at_initial_window_size`;
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_4_1_h2c_only_on_a_loopback_bind`;
`crates/grpc-bridge/tests/daemon_cli_e2e.rs::grpcbridge_2_check_config_refuses_what_the_start_refuses`;
`crates/grpc-bridge/tests/daemon_limits_e2e.rs::grpcbridge_4_1_a_header_list_above_16_kib_is_resource_exhausted`;
`crates/hpack/src/decoder.rs::tests::rfc9113_6_5_2_a_header_list_above_the_limit_is_refused`,
`::rfc7541_6_3_a_size_update_above_the_limit_is_an_error`,
`::rfc7541_4_2_a_size_update_after_a_field_is_an_error`;
`crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_4_1_tls_without_alpn_h2_gets_no_http2`,
`::grpcbridge_9_2_sigterm_sends_goaway_with_the_last_stream_and_exits_0`;
`crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_4_1_a_megabyte_of_publishes_on_one_connection_all_arrive`,
`::grpcbridge_4_1_the_server_waits_for_the_window_of_the_client`,
`::grpcbridge_4_1_a_stream_takes_no_sample_once_256_kib_wait_for_the_window`.

**Status:** done — gegen den Daemon auf dem Linux-Testrechner gemessen: 1 MiB Samples zu 16 KiB (jede Nachricht größer als ein Frame) auf einer Verbindung kommen vollständig an, der Client innerhalb der Fenster des Servers, der Server innerhalb der Fenster, die der Subscriber mit WINDOW_UPDATE gewährt; ohne WINDOW_UPDATE hält der Server bei den 65 535 Oktetten des Stream-Fensters an und sendet danach weiter. Cleartext-h2c auf einer Adresse, die nicht Loopback ist, verhindert den Start und wird von `--check-config` abgelehnt (Exit 1); ein TLS-Client ohne ALPN, der die HTTP/2-Präambel sendet, bekommt keine SETTINGS, einer mit nur `http/1.1` bekommt den HTTP/1.1-Endpunkt von §5.2 (die Präambel als Anfrage der Methode PRI: 405), mit `grpc.tls.alpn: ["h2"]` scheitert er am Handshake; GOAWAY(NO_ERROR) mit dem letzten Stream bei SIGTERM. CONTINUATION in beide Richtungen, Padding und Priorität von HEADERS, unbekannte Frame-Typen, PUSH_PROMISE (`GOAWAY(PROTOCOL_ERROR)`), ein Header-Block über 1 MiB in HEADERS und CONTINUATION und in einem einzelnen HEADERS-Frame (bei `max_frame_size` über 1 MiB; `GOAWAY(ENHANCE_YOUR_CALM)`, ein Block von genau 1 MiB wird angenommen) und eine kleinere HPACK-Tabelle des Clients im Unit-Test. Die SETTINGS nennen `MAX_HEADER_LIST_SIZE` 16 384; eine Anfrage, deren dekodierte Header-Liste darüber liegt, endet mit `RESOURCE_EXHAUSTED`, ihr Block wird zu Ende dekodiert, ohne die Felder zu halten, und die nächste Anfrage auf derselben Verbindung bezieht sich auf sein Literal (genau 16 384 Oktette sind ein Aufruf, eines mehr nicht; Unit-Test). Gegen den Daemon mit dem Block der Prüfung (ein Literal von 4 000 Oktetten, dann 60 000 Verweise darauf): `RESOURCE_EXHAUSTED`, VmHWM 19 776 → 20 300 kB (vor der Grenze 12 904 → 491 680 kB). Ein Dynamic Table Size Update über 4 096 ist `GOAWAY(COMPRESSION_ERROR)`, eines nach einem Header-Feld ein Decoding-Fehler (Unit-Tests im Server und in zerodds-hpack). Ein `Subscribe`-Stream, dessen Client kein Fenster gewährt, nimmt aus seinem DataReader nur Samples, bis 256 KiB auf das Fenster warten (gegen den Daemon über `zerodds_grpc_bridge_bytes_out_total` gemessen: höchstens 65 535 Oktette des Fensters plus 256 KiB plus eine Nachricht; von 40 Samples zu 16 KiB nicht alle); nach dem WINDOW_UPDATE kommen alle 40 in Reihenfolge an. Gemessen gegen den Testclient dieses Repositorys; grpc-go und grpc-java gegen den Daemon gehören zu §10a.

### §4.2 Service-Definition Auto-Generation pro Topic

**Spec:** §4.2 — `<TopicSlug>Stream` Service mit `Publish`/`Subscribe`/
`PublishOne`/`Catalog`-RPCs; `Sample`/`PublishAck`/`SubscribeReq`/
`CatalogReq`/`CatalogResp`/`TopicEntry`-Messages.

**Repo:** `crates/grpc-bridge/src/messages.rs` (die sechs Nachrichten),
`crates/grpc-bridge/src/service_gen.rs` (`render_proto`, die vier Methoden),
`crates/grpc-bridge/src/path.rs` (slug→service-name),
`crates/grpc-bridge/src/bridge_daemon/` (`sample_to_publish`,
`sample_of`, `catalog_resp`, `BridgeDds::publish`, `BridgeDds::next_sample`).

**Tests:** `crates/grpc-bridge/src/messages.rs::tests`: `grpcbridge_4_2_sample_carries_its_six_fields`,
`grpcbridge_4_2_publish_ack_carries_sequence_number_and_time`, `grpcbridge_4_2_subscribe_req_is_read`,
`grpcbridge_4_2_catalog_resp_lists_topic_entries`, `grpcbridge_4_2_unknown_fields_are_skipped`,
`grpcbridge_4_2_malformed_messages_are_errors`;
`crates/grpc-bridge/src/service_gen.rs::tests`: `grpcbridge_4_2_render_proto_is_the_definition_of_the_spec`,
`grpcbridge_4_2_render_proto_contains_methods`, `grpcbridge_4_2_pascal_simple`,
`grpcbridge_4_2_pascal_dotted`, `grpcbridge_4_2_pascal_mixed_separators`, `grpcbridge_4_2_pascal_empty_falls_back`,
`grpcbridge_4_2_topic_service_paths`, `grpcbridge_4_2_matches_path_publish`,
`grpcbridge_4_2_catalog_registration_dedup`, `grpcbridge_4_2_catalog_find_by_path`,
`grpcbridge_4_2_catalog_fully_qualified_names`;
`crates/grpc-bridge/src/path.rs::tests::grpc_requests_parses_standard_grpc_path`,
`crates/grpc-bridge/src/path.rs::tests::grpc_requests_parses_dotted_service_name`;
`crates/grpc-bridge/src/bridge_daemon/dispatch.rs::tests::grpcbridge_4_2_a_publish_sample_is_checked`,
`::grpcbridge_4_2_a_subscribe_sample_names_key_hash_time_type_and_topic`,
`::grpcbridge_4_2_source_ts_ns_and_the_rtps_timestamp`,
`::grpcbridge_5_2_build_catalog_emits_topics`;
`crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_4_2_catalog_answers_with_a_catalog_resp`,
`::grpcbridge_4_3_a_publish_stream_of_100_samples_writes_100_and_acks_once`,
`::grpcbridge_4_2_partition_filter_is_the_partition_of_the_streams_reader`,
`::grpcbridge_4_2_a_dispose_is_streamed_only_with_include_dispose`,
`::grpcbridge_6_transient_local_sends_the_initial_burst`,
`::grpcbridge_5_2_the_catalog_rpc_names_dds_type_service_and_idl_url`.

**Status:** done — `render_proto` schreibt den Text der Spec (im Test verglichen). Der Daemon liest und schreibt die sechs Nachrichten: `Publish` wendet `key_hash`, `source_ts_ns` und die Bits für Dispose und Unregister an und lehnt einen `type_name` oder `topic_name` ab, der nicht der des Mappings ist; `PublishAck` nennt die Sequenznummer des letzten Schreibens und die Zeit; ein Sample von `Subscribe` trägt alle sechs Felder (Schlüssel, Zeitstempel, Typ und Topic über einen DDS-Writer und -Reader des Tests), mit `include_dispose` auch das Dispose mit seinem Bit; ein `source_ts_ns` über 4 294 967 295 999 999 999 ns (die RTPS-Zeit hält ihn nicht) ist `INVALID_ARGUMENT` statt eines Schreibens ohne Zeitstempel (Unit-Test); gegen den Daemon: ohne `include_dispose` geht ein Dispose nicht auf den Stream, mit `include_dispose` kommt es zwischen den Samples mit Bit und Schlüssel und ohne Payload; `partition_filter` `"x, a*"` wird zu den Partitionen des Readers des Streams (das Sample eines Writers in Partition `ab` kommt an, das des Writers des Daemons in der Default-Partition nicht); `Catalog` antwortet mit `CatalogResp`, `TopicEntry.idl_url` nennt den IDL-Endpunkt von §5.2. `initial_burst` sendet die gehaltenen Samples zuerst (§6, gegen den Daemon). Gemessen gegen den Testclient dieses Repositorys.

### §4.3 RPC-Flows Publish/Subscribe

**Spec:** §4.3 — HEADERS/DATA/HEADERS-END_STREAM-Flow mit `:method=POST`,
`:path=/<pkg>.<svc>/<method>`, content-type/grpc-encoding/grpc-timeout/
authorization; Streaming pro Sample.

**Repo:** `crates/grpc-bridge/src/server.rs` (`CallEvent`, `GrpcCall`),
`crates/grpc-bridge/src/bridge_daemon/calls.rs` (`Conn`: `on_open`,
`on_message`, `on_half_close`, `start_stream`, `pump`),
`crates/grpc-bridge/src/timeout.rs`.

**Tests:** `crates/grpc-bridge/src/server/tests.rs`:
`grpcbridge_4_3_a_client_stream_delivers_each_message_and_the_half_close`,
`grpcbridge_4_3_two_messages_in_one_data_frame_are_two_messages`,
`grpcbridge_4_3_a_message_across_data_frames_is_one_message`,
`grpcbridge_4_3_a_stream_that_ends_inside_a_message_is_internal`,
`grpcbridge_4_3_the_call_carries_its_request_headers`,
`grpcbridge_4_3_reject_answers_with_the_http_status`,
`grpcbridge_4_3_an_answer_before_the_clients_end_resets_the_stream`,
`grpcbridge_4_3_data_on_a_stream_the_client_closed_is_stream_closed`;
`crates/grpc-bridge/src/bridge_daemon/dispatch.rs::tests::grpcbridge_4_3_grpc_timeout_units`;
`crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_4_3_a_publish_stream_of_100_samples_writes_100_and_acks_once`,
`::grpcbridge_4_3_a_wrong_content_type_is_refused`;
`crates/grpc-bridge/tests/dds_bridge_e2e.rs::grpcbridge_4_3_dds_publish_reaches_every_subscribe_stream`;
`crates/grpc-bridge/tests/bridge_e2e.rs::grpcbridge_2_http2_roundtrip_publish_topic`.

**Status:** done — gegen den Daemon gemessen: ein `Publish`-Client-Stream mit 100 Samples schreibt 100 Samples in Reihenfolge (jedes auf einem `Subscribe`-Stream) und antwortet einmal, beim Half-Close, mit Sequenznummer 100; `PublishOne`, `Subscribe` und `Catalog` nehmen eine Anfragenachricht (eine zweite ist `INTERNAL`, keine ist die leere Nachricht); content-type `text/plain` und ein content-type mit einem Mehrbyte-UTF-8-Zeichen über dem 16. Oktett sind HTTP 415 mit `grpc-status` 3 (die Verbindung bleibt offen), `:method GET` 405 mit 12, `application/grpc+json` 12; `grpc-timeout` ist die Frist (§4.5). `authorization` gehört zu §7.2. Gemessen gegen den Testclient dieses Repositorys.

> **F2b — Java-Multi-Process-Bridge:** Der pure-Java gRPC-Client
> `org.zerodds.bridge.GrpcBridgeClient` (`crates/java-omgdds/java`)
> spricht dieselbe gRPC-Wire (minimaler raw-HTTP/2-Framer über Socket,
> da der Daemon prior-knowledge-h2c ist). Java↔Rust-e2e
> `GrpcBridgeClientTest` + `crates/java-omgdds/run_grpc_bridge_e2e.sh`
> (3/3 grün): Java publisht über gRPC → DDS → Java subscribt zurück.
> Realisiert den listener-callbacks §7.3 „gRPC-Bridge-Pfad (pure-Java
> per Vendor-Extension)".

### §4.4 Length-Prefix-Wrapper

**Spec:** §4.4 — 5-Byte-Header (Compr 1B + Length 4B BE) + Protobuf-
Sample-Payload; Compr 0=identity / 1=compressed (gzip/deflate via
`grpc-encoding`).

**Repo:** `crates/grpc-bridge/src/frame.rs`,
`crates/grpc-bridge/src/compression.rs` (gzip RFC 1952, deflate RFC 1950,
die Grenze beim Entpacken), `crates/grpc-bridge/src/bridge_daemon/calls.rs`
(`Conn::on_message`, `Conn::reply`, `Conn::open`).

**Tests:** inline `#[cfg(test)] mod tests` in `crates/grpc-bridge/src/frame.rs`;
`crates/grpc-bridge/src/compression.rs::tests`: `grpcbridge_4_4_gzip_and_deflate_round_trip`,
`grpcbridge_4_4_an_inflated_size_above_the_limit_is_refused`, `grpcbridge_4_4_an_unknown_encoding_is_named`,
`grpcbridge_4_4_a_gzip_message_of_two_members_is_read_whole`;
`crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_4_4_a_gzip_publish_reaches_dds_byte_exact`,
`::grpcbridge_4_4_an_unknown_encoding_is_unimplemented_with_accept_encoding`;
`crates/grpc-bridge/src/bridge_daemon/dispatch.rs::tests::xtypes_7_6_3_1_2_cdr_payload_carries_the_header_the_sample_came_under`,
`crates/grpc-bridge/tests/dds_bridge_e2e.rs::xtypes_7_6_3_1_2_a_stream_message_carries_the_header_the_sample_came_under`
(`Sample.cdr_payload` keeps the encapsulation header the sample came under, DDS-XTypes 1.3 Table 60).

**Status:** done — gegen den Daemon gemessen: ein mit gzip komprimiertes `Publish` erreicht einen `Subscribe`-Stream Byte für Byte, das `PublishAck` kommt mit gzip komprimiert und `grpc-encoding: gzip`, ein `Subscribe` mit `grpc-encoding: deflate` bekommt seine Nachrichten mit deflate komprimiert; eine als `snappy` komprimierte Nachricht ist `UNIMPLEMENTED` mit `grpc-accept-encoding: identity,deflate,gzip`, unkomprimierte Nachrichten unter `snappy` werden gelesen; das Bit 1 ohne Encoding ist `INTERNAL`; die entpackte Größe ist durch `max_recv_message_size` begrenzt (Unit-Test). Eine gzip-Nachricht aus zwei Members wird ganz gelesen, die Grenze gilt für beide zusammen; Oktette nach dem letzten Member, ein abgeschnittenes zweites Member und Oktette nach dem zlib-Stream sind `Corrupt` (Unit-Test). Die Kompression beider Seiten ist flate2; eine Prüfung gegen grpc-go (`UseCompressor(gzip)`) gehört zu §10a.

### §4.5 Status-Mapping OK/INVALID_ARGUMENT/PERMISSION_DENIED/...

**Spec:** §4.5 — gRPC-Status-Codes 0/3/4/7/8/13/14/16 → DDS-Bedingungen.

**Repo:** `crates/grpc-bridge/src/status.rs`,
`crates/grpc-bridge/src/bridge_daemon/` (`route`,
`status_of_write_error`, `Conn::check_deadlines`, `Conn::pump`,
`Conn::drain`), `crates/grpc-bridge/src/server.rs` (`percent_encode`).

**Tests:** inline `#[cfg(test)] mod tests` in `crates/grpc-bridge/src/status.rs`;
`crates/grpc-bridge/src/bridge_daemon/dispatch.rs::tests::grpcbridge_4_5_write_errors_map_to_resource_exhausted_and_internal`,
`::grpcbridge_4_5_a_full_writer_history_is_resource_exhausted`,
`::grpcbridge_4_2_a_publish_sample_is_checked`,
`::dispatch_unknown_service_yields_not_found`,
`::dispatch_refuses_the_direction_a_topic_does_not_carry`;
`crates/grpc-bridge/src/server/tests.rs::grpcbridge_4_5_grpc_message_is_percent_encoded`;
`crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_4_5_a_call_past_its_grpc_timeout_is_deadline_exceeded`,
`::grpcbridge_4_5_a_stream_open_at_drain_ends_unavailable`,
`::grpcbridge_4_5_liveliness_lost_ends_the_stream_unavailable`;
`crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_7_2_bearer_and_acl_are_checked_per_call`;
`crates/grpc-bridge/tests/bridge_e2e.rs::grpcbridge_2_http2_unknown_service_yields_status_5`.

**Status:** done — jede Zeile wird erzeugt: `OK`; `INVALID_ARGUMENT` für eine Nachricht, die kein proto3 ist, und für ein `cdr_payload` ohne CDR-Encapsulation-Header (Unit-Test); `PERMISSION_DENIED` und `UNAUTHENTICATED` gegen den Daemon (§7.2, §7.3); `RESOURCE_EXHAUSTED` für das Schreiben eines TRANSIENT_LOCAL-Writers, dessen RESOURCE_LIMITS erreicht sind (mit einem DDS-Writer im Unit-Test), und für die Grenzen von §3; `DEADLINE_EXCEEDED` für einen `Subscribe`- und einen `Publish`-Stream nach ihrem `grpc-timeout`; `UNAVAILABLE` für die bei SIGTERM offenen Streams und für einen `Subscribe`-Stream, dessen einziger DataWriter (MANUAL_BY_TOPIC, Lease 1 s) seine Liveliness nicht mehr bestätigt, nach dessen Sample (gegen den Daemon); `INTERNAL` für andere DDS-Fehler und die Anfragefehler aus §4.3/§4.4. „CDR decode error“: die CDR-Bytes nach dem Encapsulation-Header werden unverändert weitergegeben (§4.4), die Bridge registriert keinen Typ und dekodiert sie nicht. `grpc-message` ist percent-kodiert.

### §4.6 Reflection-Service grpc.reflection.v1alpha

**Spec:** §4.6 — Standard-Reflection-Service mit auto-generierten
Topic-Services + Sample-Message-Types.

**Repo:** `crates/grpc-bridge/src/reflection.rs` (`decode_request`,
`Reflection::of`, `Reflection::respond`, `reflection_file`),
`crates/grpc-bridge/src/descriptor.rs` (`FileDescriptorProto`),
`crates/grpc-bridge/src/service_gen.rs` (`proto_file`, `proto_files`,
`proto_file_name`), `crates/grpc-bridge/src/bridge_daemon/`
(`Route::Reflection` in `Conn::on_request`, `Conn::on_half_close`,
`Conn::on_message`, `Conn::serve_held`, `reflection_catalog`),
`crates/grpc-bridge/src/server.rs` (`GrpcServer::hold_window`).

**Tests:** `crates/grpc-bridge/src/reflection.rs::tests`:
`grpcbridge_4_6_a_request_is_read_by_its_kind`,
`grpcbridge_4_6_list_services_names_the_packages_of_the_mappings`,
`grpcbridge_4_6_file_containing_symbol_returns_the_descriptor_of_the_service`,
`grpcbridge_4_6_an_unknown_symbol_is_an_error_response`,
`grpcbridge_4_6_empty_catalog_yields_empty_list_services_response`,
`grpcbridge_4_6_error_response_encodes_code_and_message`,
`grpcbridge_4_6_reflection_path_constant_matches_grpcurl_default`;
`crates/grpc-bridge/src/descriptor.rs::tests::grpcbridge_4_6_the_file_descriptor_is_read_back`,
`::grpcbridge_4_6_declares_finds_messages_fields_services_and_methods`,
`::grpcbridge_4_6_json_name_is_lower_camel_case`;
`crates/grpc-bridge/src/service_gen.rs::tests::grpcbridge_4_6_the_descriptor_declares_the_messages_of_4_2`;
`crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_4_6_the_reflection_stream_answers_each_request`
(auch eine andere Methode des Reflection-Service: UNIMPLEMENTED),
`::grpcbridge_4_6_a_reflection_stream_answers_no_request_once_256_kib_wait_for_the_window`;
`crates/grpc-bridge/src/server/tests.rs::grpcbridge_4_6_a_held_stream_returns_no_window_until_released`;
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_list_services_reflection_payload_is_well_formed`;
gegen grpcurl (Job `integration-bridges-vendor-grpc`):
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_grpcurl_describe_names_the_four_methods`,
`::grpcbridge_4_6_grpcurl_lists_the_configured_package`,
`::grpcbridge_4_6_grpcurl_list_services_against_zerodds_daemon`,
`::grpcbridge_4_6_grpcurl_lists_the_reflection_service_itself`.

**Status:** done — der Daemon dient den Reflection-Service unter `grpc.reflection.v1` und `grpc.reflection.v1alpha` als bidirektionalen Stream: jede Anfrage bekommt ihre Antwort auf dem offenen Stream (gegen den Daemon: `list_services`, `file_containing_symbol` einer Methode, ein unbekanntes Symbol als `error_response` NOT_FOUND, dann der Half-Close mit OK; eine Anfrage ohne Inhalt beendet den Stream mit `INVALID_ARGUMENT`; eine andere Methode als `ServerReflectionInfo`, etwa `/grpc.reflection.v1.ServerReflection/Foo`, endet mit `UNIMPLEMENTED` ohne Antwortnachricht). Die `FileDescriptorProto` je Paket enthält die sechs Nachrichten von §4.2 und die Services des Pakets mit den vier Methoden; `valid_host` und `original_request` wiederholen die Anfrage. Gegen grpcurl 1.9.3 (grpc-go) auf dem Linux-Testrechner: `describe` des Service nennt die vier Methoden mit ihrem Streaming, `describe` von `Sample` die sechs Felder, `describe` ohne Symbol jeden gelisteten Service samt Reflection, ein `Catalog`-Aufruf, den grpcurl allein aus den Deskriptoren baut (`-d '{}'`), antwortet mit dem Topic; `list` nennt das konfigurierte `grpc_package`. Ein Reflection-Stream, dessen Client nicht liest, beantwortet keine weitere Anfrage, sobald 256 KiB Antworten auf dessen Fenster warten (er hält also höchstens 256 KiB und die letzte Antwort, bis `max_send_message_size`); die Anfragen danach warten, und der Server gibt das Empfangsfenster des Streams so lange nicht zurück (gegen den Daemon mit `initial_window_size` 65 535 und 3 000 Anfragen zu 56 Oktetten: der Client konnte 1 170 senden, 314 Antworten zu 1 045 Oktetten standen an, also höchstens das Fenster des Clients, 256 KiB und eine Antwort; nach dem WINDOW_UPDATE des Clients gingen die übrigen Anfragen hinaus, alle 3 000 wurden beantwortet, der Half-Close dahinter endete mit OK).

## §5 Topic-Mapping

### §5.1 Service-Namen-Default Topic→ChatMessageStream

**Spec:** §5.1 — `::`-Split + CamelCase + Suffix `Stream`; Package
`zerodds.<segment>.v1`.

**Repo:** `crates/grpc-bridge/src/service_gen.rs` (`default_service`,
`default_package`, `topic_to_pascal`),
`crates/grpc-bridge/src/bridge_daemon/` (`read_topic`,
`is_proto_full_ident`, `refuse_duplicate_services`, `TopicMapping::package`,
`topic_index`, `packages_of`, `route`).

**Tests:** `crates/grpc-bridge/src/service_gen.rs::tests::grpcbridge_5_1_chat_message_becomes_chat_message_stream_in_zerodds_chat_v1`,
`::grpcbridge_5_1_a_single_segment_names_its_own_package`,
`::grpcbridge_4_2_pascal_simple`, `::grpcbridge_4_2_pascal_dotted`,
`::grpcbridge_4_2_pascal_mixed_separators`, `::grpcbridge_4_2_pascal_empty_falls_back`,
`::grpcbridge_4_2_topic_service_paths`;
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_5_1_an_entry_without_service_takes_the_default`,
`::grpcbridge_5_1_the_fields_override_the_default`,
`::grpcbridge_5_1_two_mappings_of_one_service_are_refused`,
`::grpcbridge_5_1_a_service_name_in_two_packages_needs_the_package`;
`crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_4_6_the_reflection_stream_answers_each_request`
(der Default-Service über Reflection gelistet).

**Status:** done — `Chat::Message` wird zu `ChatMessageStream` in `zerodds.chat.v1`; ein Eintrag ohne `grpc_service` und `grpc_package` (Map-Form und `--topic DDS`) bekommt die Defaults, die Felder überschreiben sie, ein `grpc_service` mit Punkten ist `<package>.<Service>`; ein `:path` mit Paket trifft den Service nur in seinem Paket; ein Service-Name ohne Paket, den Mappings in zwei Paketen tragen, trifft keines (`NOT_FOUND`, die Meldung nennt die Pakete), mit Paket trifft er jedes; zwei Einträge desselben voll qualifizierten Service und ein `grpc_service`, der kein protobuf-Bezeichner ist, verhindern den Start (Unit-Tests). Gegen den Daemon listet Reflection den Default-Service im Default-Paket.

### §5.2 Type-Discovery Reflection + Catalog + IDL-Endpoint

**Spec:** §5.2 — drei Pfade (Reflection-Service, Catalog-RPC,
HTTP-GET-Sidekick).

**Repo:** `crates/grpc-bridge/src/bridge_daemon/` (`idl_answer`,
`serve_http1`, `serve_tls` (ALPN `http/1.1` oder keines), `is_http1_start`, der GET-Zweig in `Conn::on_open`, `catalog_resp`, `idl_base`,
`topics[].idl_file` in `read_topic`), `crates/grpc-bridge/src/server.rs`
(`respond_http`, GET mit beliebigem Pfad, `GrpcCall::authority`),
`crates/grpc-bridge/src/daemon_runtime.rs` (`CatalogTopic::grpc_service`);
Reflection: §4.6.

**Tests:** `crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_5_2_get_idl_returns_the_source_as_text_x_omg_idl`,
`::grpcbridge_5_2_get_idl_of_an_unknown_topic_is_404`,
`::grpcbridge_5_2_the_catalog_rpc_names_dds_type_service_and_idl_url`;
`crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_5_2_idl_over_tls_on_the_h2_connection`,
`::grpcbridge_5_2_idl_over_tls_with_http_1_1`;
`crates/grpc-bridge/src/server/tests.rs::grpcbridge_5_2_a_get_is_answered_with_a_plain_response`;
`crates/grpc-bridge/src/bridge_daemon/catalog.rs::tests::grpcbridge_5_2_build_catalog_emits_topics`;
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_5_2_catalog_render_json_is_valid_shape`;
mit curl (Job `integration-bridges-vendor-grpc`):
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_5_2_curl_fetches_the_idl`,
`::grpcbridge_5_2_curl_over_tls_fetches_the_idl_with_http_1_1_and_h2`.

**Status:** done — gegen den Daemon: `GET /idl/<dds_name>` liefert den Text von `idl_file` als `text/x-omg-idl`, als HTTP/1.1 auf dem Cleartext-Listener und als HTTP/2-GET auf einer gRPC-Verbindung (auch percent-kodiert); über TLS auf der h2-Verbindung und als HTTP/1.1 auf einer TLS-Verbindung mit ALPN `http/1.1` oder ohne ALPN, unter der Authentifizierung von §7.2 (ohne Token 401; rustls-Client des Tests); ein Topic ohne Mapping und eines ohne `idl_file` 404, eine kaputte Escape-Sequenz 400, eine andere Methode 405. `Catalog` nennt `dds_type`, den voll qualifizierten Service und `idl_url` mit der `:authority` des Aufrufs, deren Abruf die IDL liefert. curl 8.14.1 auf dem Linux-Testrechner holt die IDL mit `--http1.1` und `--http2-prior-knowledge`, über TLS mit `--http1.1` (ALPN `http/1.1`) und `--http2` (ALPN `h2`). Der Daemon parst die IDL-Datei nicht; er liest sie beim Start.

## §6 QoS-Translation

### §6 DDS-QoS → gRPC-Behavior Map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/
Liveliness/Partition Map; `partition_filter` als Subscribe-Field.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/`
(`dds_pump`: `writer_cfg`, `reader_cfg`, `Retained`, `RetainedStore`,
`refresh_retained`, `burst_start`, `retained_after`, `lifespan_writer`,
`deadlines_missed`; `BurstJoin`, `Conn::start_stream`, `Conn::pump`,
`Conn::drop_best_effort`, `log_dropped`, `Conn::log_deadlines`, `Conn::finish`, `Conn::release`,
`lifespan_of`, `register_writer`; `qos.deadline_ms`, `qos.lifespan_ms`, `qos.history` in `read_topic`),
`crates/grpc-bridge/src/server.rs` (`close_call_with`); Keepalive:
`handle_connection`.

**Tests:** `crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_6_transient_local_sends_the_initial_burst`,
`::grpcbridge_6_the_burst_keeps_depth_per_instance_and_not_a_disposed_one`,
`::grpcbridge_6_the_burst_stops_once_256_kib_wait_for_the_window`,
`::grpcbridge_6_the_burst_and_the_live_samples_of_another_participant_have_no_gap`,
`::grpcbridge_6_a_best_effort_stream_drops_when_the_window_is_full_and_counts_it`,
`::grpcbridge_6_lifespan_metadata_reaches_the_writer`,
`::grpcbridge_6_a_ninth_lifespan_takes_the_place_of_the_least_recently_used`,
`::grpcbridge_6_a_missed_deadline_is_a_trailer`,
`::grpcbridge_4_1_a_stream_takes_no_sample_once_256_kib_wait_for_the_window` (RELIABLE),
`::grpcbridge_4_2_partition_filter_is_the_partition_of_the_streams_reader` (Partition),
`::grpcbridge_4_5_liveliness_lost_ends_the_stream_unavailable`;
`crates/grpc-bridge/tests/daemon_limits_e2e.rs::grpcbridge_3_keepalive_pings_and_closes_without_an_ack`,
`::grpcbridge_3_keepalive_with_acks_keeps_the_connection` (Liveliness);
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs::tests::grpcbridge_6_the_retained_limit_is_shared_by_the_topics_of_a_participant`,
`::grpcbridge_6_the_retained_samples_keep_depth_per_instance`,
`::grpcbridge_6_a_burst_join_sends_each_change_once`,
`::grpcbridge_6_a_lifespan_writer_waits_outside_the_pool_lock`,
`::grpcbridge_6_the_history_of_a_topic_is_the_history_of_its_writers`.

**Status:** done — jede Zeile gegen den Daemon auf dem Linux-Testrechner: RELIABLE nimmt kein weiteres Sample, sobald 256 KiB auf das Fenster des Clients warten, der Rest bleibt im DataReader; ein BEST_EFFORT-Stream ohne Fenster verwirft, was sein Reader hält, zählt es in `zerodds_grpc_bridge_samples_dropped_total` und loggt `samples_dropped` auf `warn` (beim Default-Level `info` sichtbar; höchstens eine Zeile je Sekunde und Stream, der Rest am Ende des Streams, die Summe der Zeilen gleich dem Zähler); TRANSIENT_LOCAL mit `history.depth` 3: von fünf Samples kommen mit `initial_burst` 2 die letzten zwei, mit 10 die letzten drei, danach jedes Live-Sample genau einmal; zwei Instanzen mit `key_hash` bei Tiefe 2: der Burst hält je Instanz die letzten zwei in Ankunftsreihenfolge, nach dem Dispose einer Instanz nur noch die andere; ein Burst von 640 KiB an einen Client ohne Fenster bleibt innerhalb von Fenster plus 256 KiB plus einer Nachricht und kommt nach dem WINDOW_UPDATE vollständig in Reihenfolge an; ein DDS-Writer eines zweiten Teilnehmers, der alle 2 ms schreibt, während ein Client mit `initial_burst` abonniert: der Stream liefert jedes Sample genau einmal und lückenlos (Gegenprobe mit einem Burst nur aus dem Schnappschuss: in 1 von 3 Läufen eine Lücke, 86 → 88); die Obergrenze von 10 000 gilt für alle Topics eines Teilnehmers zusammen, das älteste über alle Topics fällt zuerst (Unit-Test mit Grenze 3 und zwei Topics); `initial_burst` mit `partition_filter` ist `INVALID_ARGUMENT`, ein VOLATILE-Topic hat einen leeren Burst; `qos.lifespan_ms` ist die LIFESPAN des Writers, ein `Publish` mit `zerodds-lifespan-ms: 1500` schreibt mit einem Writer dieser LIFESPAN (ein DDS-Reader des Tests sieht beide Publikationen mit 60 000 und 1 500 ms), ein Wert, der keine Zahl ist, `INVALID_ARGUMENT`; zehn verschiedene `grpc-timeout`-Werte (30 s bis 21 s) auf einem VOLATILE-Topic schreiben alle, der DDS-Reader sieht danach die acht neuesten Lifespans neben der des Topic-Writers (die zwei am längsten unbenutzten wurden nach ihrer Bestätigung gelöscht); auf einem TRANSIENT_LOCAL-Topic ist die neunte Lifespan, solange keiner der acht Writer abgelaufen ist, `RESOURCE_EXHAUSTED`; die Wartezeiten beim Anlegen und Ersetzen laufen außerhalb der Sperre des Pools (Unit-Test mit einem DDS-Reader, der ohne Abmeldung verschwindet: während die neunte Lifespan 1 s auf die Bestätigung des ältesten Writers wartet, kommt ein vorhandener Writer in unter 300 ms zurück, ein Aufruf der zu ersetzenden Lifespan wartet das Ende ab und bekommt den alten Writer zurück, die neunte Lifespan endet mit `OutOfResources`; der Test wartet, bis der Platz der zu ersetzenden Lifespan im Zustand `Evicting` ist, statt einer festen Pause); `qos.deadline_ms` 100 ms: ein Stream, dessen Writer nach einem Sample schweigt, endet mit dem Trailer `zerodds-deadline-missed` ≥ 1 und der Logzeile `deadline_missed`; Liveliness sind die PINGs von `grpc.keepalive`; Partition ist `partition_filter`; `qos.history` ist die HISTORY des Topic-Writers und seiner Writer anderer Lifespans: `keep_all` KEEP_ALL, `keep_last` mit Tiefe 7 die Tiefe 7, ohne Tiefe KEEP_LAST 1 (Unit-Test gegen die Laufzeit); ein Fehler von `set_user_writer_history_depth` verhindert den Start (aus dem Code, ohne Test: die Laufzeit lehnt die Tiefe nur für einen unbekannten Writer oder eine vergiftete Sperre ab). Ohne die Metadaten ist `grpc-timeout` die Lifespan, auf ganze Sekunden aufgerundet (`4500m` ergibt einen Writer mit 5 000 ms). Gemessen gegen den Testclient dieses Repositorys und DDS-Teilnehmer von ZeroDDS.

## §7 Security

### §7.1 TLS h2-ALPN + Cert-Rotation

**Spec:** §7.1 — TLS 1.2+ (1.3 default), ALPN `h2`, Cipher-Suites per
`grpc.tls`, SIGHUP-Cert-Rotation.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/connection.rs::serve_tls`,
`::security_from`, `::apply_reload`, `grpc.tls.min_version` und
`grpc.tls.cipher_suites` in `DaemonConfig::from_config`;
`crates/grpc-bridge/src/daemon_runtime.rs::build_grpc_tls_config_opts`,
`::select_tls_suites`, `::provider_cipher_suites`
(`grpc.tls.client_ca_file`, `grpc.tls.require_client_cert`).

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_7_1_mtls_serves_h2_and_the_certificate_is_the_subject`
(ALPN `h2` am Client gelesen),
`::grpcbridge_7_1_a_sighup_serves_the_next_connection_with_the_new_certificate`,
`::grpcbridge_7_1_the_configured_cipher_suites_are_the_ones_offered`,
`::grpcbridge_7_1_min_version_1_3_refuses_a_tls_1_2_client`,
`::grpcbridge_7_1_an_unknown_suite_stops_the_start` (auch die leere Liste);
gegen grpc-go `crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_7_1_grpcurl_over_tls_with_a_bearer_token`
(grpcurl 1.9.3, Linux-Testhost);
`crates/grpc-bridge/tests/security_e2e.rs::grpcbridge_7_1_build_grpc_tls_config_sets_alpn_h2`,
`::grpcbridge_7_1_build_grpc_tls_config_with_client_ca_for_mtls`, `::grpcbridge_7_1_build_grpc_tls_config_rejects_missing_cert`,
`::grpcbridge_7_1_the_configured_cipher_suites_are_the_ones_in_the_config`,
`::grpcbridge_7_1_an_unknown_suite_does_not_load`.

**Status:** done — gegen den Daemon mit einem rustls-Client: ohne `cipher_suites` handelt ein Client mit TLS 1.2 und 1.3 TLS 1.3 aus, einer nur mit TLS 1.2 wird mit TLS 1.2 bedient; `min_version: "1.3"` lässt den Handshake eines TLS-1.2-Clients scheitern; mit `cipher_suites` gewinnt die Reihenfolge des Servers (ChaCha20 vor AES-128 gegen einen Client, der AES zuerst anbietet), eine Suite außerhalb der Liste scheitert am Handshake, TLS 1.2 nimmt die gelistete ECDHE-ECDSA-Suite; eine Suite, die der Build nicht anbietet, und eine leere Liste `cipher_suites: []` verhindern den Start und `--check-config` (Exit 1). ALPN bietet `h2` und `http/1.1` an; `http/1.1` dient nur dem IDL-Endpunkt von §5.2. ALPN `h2` und die Rotation bei SIGHUP wie zuvor. Eine Prüfung mit `openssl s_client` ist nicht gelaufen.

### §7.2 Auth-Modes none/jwt/mtls

**Spec:** §7.2 — JWT-Bearer-Authorization-Metadata (RS256/ES256),
mTLS-Cert-DN, `none` nur auf einer Loopback-Adresse; §3
`auth.jwt.public_key`, `audience`, `required_claim`.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/calls.rs` (`Conn::on_open`:
Prüfung pro Call auf der `authorization`-Metadata, `UNAUTHENTICATED`),
`crates/grpc-bridge/src/bridge_daemon/auth.rs` (`security_from`,
`security_settings`: `none` auf einem `grpc.bind`, der keine
Loopback-Adresse ist, verhindert den Start und `--check-config`, Exit 1;
`apply_reload`: SIGHUP prüft `none` gegen den `grpc.bind`, auf dem der
Listener lauscht, ein geänderter Bind wartet auf den Neustart);
`crates/grpc-bridge/src/server.rs` (`GrpcCall::metadata`);
`crates/bridge-security/src/auth.rs` (`JwtKey` mit `Rs256` und `Es256`,
`jwt_key_from_pem`: ein RSA-Schlüssel, PKCS#1 oder SPKI rsaEncryption,
prüft RS256, ein SPKI id-ecPublicKey auf P-256 prüft ES256, eine andere
Kurve verhindert den Start; ein `alg` im Header, der nicht der des
Schlüssels ist, wird abgelehnt, `exp`/`nbf` mit 60 s Toleranz, `aud`,
geforderter Claim; aus `main` übernommen, der Mechanismus der
WebSocket-Bridge). Zusätzlich `bearer` mit
`auth.bearer_token`/`auth.bearer_subject`. Das mTLS-Subject ist der
Subject-DN des Zertifikats (RFC 4514, `CN=alice`); eine ACL-Regel darf
stattdessen den Fingerprint `mtls:<SHA-256>` nennen.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_7_2_bearer_and_acl_are_checked_per_call`,
`::grpcbridge_7_2_jwt_checks_signature_time_audience_and_scope`,
`::grpcbridge_7_2_jwt_es256_is_accepted`,
`::grpcbridge_7_2_mode_none_on_a_public_bind_does_not_start`,
`::grpcbridge_7_2_a_sighup_does_not_switch_none_on_for_the_served_public_bind`,
`::grpcbridge_7_1_mtls_serves_h2_and_the_certificate_is_the_subject`,
`::grpcbridge_7_2_jwt_without_a_key_and_mtls_without_tls_do_not_start`;
`crates/grpc-bridge/src/bridge_daemon/auth.rs::tests::grpcbridge_7_2_mode_none_only_on_a_loopback_bind`,
`::grpcbridge_7_2_a_reload_checks_none_against_the_served_bind`,
`::grpcbridge_12_1_auth_each_mode_builds_its_check`,
`::grpcbridge_12_1_auth_a_mode_without_what_it_needs_does_not_start`;
`crates/bridge-security/src/auth.rs::tests::grpcbridge_7_2_a_signed_jwt_is_checked_for_time_audience_and_claim`,
`::jwt_es256_is_verified`,
`::jwt_alg_must_match_the_key_type`,
`::jwt_es256_rfc_7515_a_3_example_verifies`.

**Status:** done — die ES256-Token der Tests signiert
`openssl dgst -sha256 -sign` (OpenSSL 3 auf dem Linux-Testrechner), der
Daemon prüft sie; ein RS256-Header über einem P-256-Schlüssel, ein
ES256-Header über einem RSA-Schlüssel und ein P-384-Schlüssel werden
abgelehnt. `auth.mode: none` mit TLS auf `0.0.0.0` verhindert den Start mit
Exit 1 und wird von `--check-config` abgelehnt; auf `127.0.0.1` startet er.
Ein Daemon auf `0.0.0.0` mit TLS und `bearer`, dessen Datei per SIGHUP den
Bind auf `127.0.0.1` verlegt und `none` setzt, lehnt das Neuladen ab
(`reload_refused`): ein Aufruf ohne Token bleibt `UNAUTHENTICATED`, der
Token gilt weiter. Vorher schaltete dieses Neuladen `none` für den weiter
gebundenen Listener auf `0.0.0.0` ein.

### §7.3 Per-Topic-ACL

**Spec:** §7.3 — `acl.publish`/`acl.subscribe` Listen mit Subject-
Resolution.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/dispatch.rs::route`
(`Publish` = Schreiben, `Subscribe` = Lesen auf dem DDS-Topic,
`PERMISSION_DENIED`); `topics[].acl` in `DaemonConfig::from_config`.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_7_2_bearer_and_acl_are_checked_per_call`,
`::grpcbridge_7_2_jwt_checks_signature_time_audience_and_scope` (JWT-`sub` als Subject),
`::grpcbridge_7_1_mtls_serves_h2_and_the_certificate_is_the_subject` (Zertifikat als Subject).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/log.rs` (`event`, `Line`,
`call_line`, `log_call`: ein JSON-Objekt pro Zeile auf stdout, zuerst
`timestamp` nach RFC 3339 in UTC mit Millisekunden, `level`, `event`; Text
nach RFC 8259 §7 maskiert, Zahlen als Zahlen; Schwelle `log_level`),
`crates/grpc-bridge/src/bridge_daemon/calls.rs::Conn::ended` (eine Zeile
`rpc` pro beendetem Call mit `peer`, `service`, `method`, `dds_topic`,
`bytes`, `grpc_status`, `latency_us`, `stream_id`; `info` bei OK, sonst
`warn` mit `grpc_message`; ein vom Client zurückgesetzter Call und einer,
dessen Verbindung vor dem Status endet, als CANCELLED),
`crates/grpc-bridge/src/bridge_daemon/catalog.rs::log_http` (Zeile `http`
des IDL-Endpunkts über HTTP/1.1 und h2),
`crates/grpc-bridge/src/bridge_daemon/connection.rs::serve` (`listening` mit
der Adresse, die der Listener gebunden hat; bei Port 0 in `grpc.bind` der
Port, den das System gewählt hat). Alle übrigen Ereignisse laufen über
denselben Weg.

**Tests:** `crates/grpc-bridge/src/bridge_daemon/log.rs::tests::grpcbridge_8_1_a_call_line_carries_the_fields_of_the_spec`,
`::grpcbridge_8_1_a_failed_call_is_warn_with_its_message`,
`::grpcbridge_8_1_values_are_json_escaped`,
`::grpcbridge_8_1_rfc3339_is_utc_with_milliseconds`,
`::grpcbridge_8_1_the_threshold_drops_lines_below_log_level`,
`::grpcbridge_8_1_numbers_are_json_numbers`;
`crates/grpc-bridge/tests/observability_e2e.rs::grpcbridge_8_1_every_call_is_one_line_on_stdout_with_the_fields_of_the_spec`,
`::grpcbridge_8_1_log_level_warn_hides_ok_calls_and_the_flag_wins_over_the_file`,
`::grpcbridge_8_1_a_reset_call_and_a_closed_connection_are_cancelled`,
`::grpcbridge_8_1_a_request_to_the_idl_endpoint_is_an_http_line`;
`crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_2_log_level_filters_the_lines`;
`crates/grpc-bridge/tests/daemon_limits_e2e.rs::grpcbridge_3_the_limits_of_the_file_reach_the_wire`
(Listener und Admin-Endpunkt auf Port 0, die Ports aus `listening` und
`admin_endpoint`).

**Status:** done — gegen den Daemon: ein Publish und ein Call auf einen
nicht gemappten Service ergeben genau zwei `rpc`-Zeilen auf stdout, jede
Zeile auf stdout beginnt mit einem RFC-3339-Zeitstempel in UTC; die
Publish-Zeile trägt den Peer des Clients, `dds_topic` `T::Open`,
`grpc_status` 0 und `bytes` = 13 Oktette der Anfrage plus die Länge des
PublishAck samt Präfix; die zweite Zeile ist `warn` mit `grpc_status` 5 und
`grpc_message`. stderr trägt keine Logzeile. Mit `--log-level warn` über
einer Datei mit `log_level: debug` fehlt die Zeile des OK-Calls, jede Zeile
ist `warn` oder `error`. Ein Publish, den der Client nach seinen HEADERS
mit RST_STREAM(CANCEL) zurücksetzt, und ein Subscribe, dessen Verbindung
der Client schließt, ergeben je eine `rpc`-Zeile auf `warn` mit
`grpc_status` 1, dem Peer und `dds_topic` `T::Open`
(`grpc_message` mit `RST_STREAM` bzw. `the connection closed`), und
`zerodds_grpc_bridge_rpcs_total{…,status="CANCELLED"}` ist für `Publish` und
`Subscribe` je 1. Ein `GET /idl/T::Open` über HTTP/1.1 und über h2 ergibt je
eine `http`-Zeile auf `info` mit Peer, `protocol` `http/1.1` bzw. `h2`,
`method` `GET`, `path`, `http_status` 200, `bytes` = Länge der IDL und
`latency_us`; ein `GET /idl/Nothing` ist `warn` mit `http_status` 404; keine
der Anfragen ergibt eine `rpc`-Zeile.
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + 10 Counter/Gauge-Familien.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/metrics.rs::SpecMetrics`
(alle zehn `zerodds_grpc_bridge_*`-Familien). Labelwerte nur aus der
Konfiguration oder einer festen Menge: ein nicht gemappter Service zählt als
`service="unknown"`, eine fremde Methode als `method="other"`.
`bytes_in`/`bytes_out` zählen die Length-Prefixed-Message samt 5-Oktett-
Präfix; `h2_resets_total` zählt empfangene RST_STREAM nach RFC-7540-§7-Namen
(der Daemon sendet keine). Die älteren `zerodds_grpc_*` aus
`crates/grpc-bridge/src/daemon_runtime.rs` bleiben daneben.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_8_2_the_metrics_of_section_8_2_count_calls_failures_and_resets`
(Status pro Call, Auth-Fehler nach Grund, RST_STREAM(CANCEL) und die beiden
zurückgesetzten Calls als `status="CANCELLED"`, mit `dds-runtime` Publish →
DDS → Subscribe);
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_8_2_admin_endpoint_serves_metrics_path`;
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_3_a_value_outside_its_range_is_refused`
und `crates/grpc-bridge/tests/daemon_cli_e2e.rs::grpcbridge_2_check_config_refuses_what_the_start_refuses`
(`metrics.listen`).

**Status:** done — `metrics.listen` und `--metrics` sind eine IP-Adresse mit Port (IPv6 in Klammern); ein Hostname wie `localhost:9095` verhindert den Start mit Exit 1 und dem Wert und wird von `--check-config` abgelehnt, aus der Datei und von `--metrics` (gegen das Binary auf dem Linux-Testrechner geprüft). Vorher startete der Daemon dann ohne Admin-Endpunkt und ohne Logzeile.

### §8.3 OTLP-Spans + W3C-Traceparent

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` + Trace-Propagation per
Traceparent-Header.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/trace.rs` (`Tracer`,
`spans`, `span_status`: ein Server-Span `<service>/<method>` pro Call mit
`rpc.system`, `rpc.service`, `rpc.method`, `rpc.grpc.status_code` (int),
`dds.topic`, `network.peer.address`, `network.peer.port` (int); Status
`Error` mit `grpc-message` nur für UNKNOWN, DEADLINE_EXCEEDED,
UNIMPLEMENTED, INTERNAL, UNAVAILABLE, DATA_LOSS nach den
OpenTelemetry-Konventionen für gRPC-Server; Kind `dds.publish` über die
DDS-Writes eines `Publish`/`PublishOne`, `dds.subscribe` von der Öffnung des
DataReaders bis zum Ende eines `Subscribe`-Streams, jeweils mit
`dds.samples` (int); `spawn_export_loop`: Export alle 5 s auf eigenem
Thread, der nach dem Stopp ohne weiteren Export endet; `finish`: der letzte
Export; `finish_by`: das Ende des Export-Threads und der letzte Export
zusammen höchstens bis `EXPORT_STOP_BUDGET` (5 s), danach eine Zeile
`otlp_export_failed`; `ExportFailures`: ein fehlgeschlagener Export ist eine Zeile
`otlp_export_failed` auf `warn` mit `err` und `failures`, höchstens eine pro
Minute, der letzte Export immer),
`crates/observability-otlp/src/lib.rs` (`IntAttribute`,
`OtlpExporter::add_span_with_ints`: Attribute mit `intValue`),
`crates/grpc-bridge/src/bridge_daemon/calls.rs::Conn::ended` (die Spans
jedes beendeten Calls), `crates/grpc-bridge/src/bridge_daemon/mod.rs::run`
(Exporter aus `OTEL_EXPORTER_OTLP_ENDPOINT`; ein Wert, den der Exporter
nicht bedienen kann, beendet den Start mit Exit 1, auch unter
`--check-config`; nach dem Drain `finish_by`), `crates/grpc-bridge/src/daemon_runtime.rs::otlp_config_from_env`,
`::otlp_config_from_endpoint` (nur `http://host[:port]` mit optionalem `/`,
ohne Port 80; `https`, Benutzerangaben, Pfad, Query, Fragment und ein Port
außerhalb 1 bis 65535 werden mit dem Wert abgelehnt; leer gilt als nicht
gesetzt; eine IPv6-Adresse bleibt in Klammern),
`crates/observability-otlp/src/lib.rs::OtlpExporter::connect` (löst den
Host ohne die Klammern einer IPv6-Adresse auf; das Feld `Host` schreibt sie
in Klammern). Den Traceparent-Teil führt §8.3a.

**Tests:** `crates/grpc-bridge/src/bridge_daemon/trace.rs::tests::grpcbridge_8_3_a_publish_is_a_server_span_with_a_dds_publish_child`,
`::grpcbridge_8_3_attributes_name_service_method_and_topic`,
`::grpcbridge_8_3_the_status_of_a_failed_call_is_error`,
`::grpcbridge_8_3_a_subscribe_stream_is_one_span_for_its_life`,
`::grpcbridge_8_3_failed_exports_are_reported_at_most_once_a_minute`,
`::grpcbridge_8_3_the_export_thread_ends_at_the_stop_without_exporting`,
`::grpcbridge_8_3_the_last_export_ends_within_the_stop_budget`;
`crates/grpc-bridge/tests/observability_e2e.rs::grpcbridge_8_3_the_daemon_posts_the_spans_of_a_call_to_the_otlp_endpoint`,
`::grpcbridge_8_3_an_otlp_endpoint_the_exporter_cannot_serve_stops_the_start`,
`::grpcbridge_8_3_without_an_endpoint_nothing_is_recorded`,
`::grpcbridge_8_3_a_failed_export_is_a_warn_line`,
`::grpcbridge_8_3_an_ipv6_endpoint_receives_the_spans`;
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_8_3_the_opentelemetry_collector_decodes_the_spans_of_a_call`
(Job `integration-bridges-vendor-grpc`);
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_8_3_otlp_config_parses_endpoint`,
`::grpcbridge_8_3_an_otlp_endpoint_the_exporter_cannot_serve_is_refused`;
`crates/observability-otlp/src/lib.rs::tests::traces_json_writes_int_attributes_as_int_value`,
`::flush_connects_within_the_timeout` (Linux),
`::flush_reaches_a_collector_at_an_ipv6_address`.

**Status:** done — gegen den Daemon mit einem Endpunkt des Tests: drei
Publish-Calls ergeben nach SIGTERM zwei Server-Spans
`zerodds.t.v1.OpenStream/Publish` und zwei `dds.publish` unter
`/v1/traces` (der dritte Call trägt einen nicht gesampelten
`traceparent`), mit `rpc.grpc.status_code`, `network.peer.port` und
`dds.samples` als `intValue`. `https://127.0.0.1:4318`,
`http://127.0.0.1:4318/otlp` und `http://127.0.0.1:43x8` beenden den Start
mit Exit 1 und dem Wert auf stderr, `--check-config` mit `https` meldet
`configuration invalid`. Ohne die Variable erreicht ein Listener auf
127.0.0.1:4318 nach Call und SIGTERM keine Verbindung, und es entsteht keine
Zeile `otlp` (hält ein anderer Prozess den Port, prüft der Test nur die
Zeilen und sagt das auf stderr; in den CI-Jobs und auf dem Testrechner hat
jeder Lauf einen eigenen Netz-Namespace). Der Verbindungsaufbau zum
Collector wartet höchstens das Timeout des Exporters (5 s; Unit-Test in
zerodds-observability-otlp gegen einen Listener mit voller Accept-Queue:
`TimedOut` nach 300 ms); das Ende des Herunterfahrens wartet auf den
laufenden Export und den letzten Export zusammen höchstens 5 s (Unit-Test
gegen einen Collector, der die Verbindung annimmt und nie antwortet: beide
Fälle enden an der Frist von 300 ms; ein Collector, der antwortet, bekommt
den POST auf `/v1/traces`). Ein Endpunkt, an dem
nichts lauscht, ergibt nach SIGTERM eine Zeile `otlp_export_failed` auf
`warn` mit `err` `io: …`; der Daemon endet mit 0. Mit
`OTEL_EXPORTER_OTLP_ENDPOINT=http://[::1]:<Port>` erreicht der Span eines
Calls nach SIGTERM einen Endpunkt des Tests auf der IPv6-Loopback-Adresse,
ohne Zeile `otlp_export_failed` (vor der Korrektur von
`OtlpExporter::connect` scheiterte jeder Export an der Auflösung von
`[::1]`; im Unit-Test erreichen `[::1]` und `::1` den Listener, das Feld
`Host` ist `[::1]`). Gegen den OpenTelemetry
Collector 0.115.0 (`otelcol`, Exporter `debug`): ein `PublishOne` von
grpcurl 1.9.3 mit `traceparent` erscheint dekodiert mit der Trace-ID und der
Parent-ID des Headers, Kind `Server`, `rpc.system: Str(grpc)`,
`rpc.method: Str(PublishOne)`, `rpc.grpc.status_code: Int(0)`,
`network.peer.port: Int(…)`, `dds.topic: Str(Trade::Trade)` und dem Kind
`dds.publish` mit `dds.samples: Int(1)`.

### §8.3a W3C-Traceparent-Parsing

**Spec:** §8.3 — ein eingehender `traceparent`-Header wird geparst und der
Span daran gehängt.

**Repo:** `crates/monitor/src/trace_context.rs` (`impl FromStr for
TraceParent`: W3C Trace Context §3.2; Version `00` genau
`00-<32 hex>-<16 hex>-<2 hex>` in Kleinbuchstaben-Hex, eine höhere Version
über die Felder von `00` mit Ende oder Bindestrich danach und nur dem
Sampled-Flag (§3.2.4, §4.3), Version `ff` ungültig, Null-IDs ungültig),
`crates/grpc-bridge/src/bridge_daemon/trace.rs::traceparent_of` (der Header
in jeder Schreibweise; keiner, zwei oder ein ungültiger: neuer Trace mit
zufälliger Trace-ID), `::sampled` (ohne Sampled-Flag kein Span), `::spans`
(der Server-Span ist Kind des Spans des Aufrufers). `tracestate` wird nicht
gelesen: der Daemon stellt keine HTTP-Anfrage weiter, und seine DDS-Writes
tragen keinen Trace-Kontext.

**Tests:** `crates/grpc-bridge/src/bridge_daemon/trace.rs::tests::grpcbridge_8_3_the_span_of_a_call_is_a_child_of_its_traceparent`,
`::grpcbridge_8_3_a_malformed_traceparent_starts_a_new_trace`,
`::grpcbridge_8_3_an_unsampled_traceparent_records_no_span`;
`crates/monitor/src/trace_context.rs::tests::w3c_trace_context_3_2_2_a_traceparent_header_is_read`,
`::w3c_trace_context_3_2_2_a_malformed_traceparent_is_refused`,
`::w3c_trace_context_3_2_4_a_higher_version_is_read_by_the_00_fields`;
`crates/grpc-bridge/tests/observability_e2e.rs::grpcbridge_8_3_the_daemon_posts_the_spans_of_a_call_to_the_otlp_endpoint`;
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_8_3_the_opentelemetry_collector_decodes_the_spans_of_a_call`
(Job `integration-bridges-vendor-grpc`).

**Status:** done — gegen den Daemon: der Call mit dem `traceparent` des
Beispiels aus W3C Trace Context §3.2.3 erscheint am Endpunkt mit dessen
Trace-ID in Server-Span und `dds.publish`, `parentSpanId` des Server-Spans
ist die Parent-ID des Headers; der Call ohne Header hat eine eigene
Trace-ID; vom nicht gesampelten Call kommt kein Span an. Der OpenTelemetry
Collector zeigt Trace-ID und Parent-ID des Headers.

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → TLS → DCPS → Reader/Writer + FileDescriptor →
HTTP/2-Bind → Signal-Handler.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/`,
`crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::grpcbridge_2_http2_roundtrip_publish_topic`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, GOAWAY, RPCs natürlich
beenden; SIGHUP TLS+ACL-Reload.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/connection.rs::handle_connection`
(jede Verbindung prüft das Stop-Flag alle 200 ms; beim Stop sofort
`GOAWAY(NO_ERROR)` mit der letzten vom Client geöffneten Stream-ID, die
Verbindung bleibt offen, die Calls bis zu dieser ID laufen weiter, die
Verbindung endet, wenn ihr letzter Call beendet ist und keine Antwort mehr
auf das Fenster des Clients wartet (`GrpcServer::active_streams` ist 0,
auch der Body eines GET `/idl/`); nach `grpc.drain_secs` enden die offenen
Calls mit UNAVAILABLE, was noch auf das Fenster wartet, wird nicht mehr
gesendet, Ereignis `drain_limit` mit `open_calls` und `open_streams`), `::serve` (die
Accept-Schleife endet und der Listen-Socket schließt beim Signal),
`crates/grpc-bridge/src/server.rs::GrpcServer::begin_drain` (ein Stream
nach dem GOAWAY bekommt `RST_STREAM(REFUSED_STREAM)`, RFC 9113 §6.8),
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs::BridgeDds::await_writes`
(wartet im Rest der Drain-Zeit, bis die zuverlässigen DDS-Reader alles
bestätigt haben, Ereignis `dds_writes_drained`), `::close` (löscht
DataWriter und DataReader, stoppt den Participant),
`crates/grpc-bridge/src/bridge_daemon/mod.rs::run` (Reihenfolge Drain,
Bestätigungen, Entitäten, der letzte OTLP-Export höchstens 5 s
(`Tracer::finish_by`, §8.3), Exit 0),
`crates/grpc-bridge/src/bridge_daemon/config.rs` (`grpc.drain_secs`, 0 bis
300, Default 30, Neustart nötig), `crates/grpc-bridge/src/bridge_daemon/auth.rs::apply_reload`
(SIGHUP: TLS-Material, Auth, ACL, die Auth-Regeln gegen die Werte, die
auf den Neustart warten; eine Datei, die nicht lädt oder so nicht bedient
werden kann, ändert nichts). systemd `TimeoutStopSec=40`, launchd `ExitTimeOut` 40, das
Dockerfile nennt `docker stop -t 40`.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_9_2_sigterm_sends_goaway_with_the_last_stream_and_exits_0`,
`::grpcbridge_9_2_a_stream_open_at_sigterm_keeps_delivering_until_the_client_ends_it`,
`::grpcbridge_9_2_after_the_drain_limit_open_streams_end_unavailable`,
`::grpcbridge_9_2_a_new_stream_after_goaway_is_refused`,
`::grpcbridge_9_2_a_sample_published_before_sigterm_reaches_the_dds_reader`,
`::grpcbridge_9_2_an_acknowledgement_outstanding_at_sigterm_holds_the_exit_to_the_drain_limit`,
`::grpcbridge_9_2_a_sighup_applies_the_new_token_and_a_broken_file_keeps_it`,
`::grpcbridge_7_2_a_sighup_does_not_switch_none_on_for_the_served_public_bind`,
`::grpcbridge_7_1_a_sighup_serves_the_next_connection_with_the_new_certificate`;
`crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_4_5_a_stream_open_at_drain_ends_unavailable`,
`::grpcbridge_9_2_answers_waiting_for_the_window_at_sigterm_go_out_whole`;
`crates/grpc-bridge/src/server/tests.rs::grpcbridge_9_2_begin_drain_serves_the_open_streams_and_refuses_new_ones`;
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs::tests::grpcbridge_9_2_await_writes_waits_for_the_acknowledgements`,
`::grpcbridge_9_2_close_deletes_the_entities`;
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_9_2_drain_secs_is_read`.

**Status:** done — ein Subscribe-Stream, der bei SIGTERM offen ist, liefert
nach dem GOAWAY noch die DDS-Probe, die danach geschrieben wird; der Daemon
endet mit 0, sobald der Client den Stream beendet. Ein Client mit
`SETTINGS_INITIAL_WINDOW_SIZE` 16 bekommt von einer `Catalog`-Antwort (der
Call ist beendet, die Trailer warten hinter der Nachricht) und vom Body
eines GET `/idl/` je 16 Oktette, dann bei SIGTERM das GOAWAY; nach seinem
WINDOW_UPDATE 600 ms später kommen beide vollständig an, der `Catalog` mit
`grpc-status` 0, dann schließt die Verbindung und der Daemon endet mit 0
ohne `drain_limit` (vor der Korrektur schloss die Verbindung mit dem
GOAWAY, der Client bekam eine abgeschnittene Nachricht ohne Trailer). Mit `drain_secs: 1`
endet ein offener Stream nach etwa 1 s mit `grpc-status` 14. Gegen den
Daemon: eine Probe, die ein Publish direkt vor SIGTERM schreibt, erreicht
den zuverlässigen DDS-Reader des Tests, der Daemon meldet
`dds_writes_drained` mit `acknowledged:true` und endet innerhalb von 10 s
(`drain_secs` ist 30 s; ob die Probe vor oder nach dem Signal ankommt, hält der
Test nicht fest). Steht die Bestätigung bei SIGTERM aus (der Reader des
Tests ist verbunden, sein Participant hält ohne Abmeldung an), hält der
Daemon den Exit bis zur Grenze `drain_secs: 2`, meldet `acknowledged:false`
und endet mit 0. Dass `await_writes` bis zur Bestätigung wartet, zeigt der
Unit-Test mit einem DDS-Writer und -Reader.

## §10 Cross-Vendor

### §10 RTPS-Peer + grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic

**Spec:** §10 — Daemon ist normaler RTPS-Peer; gRPC-Seite gegen
grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/dds_pump.rs` (die
Participants des Daemons in SPDP/SEDP; DataWriter und Stream-DataReader als
WITH_KEY- oder, mit `keyed: false`, als NO_KEY-Endpunkte, DDSI-RTPS 2.5
§9.3.1.2), `crates/grpc-bridge/src/bridge_daemon/config.rs` (Topic-Schlüssel
`keyed`, Default `true`), `crates/grpc-bridge/src/bridge_daemon/dispatch.rs::sample_to_publish`
(`key_hash` auf einem Topic ohne Schlüssel: INVALID_ARGUMENT),
`crates/grpc-bridge/tests/interop/cyclone_chat.c` und
`crates/grpc-bridge/tests/interop/chat.idl` (der Cyclone-DDS-Participant
des Tests); die gRPC-Clients siehe §10a.

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_a_grpcurl_publish_reaches_cyclone_and_a_cyclone_sample_a_grpcurl_subscribe`
(Job `integration-bridges-vendor-grpc`);
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs::tests::grpcbridge_10_the_endpoints_of_a_keyless_topic_are_no_key_endpoints`;
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_10_keyed_is_read_and_defaults_to_true`;
`crates/grpc-bridge/src/bridge_daemon/dispatch.rs::tests::grpcbridge_10_a_key_hash_on_a_keyless_topic_is_refused`;
`crates/grpc-bridge/tests/bridge_e2e.rs::grpcbridge_12_2_grpcurl_publish_reaches_the_dds_reader_byte_exact`,
`::grpcbridge_12_2_a_dds_sample_reaches_grpcurl_subscribe_byte_exact` (Job
`integration-bridges-vendor-grpc`);
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_grpcurl_list_services_against_zerodds_daemon`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_grpcurl_lists_the_reflection_service_itself` (Job `integration-bridges-vendor-grpc`),
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_list_services_reflection_payload_is_well_formed`;
`tests/integration/tests/bridges.rs::grpc_bridge_against_grpcio` (gRPC
C-core über Python grpcio, Job `integration-bridges`);
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_grpc_go_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_grpc_java_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_tonic_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_ghz_2000_publish_one_calls_all_ok_and_all_reach_dds`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_bloomrpc_calls_publish_one_and_catalog`
(Job `integration-bridges-vendor-grpc`, siehe §10a).

**Status:** done — die RTPS-Seite ist gegen Cyclone DDS 0.10.5 gemessen:
ein `PublishOne` von grpcurl 1.9.3 erreicht den DataReader von Cyclone als
genau das gesendete Sample, ein Sample des Cyclone-DataWriters erreicht
einen grpcurl-`Subscribe`-Stream mit den Bytes von Cyclone, `typeName` und
`topicName`; das gilt für den Typ mit Schlüssel und für den ohne
(`keyed: false`). Ohne `keyed: false` sah Cyclone die WITH_KEY-Endpunkte
des Daemons bei einem Typ ohne Schlüssel, verband sie aber nicht (gemessen
mit Cyclone-Tracing). gRPC-seitig sind grpcurl (grpc-go), grpcio (C-core),
grpc-go, grpc-java, tonic, ghz und BloomRPC auf dem Linux-Testrechner
gemessen (§10a).

### §10a Client-Matrix grpc-go/grpc-java/ghz/BloomRPC/tonic

**Spec:** §10 — der Daemon wird gegen grpc-go, grpc-java, ghz, BloomRPC und
tonic gefahren.

**Repo:** `crates/grpc-bridge/proto/zerodds/chat/v1/bridge.proto` (die
Definition von §4.2 für `Chat::Message`, wie `service_gen::render_proto`
sie schreibt; daraus erzeugen die Clients ihre Stubs),
`crates/grpc-bridge/tests/interop/build_peers.sh` (baut die drei Clients:
grpc-go aus Debians `golang-google-grpc-dev` 1.64 mit `protoc-gen-go` und
`protoc-gen-go-grpc` im GOPATH-Modus ohne Download, grpc-java aus Debians
`libgrpc-java` 1.41 mit Netty und dem grpc-java-Plugin von `protoc`, tonic
0.14 mit prost als eigenes Cargo-Projekt mit eigener Lock-Datei),
`crates/grpc-bridge/tests/interop/go/main.go`,
`crates/grpc-bridge/tests/interop/java/BridgeClient.java`,
`crates/grpc-bridge/tests/interop/tonic/src/main.rs` (die Clients:
`publish`, `publish-one`, `subscribe`, `catalog`, je eine Zeile pro
Ergebnis), `crates/grpc-bridge/tests/interop/bloomrpc_drive.py` (startet
BloomRPC 1.5.3 unter Xvfb mit importierter `.proto` und den Tabs
`PublishOne` und `Catalog`, drückt den Play-Knopf über das
DevTools-Protokoll, liest den Antwortbereich), `tests/integration/provision.sh`
(`protoc`, `grpc-go`, `grpc-java`, `ghz` 0.121.0 und BloomRPC 1.5.3 mit
sha256), `ci/integration_scenarios.py` (Job
`integration-bridges-vendor-grpc` mit dem Bauschritt).

**Tests:** `crates/grpc-bridge/src/service_gen.rs::tests::grpcbridge_4_2_the_shipped_proto_is_what_render_proto_writes`;
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_grpc_go_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_grpc_java_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_tonic_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_ghz_2000_publish_one_calls_all_ok_and_all_reach_dds`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_bloomrpc_calls_publish_one_and_catalog`
(Job `integration-bridges-vendor-grpc`).

**Status:** done — auf dem Linux-Testrechner gemessen: grpc-go, grpc-java
und tonic schicken je einen Client-Stream `Publish` mit zwei Samples und
ein `PublishOne`, die beim DDS-DataReader des Tests Oktett für Oktett
ankommen (Encapsulation-Header als Representation Identifier), die
`PublishAck`-Sequenznummern 2 und 3; ein `PublishOne` ohne
Encapsulation-Header endet bei allen drei mit Status 3 und der
`grpc-message` des Daemons; `Catalog` nennt Topic, Typ, vollen
Servicenamen und die `idl_url` der Authority des Clients; ein
`Subscribe`-Stream bekommt die zwei Samples eines DDS-DataWriters des Tests
mit dem Header `CDR2_BE`. ghz 0.121.0: 2000 `PublishOne`, 20 gleichzeitig
über zwei Verbindungen, alle OK, alle 2000 Samples beim DDS-DataReader.
BloomRPC 1.5.3 (Electron, gRPC C-core über das Node-Paket `grpc` 1.24):
das `PublishOne` kommt beim DDS-DataReader an, der Antwortbereich zeigt
den `PublishAck` mit Sequenznummer 1 und den `CatalogResp` des Topics.

---

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-grpc-bridged`; Configs/Services/Docker;
Manuals.

**Repo:** `packaging/linux/systemd/zerodds-grpc-bridged.service`
(`Type=notify`, `TimeoutStopSec=40` über dem Drain von §9.2),
`packaging/macos/launchd/org.zerodds.grpc-bridged.plist` (`ExitTimeOut`
40), `packaging/macos/homebrew/zerodds-grpc-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/windows/msi/zerodds.wxs` (Dienst `ZeroDDSGrpcBridge`),
`packaging/docker/grpc-bridged/` (`STOPSIGNAL SIGTERM`, nennt
`docker stop -t 40`), `packaging/linux/configs/grpc-bridged.yaml.example`,
`man/man1/zerodds-grpc-bridged.1` (WINDOWS SERVICE),
`man/man5/zerodds-grpc-bridged.yaml.5`;
der Windows-Dienst: `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`
(`main` startet den Daemon über
`zerodds_dcps::cli::run_as_service(WINDOWS_SERVICE, …)`),
`crates/grpc-bridge/src/bridge_daemon/mod.rs` (`WINDOWS_SERVICE` =
`ZeroDDSGrpcBridge`), `crates/grpc-bridge/src/daemon_runtime.rs`
(`install_signal_watcher` außerhalb von Unix: `watch_service_requests`
setzt bei einer Stopp-Anfrage das Shutdown-Flag von SIGTERM, also den Drain
von §9.2, bei `paramchange` das Reload-Flag von SIGHUP),
`crates/dcps/src/cli.rs` (`run_as_service`, `request_service_stop`,
`request_service_reload`, `READY=1` und `STOPPING=1` als Dienstzustände) und
`crates/dcps/src/cli/windows_service.rs` (`StartServiceCtrlDispatcherW`,
`RegisterServiceCtrlHandlerExW`, `SetServiceStatus`, der Konsolen-Handler),
unverändert aus dem Dienst-Dispatcher des WebSocket-Daemons. Vorher hatte
das Binary keinen Service-Control-Dispatcher, und der Watcher außerhalb von
Unix war ein leerer Thread. Die Spec §11 nennt den Dienstnamen der
Installer (vorher `ZeroDDSGRPCBridge`).

**Tests:** `crates/grpc-bridge/src/bridge_daemon/config.rs::tests::{grpcbridge_3_example_yaml_parses_without_error, grpcbridge_3_example_yaml_key_fields_match_intent, grpcbridge_3_example_yaml_topics_reach_the_daemon, grpcbridge_3_example_yaml_has_no_unknown_top_level_keys}`,
`crates/config/tests/layering.rs::a_hash_inside_quotes_is_part_of_the_value`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel);
`crates/grpc-bridge/src/bridge_daemon/mod.rs::packaging_tests::{grpcbridge_11_the_windows_service_is_the_one_the_installers_register, grpcbridge_11_the_service_managers_wait_longer_than_the_drain, grpcbridge_11_the_service_files_start_the_binary_with_its_configuration}`
(Dienstname in Binary, PowerShell-Installer, MSI und Spec; systemd, launchd
und Docker stoppen mit SIGTERM und warten länger als der Drain; jede
Dienstdatei startet das Binary mit der Konfiguration ihrer Plattform);
`tools/traceability/tests/deployment_layout.rs::{every_systemd_unit_carries_the_hardening_set_of_the_clause, every_launchd_plist_carries_the_keys_of_the_clause, windows_service_scripts_register_and_remove_the_same_bridge_set, every_daemon_image_is_cargo_chef_multi_stage_and_runs_non_root, msi_defines_one_optional_feature_per_bridge}`;
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_11_the_service_requests_set_the_shutdown_and_reload_flags`
(der Watcher von Windows in einem Thread unter Linux: ohne Anfrage wartet
er, er endet mit dem Daemon, eine Reload-Anfrage setzt das Reload-Flag,
eine Stopp-Anfrage das Shutdown-Flag);
`crates/dcps/src/cli.rs::service_tests::{deployment_4_3_the_service_controls_are_stop_reload_and_interrogate, deployment_4_3_the_notify_states_are_service_states, deployment_4_3_the_exit_code_has_its_number, deployment_4_3_the_requests_and_the_run}`;
`crates/dcps/src/cli/windows_service.rs::tests::deployment_4_3_the_control_codes_are_the_win32_constants`
(nur Windows).

**Status:** partial — die Tests liefen grün auf dem Linux-Testrechner; der
Windows-Code wurde dort mit `cargo clippy --target x86_64-pc-windows-gnu
--all-targets` übersetzt, nicht ausgeführt. Der wöchentliche Job
`platform-windows-grpc-bridge-build` (`ci/platform_jobs.py`
`WINDOWS_BUILD`, `ci/platforms.yml`) baut alle Targets von
`zerodds-grpc-bridge` auf dem Windows-Runner, darunter den Watcher des
Dienstes und den Aufruf von `run_as_service`; er ist noch nicht gelaufen.
Es fehlt der Lauf des Dienstes
`ZeroDDSGrpcBridge` unter `sc.exe` (Start, `READY=1` als
`SERVICE_RUNNING`, Stopp mit dem Drain, `paramchange` als Reload); er
braucht einen Windows-Rechner mit dem registrierten Dienst.

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/http2_codec/hpack/grpc_status/reflection/
dds_pump/auth je ≥ 5 Tests.

**Repo:** die Module der Spec und ihre Dateien: `config` =
`crates/grpc-bridge/src/bridge_daemon/config.rs`; `http2_codec` =
`crates/http2/src/` und `crates/grpc-bridge/src/{frame.rs,server.rs}`;
`hpack` = `crates/hpack/src/`; `grpc_status` =
`crates/grpc-bridge/src/status.rs`; `reflection` =
`crates/grpc-bridge/src/{reflection.rs,descriptor.rs}`; `dds_pump` =
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs`; `auth` =
`crates/grpc-bridge/src/bridge_daemon/auth.rs` und
`crates/grpc-bridge/src/bridge_security.rs`. Der Daemon liegt seit der
Aufteilung des Binarys in Modulen unter `crates/grpc-bridge/src/bridge_daemon/`
(`config`, `connection`, `calls`, `dispatch`, `dds_pump`, `auth`,
`catalog`, `metrics`); das Binary behält `main`.

**Tests:** Inline-`#[cfg(test)]`-Module, je Modul der Spec mit der Zahl der Tests:
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_3_parse_minimal_grpc_config` (25),
`crates/grpc-bridge/src/bridge_daemon/config.rs::metrics_default_tests::the_metrics_default_is_the_one_man5_names` (1);
`crates/http2/src/frame.rs::tests::from_u8_round_trip` (9),
`crates/http2/src/flow.rs::tests::consume_reduces_window` (10),
`crates/http2/src/stream.rs::tests::idle_to_open_via_headers` (10),
`crates/http2/src/settings.rs::tests::apply_valid_settings` (8),
`crates/http2/src/preface.rs::tests::correct_preface_passes` (5),
`crates/http2/src/error.rs::tests::from_u32_round_trip` (3),
`crates/grpc-bridge/src/frame.rs::tests::grpc_requests_a_length_prefixed_message_is_read_from_its_octets` (9),
`crates/grpc-bridge/src/server/tests.rs::grpcbridge_12_1_encode_response_includes_status_and_trailers` (32);
`crates/hpack/src/decoder.rs::tests::round_trip_static_match` (8),
`crates/hpack/src/encoder.rs::tests::full_static_match_is_single_byte` (7),
`crates/hpack/src/huffman.rs::tests::round_trip_ascii` (6),
`crates/hpack/src/integer.rs::tests::rfc7541_c1_1_encoded_10_in_5bit_prefix` (8),
`crates/hpack/src/string.rs::tests::round_trip_plain` (7),
`crates/hpack/src/table.rs::tests::static_table_has_61_entries` (13);
`crates/grpc-bridge/src/status.rs::tests::grpc_responses_the_status_codes_are_the_table_of_seventeen`,
`::grpcbridge_12_1_status_code_and_from_code_are_inverse`,
`::grpcbridge_12_1_status_names_are_distinct_label_values` (5);
`crates/grpc-bridge/src/reflection.rs::tests::grpcbridge_4_6_a_request_is_read_by_its_kind` (7),
`crates/grpc-bridge/src/descriptor.rs::tests::grpcbridge_4_6_the_file_descriptor_is_read_back` (3);
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs::tests::grpcbridge_12_1_dds_pump_the_entities_follow_the_direction`,
`::grpcbridge_12_1_dds_pump_the_encapsulation_reaches_the_reader`,
`::grpcbridge_9_2_await_writes_waits_for_the_acknowledgements`,
`::grpcbridge_9_2_close_deletes_the_entities`,
`::grpcbridge_4_5_a_full_writer_history_is_resource_exhausted` (10);
`crates/grpc-bridge/src/bridge_daemon/auth.rs::tests::grpcbridge_7_2_mode_none_only_on_a_loopback_bind`,
`::grpcbridge_12_1_auth_each_mode_builds_its_check`,
`::grpcbridge_12_1_auth_a_mode_without_what_it_needs_does_not_start`,
`::grpcbridge_12_1_auth_a_reload_that_fails_keeps_the_live_settings`,
`::grpcbridge_7_2_a_reload_checks_none_against_the_served_bind`,
`::grpcbridge_1_l6_the_acl_of_a_tenant_topic_carries_its_name` (6),
`crates/grpc-bridge/src/bridge_security.rs::tests::grpc_bearer_in_metadata` (4);
dazu `crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_base64_reads_the_vectors_of_rfc_4648` (18),
`crates/grpc-bridge/src/path.rs::tests::grpc_requests_parses_standard_grpc_path` (7),
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_the_timeout_units_are_h_m_s_m_u_n` (10)
und `crates/grpc-bridge/tests/fuzz_smoke.rs` (`grpcbridge_12_1_fuzz_decode_message_no_panic`,
`grpcbridge_12_1_fuzz_parse_path_no_panic`, `grpcbridge_12_1_fuzz_decode_timeout_no_panic`,
`grpcbridge_12_1_empty_inputs_no_panic`, `grpcbridge_12_1_message_oversized_length_no_oom`).

**Status:** done — jedes der sieben Module hat mindestens fünf Tests; die
`dds_pump`-Tests laufen mit einem zweiten Participant auf einer geliehenen
Domain. Gezählt mit `#[test]` je Datei, grün auf dem Linux-Testrechner.

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, grpcurl als Client, byte-genauer
Roundtrip + Reflection.

**Repo:** `crates/grpc-bridge/tests/bridge_e2e.rs` (der Daemon als
Subprozess auf einer geliehenen Domain, `grpcurl` als Client allein mit den
Deskriptoren des Reflection-Service, ein DDS-DataReader oder -DataWriter im
Testprozess), `ci/integration_scenarios.py` (Job
`integration-bridges-vendor-grpc`, Schritt `bridge_e2e` mit `--run-ignored`).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::grpcbridge_12_2_grpcurl_publish_reaches_the_dds_reader_byte_exact`
(ein Client-Streaming-`Publish` mit zwei Samples, je 256 Byte mit jedem
Oktettwert: der DDS-Reader erhält Repräsentation `CDR2_LE` und die
CDR-Bytes unverändert, das PublishAck nennt Sequenznummer 2),
`::grpcbridge_12_2_a_dds_sample_reaches_grpcurl_subscribe_byte_exact`
(eine Probe eines DataWriters des Tests mit XCDR2 Big Endian kommt bei
grpcurls `Subscribe` als `Sample.cdr_payload` mit `CDR2_BE`-Header und denselben
Bytes an, mit Topic- und Typname),
`::grpcbridge_12_2_grpcurl_describe_round_trips_the_service_through_reflection`
(grpcurl speichert die Deskriptoren des Reflection-Service mit
`-protoset-out`; ein Aufruf allein aus dieser Datei erreicht einen Daemon
ohne Reflection, dessen `list` grpcurl nicht lesen kann, und der DDS-Reader
erhält die Bytes),
`::grpcbridge_2_http2_roundtrip_publish_topic`,
`::grpcbridge_2_http2_unknown_service_yields_status_5` (eigener
HTTP/2-Client, Crate-Ebene);
`tests/integration/tests/bridges.rs::grpc_bridge_against_grpcio` (grpcio
mit einem DDS-Peer, Job `integration-bridges`).

**Status:** done — die drei grpcurl-Tests liefen mit grpcurl 1.9.3 auf dem
Linux-Testrechner grün (`cargo nextest run -p zerodds-grpc-bridge --test
bridge_e2e --run-ignored all`: 5 von 5).

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — tonic-Client + Cyclone-DDS-Subscriber + ZeroDDS-
gRPC-Bridge im Compose.

**Repo:** `crates/grpc-bridge/tests/compose/docker-compose.yml` (die
Services `bridge` aus `packaging/docker/grpc-bridged/Dockerfile` mit
`tests/compose/grpc-bridged.yaml`, `cyclone` aus
`tests/compose/cyclone.Dockerfile`, `client` aus
`tests/compose/tonic.Dockerfile` im Netzwerk-Namespace der Bridge; der
Exit-Code des Clients ist das Ergebnis),
`crates/grpc-bridge/tests/interop/cyclone_chat.c` (Modus `echo`: beantwortet
jede `Chat::Message` mit `{ seq, "echo: " text }`),
`crates/grpc-bridge/tests/interop/tonic/src/main.rs` (Modus `echo`:
`Subscribe`-Stream, dann `PublishOne` einmal pro Sekunde bis zur Antwort
von Cyclone), `crates/grpc-bridge/tests/compose_e2e.rs`,
`ci/manual-tests.tsv`.

**Tests:** `crates/grpc-bridge/tests/compose_e2e.rs::grpcbridge_12_3_the_compose_file_runs_bridge_cyclone_and_tonic_and_the_answer_comes_back`
(Handtest auf dem Linux-Testrechner nach Entscheidung 11, `ci/manual-tests.tsv`:
die GitLab-Runner haben keinen Docker-Daemon);
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_12_3_a_tonic_publish_reaches_cyclone_and_a_cyclone_sample_a_tonic_subscribe`
(dieselben drei als Prozesse, Job `integration-bridges-vendor-grpc`).

**Status:** done — auf dem Linux-Testrechner gemessen: `docker compose up`
baut die drei Images und startet sie; das `PublishOne` des tonic-Clients
mit `{ 7, "hello from compose" }` erreicht den DataReader von Cyclone DDS
0.10.5, dessen Antwort `{ 7, "echo: hello from compose" }` kommt über den
DataReader der Bridge auf dem `Subscribe`-Stream des Clients an, der Client
endet mit 0. Als Prozesse: ein `Publish` von tonic erreicht den
DataReader von Cyclone, ein Sample des Cyclone-DataWriters einen
`Subscribe`-Stream von tonic.

## §13 Cross-References

### §13 Verwandte Library + Standards + Daemons

**Spec:** §13 — Library `crates/grpc-bridge/`/`crates/http2/`/
`crates/hpack/`, gRPC-PROTOCOL.md, RFC 7540/7541, Wire-Format,
Deployment, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config/.proto, Major=
Wire-Protocol-Change (HTTP/3 + gRPC-over-QUIC).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Regel für die Vergabe der Versionsnummer eines Release, keine Eigenschaft des Daemons.

---

## Audit-Status

26 done / 1 partial / 0 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-grpc-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (Job `merge-rust-bridge-security`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (Job `merge-rust-config`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-grpc-bridge --all-targets` (Job `merge-rust-grpc-bridge`) — 59 Tests grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 4 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor` (Job `integration-bridges-vendor`) — 3 Tests grün.

68 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-grpc-bridge-1.0.open.md`.
