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

**Status:** n/a (informative) — Zuordnung der Levels; die Anforderungen sind die Items §2 bis §12.

### §1a L6 Multi-Tenant

**Spec:** §1 L6 — mehrere DomainParticipants je Daemon; eine
Mandantenbindung je DTLS-Sitzung.

**Repo:** `crates/coap-bridge/src/daemon/config.rs::{TenantConfig, DaemonConfig::from_config}`, `crates/coap-bridge/src/daemon/server.rs::{start_with_reload, DtlsShared::core_of, dtls_session}`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_1_l6_a_dtls_session_is_bound_to_the_tenant_of_its_subject`, `crates/coap-bridge/src/daemon/config.rs::tests::coapbridge_1_l6_the_tenants`, `crates/coap-bridge/tests/example_yaml_loadback.rs::coapbridge_3_the_example_of_the_spec_loads`.

**Status:** done — ein Mandant ist ein DomainParticipant auf seiner Domäne mit eigenen Topics, Beobachtungen und Katalog; eine DTLS-Sitzung mit einem PSK, das ein Mandant nennt, schreibt auf dessen Domäne (beim DDS-Reader geprüft) und sieht nur dessen Ressourcen; Klartext und eine Sitzung ohne Mandant sehen die Topics von `domain`. Geprüft mit PSK; Zertifikat und Rohschlüssel binden über dasselbe Subjekt, sind aber nicht eigens geprüft.

## §2 CLI-Surface

### §2 zerodds-coap-bridged CLI

**Spec:** §2 — Optionen `--config`/`--bind`/`--domain`/`--dtls-*`/
`--topic`/`--log-level`/`--metrics`/`--version`/`--help`; Exit-Codes
0/1/2/3/4.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/coap-bridge/src/daemon/cli.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_post_to_configured_path_returns_2_04_changed`,
`::coapbridge_2_well_known_core_returns_link_format_catalog`,
`::coapbridge_2_observe_register_returns_initial_content_with_observe_option`,
`::coap_5_9_2_5_an_unknown_resource_is_4_04_and_a_dot_segment_4_00`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`coap`/`oscore`/`topics`/
`content_format`/`acl`/`metrics`; ENV-Substitution.

**Repo:** `crates/coap-bridge/src/daemon/config.rs`, `crates/config/src/parse.rs` (the shared parser), `crates/coap-bridge/src/daemon/mod.rs`.

**Tests:** `crates/coap-bridge/tests/example_yaml_loadback.rs::{coapbridge_3_the_example_of_the_spec_loads, coapbridge_3_example_yaml_parses_without_error, coapbridge_3_example_yaml_key_fields_match_intent}`, `crates/coap-bridge/src/daemon/config.rs::tests::coapbridge_3_the_keys_of_the_forms_the_blocks_and_the_qos`, `crates/coap-bridge/src/daemon/config.rs::tests::what_the_old_parser_turned_into_defaults_is_refused`.

**Status:** done — das Beispiel aus §3 lädt wörtlich mit den Werten, die es nennt; jeder neue Schlüssel und seine Grenzen sind geprüft.

## §4 CoAP-Wire-Protocol

### §4.1 RFC-7252 Header + Token + Options

**Spec:** §4.1 — 4-Byte-Header (Ver/T/TKL/Code/Message-ID), Token,
Options, Payload-Marker `0xFF`.

**Repo:** `crates/coap-bridge/src/codec.rs`,
`crates/coap-bridge/src/message.rs`,
`crates/coap-bridge/src/option.rs`,
`crates/coap-bridge/src/method_props.rs`.

