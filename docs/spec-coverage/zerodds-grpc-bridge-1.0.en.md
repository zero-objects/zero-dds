# `zerodds-grpc-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-grpc-bridge-1.0.md`

Implementation:

- `crates/grpc-bridge/` — DDS↔gRPC bridge.

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — assignment of the levels; the requirements are the items §1a to §12.

### §1a L6 multi-tenant

**Spec:** §1 L6 — several DomainParticipants per daemon; one tenant
binding per gRPC connection.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/` (`TenantConfig`, `DaemonConfig::from_config`, `Participant`, `participant_of`, `serve_tls`, `security_from`, `SpecMetrics::register`).

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_1_l6_a_tls_connection_is_bound_to_the_tenant_of_its_certificate`,
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_1_l6_the_tenants`,
`::grpcbridge_1_l6_a_subject_is_bound_to_one_tenant_at_most`,
`::grpcbridge_1_l6_the_acl_of_a_tenant_topic_carries_its_name`,
`::grpcbridge_1_l6_tenants_need_tls_with_a_client_ca`,
`::grpcbridge_3_every_key_of_the_spec_example_reaches_the_config`,
`crates/grpc-bridge/tests/daemon_cli_e2e.rs::grpcbridge_2_exit_3_when_the_participant_does_not_start`.

**Status:** done — an entry of `tenants` is a DomainParticipant on its domain with its own topics. A TLS connection whose client certificate (subject DN or `mtls:` fingerprint) a tenant names is bound to it for its life: its `Publish` reaches a DDS reader on the tenant's domain (checked in the test), its `Catalog` lists only the tenant's topics, the topics of `domain` are `NOT_FOUND` to it. A connection without a certificate and one with a certificate no tenant names see the topics of `domain`. ACL entries and the `dds_topic`/`service` labels are `<tenant>/<name>`. Checked with the subject DN; the fingerprint binds through the same function and is checked in the unit test only. `tenants` without `grpc.tls.enabled` and `grpc.tls.client_ca_file` is a configuration error with file and line (exit 1, also under `--check-config`), since without both no connection carries a client certificate.

## §2 CLI surface

### §2 zerodds-grpc-bridged CLI

**Spec:** §2 — options
`--config`/`--bind`/`--domain`/`--tls-*`/`--reflection`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/mod.rs` (`run`, `DaemonError`, `log_event!`, `LOG_LEVEL`), `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs` (`main`).

**Tests:** `crates/grpc-bridge/tests/daemon_cli_e2e.rs::grpcbridge_2_version_and_help_exit_0`,
`::grpcbridge_2_exit_1_on_an_unknown_argument_and_an_invalid_file`,
`::grpcbridge_2_exit_2_when_the_port_is_in_use`,
`::grpcbridge_2_exit_3_when_the_participant_does_not_start`,
`::grpcbridge_2_exit_4_when_the_tls_material_does_not_load`,
`::grpcbridge_2_each_short_form_wins_over_the_file`,
`::grpcbridge_2_check_config_refuses_what_the_start_refuses`;
`crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_2_log_level_filters_the_lines`,
`::grpcbridge_9_2_sigterm_sends_goaway_with_the_last_stream_and_exits_0` (exit 0);
`crates/grpc-bridge/tests/contract_dcps.rs::a_convenience_flag_pushed_as_set_ranks_above_the_daemon_file`,
`::the_usage_text_names_the_configuration_flags`.

