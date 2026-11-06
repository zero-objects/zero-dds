# DDS-AMQP 1.0 — ZeroDDS Vendor-Spec Coverage

**Quelle:** `documentation/specs/dds-amqp-1.0/main.tex` (Tag
`spec-dds-amqp-v1.0.0-beta1`, PDF
`documentation/specs/releases/v1.0-beta1/dds-amqp-1.0-beta1.pdf`).

**Kontext:** Die Repo-Implementation ist über zwei Crates verteilt:

- `crates/amqp-bridge/` — Codec-Profile
- `crates/amqp-endpoint/` — Endpoint-/Bridge-Profile-Operations-Schichten

---

## §2 Conformance

### §2.1 Endpoint Profile — Clause 1 (Type-System)

**Spec:** §2.1, S. 7 (PDF) — "It supports the AMQP type-system as
specified in §sec:pim-type-mapping."

**Repo:** `crates/amqp-bridge/src/types.rs::AmqpValue`,
`crates/amqp-bridge/src/extended_types.rs::AmqpExtValue`.

**Tests:** `crates/amqp-bridge/src/types.rs::tests`:
`amqp10_1_6_1_null_is_0x40`, `amqp10_1_6_2_a_boolean_is_0x41_0x42_or_0x56_and_an_octet`,
`amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`, `amqp10_1_6_10_a_long_is_smalllong_or_eight_octets`, `amqp10_1_6_19_binary_is_vbin8_or_vbin32`, `amqp10_1_6_20_a_string_is_its_utf_8_octets_after_the_length`, `amqp10_1_6_21_a_symbol_is_ascii_after_sym8_or_sym32`,
`amqp10_1_2_the_high_nibble_names_the_subcategory` sowie die drei Fehlerfälle
`amqp10_1_2_an_unassigned_code_or_a_cut_encoding_is_refused`,
`amqp10_1_6_20_octets_that_are_not_utf_8_are_refused`.
`crates/amqp-bridge/src/extended_types.rs::tests` deckt die
erweiterten Codes ab, darunter `ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`,
`ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`,
`amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`, `amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets`,
`amqp10_1_6_18_a_uuid_is_0x98_and_sixteen_octets`.

**Status:** done

### §2.1 Endpoint Profile — Clause 2 (Connection Acceptance)

**Spec:** §2.1 — "It accepts incoming AMQP connections in
accordance with §topology-direct and the AMQP-1.0 connection-state
model."

**Repo:** `crates/amqp-endpoint/src/session.rs::ConnectionState`
+ `advance_connection()` (State-Machine).
`tools/amqp-dds-endpoint/` (Daemon-Crate):
* `frame_io::read_protocol_header`/`write_protocol_header` —
  AMQP/SASL Protocol-Header.
* `frame_io::read_frame`/`write_frame` — 8B-Header + Body.
* `handler::handle_connection` — treibt Open/Begin/Attach/
  Transfer/Close-Loop, drives `advance_connection`.
* `server::run_server` — `std::net::TcpListener` + thread-per-
  connection, set_nonblocking + shutdown-Atomic.

**Tests:** `session::tests::*` (6 State-Machine-Tests),
`amqp_dds_endpoint::frame_io::tests` (10 Tests),
`handler::tests::*` (6 Tests inkl.
`ddsamqp_2_1_handle_connection_open_close_round_trip`,
`ddsamqp_2_1_handle_connection_sasl_then_amqp`),
`server::tests::ddsamqp_2_1_server_accepts_connection_and_handles_open_close`
(echte TCP-Roundtrip).

**Status:** done

### §2.1 Endpoint Profile — Clause 3 (Sender + Receiver Links)

**Spec:** §2.1 — "It accepts both Sender and Receiver link
attachments as specified in §pim-settlement, and translates them
bidirectionally into DDS DataWriter and DataReader operations."

**Repo:** `crates/amqp-endpoint/src/link.rs::LinkSession` (Settlement)
+ `tools/amqp-dds-endpoint/src/dds_host.rs::DdsHost`-Trait
+ `InMemoryDdsHost` (Topic-Registry, publish/subscribe-API)
+ `tools/amqp-dds-endpoint/src/bridge.rs::dispatch_attach`/
`dispatch_transfer`/`subscribe_outbound` translaten zwischen
AMQP-Frame-Bodies und DDS-Side. Konkrete DCPS-Wire-Anbindung
ist Folge-Welle (DcpsDdsHost-Implementer).

**Tests:** `link::tests` (7 Tests),
`dds_host::tests` (10 Tests),
`bridge::tests` (10 Tests),
`c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_amqp_producer_publishes_to_dds_via_bridge`,
`ddsamqp_2_1_dds_publish_flows_to_amqp_consumer_callback`,
`ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`,
`ddsamqp_2_1_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### §2.1 Endpoint Profile — Clause 4 (Address Resolution)

**Spec:** §2.1 — "It implements address resolution as specified in
§pim-address-resolution."

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
mit Static-Aliases, `domain://N/topic?partition=P[&partition=Q]`-
URLs (Multi-Partition-Sequence + percent-decoding), Wildcard-
Pattern-Matching, `effective_partitions` Conflict-Resolver.

**Tests:** `routing::tests` (12 Tests inkl.
`ddsamqp_2_1_domain_url_with_multi_partition_parses`,
`ddsamqp_2_1_domain_url_with_percent_encoded_partition`,
`ddsamqp_2_1_conflict_resolution_uri_wins_when_both_present`).

**Status:** done

### §2.1 Endpoint Profile — Clause 5 (Body Encoding)

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

**Status:** done — Pass-Through voll, JSON als Hex-Fallback
spec-konform (volle Type-Reflection-JSON ist Codegen-Layer).

### §2.1 Endpoint Profile — Clause 6 (SASL Mechanisms)

**Spec:** §2.1 — "It supports the SASL PLAIN, ANONYMOUS, and
EXTERNAL mechanisms as specified in §security-sasl."

**Repo:** `crates/amqp-endpoint/src/sasl.rs::SaslMechanism`
{Plain, Anonymous, External} + `SaslState::new`/`step` State-
Machine.

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests`:
`amqp10_5_plain_authenticates_on_correct_credentials`,
`amqp10_5_plain_fails_on_wrong_credentials`, `ddsamqp_2_1_plain_without_tls_yields_auth_failed`,
`ddsamqp_2_1_plain_offered_only_when_tls_active`,
`amqp10_5_anonymous_authenticates_without_credentials`,
`amqp10_5_external_uses_transport_subject`, `ddsamqp_2_1_select_outbound_prefers_external`,
`ddsamqp_2_1_select_outbound_falls_back_to_plain_with_tls`,
`ddsamqp_2_1_select_outbound_skips_plain_without_tls`,
`ddsamqp_2_1_select_outbound_no_acceptable_mechanism`;
das Endpoint-Programm (`tools/amqp-dds-endpoint/src/auth.rs`,
`tools/amqp-dds-endpoint/src/handler.rs`, Credential-Store im RFC-5803-Format):
`tools/amqp-dds-endpoint/tests/sasl_server_checks.rs::{ddsamqp_2_1_plain_on_tls_checks_the_password_against_the_store,
ddsamqp_2_1_scram_sha_256_authenticates_and_proves_the_server,
ddsamqp_2_1_an_init_with_a_mechanism_not_offered_is_auth,
ddsamqp_2_1_skipping_sasl_is_refused_when_anonymous_is_not_enabled}`,
`tools/amqp-dds-endpoint/src/auth.rs::tests::ddsamqp_2_1_the_policy_refuses_what_cannot_work_and_offers_per_connection`.
`tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_2_1_an_mtls_client_authenticates_with_external`
(mTLS mit CA-signiertem Client-Zertifikat, Subject `mtls:<SHA-256>`).

**Status:** done

### §2.1 Endpoint Profile — Clause 7 (Mandatory TLS for PLAIN)

**Spec:** §2.1 Clause 7 (zitiert aus dds-amqp-1.0 §2.1 Endpoint
Profile Item 7): "It SHALL NOT offer SASL PLAIN to a client whose
underlying transport is unencrypted (§2.x.x). A client SASL-init
frame carrying PLAIN on an unencrypted connection SHALL be rejected
with SASL outcome `auth`, and the connection SHALL be closed."

**Repo:** `SaslState::new(tls_active: bool)` filtert PLAIN aus
der Mechanism-Liste wenn `tls_active = false`
(`crates/amqp-endpoint/src/sasl.rs::SaslState::new`).
Inbound PLAIN auf Klartext liefert spec-konform
`SaslOutcome::Failed { code: SaslCode::Auth, .. }` (Wire-Code 1
nach OASIS AMQP 1.0 §5.3.3.6); neuer Helper `auth_failed()`,
neuer `SaslCode`-Enum mit `Ok/Auth/Sys/SysPerm/SysTemp` (Codes
0-4) und `wire_code()`-Accessor.

**Tests:** `sasl::tests::ddsamqp_2_1_plain_offered_only_when_tls_active`,
`ddsamqp_2_1_plain_without_tls_yields_auth_failed`,
`amqp10_5_3_3_6_the_sasl_codes_are_0_to_4`,
`amqp10_5_sasl_code_unknown_value_rejected`,
`ddsamqp_2_1_outcome_authenticated_wire_code_is_ok`,
`ddsamqp_2_1_outcome_auth_failed_helper_uses_auth_code`;
am Endpoint-Programm auf dem Draht:
`tools/amqp-dds-endpoint/tests/sasl_server_checks.rs::ddsamqp_2_1_a_plain_init_on_plaintext_is_auth_and_the_connection_closes`
(PLAIN nicht angeboten; eine PLAIN-Init auf Klartext bekommt Outcome 1,
dann EOF).

**Status:** done — Filter-Logik + Spec-konforme Outcome-Codes
nach OASIS AMQP 1.0 §5.3.3.6.

### §2.1 Endpoint Profile — Clause 8 (No-Bypass + Per-Link Governance)

**Spec:** §2.1 — "It enforces the No-Bypass guarantee for
DDS-Security identities (§security-no-bypass) and per-link
Governance resolution (§per-link-governance)."

**Repo:** `crates/amqp-endpoint/src/security.rs` Trait +
Plugins. `tools/amqp-dds-endpoint/src/handler.rs::check_access`
ruft das Plugin pre-Attach (AttachSender/AttachReceiver) und
pre-Transfer (ReceiveSample) auf. Bei Deny: Detach-Frame mit
`amqp:unauthorized-access` + `errors.unauthorized`-Counter.

**Tests:** `security::tests::ddsamqp_2_1_link_governance_caches_decision`,
`handler::tests::ddsamqp_2_1_access_control_deny_attach_yields_unauthorized_metric`,
`ddsamqp_2_1_access_control_allow_does_not_increment_unauthorized`.

**Status:** done

### §2.1 Endpoint Profile — Clause 9 (Management Surface + Audit)

**Spec:** §2.1 — "It exposes the operational management surface
defined in §management-interface (the `$catalog` and `$metrics`
addresses) and the security audit channel defined in
§audit-channel (the `$audit` address)."

**Repo:** `crates/amqp-endpoint/src/management.rs` mit
`CatalogProducer` (§7.5/§7.9.1), `metrics_snapshot` Sample-
Stream (§7.9.2), `AuditProducer` Ringbuffer (§sec:audit-channel),
`classify_address` Reserved-Address-Resolver.

**Tests:** `management::tests` (12 Tests inkl.
`ddsamqp_2_1_catalog_snapshot_emits_map_per_entry`,
`ddsamqp_2_1_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`ddsamqp_2_1_audit_producer_ringbuffer_evicts_oldest`).

**Status:** done

### §2.1 Endpoint Profile — Clause 10 (Bridge-Coexistence)

**Spec:** §2.1 — "It implements the bridge-coexistence rules
(§bridge-coexistence) for every sample it forwards between DDS and
AMQP."

**Repo:** `crates/amqp-endpoint/src/coexistence.rs`:
`CoexistenceConfig` (process-wide bridge_id + hop_cap),
`inspect_inbound` (DropLoop + DropHopCap + Forward),
`stamp_outbound` (bridge_id-list + hop-counter).

**Tests:** `coexistence::tests` (11 Tests inkl.
`ddsamqp_2_1_drop_loop_on_self_tag_string`, `ddsamqp_2_1_drop_hop_cap_when_exceeded`,
`ddsamqp_2_1_round_trip_stamp_then_inspect_drops_loop`).

**Status:** done

### §2.1 Endpoint Profile — Clause 11 (Annex C Tests)

**Spec:** §2.1 — "It satisfies the conformance test cases
enumerated in Annex C, §C.1."

**Repo:** keine Annex-C-Test-Suite im Repo.

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

### §2.2 Bridge Profile — Clause 1 (Type-System)

**Spec:** §2.2, S. 8 (PDF) — siehe Endpoint Cl. 1.

**Repo:** identisch zu Endpoint (geteilte Codec-Crate).

**Tests:** `crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_every_row_of_the_primitive_table_takes_the_code_it_names`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_an_octet_sequence_is_binary_and_a_long_sequence_is_a_list`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_a_type_outside_the_mapping_is_named_rather_than_encoded_as_something_else`,
`crates/amqp-bridge/src/types.rs::amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`,
`crates/amqp-bridge/src/extended_types.rs::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`.

**Status:** done

### §2.2 Bridge Profile — Clause 2 (Outbound Connection)

**Spec:** §2.2 — "It establishes outgoing AMQP connections to an
external broker as specified in §topology-sidecar."

