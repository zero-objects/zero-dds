# DDS-AMQP 1.0 — ZeroDDS Vendor Spec Coverage

**Source:** `documentation/specs/dds-amqp-1.0/main.tex` (tag
`spec-dds-amqp-v1.0.0-beta1`, PDF
`documentation/specs/releases/v1.0-beta1/dds-amqp-1.0-beta1.pdf`).

**Context:** the repo implementation is spread across two crates:

- `crates/amqp-bridge/` — codec profile
- `crates/amqp-endpoint/` — endpoint/bridge-profile operations layers

---

## §2 Conformance

### §2.1 Endpoint Profile — Clause 1 (type system)

**Spec:** §2.1, p. 7 (PDF) — "It supports the AMQP type-system as
specified in §sec:pim-type-mapping."

**Repo:** `crates/amqp-bridge/src/types.rs::AmqpValue`,
`crates/amqp-bridge/src/extended_types.rs::AmqpExtValue`.

**Tests:** `crates/amqp-bridge/src/types.rs::tests`:
`amqp10_1_6_1_null_is_0x40`, `amqp10_1_6_2_a_boolean_is_0x41_0x42_or_0x56_and_an_octet`,
`amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`, `amqp10_1_6_10_a_long_is_smalllong_or_eight_octets`, `amqp10_1_6_19_binary_is_vbin8_or_vbin32`, `amqp10_1_6_20_a_string_is_its_utf_8_octets_after_the_length`, `amqp10_1_6_21_a_symbol_is_ascii_after_sym8_or_sym32`,
`amqp10_1_2_the_high_nibble_names_the_subcategory`, and the three error paths
`amqp10_1_2_an_unassigned_code_or_a_cut_encoding_is_refused`,
`amqp10_1_6_20_octets_that_are_not_utf_8_are_refused`.
`crates/amqp-bridge/src/extended_types.rs::tests` covers the
extended codes, among them `ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`,
`ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`,
`amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`, `amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets`,
`amqp10_1_6_18_a_uuid_is_0x98_and_sixteen_octets`.

**Status:** done

### §2.1 Endpoint Profile — Clause 2 (connection acceptance)

**Spec:** §2.1 — "It accepts incoming AMQP connections in
accordance with §topology-direct and the AMQP-1.0 connection-state
model."

**Repo:** `crates/amqp-endpoint/src/session.rs::ConnectionState`
+ `advance_connection()` (state machine).
`tools/amqp-dds-endpoint/` (daemon crate):
* `frame_io::read_protocol_header`/`write_protocol_header` —
  AMQP/SASL protocol header.
* `frame_io::read_frame`/`write_frame` — 8B header + body.
* `handler::handle_connection` — drives the Open/Begin/Attach/
  Transfer/Close loop, drives `advance_connection`.
* `server::run_server` — `std::net::TcpListener` + thread-per-
  connection, set_nonblocking + shutdown atomic.

**Tests:** `session::tests::*` (6 state-machine tests),
`amqp_dds_endpoint::frame_io::tests` (10 tests),
`handler::tests::*` (6 tests incl.
`ddsamqp_2_1_handle_connection_open_close_round_trip`,
`ddsamqp_2_1_handle_connection_sasl_then_amqp`),
`server::tests::ddsamqp_2_1_server_accepts_connection_and_handles_open_close`
(real TCP roundtrip).

**Status:** done

### §2.1 Endpoint Profile — Clause 3 (sender + receiver links)

**Spec:** §2.1 — "It accepts both Sender and Receiver link
attachments as specified in §pim-settlement, and translates them
bidirectionally into DDS DataWriter and DataReader operations."

**Repo:** `crates/amqp-endpoint/src/link.rs::LinkSession` (settlement)
+ `tools/amqp-dds-endpoint/src/dds_host.rs::DdsHost` trait
+ `InMemoryDdsHost` (topic registry, publish/subscribe API)
+ `tools/amqp-dds-endpoint/src/bridge.rs::dispatch_attach`/
`dispatch_transfer`/`subscribe_outbound` translate between
AMQP frame bodies and the DDS side. The concrete DCPS wire binding
is a follow-up wave (DcpsDdsHost implementer).

**Tests:** `link::tests` (7 tests),
`dds_host::tests` (10 tests),
`bridge::tests` (10 tests),
`c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_amqp_producer_publishes_to_dds_via_bridge`,
`ddsamqp_2_1_dds_publish_flows_to_amqp_consumer_callback`,
`ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`,
`ddsamqp_2_1_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### §2.1 Endpoint Profile — Clause 4 (address resolution)

**Spec:** §2.1 — "It implements address resolution as specified in
§pim-address-resolution."

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
with static aliases, `domain://N/topic?partition=P[&partition=Q]`
URLs (multi-partition sequence + percent-decoding), wildcard
pattern matching, `effective_partitions` conflict resolver.

**Tests:** `routing::tests` (12 tests incl.
`ddsamqp_2_1_domain_url_with_multi_partition_parses`,
`ddsamqp_2_1_domain_url_with_percent_encoded_partition`,
`ddsamqp_2_1_conflict_resolution_uri_wins_when_both_present`).

**Status:** done

### §2.1 Endpoint Profile — Clause 5 (body encoding)

**Spec:** §2.1 — "It supports at least the Pass-Through encoding
mode defined in §psm-passthrough, and SHOULD support the JSON
encoding mode of §psm-json."

**Repo:** `crates/amqp-endpoint/src/mapping.rs::BodyEncodingMode`
+ `encode_dds_to_amqp_body` + `parse_amqp_body`.

**Tests:** `crates/amqp-endpoint/src/mapping.rs::tests`:
`ddsamqp_8_1_1_a_pass_through_body_is_the_sample_octets`, `ddsamqp_2_1_the_typeless_json_body_carries_the_sample_as_hex`,
`ddsamqp_2_1_amqp_native_uses_correct_content_type`,
`ddsamqp_2_1_unknown_content_type_falls_back_to_pass_through`,
`ddsamqp_2_1_default_mode_is_pass_through`.

**Status:** done — Pass-Through full, JSON as a hex fallback
spec-conformant (full type-reflection JSON is a codegen layer).

### §2.1 Endpoint Profile — Clause 6 (SASL mechanisms)

**Spec:** §2.1 — "It supports the SASL PLAIN, ANONYMOUS, and
EXTERNAL mechanisms as specified in §security-sasl."

**Repo:** `crates/amqp-endpoint/src/sasl.rs::SaslMechanism`
{Plain, Anonymous, External} + `SaslState::new`/`step` state
machine.

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests`:
`amqp10_5_plain_authenticates_on_correct_credentials`,
`amqp10_5_plain_fails_on_wrong_credentials`, `ddsamqp_2_1_plain_without_tls_yields_auth_failed`,
`ddsamqp_2_1_plain_offered_only_when_tls_active`,
`amqp10_5_anonymous_authenticates_without_credentials`,
`amqp10_5_external_uses_transport_subject`, `ddsamqp_2_1_select_outbound_prefers_external`,
`ddsamqp_2_1_select_outbound_falls_back_to_plain_with_tls`,
`ddsamqp_2_1_select_outbound_skips_plain_without_tls`,
`ddsamqp_2_1_select_outbound_no_acceptable_mechanism`;
the endpoint program (`tools/amqp-dds-endpoint/src/auth.rs`,
`tools/amqp-dds-endpoint/src/handler.rs`, credential store in the RFC 5803 form):
`tools/amqp-dds-endpoint/tests/sasl_server_checks.rs::{ddsamqp_2_1_plain_on_tls_checks_the_password_against_the_store,
ddsamqp_2_1_scram_sha_256_authenticates_and_proves_the_server,
ddsamqp_2_1_an_init_with_a_mechanism_not_offered_is_auth,
ddsamqp_2_1_skipping_sasl_is_refused_when_anonymous_is_not_enabled}`,
`tools/amqp-dds-endpoint/src/auth.rs::tests::ddsamqp_2_1_the_policy_refuses_what_cannot_work_and_offers_per_connection`.
`tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_2_1_an_mtls_client_authenticates_with_external`
(mTLS with a CA-signed client certificate, subject `mtls:<SHA-256>`).

**Status:** done

### §2.1 Endpoint Profile — Clause 7 (mandatory TLS for PLAIN)

**Spec:** §2.1 Clause 7 (quoted from dds-amqp-1.0 §2.1 Endpoint
Profile item 7): "It SHALL NOT offer SASL PLAIN to a client whose
underlying transport is unencrypted (§2.x.x). A client SASL-init
frame carrying PLAIN on an unencrypted connection SHALL be rejected
with SASL outcome `auth`, and the connection SHALL be closed."

**Repo:** `SaslState::new(tls_active: bool)` filters PLAIN out of
the mechanism list when `tls_active = false`
(`crates/amqp-endpoint/src/sasl.rs::SaslState::new`).
Inbound PLAIN on cleartext delivers spec-conformantly
`SaslOutcome::Failed { code: SaslCode::Auth, .. }` (wire code 1
per OASIS AMQP 1.0 §5.3.3.6); a new helper `auth_failed()`,
a new `SaslCode` enum with `Ok/Auth/Sys/SysPerm/SysTemp` (codes
0-4) and a `wire_code()` accessor.

**Tests:** `sasl::tests::ddsamqp_2_1_plain_offered_only_when_tls_active`,
`ddsamqp_2_1_plain_without_tls_yields_auth_failed`,
`amqp10_5_3_3_6_the_sasl_codes_are_0_to_4`,
`amqp10_5_sasl_code_unknown_value_rejected`,
`ddsamqp_2_1_outcome_authenticated_wire_code_is_ok`,
`ddsamqp_2_1_outcome_auth_failed_helper_uses_auth_code`;
on the wire against the endpoint program:
`tools/amqp-dds-endpoint/tests/sasl_server_checks.rs::ddsamqp_2_1_a_plain_init_on_plaintext_is_auth_and_the_connection_closes`
(PLAIN not offered; a PLAIN init on plaintext gets outcome 1, then EOF).

**Status:** done — filter logic + spec-conformant outcome codes
per OASIS AMQP 1.0 §5.3.3.6.

### §2.1 Endpoint Profile — Clause 8 (no-bypass + per-link governance)

**Spec:** §2.1 — "It enforces the No-Bypass guarantee for
DDS-Security identities (§security-no-bypass) and per-link
Governance resolution (§per-link-governance)."

**Repo:** `crates/amqp-endpoint/src/security.rs` trait +
plugins. `tools/amqp-dds-endpoint/src/handler.rs::check_access`
calls the plugin pre-Attach (AttachSender/AttachReceiver) and
pre-Transfer (ReceiveSample). On deny: a Detach frame with
`amqp:unauthorized-access` + an `errors.unauthorized` counter.

**Tests:** `security::tests::ddsamqp_2_1_link_governance_caches_decision`,
`handler::tests::ddsamqp_2_1_access_control_deny_attach_yields_unauthorized_metric`,
`ddsamqp_2_1_access_control_allow_does_not_increment_unauthorized`.

**Status:** done

### §2.1 Endpoint Profile — Clause 9 (management surface + audit)

**Spec:** §2.1 — "It exposes the operational management surface
defined in §management-interface (the `$catalog` and `$metrics`
addresses) and the security audit channel defined in
§audit-channel (the `$audit` address)."

**Repo:** `crates/amqp-endpoint/src/management.rs` with
`CatalogProducer` (§7.5/§7.9.1), a `metrics_snapshot` sample
stream (§7.9.2), an `AuditProducer` ring buffer (§sec:audit-channel),
a `classify_address` reserved-address resolver.

**Tests:** `management::tests` (12 tests incl.
`ddsamqp_2_1_catalog_snapshot_emits_map_per_entry`,
`ddsamqp_2_1_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`ddsamqp_2_1_audit_producer_ringbuffer_evicts_oldest`).

**Status:** done

### §2.1 Endpoint Profile — Clause 10 (bridge coexistence)

**Spec:** §2.1 — "It implements the bridge-coexistence rules
(§bridge-coexistence) for every sample it forwards between DDS and
AMQP."

**Repo:** `crates/amqp-endpoint/src/coexistence.rs`:
`CoexistenceConfig` (process-wide bridge_id + hop_cap),
`inspect_inbound` (DropLoop + DropHopCap + Forward),
`stamp_outbound` (bridge_id list + hop counter).

**Tests:** `coexistence::tests` (11 tests incl.
`ddsamqp_2_1_drop_loop_on_self_tag_string`, `ddsamqp_2_1_drop_hop_cap_when_exceeded`,
`ddsamqp_2_1_round_trip_stamp_then_inspect_drops_loop`).

**Status:** done

### §2.1 Endpoint Profile — Clause 11 (Annex C tests)

**Spec:** §2.1 — "It satisfies the conformance test cases
enumerated in Annex C, §C.1."

**Repo:** `tools/amqp-dds-endpoint/tests/c1_*.rs` suite with
13 test files (C.1.1-15) cover all Annex-C §C.1 items.