**Status:** done — each short form is `--set` of its file key and wins over the file (checked with its origin under `--check-config`). Exit 0 after `--version`, `--help`, a valid `--check-config` and SIGTERM; 1 on an unknown argument, a file that does not load, an unknown key, a wrong type, a value outside its range, `--tls-cert` without `--tls-key` and on §7 settings that cannot be served (`auth.mode: bearer` without `auth.bearer_token`, `jwt` without a usable key, `mtls` without TLS and a client CA, `tenants` without TLS and a client CA); `--check-config` refuses the same cases with exit 1, checked in the test against the start; the TLS material is loaded by the start only; 2 when the port is in use; 3 when the participant (also a tenant's) does not start; 4 on missing or unreadable TLS material. `--log-level` sets the threshold of the event lines on stderr. `--topic` takes `DDS=SERVICE` or `DDS:SERVICE`, as the synopsis; the default service name of §5.1 belongs to §5.1.

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`log_level`/`grpc`/`auth`/`topics`/`metrics`/`tenants`;
the limits `grpc.max_concurrent_streams`, `max_recv_message_size`, `max_send_message_size`,
`initial_window_size`, `max_frame_size`, `grpc.keepalive.*`, `grpc.tls.alpn`, `metrics.path`;
ENV substitution.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/` (`DaemonConfig::from_config`, `read_topic`,
`handle_connection`: keepalive, refusals), `crates/grpc-bridge/src/server.rs` (`ServerLimits`,
`GrpcServer::handshake`, windows, `REFUSED_STREAM`, `RESOURCE_EXHAUSTED`, PING rules),
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

**Status:** done — the example of the spec loads, each of its keys arrives with its value. The limits act on the wire (measured against the daemon): the SETTINGS carry `max_concurrent_streams`, `initial_window_size` and `max_frame_size`, a WINDOW_UPDATE opens the connection window, DATA beyond a stream's window gets `RST_STREAM(FLOW_CONTROL_ERROR)` (in the unit test), a stream window below 65 535 applies only after the ACK of the SETTINGS (RFC 9113 §6.9.3, in the unit test), the padding of PADDED DATA is not part of the request (in the unit test), a stream above the limit gets `RST_STREAM(REFUSED_STREAM)`, a message above `max_recv_message_size` or `max_send_message_size` ends with `RESOURCE_EXHAUSTED`, the daemon sends a PING after `time_secs` and GOAWAY after `timeout_secs` without ACK, PINGs without a call end in `GOAWAY(ENHANCE_YOUR_CALM)` after the third strike, with `permit_without_calls: true` a connection without a call stays open beyond the 5 s (watched for 6.5 s), `metrics.path` serves the Prometheus page. `qos.history` with `keep_all` gives KEEP_ALL (`usize::MAX`), `keep_last` without a depth leaves the runtime's default; `keep_all` with `depth` stops the start, the message names `history.depth` (unit test). `tls.alpn` takes `h2` and `http/1.1` (default both, `h2` first); an empty list, an entry named twice, another protocol and a list without `h2` stop the start (unit test). Measured only against the test client of this repository; flow control on the sending side (the client's windows) is built and checked under §4.1.

## §4 gRPC wire protocol

### §4.1 HTTP/2 setup with ALPN h2

**Spec:** §4.1 — an HTTP/2 server, ALPN `h2`, cleartext h2c only in dev
mode (`bind=127.0.0.1`); SETTINGS frame, GOAWAY on shutdown.

**Repo:** `crates/grpc-bridge/src/server.rs` (frames, flow control in both
directions, CONTINUATION, connection and stream errors),
`crates/grpc-bridge/src/bridge_daemon/` (`transport_settings`,
`bind_is_loopback`, the ALPN check in `serve_tls`, GOAWAY at the drain),
`crates/hpack/src/decoder.rs` (the limit of the decoded header list and of
a dynamic table size update).

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

**Status:** done — measured against the daemon on the Linux test host: 1 MiB of samples of 16 KiB (each message larger than a frame) on one connection arrive whole, the client within the server's windows, the server within the windows the subscriber grants with WINDOW_UPDATE; without WINDOW_UPDATE the server stops at the 65 535 octets of the stream window and goes on after it. Cleartext h2c on a bind that is not loopback stops the start and `--check-config` (exit 1); a TLS client without ALPN that sends the HTTP/2 preface gets no SETTINGS, one with only `http/1.1` gets the HTTP/1.1 endpoint of §5.2 (the preface as a request of the method PRI: 405), with `grpc.tls.alpn: ["h2"]` it fails the handshake; GOAWAY(NO_ERROR) with the last stream at SIGTERM. CONTINUATION in both directions, padding and priority of HEADERS, unknown frame types, PUSH_PROMISE (`GOAWAY(PROTOCOL_ERROR)`), a header block above 1 MiB in HEADERS and CONTINUATION and in a single HEADERS frame (with `max_frame_size` above 1 MiB; `GOAWAY(ENHANCE_YOUR_CALM)`, a block of exactly 1 MiB is taken) and a smaller HPACK table of the client in the unit tests. The SETTINGS name `MAX_HEADER_LIST_SIZE` 16 384; a request whose decoded header list is above it ends with `RESOURCE_EXHAUSTED`, its block is decoded to the end without holding the fields, and the next request on the same connection refers to its literal (exactly 16 384 octets are a call, one more is not; unit test). Against the daemon with the block of the review (a literal of 4 000 octets, then 60 000 references to it): `RESOURCE_EXHAUSTED`, VmHWM 19 776 → 20 300 kB (before the limit 12 904 → 491 680 kB). A dynamic table size update above 4 096 is `GOAWAY(COMPRESSION_ERROR)`, one after a header field a decoding error (unit tests in the server and in zerodds-hpack). A `Subscribe` stream whose client grants no window takes samples from its DataReader only until 256 KiB wait for the window (measured against the daemon through `zerodds_grpc_bridge_bytes_out_total`: at most the 65 535 octets of the window plus 256 KiB plus one message; of 40 samples of 16 KiB not all); after the WINDOW_UPDATE all 40 arrive in order. Measured against the test client of this repository; grpc-go and grpc-java against the daemon belong to §10a.

### §4.2 Service-definition auto-generation per topic

**Spec:** §4.2 — a `<TopicSlug>Stream` service with
`Publish`/`Subscribe`/`PublishOne`/`Catalog` RPCs;
`Sample`/`PublishAck`/`SubscribeReq`/`CatalogReq`/`CatalogResp`/`TopicEntry`
messages.

**Repo:** `crates/grpc-bridge/src/messages.rs` (the six messages),
`crates/grpc-bridge/src/service_gen.rs` (`render_proto`, the four methods),
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

**Status:** done — `render_proto` writes the text of the spec (compared in the test). The daemon reads and writes the six messages: `Publish` applies `key_hash`, `source_ts_ns` and the dispose and unregister flags and refuses a `type_name` or `topic_name` that is not the mapping's; `PublishAck` names the sequence number of the last write and the time; a `Subscribe` sample carries all six fields (key, timestamp, type and topic through a DDS writer and reader of the test), with `include_dispose` also the dispose with its flag; a `source_ts_ns` above 4 294 967 295 999 999 999 ns (the RTPS timestamp cannot hold it) is `INVALID_ARGUMENT` instead of a write without a timestamp (unit test); against the daemon: without `include_dispose` a dispose does not go on the stream, with `include_dispose` it arrives between the samples with its flag and key and no payload; `partition_filter` `"x, a*"` becomes the partitions of the stream's reader (the sample of a writer in partition `ab` arrives, the one of the daemon's writer in the default partition does not); `Catalog` answers a `CatalogResp`, `TopicEntry.idl_url` names the IDL endpoint of §5.2. `initial_burst` sends the retained samples first (§6, against the daemon). Measured against the test client of this repository.

### §4.3 RPC flows publish/subscribe

**Spec:** §4.3 — HEADERS/DATA/HEADERS-END_STREAM flow with `:method=POST`,
`:path=/<pkg>.<svc>/<method>`,
content-type/grpc-encoding/grpc-timeout/authorization; streaming per
sample.

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

**Status:** done — measured against the daemon: a `Publish` client stream of 100 samples writes 100 samples in order (each on a `Subscribe` stream) and answers once, at the half-close, with sequence number 100; `PublishOne`, `Subscribe` and `Catalog` take one request message (a second is `INTERNAL`, none is the empty message); content-type `text/plain` and a content-type with a multi-byte UTF-8 character across its 16th octet are HTTP 415 with `grpc-status` 3 (the connection stays open), `:method GET` 405 with 12, `application/grpc+json` 12; `grpc-timeout` is the deadline (§4.5). `authorization` is §7.2. Measured against the test client of this repository.

> **F2b — Java multi-process bridge:** the pure-Java gRPC client
> `org.zerodds.bridge.GrpcBridgeClient` (`crates/java-omgdds/java`)
> speaks the same gRPC wire (a minimal raw HTTP/2 framer over a socket,
> since the daemon is prior-knowledge h2c). Java↔Rust e2e
> `GrpcBridgeClientTest` + `crates/java-omgdds/run_grpc_bridge_e2e.sh`
> (3/3 green): Java publishes via gRPC → DDS → Java subscribes back.
> Realises the listener-callbacks §7.3 "gRPC bridge path (pure-Java via
> vendor extension)".

### §4.4 Length-prefix wrapper

**Spec:** §4.4 — a 5-byte header (compr 1B + length 4B BE) + protobuf
sample payload; compr 0=identity / 1=compressed (gzip/deflate via
`grpc-encoding`).

**Repo:** `crates/grpc-bridge/src/frame.rs`,
`crates/grpc-bridge/src/compression.rs` (gzip RFC 1952, deflate RFC 1950,
the inflate bound), `crates/grpc-bridge/src/bridge_daemon/calls.rs`
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

**Status:** done — measured against the daemon: a gzip-compressed `Publish` reaches a `Subscribe` stream byte for byte, the `PublishAck` comes compressed with gzip and `grpc-encoding: gzip`, a `Subscribe` with `grpc-encoding: deflate` gets its messages compressed with deflate; a message compressed as `snappy` is `UNIMPLEMENTED` with `grpc-accept-encoding: identity,deflate,gzip`, uncompressed messages under `snappy` are read; the flag 1 without an encoding is `INTERNAL`; the inflated size is bounded by `max_recv_message_size` (unit test). A gzip message of two members is read whole, the bound counting both; octets after the last member, a second member cut short and octets after the zlib stream are `Corrupt` (unit test). The compression of both sides is flate2; a check against grpc-go (`UseCompressor(gzip)`) belongs to §10a.

### §4.5 Status mapping OK/INVALID_ARGUMENT/PERMISSION_DENIED/...

**Spec:** §4.5 — gRPC status codes 0/3/4/7/8/13/14/16 → DDS conditions.

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

**Status:** done — each row is produced: `OK`; `INVALID_ARGUMENT` for a message that is not proto3 and for a `cdr_payload` without a CDR encapsulation header (unit test); `PERMISSION_DENIED` and `UNAUTHENTICATED` against the daemon (§7.2, §7.3); `RESOURCE_EXHAUSTED` for a write of a TRANSIENT_LOCAL writer whose RESOURCE_LIMITS are reached (with a DDS writer in the unit test) and for the §3 limits; `DEADLINE_EXCEEDED` for a `Subscribe` and a `Publish` stream past their `grpc-timeout`; `UNAVAILABLE` for the streams open at SIGTERM and for a `Subscribe` stream whose one DataWriter (MANUAL_BY_TOPIC, lease 1 s) stops asserting, after its sample (against the daemon); `INTERNAL` for other DDS errors and the request errors of §4.3/§4.4. "CDR decode error": the CDR bytes after the encapsulation header are passed on as they are (§4.4), the bridge registers no type and does not decode them. `grpc-message` is percent-encoded.

### §4.6 Reflection service grpc.reflection.v1alpha

**Spec:** §4.6 — the standard reflection service with auto-generated topic
services + sample message types.

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
(also another method of the reflection service: UNIMPLEMENTED),
`::grpcbridge_4_6_a_reflection_stream_answers_no_request_once_256_kib_wait_for_the_window`;
`crates/grpc-bridge/src/server/tests.rs::grpcbridge_4_6_a_held_stream_returns_no_window_until_released`;
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_list_services_reflection_payload_is_well_formed`;
against grpcurl (job `integration-bridges-vendor-grpc`):
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_grpcurl_describe_names_the_four_methods`,
`::grpcbridge_4_6_grpcurl_lists_the_configured_package`,
`::grpcbridge_4_6_grpcurl_list_services_against_zerodds_daemon`,
`::grpcbridge_4_6_grpcurl_lists_the_reflection_service_itself`.

**Status:** done — the daemon serves the reflection service under `grpc.reflection.v1` and `grpc.reflection.v1alpha` as a bidirectional stream: each request gets its response on the open stream (against the daemon: `list_services`, `file_containing_symbol` of a method, an unknown symbol as `error_response` NOT_FOUND, then the half-close with OK; a request without content ends the stream with `INVALID_ARGUMENT`; another method than `ServerReflectionInfo`, e.g. `/grpc.reflection.v1.ServerReflection/Foo`, ends with `UNIMPLEMENTED` and no response message). The `FileDescriptorProto` of each package carries the six messages of §4.2 and the package's services with the four methods; `valid_host` and `original_request` repeat the request. Against grpcurl 1.9.3 (grpc-go) on the Linux test host: `describe` of the service names the four methods with their streaming, `describe` of `Sample` the six fields, `describe` without a symbol every listed service including reflection, a `Catalog` call grpcurl builds from the descriptors alone (`-d '{}'`) answers with the topic; `list` names the configured `grpc_package`. A reflection stream whose client does not read answers no further request once 256 KiB of responses wait for its window (so it holds at most 256 KiB and the last response, up to `max_send_message_size`); the requests after them wait, and the server returns none of the stream's receive window meanwhile (against the daemon with `initial_window_size` 65 535 and 3 000 requests of 56 octets: the client could send 1 170, 314 responses of 1 045 octets were queued, that is at most the client's window, 256 KiB and one response; after the client's WINDOW_UPDATE the other requests went out, all 3 000 were answered, the half-close behind them ended with OK).

## §5 Topic mapping

### §5.1 Service-name default Topic→ChatMessageStream

**Spec:** §5.1 — `::` split + CamelCase + suffix `Stream`; package
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
(the default service listed through reflection).

**Status:** done — `Chat::Message` becomes `ChatMessageStream` in `zerodds.chat.v1`; an entry without `grpc_service` and `grpc_package` (map form and `--topic DDS`) gets the defaults, the fields override them, a dotted `grpc_service` is `<package>.<Service>`; a `:path` with a package reaches the service only in its package; a service name without a package that mappings in two packages have reaches neither (`NOT_FOUND`, the message names the packages), with the package it reaches each; two entries of one fully-qualified service and a `grpc_service` that is no protobuf identifier stop the start (unit tests). Against the daemon reflection lists the default service in the default package.

### §5.2 Type discovery reflection + catalog + IDL endpoint

**Spec:** §5.2 — three paths (reflection service, catalog RPC, HTTP-GET
sidekick).

**Repo:** `crates/grpc-bridge/src/bridge_daemon/` (`idl_answer`,
`serve_http1`, `serve_tls` (ALPN `http/1.1` or none), `is_http1_start`, the GET branch of `Conn::on_open`, `catalog_resp`, `idl_base`,
`topics[].idl_file` in `read_topic`), `crates/grpc-bridge/src/server.rs`
(`respond_http`, GET with any path, `GrpcCall::authority`),
`crates/grpc-bridge/src/daemon_runtime.rs` (`CatalogTopic::grpc_service`);
reflection: §4.6.

**Tests:** `crates/grpc-bridge/tests/rpc_flows_e2e.rs::grpcbridge_5_2_get_idl_returns_the_source_as_text_x_omg_idl`,
`::grpcbridge_5_2_get_idl_of_an_unknown_topic_is_404`,
`::grpcbridge_5_2_the_catalog_rpc_names_dds_type_service_and_idl_url`;
`crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_5_2_idl_over_tls_on_the_h2_connection`,
`::grpcbridge_5_2_idl_over_tls_with_http_1_1`;
`crates/grpc-bridge/src/server/tests.rs::grpcbridge_5_2_a_get_is_answered_with_a_plain_response`;
`crates/grpc-bridge/src/bridge_daemon/catalog.rs::tests::grpcbridge_5_2_build_catalog_emits_topics`;
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_5_2_catalog_render_json_is_valid_shape`;
with curl (job `integration-bridges-vendor-grpc`):
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_5_2_curl_fetches_the_idl`,
`::grpcbridge_5_2_curl_over_tls_fetches_the_idl_with_http_1_1_and_h2`.

**Status:** done — against the daemon: `GET /idl/<dds_name>` returns the text of `idl_file` as `text/x-omg-idl`, as HTTP/1.1 on the cleartext listener and as an HTTP/2 GET on a gRPC connection (percent-encoded too); over TLS on the h2 connection and as HTTP/1.1 on a TLS connection with ALPN `http/1.1` or without ALPN, under the authentication of §7.2 (without the token 401; the test's rustls client); a topic that is not mapped and one without `idl_file` 404, a malformed escape 400, another method 405. `Catalog` names `dds_type`, the fully-qualified service and `idl_url` with the `:authority` of the call, and fetching it returns the IDL. curl 8.14.1 on the Linux test host fetches the IDL with `--http1.1` and `--http2-prior-knowledge`, over TLS with `--http1.1` (ALPN `http/1.1`) and `--http2` (ALPN `h2`). The daemon does not parse the IDL file; it reads it at the start.

## §6 QoS translation

### §6 DDS-QoS → gRPC-behavior map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition
map; `partition_filter` as a subscribe field.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/`
(`dds_pump`: `writer_cfg`, `reader_cfg`, `Retained`, `RetainedStore`,
`refresh_retained`, `burst_start`, `retained_after`, `lifespan_writer`,
`deadlines_missed`; `BurstJoin`, `Conn::start_stream`, `Conn::pump`,
`Conn::drop_best_effort`, `log_dropped`, `Conn::log_deadlines`, `Conn::finish`, `Conn::release`,
`lifespan_of`, `register_writer`; `qos.deadline_ms`, `qos.lifespan_ms`, `qos.history` in `read_topic`),
`crates/grpc-bridge/src/server.rs` (`close_call_with`); keepalive:
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
`::grpcbridge_4_2_partition_filter_is_the_partition_of_the_streams_reader` (partition),
`::grpcbridge_4_5_liveliness_lost_ends_the_stream_unavailable`;
`crates/grpc-bridge/tests/daemon_limits_e2e.rs::grpcbridge_3_keepalive_pings_and_closes_without_an_ack`,
`::grpcbridge_3_keepalive_with_acks_keeps_the_connection` (liveliness);
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs::tests::grpcbridge_6_the_retained_limit_is_shared_by_the_topics_of_a_participant`,
`::grpcbridge_6_the_retained_samples_keep_depth_per_instance`,
`::grpcbridge_6_a_burst_join_sends_each_change_once`,
`::grpcbridge_6_a_lifespan_writer_waits_outside_the_pool_lock`,
`::grpcbridge_6_the_history_of_a_topic_is_the_history_of_its_writers`.

**Status:** done — each row against the daemon on the Linux test host: RELIABLE takes no further sample once 256 KiB wait for the client's window, the rest stays in the DataReader; a BEST_EFFORT stream without a window drops what its reader holds, counts it in `zerodds_grpc_bridge_samples_dropped_total` and logs `samples_dropped` at `warn` (visible at the default level `info`; at most one line a second per stream, the rest at the end of the stream, the lines summing to the counter); TRANSIENT_LOCAL with `history.depth` 3: of five samples `initial_burst` 2 brings the last two, 10 the last three, then each live sample exactly once; two instances with `key_hash` at depth 2: the burst holds the last two of each in the order they arrived, after the dispose of one instance only the other; a burst of 640 KiB to a client without a window stays within the window plus 256 KiB plus one message and arrives whole and in order after the WINDOW_UPDATE; a DDS writer of a second participant writing every 2 ms while a client subscribes with `initial_burst`: the stream delivers each sample exactly once and without a gap (counter-check with a burst from the snapshot alone: a gap in 1 of 3 runs, 86 → 88); the limit of 10 000 holds for all topics of a participant together, the oldest of all topics going first (unit test with limit 3 and two topics); `initial_burst` with a `partition_filter` is `INVALID_ARGUMENT`, a VOLATILE topic has an empty burst; `qos.lifespan_ms` is the LIFESPAN of the writer, a `Publish` with `zerodds-lifespan-ms: 1500` writes with a writer of that LIFESPAN (a DDS reader of the test sees both publications with 60 000 and 1 500 ms), without the metadata `grpc-timeout` is the lifespan rounded up to whole seconds (`4500m` gives a writer of 5 000 ms), a value that is no number `INVALID_ARGUMENT`; ten distinct `grpc-timeout` values (30 s down to 21 s) on a VOLATILE topic all write, and the DDS reader then sees the eight newest lifespans beside the topic writer's (the two least recently used were deleted after their acknowledgement); on a TRANSIENT_LOCAL topic a ninth lifespan while none of the eight writers expired is `RESOURCE_EXHAUSTED`; the waits of making and replacing run outside the pool's lock (unit test with a DDS reader that vanishes without notice: while the ninth lifespan waits 1 s for the acknowledgements of the oldest writer, an existing writer comes back in under 300 ms, a call of the lifespan being replaced waits for the end and gets the old writer back, the ninth lifespan ends with `OutOfResources`; the test waits until the slot of the lifespan being replaced is `Evicting` instead of a fixed pause); `qos.deadline_ms` 100 ms: a stream whose writer falls silent after one sample ends with the trailer `zerodds-deadline-missed` ≥ 1 and the log line `deadline_missed`; liveliness is the PINGs of `grpc.keepalive`; partition is `partition_filter`; `qos.history` is the HISTORY of the topic's writer and of its writers of other lifespans: `keep_all` KEEP_ALL, `keep_last` with depth 7 the depth 7, without a depth KEEP_LAST 1 (unit test against the runtime); an error of `set_user_writer_history_depth` stops the start (from the code, without a test: the runtime refuses the depth only for an unknown writer or a poisoned lock). Measured against the test client of this repository and ZeroDDS DDS participants.

## §7 Security

### §7.1 TLS h2-ALPN + cert rotation

**Spec:** §7.1 — TLS 1.2+ (1.3 default), ALPN `h2`, cipher suites per
`grpc.tls`, SIGHUP cert rotation.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/connection.rs::serve_tls`,
`::security_from`, `::apply_reload`, `grpc.tls.min_version` and
`grpc.tls.cipher_suites` in `DaemonConfig::from_config`;
`crates/grpc-bridge/src/daemon_runtime.rs::build_grpc_tls_config_opts`,
`::select_tls_suites`, `::provider_cipher_suites`
(`grpc.tls.client_ca_file`, `grpc.tls.require_client_cert`).

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_7_1_mtls_serves_h2_and_the_certificate_is_the_subject`
(ALPN `h2` read at the client),
`::grpcbridge_7_1_a_sighup_serves_the_next_connection_with_the_new_certificate`,
`::grpcbridge_7_1_the_configured_cipher_suites_are_the_ones_offered`,
`::grpcbridge_7_1_min_version_1_3_refuses_a_tls_1_2_client`,
`::grpcbridge_7_1_an_unknown_suite_stops_the_start` (the empty list too);
against grpc-go `crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_7_1_grpcurl_over_tls_with_a_bearer_token`
(grpcurl 1.9.3, Linux test host);
`crates/grpc-bridge/tests/security_e2e.rs::grpcbridge_7_1_build_grpc_tls_config_sets_alpn_h2`,
`::grpcbridge_7_1_build_grpc_tls_config_with_client_ca_for_mtls`, `::grpcbridge_7_1_build_grpc_tls_config_rejects_missing_cert`,
`::grpcbridge_7_1_the_configured_cipher_suites_are_the_ones_in_the_config`,
`::grpcbridge_7_1_an_unknown_suite_does_not_load`.

**Status:** done — against the daemon with a rustls client: without `cipher_suites` a client with TLS 1.2 and 1.3 negotiates TLS 1.3, one with TLS 1.2 only is served with TLS 1.2; `min_version: "1.3"` fails the handshake of a TLS 1.2 client; with `cipher_suites` the server's order wins (ChaCha20 before AES-128 against a client that offers AES first), a suite outside the list fails the handshake, TLS 1.2 takes the listed ECDHE-ECDSA suite; a suite the build does not offer and an empty list `cipher_suites: []` stop the start and `--check-config` (exit 1). ALPN offers `h2` and `http/1.1`; `http/1.1` serves only the IDL endpoint of §5.2. ALPN `h2` and the rotation on SIGHUP as before. A check with `openssl s_client` was not run.

### §7.2 Auth modes none/jwt/mtls

**Spec:** §7.2 — JWT-bearer authorization metadata (RS256/ES256), mTLS cert
DN, `none` only on a loopback bind; §3 `auth.jwt.public_key`, `audience`,
`required_claim`.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/calls.rs` (`Conn::on_open`:
checked per call on the `authorization` metadata, `UNAUTHENTICATED`),
`crates/grpc-bridge/src/bridge_daemon/auth.rs` (`security_from`,
`security_settings`: `none` on a `grpc.bind` that is not a loopback
address stops the start and `--check-config`, exit 1; `apply_reload`:
SIGHUP checks `none` against the `grpc.bind` the listener serves, a
changed bind waits for the restart);
`crates/grpc-bridge/src/server.rs` (`GrpcCall::metadata`);
`crates/bridge-security/src/auth.rs` (`JwtKey` with `Rs256` and `Es256`,
`jwt_key_from_pem`: an RSA key, PKCS#1 or SPKI rsaEncryption, verifies
RS256, an SPKI id-ecPublicKey on P-256 verifies ES256, another curve stops
the start; a header `alg` other than the key's refused, `exp`/`nbf` with
60 s leeway, `aud`, required claim; taken from `main`, the mechanism of the
WebSocket bridge). Also `bearer` with
`auth.bearer_token`/`auth.bearer_subject`. The mTLS subject is the
certificate's subject DN (RFC 4514, `CN=alice`); an ACL rule may name the
fingerprint `mtls:<SHA-256>` instead.

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

**Status:** done — the ES256 tokens of the tests are signed by
`openssl dgst -sha256 -sign` (OpenSSL 3 on the Linux test host), the
daemon checks them; an RS256 header over a P-256 key, an ES256 header over
an RSA key and a P-384 key are refused. `auth.mode: none` with TLS on
`0.0.0.0` stops the start with exit 1 and is refused by `--check-config`;
on `127.0.0.1` it starts. A daemon on `0.0.0.0` with TLS and `bearer` whose
file a SIGHUP reload moves to bind `127.0.0.1` with `none` refuses the
reload (`reload_refused`): a call without a token stays `UNAUTHENTICATED`,
the token still passes. Before, this reload switched `none` on for the
listener still bound to `0.0.0.0`.

### §7.3 Per-topic ACL

**Spec:** §7.3 — `acl.publish`/`acl.subscribe` lists with subject
resolution.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/dispatch.rs::route`
(`Publish` = write, `Subscribe` = read on the DDS topic,
`PERMISSION_DENIED`); `topics[].acl` in `DaemonConfig::from_config`.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_7_2_bearer_and_acl_are_checked_per_call`,
`::grpcbridge_7_2_jwt_checks_signature_time_audience_and_scope` (JWT `sub` as the subject),
`::grpcbridge_7_1_mtls_serves_h2_and_the_certificate_is_the_subject` (certificate as the subject).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/log.rs` (`event`, `Line`,
`call_line`, `log_call`: one JSON object per line on stdout, first
`timestamp` per RFC 3339 in UTC with milliseconds, `level`, `event`; text
escaped per RFC 8259 §7, numbers as numbers; threshold `log_level`),
`crates/grpc-bridge/src/bridge_daemon/calls.rs::Conn::ended` (one `rpc` line
per call that ended with `peer`, `service`, `method`, `dds_topic`, `bytes`,
`grpc_status`, `latency_us`, `stream_id`; `info` for OK, otherwise `warn`
with `grpc_message`; a call the client reset and one whose connection ends
before its status as CANCELLED),
`crates/grpc-bridge/src/bridge_daemon/catalog.rs::log_http` (the `http` line
of the IDL endpoint over HTTP/1.1 and h2). Every other event takes the same
path.

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
`crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_2_log_level_filters_the_lines`.

**Status:** done — against the daemon: a Publish and a call to a service
that is not mapped give exactly two `rpc` lines on stdout, every line on
stdout begins with an RFC 3339 timestamp in UTC; the Publish line carries
the client's peer, `dds_topic` `T::Open`, `grpc_status` 0 and `bytes` = the
13 octets of the request plus the length of the PublishAck with its prefix;
the second line is `warn` with `grpc_status` 5 and `grpc_message`. stderr
carries no log line. With `--log-level warn` over a file with `log_level:
debug` the line of the OK call is missing, every line is `warn` or `error`.
A Publish the client resets with RST_STREAM(CANCEL) after its HEADERS, and a
Subscribe whose connection the client closes, each give one `rpc` line at
`warn` with `grpc_status` 1, the peer and `dds_topic` `T::Open`
(`grpc_message` with `RST_STREAM` and `the connection closed`), and
`zerodds_grpc_bridge_rpcs_total{…,status="CANCELLED"}` is 1 for `Publish`
and for `Subscribe`. A `GET /idl/T::Open` over HTTP/1.1 and over h2 each
give one `http` line at `info` with the peer, `protocol` `http/1.1` and
`h2`, `method` `GET`, `path`, `http_status` 200, `bytes` = the length of
the IDL and `latency_us`; a `GET /idl/Nothing` is `warn` with `http_status`
404; none of the requests gives an `rpc` line.
### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + 10 counter/gauge families.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/metrics.rs::SpecMetrics`
(all ten `zerodds_grpc_bridge_*` families). Label values come only from the
configuration or a fixed set: a service that is not mapped counts as
`service="unknown"`, a foreign method as `method="other"`.
`bytes_in`/`bytes_out` count the Length-Prefixed-Message including its
5-octet prefix; `h2_resets_total` counts received RST_STREAM frames by their
RFC 7540 §7 names (the daemon sends none). The older `zerodds_grpc_*` from
`crates/grpc-bridge/src/daemon_runtime.rs` stay alongside.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::grpcbridge_8_2_the_metrics_of_section_8_2_count_calls_failures_and_resets`
(status per call, auth failures by reason, RST_STREAM(CANCEL) and the two
reset calls as `status="CANCELLED"`, with `dds-runtime` Publish → DDS →
Subscribe);
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_8_2_admin_endpoint_serves_metrics_path`;
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_3_a_value_outside_its_range_is_refused`
and `crates/grpc-bridge/tests/daemon_cli_e2e.rs::grpcbridge_2_check_config_refuses_what_the_start_refuses`
(`metrics.listen`).

**Status:** done — `metrics.listen` and `--metrics` are an IP address and port (IPv6 in brackets); a host name such as `localhost:9095` stops the start with exit 1 and the value and is refused by `--check-config`, from the file and from `--metrics` (checked against the binary on the Linux test host). Before, the daemon then started without an admin endpoint and without a log line.

### §8.3 OTLP spans + W3C traceparent

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` + trace propagation via a
traceparent header.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/trace.rs` (`Tracer`,
`spans`, `span_status`: one server span `<service>/<method>` per call with
`rpc.system`, `rpc.service`, `rpc.method`, `rpc.grpc.status_code` (int),
`dds.topic`, `network.peer.address`, `network.peer.port` (int); status
`Error` with the `grpc-message` only for UNKNOWN, DEADLINE_EXCEEDED,
UNIMPLEMENTED, INTERNAL, UNAVAILABLE, DATA_LOSS, as the OpenTelemetry
conventions set it for a gRPC server; the child `dds.publish` over the DDS
writes of a `Publish`/`PublishOne`, `dds.subscribe` from the opening of the
DataReader to the end of a `Subscribe` stream, each with `dds.samples`
(int); `spawn_export_loop`: an export every 5 s on a thread of its own,
which ends after the stop without a further export; `finish`: the last
export; `finish_by`: the end of the export thread and the last export
together at most until `EXPORT_STOP_BUDGET` (5 s), after it an
`otlp_export_failed` line; `ExportFailures`: a failed export is an `otlp_export_failed` line at
`warn` with `err` and `failures`, at most one per minute, the last export
always), `crates/observability-otlp/src/lib.rs` (`IntAttribute`,
`OtlpExporter::add_span_with_ints`: attributes with `intValue`),
`crates/grpc-bridge/src/bridge_daemon/calls.rs::Conn::ended` (the spans of
every call that ended), `crates/grpc-bridge/src/bridge_daemon/mod.rs::run`
(the exporter from `OTEL_EXPORTER_OTLP_ENDPOINT`; a value the exporter
cannot serve stops the start with exit 1, under `--check-config` too; after
the drain `finish_by`),
`crates/grpc-bridge/src/daemon_runtime.rs::otlp_config_from_env`,
`::otlp_config_from_endpoint` (only `http://host[:port]` with an optional
`/`, port 80 without one; `https`, user information, a path, a query, a
fragment and a port outside 1 to 65535 are refused with the value; empty
counts as unset; an IPv6 address keeps its brackets),
`crates/observability-otlp/src/lib.rs::OtlpExporter::connect` (resolves
the host without the brackets of an IPv6 address; the `Host` field writes
it in brackets). §8.3a carries the traceparent half.

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
(job `integration-bridges-vendor-grpc`);
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_8_3_otlp_config_parses_endpoint`,
`::grpcbridge_8_3_an_otlp_endpoint_the_exporter_cannot_serve_is_refused`;
`crates/observability-otlp/src/lib.rs::tests::traces_json_writes_int_attributes_as_int_value`,
`::flush_connects_within_the_timeout` (Linux),
`::flush_reaches_a_collector_at_an_ipv6_address`.

**Status:** done — against the daemon with an endpoint of the test: after
SIGTERM, three Publish calls give two server spans
`zerodds.t.v1.OpenStream/Publish` and two `dds.publish` at `/v1/traces` (the
third call carries an unsampled `traceparent`), with
`rpc.grpc.status_code`, `network.peer.port` and `dds.samples` as
`intValue`. `https://127.0.0.1:4318`, `http://127.0.0.1:4318/otlp` and
`http://127.0.0.1:43x8` stop the start with exit 1 and the value on stderr;
`--check-config` with `https` reports `configuration invalid`. Without the
variable a listener on 127.0.0.1:4318 receives no connection after a call
and SIGTERM, and no `otlp` line appears (where another process holds the
port the test checks only the lines and says so on stderr; in the CI jobs
and on the test host each run has a network namespace of its own). The
connect to the collector waits at most the exporter's timeout (5 s; unit
test in zerodds-observability-otlp against a listener with a full accept
queue: `TimedOut` after 300 ms); the end of the shutdown waits at most 5 s
for the export in flight and the last export together (unit test against a
collector that takes the connection and never answers: both cases end at
the deadline of 300 ms; a collector that answers gets the POST to
`/v1/traces`). An endpoint nothing listens on gives an `otlp_export_failed`
line at `warn` with `err` `io: …` after SIGTERM; the daemon exits 0.
With `OTEL_EXPORTER_OTLP_ENDPOINT=http://[::1]:<port>` the span of a call
reaches an endpoint of the test on the IPv6 loopback after SIGTERM, with
no `otlp_export_failed` line (before the fix of `OtlpExporter::connect`
every export failed to resolve `[::1]`; in the unit test `[::1]` and `::1`
reach the listener, the `Host` field is `[::1]`). Against the
OpenTelemetry Collector 0.115.0 (`otelcol`, exporter `debug`):
a `PublishOne` grpcurl 1.9.3 sends with a `traceparent` appears decoded
with the header's trace id and parent id, kind `Server`, `rpc.system:
Str(grpc)`, `rpc.method: Str(PublishOne)`, `rpc.grpc.status_code: Int(0)`,
`network.peer.port: Int(…)`, `dds.topic: Str(Trade::Trade)` and the child
`dds.publish` with `dds.samples: Int(1)`.

### §8.3a W3C traceparent parsing

**Spec:** §8.3 — an incoming `traceparent` header is parsed and the span is
attached to it.

**Repo:** `crates/monitor/src/trace_context.rs` (`impl FromStr for
TraceParent`: W3C Trace Context §3.2; version `00` exactly
`00-<32 hex>-<16 hex>-<2 hex>` in lower-case hex, a higher version by the
fields of `00` followed by the end or a dash and with only the sampled flag
(§3.2.4, §4.3), version `ff` invalid, all-zero ids invalid),
`crates/grpc-bridge/src/bridge_daemon/trace.rs::traceparent_of` (the header
in any case; none, two or an invalid one: a new trace with a random trace
id), `::sampled` (no sampled flag, no span), `::spans` (the server span is a
child of the caller's span). `tracestate` is not read: the daemon makes no
HTTP request onwards, and its DDS writes carry no trace context.

**Tests:** `crates/grpc-bridge/src/bridge_daemon/trace.rs::tests::grpcbridge_8_3_the_span_of_a_call_is_a_child_of_its_traceparent`,
`::grpcbridge_8_3_a_malformed_traceparent_starts_a_new_trace`,
`::grpcbridge_8_3_an_unsampled_traceparent_records_no_span`;
`crates/monitor/src/trace_context.rs::tests::w3c_trace_context_3_2_2_a_traceparent_header_is_read`,
`::w3c_trace_context_3_2_2_a_malformed_traceparent_is_refused`,
`::w3c_trace_context_3_2_4_a_higher_version_is_read_by_the_00_fields`;
`crates/grpc-bridge/tests/observability_e2e.rs::grpcbridge_8_3_the_daemon_posts_the_spans_of_a_call_to_the_otlp_endpoint`;
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_8_3_the_opentelemetry_collector_decodes_the_spans_of_a_call`
(job `integration-bridges-vendor-grpc`).

