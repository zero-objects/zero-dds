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

**Status:** n/a (informative) — assignment of the levels; the requirements are the items §2 to §12.

## §2 CLI surface

### §2 zerodds-ws-bridged CLI

**Spec:** §2 — options `--config`/`--listen`/`--domain`/`--topic`/
`--tls-cert`/`--tls-key`/`--auth-token`/`--log-level`/`--metrics`/
`--version`/`--help`; exit codes 0/1/2/3/4.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/websocket-bridge/src/daemon/cli.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_handshake_roundtrip_succeeds`,
`::wsbridge_2_rejects_non_upgrade_request`,
`::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

## §3 Config-file format

### §3 YAML/JSON/TOML config loader

**Spec:** §3 — top-level `listen`/`domain`/`log_level`/`tls`/`auth`/
`topics`/`metrics`; ENV substitution `${VAR}` and `${VAR:-default}`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs`,
`crates/websocket-bridge/src/daemon/mod.rs` (DaemonConfig).

**Tests:** `crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_3_example_yaml_parses_without_error`,
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_3_example_yaml_key_fields_match_intent`
(loads the example of the specification). Loader round-trip coverage in
`crates/websocket-bridge/src/daemon/config.rs::tests`.

**Status:** done

## §4 WebSocket wire protocol

### §4.1 RFC 6455 handshake + zerodds-ws-bridge-1.0 subprotocol

**Spec:** §4.1 — RFC-6455 upgrade; subprotocol header
`Sec-WebSocket-Protocol: zerodds-ws-bridge-1.0`, each name a token
(RFC 6455 §4.1, the `token` of RFC 7230 §3.2.6); auth header per bearer
mode; 401/403 on auth failure.

**Repo:** `crates/websocket-bridge/src/handshake.rs`,
`crates/websocket-bridge/src/negotiation.rs`,
`crates/websocket-bridge/src/daemon/server.rs` (upgrade path).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_handshake_roundtrip_succeeds`,
`::wsbridge_2_rejects_non_upgrade_request`,
`crates/websocket-bridge/src/daemon/server.rs::tests::rfc_6455_4_1_the_subprotocols_are_tokens`;
the Python library `websockets` (its own check of subprotocol names) in the
scenario `tests/integration/tests/bridges.rs::websocket_bridge_json_text_frames_against_python_websockets`.
Until this change the subprotocols were named `zerodds-ws-bridge/1.0` and
`…/1.0+json`; `websockets` refused that as "invalid subprotocol" (`/` is a
separator); the `WebSocket` constructor of the WHATWG standard throws for
such a name as well (read, not measured).

**Status:** done

### §4.2 Path routing /topics/<slug> + meta endpoints

**Spec:** §4.2 — `/topics/<slug>` default path; `ws_path` override;
`/topics/__catalog__`/`/healthz`/`/metrics` meta endpoints.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/daemon/server.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`
(topic-path routing).

**Status:** done

### §4.3 Frame format binary "ZDB1" + JSON mode

**Spec:** §4.3 — binary magic `0x5A 0x44 0x42 0x31`, flags, a 4-byte
encap header `[0x00,0x07,0x00,0x00]`, CDR payload; text-frame JSON mode
optional.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs`
(`binary_sample_frame`, `split_binary_sample`, `json_sample_frames`),
`crates/websocket-bridge/src/daemon/json_frames.rs` (JSON text frames, the
type from `idl_file`), `crates/websocket-bridge/src/codec.rs` (RFC-6455
codec).

**Tests:** `crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_3_a_sample_frame_is_topic_op_key_hash_data_and_time`,
`crates/websocket-bridge/src/daemon/json_frames.rs::tests::xtypes_7_4_2_a_client_sample_is_the_delimited_body_of_its_type`,
`crates/websocket-bridge/src/daemon/server.rs::tests::xtypes_7_6_3_1_2_a_binary_frame_carries_the_identifier_the_sample_came_under`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`,
`tests/integration/tests/bridges.rs::websocket_bridge_against_python_websockets`,
`tests/integration/tests/bridges.rs::websocket_bridge_json_text_frames_against_python_websockets`,
`crates/websocket-bridge/tests/fuzz_smoke.rs`.

