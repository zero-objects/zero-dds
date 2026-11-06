# `zerodds-opcua-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-opcua-bridge-1.0.md`

Implementation:

- `crates/opcua-bridge/` — der Daemon-Prozess (`zerodds-opcua-bridged`) über
  den vier Bibliothekscrates `opcua-gateway`, `opcua-uacp`, `opcua-server`,
  `opcua-pubsub`.

## §1 Conformance-Levels

### §1 Vier Gateway-1.0-Conformance-Points deklariert

**Spec:** §1 — alle vier Conformance-Points aus Gateway 1.0 §2 auf
Bibliotheksebene deklariert; im Daemon verdrahtet: §9 (DDS → OPC UA), §8
(OPC UA → DDS, eigener Server und Client-Rolle), Part-14-UADP in beide
Richtungen, Historical Access (§9.3.4, Point 4 „complete“).

**Repo:** `crates/opcua-gateway/src/conformance.rs::DECLARED_CONFORMANCE`;
`crates/opcua-bridge/src/daemon/server.rs::start_with_reload`;
`crates/opcua-bridge/src/ua_client.rs::ClientJob`;
`crates/opcua-bridge/src/pubsub_glue.rs::{PublisherJob, SubscriberJob}`.

**Tests:** `crates/opcua-gateway/src/conformance.rs::tests::{declares_all_four_conformance_points, complete_points_subsume_their_basic}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_1_dds_publish_reaches_an_in_tree_pubsub_subscriber, opcuabridge_1_external_pubsub_publisher_reaches_the_daemons_own_subscriber, opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end}`;
`crates/opcua-bridge/src/ua_client.rs::tests::opcuabridge_1_client_job_writes_a_monitored_scalar_node_into_dds`.

**Status:** done

## §2 CLI-Surface

### §2 zerodds-opcua-bridged CLI

**Spec:** §2 — `--config`/`--check-config` (Exit 0 gültig, 1 ungültig,
2 unlesbar)/`--set`/`--bind`/`--domain`/`--idl` (wiederholbar)/
`--gateway-config`/`--log-level`/`--metrics`/`--version`/`--help`; jedes
Komfort-Flag ist `--set` seines Dateischlüssels und gewinnt über die Dateien.

**Repo:** `crates/opcua-bridge/src/bin/zerodds-opcua-bridged.rs`,
`crates/opcua-bridge/src/daemon/cli.rs`.

**Tests:** `crates/opcua-bridge/src/daemon/cli.rs::tests::{opcuabridge_2_empty_args_default, opcuabridge_2_bind_addr_parsed, opcuabridge_2_equals_form, opcuabridge_2_domain_parses_int, opcuabridge_2_idl_is_repeatable, opcuabridge_2_unknown_flag_rejected, opcuabridge_2_sets_render_idl_as_appends, opcuabridge_2_help_short_form}`.

**Status:** partial — geprüft sind `--bind` (auch in `=`-Form), `--domain`,
wiederholtes `--idl` und seine Darstellung als `+opcua.idl=…`, `-h` und die
Ablehnung eines unbekannten Flags. Ohne Test: die Exit-Codes von
`--check-config`, `--config`, `--gateway-config`, `--log-level`,
`--metrics`, `--version` und der Vorrang von `--set` vor den Dateien.

## §3 Config-File-Format

### §3 YAML-Loader, geschichtet

**Spec:** §3 — YAML, geschichtet (Defaults < Umgebung < Dateien
`opcua-bridged.yaml` + `opcua-bridged.d/*.yaml` < `--set`); Schlüssel
`domain`/`log_level`/`opcua.{idl,gateway_config,server,client,pubsub}`/
`acl`/`metrics`; ein unbekannter Schlüssel oder ein Wert falscher Form stoppt
den Start mit Datei und Zeile.

**Repo:** `crates/opcua-bridge/src/daemon/config.rs::{DaemonConfig, KEYS,
is_key, from_config}` über den gemeinsamen Loader `zerodds-config`.

**Tests:** `crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_parses_minimal_config, opcuabridge_3_rejects_unknown_top_level_section_key, opcuabridge_3_client_server_entry_needs_endpoint_url, opcuabridge_3_parses_a_full_client_server_entry, opcuabridge_3_parses_a_full_pubsub_connection, opcuabridge_3_pubsub_security_parses_and_hex_decodes_the_keys, opcuabridge_3_otlp_endpoint_parses, opcuabridge_3_historical_topics_parse, opcuabridge_3_acl_users_parse_with_default_policy_id, opcuabridge_3_acl_rules_parse}`;
`crates/opcua-bridge/tests/example_yaml_loadback.rs::opcuabridge_3_shipped_example_config_loads_back`.

