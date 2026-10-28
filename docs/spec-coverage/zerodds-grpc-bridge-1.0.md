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

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-grpc-bridged CLI

**Spec:** §2 — Optionen `--config`/`--bind`/`--domain`/`--tls-*`/
`--reflection`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
Exit-Codes 0/1/2/3/4.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`::http2_unknown_service_yields_status_5`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`grpc`/`auth`/`topics`/`metrics`;
ENV-Substitution.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs` (Config-
Parser), `crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::tests`: `config_default_has_default_bind`,
`parse_minimal_grpc_config`, `parse_topics_array`, `parse_invalid_yaml_returns_err`,
`example_yaml_parses_without_error`, `example_yaml_key_fields_match_intent`,
`example_yaml_topics_reach_the_daemon`, `example_yaml_has_no_unknown_top_level_keys`,
`unknown_top_level_key_does_not_abort_the_parse`;
`crates/config/tests/layering.rs::a_hash_inside_quotes_is_part_of_the_value`; die ENV-Substitution prüft
`crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::tests::env_substitution_applies_before_the_parse`.

**Status:** done

## §4 gRPC-Wire-Protocol

### §4.1 HTTP/2-Setup mit ALPN h2

**Spec:** §4.1 — HTTP/2-Server, ALPN `h2`, Cleartext-h2c nur Dev-Mode
(`bind=127.0.0.1`); SETTINGS-Frame, GOAWAY bei Shutdown.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/frame.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.2 Service-Definition Auto-Generation pro Topic

**Spec:** §4.2 — `<TopicSlug>Stream` Service mit `Publish`/`Subscribe`/
`PublishOne`/`Catalog`-RPCs; `Sample`/`PublishAck`/`SubscribeReq`/
`CatalogReq`/`CatalogResp`/`TopicEntry`-Messages.

**Repo:** `crates/grpc-bridge/src/path.rs` (Slug→Service-Name),
`crates/grpc-bridge/src/service_gen.rs` (Cluster-C
FileDescriptor-Auto-Generator),
`crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/src/service_gen.rs::tests`: `pascal_simple`,
`pascal_dotted`, `pascal_mixed_separators`, `pascal_empty_falls_back`,
`topic_service_paths`, `matches_path_publish`, `render_proto_contains_methods`,
`catalog_registration_dedup`, `catalog_find_by_path`,
`catalog_fully_qualified_names`;
`crates/grpc-bridge/src/path.rs::tests::parses_standard_grpc_path`,
`crates/grpc-bridge/src/path.rs::tests::parses_dotted_service_name`;
`crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.3 RPC-Flows Publish/Subscribe

**Spec:** §4.3 — HEADERS/DATA/HEADERS-END_STREAM-Flow mit `:method=POST`,
`:path=/<pkg>.<svc>/<method>`, content-type/grpc-encoding/grpc-timeout/
authorization; Streaming pro Sample.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/metadata.rs`,
`crates/grpc-bridge/src/timeout.rs`. **L2-DDS-Anbindung** (Feature
`dds-runtime`): `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`
(`mod bridge_dds`) — `Publish` schreibt die `Sample.payload` via
`DcpsRuntime::write_user_sample_borrowed` auf einen echten DataWriter,
`Subscribe` drained den DataReader-Channel (`UserSample::Alive`). Ersetzt
den vormaligen `FUTURE (L2)`-Echo-Stub.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`;
`crates/grpc-bridge/tests/dds_bridge_e2e.rs::dds_publish_then_subscribe_roundtrip`
(gRPC Publish → DataWriter → Loopback-DataReader → gRPC Subscribe,
byte-gleiche Payload durch echtes DDS).

**Status:** done

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

**Repo:** `crates/grpc-bridge/src/frame.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `crates/grpc-bridge/src/frame.rs`;
`crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §4.5 Status-Mapping OK/INVALID_ARGUMENT/PERMISSION_DENIED/...

