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

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-grpc-bridged CLI

**Spec:** §2 — options
`--config`/`--bind`/`--domain`/`--tls-*`/`--reflection`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`::http2_unknown_service_yields_status_5`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`grpc`/`auth`/`topics`/`metrics`; ENV
substitution.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs` (config
parser), `crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::tests`: `config_default_has_default_bind`,
`parse_minimal_grpc_config`, `parse_topics_array`, `parse_invalid_yaml_returns_err`,
`example_yaml_parses_without_error`, `example_yaml_key_fields_match_intent`,
`example_yaml_topics_reach_the_daemon`, `example_yaml_has_no_unknown_top_level_keys`,
`unknown_top_level_key_does_not_abort_the_parse`;
`crates/config/tests/layering.rs::a_hash_inside_quotes_is_part_of_the_value`; ENV substitution is covered by
`crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::tests::env_substitution_applies_before_the_parse`.

**Status:** done

## §4 gRPC wire protocol

### §4.1 HTTP/2 setup with ALPN h2

**Spec:** §4.1 — an HTTP/2 server, ALPN `h2`, cleartext h2c only in dev
mode (`bind=127.0.0.1`); SETTINGS frame, GOAWAY on shutdown.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/frame.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.2 Service-definition auto-generation per topic

**Spec:** §4.2 — a `<TopicSlug>Stream` service with
`Publish`/`Subscribe`/`PublishOne`/`Catalog` RPCs;
`Sample`/`PublishAck`/`SubscribeReq`/`CatalogReq`/`CatalogResp`/`TopicEntry`
messages.

**Repo:** `crates/grpc-bridge/src/path.rs` (slug→service-name),
`crates/grpc-bridge/src/service_gen.rs` (cluster-C FileDescriptor
auto-generator), `crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/src/service_gen.rs::tests`: `pascal_simple`,
`pascal_dotted`, `pascal_mixed_separators`, `pascal_empty_falls_back`,
`topic_service_paths`, `matches_path_publish`, `render_proto_contains_methods`,
`catalog_registration_dedup`, `catalog_find_by_path`,
`catalog_fully_qualified_names`;
`crates/grpc-bridge/src/path.rs::tests::parses_standard_grpc_path`,
`crates/grpc-bridge/src/path.rs::tests::parses_dotted_service_name`;
`crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.3 RPC flows publish/subscribe

**Spec:** §4.3 — HEADERS/DATA/HEADERS-END_STREAM flow with `:method=POST`,
`:path=/<pkg>.<svc>/<method>`,
content-type/grpc-encoding/grpc-timeout/authorization; streaming per
sample.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/metadata.rs`, `crates/grpc-bridge/src/timeout.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.4 Length-prefix wrapper

**Spec:** §4.4 — a 5-byte header (compr 1B + length 4B BE) + protobuf
sample payload; compr 0=identity / 1=compressed (gzip/deflate via
`grpc-encoding`).

**Repo:** `crates/grpc-bridge/src/frame.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `crates/grpc-bridge/src/frame.rs`;
`crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.5 Status mapping OK/INVALID_ARGUMENT/PERMISSION_DENIED/...

**Spec:** §4.5 — gRPC status codes 0/3/4/7/8/13/14/16 → DDS conditions.

**Repo:** `crates/grpc-bridge/src/status.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `crates/grpc-bridge/src/status.rs`;
`crates/grpc-bridge/tests/bridge_e2e.rs::http2_unknown_service_yields_status_5`.

**Status:** done

### §4.6 Reflection service grpc.reflection.v1alpha

**Spec:** §4.6 — the standard reflection service with auto-generated topic
services + sample message types.

**Repo:** `crates/grpc-bridge/src/server.rs` (reflection hook),
`crates/grpc-bridge/src/reflection.rs` (cluster-C reflection RPC fully
wired).

**Tests:** `crates/grpc-bridge/src/reflection.rs::tests`:
`empty_catalog_yields_empty_list_services_response`, `one_service_in_list`,
`error_response_encodes_code_and_message`, `varint_basics`,
`reflection_path_constant_matches_grpcurl_default`;
`crates/grpc-bridge/tests/cross_vendor.rs::list_services_reflection_payload_is_well_formed` plus the two grpcurl
runs `crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_list_services_against_zerodds_daemon` and
`crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_lists_the_reflection_service_itself` (`#[ignore]`, gated on
`GRPC_GRPCURL=1`).

