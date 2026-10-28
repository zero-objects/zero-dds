# `zerodds-ws-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-ws-bridge-1.0.md`

Implementation:

- `crates/websocket-bridge/` — DDS↔WebSocket-Bridge.

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1, `zerodds-ws-bridge-1.0.md` — sechs Conformance-Levels
(Wire/DDS/Bridging/Config/Auth/Multi-Tenant); L1–L4 Pflicht, L5–L6
optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-ws-bridged CLI

**Spec:** §2 — Optionen `--config`/`--listen`/`--domain`/`--topic`/
`--tls-cert`/`--tls-key`/`--auth-token`/`--log-level`/`--metrics`/
`--version`/`--help`; Exit-Codes 0/1/2/3/4.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`,
`crates/websocket-bridge/src/daemon/cli.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`,
`::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

## §3 Config-File-Format

### §3 YAML/JSON/TOML Config-Loader

**Spec:** §3 — Top-Level `listen`/`domain`/`log_level`/`tls`/`auth`/
`topics`/`metrics`; ENV-Substitution `${VAR}` und `${VAR:-default}`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs`,
`crates/websocket-bridge/src/daemon/mod.rs` (DaemonConfig).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::make_test_config`
(Config-Construction). Loader-Round-Trip-Coverage in
`crates/websocket-bridge/src/daemon/config.rs::tests`.

**Status:** done

## §4 WebSocket-Wire-Protocol

### §4.1 RFC 6455 Handshake + zerodds-ws-bridge/1.0 Subprotocol

**Spec:** §4.1 — RFC-6455-Upgrade; Subprotocol-Header
`Sec-WebSocket-Protocol: zerodds-ws-bridge/1.0`; Auth-Header per
Bearer-Mode; 401/403 bei Auth-Failure.

**Repo:** `crates/websocket-bridge/src/handshake.rs`,
`crates/websocket-bridge/src/negotiation.rs`,
`crates/websocket-bridge/src/daemon/server.rs` (Upgrade-Path).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`.

**Status:** done

### §4.2 Pfad-Routing /topics/<slug> + Meta-Endpoints

**Spec:** §4.2 — `/topics/<slug>` Default-Pfad; `ws_path` Override;
`/topics/__catalog__`/`/healthz`/`/metrics` Meta-Endpoints.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/daemon/server.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(Topic-Pfad-Routing).

**Status:** done

### §4.3 Frame-Format Binary "ZDB1" + JSON-Mode

**Spec:** §4.3 — Binary-Magic `0x5A 0x44 0x42 0x31`, Flags,
4-Byte-Encap-Header `[0x00,0x07,0x00,0x00]`, CDR-Payload; Text-Frame
JSON-Mode optional.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (frame
encode/decode), `crates/websocket-bridge/src/codec.rs` (RFC-6455
codec).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`,
`crates/websocket-bridge/tests/fuzz_smoke.rs`.

**Status:** done

### §4.4 Control-Messages subscribe/unsubscribe/ping/pong/error

**Spec:** §4.4 — JSON-Control-Messages bidir + RFC-6455 PING/PONG.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs`,
`crates/websocket-bridge/src/dds_bridge.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus Topic-Name → URL

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `ws_path`.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (slug helper),
`crates/websocket-bridge/src/daemon/router.rs`.

