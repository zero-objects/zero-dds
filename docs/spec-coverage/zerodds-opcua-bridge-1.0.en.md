# `zerodds-opcua-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-opcua-bridge-1.0.md`

Implementation:

- `crates/opcua-bridge/` — the daemon process (`zerodds-opcua-bridged`) over
  the four library crates `opcua-gateway`, `opcua-uacp`, `opcua-server`,
  `opcua-pubsub`.

## §1 Conformance levels

### §1 Four Gateway 1.0 conformance points declared

**Spec:** §1 — all four conformance points of Gateway 1.0 §2 declared at
the library level; wired in the daemon: §9 (DDS → OPC UA), §8 (OPC UA →
DDS, own server and client role), Part 14 UADP in both directions,
historical access (§9.3.4, point 4 "complete").

**Repo:** `crates/opcua-gateway/src/conformance.rs::DECLARED_CONFORMANCE`;
`crates/opcua-bridge/src/daemon/server.rs::start_with_reload`;
`crates/opcua-bridge/src/ua_client.rs::ClientJob`;
`crates/opcua-bridge/src/pubsub_glue.rs::{PublisherJob, SubscriberJob}`.

**Tests:** `crates/opcua-gateway/src/conformance.rs::tests::{declares_all_four_conformance_points, complete_points_subsume_their_basic}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_1_dds_publish_reaches_an_in_tree_pubsub_subscriber, opcuabridge_1_external_pubsub_publisher_reaches_the_daemons_own_subscriber, opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end}`;
`crates/opcua-bridge/src/ua_client.rs::tests::opcuabridge_1_client_job_writes_a_monitored_scalar_node_into_dds`.

**Status:** done

## §2 CLI surface

### §2 zerodds-opcua-bridged CLI

**Spec:** §2 — `--config`/`--check-config` (exit 0 valid, 1 invalid,
2 unreadable)/`--set`/`--bind`/`--domain`/`--idl` (repeatable)/
`--gateway-config`/`--log-level`/`--metrics`/`--version`/`--help`; every
convenience flag is `--set` of its file key and wins over the files.

**Repo:** `crates/opcua-bridge/src/bin/zerodds-opcua-bridged.rs`,
`crates/opcua-bridge/src/daemon/cli.rs`.

**Tests:** `crates/opcua-bridge/src/daemon/cli.rs::tests::{opcuabridge_2_empty_args_default, opcuabridge_2_bind_addr_parsed, opcuabridge_2_equals_form, opcuabridge_2_domain_parses_int, opcuabridge_2_idl_is_repeatable, opcuabridge_2_unknown_flag_rejected, opcuabridge_2_sets_render_idl_as_appends, opcuabridge_2_help_short_form}`;
`crates/opcua-bridge/tests/binary_cli.rs::{opcuabridge_2_version_prints_the_name_and_exits_zero, opcuabridge_2_check_config_answers_with_its_exit_codes, opcuabridge_2_a_log_level_outside_the_five_names_is_refused, opcuabridge_2_a_set_assignment_wins_over_the_file, opcuabridge_9_2_sigterm_ends_the_running_daemon_with_exit_code_zero}`
**Status:** done — the flags are tested at the parser and at the binary: `--version`, the exit codes 0, 1 and 2 of `--check-config` with `--config`, `--log-level` with its five values, the run with `--bind`, `--domain`, `--idl`, `--gateway-config` and `--metrics`, and `--set` over the file.

## §3 Config-file format

### §3 Layered YAML loader

**Spec:** §3 — YAML, layered (defaults < environment < files
`opcua-bridged.yaml` + `opcua-bridged.d/*.yaml` < `--set`); keys
`domain`/`log_level`/`opcua.{idl,gateway_config,server,client,pubsub}`/
`acl`/`metrics`; an unknown key or a value of the wrong shape stops the
start with file and line.

**Repo:** `crates/opcua-bridge/src/daemon/config.rs::{DaemonConfig, KEYS,
is_key, from_config}` over the shared loader `zerodds-config`.

**Tests:** `crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_parses_minimal_config, opcuabridge_3_rejects_unknown_top_level_section_key, opcuabridge_3_client_server_entry_needs_endpoint_url, opcuabridge_3_parses_a_full_client_server_entry, opcuabridge_3_parses_a_full_pubsub_connection, opcuabridge_3_pubsub_security_parses_and_hex_decodes_the_keys, opcuabridge_3_otlp_endpoint_parses, opcuabridge_3_historical_topics_parse, opcuabridge_3_acl_users_parse_with_default_policy_id, opcuabridge_3_acl_rules_parse}`;
`crates/opcua-bridge/tests/example_yaml_loadback.rs::opcuabridge_3_shipped_example_config_loads_back`.

