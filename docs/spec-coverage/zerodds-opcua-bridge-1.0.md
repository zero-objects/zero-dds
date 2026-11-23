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

**Tests:** `crates/opcua-bridge/src/daemon/cli.rs::tests::{opcuabridge_2_empty_args_default, opcuabridge_2_bind_addr_parsed, opcuabridge_2_equals_form, opcuabridge_2_domain_parses_int, opcuabridge_2_idl_is_repeatable, opcuabridge_2_unknown_flag_rejected, opcuabridge_2_sets_render_idl_as_appends, opcuabridge_2_help_short_form}`;
`crates/opcua-bridge/tests/binary_cli.rs::{opcuabridge_2_version_prints_the_name_and_exits_zero, opcuabridge_2_check_config_answers_with_its_exit_codes, opcuabridge_2_a_log_level_outside_the_five_names_is_refused, opcuabridge_2_a_set_assignment_wins_over_the_file, opcuabridge_9_2_sigterm_ends_the_running_daemon_with_exit_code_zero}`
**Status:** done — die Flags sind am Parser und am Binary geprüft: `--version`, die Exit-Codes 0, 1 und 2 von `--check-config` mit `--config`, `--log-level` mit seinen fünf Werten, der Lauf mit `--bind`, `--domain`, `--idl`, `--gateway-config` und `--metrics`, und `--set` vor der Datei.

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
`pubsub-security`) mit Schlüsseln aus einer von vier Quellen (`static`,
`local`, `sks`, `push`); die `DataSetMetaData` eines Subscribers beschreibt
jedes Feld mit Built-in-Typ, DataType, ValueRank, ArrayDimensions und einer
eigenen `DataSetFieldId`, dazu `namespaces`, `structureDataTypes` und
`enumDataTypes` (Part 14 §6.2.3.2).

**Repo:** `crates/opcua-bridge/src/pubsub_glue.rs::{PublisherJob,
SubscriberJob, set_security, dataset_metadata}`; `crates/opcua-bridge/src/daemon/config.rs::PubSubSecurityConfig`;
`crates/opcua-bridge/src/daemon/server.rs::build_pubsub_security`; `crates/opcua-bridge/src/ua_types.rs::{type_at_path, builtin_of, type_descriptions}`.

**Tests:** `crates/opcua-bridge/src/pubsub_glue.rs::tests::{opcuabridge_4_3_a_scalar_topic_is_the_field_value, opcuabridge_4_3_publisher_job_builds_a_pds_field_per_leaf, opcuabridge_4_3_the_networkmessage_carries_the_connections_publisher_id, opcuabridge_4_3_a_secured_publisher_job_delivers_a_dds_sample_to_a_subscriber_job, opcuabridge_4_3_subscriber_with_the_wrong_key_refuses_not_silently_drops, uapubsub_6_2_3_2_the_metadata_describes_each_field_and_the_datatypes}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_pubsub_security_parses_and_hex_decodes_the_keys, opcuabridge_4_3_pubsub_security_with_wrong_key_length_is_refused, opcuabridge_4_3_pubsub_security_odd_length_hex_is_refused, opcuabridge_4_3_pubsub_security_unknown_policy_is_refused, opcuabridge_4_3_no_security_key_leaves_it_none}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dds_publish_reaches_an_in_tree_pubsub_subscriber, opcuabridge_1_external_pubsub_publisher_reaches_the_daemons_own_subscriber}`.

**Status:** done — Publisher und Subscriber in beide Richtungen, die vier Schlüsselquellen (§4.3.1) und die Metadaten sind gebaut und geprüft. Nicht geprüft: RawData-Felder mit Arrays mehrerer Dimensionen (der RawData-Leser von `zerodds-opcua-pubsub` liest nur eine Dimension); Discovery-Ankündigungen sendet der Publisher nicht (OPEN-ITEMS 127).

### §4.3.1 SKS-Schlüsselrotation

**Spec:** §4.3 — der Daemon als Security Key Service (Part 14 §8): `GetSecurityKeys`, `GetSecurityGroup`, `InvalidateKeys`, `ForceKeyRotation`, Rotation nach `key_lifetime_ms`; SecurityGroups und ihre Ordner zur Laufzeit (§8.5); Push-Ziele, an die er die Schlüssel mit `SetSecurityKeys` schiebt (§8.6, §8.7); die Rollen von §8.8 als `opcua.server.roles`; als Client holt er Schlüssel mit `GetSecurityKeys` (`key_source: sks`) oder nimmt sie mit `SetSecurityKeys` an (`key_source: push`).