**Tests:** `crates/coap-bridge/src/codec.rs::tests` (19 Tests):
`coap_3_encodes_minimum_get_request_to_4_byte_header`,
`coap_3_the_header_of_each_message_type_is_the_octets_of_the_layout`,
`coap_3_token_length_above_8_is_rejected_on_encode`, `coapbridge_4_1_header_too_short_decode_fails`,
`coap_3_unsupported_version_decode_fails`,
`coap_3_reserved_token_length_9_through_15_decode_fails`, `coap_3_token_truncated_decode_fails`,
`coap_3_1_single_short_option_encodes_in_one_byte_header`,
`coap_3_1_option_delta_extended_13_uses_one_extra_byte`,
`coap_3_1_option_delta_extended_14_uses_two_extra_bytes`,
`coap_3_1_option_length_extended_13_uses_one_extra_byte`,
`coap_3_1_delta_encoding_sums_across_multiple_options`,
`coap_3_payload_marker_separates_options_from_payload`,
`coap_3_payload_marker_without_payload_is_format_error`, `coap_3_1_options_are_sorted_on_encode`,
`coapbridge_4_1_an_empty_acknowledgement_is_four_octets`;
`crates/coap-bridge/src/message.rs::tests` (`coap_1_1_the_message_types_are_the_numbers_of_section_3`,
`coapbridge_4_1_a_code_is_three_bits_class_and_five_bits_detail`, `coap_1_1_well_known_codes_match_spec_values`,
`coapbridge_4_1_code_display_uses_dotted_format`);
`crates/coap-bridge/src/option.rs::tests`
(`coap_5_4_critical_unsafe_nocachekey_bits_per_spec_5_4_6`,
`coap_5_4_repeatable_options_match_spec_table_4`, `coap_3_2_uint_strips_leading_zero_bytes`);
`crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_post_to_configured_path_returns_2_04_changed`.

**Status:** done

### §4.2 POST/PUT/DELETE → DDS-Write/Dispose

**Spec:** §4.2 — POST → DDS-Write (`2.04 Changed`), PUT idempotent,
DELETE → Dispose; `4.00`/`4.13`/`5.00` Error-Mapping.

**Repo:** `crates/coap-bridge/src/daemon/server.rs::{serve_write, write_sample, serve_delete, dispatch, suppressed}`, `crates/coap-bridge/src/daemon/proxy.rs::deliver` (No-Response at the proxy), `crates/coap-bridge/src/daemon/representation.rs::TopicType::{read, key_hash, key_holder}`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form, coapbridge_4_2_delete_disposes_the_instance_it_names, rfc7967_a_suppressed_response_does_not_leave}`, `crates/coap-bridge/src/daemon/server.rs::tests::{coapbridge_4_2_a_failed_dds_write_is_5_00, rfc7967_2_1_the_bits_suppress_the_classes, coapbridge_4_2_a_key_hash_is_32_hex_digits}`, `crates/coap-bridge/tests/proxy_e2e.rs::rfc7967_2_the_proxy_forwards_no_response_and_suppresses_its_response`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_post_to_configured_path_returns_2_04_changed`.

**Status:** done — POST/PUT in jeder Form nach DDS, beim Reader geprüft; `4.00` für einen Körper, der kein Sample ist, `4.15` für ein Content-Format ohne Form, `5.00` für einen fehlgeschlagenen DDS-Write; DELETE disponiert die Instanz nach `key_hash` oder nach einem Sample im Körper, der Reader sieht den Dispose; `4.05` ohne Key oder Writer; No-Response unterdrückt die Klassen seiner Bits, auch am Proxy, ein CON bekommt ein leeres ACK. Gegen DDS-Reader anderer Hersteller ist der Dispose nicht gemessen.

### §4.3 GET + Observe (RFC 7641) → DDS→CoAP-Push

**Spec:** §4.3 — GET mit `Observe:0` registriert, Notify pro Sample
mit `Observe:<seq>`; Cancel via `Observe:1` oder RST.