**Status:** done

### §4.4 Control messages subscribe/unsubscribe/ping/pong/error

**Spec:** §4.4 — JSON control messages bidir + RFC-6455 PING/PONG.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/daemon/json_frames.rs` (`parse_client`,
`subscribed`, `error`, `pong`), `crates/websocket-bridge/src/daemon/server.rs`
(`handle_inbound_frame`).

**Tests:** `crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_4_the_control_messages`,
`crates/websocket-bridge/src/daemon/json_frames.rs::tests::wsbridge_4_4_a_message_of_another_form_is_refused`,
`crates/websocket-bridge/tests/daemon_e2e.rs::direction_in_writes_into_dds_and_out_reads_from_it`.

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

**Spec:** §5.2 — catalog frame + a `/schema/<slug>` IDL endpoint.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs` (catalog + schema
endpoints).

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_catalog_render_json_contains_topic_fields`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_admin_endpoint_serves_catalog_and_healthz_and_metrics`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_admin_endpoint_serves_catalog_metrics_healthz`. No `/schema/<slug>`
endpoint exists in the tree — see §5.2a.

**Status:** partial — the catalog frame is covered; the IDL endpoint is carried as
§5.2a `open`.
### §5.2a `/schema/<slug>` IDL endpoint

**Spec:** §5.2 — an HTTP endpoint `/schema/<slug>` serves the topic's IDL.

**Repo:** —

**Tests:** —

**Status:** open — the part of §5.2 beyond the catalog frame. The tree has
no `/schema` path.

---

## §6 QoS translation

### §6 DDS-QoS → WS-behavior map

**Spec:** §6 — Reliability/Durability/History/Deadline/Liveliness/Partition
mapping.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (QoS pump);
`crates/websocket-bridge/src/daemon/qos_translation.rs`
(Reliability/Durability/History/Deadline/Liveliness/Partition map);
`crates/websocket-bridge/src/daemon/config.rs` (QoS fields).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`
(reliable round-trip); a cluster-A QoS matrix in
`crates/websocket-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS wss:// + cert rotation

**Spec:** §7.1 — `wss://` mode per `tls.enabled`, SIGHUP cert rotation.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (TLS hook),
`crates/websocket-bridge/src/daemon/security.rs` (rustls wire-up),
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::ws_10_6_tls_plain_handshake_succeeds_then_data_flows`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_sighup_cert_reload_keeps_daemon_running_with_new_cert`;
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

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_a_jwt_is_checked_and_its_subject_meets_the_acl`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_jwt_without_a_key_and_mtls_without_a_client_ca_are_config_errors`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_auth_missing_bearer_yields_401`, `crates/websocket-bridge/tests/security_e2e.rs::ws_10_7_auth_invalid_bearer_yields_401`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_auth_valid_bearer_then_acl_allows_subscribed_topic`;
`crates/websocket-bridge/src/daemon/security.rs::tests::wsbridge_7_2_ws_extracts_bearer_from_headers_case_insensitive`,
`::wsbridge_7_2_ws_missing_authorization_header_returns_missing`,
`::wsbridge_7_2_ws_none_auth_yields_anonymous_without_header`,
`::wsbridge_7_2_ctx_from_config_with_bearer_tokens`.

**Status:** done

### §7.3 Per-topic ACL read/write