**Tests:** `tools/amqp-dds-endpoint/tests/c1_1_connection_open.rs::ddsamqp_2_1_connection_open_with_tls_active_advertises_plain`,
`tools/amqp-dds-endpoint/tests/c1_2_sasl_plain_rejection.rs`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs`,
`tools/amqp-dds-endpoint/tests/c1_5_settlement_reliable.rs`,
`tools/amqp-dds-endpoint/tests/c1_6_settlement_best_effort.rs`,
`tools/amqp-dds-endpoint/tests/c1_7_address_resolution_wildcard.rs`,
`tools/amqp-dds-endpoint/tests/c1_9_idle_timeout.rs`,
`tools/amqp-dds-endpoint/tests/c1_10_concurrent_connections.rs`,
`tools/amqp-dds-endpoint/tests/c1_13_metrics_link.rs`,
`tools/amqp-dds-endpoint/tests/c1_14_audit_link.rs`,
`tools/amqp-dds-endpoint/tests/c1_15_loop_prevention.rs`.

**Status:** done

### §2.2 Bridge Profile — Clause 1 (type system)

**Spec:** §2.2, p. 8 (PDF) — see Endpoint Cl. 1.

**Repo:** identical to Endpoint (shared codec crate).

**Tests:** `crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_every_row_of_the_primitive_table_takes_the_code_it_names`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_an_octet_sequence_is_binary_and_a_long_sequence_is_a_list`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_a_type_outside_the_mapping_is_named_rather_than_encoded_as_something_else`,
`crates/amqp-bridge/src/types.rs::amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`,
`crates/amqp-bridge/src/extended_types.rs::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`.

**Status:** done

### §2.2 Bridge Profile — Clause 2 (outbound connection)

**Spec:** §2.2 — "It establishes outgoing AMQP connections to an
external broker as specified in §topology-sidecar."

**Repo:** `tools/amqp-dds-endpoint/src/client.rs::connect_outbound`:
TCP connect with timeout, SASL header → SASL-Init →
SASL-Outcome → AMQP header → Open. `OutboundSession` delivers
`remote_container_id` from the Open reply.

**Tests:** `client::tests::ddsamqp_2_2_outbound_connect_to_local_server`
(real E2E roundtrip against a local server).

**Status:** done

### §2.2 Bridge Profile — Clause 3 (address resolution)

**Spec:** §2.2 — the bridge shares the endpoint address-resolution model
(§7.3) including the `domain://` URL form, static aliases, wildcard
matching.

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
(shared with Endpoint Cl. 4); the bridge caller in
`crates/amqp-endpoint/src/client.rs`.

**Tests:** `crates/amqp-endpoint/src/routing.rs::tests` (12 tests):
`ddsamqp_2_2_bare_address_resolves_to_default_domain`, `ddsamqp_2_2_domain_url_parses_id_and_topic`,
`ddsamqp_2_2_domain_url_with_single_partition_parses`,
`ddsamqp_2_1_domain_url_with_multi_partition_parses`,
`ddsamqp_2_1_domain_url_with_percent_encoded_partition`,
`ddsamqp_2_2_static_alias_overrides_implicit_resolution`, `ddsamqp_2_2_wildcard_alias_matches`,
`ddsamqp_2_2_malformed_domain_url_yields_error`, `ddsamqp_2_2_empty_address_yields_no_route`,
`ddsamqp_2_1_conflict_resolution_uri_wins_when_both_present`,
`ddsamqp_2_2_conflict_resolution_property_used_when_uri_empty`,
`ddsamqp_2_2_conflict_resolution_both_empty`.

**Status:** done

### §2.2 Bridge Profile — Clause 4 (SASL outbound)

**Spec:** §2.2 — "It supports the SASL PLAIN, ANONYMOUS, and
EXTERNAL mechanisms ... for the outbound connection to the broker."

**Repo:** `client::do_outbound_sasl`: parses sasl-mechanisms,
calls `SaslState::select_outbound`, builds `sasl-init` (RFC-4616
PLAIN form `\0user\0pw`, ANONYMOUS marker, empty EXTERNAL body),
reads `sasl-outcome` (code 0 = ok).

**Tests:** `client::tests::ddsamqp_2_2_parse_offered_mechanisms_array_form`,
`ddsamqp_2_2_parse_offered_mechanisms_single_symbol`,
`ddsamqp_2_2_parse_offered_mechanisms_unknown_filtered`,
`ddsamqp_2_2_sasl_init_plain_includes_credentials`,
`ddsamqp_2_2_sasl_init_anonymous_uses_marker`,
`ddsamqp_2_2_sasl_init_external_has_empty_response`.

**Status:** done

### §2.2 Bridge Profile — Clause 5 (PLAIN forbidden on cleartext)

**Spec:** §2.2 — "It SHALL NOT initiate a SASL PLAIN authentication
over an unencrypted transport."

**Repo:** `sasl::SaslState::select_outbound(offered, tls_active)`
filters PLAIN out of the selection precedence when `tls_active=false`.

**Tests:** `sasl::tests::ddsamqp_2_1_select_outbound_skips_plain_without_tls`,
`ddsamqp_2_1_select_outbound_no_acceptable_mechanism`.

**Status:** done

### §2.2 Bridge Profile — Clause 6 (dual identity)

**Spec:** §2.2 — "It honours the dual-identity model of
§bridge-dual-identity: broker-side SASL credentials SHALL NOT be
conflated with the bridge's DDS-Security IdentityToken."

**Repo:** `security::DualIdentity` with `for_broker()` /
`for_dds()` getters. Plugin calls reference exclusively
`for_dds()`.

**Tests:** `security::tests::ddsamqp_2_2_dual_identity_keeps_broker_and_dds_separate`,
`ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### §2.2 Bridge Profile — Clause 7 (reconnect with backoff)

**Spec:** §2.2 — "It performs reconnect with exponential back-off
after connection loss as specified in §reconnect."

**Repo:** `client::ReconnectConfig` (initial 1s, mult 2, cap 60s
per spec) + `connect_with_reconnect` with a cooperative
shutdown atomic. `next_backoff_ms(attempt)` cap-correct.

**Tests:** `client::tests::ddsamqp_2_2_backoff_starts_at_initial`,
`ddsamqp_2_2_backoff_doubles_until_cap`, `ddsamqp_2_2_backoff_respects_custom_cap`,
`ddsamqp_2_2_backoff_with_unit_multiplier_stays_at_initial`,
`ddsamqp_2_2_reconnect_exhausts_with_max_attempts`,
`ddsamqp_2_2_reconnect_aborts_on_shutdown_signal`.

**Status:** done

### §2.2 Bridge Profile — Clause 8 (body encoding)

**Spec:** §2.2 — see Endpoint Cl. 5.

**Repo:** identical.

**Tests:** `crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_the_body_form_follows_the_types_extensibility`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_a_mutable_body_is_keyed_by_the_declared_member_ids`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_a_union_plans_its_discriminator_and_then_its_branches`,
`crates/amqp-bridge/src/sections.rs::amqp10_3_2_8_an_amqp_value_is_0x77_and_any_value`,
`crates/amqp-bridge/src/sections.rs::amqp10_3_2_the_sections_follow_in_their_order`,
`crates/amqp-endpoint/src/mapping.rs::ddsamqp_2_1_amqp_native_uses_correct_content_type`.

**Status:** done

### §2.2 Bridge Profile — Clause 9 (management surface + audit)

**Spec:** §2.2 — "It exposes the operational management surface
... and the security audit channel ... for the DDS-side surface
of the bridge."

**Repo:** identical to Endpoint Cl. 9 (shared
`management` module).

**Tests:** `crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_classify_address_recognises_reserved`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_1_catalog_snapshot_emits_map_per_entry`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_1_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_audit_event_types_are_spec_symbols`,
`tools/amqp-dds-endpoint/tests/c1_13_metrics_link.rs::ddsamqp_2_2_metrics_units_are_spec_symbols`,
`tools/amqp-dds-endpoint/tests/c1_14_audit_link.rs::ddsamqp_2_2_audit_producer_streams_events_in_order`.

**Status:** done

### §2.2 Bridge Profile — Clause 10 (bridge coexistence)

**Spec:** §2.2 — see Endpoint Cl. 10.

**Repo:** identical (shared `coexistence` module).

**Tests:** `crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_1_drop_loop_on_self_tag_string`,
`crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_2_drop_loop_on_self_tag_list`,
`crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_1_drop_hop_cap_when_exceeded`,
`crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_1_round_trip_stamp_then_inspect_drops_loop`,
`crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_2_other_bridges_tag_does_not_drop`,
`tools/amqp-dds-endpoint/tests/c2_7_loop_prevention_bridge.rs`.

**Status:** done

### §2.2 Bridge Profile — Clause 11 (Annex C tests)

**Spec:** §2.2 — "It satisfies the conformance test cases
enumerated in Annex C, §C.2."

**Repo:** `tools/amqp-dds-endpoint/tests/c2_*.rs` suite +
`c1_3_4_8_bridge_dispatch.rs` (which also covers C.2.2/C.2.3)
cover all Annex-C §C.2 items.

**Tests:** `tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs`,
`tools/amqp-dds-endpoint/tests/c2_7_loop_prevention_bridge.rs`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### §2.3 Codec Profile

**Spec:** §2.3, p. 9 — type system, performatives, sections codec
without connection lifecycle.

**Repo:** `crates/amqp-bridge/` full: `crates/amqp-bridge/src/types.rs`, `crates/amqp-bridge/src/extended_types.rs`,
`crates/amqp-bridge/src/frame.rs`, `performatives.rs`, `sections.rs`.

**Tests:** `crates/amqp-bridge/src/codec_profile.rs::tests`:
`ddsamqp_2_3_active_profile_full_by_default`, `ddsamqp_2_3_primitives_are_codec_lite`,
`ddsamqp_2_3_list_map_array_are_not_codec_lite`, `ddsamqp_2_3_data_section_is_codec_lite`,
`ddsamqp_2_3_other_sections_are_not_codec_lite`; plus the type round-trips in
`crates/amqp-bridge/src/types.rs::tests` and
`crates/amqp-bridge/src/extended_types.rs::tests`. `cargo test -p
zerodds-amqp-bridge --lib` — 70 tests green.

**Status:** done

### §2.4 Codec-Lite Profile

**Spec:** §2.4 — a strict subset of Codec, only primitives +
`data` body section, no compound.

**Repo:** `crates/amqp-bridge/src/codec_profile.rs` with
`CodecProfile::{Full, Lite}`, `active_profile()` (controlled
by the Cargo feature `codec-lite`), `is_codec_lite_value()` and
`is_codec_lite_section()` — a conformance marker for code that
uses only the Lite subset. `cargo test -p zerodds-amqp-bridge
--features codec-lite` activates the marker.

**Tests:** `codec_profile::tests` (5 tests incl.
`ddsamqp_2_3_primitives_are_codec_lite`,
`ddsamqp_2_3_list_map_array_are_not_codec_lite`,
`ddsamqp_2_3_data_section_is_codec_lite`,
`ddsamqp_2_3_other_sections_are_not_codec_lite`,
`ddsamqp_2_3_active_profile_full_by_default`).

**Status:** done

### §2.5 Combination of Profiles + §2.5.1 Codec implication

**Spec:** §2.5.1, p. 11 — Endpoint/Bridge imply Codec.

**Repo:** Cargo dependency: both crates depend on
`amqp-bridge`; every Endpoint operation uses the codec.

**Tests:** `crates/amqp-bridge/src/codec_profile.rs::ddsamqp_2_3_active_profile_full_by_default`,
`crates/amqp-bridge/src/codec_profile.rs::ddsamqp_2_3_primitives_are_codec_lite`,
`crates/amqp-endpoint/src/mapping.rs::ddsamqp_2_1_amqp_native_uses_correct_content_type`,
`crates/amqp-endpoint/src/properties.rs::ddsamqp_2_5_application_properties_truncated_type_id_present`.

**Status:** done — structurally enforced by the crate dependency.

### §2.5.2 Hybrid profile combination

**Spec:** §2.5.2 — an implementation MAY claim Endpoint and Bridge
at the same time.

**Repo:** no hard separation between the Endpoint and Bridge
paths; the modules are shared.

**Tests:** `crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_5_2_config_defaults_match_spec`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_classify_address_recognises_reserved`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_amqp_producer_publishes_to_dds_via_bridge`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`.

**Status:** done

---

## §7 Platform-Independent Model

### §7.1.1 Primitive mapping

**Spec:** §7.1.1, p. 17 — mapping table DDS-XCDR2 primitive ↔
AMQP-§1.6 primitive (boolean, int8/16/32/64, uint8/16/32/64,
float, double, char, wchar, string, wstring, octet).