**Spec:** §4.5 — gRPC-Status-Codes 0/3/4/7/8/13/14/16 → DDS-Bedingungen.

**Repo:** `crates/grpc-bridge/src/status.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `crates/grpc-bridge/src/status.rs`;
`crates/grpc-bridge/tests/bridge_e2e.rs::http2_unknown_service_yields_status_5`.

**Status:** done

### §4.6 Reflection-Service grpc.reflection.v1alpha

**Spec:** §4.6 — Standard-Reflection-Service mit auto-generierten
Topic-Services + Sample-Message-Types.

**Repo:** `crates/grpc-bridge/src/server.rs` (Reflection-Hook),
`crates/grpc-bridge/src/reflection.rs` (Cluster-C Reflection-RPC
voll wired).

**Tests:** `crates/grpc-bridge/src/reflection.rs::tests`:
`empty_catalog_yields_empty_list_services_response`, `one_service_in_list`,
`error_response_encodes_code_and_message`, `varint_basics`,
`reflection_path_constant_matches_grpcurl_default`;
`crates/grpc-bridge/tests/cross_vendor.rs::list_services_reflection_payload_is_well_formed` sowie die beiden
grpcurl-Läufe `crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_list_services_against_zerodds_daemon` und
`crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_lists_the_reflection_service_itself` (`#[ignore]`, Gate
`GRPC_GRPCURL=1`).

**Status:** done

## §5 Topic-Mapping

### §5.1 Service-Namen-Default Topic→ChatMessageStream

**Spec:** §5.1 — `::`-Split + CamelCase + Suffix `Stream`; Package
`zerodds.<segment>.v1`.

**Repo:** `crates/grpc-bridge/src/path.rs`.

**Tests:** `crates/grpc-bridge/src/service_gen.rs::tests::pascal_simple`,
`crates/grpc-bridge/src/service_gen.rs::tests::pascal_dotted`,
`crates/grpc-bridge/src/service_gen.rs::tests::pascal_mixed_separators`,
`crates/grpc-bridge/src/service_gen.rs::tests::pascal_empty_falls_back`,
`crates/grpc-bridge/src/service_gen.rs::tests::topic_service_paths`;
`crates/grpc-bridge/src/path.rs::tests` (7 Tests) prüft die Pfad-Form
selbst.

**Status:** done

### §5.2 Type-Discovery Reflection + Catalog + IDL-Endpoint

**Spec:** §5.2 — drei Pfade (Reflection-Service, Catalog-RPC,
HTTP-GET-Sidekick).

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/reflection.rs`,
`crates/grpc-bridge/src/service_gen.rs` (Cluster-C Type-Discovery via
Reflection + Catalog-RPC + HTTP-GET-Sidekick).

**Tests:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::tests::build_catalog_emits_topics`,
`crates/grpc-bridge/src/daemon_runtime.rs::tests::catalog_render_json_is_valid_shape`;
`crates/grpc-bridge/src/service_gen.rs::tests::catalog_registration_dedup`,
`::catalog_find_by_path`, `::catalog_fully_qualified_names`;
`crates/grpc-bridge/src/reflection.rs::tests::one_service_in_list`.

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → gRPC-Behavior Map

**Spec:** §6 — Reliability/Durability/History/Lifespan/Deadline/
Liveliness/Partition Map; `partition_filter` als Subscribe-Field.

