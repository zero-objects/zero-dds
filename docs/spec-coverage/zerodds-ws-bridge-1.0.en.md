# `zerodds-ws-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-ws-bridge-1.0.md`

Implementation:

- `crates/websocket-bridge/` — DDS↔WebSocket bridge.

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1, `zerodds-ws-bridge-1.0.md` — six conformance levels
(Wire/DDS/Bridging/Config/Auth/Multi-Tenant); L1–L4 mandatory, L5–L6
optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — assignment of the levels; the requirements are the items §1a (L6), §1b (L1, RFC 7692) and §2 to §12.

### §1a L6 Multi-tenant

**Spec:** §1 L6 — several DomainParticipants per daemon; one tenant binding
per WebSocket connection; §3 `tenants`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs::{TenantConfig, TenantConfig::binds, tenant_acl_topic, read_topics}`,
`crates/websocket-bridge/src/daemon/server.rs::{Core, Cores::of, build_core, serve_connection, settings_that_need_a_restart}`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::BridgeMetrics::for_tenant`.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::wsbridge_1_l6_a_connection_is_bound_to_the_tenant_of_its_subject`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_1_l6_the_tenants`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_3_the_example_of_the_spec_loads`,
`crates/websocket-bridge/src/daemon/server.rs::tests::wsbridge_9_2_a_reload_names_the_changed_settings_that_need_a_restart`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_1_l6_the_log_line_and_the_span_name_the_tenant_of_a_connection`
(the binary with the bearer subject `alice` in a tenant: `connection_open`
with `tenant: plant-a`, `ws.connection` with `ws.tenant`);
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_1_the_binary_logs_json_lines_for_a_frame_exchange`
and `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_3_the_spans_of_a_connection_and_its_frames_reach_a_collector`
check `tenant: ""` and no `ws.tenant` without a tenant.

**Status:** done — the tests ran green on the Linux test host. A tenant is a
DomainParticipant on its domain with its own topics, endpoints, router and
catalog. A connection whose JWT subject a tenant names writes into its
domain and gets only its samples (checked with one DDS participant of the
test on each domain and the topic `Chat` on both); a connection without a
tenant uses `domain`. The ACL of a tenant's topic is keyed
`<tenant>/<topic>`, and the metrics count under the same label. A top-level
topic named `<tenant>/<topic>` of a tenant's topic prevents the start, and
so does a second entry of a topic with an `acl` of its own (before, the
later ACL replaced the earlier one without an error). Checked with JWT and
bearer; mTLS DN and `mtls:` fingerprint bind through the same subject
(`TenantConfig::binds`, unit test) but are not checked end to end.

### §1b L1 permessage-deflate (RFC 7692)

**Spec:** §1 L1 — the daemon speaks RFC 6455 and RFC 7692 (permessage-deflate
optional); §4.1, §4.5: the daemon agrees to the first valid
`permessage-deflate` offer, `limits.max_message_size` holds after
decompression.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs::serve_connection`
(`permessage_deflate::accept_offers`, `render_accept`, `Receiver`/`Sender`
with `with_permessage_deflate`), `crates/websocket-bridge/src/permessage_deflate.rs`
(negotiation, `Deflater`/`Inflater` over `flate2` with `zlib-rs`),
`crates/websocket-bridge/src/connection.rs` (RSV1, context takeover, bound
of the decompressed size). The details of RFC 7692 are in
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

**Status:** done — the tests ran green on the Linux test host, the Python
test with `websockets` 17.4 (weekly job `integration-bridges-vendor`). The
daemon always negotiates the extension when the client offers it; the spec
has no key to turn it off.

## §2 CLI surface

### §2 zerodds-ws-bridged CLI

**Spec:** §2 — options `--config`/`--set`/`--check-config`/`--listen`
(default `127.0.0.1:8080`)/`--domain`/`--topic <NAME[:WS_PATH]>`/
`--tls-cert`/`--tls-key`/`--auth-token`/`--log-level`/`--metrics`/
`--version`/`--help`; exit codes 0/1/2/3/4; the short forms are `--set` of
their key and win over the files.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/websocket-bridge/src/daemon/cli.rs` (`HELP_TEXT`, `VERSION_TEXT`, `CliArgs::sets`),
`crates/websocket-bridge/src/daemon/config.rs` (`read_topic`: `NAME:WS_PATH` through
`zerodds_config::read::split_topic_shorthand`).

**Tests:** Through the binary:
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_a_config_error_is_exit_1`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_a_port_in_use_is_exit_2`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_a_participant_that_does_not_start_is_exit_3`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_a_certificate_that_does_not_load_is_exit_4`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_check_config_prints_the_values_and_exits`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_help_and_version`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_the_flags_override_the_file_and_the_topic_flag_names_its_path`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_the_binary_exits_0_on_sigterm`.
In the code:
`crates/websocket-bridge/src/daemon/cli.rs::tests::wsbridge_2_the_help_names_the_options_the_exit_codes_and_the_default`
(the help against the block of §2 of the specification),
`crates/websocket-bridge/src/daemon/cli.rs::tests::wsbridge_2_every_short_form_is_a_set_of_its_key`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_2_a_bare_topic_entry_names_its_path_behind_a_colon`.

**Status:** done — the tests ran green on the Linux test host.

## §3 Config-file format

### §3 YAML/JSON/TOML config loader

**Spec:** §3 — top-level `listen`/`domain`/`log_level`/`tls`/`auth`/
`topics`/`metrics`/`limits`/`origin`; ENV substitution `${VAR}` and
`${VAR:-default}`. `metrics.path` (default `/metrics`),
`qos.history.kind` (`keep_last`|`keep_all`), `ws_path` as an absolute path,
not the one of a meta endpoint, one of its own per topic.
`limits.max_frame_size` (at least 125) and
`limits.max_message_size` (at least 1) in octets, 16 MiB each by default;
`origin.allowed` as a list of origins `scheme://host[:port]`, `null` or `*`,
`origin.required` only with a list; a value outside of that prevents the
start, an entry with `*` inside a host (`https://*.example.org`) among them.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs`,
`crates/websocket-bridge/src/daemon/mod.rs` (DaemonConfig).

**Tests:** `crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_3_the_example_of_the_spec_loads`
(reads the YAML block out of §3 of the specification, checks its values and
that it names every key of `KEYS`),
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_3_the_history_kind_and_the_metrics_path_are_read`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_4_2_a_ws_path_that_cannot_be_served_is_refused`;
the packaged example
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_3_example_yaml_parses_without_error`,
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_3_example_yaml_key_fields_match_intent`;
man page against example `tools/traceability/tests/bridge_config_manpages.rs`. The keys
`limits.*` and `origin.*`:
`crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_4_the_limits_are_read_and_default_to_16_mib`,
`crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_4_a_limit_that_cannot_be_served_is_refused_with_its_line`,
`crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_2_the_origin_keys_are_read`,
`crates/websocket-bridge/src/daemon/config.rs::tests::ws_10_2_an_origin_setting_that_cannot_be_served_is_refused_with_its_line`,
`crates/websocket-bridge/src/origin.rs::tests::ws_10_2_a_list_entry_with_a_pattern_in_its_host_is_refused`. Through `--set`:
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs::tests::ws_10_the_limits_and_the_origins_are_set_with_set`.