**Repo:** `crates/coap-bridge/src/daemon/observe.rs` (registry, notifications, retransmission, Reset, ping, deadline), `crates/coap-bridge/src/daemon/server.rs::serve_get`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset, coapbridge_6_replay_lifespan_and_deadline, coapbridge_6_the_ping_and_the_bound_of_unconfirmed_notifications}`, `crates/coap-bridge/src/daemon/observe.rs::tests::{coap_4_2_a_confirmable_message_is_retransmitted_with_back_off, rfc7641_3_4_the_sequence_number_is_24_bits, rfc7641_4_5_the_bound_of_unconfirmed_notifications, rfc7641_4_1_an_observer_is_named_by_endpoint_and_token}`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_observe_register_returns_initial_content_with_observe_option`.

**Status:** done — eine Benachrichtigung je DDS-Sample in der Form der Registrierung (XCDR1 vom DDS-Writer, JSON beim Beobachter); CON bei RELIABLE mit Wiederholung, eine neuere ersetzt die laufende; RST, eine nicht bestätigte CON-Benachrichtigung, ein unbeantworteter Ping und eine verpasste Deadline beenden die Beobachtung; VOLATILE gibt zuerst nichts; `observe_max_age_secs` macht nach einer Sekunde ohne ACK eine CON-Benachrichtigung.

### §4.4 Block-Wise-Transfer (RFC 7959)

**Spec:** §4.4 — Block1 (POST) + Block2 (Notify); `block_size` (SZX
16..1024); Defragmentation-Cap.

**Repo:** `crates/coap-bridge/src/blockwise.rs`, `crates/coap-bridge/src/daemon/server.rs::{serve_write, representation, content_response}`, `crates/coap-bridge/src/daemon/observe.rs::notify_one`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{rfc7959_block_size_and_the_limits_of_a_transfer, coapbridge_12_2_a_4_kib_sample_by_block1_and_back_by_block2}`, `crates/coap-bridge/src/blockwise.rs::tests` (10), `crates/coap-bridge/src/daemon/server.rs::tests::{coap_11_3_an_unauthenticated_response_is_sliced_into_small_blocks, rfc7959_2_2_the_block_size_as_szx}`, `crates/coap-bridge/tests/daemon_e2e.rs::{coapbridge_4_4_block1_chunked_post_completes_with_2_04_changed, coapbridge_4_4_block1_out_of_order_returns_2_31_continue_only_in_order}`.

**Status:** done — `block_size` 16 schneidet in Blöcke zu 16 Oktetten, ein Block außer der Reihe ist `4.08`, eine Übertragung über `max_body_size` `4.13` mit Size1; 4 KiB per Block1 hinein und per Block2 zurück.

### §4.5 Content-Format-Registry 65000/65001/65002/50/60

**Spec:** §4.5 — Vendor-Range 65000-65535 für CDR2-LE/BE + CDR1-LE;
`50=application/json`, `60=application/cbor`.

**Repo:** `crates/coap-bridge/src/daemon/representation.rs` (`Form`, `ContentFormats`, `Extensibility`, `TopicType`), `crates/coap-bridge/src/content_format.rs`, `crates/coap-bridge/src/daemon/server.rs::Core::represent`.

**Tests:** `crates/coap-bridge/src/daemon/representation.rs::tests::{coapbridge_4_5_the_five_forms_and_their_numbers, coapbridge_4_5_the_identifier_names_the_form, xtypes_7_4_the_encoding_of_a_form_follows_the_extensibility, coapbridge_4_5_every_form_carries_the_same_sample, rfc8949_6_2_the_cbor_form_is_the_json_value_in_cbor}`, `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form`, `crates/coap-bridge/src/content_format.rs::tests::coap_12_3_the_initial_registry_entries`.

**Status:** done — die fünf Formen mit ihren Nummern; jede Form trägt dasselbe Sample (Oktette des DHEADER und der Ausrichtung geprüft), DDS-Seite mit `D_CDR2_LE`.

## §5 Topic-Mapping

### §4.5a Content-Format-Werte 65001/65002/50/60

**Spec:** §4.5 — CDR2-BE (65001), CDR1-LE (65002), `50=application/json`,
`60=application/cbor`.

**Repo:** `crates/coap-bridge/src/daemon/representation.rs`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form, coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset}`, `crates/coap-bridge/src/daemon/representation.rs::tests::coapbridge_4_5_every_form_carries_the_same_sample`.