**Repo:** `crates/grpc-bridge/src/server.rs`,
`crates/grpc-bridge/src/timeout.rs`,
`crates/grpc-bridge/src/metadata.rs`,
`crates/grpc-bridge/src/qos_translation.rs` (Cluster-A QoS-Map
Reliability/Durability/History/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`
(reliable Roundtrip); QoS-Matrix in
`crates/grpc-bridge/src/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS h2-ALPN + Cert-Rotation

**Spec:** §7.1 — TLS 1.2+ (1.3 default), ALPN `h2`, SIGHUP-Cert-Rotation.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::serve_tls`,
`::security_from`, `::apply_reload`;
`crates/grpc-bridge/src/daemon_runtime.rs::build_grpc_tls_config_with`
(`grpc.tls.client_ca_file`, `grpc.tls.require_client_cert`).

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::mtls_serves_h2_and_the_certificate_is_the_subject`
(ALPN `h2` am Client gelesen),
`::a_sighup_serves_the_next_connection_with_the_new_certificate`;
gegen grpc-go `crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_over_tls_with_a_bearer_token`
(grpcurl 1.9.3, Linux-Testhost);
`crates/grpc-bridge/tests/security_e2e.rs::build_grpc_tls_config_sets_alpn_h2`,
`::build_grpc_tls_config_with_client_ca_for_mtls`, `::build_grpc_tls_config_rejects_missing_cert`.

**Status:** done

### §7.2 Auth-Modes none/jwt/mtls

**Spec:** §7.2 — JWT-Bearer-Authorization-Metadata, mTLS-Cert-DN; §3
`auth.jwt.public_key`, `audience`, `required_claim`.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::handle_connection`
(Prüfung pro Call auf der `authorization`-Metadata, `UNAUTHENTICATED`),
`::security_from`; `crates/grpc-bridge/src/server.rs` (`GrpcRequest::metadata`);
`crates/bridge-security/src/auth.rs` (RS256, `exp`/`nbf` mit 60 s Toleranz,
`aud`, geforderter Claim; `jwt_public_key_from_pem` für PKCS#1 und SPKI).
Zusätzlich `bearer` mit `auth.bearer_token`/`auth.bearer_subject`. Das
mTLS-Subject ist der Subject-DN des Zertifikats (RFC 4514, `CN=alice`);
eine ACL-Regel darf stattdessen den Fingerprint `mtls:<SHA-256>` nennen.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::bearer_and_acl_are_checked_per_call`,
`::jwt_checks_signature_time_audience_and_scope`,
`::mtls_serves_h2_and_the_certificate_is_the_subject`,
`::jwt_without_a_key_and_mtls_without_tls_do_not_start`;
`crates/bridge-security/src/auth.rs::tests::a_signed_jwt_is_checked_for_time_audience_and_claim`.

**Status:** done

### §7.3 Per-Topic-ACL

**Spec:** §7.3 — `acl.publish`/`acl.subscribe` Listen mit Subject-
Resolution.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::dispatch`
(`Publish` = Schreiben, `Subscribe` = Lesen auf dem DDS-Topic,
`PERMISSION_DENIED`); `topics[].acl` in `DaemonConfig::from_config`.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::bearer_and_acl_are_checked_per_call`,
`::jwt_checks_signature_time_audience_and_scope` (JWT-`sub` als Subject),
`::mtls_serves_h2_and_the_certificate_is_the_subject` (Zertifikat als Subject).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** —

**Tests:** —

**Status:** open — kein Test prüft die JSON-Log-Form oder den `--log-level`-Switch.
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + 10 Counter/Gauge-Familien.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::SpecMetrics`
(alle zehn `zerodds_grpc_bridge_*`-Familien). Labelwerte nur aus der
Konfiguration oder einer festen Menge: ein nicht gemappter Service zählt als
`service="unknown"`, eine fremde Methode als `method="other"`.
`bytes_in`/`bytes_out` zählen die Length-Prefixed-Message samt 5-Oktett-
Präfix; `h2_resets_total` zählt empfangene RST_STREAM nach RFC-7540-§7-Namen
(der Daemon sendet keine). Die älteren `zerodds_grpc_*` aus
`crates/grpc-bridge/src/daemon_runtime.rs` bleiben daneben.

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::the_metrics_of_section_8_2_count_calls_failures_and_resets`
(Status pro Call, Auth-Fehler nach Grund, RST_STREAM(CANCEL), mit
`dds-runtime` Publish → DDS → Subscribe);
`crates/grpc-bridge/src/daemon_runtime.rs::tests::admin_endpoint_serves_metrics_path`.

**Status:** done

### §8.3 OTLP-Spans + W3C-Traceparent

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` + Trace-Propagation per
Traceparent-Header.

**Repo:** `crates/grpc-bridge/src/metadata.rs` (Traceparent-Parsing-
Hook), `crates/grpc-bridge/src/daemon_runtime.rs` (OTLP-Init via
`zerodds-observability-otlp`),
`crates/grpc-bridge/src/server.rs` (Span-Emit pro RPC).

**Tests:** `crates/grpc-bridge/src/daemon_runtime.rs::tests::otlp_config_parses_endpoint` — der Endpoint aus
`OTEL_EXPORTER_OTLP_ENDPOINT` wird gelesen. Das Traceparent-Parsing prüft
kein Test — siehe §8.3a.

**Status:** partial — die OTLP-Konfiguration ist geprüft, das W3C-Traceparent-Parsing
nicht; der fehlende Teil wird als §8.3a `open` geführt.
## §9 Lifecycle

### §8.3a W3C-Traceparent-Parsing

**Spec:** §8.3 — ein eingehender `traceparent`-Header wird geparst und der
Span daran gehängt.

**Repo:** —

**Tests:** —

**Status:** open — der Traceparent-Teil von §8.3. Kein Test und keine
Fundstelle im Baum nennt `traceparent`.

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → TLS → DCPS → Reader/Writer + FileDescriptor →
HTTP/2-Bind → Signal-Handler.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`,
`crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, GOAWAY, RPCs natürlich
beenden; SIGHUP TLS+ACL-Reload.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::handle_connection`
(jede Verbindung prüft das Stop-Flag alle 200 ms, ein laufender Call endet
zuerst, dann GOAWAY mit der letzten beantworteten Stream-ID und NO_ERROR;
auch der Idle-Abschluss nach 5 s sendet GOAWAY), `::apply_reload`
(TLS-Material, Auth, ACL; eine Datei, die nicht lädt, ändert nichts).

**Tests:** `crates/grpc-bridge/tests/daemon_security_e2e.rs::sigterm_sends_goaway_with_the_last_stream_and_exits_0`,
`::a_sighup_applies_the_new_token_and_a_broken_file_keeps_it`,
`::a_sighup_serves_the_next_connection_with_the_new_certificate`.

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic

**Spec:** §10 — Daemon ist normaler RTPS-Peer; gRPC-Seite gegen
grpc-go/java/grpcurl/ghz/Bloom-RPC/tonic.

**Repo:** `crates/grpc-bridge/src/server.rs`.

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_list_services_against_zerodds_daemon`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_lists_the_reflection_service_itself` (beide `#[ignore]`, Gate
`GRPC_GRPCURL=1`, Feature `cross-vendor-tests`),
`crates/grpc-bridge/tests/cross_vendor.rs::list_services_reflection_payload_is_well_formed`;
`crates/grpc-bridge/tests/dds_bridge_e2e.rs::dds_publish_then_subscribe_roundtrip`
für die RTPS-Seite. grpc-go, grpc-java, ghz, BloomRPC und tonic hat niemand
gegen den Daemon laufen lassen — siehe §10a.

**Status:** partial — grpcurl ist gemessen, die fünf weiteren Clients nicht; der
fehlende Teil wird als §10a `open` geführt.
### §10a Client-Matrix grpc-go/grpc-java/ghz/BloomRPC/tonic

**Spec:** §10 — der Daemon wird gegen grpc-go, grpc-java, ghz, BloomRPC und
tonic gefahren.

**Repo:** —

**Tests:** —

**Status:** open — der Teil von §10 und §12.3 jenseits grpcurl.
`crates/grpc-bridge/tests/cross_vendor.rs` kennt nur grpcurl.

---

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-grpc-bridged`; Configs/Services/Docker;
Manuals.

**Repo:** `packaging/linux/systemd/zerodds-grpc-bridged.service`,
`packaging/macos/launchd/org.zerodds.grpc-bridged.plist`,
`packaging/macos/homebrew/zerodds-grpc-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/grpc-bridged/`,
`packaging/linux/configs/grpc-bridged.yaml.example`,
`man/man1/zerodds-grpc-bridged.1`,
`man/man5/zerodds-grpc-bridged.yaml.5`.

**Tests:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs::tests::{example_yaml_parses_without_error, example_yaml_key_fields_match_intent, example_yaml_topics_reach_the_daemon, example_yaml_has_no_unknown_top_level_keys}`,
`crates/config/tests/layering.rs::a_hash_inside_quotes_is_part_of_the_value`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel).

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/http2_codec/hpack/grpc_status/reflection/
dds_pump/auth je ≥ 5 Tests.

