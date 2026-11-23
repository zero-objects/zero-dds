# `zerodds-coap-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-coap-bridge-1.0.md`

Implementation:

- `crates/coap-bridge/` — DDS↔CoAP bridge.

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — assignment of the levels; the requirements are the items §2 to §12.

### §1a L6 multi-tenant

**Spec:** §1 L6 — several DomainParticipants per daemon; one tenant
binding per DTLS session.

**Repo:** `crates/coap-bridge/src/daemon/config.rs::{TenantConfig, DaemonConfig::from_config}`, `crates/coap-bridge/src/daemon/server.rs::{start_with_reload, DtlsShared::core_of, dtls_session}`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_1_l6_a_dtls_session_is_bound_to_the_tenant_of_its_subject`, `crates/coap-bridge/src/daemon/config.rs::tests::coapbridge_1_l6_the_tenants`, `crates/coap-bridge/tests/example_yaml_loadback.rs::coapbridge_3_the_example_of_the_spec_loads`.

**Status:** done — a tenant is a DomainParticipant on its domain with its own topics, observations and catalog; a DTLS session with a PSK a tenant names writes to its domain (checked at the DDS reader) and sees its resources only; plaintext and a session without a tenant see the topics of `domain`. Checked with a PSK; a certificate and a raw public key bind through the same subject but are not checked by themselves.

## §2 CLI surface

### §2 zerodds-coap-bridged CLI

**Spec:** §2 — options
`--config`/`--bind`/`--domain`/`--dtls-*`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/coap-bridge/src/daemon/cli.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_post_to_configured_path_returns_2_04_changed`,
`::coapbridge_2_well_known_core_returns_link_format_catalog`,
`::coapbridge_2_observe_register_returns_initial_content_with_observe_option`,
`::coap_5_9_2_5_an_unknown_resource_is_4_04_and_a_dot_segment_4_00`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level
`domain`/`coap`/`oscore`/`topics`/`content_format`/`acl`/`metrics`; ENV
substitution.

**Repo:** `crates/coap-bridge/src/daemon/config.rs`, `crates/config/src/parse.rs` (the shared parser), `crates/coap-bridge/src/daemon/mod.rs`.

**Tests:** `crates/coap-bridge/tests/example_yaml_loadback.rs::{coapbridge_3_the_example_of_the_spec_loads, coapbridge_3_example_yaml_parses_without_error, coapbridge_3_example_yaml_key_fields_match_intent}`, `crates/coap-bridge/src/daemon/config.rs::tests::coapbridge_3_the_keys_of_the_forms_the_blocks_and_the_qos`, `crates/coap-bridge/src/daemon/config.rs::tests::what_the_old_parser_turned_into_defaults_is_refused`.

**Status:** done — the example of §3 loads word for word with the values it names; every new key and its bounds are checked.

## §4 CoAP wire protocol

### §4.1 RFC-7252 header + token + options

**Spec:** §4.1 — a 4-byte header (Ver/T/TKL/Code/Message-ID), token,
options, payload marker `0xFF`.

**Repo:** `crates/coap-bridge/src/codec.rs`,
`crates/coap-bridge/src/message.rs`, `crates/coap-bridge/src/option.rs`,
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

### §4.2 POST/PUT/DELETE → DDS write/dispose

**Spec:** §4.2 — POST → DDS write (`2.04 Changed`), PUT idempotent, DELETE
→ dispose; `4.00`/`4.13`/`5.00` error mapping.

**Repo:** `crates/coap-bridge/src/daemon/server.rs::{serve_write, write_sample, serve_delete, dispatch, suppressed}`, `crates/coap-bridge/src/daemon/proxy.rs::deliver` (No-Response at the proxy), `crates/coap-bridge/src/daemon/representation.rs::TopicType::{read, key_hash, key_holder}`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form, coapbridge_4_2_delete_disposes_the_instance_it_names, rfc7967_a_suppressed_response_does_not_leave}`, `crates/coap-bridge/src/daemon/server.rs::tests::{coapbridge_4_2_a_failed_dds_write_is_5_00, rfc7967_2_1_the_bits_suppress_the_classes, coapbridge_4_2_a_key_hash_is_32_hex_digits}`, `crates/coap-bridge/tests/proxy_e2e.rs::rfc7967_2_the_proxy_forwards_no_response_and_suppresses_its_response`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_post_to_configured_path_returns_2_04_changed`.