**Status:** done

## §4 WebSocket wire protocol

### §4.1 RFC 6455 handshake + zerodds-ws-bridge-1.0 subprotocol

**Spec:** §4.1 — RFC-6455 upgrade; subprotocol header
`Sec-WebSocket-Protocol: zerodds-ws-bridge-1.0`, each name a token
(RFC 6455 §4.1, the `token` of RFC 7230 §3.2.6); auth header per bearer
and JWT mode; 401 with `WWW-Authenticate: Bearer` when the credentials
authenticate no one, 403 with `insufficient_scope` when a verified JWT lacks
the claim of `auth.jwt.required_claim` (RFC 6750 §3, §3.1). An invalid handshake gets its HTTP status
(405 with `Allow: GET`, 426 with `Sec-WebSocket-Version: 13`, otherwise 400,
also for a request head of more than 65536 octets). A request head that is
not complete 10 s after the TCP connection gets 408.
With a list `origin.allowed` an `Origin` outside the list gets 403, before
the authentication; a handshake without `Origin` is accepted, and refused
with `origin.required`. `permessage-deflate` (RFC 7692): the daemon agrees to
the first valid offer and compresses the messages of the connection.

**Repo:** `crates/websocket-bridge/src/handshake.rs`,
`crates/websocket-bridge/src/negotiation.rs`,
`crates/websocket-bridge/src/permessage_deflate.rs`,
`crates/websocket-bridge/src/daemon/server.rs` (upgrade path),
`crates/websocket-bridge/src/daemon/security.rs` (`authenticate_ws`,
`render_refusal`), `crates/bridge-security/src/auth.rs`
(`AuthMode::validate_scoped`, `Refusal`, `MissingClaim`).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_handshake_roundtrip_succeeds`,
`::wsbridge_2_rejects_non_upgrade_request`,
`crates/websocket-bridge/src/daemon/server.rs::tests::rfc_6455_4_1_the_subprotocols_are_tokens`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::wsbridge_4_1_a_raw_tcp_client_gets_101_and_the_accept_hash`;
the Python library `websockets` (its own check of subprotocol names) in the
scenario `tests/integration/tests/bridges.rs::websocket_bridge_json_text_frames_against_python_websockets`.
Until this change the subprotocols were named `zerodds-ws-bridge/1.0` and
`…/1.0+json`; `websockets` refused that as "invalid subprotocol" (`/` is a
separator); the `WebSocket` constructor of the WHATWG standard throws for
such a name as well (read, not measured). HTTP status of the invalid
handshake:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_1_the_daemon_answers_an_invalid_handshake_with_its_http_status`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_4_2_2_the_daemon_answers_an_unsupported_version_with_426_and_names_13`.
Length and time of the request head (`MAX_HANDSHAKE_SIZE`,
`HANDSHAKE_TIMEOUT`):
`crates/websocket-bridge/tests/daemon_rfc6455.rs::wsbridge_4_1_a_request_head_over_64_kib_is_answered_with_400`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::wsbridge_4_1_a_handshake_that_does_not_arrive_in_time_gets_408`.
Before, the daemon closed a head over 64 KiB without a response, a log line,
the error counter or a span, and a client that sent nothing after the
connect held its thread until the shutdown.
`permessage-deflate`:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_9_the_daemon_agrees_to_permessage_deflate_and_compresses_both_directions`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_9_the_daemon_declines_an_offer_that_is_invalid`; with Python
`websockets` as the client
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::python_websockets_client_talks_compressed_to_the_daemon_and_closes`.
The check of the `Origin`:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_answers_an_origin_it_does_not_expect_with_403`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_accepts_an_origin_it_expects_and_a_client_without_one`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_refuses_a_handshake_without_origin_when_one_is_required`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_without_a_list_the_daemon_accepts_every_origin`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_2_the_daemon_checks_the_origin_before_the_credentials`,
`crates/websocket-bridge/tests/websockets_py_interop_e2e.rs::daemon::ws_10_2_the_daemon_checks_the_origin_python_websockets_sends`
(Python `websockets` as the client, weekly job `integration-bridges-vendor`).
401 and 403:
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_4_1_a_token_without_the_required_claim_is_403`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_a_jwt_is_checked_and_its_subject_meets_the_acl`,
`crates/websocket-bridge/src/daemon/security.rs::tests::wsbridge_4_1_the_refusal_is_401_or_403_with_its_challenge`,
`crates/bridge-security/src/auth.rs::tests::rfc_6750_3_1_a_verified_token_without_the_required_claim_is_told_apart`.
Until this change a verified JWT without the claim got 401.

**Status:** done

### §4.2 Path routing /topics/<slug> + meta endpoints

**Spec:** §4.2 — `/topics/<slug>` default path; `ws_path` replaces it (the
topic is reachable there alone); `/topics/__catalog__` over WebSocket as the
first frame and to a plain HTTP GET, with the topics the client may read or
write; `/healthz` on the WebSocket listener, without credentials, also
without the metrics endpoint; the metrics at `metrics.path` on the admin
endpoint.

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

**Status:** done — the tests ran green on the Linux test host.

### §4.3 Frame format binary "ZDB1" + JSON mode

**Spec:** §4.3 — binary magic `0x5A 0x44 0x42 0x31`, flags, a 4-byte
encap header `[0x00,0x07,0x00,0x00]`, CDR payload; text-frame JSON mode
optional. Flag bit 1 on a sample its writer wrote inside a coherent set; a
client's binary frame with a flag bit set gets an error 400. On a topic with
`idl_file` a binary frame under XCDR1 or another XCDR2 format than the
type's gets an error 400. The samples of two clients that write to the same
topic at the same time each go out under their own header.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs`
(`binary_sample_frame`, `split_binary_sample`, `json_sample_frames`,
`dds_sample_of`, `handle_binary_sample`),
`crates/websocket-bridge/src/daemon/router.rs` (`DdsSample::coherent_set`),
`crates/websocket-bridge/src/daemon/json_frames.rs` (JSON text frames, the
type from `idl_file`), `crates/websocket-bridge/src/codec.rs` (RFC-6455
codec).