**Repo:** `crates/opcua-bridge/src/daemon/keys.rs::{KeyService, spawn_puller}`; `crates/opcua-bridge/src/daemon/push.rs::{PushTargets, spawn_pusher}`; `crates/opcua-server/src/address_space.rs::{add_method_mut, call_mut, remove_subtree}`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_4_3_the_daemon_hands_out_the_keys_of_its_security_group, opcuabridge_4_3_a_daemon_pulls_the_keys_from_another_and_they_agree, opcuabridge_4_3_an_sks_pushes_the_keys_and_the_publisher_starts, opcuabridge_4_3_security_groups_and_their_folders_are_added_and_removed_at_run_time, opcuabridge_4_3_the_sks_pushes_the_keys_of_a_connected_group_to_its_push_target}`;
`crates/opcua-server/src/address_space.rs::tests::uacs_5_12_2_a_method_adds_and_removes_nodes`.

**Status:** done — Rotation, Holen, Annehmen, Verwaltung der Gruppen und Ordner und das Schieben an ein Ziel sind am laufenden Daemon geprüft, das Schieben zwischen zwei Daemons über einen SignAndEncrypt-Kanal. Die Rollen-Objekte nach OPC 10000-18 und `SessionlessInvoke` baut die eigene Spec nicht (OPEN-ITEMS 121). Nicht geprüft: ein SKS oder ein Ziel einer fremden Implementierung.

## §5 Topic-Mapping

### §5.1 TypeObject/TypeLookup → SerializedTypeSupport

**Spec:** §5.1 — Typauflösung ohne kompilierten Rust-Typ: `opcua.idl` →
`CompleteTypeObject` mit den TypeObjects der Member-Typen →
`SerializedTypeSupport`; ein Typ aus keiner IDL-Datei kommt über
TypeLookup von den entfernten Endpunkten des Topics (§5.1.1).

**Repo:** `crates/opcua-bridge/src/idl.rs::{compile, type_support_for,
is_keyed}`; `crates/opcua-bridge/src/dds_bridge.rs::resolve_discovered`; `crates/opcua-bridge/src/daemon/server.rs::LateTopics`.

**Tests:** `crates/opcua-bridge/src/idl.rs::tests::{opcuabridge_5_1_compiles_a_struct_and_resolves_its_type_support, opcuabridge_5_1_a_type_with_struct_enum_and_union_members_has_its_type_support, opcuabridge_5_1_unknown_type_is_reported_by_name, opcuabridge_5_1_broken_idl_is_reported_with_diagnostics}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_5_1_a_type_in_no_idl_file_comes_from_the_remote_writer, opcuabridge_5_1_a_type_in_no_idl_file_comes_from_the_remote_reader}`.

**Status:** done — ein Typ mit Struktur-, Enum- und Union-Membern bekam vorher keine Type-Support (die TypeObjects der Member-Typen fehlten); beide Wege sind geprüft.

### §5.1.1 getTypes-Abfrage für einen unbekannten Typ

**Spec:** §5.1 — ein Topic, dessen Typ in keiner IDL-Datei steht, wartet; der Daemon fragt jeden entfernten Endpunkt des Topics, der einen vollständigen `TypeIdentifier` ankündigt, mit `getTypes` und `getTypeDependencies` und baut das Topic, sobald die Registry den Typ hält; seine Endpunkte kündigen den Typ selbst an.

**Repo:** `crates/opcua-bridge/src/dds_bridge.rs::{TypeQuery, announce_type}`; `crates/opcua-bridge/src/daemon/server.rs::LateTopics`; `crates/dcps/src/runtime.rs::DiscoveredEndpointInfo::complete_type_identifier`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_5_1_a_type_in_no_idl_file_comes_from_the_remote_writer, opcuabridge_5_1_a_type_in_no_idl_file_comes_from_the_remote_reader}`.

**Status:** done — beide Richtungen (entfernter Writer, entfernter Reader) am laufenden Daemon geprüft. Nicht geprüft: ein Endpunkt einer fremden Implementierung.

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