**Status:** done — 65001, 65002, 50 und 60 hinein und hinaus, beim DDS-Reader und vom DDS-Writer geprüft.

### §5.1 Slug-Algorithmus DDS → CoAP-URI

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `coap_uri_path`.

**Repo:** `crates/coap-bridge/src/uri.rs`,
`crates/coap-bridge/src/daemon/config.rs`.

**Tests:** `crates/coap-bridge/src/uri.rs::tests` (11 Tests):
`coap_6_1_parses_basic_coap_uri`, `coap_6_1_parses_coaps_default_port`, `coap_6_1_parses_explicit_port`,
`coap_6_1_parses_query_params`, `coap_6_1_parses_empty_path_when_no_slash`,
`coap_6_1_rejects_unknown_scheme`, `coap_6_1_rejects_missing_host`, `coap_6_1_rejects_invalid_port`,
`coap_6_1_rejects_fragment`, `coap_1_1_default_port_returns_5684_for_coaps`,
`coap_6_4_an_empty_segment_is_a_uri_path_option`;
den Slug `crates/coap-bridge/src/daemon/config.rs::tests::{slug_handles_double_colon, slug_replaces_unsafe_chars}` und
`crates/coap-bridge/tests/example_yaml_loadback.rs::coapbridge_5_1_example_yaml_topics_reach_the_daemon`.

**Status:** done

### §5.2 /.well-known/core (RFC 6690) Catalog

**Spec:** §5.2 — `/.well-known/core` liefert Link-Format-Resource-List
mit `rt="dds.topic"`, `ct=65000`, `type="..."`.

**Repo:** `crates/coap-bridge/src/daemon/server.rs::{render_well_known_core, serve_schema}`, `crates/coap-bridge/src/core_link.rs`, `crates/coap-bridge/src/daemon/representation.rs::TopicType::idl`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form`, `crates/coap-bridge/src/daemon/representation.rs::tests::coapbridge_5_2_the_schema_is_the_idl_of_the_type`, `crates/coap-bridge/tests/contract_idl_compose.rs::idl_7_3_a_file_compiles_with_its_includes_from_its_own_directory`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_well_known_core_returns_link_format_catalog`.

**Status:** done — der Katalog nennt `ct="65000 65001 65002 50 60"`, `type="Sensor::Reading"` und den Link `</schema/…>;rt="dds.schema"`; `GET /schema/<slug>` gibt die IDL, die als Typ wieder kompiliert; POST darauf `4.05`.

## §6 QoS-Translation

### §6 DDS-QoS → CoAP-Behavior Map

**Spec:** §6 — Reliable→CON, BestEffort→NON, Volatile/TransientLocal,
Lifespan→`Max-Age`, Deadline→`5.03`, Liveliness→Ping, Partition→Filter.

**Repo:** `crates/coap-bridge/src/daemon/qos_translation.rs::topic_behavior`, `crates/coap-bridge/src/daemon/observe.rs`, `crates/coap-bridge/src/daemon/server.rs::register_topic`.

**Tests:** `crates/coap-bridge/src/daemon/qos_translation.rs::tests::{coapbridge_6_reliability_picks_the_message_type, coapbridge_6_durability_and_history_set_the_replay, coapbridge_6_lifespan_and_deadline}`, `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_6_replay_lifespan_and_deadline, coapbridge_6_the_ping_and_the_bound_of_unconfirmed_notifications, coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset}`, `crates/coap-bridge/src/daemon/observe.rs::tests::coapbridge_6_the_max_age_is_what_is_left_of_the_lifespan`.

**Status:** done — RELIABLE → CON, BEST_EFFORT → NON; TRANSIENT_LOCAL mit KEEP_LAST(3) gibt die letzten drei; Max-Age aus `lifespan_ms`; `5.03` mit Max-Age 0 bei verpasster `deadline_ms` an GET und Beobachter; der Ping alle `keep_alive_secs`. Partition und Liveliness-Lease gehen an die DDS-Endpunkte; ihre Wirkung auf DDS-Seite ist nicht eigens geprüft.