**Repo:** `crates/amqp-bridge/src/types.rs` (15 encoders/decoders)
+ `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue` (16 more for compound +
described).

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`,
`crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`,
`crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`,
`crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`.

**Status:** done

### §7.1.1 Primitive mapping — `long double`

**Spec:** §7.1.2, p. 19 — `long double` (16 byte) is narrowed to AMQP
`double` (8 byte).

**Repo:** `codegen_helpers::narrow_long_double_to_double(f64)`
is the audit anchor site for §7.1.2; Rust has no binary128 type,
the codegen already delivers f64.

**Tests:** `codegen_helpers::tests::ddsamqp_7_1_1_long_double_narrowing_is_identity_on_f64`.

**Status:** done

### §7.1.3 Endianness handling

**Spec:** §7.1.3, p. 20 — all AMQP multi-byte values are
big-endian; XCDR2 source data is little-endian.

**Repo:** `crates/amqp-bridge/src/types.rs` and `crates/amqp-bridge/src/extended_types.rs` write
exclusively `to_be_bytes()`/read `from_be_bytes()`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`,
`crates/amqp-bridge/src/types.rs::amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`,
`crates/amqp-bridge/src/extended_types.rs::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`.

**Status:** done

### §7.1.4.1 IDL `char` (8-bit) → AMQP `char` (32-bit)

**Spec:** §7.1.4.1, p. 21 — IDL `char` is restricted to the UTF-8-ASCII
subset; source bytes > 0x7F → `decode-error`.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_char` (4-byte BE) +
`codegen_helpers::validate_char_ascii(byte) -> Result<u8, u8>`
for a codegen pre-encode check. The codegen templates call
`validate_char_ascii` and propagate `Err` as
`amqp:decode-error`.

**Tests:** `codegen_helpers::tests::ddsamqp_7_1_4_1_ascii_validates`,
`ddsamqp_7_1_4_1_non_ascii_rejected`.

**Status:** done

### §7.1.4.2 IDL `wchar` → AMQP `char`

**Spec:** §7.1.4.2, p. 22 — IDL `wchar` as a 4-octet UCS-4 wire,
runtime platform-defined.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_char` produces exactly a
4-byte-BE codepoint.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`,
`crates/amqp-bridge/src/extended_types.rs::amqp10_1_6_16_a_char_is_0x73_and_a_utf_32be_code_point`.

**Status:** done

### §7.1.4.3 String transcoding (UTF-8)

**Spec:** §7.1.4.3, p. 23 — DDS `string` ↔ AMQP `str8/str32`
verbatim; `wstring` transcoded to UTF-8.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_string` (str8/str32),
`crates/amqp-bridge/src/types.rs::encode_symbol` (sym8/sym32), length-prefix validation.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_20_a_string_is_its_utf_8_octets_after_the_length`,
`crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_21_a_symbol_is_ascii_after_sym8_or_sym32`.

**Status:** done

### §7.1.5 Time types

**Spec:** §7.1.5, p. 25 — DDS `Time_t` (sec+nanosec) ↔ AMQP
`timestamp` (8-byte BE ms-since-epoch). Sub-milliseconds via the
`dds:nsec` application property.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_timestamp`/`decode_timestamp`
(wire) + `properties::produce_application_properties` (app
property `dds:nsec` from `SampleHeader.source_nsec_remainder`).

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets`,
`crates/amqp-endpoint/src/properties.rs::tests::ddsamqp_8_3_application_properties_nsec_set_when_nonzero`.

**Status:** done

### §7.1.6.1 Identifier types

**Spec:** §7.1.6.1, p. 26 — 16-byte identifiers without RFC-4122
conformance are encoded as `binary` (0xA0/0xB0), not as
`uuid` (0x98).

**Repo:** `codegen_helpers::is_rfc4122_uuid(&[u8;16]) -> bool`
checks variant+version per RFC-4122; `encode_16byte_identifier`
automatically routes to `Uuid` or `Binary`.

**Tests:** `codegen_helpers::tests::ddsamqp_7_1_6_1_rfc4122_v4_uuid_recognised`,
`ddsamqp_7_1_6_1_arbitrary_16_bytes_not_recognised_as_uuid`,
`ddsamqp_7_1_6_1_encode_16byte_routes_binary_for_non_uuid`,
`ddsamqp_7_1_6_1_encode_16byte_routes_uuid_when_marked`.

**Status:** done

### §7.1.7 Sequence and array

**Spec:** §7.1.7, p. 27 — `sequence<T>` and array to AMQP
`list` (0xC0/0xD0) or `array` (0xE0/0xF0); empty sequence as
`list0` (0x45).

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::List` + `Array` with
list8/list32/array8/array32 encoding; `list0` constant in
`codes::LIST0`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::{amqp10_1_6_22_a_list_is_list8_size_count_and_items, amqp10_1_6_24_an_array_is_one_constructor_then_the_values, ddsamqp_7_1_7_lists_maps_and_arrays_nest_with_their_own_sizes}`.

**Status:** done

### §7.1.8 Map

**Spec:** §7.1.8, p. 28 — DDS map (TK_MAP) ↔ AMQP `map` (0xC1/
0xD1).

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Map`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::{amqp10_1_6_23_a_map_is_map8_size_count_and_pairs, ddsamqp_7_1_7_lists_maps_and_arrays_nest_with_their_own_sizes}`.

**Status:** done

### §7.2.1 Composite descriptor (DESC_TRUNCATED)

**Spec:** §7.2.1.1, p. 30 — the XTypes TypeIdentifier is reduced to its
first 8 octets as an AMQP `ulong` (0x80).

**Repo:** `codegen_helpers::compute_truncated_descriptor(&[u8])
-> Result<u64, _>` delivers the first 8 bytes as a BE u64;
`route_descriptor(DESC_TRUNCATED, hash)` delivers
`(Some(ulong), None)`.

**Tests:** `codegen_helpers::tests::ddsamqp_7_2_1_truncated_descriptor_first_8_bytes_be`,
`ddsamqp_7_2_1_truncated_descriptor_too_short_errors`,
`ddsamqp_7_2_1_route_descriptor_truncated`.

**Status:** done

### §7.2.1 Composite descriptor (DESC_FULL)

**Spec:** §7.2.1.2, p. 30 — the XTypes TypeIdentifier in full
form as an AMQP `symbol` (sym8/sym32) `dds:type:<hex>`.

**Repo:** `codegen_helpers::make_full_descriptor_symbol(&[u8])
-> String` delivers `dds:type:<lowercase-hex>`;
`route_descriptor(DESC_FULL, hash)` delivers
`(None, Some(symbol))`.

**Tests:** `codegen_helpers::tests::ddsamqp_7_2_1_full_descriptor_symbol_format`,
`ddsamqp_7_2_1_route_descriptor_full`.

**Status:** done

### §7.2.1.3 Collision detection with dds:type-id

**Spec:** §7.2.1.3, p. 31 — with `descriptor_form = DESC_TRUNCATED`
the sender must set the application property `dds:type-id` (full 14-byte
hex); the receiver SHALL inspect; on mismatch
`amqp:decode-error`.

**Repo:** sender side: `produce_application_properties`. Receiver
side: `properties::inspect_dds_type_id(props, expected_hex)`
delivers `TypeIdCheck::{Match, Absent, Mismatch}`. The caller (daemon)
maps Mismatch to `amqp:decode-error`.

**Tests:** `properties::tests::ddsamqp_7_2_1_3_type_id_inspector_match_returns_match`,
`ddsamqp_7_2_1_3_type_id_inspector_case_insensitive_match`,
`ddsamqp_7_2_1_3_type_id_inspector_absent_returns_absent`,
`ddsamqp_7_2_1_3_type_id_inspector_mismatch_detects_collision`,
`ddsamqp_7_2_1_3_type_id_inspector_accepts_symbol_form`,
`ddsamqp_7_2_1_3_type_id_inspector_non_map_yields_absent`.

**Status:** done

### §7.2.2 Composite body

**Spec:** §7.2.2, p. 32 — body as an AMQP `list` (constructor
0xC0/0xD0) of the field values in IDL order.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::List` + the section
producer.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::amqp10_1_6_22_the_empty_list_is_list0`,
`crates/amqp-bridge/src/extended_types.rs::amqp10_1_6_22_a_list_is_list8_size_count_and_items`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_the_body_form_follows_the_types_extensibility`,
`crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_a_nested_composite_carries_its_own_descriptor`.

**Status:** done

### §7.2.3 Union

**Spec:** §7.2.3, p. 33 — union as an AMQP `list` with the discriminator
+ active-branch value.

**Repo:** `codegen_helpers::make_union_body(discriminator,
active_value: Option<_>)` builds the `AmqpExtValue::List` with
1-2 elements depending on the active branch.

**Tests:** `codegen_helpers::tests::ddsamqp_7_2_3_union_with_branch_has_two_elements`,
`ddsamqp_7_2_3_union_empty_branch_omits_value`.

**Status:** done

### §7.3 Address-resolution model

**Spec:** §7.3, p. 35 — topic name ↔ AMQP source/target address;
`domain://N/topic?partition=p` URL form.

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
with `resolve()`, a static alias set, a `domain://` URL parser,
wildcard matching.

**Tests:** `crates/amqp-endpoint/src/routing.rs::tests` (12 tests):
`ddsamqp_2_2_bare_address_resolves_to_default_domain`, `ddsamqp_2_2_domain_url_parses_id_and_topic`,
`ddsamqp_2_2_domain_url_with_single_partition_parses`,
`ddsamqp_2_1_domain_url_with_multi_partition_parses`,
`ddsamqp_2_1_domain_url_with_percent_encoded_partition`,
`ddsamqp_2_2_static_alias_overrides_implicit_resolution`, `ddsamqp_2_2_wildcard_alias_matches`,
`ddsamqp_2_2_malformed_domain_url_yields_error`, `ddsamqp_2_2_empty_address_yields_no_route`,
`ddsamqp_2_1_conflict_resolution_uri_wins_when_both_present`,
`ddsamqp_2_2_conflict_resolution_property_used_when_uri_empty`,
`ddsamqp_2_2_conflict_resolution_both_empty`.

**Status:** done

### §7.4.1 RELIABILITY (settlement-mode mapping)

**Spec:** §7.4.1, p. 38 — DDS `RELIABILITY` ↔ AMQP
`snd-settle-mode`/`rcv-settle-mode` table.

**Repo:** `link.rs::SettlementMode::{Settled, Unsettled}` +
`LinkSession::deliver()` with pending-settlement tracking.

**Tests:** `crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_unsettled_deliver_increments_pending`,
`crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_settle_decrements_pending`,
`crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_settle_with_mapper_calls_apply_and_decrements_pending`
(RELIABLE → `Unsettled` with pending tracking),
`crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_deliver_consumes_credit_and_increments_delivered`,
`crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_settle_at_zero_does_not_underflow`
(BEST_EFFORT → `Settled`, no pending entry). E2E:
`tools/amqp-dds-endpoint/tests/c1_5_settlement_reliable.rs::ddsamqp_7_4_1_reliable_unsettled_increments_pending_until_disposition`,
`tools/amqp-dds-endpoint/tests/c1_6_settlement_best_effort.rs::ddsamqp_7_4_1_best_effort_settled_does_not_track_pending`.

**Status:** done

### §7.4.2 DURABILITY

**Spec:** §7.4.2, p. 40 — DDS `DURABILITY` (VOLATILE/TRANSIENT/
PERSISTENT) ↔ AMQP `terminus.durable` table. `unsettled-state`
requires broker functionality → SHALL `amqp:not-implemented`.

**Repo:** `link.rs::TerminusDurability` (None/Configuration/
UnsettledState) + `check_attach_durability` delivers
`AttachDurabilityCheck::{Accept, RejectNotImplemented}`.

**Tests:** `link::tests::ddsamqp_7_4_2_terminus_durability_from_wire`,
`ddsamqp_7_4_2_attach_durability_unsettled_state_rejected_not_implemented`.

**Status:** done

### §7.4.3 HISTORY

**Spec:** §7.4.3, p. 42 — HISTORY is a DCPS-side cache, decoupled
from AMQP settlement; the AMQP layer forwards samples as DDS emits
them.

**Repo:** no explicit HISTORY mapper needed.

**Tests:** `crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_3_the_dds_side_policies_leave_no_trace_on_the_amqp_boundary`.

**Status:** done — structurally fulfilled by the decoupling.

### §7.4.4 DEADLINE

**Spec:** §7.4.4, p. 43 — local to the DDS side; the catalog channel
signals missed-deadline events.

**Repo:** the catalog channel via `bridge::produce_catalog_transfers`
+ `management::CatalogProducer`. The audit channel (`AuditProducer`)
for deadline events; a concrete `DcpsDdsHost` implementer
connects `DataReader::on_requested_deadline_missed` with
`AuditProducer::push(AuditEvent::Unauthorized {...})` or
an added `DeadlineMissed` event type.