**Status:** done

## §4 Wire protocols

### §4.1 opc.tcp + SecureChannel

**Spec:** §4.1 — Part 6 §7.1/§6.7; SecurityMode `None` always; with the
`crypto` feature `basic256sha256`/`aes128_sha256_rsaoaep`/
`aes256_sha256_rsapss`.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{accept_loop,
serve_connection_locked, build_server_security}`;
`crates/opcua-uacp/src/crypto.rs`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_4_1_secured_basic256sha256_sign_and_encrypt_session_through_the_daemon}`
(SecurityMode `None` and Basic256Sha256 SignAndEncrypt over a real
`opc.tcp` socket against the running daemon);
`crates/opcua-uacp/src/crypto.rs::tests::{uacs_6_7_2_an_open_secure_channel_chunk_is_the_layout_of_figure_13, uacs_6_7_2_an_open_secure_channel_chunk_is_the_layout_of_figure_13, uacs_6_7_full_secured_handshake_flow}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::opcuabridge_4_1_unknown_security_policy_is_refused`.

**Status:** done

### §4.2 Part 4 services

**Spec:** §4.2 — Session, Read/Write, Browse, Call,
CreateSubscription/CreateMonitoredItems/Publish and HistoryRead from
`zerodds-opcua-server`; one `Server` per process for all connections;
`Server::set_write_hook` offers every `Write` target with the session
identity before it reaches the AddressSpace.

**Repo:** `crates/opcua-server/src/server.rs::{Server::set_write_hook,
handle_service}`; `crates/opcua-bridge/src/daemon/server.rs::start_with_reload`
(one `Arc<Mutex<Server>>`, hook installation).

**Tests:** `crates/opcua-server/src/client.rs::tests::{opcuabridge_4_2_write_hook_can_refuse_a_write_before_it_reaches_the_address_space, opcuabridge_4_2_write_hook_observes_the_calling_sessions_identity}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other, opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end}`.
The library services themselves are in `opcua-client-server-1.05.en.md`.

**Status:** done

### §4.3 Part 14 UADP

**Spec:** §4.3 — one Publisher/Subscriber pair per
`opcua.pubsub.connections[]` over UDP; message security (feature
`pubsub-security`) with keys from one of four sources (`static`, `local`,
`sks`, `push`); a subscriber's `DataSetMetaData` describes each field with
its built-in type, DataType, ValueRank, ArrayDimensions and a
`DataSetFieldId` of its own, with the `namespaces`, `structureDataTypes`
and `enumDataTypes` (Part 14 §6.2.3.2).

**Repo:** `crates/opcua-bridge/src/pubsub_glue.rs::{PublisherJob,
SubscriberJob, set_security, dataset_metadata}`; `crates/opcua-bridge/src/daemon/config.rs::PubSubSecurityConfig`;
`crates/opcua-bridge/src/daemon/server.rs::build_pubsub_security`; `crates/opcua-bridge/src/ua_types.rs::{type_at_path, builtin_of, type_descriptions}`.

**Tests:** `crates/opcua-bridge/src/pubsub_glue.rs::tests::{opcuabridge_4_3_a_scalar_topic_is_the_field_value, opcuabridge_4_3_publisher_job_builds_a_pds_field_per_leaf, opcuabridge_4_3_the_networkmessage_carries_the_connections_publisher_id, opcuabridge_4_3_a_secured_publisher_job_delivers_a_dds_sample_to_a_subscriber_job, opcuabridge_4_3_subscriber_with_the_wrong_key_refuses_not_silently_drops, uapubsub_6_2_3_2_the_metadata_describes_each_field_and_the_datatypes}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_pubsub_security_parses_and_hex_decodes_the_keys, opcuabridge_4_3_pubsub_security_with_wrong_key_length_is_refused, opcuabridge_4_3_pubsub_security_odd_length_hex_is_refused, opcuabridge_4_3_pubsub_security_unknown_policy_is_refused, opcuabridge_4_3_no_security_key_leaves_it_none}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dds_publish_reaches_an_in_tree_pubsub_subscriber, opcuabridge_1_external_pubsub_publisher_reaches_the_daemons_own_subscriber}`.

**Status:** done — publisher and subscriber in both directions, the four key sources (§4.3.1) and the metadata are built and checked. Not checked: RawData fields with arrays of several dimensions (the RawData reader of `zerodds-opcua-pubsub` reads one dimension); the publisher sends no discovery announcements (OPEN-ITEMS 127).

### §4.3.1 SKS key rotation