**Repo:** `tools/amqp-dds-endpoint/src/client.rs::connect_outbound`:
TCP-Connect mit Timeout, SASL-Header → SASL-Init →
SASL-Outcome → AMQP-Header → Open. `OutboundSession` liefert
`remote_container_id` aus dem Open-Reply.

**Tests:** `client::tests::ddsamqp_2_2_outbound_connect_to_local_server`
(echte E2E-Roundtrip gegen lokalen Server).

**Status:** done

### §2.2 Bridge Profile — Clause 3 (Address Resolution)

**Spec:** §2.2 — Bridge teilt das Endpoint-Address-Resolution-Modell
(§7.3) inklusive `domain://` URL-Form, statischer Aliase, Wildcard-
Matching.

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
(geteilt mit Endpoint Cl. 4); Bridge-Aufrufer in
`crates/amqp-endpoint/src/client.rs`.

**Tests:** `crates/amqp-endpoint/src/routing.rs::tests` (12 Tests):
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

**Repo:** `client::do_outbound_sasl`: parst sasl-mechanisms,
ruft `SaslState::select_outbound`, baut `sasl-init` (RFC-4616
PLAIN-Form `\0user\0pw`, ANONYMOUS-Marker, leerer EXTERNAL-Body),
liest `sasl-outcome` (code 0 = ok).

**Tests:** `client::tests::ddsamqp_2_2_parse_offered_mechanisms_array_form`,
`ddsamqp_2_2_parse_offered_mechanisms_single_symbol`,
`ddsamqp_2_2_parse_offered_mechanisms_unknown_filtered`,
`ddsamqp_2_2_sasl_init_plain_includes_credentials`,
`ddsamqp_2_2_sasl_init_anonymous_uses_marker`,
`ddsamqp_2_2_sasl_init_external_has_empty_response`.

**Status:** done

### §2.2 Bridge Profile — Clause 5 (PLAIN auf Klartext verboten)

**Spec:** §2.2 — "It SHALL NOT initiate a SASL PLAIN authentication
over an unencrypted transport."

**Repo:** `sasl::SaslState::select_outbound(offered, tls_active)`
filtert PLAIN aus der Auswahl-Präzedenz wenn `tls_active=false`.

**Tests:** `sasl::tests::ddsamqp_2_1_select_outbound_skips_plain_without_tls`,
`ddsamqp_2_1_select_outbound_no_acceptable_mechanism`.

**Status:** done

### §2.2 Bridge Profile — Clause 6 (Dual-Identity)

**Spec:** §2.2 — "It honours the dual-identity model of
§bridge-dual-identity: broker-side SASL credentials SHALL NOT be
conflated with the bridge's DDS-Security IdentityToken."

**Repo:** `security::DualIdentity` mit `for_broker()` /
`for_dds()` Getter. Plugin-Calls referenzieren ausschließlich
`for_dds()`.

**Tests:** `security::tests::ddsamqp_2_2_dual_identity_keeps_broker_and_dds_separate`,
`ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### §2.2 Bridge Profile — Clause 7 (Reconnect mit Backoff)

**Spec:** §2.2 — "It performs reconnect with exponential back-off
after connection loss as specified in §reconnect."

**Repo:** `client::ReconnectConfig` (initial 1s, mult 2, cap 60s
gemäß Spec) + `connect_with_reconnect` mit kooperativem
shutdown-Atomic. `next_backoff_ms(attempt)` cap-correct.

**Tests:** `client::tests::ddsamqp_2_2_backoff_starts_at_initial`,
`ddsamqp_2_2_backoff_doubles_until_cap`, `ddsamqp_2_2_backoff_respects_custom_cap`,
`ddsamqp_2_2_backoff_with_unit_multiplier_stays_at_initial`,
`ddsamqp_2_2_reconnect_exhausts_with_max_attempts`,
`ddsamqp_2_2_reconnect_aborts_on_shutdown_signal`.

**Status:** done

### §2.2 Bridge Profile — Clause 8 (Body Encoding)

**Spec:** §2.2 — siehe Endpoint Cl. 5.

**Repo:** identisch.

**Tests:** `crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_the_body_form_follows_the_types_extensibility`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_a_mutable_body_is_keyed_by_the_declared_member_ids`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_a_union_plans_its_discriminator_and_then_its_branches`,
`crates/amqp-bridge/src/sections.rs::amqp10_3_2_8_an_amqp_value_is_0x77_and_any_value`,
`crates/amqp-bridge/src/sections.rs::amqp10_3_2_the_sections_follow_in_their_order`,
`crates/amqp-endpoint/src/mapping.rs::ddsamqp_2_1_amqp_native_uses_correct_content_type`.

**Status:** done

### §2.2 Bridge Profile — Clause 9 (Management Surface + Audit)

**Spec:** §2.2 — "It exposes the operational management surface
... and the security audit channel ... for the DDS-side surface
of the bridge."

**Repo:** identisch zu Endpoint Cl. 9 (geteiltes
`management`-Modul).

**Tests:** `crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_classify_address_recognises_reserved`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_1_catalog_snapshot_emits_map_per_entry`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_1_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_audit_event_types_are_spec_symbols`,
`tools/amqp-dds-endpoint/tests/c1_13_metrics_link.rs::ddsamqp_2_2_metrics_units_are_spec_symbols`,
`tools/amqp-dds-endpoint/tests/c1_14_audit_link.rs::ddsamqp_2_2_audit_producer_streams_events_in_order`.

**Status:** done

### §2.2 Bridge Profile — Clause 10 (Bridge-Coexistence)

**Spec:** §2.2 — siehe Endpoint Cl. 10.

**Repo:** identisch (geteiltes `coexistence`-Modul).

**Tests:** `crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_1_drop_loop_on_self_tag_string`,
`crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_2_drop_loop_on_self_tag_list`,
`crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_1_drop_hop_cap_when_exceeded`,
`crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_1_round_trip_stamp_then_inspect_drops_loop`,
`crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_2_other_bridges_tag_does_not_drop`,
`tools/amqp-dds-endpoint/tests/c2_7_loop_prevention_bridge.rs`.

**Status:** done

### §2.2 Bridge Profile — Clause 11 (Annex C Tests)

**Spec:** §2.2 — "It satisfies the conformance test cases
enumerated in Annex C, §C.2."

**Repo:** —

**Tests:** `tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs`,
`tools/amqp-dds-endpoint/tests/c2_7_loop_prevention_bridge.rs`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### §2.3 Codec Profile

**Spec:** §2.3, S. 9 — Type-System, Performatives, Sections-Codec
ohne Connection-Lifecycle.

**Repo:** `crates/amqp-bridge/` voll: `crates/amqp-bridge/src/types.rs`, `crates/amqp-bridge/src/extended_types.rs`,
`crates/amqp-bridge/src/frame.rs`, `performatives.rs`, `sections.rs`.

**Tests:** `crates/amqp-bridge/src/codec_profile.rs::tests`:
`ddsamqp_2_3_active_profile_full_by_default`, `ddsamqp_2_3_primitives_are_codec_lite`,
`ddsamqp_2_3_list_map_array_are_not_codec_lite`, `ddsamqp_2_3_data_section_is_codec_lite`,
`ddsamqp_2_3_other_sections_are_not_codec_lite`; dazu die Typ-Roundtrips aus
`crates/amqp-bridge/src/types.rs::tests` und
`crates/amqp-bridge/src/extended_types.rs::tests`. `cargo test -p
zerodds-amqp-bridge --lib` — 70 Tests grün.

**Status:** done

### §2.4 Codec-Lite Profile

**Spec:** §2.4 — strikte Untermenge von Codec, nur Primitive +
`data` Body-Section, kein Compound.

**Repo:** `crates/amqp-bridge/src/codec_profile.rs` mit
`CodecProfile::{Full, Lite}`, `active_profile()` (steuert
durch Cargo-Feature `codec-lite`), `is_codec_lite_value()` und
`is_codec_lite_section()` — Conformance-Marker für Code, der
nur das Lite-Subset benutzt. `cargo test -p zerodds-amqp-bridge
--features codec-lite` aktiviert den Marker.

**Tests:** `codec_profile::tests` (5 Tests inkl.
`ddsamqp_2_3_primitives_are_codec_lite`,
`ddsamqp_2_3_list_map_array_are_not_codec_lite`,
`ddsamqp_2_3_data_section_is_codec_lite`,
`ddsamqp_2_3_other_sections_are_not_codec_lite`,
`ddsamqp_2_3_active_profile_full_by_default`).

**Status:** done

### §2.5 Combination of Profiles + §2.5.1 Codec-Implication

**Spec:** §2.5.1, S. 11 — Endpoint/Bridge implizieren Codec.

**Repo:** Cargo-Dependency: beide Crates hängen von
`amqp-bridge` ab; jede Endpoint-Operation nutzt den Codec.

**Tests:** `crates/amqp-bridge/src/codec_profile.rs::ddsamqp_2_3_active_profile_full_by_default`,
`crates/amqp-bridge/src/codec_profile.rs::ddsamqp_2_3_primitives_are_codec_lite`,
`crates/amqp-endpoint/src/mapping.rs::ddsamqp_2_1_amqp_native_uses_correct_content_type`,
`crates/amqp-endpoint/src/properties.rs::ddsamqp_2_5_application_properties_truncated_type_id_present`.

**Status:** done — strukturell durch Crate-Abhängigkeit
erzwungen.

### §2.5.2 Hybrid Profile Combination

**Spec:** §2.5.2 — Implementation MAY claim Endpoint und Bridge
gleichzeitig.

**Repo:** keine harte Trennung zwischen Endpoint- und Bridge-
Pfaden; Module sind shared.

**Tests:** `crates/amqp-endpoint/src/coexistence.rs::ddsamqp_2_5_2_config_defaults_match_spec`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_classify_address_recognises_reserved`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_amqp_producer_publishes_to_dds_via_bridge`,
`tools/amqp-dds-endpoint/tests/c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`.

**Status:** done

---

## §7 Platform-Independent Model

### §7.1.1 Primitive Mapping

**Spec:** §7.1.1, S. 17 — Mapping-Tabelle DDS-XCDR2-Primitive ↔
AMQP-§1.6-Primitive (boolean, int8/16/32/64, uint8/16/32/64,
float, double, char, wchar, string, wstring, octet).

**Repo:** `crates/amqp-bridge/src/types.rs` (15 Encoder/Decoder)
+ `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue` (16 weitere für Compound +
described).

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`,
`crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`,
`crates/amqp-bridge/src/extended_types.rs::tests::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`,
`crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_13_the_decimals_are_0x74_0x84_0x94_with_4_8_16_octets`.

**Status:** done

### §7.1.1 Primitive Mapping — `long double`

**Spec:** §7.1.2, S. 19 — `long double` (16 Byte) wird auf AMQP
`double` (8 Byte) verengt.

**Repo:** `codegen_helpers::narrow_long_double_to_double(f64)`
ist die Audit-Anker-Stelle für §7.1.2; Rust hat keinen
binary128-Typ, der Codegen liefert bereits f64.

**Tests:** `codegen_helpers::tests::ddsamqp_7_1_1_long_double_narrowing_is_identity_on_f64`.

**Status:** done

### §7.1.3 Endianness Handling

**Spec:** §7.1.3, S. 20 — alle AMQP-Multi-Byte-Werte sind
big-endian; XCDR2-Quelldaten sind little-endian.

**Repo:** `crates/amqp-bridge/src/types.rs` und `crates/amqp-bridge/src/extended_types.rs` schreiben
ausnahmslos `to_be_bytes()`/lesen `from_be_bytes()`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::ddsamqp_7_1_3_every_multi_octet_primitive_is_big_endian_on_the_wire`,
`crates/amqp-bridge/src/types.rs::amqp10_1_6_6_a_ulong_is_ulong0_smallulong_or_eight_octets`,
`crates/amqp-bridge/src/extended_types.rs::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`.

**Status:** done

### §7.1.4.1 IDL `char` (8-bit) → AMQP `char` (32-bit)

**Spec:** §7.1.4.1, S. 21 — IDL-`char` ist auf UTF-8-ASCII-
Subset beschränkt; Source-Bytes > 0x7F → `decode-error`.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_char` (4-Byte BE) +
`codegen_helpers::validate_char_ascii(byte) -> Result<u8, u8>`
für Codegen-Pre-Encode-Check. Codegen-Templates rufen
`validate_char_ascii` und propagieren `Err` als
`amqp:decode-error`.

**Tests:** `codegen_helpers::tests::ddsamqp_7_1_4_1_ascii_validates`,
`ddsamqp_7_1_4_1_non_ascii_rejected`.

**Status:** done

### §7.1.4.2 IDL `wchar` → AMQP `char`

**Spec:** §7.1.4.2, S. 22 — IDL-`wchar` als 4-Octet UCS-4 wire,
runtime platform-defined.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_char` produziert exakt
4-Byte-BE-Codepoint.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::ddsamqp_7_1_4_2_a_wchar_is_four_octet_ucs4_big_endian`,
`crates/amqp-bridge/src/extended_types.rs::amqp10_1_6_16_a_char_is_0x73_and_a_utf_32be_code_point`.

**Status:** done

### §7.1.4.3 String Transcoding (UTF-8)

**Spec:** §7.1.4.3, S. 23 — DDS `string` ↔ AMQP `str8/str32`
verbatim; `wstring` zu UTF-8 transkodiert.