**Status:** done — POST/PUT in every form into DDS, checked at the reader; `4.00` for a body that is no sample, `4.15` for a Content-Format of no form, `5.00` for a failed DDS write; DELETE disposes the instance named by `key_hash` or by a sample in the body, the reader sees the dispose; `4.05` without key or writer; No-Response suppresses the classes of its bits, also at the proxy, a CON gets an empty ACK. Against DDS readers of other vendors the dispose is not measured.

### §4.3 GET + Observe (RFC 7641) → DDS→CoAP push

**Spec:** §4.3 — GET with `Observe:0` registers, notify per sample with
`Observe:<seq>`; cancel via `Observe:1` or RST.

**Repo:** `crates/coap-bridge/src/daemon/observe.rs` (registry, notifications, retransmission, Reset, ping, deadline), `crates/coap-bridge/src/daemon/server.rs::serve_get`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset, coapbridge_6_replay_lifespan_and_deadline, coapbridge_6_the_ping_and_the_bound_of_unconfirmed_notifications}`, `crates/coap-bridge/src/daemon/observe.rs::tests::{coap_4_2_a_confirmable_message_is_retransmitted_with_back_off, rfc7641_3_4_the_sequence_number_is_24_bits, rfc7641_4_5_the_bound_of_unconfirmed_notifications, rfc7641_4_1_an_observer_is_named_by_endpoint_and_token}`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_observe_register_returns_initial_content_with_observe_option`.

**Status:** done — a notification per DDS sample in the form of the registration (XCDR1 from the DDS writer, JSON at the observer); CON for RELIABLE with retransmission, a newer one replaces the one in flight; a Reset, an unacknowledged CON notification, an unanswered ping and a missed deadline end the observation; VOLATILE gives nothing first; `observe_max_age_secs` makes a CON notification after a second without an ACK.

### §4.4 Block-wise transfer (RFC 7959)

**Spec:** §4.4 — Block1 (POST) + Block2 (notify); `block_size` (SZX
16..1024); defragmentation cap.

**Repo:** `crates/coap-bridge/src/blockwise.rs`, `crates/coap-bridge/src/daemon/server.rs::{serve_write, representation, content_response}`, `crates/coap-bridge/src/daemon/observe.rs::notify_one`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{rfc7959_block_size_and_the_limits_of_a_transfer, coapbridge_12_2_a_4_kib_sample_by_block1_and_back_by_block2}`, `crates/coap-bridge/src/blockwise.rs::tests` (10), `crates/coap-bridge/src/daemon/server.rs::tests::{coap_11_3_an_unauthenticated_response_is_sliced_into_small_blocks, rfc7959_2_2_the_block_size_as_szx}`, `crates/coap-bridge/tests/daemon_e2e.rs::{coapbridge_4_4_block1_chunked_post_completes_with_2_04_changed, coapbridge_4_4_block1_out_of_order_returns_2_31_continue_only_in_order}`.

**Status:** done — `block_size` 16 cuts into blocks of 16 octets, a block out of order is `4.08`, a transfer beyond `max_body_size` `4.13` with Size1; 4 KiB in by Block1 and back by Block2.

### §4.5 Content-format registry 65000/65001/65002/50/60

**Spec:** §4.5 — vendor range 65000-65535 for CDR2-LE/BE + CDR1-LE;
`50=application/json`, `60=application/cbor`.

**Repo:** `crates/coap-bridge/src/daemon/representation.rs` (`Form`, `ContentFormats`, `Extensibility`, `TopicType`), `crates/coap-bridge/src/content_format.rs`, `crates/coap-bridge/src/daemon/server.rs::Core::represent`.

**Tests:** `crates/coap-bridge/src/daemon/representation.rs::tests::{coapbridge_4_5_the_five_forms_and_their_numbers, coapbridge_4_5_the_identifier_names_the_form, xtypes_7_4_the_encoding_of_a_form_follows_the_extensibility, coapbridge_4_5_every_form_carries_the_same_sample, rfc8949_6_2_the_cbor_form_is_the_json_value_in_cbor}`, `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form`, `crates/coap-bridge/src/content_format.rs::tests::coap_12_3_the_initial_registry_entries`.

**Status:** done — the five forms with their numbers; every form carries the same sample (octets of the DHEADER and the alignment checked), the DDS side under `D_CDR2_LE`.

## §5 Topic mapping

### §4.5a Content-format values 65001/65002/50/60

**Spec:** §4.5 — CDR2-BE (65001), CDR1-LE (65002), `50=application/json`,
`60=application/cbor`.

**Repo:** `crates/coap-bridge/src/daemon/representation.rs`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form, coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset}`, `crates/coap-bridge/src/daemon/representation.rs::tests::coapbridge_4_5_every_form_carries_the_same_sample`.

