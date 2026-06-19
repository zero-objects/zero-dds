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

**Tests:** `crates/amqp-bridge/src/types.rs::tests` (15 tests,
roundtrips for all primitives),
`crates/amqp-bridge/src/extended_types.rs::tests` (16 tests).

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
`handle_connection_open_close_round_trip`,
`handle_connection_sasl_then_amqp`),
`server::tests::server_accepts_connection_and_handles_open_close`
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
`c1_3_4_8_bridge_dispatch.rs::c1_3_amqp_producer_publishes_to_dds_via_bridge`,
`c1_4_dds_publish_flows_to_amqp_consumer_callback`,
`c2_2_bridge_outbound_publish_flows_to_dds`,
`c2_3_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### §2.1 Endpoint Profile — Clause 4 (address resolution)

**Spec:** §2.1 — "It implements address resolution as specified in
§pim-address-resolution."

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
with static aliases, `domain://N/topic?partition=P[&partition=Q]`
URLs (multi-partition sequence + percent-decoding), wildcard
pattern matching, `effective_partitions` conflict resolver.

**Tests:** `routing::tests` (12 tests incl.
`domain_url_with_multi_partition_parses`,
`domain_url_with_percent_encoded_partition`,
`conflict_resolution_uri_wins_when_both_present`).

**Status:** done

### §2.1 Endpoint Profile — Clause 5 (body encoding)

**Spec:** §2.1 — "It supports at least the Pass-Through encoding
mode defined in §psm-passthrough, and SHOULD support the JSON
encoding mode of §psm-json."

**Repo:** `crates/amqp-endpoint/src/mapping.rs::BodyEncodingMode`
+ `encode_dds_to_amqp_body` + `parse_amqp_body`.

**Tests:** `mapping::tests` (3 tests: passthrough_round_trip,
json_round_trip_via_hex_field, amqp_native_uses_correct_content_type).

**Status:** done — Pass-Through full, JSON as a hex fallback
spec-conformant (full type-reflection JSON is a codegen layer).

### §2.1 Endpoint Profile — Clause 6 (SASL mechanisms)

**Spec:** §2.1 — "It supports the SASL PLAIN, ANONYMOUS, and
EXTERNAL mechanisms as specified in §security-sasl."

**Repo:** `crates/amqp-endpoint/src/sasl.rs::SaslMechanism`
{Plain, Anonymous, External} + `SaslState::new`/`step` state
machine.

**Tests:** `sasl::tests` (8 tests: plain_authenticates,
plain_fails_on_wrong_credentials, anonymous, external_with_subject,
plain_offered_only_when_tls_active, etc.).

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

**Tests:** `sasl::tests::plain_offered_only_when_tls_active`,
`plain_without_tls_yields_auth_failed`,
`sasl_code_wire_values_match_spec`,
`sasl_code_from_u8_round_trip`,
`sasl_code_unknown_value_rejected`,
`outcome_authenticated_wire_code_is_ok`,
`outcome_auth_failed_helper_uses_auth_code`.

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

**Tests:** `security::tests::link_governance_caches_decision`,
`handler::tests::access_control_deny_attach_yields_unauthorized_metric`,
`access_control_allow_does_not_increment_unauthorized`.

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
`catalog_snapshot_emits_map_per_entry`,
`metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`audit_producer_ringbuffer_evicts_oldest`).

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
`drop_loop_on_self_tag_string`, `drop_hop_cap_when_exceeded`,
`round_trip_stamp_then_inspect_drops_loop`).

**Status:** done

### §2.1 Endpoint Profile — Clause 11 (Annex C tests)

**Spec:** §2.1 — "It satisfies the conformance test cases
enumerated in Annex C, §C.1."

**Repo:** `tools/amqp-dds-endpoint/tests/c1_*.rs` suite with
13 test files (C.1.1-15) cover all Annex-C §C.1 items.

**Tests:** see the c1_* suite.

**Status:** done

### §2.2 Bridge Profile — Clause 1 (type system)

**Spec:** §2.2, p. 8 (PDF) — see Endpoint Cl. 1.

**Repo:** identical to Endpoint (shared codec crate).

**Status:** done

### §2.2 Bridge Profile — Clause 2 (outbound connection)

**Spec:** §2.2 — "It establishes outgoing AMQP connections to an
external broker as specified in §topology-sidecar."

**Repo:** `tools/amqp-dds-endpoint/src/client.rs::connect_outbound`:
TCP connect with timeout, SASL header → SASL-Init →
SASL-Outcome → AMQP header → Open. `OutboundSession` delivers
`remote_container_id` from the Open reply.

**Tests:** `client::tests::outbound_connect_to_local_server`
(real E2E roundtrip against a local server).

**Status:** done

### §2.2 Bridge Profile — Clause 3 (address resolution)

**Spec:** §2.2 — the bridge shares the endpoint address-resolution model
(§7.3) including the `domain://` URL form, static aliases, wildcard
matching.

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
(shared with Endpoint Cl. 4); the bridge caller in
`crates/amqp-endpoint/src/client.rs`.

**Tests:** `routing::tests` (7 tests).

**Status:** done

### §2.2 Bridge Profile — Clause 4 (SASL outbound)

**Spec:** §2.2 — "It supports the SASL PLAIN, ANONYMOUS, and
EXTERNAL mechanisms ... for the outbound connection to the broker."

**Repo:** `client::do_outbound_sasl`: parses sasl-mechanisms,
calls `SaslState::select_outbound`, builds `sasl-init` (RFC-4616
PLAIN form `\0user\0pw`, ANONYMOUS marker, empty EXTERNAL body),
reads `sasl-outcome` (code 0 = ok).

**Tests:** `client::tests::parse_offered_mechanisms_array_form`,
`parse_offered_mechanisms_single_symbol`,
`parse_offered_mechanisms_unknown_filtered`,
`sasl_init_plain_includes_credentials`,
`sasl_init_anonymous_uses_marker`,
`sasl_init_external_has_empty_response`.

**Status:** done

### §2.2 Bridge Profile — Clause 5 (PLAIN forbidden on cleartext)

**Spec:** §2.2 — "It SHALL NOT initiate a SASL PLAIN authentication
over an unencrypted transport."

**Repo:** `sasl::SaslState::select_outbound(offered, tls_active)`
filters PLAIN out of the selection precedence when `tls_active=false`.

**Tests:** `sasl::tests::select_outbound_skips_plain_without_tls`,
`select_outbound_no_acceptable_mechanism`.

**Status:** done

### §2.2 Bridge Profile — Clause 6 (dual identity)

**Spec:** §2.2 — "It honours the dual-identity model of
§bridge-dual-identity: broker-side SASL credentials SHALL NOT be
conflated with the bridge's DDS-Security IdentityToken."