**Status:** done

## §4 Wire-Protokolle

### §4.1 opc.tcp + SecureChannel

**Spec:** §4.1 — Part 6 §7.1/§6.7; SecurityMode `None` immer; mit Feature
`crypto` `basic256sha256`/`aes128_sha256_rsaoaep`/`aes256_sha256_rsapss`.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{accept_loop,
serve_connection_locked, build_server_security}`;
`crates/opcua-uacp/src/crypto.rs`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_4_1_secured_basic256sha256_sign_and_encrypt_session_through_the_daemon}`
(SecurityMode `None` und Basic256Sha256 SignAndEncrypt über einen echten
`opc.tcp`-Socket gegen den laufenden Daemon);
`crates/opcua-uacp/src/crypto.rs::tests::{uacs_6_7_2_an_open_secure_channel_chunk_is_the_layout_of_figure_13, uacs_6_7_2_an_open_secure_channel_chunk_is_the_layout_of_figure_13, uacs_6_7_full_secured_handshake_flow}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::opcuabridge_4_1_unknown_security_policy_is_refused`.

**Status:** done

### §4.2 Part-4-Services

**Spec:** §4.2 — Session, Read/Write, Browse, Call,
CreateSubscription/CreateMonitoredItems/Publish und HistoryRead aus
`zerodds-opcua-server`; ein `Server` pro Prozess für alle Verbindungen;
`Server::set_write_hook` bietet jedes `Write`-Ziel mit der Session-Identität
an, bevor es das AddressSpace erreicht.

**Repo:** `crates/opcua-server/src/server.rs::{Server::set_write_hook,
handle_service}`; `crates/opcua-bridge/src/daemon/server.rs::start_with_reload`
(ein `Arc<Mutex<Server>>`, Hook-Installation).

**Tests:** `crates/opcua-server/src/client.rs::tests::{opcuabridge_4_2_write_hook_can_refuse_a_write_before_it_reaches_the_address_space, opcuabridge_4_2_write_hook_observes_the_calling_sessions_identity}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other, opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end}`.
Die Bibliotheks-Services selbst stehen in `opcua-client-server-1.05.md`.

**Status:** done

### §4.3 Part-14 UADP

**Spec:** §4.3 — ein Publisher/Subscriber-Paar pro
`opcua.pubsub.connections[]` über UDP; Message-Security (Feature
`pubsub-security`) mit einem statischen Schlüssel aus
`opcua.pubsub.connections[].security`; ein Schlüssel, dessen Länge nicht zur
Policy passt, wird beim Laden abgelehnt.

**Repo:** `crates/opcua-bridge/src/pubsub_glue.rs::{PublisherJob,
SubscriberJob, set_security}`; `crates/opcua-bridge/src/daemon/config.rs::PubSubSecurityConfig`;
`crates/opcua-bridge/src/daemon/server.rs::build_pubsub_security`.

**Tests:** `crates/opcua-bridge/src/pubsub_glue.rs::tests::{opcuabridge_4_3_a_scalar_topic_is_the_field_value, opcuabridge_4_3_publisher_job_builds_a_pds_field_per_leaf, opcuabridge_4_3_a_secured_publisher_job_delivers_a_dds_sample_to_a_subscriber_job, opcuabridge_4_3_subscriber_with_the_wrong_key_refuses_not_silently_drops}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_pubsub_security_parses_and_hex_decodes_the_keys, opcuabridge_4_3_pubsub_security_with_wrong_key_length_is_refused, opcuabridge_4_3_pubsub_security_odd_length_hex_is_refused, opcuabridge_4_3_pubsub_security_unknown_policy_is_refused, opcuabridge_4_3_no_security_key_leaves_it_none}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dds_publish_reaches_an_in_tree_pubsub_subscriber, opcuabridge_1_external_pubsub_publisher_reaches_the_daemons_own_subscriber}`.

**Status:** partial — Publisher/Subscriber in beide Richtungen und der
statische Schlüssel sind gebaut und geprüft; die Schlüsselrotation über
`GetSecurityKeys`/SKS fehlt (§4.3.1).