**Spec:** §5.3 — Primitive, Strings, Enums, Bitmasks, Structs, Unions, `map<K,V>` und Sammlungen davon in beide Richtungen; je Struktur, Union, Enum und Bitmask ein DataType mit `DataTypeDefinition` und `Default Binary`-Kodierung, je Struktur ein `<Name>VariableType`, an jeder Variable DataType, ValueRank und TypeDefinition (Gateway 1.0 §9.2); der Wert einer Aggregat-Variable als ExtensionObject (OPC 10000-6 §5.2.6 bis §5.2.8); Bitmask als `OptionSet`; ein abwesendes `@optional` und ein nicht aktiver Union-Zweig lesen `Bad_NoData`; das Server-Objekt mit `NamespaceArray`.

**Repo:** `crates/opcua-bridge/src/dynamic_value.rs::{flatten, unflatten}`;
`crates/opcua-bridge/src/dynamic_type.rs`; `crates/opcua-bridge/src/ua_types.rs::{ensure_types, shape_of}`; `crates/opcua-bridge/src/ua_structure.rs::{aggregate_values, to_extension_object, from_extension_object, replace_at}`; `crates/opcua-bridge/src/address_space_build.rs::{materialize_instance_with_override, apply_sample}`; `crates/opcua-server/src/address_space.rs::ensure_server_object`.

**Tests:** `crates/opcua-bridge/src/dynamic_value.rs::tests::{opcuabridge_5_3_a_flat_struct_is_one_variable_per_member_and_back, opcuabridge_5_3_nested_struct_flattens_with_dotted_paths, opcuabridge_5_3_partial_unflatten_keeps_the_rest_of_the_default, opcuabridge_5_3_enum_flattens_to_its_ordinal_and_back, opcuabridge_5_3_sequence_of_primitives_is_one_variant, opcuabridge_5_3_wide_char_maps_to_uint16_and_back, opcuabridge_5_3_wide_string_maps_to_string_and_back, opcuabridge_5_3_map_of_primitives_decomposes_to_one_leaf_per_key_and_back, opcuabridge_5_3_map_partial_write_keeps_the_untouched_key, opcuabridge_5_3_a_value_nested_past_the_bound_is_refused_not_walked, opcuabridge_5_3_a_union_is_its_switch_field_and_its_active_case_and_back, opcuabridge_5_3_an_absent_optional_member_has_no_leaf_until_it_is_written, opcuabridge_5_3_an_array_of_two_dimensions_is_one_leaf_and_back, opcuabridge_5_3_a_bitmask_is_an_option_set_and_a_write_keeps_the_bits_not_valid}`;
`crates/opcua-bridge/src/address_space_build.rs::tests::{opcuabridge_5_3_unmatched_path_is_reported_not_dropped, opcuabridge_5_3_an_inner_sequence_is_a_variable_with_its_datatype}`;
`crates/opcua-bridge/src/ua_structure.rs::tests::{opcuabridge_5_3_a_structure_is_its_default_binary_extension_object, opcuabridge_5_3_optional_members_are_an_encoding_mask_first, opcuabridge_5_3_a_union_is_its_switch_then_the_case}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_5_3_structure_and_union_variables_carry_their_datatypes_and_whole_values, opcuabridge_6_2_a_write_keeps_the_members_it_does_not_name_and_a_structure_is_written_whole}`;
`crates/opcua-server/src/address_space.rs::tests::uacs_5_8_3_2_the_server_object_names_the_namespaces`.

**Status:** done — gegengeprüft mit dem Client von `asyncua` (FreeOpcUa) auf dem Linux-Testhost: er lädt die DataTypeDefinitions und liest und schreibt Strukturen mit Struktur-, Sequenz- und Enum-Membern, Unions, `@optional`-Member und Sequenzen von Sequenzen in beide Richtungen. Gemessen dort: `asyncua` liest die Inline-Matrix eines Arrays mehrerer Dimensionen (Part 6 1.05 §5.2.5) nicht und kennt ein OptionSet nur als Integer (OPEN-ITEMS 125). Der Fehler, dass ein Sample mit Union, abwesendem `@optional` oder mehrdimensionalem Array verworfen wurde, ist behoben.