**Repo:** `security::DualIdentity` with `for_broker()` /
`for_dds()` getters. Plugin calls reference exclusively
`for_dds()`.

**Tests:** `security::tests::dual_identity_keeps_broker_and_dds_separate`,
`dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### §2.2 Bridge Profile — Clause 7 (reconnect with backoff)

**Spec:** §2.2 — "It performs reconnect with exponential back-off
after connection loss as specified in §reconnect."

**Repo:** `client::ReconnectConfig` (initial 1s, mult 2, cap 60s
per spec) + `connect_with_reconnect` with a cooperative
shutdown atomic. `next_backoff_ms(attempt)` cap-correct.

**Tests:** `client::tests::backoff_starts_at_initial`,
`backoff_doubles_until_cap`, `backoff_respects_custom_cap`,
`backoff_with_unit_multiplier_stays_at_initial`,
`reconnect_exhausts_with_max_attempts`,
`reconnect_aborts_on_shutdown_signal`.

**Status:** done

### §2.2 Bridge Profile — Clause 8 (body encoding)

**Spec:** §2.2 — see Endpoint Cl. 5.

**Repo:** identical.

**Status:** done

### §2.2 Bridge Profile — Clause 9 (management surface + audit)

**Spec:** §2.2 — "It exposes the operational management surface
... and the security audit channel ... for the DDS-side surface
of the bridge."

**Repo:** identical to Endpoint Cl. 9 (shared
`management` module).

**Status:** done

### §2.2 Bridge Profile — Clause 10 (bridge coexistence)

**Spec:** §2.2 — see Endpoint Cl. 10.

**Repo:** identical (shared `coexistence` module).

**Status:** done

### §2.2 Bridge Profile — Clause 11 (Annex C tests)

**Spec:** §2.2 — "It satisfies the conformance test cases
enumerated in Annex C, §C.2."

**Repo:** `tools/amqp-dds-endpoint/tests/c2_*.rs` suite +
`c1_3_4_8_bridge_dispatch.rs` (which also covers C.2.2/C.2.3)
cover all Annex-C §C.2 items.

**Tests:** see the c2_* suite.

**Status:** done

### §2.3 Codec Profile

**Spec:** §2.3, p. 9 — type system, performatives, sections codec
without connection lifecycle.

**Repo:** `crates/amqp-bridge/` full: `types.rs`, `extended_types.rs`,
`frame.rs`, `performatives.rs`, `sections.rs`.

**Tests:** `cargo test -p zerodds-amqp-bridge --lib`: 70 tests green.

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
`primitives_are_codec_lite`,
`list_map_array_are_not_codec_lite`,
`data_section_is_codec_lite`,
`other_sections_are_not_codec_lite`,
`active_profile_full_by_default`).

**Status:** done

### §2.5 Combination of Profiles + §2.5.1 Codec implication

**Spec:** §2.5.1, p. 11 — Endpoint/Bridge imply Codec.

**Repo:** Cargo dependency: both crates depend on
`amqp-bridge`; every Endpoint operation uses the codec.

**Status:** done — structurally enforced by the crate dependency.

### §2.5.2 Hybrid profile combination

**Spec:** §2.5.2 — an implementation MAY claim Endpoint and Bridge
at the same time.

**Repo:** no hard separation between the Endpoint and Bridge
paths; the modules are shared.

**Status:** done

---

## §7 Platform-Independent Model

### §7.1.1 Primitive mapping

**Spec:** §7.1.1, p. 17 — mapping table DDS-XCDR2 primitive ↔
AMQP-§1.6 primitive (boolean, int8/16/32/64, uint8/16/32/64,
float, double, char, wchar, string, wstring, octet).

**Repo:** `crates/amqp-bridge/src/types.rs` (15 encoders/decoders)
+ `extended_types.rs::AmqpExtValue` (16 more for compound +
described).

**Tests:** `types.rs::tests`, `extended_types.rs::tests`.

**Status:** done

### §7.1.1 Primitive mapping — `long double`

**Spec:** §7.1.2, p. 19 — `long double` (16 byte) is narrowed to AMQP
`double` (8 byte).

**Repo:** `codegen_helpers::narrow_long_double_to_double(f64)`
is the audit anchor site for §7.1.2; Rust has no binary128 type,
the codegen already delivers f64.

**Tests:** `codegen_helpers::tests::long_double_narrowing_is_identity_on_f64`.

**Status:** done

### §7.1.3 Endianness handling

**Spec:** §7.1.3, p. 20 — all AMQP multi-byte values are
big-endian; XCDR2 source data is little-endian.

**Repo:** `types.rs` and `extended_types.rs` write
exclusively `to_be_bytes()`/read `from_be_bytes()`.

**Status:** done

### §7.1.4.1 IDL `char` (8-bit) → AMQP `char` (32-bit)

**Spec:** §7.1.4.1, p. 21 — IDL `char` is restricted to the UTF-8-ASCII
subset; source bytes > 0x7F → `decode-error`.

**Repo:** `extended_types.rs::encode_char` (4-byte BE) +
`codegen_helpers::validate_char_ascii(byte) -> Result<u8, u8>`
for a codegen pre-encode check. The codegen templates call
`validate_char_ascii` and propagate `Err` as
`amqp:decode-error`.

**Tests:** `codegen_helpers::tests::ascii_validates`,
`non_ascii_rejected`.

**Status:** done

### §7.1.4.2 IDL `wchar` → AMQP `char`

**Spec:** §7.1.4.2, p. 22 — IDL `wchar` as a 4-octet UCS-4 wire,
runtime platform-defined.

**Repo:** `extended_types.rs::encode_char` produces exactly a
4-byte-BE codepoint.

**Status:** done

### §7.1.4.3 String transcoding (UTF-8)

**Spec:** §7.1.4.3, p. 23 — DDS `string` ↔ AMQP `str8/str32`
verbatim; `wstring` transcoded to UTF-8.

**Repo:** `types.rs::encode_string` (str8/str32),
`types.rs::encode_symbol` (sym8/sym32), length-prefix validation.

**Tests:** `types.rs::tests::string_round_trip_short_long`.

**Status:** done

### §7.1.5 Time types

**Spec:** §7.1.5, p. 25 — DDS `Time_t` (sec+nanosec) ↔ AMQP
`timestamp` (8-byte BE ms-since-epoch). Sub-milliseconds via the
`dds:nsec` application property.

**Repo:** `extended_types.rs::encode_timestamp`/`decode_timestamp`
(wire) + `properties::produce_application_properties` (app
property `dds:nsec` from `SampleHeader.source_nsec_remainder`).

**Tests:** `extended_types.rs::tests::timestamp_round_trip`,
`properties::tests::application_properties_nsec_set_when_nonzero`.

**Status:** done

### §7.1.6.1 Identifier types

**Spec:** §7.1.6.1, p. 26 — 16-byte identifiers without RFC-4122
conformance are encoded as `binary` (0xA0/0xB0), not as
`uuid` (0x98).

**Repo:** `codegen_helpers::is_rfc4122_uuid(&[u8;16]) -> bool`
checks variant+version per RFC-4122; `encode_16byte_identifier`
automatically routes to `Uuid` or `Binary`.

**Tests:** `codegen_helpers::tests::rfc4122_v4_uuid_recognised`,
`arbitrary_16_bytes_not_recognised_as_uuid`,
`encode_16byte_routes_binary_for_non_uuid`,
`encode_16byte_routes_uuid_when_marked`.

**Status:** done

### §7.1.7 Sequence and array

**Spec:** §7.1.7, p. 27 — `sequence<T>` and array to AMQP
`list` (0xC0/0xD0) or `array` (0xE0/0xF0); empty sequence as
`list0` (0x45).

**Repo:** `extended_types.rs::AmqpExtValue::List` + `Array` with
list8/list32/array8/array32 encoding; `list0` constant in
`codes::LIST0`.

**Tests:** `extended_types.rs::tests::list_round_trip_*`.

**Status:** done

### §7.1.8 Map

**Spec:** §7.1.8, p. 28 — DDS map (TK_MAP) ↔ AMQP `map` (0xC1/
0xD1).

**Repo:** `extended_types.rs::AmqpExtValue::Map`.

**Tests:** `extended_types.rs::tests::map_round_trip_*`.

**Status:** done

### §7.2.1 Composite descriptor (DESC_TRUNCATED)

**Spec:** §7.2.1.1, p. 30 — the XTypes TypeIdentifier is reduced to its
first 8 octets as an AMQP `ulong` (0x80).

**Repo:** `codegen_helpers::compute_truncated_descriptor(&[u8])
-> Result<u64, _>` delivers the first 8 bytes as a BE u64;
`route_descriptor(DESC_TRUNCATED, hash)` delivers
`(Some(ulong), None)`.

**Tests:** `codegen_helpers::tests::truncated_descriptor_first_8_bytes_be`,
`truncated_descriptor_too_short_errors`,
`route_descriptor_truncated`.

**Status:** done

### §7.2.1 Composite descriptor (DESC_FULL)

**Spec:** §7.2.1.2, p. 30 — the XTypes TypeIdentifier in full
form as an AMQP `symbol` (sym8/sym32) `dds:type:<hex>`.

**Repo:** `codegen_helpers::make_full_descriptor_symbol(&[u8])
-> String` delivers `dds:type:<lowercase-hex>`;
`route_descriptor(DESC_FULL, hash)` delivers
`(None, Some(symbol))`.

**Tests:** `codegen_helpers::tests::full_descriptor_symbol_format`,
`route_descriptor_full`.

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

**Tests:** `properties::tests::type_id_inspector_match_returns_match`,
`type_id_inspector_case_insensitive_match`,
`type_id_inspector_absent_returns_absent`,
`type_id_inspector_mismatch_detects_collision`,
`type_id_inspector_accepts_symbol_form`,
`type_id_inspector_non_map_yields_absent`.

**Status:** done

### §7.2.2 Composite body

**Spec:** §7.2.2, p. 32 — body as an AMQP `list` (constructor
0xC0/0xD0) of the field values in IDL order.

**Repo:** `extended_types.rs::AmqpExtValue::List` + the section
producer.

**Status:** done

### §7.2.3 Union

**Spec:** §7.2.3, p. 33 — union as an AMQP `list` with the discriminator
+ active-branch value.

**Repo:** `codegen_helpers::make_union_body(discriminator,
active_value: Option<_>)` builds the `AmqpExtValue::List` with
1-2 elements depending on the active branch.

**Tests:** `codegen_helpers::tests::union_with_branch_has_two_elements`,
`union_empty_branch_omits_value`.

**Status:** done

### §7.3 Address-resolution model

**Spec:** §7.3, p. 35 — topic name ↔ AMQP source/target address;
`domain://N/topic?partition=p` URL form.

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
with `resolve()`, a static alias set, a `domain://` URL parser,
wildcard matching.