**Status:** done — against the daemon: the call with the `traceparent` of
the W3C Trace Context §3.2.3 example reaches the endpoint with that trace id
in the server span and in `dds.publish`, and the server span's
`parentSpanId` is the header's parent id; the call without a header has a
trace id of its own; no span of the unsampled call arrives. The
OpenTelemetry Collector shows the header's trace id and parent id.

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → reader/writer + FileDescriptor →
HTTP/2 bind → signal handler.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/`,
`crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::grpcbridge_2_http2_roundtrip_publish_topic`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, GOAWAY, RPCs end naturally; SIGHUP
TLS+ACL reload.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/connection.rs::handle_connection`
(every connection checks the stop flag every 200 ms; at the stop it sends
`GOAWAY(NO_ERROR)` with the last stream the client opened at once and
stays open, the calls up to that id run on, the connection ends when its
last call ended and no answer waits for the client's window any more
(`GrpcServer::active_streams` is 0, the body of a GET `/idl/` too); after
`grpc.drain_secs` the open calls end with UNAVAILABLE, what still waits
for the window is not sent, event `drain_limit` with `open_calls` and
`open_streams`), `::serve` (the accept loop ends and the listening
socket closes at the signal),
`crates/grpc-bridge/src/server.rs::GrpcServer::begin_drain` (a stream after
the GOAWAY gets `RST_STREAM(REFUSED_STREAM)`, RFC 9113 §6.8),
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs::BridgeDds::await_writes`
(waits, within what is left of the drain time, until the reliable DDS
readers acknowledged everything, event `dds_writes_drained`), `::close`
(deletes the DataWriters and DataReaders, stops the participant),
`crates/grpc-bridge/src/bridge_daemon/mod.rs::run` (order: drain,
acknowledgements, entities, the last OTLP export within 5 s
(`Tracer::finish_by`, §8.3), exit 0),
`crates/grpc-bridge/src/bridge_daemon/config.rs` (`grpc.drain_secs`, 0 to
300, default 30, needs a restart), `crates/grpc-bridge/src/bridge_daemon/auth.rs::apply_reload`
(SIGHUP: TLS material, auth, ACL, the auth rules checked against the
values that wait for a restart; a file that does not load, or cannot be
served so, changes nothing). systemd `TimeoutStopSec=40`, launchd `ExitTimeOut` 40, the
Dockerfile names `docker stop -t 40`.

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

**Status:** done — a Subscribe stream open at SIGTERM still delivers, after
the GOAWAY, the DDS sample written after it; the daemon exits 0 as soon as
the client ends the stream. A client with `SETTINGS_INITIAL_WINDOW_SIZE`
16 gets 16 octets each of a `Catalog` answer (the call has ended, its
trailers wait behind the message) and of the body of a GET `/idl/`, then
the GOAWAY at SIGTERM; after its WINDOW_UPDATE 600 ms later both arrive
whole, the `Catalog` with `grpc-status` 0, then the connection closes and
the daemon exits 0 without `drain_limit` (before the fix the connection
closed with the GOAWAY and the client got a truncated message without
trailers). With `drain_secs: 1` an open stream ends after about 1 s with
`grpc-status` 14. Against the daemon: a sample a Publish
writes right before SIGTERM reaches the reliable DDS reader of the test,
the daemon reports `dds_writes_drained` with `acknowledged:true` and exits
within 10 s (`drain_secs` is 30 s; the test does not fix whether the
sample arrives before or after the signal). With the acknowledgement
outstanding at SIGTERM (the test's reader is matched, its participant
stops without notice), the daemon holds the exit to the limit of
`drain_secs: 2`, reports `acknowledged:false` and exits 0. That
`await_writes` waits for the acknowledgement is shown by the unit test with
a DDS writer and reader.

## §10 Cross-vendor

### §10 RTPS peer + grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic

**Spec:** §10 — the daemon is a normal RTPS peer; the gRPC side against
grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic.

**Repo:** `crates/grpc-bridge/src/bridge_daemon/dds_pump.rs` (the daemon's
participants in SPDP/SEDP; the DataWriter and the stream DataReaders as
WITH_KEY or, with `keyed: false`, NO_KEY endpoints, DDSI-RTPS 2.5
§9.3.1.2), `crates/grpc-bridge/src/bridge_daemon/config.rs` (the topic key
`keyed`, default `true`), `crates/grpc-bridge/src/bridge_daemon/dispatch.rs::sample_to_publish`
(a `key_hash` on a topic without a key: INVALID_ARGUMENT),
`crates/grpc-bridge/tests/interop/cyclone_chat.c` and
`crates/grpc-bridge/tests/interop/chat.idl` (the test's Cyclone DDS
participant); the gRPC clients see §10a.

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_a_grpcurl_publish_reaches_cyclone_and_a_cyclone_sample_a_grpcurl_subscribe`
(job `integration-bridges-vendor-grpc`);
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs::tests::grpcbridge_10_the_endpoints_of_a_keyless_topic_are_no_key_endpoints`;
`crates/grpc-bridge/src/bridge_daemon/config.rs::tests::grpcbridge_10_keyed_is_read_and_defaults_to_true`;
`crates/grpc-bridge/src/bridge_daemon/dispatch.rs::tests::grpcbridge_10_a_key_hash_on_a_keyless_topic_is_refused`;
`crates/grpc-bridge/tests/bridge_e2e.rs::grpcbridge_12_2_grpcurl_publish_reaches_the_dds_reader_byte_exact`,
`::grpcbridge_12_2_a_dds_sample_reaches_grpcurl_subscribe_byte_exact` (job
`integration-bridges-vendor-grpc`);
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_grpcurl_list_services_against_zerodds_daemon`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_grpcurl_lists_the_reflection_service_itself` (job `integration-bridges-vendor-grpc`),
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_4_6_list_services_reflection_payload_is_well_formed`;
`tests/integration/tests/bridges.rs::grpc_bridge_against_grpcio` (gRPC
C-core through Python grpcio, job `integration-bridges`);
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_grpc_go_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_grpc_java_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_tonic_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_ghz_2000_publish_one_calls_all_ok_and_all_reach_dds`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_bloomrpc_calls_publish_one_and_catalog`
(job `integration-bridges-vendor-grpc`, see §10a).