**Repo:** `crates/amqp-bridge/src/types.rs::encode_string` (str8/str32),
`crates/amqp-bridge/src/types.rs::encode_symbol` (sym8/sym32), Length-Prefix-Validation.

**Tests:** `crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_20_a_string_is_its_utf_8_octets_after_the_length`,
`crates/amqp-bridge/src/types.rs::tests::amqp10_1_6_21_a_symbol_is_ascii_after_sym8_or_sym32`.

**Status:** done

### §7.1.5 Time Types

**Spec:** §7.1.5, S. 25 — DDS `Time_t` (sec+nanosec) ↔ AMQP
`timestamp` (8-Byte BE ms-since-epoch). Sub-Millisekunden via
`dds:nsec` Application-Property.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::encode_timestamp`/`decode_timestamp`
(Wire) + `properties::produce_application_properties` (App-
Property `dds:nsec` aus `SampleHeader.source_nsec_remainder`).

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::amqp10_1_6_17_a_timestamp_is_0x83_and_milliseconds_as_eight_octets`,
`crates/amqp-endpoint/src/properties.rs::tests::ddsamqp_8_3_application_properties_nsec_set_when_nonzero`.

**Status:** done

### §7.1.6.1 Identifier Types

**Spec:** §7.1.6.1, S. 26 — 16-Byte-Identifier ohne RFC-4122-
Konformität werden als `binary` (0xA0/0xB0) codiert, nicht als
`uuid` (0x98).

**Repo:** `codegen_helpers::is_rfc4122_uuid(&[u8;16]) -> bool`
prüft Variant+Version per RFC-4122; `encode_16byte_identifier`
routet automatisch auf `Uuid` oder `Binary`.

**Tests:** `codegen_helpers::tests::ddsamqp_7_1_6_1_rfc4122_v4_uuid_recognised`,
`ddsamqp_7_1_6_1_arbitrary_16_bytes_not_recognised_as_uuid`,
`ddsamqp_7_1_6_1_encode_16byte_routes_binary_for_non_uuid`,
`ddsamqp_7_1_6_1_encode_16byte_routes_uuid_when_marked`.

**Status:** done

### §7.1.7 Sequence and Array

**Spec:** §7.1.7, S. 27 — `sequence<T>` und Array auf AMQP
`list` (0xC0/0xD0) oder `array` (0xE0/0xF0); leere Sequenz als
`list0` (0x45).

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::List` + `Array` mit
list8/list32/array8/array32-Encoding; `list0`-Constant in
`codes::LIST0`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::{amqp10_1_6_22_a_list_is_list8_size_count_and_items, amqp10_1_6_24_an_array_is_one_constructor_then_the_values, ddsamqp_7_1_7_lists_maps_and_arrays_nest_with_their_own_sizes}`.

**Status:** done

### §7.1.8 Map

**Spec:** §7.1.8, S. 28 — DDS-Map (TK_MAP) ↔ AMQP `map` (0xC1/
0xD1).

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::Map`.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::tests::{amqp10_1_6_23_a_map_is_map8_size_count_and_pairs, ddsamqp_7_1_7_lists_maps_and_arrays_nest_with_their_own_sizes}`.

**Status:** done

### §7.2.1 Composite Descriptor (DESC_TRUNCATED)

**Spec:** §7.2.1.1, S. 30 — XTypes-TypeIdentifier wird auf erste
8 Octets als AMQP `ulong` (0x80) reduziert.

**Repo:** `codegen_helpers::compute_truncated_descriptor(&[u8])
-> Result<u64, _>` liefert die ersten 8 Bytes als BE-u64;
`route_descriptor(DESC_TRUNCATED, hash)` liefert
`(Some(ulong), None)`.

**Tests:** `codegen_helpers::tests::ddsamqp_7_2_1_truncated_descriptor_first_8_bytes_be`,
`ddsamqp_7_2_1_truncated_descriptor_too_short_errors`,
`ddsamqp_7_2_1_route_descriptor_truncated`.

**Status:** done

### §7.2.1 Composite Descriptor (DESC_FULL)

**Spec:** §7.2.1.2, S. 30 — XTypes-TypeIdentifier in voller
Form als AMQP `symbol` (sym8/sym32) `dds:type:<hex>`.

**Repo:** `codegen_helpers::make_full_descriptor_symbol(&[u8])
-> String` liefert `dds:type:<lowercase-hex>`;
`route_descriptor(DESC_FULL, hash)` liefert
`(None, Some(symbol))`.

**Tests:** `codegen_helpers::tests::ddsamqp_7_2_1_full_descriptor_symbol_format`,
`ddsamqp_7_2_1_route_descriptor_full`.

**Status:** done

### §7.2.1.3 Collision Detection mit dds:type-id

**Spec:** §7.2.1.3, S. 31 — bei `descriptor_form = DESC_TRUNCATED`
muss Sender Application-Property `dds:type-id` (volle 14-Byte-
Hex) setzen; Empfänger SHALL inspect; bei Mismatch
`amqp:decode-error`.

**Repo:** Sender-Side: `produce_application_properties`. Empfänger-
Side: `properties::inspect_dds_type_id(props, expected_hex)`
liefert `TypeIdCheck::{Match, Absent, Mismatch}`. Caller (Daemon)
mappt Mismatch auf `amqp:decode-error`.

**Tests:** `properties::tests::ddsamqp_7_2_1_3_type_id_inspector_match_returns_match`,
`ddsamqp_7_2_1_3_type_id_inspector_case_insensitive_match`,
`ddsamqp_7_2_1_3_type_id_inspector_absent_returns_absent`,
`ddsamqp_7_2_1_3_type_id_inspector_mismatch_detects_collision`,
`ddsamqp_7_2_1_3_type_id_inspector_accepts_symbol_form`,
`ddsamqp_7_2_1_3_type_id_inspector_non_map_yields_absent`.

**Status:** done

### §7.2.2 Composite Body

**Spec:** §7.2.2, S. 32 — Body als AMQP `list` (Constructor
0xC0/0xD0) der Field-Werte in IDL-Order.

**Repo:** `crates/amqp-bridge/src/extended_types.rs::AmqpExtValue::List` + Section-
Producer.

**Tests:** `crates/amqp-bridge/src/extended_types.rs::amqp10_1_6_22_the_empty_list_is_list0`,
`crates/amqp-bridge/src/extended_types.rs::amqp10_1_6_22_a_list_is_list8_size_count_and_items`,
`crates/amqp-bridge/tests/type_mapping.rs::ddsamqp_2_2_the_body_form_follows_the_types_extensibility`,
`crates/amqp-bridge/tests/type_mapping.rs::amqp10_1_1_a_nested_composite_carries_its_own_descriptor`.

**Status:** done

### §7.2.3 Union

**Spec:** §7.2.3, S. 33 — Union als AMQP `list` mit Discriminator
+ Active-Branch-Wert.

**Repo:** `codegen_helpers::make_union_body(discriminator,
active_value: Option<_>)` baut die `AmqpExtValue::List` mit
1-2 Elementen je nach aktivem Branch.

**Tests:** `codegen_helpers::tests::ddsamqp_7_2_3_union_with_branch_has_two_elements`,
`ddsamqp_7_2_3_union_empty_branch_omits_value`.

**Status:** done

### §7.3 Address-Resolution Model

**Spec:** §7.3, S. 35 — Topic-Name ↔ AMQP source/target address;
`domain://N/topic?partition=p` URL-Form.

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
mit `resolve()`, statischem Alias-Set, `domain://`-URL-Parser,
Wildcard-Matching.

**Tests:** `crates/amqp-endpoint/src/routing.rs::tests` (12 Tests):
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

### §7.4.1 RELIABILITY (Settlement-Mode-Mapping)

**Spec:** §7.4.1, S. 38 — DDS `RELIABILITY` ↔ AMQP
`snd-settle-mode`/`rcv-settle-mode`-Tabelle.

**Repo:** `link.rs::SettlementMode::{Settled, Unsettled}` +
`LinkSession::deliver()` mit Pending-Settlement-Tracking.

**Tests:** `crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_unsettled_deliver_increments_pending`,
`crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_settle_decrements_pending`,
`crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_settle_with_mapper_calls_apply_and_decrements_pending`
(RELIABLE → `Unsettled` mit Pending-Tracking),
`crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_deliver_consumes_credit_and_increments_delivered`,
`crates/amqp-endpoint/src/link.rs::tests::ddsamqp_7_4_1_settle_at_zero_does_not_underflow`
(BEST_EFFORT → `Settled`, kein Pending-Eintrag). E2E:
`tools/amqp-dds-endpoint/tests/c1_5_settlement_reliable.rs::ddsamqp_7_4_1_reliable_unsettled_increments_pending_until_disposition`,
`tools/amqp-dds-endpoint/tests/c1_6_settlement_best_effort.rs::ddsamqp_7_4_1_best_effort_settled_does_not_track_pending`.

**Status:** done

### §7.4.2 DURABILITY

**Spec:** §7.4.2, S. 40 — DDS `DURABILITY` (VOLATILE/TRANSIENT/
PERSISTENT) ↔ AMQP `terminus.durable`-Tabelle. `unsettled-state`
verlangt Broker-Funktionalität → SHALL `amqp:not-implemented`.

**Repo:** `link.rs::TerminusDurability` (None/Configuration/
UnsettledState) + `check_attach_durability` liefert
`AttachDurabilityCheck::{Accept, RejectNotImplemented}`.

**Tests:** `link::tests::ddsamqp_7_4_2_terminus_durability_from_wire`,
`ddsamqp_7_4_2_attach_durability_unsettled_state_rejected_not_implemented`.

**Status:** done

### §7.4.3 HISTORY

**Spec:** §7.4.3, S. 42 — HISTORY ist DCPS-side-Cache, decoupled
von AMQP-Settlement; AMQP-Layer leitet Samples weiter wie DDS sie
emittiert.

**Repo:** kein expliziter HISTORY-Mapper nötig.

**Tests:** `crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_3_the_dds_side_policies_leave_no_trace_on_the_amqp_boundary`.

**Status:** done — strukturell durch Decoupling erfüllt.

### §7.4.4 DEADLINE

**Spec:** §7.4.4, S. 43 — DDS-side lokal; Catalog-Channel
signalisiert missed-deadline-Events.

**Repo:** Catalog-Channel via `bridge::produce_catalog_transfers`
+ `management::CatalogProducer`. Audit-Channel (`AuditProducer`)
für Deadline-Events; ein konkreter `DcpsDdsHost`-Implementer
verbindet `DataReader::on_requested_deadline_missed` mit
`AuditProducer::push(AuditEvent::Unauthorized {...})` oder
einem ergänzten `DeadlineMissed`-Event-Typ.