**Tests:** `crates/websocket-bridge/src/daemon/config.rs::tests::slug_strips_double_colon`,
`::slug_replaces_unsafe_chars`;
`crates/websocket-bridge/src/uri.rs::tests` (14 Tests) prüft die URL-Form;
das Pfad-Routing `crates/websocket-bridge/src/daemon/router.rs::tests::dispatch_to_subscribed_connection`
und `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

### §5.2 Type-Discovery Catalog-Frame + /schema/<slug>

**Spec:** §5.2 — Catalog-Frame + `/schema/<slug>` IDL-Endpoint.

**Repo:** `crates/websocket-bridge/src/daemon/router.rs` (catalog +
schema endpoints).

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::catalog_render_json_contains_topic_fields`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::admin_endpoint_serves_catalog_and_healthz_and_metrics`,
`crates/websocket-bridge/tests/daemon_e2e.rs::admin_endpoint_serves_catalog_metrics_healthz`. Ein
`/schema/<slug>`-Endpoint existiert im Baum nicht — siehe §5.2a.

**Status:** partial — der Catalog-Frame ist geprüft; der IDL-Endpoint wird als §5.2a
`open` geführt.
### §5.2a `/schema/<slug>`-IDL-Endpoint

**Spec:** §5.2 — ein HTTP-Endpoint `/schema/<slug>` liefert die IDL des
Topics.

**Repo:** —

**Tests:** —

**Status:** open — der Teil von §5.2 jenseits des Catalog-Frames. Im Baum
gibt es keinen `/schema`-Pfad.

---

## §6 QoS-Translation

### §6 DDS-QoS → WS-Behavior Map

**Spec:** §6 — Reliability/Durability/History/Deadline/Liveliness/
Partition-Mapping.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (QoS-Pump);
`crates/websocket-bridge/src/daemon/qos_translation.rs` (Reliability/
Durability/History/Deadline/Liveliness/Partition-Map);
`crates/websocket-bridge/src/daemon/config.rs` (QoS-Felder).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(reliable Roundtrip); Cluster-A QoS-Matrix in
`crates/websocket-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 TLS wss:// + Cert-Rotation

**Spec:** §7.1 — `wss://`-Mode per `tls.enabled`, SIGHUP-Cert-Rotation.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (TLS-Hook),
`crates/websocket-bridge/src/daemon/security.rs` (rustls-Wireup),
`crates/bridge-security/src/tls.rs`.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::tls_plain_handshake_succeeds_then_data_flows`,
`crates/websocket-bridge/tests/security_e2e.rs::sighup_cert_reload_keeps_daemon_running_with_new_cert`;
`crates/websocket-bridge/src/daemon/security.rs::tests::ctx_from_config_tls_enabled_without_paths_rejected`;
`crates/bridge-security/src/connection.rs::tests::rotating_config_reload_swaps_inner_arc`.

**Status:** done

### §7.2 Auth-Modes none/bearer/jwt/mtls

**Spec:** §7.2 — vier Auth-Modes über Header bzw. Cert.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs`,
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/auth.rs` (none/bearer/jwt/mtls);
`jwt` mit dem Block `auth.jwt` aus §3 (`public_key`, `audience`,
`required_claim`, dazu `issuer`) über `SecurityConfig::apply_jwt`. Vorher
ließ sich `jwt` wählen, aber kein Schlüssel angeben, und der Start endete als
Bind-Fehler (Exit 2, gemessen). Das mTLS-Subject ist
der Subject-DN des Zertifikats (RFC 4514); eine ACL-Regel darf stattdessen
den Fingerprint `mtls:<SHA-256>` nennen.

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::a_jwt_is_checked_and_its_subject_meets_the_acl`,
`crates/websocket-bridge/tests/security_e2e.rs::jwt_without_a_key_and_mtls_without_a_client_ca_are_config_errors`,
`crates/websocket-bridge/tests/security_e2e.rs::auth_missing_bearer_yields_401`, `crates/websocket-bridge/tests/security_e2e.rs::auth_invalid_bearer_yields_401`,
`crates/websocket-bridge/tests/security_e2e.rs::auth_valid_bearer_then_acl_allows_subscribed_topic`;
`crates/websocket-bridge/src/daemon/security.rs::tests::ws_extracts_bearer_from_headers_case_insensitive`,
`::ws_missing_authorization_header_returns_missing`,
`::ws_none_auth_yields_anonymous_without_header`,
`::ctx_from_config_with_bearer_tokens`.

**Status:** done

### §7.3 Per-Topic-ACL read/write

**Spec:** §7.3 — `acl.read`/`acl.write` Listen mit `*`/exact/group-Match.

**Repo:** `crates/websocket-bridge/src/daemon/config.rs` (ACL-Felder),
`crates/websocket-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs` (Match-Engine).