### §4.3.1 SKS-Schlüsselrotation

**Spec:** §4.3, §14 — Message-Security mit einem statischen aktuellen
Schlüssel, „not a live `GetSecurityKeys`/SKS rotation exchange“; §14 führt
das als offen.

**Repo:** —

**Tests:** —

**Status:** open — der Daemon holt keine Schlüssel über `GetSecurityKeys`
und rotiert sie nicht; ein neuer Schlüssel braucht einen Neustart (§9.3).

## §5 Topic-Mapping

### §5.1 TypeObject/TypeLookup → SerializedTypeSupport

**Spec:** §5.1 — Typauflösung ohne kompilierten Rust-Typ: `opcua.idl` →
`CompleteTypeObject` → `SerializedTypeSupport`; zweiter Pfad
`dds_bridge::resolve_discovered` über die TypeLookup-Registry der Runtime.

**Repo:** `crates/opcua-bridge/src/idl.rs::{compile, type_support_for,
is_keyed}`; `crates/opcua-bridge/src/dds_bridge.rs::resolve_discovered`.

**Tests:** `crates/opcua-bridge/src/idl.rs::tests::{opcuabridge_5_1_compiles_a_struct_and_resolves_its_type_support, opcuabridge_5_1_unknown_type_is_reported_by_name, opcuabridge_5_1_broken_idl_is_reported_with_diagnostics}`.

**Status:** partial — der IDL-Pfad ist geprüft; `resolve_discovered` hat
keinen Test; die eigene `getTypes`-Abfrage fehlt (§5.1.1).

### §5.1.1 getTypes-Abfrage für einen unbekannten Typ

**Spec:** §5.1 — „This daemon does **not** itself drive a live
cross-participant `getTypes` RPC exchange for a topic whose type is
otherwise unknown.“

**Repo:** —

**Tests:** —

**Status:** open — ein Topic, dessen Typ weder aus `opcua.idl` noch aus der
Registry der Runtime kommt, wird nicht gebrückt; der Daemon fragt den Typ
nicht über TypeLookup an.

### §5.2 AddressSpace-Form (§9.3)

**Spec:** §5.2 — `Domain<id>` → `Topic:<name>` → Sample-Baum unter
`Topic:<name>/Value[/<Pfad>]`; `<node_id>`/`<browse_path>`-Override aus der
§10-XML wird angewandt, eine Kollision oder ein leeres Pfadsegment ist ein
Ladezeit-Fehler; `dds_to_ua`-Leaf nur `CurrentRead`, `ua_to_dds`-Leaf
`CurrentRead | CurrentWrite`.

**Repo:** `crates/opcua-bridge/src/address_space_build.rs::{ensure_domain,
ensure_topic, ensure_objects_folder, materialize_instance,
materialize_instance_with_override, ensure_browse_path, apply_sample}`.

**Tests:** `crates/opcua-bridge/src/address_space_build.rs::tests::{opcuabridge_5_2_scalar_topic_gets_one_leaf_at_the_empty_path, opcuabridge_5_2_struct_topic_gets_one_leaf_per_member, opcuabridge_5_2_objects_folder_is_browsable_after_materializing_a_topic, opcuabridge_5_2_two_topics_do_not_collide, opcuabridge_5_2_node_id_override_replaces_the_default_root_identity, opcuabridge_5_2_node_id_override_colliding_with_an_existing_node_is_refused, opcuabridge_5_2_empty_browse_path_segment_is_refused, opcuabridge_5_2_browse_path_organizes_the_root_from_the_objects_folder, opcuabridge_5_2_dds_to_ua_leaf_is_read_only_ua_to_dds_leaf_is_writable}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_5_2_node_id_and_browse_path_override_is_applied_not_just_parsed, opcuabridge_5_2_colliding_node_id_override_is_a_load_time_error_not_a_log_line}`.

**Status:** done

### §5.3 Typformen-Abdeckung

**Spec:** §5.3 — Primitive, Strings (narrow und wide), Enums (Ordinalwert),
Bitmasks, verschachtelte Structs, `map<K,V>` (ein Leaf pro Eintrag,
`<feld>.<schlüssel>`) und Arrays/Sequences davon in beide Richtungen; nicht
abbildbare Formen werden mit Pfad gemeldet, nie still verworfen.

**Repo:** `crates/opcua-bridge/src/dynamic_value.rs::{flatten, unflatten}`;
`crates/opcua-bridge/src/dynamic_type.rs`.