**Spec:** §7.3 — `acl.read`/`acl.write` lists with `*`/exact/group match.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (ACL fields),
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs` (match engine).

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_2_auth_valid_bearer_then_acl_allows_subscribed_topic`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_3_auth_valid_bearer_but_acl_denies_forbidden_topic`;
`crates/websocket-bridge/src/daemon/security.rs::tests::wsbridge_7_3_ws_acl_check_allows_listed_user`;
`crates/websocket-bridge/tests/example_yaml_loadback.rs::wsbridge_7_3_example_yaml_topic_acl_reaches_the_daemon`.

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log on stdout, fields
timestamp/level/event/connection_id/topic/bytes/peer/latency_us; a
`--log-level` switch.

**Repo:** —

**Tests:** —

**Status:** open — `crates/websocket-bridge/src/daemon/cli.rs::tests` covers the flag
splitting, but nothing tests the JSON log form or the effect of
`--log-level`.
### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + `metrics.*` config; 11 counter/gauge
metric families.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (metrics listener),
`crates/websocket-bridge/src/daemon/config.rs` (metrics config block),
`crates/websocket-bridge/src/daemon/runtime_common.rs` (counter/gauge
families cluster-A wire-up).

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_2_metrics_register_is_idempotent`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_2_metrics_counter_visible_in_prometheus_render`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_5_2_admin_endpoint_serves_catalog_and_healthz_and_metrics`;
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_8_2_metrics_counters_track_frames_in_and_out`,
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_5_2_admin_endpoint_serves_catalog_metrics_healthz`.

**Status:** done

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` ENV enables span emission via
`zerodds-observability-otlp`.

**Repo:** `crates/websocket-bridge/src/daemon/runtime_common.rs` (OTLP init
via `zerodds-observability-otlp`),
`crates/websocket-bridge/src/daemon/server.rs` (span emit per frame).

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::wsbridge_8_3_otlp_config_from_endpoint_parses_http_url`,
`::wsbridge_8_3_otlp_config_from_endpoint_parses_bare_host_port`,
`::wsbridge_8_3_otlp_config_from_endpoint_handles_https_and_trailing_slash`,
`::wsbridge_8_3_otlp_config_from_endpoint_falls_back_to_default_port`. Nothing tests that
a span is emitted per frame roundtrip — see §8.3a.

**Status:** partial — the endpoint resolution is covered, the span emit is not.
## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config parse → TLS → DCPS → reader/writer register → WS
bind → signal handler.

**Repo:** `crates/websocket-bridge/src/daemon/mod.rs`,
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_handshake_roundtrip_succeeds`
(spawn sequence).

**Status:** done

### §8.3a Frame roundtrip span

**Spec:** §8.3 — with `OTEL_EXPORTER_OTLP_ENDPOINT` set, one span is emitted
per frame roundtrip.

**Repo:** —

**Tests:** —

**Status:** open — the emit half of §8.3.

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain (max 30 s), `op:shutdown` frame, close
1001, cleanup; SIGHUP hot-reload TLS+ACL.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (signal + drain),
`crates/websocket-bridge/src/daemon/runtime_common.rs`
(SIGTERM/SIGINT/SIGHUP via the cluster-A signal handler);
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_the_binary_exits_0_on_sigterm`
(the binary was still running 3 s after SIGTERM before, measured),
`crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_9_2_graceful_shutdown_completes_within_5s`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_7_1_sighup_cert_reload_keeps_daemon_running_with_new_cert`,
`crates/websocket-bridge/tests/security_e2e.rs::wsbridge_9_2_sighup_applies_the_reloaded_acl` (ACL and auth, 403 before the reload, 101 after);
`crates/websocket-bridge/src/daemon/router.rs::tests::wsbridge_9_2_shutdown_broadcasts_to_all`;
the close codes themselves `crates/websocket-bridge/src/close.rs::tests` (25).

**Status:** done

## §10 Cross-vendor

### §10 RTPS-peer co-existence Cyclone/RTI/Fast-DDS

**Spec:** §10 — the daemon is a normal RTPS peer; verified in
`crates/websocket-bridge/tests/cross_vendor.rs`.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (DCPS wire-up).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`
(DDS round-trip via DomainParticipant),
`crates/websocket-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
RTPS peer Cyclone/RTI/Fast-DDS).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-ws-bridged`; config defaults per OS;
systemd/launchd/Windows service; Docker `zerodds/ws-bridged:1.0`; manuals
1+5.