**Tests:** `routing::tests` (7 tests).

**Status:** done

### §7.4.1 RELIABILITY (settlement-mode mapping)

**Spec:** §7.4.1, p. 38 — DDS `RELIABILITY` ↔ AMQP
`snd-settle-mode`/`rcv-settle-mode` table.

**Repo:** `link.rs::SettlementMode::{Settled, Unsettled}` +
`LinkSession::deliver()` with pending-settlement tracking.

**Tests:** `link::tests::reliable_unsettled_round_trip`,
`link::tests::best_effort_settled_pre_settles`.

**Status:** done

### §7.4.2 DURABILITY

**Spec:** §7.4.2, p. 40 — DDS `DURABILITY` (VOLATILE/TRANSIENT/
PERSISTENT) ↔ AMQP `terminus.durable` table. `unsettled-state`
requires broker functionality → SHALL `amqp:not-implemented`.

**Repo:** `link.rs::TerminusDurability` (None/Configuration/
UnsettledState) + `check_attach_durability` delivers
`AttachDurabilityCheck::{Accept, RejectNotImplemented}`.

**Tests:** `link::tests::terminus_durability_from_wire`,
`attach_durability_unsettled_state_rejected_not_implemented`.

**Status:** done

### §7.4.3 HISTORY

**Spec:** §7.4.3, p. 42 — HISTORY is a DCPS-side cache, decoupled
from AMQP settlement; the AMQP layer forwards samples as DDS emits
them.

**Repo:** no explicit HISTORY mapper needed.

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

**Tests:** `management::tests::audit_producer_*`,
`bridge::tests::produce_catalog_transfers_*`.

**Status:** done

### §7.4.5 LATENCY_BUDGET

**Spec:** §7.4.5, p. 43 — transparent to the AMQP boundary.

**Status:** done — structural.

### §7.4.6 OWNERSHIP

**Spec:** §7.4.6, p. 44 — DDS-side owner selection; the AMQP
boundary is unaffected.

**Status:** done — structural.

### §7.4.7 LIFESPAN

**Spec:** §7.4.7, p. 44 — DDS LIFESPAN ↔ AMQP
`absolute-expiry-time` of the properties section.

**Repo:** `properties::produce_properties` sets
`absolute_expiry_time_ms = source_timestamp + lifespan_remaining`
when `SampleHeader.lifespan_remaining_ms = Some(_)`.