**Spec:** §4.3 — the daemon as a Security Key Service (Part 14 §8): `GetSecurityKeys`, `GetSecurityGroup`, `InvalidateKeys`, `ForceKeyRotation`, rotation after `key_lifetime_ms`; SecurityGroups and their folders at run time (§8.5); push targets it pushes the keys to with `SetSecurityKeys` (§8.6, §8.7); the roles of §8.8 as `opcua.server.roles`; as a client it pulls keys with `GetSecurityKeys` (`key_source: sks`) or takes them with `SetSecurityKeys` (`key_source: push`).

**Repo:** `crates/opcua-bridge/src/daemon/keys.rs::{KeyService, spawn_puller}`; `crates/opcua-bridge/src/daemon/push.rs::{PushTargets, spawn_pusher}`; `crates/opcua-server/src/address_space.rs::{add_method_mut, call_mut, remove_subtree}`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_4_3_the_daemon_hands_out_the_keys_of_its_security_group, opcuabridge_4_3_a_daemon_pulls_the_keys_from_another_and_they_agree, opcuabridge_4_3_an_sks_pushes_the_keys_and_the_publisher_starts, opcuabridge_4_3_security_groups_and_their_folders_are_added_and_removed_at_run_time, opcuabridge_4_3_the_sks_pushes_the_keys_of_a_connected_group_to_its_push_target}`;
`crates/opcua-server/src/address_space.rs::tests::uacs_5_12_2_a_method_adds_and_removes_nodes`.

**Status:** done — rotation, pulling, taking pushed keys, managing groups and folders and pushing to a target are checked on the running daemon, the push between two daemons over a SignAndEncrypt channel. The role Objects of OPC 10000-18 and `SessionlessInvoke` are not built by the own spec (OPEN-ITEMS 121). Not checked: an SKS or a target of another implementation.

## §5 Topic mapping

### §5.1 TypeObject/TypeLookup → SerializedTypeSupport

**Spec:** §5.1 — type resolution without a compiled Rust type: `opcua.idl`
→ `CompleteTypeObject` with the TypeObjects of the member types →
`SerializedTypeSupport`; a type in no IDL file comes over TypeLookup from
the remote endpoints of the topic (§5.1.1).

**Repo:** `crates/opcua-bridge/src/idl.rs::{compile, type_support_for,
is_keyed}`; `crates/opcua-bridge/src/dds_bridge.rs::resolve_discovered`; `crates/opcua-bridge/src/daemon/server.rs::LateTopics`.

**Tests:** `crates/opcua-bridge/src/idl.rs::tests::{opcuabridge_5_1_compiles_a_struct_and_resolves_its_type_support, opcuabridge_5_1_a_type_with_struct_enum_and_union_members_has_its_type_support, opcuabridge_5_1_unknown_type_is_reported_by_name, opcuabridge_5_1_broken_idl_is_reported_with_diagnostics}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_5_1_a_type_in_no_idl_file_comes_from_the_remote_writer, opcuabridge_5_1_a_type_in_no_idl_file_comes_from_the_remote_reader}`.

**Status:** done — a type with a structure, enumeration or union member got no type support before (the TypeObjects of the member types were missing); both ways are checked.

### §5.1.1 getTypes request for an unknown type

**Spec:** §5.1 — a topic whose type is in no IDL file waits; the daemon asks every remote endpoint of the topic that announces a complete `TypeIdentifier` with `getTypes` and `getTypeDependencies` and builds the topic once the registry holds the type; its endpoints announce the type themselves.

**Repo:** `crates/opcua-bridge/src/dds_bridge.rs::{TypeQuery, announce_type}`; `crates/opcua-bridge/src/daemon/server.rs::LateTopics`; `crates/dcps/src/runtime.rs::DiscoveredEndpointInfo::complete_type_identifier`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_5_1_a_type_in_no_idl_file_comes_from_the_remote_writer, opcuabridge_5_1_a_type_in_no_idl_file_comes_from_the_remote_reader}`.

**Status:** done — both directions (remote writer, remote reader) checked on the running daemon. Not checked: an endpoint of another implementation.

### §5.2 AddressSpace shape (§9.3)

**Spec:** §5.2 — `Domain<id>` → `Topic:<name>` → sample tree under
`Topic:<name>/Value[/<path>]`; the `<node_id>`/`<browse_path>` override from
the §10 XML is applied, a collision or an empty path segment is a load-time
error; a `dds_to_ua` leaf is `CurrentRead` only, a `ua_to_dds` leaf
`CurrentRead | CurrentWrite`.

**Repo:** `crates/opcua-bridge/src/address_space_build.rs::{ensure_domain,
ensure_topic, ensure_objects_folder, materialize_instance,
materialize_instance_with_override, ensure_browse_path, apply_sample}`.