**Status:** done — 65001, 65002, 50 and 60 in and out, checked at the DDS reader and from the DDS writer.

### §5.1 Slug algorithm DDS → CoAP URI

**Spec:** §5.1 — lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; override per
`coap_uri_path`.

**Repo:** `crates/coap-bridge/src/uri.rs`,
`crates/coap-bridge/src/daemon/config.rs`.

**Tests:** `crates/coap-bridge/src/uri.rs::tests` (11 tests):
`coap_6_1_parses_basic_coap_uri`, `coap_6_1_parses_coaps_default_port`, `coap_6_1_parses_explicit_port`,
`coap_6_1_parses_query_params`, `coap_6_1_parses_empty_path_when_no_slash`,
`coap_6_1_rejects_unknown_scheme`, `coap_6_1_rejects_missing_host`, `coap_6_1_rejects_invalid_port`,
`coap_6_1_rejects_fragment`, `coap_1_1_default_port_returns_5684_for_coaps`,
`coap_6_4_an_empty_segment_is_a_uri_path_option`;
the slug `crates/coap-bridge/src/daemon/config.rs::tests::{slug_handles_double_colon, slug_replaces_unsafe_chars}` and
`crates/coap-bridge/tests/example_yaml_loadback.rs::coapbridge_5_1_example_yaml_topics_reach_the_daemon`.

**Status:** done

### §5.2 /.well-known/core (RFC 6690) catalog

**Spec:** §5.2 — `/.well-known/core` returns a link-format resource list
with `rt="dds.topic"`, `ct=65000`, `type="..."`.

**Repo:** `crates/coap-bridge/src/daemon/server.rs::{render_well_known_core, serve_schema}`, `crates/coap-bridge/src/core_link.rs`, `crates/coap-bridge/src/daemon/representation.rs::TopicType::idl`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form`, `crates/coap-bridge/src/daemon/representation.rs::tests::coapbridge_5_2_the_schema_is_the_idl_of_the_type`, `crates/coap-bridge/tests/contract_idl_compose.rs::idl_7_3_a_file_compiles_with_its_includes_from_its_own_directory`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_well_known_core_returns_link_format_catalog`.

**Status:** done — the catalog names `ct="65000 65001 65002 50 60"`, `type="Sensor::Reading"` and the link `</schema/…>;rt="dds.schema"`; `GET /schema/<slug>` gives the IDL, which compiles back into the type; a POST to it is `4.05`.

## §6 QoS translation

### §6 DDS-QoS → CoAP-behavior map

**Spec:** §6 — Reliable→CON, BestEffort→NON, Volatile/TransientLocal,
Lifespan→`Max-Age`, Deadline→`5.03`, Liveliness→ping, Partition→filter.

**Repo:** `crates/coap-bridge/src/daemon/qos_translation.rs::topic_behavior`, `crates/coap-bridge/src/daemon/observe.rs`, `crates/coap-bridge/src/daemon/server.rs::register_topic`.