**Tests:** `crates/opcua-bridge/src/dynamic_value.rs::tests::{opcuabridge_5_3_a_flat_struct_is_one_variable_per_member_and_back, opcuabridge_5_3_nested_struct_flattens_with_dotted_paths, opcuabridge_5_3_partial_unflatten_keeps_the_rest_of_the_default, opcuabridge_5_3_enum_flattens_to_its_ordinal_and_back, opcuabridge_5_3_sequence_of_primitives_is_one_variant, opcuabridge_5_3_sequence_of_struct_is_reported_skipped_not_dropped_silently, opcuabridge_5_3_wide_char_maps_to_uint16_and_back, opcuabridge_5_3_wide_string_maps_to_string_and_back, opcuabridge_5_3_map_of_primitives_decomposes_to_one_leaf_per_key_and_back, opcuabridge_5_3_map_partial_write_keeps_the_untouched_key, opcuabridge_5_3_a_value_nested_past_the_bound_is_refused_not_walked}`;
`crates/opcua-bridge/src/dynamic_type.rs::tests::opcuabridge_5_3_fixed_point_type_is_a_named_unmapped_error`;
`crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_5_3_unmatched_path_is_reported_not_dropped`.

**Status:** partial — die genannten Formen sind abgebildet und geprüft;
Array/Sequence von Struct/Union, `bitset`, `fixed<P,S>` und `float128`
haben keinen Live-Wert (§5.3.1); ein neuer `map`-Schlüssel wird nicht als
Knoten angelegt (§5.3.2).

### §5.3.1 Live-Wert für Array/Sequence von Struct/Union, bitset, fixed, float128

**Spec:** §5.3 — diese Formen bekommen keinen Live-Wert, nur einen
Knoten ohne Wert; DDS-OPCUA 1.0 §9.2.5 bildet Arrays und Sequences von
Structures und Unions auf je eine Array-Variable ab (Figure 9.19, 9.20,
9.25, 9.26).

**Repo:** —

**Tests:** —

**Status:** open — `dynamic_value.rs::flatten` meldet diese Formen mit
Pfad und lässt den Knoten ohne Wert; es fehlt die Abbildung auf eine
Array-Variable bzw. einen passenden Wert.

### §5.3.2 Neuer map-Schlüssel zur Laufzeit

**Spec:** §5.3 — ein `map`-Schlüssel, den ein Write neu einführt, wird als
unbekannter Pfad gemeldet; „dynamic node/field creation is a tracked
follow-up, not implemented“.

**Repo:** —

**Tests:** —

**Status:** open — `AddressSpace` und `PublishedDataSet` bauen ihre
Knoten- und Feldmenge einmal aus dem Table-9-Default; ein später
auftauchender Schlüssel bekommt keinen Knoten.

### §5.4 Historical Access (§9.3.4)

**Spec:** §5.4 — `opcua.server.historical.topics[]` markiert jedes Leaf
eines `dds_to_ua`-Topics historisierend (`Historizing`, `HistoryRead`-Bit),
hält bis zu `depth` Samples pro Leaf; HistoryRead Raw-Range und AtTime mit
Continuation-Point; `is_read_modified == true` wird abgelehnt; ein
`dds_topic` ohne passende `dds_to_ua_connection` ist ein Ladezeit-Fehler;
Ordnung nach `INFO_TS`-Zeitstempel.

**Repo:** `crates/opcua-bridge/src/daemon/config.rs::{HistoricalConfig,
HistoricalTopicConfig}`; `crates/opcua-bridge/src/address_space_build.rs`;
`crates/opcua-bridge/src/daemon/server.rs::start_with_reload`;
`crates/opcua-server/src/server.rs::Server::history_read`;
`crates/opcua-gateway/src/historical/adapter.rs::InMemoryHistoryCache::push_bounded`.