**Tests:** `crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_3_a_sample_frame_is_topic_op_key_hash_data_and_time`,
`crates/websocket-bridge/src/daemon/json_frames.rs::tests::xtypes_7_4_2_a_client_sample_is_the_delimited_body_of_its_type`,
`crates/websocket-bridge/src/daemon/pump.rs::tests::xtypes_7_6_3_1_2_a_binary_frame_carries_the_identifier_the_sample_came_under`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`,
`tests/integration/tests/bridges.rs::websocket_bridge_against_python_websockets`,
`tests/integration/tests/bridges.rs::websocket_bridge_json_text_frames_against_python_websockets`,
`crates/websocket-bridge/tests/fuzz_smoke.rs`. Flag bit 1:
`crates/websocket-bridge/src/daemon/pump.rs::tests::wsbridge_4_3_a_sample_of_a_coherent_set_carries_bit_1`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_3_a_coherent_set_reaches_a_binary_client_with_bit_1`
(a writer in the daemon's participant writes two samples in a coherent set
and one behind it; the binary client gets the flags `02 02 00`; its frame
with bit 1 gets an error 400). Before, the daemon never set bit 1.
The representation of the DataWriter: Core::hold_writer holds the topic's
writer from setting the header to the write;
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_3_a_binary_frame_in_a_format_the_typed_writer_does_not_offer_is_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_3_samples_of_concurrent_clients_go_out_under_their_own_header`
(a binary client writes big endian, a JSON client little endian, 200
samples each at the same time; a third client checks header against body).
Without the lock this test failed on the Linux test host in 2 of 3 runs.
Before, a binary frame under XCDR1 set the header of a typed writer to
XCDR1, although it offers XCDR2 alone, and errors of the three setters were
discarded.

**Status:** done — the tests ran green on the Linux test host.

### §4.4 Control messages subscribe/unsubscribe/ping/pong/error

**Spec:** §4.4 — JSON control messages bidir + RFC-6455 PING/PONG. The
control messages are text frames; a binary message without `ZDB1` gets an
error 400, also when it holds UTF-8 JSON. A DDS write that fails gets the
error frame 500, for a JSON sample frame and a binary frame.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/daemon/json_frames.rs` (`parse_client`,
`subscribed`, `error`, `pong`), `crates/websocket-bridge/src/daemon/server.rs`
(`handle_inbound_frame`).

**Tests:** `crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_4_the_control_messages`,
`crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_4_a_message_of_another_form_is_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::direction_in_writes_into_dds_and_out_reads_from_it`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_4_a_control_message_in_a_binary_frame_is_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_4_a_dds_write_that_fails_is_an_error_frame_500`
(the test removes the daemon's DataWriter; a binary frame and a JSON sample
each get `{"op":"error","code":500}`, the counter errors_total rises by 2). Before, a
binary frame whose write failed got no error frame, and a binary message
with `{"op":"ping"}` was answered with `pong`.

**Status:** done — the tests ran green on the Linux test host.

### §4.5 Invalid data and closing

**Spec:** §4.5 — the daemon validates every frame of a client (RFC 6455
§10.7). Invalid data ends the connection with the Close frame of its status
code: 1002 (an unmasked frame, a reserved bit, a reserved opcode, a length
not in its shortest form, a fragmented or too long control frame, a broken
fragment sequence, an invalid Close body), 1007 (a text or a Close reason
that is not UTF-8, with the octet that makes it malformed), 1009 (a frame
over `limits.max_frame_size` or a message over `limits.max_message_size`,
16 MiB each by default, the frame from its header). A fragmented message is
reassembled. A Close of the client is answered with a Close of the same
status code. Behind every Close frame the daemon sends (the answer, the
Close for invalid data, the Close 1001 of the shutdown, §9.2) it ends its
sending direction, over TLS with `close_notify` before the FIN, and reads
for at most 500 ms until the client closes.

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
(both sides of the 500 ms: after 250 ms the daemon holds the connection and
takes octets, after 1 s the connection has ended; after a Close of the
client and after an unmasked frame),
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_1_the_daemon_reads_on_behind_the_close_of_its_shutdown`
(the same behind the Close 1001),
`crates/websocket-bridge/tests/security_e2e.rs::ws_7_1_the_daemon_ends_a_tls_session_with_close_notify_behind_its_close_frame`
(over TLS: Close frame, the close_notify alert, FIN). The limits of the
configuration:
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_holds_the_frame_limit_of_its_configuration`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_10_4_the_daemon_holds_the_message_limit_of_its_configuration`.

**Status:** done

## §5 Topic mapping

### §5.1 Slug algorithm topic name → URL

**Spec:** §5.1 — lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; override per
`ws_path`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (slug helper),
`crates/websocket-bridge/src/daemon/router.rs`.