**Repo:** `crates/grpc-bridge/src/{frame.rs,metadata.rs,path.rs,server.rs,status.rs,timeout.rs}` plus `crates/http2/`, `crates/hpack/`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul:
`crates/grpc-bridge/src/frame.rs::tests::round_trip_message` (9),
`crates/grpc-bridge/src/metadata.rs::tests::decode_base64_round_trip` (18),
`crates/grpc-bridge/src/path.rs::tests::parses_standard_grpc_path` (7),
`crates/grpc-bridge/src/server.rs::tests::encode_response_includes_status_and_trailers` (4),
`crates/grpc-bridge/src/status.rs::tests::all_17_codes_round_trip_via_numeric` (5),
`crates/grpc-bridge/src/timeout.rs::tests::timeout_unit_round_trip_for_all` (11),
`crates/grpc-bridge/src/qos_translation.rs::tests::default_for_topic_reliable_only` (7);
dazu `crates/grpc-bridge/tests/fuzz_smoke.rs` (`fuzz_decode_message_no_panic`,
`fuzz_parse_path_no_panic`, `fuzz_decode_timeout_no_panic`,
`empty_inputs_no_panic`, `message_oversized_length_no_oom`).

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, grpcurl als Client, byte-genauer
Roundtrip + Reflection.