**Status:** done

## §5 Topic mapping

### §5.1 Service-name default Topic→ChatMessageStream

**Spec:** §5.1 — `::` split + CamelCase + suffix `Stream`; package
`zerodds.<segment>.v1`.

**Repo:** `crates/grpc-bridge/src/path.rs`.

**Tests:** `crates/grpc-bridge/src/service_gen.rs::tests::pascal_simple`,
`crates/grpc-bridge/src/service_gen.rs::tests::pascal_dotted`,
`crates/grpc-bridge/src/service_gen.rs::tests::pascal_mixed_separators`,
`crates/grpc-bridge/src/service_gen.rs::tests::pascal_empty_falls_back`,
`crates/grpc-bridge/src/service_gen.rs::tests::topic_service_paths`;
`crates/grpc-bridge/src/path.rs::tests` (7 tests) covers the path form
itself.

**Status:** done

### §5.2 Type discovery reflection + catalog + IDL endpoint

**Spec:** §5.2 — three paths (reflection service, catalog RPC, HTTP-GET
sidekick).

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/reflection.rs`,
`crates/grpc-bridge/src/service_gen.rs` (cluster-C type discovery via
reflection + catalog RPC + HTTP-GET sidekick).

**Tests:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::tests::build_catalog_emits_topics`,
`crates/grpc-bridge/src/daemon_runtime.rs::tests::catalog_render_json_is_valid_shape`;
`crates/grpc-bridge/src/service_gen.rs::tests::catalog_registration_dedup`,
`::catalog_find_by_path`, `::catalog_fully_qualified_names`;
`crates/grpc-bridge/src/reflection.rs::tests::one_service_in_list`.

**Status:** done

## §6 QoS translation

