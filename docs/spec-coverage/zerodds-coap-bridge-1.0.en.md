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

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-coap-bridged CLI

**Spec:** §2 — options
`--config`/`--bind`/`--domain`/`--dtls-*`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/coap-bridge/src/daemon/cli.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::well_known_core_returns_link_format_catalog`,
`::observe_register_returns_initial_content_with_observe_option`,
`::unknown_path_returns_bad_request`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level
`domain`/`coap`/`oscore`/`topics`/`content_format`/`acl`/`metrics`; ENV
substitution.

**Repo:** `crates/coap-bridge/src/daemon/config.rs`,
`crates/config/src/parse.rs` (the shared parser), `crates/coap-bridge/src/daemon/mod.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::make_test_config`
(config construction). Inline tests in `crates/coap-bridge/src/daemon/config.rs::tests`.

**Status:** done

## §4 CoAP wire protocol

### §4.1 RFC-7252 header + token + options

**Spec:** §4.1 — a 4-byte header (Ver/T/TKL/Code/Message-ID), token,
options, payload marker `0xFF`.

**Repo:** `crates/coap-bridge/src/codec.rs`,
`crates/coap-bridge/src/message.rs`, `crates/coap-bridge/src/option.rs`,
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

### §4.2 POST/PUT/DELETE → DDS write/dispose

**Spec:** §4.2 — POST → DDS write (`2.04 Changed`), PUT idempotent, DELETE
→ dispose; `4.00`/`4.13`/`5.00` error mapping.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/bridge.rs`, `crates/coap-bridge/src/method_props.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::unknown_path_returns_bad_request`.

**Status:** done

### §4.3 GET + Observe (RFC 7641) → DDS→CoAP push

**Spec:** §4.3 — GET with `Observe:0` registers, notify per sample with
`Observe:<seq>`; cancel via `Observe:1` or RST.

**Repo:** `crates/coap-bridge/src/observe.rs`,
`crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::observe_register_returns_initial_content_with_observe_option`.

**Status:** done

### §4.4 Block-wise transfer (RFC 7959)

**Spec:** §4.4 — Block1 (POST) + Block2 (notify); `block_size` (SZX
16..1024); defragmentation cap.

**Repo:** `crates/coap-bridge/src/blockwise.rs`,
`crates/coap-bridge/src/reliability.rs`,
`crates/coap-bridge/src/option.rs` (Block1/Block2 options),
`crates/coap-bridge/src/daemon/server.rs` (block wire-up).

**Tests:** `crates/coap-bridge/src/blockwise.rs::tests` (10 tests):
`block_size_matches_spec_szx_table`, `szx_7_rejected`,
`round_trip_small_block_value`, `round_trip_large_block_value`,
`num_too_large_rejected`, `decode_rejects_bad_length`,
`reassembler_combines_blocks_in_order`, `out_of_order_block_rejected`,
`intermediate_block_size_must_match`, `final_block_can_be_smaller`;
`crates/coap-bridge/tests/daemon_e2e.rs::block1_chunked_post_completes_with_2_04_changed`,
`crates/coap-bridge/tests/daemon_e2e.rs::block1_out_of_order_returns_2_31_continue_only_in_order`.

**Status:** done

### §4.5 Content-format registry 65000/65001/65002/50/60

**Spec:** §4.5 — vendor range 65000-65535 for CDR2-LE/BE + CDR1-LE;
`50=application/json`, `60=application/cbor`.

**Repo:** `crates/coap-bridge/src/option.rs` (content-format tags),
`crates/coap-bridge/src/bridge.rs` (CDR decoder).

**Tests:** `crates/coap-bridge/src/option.rs::tests::content_format_uses_number_12`,
`crates/coap-bridge/src/codec.rs::tests::representation_carries_content_format_per_spec_5_5`,
`crates/coap-bridge/src/codec.rs::tests::diagnostic_payload_of_error_response_carries_no_content_format`;
the value 65000 is set by `crates/coap-bridge/src/daemon/server.rs` and read
back by `crates/coap-bridge/tests/daemon_e2e.rs::observe_register_returns_initial_content_with_observe_option`.
65001, 65002, 50 and 60 do not exist as a registry — see §4.5a.