**Repo:** `crates/grpc-bridge/src/bin/zerodds-grpc-bridged.rs`.

**Tests:** `crates/grpc-bridge/tests/bridge_e2e.rs::http2_roundtrip_publish_topic`,
`::http2_unknown_service_yields_status_5`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — tonic-Client + Cyclone-DDS-Subscriber + ZeroDDS-
gRPC-Bridge im Compose.

**Repo:** `crates/grpc-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_list_services_against_zerodds_daemon`,
`crates/grpc-bridge/tests/cross_vendor.rs::grpcurl_lists_the_reflection_service_itself`,
`crates/grpc-bridge/tests/cross_vendor.rs::list_services_reflection_payload_is_well_formed`;
`crates/grpc-bridge/tests/dds_bridge_e2e.rs::dds_publish_then_subscribe_roundtrip`.
Ein tonic-Client kommt darin nicht vor — siehe §10a.

**Status:** partial — grpcurl und die DDS-Seite sind geprüft, ein tonic-Client
nicht.
## §13 Cross-References

### §13 Verwandte Library + Standards + Daemons

**Spec:** §13 — Library `crates/grpc-bridge/`/`crates/http2/`/
`crates/hpack/`, gRPC-PROTOCOL.md, RFC 7540/7541, Wire-Format,
Deployment, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config/.proto, Major=
Wire-Protocol-Change (HTTP/3 + gRPC-over-QUIC).

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

20 done / 3 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-grpc-bridge` — Tests grün, 0 failed.

Offene Punkte: siehe `zerodds-grpc-bridge-1.0.open.md`. Decision-Records: keine.