## §7 Security

### §7.1 DTLS coaps:// + Cipher-Suites

**Spec:** §7.1 — „`coaps://`-Mode aktiviert über `coap.dtls.enabled: true`";
die Modi PreSharedKey (`coap.dtls.psk`), RawPublicKey (`rpk_key_file`,
`rpk_trusted` mit Kennungen nach RFC 6920) und Certificate (`cert_file`,
`key_file`, `ca_file`); die Cipher-Suites `TLS_PSK_WITH_AES_128_CCM_8`,
`TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` und `TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256`;
„Cert rotation via SIGHUP (the daemon re-reads cert/key, sessions are kept)".
Das Subjekt eines DTLS-Clients für §7.2/§7.3 ist `psk:<identity>`, der DN des
Zertifikats oder `rpk:` mit der `ni`-URI des Rohschlüssels.

**Repo:** `crates/coap-bridge/src/dtls.rs` — `DtlsMode`, `DtlsCipherSuite` mit
den drei §7.1-Suiten samt IANA-Code-Points, `PskStore`, `parse_secret_hex`,
`DtlsCredentials` (mit `rpk_key_file`, `rpk_trusted`).
`crates/coap-bridge/src/dtls_transport.rs` (Feature `dtls`) über
`crates/dtls-openssl` (OpenSSL ab 3.2, ADR 0014) — `server_config`,
`DtlsCoapServer::{bind_with, rotate, accept}`, `DtlsCoapClient::{connect,
connect_psk, connect_verified, connect_rpk}`. `crates/coap-bridge/src/daemon/config.rs`
liest den `coap.dtls`-Block und weist unbekannte Suiten und Kennungen ab, die
keine RFC-6920-Namen sind; `crates/coap-bridge/src/daemon/server.rs`
(`spawn_dtls_server`, `dtls_session`, `dtls_subject`) führt je Sitzung einen
Thread, rotiert beim SIGHUP-`reload_flag` und bildet das Subjekt.