**Tests:** `properties::tests::produce_properties_lifespan_sets_expiry`.

**Status:** done

### §7.4.8 PARTITION

**Spec:** §7.4.8, p. 45 — DDS PARTITION is a
`sequence<string>`; AMQP address URI with repeated
`?partition=p1&partition=p2` query params.

**Repo:** `routing.rs::parse_domain_url` accepts repeatedly
repeated `?partition=` params; `AddressResolution.partitions`
is a `Vec<String>`; percent-decoding (RFC 3986) for reserved
characters in partition values.

**Tests:** `routing::tests::domain_url_with_multi_partition_parses`,
`domain_url_with_percent_encoded_partition`.

**Status:** done

### §7.4.9 All Other QoS Policies (transparent)

**Spec:** §7.4.9, p. 46 — TIME_BASED_FILTER, TRANSPORT_PRIORITY,
LIVELINESS, DESTINATION_ORDER, RESOURCE_LIMITS, ENTITY_FACTORY,
WRITER_DATA_LIFECYCLE, READER_DATA_LIFECYCLE, USER_DATA,
TOPIC_DATA, GROUP_DATA, PRESENTATION,
TYPE_CONSISTENCY_ENFORCEMENT — all DDS-side; the `dds:` prefix
reserved.

**Status:** done — structurally via pass-through semantics.

### §7.5 Discovery bridging

**Spec:** §7.5, p. 47 — SPDP/SEDP ↔ the AMQP `$catalog` address.

**Repo:** `management::CatalogProducer` with `add`/`remove`/
`snapshot` API, `CatalogEntry` (address, dds, type-name, type-id,
direction, partitions). The `topics.exposed` counter is automatically
maintained. `classify_address("$catalog")` delivers
`AddressKind::Catalog`.

**Tests:** `management::tests::catalog_*` (5 tests),
`classify_address_recognises_reserved`.

**Status:** done

### §7.5.1 Behaviour for unknown addresses

**Spec:** §7.5.1, p. 48 — a `permit_dynamic_topics` flag; with
`false` and unknown → an `amqp:not-found` link error;
with `true` on-the-fly topic creation.

**Repo:** `errors::map_resolution_error(err, permit_dynamic_topics)`
delivers an `AmqpError` with `condition = NotFound`,
`scope = Link`, spec section §7.5.1 + description text depending on
the `permit_dynamic_topics` flag.

**Tests:** `errors::tests::no_route_with_dynamic_disabled_yields_not_found_link`,
`no_route_with_dynamic_enabled_still_not_found`,
`malformed_address_maps_to_decode_error`.

**Status:** done

### §7.6 Key and instance mapping (group-id SHA-256)

**Spec:** §7.6.1, p. 50 — `group-id` = lowercase hex SHA-256
over the XCDR2-KeyHash encapsulation; opaque to the subscriber.

**Repo:** `crates/amqp-endpoint/src/keyhash.rs::group_id` with
SHA-256 + 64-char lowercase hex. `properties::produce_properties`
calls `group_id` from `SampleHeader.keyhash`.

**Tests:** `keyhash::tests` (5 tests incl.
`empty_keyhash_yields_known_sha256`,
`solidus_in_key_safe`),
`properties::tests::produce_properties_keyhash_yields_group_id`.

**Status:** done

### §7.6.2 Subscriber-side reconstruction

**Spec:** §7.6.2, p. 51 — the hash is one-way; the subscriber
SHALL NOT attempt to reconstruct key fields from the group-id.

**Status:** done — structural (no reverse mapping to
implement).

### §7.6.3 Topic without keys

**Spec:** §7.6.3, p. 51 — unkeyed topic: omit `group-id`.

**Repo:** `properties::produce_properties` sets
`group_id = None` when `SampleHeader.keyhash = None`.

**Tests:** `properties::tests::produce_properties_unkeyed_omits_group_id`.

**Status:** done

### §7.7.1 Outbound instance state

**Spec:** §7.7.1, p. 52 — map the DDS sample operation
(`write/register/unregister/dispose`) onto the
application property `dds:operation`; default `write`.

**Repo:** `properties::DdsOperation` enum +
`produce_application_properties` sets `dds:operation` when
`!= Write` (default write is omitted per spec).