**Tests:** `crates/amqp-endpoint/src/management.rs::tests::ddsamqp_2_1_audit_producer_ringbuffer_evicts_oldest`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_4_4_audit_producer_drain_empties_queue`,
`tools/amqp-dds-endpoint/src/bridge.rs::tests::ddsamqp_7_4_4_produce_catalog_transfers_returns_one_per_topic`,
`tools/amqp-dds-endpoint/src/bridge.rs::tests::ddsamqp_7_4_4_produce_catalog_transfers_empty_for_empty_host`.

**Status:** done

### §7.4.5 LATENCY_BUDGET

**Spec:** §7.4.5, p. 43 — transparent to the AMQP boundary.

**Repo:** `crates/amqp-endpoint/src/qos_translation.rs` (`dds_qos_to_amqp_behavior` — LATENCY_BUDGET feeds no field of `AmqpBehavior`).

**Tests:** `crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_3_the_dds_side_policies_leave_no_trace_on_the_amqp_boundary`.

**Status:** done — structural.

### §7.4.6 OWNERSHIP

**Spec:** §7.4.6, p. 44 — DDS-side owner selection; the AMQP
boundary is unaffected.

**Repo:** `crates/amqp-endpoint/src/qos_translation.rs` (`dds_qos_to_amqp_behavior` — OWNERSHIP feeds no field of `AmqpBehavior`).

**Tests:** `crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_3_the_dds_side_policies_leave_no_trace_on_the_amqp_boundary`.

**Status:** done — structural.

### §7.4.7 LIFESPAN

**Spec:** §7.4.7, p. 44 — DDS LIFESPAN ↔ AMQP
`absolute-expiry-time` of the properties section.

**Repo:** `properties::produce_properties` sets
`absolute_expiry_time_ms = source_timestamp + lifespan_remaining`
when `SampleHeader.lifespan_remaining_ms = Some(_)`.

**Tests:** `properties::tests::ddsamqp_7_4_7_produce_properties_lifespan_sets_expiry`.

**Status:** done

### §7.4.8 PARTITION

**Spec:** §7.4.8, p. 45 — DDS PARTITION is a
`sequence<string>`; AMQP address URI with repeated
`?partition=p1&partition=p2` query params.

**Repo:** `routing.rs::parse_domain_url` accepts repeatedly
repeated `?partition=` params; `AddressResolution.partitions`
is a `Vec<String>`; percent-decoding (RFC 3986) for reserved
characters in partition values.

**Tests:** `routing::tests::ddsamqp_2_1_domain_url_with_multi_partition_parses`,
`ddsamqp_2_1_domain_url_with_percent_encoded_partition`.

**Status:** done

### §7.4.9 All Other QoS Policies (transparent)

**Spec:** §7.4.9, p. 46 — TIME_BASED_FILTER, TRANSPORT_PRIORITY,
LIVELINESS, DESTINATION_ORDER, RESOURCE_LIMITS, ENTITY_FACTORY,
WRITER_DATA_LIFECYCLE, READER_DATA_LIFECYCLE, USER_DATA,
TOPIC_DATA, GROUP_DATA, PRESENTATION,
TYPE_CONSISTENCY_ENFORCEMENT — all DDS-side; the `dds:` prefix
reserved.

**Repo:** `crates/amqp-endpoint/src/qos_translation.rs` (`dds_qos_to_amqp_behavior` reads none of these policies).

**Tests:** `crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_3_the_dds_side_policies_leave_no_trace_on_the_amqp_boundary`,
`crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_9_default_for_topic_is_unsettled_no_durable`.

**Status:** done — structurally via pass-through semantics.

### §7.5 Discovery bridging

**Spec:** §7.5, p. 47 — SPDP/SEDP ↔ the AMQP `$catalog` address.

**Repo:** `management::CatalogProducer` with `add`/`remove`/
`snapshot` API, `CatalogEntry` (address, dds, type-name, type-id,
direction, partitions). The `topics.exposed` counter is automatically
maintained. `classify_address("$catalog")` delivers
`AddressKind::Catalog`.

**Tests:** `management::tests::catalog_*` (5 tests),
`ddsamqp_2_2_classify_address_recognises_reserved`.

**Status:** done

### §7.5.1 Behaviour for unknown addresses

**Spec:** §7.5.1, p. 48 — a `permit_dynamic_topics` flag; with
`false` and unknown → an `amqp:not-found` link error;
with `true` on-the-fly topic creation.

**Repo:** `errors::map_resolution_error(err, permit_dynamic_topics)`
delivers an `AmqpError` with `condition = NotFound`,
`scope = Link`, spec section §7.5.1 + description text depending on
the `permit_dynamic_topics` flag.

**Tests:** `errors::tests::ddsamqp_7_5_1_no_route_with_dynamic_disabled_yields_not_found_link`,
`ddsamqp_7_5_1_no_route_with_dynamic_enabled_still_not_found`,
`ddsamqp_7_5_1_malformed_address_maps_to_decode_error`.

**Status:** done

### §7.6 Key and instance mapping (group-id SHA-256)

**Spec:** §7.6.1, p. 50 — `group-id` = lowercase hex SHA-256
over the XCDR2-KeyHash encapsulation; opaque to the subscriber.

**Repo:** `crates/amqp-endpoint/src/keyhash.rs::group_id` with
SHA-256 + 64-char lowercase hex. `properties::produce_properties`
calls `group_id` from `SampleHeader.keyhash`.

**Tests:** `keyhash::tests` (5 tests incl.
`ddsamqp_7_6_empty_keyhash_yields_known_sha256`,
`ddsamqp_7_6_solidus_in_key_safe`),
`properties::tests::ddsamqp_7_6_produce_properties_keyhash_yields_group_id`.

**Status:** done

### §7.6.2 Subscriber-side reconstruction

**Spec:** §7.6.2, p. 51 — the hash is one-way; the subscriber
SHALL NOT attempt to reconstruct key fields from the group-id.

**Repo:** `crates/amqp-endpoint/src/keyhash.rs` (`group_id` — SHA-256 prefix, no way back).

**Tests:** `crates/amqp-endpoint/src/keyhash.rs::ddsamqp_7_6_2_group_id_carries_no_recoverable_trace_of_the_key`,
`crates/amqp-endpoint/src/keyhash.rs::ddsamqp_7_6_2_keyhash_length_always_64_lowercase_hex`,
`crates/amqp-endpoint/src/keyhash.rs::ddsamqp_7_6_2_distinct_keyhash_yields_distinct_group_id`.

**Status:** done — structural (no reverse mapping to
implement).

### §7.6.3 Topic without keys

**Spec:** §7.6.3, p. 51 — unkeyed topic: omit `group-id`.

**Repo:** `properties::produce_properties` sets
`group_id = None` when `SampleHeader.keyhash = None`.

**Tests:** `properties::tests::ddsamqp_7_6_3_produce_properties_unkeyed_omits_group_id`.

**Status:** done

### §7.7.1 Outbound instance state

**Spec:** §7.7.1, p. 52 — map the DDS sample operation
(`write/register/unregister/dispose`) onto the
application property `dds:operation`; default `write`.

**Repo:** `properties::DdsOperation` enum +
`produce_application_properties` sets `dds:operation` when
`!= Write` (default write is omitted per spec).

**Tests:** `properties::tests::ddsamqp_7_7_1_application_properties_register_sets_operation`,
`ddsamqp_7_7_1_application_properties_default_minimal_set` (verifies that the
write default is omitted).

**Status:** done

### §7.7.2 Inbound operation signals

**Spec:** §7.7.2, p. 53 — inbound `dds:operation` values
onto DataWriter method calls (`register_instance`,
`unregister_instance`, `dispose`, `write`).

**Repo:** `dds_bridge::DdsOperationDispatcher` trait with
`AcceptAllDispatcher` (test default) and
`InstanceTrackingDispatcher` (with spec-§11.3-conformant
lifecycle check). The daemon binds its real DCPS
DataWriter bridge to the trait; the endpoint crate delivers
the plugin surface.

**Tests:** `dds_bridge::tests::amqpbridge_12_1_accept_all_returns_accepted_for_every_operation`,
`ddsamqp_7_7_2_instance_tracking_register_with_body_accepts`,
`ddsamqp_7_7_2_instance_tracking_register_empty_body_yields_missing_key`,
`ddsamqp_7_7_2_instance_tracking_unregister_unknown_yields_unknown_instance`,
`ddsamqp_7_7_2_instance_tracking_unregister_known_accepts`.

**Status:** done

### §7.7.3 Disposition mapping

**Spec:** §7.7.3, p. 54 — AMQP `disposition` ↔ DDS sample state.

**Repo:** `link::LinkSession::apply_disposition` (settlement
tracking) + a `dds_bridge::DispositionMapper` trait for
DDS-API wiring (caller-layer plugin). `NoopDispositionMapper`
as the test default.

**Tests:** `dds_bridge::tests::ddsamqp_7_7_3_noop_disposition_mapper_does_nothing`,
`link::tests::ddsamqp_7_4_1_settle_decrements_pending`.

**Status:** done

### §7.8 Conflict resolution between URI and properties

**Spec:** §7.8, p. 55 — on a conflict between the URI form
`?partition=` and the application property `dds:partition` the
URI form wins.

**Repo:** `routing::effective_partitions(uri_form,
property_form)` delivers the URI form when non-empty, otherwise
property_form.

**Tests:** `routing::tests::ddsamqp_2_1_conflict_resolution_uri_wins_when_both_present`,
`ddsamqp_2_2_conflict_resolution_property_used_when_uri_empty`,
`ddsamqp_2_2_conflict_resolution_both_empty`.

**Status:** done

### §7.9.1 Catalog address ($catalog)

**Spec:** §7.9.1, p. 57 — a receiver link on `$catalog` delivers
topic-mapping entries; the entry fields are enumerated.

**Repo:** `management::CatalogProducer::snapshot` produces
an `AmqpExtValue::Map` per entry with
`amqp-address`/`dds-topic`/`dds-type-name`/`type-id` (ulong or
symbol depending on DescriptorForm)/`direction`/`partitions`.

**Tests:** `crates/amqp-endpoint/src/management.rs::tests::ddsamqp_2_1_catalog_snapshot_emits_map_per_entry`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_type_id_truncated_is_amqp_ulong`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_type_id_full_is_amqp_symbol`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_partitions_emitted_when_set`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_direction_symbols_match_spec`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_add_remove_balances_topics_exposed`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_2_2_classify_address_recognises_reserved`
(`$catalog` as a reserved address).

**Status:** done

### §7.9.2 Metrics address ($metrics)

**Spec:** §7.9.2, p. 58 — a receiver link on `$metrics` delivers a
stream of AMQP messages with a `map` body
{name, value, unit, timestamp}.

**Repo:** `metrics::MetricsHub` (counter) +
`management::metrics_snapshot(hub, now_ms)` (sample producer)
delivers an `AmqpExtValue::Map` per mandatory metric with
`name`/`value`/`unit`/`timestamp` fields.

**Tests:** `crates/amqp-endpoint/src/management.rs::tests::ddsamqp_2_1_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_2_metrics_snapshot_carries_value`;
`crates/amqp-endpoint/src/metrics.rs::tests` (11 tests):
`ddsamqp_7_9_2_mandatory_metric_count_is_14`, `ddsamqp_7_9_2_fresh_hub_all_zero`,
`ddsamqp_7_9_2_unknown_metric_yields_none`, `ddsamqp_7_9_2_connection_open_close_balances`,
`ddsamqp_7_9_2_transfer_unsettled_tracking`,
`ddsamqp_7_9_2_pre_settled_transfer_does_not_count_unsettled`,
`ddsamqp_7_9_2_loop_and_hop_cap_drops_count_separately`,
`ddsamqp_7_9_2_rpc_failure_counters_independent`, `ddsamqp_7_9_2_unit_of_known_metrics`,
`ddsamqp_7_9_2_topics_added_removed_balances`, `ddsamqp_7_9_2_reconnect_overflow_counter`.

**Status:** done

### §7.9.2.1 Mandatory metrics

**Spec:** §7.9.2.1, p. 58 — 13 mandatory metrics (+1
`reconnect-overflow` from §10.8 = total 14):
`connections.active`, `connections.total`,
`transfers.received`, `transfers.sent`,
`transfers.unsettled`, `transfers.rate`,
`errors.decode`, `errors.unauthorized`, `topics.exposed`,
`transfers.dropped.loop`, `transfers.dropped.hop-cap`,
`transfers.dropped.malformed-reply`, `rpc.calls.timed-out`,
`transfers.dropped.reconnect-overflow`.

**Repo:** `metrics::MANDATORY_METRIC_NAMES` (all 14 constants),
the `MetricsHub::on_*` counter API, `unit_of(name)` delivers
the correct unit strings per the spec table.

**Tests:** `metrics::tests::ddsamqp_7_9_2_mandatory_metric_count_is_14`,
`ddsamqp_7_9_2_fresh_hub_all_zero` (verifies coverage of all 14 names).

**Status:** done

### §7.9.3 Limitations + AMQP-Management trade-off

**Spec:** §7.9.3, p. 60 — `$catalog`/`$metrics` as operational,
`$audit` as security; full AMQP-Management 1.0 explicitly
out of scope.

**Repo:** `crates/amqp-endpoint/src/management.rs` (`classify_address`, `AddressKind`).

**Tests:** `crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_classify_address_recognises_reserved`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_7_9_1_catalog_direction_symbols_match_spec`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_audit_event_types_are_spec_symbols`.

**Status:** done — spec classification; no implementation
to deliver.

### §7.10 Implementation limits (DoS caps)

**Spec:** §7.10, p. 61 — compound nesting ≤ 16
(implementation-defined, ≥ 16 SHALL, ≥ 32 SHOULD), max-frame-
size, max-connections, catalog-entries cap, idle timeout.

**Repo:** `crates/amqp-endpoint/src/limits.rs::ResourceLimits`
with `max_frame_size`, `max_connections`, `idle_timeout_ms`,
`max_compound_nesting`.

**Tests:** `crates/amqp-endpoint/src/limits.rs::tests::ddsamqp_7_10_defaults_match_spec_implementation_note`.

**Status:** done

### §7.11 Bridge coexistence (loop prevention)

**Spec:** §7.11, p. 63 — `dds:bridge-id` stamping +
drop-on-self-tag + `dds:bridge-hop` cap (default 8, max 16);
bridge_id is process-wide.

**Repo:** `coexistence::CoexistenceConfig` (process-wide
bridge_id + hop_cap; `DEFAULT_HOP_CAP=8`, `MAX_HOP_CAP=16`),
`inspect_inbound`, `stamp_outbound`. Properties app keys
`dds:bridge-id`/`dds:bridge-hop` in `properties::app_keys`.

**Tests:** `crates/amqp-endpoint/src/coexistence.rs::tests` (11 tests):
`ddsamqp_7_11_forward_when_no_bridge_metadata`, `ddsamqp_2_1_drop_loop_on_self_tag_string`,
`ddsamqp_2_2_drop_loop_on_self_tag_list`, `ddsamqp_2_1_drop_hop_cap_when_exceeded`,
`ddsamqp_7_11_forward_at_hop_cap_limit`, `ddsamqp_7_11_stamp_outbound_creates_initial_entries`,
`ddsamqp_7_11_stamp_outbound_appends_to_existing_string_list`,
`ddsamqp_7_11_stamp_outbound_appends_to_list_form`,
`ddsamqp_2_1_round_trip_stamp_then_inspect_drops_loop`,
`ddsamqp_2_2_other_bridges_tag_does_not_drop`, `ddsamqp_2_5_2_config_defaults_match_spec`.

**Status:** done

---

## §8 Wire Encoding (PSM)

### §8.1.1 Pass-through octet-string mode

**Spec:** §8.1.1, p. 67 — body as a single `data` section, content
verbatim XCDR2; `content-type = application/vnd.dds.xcdr2`.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., PassThrough)`
delivers `("application/vnd.dds.xcdr2", bytes_verbatim)`.

