# `zerodds-coap-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-coap-bridge-1.0.md`

Implementation:

- `crates/coap-bridge/` — DDS↔CoAP-Bridge.

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-coap-bridged CLI

**Spec:** §2 — Optionen `--config`/`--bind`/`--domain`/`--dtls-*`/
`--topic`/`--log-level`/`--metrics`/`--version`/`--help`; Exit-Codes
0/1/2/3/4.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/coap-bridge/src/daemon/cli.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::well_known_core_returns_link_format_catalog`,
`::observe_register_returns_initial_content_with_observe_option`,
`::unknown_path_returns_bad_request`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`coap`/`oscore`/`topics`/
`content_format`/`acl`/`metrics`; ENV-Substitution.

**Repo:** `crates/coap-bridge/src/daemon/config.rs`,
`crates/config/src/parse.rs` (the shared parser),
`crates/coap-bridge/src/daemon/mod.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::make_test_config`
(Config-Construction). Inline-Tests in `crates/coap-bridge/src/daemon/config.rs::tests`.

**Status:** done

## §4 CoAP-Wire-Protocol

### §4.1 RFC-7252 Header + Token + Options

**Spec:** §4.1 — 4-Byte-Header (Ver/T/TKL/Code/Message-ID), Token,
Options, Payload-Marker `0xFF`.

**Repo:** `crates/coap-bridge/src/codec.rs`,
`crates/coap-bridge/src/message.rs`,
`crates/coap-bridge/src/option.rs`,
`crates/coap-bridge/src/method_props.rs`.

**Tests:** `crates/coap-bridge/src/codec.rs::tests` (19 Tests):
`encodes_minimum_get_request_to_4_byte_header`,
`encode_decode_round_trip_preserves_header_fields`,
`token_length_above_8_is_rejected_on_encode`, `header_too_short_decode_fails`,
`unsupported_version_decode_fails`,
`reserved_token_length_9_through_15_decode_fails`, `token_truncated_decode_fails`,
`single_short_option_encodes_in_one_byte_header`,
`option_delta_extended_13_uses_one_extra_byte`,
`option_delta_extended_14_uses_two_extra_bytes`,
`option_length_extended_13_uses_one_extra_byte`,
`delta_encoding_sums_across_multiple_options`,
`payload_marker_separates_options_from_payload`,
`payload_marker_without_payload_is_format_error`, `options_are_sorted_on_encode`,
`empty_message_round_trip`;
`crates/coap-bridge/src/message.rs::tests` (`message_type_round_trips_via_bits`,
`code_byte_round_trip`, `well_known_codes_match_spec_values`,
`code_display_uses_dotted_format`);
`crates/coap-bridge/src/option.rs::tests`
(`critical_unsafe_nocachekey_bits_per_spec_5_4_6`,
`repeatable_options_match_spec_table_4`, `uint_strips_leading_zero_bytes`);
`crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`.

**Status:** done

### §4.2 POST/PUT/DELETE → DDS-Write/Dispose

**Spec:** §4.2 — POST → DDS-Write (`2.04 Changed`), PUT idempotent,
DELETE → Dispose; `4.00`/`4.13`/`5.00` Error-Mapping.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/bridge.rs`,
`crates/coap-bridge/src/method_props.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::unknown_path_returns_bad_request`.

**Status:** done

### §4.3 GET + Observe (RFC 7641) → DDS→CoAP-Push

**Spec:** §4.3 — GET mit `Observe:0` registriert, Notify pro Sample
mit `Observe:<seq>`; Cancel via `Observe:1` oder RST.

**Repo:** `crates/coap-bridge/src/observe.rs`,
`crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::observe_register_returns_initial_content_with_observe_option`.

**Status:** done

### §4.4 Block-Wise-Transfer (RFC 7959)

**Spec:** §4.4 — Block1 (POST) + Block2 (Notify); `block_size` (SZX
16..1024); Defragmentation-Cap.

**Repo:** `crates/coap-bridge/src/blockwise.rs`,
`crates/coap-bridge/src/reliability.rs`,
`crates/coap-bridge/src/option.rs` (Block1/Block2-Options),
`crates/coap-bridge/src/daemon/server.rs` (Block-Wireup).