**Tests:** `crates/opcua-server/src/client.rs::tests::{opcuabridge_5_4_e2e_loopback_history_read_raw_range_returns_retained_samples_in_order, opcuabridge_5_4_e2e_loopback_history_read_paginates_via_continuation_point, opcuabridge_5_4_e2e_loopback_history_read_on_a_non_historizing_node_is_bad_node_id_unknown, opcuabridge_5_4_e2e_loopback_history_read_modified_is_refused_not_silently_answered, opcuabridge_5_4_e2e_loopback_history_read_on_a_historizing_node_with_no_samples_is_bad_no_data, opcuabridge_5_4_e2e_loopback_history_read_at_time_returns_nearest_prior_sample}`;
`crates/opcua-server/src/services.rs::tests::{uacs_5_11_3_a_history_read_request_carries_its_details_as_an_extension_object, opcua11_6_4_5_read_at_time_details_are_req_times_and_use_simple_bounds}`;
`crates/opcua-gateway/src/historical/adapter.rs::tests::{push_bounded_evicts_the_oldest_sample_past_max_depth, ensure_stream_makes_an_empty_read_ok_not_unknown_instance}`;
`crates/opcua-bridge/src/address_space_build.rs::tests::{opcuabridge_5_4_historical_depth_marks_every_leaf_historizing, opcuabridge_5_4_no_historical_depth_leaves_a_leaf_non_historizing}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_historical_topics_parse, opcuabridge_5_4_historical_topic_without_depth_is_refused, opcuabridge_5_4_historical_topic_zero_depth_is_refused}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end`.

**Status:** partial — Markierung, Retention, Raw-Range, AtTime,
Continuation-Point und die Ablehnung von `is_read_modified` sind geprüft.
Ohne Test: der Ladezeit-Fehler für einen `dds_topic` ohne
`dds_to_ua_connection` (`crates/opcua-bridge/src/daemon/server.rs`,
„names no dds_to_ua_connection topic“).

## §6 OPC UA ↔ DDS Bridging-Verhalten

### §6.1 DDS → OPC UA

**Spec:** §6.1 — jedes `dds_to_ua_connection`-Topic bekommt einen
DDS-Reader; jedes Sample wird dekodiert, in benannte Leaf-Werte zerlegt und
mit `AddressSpace::set_value` übernommen.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop`
(`dds_to_ua_jobs`); `crates/opcua-bridge/src/dds_bridge.rs`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_1_dcps_writer_reaches_the_opcua_client_read`;
`crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_6_1_apply_sample_sets_the_right_node_values`;
`crates/opcua-bridge/src/dds_bridge.rs::tests::opcuabridge_6_1_the_pump_names_xcdr2_the_way_read_body_takes_it`, `tests/integration/tests/bridges.rs::opcua_bridge_against_open62541`.

**Status:** done

### §6.2 OPC UA → DDS

**Spec:** §6.2 — (1) ein Fremd-Client schreibt in den eigenen Server: der
Write-Hook prüft `acl.rules[].subject` pro Session (Ablehnung
`Bad_UserAccessDenied`), ein erlaubter Write erreicht DDS über einen
100-ms-Poll; (2) Client-Rolle gegen konfigurierte Fremd-Server, ein
MonitoredItem pro Knoten, jede Notification wird nach DDS geschrieben. Ein
fehlgeschlagener DDS-Write wird geloggt, der schreibende OPC-UA-Client
erfährt davon nichts („remains, named, not fixed“).

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop`
(`ua_to_dds_jobs`); `crates/opcua-bridge/src/ua_client.rs::ClientJob`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other}`;
`crates/opcua-bridge/src/ua_client.rs::tests::opcuabridge_1_client_job_writes_a_monitored_scalar_node_into_dds`;
`crates/opcua-bridge/src/dds_bridge.rs::tests::opcuabridge_6_2_write_against_unknown_topic_is_reported`.

**Status:** partial — beide Mechanismen sind gebaut und geprüft; die
Rückmeldung eines fehlgeschlagenen DDS-Writes an den Client fehlt (§6.2.1).

### §6.2.1 Rückmeldung eines fehlgeschlagenen DDS-Writes

**Spec:** §6.2, §14 — die Write-Antwort an den OPC-UA-Client hängt nicht
davon ab, ob der DDS-Write gelingt; §14 führt das als offen.

**Repo:** —

**Tests:** —

**Status:** open — der Weg nach DDS bleibt ein Poll; ein Fehler beim
DDS-Write wird nur geloggt.

## §7 Security

### §7.1 SecureChannel-Materialbeschaffung

**Spec:** §7.1 — `opcua.server.security_policies`; eigenes Material
`certificate_der`/`private_key_pem`; Peer-Material als zwei Dateien
`trusted_client_certificate_der`/`trusted_client_public_key_pem`.