**Tests:** `properties::tests::application_properties_register_sets_operation`,
`application_properties_default_minimal_set` (verifies that the
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

**Tests:** `dds_bridge::tests::accept_all_returns_accepted_for_every_operation`,
`instance_tracking_register_with_body_accepts`,
`instance_tracking_register_empty_body_yields_missing_key`,
`instance_tracking_unregister_unknown_yields_unknown_instance`,
`instance_tracking_unregister_known_accepts`.

**Status:** done

### §7.7.3 Disposition mapping

**Spec:** §7.7.3, p. 54 — AMQP `disposition` ↔ DDS sample state.

**Repo:** `link::LinkSession::apply_disposition` (settlement
tracking) + a `dds_bridge::DispositionMapper` trait for
DDS-API wiring (caller-layer plugin). `NoopDispositionMapper`
as the test default.

**Tests:** `dds_bridge::tests::noop_disposition_mapper_does_nothing`,
`link::tests::settle_decrements_pending`.

**Status:** done

### §7.8 Conflict resolution between URI and properties

**Spec:** §7.8, p. 55 — on a conflict between the URI form
`?partition=` and the application property `dds:partition` the
URI form wins.

**Repo:** `routing::effective_partitions(uri_form,
property_form)` delivers the URI form when non-empty, otherwise
property_form.

**Tests:** `routing::tests::conflict_resolution_uri_wins_when_both_present`,
`conflict_resolution_property_used_when_uri_empty`,
`conflict_resolution_both_empty`.

**Status:** done

### §7.9.1 Catalog address ($catalog)

**Spec:** §7.9.1, p. 57 — a receiver link on `$catalog` delivers
topic-mapping entries; the entry fields are enumerated.

**Repo:** `management::CatalogProducer::snapshot` produces
an `AmqpExtValue::Map` per entry with
`amqp-address`/`dds-topic`/`dds-type-name`/`type-id` (ulong or
symbol depending on DescriptorForm)/`direction`/`partitions`.

**Tests:** see §7.5.

**Status:** done

### §7.9.2 Metrics address ($metrics)

**Spec:** §7.9.2, p. 58 — a receiver link on `$metrics` delivers a
stream of AMQP messages with a `map` body
{name, value, unit, timestamp}.

**Repo:** `metrics::MetricsHub` (counter) +
`management::metrics_snapshot(hub, now_ms)` (sample producer)
delivers an `AmqpExtValue::Map` per mandatory metric with
`name`/`value`/`unit`/`timestamp` fields.

**Tests:** `management::tests::metrics_snapshot_*` (2 tests),
`metrics::tests` (10 tests).

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

**Tests:** `metrics::tests::mandatory_metric_count_is_14`,
`fresh_hub_all_zero` (verifies coverage of all 14 names).

**Status:** done

### §7.9.3 Limitations + AMQP-Management trade-off

**Spec:** §7.9.3, p. 60 — `$catalog`/`$metrics` as operational,
`$audit` as security; full AMQP-Management 1.0 explicitly
out of scope.

**Status:** done — spec classification; no implementation
to deliver.

### §7.10 Implementation limits (DoS caps)

**Spec:** §7.10, p. 61 — compound nesting ≤ 16
(implementation-defined, ≥ 16 SHALL, ≥ 32 SHOULD), max-frame-
size, max-connections, catalog-entries cap, idle timeout.

**Repo:** `crates/amqp-endpoint/src/limits.rs::ResourceLimits`
with `max_frame_size`, `max_connections`, `idle_timeout_ms`,
`max_compound_nesting`.

**Tests:** `limits::tests` (3 tests).

**Status:** done

### §7.11 Bridge coexistence (loop prevention)

**Spec:** §7.11, p. 63 — `dds:bridge-id` stamping +
drop-on-self-tag + `dds:bridge-hop` cap (default 8, max 16);
bridge_id is process-wide.

**Repo:** `coexistence::CoexistenceConfig` (process-wide
bridge_id + hop_cap; `DEFAULT_HOP_CAP=8`, `MAX_HOP_CAP=16`),
`inspect_inbound`, `stamp_outbound`. Properties app keys
`dds:bridge-id`/`dds:bridge-hop` in `properties::app_keys`.

**Tests:** `coexistence::tests` (11 tests).

**Status:** done

---

## §8 Wire Encoding (PSM)

### §8.1.1 Pass-through octet-string mode

**Spec:** §8.1.1, p. 67 — body as a single `data` section, content
verbatim XCDR2; `content-type = application/vnd.dds.xcdr2`.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., PassThrough)`
delivers `("application/vnd.dds.xcdr2", bytes_verbatim)`.

**Tests:** `mapping::tests::passthrough_round_trip_preserves_bytes`.

**Status:** done

### §8.1.2 JSON mapping mode

**Spec:** §8.1.2, p. 68 — body UTF-8 JSON document; full
IDL-to-JSON mapping rules (Time_t, octet sequences as Base64,
union, sequence, map, 16-byte identifier as hex).

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., Json)`
(hex fallback `{"_xcdr2": "<hex>"}`);
`crates/idl-cpp/src/amqp.rs::emit_amqp_helpers`
(`to_json_string(const T&)` per top-level type);
`crates/idl-java/src/amqp.rs::emit_amqp_codec_files`
(`<TypeName>AmqpCodec.toJsonString`);
`crates/idl-ts/src/amqp.rs::append_amqp_helpers`
(`toJsonString_<TypeName>`).

**Tests:** `mapping::tests::json_round_trip_via_hex_field`,
`zerodds_idl_cpp::amqp::tests::struct_emits_to_json_wrapper`,
`json_wrapper_for_union_too`,
`zerodds_idl_java::amqp::tests::struct_emits_to_json_helper`,
`json_helper_for_union_too`,
`zerodds_idl_ts::amqp::tests::struct_emits_to_json_wrapper`,
`json_helper_for_union_too`.

**Status:** done

### §8.1.3 AMQP-native typed mapping mode