**Tests:** `crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_5_1_slug_strips_double_colon`,
`::wsbridge_5_1_slug_replaces_unsafe_chars`;
`crates/websocket-bridge/src/uri.rs::tests` (14 tests) covers the URL form;
the path routing `crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_5_1_dispatch_to_subscribed_connection`
and `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

### §5.2 Type discovery catalog frame + /schema/<slug>

**Spec:** §5.2 — the catalog frame `{"op":"catalog","topics":[…]}` with
`name`, `type`, `direction`, `ws_path`, `qos` (`reliability`, `durability`,
`history` with `kind` and `depth`) and `schema_url` for a topic with
`idl_file`; as the first text frame on `/topics/__catalog__` and, with
`catalog_on_connect: true`, on every connection; only the topics the client
may read or write.

**Repo:** `crates/websocket-bridge/src/daemon/runtime_common.rs`
(`CatalogSnapshot::render_json_where`: `history` as an object,
`schema_url`), `crates/websocket-bridge/src/daemon/config.rs` (key
`catalog_on_connect`), `crates/websocket-bridge/src/daemon/server.rs`
(`serve_connection`: the frame on every connection with the key;
`settings_that_need_a_restart`), `man/man5/zerodds-ws-bridged.yaml.5`,
`packaging/linux/configs/ws-bridged.yaml.example`,
`docs/doc-map/config.en.md`. Before, the item named `router.rs` and a
catalog frame that no connection but one of the catalog path got, without
`schema_url`.

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_the_catalog_frame_names_topics_qos_and_schema_url`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_catalog_render_json_contains_topic_fields`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_5_2_catalog_on_connect_is_read`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_a_connection_gets_the_catalog_frame_when_configured`
(with the key, the catalog is the first frame on a topic path and on `/`,
and on the path of a topic without a type before the 415 error frame;
without the key, not),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_4_2_the_catalog_path_serves_the_catalog_over_ws_and_http`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_4_2_the_catalog_names_the_topics_its_client_may_use`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_admin_endpoint_serves_catalog_and_healthz_and_metrics`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_admin_endpoint_serves_catalog_metrics_healthz`.

**Status:** done — the tests ran green on the Linux test host.

### §5.2a `/schema/<slug>` IDL endpoint

**Spec:** §5.2 — a plain HTTP GET of `/schema/<slug>` on the WebSocket
listener serves the topic's IDL (or the XML type representation); `<slug>`
is the slug of §5.1 of the topic's name.

**Repo:** `crates/websocket-bridge/src/daemon/schema.rs` (`slug`,
`schema_url`, `route`, `expand_includes`: the IDL of `idl_file` with the
`#include` files in place; `xml_type_representation`: the XML type
representation of DDS-XTypes 1.3 §7.3.2 and Annex A, each type used before
its first use, in its `<module>`s),
`crates/websocket-bridge/src/daemon/json_frames.rs` (`TopicType::idl`,
`TopicType::dynamic_type`), `crates/websocket-bridge/src/daemon/server.rs`
(`schema_response`: 200 as `text/plain` or `application/xml`, 404 for a slug
of no topic or a topic without `idl_file`, 403 without a read or write
grant, 501 for a type without an XML form; origin and credentials as for a
topic path), `crates/websocket-bridge/src/daemon/config.rs` (two topics with
`idl_file` and one slug prevent the start).

**Tests:** `crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_the_schema_path_maps_a_slug_to_its_topic`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_the_includes_are_in_place`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_an_include_of_no_file_stays`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_an_include_cycle_is_an_error`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_a_header_with_pragma_once_is_in_place_once`
(a header with `#pragma once` that two files include is in the document
once, without the pragma line; a header without the line at each
`#include`),
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_an_include_line_is_recognised`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_types_nested_deeper_than_64_have_no_xml_form`
(a type nested deeper than 64 levels has no XML form: an error),
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_the_xml_form_is_the_type_of_the_idl`,
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_the_xml_form_of_a_union_a_bitmask_and_a_mutable_struct`
(the XML form, read by the DDS-XML front end `zerodds-xml-compose`, is the
same type as the one of the IDL front end: `equals_ignoring_annotations`;
both front ends are own code, not a foreign implementation),
`crates/websocket-bridge/src/daemon/schema.rs::tests::wsbridge_5_2_a_collection_of_collections_has_no_xml_form`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_5_2_two_topics_of_one_schema_slug_are_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_the_schema_endpoint_returns_the_idl_of_the_topic`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_5_2_the_schema_needs_the_credentials_and_a_grant`;
with a foreign IDL compiler
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_5_2_cyclone_idlc_compiles_the_served_idl`
(the served IDL of a topic whose `idl_file` includes a second file compiles
with Cyclone DDS `idlc` in a directory without that file; weekly job
`integration-bridges-vendor`, scenario `websocket-cyclone-idlc`).

**Status:** done — the tests ran green on the Linux test host (the Cyclone
test with `--features cross-vendor-tests --run-ignored all`). Not checked:
the XML form with a foreign tool (`rtiddsgen` is not on the test host).

---

## §6 QoS translation

### §6 DDS-QoS → WS-behavior map

**Spec:** §6 — RELIABLE: on a full send queue
(`max_send_queue_per_connection`) the topic waits for the client, no sample
is lost for a connected client; a client that takes no frame for
`ping_interval_ms` is closed; BEST_EFFORT: the oldest BEST_EFFORT sample of
the full queue is dropped, the new one when the queue holds RELIABLE samples
only, and every drop is counted; durability other than VOLATILE: the last N samples of each instance
(KEEP_LAST) or all (KEEP_ALL), at most `max_burst_samples`, go to a
connection when it subscribes, before the live samples; deadline
(`qos.deadline_ms`): `{"op":"deadline_missed"}` per missed period;
liveliness: the lease of the DDS endpoints (`qos.liveliness_lease_ms`), a
WebSocket Ping every `ping_interval_ms`, without a Pong Close 1011; partition
(`qos.partition`) on the DataReader and the DataWriter.

**Repo:** `crates/websocket-bridge/src/daemon/qos_translation.rs` (`topic_qos`, `behavior_for`,
`dds_qos_to_ws_behavior`: the one map),
`crates/websocket-bridge/src/daemon/router.rs` (`ConnQueue`, `Router::route`, `Route::deliver`,
`Router::subscribe` with the burst, `Router::deadlines_missed`),
`crates/websocket-bridge/src/daemon/server.rs` (pump, deadline thread, Ping in the writer thread,
`register_topic_endpoints`), `crates/websocket-bridge/src/daemon/config.rs` (keys),
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
The bound of the control messages and the bursts in the queue
(`MAX_QUEUED_CONTROLS`, one burst per topic, Close 1008 on overflow):
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_a_newer_burst_of_a_topic_replaces_the_waiting_one`,
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_control_messages_over_the_bound_overflow_the_queue`;
the Close 1008 of the writer thread has no end-to-end test.
Before, `crates/websocket-bridge/src/daemon/qos_translation.rs` was used nowhere, the queue was unbounded, and
deadline, liveliness, partition and the burst were missing. Until this
change the control entries of the queue had no bound: a client that did not
read and repeated subscribe/unsubscribe queued a burst per subscribe.

**Status:** partial — the tests ran green on the Linux test host. Open:
the daemon does not compute the KeyHash from the payload. The readers of a
topic with key get no type support for the key (`zerodds-dcps` lacks the
reader counterpart of `set_user_writer_key_support`, and the daemon would
have to build a `SerializedTypeSupport` from the `idl_file`). A sample
without `PID_KEY_HASH` on such a topic therefore names no instance: it
reaches the clients, but it is not kept for later subscribers and starts no
deadline; it is counted in `zerodds_ws_bridge_errors_total` and logged once
per topic (specification §6). A topic without `idl_file` is a topic without
key. Each drop counts in
`zerodds_ws_bridge_send_queue_drops_total{connection_id}` (§8.2).

## §7 Security

### §7.1 TLS wss:// + cert rotation

**Spec:** §7.1 — `wss://` mode per `tls.enabled`, SIGHUP cert rotation: the
daemon reads certificate and key again, new sessions get the new
certificate, existing connections keep their session.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (TLS hook),
`crates/websocket-bridge/src/daemon/security.rs` (rustls wire-up),
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::ws_10_6_tls_plain_handshake_succeeds_then_data_flows`
(frames in both directions over the TLS session),
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_sighup_cert_reload_keeps_daemon_running_with_new_cert`
(after the reload: a new session with the new certificate, the old
connection goes on exchanging frames, a client that trusts the old
certificate alone fails the handshake),
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_the_binary_reloads_the_certificate_on_sighup`
(the same with SIGHUP to the binary);
`crates/websocket-bridge/src/daemon/security.rs::tests::ws_10_6_ctx_from_config_tls_enabled_without_paths_rejected`;
`crates/bridge-security/src/connection.rs::tests::mqttbridge_7_1_rotating_config_reload_swaps_inner_arc`.

**Status:** done

### §7.2 Auth modes none/bearer/jwt/mtls

**Spec:** §7.2 — four auth modes via header or cert.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs`,
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/auth.rs` (none/bearer/jwt/mtls);
`jwt` with the `auth.jwt` block of §3 (`public_key`, `audience`,
`required_claim`, plus `issuer`) through `SecurityConfig::apply_jwt`. Before,
`jwt` could be chosen but no key given, and the start ended as a bind error
(exit 2, measured). The mTLS subject is the certificate's subject DN
(RFC 4514); an ACL rule may name the fingerprint `mtls:<SHA-256>` instead.
`jwt` checks RS256 and ES256 (`JwtKey`, `jwt_key_from_pem`: an RSA key for
RS256, a P-256 `PUBLIC KEY` for ES256, signature `R || S` of 64 octets); the
token's `alg` has to fit the key. Before, `crates/bridge-security/src/auth.rs` accepted RS256 only.
`none` starts on a loopback address only
(`crates/websocket-bridge/src/daemon/config.rs::listen_is_loopback`, `DaemonConfig::check_auth`); on `0.0.0.0`
it is a configuration error (exit 1), `--check-config` reports the
configuration invalid (exit 1), a SIGHUP reload to `none` is refused. The
reload checks against the address the daemon serves (`crates/websocket-bridge/src/daemon/server.rs::reload`
with the `listen` of the start configuration), because `listen` waits for a
restart. Before, `none` started on any address, and a reload that set
`listen` to `127.0.0.1` and `mode` to `none` turned `none` on for the
`0.0.0.0` that stayed bound. `bearer` without a token or with an empty one
(an unset `${VAR}`) is a configuration error; before, the daemon started
with it.

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
(also the reload that moves `listen` to `127.0.0.1`: still 401),
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_a_client_certificate_subject_meets_the_acl`;
`crates/bridge-security/src/auth.rs::tests::jwt_es256_rfc_7515_a_3_example_verifies`,
`::jwt_es256_is_verified`, `::jwt_alg_must_match_the_key_type`.