**Status:** done — the RTPS side is measured against Cyclone DDS 0.10.5:
a `PublishOne` of grpcurl 1.9.3 reaches Cyclone's DataReader as exactly the
sample sent, a sample of Cyclone's DataWriter reaches a grpcurl `Subscribe`
stream with Cyclone's bytes, `typeName` and `topicName`; this holds for the
type with a key and for the one without (`keyed: false`). Without
`keyed: false`, Cyclone saw the daemon's WITH_KEY endpoints for a type
without a key but did not connect them (measured with Cyclone's tracing).
On the gRPC side grpcurl (grpc-go), grpcio (C-core), grpc-go, grpc-java,
tonic, ghz and BloomRPC are measured on the Linux test host (§10a).

### §10a Client matrix grpc-go/grpc-java/ghz/BloomRPC/tonic

**Spec:** §10 — the daemon is driven by grpc-go, grpc-java, ghz, BloomRPC and
tonic.

**Repo:** `crates/grpc-bridge/proto/zerodds/chat/v1/bridge.proto` (the
definition of §4.2 for `Chat::Message` as `service_gen::render_proto`
writes it; the clients generate their stubs from it),
`crates/grpc-bridge/tests/interop/build_peers.sh` (builds the three
clients: grpc-go from Debian's `golang-google-grpc-dev` 1.64 with
`protoc-gen-go` and `protoc-gen-go-grpc` in GOPATH mode without a download,
grpc-java from Debian's `libgrpc-java` 1.41 with Netty and protoc's
grpc-java plugin, tonic 0.14 with prost as a cargo project of its own with
its own lock file), `crates/grpc-bridge/tests/interop/go/main.go`,
`crates/grpc-bridge/tests/interop/java/BridgeClient.java`,
`crates/grpc-bridge/tests/interop/tonic/src/main.rs` (the clients:
`publish`, `publish-one`, `subscribe`, `catalog`, one line per result),
`crates/grpc-bridge/tests/interop/bloomrpc_drive.py` (starts BloomRPC 1.5.3
under Xvfb with the `.proto` imported and the tabs `PublishOne` and
`Catalog` open, presses the play button over the DevTools protocol, reads
the response pane), `tests/integration/provision.sh` (`protoc`, `grpc-go`,
`grpc-java`, `ghz` 0.121.0 and BloomRPC 1.5.3 with their sha256),
`ci/integration_scenarios.py` (job `integration-bridges-vendor-grpc` with
the build step).

**Tests:** `crates/grpc-bridge/src/service_gen.rs::tests::grpcbridge_4_2_the_shipped_proto_is_what_render_proto_writes`;
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_grpc_go_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_grpc_java_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_tonic_publishes_subscribes_and_reads_the_catalog`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_ghz_2000_publish_one_calls_all_ok_and_all_reach_dds`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_10_bloomrpc_calls_publish_one_and_catalog`
(job `integration-bridges-vendor-grpc`).

**Status:** done — measured on the Linux test host: grpc-go, grpc-java and
tonic each send a client-streaming `Publish` of two samples and a
`PublishOne`, which reach the test's DDS DataReader octet for octet (the
encapsulation header as the representation identifier), `PublishAck`
sequence numbers 2 and 3; a `PublishOne` without an encapsulation header
ends with status 3 and the daemon's `grpc-message` in all three; `Catalog`
names the topic, the type, the full service name and the `idl_url` of the
client's authority; a `Subscribe` stream gets the two samples of a DDS
DataWriter of the test with the header `CDR2_BE`. ghz 0.121.0: 2000
`PublishOne`, 20 at a time over two connections, all OK, all 2000 samples
at the DDS DataReader. BloomRPC 1.5.3 (Electron, gRPC C-core through the
Node package `grpc` 1.24): the `PublishOne` reaches the DDS DataReader, the
response pane shows the `PublishAck` with sequence number 1 and the
`CatalogResp` of the topic.

---

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-grpc-bridged`; configs/services/Docker;
manuals.

**Repo:** `packaging/linux/systemd/zerodds-grpc-bridged.service`
(`Type=notify`, `TimeoutStopSec=40` above the drain of §9.2),
`packaging/macos/launchd/org.zerodds.grpc-bridged.plist` (`ExitTimeOut`
40), `packaging/macos/homebrew/zerodds-grpc-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/windows/msi/zerodds.wxs` (service `ZeroDDSGrpcBridge`),
`packaging/docker/grpc-bridged/` (`STOPSIGNAL SIGTERM`, names
`docker stop -t 40`), `packaging/linux/configs/grpc-bridged.yaml.example`,
`man/man1/zerodds-grpc-bridged.1` (WINDOWS SERVICE),
`man/man5/zerodds-grpc-bridged.yaml.5`;
the Windows service: `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`
(`main` starts the daemon through
`zerodds_dcps::cli::run_as_service(WINDOWS_SERVICE, …)`),
`crates/grpc-bridge/src/bridge_daemon/mod.rs` (`WINDOWS_SERVICE` =
`ZeroDDSGrpcBridge`), `crates/grpc-bridge/src/daemon_runtime.rs`
(`install_signal_watcher` off Unix: `watch_service_requests` sets the
shutdown flag of SIGTERM on a stop request, which starts the drain of §9.2,
and the reload flag of SIGHUP on `paramchange`), `crates/dcps/src/cli.rs`
(`run_as_service`, `request_service_stop`, `request_service_reload`,
`READY=1` and `STOPPING=1` as service states) and
`crates/dcps/src/cli/windows_service.rs` (`StartServiceCtrlDispatcherW`,
`RegisterServiceCtrlHandlerExW`, `SetServiceStatus`, the console handler),
unchanged from the service dispatcher of the WebSocket daemon. Before, the
binary had no service control dispatcher, and the watcher off Unix was an
empty thread. Spec §11 names the service name of the installers (before:
`ZeroDDSGRPCBridge`).

**Tests:** `crates/grpc-bridge/src/bridge_daemon/config.rs::tests::{grpcbridge_3_example_yaml_parses_without_error, grpcbridge_3_example_yaml_key_fields_match_intent, grpcbridge_3_example_yaml_topics_reach_the_daemon, grpcbridge_3_example_yaml_has_no_unknown_top_level_keys}`,
`crates/config/tests/layering.rs::a_hash_inside_quotes_is_part_of_the_value`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys);
`crates/grpc-bridge/src/bridge_daemon/mod.rs::packaging_tests::{grpcbridge_11_the_windows_service_is_the_one_the_installers_register, grpcbridge_11_the_service_managers_wait_longer_than_the_drain, grpcbridge_11_the_service_files_start_the_binary_with_its_configuration}`
(the service name in the binary, the PowerShell installer, the MSI and the
spec; systemd, launchd and Docker stop with SIGTERM and wait longer than the
drain; each service file starts the binary with the configuration of its
platform);
`tools/traceability/tests/deployment_layout.rs::{every_systemd_unit_carries_the_hardening_set_of_the_clause, every_launchd_plist_carries_the_keys_of_the_clause, windows_service_scripts_register_and_remove_the_same_bridge_set, every_daemon_image_is_cargo_chef_multi_stage_and_runs_non_root, msi_defines_one_optional_feature_per_bridge}`;
`crates/grpc-bridge/src/daemon_runtime.rs::tests::grpcbridge_11_the_service_requests_set_the_shutdown_and_reload_flags`
(the Windows watcher in a thread under Linux: without a request it waits,
it ends with the daemon, a reload request sets the reload flag, a stop
request the shutdown flag);
`crates/dcps/src/cli.rs::service_tests::{deployment_4_3_the_service_controls_are_stop_reload_and_interrogate, deployment_4_3_the_notify_states_are_service_states, deployment_4_3_the_exit_code_has_its_number, deployment_4_3_the_requests_and_the_run}`;
`crates/dcps/src/cli/windows_service.rs::tests::deployment_4_3_the_control_codes_are_the_win32_constants`
(Windows only).

