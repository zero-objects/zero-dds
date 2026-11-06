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

**Tests:** `crates/opcua-bridge/src/daemon/cli.rs::tests::{opcuabridge_2_empty_args_default, opcuabridge_2_bind_addr_parsed, opcuabridge_2_equals_form, opcuabridge_2_domain_parses_int, opcuabridge_2_idl_is_repeatable, opcuabridge_2_unknown_flag_rejected, opcuabridge_2_sets_render_idl_as_appends, opcuabridge_2_help_short_form}`.

**Status:** partial — checked are `--bind` (also in `=` form), `--domain`,
repeated `--idl` and its rendering as `+opcua.idl=…`, `-h` and the refusal
of an unknown flag. Without a test: the exit codes of `--check-config`,
`--config`, `--gateway-config`, `--log-level`, `--metrics`, `--version` and
the precedence of `--set` over the files.

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

**Spec:** §4.3 — one publisher/subscriber pair per
`opcua.pubsub.connections[]` over UDP; message security (feature
`pubsub-security`) with a static key from
`opcua.pubsub.connections[].security`; a key whose length does not fit the
policy is refused at load time.

**Repo:** `crates/opcua-bridge/src/pubsub_glue.rs::{PublisherJob,
SubscriberJob, set_security}`; `crates/opcua-bridge/src/daemon/config.rs::PubSubSecurityConfig`;
`crates/opcua-bridge/src/daemon/server.rs::build_pubsub_security`.

**Tests:** `crates/opcua-bridge/src/pubsub_glue.rs::tests::{opcuabridge_4_3_a_scalar_topic_is_the_field_value, opcuabridge_4_3_publisher_job_builds_a_pds_field_per_leaf, opcuabridge_4_3_a_secured_publisher_job_delivers_a_dds_sample_to_a_subscriber_job, opcuabridge_4_3_subscriber_with_the_wrong_key_refuses_not_silently_drops}`;
`crates/opcua-bridge/src/daemon/config.rs::tests::{opcuabridge_3_pubsub_security_parses_and_hex_decodes_the_keys, opcuabridge_4_3_pubsub_security_with_wrong_key_length_is_refused, opcuabridge_4_3_pubsub_security_odd_length_hex_is_refused, opcuabridge_4_3_pubsub_security_unknown_policy_is_refused, opcuabridge_4_3_no_security_key_leaves_it_none}`;
`crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dds_publish_reaches_an_in_tree_pubsub_subscriber, opcuabridge_1_external_pubsub_publisher_reaches_the_daemons_own_subscriber}`.

**Status:** partial — publisher/subscriber in both directions and the
static key are built and checked; key rotation via `GetSecurityKeys`/SKS
is missing (§4.3.1).

### §4.3.1 SKS key rotation

**Spec:** §4.3, §14 — message security with a static current key, "not a
live `GetSecurityKeys`/SKS rotation exchange"; §14 lists it as open.

**Repo:** —

**Tests:** —

**Status:** open — the daemon fetches no keys via `GetSecurityKeys` and
does not rotate them; a new key needs a restart (§9.3).

## §5 Topic mapping

### §5.1 TypeObject/TypeLookup → SerializedTypeSupport

**Spec:** §5.1 — type resolution without a compiled Rust type:
`opcua.idl` → `CompleteTypeObject` → `SerializedTypeSupport`; a second
path `dds_bridge::resolve_discovered` via the runtime's TypeLookup
registry.

**Repo:** `crates/opcua-bridge/src/idl.rs::{compile, type_support_for,
is_keyed}`; `crates/opcua-bridge/src/dds_bridge.rs::resolve_discovered`.

**Tests:** `crates/opcua-bridge/src/idl.rs::tests::{opcuabridge_5_1_compiles_a_struct_and_resolves_its_type_support, opcuabridge_5_1_unknown_type_is_reported_by_name, opcuabridge_5_1_broken_idl_is_reported_with_diagnostics}`.

**Status:** partial — the IDL path is checked; `resolve_discovered` has
no test; the daemon's own `getTypes` request is missing (§5.1.1).

### §5.1.1 getTypes request for an unknown type