**Tests:** `crates/coap-bridge/src/daemon/qos_translation.rs::tests::{coapbridge_6_reliability_picks_the_message_type, coapbridge_6_durability_and_history_set_the_replay, coapbridge_6_lifespan_and_deadline}`, `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_6_replay_lifespan_and_deadline, coapbridge_6_the_ping_and_the_bound_of_unconfirmed_notifications, coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset}`, `crates/coap-bridge/src/daemon/observe.rs::tests::coapbridge_6_the_max_age_is_what_is_left_of_the_lifespan`.

**Status:** done — RELIABLE → CON, BEST_EFFORT → NON; TRANSIENT_LOCAL with KEEP_LAST(3) gives the last three; Max-Age from `lifespan_ms`; `5.03` with Max-Age 0 on a missed `deadline_ms` to GET and observers; the ping every `keep_alive_secs`. Partition and liveliness lease go to the DDS endpoints; their effect on the DDS side is not checked by itself.

## §7 Security

### §7.1 DTLS coaps:// + cipher suites

**Spec:** §7.1 — "`coaps://` mode is enabled via `coap.dtls.enabled: true`";
the modes PreSharedKey (`coap.dtls.psk`), RawPublicKey (`rpk_key_file`,
`rpk_trusted` with RFC 6920 names) and Certificate (`cert_file`, `key_file`,
`ca_file`); the cipher suites `TLS_PSK_WITH_AES_128_CCM_8`,
`TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` and `TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256`;
"Cert rotation via SIGHUP (the daemon re-reads cert/key, sessions are kept)".
The subject of a DTLS client for §7.2/§7.3 is `psk:<identity>`, the
certificate's DN, or `rpk:` with the `ni` URI of the raw public key.

**Repo:** `crates/coap-bridge/src/dtls.rs` — `DtlsMode`, `DtlsCipherSuite` with
the three §7.1 suites and their IANA code points, `PskStore`,
`parse_secret_hex`, `DtlsCredentials` (with `rpk_key_file`, `rpk_trusted`).
`crates/coap-bridge/src/dtls_transport.rs` (feature `dtls`) on
`crates/dtls-openssl` (OpenSSL 3.2 or later, ADR 0014) — `server_config`,
`DtlsCoapServer::{bind_with, rotate, accept}`, `DtlsCoapClient::{connect,
connect_psk, connect_verified, connect_rpk}`. `crates/coap-bridge/src/daemon/config.rs`
reads the `coap.dtls` block and refuses unknown suites and identifiers that
are no RFC 6920 names; `crates/coap-bridge/src/daemon/server.rs`
(`spawn_dtls_server`, `dtls_session`, `dtls_subject`) runs a thread per
session, rotates on the SIGHUP `reload_flag` and forms the subject.

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
`crates/coap-bridge/tests/security_e2e.rs` (option-65000 auth).

**Status:** done — the three modes, the three suites of §7.1, the `ca_file`
check of client certificates, `rpk_trusted`, the SIGHUP rotation and the
subjects are built and tested; checked against libcoap 4.3.4 (GnuTLS: raw
public keys in both directions and PSK; OpenSSL: PSK and certificate).

### §7.2 OSCORE (RFC 8613)

**Spec:** §7.2 — master-secret/salt/ID-context, per-`kid` HKDF sender/
recipient context, replay window 32; daemon refuses unprotected requests
with `4.01`, protects responses (§8.3) and Observe notifications with its
own Partial IV (§4.1.3.5.2); fail-closed without the `oscore` feature.

**Repo:** `crates/coap-bridge/src/oscore/` — `crates/coap-bridge/src/oscore/mod.rs` (security context +
HKDF key derivation, RFC 8613 §3.2 / RFC 5869), `crates/coap-bridge/src/oscore/aead.rs` (AES-CCM-16-64-
128, nonce §5.2, AAD §5.4), `crates/coap-bridge/src/oscore/message.rs` (protect/unprotect for requests
§8.1/§8.2 and responses §8.3/§8.4, option class split §4.1.2), `crates/coap-bridge/src/oscore/wire.rs`
(OSCORE option codec §6.1 + anti-replay window §3.2.2, default width 32);
`crates/coap-bridge/src/daemon/config.rs` (`oscore:` block, spec schema),
`crates/coap-bridge/src/daemon/oscore_gateway.rs` (per-`kid` server state),
`crates/coap-bridge/src/daemon/server.rs` (unprotect/protect in `dispatch`
+ protected notify path).

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