**Tests:** `crates/websocket-bridge/tests/security_e2e.rs::auth_valid_bearer_then_acl_allows_subscribed_topic`,
`crates/websocket-bridge/tests/security_e2e.rs::auth_valid_bearer_but_acl_denies_forbidden_topic`;
`crates/websocket-bridge/src/daemon/security.rs::tests::ws_acl_check_allows_listed_user`;
`crates/websocket-bridge/tests/example_yaml_loadback.rs::example_yaml_topic_acl_reaches_the_daemon`.

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log auf stdout, Felder timestamp/level/event/
connection_id/topic/bytes/peer/latency_us; `--log-level`-Switch.

**Repo:** —

**Tests:** —

**Status:** open — `crates/websocket-bridge/src/daemon/cli.rs::tests` prüft die
Flag-Zerlegung, aber kein Test prüft die JSON-Log-Form oder die Wirkung von
`--log-level`.
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + `metrics.*`-Config; 11 Counter/Gauge
Metric-Familien.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (metrics
listener), `crates/websocket-bridge/src/daemon/config.rs` (metrics
config block), `crates/websocket-bridge/src/daemon/runtime_common.rs`
(Counter/Gauge-Familien Cluster-A-Wireup).

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::metrics_register_is_idempotent`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::metrics_counter_visible_in_prometheus_render`,
`crates/websocket-bridge/src/daemon/runtime_common.rs::tests::admin_endpoint_serves_catalog_and_healthz_and_metrics`;
`crates/websocket-bridge/tests/daemon_e2e.rs::metrics_counters_track_frames_in_and_out`,
`crates/websocket-bridge/tests/daemon_e2e.rs::admin_endpoint_serves_catalog_metrics_healthz`.

**Status:** done

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT`-ENV aktiviert
Span-Emission per `zerodds-observability-otlp`.

**Repo:** `crates/websocket-bridge/src/daemon/runtime_common.rs`
(OTLP-Init via `zerodds-observability-otlp`),
`crates/websocket-bridge/src/daemon/server.rs` (Span-Emit pro Frame).

**Tests:** `crates/websocket-bridge/src/daemon/runtime_common.rs::tests::otlp_config_from_endpoint_parses_http_url`,
`::otlp_config_from_endpoint_parses_bare_host_port`,
`::otlp_config_from_endpoint_handles_https_and_trailing_slash`,
`::otlp_config_from_endpoint_falls_back_to_default_port`. Dass pro
Frame-Roundtrip ein Span entsteht, prüft kein Test — siehe §8.3a.

**Status:** partial — die Endpoint-Auflösung ist geprüft, der Span-Emit nicht.
## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config-Parse → TLS → DCPS → Reader/Writer-Register →
WS-Bind → Signal-Handler.

**Repo:** `crates/websocket-bridge/src/daemon/mod.rs`,
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`
(Spawn-Sequence).

**Status:** done

### §8.3a Frame-Roundtrip-Span

**Spec:** §8.3 — bei gesetztem `OTEL_EXPORTER_OTLP_ENDPOINT` entsteht pro
Frame-Roundtrip ein Span.

**Repo:** —

**Tests:** —

**Status:** open — der Emit-Teil von §8.3.

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain (max 30 s), `op:shutdown`-Frame, Close
1001, Cleanup; SIGHUP Hot-Reload TLS+ACL.

**Repo:** `crates/websocket-bridge/src/daemon/server.rs` (signal +
drain), `crates/websocket-bridge/src/daemon/runtime_common.rs` (SIGTERM/
SIGINT/SIGHUP via Cluster-A-Signal-Handler);
`crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::the_binary_exits_0_on_sigterm`
(das Binary lief vorher 3 s nach SIGTERM weiter, gemessen),
`crates/websocket-bridge/tests/daemon_e2e.rs::graceful_shutdown_completes_within_5s`,
`crates/websocket-bridge/tests/security_e2e.rs::sighup_cert_reload_keeps_daemon_running_with_new_cert`,
`crates/websocket-bridge/tests/security_e2e.rs::sighup_applies_the_reloaded_acl` (ACL und Auth, 403 vor dem Reload, 101 danach);
`crates/websocket-bridge/src/daemon/router.rs::tests::shutdown_broadcasts_to_all`;
die Close-Codes selbst `crates/websocket-bridge/src/close.rs::tests` (25).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer Co-Existence Cyclone/RTI/Fast-DDS