**Spec:** §5.1 — "This daemon does **not** itself drive a live
cross-participant `getTypes` RPC exchange for a topic whose type is
otherwise unknown."

**Repo:** —

**Tests:** —

**Status:** open — a topic whose type comes neither from `opcua.idl` nor
from the runtime's registry is not bridged; the daemon does not request
the type via TypeLookup.

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

**Spec:** §5.3 — primitives, strings (narrow and wide), enums (ordinal),
bitmasks, nested structs, `map<K,V>` (one leaf per entry,
`<field>.<key>`) and arrays/sequences of these in both directions; shapes
that cannot be mapped are reported by path, never silently dropped.

**Repo:** `crates/opcua-bridge/src/dynamic_value.rs::{flatten, unflatten}`;
`crates/opcua-bridge/src/dynamic_type.rs`.

**Tests:** `crates/opcua-bridge/src/dynamic_value.rs::tests::{opcuabridge_5_3_a_flat_struct_is_one_variable_per_member_and_back, opcuabridge_5_3_nested_struct_flattens_with_dotted_paths, opcuabridge_5_3_partial_unflatten_keeps_the_rest_of_the_default, opcuabridge_5_3_enum_flattens_to_its_ordinal_and_back, opcuabridge_5_3_sequence_of_primitives_is_one_variant, opcuabridge_5_3_sequence_of_struct_is_reported_skipped_not_dropped_silently, opcuabridge_5_3_wide_char_maps_to_uint16_and_back, opcuabridge_5_3_wide_string_maps_to_string_and_back, opcuabridge_5_3_map_of_primitives_decomposes_to_one_leaf_per_key_and_back, opcuabridge_5_3_map_partial_write_keeps_the_untouched_key, opcuabridge_5_3_a_value_nested_past_the_bound_is_refused_not_walked}`;
`crates/opcua-bridge/src/dynamic_type.rs::tests::opcuabridge_5_3_fixed_point_type_is_a_named_unmapped_error`;
`crates/opcua-bridge/src/address_space_build.rs::tests::opcuabridge_5_3_unmatched_path_is_reported_not_dropped`.

**Status:** partial — the listed shapes are mapped and checked; an
array/sequence of struct/union, `bitset`, `fixed<P,S>` and `float128` have
no live value (§5.3.1); a new `map` key gets no node (§5.3.2).

### §5.3.1 Live value for array/sequence of struct/union, bitset, fixed, float128

**Spec:** §5.3 — these shapes get no live value, only a node without a
value; DDS-OPCUA 1.0 §9.2.5 maps arrays and sequences of structures and
unions to one array Variable each (Figure 9.19, 9.20, 9.25, 9.26).

**Repo:** —

**Tests:** —

**Status:** open — `dynamic_value.rs::flatten` reports these shapes by
path and leaves the node without a value; the mapping to an array Variable
or a fitting value is missing.

### §5.3.2 New map key at run time

**Spec:** §5.3 — a `map` key a write introduces is reported as an
unmatched path; "dynamic node/field creation is a tracked follow-up, not
implemented".

**Repo:** —

**Tests:** —

**Status:** open — `AddressSpace` and `PublishedDataSet` build their node
and field set once from the Table 9 default; a key that appears later gets
no node.

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
`crates/opcua-bridge/tests/daemon_e2e.rs::opcuabridge_1_historical_topic_config_makes_history_read_work_end_to_end`.