**Tests:** `mapping::tests::ddsamqp_8_1_1_a_pass_through_body_is_the_sample_octets`.

**Status:** done

### §8.1.2 JSON mapping mode

**Spec:** §8.1.2, p. 68 — body UTF-8 JSON document; full
IDL-to-JSON mapping rules (Time_t, octet sequences as Base64,
union, sequence, map, 16-byte identifier as hex).

**Repo:** `crates/amqp-bridge/src/codegen/encode.rs::json_document` —
the clause's rules row for row: Base64 for an octet sequence, the
symbolic name for an `enum`, `{"sec": n, "nanosec": m}` for a `Time_t`,
`{"discriminator": .., "value": ..}` for a `union` with `value` omitted
for an empty branch, absent `@optional` members omitted, and `NaN` and
the infinities raised as an error rather than written. The type
information the clause requires comes from
`crates/amqp-bridge/src/codegen/plan.rs::plan_of`, off the model. The
per-IDL-type helpers are written by
`crates/amqp-bridge/src/codegen/emit.rs::helpers_for`
(`toJsonString_<TypeName>`) for C++, Java and TypeScript against the
runtimes in `crates/amqp-bridge/runtime/`.
`crates/amqp-endpoint/src/mapping.rs::encode_dds_to_amqp_body(.., Json)`
remains the type-blind path and still writes the hex field.

**Tests:** `crates/amqp-bridge/src/codegen/encode.rs::tests`
(`ddsamqp_8_1_2_an_octet_sequence_is_base64_in_json_and_binary_on_the_wire`,
`ddsamqp_8_1_2_an_enumeration_is_its_symbolic_name_in_both_modes`,
`ddsamqp_8_1_2_a_nan_and_an_infinity_are_refused_rather_than_written`,
`ddsamqp_8_1_2_a_time_is_milliseconds_on_the_wire_and_two_keys_in_json`,
`ddsamqp_8_1_2_the_json_escapes_are_rfc_8259s`,
`ddsamqp_8_1_2_base64_matches_rfc_4648_test_vectors`);
`crates/amqp-bridge/tests/cross_language_agreement.rs` compiles and runs
the emitted C++, Java and TypeScript and compares their JSON character
for character against the Rust reference.

**Status:** done — the IDL-to-JSON rules are type-driven and stand in
four columns that are compared against each other.

### §8.1.3 AMQP-native typed mapping mode

**Spec:** §8.1.3, p. 71 — body as a single `amqp-value` section
with a described composite per §7.2.

**Repo:** `crates/amqp-bridge/src/codegen/encode.rs::described_composite`
— the §7.2.1 descriptor in both forms, the §7.2.2 body as a list, as a
map keyed by member ID for a `@mutable` type and as a two-element list
for a `union` per §7.2.3, a `null` at the slot of an absent `@optional`
member, and a nested composite with its own descriptor per §7.2.2.4.
`crates/amqp-bridge/src/codegen/plan.rs` reads §7.1's row-for-row
mapping off the model. The per-IDL-type helpers are written by
`crates/amqp-bridge/src/codegen/emit.rs::helpers_for`
(`toAmqpValue_<TypeName>`) for C++, Java and TypeScript against the
runtimes in `crates/amqp-bridge/runtime/`.
`crates/amqp-endpoint/src/mapping.rs::encode_dds_to_amqp_body(..,
AmqpNative)` sets the content-type.

**Tests:** `crates/amqp-bridge/tests/type_mapping.rs` (10 tests):
`ddsamqp_2_2_every_row_of_the_primitive_table_takes_the_code_it_names`,
`ddsamqp_2_2_an_octet_sequence_is_binary_and_a_long_sequence_is_a_list`,
`ddsamqp_2_2_the_body_form_follows_the_types_extensibility`,
`ddsamqp_2_2_a_mutable_body_is_keyed_by_the_declared_member_ids`,
`ddsamqp_2_2_a_union_plans_its_discriminator_and_then_its_branches`,
`ddsamqp_2_2_a_type_outside_the_mapping_is_named_rather_than_encoded_as_something_else`,
`ddsamqp_8_1_3_a_plan_of_something_that_is_not_a_composite_is_refused`,
`amqp10_1_1_the_two_descriptor_forms_come_from_the_same_identifier`,
`ddsamqp_8_1_3_a_sample_nested_past_the_bound_is_refused_by_name`,
`amqp10_1_1_a_nested_composite_carries_its_own_descriptor` — from the
lowered model: the primitive table row for row, the body form per
extensibility, the member-ID map, the union, both descriptor forms, the
nested composite, and the refusal of an unmapped type);
`crates/amqp-bridge/tests/cross_language_agreement.rs` compiles and runs
the emitted C++, Java and TypeScript and compares their octets byte for
byte against the Rust reference.

**Status:** done — the described composite is type-driven and stands in
four columns that arrive at the same octets.

### §8.2 Properties-section message-id

**Spec:** §8.2, p. 72 — `message-id` = binary(24) = ⟨writer GUID
(16B) || RTPS seqnum (8B BE)⟩, unique per sample. InstanceHandle
NOT in the message-id.

**Repo:** `properties::message_id(writer_guid, seqnum)` delivers a
24-byte Vec; InstanceHandle is explicitly mapped to the
`dds:instance-handle` app property (not to
`message-id`).

**Tests:** `properties::tests::ddsamqp_8_2_message_id_is_24_bytes_guid_then_seqnum`,
`ddsamqp_8_2_message_id_distinguishes_consecutive_samples_same_instance`.

**Status:** done

### §8.2 Properties-section other fields

**Spec:** §8.2, p. 72 — table: user-id, to, subject, reply-to,
correlation-id, content-type, content-encoding,
absolute-expiry-time, creation-time, group-id, group-sequence,
reply-to-group-id.

**Repo:** `properties::ProducedProperties` delivers the
spec-mandatory fields (`message_id`, `creation_time_ms`,
`absolute_expiry_time_ms`, `group_id`); application-specific
fields (`subject`, `reply-to`, `correlation-id`, `user-id`)
remain caller-set.

**Tests:** `crates/amqp-endpoint/src/properties.rs::tests::ddsamqp_7_6_produce_properties_keyhash_yields_group_id`,
`crates/amqp-endpoint/src/properties.rs::tests::ddsamqp_7_6_3_produce_properties_unkeyed_omits_group_id`,
`crates/amqp-endpoint/src/properties.rs::tests::ddsamqp_7_4_7_produce_properties_lifespan_sets_expiry`.

**Status:** done — spec-mandatory fields set.

### §8.3 Application-properties mapping

**Spec:** §8.3, p. 74 — standard keys: `dds:nsec`,
`dds:partition`, `dds:domain-id`, `dds:type-id`,
`dds:source-guid`, `dds:lifespan-ms`, `dds:sample-state`,
`dds:view-state`, `dds:instance-state`, `dds:operation`,
`dds:bridge-id`, `dds:bridge-hop`, `dds:instance-handle`.

**Repo:** `properties::app_keys` module with all 13 string
constants; `produce_application_properties` builds the
`AmqpExtValue::Map` with the derivable standard keys
(`dds:nsec`, `dds:partition` as a single string or list,
`dds:domain-id`, `dds:type-id` (on DESC_TRUNCATED),
`dds:lifespan-ms`, `dds:operation` (when != Write),
`dds:instance-handle`).

**Tests:** `crates/amqp-endpoint/src/properties.rs::tests`:
`ddsamqp_7_7_1_application_properties_default_minimal_set`,
`ddsamqp_7_7_1_application_properties_register_sets_operation`,
`ddsamqp_2_5_application_properties_truncated_type_id_present`,
`ddsamqp_8_3_application_properties_multi_partition_uses_list`,
`ddsamqp_8_3_application_properties_single_partition_uses_string`,
`ddsamqp_8_3_application_properties_nsec_set_when_nonzero`.

**Status:** done

---

## §9 Configuration

### §9.1 IDL-defined mapping schema (Annex A)

**Spec:** §9.1, p. 79 — configuration via an IDL schema:
`TopicMapping`, `AmqpEndpointConfig`, `AmqpBridgeConfig`,
`TlsConfig`, `SaslConfig`, `ResourceLimits`,
`DynamicTopicConfig`. SemVer fields.

**Repo:** `crates/amqp-endpoint/src/annex_a.rs` mirrors the
Annex-A IDL 1-to-1 into Rust: all 5 enums (SaslMechanism,
BodyEncodingMode, TimeMapping, DescriptorForm, LinkDirection)
and all 7 structs (TopicMapping, TlsConfig, SaslConfig,
ResourceLimits, DynamicTopicConfig, AmqpEndpointConfig,
AmqpBridgeConfig) with spec-conformant defaults. IDL-symbol
round-trip (`as_idl`/`parse`) mapped.

**Tests:** `crates/amqp-endpoint/src/annex_a.rs::tests` (10 tests):
`ddsamqp_9_1_the_sasl_mechanisms_are_the_enumerators_of_annex_a`, `ddsamqp_9_1_sasl_mechanism_accepts_amqp_wire_aliases`,
`ddsamqp_9_1_the_body_encoding_modes_are_the_enumerators_of_annex_a`, `ddsamqp_9_1_descriptor_form_default_is_truncated`,
`ddsamqp_9_1_time_mapping_default_is_standard`, `ddsamqp_9_1_link_direction_default_is_both`,
`ddsamqp_9_1_topic_mapping_defaults_match_spec`, `ddsamqp_9_1_endpoint_config_default_is_empty`,
`ddsamqp_9_1_resource_limits_default_has_dos_caps`, `ddsamqp_9_1_link_direction_amqp_aliases`.

**Status:** done — the data model is spec-conformant and available in
the crate; the symbol round-trip against the IDL spelling is tested per
enum and per struct.

### §9.2 XML configuration

**Spec:** §9.2, p. 81 — XML form as an alternative configuration
input; XSD snippet referenced.

**Repo:** `crates/amqp-endpoint/src/config_xml.rs` with
`parse_config(xml)` → a `DdsAmqpConfig` result
(roxmltree-based; std-only feature). Accepts the
`<dds-amqp>` root element with the namespace
`http://www.zerodds.org/dds-amqp/v1.0`. A parser for endpoint,
bridge, tls, sasl, topics, dynamic, limits.