**Spec:** §10 — Daemon ist normaler RTPS-Peer; verifiziert in
`crates/websocket-bridge/tests/cross_vendor.rs`.

**Repo:** `crates/websocket-bridge/src/dds_bridge.rs` (DCPS-Wire-Up).

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber`
(DDS-Roundtrip via DomainParticipant),
`crates/websocket-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor RTPS-Peer Cyclone/RTI/Fast-DDS).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-ws-bridged`; Config-Defaults pro OS;
systemd/launchd/Win-Service; Docker `zerodds/ws-bridged:1.0`; Manuals
1+5.

**Repo:** `packaging/linux/systemd/zerodds-ws-bridged.service`,
`packaging/macos/launchd/org.zerodds.ws-bridged.plist`,
`packaging/macos/homebrew/zerodds-ws-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/ws-bridged/`,
`packaging/linux/configs/ws-bridged.yaml.example`,
`man/man1/zerodds-ws-bridged.1`,
`man/man5/zerodds-ws-bridged.yaml.5`.

**Tests:** `crates/websocket-bridge/tests/example_yaml_loadback.rs::{example_yaml_parses_without_error, example_yaml_key_fields_match_intent, example_yaml_topic_acl_reaches_the_daemon, example_yaml_has_no_unknown_top_level_keys}`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel).

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/topic_map/auth/frame_codec/dds_pump je ≥ 5
Tests.

**Repo:** `crates/websocket-bridge/src/{daemon/config.rs,codec.rs,frame.rs,handshake.rs,masking.rs,negotiation.rs,permessage_deflate.rs,uri.rs,utf8.rs,dds_bridge.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul:
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
`crates/websocket-bridge/src/dds_bridge.rs::tests` (12),
`crates/websocket-bridge/src/daemon/config.rs::tests::parse_minimal_config` (9),
`crates/websocket-bridge/src/daemon/qos_translation.rs::tests::default_for_topic_uses_writer_defaults` (8);
dazu `crates/websocket-bridge/tests/fuzz_smoke.rs::fuzz_websocket_decode_no_panic`,
`::empty_input_no_panic`, `::single_byte_inputs_no_panic`,
`::extended_length_overflow_no_oom`.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, WS-Client connect, DDS-Sub empfängt;
byte-genauer Roundtrip.

**Repo:** `crates/websocket-bridge/src/bin/zerodds-ws-bridged.rs`.

**Tests:** `crates/websocket-bridge/tests/daemon_e2e.rs::handshake_roundtrip_succeeds`,
`::rejects_non_upgrade_request`,
`::cross_daemon_publish_pump_delivers_to_subscriber`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-Subscriber im Docker-Compose, ZeroDDS-Bridge
published.

**Repo:** `crates/websocket-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/websocket-bridge/tests/cross_vendor.rs::rfc6455_handshake_against_zerodds_daemon_returns_101`;
die DDS-Seite `crates/websocket-bridge/tests/daemon_e2e.rs::cross_daemon_publish_pump_delivers_to_subscriber` und
`crates/websocket-bridge/src/dds_bridge.rs::tests` (12).

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + Daemons

**Spec:** §13 — Library `crates/websocket-bridge/`, Wire-Format-Spec
`zerodds-xcdr2-bindings-conformance-1.0` §3, Deployment-Spec
`zerodds-deployment-1.0`, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config, Major=Wire-
Protocol-Change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

19 done / 2 partial / 3 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-websocket-bridge` — Tests grün, 0 failed.

Offene Punkte: siehe `zerodds-ws-bridge-1.0.open.md`. Decision-Records: keine.