### §6 DDS-QoS → gRPC-behavior map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition
map; `partition_filter` as a subscribe field.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/timeout.rs`, `crates/grpc-bridge/src/metadata.rs`,
`crates/grpc-bridge/src/qos_translation.rs` (cluster-A QoS map
Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`
(reliable round-trip); a QoS matrix in
`crates/grpc-bridge/src/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS h2-ALPN + cert rotation

**Spec:** §7.1 — TLS 1.2+ (1.3 default), ALPN `h2`, SIGHUP cert rotation.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::serve_tls`,
`::security_from`, `::apply_reload`;
`crates/grpc-bridge/src/daemon_runtime.rs::build_grpc_tls_config_with`
(`grpc.tls.client_ca_file`, `grpc.tls.require_client_cert`).

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::mtls_serves_h2_and_the_certificate_is_the_subject`
(ALPN `h2` read at the client),
`::a_sighup_serves_the_next_connection_with_the_new_certificate`;
against grpc-go `crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_over_tls_with_a_bearer_token`
(grpcurl 1.9.3, Linux test host);
`crates/grpc-bridge/tests/security_e2e.rs::build_grpc_tls_config_sets_alpn_h2`,
`::build_grpc_tls_config_with_client_ca_for_mtls`, `::build_grpc_tls_config_rejects_missing_cert`.

**Status:** done

### §7.2 Auth modes none/jwt/mtls

**Spec:** §7.2 — JWT-bearer authorization metadata, mTLS cert DN; §3
`auth.jwt.public_key`, `audience`, `required_claim`.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::handle_connection`
(checked per call on the `authorization` metadata, `UNAUTHENTICATED`),
`::security_from`; `crates/grpc-bridge/src/server.rs` (`GrpcRequest::metadata`);
`crates/bridge-security/src/auth.rs` (RS256, `exp`/`nbf` with 60 s leeway,
`aud`, required claim; `jwt_public_key_from_pem` for PKCS#1 and SPKI).
Also `bearer` with `auth.bearer_token`/`auth.bearer_subject`. The mTLS
subject is the certificate's subject DN (RFC 4514, `CN=alice`); an ACL
rule may name the fingerprint `mtls:<SHA-256>` instead.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::bearer_and_acl_are_checked_per_call`,
`::jwt_checks_signature_time_audience_and_scope`,
`::mtls_serves_h2_and_the_certificate_is_the_subject`,
`::jwt_without_a_key_and_mtls_without_tls_do_not_start`;
`crates/bridge-security/src/auth.rs::tests::a_signed_jwt_is_checked_for_time_audience_and_claim`.

**Status:** done

### §7.3 Per-topic ACL

**Spec:** §7.3 — `acl.publish`/`acl.subscribe` lists with subject
resolution.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::dispatch`
(`Publish` = write, `Subscribe` = read on the DDS topic,
`PERMISSION_DENIED`); `topics[].acl` in `DaemonConfig::from_config`.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::bearer_and_acl_are_checked_per_call`,
`::jwt_checks_signature_time_audience_and_scope` (JWT `sub` as the subject),
`::mtls_serves_h2_and_the_certificate_is_the_subject` (certificate as the subject).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** —

**Tests:** —

**Status:** open — nothing tests the JSON log form or the `--log-level` switch.
### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + 10 counter/gauge families.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::SpecMetrics`
(all ten `zerodds_grpc_bridge_*` families). Label values come only from the
configuration or a fixed set: a service that is not mapped counts as
`service="unknown"`, a foreign method as `method="other"`.
`bytes_in`/`bytes_out` count the Length-Prefixed-Message including its
5-octet prefix; `h2_resets_total` counts received RST_STREAM frames by their
RFC 7540 §7 names (the daemon sends none). The older `zerodds_grpc_*` from
`crates/grpc-bridge/src/daemon_runtime.rs` stay alongside.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::the_metrics_of_section_8_2_count_calls_failures_and_resets`
(status per call, auth failures by reason, RST_STREAM(CANCEL), with
`dds-runtime` Publish → DDS → Subscribe);
`crates/grpc-bridge/src/daemon_runtime.rs::tests::admin_endpoint_serves_metrics_path`.

**Status:** done

### §8.3 OTLP spans + W3C traceparent

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` + trace propagation via a
traceparent header.

**Repo:** `crates/grpc-bridge/src/metadata.rs` (traceparent parsing hook),
`crates/grpc-bridge/src/daemon_runtime.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/grpc-bridge/src/server.rs` (span emit
per RPC).

**Tests:** `crates/grpc-bridge/src/daemon_runtime.rs::tests::otlp_config_parses_endpoint` — the endpoint from
`OTEL_EXPORTER_OTLP_ENDPOINT` is read. Nothing tests the traceparent
parsing — see §8.3a.

**Status:** partial — the OTLP configuration is covered, the W3C traceparent parsing
is not; the missing half is carried as §8.3a `open`.
## §9 Lifecycle

### §8.3a W3C traceparent parsing

**Spec:** §8.3 — an incoming `traceparent` header is parsed and the span is
attached to it.

**Repo:** —

**Tests:** —

**Status:** open — the traceparent half of §8.3. No test and no occurrence in
the tree names `traceparent`.

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → reader/writer + FileDescriptor →
HTTP/2 bind → signal handler.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`,
`crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, GOAWAY, RPCs end naturally; SIGHUP
TLS+ACL reload.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::handle_connection`
(every connection checks the stop flag every 200 ms, a call being served
finishes first, then GOAWAY with the last answered stream id and NO_ERROR;
the idle close after 5 s sends GOAWAY too), `::apply_reload` (TLS material,
auth, ACL; a file that does not load changes nothing).

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::sigterm_sends_goaway_with_the_last_stream_and_exits_0`,
`::a_sighup_applies_the_new_token_and_a_broken_file_keeps_it`,
`::a_sighup_serves_the_next_connection_with_the_new_certificate`.

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic

**Spec:** §10 — the daemon is a normal RTPS peer; the gRPC side against
grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic.

**Repo:** `crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_list_services_against_zerodds_daemon`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_lists_the_reflection_service_itself` (both `#[ignore]`, gated
on `GRPC_GRPCURL=1`, feature `cross-vendor-tests`),
`crates/grpc-bridge/tests/cross_vendor.rs::list_services_reflection_payload_is_well_formed`;
`crates/grpc-bridge/tests/dds_bridge_e2e.rs::dds_publish_then_subscribe_roundtrip`
for the RTPS side. grpc-go, grpc-java, ghz, BloomRPC and tonic have not been
run against the daemon — see §10a.