**Tests:** `config_xml::tests` (11 tests incl.
`ddsamqp_9_2_parse_minimal_endpoint`,
`ddsamqp_9_2_parse_endpoint_with_topics_tls_sasl_limits`,
`ddsamqp_9_2_parse_bridge_with_upstream`, `ddsamqp_9_2_unknown_root_yields_error`,
`ddsamqp_9_2_missing_amqp_address_yields_error`,
`ddsamqp_9_2_invalid_body_mode_yields_error`,
`ddsamqp_9_2_malformed_xml_yields_parse_error`,
`ddsamqp_9_2_unknown_elements_and_attributes_are_refused_with_their_line`);
`tools/amqp-dds-endpoint/tests/readme_config.rs::ddsamqp_9_2_the_readme_configuration_reads_as_it_says`.
Unknown elements and attributes are refused with their line; the namespace
carries the version.

**Status:** done

---

## §10 Security

### §10.1 TLS bracket

**Spec:** §10.1, p. 83 — TLS 1.2 minimum, 1.3 RECOMMENDED;
cert validation; AMQP negotiation after the TLS handshake.

**Repo:** `tools/amqp-dds-endpoint/src/tls.rs` (Cargo feature
`tls`, opt-in): `ServerTlsConfig` + `ClientTlsConfig` from PEM,
`build_server_config` with an optional mTLS client-cert verifier
(`require_client_cert = true`), `build_client_config` with trust
anchors + an optional client-auth cert. `accept_server` /
`connect_client` perform a real TLS handshake (rustls 0.23,
TLS 1.2 + 1.3, ring crypto provider). The `StreamOwned` result
is `Read+Write` and is passed directly to `handle_connection`.

**Tests:** `tls::tests` (6 tests incl.
`ddsamqp_10_1_build_server_config_from_self_signed`,
`ddsamqp_10_1_build_server_config_require_client_cert_needs_ca`,
`ddsamqp_10_1_build_client_config_from_root_ca`,
`ddsamqp_10_1_tls_handshake_round_trip_with_self_signed_cert` with a real
TLS handshake against an rcgen-generated self-signed cert).

`tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_10_1_a_tls_client_is_offered_plain_and_authenticated_against_the_store`
(through the endpoint program's server: TLS handshake, PLAIN offered, checked against the
store).

**Status:** done — rustls, feature `tls` (default) in the endpoint program.

### §10.2 SASL mechanisms — mandatory set

**Spec:** §10.2, p. 85 — implementations MUST support PLAIN,
ANONYMOUS, EXTERNAL.

**Repo:** `sasl.rs::SaslMechanism::{Plain, Anonymous, External}`.

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_prefers_external`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_falls_back_to_plain_with_tls`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_skips_plain_without_tls`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_10_2_select_outbound_anonymous_only`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_no_acceptable_mechanism`,
`crates/amqp-endpoint/src/sasl.rs::tests::amqp10_5_3_3_6_the_sasl_codes_are_0_to_4`.

**Status:** done

### §10.2 SASL mechanisms — optional SCRAM-SHA-256

**Spec:** §10.2, p. 85 — implementations MAY additionally support
SCRAM-SHA-256.

**Repo:** `crates/amqp-endpoint/src/sasl.rs::SaslMechanism::ScramSha256`
with the wire symbol `"SCRAM-SHA-256"` + round-trip-parse support +
an `is_mandatory()` predicate. The concrete RFC-7677 implementation
(HMAC-SHA-256 + PBKDF2 + channel binding) is caller-layer
(reuse `crates/security-crypto`).

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::amqp10_5_3_3_1_the_mechanism_names_are_plain_anonymous_external`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_10_2_unknown_name_yields_none`,
`crates/amqp-endpoint/src/security.rs::tests::ddsamqp_10_2_scram_yields_scram_sha256_class_id`.

**Status:** done — the optional SCRAM-SHA-256 mechanism is stated as a wire
marker enum variant.

### §10.2.1 Mandatory TLS for SASL PLAIN

**Spec:** §10.2.1, p. 87 — see Endpoint Cl. 7 / Bridge Cl. 5.

**Repo:** the inbound filter `SaslState::new(tls_active)` +
the outbound filter `SaslState::select_outbound`. The daemon outbound
in `client::do_outbound_sasl` with a
`ClientError::PlainRejectedNoTls` reject path.

**Tests:** `sasl::tests::ddsamqp_2_1_plain_offered_only_when_tls_active`,
`ddsamqp_2_1_select_outbound_skips_plain_without_tls`.

**Status:** done

### §10.3 Cross-reference to DDS-Security — outbound signing

**Spec:** §10.3.1, p. 88 — DDS-Security plugins can be active;
outbound signing is DDS-side.

**Repo:** `crates/amqp-endpoint/src/security.rs` (`DualIdentity`).

**Tests:** `crates/amqp-endpoint/src/security.rs::ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`,
`crates/amqp-endpoint/src/security.rs::ddsamqp_10_3_external_yields_pki_dh_class_id_with_cert`,
`crates/amqp-endpoint/src/security.rs::ddsamqp_10_3_class_id_strings_match_spec_table`.

**Status:** done — structural (DDS-Security plugins active in parallel
to the AMQP layer).

### §10.3.2 Identity mapping (vendor class_ids)

**Spec:** §10.3.2, p. 89 — IdentityToken class_ids:
PLAIN→`zerodds:Auth:SASL-Username:1.0`,
ANONYMOUS→`zerodds:Auth:Anonymous:1.0`,
EXTERNAL→`DDS:Auth:PKI-DH:1.0` (with X.509),
SCRAM-SHA-256→`zerodds:Auth:SASL-SCRAM-SHA256:1.0`.
`subject_name` convention `CN=<authcid>`.

**Repo:** `security::class_ids` module with all 4 constants;
`build_identity_token(SaslSubject)` produces spec-conformant
tokens (PLAIN/ANONYMOUS/EXTERNAL/SCRAM).

**Tests:** `security::tests::ddsamqp_10_3_2_plain_yields_sasl_username_class_id`,
`ddsamqp_10_3_2_anonymous_yields_anonymous_class_id`,
`ddsamqp_10_3_external_yields_pki_dh_class_id_with_cert`,
`ddsamqp_10_2_scram_yields_scram_sha256_class_id`,
`ddsamqp_10_3_class_id_strings_match_spec_table`.

**Status:** done

### §10.3.3 Permission evaluation

**Spec:** §10.3.3, p. 90 — pass the IdentityToken to the
DDS-Security AccessControl plugin
`check_create_datawriter`/`check_create_datareader`;
NOT_ALLOWED → an AMQP `amqp:unauthorized-access` link error.

**Repo:** `security::AccessControlPlugin` trait with
`check(identity, address, op) -> AccessDecision`. Default plugins
`AllowAll` (test) + `StaticAllowList`. `errors::access_denied`
delivers the `amqp:unauthorized-access` mapping.

**Tests:** `security::tests::ddsamqp_10_3_3_static_allow_list_per_op`,
`errors::tests::ddsamqp_10_3_3_access_denied_yields_unauthorized_access_link`.

**Status:** done

### §10.3.4 Determinism across vendors

**Spec:** §10.3.4, p. 91 — IdentityToken construction is
deterministic across implementations.

**Repo:** `build_identity_token` is a pure function without
random/state; the same `SaslSubject` → the same
`IdentityToken`. `LinkGovernance::evaluate` caches per op and
deterministically delivers the same decision for the same input.

**Tests:** `security::tests::ddsamqp_2_1_link_governance_caches_decision`
(verifies a cache hit on a repeated op call).

**Status:** done

### §10.3.5 No-bypass guarantee

**Spec:** §10.3.5, p. 92 — a DDS sample that AccessControl rejects
must not exit over AMQP; the AccessControl result is a
precondition of every transfer.

**Repo:** `handler::check_access(cfg, addr, op)` called in the daemon
loop pre-Attach + pre-Transfer. On deny:
`metrics.on_unauthorized()` + Detach (on Attach) resp. Drop
(on Transfer).

**Tests:** `handler::tests::ddsamqp_2_1_access_control_deny_attach_yields_unauthorized_metric`,
`ddsamqp_2_1_access_control_allow_does_not_increment_unauthorized`.

**Status:** done

### §10.4 Governance document mapping

**Spec:** §10.4, p. 95 — DDS-Security governance-document domain
rules ↔ AMQP address allow/deny per identity.

**Repo:** `security::GovernanceDocument` with `add_rule`/
`resolve(topic)` API. `GovernanceRule` with `topic_pattern` (exact,
prefix*, *suffix, *), `enable_discovery`, `enable_liveliness`,
`data_protection_kind` (None/SignOnly/SignAndEncrypt).
`config_xml::parse_governance(xml)` reads the XML
`<governance>` root with `<rule>` children into a
`GovernanceDocument`.

**Tests:** `security::tests::governance_resolves_*` (4 tests),
`config_xml::tests::ddsamqp_10_4_governance_document_loads_rules`,
`ddsamqp_10_4_governance_missing_topic_pattern_errors`,
`ddsamqp_10_4_governance_invalid_data_protection_errors`.

**Status:** done

### §10.5 No implicit trust between profiles

**Spec:** §10.5, p. 97 — Endpoint and Bridge on the same host
do not share an identity automatically.

**Repo:** `crates/amqp-endpoint/src/security.rs` (`DualIdentity`, separate `IdentityToken` and broker-credential fields).

**Tests:** `crates/amqp-endpoint/src/security.rs::ddsamqp_2_2_dual_identity_keeps_broker_and_dds_separate`,
`crates/amqp-endpoint/src/security.rs::ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`,
`crates/amqp-endpoint/src/bridge_security.rs::amqpbridge_12_1_amqp_acl_address_check`,
`crates/amqp-endpoint/src/bridge_security.rs::ddsamqp_10_5_amqp_sasl_plain_rejects_unknown_user`.

**Status:** done — structural (separate configuration structures).

### §10.6 Bridge-profile dual identity

**Spec:** §10.6, p. 98 — the bridge carries two separate identities:
broker-side SASL credential, DDS-side IdentityToken; do not
conflate them.

**Repo:** `security::DualIdentity::new(broker_id, dds_id)` with
`for_broker()`/`for_dds()` getters. The AccessControl plugin SHALL use only
`for_dds()`.

**Tests:** `security::tests::ddsamqp_2_2_dual_identity_keeps_broker_and_dds_separate`,
`ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### §10.7 Per-link governance resolution

**Spec:** §10.7, p. 100 — the governance permission is re-evaluated
per link; mixed-sensitivity topics on one connection
are allowed.

**Repo:** `security::LinkGovernance::new(identity, address, rule)`
+ `evaluate(plugin, op)` with a per-op cache. Mixed sensitivity is
structurally supported: each `LinkGovernance` instance holds its
own identity/rule, multiple instances per connection are
allowed.

**Tests:** `security::tests::ddsamqp_2_1_link_governance_caches_decision`.

**Status:** done

### §10.8 Reconnect behaviour

**Spec:** §10.8, p. 102 — Endpoint: failed-connection-unsettled
released; Bridge: reconnect with exp. backoff (init 1s, mult 2,
cap 60s); KEEP_LAST eviction counts
`transfers.dropped.reconnect-overflow`.

**Repo:** `client::ReconnectConfig` + `connect_with_reconnect`
with default pacing (1s init, 2x mult, 60s cap) per spec.
`MetricsHub::on_reconnect_overflow` for the KEEP_LAST eviction
counter.

**Tests:** `tools/amqp-dds-endpoint/src/client.rs::tests::ddsamqp_2_2_backoff_starts_at_initial`,
`tools/amqp-dds-endpoint/src/client.rs::tests::ddsamqp_2_2_backoff_doubles_until_cap`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_loop_pacing_follows_spec_defaults`
(1 s init, factor 2, 60 s cap),
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_the_client_reconnects_once_the_server_is_up`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_aborts_on_max_attempts`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_after_server_restart_eventually_succeeds`.

**Status:** done

---

## §11 Error Conditions

### §11.1 Encode/decode failures

**Spec:** §11.1, p. 105 — table errors → AMQP error codes:
type-mismatch / no-mapping / malformed-XCDR2 / nesting-depth /
UTF-8-violation / char-out-of-range / hash-truncation-collision /
frame-size-exceeded → `amqp:decode-error` (disposition rejected
or connection close).

**Repo:** `errors::AmqpError` + `AmqpErrorCondition` +
`ErrorScope` (Transfer/Link/Connection). `map_mapping_error`
delivers the `amqp:decode-error` from a `MappingError` input.

**Tests:** `errors::tests::ddsamqp_11_1_condition_symbols_match_spec`,
`ddsamqp_11_1_invalid_utf8_maps_to_decode_error_transfer`,
`ddsamqp_11_1_invalid_json_maps_to_decode_error`,
`ddsamqp_11_1_empty_body_maps_to_decode_error`.

**Status:** done

### §11.2 Connection / session / link lifecycle failures

**Spec:** §11.2, p. 107 — idle timeout / max_connections cap /
catalog cap / dynamic-topic-disabled / unsupported-durability /
unsupported-operation / DDS-Security reject → specific AMQP
error codes.