**Tests:** `crates/opcua-bridge/src/address_space_build.rs::tests::{opcuabridge_5_2_scalar_topic_gets_one_leaf_at_the_empty_path, opcuabridge_5_2_struct_topic_gets_one_leaf_per_member, opcuabridge_5_2_objects_folder_is_browsable_after_materializing_a_topic, opcuabridge_5_2_two_topics_do_not_collide, opcuabridge_5_2_node_id_override_replaces_the_default_root_identity, opcuabridge_5_2_node_id_override_colliding_with_an_existing_node_is_refused, opcuabridge_5_2_empty_browse_path_segment_is_refused, opcuabridge_5_2_browse_path_organizes_the_root_from_the_objects_folder, opcuabridge_5_2_dds_to_ua_leaf_is_read_only_ua_to_dds_leaf_is_writable}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_5_2_node_id_and_browse_path_override_is_applied_not_just_parsed, opcuabridge_5_2_colliding_node_id_override_is_a_load_time_error_not_a_log_line}`.

**Status:** done

### §5.3 Type-shape coverage

**Spec:** §5.3 — primitives, strings, enumerations, bitmasks, structs, unions, `map<K,V>` and collections of them, both directions; a DataType with `DataTypeDefinition` and `Default Binary` encoding per structure, union, enumeration and bitmask, a `<Name>VariableType` per structure, DataType, ValueRank and TypeDefinition on every Variable (Gateway 1.0 §9.2); the value of an aggregate Variable as an ExtensionObject (OPC 10000-6 §5.2.6 to §5.2.8); a bitmask as an `OptionSet`; an absent `@optional` member and an inactive union case read `Bad_NoData`; the Server Object with `NamespaceArray`.

**Repo:** `crates/opcua-bridge/src/dynamic_value.rs::{flatten, unflatten}`;
`crates/opcua-bridge/src/dynamic_type.rs`; `crates/opcua-bridge/src/ua_types.rs::{ensure_types, shape_of}`; `crates/opcua-bridge/src/ua_structure.rs::{aggregate_values, to_extension_object, from_extension_object, replace_at}`; `crates/opcua-bridge/src/address_space_build.rs::{materialize_instance_with_override, apply_sample}`; `crates/opcua-server/src/address_space.rs::ensure_server_object`.

**Tests:** `crates/opcua-bridge/src/dynamic_value.rs::tests::{opcuabridge_5_3_a_flat_struct_is_one_variable_per_member_and_back, opcuabridge_5_3_nested_struct_flattens_with_dotted_paths, opcuabridge_5_3_partial_unflatten_keeps_the_rest_of_the_default, opcuabridge_5_3_enum_flattens_to_its_ordinal_and_back, opcuabridge_5_3_sequence_of_primitives_is_one_variant, opcuabridge_5_3_wide_char_maps_to_uint16_and_back, opcuabridge_5_3_wide_string_maps_to_string_and_back, opcuabridge_5_3_map_of_primitives_decomposes_to_one_leaf_per_key_and_back, opcuabridge_5_3_map_partial_write_keeps_the_untouched_key, opcuabridge_5_3_a_value_nested_past_the_bound_is_refused_not_walked, opcuabridge_5_3_a_union_is_its_switch_field_and_its_active_case_and_back, opcuabridge_5_3_an_absent_optional_member_has_no_leaf_until_it_is_written, opcuabridge_5_3_an_array_of_two_dimensions_is_one_leaf_and_back, opcuabridge_5_3_a_bitmask_is_an_option_set_and_a_write_keeps_the_bits_not_valid}`;
`crates/opcua-bridge/src/address_space_build.rs::tests::{opcuabridge_5_3_unmatched_path_is_reported_not_dropped, opcuabridge_5_3_an_inner_sequence_is_a_variable_with_its_datatype}`;
`crates/opcua-bridge/src/ua_structure.rs::tests::{opcuabridge_5_3_a_structure_is_its_default_binary_extension_object, opcuabridge_5_3_optional_members_are_an_encoding_mask_first, opcuabridge_5_3_a_union_is_its_switch_then_the_case}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_5_3_structure_and_union_variables_carry_their_datatypes_and_whole_values, opcuabridge_6_2_a_write_keeps_the_members_it_does_not_name_and_a_structure_is_written_whole}`;
`crates/opcua-server/src/address_space.rs::tests::uacs_5_8_3_2_the_server_object_names_the_namespaces`.

**Status:** done — cross-checked with the client of `asyncua` (FreeOpcUa) on the Linux test host: it loads the DataTypeDefinitions and reads and writes structures with structure, sequence and enumeration members, unions, `@optional` members and sequences of sequences in both directions. Measured there: `asyncua` does not read the inline matrix of an array of several dimensions (Part 6 1.05 §5.2.5) and knows an OptionSet only as an integer (OPEN-ITEMS 125). The fault that a sample with a union, an absent `@optional` member or an array of several dimensions was refused is fixed.