**Status:** partial — the tests ran green on the Linux test host; the
Windows code was compiled there with `cargo clippy --target
x86_64-pc-windows-gnu --all-targets`, not run. The weekly job
`platform-windows-grpc-bridge-build` (`ci/platform_jobs.py`
`WINDOWS_BUILD`, `ci/platforms.yml`) builds every target of
`zerodds-grpc-bridge` on the Windows runner, the service watcher and the
`run_as_service` call among them; it has not run yet. Missing: the run of the service
`ZeroDDSGrpcBridge` under `sc.exe` (start, `READY=1` as `SERVICE_RUNNING`,
stop with the drain, `paramchange` as reload); it needs a Windows host with
the registered service.

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/http2_codec/hpack/grpc_status/reflection/dds_pump/auth,
≥ 5 tests each.

**Repo:** the modules of the spec and their files: `config` =
`crates/grpc-bridge/src/bridge_daemon/config.rs`; `http2_codec` =
`crates/http2/src/` and `crates/grpc-bridge/src/{frame.rs,server.rs}`;
`hpack` = `crates/hpack/src/`; `grpc_status` =
`crates/grpc-bridge/src/status.rs`; `reflection` =
`crates/grpc-bridge/src/{reflection.rs,descriptor.rs}`; `dds_pump` =
`crates/grpc-bridge/src/bridge_daemon/dds_pump.rs`; `auth` =
`crates/grpc-bridge/src/bridge_daemon/auth.rs` and
`crates/grpc-bridge/src/bridge_security.rs`. Since the split of the binary
the daemon lives in modules under `crates/grpc-bridge/src/bridge_daemon/`
(`config`, `connection`, `calls`, `dispatch`, `dds_pump`, `auth`,
`catalog`, `metrics`); the binary keeps `main`.