**Status:** done — full OSCORE implementation including the daemon wiring
(ADR 0010); correctness anchored byte-exact to the RFC 8613 Appendix C
vectors C.1-C.8.

### §7.3 ACL per topic

**Spec:** §7.3 — subject = vendor-auth-token ID (CoAP option 65000) or a
cert subject DN.

**Repo:** `crates/coap-bridge/src/daemon/config.rs` (ACL fields),
`crates/coap-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs`;
`crates/coap-bridge/src/dtls_transport.rs::DtlsCoapSession::peer_identity`
(the DTLS identity for `auth.mode: mtls`: the client certificate's
subject DN per RFC 4514 — a rule may name the fingerprint `mtls:<SHA-256>`
too — or `psk:<identity>`). Before, the DTLS
identity did not reach the request path; `mtls` answered every request 4.01
(from the code). `auth.jwt` as in the WS bridge.

**Tests:** `crates/coap-bridge/tests/daemon_dtls_e2e.rs::coapbridge_7_3_mtls_takes_the_psk_identity_as_the_acl_subject`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::coapbridge_7_3_mtls_without_dtls_is_a_config_error`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::coapbridge_7_3_mtls_takes_the_certificate_dn_as_the_acl_subject`
(`CN=sensor-9` 2.04, `CN=other` of the same CA 4.03),
`crates/coap-bridge/src/daemon/security.rs::tests::coapbridge_7_3_jwt_without_a_public_key_is_a_config_error`,
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_7_3_acl_write_deny_yields_4_03_forbidden`,
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_7_3_acl_observe_register_deny_yields_4_03`,
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_7_3_auth_bearer_token_in_option_is_accepted`,
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_7_3_auth_missing_bearer_token_yields_4_01_unauthorized`;
`crates/coap-bridge/src/daemon/security.rs::tests::coapbridge_7_3_coap_acl_subscribe_check`,
`::coapbridge_7_3_coap_bearer_in_option_accepted`, `::coapbridge_7_3_coap_missing_option_with_bearer_rejected`;
`crates/coap-bridge/tests/example_yaml_loadback.rs::coapbridge_7_3_example_yaml_acl_reaches_the_daemon`.

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** `crates/coap-bridge/src/daemon/log.rs`, `crates/coap-bridge/src/daemon/server.rs::finish`, `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/src/daemon/log.rs::tests::{coapbridge_8_1_a_line_is_a_json_object_with_the_spec_fields, rfc3339_5_6_the_utc_date_and_time, coapbridge_8_1_the_log_level_is_the_threshold}`, `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_8_1_8_3_the_log_lines_and_the_spans_of_an_exchange`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_9_2_the_binary_exits_0_on_sigterm`.

**Status:** done — every line is a JSON object with `timestamp`, `level`, `event` and the fields of §8.1; checked through the binary with `log_level: debug`.

### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + 10 counter/gauge families.

**Repo:** `crates/coap-bridge/src/daemon/runtime_common.rs::{BridgeMetrics, family, serve_admin_endpoints}`, `crates/coap-bridge/src/daemon/config.rs` (`metrics.path`), `crates/dtls-openssl/src/transport.rs::Server::failed_handshakes`.

**Tests:** `crates/coap-bridge/src/daemon/runtime_common.rs::tests::{coapbridge_8_2_admin_endpoint_serves_metrics, coapbridge_8_2_metrics_register_idempotent, coapbridge_8_2_catalog_render_contains_topic_fields}`, `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form`.

**Status:** done — the ten families with their labels at `metrics.path`; counted in the end-to-end run.

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables span emission.

**Repo:** `crates/coap-bridge/src/daemon/runtime_common.rs::{Tracer, ExchangeSpan, otlp_config_from_env, spawn_otlp_flush_loop}`, `crates/coap-bridge/src/daemon/server.rs::finish`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_8_1_8_3_the_log_lines_and_the_spans_of_an_exchange`.