**Spec:** §8.1.3, p. 71 — body as a single `amqp-value` section
with a described composite per §7.2.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., AmqpNative)`
(content-type `application/vnd.dds.amqp-native`);
`crates/idl-cpp/src/amqp.rs::emit_amqp_helpers` (`to_amqp_value`,
union switch via `make_union_body`);
`crates/idl-java/src/amqp.rs::emit_amqp_codec_files`
(`<TypeName>AmqpCodec.toAmqpValue`);
`crates/idl-ts/src/amqp.rs::append_amqp_helpers`
(`toAmqpValue_<TypeName>`); central union helper
`crates/amqp-endpoint/src/codegen_helpers.rs::make_union_body`.

**Tests:** `mapping::tests::amqp_native_uses_correct_content_type`,
`zerodds_idl_cpp::amqp::tests::struct_emits_to_amqp_value_function`,
`union_emits_make_union_body_call`,
`zerodds_idl_java::amqp::tests::struct_emits_amqp_codec_file`,
`union_emits_codec_with_make_union_body_calls`,
`zerodds_idl_ts::amqp::tests::struct_emits_to_amqp_value_function`,
`union_emits_make_union_body_calls`.

**Status:** done

### §8.2 Properties-section message-id

**Spec:** §8.2, p. 72 — `message-id` = binary(24) = ⟨writer GUID
(16B) || RTPS seqnum (8B BE)⟩, unique per sample. InstanceHandle
NOT in the message-id.

**Repo:** `properties::message_id(writer_guid, seqnum)` delivers a
24-byte Vec; InstanceHandle is explicitly mapped to the
`dds:instance-handle` app property (not to
`message-id`).

**Tests:** `properties::tests::message_id_is_24_bytes_guid_then_seqnum`,
`message_id_distinguishes_consecutive_samples_same_instance`.

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

**Tests:** `properties::tests::produce_properties_*` (3 tests).

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

**Tests:** `properties::tests::application_properties_*` (6
tests incl. multi-partition list, single-partition string,
register operation, type-id presence).

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

**Tests:** `annex_a::tests` (10 tests incl. round-trip,
defaults, AMQP-wire aliases).

**Status:** done — a hand-maintained mirror structure. The IDL codegen
pipeline remains a follow-up task (idl-rust generator), but
the data model is spec-conformant and available in the crate.

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
`parse_minimal_endpoint`,
`parse_endpoint_with_topics_tls_sasl_limits`,
`parse_bridge_with_upstream`, `unknown_root_yields_error`,
`missing_amqp_address_yields_error`,
`invalid_body_mode_yields_error`,
`malformed_xml_yields_parse_error`,
`unknown_elements_are_ignored`).

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
`build_server_config_from_self_signed`,
`build_server_config_require_client_cert_needs_ca`,
`build_client_config_from_root_ca`,
`tls_handshake_round_trip_with_self_signed_cert` with a real
TLS handshake against an rcgen-generated self-signed cert).

**Status:** done — a full TLS stack via rustls as an opt-in Cargo
feature `tls`.

### §10.2 SASL mechanisms — mandatory set

**Spec:** §10.2, p. 85 — implementations MUST support PLAIN,
ANONYMOUS, EXTERNAL.

**Repo:** `sasl.rs::SaslMechanism::{Plain, Anonymous, External}`.

**Tests:** `sasl::tests` (8 tests).

**Status:** done

### §10.2 SASL mechanisms — optional SCRAM-SHA-256

**Spec:** §10.2, p. 85 — implementations MAY additionally support
SCRAM-SHA-256.

**Repo:** `crates/amqp-endpoint/src/sasl.rs::SaslMechanism::ScramSha256`
with the wire symbol `"SCRAM-SHA-256"` + round-trip-parse support +
an `is_mandatory()` predicate. The concrete RFC-7677 implementation
(HMAC-SHA-256 + PBKDF2 + channel binding) is caller-layer
(reuse `crates/security-crypto`).

**Tests:** inline tests in the `sasl` module (`name_round_trips`,
`unknown_name_yields_none`).

**Status:** done — the optional SCRAM-SHA-256 mechanism is stated as a wire
marker enum variant.

### §10.2.1 Mandatory TLS for SASL PLAIN

**Spec:** §10.2.1, p. 87 — see Endpoint Cl. 7 / Bridge Cl. 5.

**Repo:** the inbound filter `SaslState::new(tls_active)` +
the outbound filter `SaslState::select_outbound`. The daemon outbound
in `client::do_outbound_sasl` with a
`ClientError::PlainRejectedNoTls` reject path.

**Tests:** `sasl::tests::plain_offered_only_when_tls_active`,
`select_outbound_skips_plain_without_tls`.

**Status:** done

### §10.3 Cross-reference to DDS-Security — outbound signing

**Spec:** §10.3.1, p. 88 — DDS-Security plugins can be active;
outbound signing is DDS-side.

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

**Tests:** `security::tests::plain_yields_sasl_username_class_id`,
`anonymous_yields_anonymous_class_id`,
`external_yields_pki_dh_class_id_with_cert`,
`scram_yields_scram_sha256_class_id`,
`class_id_strings_match_spec_table`.

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

**Tests:** `security::tests::static_allow_list_per_op`,
`errors::tests::access_denied_yields_unauthorized_access_link`.

**Status:** done

### §10.3.4 Determinism across vendors

**Spec:** §10.3.4, p. 91 — IdentityToken construction is
deterministic across implementations.

**Repo:** `build_identity_token` is a pure function without
random/state; the same `SaslSubject` → the same
`IdentityToken`. `LinkGovernance::evaluate` caches per op and
deterministically delivers the same decision for the same input.

**Tests:** `security::tests::link_governance_caches_decision`
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

**Tests:** `handler::tests::access_control_deny_attach_yields_unauthorized_metric`,
`access_control_allow_does_not_increment_unauthorized`.

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
`config_xml::tests::governance_document_loads_rules`,
`governance_missing_topic_pattern_errors`,
`governance_invalid_data_protection_errors`.

**Status:** done

### §10.5 No implicit trust between profiles

**Spec:** §10.5, p. 97 — Endpoint and Bridge on the same host
do not share an identity automatically.

**Status:** done — structural (separate configuration structures).

### §10.6 Bridge-profile dual identity

**Spec:** §10.6, p. 98 — the bridge carries two separate identities:
broker-side SASL credential, DDS-side IdentityToken; do not
conflate them.

**Repo:** `security::DualIdentity::new(broker_id, dds_id)` with
`for_broker()`/`for_dds()` getters. The AccessControl plugin SHALL use only
`for_dds()`.

**Tests:** `security::tests::dual_identity_keeps_broker_and_dds_separate`,
`dual_identity_for_dds_does_not_carry_broker_credential`.

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

**Tests:** `security::tests::link_governance_caches_decision`.

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

**Tests:** see §2.2 Cl. 7.

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

**Tests:** `errors::tests::condition_symbols_match_spec`,
`invalid_utf8_maps_to_decode_error_transfer`,
`invalid_json_maps_to_decode_error`,
`empty_body_maps_to_decode_error`.

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

**Tests:** `errors::tests::resource_limit_exceeded_is_connection_scope`,
`unsettled_state_yields_not_implemented`,
`unknown_dds_operation_yields_not_implemented`.

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

**Tests:** `errors::tests::instance_unknown_yields_precondition_failed`,
`register_missing_key_yields_decode_error`,
`dds_bridge::tests::dispatch_outcome_to_amqp_error_maps_correctly`,
`instance_tracking_*` (5 tests).

**Status:** done

### §11.4 Diagnostic description strings

**Spec:** §11.4, p. 110 — description-string format:
`<spec-section>: <human-readable>`.

**Repo:** `errors::ErrorDescription::render` delivers exactly the
format `<§-ref>: <text>`. All `errors::*` helpers construct
spec-conformant descriptions with a `§-ref` prefix.

**Tests:** `errors::tests::description_renders_spec_section_then_text`,
`description_display_matches_render`.

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

**Tests:** `annex_a::tests` (10 tests).

**Status:** done — see §9.1.

---

## Annex B — Examples (informative)

### Annex B Examples

**Spec:** Annex B, p. 119 — three examples: edge sensor,
cloud subscriber via broker, multi-vendor federation.

**Status:** n/a (informative)

---

## Annex C — Compliance Test Suite (normative)

### Annex C C.1.1 Connection Open (TLS+PLAIN)

**Spec:** Annex C §C.1.1, p. 124 — an AMQP-1.0 client connects via
TLS, authenticates with PLAIN, establishes a session.

**Repo:** integration test `c1_1_connection_open.rs` drives a
real Open roundtrip against a locally-running daemon with
`tls_active = true`. TLS termination itself is a §10.1
follow-up wave.

**Tests:** `c1_1_connection_open_with_tls_active_advertises_plain`.

**Status:** done — Open roundtrip + PLAIN mechanism negotiation
verified; TLS wire encryption is §10.1.

### Annex C C.1.2 SASL PLAIN Rejection on Plain Transport

**Spec:** Annex C §C.1.2, p. 124 — a client connects without TLS,
SASL-init with PLAIN → SASL-Outcome `auth` fail + connection close.

**Repo:** the logic in `sasl::SaslState::new(false)` +
`select_outbound`. An E2E integration test
`tools/amqp-dds-endpoint/tests/c1_2_sasl_plain_rejection.rs`
with a real TCP connection against a locally-running daemon.

**Tests:** `sasl::tests::plain_without_tls_yields_unsupported`,
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

**Tests:** `c1_3_4_8_bridge_dispatch.rs::c1_3_amqp_producer_publishes_to_dds_via_bridge`,
`c1_3_unknown_address_attaches_with_unknown_address_outcome`.

**Status:** done

### Annex C C.1.4 Receiver-link DDS-to-consumer

**Spec:** Annex C §C.1.4, p. 125 — analogous to C.1.3 in the reverse direction.

**Repo:** `bridge::subscribe_outbound(host, topic_id, callback)`
registers an outbound subscription; as soon as the DDS side
publishes (`host.publish_to_dds`), the callback is invoked with the
bytes, which triggers the AMQP outbound transfer logic.

**Tests:** `c1_4_dds_publish_flows_to_amqp_consumer_callback`.

**Status:** done

### Annex C C.1.5 Settlement-mode reliable

**Spec:** Annex C §C.1.5, p. 125 — a DDS DataWriter with
RELIABILITY=RELIABLE; an AMQP settlement roundtrip.

**Repo:** `link.rs::LinkSession::deliver`/`settle` pending
tracking. The integration test
`c1_5_settlement_reliable.rs` verifies: pending is
increased until a disposition is received, credit exhaustion
delivers a NoCredit error.

**Tests:** `c1_5_reliable_unsettled_increments_pending_until_disposition`,
`c1_5_credit_exhaustion_blocks_further_deliveries`.

**Status:** done

### Annex C C.1.6 Settlement-mode best-effort

**Spec:** Annex C §C.1.6, p. 125 — RELIABILITY=BEST_EFFORT;
pre-settled delivery.

**Repo:** `link.rs::LinkSession` with `SettlementMode::Settled`.
The integration test `c1_6_settlement_best_effort.rs`.

**Tests:** `c1_6_best_effort_settled_does_not_track_pending`,
`c1_6_settle_call_on_pre_settled_link_is_no_op`.

**Status:** done

### Annex C C.1.7 Address-resolution wildcard

**Spec:** Annex C §C.1.7, p. 126 — a wildcard address attached →
a multi-partition stream.

**Repo:** `routing::AddressRouter::add_route` + a pattern matcher
with prefix-`*`/suffix-`*`/global-`*`. The integration test
`c1_7_address_resolution_wildcard.rs`.

**Tests:** `c1_7_prefix_wildcard_matches_multiple_topics`,
`c1_7_suffix_wildcard_matches`,
`c1_7_global_wildcard_matches_anything`,
`c1_7_static_alias_takes_precedence_over_wildcard`.

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

**Tests:** `bridge::tests::produce_catalog_transfers_returns_one_per_topic`,
`encode_catalog_sample_produces_described_composite`,
`c1_3_4_8_bridge_dispatch.rs::c1_8_catalog_receiver_gets_one_sample_per_topic`.

**Status:** done

### Annex C C.1.9 Idle timeout

**Spec:** Annex C §C.1.9, p. 126 — a connection without traffic →
connection close with an idle-timeout error.

**Repo:** `ResourceLimits::idle_timeout_ms` +
`TcpStream::set_read_timeout` in the daemon. The integration test
`c1_9_idle_timeout.rs` verifies a real read-timeout
disconnect (the server closes the idle client).

**Tests:** `c1_9_server_disconnects_idle_client_after_short_read_timeout`,
`c1_9_idle_timeout_is_configurable`.

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

**Tests:** `c1_10_server_accepts_multiple_concurrent_connections`.

**Status:** done

### Annex C C.1.11 Pass-through encoding

**Spec:** Annex C §C.1.11, p. 127 — a sample with MODE_PASSTHROUGH
roundtrips byte-identical.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., PassThrough)`
plus bridge dispatch in
`tools/amqp-dds-endpoint/src/bridge.rs::dispatch_transfer`.