**Tests:** `crates/amqp-endpoint/src/management.rs::tests::ddsamqp_2_1_audit_producer_ringbuffer_evicts_oldest`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_4_4_audit_producer_drain_empties_queue`,
`tools/amqp-dds-endpoint/src/bridge.rs::tests::ddsamqp_7_4_4_produce_catalog_transfers_returns_one_per_topic`,
`tools/amqp-dds-endpoint/src/bridge.rs::tests::ddsamqp_7_4_4_produce_catalog_transfers_empty_for_empty_host`.

**Status:** done

### §7.4.5 LATENCY_BUDGET

**Spec:** §7.4.5, S. 43 — transparent zur AMQP-Boundary.

**Repo:** `crates/amqp-endpoint/src/qos_translation.rs` (`dds_qos_to_amqp_behavior` — LATENCY_BUDGET geht in kein Feld von `AmqpBehavior` ein).

**Tests:** `crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_3_the_dds_side_policies_leave_no_trace_on_the_amqp_boundary`.

**Status:** done — strukturell.

### §7.4.6 OWNERSHIP

**Spec:** §7.4.6, S. 44 — DDS-side Owner-Selection; AMQP
boundary unbeeinflusst.

**Repo:** `crates/amqp-endpoint/src/qos_translation.rs` (`dds_qos_to_amqp_behavior` — OWNERSHIP geht in kein Feld von `AmqpBehavior` ein).

**Tests:** `crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_3_the_dds_side_policies_leave_no_trace_on_the_amqp_boundary`.

**Status:** done — strukturell.

### §7.4.7 LIFESPAN

**Spec:** §7.4.7, S. 44 — DDS-LIFESPAN ↔ AMQP
`absolute-expiry-time` der Properties-Section.

**Repo:** `properties::produce_properties` setzt
`absolute_expiry_time_ms = source_timestamp + lifespan_remaining`
wenn `SampleHeader.lifespan_remaining_ms = Some(_)`.

**Tests:** `properties::tests::ddsamqp_7_4_7_produce_properties_lifespan_sets_expiry`.

**Status:** done

### §7.4.8 PARTITION

**Spec:** §7.4.8, S. 45 — DDS PARTITION ist
`sequence<string>`; AMQP-Address-URI mit wiederholten
`?partition=p1&partition=p2` Query-Params.

**Repo:** `routing.rs::parse_domain_url` akzeptiert mehrfach
wiederholte `?partition=`-Params; `AddressResolution.partitions`
ist `Vec<String>`; percent-decoding (RFC 3986) für reservierte
Zeichen in Partition-Werten.

**Tests:** `routing::tests::ddsamqp_2_1_domain_url_with_multi_partition_parses`,
`ddsamqp_2_1_domain_url_with_percent_encoded_partition`.

**Status:** done

### §7.4.9 All Other QoS Policies (transparent)

**Spec:** §7.4.9, S. 46 — TIME_BASED_FILTER, TRANSPORT_PRIORITY,
LIVELINESS, DESTINATION_ORDER, RESOURCE_LIMITS, ENTITY_FACTORY,
WRITER_DATA_LIFECYCLE, READER_DATA_LIFECYCLE, USER_DATA,
TOPIC_DATA, GROUP_DATA, PRESENTATION,
TYPE_CONSISTENCY_ENFORCEMENT — alle DDS-side; `dds:`-Prefix
reserviert.

**Repo:** `crates/amqp-endpoint/src/qos_translation.rs` (`dds_qos_to_amqp_behavior` liest von diesen Policies keine).

**Tests:** `crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_3_the_dds_side_policies_leave_no_trace_on_the_amqp_boundary`,
`crates/amqp-endpoint/src/qos_translation.rs::ddsamqp_7_4_9_default_for_topic_is_unsettled_no_durable`.

**Status:** done — strukturell durch Pass-Through-Semantik.

### §7.5 Discovery Bridging

**Spec:** §7.5, S. 47 — SPDP/SEDP ↔ AMQP `$catalog`-Address.

**Repo:** `management::CatalogProducer` mit `add`/`remove`/
`snapshot`-API, `CatalogEntry` (address, dds, type-name, type-id,
direction, partitions). `topics.exposed`-Counter wird automatisch
geführt. `classify_address("$catalog")` liefert
`AddressKind::Catalog`.

**Tests:** `management::tests::catalog_*` (5 Tests),
`ddsamqp_2_2_classify_address_recognises_reserved`.

**Status:** done

### §7.5.1 Behaviour for Unknown Addresses

**Spec:** §7.5.1, S. 48 — `permit_dynamic_topics`-Flag; bei
`false` und Unbekannt → `amqp:not-found` Link-Error;
bei `true` On-the-Fly-Topic-Erstellung.

**Repo:** `errors::map_resolution_error(err, permit_dynamic_topics)`
liefert `AmqpError` mit `condition = NotFound`,
`scope = Link`, Spec-Section §7.5.1 + Description-Text je nach
`permit_dynamic_topics`-Flag.

**Tests:** `errors::tests::ddsamqp_7_5_1_no_route_with_dynamic_disabled_yields_not_found_link`,
`ddsamqp_7_5_1_no_route_with_dynamic_enabled_still_not_found`,
`ddsamqp_7_5_1_malformed_address_maps_to_decode_error`.

**Status:** done

### §7.6 Key and Instance Mapping (group-id SHA-256)

**Spec:** §7.6.1, S. 50 — `group-id` = lowercase Hex SHA-256
über XCDR2-KeyHash-Encapsulation; opak für Subscriber.

**Repo:** `crates/amqp-endpoint/src/keyhash.rs::group_id` mit
SHA-256 + 64-char-Hex-Lowercase. `properties::produce_properties`
ruft `group_id` aus `SampleHeader.keyhash` auf.

**Tests:** `keyhash::tests` (5 Tests inkl.
`ddsamqp_7_6_empty_keyhash_yields_known_sha256`,
`ddsamqp_7_6_solidus_in_key_safe`),
`properties::tests::ddsamqp_7_6_produce_properties_keyhash_yields_group_id`.

**Status:** done

### §7.6.2 Subscriber-Side Reconstruction

**Spec:** §7.6.2, S. 51 — Hash ist one-way; Subscriber
SHALL NOT versuchen Key-Felder aus group-id zu rekonstruieren.

**Repo:** `crates/amqp-endpoint/src/keyhash.rs` (`group_id` — SHA-256-Präfix, kein Rückweg).

**Tests:** `crates/amqp-endpoint/src/keyhash.rs::ddsamqp_7_6_2_group_id_carries_no_recoverable_trace_of_the_key`,
`crates/amqp-endpoint/src/keyhash.rs::ddsamqp_7_6_2_keyhash_length_always_64_lowercase_hex`,
`crates/amqp-endpoint/src/keyhash.rs::ddsamqp_7_6_2_distinct_keyhash_yields_distinct_group_id`.

**Status:** done — strukturell (kein Reverse-Mapping zu
implementieren).

### §7.6.3 Topic without Keys

**Spec:** §7.6.3, S. 51 — Unkeyed-Topic: `group-id` weglassen.

**Repo:** `properties::produce_properties` setzt
`group_id = None` wenn `SampleHeader.keyhash = None`.

**Tests:** `properties::tests::ddsamqp_7_6_3_produce_properties_unkeyed_omits_group_id`.

**Status:** done

### §7.7.1 Outbound Instance State

**Spec:** §7.7.1, S. 52 — DDS-Sample-Operation
(`write/register/unregister/dispose`) auf
Application-Property `dds:operation` mappen; Default `write`.

**Repo:** `properties::DdsOperation` Enum +
`produce_application_properties` setzt `dds:operation` wenn
`!= Write` (Default-write wird per Spec weggelassen).

**Tests:** `properties::tests::ddsamqp_7_7_1_application_properties_register_sets_operation`,
`ddsamqp_7_7_1_application_properties_default_minimal_set` (verifiziert dass
write-Default weggelassen wird).

**Status:** done

### §7.7.2 Inbound Operation Signals

**Spec:** §7.7.2, S. 53 — Inbound `dds:operation`-Werte
auf DataWriter-Method-Calls (`register_instance`,
`unregister_instance`, `dispose`, `write`).

**Repo:** `dds_bridge::DdsOperationDispatcher`-Trait mit
`AcceptAllDispatcher` (Test-Default) und
`InstanceTrackingDispatcher` (mit Spec-§11.3-Konformer
Lifecycle-Prüfung). Daemon bindet seine echte DCPS-
DataWriter-Brücke an den Trait; das Endpoint-Crate liefert
das Plugin-Surface.

**Tests:** `dds_bridge::tests::amqpbridge_12_1_accept_all_returns_accepted_for_every_operation`,
`ddsamqp_7_7_2_instance_tracking_register_with_body_accepts`,
`ddsamqp_7_7_2_instance_tracking_register_empty_body_yields_missing_key`,
`ddsamqp_7_7_2_instance_tracking_unregister_unknown_yields_unknown_instance`,
`ddsamqp_7_7_2_instance_tracking_unregister_known_accepts`.

**Status:** done

### §7.7.3 Disposition Mapping

**Spec:** §7.7.3, S. 54 — AMQP-`disposition` ↔ DDS-Sample-State.

**Repo:** `link::LinkSession::apply_disposition` (Settlement-
Tracking) + `dds_bridge::DispositionMapper`-Trait für
DDS-API-Wiring (Caller-Layer Plugin). `NoopDispositionMapper`
als Test-Default.

**Tests:** `dds_bridge::tests::ddsamqp_7_7_3_noop_disposition_mapper_does_nothing`,
`link::tests::ddsamqp_7_4_1_settle_decrements_pending`.

**Status:** done

### §7.8 Conflict Resolution between URI and Properties

**Spec:** §7.8, S. 55 — bei Konflikt zwischen URI-Form
`?partition=` und Application-Property `dds:partition` gewinnt
URI-Form.

**Repo:** `routing::effective_partitions(uri_form,
property_form)` liefert URI-Form wenn nicht-leer, sonst
property_form.

**Tests:** `routing::tests::ddsamqp_2_1_conflict_resolution_uri_wins_when_both_present`,
`ddsamqp_2_2_conflict_resolution_property_used_when_uri_empty`,
`ddsamqp_2_2_conflict_resolution_both_empty`.

**Status:** done

### §7.9.1 Catalog Address ($catalog)

**Spec:** §7.9.1, S. 57 — Receiver-Link auf `$catalog` liefert
Topic-Mapping-Einträge; Eintrag-Felder enumeriert.

**Repo:** `management::CatalogProducer::snapshot` produziert
pro Eintrag eine `AmqpExtValue::Map` mit
`amqp-address`/`dds-topic`/`dds-type-name`/`type-id` (ulong oder
symbol je nach DescriptorForm)/`direction`/`partitions`.

**Tests:** `crates/amqp-endpoint/src/management.rs::tests::ddsamqp_2_1_catalog_snapshot_emits_map_per_entry`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_type_id_truncated_is_amqp_ulong`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_type_id_full_is_amqp_symbol`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_partitions_emitted_when_set`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_direction_symbols_match_spec`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_1_catalog_add_remove_balances_topics_exposed`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_2_2_classify_address_recognises_reserved`
(`$catalog` als reservierte Adresse).

**Status:** done

### §7.9.2 Metrics Address ($metrics)

**Spec:** §7.9.2, S. 58 — Receiver-Link auf `$metrics` liefert
Stream von AMQP-Messages mit `map`-Body
{name, value, unit, timestamp}.

**Repo:** `metrics::MetricsHub` (Counter) +
`management::metrics_snapshot(hub, now_ms)` (Sample-Producer)
liefert pro Mandatory-Metric eine `AmqpExtValue::Map` mit
`name`/`value`/`unit`/`timestamp`-Feldern.

**Tests:** `crates/amqp-endpoint/src/management.rs::tests::ddsamqp_2_1_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`crates/amqp-endpoint/src/management.rs::tests::ddsamqp_7_9_2_metrics_snapshot_carries_value`;
`crates/amqp-endpoint/src/metrics.rs::tests` (11 Tests):
`ddsamqp_7_9_2_mandatory_metric_count_is_14`, `ddsamqp_7_9_2_fresh_hub_all_zero`,
`ddsamqp_7_9_2_unknown_metric_yields_none`, `ddsamqp_7_9_2_connection_open_close_balances`,
`ddsamqp_7_9_2_transfer_unsettled_tracking`,
`ddsamqp_7_9_2_pre_settled_transfer_does_not_count_unsettled`,
`ddsamqp_7_9_2_loop_and_hop_cap_drops_count_separately`,
`ddsamqp_7_9_2_rpc_failure_counters_independent`, `ddsamqp_7_9_2_unit_of_known_metrics`,
`ddsamqp_7_9_2_topics_added_removed_balances`, `ddsamqp_7_9_2_reconnect_overflow_counter`.

**Status:** done

### §7.9.2.1 Mandatory Metrics

**Spec:** §7.9.2.1, S. 58 — 13 Mandatory-Metrics (+1
`reconnect-overflow` aus §10.8 = total 14):
`connections.active`, `connections.total`,
`transfers.received`, `transfers.sent`,
`transfers.unsettled`, `transfers.rate`,
`errors.decode`, `errors.unauthorized`, `topics.exposed`,
`transfers.dropped.loop`, `transfers.dropped.hop-cap`,
`transfers.dropped.malformed-reply`, `rpc.calls.timed-out`,
`transfers.dropped.reconnect-overflow`.

**Repo:** `metrics::MANDATORY_METRIC_NAMES` (alle 14 Konstanten),
`MetricsHub::on_*`-Counter-API, `unit_of(name)` liefert
korrekte Unit-Strings gemäß Spec-Tabelle.

**Tests:** `metrics::tests::ddsamqp_7_9_2_mandatory_metric_count_is_14`,
`ddsamqp_7_9_2_fresh_hub_all_zero` (verifiziert Coverage aller 14 Namen).

**Status:** done

### §7.9.3 Limitations + AMQP-Management Trade-off

**Spec:** §7.9.3, S. 60 — `$catalog`/`$metrics` als operational,
`$audit` als security; volles AMQP-Management-1.0 explizit
out-of-scope.

**Repo:** `crates/amqp-endpoint/src/management.rs` (`classify_address`, `AddressKind`).

**Tests:** `crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_classify_address_recognises_reserved`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_7_9_1_catalog_direction_symbols_match_spec`,
`crates/amqp-endpoint/src/management.rs::ddsamqp_2_2_audit_event_types_are_spec_symbols`.

**Status:** done — Spec-Klassifikation; keine Implementation
zu erbringen.

### §7.10 Implementation Limits (DoS-Caps)

**Spec:** §7.10, S. 61 — Compound-Nesting ≤ 16
(implementation-defined, ≥ 16 SHALL, ≥ 32 SHOULD), Max-Frame-
Size, Max-Connections, Catalog-Entries-Cap, Idle-Timeout.

**Repo:** `crates/amqp-endpoint/src/limits.rs::ResourceLimits`
mit `max_frame_size`, `max_connections`, `idle_timeout_ms`,
`max_compound_nesting`.

**Tests:** `crates/amqp-endpoint/src/limits.rs::tests::ddsamqp_7_10_defaults_match_spec_implementation_note`.

**Status:** done

### §7.11 Bridge Coexistence (Loop-Prevention)

**Spec:** §7.11, S. 63 — `dds:bridge-id`-Stamping +
Drop-on-Self-Tag + `dds:bridge-hop`-Cap (default 8, max 16);
bridge_id ist Process-Wide.

**Repo:** `coexistence::CoexistenceConfig` (process-wide
bridge_id + hop_cap; `DEFAULT_HOP_CAP=8`, `MAX_HOP_CAP=16`),
`inspect_inbound`, `stamp_outbound`. Properties-App-Keys
`dds:bridge-id`/`dds:bridge-hop` in `properties::app_keys`.

**Tests:** `crates/amqp-endpoint/src/coexistence.rs::tests` (11 Tests):
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

### §8.1.1 Pass-Through Octet-String Mode

**Spec:** §8.1.1, S. 67 — Body als single `data`-Section, content
verbatim XCDR2; `content-type = application/vnd.dds.xcdr2`.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., PassThrough)`
liefert `("application/vnd.dds.xcdr2", bytes_verbatim)`.

**Tests:** `mapping::tests::ddsamqp_8_1_1_a_pass_through_body_is_the_sample_octets`.