**Status:** partial — grpcurl is measured, the five other clients are not; the
missing part is carried as §10a `open`.
### §10a Client matrix grpc-go/grpc-java/ghz/BloomRPC/tonic

**Spec:** §10 — the daemon is driven by grpc-go, grpc-java, ghz, BloomRPC and
tonic.

**Repo:** —

**Tests:** —

**Status:** open — the part of §10 and §12.3 beyond grpcurl.
`crates/grpc-bridge/tests/cross_vendor.rs` knows only grpcurl.

---

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-grpc-bridged`; configs/services/Docker;
manuals.

**Repo:** `packaging/linux/systemd/zerodds-grpc-bridged.service`,
`packaging/macos/launchd/org.zerodds.grpc-bridged.plist`,
`packaging/macos/homebrew/zerodds-grpc-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/grpc-bridged/`,
`packaging/linux/configs/grpc-bridged.yaml.example`,
`man/man1/zerodds-grpc-bridged.1`, `man/man5/zerodds-grpc-bridged.yaml.5`.

**Tests:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::tests::{example_yaml_parses_without_error, example_yaml_key_fields_match_intent, example_yaml_topics_reach_the_daemon, example_yaml_has_no_unknown_top_level_keys}`,
`crates/config/tests/layering.rs::a_hash_inside_quotes_is_part_of_the_value`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys).

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/http2_codec/hpack/grpc_status/reflection/dds_pump/auth,
≥ 5 tests each.

**Repo:** `crates/grpc-bridge/src/{frame.rs,metadata.rs,path.rs,server.rs,status.rs,timeout.rs}`
plus `crates/http2/`, `crates/hpack/`.

**Tests:** Inline `#[cfg(test)] mod tests` per module:
`crates/grpc-bridge/src/frame.rs::tests::round_trip_message` (9),
`crates/grpc-bridge/src/metadata.rs::tests::decode_base64_round_trip` (18),
`crates/grpc-bridge/src/path.rs::tests::parses_standard_grpc_path` (7),
`crates/grpc-bridge/src/server.rs::tests::encode_response_includes_status_and_trailers` (4),
`crates/grpc-bridge/src/status.rs::tests::all_17_codes_round_trip_via_numeric` (5),
`crates/grpc-bridge/src/timeout.rs::tests::timeout_unit_round_trip_for_all` (11),
`crates/grpc-bridge/src/qos_translation.rs::tests::default_for_topic_reliable_only` (7);
plus `crates/grpc-bridge/tests/fuzz_smoke.rs` (`fuzz_decode_message_no_panic`,
`fuzz_parse_path_no_panic`, `fuzz_decode_timeout_no_panic`,
`empty_inputs_no_panic`, `message_oversized_length_no_oom`).

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, grpcurl as the client, byte-exact
round-trip + reflection.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`::http2_unknown_service_yields_status_5`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a tonic client + a Cyclone-DDS subscriber + the ZeroDDS
gRPC bridge in compose.

**Repo:** `crates/grpc-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
harness).

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_list_services_against_zerodds_daemon`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_lists_the_reflection_service_itself`,
`crates/grpc-bridge/tests/cross_vendor.rs::list_services_reflection_payload_is_well_formed`;
`crates/grpc-bridge/tests/dds_bridge_e2e.rs::dds_publish_then_subscribe_roundtrip`.
No tonic client appears in it — see §10a.

**Status:** partial — grpcurl and the DDS side are covered, a tonic client is
not.
## §13 Cross-references

### §13 Related library + standards + daemons

**Spec:** §13 — library `crates/grpc-bridge/`/`crates/http2/`/`crates/hpack/`,
gRPC-PROTOCOL.md, RFC 7540/7541, wire format, deployment, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config/.proto, major=wire-protocol
change (HTTP/3 + gRPC-over-QUIC).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

20 done / 3 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-grpc-bridge` — tests green, 0 failed.

Open items: see `zerodds-grpc-bridge-1.0.open.en.md`. Decision records: none.