**Status:** done — with `OTEL_EXPORTER_OTLP_ENDPOINT` a span `coap.request` with its child `dds.write` reaches a collector in the test, at the latest at the shutdown.

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → DTLS → DCPS → reader/writer → UDP bind 5683/5684 →
signal handler.

**Repo:** `crates/coap-bridge/src/daemon/mod.rs`,
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_2_post_to_configured_path_returns_2_04_changed`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, observer deregister, cleanup;
SIGHUP TLS+ACL reload.

**Repo:** `crates/coap-bridge/src/daemon/server.rs::DaemonHandle::shutdown`,
`::end_observations` (the pumps first, then every observer gets a
notification 5.03 without Observe option with its token, RFC 7641 §4.2;
before, the shutdown dropped the registry without a message),
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs` (waits for the stop
flag; the binary was still running 3 s after SIGTERM before, measured),
`::reload` (ACL and auth, DTLS material through the DTLS reload).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_9_2_the_shutdown_ends_every_observation_with_5_03`,
`::coapbridge_9_2_the_binary_exits_0_on_sigterm`;
`crates/coap-bridge/tests/security_e2e.rs::coapbridge_9_2_sighup_applies_the_reloaded_acl`.

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + libcoap/californium/aiocoap

**Spec:** §10 — the daemon is a normal RTPS peer; the CoAP side against
libcoap/californium/aiocoap/Eclipse-Wakaama.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`, `crates/coap-bridge/tests/cross_vendor.rs`, `crates/coap-bridge/tests/interop/{aiocoap_client.py, CfClient.java, reading.idl}`, `tests/integration/provision.sh` (`aiocoap`, `californium`, `wakaama`), `ci/integration_scenarios.py` (`integration-bridges-vendor`).

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::{coapbridge_10_libcoap_post_to_zerodds_daemon_returns_2_04_changed, coapbridge_10_libcoap_get_well_known_core_returns_catalog, coapbridge_10_libcoap_observe_receives_notification, coapbridge_10_aiocoap_posts_reads_and_observes_json, coapbridge_10_californium_posts_reads_and_observes_json, coapbridge_10_wakaama_registration_is_read_and_answered}` (`#[ignore]`, feature `cross-vendor-tests`, weekly in `integration-bridges-vendor`); the RTPS side `crates/coap-bridge/tests/topic_e2e.rs::coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form`.

**Status:** done — measured on the Linux test host against libcoap 4.3.4, aiocoap 0.4.14, Eclipse Californium 3.14.0 and Eclipse Wakaama `snapshots/2026-05-26` (7 of 7 tests of `cross_vendor.rs`): POST, GET and an observation in JSON with aiocoap and Californium; Wakaama's registration (`POST /rd`) is read and answered with `4.05`, Wakaama reads the answer with its Message ID and does not send again.

### §10a Client matrix Californium/aiocoap

**Spec:** §10 — the CoAP side runs against Californium and aiocoap.

**Repo:** `crates/coap-bridge/tests/cross_vendor.rs`, `crates/coap-bridge/tests/interop/{aiocoap_client.py, CfClient.java}`.

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::{coapbridge_10_aiocoap_posts_reads_and_observes_json, coapbridge_10_californium_posts_reads_and_observes_json}`.

**Status:** done — Californium 3.14.0 and aiocoap 0.4.14 measured against the daemon (see §10).

---

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-coap-bridged`; configs/services/Docker;
manuals.

**Repo:** `packaging/linux/systemd/zerodds-coap-bridged.service`,
`packaging/macos/launchd/org.zerodds.coap-bridged.plist`,
`packaging/macos/homebrew/zerodds-coap-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/coap-bridged/`,
`packaging/linux/configs/coap-bridged.yaml.example`,
`man/man1/zerodds-coap-bridged.1`, `man/man5/zerodds-coap-bridged.yaml.5`.