**Tests:** `crates/coap-bridge/src/dtls.rs` inline —
`coapbridge_7_1_cipher_suite_names_and_code_points_match_iana`,
`coapbridge_7_1_psk_store_looks_up_by_identity_and_refuses_short_secrets`,
`coapbridge_7_1_secret_hex_round_trips_and_rejects_garbage`,
`coapbridge_7_1_credentials_pick_the_mode_they_were_configured_for`;
`crates/coap-bridge/tests/dtls_coap_e2e.rs::{coap_9_dtls_coap_get_content_roundtrip,
coap_9_dtls_psk_handshake_carries_coap, coap_9_dtls_psk_unknown_identity_is_refused,
coap_9_1_3_2_raw_public_keys_carry_coap,
coap_9_dtls_certificate_files_and_rotation_keep_sessions_alive,
coapbridge_7_1_the_three_suites_of_section_7_1_are_negotiated,
coapbridge_7_1_dtls_refuses_a_key_file_it_cannot_read}`;
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::{coap_9_daemon_serves_coaps_over_dtls_request_response_and_observe,
coapbridge_7_3_mtls_takes_the_psk_identity_as_the_acl_subject,
coapbridge_7_3_mtls_takes_the_certificate_dn_as_the_acl_subject,
coap_9_1_3_2_the_daemon_takes_raw_public_keys_and_names_them_in_the_acl}`;
`crates/coap-bridge/tests/security_e2e.rs` (Option-65000-Auth).

**Status:** done — die drei Modi, die drei Suiten aus §7.1, `ca_file`-Prüfung
von Client-Zertifikaten, `rpk_trusted`, SIGHUP-Rotation und die Subjekte sind
gebaut und getestet; gegengeprüft mit libcoap 4.3.4 (GnuTLS: RawPublicKey in
beide Richtungen und PSK; OpenSSL: PSK und Zertifikat).

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
`coapbridge_7_2_c4_protected_request_is_byte_exact`, `coapbridge_7_2_c5_protected_request_is_byte_exact`,
`coapbridge_7_2_c6_protected_request_with_kid_context_is_byte_exact`,
`coapbridge_7_2_c7_protected_response_is_byte_exact`,
`coapbridge_7_2_c8_protected_response_with_partial_iv_is_byte_exact`;
`crates/coap-bridge/tests/daemon_oscore_e2e.rs`:
`coapbridge_7_2_protected_post_round_trips_and_replay_is_refused`,
`coapbridge_7_2_unprotected_request_is_refused_with_4_01`,
`coapbridge_7_2_well_known_core_stays_reachable_in_the_clear`,
`coapbridge_7_2_notify_to_protected_observer_arrives_protected`,
`coapbridge_7_2_protected_observe_register_is_answered_protected`;
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

**Tests:** `crates/coap-bridge/tests/daemon_dtls_e2e.rs::coapbridge_7_3_mtls_takes_the_psk_identity_as_the_acl_subject`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::coapbridge_7_3_mtls_without_dtls_is_a_config_error`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::coapbridge_7_3_mtls_takes_the_certificate_dn_as_the_acl_subject`
(`CN=sensor-9` 2.04, `CN=other` derselben CA 4.03),
`crates/coap-bridge/src/daemon/security.rs::tests::coapbridge_7_3_jwt_without_a_public_key_is_a_config_error`,
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_7_3_acl_write_deny_yields_4_03_forbidden`,
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_7_3_acl_observe_register_deny_yields_4_03`,
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_7_3_auth_bearer_token_in_option_is_accepted`,
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_7_3_auth_missing_bearer_token_yields_4_01_unauthorized`;
`crates/coap-bridge/src/daemon/security.rs::tests::coapbridge_7_3_coap_acl_subscribe_check`,
`::coapbridge_7_3_coap_bearer_in_option_accepted`, `::coapbridge_7_3_coap_missing_option_with_bearer_rejected`;
`crates/coap-bridge/tests/example_yaml_loadback.rs::coapbridge_7_3_example_yaml_acl_reaches_the_daemon`.

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** `crates/coap-bridge/src/daemon/log.rs`, `crates/coap-bridge/src/daemon/server.rs::finish`, `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/src/daemon/log.rs::tests::{coapbridge_8_1_a_line_is_a_json_object_with_the_spec_fields, rfc3339_5_6_the_utc_date_and_time, coapbridge_8_1_the_log_level_is_the_threshold}`, `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_8_1_8_3_the_log_lines_and_the_spans_of_an_exchange`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_9_2_the_binary_exits_0_on_sigterm`.

**Status:** done — jede Zeile ist ein JSON-Objekt mit `timestamp`, `level`, `event` und den Feldern aus §8.1; über das Binary mit `log_level: debug` geprüft.

### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + 10 Counter/Gauge-Familien.

**Repo:** `crates/coap-bridge/src/daemon/runtime_common.rs::{BridgeMetrics, family, serve_admin_endpoints}`, `crates/coap-bridge/src/daemon/config.rs` (`metrics.path`), `crates/dtls-openssl/src/transport.rs::Server::failed_handshakes`.

**Tests:** `crates/coap-bridge/src/daemon/runtime_common.rs::tests::{coapbridge_8_2_admin_endpoint_serves_metrics, coapbridge_8_2_metrics_register_idempotent, coapbridge_8_2_catalog_render_contains_topic_fields}`, `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form`.

**Status:** done — die zehn Familien mit ihren Labels auf `metrics.path`; im Ende-zu-Ende-Lauf gezählt.

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert Span-Emission.