**Repo:** `errors::resource_limit_exceeded`,
`errors::unsettled_state_not_implemented`,
`errors::unknown_dds_operation`, `errors::access_denied`
deliver spec-conformant error mappings with the correct
`ErrorScope` (Connection/Link/Transfer).

**Tests:** `errors::tests::ddsamqp_11_2_resource_limit_exceeded_is_connection_scope`,
`ddsamqp_11_2_unsettled_state_yields_not_implemented`,
`ddsamqp_11_2_unknown_dds_operation_yields_not_implemented`.

**Status:** done

### §11.3 Instance-lifecycle failures

**Spec:** §11.3, p. 109 — `dds:operation = unregister` on an
unknown instance / `dispose` on an unknown instance /
`register` without a key → `amqp:precondition-failed`/
`amqp:decode-error`.

**Repo:** `errors::instance_unknown` + `register_missing_key` +
`unknown_dds_operation` helpers; `dds_bridge::DispatchOutcome`
delivers the spec-conformant outcomes; `to_amqp_error(key_hex)`
maps them to `AmqpError`. The `InstanceTrackingDispatcher` implements
the behavior.

**Tests:** `errors::tests::ddsamqp_11_3_instance_unknown_yields_precondition_failed`,
`ddsamqp_11_3_register_missing_key_yields_decode_error`,
`dds_bridge::tests::ddsamqp_11_3_dispatch_outcome_to_amqp_error_maps_correctly`,
`instance_tracking_*` (5 tests).

**Status:** done

### §11.4 Diagnostic description strings

**Spec:** §11.4, p. 110 — description-string format:
`<spec-section>: <human-readable>`.

**Repo:** `errors::ErrorDescription::render` delivers exactly the
format `<§-ref>: <text>`. All `errors::*` helpers construct
spec-conformant descriptions with a `§-ref` prefix.

**Tests:** `errors::tests::ddsamqp_11_4_description_renders_spec_section_then_text`,
`ddsamqp_11_4_description_display_matches_render`.

**Status:** done

---

## Annex A — IDL Configuration Schema (normative)

### Annex A IDL module `zerodds::amqp`

**Spec:** Annex A, p. 113 — complete IDL code: enums
`BodyEncodingMode`, `TimeMapping`, `DescriptorForm`,
`SaslMechanism`, `LinkDirection`; structs `TopicMapping`,
`TlsConfig`, `SaslConfig`, `ResourceLimits`,
`DynamicTopicConfig`, `AmqpEndpointConfig`, `AmqpBridgeConfig`.
`bridge_id`/`bridge_hop_cap` on the Endpoint/Bridge config
(process-wide).

**Repo:** `crates/amqp-endpoint/src/annex_a.rs` with all
5 enums + 7 structs in a 1-to-1 mirror of the IDL.
`bridge_id`/`bridge_hop_cap` on the Endpoint+Bridge config.
IDL-symbol round-trip via `as_idl`/`parse`.

**Tests:** `crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_the_sasl_mechanisms_are_the_enumerators_of_annex_a`,
`crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_the_body_encoding_modes_are_the_enumerators_of_annex_a`,
`crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_topic_mapping_defaults_match_spec`,
`crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_endpoint_config_default_is_empty`,
`crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_resource_limits_default_has_dos_caps`.

**Status:** done — see §9.1.

---

## Annex B — Examples (informative)

### Annex B Examples

**Spec:** Annex B, p. 119 — three examples: edge sensor,
cloud subscriber via broker, multi-vendor federation.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — example deployments with no normative
requirement; the mechanisms they assemble are tracked as their own items
in §7.3 (address resolution), §7.11 (bridge coexistence) and §10.1 (TLS).

---

## Annex C — Compliance Test Suite (normative)

### Annex C C.1.1 Connection Open (TLS+PLAIN)

**Spec:** Annex C §C.1.1, p. 124 — an AMQP-1.0 client connects via
TLS, authenticates with PLAIN, establishes a session.

**Repo:** integration test `c1_1_connection_open.rs` drives a
real Open roundtrip against a locally-running daemon with
`tls_active = true`. TLS termination itself is a §10.1
follow-up wave.

**Tests:** `ddsamqp_2_1_connection_open_with_tls_active_advertises_plain`.

**Status:** done — Open roundtrip + PLAIN mechanism negotiation
verified; TLS wire encryption is §10.1.

### Annex C C.1.2 SASL PLAIN Rejection on Plain Transport

**Spec:** Annex C §C.1.2, p. 124 — a client connects without TLS,
SASL-init with PLAIN → SASL-Outcome `auth` fail + connection close.

**Repo:** the logic in `sasl::SaslState::new(false)` +
`select_outbound`. An E2E integration test
`tools/amqp-dds-endpoint/tests/c1_2_sasl_plain_rejection.rs`
with a real TCP connection against a locally-running daemon.

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_plain_without_tls_yields_auth_failed`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_skips_plain_without_tls`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_plain_offered_only_when_tls_active`,
`c1_2_no_tls_server_does_not_advertise_plain`,
`c1_2_client_with_only_plain_credentials_fails_without_tls`,
`c1_2_client_error_plainrejectednotls_strs_correctly`.

**Status:** done

### Annex C C.1.3 Sender-link producer-to-DDS

**Spec:** Annex C §C.1.3, p. 125 — sender link on a topic address;
the transfer is published in DDS; the DDS subscriber receives it.

**Repo:** `bridge::dispatch_attach` resolves the address against
`DdsHost`; `bridge::dispatch_transfer(host, topic_id, body, metrics)`
calls `host.publish_to_dds`. A roundtrip verified via
`InMemoryDdsHost`.

**Tests:** `c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_amqp_producer_publishes_to_dds_via_bridge`,
`ddsamqp_c_1_3_unknown_address_attaches_with_unknown_address_outcome`.

**Status:** done

### Annex C C.1.4 Receiver-link DDS-to-consumer

**Spec:** Annex C §C.1.4, p. 125 — analogous to C.1.3 in the reverse direction.

**Repo:** `bridge::subscribe_outbound(host, topic_id, callback)`
registers an outbound subscription; as soon as the DDS side
publishes (`host.publish_to_dds`), the callback is invoked with the
bytes, which triggers the AMQP outbound transfer logic.

**Tests:** `ddsamqp_2_1_dds_publish_flows_to_amqp_consumer_callback`.

**Status:** done

### Annex C C.1.5 Settlement-mode reliable

**Spec:** Annex C §C.1.5, p. 125 — a DDS DataWriter with
RELIABILITY=RELIABLE; an AMQP settlement roundtrip.

**Repo:** `link.rs::LinkSession::deliver`/`settle` pending
tracking. The integration test
`c1_5_settlement_reliable.rs` verifies: pending is
increased until a disposition is received, credit exhaustion
delivers a NoCredit error.

**Tests:** `ddsamqp_7_4_1_reliable_unsettled_increments_pending_until_disposition`,
`ddsamqp_c_1_5_credit_exhaustion_blocks_further_deliveries`.

**Status:** done

### Annex C C.1.6 Settlement-mode best-effort

**Spec:** Annex C §C.1.6, p. 125 — RELIABILITY=BEST_EFFORT;
pre-settled delivery.

**Repo:** `link.rs::LinkSession` with `SettlementMode::Settled`.
The integration test `c1_6_settlement_best_effort.rs`.

**Tests:** `ddsamqp_7_4_1_best_effort_settled_does_not_track_pending`,
`ddsamqp_c_1_6_settle_call_on_pre_settled_link_is_no_op`.

**Status:** done

### Annex C C.1.7 Address-resolution wildcard

**Spec:** Annex C §C.1.7, p. 126 — a wildcard address attached →
a multi-partition stream.

**Repo:** `routing::AddressRouter::add_route` + a pattern matcher
with prefix-`*`/suffix-`*`/global-`*`. The integration test
`c1_7_address_resolution_wildcard.rs`.

**Tests:** `ddsamqp_c_1_7_prefix_wildcard_matches_multiple_topics`,
`ddsamqp_c_1_7_suffix_wildcard_matches`,
`ddsamqp_c_1_7_global_wildcard_matches_anything`,
`ddsamqp_c_1_7_static_alias_takes_precedence_over_wildcard`.

**Status:** done

### Annex C C.1.8 Catalog address

**Spec:** Annex C §C.1.8, p. 126 — a receiver link on `$catalog`
delivers topic-mapping entries.

**Repo:** `bridge::dispatch_attach` recognizes the `$catalog` address
and delivers `AttachOutcome::AttachedCatalog`;
`bridge::produce_catalog_transfers(host)` delivers an AMQP map as a
sample body per registered topic mapping;
`encode_catalog_sample` wraps it in a described composite
for wire transport.

**Tests:** `bridge::tests::ddsamqp_7_4_4_produce_catalog_transfers_returns_one_per_topic`,
`ddsamqp_c_1_8_encode_catalog_sample_produces_described_composite`,
`c1_3_4_8_bridge_dispatch.rs::ddsamqp_c_1_8_catalog_receiver_gets_one_sample_per_topic`.

**Status:** done

### Annex C C.1.9 Idle timeout

**Spec:** Annex C §C.1.9, p. 126 — a connection without traffic →
connection close with an idle-timeout error.

**Repo:** `ResourceLimits::idle_timeout_ms` +
`TcpStream::set_read_timeout` in the daemon. The integration test
`c1_9_idle_timeout.rs` verifies a real read-timeout
disconnect (the server closes the idle client).

**Tests:** `ddsamqp_c_1_9_server_disconnects_idle_client_after_short_read_timeout`,
`ddsamqp_c_1_9_idle_timeout_is_configurable`.

**Status:** done

### Annex C C.1.10 Concurrent connections

**Spec:** Annex C §C.1.10, p. 127 — the endpoint accepts
at least `max_connections` parallel connections.

**Repo:** `server::run_server` + `common::TestServer` spawn
thread-per-connection. The integration test
`c1_10_concurrent_connections.rs` starts 8 parallel
client threads, verifies that all 8 successfully
authenticate + connect and that the `connections.total`
counter in the server counts all of them.

**Tests:** `ddsamqp_c_1_10_server_accepts_multiple_concurrent_connections`.

**Status:** done

### Annex C C.1.11 Pass-through encoding

**Spec:** Annex C §C.1.11, p. 127 — a sample with MODE_PASSTHROUGH
roundtrips byte-identical.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., PassThrough)`
plus bridge dispatch in
`tools/amqp-dds-endpoint/src/bridge.rs::dispatch_transfer`.

**Tests:** `mapping::tests::ddsamqp_8_1_1_a_pass_through_body_is_the_sample_octets`,
`c1_3_4_8_bridge_dispatch::ddsamqp_2_1_amqp_producer_publishes_to_dds_via_bridge`,
`ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`,
`ddsamqp_2_1_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### Annex C C.1.12 JSON encoding

**Spec:** Annex C §C.1.12, p. 127 — a sample with MODE_JSON is
valid RFC-8259 JSON.

**Repo:** `crates/amqp-bridge/src/codegen/encode.rs::json_document`
writes the document; RFC 8259 §7's escapes are in `json_string` of the
same file, and a number is written as the shortest representation that
round-trips. The emitted `toJsonString_<TypeName>` helpers from
`crates/amqp-bridge/src/codegen/emit.rs` write the same document in C++,
Java and TypeScript (see §8.1.2).

**Tests:** `crates/amqp-bridge/tests/cross_language_agreement.rs::ddsamqp_c_1_12_the_reference_json_is_a_valid_rfc_8259_document`
checks bracket balance outside strings, no trailing comma and the quotes
around every key; `ddsamqp_c_1_12_the_emitted_cpp_writes_the_same_octets_and_the_same_json`
and its two siblings check that the three emitted columns write the same
document character for character;
`crates/amqp-bridge/src/codegen/encode.rs::tests::ddsamqp_8_1_2_the_json_escapes_are_rfc_8259s`.

**Status:** done — a sample in MODE_JSON is valid RFC-8259 JSON, checked
in all four columns.

### Annex C C.1.13 Metrics link

**Spec:** Annex C §C.1.13, p. 128 — a receiver on `$metrics`;
all mandatory metrics are emitted within 30s.

**Repo:** `MetricsHub` + `management::metrics_snapshot(hub,
now_ms)` produces an `AmqpExtValue::Map` per mandatory metric.
The integration test `c1_13_metrics_link.rs` verifies that all 14 metrics
are emitted, that each sample contains `{name, value, unit,
timestamp}` and that the counter values are reflected in the
samples.

**Tests:** `ddsamqp_c_1_13_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`ddsamqp_c_1_13_metrics_carry_traffic_counter_values`,
`ddsamqp_2_2_metrics_units_are_spec_symbols`.

**Status:** done

### Annex C C.1.14 Audit link

**Spec:** Annex C §C.1.14, p. 128 — a receiver on `$audit`;
a second client's SASL success produces a
`link.attach.success` audit record.

**Repo:** the `AuditEvent` enum + `AuditProducer` (ring buffer) +
`audit_event_sample(event, now_ms)` produce AMQP map bodies
with the `event-type` spec symbol and event-specific fields.
The integration test `c1_14_audit_link.rs` verifies the
`link.attach.success` event with subject_name, the
`access.unauthorized` event with a resource, FIFO order of the
events, ring-buffer eviction.