**Status:** done — the tests ran green on the Linux test host.

### §7.3 Per-topic ACL read/write

**Spec:** §7.3 — `acl.read`/`acl.write` lists with `*`/exact/group match.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (ACL fields),
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs` (match engine).

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_auth_valid_bearer_then_acl_allows_subscribed_topic`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_3_auth_valid_bearer_but_acl_denies_forbidden_topic`;
`crates/websocket-bridge/src/daemon/security.rs::tests::wsbridge_7_3_ws_acl_check_allows_listed_user`;
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_7_3_example_yaml_topic_acl_reaches_the_daemon`;
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_3_a_jwt_group_meets_a_group_rule`
(`*group:engineers*` against the `groups` claim of a JWT through the daemon: 101 with the group, 403 without);
`crates/bridge-security/src/acl.rs::tests::group_wildcard_allows_members`.

**Status:** done — the tests ran green on the Linux test host.

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log on stdout, one object per line, fields
timestamp/level/event/connection_id/topic/bytes/peer/latency_us; `log_level`
(`--log-level`) as the threshold, a SIGHUP reload applies a changed value;
the table of the events with level and fields.

**Repo:** `crates/websocket-bridge/src/daemon/log.rs` (`Level`, `set_level`,
`Event::field/number/integer/latency/render/emit`, `rfc3339`),
`crates/websocket-bridge/src/daemon/server.rs` (`ConnLog`, `log_frame_in`,
`WsOut::message_on` for `frame_out`, the events of the start, the
connections, the refusals, the reload and the shutdown; `peer` from
`TcpStream::peer_addr` in the accept thread),
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs` (`running`,
`stopped`, `start_failed` on stdout and as text on stderr). Before, the
daemon wrote free text with `eprintln!` to stderr, and `log_level` was read
and used by no code path.