**Repo:** `crates/coap-bridge/src/daemon/runtime_common.rs::{Tracer, ExchangeSpan, otlp_config_from_env, spawn_otlp_flush_loop}`, `crates/coap-bridge/src/daemon/server.rs::finish`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_8_1_8_3_the_log_lines_and_the_spans_of_an_exchange`.

**Status:** done — mit `OTEL_EXPORTER_OTLP_ENDPOINT` erreicht ein Span `coap.request` mit dem Kind `dds.write` einen Kollektor im Test, spätestens beim Beenden.

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → DTLS → DCPS → Reader/Writer → UDP-Bind 5683/
5684 → Signal-Handler.

**Repo:** `crates/coap-bridge/src/daemon/mod.rs`,
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_post_to_configured_path_returns_2_04_changed`.

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

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_9_2_the_shutdown_ends_every_observation_with_5_03`,
`::coapbridge_9_2_the_binary_exits_0_on_sigterm`;
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_9_2_sighup_applies_the_reloaded_acl`.

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + libcoap/californium/aiocoap

**Spec:** §10 — Daemon ist normaler RTPS-Peer; CoAP-Seite gegen
libcoap/californium/aiocoap/Eclipse-Wakaama.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`, `crates/coap-bridge/tests/cross_vendor.rs`, `crates/coap-bridge/tests/interop/{aiocoap_client.py, CfClient.java, reading.idl}`, `tests/integration/provision.sh` (`aiocoap`, `californium`, `wakaama`), `ci/integration_scenarios.py` (`integration-bridges-vendor`).

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::{coapbridge_10_libcoap_post_to_zerodds_daemon_returns_2_04_changed, coapbridge_10_libcoap_get_well_known_core_returns_catalog, coapbridge_10_libcoap_observe_receives_notification, coapbridge_10_aiocoap_posts_reads_and_observes_json, coapbridge_10_californium_posts_reads_and_observes_json, coapbridge_10_wakaama_registration_is_read_and_answered}` (`#[ignore]`, feature `cross-vendor-tests`, weekly in `integration-bridges-vendor`); the RTPS side `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form`.

**Status:** done — gemessen auf dem Linux-Testrechner gegen libcoap 4.3.4, aiocoap 0.4.14, Eclipse Californium 3.14.0 und Eclipse Wakaama `snapshots/2026-05-26` (7 von 7 Tests von `cross_vendor.rs`): POST, GET und Beobachtung in JSON mit aiocoap und Californium; Wakaamas Registrierung (`POST /rd`) wird gelesen und mit `4.05` beantwortet, Wakaama liest die Antwort mit seiner Message-ID und sendet nicht noch einmal.

### §10a Client-Matrix Californium/aiocoap

**Spec:** §10 — die CoAP-Seite läuft gegen Californium und aiocoap.