### §5.3.1 Live-Wert für Array/Sequence von Struct/Union, bitset, fixed, float128

**Spec:** §5.3 — `long double` ist ein `ByteString` mit 16 Oktetten (DDS-OPCUA 1.0 §9.2.1); `fixed<P,S>` ein `Decimal` (OPC 10000-6 §5.1.10); ein `bitset` die Struktur seiner benannten Bitfelder; ein Array oder eine Sequence von Structures oder Unions hat je Element eine Variable `<Typname>_<Index>` (§9.2.5.1.2.3, §9.2.5.2.2.3), eine Sammlung von Sammlungen je innerer Sammlung eine Variable `<Name>_<Index>` (§9.2.5.2.2.5); die Sammlung von Strukturen hat als Wert das Array ihrer ExtensionObjects.

**Repo:** `crates/opcua-bridge/src/dynamic_value.rs::{flatten, unflatten, element_name}`;
`crates/opcua-bridge/src/dynamic_type.rs::{to_dds_type, bitfields, fixed_of}`;
`crates/opcua-bridge/src/ua_structure.rs::aggregate_values`;
`crates/opcua-bridge/src/address_space_build.rs::{apply_sample, TopicAddressSpace}`;
`crates/opcua-gateway/src/dds_to_ua/walker.rs::DdsType`

**Tests:** `crates/opcua-bridge/src/dynamic_value.rs::tests::{opcuabridge_5_3_long_double_is_a_byte_string_of_sixteen_and_back, opcuabridge_5_3_a_bitset_is_one_leaf_per_bitfield_and_back, opcuabridge_5_3_a_fixed_is_a_decimal_and_back, opcuabridge_5_3_a_sequence_of_structs_is_one_variable_per_element_and_back, opcuabridge_5_3_an_element_is_named_by_its_position_in_each_dimension, opcuabridge_5_3_a_write_to_a_collection_of_numbers_replaces_it, opcuabridge_5_3_a_sequence_of_sequences_is_a_variable_per_inner_sequence_and_back}`;
`crates/opcua-bridge/src/dynamic_type.rs::tests::{opcuabridge_5_3_long_double_is_a_byte_string, opcuabridge_5_3_a_fixed_is_a_decimal, opcuabridge_5_3_a_bitset_is_the_structure_of_its_named_bitfields}`;
`crates/opcua-bridge/src/ua_structure.rs::tests::{opcuabridge_5_3_an_array_of_two_dimensions_is_the_inline_matrix_and_the_older_form_is_read, opcuabridge_5_3_a_collection_of_collections_is_a_field_of_variants, opcuabridge_5_3_every_aggregate_gets_its_value_and_a_write_replaces_it}`;
`crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_5_3_the_elements_of_a_collection_of_structs_are_variables`.

**Status:** done — alle genannten Formen in beide Richtungen gebaut und geprüft, die Sammlung von Sammlungen auch mit `asyncua`. Die äußere Variable einer Sammlung von Sammlungen hat keinen eigenen Wert (das Variant-Modell des Daemons hält kein Variant in einem Variant); ihre Elemente haben ihn.

### §5.3.2 Neuer map-Schlüssel zur Laufzeit

**Spec:** §5.3 — ein `map`-Schlüssel, den ein DDS-Sample bringt und der noch keinen Knoten hat, bekommt ihn beim Anwenden des Samples (eine Variable unter dem Knoten des Map-Members, für einen Struct-Wert dessen Members darunter); im `PublishedDataSet` ist er ein am Ende angehängtes Feld mit neuer `MinorVersion`. Höchstens 4096 Einträge je Topic, Schlüssel bis 256 Oktette.

**Repo:** `crates/opcua-bridge/src/address_space_build.rs::{apply_sample, TopicAddressSpace}`;
`crates/opcua-bridge/src/pubsub_glue.rs::PublisherJob::tick`;
`crates/opcua-pubsub/src/daemon.rs::Publisher::append_field`; `crates/opcua-pubsub/src/writer.rs::DataSetWriter::fields_appended`