**Repo:** `packaging/linux/systemd/zerodds-ws-bridged.service`,
`packaging/macos/launchd/org.zerodds.ws-bridged.plist`,
`packaging/macos/homebrew/zerodds-ws-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ws-bridged/`,
`packaging/linux/configs/ws-bridged.yaml.example`,
`man/man1/zerodds-ws-bridged.1`, `man/man5/zerodds-ws-bridged.yaml.5`.

**Tests:** `crates/websocket-bridge/tests/example_yaml_loadback.rs::{wsbridge_3_example_yaml_parses_without_error, wsbridge_3_example_yaml_key_fields_match_intent, wsbridge_7_3_example_yaml_topic_acl_reaches_the_daemon, wsbridge_11_example_yaml_has_no_unknown_top_level_keys}`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys).

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/topic_map/auth/frame_codec/dds_pump, ≥ 5 tests
each.

**Repo:** `crates/websocket-bridge/src/{daemon/config.rs,codec.rs,frame.rs,handshake.rs,masking.rs,negotiation.rs,permessage_deflate.rs,uri.rs,utf8.rs,daemon/json_frames.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` per module:
`crates/websocket-bridge/src/codec.rs::tests` (21),
`crates/websocket-bridge/src/frame.rs::tests` (9),
`crates/websocket-bridge/src/message.rs::tests` (19),
`crates/websocket-bridge/src/masking.rs::tests` (10),
`crates/websocket-bridge/src/handshake.rs::tests` (13),
`crates/websocket-bridge/src/negotiation.rs::tests` (12),
`crates/websocket-bridge/src/permessage_deflate.rs::tests` (11),
`crates/websocket-bridge/src/close.rs::tests` (25),
`crates/websocket-bridge/src/utf8.rs::tests` (16),
`crates/websocket-bridge/src/uri.rs::tests` (14),
`crates/websocket-bridge/src/daemon/json_frames.rs::tests` (6),
`crates/websocket-bridge/src/daemon/config.rs::tests::wsbridge_12_1_parse_minimal_config` (9),
`crates/websocket-bridge/src/daemon/qos_translation.rs::tests::wsbridge_12_1_default_for_topic_uses_writer_defaults` (8);
plus `crates/websocket-bridge/tests/fuzz_smoke.rs::wsbridge_12_1_fuzz_websocket_decode_no_panic`,
`::wsbridge_12_1_empty_input_no_panic`, `::wsbridge_12_1_single_byte_inputs_no_panic`,
`::wsbridge_12_1_extended_length_overflow_no_oom`.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, WS client connect, DDS sub receives;
byte-exact round-trip.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_handshake_roundtrip_succeeds`,
`::wsbridge_2_rejects_non_upgrade_request`,
`::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone subscriber in docker-compose, the ZeroDDS bridge
publishes.

**Repo:** `crates/websocket-bridge/tests/cross_vendor.rs` (cluster-C
cross-vendor harness).

**Tests:** `crates/websocket-bridge/tests/cross_vendor.rs::wsbridge_12_3_rfc6455_handshake_against_zerodds_daemon_returns_101`;
the DDS side `crates/websocket-bridge/tests/daemon_e2e.rs::wsbridge_2_cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

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

19 done / 2 partial / 3 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-ws-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-bridge-security --all-targets` (job `merge-rust-bridge-security`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-websocket-bridge --all-targets` (job `merge-rust-websocket-bridge`) — 52 tests green.
* `python3 ci/run_integration.py integration-bridges` (job `integration-bridges`) — 2 tests green.

59 cited tests green, 0 not green.

Open items: `zerodds-ws-bridge-1.0.open.en.md`.