**Repo:** `crates/coap-bridge/tests/cross_vendor.rs`, `crates/coap-bridge/tests/interop/{aiocoap_client.py, CfClient.java}`.

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::{coapbridge_10_aiocoap_posts_reads_and_observes_json, coapbridge_10_californium_posts_reads_and_observes_json}`.

**Status:** done — Californium 3.14.0 und aiocoap 0.4.14 gegen den Daemon gemessen (siehe §10).

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

**Tests:** `crates/coap-bridge/tests/example_yaml_loadback.rs::{coapbridge_3_example_yaml_parses_without_error, coapbridge_3_example_yaml_key_fields_match_intent, coapbridge_5_1_example_yaml_topics_reach_the_daemon, coapbridge_7_3_example_yaml_acl_reaches_the_daemon, coapbridge_11_example_yaml_has_no_unknown_top_level_keys}`
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
`crates/coap-bridge/src/codec.rs::tests::coapbridge_4_1_an_empty_acknowledgement_is_four_octets` (19),
`crates/coap-bridge/src/option.rs::tests::coap_5_4_repeatable_options_match_spec_table_4` (12),
`crates/coap-bridge/src/message.rs::tests::coapbridge_4_1_a_code_is_three_bits_class_and_five_bits_detail` (7),
`crates/coap-bridge/src/blockwise.rs::tests::coapbridge_4_4_the_largest_block_number_is_three_octets` (10),
`crates/coap-bridge/src/bridge.rs::tests::coapbridge_12_1_bridge_discover_lists_keys` (16),
`crates/coap-bridge/src/observe.rs::tests` (11),
`crates/coap-bridge/src/reliability.rs::tests` (16),
`crates/coap-bridge/src/caching_proxy.rs::tests` (15),
`crates/coap-bridge/src/dtls.rs::tests` (15),
`crates/coap-bridge/src/multicast.rs::tests` (9),
`crates/coap-bridge/src/uri.rs::tests::coap_6_1_parses_basic_coap_uri` (11),
`crates/coap-bridge/src/matching.rs::tests` (7),
`crates/coap-bridge/src/core_link.rs::tests` (8),
`crates/coap-bridge/src/method_props.rs::tests` (5);
dazu `crates/coap-bridge/tests/fuzz_smoke.rs::coap_11_1_fuzz_coap_decode_no_panic`,
`::coapbridge_12_1_empty_input_returns_err_not_panic`, `::coapbridge_12_1_single_byte_inputs_no_panic`.

**Status:** done — jedes Modul hat mindestens fünf Tests; der Pumpenfaden ist `crates/coap-bridge/src/daemon/observe.rs` (`on_sample`, `notify_one`, `tick`) mit fünf Unit-Tests und den Ende-zu-Ende-Tests aus §12.2.

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, libcoap-Client, POST/Observe/Block
Roundtrip.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`, `crates/coap-bridge/tests/topic_e2e.rs`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form, coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset, coapbridge_12_2_a_4_kib_sample_by_block1_and_back_by_block2, coapbridge_8_1_8_3_the_log_lines_and_the_spans_of_an_exchange}`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_9_2_the_binary_exits_0_on_sigterm`.

**Status:** done — ein DDS-Reader erhält das Sample eines POST, ein DDS-Writer erzeugt eine Benachrichtigung, 4 KiB per Block1/Block2, das Binary als Unterprozess (SIGTERM, Log, Spans); libcoap als Gegenstelle steht in §12.3.

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-DDS-Subscriber + libcoap/californium-Client
im Compose.

**Repo:** `crates/coap-bridge/tests/cross_vendor.rs`, `crates/coap-bridge/tests/interop/{cyclone_reading.c, reading.idl}`, `tests/integration/provision.sh` (`cyclonedds-dev`).

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::coapbridge_12_3_a_coap_post_reaches_cyclone_and_a_cyclone_sample_an_observer`, `crates/coap-bridge/tests/cross_vendor.rs::coapbridge_10_libcoap_post_to_zerodds_daemon_returns_2_04_changed`.

**Status:** done — gemessen gegen Cyclone DDS 0.10.5 und libcoap 4.3.4: ein JSON-Sample per libcoap-POST kommt beim Cyclone-Reader als dasselbe Sample an, ein Sample des Cyclone-Writers bei libcoaps Beobachtung als JSON. Statt Docker Compose stellt der wöchentliche Job die Programme bereit.

## §13 Cross-References

### §13 Verwandte Library + RFCs + Daemons

**Spec:** §13 — Library `crates/coap-bridge/`, RFC 7252/7641/7959/8613,
Wire-Format, Deployment, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config (z.B.
Content-Format-IDs), Major=Wire-Protocol-Change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Regel für die Vergabe der Versionsnummer eines Release, keine Eigenschaft des Daemons.

---

## Audit-Status

26 done / 0 partial / 0 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9860 von `main` (`python3 ci/coverage_results.py --document zerodds-coap-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets` (Job `merge-rust-coap-bridge`) — 110 Tests grün.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (Job `merge-rust-traceability`) — 4 Tests grün.
* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets --features dtls,oscore` (Job `merge-rust-coap-bridge-features-dtls-oscore`) — 23 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor` (Job `integration-bridges-vendor`) — 3 Tests grün.

140 zitierte Tests grün, 4 nicht grün.