**Tests:** `crates/websocket-bridge/src/daemon/log.rs::tests::wsbridge_8_1_a_line_is_a_json_object_with_the_spec_fields`,
`crates/websocket-bridge/src/daemon/log.rs::tests::wsbridge_8_1_the_log_level_is_the_threshold`,
`crates/websocket-bridge/src/daemon/log.rs::tests::rfc3339_5_6_the_utc_date_and_time`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_1_the_binary_logs_json_lines_for_a_frame_exchange`
(the binary with `log_level: debug`: every line is JSON; `connection_open`,
`frame_in` with `connection_id`, `topic`, `bytes`, `peer`, `latency_us`,
the `frame_out` of the answer, `frame_refused` with its reason,
`connection_close`; with `log_level: error` the same exchange writes no
line);
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_1_a_sighup_reload_applies_a_changed_log_level`
(the binary starts at `error`, a frame exchange writes no line; after the
file is rewritten to `debug` and SIGHUP, the same connection writes
`frame_in` and `frame_out`; after it is rewritten to `warn` and SIGHUP, no
line again);
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_the_binary_reloads_the_certificate_on_sighup`
reads `running` and `reload` from the log.

**Status:** done — the tests ran green on the Linux test host.
### §8.2 Prometheus metrics

**Spec:** §8.2 — `metrics.*` config; the ten families
`zerodds_ws_bridge_*` with their labels, plus `errors_total`.

**Repo:** `crates/websocket-bridge/src/daemon/runtime_common.rs`
(`BridgeMetrics`, `family`, `auth_reason`: `frame_in`/`frame_out` under
`{topic, direction}`, `send_queue_drop` under `{connection_id}`,
`dds_received`/`dds_published` under `{topic}`, `auth_failure` under
`{reason}`), `crates/websocket-bridge/src/daemon/server.rs` (the counting
sites: `WsOut::message_on`, `handle_inbound_frame`, `handle_binary_sample`,
the pumps, auth and ACL), `crates/websocket-bridge/src/daemon/router.rs`
(`Delivery::dropped_on`), `crates/websocket-bridge/src/daemon/config.rs`
(`metrics.*`). Before, the code carried nine families `zerodds_ws_*`
without labels; `send_queue_drops_total{connection_id}` and
`auth_failures_total` were missing. A sample frame whose `topic` is not
configured counts under `topic=""` (`BridgeMetrics::by_topic`); before, each
new name a client sent added two series. `connections_total` counts in the
accept thread, so also a connection whose TLS handshake fails; before, only
after the TLS handshake. An ACL refusal counts in `auth_failures_total`
only, an error frame of §4.4 once in `errors_total`; before, an ACL refusal
of a sample frame also counted in `errors_total`, and a JSON error frame
counted there twice.

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_2_metrics_register_is_idempotent`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_2_the_ten_families_carry_their_labels`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_admin_endpoint_serves_catalog_and_healthz_and_metrics`;
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_2_a_frame_each_way_counts_under_its_topic`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_2_a_sample_frame_of_an_unknown_topic_counts_under_no_topic`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_admin_endpoint_serves_catalog_metrics_healthz`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_6_a_best_effort_drop_is_counted`
(the drops under `connection_id="1"`, from the Prometheus text of the admin endpoint);
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_8_2_a_refused_token_counts_as_auth_failure`
(ACL refusal of `subscribe`, of a JSON and of a binary sample frame, `errors_total` 0),
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_8_2_a_failed_tls_handshake_counts_as_an_accepted_connection`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_4_1_a_token_without_the_required_claim_is_403`;
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_6_a_best_effort_sample_never_drops_a_reliable_one`.

**Status:** done — the tests ran green on the Linux test host. The spec
names what `direction` means (the topic's `direction` in the
configuration), the values of `reason`, and `errors_total`.

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` ENV enables span emission via
`zerodds-observability-otlp`: one trace per connection with the root
`ws.connection`, per message `ws.frame.in` (with `dds.write`) and
`ws.frame.out`; a refused opening handshake as a `ws.connection` with status
error.

**Repo:** `crates/websocket-bridge/src/daemon/trace.rs` (`Tracer`,
`ConnectionSpan`, the spans and their attributes),
`crates/websocket-bridge/src/daemon/server.rs` (`serve_connection`: the root
from the opening handshake, `trace_frame_in`, `WsOut::message_on`, the times
of `dds.write` in `handle_inbound_frame` and `handle_binary_sample`;
`DaemonHandle::shutdown` sends once more after the last connection),
`crates/websocket-bridge/src/daemon/runtime_common.rs` (endpoint from the
environment, flush every 5 s, the stop is checked every 100 ms).

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
(also a request that is no UTF-8 as a 400 with status error, a connection
the client resets with status error and the reason of the read),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_3_a_connection_that_fails_on_a_write_timeout_is_an_error_span`,
`crates/websocket-bridge/src/daemon/server.rs::tests::wsbridge_8_3_a_failed_write_fails_the_connection_span`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_3_otlp_config_from_endpoint_parses_http_url`,
`::wsbridge_8_3_otlp_config_from_endpoint_parses_bare_host_port`,
`::wsbridge_8_3_otlp_config_from_endpoint_handles_https_and_trailing_slash`,
`::wsbridge_8_3_otlp_config_from_endpoint_falls_back_to_default_port`.

**Status:** done — the tests ran green on the Linux test host. The end-to-end
test starts the binary with an OTLP collector in the test and checks root,
children, parent span ids, trace id and attributes in the
`POST /v1/traces`. Every refused opening handshake is a `ws.connection`
with status error, also the 400 of a request that is no UTF-8; a connection
that fails on a socket error (a read error, a write the client does not
take within `ping_interval_ms`, a Ping that cannot be written) has status
error with the reason, and a write after the daemon's Close does not count
as one; a failed write of the 101 response ends the root span with status
error. §8.3a is folded into this item.

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config parse → TLS → DCPS → reader/writer register → WS
bind → signal handler.

**Repo:** `crates/websocket-bridge/src/daemon/mod.rs`,
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_handshake_roundtrip_succeeds`
(spawn sequence).

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain (max 30 s), `op:shutdown` frame, close
1001, cleanup; SIGHUP hot-reload TLS+ACL.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (signal + drain),
`crates/websocket-bridge/src/daemon/runtime_common.rs`
(SIGTERM/SIGINT/SIGHUP via the cluster-A signal handler);
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`. The order of
`DaemonHandle::shutdown`: accepting ends, the pumps hand on what their
DataReaders hold and end (`draining`); then `{"op":"shutdown"}` behind the
samples of each queue (`json_frames::shutdown`), Close 1001, reading the
client's Close; all within `shutdown_drain_ms` (default 30000,
`crates/websocket-bridge/src/daemon/config.rs`), then the stop flag and at most 2 s for the remaining
connections. SIGTERM/SIGINT set `shutdown_requested`, no longer the stop
flag. systemd unit: `TimeoutStopSec=40`. Before, the daemon sent no
`shutdown` frame, and the stop flag ended the pumps before their channels
were empty.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_the_binary_exits_0_on_sigterm`
(the binary was still running 3 s after SIGTERM before, measured),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_graceful_shutdown_completes_within_5s`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_sighup_cert_reload_keeps_daemon_running_with_new_cert`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_9_2_sighup_applies_the_reloaded_acl` (ACL and auth, 403 before the reload, 101 after),
`crates/websocket-bridge/src/daemon/server.rs::tests::wsbridge_9_2_a_reload_names_the_changed_settings_that_need_a_restart` (the list of changed settings that wait for a restart: `limits`, `origin`, topic map, TLS file paths, `listen`, `domain`);
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_9_2_shutdown_broadcasts_to_all`;
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_samples_queued_before_the_signal_arrive_before_the_shutdown_frame`
(300 samples that still wait in the daemon when the shutdown begins arrive, all of them and in order, before `{"op":"shutdown"}`, then Close 1001),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_the_drain_is_bounded_by_shutdown_drain_ms`,
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_the_daemon_closes_every_connection_with_1001_when_it_shuts_down`
(16 connections: `{"op":"shutdown"}`, then Close 1001),
`crates/websocket-bridge/tests/daemon_rfc6455.rs::ws_7_1_the_binary_closes_every_connection_with_1001_on_sigterm`
(the same through the binary and SIGTERM, octet by octet),
`crates/websocket-bridge/tests/security_e2e.rs::ws_7_1_the_daemon_ends_a_tls_session_with_close_notify_behind_its_close_frame`,
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_9_2_shutdown_drain_ms_is_read`;
the close codes themselves `crates/websocket-bridge/src/close.rs::tests` (25).