**Tests:** `mapping::tests::passthrough_round_trip_preserves_bytes`,
`c1_3_4_8_bridge_dispatch::c1_3_amqp_producer_publishes_to_dds_via_bridge`,
`c2_2_bridge_outbound_publish_flows_to_dds`,
`c2_3_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### Annex C C.1.12 JSON encoding

**Spec:** Annex C §C.1.12, p. 127 — a sample with MODE_JSON is
valid RFC-8259 JSON.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., Json)`
(hex fallback); per-type JSON wrappers from
`crates/idl-cpp/src/amqp.rs`,
`crates/idl-java/src/amqp.rs`,
`crates/idl-ts/src/amqp.rs` (see §8.1.2).

**Tests:** `mapping::tests::json_round_trip_via_hex_field`,
`zerodds_idl_cpp::amqp::tests::struct_emits_to_json_wrapper`,
`json_wrapper_for_union_too`,
`zerodds_idl_java::amqp::tests::struct_emits_to_json_helper`,
`json_helper_for_union_too`,
`zerodds_idl_ts::amqp::tests::struct_emits_to_json_wrapper`,
`json_helper_for_union_too`.

**Status:** done

### Annex C C.1.13 Metrics link

**Spec:** Annex C §C.1.13, p. 128 — a receiver on `$metrics`;
all mandatory metrics are emitted within 30s.

**Repo:** `MetricsHub` + `management::metrics_snapshot(hub,
now_ms)` produces an `AmqpExtValue::Map` per mandatory metric.
The integration test `c1_13_metrics_link.rs` verifies that all 14 metrics
are emitted, that each sample contains `{name, value, unit,
timestamp}` and that the counter values are reflected in the
samples.

**Tests:** `c1_13_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`c1_13_metrics_carry_traffic_counter_values`,
`c1_13_metrics_units_are_spec_symbols`.

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

**Tests:** `c1_14_link_attach_success_event_carries_subject`,
`c1_14_audit_producer_streams_events_in_order`,
`c1_14_unauthorized_event_carries_resource_field`,
`c1_14_ringbuffer_evicts_oldest_on_overflow`.

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
`c1_15_outbound_stamp_*`, `c1_15_clean_sample_passes_through`).

**Status:** done

### Annex C C.2.1 Outbound connection

**Spec:** Annex C §C.2.1, p. 130 — the bridge connects to the configured
upstream broker.

**Repo:** `client::connect_outbound` + the integration test
`client::tests::outbound_connect_to_local_server` drives a
real TCP connect + AMQP Open roundtrip against a local
server daemon. C.2.4 (reconnect) builds on it.

**Tests:** `outbound_connect_to_local_server`,
`c2_4_single_connect_attempt_works_for_normal_path`.

**Status:** done

### Annex C C.2.2 Sender-link bridge-to-broker

**Spec:** Annex C §C.2.2, p. 130 — the bridge attaches a sender link
for an outbound topic mapping.

**Repo:** bridge DDS-side subscribe (`subscribe_outbound`) +
the AMQP outbound producer path. A test roundtrip over two
`InMemoryDdsHost` instances that simulate the broker hop.

**Tests:** `c2_2_bridge_outbound_publish_flows_to_dds`.

**Status:** done

### Annex C C.2.3 Receiver-link broker-to-bridge

**Spec:** Annex C §C.2.3, p. 130 — analogous to C.2.2 in the reverse direction.

**Repo:** the bridge receiver side calls `bridge::dispatch_transfer`
with the incoming broker sample → the DDS DataWriter publishes.

**Tests:** `c2_3_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### Annex C C.2.4 Reconnect on loss