### §5.3.1 Live value for array/sequence of struct/union, bitset, fixed, float128

**Spec:** §5.3 — `long double` is a `ByteString` of 16 octets (DDS-OPCUA 1.0 §9.2.1); `fixed<P,S>` a `Decimal` (OPC 10000-6 §5.1.10); a `bitset` the structure of its named bitfields; an array or a sequence of structures or unions has a Variable `<TypeName>_<index>` per element (§9.2.5.1.2.3, §9.2.5.2.2.3), a collection of collections a Variable `<Name>_<index>` per inner collection (§9.2.5.2.2.5); a collection of structures has the array of their ExtensionObjects as its value.

**Repo:** `crates/opcua-bridge/src/dynamic_value.rs::{flatten, unflatten, element_name}`;
`crates/opcua-bridge/src/dynamic_type.rs::{to_dds_type, bitfields, fixed_of}`;
`crates/opcua-bridge/src/ua_structure.rs::aggregate_values`;
`crates/opcua-bridge/src/address_space_build.rs::{apply_sample, TopicAddressSpace}`;
`crates/opcua-gateway/src/dds_to_ua/walker.rs::DdsType`

**Tests:** `crates/opcua-bridge/src/dynamic_value.rs::tests::{opcuabridge_5_3_long_double_is_a_byte_string_of_sixteen_and_back, opcuabridge_5_3_a_bitset_is_one_leaf_per_bitfield_and_back, opcuabridge_5_3_a_fixed_is_a_decimal_and_back, opcuabridge_5_3_a_sequence_of_structs_is_one_variable_per_element_and_back, opcuabridge_5_3_an_element_is_named_by_its_position_in_each_dimension, opcuabridge_5_3_a_write_to_a_collection_of_numbers_replaces_it, opcuabridge_5_3_a_sequence_of_sequences_is_a_variable_per_inner_sequence_and_back}`;
`crates/opcua-bridge/src/dynamic_type.rs::tests::{opcuabridge_5_3_long_double_is_a_byte_string, opcuabridge_5_3_a_fixed_is_a_decimal, opcuabridge_5_3_a_bitset_is_the_structure_of_its_named_bitfields}`;
`crates/opcua-bridge/src/ua_structure.rs::tests::{opcuabridge_5_3_an_array_of_two_dimensions_is_the_inline_matrix_and_the_older_form_is_read, opcuabridge_5_3_a_collection_of_collections_is_a_field_of_variants, opcuabridge_5_3_every_aggregate_gets_its_value_and_a_write_replaces_it}`;
`crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_5_3_the_elements_of_a_collection_of_structs_are_variables`.