**Repo:** `crates/opcua-bridge/src/daemon/security.rs::{load_private_key_pem,
load_public_key_pem, load_der}`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_4_1_secured_basic256sha256_sign_and_encrypt_session_through_the_daemon`
(echte RSA-signierte X.509-Zertifikate aus `rcgen`, über
`load_der`/`load_private_key_pem`/`load_public_key_pem` geladen);
`crates/opcua-bridge/src/daemon/config.rs::tests::opcuabridge_7_1_secured_policy_without_certificate_is_refused`.

**Status:** done

### §7.2 ACL

**Spec:** §7.2 — `acl.rules` (`subject`, `allow_read[]`, `allow_write[]`);
Themen-Exposition zur Ladezeit und, für Writes gegen den eigenen Server,
pro Session über den Write-Hook; `acl.allow_anonymous`/`acl.users[]` geben
unterscheidbare Identitäten; `client_subject` ist `"anonymous"`, der
Benutzername oder die Token-Bytes als Hex.

**Repo:** `crates/opcua-bridge/src/daemon/security.rs::{Acl, AclOp,
topic_exposed, allow_write, client_subject}`;
`crates/opcua-bridge/src/daemon/config.rs::{AclUserConfig, acl_users,
acl_allow_anonymous}`; `crates/opcua-bridge/src/daemon/server.rs::start_with_reload`.

**Tests:** `crates/opcua-bridge/src/daemon/security.rs::tests::{opcuabridge_7_2_empty_acl_is_open, opcuabridge_7_2_matching_rule_allows_named_topic_only, opcuabridge_7_2_default_deny_with_rules_blocks_unnamed_subject, opcuabridge_7_2_default_deny_false_with_rules_still_opens_unmatched_topics, opcuabridge_7_2_client_subject_renders_each_identity_kind, opcuabridge_7_2_topic_exposed_ignores_the_subject_dimension}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_acl_users_parse_with_default_policy_id, opcuabridge_7_2_acl_user_without_password_is_refused, opcuabridge_3_acl_rules_parse}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other`.

**Status:** done

## §8 Betrieb + Observability

### §8.1 Logging

**Spec:** §8.1 — `log_level` (`trace`…`error`) nach stderr; jeder
Skip, jeder Dekodierfehler und jeder fehlgeschlagene DDS-Write wird mit
Topic/Pfad geloggt.

**Repo:** —

**Tests:** —

**Status:** open — die Meldungen stehen als `eprintln!` in
`crates/opcua-bridge/src/daemon/server.rs`; kein Test prüft `log_level`
oder eine Meldung.

### §8.2 Prometheus-Metriken

**Spec:** §8.2 — `metrics.listen` (Default `127.0.0.1:9095`) liefert
`GET /metrics` mit `zerodds_opcua_{dds_to_ua,ua_to_dds}_samples_total`,
`zerodds_opcua_pubsub_{published,received}_total`,
`zerodds_opcua_connections_total`, `zerodds_opcua_errors_total`.

**Repo:** —

**Tests:** —

**Status:** open — `crates/opcua-bridge/src/daemon/server.rs::{BridgeMetrics,
serve_metrics}` ist gebaut; kein Test liest den Endpunkt.

### §8.3 OTLP-Spans

**Spec:** §8.3 — `metrics.otlp_endpoint` exportiert einen Span
`zerodds-opcua-bridged.opc_tcp.accept` (`SpanKind::Server`, Peer-Adresse
als Attribut) pro angenommener `opc.tcp`-Verbindung; Flush alle 5 s und
beim Shutdown.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{otlp_config_from_endpoint,
emit_lifecycle_span, new_trace_id, new_span_id}`;
`crates/opcua-bridge/src/daemon/config.rs::metrics_otlp_endpoint`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_8_3_otlp_endpoint_receives_a_span_on_opc_tcp_accept`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_otlp_endpoint_parses, opcuabridge_8_3_no_otlp_endpoint_leaves_it_none}`.

**Status:** done

## §9 Lifecycle

### §9.1/§9.2 Start + Shutdown

**Spec:** §9.1/§9.2 — Start: IDL, Gateway-XML, `DcpsRuntime`,
AddressSpace, Client-/PubSub-Jobs, dann `opc.tcp` und Metrics-Listener;
SIGTERM/SIGINT setzen ein Stop-Flag, Accept-Loop und Metrics-Loop wachen
auf, alle Threads werden gejoint, `ExitCode 0`.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{start_with_reload,
DaemonHandle::shutdown, serve_metrics}`;
`crates/opcua-bridge/src/bin/zerodds-opcua-bridged.rs`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_8_3_otlp_endpoint_receives_a_span_on_opc_tcp_accept}`.