**Tests:** inline `#[cfg(test)]` modules, per module of the spec with its number of tests:
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
plus `crates/grpc-bridge/src/metadata.rs::tests::grpc_metadata_base64_reads_the_vectors_of_rfc_4648` (18),
`crates/grpc-bridge/src/path.rs::tests::grpc_requests_parses_standard_grpc_path` (7),
`crates/grpc-bridge/src/timeout.rs::tests::grpc_requests_the_timeout_units_are_h_m_s_m_u_n` (10)
and `crates/grpc-bridge/tests/fuzz_smoke.rs` (`grpcbridge_12_1_fuzz_decode_message_no_panic`,
`grpcbridge_12_1_fuzz_parse_path_no_panic`, `grpcbridge_12_1_fuzz_decode_timeout_no_panic`,
`grpcbridge_12_1_empty_inputs_no_panic`, `grpcbridge_12_1_message_oversized_length_no_oom`).

**Status:** done — each of the seven modules has at least five tests; the
`dds_pump` tests run with a second participant on a leased domain. Counted
by `#[test]` per file, green on the Linux test host.

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, grpcurl as the client, byte-exact
round-trip + reflection.

**Repo:** `crates/grpc-bridge/tests/bridge_e2e.rs` (the daemon as a
subprocess on a leased domain, `grpcurl` as the client with the descriptors
of the reflection service alone, a DDS DataReader or DataWriter in the test
process), `ci/integration_scenarios.py` (job `integration-bridges-vendor-grpc`,
step `bridge_e2e` with `--run-ignored`).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::grpcbridge_12_2_grpcurl_publish_reaches_the_dds_reader_byte_exact`
(a client-streaming `Publish` of two Samples of 256 octets with every octet
value: the DDS reader receives representation `CDR2_LE` and the CDR bytes
unchanged, the PublishAck names sequence number 2),
`::grpcbridge_12_2_a_dds_sample_reaches_grpcurl_subscribe_byte_exact`
(a sample of a DataWriter of the test with XCDR2 big endian reaches
grpcurl's `Subscribe` as `Sample.cdr_payload` with the `CDR2_BE` header and the
same bytes, with topic and type name),
`::grpcbridge_12_2_grpcurl_describe_round_trips_the_service_through_reflection`
(grpcurl saves the descriptors of the reflection service with
`-protoset-out`; a call from that file alone reaches a daemon without
reflection, whose `list` grpcurl cannot read, and the DDS reader receives
the bytes),
`::grpcbridge_2_http2_roundtrip_publish_topic`,
`::grpcbridge_2_http2_unknown_service_yields_status_5` (own HTTP/2 client,
crate tier);
`tests/integration/tests/bridges.rs::grpc_bridge_against_grpcio` (grpcio
with a DDS peer, job `integration-bridges`).

**Status:** done — the three grpcurl tests ran green with grpcurl 1.9.3 on
the Linux test host (`cargo nextest run -p zerodds-grpc-bridge --test
bridge_e2e --run-ignored all`: 5 of 5).

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a tonic client + a Cyclone-DDS subscriber + the ZeroDDS
gRPC bridge in compose.

**Repo:** `crates/grpc-bridge/tests/compose/docker-compose.yml` (the
services `bridge` from `packaging/docker/grpc-bridged/Dockerfile` with
`tests/compose/grpc-bridged.yaml`, `cyclone` from
`tests/compose/cyclone.Dockerfile`, `client` from
`tests/compose/tonic.Dockerfile` in the bridge's network namespace; the
client's exit code is the result),
`crates/grpc-bridge/tests/interop/cyclone_chat.c` (mode `echo`: answers
each `Chat::Message` with `{ seq, "echo: " text }`),
`crates/grpc-bridge/tests/interop/tonic/src/main.rs` (mode `echo`: a
`Subscribe` stream, then `PublishOne` once a second until Cyclone's answer
comes), `crates/grpc-bridge/tests/compose_e2e.rs`, `ci/manual-tests.tsv`.

**Tests:** `crates/grpc-bridge/tests/compose_e2e.rs::grpcbridge_12_3_the_compose_file_runs_bridge_cyclone_and_tonic_and_the_answer_comes_back`
(manual test on the Linux test host by decision 11, `ci/manual-tests.tsv`:
the GitLab runners have no Docker daemon);
`crates/grpc-bridge/tests/cross_vendor.rs::grpcbridge_12_3_a_tonic_publish_reaches_cyclone_and_a_cyclone_sample_a_tonic_subscribe`
(the same three as processes, job `integration-bridges-vendor-grpc`).

**Status:** done — measured on the Linux test host: `docker compose up`
builds the three images and starts them; the tonic client's `PublishOne`
with `{ 7, "hello from compose" }` reaches the DataReader of Cyclone DDS
0.10.5, whose answer `{ 7, "echo: hello from compose" }` comes through the
bridge's DataReader onto the client's `Subscribe` stream, the client exits
0. As processes: a `Publish` of tonic reaches Cyclone's DataReader, a
sample of Cyclone's DataWriter a `Subscribe` stream of tonic.

## §13 Cross-references

### §13 Related library + standards + daemons

**Spec:** §13 — library `crates/grpc-bridge/`/`crates/http2/`/`crates/hpack/`,
gRPC-PROTOCOL.md, RFC 7540/7541, wire format, deployment, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config/.proto, major=wire-protocol
change (HTTP/3 + gRPC-over-QUIC).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — rule for assigning the version number of a release, not a property of the daemon.

---

## Audit status

26 done / 1 partial / 0 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-grpc-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (job `merge-rust-bridge-security`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-config --all-targets` (job `merge-rust-config`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-grpc-bridge --all-targets` (job `merge-rust-grpc-bridge`) — 59 tests green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 4 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor` (job `integration-bridges-vendor`) — 3 tests green.

68 cited tests green, 0 not green.

Open items: `zerodds-grpc-bridge-1.0.open.en.md`.