**Tests:** `crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_5_3_a_new_map_key_gets_its_nodes_when_a_sample_brings_it`;
`crates/opcua-bridge/src/pubsub_glue.rs::tests::opcuabridge_5_3_a_new_map_key_is_a_field_appended_to_the_published_dataset`;
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_5_3_a_map_key_a_sample_brings_is_a_node_a_client_reads`

**Status:** done — der Knoten entsteht beim ersten Sample mit dem Schlüssel und bleibt; ein Schlüssel über einer der Grenzen und ein Pfad unter keiner Map werden als unbekannter Pfad gemeldet. Über einen Write kann ein Client keinen Schlüssel einführen (der Daemon bedient kein AddNodes). Nicht geprüft: Schlüssel in einer Map, die selbst Wert einer Map ist; ein Subscriber einer fremden Implementierung, der die neue `MinorVersion` sieht.

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
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end`;
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_5_4_a_historical_topic_without_a_connection_is_a_load_time_error`
**Status:** done — Markierung, Retention, Raw-Range, AtTime, Continuation-Point, die Ablehnung von `is_read_modified` und der Ladezeit-Fehler für einen `dds_topic` ohne `dds_to_ua_connection` sind geprüft.

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

**Spec:** §6.2 — (1) ein Fremd-Client schreibt in den eigenen Server: der Write-Hook prüft `acl.rules[].subject` pro Session (Ablehnung `Bad_UserAccessDenied`); ein erlaubter Write geht nach DDS, bevor der Client die Antwort bekommt, und die Antwort ist das Ergebnis; (2) Client-Rolle gegen konfigurierte Fremd-Server, ein MonitoredItem pro Knoten, jede Notification wird nach DDS geschrieben.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop`
(`ua_to_dds_jobs`); `crates/opcua-bridge/src/ua_client.rs::ClientJob`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other}`;
`crates/opcua-bridge/src/ua_client.rs::tests::opcuabridge_1_client_job_writes_a_monitored_scalar_node_into_dds`;
`crates/opcua-bridge/src/dds_bridge.rs::tests::opcuabridge_6_2_write_against_unknown_topic_is_reported`;
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_6_2_a_write_that_does_not_reach_dds_is_answered_bad`;
`crates/opcua-server/src/client.rs::tests::opcuabridge_6_2_a_write_the_commit_refuses_is_answered_bad_and_taken_back`
**Status:** done — beide Mechanismen sind gebaut und geprüft; die Antwort auf einen erlaubten Write ist das Ergebnis des Writes nach DDS (§6.2.1).

### §6.2.1 Rückmeldung eines fehlgeschlagenen DDS-Writes

**Spec:** §6.2 — die Antwort auf einen Write ist das Ergebnis des Writes nach DDS; ein Wert, den DDS nicht nimmt, wird mit `Bad_CommunicationError` beantwortet und aus dem AddressSpace zurückgenommen.

**Repo:** `crates/opcua-server/src/server.rs::{Server::set_write_commit, Server::write_one}`;
`crates/opcua-bridge/src/daemon/server.rs::{start_with_reload, flush_ua_to_dds}`

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_6_2_a_write_that_does_not_reach_dds_is_answered_bad`;
`crates/opcua-server/src/client.rs::tests::opcuabridge_6_2_a_write_the_commit_refuses_is_answered_bad_and_taken_back`
**Status:** done — ein Write, den der AddressSpace annimmt, geht über `Server::set_write_commit` nach DDS, bevor der Client die Antwort bekommt. Kodiert der Wert nicht für den Typ des Topics oder nimmt DDS ihn nicht, ist die Antwort `Bad_CommunicationError`, und der Server setzt den Wert davor wieder ein. Geprüft ist der Fall, dass der Wert nicht kodiert (`string<4>`); ein Write, den DDS selbst ablehnt, ist nicht nachgestellt.

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

**Repo:** `crates/opcua-bridge/src/daemon/log.rs`; `crates/opcua-bridge/src/daemon/server.rs`

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_8_1_a_decode_failure_is_logged_by_topic_and_log_level_filters`;
`crates/opcua-bridge/src/daemon/log.rs::tests::opcuabridge_8_1_a_line_below_the_log_level_is_not_written`
**Status:** done — jede Zeile trägt ihre Stufe und wird geschrieben, wenn die Stufe `log_level` erreicht; Dekodierfehler und fehlgeschlagene DDS-Writes nennen ihr Topic. Geprüft sind der Dekodierfehler mit Topic und das Filtern einer `info`-Zeile bei `warn`; die Zeilen der übrigen Skips sind nicht einzeln geprüft.