**Status:** partial — Start (`start_with_reload`) und Ende
(`DaemonHandle::shutdown`) im Prozess sind geprüft. Ohne
Test: die Signalbehandlung (SIGTERM/SIGINT) und der Exit-Code 0 des
Binaries.

### §9.3 SIGHUP

**Spec:** §9.3 — SIGHUP lädt `acl.default_deny`/`acl.rules` aus denselben
Quellen neu; eine Datei, die nicht lädt, ändert nichts; alle anderen
Schlüssel brauchen einen Neustart.

**Repo:** —

**Tests:** —

**Status:** open — der Reload-Thread in
`crates/opcua-bridge/src/daemon/server.rs::start_with_reload` ist gebaut;
kein Test sendet SIGHUP.

## §10 Cross-Vendor-Interop

### §10 open62541

**Spec:** §10 — Bibliotheken gemessen gegen open62541 1.5.4; auf
Daemon-Ebene steuert ein C-Client gegen `libopen62541` einen laufenden
`zerodds-opcua-bridged`: Browse des `Objects`-Ordners, Read eines
`dds_to_ua`-Leafs, Write eines `ua_to_dds`-Leafs, Write kommt in DDS an.

**Repo:** `tests/integration/tests/bridges.rs`,
`crates/opcua-bridge/tests/foreign/open62541_client.c`;
`crates/opcua-server/proofs/open62541.sh`,
`crates/opcua-pubsub/proofs/open62541_uadp.sh`.

**Tests:** `tests/integration/tests/bridges.rs::opcua_bridge_against_open62541` (Integrations-Säule, Job
`integration-bridges`, Szenario `bridge-opcua`).

**Status:** done

## §11 Packaging

### §11 Paket, systemd-Unit, Beispielkonfiguration, Handbuchseiten

**Spec:** §11 — Paket `zerodds-opcua-bridge`; systemd-Unit
`packaging/linux/systemd/zerodds-opcua-bridged.service` (`Type=notify`);
Beispielkonfiguration `packaging/linux/configs/opcua-bridged.yaml.example`;
`zerodds-opcua-bridged(1)` + `zerodds-opcua-bridged.yaml(5)`;
deb/rpm/Arch/AppImage-Einträge.

**Repo:** `packaging/linux/systemd/zerodds-opcua-bridged.service`,
`packaging/linux/configs/opcua-bridged.yaml.example`,
`man/man1/zerodds-opcua-bridged.1`, `man/man5/zerodds-opcua-bridged.yaml.5`,
`packaging/linux/deb/assemble-debs.sh`, `packaging/linux/rpm/zerodds.spec`,
`packaging/linux/arch/PKGBUILD`, `packaging/linux/appimage/build.sh`;
`READY=1`/`STOPPING=1` in `crates/opcua-bridge/src/bin/zerodds-opcua-bridged.rs`.

**Tests:** `crates/opcua-bridge/tests/example_yaml_loadback.rs::opcuabridge_3_shipped_example_config_loads_back`.

**Status:** partial — die Beispielkonfiguration lädt durch den echten
Parser. Ohne Test: Unit, Handbuchseiten und Paketeinträge;
`tools/traceability/tests/deployment_layout.rs` und
`tools/traceability/tests/bridge_config_manpages.rs` führen den
OPC-UA-Daemon nicht in ihrer Bridge-Liste.

## §12 Testing

### §12.1 Unit-Tests

**Spec:** §12.1 — `cargo test -p zerodds-opcua-bridge --lib`: Typ-Konversion,
Flatten/Unflatten, IDL-Compiler, DDS-Port, AddressSpace, PubSub-Glue,
Client-Glue, CLI/Config/ACL/Security.

**Repo:** `crates/opcua-bridge/src/`.