**Tests:** `crates/coap-bridge/tests/example_yaml_loadback.rs::{coapbridge_3_example_yaml_parses_without_error, coapbridge_3_example_yaml_key_fields_match_intent, coapbridge_5_1_example_yaml_topics_reach_the_daemon, coapbridge_7_3_example_yaml_acl_reaches_the_daemon, coapbridge_11_example_yaml_has_no_unknown_top_level_keys}`
(the shipped example configuration through the real parser, with the values it states);
`tools/traceability/tests/bridge_config_manpages.rs::{every_example_key_is_documented_in_the_man_page, the_man_page_names_no_key_the_example_does_not_declare, the_pre_spec_vocabulary_is_gone_from_pages_and_examples, every_man_page_names_its_parser_and_what_an_unknown_key_does}`
(the manual page and the example file name the same keys).

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/coap_codec/block_assembler/observe_table/dtls/oscore/dds_pump,
≥ 5 tests each.

**Repo:** `crates/coap-bridge/src/{daemon/config.rs,codec.rs,message.rs,option.rs,blockwise.rs,observe.rs,reliability.rs,uri.rs,core_link.rs,bridge.rs,dtls.rs,multicast.rs,matching.rs,caching_proxy.rs,method_props.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` per module:
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
plus `crates/coap-bridge/tests/fuzz_smoke.rs::coap_11_1_fuzz_coap_decode_no_panic`,
`::coapbridge_12_1_empty_input_returns_err_not_panic`, `::coapbridge_12_1_single_byte_inputs_no_panic`.

**Status:** done — every module has at least five tests; the pump is `crates/coap-bridge/src/daemon/observe.rs` (`on_sample`, `notify_one`, `tick`) with five unit tests and the end-to-end tests of §12.2.

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, libcoap client, POST/Observe/Block
round-trip.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`, `crates/coap-bridge/tests/topic_e2e.rs`.

**Tests:** `crates/coap-bridge/tests/topic_e2e.rs::{coapbridge_4_5_every_form_goes_to_dds_and_comes_back_in_every_form, coapbridge_4_3_a_dds_sample_is_a_confirmable_notification_until_a_reset, coapbridge_12_2_a_4_kib_sample_by_block1_and_back_by_block2, coapbridge_8_1_8_3_the_log_lines_and_the_spans_of_an_exchange}`, `crates/coap-bridge/tests/daemon_e2e.rs::coapbridge_9_2_the_binary_exits_0_on_sigterm`.

**Status:** done — a DDS reader receives the sample of a POST, a DDS writer makes a notification, 4 KiB by Block1/Block2, the binary as a subprocess (SIGTERM, log, spans); libcoap as the peer is in §12.3.

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone-DDS subscriber + libcoap/californium client in
compose.

**Repo:** `crates/coap-bridge/tests/cross_vendor.rs`, `crates/coap-bridge/tests/interop/{cyclone_reading.c, reading.idl}`, `tests/integration/provision.sh` (`cyclonedds-dev`).

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::coapbridge_12_3_a_coap_post_reaches_cyclone_and_a_cyclone_sample_an_observer`, `crates/coap-bridge/tests/cross_vendor.rs::coapbridge_10_libcoap_post_to_zerodds_daemon_returns_2_04_changed`.

**Status:** done — measured against Cyclone DDS 0.10.5 and libcoap 4.3.4: a JSON sample a libcoap POST sends reaches the Cyclone reader as that sample, a sample of the Cyclone writer reaches libcoap's observation as JSON. Instead of Docker Compose the weekly job provisions the programs.

## §13 Cross-references

### §13 Related library + RFCs + daemons

**Spec:** §13 — library `crates/coap-bridge/`, RFC 7252/7641/7959/8613, wire
format, deployment, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config (e.g. content-format
IDs), major=wire-protocol change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — rule for assigning the version number of a release, not a property of the daemon.

---

## Audit status

26 done / 0 partial / 0 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9860 of `main` (`python3 ci/coverage_results.py --document zerodds-coap-bridge-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets` (job `merge-rust-coap-bridge`) — 110 tests green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets --features dtls,oscore` (job `merge-rust-coap-bridge-features-dtls-oscore`) — 23 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor` (job `integration-bridges-vendor`) — 3 tests green.

140 cited tests green, 4 not green.