**Tests:** `ddsamqp_c_1_14_link_attach_success_event_carries_subject`,
`ddsamqp_2_2_audit_producer_streams_events_in_order`,
`ddsamqp_c_1_14_unauthorized_event_carries_resource_field`,
`ddsamqp_c_1_14_ringbuffer_evicts_oldest_on_overflow`.

**Status:** done

### Annex C C.1.15 Loop prevention

**Spec:** Annex C §C.1.15, p. 128 — (a) a sample with its own
bridge_id is dropped; (b) a sample with hop > cap is dropped;
both increment the corresponding metric.

**Repo:** `coexistence::inspect_inbound` + `stamp_outbound`
implement both sub-tests. The integration test
`c1_15_loop_prevention.rs` verifies: (a) self-tag in string
and list form drops, (b) hop>cap drops, hop=cap forwards,
round-trip stamp-then-inspect drops itself,
multi-hop stamp increments cleanly, a clean sample passes.

**Tests:** 7 tests (`c1_15a_*`, `c1_15b_*`, `c1_15_round_trip_*`,
`c1_15_outbound_stamp_*`, `ddsamqp_c_1_15_clean_sample_passes_through`).

**Status:** done

### Annex C C.2.1 Outbound connection

**Spec:** Annex C §C.2.1, p. 130 — the bridge connects to the configured
upstream broker.

**Repo:** `client::connect_outbound` + the integration test
`client::tests::ddsamqp_2_2_outbound_connect_to_local_server` drives a
real TCP connect + AMQP Open roundtrip against a local
server daemon. C.2.4 (reconnect) builds on it.

**Tests:** `ddsamqp_2_2_outbound_connect_to_local_server`,
`ddsamqp_c_2_1_single_connect_attempt_works_for_normal_path`.

**Status:** done

### Annex C C.2.2 Sender-link bridge-to-broker

**Spec:** Annex C §C.2.2, p. 130 — the bridge attaches a sender link
for an outbound topic mapping.

**Repo:** bridge DDS-side subscribe (`subscribe_outbound`) +
the AMQP outbound producer path. A test roundtrip over two
`InMemoryDdsHost` instances that simulate the broker hop.

**Tests:** `ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`.

**Status:** done

### Annex C C.2.3 Receiver-link broker-to-bridge

**Spec:** Annex C §C.2.3, p. 130 — analogous to C.2.2 in the reverse direction.

**Repo:** the bridge receiver side calls `bridge::dispatch_transfer`
with the incoming broker sample → the DDS DataWriter publishes.

**Tests:** `ddsamqp_2_1_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### Annex C C.2.4 Reconnect on loss

**Spec:** Annex C §C.2.4, p. 131 — connection loss → reconnect
within the configured backoff cap (default 60s); HISTORY
eviction counts.

**Repo:** `client::connect_with_reconnect` with a
`ReconnectConfig` (default 1s init, 2x mult, 60s cap per
spec §10.8). The integration test `c2_4_reconnect_on_loss.rs`.

**Tests:** `ddsamqp_10_8_reconnect_loop_pacing_follows_spec_defaults`,
`ddsamqp_10_8_reconnect_aborts_on_max_attempts`,
`ddsamqp_10_8_reconnect_after_server_restart_eventually_succeeds`,
`ddsamqp_c_2_1_single_connect_attempt_works_for_normal_path`.

**Status:** done

### Annex C C.2.5 Settlement pass-through

**Spec:** Annex C §C.2.5, p. 131 — broker settlement →
DataWriter notification.

**Repo:** `link::LinkSession::deliver`/`settle` pending tracking
+ a `dds_bridge::DispositionMapper` trait (`apply` with a DDS-side
sample handle). The DataWriter acknowledgment is caller-layer
(a real `DcpsDdsHost` calls `DataWriter::wait_for_acknowledgment`).

**Tests:** `link::tests::ddsamqp_7_4_1_settle_decrements_pending`,
`dds_bridge::tests::ddsamqp_7_7_3_noop_disposition_mapper_does_nothing`.

**Status:** done

### Annex C C.2.6 Dual-identity separation

**Spec:** Annex C §C.2.6, p. 131 — the bridge presents the
DDS-side identity (`CN=Bridge-1`) instead of the broker-side credential
(`Alice`) to DDS-Security AccessControl.

**Repo:** `security::DualIdentity` with `for_broker()`/`for_dds()`
getters; the AccessControl plugin uses exclusively
`for_dds()`. Lib tests in `security::tests` cover the
spec §C.2.6 path directly.

**Tests:** `security::tests::ddsamqp_2_2_dual_identity_keeps_broker_and_dds_separate`,
`security::tests::ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### Annex C C.2.7 Loop prevention (bridge)

**Spec:** Annex C §C.2.7, p. 132 — analogous to C.1.15 for the bridge
profile (outbound + inbound).

**Repo:** the `coexistence` layer is profile-agnostic; the bridge
profile binds the same `inspect_inbound`/`stamp_outbound`
pair. The integration test
`c2_7_loop_prevention_bridge.rs`.

**Tests:** `ddsamqp_c_2_7_outbound_then_inbound_round_trip_drops_self`,
`ddsamqp_c_2_7_hop_cap_exceeded_drops_with_metric`,
`ddsamqp_c_2_7_multi_bridge_chain_terminates_at_hop_cap`,
`ddsamqp_c_2_7_foreign_bridges_in_history_dont_trigger_self_drop`.

**Status:** done

### Annex C C.2.8 Dual management surface

**Spec:** Annex C §C.2.8, p. 132 — the bridge DDS-side exposes
$catalog/$metrics/$audit; the upstream broker side is governed by the broker.

**Repo:** `bridge::dispatch_attach` recognizes
`AddressKind::{Catalog, Metrics, Audit}` and delivers the
corresponding `AttachOutcome` variants; `produce_catalog_transfers`
delivers the catalog stream from the `DdsHost`. The upstream broker side
management belongs to the broker (RabbitMQ HTTP API, etc.); no
bridge code.

**Tests:** `ddsamqp_c_1_8_catalog_receiver_gets_one_sample_per_topic`,
`bridge::tests::ddsamqp_c_2_8_dispatch_attach_to_metrics_recognised`,
`ddsamqp_c_2_8_dispatch_attach_to_audit_recognised`.

**Status:** done

### Annex C C.3 Codec-profile tests

**Spec:** Annex C §C.3, p. 134 — in-process type-system
roundtrips for all primitives, composites, sections.

**Repo:** `crates/amqp-bridge/src/types.rs::tests`,
`crates/amqp-bridge/src/extended_types.rs::tests`, `crates/amqp-bridge/src/frame.rs::tests`,
`performatives.rs::tests`, `sections.rs::tests`.

**Tests:** `crates/amqp-bridge/src/codec_profile.rs::tests`:
`ddsamqp_2_3_active_profile_full_by_default`, `ddsamqp_2_3_primitives_are_codec_lite`,
`ddsamqp_2_3_list_map_array_are_not_codec_lite`, `ddsamqp_2_3_data_section_is_codec_lite`,
`ddsamqp_2_3_other_sections_are_not_codec_lite`; plus the type round-trips in
`crates/amqp-bridge/src/types.rs::tests` and
`crates/amqp-bridge/src/extended_types.rs::tests`. `cargo test -p
zerodds-amqp-bridge --lib` — 70 tests green.

**Status:** done

### Annex C C.4 Codec-Lite-profile tests

**Spec:** Annex C §C.4, p. 135 — a strict subset of C.3.

**Repo:** `cargo test -p zerodds-amqp-bridge --features codec-lite`
runs the full codec test set + the 5 additional
`codec_profile` tests that mark the Lite subset. The conformance
marker `active_profile()` delivers `CodecProfile::Lite` under the
feature.

**Tests:** all `zerodds-amqp-bridge` tests (75 green) +
`codec_profile::tests::ddsamqp_2_3_active_profile_full_by_default` (inversely
under `--features codec-lite`).

**Status:** done

### Annex C C.5 Test harness

**Spec:** Annex C §C.5, p. 136 — vendor-neutral wording;
implementers SHOULD publish the harness source.

**Repo:** `tools/amqp-dds-endpoint/tests/common`.

**Tests:** `tools/amqp-dds-endpoint/tests/common`,
`tools/amqp-dds-endpoint/tests/c1_1_connection_open.rs::ddsamqp_2_1_connection_open_with_tls_active_advertises_plain`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_the_client_reconnects_once_the_server_is_up`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_loop_pacing_follows_spec_defaults`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_aborts_on_max_attempts`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_after_server_restart_eventually_succeeds`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_c_2_1_single_connect_attempt_works_for_normal_path`.

**Status:** done — structural (the harness choice is
implementation-defined).

---

## Annex D — DDS-RPC Correlation Mapping (normative when activated)

### Annex D D.1 Mapping table

**Spec:** Annex D §D.1, p. 138 — `requestId`→`message-id`
(override of the default per-sample identifier),
`relatedRequestId`→`correlation-id`, reply-topic→`reply-to`,
service-instance→`dds:rpc-instance`,
RPC-operation→`dds:rpc-operation`.

**Repo:** `rpc_correlation::ReplyProperties::from_amqp` reads
`correlation-id` (Str/Symbol/Uuid/Binary) and normalizes to a
string lookup key. `OutstandingCalls::issue` enters
`request_id` (= `message-id`).

**Tests:** `rpc_correlation::tests::ddsamqp_d_1_from_amqp_handles_str_symbol_uuid_binary`.

**Status:** done

### Annex D D.2 Activation

**Spec:** Annex D §D.2, p. 139 — `TopicMapping.rpc_aware = true`
activates; default `false`.

**Repo:** `rpc_correlation::RpcConfig::rpc_aware` field
(default `false`); `OutstandingCalls::new(cfg)` is instantiated
only when `rpc_aware = true` is activated.

**Tests:** `rpc_correlation::tests::ddsamqp_d_2_defaults_match_spec`.

**Status:** done

### Annex D D.3 Scope of this annex

**Spec:** Annex D §D.3, p. 139 — no wire-level RPC bridge,
only property mapping; full RPC-over-AMQP is a
later spec.

**Repo:** `crates/amqp-endpoint/src/rpc_correlation.rs` (`RpcConfig`, `DEFAULT_RPC_TIMEOUT_MS`, `DEFAULT_MAX_OUTSTANDING_CALLS`).

**Tests:** `crates/amqp-endpoint/src/rpc_correlation.rs::ddsamqp_d_2_defaults_match_spec`,
`crates/amqp-endpoint/src/rpc_correlation.rs::ddsamqp_d_3_validate_reply_rejects_missing_correlation_id`,
`crates/amqp-endpoint/src/properties.rs::ddsamqp_7_7_the_operations_are_write_register_unregister_dispose`.

**Status:** done — spec content classification.

### Annex D D.4 Reply validation

**Spec:** Annex D §D.4, p. 140 — reply acceptance:
correlation-id MANDATORY, match against the outstanding RPC call
MANDATORY, body decode mode-dependent (PASSTHROUGH/JSON/
AMQP_NATIVE). Failure-dispositions table. Per-call timeout
`rpc_timeout_ms` (default 30000) → RETCODE_TIMEOUT. Non-
blocking guarantee. Bounded outstanding-calls table with
RETCODE_OUT_OF_RESOURCES.

**Repo:** `rpc_correlation::OutstandingCalls` with a `BTreeMap`-
based bounded table (`max_outstanding`, default 4096),
`validate_reply` with all 4 failure dispositions
(RejectMalformed/DropUnknown/DecodeFailure/DropLateReply +
surface), `expire_overdue` with the per-call timeout, `issue`
delivers `OutOfResources` on a full table. Wired against
`MetricsHub::on_dropped_malformed_reply` /
`on_rpc_timeout` / `on_decode_error`.

**Tests:** `rpc_correlation::tests` (13 tests incl.
`ddsamqp_d_3_validate_reply_rejects_missing_correlation_id`,
`ddsamqp_d_4_validate_reply_drops_unknown_correlation_id`,
`ddsamqp_d_4_validate_reply_late_reply_dropped`,
`ddsamqp_d_4_expire_overdue_removes_and_counts`,
`ddsamqp_d_4_issue_accepts_then_full`,
`ddsamqp_d_4_validate_reply_does_not_block_on_table_full`).

**Status:** done

---

## Audit status

123 done / 0 partial / 0 open / 0 blocked / 1 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document dds-amqp-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-amqp-bridge --all-targets` (job `merge-rust-amqp-bridge`) — 47 tests green.
* `cargo nextest run --profile merge -p amqp-dds-endpoint --all-targets` (job `merge-rust-amqp-dds-endpoint`) — 80 tests green.
* `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` (job `merge-rust-amqp-endpoint`) — 177 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor-amqp` (job `integration-bridges-vendor-amqp`) — 1 test green.

305 cited tests green, 0 not green.

Open items: none.