**Status:** done — every form named is built and checked in both directions, the collection of collections also with `asyncua`. The outer Variable of a collection of collections has no value of its own (the daemon's Variant model holds no Variant in a Variant); its elements have theirs.

### §5.3.2 New map key at run time

**Spec:** §5.3 — a `map` key a DDS sample brings and that has no node yet gets it when the sample is applied (a Variable under the map member's node, for a struct value its members under it); in the `PublishedDataSet` it is a field appended at the end with a new `MinorVersion`. At most 4096 entries a topic, keys up to 256 octets.

**Repo:** `crates/opcua-bridge/src/address_space_build.rs::{apply_sample, TopicAddressSpace}`;
`crates/opcua-bridge/src/pubsub_glue.rs::PublisherJob::tick`;
`crates/opcua-pubsub/src/daemon.rs::Publisher::append_field`; `crates/opcua-pubsub/src/writer.rs::DataSetWriter::fields_appended`

**Tests:** `crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_5_3_a_new_map_key_gets_its_nodes_when_a_sample_brings_it`;
`crates/opcua-bridge/src/pubsub_glue.rs::tests::opcuabridge_5_3_a_new_map_key_is_a_field_appended_to_the_published_dataset`;
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_5_3_a_map_key_a_sample_brings_is_a_node_a_client_reads`

**Status:** done — the node is created with the first sample that has the key and stays; a key past either bound and a path under no map are reported as an unmatched path. A client cannot introduce a key through a Write (the daemon does not serve AddNodes). Not tested: keys of a map that is itself the value of a map; a subscriber of another implementation that sees the new `MinorVersion`.

### §5.4 Historical access (§9.3.4)

**Spec:** §5.4 — `opcua.server.historical.topics[]` marks every leaf of a
`dds_to_ua` topic historizing (`Historizing`, the `HistoryRead` bit), keeps
up to `depth` samples per leaf; HistoryRead raw range and AtTime with a
continuation point; `is_read_modified == true` is refused; a `dds_topic`
without a matching `dds_to_ua_connection` is a load-time error; order by
the `INFO_TS` timestamp.

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
**Status:** done — marking, retention, raw range, AtTime, continuation point, the refusal of `is_read_modified` and the load-time error for a `dds_topic` without a `dds_to_ua_connection` are tested.

## §6 OPC UA ↔ DDS bridging behavior

### §6.1 DDS → OPC UA

**Spec:** §6.1 — every `dds_to_ua_connection` topic gets a DDS reader;
every sample is decoded, flattened into named leaf values and applied with
`AddressSpace::set_value`.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop`
(`dds_to_ua_jobs`); `crates/opcua-bridge/src/dds_bridge.rs`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_1_dcps_writer_reaches_the_opcua_client_read`;
`crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_6_1_apply_sample_sets_the_right_node_values`;
`crates/opcua-bridge/src/dds_bridge.rs::tests::opcuabridge_6_1_the_pump_names_xcdr2_the_way_read_body_takes_it`, `tests/integration/tests/bridges.rs::opcua_bridge_against_open62541`.

**Status:** done

### §6.2 OPC UA → DDS

**Spec:** §6.2 — (1) a foreign client writes to the own server: the write hook checks `acl.rules[].subject` per session (denial `Bad_UserAccessDenied`); an allowed write goes into DDS before the client is answered, and the answer is the outcome; (2) client role against configured foreign servers, one MonitoredItem per node, every notification is written to DDS.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop`
(`ua_to_dds_jobs`); `crates/opcua-bridge/src/ua_client.rs::ClientJob`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other}`;
`crates/opcua-bridge/src/ua_client.rs::tests::opcuabridge_1_client_job_writes_a_monitored_scalar_node_into_dds`;
`crates/opcua-bridge/src/dds_bridge.rs::tests::opcuabridge_6_2_write_against_unknown_topic_is_reported`;
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_6_2_a_write_that_does_not_reach_dds_is_answered_bad`;
`crates/opcua-server/src/client.rs::tests::opcuabridge_6_2_a_write_the_commit_refuses_is_answered_bad_and_taken_back`
**Status:** done — both mechanisms are built and tested; the answer to an allowed Write is the outcome of the write into DDS (§6.2.1).

### §6.2.1 Reporting a failed DDS write

**Spec:** §6.2 — the answer to a Write is the outcome of the write into DDS; a value DDS does not take is answered `Bad_CommunicationError` and taken back out of the AddressSpace.

**Repo:** `crates/opcua-server/src/server.rs::{Server::set_write_commit, Server::write_one}`;
`crates/opcua-bridge/src/daemon/server.rs::{start_with_reload, flush_ua_to_dds}`

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_6_2_a_write_that_does_not_reach_dds_is_answered_bad`;
`crates/opcua-server/src/client.rs::tests::opcuabridge_6_2_a_write_the_commit_refuses_is_answered_bad_and_taken_back`
**Status:** done — a Write the AddressSpace accepts goes into DDS through `Server::set_write_commit` before the client is answered. When the value does not encode for the topic's type or DDS does not take it, the answer is `Bad_CommunicationError` and the server puts the value before it back. Tested is the case of a value that does not encode (`string<4>`); a write DDS itself refuses is not reproduced.

## §7 Security

### §7.1 SecureChannel material loading

**Spec:** §7.1 — `opcua.server.security_policies`; own material
`certificate_der`/`private_key_pem`; peer material as two files
`trusted_client_certificate_der`/`trusted_client_public_key_pem`.

**Repo:** `crates/opcua-bridge/src/daemon/security.rs::{load_private_key_pem,
load_public_key_pem, load_der}`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_4_1_secured_basic256sha256_sign_and_encrypt_session_through_the_daemon`
(real RSA-signed X.509 certificates from `rcgen`, loaded via
`load_der`/`load_private_key_pem`/`load_public_key_pem`);
`crates/opcua-bridge/src/daemon/config.rs::tests::opcuabridge_7_1_secured_policy_without_certificate_is_refused`.

**Status:** done

### §7.2 ACL

**Spec:** §7.2 — `acl.rules` (`subject`, `allow_read[]`, `allow_write[]`);
topic exposure at load time and, for writes against the own server, per
session via the write hook; `acl.allow_anonymous`/`acl.users[]` give
distinguishable identities; `client_subject` is `"anonymous"`, the user
name or the token bytes as hex.

**Repo:** `crates/opcua-bridge/src/daemon/security.rs::{Acl, AclOp,
topic_exposed, allow_write, client_subject}`;
`crates/opcua-bridge/src/daemon/config.rs::{AclUserConfig, acl_users,
acl_allow_anonymous}`; `crates/opcua-bridge/src/daemon/server.rs::start_with_reload`.

**Tests:** `crates/opcua-bridge/src/daemon/security.rs::tests::{opcuabridge_7_2_empty_acl_is_open, opcuabridge_7_2_matching_rule_allows_named_topic_only, opcuabridge_7_2_default_deny_with_rules_blocks_unnamed_subject, opcuabridge_7_2_default_deny_false_with_rules_still_opens_unmatched_topics, opcuabridge_7_2_client_subject_renders_each_identity_kind, opcuabridge_7_2_topic_exposed_ignores_the_subject_dimension}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_acl_users_parse_with_default_policy_id, opcuabridge_7_2_acl_user_without_password_is_refused, opcuabridge_3_acl_rules_parse}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other`.

**Status:** done

## §8 Operations + observability

### §8.1 Logging

**Spec:** §8.1 — `log_level` (`trace`…`error`) to stderr; every skip,
every decode failure and every failed DDS write is logged with topic/path.

**Repo:** `crates/opcua-bridge/src/daemon/log.rs`; `crates/opcua-bridge/src/daemon/server.rs`

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_8_1_a_decode_failure_is_logged_by_topic_and_log_level_filters`;
`crates/opcua-bridge/src/daemon/log.rs::tests::opcuabridge_8_1_a_line_below_the_log_level_is_not_written`
**Status:** done — each line carries its level and is written when the level reaches `log_level`; decode failures and failed DDS writes name their topic. Tested are the decode failure with its topic and the filtering of an `info` line at `warn`; the lines of the other skips are not tested one by one.

### §8.2 Prometheus metrics

**Spec:** §8.2 — `metrics.listen` (default `127.0.0.1:9095`) serves
`GET /metrics` with `zerodds_opcua_{dds_to_ua,ua_to_dds}_samples_total`,
`zerodds_opcua_pubsub_{published,received}_total`,
`zerodds_opcua_connections_total`, `zerodds_opcua_errors_total`.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{BridgeMetrics, serve_metrics}`

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_8_2_the_metrics_endpoint_serves_the_six_counters`
**Status:** done — the test reads `GET /metrics` and finds the six counters; `dds_to_ua_samples_total` and `connections_total` count the sample and the connection of the test.

### §8.3 OTLP spans

**Spec:** §8.3 — `metrics.otlp_endpoint` exports one span
`zerodds-opcua-bridged.opc_tcp.accept` (`SpanKind::Server`, peer address as
an attribute) per accepted `opc.tcp` connection; flush every 5 s and on
shutdown.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{otlp_config_from_endpoint,
emit_lifecycle_span, new_trace_id, new_span_id}`;
`crates/opcua-bridge/src/daemon/config.rs::metrics_otlp_endpoint`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_8_3_otlp_endpoint_receives_a_span_on_opc_tcp_accept`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_otlp_endpoint_parses, opcuabridge_8_3_no_otlp_endpoint_leaves_it_none}`.

**Status:** done

## §9 Lifecycle

### §9.1/§9.2 Startup + shutdown

**Spec:** §9.1/§9.2 — startup: IDL, gateway XML, `DcpsRuntime`,
AddressSpace, client/PubSub jobs, then `opc.tcp` and the metrics listener;
SIGTERM/SIGINT set a stop flag, accept loop and metrics loop wake up, all
threads are joined, `ExitCode 0`.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{start_with_reload,
DaemonHandle::shutdown, serve_metrics}`;
`crates/opcua-bridge/src/bin/zerodds-opcua-bridged.rs`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_8_3_otlp_endpoint_receives_a_span_on_opc_tcp_accept}`;
`crates/opcua-bridge/tests/binary_cli.rs::{opcuabridge_9_2_sigterm_ends_the_running_daemon_with_exit_code_zero, opcuabridge_9_2_sigint_ends_the_running_daemon_with_exit_code_zero}`
**Status:** done — startup and shutdown are tested in the process and at the binary: SIGTERM and SIGINT end the running daemon with exit code 0.

### §9.3 SIGHUP

**Spec:** §9.3 — SIGHUP reloads `acl.default_deny`/`acl.rules` from the
same sources; a file that fails to load changes nothing; every other key
needs a restart.

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::{start_with_reload, install_signal_watcher}`

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_9_3_sighup_reloads_the_acl_and_a_failed_load_changes_nothing`
**Status:** done — the test sends SIGHUP to the process: a load that fails changes nothing; the next one puts the new `acl.rules` in force.

## §10 Cross-vendor interop

### §10 open62541

**Spec:** §10 — libraries measured against open62541 1.5.4; at the daemon
level a C client against `libopen62541` drives a running
`zerodds-opcua-bridged`: browse the `Objects` folder, read a `dds_to_ua`
leaf, write a `ua_to_dds` leaf, the write arrives in DDS.

**Repo:** `tests/integration/tests/bridges.rs`,
`crates/opcua-bridge/tests/foreign/open62541_client.c`;
`crates/opcua-server/proofs/open62541.sh`,
`crates/opcua-pubsub/proofs/open62541_uadp.sh`.

**Tests:** `tests/integration/tests/bridges.rs::opcua_bridge_against_open62541` (integration pillar, job
`integration-bridges`, scenario `bridge-opcua`).

**Status:** done

## §11 Packaging

### §11 Package, systemd unit, example configuration, manual pages

**Spec:** §11 — package `zerodds-opcua-bridge`; systemd unit
`packaging/linux/systemd/zerodds-opcua-bridged.service` (`Type=notify`);
example configuration `packaging/linux/configs/opcua-bridged.yaml.example`;
`zerodds-opcua-bridged(1)` + `zerodds-opcua-bridged.yaml(5)`;
deb/rpm/Arch/AppImage entries.

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
**Status:** done — the example configuration loads through the real parser; the unit, the package entries (deb, rpm, Arch, AppImage) and both manual pages are checked by the traceability tests and a test at the binary.

## §12 Testing

### §12.1 Unit tests

**Spec:** §12.1 — `cargo test -p zerodds-opcua-bridge --lib`: type
conversion, flatten/unflatten, IDL compiler, DDS port, AddressSpace,
PubSub glue, client glue, CLI/config/ACL/security.

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

### §12.2 Integration (in-process e2e)

**Spec:** §12.2 — nine e2e cases in `crates/opcua-bridge/tests/daemon_e2e.rs`
against `server::start_with_reload`.

**Repo:** `crates/opcua-bridge/tests/daemon_e2e.rs`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_1_dds_publish_reaches_an_in_tree_pubsub_subscriber, opcuabridge_5_2_node_id_and_browse_path_override_is_applied_not_just_parsed, opcuabridge_5_2_colliding_node_id_override_is_a_load_time_error_not_a_log_line, opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end, opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other, opcuabridge_8_3_otlp_endpoint_receives_a_span_on_opc_tcp_accept, opcuabridge_4_1_secured_basic256sha256_sign_and_encrypt_session_through_the_daemon, opcuabridge_1_external_pubsub_publisher_reaches_the_daemons_own_subscriber}`.

**Status:** done — per the spec unstable under default parallelism
(DDS domain lease) and stable with `--test-threads=1`; case 3 only reports
on macOS instead of checking. Not re-measured here.

### §12.3 Multi-vendor

**Spec:** §12.3 — the scenario `bridge-opcua` of the integration pillar
(§10) against `libopen62541`; `crates/opcua-server/proofs/open62541.sh` and
`crates/opcua-pubsub/proofs/open62541_uadp.sh` at the library level.

**Repo:** `tests/integration/tests/bridges.rs`,
`crates/opcua-server/proofs/open62541.sh`,
`crates/opcua-pubsub/proofs/open62541_uadp.sh`.

**Tests:** `tests/integration/tests/bridges.rs::opcua_bridge_against_open62541`.

**Status:** done

## §13 Cross-references

### §13 References to the library coverage

**Spec:** §13 — `dds-opcua-1.0`, `opcua-client-server-1.05`,
`opcua-pubsub-1.05`, this file.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — list of references.

## §14 Versioning

### §14 Version state

**Spec:** §14 — v1.0 first release; list of the closed and the still open
items.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — version history; the items it lists as
open (the role Objects of OPC 10000-18, `SessionlessInvoke`) are in
OPEN-ITEMS 121.

---

## Audit status

29 done / 0 partial / 0 open / 0 blocked / 2 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 9695 to 9813 of `main` (`python3 ci/coverage_results.py --document zerodds-opcua-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-opcua-bridge --all-targets` (job `merge-rust-opcua-bridge`) — 123 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-gateway --all-targets` (job `merge-rust-opcua-gateway`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-server --all-targets` (job `merge-rust-opcua-server`) — 13 tests green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 9 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-bridge --all-targets --features crypto,pubsub-security` (job `merge-rust-opcua-bridge-features-crypto-pubsub-security`) — 8 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-uacp --all-targets --features crypto` (job `merge-rust-opcua-uacp-features-crypto`) — 2 tests green.
* `python3 ci/run_integration.py integration-bridges` (job `integration-bridges`) — 1 test green.

160 cited tests green, 0 not green.