**Status:** partial — marking, retention, raw range, AtTime, continuation
point and the refusal of `is_read_modified` are checked. Without a test:
the load-time error for a `dds_topic` without a `dds_to_ua_connection`
(`crates/opcua-bridge/src/daemon/server.rs`, "names no dds_to_ua_connection
topic").

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

**Spec:** §6.2 — (1) a foreign client writes to the own server: the write
hook checks `acl.rules[].subject` per session (denial
`Bad_UserAccessDenied`), an allowed write reaches DDS via a 100 ms poll;
(2) client role against configured foreign servers, one MonitoredItem per
node, every notification is written to DDS. A failed DDS write is logged,
the writing OPC UA client is not told ("remains, named, not fixed").

**Repo:** `crates/opcua-bridge/src/daemon/server.rs::pump_loop`
(`ua_to_dds_jobs`); `crates/opcua-bridge/src/ua_client.rs::ClientJob`.

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_4_2_per_session_write_acl_allows_the_named_subject_and_refuses_the_other}`;
`crates/opcua-bridge/src/ua_client.rs::tests::opcuabridge_1_client_job_writes_a_monitored_scalar_node_into_dds`;
`crates/opcua-bridge/src/dds_bridge.rs::tests::opcuabridge_6_2_write_against_unknown_topic_is_reported`.

**Status:** partial — both mechanisms are built and checked; reporting a
failed DDS write back to the client is missing (§6.2.1).

### §6.2.1 Reporting a failed DDS write

**Spec:** §6.2, §14 — the write response to the OPC UA client does not
depend on whether the DDS write succeeds; §14 lists it as open.

**Repo:** —

**Tests:** —

**Status:** open — the path to DDS stays a poll; a failed DDS write is
only logged.

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

**Repo:** —

**Tests:** —

**Status:** open — the messages are `eprintln!` calls in
`crates/opcua-bridge/src/daemon/server.rs`; no test checks `log_level` or a
message.

### §8.2 Prometheus metrics

**Spec:** §8.2 — `metrics.listen` (default `127.0.0.1:9095`) serves
`GET /metrics` with `zerodds_opcua_{dds_to_ua,ua_to_dds}_samples_total`,
`zerodds_opcua_pubsub_{published,received}_total`,
`zerodds_opcua_connections_total`, `zerodds_opcua_errors_total`.

**Repo:** —

**Tests:** —

**Status:** open — `crates/opcua-bridge/src/daemon/server.rs::{BridgeMetrics,
serve_metrics}` is built; no test reads the endpoint.

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

**Tests:** `crates/opcua-bridge/tests/daemon_e2e.rs::{opcuabridge_1_dcps_writer_reaches_the_opcua_client_read, opcuabridge_1_opcua_client_write_reaches_the_dds_reader, opcuabridge_8_3_otlp_endpoint_receives_a_span_on_opc_tcp_accept}`.

**Status:** partial — in-process start (`start_with_reload`) and end
(`DaemonHandle::shutdown`) are checked.
Without a test: signal handling (SIGTERM/SIGINT) and exit code 0 of the
binary.

### §9.3 SIGHUP

**Spec:** §9.3 — SIGHUP reloads `acl.default_deny`/`acl.rules` from the
same sources; a file that fails to load changes nothing; every other key
needs a restart.

**Repo:** —

**Tests:** —

**Status:** open — the reload thread in
`crates/opcua-bridge/src/daemon/server.rs::start_with_reload` is built; no
test sends SIGHUP.

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

**Tests:** `crates/opcua-bridge/tests/example_yaml_loadback.rs::opcuabridge_3_shipped_example_config_loads_back`.

**Status:** partial — the example configuration loads through the real
parser. Without a test: unit, manual pages and package entries;
`tools/traceability/tests/deployment_layout.rs` and
`tools/traceability/tests/bridge_config_manpages.rs` do not have the OPC UA
daemon in their bridge list.

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
open are items §4.3.1, §5.3.1, §5.3.2 and §6.2.1.

---

## Audit status

13 done / 8 partial / 8 open / 0 blocked / 2 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-opcua-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-opcua-bridge --all-targets` (job `merge-rust-opcua-bridge`) — 80 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-gateway --all-targets` (job `merge-rust-opcua-gateway`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-server --all-targets` (job `merge-rust-opcua-server`) — 10 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-bridge --all-targets --features crypto,pubsub-security` (job `merge-rust-opcua-bridge-features-crypto-pubsub-security`) — 3 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-uacp --all-targets --features crypto` (job `merge-rust-opcua-uacp-features-crypto`) — 2 tests green.
* `python3 ci/run_integration.py integration-bridges` (job `integration-bridges`) — 1 test green.

100 cited tests green, 0 not green.

Open items: `zerodds-opcua-bridge-1.0.open.en.md`.