**Status:** done

### §8.1.2 JSON Mapping Mode

**Spec:** §8.1.2, S. 68 — Body UTF-8 JSON-Document; volle
IDL-zu-JSON-Mapping-Regeln (Time_t, octet sequences als Base64,
union, sequence, map, 16-Byte-Identifier als Hex).

**Repo:** `crates/amqp-bridge/src/codegen/encode.rs::json_document` —
die Regeln der Klausel je Zeile: Base64 für Octet-Sequenzen, der
symbolische Name für ein `enum`, `{"sec": n, "nanosec": m}` für
`Time_t`, `{"discriminator": .., "value": ..}` für eine `union` mit
weggelassenem `value` bei leerem Branch, ausgelassene abwesende
`@optional`-Member, und `NaN` und die Unendlichkeiten als Fehler statt
als Ausgabe. Die Typinformation, die die Klausel verlangt, liefert
`crates/amqp-bridge/src/codegen/plan.rs::plan_of` aus dem Modell.
Die Helfer je IDL-Typ erzeugt
`crates/amqp-bridge/src/codegen/emit.rs::helpers_for`
(`toJsonString_<Typname>`) für C++, Java und TypeScript gegen die
Laufzeiten in `crates/amqp-bridge/runtime/`.
`crates/amqp-endpoint/src/mapping.rs::encode_dds_to_amqp_body(.., Json)`
bleibt der typlose Weg und schreibt weiter das Hex-Feld.

**Tests:** `crates/amqp-bridge/src/codegen/encode.rs::tests`
(`ddsamqp_8_1_2_an_octet_sequence_is_base64_in_json_and_binary_on_the_wire`,
`ddsamqp_8_1_2_an_enumeration_is_its_symbolic_name_in_both_modes`,
`ddsamqp_8_1_2_a_nan_and_an_infinity_are_refused_rather_than_written`,
`ddsamqp_8_1_2_a_time_is_milliseconds_on_the_wire_and_two_keys_in_json`,
`ddsamqp_8_1_2_the_json_escapes_are_rfc_8259s`,
`ddsamqp_8_1_2_base64_matches_rfc_4648_test_vectors`);
`crates/amqp-bridge/tests/cross_language_agreement.rs` compiliert und
startet den erzeugten C++-, Java- und TypeScript-Code und vergleicht
dessen JSON zeichengleich mit dem der Rust-Referenz.

**Status:** done — die IDL-zu-JSON-Regeln stehen typgetrieben und in
vier Spalten, die miteinander verglichen werden.

### §8.1.3 AMQP-Native Typed Mapping Mode

**Spec:** §8.1.3, S. 71 — Body als single `amqp-value`-Section
mit described composite per §7.2.

**Repo:** `crates/amqp-bridge/src/codegen/encode.rs::described_composite`
— der Descriptor nach §7.2.1 in beiden Formen, der Body nach §7.2.2 als
Liste, als Map nach Member-ID für einen `@mutable`-Typ und als
Zwei-Element-Liste für eine `union` nach §7.2.3, ein `null` am Platz
eines abwesenden `@optional`-Members und die verschachtelte Komposite
mit eigenem Descriptor nach §7.2.2.4.
`crates/amqp-bridge/src/codegen/plan.rs` liest die Zuordnung je Zeile
von §7.1 aus dem Modell. Die Helfer je IDL-Typ erzeugt
`crates/amqp-bridge/src/codegen/emit.rs::helpers_for`
(`toAmqpValue_<Typname>`) für C++, Java und TypeScript gegen die
Laufzeiten in `crates/amqp-bridge/runtime/`.
`crates/amqp-endpoint/src/mapping.rs::encode_dds_to_amqp_body(..,
AmqpNative)` setzt den content-type.

**Tests:** `crates/amqp-bridge/tests/type_mapping.rs` (10 Tests):
`ddsamqp_2_2_every_row_of_the_primitive_table_takes_the_code_it_names`,
`ddsamqp_2_2_an_octet_sequence_is_binary_and_a_long_sequence_is_a_list`,
`ddsamqp_2_2_the_body_form_follows_the_types_extensibility`,
`ddsamqp_2_2_a_mutable_body_is_keyed_by_the_declared_member_ids`,
`ddsamqp_2_2_a_union_plans_its_discriminator_and_then_its_branches`,
`ddsamqp_2_2_a_type_outside_the_mapping_is_named_rather_than_encoded_as_something_else`,
`ddsamqp_8_1_3_a_plan_of_something_that_is_not_a_composite_is_refused`,
`amqp10_1_1_the_two_descriptor_forms_come_from_the_same_identifier`,
`ddsamqp_8_1_3_a_sample_nested_past_the_bound_is_refused_by_name`,
`amqp10_1_1_a_nested_composite_carries_its_own_descriptor` — aus dem
gesenkten Modell: die Primitiv-Tabelle Zeile für Zeile, die Body-Form je
Extensibility, die Member-ID-Map, die Union, die beiden
Descriptor-Formen, die verschachtelte Komposite und die Ablehnung eines
nicht abgebildeten Typs); `crates/amqp-bridge/tests/cross_language_agreement.rs`
compiliert und startet den erzeugten C++-, Java- und TypeScript-Code und
vergleicht dessen Oktette byte-gleich mit denen der Rust-Referenz.

**Status:** done — die described composite steht typgetrieben und in
vier Spalten, die auf dieselben Oktette kommen.

### §8.2 Properties-Section message-id

**Spec:** §8.2, S. 72 — `message-id` = binary(24) = ⟨writer-GUID
(16B) || RTPS-seqnum (8B BE)⟩, eindeutig pro Sample. InstanceHandle
NICHT in message-id.

**Repo:** `properties::message_id(writer_guid, seqnum)` liefert
24-Byte-Vec; InstanceHandle ist explizit auf
`dds:instance-handle` App-Property gemappt (nicht auf
`message-id`).

**Tests:** `properties::tests::ddsamqp_8_2_message_id_is_24_bytes_guid_then_seqnum`,
`ddsamqp_8_2_message_id_distinguishes_consecutive_samples_same_instance`.

**Status:** done

### §8.2 Properties-Section Other Fields

**Spec:** §8.2, S. 72 — Tabelle: user-id, to, subject, reply-to,
correlation-id, content-type, content-encoding,
absolute-expiry-time, creation-time, group-id, group-sequence,
reply-to-group-id.

**Repo:** `properties::ProducedProperties` liefert die
spec-mandatorischen Felder (`message_id`, `creation_time_ms`,
`absolute_expiry_time_ms`, `group_id`); applikations-spezifische
Felder (`subject`, `reply-to`, `correlation-id`, `user-id`)
bleiben Caller-Belegung.

**Tests:** `crates/amqp-endpoint/src/properties.rs::tests::ddsamqp_7_6_produce_properties_keyhash_yields_group_id`,
`crates/amqp-endpoint/src/properties.rs::tests::ddsamqp_7_6_3_produce_properties_unkeyed_omits_group_id`,
`crates/amqp-endpoint/src/properties.rs::tests::ddsamqp_7_4_7_produce_properties_lifespan_sets_expiry`.

**Status:** done — spec-mandatorische Felder belegt.

### §8.3 Application-Properties Mapping

**Spec:** §8.3, S. 74 — Standard-Keys: `dds:nsec`,
`dds:partition`, `dds:domain-id`, `dds:type-id`,
`dds:source-guid`, `dds:lifespan-ms`, `dds:sample-state`,
`dds:view-state`, `dds:instance-state`, `dds:operation`,
`dds:bridge-id`, `dds:bridge-hop`, `dds:instance-handle`.