**Status:** partial — the option itself and the CDR2-LE value 65000 are covered; the
four other values are carried as §4.5a `open`.
## §5 Topic mapping

### §4.5a Content-format values 65001/65002/50/60

**Spec:** §4.5 — CDR2-BE (65001), CDR1-LE (65002), `50=application/json`,
`60=application/cbor`.

**Repo:** —

**Tests:** —

**Status:** open — the part of §4.5 beyond 65000. The tree carries no
registry type and no test for the four values.

### §5.1 Slug algorithm DDS → CoAP URI

**Spec:** §5.1 — lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; override per
`coap_uri_path`.

**Repo:** `crates/coap-bridge/src/uri.rs`,
`crates/coap-bridge/src/daemon/config.rs`.

**Tests:** `crates/coap-bridge/src/uri.rs::tests` (11 tests):
`parses_basic_coap_uri`, `parses_coaps_default_port`, `parses_explicit_port`,
`parses_query_params`, `parses_empty_path_when_no_slash`,
`rejects_unknown_scheme`, `rejects_missing_host`, `rejects_invalid_port`,
`rejects_fragment`, `default_port_returns_5684_for_coaps`,
`round_trip_path_filtering_strips_empty_segments`;
the override path `crates/coap-bridge/src/daemon/config.rs::tests` and
`crates/coap-bridge/tests/example_yaml_loadback.rs::example_yaml_topics_reach_the_daemon`.

**Status:** done

### §5.2 /.well-known/core (RFC 6690) catalog

**Spec:** §5.2 — `/.well-known/core` returns a link-format resource list
with `rt="dds.topic"`, `ct=65000`, `type="..."`.

**Repo:** `crates/coap-bridge/src/core_link.rs`,
`crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::well_known_core_returns_link_format_catalog`.

**Status:** done

## §6 QoS translation

### §6 DDS-QoS → CoAP-behavior map

**Spec:** §6 — Reliable→CON, BestEffort→NON, Volatile/TransientLocal,
Lifespan→`Max-Age`, Deadline→`5.03`, Liveliness→ping, Partition→filter.