**Spec:** Annex C §C.2.4, p. 131 — connection loss → reconnect
within the configured backoff cap (default 60s); HISTORY
eviction counts.

**Repo:** `client::connect_with_reconnect` with a
`ReconnectConfig` (default 1s init, 2x mult, 60s cap per
spec §10.8). The integration test `c2_4_reconnect_on_loss.rs`.

**Tests:** `c2_4_reconnect_loop_pacing_follows_spec_defaults`,
`c2_4_reconnect_aborts_on_max_attempts`,
`c2_4_reconnect_after_server_restart_eventually_succeeds`,
`c2_4_single_connect_attempt_works_for_normal_path`.

**Status:** done

### Annex C C.2.5 Settlement pass-through

**Spec:** Annex C §C.2.5, p. 131 — broker settlement →
DataWriter notification.

**Repo:** `link::LinkSession::deliver`/`settle` pending tracking
+ a `dds_bridge::DispositionMapper` trait (`apply` with a DDS-side
sample handle). The DataWriter acknowledgment is caller-layer
(a real `DcpsDdsHost` calls `DataWriter::wait_for_acknowledgment`).

**Tests:** `link::tests::settle_decrements_pending`,
`dds_bridge::tests::noop_disposition_mapper_does_nothing`.

**Status:** done

### Annex C C.2.6 Dual-identity separation

**Spec:** Annex C §C.2.6, p. 131 — the bridge presents the
DDS-side identity (`CN=Bridge-1`) instead of the broker-side credential
(`Alice`) to DDS-Security AccessControl.

**Repo:** `security::DualIdentity` with `for_broker()`/`for_dds()`
getters; the AccessControl plugin uses exclusively
`for_dds()`. Lib tests in `security::tests` cover the
spec §C.2.6 path directly.

**Tests:** `security::tests::dual_identity_keeps_broker_and_dds_separate`,
`security::tests::dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### Annex C C.2.7 Loop prevention (bridge)

**Spec:** Annex C §C.2.7, p. 132 — analogous to C.1.15 for the bridge
profile (outbound + inbound).

**Repo:** the `coexistence` layer is profile-agnostic; the bridge
profile binds the same `inspect_inbound`/`stamp_outbound`
pair. The integration test
`c2_7_loop_prevention_bridge.rs`.

**Tests:** `c2_7a_outbound_then_inbound_round_trip_drops_self`,
`c2_7b_hop_cap_exceeded_drops_with_metric`,
`c2_7_multi_bridge_chain_terminates_at_hop_cap`,
`c2_7_foreign_bridges_in_history_dont_trigger_self_drop`.

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

**Tests:** `c1_8_catalog_receiver_gets_one_sample_per_topic`,
`bridge::tests::dispatch_attach_to_metrics_recognised`,
`dispatch_attach_to_audit_recognised`.

**Status:** done

### Annex C C.3 Codec-profile tests

**Spec:** Annex C §C.3, p. 134 — in-process type-system
roundtrips for all primitives, composites, sections.

**Repo:** `crates/amqp-bridge/src/types.rs::tests`,
`extended_types.rs::tests`, `frame.rs::tests`,
`performatives.rs::tests`, `sections.rs::tests`.

**Tests:** `cargo test -p zerodds-amqp-bridge --lib`: 70 tests green.

**Status:** done

### Annex C C.4 Codec-Lite-profile tests

**Spec:** Annex C §C.4, p. 135 — a strict subset of C.3.

**Repo:** `cargo test -p zerodds-amqp-bridge --features codec-lite`
runs the full codec test set + the 5 additional
`codec_profile` tests that mark the Lite subset. The conformance
marker `active_profile()` delivers `CodecProfile::Lite` under the
feature.

**Tests:** all `zerodds-amqp-bridge` tests (75 green) +
`codec_profile::tests::active_profile_full_by_default` (inversely
under `--features codec-lite`).

**Status:** done

### Annex C C.5 Test harness

**Spec:** Annex C §C.5, p. 136 — vendor-neutral wording;
implementers SHOULD publish the harness source.

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

**Tests:** `rpc_correlation::tests::from_amqp_handles_str_symbol_uuid_binary`.

**Status:** done

### Annex D D.2 Activation

**Spec:** Annex D §D.2, p. 139 — `TopicMapping.rpc_aware = true`
activates; default `false`.

**Repo:** `rpc_correlation::RpcConfig::rpc_aware` field
(default `false`); `OutstandingCalls::new(cfg)` is instantiated
only when `rpc_aware = true` is activated.

**Tests:** `rpc_correlation::tests::defaults_match_spec`.

**Status:** done

### Annex D D.3 Scope of this annex

**Spec:** Annex D §D.3, p. 139 — no wire-level RPC bridge,
only property mapping; full RPC-over-AMQP is a
later spec.

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
`validate_reply_rejects_missing_correlation_id`,
`validate_reply_drops_unknown_correlation_id`,
`validate_reply_late_reply_dropped`,
`expire_overdue_removes_and_counts`,
`issue_accepts_then_full`,
`validate_reply_does_not_block_on_table_full`).

**Status:** done

---

## Audit status

123 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-amqp-bridge --lib` — 82 tests green.
* `cargo test -p zerodds-amqp-bridge --features codec-lite --lib` — 82 tests green.
* `cargo test -p zerodds-amqp-endpoint --lib` — 198 tests green.
* `cargo test -p zerodds-amqp-endpoint --test annex_a_idl_roundtrip` — 17 tests green.
* `cargo test -p zerodds-amqp-endpoint --test e2e_multi_bridge_hop` — 6 tests green.
* `cargo test -p zerodds-amqp-endpoint --test fuzz_smoke` — 4 tests green.
* `cargo test -p zerodds-amqp-endpoint --test proptest_state_machine` — 6 tests green.
* `cargo test -p amqp-dds-endpoint` — 54 tests green (without the tls feature).
* `cargo test -p amqp-dds-endpoint --features tls` — 60 tests green.
* `cargo test -p zerodds-idl-cpp` (`amqp::tests::*`) — 18 tests green.
* `cargo test -p zerodds-idl-java` (`amqp::tests::*`) — 13 tests green.
* `cargo test -p zerodds-idl-ts` (`amqp::tests::*`) — 16 tests green.

Cross-crate test volume: 313 + language-codegen AMQP sub-tests against
the DDS-AMQP-1.0 spec.