**Repo:** `properties::app_keys`-Modul mit allen 13 String-
Konstanten; `produce_application_properties` baut die
`AmqpExtValue::Map` mit den ableitbaren Standard-Keys
(`dds:nsec`, `dds:partition` als single-string oder list,
`dds:domain-id`, `dds:type-id` (bei DESC_TRUNCATED),
`dds:lifespan-ms`, `dds:operation` (wenn != Write),
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

### §9.1 IDL-defined Mapping Schema (Annex A)

**Spec:** §9.1, S. 79 — Konfiguration über IDL-Schema:
`TopicMapping`, `AmqpEndpointConfig`, `AmqpBridgeConfig`,
`TlsConfig`, `SaslConfig`, `ResourceLimits`,
`DynamicTopicConfig`. SemVer-Felder.

**Repo:** `crates/amqp-endpoint/src/annex_a.rs` spiegelt das
Annex-A-IDL 1-zu-1 nach Rust: alle 5 Enums (SaslMechanism,
BodyEncodingMode, TimeMapping, DescriptorForm, LinkDirection)
und alle 7 Structs (TopicMapping, TlsConfig, SaslConfig,
ResourceLimits, DynamicTopicConfig, AmqpEndpointConfig,
AmqpBridgeConfig) mit Spec-konformen Defaults. IDL-Symbol-
Round-Trip (`as_idl`/`parse`) abgebildet.

**Tests:** `crates/amqp-endpoint/src/annex_a.rs::tests` (10 Tests):
`ddsamqp_9_1_the_sasl_mechanisms_are_the_enumerators_of_annex_a`, `ddsamqp_9_1_sasl_mechanism_accepts_amqp_wire_aliases`,
`ddsamqp_9_1_the_body_encoding_modes_are_the_enumerators_of_annex_a`, `ddsamqp_9_1_descriptor_form_default_is_truncated`,
`ddsamqp_9_1_time_mapping_default_is_standard`, `ddsamqp_9_1_link_direction_default_is_both`,
`ddsamqp_9_1_topic_mapping_defaults_match_spec`, `ddsamqp_9_1_endpoint_config_default_is_empty`,
`ddsamqp_9_1_resource_limits_default_has_dos_caps`, `ddsamqp_9_1_link_direction_amqp_aliases`.

**Status:** done — das Datenmodell ist spec-konform und in der Crate
verfügbar; der Symbol-Round-Trip gegen die IDL-Schreibweise ist je Enum
und je Struct getestet.

### §9.2 XML Configuration

**Spec:** §9.2, S. 81 — XML-Form als alternative Konfigurations-
Eingabe; XSD-Snippet referenziert.

**Repo:** `crates/amqp-endpoint/src/config_xml.rs` mit
`parse_config(xml)` → `DdsAmqpConfig`-Resultat
(roxmltree-basiert; std-only Feature). Akzeptiert das
`<dds-amqp>`-Root-Element mit Namespace
`http://www.zerodds.org/dds-amqp/v1.0`. Parser für endpoint,
bridge, tls, sasl, topics, dynamic, limits.

**Tests:** `config_xml::tests` (11 Tests inkl.
`ddsamqp_9_2_parse_minimal_endpoint`,
`ddsamqp_9_2_parse_endpoint_with_topics_tls_sasl_limits`,
`ddsamqp_9_2_parse_bridge_with_upstream`, `ddsamqp_9_2_unknown_root_yields_error`,
`ddsamqp_9_2_missing_amqp_address_yields_error`,
`ddsamqp_9_2_invalid_body_mode_yields_error`,
`ddsamqp_9_2_malformed_xml_yields_parse_error`,
`ddsamqp_9_2_unknown_elements_and_attributes_are_refused_with_their_line`);
`tools/amqp-dds-endpoint/tests/readme_config.rs::ddsamqp_9_2_the_readme_configuration_reads_as_it_says`.
Unbekannte Elemente und Attribute werden mit Zeile abgelehnt; der Namespace
trägt die Version.

**Status:** done

---

## §10 Security

### §10.1 TLS Bracket

**Spec:** §10.1, S. 83 — TLS 1.2 mindestens, 1.3 RECOMMENDED;
Cert-Validation; AMQP-Negotiation nach TLS-Handshake.

**Repo:** `tools/amqp-dds-endpoint/src/tls.rs` (Cargo-Feature
`tls`, opt-in): `ServerTlsConfig` + `ClientTlsConfig` aus PEM,
`build_server_config` mit optionalem mTLS-Client-Cert-Verifier
(`require_client_cert = true`), `build_client_config` mit Trust-
Anchors + optionalem Client-Auth-Cert. `accept_server` /
`connect_client` führen echten TLS-Handshake (rustls 0.23,
TLS 1.2 + 1.3, ring-Crypto-Provider). `StreamOwned`-Resultat
ist `Read+Write` und wird direkt an `handle_connection`
gereicht.

**Tests:** `tls::tests` (6 Tests inkl.
`ddsamqp_10_1_build_server_config_from_self_signed`,
`ddsamqp_10_1_build_server_config_require_client_cert_needs_ca`,
`ddsamqp_10_1_build_client_config_from_root_ca`,
`ddsamqp_10_1_tls_handshake_round_trip_with_self_signed_cert` mit echtem
TLS-Handshake gegen rcgen-generiertes self-signed Cert).

`tools/amqp-dds-endpoint/tests/tls_sasl_e2e.rs::ddsamqp_10_1_a_tls_client_is_offered_plain_and_authenticated_against_the_store`
(über den Server des Endpoint-Programms: TLS-Handshake, PLAIN angeboten, gegen den Store
geprüft).

**Status:** done — rustls, Feature `tls` (Default) im Endpoint-Programm.

### §10.2 SASL Mechanisms — Mandatory Set

**Spec:** §10.2, S. 85 — implementations MUST support PLAIN,
ANONYMOUS, EXTERNAL.

**Repo:** `sasl.rs::SaslMechanism::{Plain, Anonymous, External}`.

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_prefers_external`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_falls_back_to_plain_with_tls`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_skips_plain_without_tls`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_10_2_select_outbound_anonymous_only`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_no_acceptable_mechanism`,
`crates/amqp-endpoint/src/sasl.rs::tests::amqp10_5_3_3_6_the_sasl_codes_are_0_to_4`.

**Status:** done

### §10.2 SASL Mechanisms — Optional SCRAM-SHA-256

**Spec:** §10.2, S. 85 — implementations MAY additionally support
SCRAM-SHA-256.

**Repo:** `crates/amqp-endpoint/src/sasl.rs::SaslMechanism::ScramSha256`
mit Wire-Symbol `"SCRAM-SHA-256"` + Round-Trip-Parse-Support +
`is_mandatory()`-Predicate. Konkrete RFC-7677-Implementation
(HMAC-SHA-256 + PBKDF2 + Channel-Binding) ist Caller-Layer
(Reuse `crates/security-crypto`).

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::amqp10_5_3_3_1_the_mechanism_names_are_plain_anonymous_external`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_10_2_unknown_name_yields_none`,
`crates/amqp-endpoint/src/security.rs::tests::ddsamqp_10_2_scram_yields_scram_sha256_class_id`.

**Status:** done — Optional SCRAM-SHA-256-Mechanism als Wire-
Marker-Enum-Variant ausgewiesen.

### §10.2.1 Mandatory TLS for SASL PLAIN

**Spec:** §10.2.1, S. 87 — siehe Endpoint Cl. 7 / Bridge Cl. 5.

**Repo:** Inbound-Filter `SaslState::new(tls_active)` +
Outbound-Filter `SaslState::select_outbound`. Daemon-Outbound
in `client::do_outbound_sasl` mit
`ClientError::PlainRejectedNoTls`-Reject-Pfad.

**Tests:** `sasl::tests::ddsamqp_2_1_plain_offered_only_when_tls_active`,
`ddsamqp_2_1_select_outbound_skips_plain_without_tls`.

**Status:** done

### §10.3 Cross-Reference to DDS-Security — Outbound Signing

**Spec:** §10.3.1, S. 88 — DDS-Security-Plugins können aktiv
sein; Outbound-Signing ist DDS-side.

**Repo:** `crates/amqp-endpoint/src/security.rs` (`DualIdentity`).

**Tests:** `crates/amqp-endpoint/src/security.rs::ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`,
`crates/amqp-endpoint/src/security.rs::ddsamqp_10_3_external_yields_pki_dh_class_id_with_cert`,
`crates/amqp-endpoint/src/security.rs::ddsamqp_10_3_class_id_strings_match_spec_table`.

**Status:** done — strukturell (DDS-Security-Plugins parallel
zum AMQP-Layer aktiv).

### §10.3.2 Identity Mapping (vendor class_ids)

**Spec:** §10.3.2, S. 89 — IdentityToken-class_ids:
PLAIN→`zerodds:Auth:SASL-Username:1.0`,
ANONYMOUS→`zerodds:Auth:Anonymous:1.0`,
EXTERNAL→`DDS:Auth:PKI-DH:1.0` (mit X.509),
SCRAM-SHA-256→`zerodds:Auth:SASL-SCRAM-SHA256:1.0`.
`subject_name` Konvention `CN=<authcid>`.

**Repo:** `security::class_ids`-Modul mit allen 4 Konstanten;
`build_identity_token(SaslSubject)` erzeugt Spec-konforme
Tokens (PLAIN/ANONYMOUS/EXTERNAL/SCRAM).

**Tests:** `security::tests::ddsamqp_10_3_2_plain_yields_sasl_username_class_id`,
`ddsamqp_10_3_2_anonymous_yields_anonymous_class_id`,
`ddsamqp_10_3_external_yields_pki_dh_class_id_with_cert`,
`ddsamqp_10_2_scram_yields_scram_sha256_class_id`,
`ddsamqp_10_3_class_id_strings_match_spec_table`.

**Status:** done

### §10.3.3 Permission Evaluation

**Spec:** §10.3.3, S. 90 — IdentityToken an
DDS-Security AccessControl-Plugin
`check_create_datawriter`/`check_create_datareader` übergeben;
NOT_ALLOWED → AMQP `amqp:unauthorized-access` Link-Error.

**Repo:** `security::AccessControlPlugin`-Trait mit
`check(identity, address, op) -> AccessDecision`. Default-Plugins
`AllowAll` (Test) + `StaticAllowList`. `errors::access_denied`
liefert das `amqp:unauthorized-access`-Mapping.

**Tests:** `security::tests::ddsamqp_10_3_3_static_allow_list_per_op`,
`errors::tests::ddsamqp_10_3_3_access_denied_yields_unauthorized_access_link`.

**Status:** done

### §10.3.4 Determinism Across Vendors

**Spec:** §10.3.4, S. 91 — IdentityToken-Konstruktion
deterministisch über Implementations.

**Repo:** `build_identity_token` ist pure Funktion ohne
Random/State; gleiche `SaslSubject` → gleicher
`IdentityToken`. `LinkGovernance::evaluate` cached pro Op und
liefert deterministisch dieselbe Decision für gleiche Eingabe.

**Tests:** `security::tests::ddsamqp_2_1_link_governance_caches_decision`
(verifiziert Cache-Hit bei wiederholtem Op-Call).

**Status:** done

### §10.3.5 No-Bypass Guarantee

**Spec:** §10.3.5, S. 92 — DDS-Sample, das AccessControl ablehnt,
darf nicht über AMQP austreten; AccessControl-Resultat ist
Pre-Condition jedes Transfer.

**Repo:** `handler::check_access(cfg, addr, op)` im Daemon-Loop
pre-Attach + pre-Transfer aufgerufen. Bei Deny:
`metrics.on_unauthorized()` + Detach (bei Attach) bzw. Drop
(bei Transfer).

**Tests:** `handler::tests::ddsamqp_2_1_access_control_deny_attach_yields_unauthorized_metric`,
`ddsamqp_2_1_access_control_allow_does_not_increment_unauthorized`.

**Status:** done

### §10.4 Governance Document Mapping

**Spec:** §10.4, S. 95 — DDS-Security Governance-Document-Domain-
Rules ↔ AMQP-Address-Allow/Deny pro Identity.

**Repo:** `security::GovernanceDocument` mit `add_rule`/
`resolve(topic)`-API. `GovernanceRule` mit `topic_pattern` (exact,
prefix*, *suffix, *), `enable_discovery`, `enable_liveliness`,
`data_protection_kind` (None/SignOnly/SignAndEncrypt).
`config_xml::parse_governance(xml)` lieast XML
`<governance>`-Root mit `<rule>`-Kindern in
`GovernanceDocument`.

**Tests:** `security::tests::governance_resolves_*` (4 Tests),
`config_xml::tests::ddsamqp_10_4_governance_document_loads_rules`,
`ddsamqp_10_4_governance_missing_topic_pattern_errors`,
`ddsamqp_10_4_governance_invalid_data_protection_errors`.

**Status:** done

### §10.5 No Implicit Trust Between Profiles

**Spec:** §10.5, S. 97 — Endpoint und Bridge auf demselben Host
teilen keine Identity automatisch.

**Repo:** `crates/amqp-endpoint/src/security.rs` (`DualIdentity`, getrennte `IdentityToken`- und Broker-Credential-Felder).

**Tests:** `crates/amqp-endpoint/src/security.rs::ddsamqp_2_2_dual_identity_keeps_broker_and_dds_separate`,
`crates/amqp-endpoint/src/security.rs::ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`,
`crates/amqp-endpoint/src/bridge_security.rs::amqpbridge_12_1_amqp_acl_address_check`,
`crates/amqp-endpoint/src/bridge_security.rs::ddsamqp_10_5_amqp_sasl_plain_rejects_unknown_user`.

**Status:** done — strukturell (separate Konfigurations-Strukturen).

### §10.6 Bridge-Profile Dual Identity

**Spec:** §10.6, S. 98 — Bridge führt zwei getrennte Identitäten:
Broker-side SASL-Credential, DDS-side IdentityToken; nicht
miteinander verschmelzen.

**Repo:** `security::DualIdentity::new(broker_id, dds_id)` mit
`for_broker()`/`for_dds()`-Getter. AccessControl-Plugin SHALL nur
`for_dds()` benutzen.

**Tests:** `security::tests::ddsamqp_2_2_dual_identity_keeps_broker_and_dds_separate`,
`ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### §10.7 Per-Link Governance Resolution

**Spec:** §10.7, S. 100 — Pro Link wird Governance-Permission
neu evaluiert; Mixed-Sensitivity-Topics auf einer Connection
sind erlaubt.

**Repo:** `security::LinkGovernance::new(identity, address, rule)`
+ `evaluate(plugin, op)` mit Per-Op-Cache. Mixed-Sensitivity ist
strukturell unterstützt: jede `LinkGovernance`-Instanz hält
eigene Identität/Rule, mehrere Instanzen pro Connection sind
erlaubt.

**Tests:** `security::tests::ddsamqp_2_1_link_governance_caches_decision`.

**Status:** done

### §10.8 Reconnect Behaviour

**Spec:** §10.8, S. 102 — Endpoint: failed-Connection-unsettled
released; Bridge: Reconnect mit exp. Backoff (init 1s, mult 2,
cap 60s); KEEP_LAST-Eviction zählt
`transfers.dropped.reconnect-overflow`.

**Repo:** `client::ReconnectConfig` + `connect_with_reconnect`
mit Default-Pacing (1s init, 2x mult, 60s cap) gemäß Spec.
`MetricsHub::on_reconnect_overflow` für KEEP_LAST-Eviction-
Counter.

**Tests:** `tools/amqp-dds-endpoint/src/client.rs::tests::ddsamqp_2_2_backoff_starts_at_initial`,
`tools/amqp-dds-endpoint/src/client.rs::tests::ddsamqp_2_2_backoff_doubles_until_cap`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_loop_pacing_follows_spec_defaults`
(1 s init, Faktor 2, 60 s Cap),
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_the_client_reconnects_once_the_server_is_up`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_aborts_on_max_attempts`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_after_server_restart_eventually_succeeds`.

**Status:** done

---

## §11 Error Conditions

### §11.1 Encode/Decode Failures

**Spec:** §11.1, S. 105 — Tabelle Errors → AMQP-Error-Codes:
type-mismatch / no-mapping / malformed-XCDR2 / nesting-depth /
UTF-8-violation / char-out-of-range / hash-truncation-collision /
frame-size-exceeded → `amqp:decode-error` (Disposition rejected
oder Connection-Close).

**Repo:** `errors::AmqpError` + `AmqpErrorCondition` +
`ErrorScope` (Transfer/Link/Connection). `map_mapping_error`
liefert das `amqp:decode-error` aus `MappingError`-Eingabe.

**Tests:** `errors::tests::ddsamqp_11_1_condition_symbols_match_spec`,
`ddsamqp_11_1_invalid_utf8_maps_to_decode_error_transfer`,
`ddsamqp_11_1_invalid_json_maps_to_decode_error`,
`ddsamqp_11_1_empty_body_maps_to_decode_error`.

**Status:** done

### §11.2 Connection / Session / Link Lifecycle Failures

**Spec:** §11.2, S. 107 — Idle-Timeout / max_connections-Cap /
Catalog-Cap / dynamic-topic-disabled / unsupported-durability /
unsupported-operation / DDS-Security-Reject → spezifische AMQP-
Error-Codes.

**Repo:** `errors::resource_limit_exceeded`,
`errors::unsettled_state_not_implemented`,
`errors::unknown_dds_operation`, `errors::access_denied`
liefern Spec-konforme Error-Mappings mit korrektem
`ErrorScope` (Connection/Link/Transfer).

**Tests:** `errors::tests::ddsamqp_11_2_resource_limit_exceeded_is_connection_scope`,
`ddsamqp_11_2_unsettled_state_yields_not_implemented`,
`ddsamqp_11_2_unknown_dds_operation_yields_not_implemented`.

**Status:** done

### §11.3 Instance-Lifecycle Failures

**Spec:** §11.3, S. 109 — `dds:operation = unregister` auf
unbekannter Instanz / `dispose` auf unbekannter Instanz /
`register` ohne Key → `amqp:precondition-failed`/
`amqp:decode-error`.

**Repo:** `errors::instance_unknown` + `register_missing_key` +
`unknown_dds_operation` Helpers; `dds_bridge::DispatchOutcome`
liefert die Spec-konformen Outcomes; `to_amqp_error(key_hex)`
mappt sie auf `AmqpError`. `InstanceTrackingDispatcher` setzt
das Verhalten um.

**Tests:** `errors::tests::ddsamqp_11_3_instance_unknown_yields_precondition_failed`,
`ddsamqp_11_3_register_missing_key_yields_decode_error`,
`dds_bridge::tests::ddsamqp_11_3_dispatch_outcome_to_amqp_error_maps_correctly`,
`instance_tracking_*` (5 Tests).

**Status:** done

### §11.4 Diagnostic Description Strings

**Spec:** §11.4, S. 110 — Description-String-Format:
`<spec-section>: <human-readable>`.

**Repo:** `errors::ErrorDescription::render` liefert exakt das
Format `<§-Ref>: <Text>`. Alle `errors::*`-Helper konstruieren
Spec-konforme Descriptions mit `§-Ref`-Präfix.

**Tests:** `errors::tests::ddsamqp_11_4_description_renders_spec_section_then_text`,
`ddsamqp_11_4_description_display_matches_render`.

**Status:** done

---

## Annex A — IDL Configuration Schema (normative)

### Annex A IDL Module `zerodds::amqp`

**Spec:** Annex A, S. 113 — vollständiger IDL-Code: enums
`BodyEncodingMode`, `TimeMapping`, `DescriptorForm`,
`SaslMechanism`, `LinkDirection`; structs `TopicMapping`,
`TlsConfig`, `SaslConfig`, `ResourceLimits`,
`DynamicTopicConfig`, `AmqpEndpointConfig`, `AmqpBridgeConfig`.
`bridge_id`/`bridge_hop_cap` auf Endpoint/Bridge-Config
(process-wide).

**Repo:** `crates/amqp-endpoint/src/annex_a.rs` mit allen
5 Enums + 7 Structs in 1-zu-1-Spiegelung des IDL.
`bridge_id`/`bridge_hop_cap` an Endpoint+Bridge-Konfig.
IDL-Symbol-Round-Trip via `as_idl`/`parse`.

**Tests:** `crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_the_sasl_mechanisms_are_the_enumerators_of_annex_a`,
`crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_the_body_encoding_modes_are_the_enumerators_of_annex_a`,
`crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_topic_mapping_defaults_match_spec`,
`crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_endpoint_config_default_is_empty`,
`crates/amqp-endpoint/src/annex_a.rs::tests::ddsamqp_9_1_resource_limits_default_has_dos_caps`.

**Status:** done — siehe §9.1.

---

## Annex B — Examples (informative)

### Annex B Examples

**Spec:** Annex B, S. 119 — drei Beispiele: Edge-Sensor,
Cloud-Subscriber via Broker, Multi-Vendor-Federation.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Beispiel-Deployments ohne normative
Anforderung; die Mechanismen, die sie zusammensetzen, sind in §7.3
(Address-Resolution), §7.11 (Bridge-Coexistence) und §10.1 (TLS) als
eigene Items geführt.

---

## Annex C — Compliance Test Suite (normative)

### Annex C C.1.1 Connection Open (TLS+PLAIN)

**Spec:** Annex C §C.1.1, S. 124 — AMQP-1.0-Client connectet via
TLS, authentifiziert mit PLAIN, etabliert Session.

**Repo:** Integration-Test `c1_1_connection_open.rs` fährt
echten Open-Roundtrip gegen lokal-laufenden Daemon mit
`tls_active = true`. TLS-Termination selbst ist §10.1
Folge-Welle.

**Tests:** `ddsamqp_2_1_connection_open_with_tls_active_advertises_plain`.

**Status:** done — Open-Roundtrip + PLAIN-Mechanism-Negotiation
verifiziert; TLS-Wire-Encryption ist §10.1.

### Annex C C.1.2 SASL PLAIN Rejection on Plain Transport

**Spec:** Annex C §C.1.2, S. 124 — Client connectet ohne TLS,
SASL-init mit PLAIN → SASL-Outcome `auth`-fail + Connection-Close.

**Repo:** Logik in `sasl::SaslState::new(false)` +
`select_outbound`. E2E Integration-Test
`tools/amqp-dds-endpoint/tests/c1_2_sasl_plain_rejection.rs`
mit echter TCP-Verbindung gegen lokal-laufenden Daemon.

**Tests:** `crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_plain_without_tls_yields_auth_failed`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_select_outbound_skips_plain_without_tls`,
`crates/amqp-endpoint/src/sasl.rs::tests::ddsamqp_2_1_plain_offered_only_when_tls_active`,
`c1_2_no_tls_server_does_not_advertise_plain`,
`c1_2_client_with_only_plain_credentials_fails_without_tls`,
`c1_2_client_error_plainrejectednotls_strs_correctly`.

**Status:** done

### Annex C C.1.3 Sender-Link Producer-to-DDS

**Spec:** Annex C §C.1.3, S. 125 — Sender-Link auf Topic-Address;
Transfer wird in DDS publiziert; DDS-Subscriber empfängt.

**Repo:** `bridge::dispatch_attach` resolved Address gegen
`DdsHost`; `bridge::dispatch_transfer(host, topic_id, body, metrics)`
ruft `host.publish_to_dds`. Per `InMemoryDdsHost`
verifizierter Roundtrip.

**Tests:** `c1_3_4_8_bridge_dispatch.rs::ddsamqp_2_1_amqp_producer_publishes_to_dds_via_bridge`,
`ddsamqp_c_1_3_unknown_address_attaches_with_unknown_address_outcome`.

**Status:** done

### Annex C C.1.4 Receiver-Link DDS-to-Consumer

**Spec:** Annex C §C.1.4, S. 125 — analog zu C.1.3 in Gegenrichtung.

**Repo:** `bridge::subscribe_outbound(host, topic_id, callback)`
registriert eine Outbound-Subscription; sobald die DDS-Side
publishtet (`host.publish_to_dds`), wird der Callback mit den
Bytes invoked, der die AMQP-Outbound-Transfer-Logik triggert.

**Tests:** `ddsamqp_2_1_dds_publish_flows_to_amqp_consumer_callback`.

**Status:** done

### Annex C C.1.5 Settlement-Mode Reliable

**Spec:** Annex C §C.1.5, S. 125 — DDS-DataWriter mit
RELIABILITY=RELIABLE; AMQP-Settlement-Roundtrip.

**Repo:** `link.rs::LinkSession::deliver`/`settle` Pending-
Tracking. Integration-Test
`c1_5_settlement_reliable.rs` verifiziert: pending wird
erhöht bis Disposition empfangen wird, Credit-Erschöpfung
liefert NoCredit-Error.

**Tests:** `ddsamqp_7_4_1_reliable_unsettled_increments_pending_until_disposition`,
`ddsamqp_c_1_5_credit_exhaustion_blocks_further_deliveries`.

**Status:** done

### Annex C C.1.6 Settlement-Mode Best-Effort

**Spec:** Annex C §C.1.6, S. 125 — RELIABILITY=BEST_EFFORT;
pre-settled Delivery.

**Repo:** `link.rs::LinkSession` mit `SettlementMode::Settled`.
Integration-Test `c1_6_settlement_best_effort.rs`.

**Tests:** `ddsamqp_7_4_1_best_effort_settled_does_not_track_pending`,
`ddsamqp_c_1_6_settle_call_on_pre_settled_link_is_no_op`.

**Status:** done

### Annex C C.1.7 Address-Resolution Wildcard

**Spec:** Annex C §C.1.7, S. 126 — Wildcard-Address attached →
multi-Partition-Stream.

**Repo:** `routing::AddressRouter::add_route` + Pattern-Matcher
mit prefix-`*`/suffix-`*`/global-`*`. Integration-Test
`c1_7_address_resolution_wildcard.rs`.

**Tests:** `ddsamqp_c_1_7_prefix_wildcard_matches_multiple_topics`,
`ddsamqp_c_1_7_suffix_wildcard_matches`,
`ddsamqp_c_1_7_global_wildcard_matches_anything`,
`ddsamqp_c_1_7_static_alias_takes_precedence_over_wildcard`.

**Status:** done

### Annex C C.1.8 Catalog-Address

**Spec:** Annex C §C.1.8, S. 126 — Receiver-Link auf `$catalog`
liefert Topic-Mapping-Entries.

**Repo:** `bridge::dispatch_attach` erkennt `$catalog`-Address
und liefert `AttachOutcome::AttachedCatalog`;
`bridge::produce_catalog_transfers(host)` liefert pro
registriertem Topic-Mapping eine AMQP-Map als Sample-Body;
`encode_catalog_sample` wickelt es in described-composite
für Wire-Transport.

**Tests:** `bridge::tests::ddsamqp_7_4_4_produce_catalog_transfers_returns_one_per_topic`,
`ddsamqp_c_1_8_encode_catalog_sample_produces_described_composite`,
`c1_3_4_8_bridge_dispatch.rs::ddsamqp_c_1_8_catalog_receiver_gets_one_sample_per_topic`.

**Status:** done

### Annex C C.1.9 Idle-Timeout

**Spec:** Annex C §C.1.9, S. 126 — Connection ohne Traffic →
Connection-Close mit Idle-Timeout-Error.

**Repo:** `ResourceLimits::idle_timeout_ms` +
`TcpStream::set_read_timeout` im Daemon. Integration-Test
`c1_9_idle_timeout.rs` verifiziert echten Read-Timeout-
Disconnect (Server schließt idle-Klient).

**Tests:** `ddsamqp_c_1_9_server_disconnects_idle_client_after_short_read_timeout`,
`ddsamqp_c_1_9_idle_timeout_is_configurable`.

**Status:** done

### Annex C C.1.10 Concurrent Connections

**Spec:** Annex C §C.1.10, S. 127 — Endpoint akzeptiert
mindestens `max_connections` parallele Connections.

**Repo:** `server::run_server` + `common::TestServer` spawnen
thread-per-connection. Integration-Test
`c1_10_concurrent_connections.rs` startet 8 parallele
Klient-Threads, verifiziert dass alle 8 erfolgreich
authentifiziert + connectet sind und `connections.total`-
Counter im Server alle zählt.

**Tests:** `ddsamqp_c_1_10_server_accepts_multiple_concurrent_connections`.

**Status:** done

### Annex C C.1.11 Pass-Through Encoding

**Spec:** Annex C §C.1.11, S. 127 — Sample mit MODE_PASSTHROUGH
roundtrips byte-identisch.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., PassThrough)`
plus Bridge-Dispatch in
`tools/amqp-dds-endpoint/src/bridge.rs::dispatch_transfer`.

**Tests:** `mapping::tests::ddsamqp_8_1_1_a_pass_through_body_is_the_sample_octets`,
`c1_3_4_8_bridge_dispatch::ddsamqp_2_1_amqp_producer_publishes_to_dds_via_bridge`,
`ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`,
`ddsamqp_2_1_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### Annex C C.1.12 JSON Encoding

**Spec:** Annex C §C.1.12, S. 127 — Sample mit MODE_JSON ist
gültiges RFC-8259-JSON.

**Repo:** `crates/amqp-bridge/src/codegen/encode.rs::json_document`
schreibt das Dokument; die Escapes von RFC 8259 §7 stehen in
`json_string` derselben Datei, und die Zahl wird als die kürzeste
Darstellung geschrieben, die zurückliest. Die erzeugten
`toJsonString_<Typname>`-Helfer aus
`crates/amqp-bridge/src/codegen/emit.rs` schreiben dasselbe Dokument in
C++, Java und TypeScript (siehe §8.1.2).

**Tests:** `crates/amqp-bridge/tests/cross_language_agreement.rs::ddsamqp_c_1_12_the_reference_json_is_a_valid_rfc_8259_document`
prüft Klammer-Balance außerhalb von Strings, kein nachlaufendes Komma
und die Anführungszeichen um jeden Schlüssel;
`ddsamqp_c_1_12_the_emitted_cpp_writes_the_same_octets_and_the_same_json` und die
beiden Geschwister-Tests prüfen, dass die drei erzeugten Spalten
zeichengleich dasselbe Dokument schreiben;
`crates/amqp-bridge/src/codegen/encode.rs::tests::ddsamqp_8_1_2_the_json_escapes_are_rfc_8259s`.

**Status:** done — ein Sample in MODE_JSON ist gültiges RFC-8259-JSON,
in allen vier Spalten geprüft.

### Annex C C.1.13 Metrics Link

**Spec:** Annex C §C.1.13, S. 128 — Receiver auf `$metrics`;
alle Mandatory-Metrics werden innerhalb 30s emittiert.

**Repo:** `MetricsHub` + `management::metrics_snapshot(hub,
now_ms)` produziert pro Mandatory-Metric eine
`AmqpExtValue::Map`. Integration-Test
`c1_13_metrics_link.rs` verifiziert dass alle 14 Metrics
emittiert werden, dass jedes Sample `{name, value, unit,
timestamp}` enthält und dass Counter-Werte sich in den
Samples widerspiegeln.

**Tests:** `ddsamqp_c_1_13_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`ddsamqp_c_1_13_metrics_carry_traffic_counter_values`,
`ddsamqp_2_2_metrics_units_are_spec_symbols`.

**Status:** done

### Annex C C.1.14 Audit Link

**Spec:** Annex C §C.1.14, S. 128 — Receiver auf `$audit`;
SASL-Erfolg eines zweiten Clients erzeugt
`link.attach.success`-Audit-Record.

**Repo:** `AuditEvent` enum + `AuditProducer` (Ringbuffer) +
`audit_event_sample(event, now_ms)` produzieren AMQP-Map-Bodies
mit `event-type` Spec-Symbol und event-spezifischen Feldern.
Integration-Test `c1_14_audit_link.rs` verifiziert
`link.attach.success`-Event mit subject_name,
`access.unauthorized`-Event mit Resource, FIFO-Order der
Events, Ringbuffer-Eviction.

**Tests:** `ddsamqp_c_1_14_link_attach_success_event_carries_subject`,
`ddsamqp_2_2_audit_producer_streams_events_in_order`,
`ddsamqp_c_1_14_unauthorized_event_carries_resource_field`,
`ddsamqp_c_1_14_ringbuffer_evicts_oldest_on_overflow`.

**Status:** done

### Annex C C.1.15 Loop-Prevention

**Spec:** Annex C §C.1.15, S. 128 — (a) Sample mit eigener
bridge_id wird gedroppt; (b) Sample mit hop > cap wird gedroppt;
beide inkrementieren entsprechende Metric.

**Repo:** `coexistence::inspect_inbound` + `stamp_outbound`
realisieren beide Sub-Tests. Integration-Test
`c1_15_loop_prevention.rs` verifiziert: (a) Self-Tag in String-
und List-Form droppt, (b) hop>cap droppt, hop=cap forwards,
Round-Trip Stamp-then-Inspect droppt sich selbst,
Multi-Hop-Stamp inkrementiert sauber, sauberes Sample passiert.

**Tests:** 7 Tests (`c1_15a_*`, `c1_15b_*`, `c1_15_round_trip_*`,
`c1_15_outbound_stamp_*`, `ddsamqp_c_1_15_clean_sample_passes_through`).

**Status:** done

### Annex C C.2.1 Outbound Connection

**Spec:** Annex C §C.2.1, S. 130 — Bridge connectet zu konfiguriertem
Upstream-Broker.

**Repo:** `client::connect_outbound` + Integration-Test
`client::tests::ddsamqp_2_2_outbound_connect_to_local_server` fährt
echten TCP-Connect + AMQP-Open-Roundtrip gegen lokalen
Server-Daemon. C.2.4 (Reconnect) baut darauf auf.

**Tests:** `ddsamqp_2_2_outbound_connect_to_local_server`,
`ddsamqp_c_2_1_single_connect_attempt_works_for_normal_path`.

**Status:** done

### Annex C C.2.2 Sender-Link Bridge-to-Broker

**Spec:** Annex C §C.2.2, S. 130 — Bridge attached Sender-Link
für outbound Topic-Mapping.

**Repo:** Bridge-DDS-Side-Subscribe (`subscribe_outbound`) +
AMQP-Outbound-Producer-Pfad. Test-Roundtrip über zwei
`InMemoryDdsHost`-Instanzen, die den Broker-Hop simulieren.

**Tests:** `ddsamqp_2_1_bridge_outbound_publish_flows_to_dds`.

**Status:** done

### Annex C C.2.3 Receiver-Link Broker-to-Bridge

**Spec:** Annex C §C.2.3, S. 130 — analog zu C.2.2 in Gegenrichtung.

**Repo:** Bridge-Receiver-Side ruft `bridge::dispatch_transfer`
mit eingehendem Broker-Sample → DDS-DataWriter publishtet.

**Tests:** `ddsamqp_2_1_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### Annex C C.2.4 Reconnect on Loss

**Spec:** Annex C §C.2.4, S. 131 — Connection-Loss → Reconnect
innerhalb konfiguriertem Backoff-Cap (default 60s); HISTORY-
Eviction zählt.

**Repo:** `client::connect_with_reconnect` mit
`ReconnectConfig` (Default 1s init, 2x mult, 60s cap gemäß
Spec §10.8). Integration-Test `c2_4_reconnect_on_loss.rs`.

**Tests:** `ddsamqp_10_8_reconnect_loop_pacing_follows_spec_defaults`,
`ddsamqp_10_8_reconnect_aborts_on_max_attempts`,
`ddsamqp_10_8_reconnect_after_server_restart_eventually_succeeds`,
`ddsamqp_c_2_1_single_connect_attempt_works_for_normal_path`.

**Status:** done

### Annex C C.2.5 Settlement Pass-Through

**Spec:** Annex C §C.2.5, S. 131 — Broker-Settlement →
DataWriter-Notification.

**Repo:** `link::LinkSession::deliver`/`settle` Pending-Tracking
+ `dds_bridge::DispositionMapper`-Trait (`apply` mit DDS-Side-
Sample-Handle). DataWriter-Acknowledgment ist Caller-Layer
(echter `DcpsDdsHost` ruft `DataWriter::wait_for_acknowledgment`).

**Tests:** `link::tests::ddsamqp_7_4_1_settle_decrements_pending`,
`dds_bridge::tests::ddsamqp_7_7_3_noop_disposition_mapper_does_nothing`.

**Status:** done

### Annex C C.2.6 Dual-Identity Separation

**Spec:** Annex C §C.2.6, S. 131 — Bridge präsentiert
DDS-Side-Identity (`CN=Bridge-1`) statt Broker-Side-Credential
(`Alice`) zu DDS-Security-AccessControl.

**Repo:** `security::DualIdentity` mit `for_broker()`/`for_dds()`
Getter; AccessControl-Plugin verwendet ausschließlich
`for_dds()`. Lib-Tests in `security::tests` decken den
Spec-§C.2.6-Pfad direkt ab.

**Tests:** `security::tests::ddsamqp_2_2_dual_identity_keeps_broker_and_dds_separate`,
`security::tests::ddsamqp_2_2_dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### Annex C C.2.7 Loop-Prevention (Bridge)

**Spec:** Annex C §C.2.7, S. 132 — analog C.1.15 für Bridge-
Profile (Outbound + Inbound).

**Repo:** `coexistence`-Layer ist Profile-agnostisch; Bridge-
Profile bindet das gleiche `inspect_inbound`/`stamp_outbound`-
Paar. Integration-Test
`c2_7_loop_prevention_bridge.rs`.

**Tests:** `ddsamqp_c_2_7_outbound_then_inbound_round_trip_drops_self`,
`ddsamqp_c_2_7_hop_cap_exceeded_drops_with_metric`,
`ddsamqp_c_2_7_multi_bridge_chain_terminates_at_hop_cap`,
`ddsamqp_c_2_7_foreign_bridges_in_history_dont_trigger_self_drop`.

**Status:** done

### Annex C C.2.8 Dual Management Surface

**Spec:** Annex C §C.2.8, S. 132 — Bridge DDS-side exponiert
$catalog/$metrics/$audit; Upstream-Broker-Side governed by broker.

**Repo:** `bridge::dispatch_attach` erkennt
`AddressKind::{Catalog, Metrics, Audit}` und liefert die
entsprechenden `AttachOutcome`-Varianten; `produce_catalog_transfers`
liefert Catalog-Stream aus dem `DdsHost`. Upstream-Broker-Side
managementgehört zum Broker (RabbitMQ-HTTP-API, etc.); kein
Bridge-Code.

**Tests:** `ddsamqp_c_1_8_catalog_receiver_gets_one_sample_per_topic`,
`bridge::tests::ddsamqp_c_2_8_dispatch_attach_to_metrics_recognised`,
`ddsamqp_c_2_8_dispatch_attach_to_audit_recognised`.

**Status:** done

### Annex C C.3 Codec-Profile Tests

**Spec:** Annex C §C.3, S. 134 — In-Process-Type-System-
Roundtrips für alle Primitives, Composites, Sections.

**Repo:** `crates/amqp-bridge/src/types.rs::tests`,
`crates/amqp-bridge/src/extended_types.rs::tests`, `crates/amqp-bridge/src/frame.rs::tests`,
`performatives.rs::tests`, `sections.rs::tests`.

**Tests:** `crates/amqp-bridge/src/codec_profile.rs::tests`:
`ddsamqp_2_3_active_profile_full_by_default`, `ddsamqp_2_3_primitives_are_codec_lite`,
`ddsamqp_2_3_list_map_array_are_not_codec_lite`, `ddsamqp_2_3_data_section_is_codec_lite`,
`ddsamqp_2_3_other_sections_are_not_codec_lite`; dazu die Typ-Roundtrips aus
`crates/amqp-bridge/src/types.rs::tests` und
`crates/amqp-bridge/src/extended_types.rs::tests`. `cargo test -p
zerodds-amqp-bridge --lib` — 70 Tests grün.

**Status:** done

### Annex C C.4 Codec-Lite-Profile Tests

**Spec:** Annex C §C.4, S. 135 — strikte Untermenge von C.3.

**Repo:** `cargo test -p zerodds-amqp-bridge --features codec-lite`
läuft das volle Codec-Test-Set + die 5 zusätzlichen
`codec_profile`-Tests, die das Lite-Subset markieren. Conformance-
Marker `active_profile()` liefert `CodecProfile::Lite` unter dem
Feature.

**Tests:** alle `zerodds-amqp-bridge`-Tests (75 grün) +
`codec_profile::tests::ddsamqp_2_3_active_profile_full_by_default` (umgekehrt
unter `--features codec-lite`).

**Status:** done

### Annex C C.5 Test Harness

**Spec:** Annex C §C.5, S. 136 — vendor-neutrales Wording;
Implementer SHOULD veröffentlicht harness-source.

**Repo:** `tools/amqp-dds-endpoint/tests/common`.

**Tests:** `tools/amqp-dds-endpoint/tests/common`,
`tools/amqp-dds-endpoint/tests/c1_1_connection_open.rs::ddsamqp_2_1_connection_open_with_tls_active_advertises_plain`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_the_client_reconnects_once_the_server_is_up`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_loop_pacing_follows_spec_defaults`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_aborts_on_max_attempts`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_10_8_reconnect_after_server_restart_eventually_succeeds`,
`tools/amqp-dds-endpoint/tests/c2_4_reconnect_on_loss.rs::ddsamqp_c_2_1_single_connect_attempt_works_for_normal_path`.

**Status:** done — strukturell (Harness-Wahl ist
implementation-defined).

---

## Annex D — DDS-RPC Correlation Mapping (normative when activated)

### Annex D D.1 Mapping Table

**Spec:** Annex D §D.1, S. 138 — `requestId`→`message-id`
(override des default per-Sample-Identifier),
`relatedRequestId`→`correlation-id`, reply-Topic→`reply-to`,
service-instance→`dds:rpc-instance`,
RPC-operation→`dds:rpc-operation`.

**Repo:** `rpc_correlation::ReplyProperties::from_amqp` liest
`correlation-id` (Str/Symbol/Uuid/Binary) und normalisiert auf
String-Lookup-Key. `OutstandingCalls::issue` trägt
`request_id` (= `message-id`) ein.

**Tests:** `rpc_correlation::tests::ddsamqp_d_1_from_amqp_handles_str_symbol_uuid_binary`.

**Status:** done

### Annex D D.2 Activation

**Spec:** Annex D §D.2, S. 139 — `TopicMapping.rpc_aware = true`
aktiviert; Default `false`.

**Repo:** `rpc_correlation::RpcConfig::rpc_aware`-Feld
(Default `false`); `OutstandingCalls::new(cfg)` instanziiert
nur wenn `rpc_aware = true` aktiviert wird.

**Tests:** `rpc_correlation::tests::ddsamqp_d_2_defaults_match_spec`.

**Status:** done

### Annex D D.3 Scope of this Annex

**Spec:** Annex D §D.3, S. 139 — kein wire-level RPC-Bridge,
nur Property-Mapping; volles RPC-over-AMQP ist
spätere Spec.

**Repo:** `crates/amqp-endpoint/src/rpc_correlation.rs` (`RpcConfig`, `DEFAULT_RPC_TIMEOUT_MS`, `DEFAULT_MAX_OUTSTANDING_CALLS`).

**Tests:** `crates/amqp-endpoint/src/rpc_correlation.rs::ddsamqp_d_2_defaults_match_spec`,
`crates/amqp-endpoint/src/rpc_correlation.rs::ddsamqp_d_3_validate_reply_rejects_missing_correlation_id`,
`crates/amqp-endpoint/src/properties.rs::ddsamqp_7_7_the_operations_are_write_register_unregister_dispose`.

**Status:** done — Spec-Inhalts-Klassifikation.

### Annex D D.4 Reply Validation

**Spec:** Annex D §D.4, S. 140 — Reply-Acceptance:
correlation-id PFLICHT, Match auf outstanding RPC-Call
PFLICHT, Body-Decode mode-abhängig (PASSTHROUGH/JSON/
AMQP_NATIVE). Failure-Dispositions Tabelle. Per-Call-Timeout
`rpc_timeout_ms` (default 30000) → RETCODE_TIMEOUT. Non-
Blocking-Guarantee. Bounded outstanding-calls table mit
RETCODE_OUT_OF_RESOURCES.

**Repo:** `rpc_correlation::OutstandingCalls` mit `BTreeMap`-
basierter bounded Tabelle (`max_outstanding`, default 4096),
`validate_reply` mit allen 4 Failure-Dispositionen
(RejectMalformed/DropUnknown/DecodeFailure/DropLateReply +
Surface), `expire_overdue` mit Per-Call-Timeout, `issue`
liefert `OutOfResources` bei voller Tabelle. Wires gegen
`MetricsHub::on_dropped_malformed_reply` /
`on_rpc_timeout` / `on_decode_error`.

**Tests:** `rpc_correlation::tests` (13 Tests inkl.
`ddsamqp_d_3_validate_reply_rejects_missing_correlation_id`,
`ddsamqp_d_4_validate_reply_drops_unknown_correlation_id`,
`ddsamqp_d_4_validate_reply_late_reply_dropped`,
`ddsamqp_d_4_expire_overdue_removes_and_counts`,
`ddsamqp_d_4_issue_accepts_then_full`,
`ddsamqp_d_4_validate_reply_does_not_block_on_table_full`).

**Status:** done

---

## Audit-Status

123 done / 0 partial / 0 open / 0 blocked / 1 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document dds-amqp-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-amqp-bridge --all-targets` (Job `merge-rust-amqp-bridge`) — 47 Tests grün.
* `cargo nextest run --profile merge -p amqp-dds-endpoint --all-targets` (Job `merge-rust-amqp-dds-endpoint`) — 80 Tests grün.
* `cargo nextest run --profile merge -p zerodds-amqp-endpoint --all-targets` (Job `merge-rust-amqp-endpoint`) — 177 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor-amqp` (Job `integration-bridges-vendor-amqp`) — 1 Test grün.

305 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