**Status:** partial — the tests ran green on the Linux test host. Open:
on Windows nothing sets the shutdown flag (`install_signal_watcher` is an
empty thread there; the console handler for Ctrl+C and the stop of the
Windows service `ZeroDDSWSBridge` are missing). The Linux test host has no
Windows target and there is no Windows runner; no unchecked code was
written.

## §10 Cross-vendor

### §10 RTPS peer co-existence Cyclone/RTI/Fast DDS

**Spec:** §10 — the daemon is a normal RTPS peer; verified in
`crates/websocket-bridge/tests/cross_vendor.rs`.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (DCPS wire-up;
`register_topic_endpoints`: the DataWriter of a topic with `idl_file` offers
XCDR2 alone and carries the header of the type's extensibility from the
start, `json_frames::TopicType::identifier`),
`crates/websocket-bridge/tests/interop/{reading.idl,cyclone_reading.c}`
(Cyclone DDS participant, built by the test with `idlc` and the C compiler),
`crates/websocket-bridge/tests/interop/fastdds_reading.cpp` (Fast DDS 3
participant, the type through the DynamicTypes API, XCDR2, built by the test
with the C++ compiler), the `shape_main` of RTI Connext DDS 7.7.0 from the
release of the OMG suite `dds-rtps` (`interop/shapes/omg-dds-rtps.idl`,
`ShapeType`, `-x 2`), `tests/integration/provision.sh` (`fastdds-dev`,
`connext-shape-main`: the release asset with the sha256 of
`tests/interop/omg/vendors.tsv` into `/opt/omg-connext`),
`ci/integration_scenarios.py` (job `integration-bridges-vendor`, feature
`cross-vendor-tests`, scenarios `websocket-cyclone`, `websocket-fastdds`,
`websocket-connext`). Before, the writer
offered `[XCDR, XCDR2]` and sent XCDR2; a Cyclone DDS reader did not take a
client's JSON samples (measured: matched, no sample).

**Tests:** `crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_a_ws_publish_reaches_cyclone_and_a_cyclone_sample_a_ws_client`
(Cyclone DDS 0.10.5: a JSON `publish` reaches the Cyclone reader as that
sample; a sample of the Cyclone writer reaches a JSON client as JSON and a
binary client as a ZDB1 frame; the ZDB1 frame, sent back by the client,
reaches the Cyclone reader again),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`
(DDS round trip through a DomainParticipant),
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_fastdds_a_ws_publish_reaches_fastdds_and_a_fastdds_sample_a_ws_client`
(the same ways with Fast DDS 3.1.2),
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_rti_a_ws_publish_reaches_connext_and_a_connext_sample_a_ws_client`
(the same ways with RTI Connext DDS 7.7.0 on `Square`: the Connext reader
prints the shape of the JSON `publish`, a shape of the Connext writer
reaches the JSON client and, as a ZDB1 frame with `D_CDR2`, the binary
client, the ZDB1 frame reaches the Connext reader again with the same `x`,
`y`).

**Status:** done — the tests ran green on the Linux test host
(`--features cross-vendor-tests --run-ignored all`); the Connext test with
the `shape_main` of the release asset (`CONNEXT_SHAPE_MAIN`).

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-ws-bridged`; config defaults per OS;
systemd/launchd/Windows service; Docker `zerodds/ws-bridged:1.0`; manuals
1+5.