**Tests:** `crates/coap-bridge/src/blockwise.rs::tests` (10 Tests):
`block_size_matches_spec_szx_table`, `szx_7_rejected`,
`round_trip_small_block_value`, `round_trip_large_block_value`,
`num_too_large_rejected`, `decode_rejects_bad_length`,
`reassembler_combines_blocks_in_order`, `out_of_order_block_rejected`,
`intermediate_block_size_must_match`, `final_block_can_be_smaller`;
`crates/coap-bridge/tests/daemon_e2e.rs::block1_chunked_post_completes_with_2_04_changed`,
`crates/coap-bridge/tests/daemon_e2e.rs::block1_out_of_order_returns_2_31_continue_only_in_order`.

**Status:** done

### §4.5 Content-Format-Registry 65000/65001/65002/50/60

**Spec:** §4.5 — Vendor-Range 65000-65535 für CDR2-LE/BE + CDR1-LE;
`50=application/json`, `60=application/cbor`.

**Repo:** `crates/coap-bridge/src/option.rs` (Content-Format-Tags),
`crates/coap-bridge/src/bridge.rs` (CDR-Decoder).

**Tests:** `crates/coap-bridge/src/option.rs::tests::content_format_uses_number_12`,
`crates/coap-bridge/src/codec.rs::tests::representation_carries_content_format_per_spec_5_5`,
`crates/coap-bridge/src/codec.rs::tests::diagnostic_payload_of_error_response_carries_no_content_format`;
den Wert 65000 setzt `crates/coap-bridge/src/daemon/server.rs` und
`crates/coap-bridge/tests/daemon_e2e.rs::observe_register_returns_initial_content_with_observe_option` liest
ihn zurück. 65001, 65002, 50 und 60 gibt es als Registry nicht — siehe
§4.5a.

**Status:** partial — die Option selbst und der CDR2-LE-Wert 65000 sind geprüft; die
vier weiteren Werte werden als §4.5a `open` geführt.
## §5 Topic-Mapping

### §4.5a Content-Format-Werte 65001/65002/50/60

**Spec:** §4.5 — CDR2-BE (65001), CDR1-LE (65002), `50=application/json`,
`60=application/cbor`.

**Repo:** —

**Tests:** —

**Status:** open — der Teil von §4.5 jenseits 65000. Im Baum steht kein
Registry-Typ und kein Test für die vier Werte.

### §5.1 Slug-Algorithmus DDS → CoAP-URI

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `coap_uri_path`.

**Repo:** `crates/coap-bridge/src/uri.rs`,
`crates/coap-bridge/src/daemon/config.rs`.

**Tests:** `crates/coap-bridge/src/uri.rs::tests` (11 Tests):
`parses_basic_coap_uri`, `parses_coaps_default_port`, `parses_explicit_port`,
`parses_query_params`, `parses_empty_path_when_no_slash`,
`rejects_unknown_scheme`, `rejects_missing_host`, `rejects_invalid_port`,
`rejects_fragment`, `default_port_returns_5684_for_coaps`,
`round_trip_path_filtering_strips_empty_segments`;
den Override-Pfad `crates/coap-bridge/src/daemon/config.rs::tests` und
`crates/coap-bridge/tests/example_yaml_loadback.rs::example_yaml_topics_reach_the_daemon`.

**Status:** done

### §5.2 /.well-known/core (RFC 6690) Catalog

**Spec:** §5.2 — `/.well-known/core` liefert Link-Format-Resource-List
mit `rt="dds.topic"`, `ct=65000`, `type="..."`.

**Repo:** `crates/coap-bridge/src/core_link.rs`,
`crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::well_known_core_returns_link_format_catalog`.

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → CoAP-Behavior Map

**Spec:** §6 — Reliable→CON, BestEffort→NON, Volatile/TransientLocal,
Lifespan→`Max-Age`, Deadline→`5.03`, Liveliness→Ping, Partition→Filter.