**Repo:** `crates/coap-bridge/src/reliability.rs`,
`crates/coap-bridge/src/observe.rs`, `crates/coap-bridge/src/bridge.rs`,
`crates/coap-bridge/src/daemon/qos_translation.rs` (cluster-A QoS map
Reliability/Durability/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`
(reliable POST→write); a QoS matrix in
`crates/coap-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 DTLS coaps:// + cipher suites

**Spec:** §7.1 — "`coaps://`-Mode aktiviert über `coap.dtls.enabled: true`";
the cipher suites §7.1 recommends are `TLS_PSK_WITH_AES_128_CCM_8`
(PSK mode, RFC 7252 §9.1.3), `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` (cert mode)
and `TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` (hybrid); "Cert-Rotation via SIGHUP
(Daemon liest cert/key neu, Sessions halten)". The config schema (§3) names
`coap.dtls.cert_file`/`key_file`/`ca_file` and the `psk` list of `identity` +
`secret_hex`, the CLI (§3) `--dtls-psk-id`/`--dtls-psk`/`--dtls-cert`/
`--dtls-key`. Auth+ACL run over vendor option 65000 (CoAP application auth
token).

**Repo:** `crates/coap-bridge/src/dtls.rs` — `DtlsMode` (RFC 7252 §9.1),
`DtlsCipherSuite` with the three §7.1 suites and their IANA code points,
`PskStore`/`PskEntry` (lookup per identity, identity hint per RFC 4279 §5.2),
`parse_secret_hex` and `DtlsCredentials` with
`effective_cipher_suites`/`uses_psk`. `crates/coap-bridge/src/dtls_transport.rs`
(feature `dtls`) — `load_certificate` reads `cert_file`+`key_file` as PEM
(PKCS#8; a SEC1/PKCS#1 file is refused with the conversion named),
`load_client_cas` turns `ca_file` into a trust store and switches on
`RequireAndVerifyClientCert`, `server_config` builds either the PSK or the cert
handshake through `supported_cipher_suites`, `DtlsCoapServer::bind_with` keeps
the UDP listener and the DTLS configuration apart, `rotate` swaps the
configuration used by the next handshake on the same socket,
`DtlsCoapClient::connect` / `connect_psk`.
`crates/coap-bridge/src/daemon/config.rs` reads the `coap.dtls` block
(`cert_file`, `key_file`, `ca_file`, `cipher_suites`, `psk`) and rejects
unknown suite names, missing `secret_hex` fields and a `cert_file` without its
`key_file`; `crates/coap-bridge/src/daemon/cli.rs` +
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs` wire up the four DTLS
flags; `crates/coap-bridge/src/daemon/server.rs`
(`spawn_dtls_server`/`dtls_accept_loop`) hands the credentials to the listener
and performs the rotation on the SIGHUP `reload_flag`;
`crates/coap-bridge/src/daemon/security.rs` (option-65000 auth wire-up).

**Tests:** inline in `crates/coap-bridge/src/dtls.rs` —
`cipher_suite_names_and_code_points_match_iana`,
`psk_store_looks_up_by_identity_and_refuses_short_secrets`,
`secret_hex_round_trips_and_rejects_garbage`,
`credentials_pick_the_mode_they_were_configured_for` plus the
`validate_dtls_mode` cases; `crates/coap-bridge/tests/dtls_coap_e2e.rs`
(feature `dtls`) — `dtls_coap_get_content_roundtrip` (handshake + CoAP-GET →
2.05 Content), `dtls_psk_handshake_carries_coap` (PSK mode, no certificate),
`dtls_psk_unknown_identity_is_refused`,
`dtls_certificate_files_and_rotation_keep_sessions_alive` (cert/key from disk,
SIGHUP rotation: a new client handshakes against the new certificate while the
session opened before the rotation keeps carrying CoAP),
`dtls_rejects_a_non_pkcs8_key_file`;
`crates/coap-bridge/tests/security_e2e.rs` (option-65000 auth).

**Status:** done — PSK and cert mode including `ca_file` client verification,
the config and CLI path and the SIGHUP cert rotation are implemented and
measured. Of the three suites §7.1 *recommends*, the endpoint offers
`TLS_PSK_WITH_AES_128_CCM_8` and `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8`;
`TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` (0xc037) is not in webrtc-dtls 0.12's
`CipherSuiteId`, and `supported_cipher_suites` refuses to start naming the
suite rather than quietly negotiating a different one.

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

**Tests:** `crates/coap-bridge/tests/daemon_dtls_e2e.rs::mtls_takes_the_psk_identity_as_the_acl_subject`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::mtls_without_dtls_is_a_config_error`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::mtls_takes_the_certificate_dn_as_the_acl_subject`
(`CN=sensor-9` 2.04, `CN=other` of the same CA 4.03),
`crates/coap-bridge/src/daemon/security.rs::tests::jwt_without_a_public_key_is_a_config_error`,
`crates/coap-bridge/tests/security_e2e.rs::acl_write_deny_yields_4_03_forbidden`,
`crates/coap-bridge/tests/security_e2e.rs::acl_observe_register_deny_yields_4_03`,
`crates/coap-bridge/tests/security_e2e.rs::auth_bearer_token_in_option_is_accepted`,
`crates/coap-bridge/tests/security_e2e.rs::auth_missing_bearer_token_yields_4_01_unauthorized`;
`crates/coap-bridge/src/daemon/security.rs::tests::coap_acl_subscribe_check`,
`::coap_bearer_in_option_accepted`, `::coap_missing_option_with_bearer_rejected`;
`crates/coap-bridge/tests/example_yaml_loadback.rs::example_yaml_acl_reaches_the_daemon`.

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/daemon/cli.rs::tests` covers the flag
splitting, but nothing tests the JSON log form or the effect of
`--log-level`.
### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + 10 counter/gauge families.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/daemon/config.rs`,
`crates/coap-bridge/src/daemon/runtime_common.rs` (counter/gauge families
cluster-A wire-up).

**Tests:** `crates/coap-bridge/src/daemon/runtime_common.rs::tests::metrics_register_idempotent`,
`crates/coap-bridge/src/daemon/runtime_common.rs::tests::admin_endpoint_serves_metrics`,
`crates/coap-bridge/src/daemon/runtime_common.rs::tests::catalog_render_contains_topic_fields`.

**Status:** done

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables span emission.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/daemon/runtime_common.rs` builds the
`OtlpConfig`, but nothing tests that `OTEL_EXPORTER_OTLP_ENDPOINT` is read
or that a span is emitted per CoAP exchange.
## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → DTLS → DCPS → reader/writer → UDP bind 5683/5684 →
signal handler.

**Repo:** `crates/coap-bridge/src/daemon/mod.rs`,
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`.

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

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::the_shutdown_ends_every_observation_with_5_03`,
`::the_binary_exits_0_on_sigterm`;
`crates/coap-bridge/tests/security_e2e.rs::sighup_applies_the_reloaded_acl`.

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + libcoap/californium/aiocoap

**Spec:** §10 — the daemon is a normal RTPS peer; the CoAP side against
libcoap/californium/aiocoap/Eclipse-Wakaama.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::libcoap_post_to_zerodds_daemon_returns_2_04_changed`,
`crates/coap-bridge/tests/cross_vendor.rs::libcoap_get_well_known_core_returns_catalog`,
`crates/coap-bridge/tests/cross_vendor.rs::libcoap_observe_receives_notification` (all `#[ignore]`, feature
`cross-vendor-tests`); the RTPS side
`crates/coap-bridge/src/bridge.rs::tests::bridge_apply_write_then_read_round_trip`.
Californium and aiocoap have not been run against the daemon — see §10a.

**Status:** partial — libcoap is measured, Californium and aiocoap are not; the
missing part is carried as §10a `open`.
### §10a Client matrix Californium/aiocoap

**Spec:** §10 — the CoAP side runs against Californium and aiocoap.

**Repo:** —

**Tests:** —

**Status:** open — the part of §10 and §12.3 beyond libcoap.
`crates/coap-bridge/tests/cross_vendor.rs` knows only libcoap.

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

**Tests:** `crates/coap-bridge/tests/example_yaml_loadback.rs::{example_yaml_parses_without_error, example_yaml_key_fields_match_intent, example_yaml_topics_reach_the_daemon, example_yaml_acl_reaches_the_daemon, example_yaml_has_no_unknown_top_level_keys}`
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
plus `crates/coap-bridge/tests/fuzz_smoke.rs::fuzz_coap_decode_no_panic`,
`::empty_input_returns_err_not_panic`, `::single_byte_inputs_no_panic`.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, libcoap client, POST/Observe/Block
round-trip.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::well_known_core_returns_link_format_catalog`,
`::observe_register_returns_initial_content_with_observe_option`,
`::unknown_path_returns_bad_request`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone-DDS subscriber + libcoap/californium client in
compose.

**Repo:** `crates/coap-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
harness).

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs::libcoap_post_to_zerodds_daemon_returns_2_04_changed`,
`crates/coap-bridge/tests/cross_vendor.rs::libcoap_get_well_known_core_returns_catalog`,
`crates/coap-bridge/tests/cross_vendor.rs::libcoap_observe_receives_notification`; the DDS side
`crates/coap-bridge/src/bridge.rs::tests::bridge_apply_write_then_read_round_trip`.
The client matrix beyond libcoap is not measured — see §10a.

**Status:** partial — libcoap and the DDS side are covered, the rest of the client
matrix is not.
## §13 Cross-references

### §13 Related library + RFCs + daemons

**Spec:** §13 — library `crates/coap-bridge/`, RFC 7252/7641/7959/8613, wire
format, deployment, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config (e.g. content-format
IDs), major=wire-protocol change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

18 done / 3 partial / 4 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-coap-bridge --features daemon,dtls,oscore`
— 251 tests green, 0 failed (218 unit tests plus the daemon, DTLS, OSCORE and
security end-to-end suites).

Open items: see `zerodds-coap-bridge-1.0.open.en.md`. Decision records: none.