**Tests:** `crates/opcua-bridge/src/dynamic_type.rs::tests::{opcuabridge_12_1_primitive_members_map_1_to_1, opcuabridge_12_1_nested_struct_recurses, opcuabridge_12_1_union_carries_discriminator_and_cases}`;
`crates/opcua-bridge/src/dynamic_value.rs::tests::opcuabridge_5_3_a_flat_struct_is_one_variable_per_member_and_back`;
`crates/opcua-bridge/src/idl.rs::tests::opcuabridge_5_1_compiles_a_struct_and_resolves_its_type_support`;
`tests/integration/tests/bridges.rs::opcua_bridge_against_open62541`;
`crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_5_2_struct_topic_gets_one_leaf_per_member`;
`crates/opcua-bridge/src/pubsub_glue.rs::tests::opcuabridge_4_3_publisher_job_builds_a_pds_field_per_leaf`;
`crates/opcua-bridge/src/ua_client.rs::tests::opcuabridge_1_client_job_writes_a_monitored_scalar_node_into_dds`;
`crates/opcua-bridge/src/daemon/cli.rs::tests::opcuabridge_2_bind_addr_parsed`;
`crates/opcua-bridge/src/daemon/config.rs::tests::opcuabridge_3_parses_minimal_config`;
`crates/opcua-bridge/src/daemon/security.rs::tests::opcuabridge_7_2_matching_rule_allows_named_topic_only`.

**Status:** done

### §12.2 Integration (In-Process-E2E)

**Spec:** §12.2 — neun E2E-Fälle in `crates/opcua-bridge/tests/daemon_e2e.rs`
gegen `server::start_with_reload`.

**Repo:** `crates/opcua-bridge/tests/daemon_e2e.rs`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_1_dds_publish_reaches_an_in_tree_pubsub_subscriber, opcuabridge_5_2_node_id_and_browse_path_override_is_applied_not_just_parsed, opcuabridge_5_2_colliding_node_id_override_is_a_load_time_error_not_a_log_line, opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end, opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other, opcuabridge_8_3_otlp_endpoint_receives_a_span_on_opc_tcp_accept, opcuabridge_4_1_secured_basic256sha256_sign_and_encrypt_session_through_the_daemon, opcuabridge_1_external_pubsub_publisher_reaches_the_daemons_own_subscriber}`.

**Status:** done — laut Spec unter Standard-Parallelität instabil (DDS-Domain-Lease)
und mit `--test-threads=1` stabil; Fall 3 meldet auf macOS nur, statt zu
prüfen. Hier nicht nachgemessen.

### §12.3 Multi-Vendor

**Spec:** §12.3 — das Szenario `bridge-opcua` der Integrations-Säule (§10)
gegen `libopen62541`; `crates/opcua-server/proofs/open62541.sh` und
`crates/opcua-pubsub/proofs/open62541_uadp.sh` auf Bibliotheksebene.

**Repo:** `tests/integration/tests/bridges.rs`,
`crates/opcua-server/proofs/open62541.sh`,
`crates/opcua-pubsub/proofs/open62541_uadp.sh`.

**Tests:** `tests/integration/tests/bridges.rs::opcua_bridge_against_open62541`.

**Status:** done

## §13 Querverweise

### §13 Verweise auf die Bibliotheks-Coverage

**Spec:** §13 — `dds-opcua-1.0`, `opcua-client-server-1.05`,
`opcua-pubsub-1.05`, diese Datei.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

## §14 Versionierung

### §14 Versionsstand

**Spec:** §14 — v1.0 erste Fassung; Liste der geschlossenen und der noch
offenen Punkte.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Versionshistorie; die dort offenen Punkte
stehen als Items §4.3.1, §5.3.1, §5.3.2 und §6.2.1.

---

## Audit-Status

13 done / 8 partial / 8 open / 0 blocked / 2 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-opcua-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-opcua-bridge --all-targets` (Job `merge-rust-opcua-bridge`) — 80 Tests grün.
* `cargo nextest run --profile merge -p zerodds-opcua-gateway --all-targets` (Job `merge-rust-opcua-gateway`) — 4 Tests grün.
* `cargo nextest run --profile merge -p zerodds-opcua-server --all-targets` (Job `merge-rust-opcua-server`) — 10 Tests grün.
* `cargo nextest run --profile merge -p zerodds-opcua-bridge --all-targets --features crypto,pubsub-security` (Job `merge-rust-opcua-bridge-features-crypto-pubsub-security`) — 3 Tests grün.
* `cargo nextest run --profile merge -p zerodds-opcua-uacp --all-targets --features crypto` (Job `merge-rust-opcua-uacp-features-crypto`) — 2 Tests grün.
* `python3 ci/run_integration.py integration-bridges` (Job `integration-bridges`) — 1 Test grün.

100 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-opcua-bridge-1.0.open.md`.