### §8.2 Prometheus-Metriken

**Spec:** §8.2 — `metrics.listen` (Default `127.0.0.1:9095`) liefert
`GET /metrics` mit `zerodds_opcua_{dds_to_ua,ua_to_dds}_samples_total`,
`zerodds_opcua_pubsub_{published,received}_total`,
`zerodds_opcua_connections_total`, `zerodds_opcua_errors_total`.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{BridgeMetrics, serve_metrics}`

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_8_2_the_metrics_endpoint_serves_the_six_counters`
**Status:** done — der Test liest `GET /metrics` und findet die sechs Zähler; `dds_to_ua_samples_total` und `connections_total` zählen das Sample und die Verbindung des Tests.

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

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_8_3_otlp_endpoint_receives_a_span_on_opc_tcp_accept}`;
`crates/opcua-bridge/tests/binary_cli.rs::{opcuabridge_9_2_sigterm_ends_the_running_daemon_with_exit_code_zero, opcuabridge_9_2_sigint_ends_the_running_daemon_with_exit_code_zero}`
**Status:** done — Start und Ende sind im Prozess und am Binary geprüft: SIGTERM und SIGINT beenden den laufenden Daemon mit Exit-Code 0.

### §9.3 SIGHUP

**Spec:** §9.3 — SIGHUP lädt `acl.default_deny`/`acl.rules` aus denselben
Quellen neu; eine Datei, die nicht lädt, ändert nichts; alle anderen
Schlüssel brauchen einen Neustart.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{start_with_reload, install_signal_watcher}`

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_9_3_sighup_reloads_the_acl_and_a_failed_load_changes_nothing`
**Status:** done — der Test sendet SIGHUP an den Prozess: ein Laden, das scheitert, ändert nichts; das nächste setzt die neuen `acl.rules` in Kraft.

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

**Tests:** `crates/opcua-bridge/tests/example_yaml_loadback.rs::opcuabridge_3_shipped_example_config_loads_back`;
`tools/traceability/tests/deployment_layout.rs::{fhs_layout_paths_are_the_ones_the_clause_names, debian_control_declares_the_package_split_of_the_clause, rpm_spec_declares_the_same_split_and_systemd_scriptlets, arch_pkgbuild_carries_the_aur_fields_and_the_daemon_list, appimage_build_targets_musl_and_covers_every_daemon, every_systemd_unit_carries_the_hardening_set_of_the_clause}`;
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, every_man_page_names_its_parser_and_what_an_unknown_key_does}`;
`crates/opcua-bridge/tests/binary_cli.rs::opcuabridge_11_the_manual_page_names_every_flag_of_the_binary`
**Status:** done — die Beispielkonfiguration lädt durch den echten Parser; Unit, Paketeinträge (deb, rpm, Arch, AppImage) und beide Handbuchseiten sind von den Traceability-Tests und einem Test am Binary geprüft.

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
(Rollen-Objekte nach OPC 10000-18, `SessionlessInvoke`) stehen in
OPEN-ITEMS 121.

---

## Audit-Status

29 done / 0 partial / 0 open / 0 blocked / 2 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 9695 bis 9813 von `main` (`python3 ci/coverage_results.py --document zerodds-opcua-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-opcua-bridge --all-targets` (Job `merge-rust-opcua-bridge`) — 123 Tests grün.
* `cargo nextest run --profile merge -p zerodds-opcua-gateway --all-targets` (Job `merge-rust-opcua-gateway`) — 4 Tests grün.
* `cargo nextest run --profile merge -p zerodds-opcua-server --all-targets` (Job `merge-rust-opcua-server`) — 13 Tests grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 9 Tests grün.
* `cargo nextest run --profile merge -p zerodds-opcua-bridge --all-targets --features crypto,pubsub-security` (Job `merge-rust-opcua-bridge-features-crypto-pubsub-security`) — 8 Tests grün.
* `cargo nextest run --profile merge -p zerodds-opcua-uacp --all-targets --features crypto` (Job `merge-rust-opcua-uacp-features-crypto`) — 2 Tests grün.
* `python3 ci/run_integration.py integration-bridges` (Job `integration-bridges`) — 1 Test grün.

160 zitierte Tests grün, 0 nicht grün.