**Repo:** `crates/coap-bridge/src/reliability.rs`,
`crates/coap-bridge/src/observe.rs`,
`crates/coap-bridge/src/bridge.rs`,
`crates/coap-bridge/src/daemon/qos_translation.rs` (Cluster-A QoS-Map
Reliability/Durability/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`
(reliable POST→Write); QoS-Matrix in
`crates/coap-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 DTLS coaps:// + Cipher-Suites

**Spec:** §7.1 — „`coaps://`-Mode aktiviert über `coap.dtls.enabled: true`";
die Cipher-Suites, die §7.1 empfiehlt, sind `TLS_PSK_WITH_AES_128_CCM_8`
(PSK-Mode, RFC 7252 §9.1.3), `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` (Cert-Mode)
und `TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` (Hybrid); „Cert-Rotation via SIGHUP
(Daemon liest cert/key neu, Sessions halten)". Das Config-Schema (§3) nennt
`coap.dtls.cert_file`/`key_file`/`ca_file` und die `psk`-Liste aus
`identity` + `secret_hex`, die CLI (§3) `--dtls-psk-id`/`--dtls-psk`/
`--dtls-cert`/`--dtls-key`. Auth+ACL laufen über Vendor-Option 65000
(CoAP-Application-Auth-Token).

**Repo:** `crates/coap-bridge/src/dtls.rs` — `DtlsMode` (RFC 7252 §9.1),
`DtlsCipherSuite` mit den drei §7.1-Suiten samt IANA-Code-Points,
`PskStore`/`PskEntry` (Lookup je Identity, Identity-Hint nach RFC 4279 §5.2),
`parse_secret_hex` und `DtlsCredentials` mit
`effective_cipher_suites`/`uses_psk`. `crates/coap-bridge/src/dtls_transport.rs`
(Feature `dtls`) — `load_certificate` liest `cert_file`+`key_file` als PEM
(PKCS#8; eine SEC1/PKCS#1-Datei wird mit dem Konvertierungshinweis abgewiesen),
`load_client_cas` legt `ca_file` als Trust-Store an und schaltet
`RequireAndVerifyClientCert`, `server_config` baut daraus den PSK- oder den
Cert-Handshake mit `supported_cipher_suites`, `DtlsCoapServer::bind_with`
trennt UDP-Listener und DTLS-Konfiguration, `rotate` tauscht die Konfiguration
für den nächsten Handshake auf demselben Socket, `DtlsCoapClient::connect` /
`connect_psk`. `crates/coap-bridge/src/daemon/config.rs` liest den
`coap.dtls`-Block (`cert_file`, `key_file`, `ca_file`, `cipher_suites`, `psk`)
und weist unbekannte Suite-Namen, fehlende `secret_hex`-Felder und eine
`cert_file`-ohne-`key_file`-Konfiguration ab;
`crates/coap-bridge/src/daemon/cli.rs` +
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs` verdrahten die vier
DTLS-Flags; `crates/coap-bridge/src/daemon/server.rs`
(`spawn_dtls_server`/`dtls_accept_loop`) gibt die Credentials an den Listener
und führt beim SIGHUP-`reload_flag` die Rotation aus;
`crates/coap-bridge/src/daemon/security.rs` (Option-65000-Auth-Wireup).

**Tests:** `crates/coap-bridge/src/dtls.rs` inline —
`cipher_suite_names_and_code_points_match_iana`,
`psk_store_looks_up_by_identity_and_refuses_short_secrets`,
`secret_hex_round_trips_and_rejects_garbage`,
`credentials_pick_the_mode_they_were_configured_for` plus die
`validate_dtls_mode`-Fälle; `crates/coap-bridge/tests/dtls_coap_e2e.rs`
(Feature `dtls`) — `dtls_coap_get_content_roundtrip` (Handshake + CoAP-GET →
2.05 Content), `dtls_psk_handshake_carries_coap` (PSK-Mode ohne Zertifikat),
`dtls_psk_unknown_identity_is_refused`,
`dtls_certificate_files_and_rotation_keep_sessions_alive` (Cert/Key von Platte,
SIGHUP-Rotation: der neue Client handshaked gegen das neue Zertifikat, die vor
der Rotation geöffnete Session trägt weiter CoAP),
`dtls_rejects_a_non_pkcs8_key_file`;
`crates/coap-bridge/tests/security_e2e.rs` (Option-65000-Auth).

**Status:** done — PSK- und Cert-Mode inklusive `ca_file`-Client-Verifikation,
Config- und CLI-Pfad und SIGHUP-Cert-Rotation sind implementiert und gemessen.
Von den drei Suiten, die §7.1 *empfiehlt*, bietet der Endpoint
`TLS_PSK_WITH_AES_128_CCM_8` und `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` an;
`TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` (0xc037) führt webrtc-dtls 0.12 in
`CipherSuiteId` nicht, und `supported_cipher_suites` verweigert den Start mit
Nennung der Suite, statt still auf eine andere auszuweichen.

### §7.2 OSCORE (RFC 8613)

**Spec:** §7.2 — Master-Secret/Salt/ID-Context, HKDF-Sender/Recipient-
Context pro `kid`, Replay-Window 32; der Daemon weist ungeschützte
Requests mit `4.01` ab, schützt Responses (§8.3) und Observe-
Notifications mit eigener Partial IV (§4.1.3.5.2); fail-closed ohne
`oscore`-Feature.

**Repo:** `crates/coap-bridge/src/oscore/` — `crates/coap-bridge/src/oscore/mod.rs` (Security-Context +
HKDF-Key-Derivation, RFC 8613 §3.2 / RFC 5869), `crates/coap-bridge/src/oscore/aead.rs` (AES-CCM-16-64-
128, Nonce §5.2, AAD §5.4), `crates/coap-bridge/src/oscore/message.rs` (Protect/Unprotect für Requests
§8.1/§8.2 und Responses §8.3/§8.4, Option-Klassen-Split §4.1.2),
`crates/coap-bridge/src/oscore/wire.rs` (OSCORE-Option-Codec §6.1 + Anti-Replay-Window §3.2.2, Default-
Breite 32); `crates/coap-bridge/src/daemon/config.rs` (`oscore:`-Block,
Spec-Schema), `crates/coap-bridge/src/daemon/oscore_gateway.rs` (Server-
State pro `kid`), `crates/coap-bridge/src/daemon/server.rs`
(Unprotect/Protect in `dispatch` + geschützter Notify-Pfad).

**Tests:** `crates/coap-bridge/tests/oscore_rfc8613_vectors.rs`:
`c4_protected_request_is_byte_exact`, `c5_protected_request_is_byte_exact`,
`c6_protected_request_with_kid_context_is_byte_exact`,
`c7_protected_response_is_byte_exact`,
`c8_protected_response_with_partial_iv_is_byte_exact`;
`crates/coap-bridge/tests/daemon_oscore_e2e.rs`:
`protected_post_round_trips_and_replay_is_refused`,
`unprotected_request_is_refused_with_4_01`,
`well_known_core_stays_reachable_in_the_clear`,
`notify_to_protected_observer_arrives_protected`,
`protected_observe_register_is_answered_protected`;
inline `crates/coap-bridge/src/oscore/mod.rs::tests` (7),
`crates/coap-bridge/src/oscore/aead.rs::tests` (4),
`crates/coap-bridge/src/oscore/wire.rs::tests` (9),
`crates/coap-bridge/src/oscore/message.rs::tests` (6),
`crates/coap-bridge/src/daemon/oscore_gateway.rs::tests` (6).

**Status:** done — volle OSCORE-Implementation inklusive Daemon-
Verdrahtung (ADR 0010); Korrektheit byte-exakt gegen die RFC-8613-
Appendix-C-Vektoren C.1-C.8 verankert.

### §7.3 ACL pro Topic

**Spec:** §7.3 — Subject = Vendor-Auth-Token-ID (CoAP-Option-65000) oder
Cert-Subject-DN.

**Repo:** `crates/coap-bridge/src/daemon/config.rs` (ACL-Felder),
`crates/coap-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs`;
`crates/coap-bridge/src/dtls_transport.rs::DtlsCoapSession::peer_identity`
(die DTLS-Identität für `auth.mode: mtls`: der Subject-DN des
Client-Zertifikats nach RFC 4514 — eine Regel darf auch den Fingerprint
`mtls:<SHA-256>` nennen — oder `psk:<Identität>`). Vorher erreichte die DTLS-Identität
den Request-Pfad nicht; `mtls` beantwortete jede Anfrage mit 4.01 (aus dem
Code). `auth.jwt` wie in der WS-Bridge.

**Tests:** `crates/coap-bridge/tests/daemon_dtls_e2e.rs::mtls_takes_the_psk_identity_as_the_acl_subject`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::mtls_without_dtls_is_a_config_error`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::mtls_takes_the_certificate_dn_as_the_acl_subject`
(`CN=sensor-9` 2.04, `CN=other` derselben CA 4.03),
`crates/coap-bridge/src/daemon/security.rs::tests::jwt_without_a_public_key_is_a_config_error`,
`crates/coap-bridge/tests/security_e2e.rs::acl_write_deny_yields_4_03_forbidden`,
`crates/coap-bridge/tests/security_e2e.rs::acl_observe_register_deny_yields_4_03`,
`crates/coap-bridge/tests/security_e2e.rs::auth_bearer_token_in_option_is_accepted`,
`crates/coap-bridge/tests/security_e2e.rs::auth_missing_bearer_token_yields_4_01_unauthorized`;
`crates/coap-bridge/src/daemon/security.rs::tests::coap_acl_subscribe_check`,
`::coap_bearer_in_option_accepted`, `::coap_missing_option_with_bearer_rejected`;
`crates/coap-bridge/tests/example_yaml_loadback.rs::example_yaml_acl_reaches_the_daemon`.

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/daemon/cli.rs::tests` prüft die
Flag-Zerlegung, aber kein Test prüft die JSON-Log-Form oder die Wirkung von
`--log-level`.
### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + 10 Counter/Gauge-Familien.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/daemon/config.rs`,
`crates/coap-bridge/src/daemon/runtime_common.rs` (Counter/Gauge-
Familien Cluster-A-Wireup).

**Tests:** `crates/coap-bridge/src/daemon/runtime_common.rs::tests::metrics_register_idempotent`,
`crates/coap-bridge/src/daemon/runtime_common.rs::tests::admin_endpoint_serves_metrics`,
`crates/coap-bridge/src/daemon/runtime_common.rs::tests::catalog_render_contains_topic_fields`.

**Status:** done

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert Span-Emission.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/daemon/runtime_common.rs` baut die
`OtlpConfig`, aber kein Test prüft, dass `OTEL_EXPORTER_OTLP_ENDPOINT`
gelesen wird oder dass pro CoAP-Exchange ein Span entsteht.
## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → DTLS → DCPS → Reader/Writer → UDP-Bind 5683/
5684 → Signal-Handler.

**Repo:** `crates/coap-bridge/src/daemon/mod.rs`,
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, Observer-Deregister, Cleanup;
SIGHUP TLS+ACL-Reload.

**Repo:** `crates/coap-bridge/src/daemon/server.rs::DaemonHandle::shutdown`,
`::end_observations` (Pumps zuerst, dann an jeden Observer eine
Notification 5.03 ohne Observe-Option mit seinem Token, RFC 7641 §4.2;
vorher verwarf der Shutdown die Registry ohne Nachricht),
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs` (wartet auf das
Stop-Flag; das Binary lief vorher 3 s nach SIGTERM weiter, gemessen),
`::reload` (ACL und Auth, DTLS-Material über den DTLS-Reload).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::the_shutdown_ends_every_observation_with_5_03`,
`::the_binary_exits_0_on_sigterm`;
`crates/coap-bridge/tests/security_e2e.rs::sighup_applies_the_reloaded_acl`.

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + libcoap/californium/aiocoap

**Spec:** §10 — Daemon ist normaler RTPS-Peer; CoAP-Seite gegen
libcoap/californium/aiocoap/Eclipse-Wakaama.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::libcoap_post_to_zerodds_daemon_returns_2_04_changed`,
`crates/coap-bridge/tests/cross_vendor.rs::libcoap_get_well_known_core_returns_catalog`,
`crates/coap-bridge/tests/cross_vendor.rs::libcoap_observe_receives_notification` (alle `#[ignore]`, Feature
`cross-vendor-tests`); die RTPS-Seite
`crates/coap-bridge/src/bridge.rs::tests::bridge_apply_write_then_read_round_trip`.
Californium und aiocoap hat niemand gegen den Daemon laufen lassen — siehe
§10a.

**Status:** partial — libcoap ist gemessen, Californium und aiocoap nicht; der
fehlende Teil wird als §10a `open` geführt.
### §10a Client-Matrix Californium/aiocoap

**Spec:** §10 — die CoAP-Seite läuft gegen Californium und aiocoap.

**Repo:** —

**Tests:** —

**Status:** open — der Teil von §10 und §12.3 jenseits libcoap.
`crates/coap-bridge/tests/cross_vendor.rs` kennt nur libcoap.

---

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-coap-bridged`; Configs/Services/Docker;
Manuals.

**Repo:** `packaging/linux/systemd/zerodds-coap-bridged.service`,
`packaging/macos/launchd/org.zerodds.coap-bridged.plist`,
`packaging/macos/homebrew/zerodds-coap-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/coap-bridged/`,
`packaging/linux/configs/coap-bridged.yaml.example`,
`man/man1/zerodds-coap-bridged.1`,
`man/man5/zerodds-coap-bridged.yaml.5`.

**Tests:** `crates/coap-bridge/tests/example_yaml_loadback.rs::{example_yaml_parses_without_error, example_yaml_key_fields_match_intent, example_yaml_topics_reach_the_daemon, example_yaml_acl_reaches_the_daemon, example_yaml_has_no_unknown_top_level_keys}`
(die ausgelieferte Beispiel-Konfiguration durch den echten Parser, mit den Werten, die sie nennt);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(die Handbuchseite und die Beispieldatei nennen dieselben Schlüssel).

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/coap_codec/block_assembler/observe_table/dtls/
oscore/dds_pump je ≥ 5 Tests.

**Repo:** `crates/coap-bridge/src/{daemon/config.rs,codec.rs,message.rs,option.rs,blockwise.rs,observe.rs,reliability.rs,uri.rs,core_link.rs,bridge.rs,dtls.rs,multicast.rs,matching.rs,caching_proxy.rs,method_props.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul:
`crates/coap-bridge/src/codec.rs::tests::empty_message_round_trip` (19),
`crates/coap-bridge/src/option.rs::tests::repeatable_options_match_spec_table_4` (12),
`crates/coap-bridge/src/message.rs::tests::code_byte_round_trip` (7),
`crates/coap-bridge/src/blockwise.rs::tests::round_trip_large_block_value` (10),
`crates/coap-bridge/src/bridge.rs::tests::bridge_discover_lists_keys` (16),
`crates/coap-bridge/src/observe.rs::tests` (11),
`crates/coap-bridge/src/reliability.rs::tests` (16),
`crates/coap-bridge/src/caching_proxy.rs::tests` (15),
`crates/coap-bridge/src/dtls.rs::tests` (15),
`crates/coap-bridge/src/multicast.rs::tests` (9),
`crates/coap-bridge/src/uri.rs::tests::parses_basic_coap_uri` (11),
`crates/coap-bridge/src/matching.rs::tests` (7),
`crates/coap-bridge/src/core_link.rs::tests` (8),
`crates/coap-bridge/src/method_props.rs::tests` (5);
dazu `crates/coap-bridge/tests/fuzz_smoke.rs::fuzz_coap_decode_no_panic`,
`::empty_input_returns_err_not_panic`, `::single_byte_inputs_no_panic`.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, libcoap-Client, POST/Observe/Block
Roundtrip.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::well_known_core_returns_link_format_catalog`,
`::observe_register_returns_initial_content_with_observe_option`,
`::unknown_path_returns_bad_request`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-DDS-Subscriber + libcoap/californium-Client
im Compose.

**Repo:** `crates/coap-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::libcoap_post_to_zerodds_daemon_returns_2_04_changed`,
`crates/coap-bridge/tests/cross_vendor.rs::libcoap_get_well_known_core_returns_catalog`,
`crates/coap-bridge/tests/cross_vendor.rs::libcoap_observe_receives_notification`; die DDS-Seite
`crates/coap-bridge/src/bridge.rs::tests::bridge_apply_write_then_read_round_trip`.
Die Client-Matrix jenseits libcoap ist nicht gemessen — siehe §10a.

**Status:** partial — libcoap und die DDS-Seite sind geprüft, die restliche
Client-Matrix nicht.
## §13 Cross-References

### §13 Verwandte Library + RFCs + Daemons

**Spec:** §13 — Library `crates/coap-bridge/`, RFC 7252/7641/7959/8613,
Wire-Format, Deployment, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config (z.B.
Content-Format-IDs), Major=Wire-Protocol-Change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

18 done / 3 partial / 4 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-coap-bridge --features daemon,dtls,oscore`
— 251 Tests grün, 0 failed (218 Unit-Tests plus die End-to-End-Suiten für
Daemon, DTLS, OSCORE und Security).

Offene Punkte: siehe `zerodds-coap-bridge-1.0.open.md`. Decision-Records: keine.