**Repo:** `packaging/linux/systemd/zerodds-ws-bridged.service`,
`packaging/macos/launchd/org.zerodds.ws-bridged.plist`,
`packaging/macos/homebrew/zerodds-ws-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ws-bridged/` (the image carries a configuration of its
own, `ws-bridged.yaml`: `0.0.0.0:8080`, `bearer` with the token of
`WS_BRIDGE_TOKEN`; before, it copied the example with `127.0.0.1` and
`none`, which a published port could not reach),
`packaging/docker/docker-compose.yml` (refers to that file instead of the
missing `conf/ws-bridged.yaml`),
`packaging/linux/configs/ws-bridged.yaml.example`,
`man/man1/zerodds-ws-bridged.1`, `man/man5/zerodds-ws-bridged.yaml.5`;
the Windows service: `crates/dcps/src/cli.rs` (`run_as_service`,
`request_service_stop`, `request_service_reload`, `READY=1` and
`STOPPING=1` of `notify_service_manager` as service states),
`crates/dcps/src/cli/windows_service.rs` (`StartServiceCtrlDispatcherW`,
`RegisterServiceCtrlHandlerExW` with stop, shutdown, paramchange and
interrogate, `SetServiceStatus` from `SERVICE_START_PENDING` to
`SERVICE_STOPPED` with the exit code, the console control handler when
the process runs from a console),
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs` (`main` runs the
daemon through `run_as_service("ZeroDDSWSBridge", …)`),
`crates/websocket-bridge/src/daemon/runtime_common.rs`
(`install_signal_watcher` off Unix: `watch_service_requests` sets the
shutdown and reload flags of SIGTERM and SIGHUP), the spec §11 and
`man/man1/zerodds-ws-bridged.1` (WINDOWS SERVICE) name the service states
and controls. Before, the binary had no
service control dispatcher and the watcher off Unix was an empty thread.

**Tests:** `crates/websocket-bridge/tests/example_yaml_loadback.rs::{wsbridge_3_example_yaml_parses_without_error, wsbridge_3_example_yaml_key_fields_match_intent, wsbridge_7_3_example_yaml_topic_acl_reaches_the_daemon, wsbridge_11_example_yaml_has_no_unknown_top_level_keys}`
(the shipped example configuration through the real parser, with the values it states);
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_11_the_docker_image_serves_its_published_port_with_a_token`
(the image's configuration: valid with a token, refused without);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys);
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_11_the_service_requests_set_the_shutdown_and_reload_flags`
(the watcher of Windows in a thread on Linux: without a request it waits,
it ends with the daemon, a reload request sets the reload flag, a stop
request the shutdown flag);
`crates/dcps/src/cli.rs::service_tests::{deployment_4_3_the_service_controls_are_stop_reload_and_interrogate, deployment_4_3_the_notify_states_are_service_states, deployment_4_3_the_exit_code_has_its_number, deployment_4_3_the_requests_and_the_run}`
(the control codes, the states, the exit code of a stopped service, the
requests and the run outside the service manager);
`crates/dcps/src/cli/windows_service.rs::tests::deployment_4_3_the_control_codes_are_the_win32_constants`
(Windows only).

**Status:** partial — the tests ran green on the Linux test host; the
Windows code was compiled there with `cargo clippy --target
x86_64-pc-windows-gnu -p zerodds-websocket-bridge --all-targets -- -D
warnings` and the same for `zerodds-dcps` (`--lib --tests`), not run.
Missing: (1) the run of the service `ZeroDDSWSBridge` under `sc.exe`
(start, `READY=1` as `SERVICE_RUNNING`, stop with the drain, `paramchange`
as reload); it needs a Windows host with the service registered, which
the Linux test host is not; the Windows job of `zerodds-dcps` (the Windows
runner, user SYSTEM) runs the tests of `cli`, among them the console fallback of
`run_as_service`, and has not run on this change. (2) No job starts the image
`packaging/docker/ws-bridged/` and talks to it with a client; `release.yml`
builds it only for a release, and on the measurement host (an unprivileged
LXC container) `docker build` fails according to `ci/release-packaging-check.yml` (read, not measured).

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — ≥ 5 unit tests each in the modules configuration
(`src/daemon/config.rs`), topic map and send queues (`src/daemon/router.rs`),
authentication and ACL (`src/daemon/security.rs`), frame codec
(`src/codec.rs`) and DDS pump (`src/daemon/pump.rs`). Before, the spec named
`src/bin/`, where no module lives.

**Repo:** `crates/websocket-bridge/src/daemon/pump.rs` (taken out of
`crates/websocket-bridge/src/daemon/server.rs`: `dds_sample_of`, `binary_sample_frame`, `split_binary_sample`,
`check_client_flags`, `writer_form`, `json_sample_frames`),
`crates/websocket-bridge/src/{codec.rs,daemon/config.rs,daemon/router.rs,daemon/security.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` per module (number of `#[test]`
in the source):
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
The pump: `crates/websocket-bridge/src/daemon/pump.rs::tests::wsbridge_12_1_an_alive_sample_is_a_frame_with_flags_0`,
`::wsbridge_12_1_a_dispose_carries_bit_0_and_the_key_holder`,
`::wsbridge_12_1_an_unregister_carries_bit_2`,
`::wsbridge_12_1_a_marker_that_is_both_carries_both`,
`::wsbridge_12_1_the_coherent_set_end_is_no_frame`,
`::wsbridge_12_1_a_client_frame_with_any_flag_bit_is_refused`,
`::wsbridge_12_1_a_sample_of_a_topic_without_type_is_the_error_415`
(sample → frame for a sample, a dispose, an unregister, both, the end of a
coherent set; the flags of a client frame). Plus `crates/websocket-bridge/tests/fuzz_smoke.rs::wsbridge_12_1_fuzz_websocket_decode_no_panic`,
`::wsbridge_12_1_empty_input_no_panic`, `::wsbridge_12_1_single_byte_inputs_no_panic`,
`::wsbridge_12_1_extended_length_overflow_no_oom`.

**Status:** done — the tests ran green on the Linux test host.

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — the daemon as a subprocess, a WS client publishes, a DDS
reader in the test process receives; byte-exact round-trip. The file is
`tests/daemon_e2e.rs` (the spec named `tests/bridge_e2e.rs`, which did not
exist).

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_12_2_a_ws_publish_reaches_a_dds_reader_byte_exact`
(the binary as a subprocess, a `DcpsRuntime` of the test process on the
leased domain with a reader and a writer, discovery between the processes:
the ZDB1 frame of a binary client reaches the reader with the same CDR bytes
and the header `00 07`; a sample of the writer reaches the client as a ZDB1
frame with the same bytes and header);
the scenarios of the integration pillar with Python `websockets` as the
client against the binary: `tests/integration/tests/bridges.rs::websocket_bridge_against_python_websockets`,
`tests/integration/tests/bridges.rs::websocket_bridge_json_text_frames_against_python_websockets`.
Before, this item cited tests with the daemon in the test process and
injected reader samples, without a WS publish that a DDS reader receives.

**Status:** done — the tests ran green on the Linux test host.

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — the daemon in the test process against a foreign DDS
participant in both directions (Cyclone DDS, Fast DDS, RTI Connext DDS),
feature `cross-vendor-tests`, job `integration-bridges-vendor`. Before, the
spec named a Cyclone subscriber in Docker Compose that did not exist.

**Repo:** `crates/websocket-bridge/tests/cross_vendor.rs`,
`crates/websocket-bridge/tests/interop/`, `tests/integration/provision.sh`,
`ci/integration_scenarios.py`.

**Tests:** `crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_a_ws_publish_reaches_cyclone_and_a_cyclone_sample_a_ws_client`,
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_fastdds_a_ws_publish_reaches_fastdds_and_a_fastdds_sample_a_ws_client`,
`crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_10_rti_a_ws_publish_reaches_connext_and_a_connext_sample_a_ws_client`.
The handshake test of a raw client that stood here ran against the own
daemon without a foreign participant; it is now
`crates/websocket-bridge/tests/daemon_rfc6455.rs::wsbridge_4_1_a_raw_tcp_client_gets_101_and_the_accept_hash`
under §4.1.

**Status:** done — the tests ran green on the Linux test host.

## §13 Cross-references

### §13 Related library + daemons

**Spec:** §13 — library `crates/websocket-bridge/`, the wire-format spec
`zerodds-xcdr2-bindings-conformance-1.0` §3, the deployment spec
`zerodds-deployment-1.0`, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config, major=wire-protocol
change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — rule for assigning the version number of a release, not a property of the daemon.

---

## Audit status

23 done / 3 partial / 0 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-ws-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (job `merge-rust-bridge-security`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (job `merge-rust-websocket-bridge`) — 52 tests green.
* `python3 ci/run_integration.py integration-bridges` (job `integration-bridges`) — 2 tests green.

59 cited tests green, 0 not green.

Open items: `zerodds-ws-bridge-1.0.open.en.md`.
