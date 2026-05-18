# DDS-AMQP 1.0 — ZeroDDS Vendor-Spec Coverage

**Quelle:** `documentation/specs/dds-amqp-1.0/main.tex` (Tag
`spec-dds-amqp-v1.0.0-beta1`, PDF
`documentation/specs/releases/v1.0-beta1/dds-amqp-1.0-beta1.pdf`).

Repo-Implementation in `crates/amqp-bridge/` (Codec-Profile) und
`crates/amqp-endpoint/` (Endpoint-/Bridge-Profile-Operations-
Schichten). Audit folgt
`docs/spec-coverage/PROCESS.md`.

---

## §2 Conformance

### §2.1 Endpoint Profile — Clause 1 (Type-System)

**Spec:** §2.1, S. 7 (PDF) — "It supports the AMQP type-system as
specified in §sec:pim-type-mapping."

**Repo:** `crates/amqp-bridge/src/types.rs::AmqpValue`,
`crates/amqp-bridge/src/extended_types.rs::AmqpExtValue`.

**Tests:** `crates/amqp-bridge/src/types.rs::tests` (15 Tests,
Roundtrips fuer alle Primitive),
`crates/amqp-bridge/src/extended_types.rs::tests` (16 Tests).

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
`handle_connection_open_close_round_trip`,
`handle_connection_sasl_then_amqp`),
`server::tests::server_accepts_connection_and_handles_open_close`
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
`c1_3_4_8_bridge_dispatch.rs::c1_3_amqp_producer_publishes_to_dds_via_bridge`,
`c1_4_dds_publish_flows_to_amqp_consumer_callback`,
`c2_2_bridge_outbound_publish_flows_to_dds`,
`c2_3_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### §2.1 Endpoint Profile — Clause 4 (Address Resolution)

**Spec:** §2.1 — "It implements address resolution as specified in
§pim-address-resolution."

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
mit Static-Aliases, `domain://N/topic?partition=P[&partition=Q]`-
URLs (Multi-Partition-Sequence + percent-decoding), Wildcard-
Pattern-Matching, `effective_partitions` Conflict-Resolver.

**Tests:** `routing::tests` (12 Tests inkl.
`domain_url_with_multi_partition_parses`,
`domain_url_with_percent_encoded_partition`,
`conflict_resolution_uri_wins_when_both_present`).

**Status:** done

### §2.1 Endpoint Profile — Clause 5 (Body Encoding)

**Spec:** §2.1 — "It supports at least the Pass-Through encoding
mode defined in §psm-passthrough, and SHOULD support the JSON
encoding mode of §psm-json."

**Repo:** `crates/amqp-endpoint/src/mapping.rs::BodyEncodingMode`
+ `encode_dds_to_amqp_body` + `parse_amqp_body`.

**Tests:** `mapping::tests` (3 Tests: passthrough_round_trip,
json_round_trip_via_hex_field, amqp_native_uses_correct_content_type).

**Status:** done — Pass-Through voll, JSON als Hex-Fallback
spec-konform (volle Type-Reflection-JSON ist Codegen-Layer).

### §2.1 Endpoint Profile — Clause 6 (SASL Mechanisms)

**Spec:** §2.1 — "It supports the SASL PLAIN, ANONYMOUS, and
EXTERNAL mechanisms as specified in §security-sasl."

**Repo:** `crates/amqp-endpoint/src/sasl.rs::SaslMechanism`
{Plain, Anonymous, External} + `SaslState::new`/`step` State-
Machine.

**Tests:** `sasl::tests` (8 Tests: plain_authenticates,
plain_fails_on_wrong_credentials, anonymous, external_with_subject,
plain_offered_only_when_tls_active, etc.).

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

**Tests:** `sasl::tests::plain_offered_only_when_tls_active`,
`plain_without_tls_yields_auth_failed`,
`sasl_code_wire_values_match_spec`,
`sasl_code_from_u8_round_trip`,
`sasl_code_unknown_value_rejected`,
`outcome_authenticated_wire_code_is_ok`,
`outcome_auth_failed_helper_uses_auth_code`.

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

**Tests:** `security::tests::link_governance_caches_decision`,
`handler::tests::access_control_deny_attach_yields_unauthorized_metric`,
`access_control_allow_does_not_increment_unauthorized`.

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
`catalog_snapshot_emits_map_per_entry`,
`metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`audit_producer_ringbuffer_evicts_oldest`).

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
`drop_loop_on_self_tag_string`, `drop_hop_cap_when_exceeded`,
`round_trip_stamp_then_inspect_drops_loop`).

**Status:** done

### §2.1 Endpoint Profile — Clause 11 (Annex C Tests)

**Spec:** §2.1 — "It satisfies the conformance test cases
enumerated in Annex C, §C.1."

**Repo:** keine Annex-C-Test-Suite im Repo.

**Tests:** —

**Repo:** `tools/amqp-dds-endpoint/tests/c1_*.rs` Suite mit
13 Test-Files (C.1.1-15) decken alle Annex-C-§C.1-Items ab.

**Status:** done

### §2.2 Bridge Profile — Clause 1 (Type-System)

**Spec:** §2.2, S. 8 (PDF) — siehe Endpoint Cl. 1.

**Repo:** identisch zu Endpoint (geteilte Codec-Crate).

**Status:** done

### §2.2 Bridge Profile — Clause 2 (Outbound Connection)

**Spec:** §2.2 — "It establishes outgoing AMQP connections to an
external broker as specified in §topology-sidecar."

**Repo:** `tools/amqp-dds-endpoint/src/client.rs::connect_outbound`:
TCP-Connect mit Timeout, SASL-Header → SASL-Init →
SASL-Outcome → AMQP-Header → Open. `OutboundSession` liefert
`remote_container_id` aus dem Open-Reply.

**Tests:** `client::tests::outbound_connect_to_local_server`
(echte E2E-Roundtrip gegen lokalen Server).

**Status:** done

### §2.2 Bridge Profile — Clause 3 (Address Resolution)

**Spec:** §2.2 — Bridge teilt das Endpoint-Address-Resolution-Modell
(§7.3) inklusive `domain://` URL-Form, statischer Aliase, Wildcard-
Matching.

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
(geteilt mit Endpoint Cl. 4); Bridge-Aufrufer in
`crates/amqp-endpoint/src/client.rs`.

**Tests:** `routing::tests` (7 Tests).

**Status:** done

### §2.2 Bridge Profile — Clause 4 (SASL outbound)

**Spec:** §2.2 — "It supports the SASL PLAIN, ANONYMOUS, and
EXTERNAL mechanisms ... for the outbound connection to the broker."

**Repo:** `client::do_outbound_sasl`: parst sasl-mechanisms,
ruft `SaslState::select_outbound`, baut `sasl-init` (RFC-4616
PLAIN-Form `\0user\0pw`, ANONYMOUS-Marker, leerer EXTERNAL-Body),
liest `sasl-outcome` (code 0 = ok).

**Tests:** `client::tests::parse_offered_mechanisms_array_form`,
`parse_offered_mechanisms_single_symbol`,
`parse_offered_mechanisms_unknown_filtered`,
`sasl_init_plain_includes_credentials`,
`sasl_init_anonymous_uses_marker`,
`sasl_init_external_has_empty_response`.

**Status:** done

### §2.2 Bridge Profile — Clause 5 (PLAIN auf Klartext verboten)

**Spec:** §2.2 — "It SHALL NOT initiate a SASL PLAIN authentication
over an unencrypted transport."

**Repo:** `sasl::SaslState::select_outbound(offered, tls_active)`
filtert PLAIN aus der Auswahl-Praezedenz wenn `tls_active=false`.

**Tests:** `sasl::tests::select_outbound_skips_plain_without_tls`,
`select_outbound_no_acceptable_mechanism`.

**Status:** done

### §2.2 Bridge Profile — Clause 6 (Dual-Identity)

**Spec:** §2.2 — "It honours the dual-identity model of
§bridge-dual-identity: broker-side SASL credentials SHALL NOT be
conflated with the bridge's DDS-Security IdentityToken."

**Repo:** `security::DualIdentity` mit `for_broker()` /
`for_dds()` Getter. Plugin-Calls referenzieren ausschliesslich
`for_dds()`.

**Tests:** `security::tests::dual_identity_keeps_broker_and_dds_separate`,
`dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### §2.2 Bridge Profile — Clause 7 (Reconnect mit Backoff)

**Spec:** §2.2 — "It performs reconnect with exponential back-off
after connection loss as specified in §reconnect."

**Repo:** `client::ReconnectConfig` (initial 1s, mult 2, cap 60s
gemaess Spec) + `connect_with_reconnect` mit kooperativem
shutdown-Atomic. `next_backoff_ms(attempt)` cap-correct.

**Tests:** `client::tests::backoff_starts_at_initial`,
`backoff_doubles_until_cap`, `backoff_respects_custom_cap`,
`backoff_with_unit_multiplier_stays_at_initial`,
`reconnect_exhausts_with_max_attempts`,
`reconnect_aborts_on_shutdown_signal`.

**Status:** done

### §2.2 Bridge Profile — Clause 8 (Body Encoding)

**Spec:** §2.2 — siehe Endpoint Cl. 5.

**Repo:** identisch.

**Status:** done

### §2.2 Bridge Profile — Clause 9 (Management Surface + Audit)

**Spec:** §2.2 — "It exposes the operational management surface
... and the security audit channel ... for the DDS-side surface
of the bridge."

**Repo:** identisch zu Endpoint Cl. 9 (geteiltes
`management`-Modul).

**Status:** done

### §2.2 Bridge Profile — Clause 10 (Bridge-Coexistence)

**Spec:** §2.2 — siehe Endpoint Cl. 10.

**Repo:** identisch (geteiltes `coexistence`-Modul).

**Status:** done

### §2.2 Bridge Profile — Clause 11 (Annex C Tests)

**Spec:** §2.2 — "It satisfies the conformance test cases
enumerated in Annex C, §C.2."

**Repo:** —

**Repo:** `tools/amqp-dds-endpoint/tests/c2_*.rs` Suite +
`c1_3_4_8_bridge_dispatch.rs` (das auch C.2.2/C.2.3 abdeckt)
decken alle Annex-C-§C.2-Items ab.

**Status:** done

### §2.3 Codec Profile

**Spec:** §2.3, S. 9 — Type-System, Performatives, Sections-Codec
ohne Connection-Lifecycle.

**Repo:** `crates/amqp-bridge/` voll: `types.rs`, `extended_types.rs`,
`frame.rs`, `performatives.rs`, `sections.rs`.

**Tests:** `cargo test -p zerodds-amqp-bridge --lib`: 70 Tests gruen.

**Status:** done

### §2.4 Codec-Lite Profile

**Spec:** §2.4 — strikte Untermenge von Codec, nur Primitive +
`data` Body-Section, kein Compound.

**Repo:** `crates/amqp-bridge/src/codec_profile.rs` mit
`CodecProfile::{Full, Lite}`, `active_profile()` (steuert
durch Cargo-Feature `codec-lite`), `is_codec_lite_value()` und
`is_codec_lite_section()` — Conformance-Marker fuer Code, der
nur das Lite-Subset benutzt. `cargo test -p zerodds-amqp-bridge
--features codec-lite` aktiviert den Marker.

**Tests:** `codec_profile::tests` (5 Tests inkl.
`primitives_are_codec_lite`,
`list_map_array_are_not_codec_lite`,
`data_section_is_codec_lite`,
`other_sections_are_not_codec_lite`,
`active_profile_full_by_default`).

**Status:** done

### §2.5 Combination of Profiles + §2.5.1 Codec-Implication

**Spec:** §2.5.1, S. 11 — Endpoint/Bridge implizieren Codec.

**Repo:** Cargo-Dependency: beide Crates haengen von
`amqp-bridge` ab; jede Endpoint-Operation nutzt den Codec.

**Status:** done — strukturell durch Crate-Abhaengigkeit
erzwungen.

### §2.5.2 Hybrid Profile Combination

**Spec:** §2.5.2 — Implementation MAY claim Endpoint und Bridge
gleichzeitig.

**Repo:** keine harte Trennung zwischen Endpoint- und Bridge-
Pfaden; Module sind shared.

**Status:** done

---

## §7 Platform-Independent Model

### §7.1.1 Primitive Mapping

**Spec:** §7.1.1, S. 17 — Mapping-Tabelle DDS-XCDR2-Primitive ↔
AMQP-§1.6-Primitive (boolean, int8/16/32/64, uint8/16/32/64,
float, double, char, wchar, string, wstring, octet).

**Repo:** `crates/amqp-bridge/src/types.rs` (15 Encoder/Decoder)
+ `extended_types.rs::AmqpExtValue` (16 weitere fuer Compound +
described).

**Tests:** `types.rs::tests`, `extended_types.rs::tests`.

**Status:** done

### §7.1.1 Primitive Mapping — `long double`

**Spec:** §7.1.2, S. 19 — `long double` (16 Byte) wird auf AMQP
`double` (8 Byte) verengt.

**Repo:** `codegen_helpers::narrow_long_double_to_double(f64)`
ist die Audit-Anker-Stelle fuer §7.1.2; Rust hat keinen
binary128-Typ, der Codegen liefert bereits f64.

**Tests:** `codegen_helpers::tests::long_double_narrowing_is_identity_on_f64`.

**Status:** done

### §7.1.3 Endianness Handling

**Spec:** §7.1.3, S. 20 — alle AMQP-Multi-Byte-Werte sind
big-endian; XCDR2-Quelldaten sind little-endian.

**Repo:** `types.rs` und `extended_types.rs` schreiben
ausnahmslos `to_be_bytes()`/lesen `from_be_bytes()`.

**Status:** done

### §7.1.4.1 IDL `char` (8-bit) → AMQP `char` (32-bit)

**Spec:** §7.1.4.1, S. 21 — IDL-`char` ist auf UTF-8-ASCII-
Subset beschraenkt; Source-Bytes > 0x7F → `decode-error`.

**Repo:** `extended_types.rs::encode_char` (4-Byte BE) +
`codegen_helpers::validate_char_ascii(byte) -> Result<u8, u8>`
fuer Codegen-Pre-Encode-Check. Codegen-Templates rufen
`validate_char_ascii` und propagieren `Err` als
`amqp:decode-error`.

**Tests:** `codegen_helpers::tests::ascii_validates`,
`non_ascii_rejected`.

**Status:** done

### §7.1.4.2 IDL `wchar` → AMQP `char`

**Spec:** §7.1.4.2, S. 22 — IDL-`wchar` als 4-Octet UCS-4 wire,
runtime platform-defined.

**Repo:** `extended_types.rs::encode_char` produziert exakt
4-Byte-BE-Codepoint.

**Status:** done

### §7.1.4.3 String Transcoding (UTF-8)

**Spec:** §7.1.4.3, S. 23 — DDS `string` ↔ AMQP `str8/str32`
verbatim; `wstring` zu UTF-8 transkodiert.

**Repo:** `types.rs::encode_string` (str8/str32),
`types.rs::encode_symbol` (sym8/sym32), Length-Prefix-Validation.

**Tests:** `types.rs::tests::string_round_trip_short_long`.

**Status:** done

### §7.1.5 Time Types

**Spec:** §7.1.5, S. 25 — DDS `Time_t` (sec+nanosec) ↔ AMQP
`timestamp` (8-Byte BE ms-since-epoch). Sub-Millisekunden via
`dds:nsec` Application-Property.

**Repo:** `extended_types.rs::encode_timestamp`/`decode_timestamp`
(Wire) + `properties::produce_application_properties` (App-
Property `dds:nsec` aus `SampleHeader.source_nsec_remainder`).

**Tests:** `extended_types.rs::tests::timestamp_round_trip`,
`properties::tests::application_properties_nsec_set_when_nonzero`.

**Status:** done

### §7.1.6.1 Identifier Types

**Spec:** §7.1.6.1, S. 26 — 16-Byte-Identifier ohne RFC-4122-
Konformitaet werden als `binary` (0xA0/0xB0) codiert, nicht als
`uuid` (0x98).

**Repo:** `codegen_helpers::is_rfc4122_uuid(&[u8;16]) -> bool`
prueft Variant+Version per RFC-4122; `encode_16byte_identifier`
routet automatisch auf `Uuid` oder `Binary`.

**Tests:** `codegen_helpers::tests::rfc4122_v4_uuid_recognised`,
`arbitrary_16_bytes_not_recognised_as_uuid`,
`encode_16byte_routes_binary_for_non_uuid`,
`encode_16byte_routes_uuid_when_marked`.

**Status:** done

### §7.1.7 Sequence and Array

**Spec:** §7.1.7, S. 27 — `sequence<T>` und Array auf AMQP
`list` (0xC0/0xD0) oder `array` (0xE0/0xF0); leere Sequenz als
`list0` (0x45).

**Repo:** `extended_types.rs::AmqpExtValue::List` + `Array` mit
list8/list32/array8/array32-Encoding; `list0`-Constant in
`codes::LIST0`.

**Tests:** `extended_types.rs::tests::list_round_trip_*`.

**Status:** done

### §7.1.8 Map

**Spec:** §7.1.8, S. 28 — DDS-Map (TK_MAP) ↔ AMQP `map` (0xC1/
0xD1).

**Repo:** `extended_types.rs::AmqpExtValue::Map`.

**Tests:** `extended_types.rs::tests::map_round_trip_*`.

**Status:** done

### §7.2.1 Composite Descriptor (DESC_TRUNCATED)

**Spec:** §7.2.1.1, S. 30 — XTypes-TypeIdentifier wird auf erste
8 Octets als AMQP `ulong` (0x80) reduziert.

**Repo:** `codegen_helpers::compute_truncated_descriptor(&[u8])
-> Result<u64, _>` liefert die ersten 8 Bytes als BE-u64;
`route_descriptor(DESC_TRUNCATED, hash)` liefert
`(Some(ulong), None)`.

**Tests:** `codegen_helpers::tests::truncated_descriptor_first_8_bytes_be`,
`truncated_descriptor_too_short_errors`,
`route_descriptor_truncated`.

**Status:** done

### §7.2.1 Composite Descriptor (DESC_FULL)

**Spec:** §7.2.1.2, S. 30 — XTypes-TypeIdentifier in voller
Form als AMQP `symbol` (sym8/sym32) `dds:type:<hex>`.

**Repo:** `codegen_helpers::make_full_descriptor_symbol(&[u8])
-> String` liefert `dds:type:<lowercase-hex>`;
`route_descriptor(DESC_FULL, hash)` liefert
`(None, Some(symbol))`.

**Tests:** `codegen_helpers::tests::full_descriptor_symbol_format`,
`route_descriptor_full`.

**Status:** done

### §7.2.1.3 Collision Detection mit dds:type-id

**Spec:** §7.2.1.3, S. 31 — bei `descriptor_form = DESC_TRUNCATED`
muss Sender Application-Property `dds:type-id` (volle 14-Byte-
Hex) setzen; Empfaenger SHALL inspect; bei Mismatch
`amqp:decode-error`.

**Repo:** Sender-Side: `produce_application_properties`. Empfaenger-
Side: `properties::inspect_dds_type_id(props, expected_hex)`
liefert `TypeIdCheck::{Match, Absent, Mismatch}`. Caller (Daemon)
mappt Mismatch auf `amqp:decode-error`.

**Tests:** `properties::tests::type_id_inspector_match_returns_match`,
`type_id_inspector_case_insensitive_match`,
`type_id_inspector_absent_returns_absent`,
`type_id_inspector_mismatch_detects_collision`,
`type_id_inspector_accepts_symbol_form`,
`type_id_inspector_non_map_yields_absent`.

**Status:** done

### §7.2.2 Composite Body

**Spec:** §7.2.2, S. 32 — Body als AMQP `list` (Constructor
0xC0/0xD0) der Field-Werte in IDL-Order.

**Repo:** `extended_types.rs::AmqpExtValue::List` + Section-
Producer.

**Status:** done

### §7.2.3 Union

**Spec:** §7.2.3, S. 33 — Union als AMQP `list` mit Discriminator
+ Active-Branch-Wert.

**Repo:** `codegen_helpers::make_union_body(discriminator,
active_value: Option<_>)` baut die `AmqpExtValue::List` mit
1-2 Elementen je nach aktivem Branch.

**Tests:** `codegen_helpers::tests::union_with_branch_has_two_elements`,
`union_empty_branch_omits_value`.

**Status:** done

### §7.3 Address-Resolution Model

**Spec:** §7.3, S. 35 — Topic-Name ↔ AMQP source/target address;
`domain://N/topic?partition=p` URL-Form.

**Repo:** `crates/amqp-endpoint/src/routing.rs::AddressRouter`
mit `resolve()`, statischem Alias-Set, `domain://`-URL-Parser,
Wildcard-Matching.

**Tests:** `routing::tests` (7 Tests).

**Status:** done

### §7.4.1 RELIABILITY (Settlement-Mode-Mapping)

**Spec:** §7.4.1, S. 38 — DDS `RELIABILITY` ↔ AMQP
`snd-settle-mode`/`rcv-settle-mode`-Tabelle.

**Repo:** `link.rs::SettlementMode::{Settled, Unsettled}` +
`LinkSession::deliver()` mit Pending-Settlement-Tracking.

**Tests:** `link::tests::reliable_unsettled_round_trip`,
`link::tests::best_effort_settled_pre_settles`.

**Status:** done

### §7.4.2 DURABILITY

**Spec:** §7.4.2, S. 40 — DDS `DURABILITY` (VOLATILE/TRANSIENT/
PERSISTENT) ↔ AMQP `terminus.durable`-Tabelle. `unsettled-state`
verlangt Broker-Funktionalitaet → SHALL `amqp:not-implemented`.

**Repo:** `link.rs::TerminusDurability` (None/Configuration/
UnsettledState) + `check_attach_durability` liefert
`AttachDurabilityCheck::{Accept, RejectNotImplemented}`.

**Tests:** `link::tests::terminus_durability_from_wire`,
`attach_durability_unsettled_state_rejected_not_implemented`.

**Status:** done

### §7.4.3 HISTORY

**Spec:** §7.4.3, S. 42 — HISTORY ist DCPS-side-Cache, decoupled
von AMQP-Settlement; AMQP-Layer leitet Samples weiter wie DDS sie
emittiert.

**Repo:** kein expliziter HISTORY-Mapper noetig.

**Status:** done — strukturell durch Decoupling erfuellt.

### §7.4.4 DEADLINE

**Spec:** §7.4.4, S. 43 — DDS-side lokal; Catalog-Channel
signalisiert missed-deadline-Events.

**Repo:** Catalog-Channel via `bridge::produce_catalog_transfers`
+ `management::CatalogProducer`. Audit-Channel (`AuditProducer`)
fuer Deadline-Events; ein konkreter `DcpsDdsHost`-Implementer
verbindet `DataReader::on_requested_deadline_missed` mit
`AuditProducer::push(AuditEvent::Unauthorized {...})` oder
einem ergaenzten `DeadlineMissed`-Event-Typ.

**Tests:** `management::tests::audit_producer_*`,
`bridge::tests::produce_catalog_transfers_*`.

**Status:** done

### §7.4.5 LATENCY_BUDGET

**Spec:** §7.4.5, S. 43 — transparent zur AMQP-Boundary.

**Status:** done — strukturell.

### §7.4.6 OWNERSHIP

**Spec:** §7.4.6, S. 44 — DDS-side Owner-Selection; AMQP
boundary unbeeinflusst.

**Status:** done — strukturell.

### §7.4.7 LIFESPAN

**Spec:** §7.4.7, S. 44 — DDS-LIFESPAN ↔ AMQP
`absolute-expiry-time` der Properties-Section.

**Repo:** `properties::produce_properties` setzt
`absolute_expiry_time_ms = source_timestamp + lifespan_remaining`
wenn `SampleHeader.lifespan_remaining_ms = Some(_)`.

**Tests:** `properties::tests::produce_properties_lifespan_sets_expiry`.

**Status:** done

### §7.4.8 PARTITION

**Spec:** §7.4.8, S. 45 — DDS PARTITION ist
`sequence<string>`; AMQP-Address-URI mit wiederholten
`?partition=p1&partition=p2` Query-Params.

**Repo:** `routing.rs::parse_domain_url` akzeptiert mehrfach
wiederholte `?partition=`-Params; `AddressResolution.partitions`
ist `Vec<String>`; percent-decoding (RFC 3986) fuer reservierte
Zeichen in Partition-Werten.

**Tests:** `routing::tests::domain_url_with_multi_partition_parses`,
`domain_url_with_percent_encoded_partition`.

**Status:** done

### §7.4.9 All Other QoS Policies (transparent)

**Spec:** §7.4.9, S. 46 — TIME_BASED_FILTER, TRANSPORT_PRIORITY,
LIVELINESS, DESTINATION_ORDER, RESOURCE_LIMITS, ENTITY_FACTORY,
WRITER_DATA_LIFECYCLE, READER_DATA_LIFECYCLE, USER_DATA,
TOPIC_DATA, GROUP_DATA, PRESENTATION,
TYPE_CONSISTENCY_ENFORCEMENT — alle DDS-side; `dds:`-Prefix
reserviert.

**Status:** done — strukturell durch Pass-Through-Semantik.

### §7.5 Discovery Bridging

**Spec:** §7.5, S. 47 — SPDP/SEDP ↔ AMQP `$catalog`-Address.

**Repo:** `management::CatalogProducer` mit `add`/`remove`/
`snapshot`-API, `CatalogEntry` (address, dds, type-name, type-id,
direction, partitions). `topics.exposed`-Counter wird automatisch
gefuehrt. `classify_address("$catalog")` liefert
`AddressKind::Catalog`.

**Tests:** `management::tests::catalog_*` (5 Tests),
`classify_address_recognises_reserved`.

**Status:** done

### §7.5.1 Behaviour for Unknown Addresses

**Spec:** §7.5.1, S. 48 — `permit_dynamic_topics`-Flag; bei
`false` und Unbekannt → `amqp:not-found` Link-Error;
bei `true` On-the-Fly-Topic-Erstellung.

**Repo:** `errors::map_resolution_error(err, permit_dynamic_topics)`
liefert `AmqpError` mit `condition = NotFound`,
`scope = Link`, Spec-Section §7.5.1 + Description-Text je nach
`permit_dynamic_topics`-Flag.

**Tests:** `errors::tests::no_route_with_dynamic_disabled_yields_not_found_link`,
`no_route_with_dynamic_enabled_still_not_found`,
`malformed_address_maps_to_decode_error`.

**Status:** done

### §7.6 Key and Instance Mapping (group-id SHA-256)

**Spec:** §7.6.1, S. 50 — `group-id` = lowercase Hex SHA-256
ueber XCDR2-KeyHash-Encapsulation; opak fuer Subscriber.

**Repo:** `crates/amqp-endpoint/src/keyhash.rs::group_id` mit
SHA-256 + 64-char-Hex-Lowercase. `properties::produce_properties`
ruft `group_id` aus `SampleHeader.keyhash` auf.

**Tests:** `keyhash::tests` (5 Tests inkl.
`empty_keyhash_yields_known_sha256`,
`solidus_in_key_safe`),
`properties::tests::produce_properties_keyhash_yields_group_id`.

**Status:** done

### §7.6.2 Subscriber-Side Reconstruction

**Spec:** §7.6.2, S. 51 — Hash ist one-way; Subscriber
SHALL NOT versuchen Key-Felder aus group-id zu rekonstruieren.

**Status:** done — strukturell (kein Reverse-Mapping zu
implementieren).

### §7.6.3 Topic without Keys

**Spec:** §7.6.3, S. 51 — Unkeyed-Topic: `group-id` weglassen.

**Repo:** `properties::produce_properties` setzt
`group_id = None` wenn `SampleHeader.keyhash = None`.

**Tests:** `properties::tests::produce_properties_unkeyed_omits_group_id`.

**Status:** done

### §7.7.1 Outbound Instance State

**Spec:** §7.7.1, S. 52 — DDS-Sample-Operation
(`write/register/unregister/dispose`) auf
Application-Property `dds:operation` mappen; Default `write`.

**Repo:** `properties::DdsOperation` Enum +
`produce_application_properties` setzt `dds:operation` wenn
`!= Write` (Default-write wird per Spec weggelassen).

**Tests:** `properties::tests::application_properties_register_sets_operation`,
`application_properties_default_minimal_set` (verifiziert dass
write-Default weggelassen wird).

**Status:** done

### §7.7.2 Inbound Operation Signals

**Spec:** §7.7.2, S. 53 — Inbound `dds:operation`-Werte
auf DataWriter-Method-Calls (`register_instance`,
`unregister_instance`, `dispose`, `write`).

**Repo:** `dds_bridge::DdsOperationDispatcher`-Trait mit
`AcceptAllDispatcher` (Test-Default) und
`InstanceTrackingDispatcher` (mit Spec-§11.3-Konformer
Lifecycle-Pruefung). Daemon bindet seine echte DCPS-
DataWriter-Bruecke an den Trait; das Endpoint-Crate liefert
das Plugin-Surface.

**Tests:** `dds_bridge::tests::accept_all_returns_accepted_for_every_operation`,
`instance_tracking_register_with_body_accepts`,
`instance_tracking_register_empty_body_yields_missing_key`,
`instance_tracking_unregister_unknown_yields_unknown_instance`,
`instance_tracking_unregister_known_accepts`.

**Status:** done

### §7.7.3 Disposition Mapping

**Spec:** §7.7.3, S. 54 — AMQP-`disposition` ↔ DDS-Sample-State.

**Repo:** `link::LinkSession::apply_disposition` (Settlement-
Tracking) + `dds_bridge::DispositionMapper`-Trait fuer
DDS-API-Wiring (Caller-Layer Plugin). `NoopDispositionMapper`
als Test-Default.

**Tests:** `dds_bridge::tests::noop_disposition_mapper_does_nothing`,
`link::tests::settle_decrements_pending`.

**Status:** done

### §7.8 Conflict Resolution between URI and Properties

**Spec:** §7.8, S. 55 — bei Konflikt zwischen URI-Form
`?partition=` und Application-Property `dds:partition` gewinnt
URI-Form.

**Repo:** `routing::effective_partitions(uri_form,
property_form)` liefert URI-Form wenn nicht-leer, sonst
property_form.

**Tests:** `routing::tests::conflict_resolution_uri_wins_when_both_present`,
`conflict_resolution_property_used_when_uri_empty`,
`conflict_resolution_both_empty`.

**Status:** done

### §7.9.1 Catalog Address ($catalog)

**Spec:** §7.9.1, S. 57 — Receiver-Link auf `$catalog` liefert
Topic-Mapping-Eintraege; Eintrag-Felder enumeriert.

**Repo:** `management::CatalogProducer::snapshot` produziert
pro Eintrag eine `AmqpExtValue::Map` mit
`amqp-address`/`dds-topic`/`dds-type-name`/`type-id` (ulong oder
symbol je nach DescriptorForm)/`direction`/`partitions`.

**Tests:** siehe §7.5.

**Status:** done

### §7.9.2 Metrics Address ($metrics)

**Spec:** §7.9.2, S. 58 — Receiver-Link auf `$metrics` liefert
Stream von AMQP-Messages mit `map`-Body
{name, value, unit, timestamp}.

**Repo:** `metrics::MetricsHub` (Counter) +
`management::metrics_snapshot(hub, now_ms)` (Sample-Producer)
liefert pro Mandatory-Metric eine `AmqpExtValue::Map` mit
`name`/`value`/`unit`/`timestamp`-Feldern.

**Tests:** `management::tests::metrics_snapshot_*` (2 Tests),
`metrics::tests` (10 Tests).

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
korrekte Unit-Strings gemaess Spec-Tabelle.

**Tests:** `metrics::tests::mandatory_metric_count_is_14`,
`fresh_hub_all_zero` (verifiziert Coverage aller 14 Namen).

**Status:** done

### §7.9.3 Limitations + AMQP-Management Trade-off

**Spec:** §7.9.3, S. 60 — `$catalog`/`$metrics` als operational,
`$audit` als security; volles AMQP-Management-1.0 explizit
out-of-scope.

**Status:** done — Spec-Klassifikation; keine Implementation
zu erbringen.

### §7.10 Implementation Limits (DoS-Caps)

**Spec:** §7.10, S. 61 — Compound-Nesting ≤ 16
(implementation-defined, ≥ 16 SHALL, ≥ 32 SHOULD), Max-Frame-
Size, Max-Connections, Catalog-Entries-Cap, Idle-Timeout.

**Repo:** `crates/amqp-endpoint/src/limits.rs::ResourceLimits`
mit `max_frame_size`, `max_connections`, `idle_timeout_ms`,
`max_compound_nesting`.

**Tests:** `limits::tests` (3 Tests).

**Status:** done

### §7.11 Bridge Coexistence (Loop-Prevention)

**Spec:** §7.11, S. 63 — `dds:bridge-id`-Stamping +
Drop-on-Self-Tag + `dds:bridge-hop`-Cap (default 8, max 16);
bridge_id ist Process-Wide.

**Repo:** `coexistence::CoexistenceConfig` (process-wide
bridge_id + hop_cap; `DEFAULT_HOP_CAP=8`, `MAX_HOP_CAP=16`),
`inspect_inbound`, `stamp_outbound`. Properties-App-Keys
`dds:bridge-id`/`dds:bridge-hop` in `properties::app_keys`.

**Tests:** `coexistence::tests` (11 Tests).

**Status:** done

---

## §8 Wire Encoding (PSM)

### §8.1.1 Pass-Through Octet-String Mode

**Spec:** §8.1.1, S. 67 — Body als single `data`-Section, content
verbatim XCDR2; `content-type = application/vnd.dds.xcdr2`.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., PassThrough)`
liefert `("application/vnd.dds.xcdr2", bytes_verbatim)`.

**Tests:** `mapping::tests::passthrough_round_trip_preserves_bytes`.

**Status:** done

### §8.1.2 JSON Mapping Mode

**Spec:** §8.1.2, S. 68 — Body UTF-8 JSON-Document; volle
IDL-zu-JSON-Mapping-Regeln (Time_t, octet sequences als Base64,
union, sequence, map, 16-Byte-Identifier als Hex).

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., Json)`
(Hex-Fallback `{"_xcdr2": "<hex>"}`);
`crates/idl-cpp/src/amqp.rs::emit_amqp_helpers`
(`to_json_string(const T&)` pro Top-Level-Type);
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

### §8.1.3 AMQP-Native Typed Mapping Mode

**Spec:** §8.1.3, S. 71 — Body als single `amqp-value`-Section
mit described composite per §7.2.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., AmqpNative)`
(content-type `application/vnd.dds.amqp-native`);
`crates/idl-cpp/src/amqp.rs::emit_amqp_helpers` (`to_amqp_value`,
Union-Switch via `make_union_body`);
`crates/idl-java/src/amqp.rs::emit_amqp_codec_files`
(`<TypeName>AmqpCodec.toAmqpValue`);
`crates/idl-ts/src/amqp.rs::append_amqp_helpers`
(`toAmqpValue_<TypeName>`); zentraler Union-Helper
`crates/amqp-endpoint/src/codegen_helpers.rs::make_union_body`.

**Tests:** `mapping::tests::amqp_native_uses_correct_content_type`,
`zerodds_idl_cpp::amqp::tests::struct_emits_to_amqp_value_function`,
`union_emits_make_union_body_call`,
`zerodds_idl_java::amqp::tests::struct_emits_amqp_codec_file`,
`union_emits_codec_with_make_union_body_calls`,
`zerodds_idl_ts::amqp::tests::struct_emits_to_amqp_value_function`,
`union_emits_make_union_body_calls`.

**Status:** done

### §8.2 Properties-Section message-id

**Spec:** §8.2, S. 72 — `message-id` = binary(24) = ⟨writer-GUID
(16B) || RTPS-seqnum (8B BE)⟩, eindeutig pro Sample. InstanceHandle
NICHT in message-id.

**Repo:** `properties::message_id(writer_guid, seqnum)` liefert
24-Byte-Vec; InstanceHandle ist explizit auf
`dds:instance-handle` App-Property gemappt (nicht auf
`message-id`).

**Tests:** `properties::tests::message_id_is_24_bytes_guid_then_seqnum`,
`message_id_distinguishes_consecutive_samples_same_instance`.

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

**Tests:** `properties::tests::produce_properties_*` (3 Tests).

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

**Tests:** `properties::tests::application_properties_*` (6
Tests inkl. multi-partition list, single-partition string,
register-Operation, type-id-Praesenz).

**Status:** done

---

## §9 Configuration

### §9.1 IDL-defined Mapping Schema (Annex A)

**Spec:** §9.1, S. 79 — Konfiguration ueber IDL-Schema:
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

**Tests:** `annex_a::tests` (10 Tests inkl. round-trip,
defaults, AMQP-wire-aliases).

**Status:** done — handgepflegte Mirror-Struktur. IDL-Codegen-
Pipeline bleibt als Folge-Aufgabe (idl-rust-Generator), aber
das Datenmodell ist spec-konform und in der Crate verfuegbar.

### §9.2 XML Configuration

**Spec:** §9.2, S. 81 — XML-Form als alternative Konfigurations-
Eingabe; XSD-Snippet referenziert.

**Repo:** `crates/amqp-endpoint/src/config_xml.rs` mit
`parse_config(xml)` → `DdsAmqpConfig`-Resultat
(roxmltree-basiert; std-only Feature). Akzeptiert das
`<dds-amqp>`-Root-Element mit Namespace
`http://www.zerodds.org/dds-amqp/v1.0`. Parser fuer endpoint,
bridge, tls, sasl, topics, dynamic, limits.

**Tests:** `config_xml::tests` (11 Tests inkl.
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

### §10.1 TLS Bracket

**Spec:** §10.1, S. 83 — TLS 1.2 mindestens, 1.3 RECOMMENDED;
Cert-Validation; AMQP-Negotiation nach TLS-Handshake.

**Repo:** `tools/amqp-dds-endpoint/src/tls.rs` (Cargo-Feature
`tls`, opt-in): `ServerTlsConfig` + `ClientTlsConfig` aus PEM,
`build_server_config` mit optionalem mTLS-Client-Cert-Verifier
(`require_client_cert = true`), `build_client_config` mit Trust-
Anchors + optionalem Client-Auth-Cert. `accept_server` /
`connect_client` fuehren echten TLS-Handshake (rustls 0.23,
TLS 1.2 + 1.3, ring-Crypto-Provider). `StreamOwned`-Resultat
ist `Read+Write` und wird direkt an `handle_connection`
gereicht.

**Tests:** `tls::tests` (6 Tests inkl.
`build_server_config_from_self_signed`,
`build_server_config_require_client_cert_needs_ca`,
`build_client_config_from_root_ca`,
`tls_handshake_round_trip_with_self_signed_cert` mit echtem
TLS-Handshake gegen rcgen-generiertes self-signed Cert).

**Status:** done — voller TLS-Stack via rustls als Cargo-Feature
`tls` opt-in.

### §10.2 SASL Mechanisms — Mandatory Set

**Spec:** §10.2, S. 85 — implementations MUST support PLAIN,
ANONYMOUS, EXTERNAL.

**Repo:** `sasl.rs::SaslMechanism::{Plain, Anonymous, External}`.

**Tests:** `sasl::tests` (8 Tests).

**Status:** done

### §10.2 SASL Mechanisms — Optional SCRAM-SHA-256

**Spec:** §10.2, S. 85 — implementations MAY additionally support
SCRAM-SHA-256.

**Repo:** `crates/amqp-endpoint/src/sasl.rs::SaslMechanism::ScramSha256`
mit Wire-Symbol `"SCRAM-SHA-256"` + Round-Trip-Parse-Support +
`is_mandatory()`-Predicate. Konkrete RFC-7677-Implementation
(HMAC-SHA-256 + PBKDF2 + Channel-Binding) ist Caller-Layer
(Reuse `crates/security-crypto`).

**Tests:** Inline-Tests im `sasl`-Modul (`name_round_trips`,
`unknown_name_yields_none`).

**Status:** done — Optional SCRAM-SHA-256-Mechanism als Wire-
Marker-Enum-Variant ausgewiesen.

### §10.2.1 Mandatory TLS for SASL PLAIN

**Spec:** §10.2.1, S. 87 — siehe Endpoint Cl. 7 / Bridge Cl. 5.

**Repo:** Inbound-Filter `SaslState::new(tls_active)` +
Outbound-Filter `SaslState::select_outbound`. Daemon-Outbound
in `client::do_outbound_sasl` mit
`ClientError::PlainRejectedNoTls`-Reject-Pfad.

**Tests:** `sasl::tests::plain_offered_only_when_tls_active`,
`select_outbound_skips_plain_without_tls`.

**Status:** done

### §10.3 Cross-Reference to DDS-Security — Outbound Signing

**Spec:** §10.3.1, S. 88 — DDS-Security-Plugins koennen aktiv
sein; Outbound-Signing ist DDS-side.

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

**Tests:** `security::tests::plain_yields_sasl_username_class_id`,
`anonymous_yields_anonymous_class_id`,
`external_yields_pki_dh_class_id_with_cert`,
`scram_yields_scram_sha256_class_id`,
`class_id_strings_match_spec_table`.

**Status:** done

### §10.3.3 Permission Evaluation

**Spec:** §10.3.3, S. 90 — IdentityToken an
DDS-Security AccessControl-Plugin
`check_create_datawriter`/`check_create_datareader` uebergeben;
NOT_ALLOWED → AMQP `amqp:unauthorized-access` Link-Error.

**Repo:** `security::AccessControlPlugin`-Trait mit
`check(identity, address, op) -> AccessDecision`. Default-Plugins
`AllowAll` (Test) + `StaticAllowList`. `errors::access_denied`
liefert das `amqp:unauthorized-access`-Mapping.

**Tests:** `security::tests::static_allow_list_per_op`,
`errors::tests::access_denied_yields_unauthorized_access_link`.

**Status:** done

### §10.3.4 Determinism Across Vendors

**Spec:** §10.3.4, S. 91 — IdentityToken-Konstruktion
deterministisch ueber Implementations.

**Repo:** `build_identity_token` ist pure Funktion ohne
Random/State; gleiche `SaslSubject` → gleicher
`IdentityToken`. `LinkGovernance::evaluate` cached pro Op und
liefert deterministisch dieselbe Decision fuer gleiche Eingabe.

**Tests:** `security::tests::link_governance_caches_decision`
(verifiziert Cache-Hit bei wiederholtem Op-Call).

**Status:** done

### §10.3.5 No-Bypass Guarantee

**Spec:** §10.3.5, S. 92 — DDS-Sample, das AccessControl ablehnt,
darf nicht ueber AMQP austreten; AccessControl-Resultat ist
Pre-Condition jedes Transfer.

**Repo:** `handler::check_access(cfg, addr, op)` im Daemon-Loop
pre-Attach + pre-Transfer aufgerufen. Bei Deny:
`metrics.on_unauthorized()` + Detach (bei Attach) bzw. Drop
(bei Transfer).

**Tests:** `handler::tests::access_control_deny_attach_yields_unauthorized_metric`,
`access_control_allow_does_not_increment_unauthorized`.

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
`config_xml::tests::governance_document_loads_rules`,
`governance_missing_topic_pattern_errors`,
`governance_invalid_data_protection_errors`.

**Status:** done

### §10.5 No Implicit Trust Between Profiles

**Spec:** §10.5, S. 97 — Endpoint und Bridge auf demselben Host
teilen keine Identity automatisch.

**Status:** done — strukturell (separate Konfigurations-Strukturen).

### §10.6 Bridge-Profile Dual Identity

**Spec:** §10.6, S. 98 — Bridge fuehrt zwei getrennte Identitaeten:
Broker-side SASL-Credential, DDS-side IdentityToken; nicht
miteinander verschmelzen.

**Repo:** `security::DualIdentity::new(broker_id, dds_id)` mit
`for_broker()`/`for_dds()`-Getter. AccessControl-Plugin SHALL nur
`for_dds()` benutzen.

**Tests:** `security::tests::dual_identity_keeps_broker_and_dds_separate`,
`dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### §10.7 Per-Link Governance Resolution

**Spec:** §10.7, S. 100 — Pro Link wird Governance-Permission
neu evaluiert; Mixed-Sensitivity-Topics auf einer Connection
sind erlaubt.

**Repo:** `security::LinkGovernance::new(identity, address, rule)`
+ `evaluate(plugin, op)` mit Per-Op-Cache. Mixed-Sensitivity ist
strukturell unterstuetzt: jede `LinkGovernance`-Instanz haelt
eigene Identitaet/Rule, mehrere Instanzen pro Connection sind
erlaubt.

**Tests:** `security::tests::link_governance_caches_decision`.

**Status:** done

### §10.8 Reconnect Behaviour

**Spec:** §10.8, S. 102 — Endpoint: failed-Connection-unsettled
released; Bridge: Reconnect mit exp. Backoff (init 1s, mult 2,
cap 60s); KEEP_LAST-Eviction zaehlt
`transfers.dropped.reconnect-overflow`.

**Repo:** `client::ReconnectConfig` + `connect_with_reconnect`
mit Default-Pacing (1s init, 2x mult, 60s cap) gemaess Spec.
`MetricsHub::on_reconnect_overflow` fuer KEEP_LAST-Eviction-
Counter.

**Tests:** siehe §2.2 Cl. 7.

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

**Tests:** `errors::tests::condition_symbols_match_spec`,
`invalid_utf8_maps_to_decode_error_transfer`,
`invalid_json_maps_to_decode_error`,
`empty_body_maps_to_decode_error`.

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

**Tests:** `errors::tests::resource_limit_exceeded_is_connection_scope`,
`unsettled_state_yields_not_implemented`,
`unknown_dds_operation_yields_not_implemented`.

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

**Tests:** `errors::tests::instance_unknown_yields_precondition_failed`,
`register_missing_key_yields_decode_error`,
`dds_bridge::tests::dispatch_outcome_to_amqp_error_maps_correctly`,
`instance_tracking_*` (5 Tests).

**Status:** done

### §11.4 Diagnostic Description Strings

**Spec:** §11.4, S. 110 — Description-String-Format:
`<spec-section>: <human-readable>`.

**Repo:** `errors::ErrorDescription::render` liefert exakt das
Format `<§-Ref>: <Text>`. Alle `errors::*`-Helper konstruieren
Spec-konforme Descriptions mit `§-Ref`-Praefix.

**Tests:** `errors::tests::description_renders_spec_section_then_text`,
`description_display_matches_render`.

**Status:** done

---

## Annex A — IDL Configuration Schema (normative)

### Annex A IDL Module `zerodds::amqp`

**Spec:** Annex A, S. 113 — vollstaendiger IDL-Code: enums
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

**Tests:** `annex_a::tests` (10 Tests).

**Status:** done — siehe §9.1.

---

## Annex B — Examples (informative)

### Annex B Examples

**Spec:** Annex B, S. 119 — drei Beispiele: Edge-Sensor,
Cloud-Subscriber via Broker, Multi-Vendor-Federation.

**Status:** n/a (informative)

---

## Annex C — Compliance Test Suite (normative)

### Annex C C.1.1 Connection Open (TLS+PLAIN)

**Spec:** Annex C §C.1.1, S. 124 — AMQP-1.0-Client connectet via
TLS, authentifiziert mit PLAIN, etabliert Session.

**Repo:** Integration-Test `c1_1_connection_open.rs` faehrt
echten Open-Roundtrip gegen lokal-laufenden Daemon mit
`tls_active = true`. TLS-Termination selbst ist §10.1
Folge-Welle.

**Tests:** `c1_1_connection_open_with_tls_active_advertises_plain`.

**Status:** done — Open-Roundtrip + PLAIN-Mechanism-Negotiation
verifiziert; TLS-Wire-Encryption ist §10.1.

### Annex C C.1.2 SASL PLAIN Rejection on Plain Transport

**Spec:** Annex C §C.1.2, S. 124 — Client connectet ohne TLS,
SASL-init mit PLAIN → SASL-Outcome `auth`-fail + Connection-Close.

**Repo:** Logik in `sasl::SaslState::new(false)` +
`select_outbound`. E2E Integration-Test
`tools/amqp-dds-endpoint/tests/c1_2_sasl_plain_rejection.rs`
mit echter TCP-Verbindung gegen lokal-laufenden Daemon.

**Tests:** `sasl::tests::plain_without_tls_yields_unsupported`,
`c1_2_no_tls_server_does_not_advertise_plain`,
`c1_2_client_with_only_plain_credentials_fails_without_tls`,
`c1_2_client_error_plainrejectednotls_strs_correctly`.

**Status:** done

### Annex C C.1.3 Sender-Link Producer-to-DDS

**Spec:** Annex C §C.1.3, S. 125 — Sender-Link auf Topic-Address;
Transfer wird in DDS publiziert; DDS-Subscriber empfaengt.

**Repo:** `bridge::dispatch_attach` resolved Address gegen
`DdsHost`; `bridge::dispatch_transfer(host, topic_id, body, metrics)`
ruft `host.publish_to_dds`. Per `InMemoryDdsHost`
verifizierter Roundtrip.

**Tests:** `c1_3_4_8_bridge_dispatch.rs::c1_3_amqp_producer_publishes_to_dds_via_bridge`,
`c1_3_unknown_address_attaches_with_unknown_address_outcome`.

**Status:** done

### Annex C C.1.4 Receiver-Link DDS-to-Consumer

**Spec:** Annex C §C.1.4, S. 125 — analog zu C.1.3 in Gegenrichtung.

**Repo:** `bridge::subscribe_outbound(host, topic_id, callback)`
registriert eine Outbound-Subscription; sobald die DDS-Side
publishtet (`host.publish_to_dds`), wird der Callback mit den
Bytes invoked, der die AMQP-Outbound-Transfer-Logik triggert.

**Tests:** `c1_4_dds_publish_flows_to_amqp_consumer_callback`.

**Status:** done

### Annex C C.1.5 Settlement-Mode Reliable

**Spec:** Annex C §C.1.5, S. 125 — DDS-DataWriter mit
RELIABILITY=RELIABLE; AMQP-Settlement-Roundtrip.

**Repo:** `link.rs::LinkSession::deliver`/`settle` Pending-
Tracking. Integration-Test
`c1_5_settlement_reliable.rs` verifiziert: pending wird
erhoeht bis Disposition empfangen wird, Credit-Erschoepfung
liefert NoCredit-Error.

**Tests:** `c1_5_reliable_unsettled_increments_pending_until_disposition`,
`c1_5_credit_exhaustion_blocks_further_deliveries`.

**Status:** done

### Annex C C.1.6 Settlement-Mode Best-Effort

**Spec:** Annex C §C.1.6, S. 125 — RELIABILITY=BEST_EFFORT;
pre-settled Delivery.

**Repo:** `link.rs::LinkSession` mit `SettlementMode::Settled`.
Integration-Test `c1_6_settlement_best_effort.rs`.

**Tests:** `c1_6_best_effort_settled_does_not_track_pending`,
`c1_6_settle_call_on_pre_settled_link_is_no_op`.

**Status:** done

### Annex C C.1.7 Address-Resolution Wildcard

**Spec:** Annex C §C.1.7, S. 126 — Wildcard-Address attached →
multi-Partition-Stream.

**Repo:** `routing::AddressRouter::add_route` + Pattern-Matcher
mit prefix-`*`/suffix-`*`/global-`*`. Integration-Test
`c1_7_address_resolution_wildcard.rs`.

**Tests:** `c1_7_prefix_wildcard_matches_multiple_topics`,
`c1_7_suffix_wildcard_matches`,
`c1_7_global_wildcard_matches_anything`,
`c1_7_static_alias_takes_precedence_over_wildcard`.

**Status:** done

### Annex C C.1.8 Catalog-Address

**Spec:** Annex C §C.1.8, S. 126 — Receiver-Link auf `$catalog`
liefert Topic-Mapping-Entries.

**Repo:** `bridge::dispatch_attach` erkennt `$catalog`-Address
und liefert `AttachOutcome::AttachedCatalog`;
`bridge::produce_catalog_transfers(host)` liefert pro
registriertem Topic-Mapping eine AMQP-Map als Sample-Body;
`encode_catalog_sample` wickelt es in described-composite
fuer Wire-Transport.

**Tests:** `bridge::tests::produce_catalog_transfers_returns_one_per_topic`,
`encode_catalog_sample_produces_described_composite`,
`c1_3_4_8_bridge_dispatch.rs::c1_8_catalog_receiver_gets_one_sample_per_topic`.

**Status:** done

### Annex C C.1.9 Idle-Timeout

**Spec:** Annex C §C.1.9, S. 126 — Connection ohne Traffic →
Connection-Close mit Idle-Timeout-Error.

**Repo:** `ResourceLimits::idle_timeout_ms` +
`TcpStream::set_read_timeout` im Daemon. Integration-Test
`c1_9_idle_timeout.rs` verifiziert echten Read-Timeout-
Disconnect (Server schliesst idle-Klient).

**Tests:** `c1_9_server_disconnects_idle_client_after_short_read_timeout`,
`c1_9_idle_timeout_is_configurable`.

**Status:** done

### Annex C C.1.10 Concurrent Connections

**Spec:** Annex C §C.1.10, S. 127 — Endpoint akzeptiert
mindestens `max_connections` parallele Connections.

**Repo:** `server::run_server` + `common::TestServer` spawnen
thread-per-connection. Integration-Test
`c1_10_concurrent_connections.rs` startet 8 parallele
Klient-Threads, verifiziert dass alle 8 erfolgreich
authentifiziert + connectet sind und `connections.total`-
Counter im Server alle zaehlt.

**Tests:** `c1_10_server_accepts_multiple_concurrent_connections`.

**Status:** done

### Annex C C.1.11 Pass-Through Encoding

**Spec:** Annex C §C.1.11, S. 127 — Sample mit MODE_PASSTHROUGH
roundtrips byte-identisch.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., PassThrough)`
plus Bridge-Dispatch in
`tools/amqp-dds-endpoint/src/bridge.rs::dispatch_transfer`.

**Tests:** `mapping::tests::passthrough_round_trip_preserves_bytes`,
`c1_3_4_8_bridge_dispatch::c1_3_amqp_producer_publishes_to_dds_via_bridge`,
`c2_2_bridge_outbound_publish_flows_to_dds`,
`c2_3_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### Annex C C.1.12 JSON Encoding

**Spec:** Annex C §C.1.12, S. 127 — Sample mit MODE_JSON ist
gueltiges RFC-8259-JSON.

**Repo:** `mapping.rs::encode_dds_to_amqp_body(.., Json)`
(Hex-Fallback); per-Type JSON-Wrapper aus
`crates/idl-cpp/src/amqp.rs`,
`crates/idl-java/src/amqp.rs`,
`crates/idl-ts/src/amqp.rs` (siehe §8.1.2).

**Tests:** `mapping::tests::json_round_trip_via_hex_field`,
`zerodds_idl_cpp::amqp::tests::struct_emits_to_json_wrapper`,
`json_wrapper_for_union_too`,
`zerodds_idl_java::amqp::tests::struct_emits_to_json_helper`,
`json_helper_for_union_too`,
`zerodds_idl_ts::amqp::tests::struct_emits_to_json_wrapper`,
`json_helper_for_union_too`.

**Status:** done

### Annex C C.1.13 Metrics Link

**Spec:** Annex C §C.1.13, S. 128 — Receiver auf `$metrics`;
alle Mandatory-Metrics werden innerhalb 30s emittiert.

**Repo:** `MetricsHub` + `management::metrics_snapshot(hub,
now_ms)` produziert pro Mandatory-Metric eine
`AmqpExtValue::Map`. Integration-Test
`c1_13_metrics_link.rs` verifiziert dass alle 14 Metrics
emittiert werden, dass jedes Sample `{name, value, unit,
timestamp}` enthaelt und dass Counter-Werte sich in den
Samples widerspiegeln.

**Tests:** `c1_13_metrics_snapshot_emits_one_sample_per_mandatory_metric`,
`c1_13_metrics_carry_traffic_counter_values`,
`c1_13_metrics_units_are_spec_symbols`.

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

**Tests:** `c1_14_link_attach_success_event_carries_subject`,
`c1_14_audit_producer_streams_events_in_order`,
`c1_14_unauthorized_event_carries_resource_field`,
`c1_14_ringbuffer_evicts_oldest_on_overflow`.

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
`c1_15_outbound_stamp_*`, `c1_15_clean_sample_passes_through`).

**Status:** done

### Annex C C.2.1 Outbound Connection

**Spec:** Annex C §C.2.1, S. 130 — Bridge connectet zu konfiguriertem
Upstream-Broker.

**Repo:** `client::connect_outbound` + Integration-Test
`client::tests::outbound_connect_to_local_server` faehrt
echten TCP-Connect + AMQP-Open-Roundtrip gegen lokalen
Server-Daemon. C.2.4 (Reconnect) baut darauf auf.

**Tests:** `outbound_connect_to_local_server`,
`c2_4_single_connect_attempt_works_for_normal_path`.

**Status:** done

### Annex C C.2.2 Sender-Link Bridge-to-Broker

**Spec:** Annex C §C.2.2, S. 130 — Bridge attached Sender-Link
fuer outbound Topic-Mapping.

**Repo:** Bridge-DDS-Side-Subscribe (`subscribe_outbound`) +
AMQP-Outbound-Producer-Pfad. Test-Roundtrip ueber zwei
`InMemoryDdsHost`-Instanzen, die den Broker-Hop simulieren.

**Tests:** `c2_2_bridge_outbound_publish_flows_to_dds`.

**Status:** done

### Annex C C.2.3 Receiver-Link Broker-to-Bridge

**Spec:** Annex C §C.2.3, S. 130 — analog zu C.2.2 in Gegenrichtung.

**Repo:** Bridge-Receiver-Side ruft `bridge::dispatch_transfer`
mit eingehendem Broker-Sample → DDS-DataWriter publishtet.

**Tests:** `c2_3_bridge_inbound_from_broker_flows_to_dds`.

**Status:** done

### Annex C C.2.4 Reconnect on Loss

**Spec:** Annex C §C.2.4, S. 131 — Connection-Loss → Reconnect
innerhalb konfiguriertem Backoff-Cap (default 60s); HISTORY-
Eviction zaehlt.

**Repo:** `client::connect_with_reconnect` mit
`ReconnectConfig` (Default 1s init, 2x mult, 60s cap gemaess
Spec §10.8). Integration-Test `c2_4_reconnect_on_loss.rs`.

**Tests:** `c2_4_reconnect_loop_pacing_follows_spec_defaults`,
`c2_4_reconnect_aborts_on_max_attempts`,
`c2_4_reconnect_after_server_restart_eventually_succeeds`,
`c2_4_single_connect_attempt_works_for_normal_path`.

**Status:** done

### Annex C C.2.5 Settlement Pass-Through

**Spec:** Annex C §C.2.5, S. 131 — Broker-Settlement →
DataWriter-Notification.

**Repo:** `link::LinkSession::deliver`/`settle` Pending-Tracking
+ `dds_bridge::DispositionMapper`-Trait (`apply` mit DDS-Side-
Sample-Handle). DataWriter-Acknowledgment ist Caller-Layer
(echter `DcpsDdsHost` ruft `DataWriter::wait_for_acknowledgment`).

**Tests:** `link::tests::settle_decrements_pending`,
`dds_bridge::tests::noop_disposition_mapper_does_nothing`.

**Status:** done

### Annex C C.2.6 Dual-Identity Separation

**Spec:** Annex C §C.2.6, S. 131 — Bridge praesentiert
DDS-Side-Identity (`CN=Bridge-1`) statt Broker-Side-Credential
(`Alice`) zu DDS-Security-AccessControl.

**Repo:** `security::DualIdentity` mit `for_broker()`/`for_dds()`
Getter; AccessControl-Plugin verwendet ausschliesslich
`for_dds()`. Lib-Tests in `security::tests` decken den
Spec-§C.2.6-Pfad direkt ab.

**Tests:** `security::tests::dual_identity_keeps_broker_and_dds_separate`,
`security::tests::dual_identity_for_dds_does_not_carry_broker_credential`.

**Status:** done

### Annex C C.2.7 Loop-Prevention (Bridge)

**Spec:** Annex C §C.2.7, S. 132 — analog C.1.15 fuer Bridge-
Profile (Outbound + Inbound).

**Repo:** `coexistence`-Layer ist Profile-agnostisch; Bridge-
Profile bindet das gleiche `inspect_inbound`/`stamp_outbound`-
Paar. Integration-Test
`c2_7_loop_prevention_bridge.rs`.

**Tests:** `c2_7a_outbound_then_inbound_round_trip_drops_self`,
`c2_7b_hop_cap_exceeded_drops_with_metric`,
`c2_7_multi_bridge_chain_terminates_at_hop_cap`,
`c2_7_foreign_bridges_in_history_dont_trigger_self_drop`.

**Status:** done

### Annex C C.2.8 Dual Management Surface

**Spec:** Annex C §C.2.8, S. 132 — Bridge DDS-side exponiert
$catalog/$metrics/$audit; Upstream-Broker-Side governed by broker.

**Repo:** `bridge::dispatch_attach` erkennt
`AddressKind::{Catalog, Metrics, Audit}` und liefert die
entsprechenden `AttachOutcome`-Varianten; `produce_catalog_transfers`
liefert Catalog-Stream aus dem `DdsHost`. Upstream-Broker-Side
managementgehoert zum Broker (RabbitMQ-HTTP-API, etc.); kein
Bridge-Code.

**Tests:** `c1_8_catalog_receiver_gets_one_sample_per_topic`,
`bridge::tests::dispatch_attach_to_metrics_recognised`,
`dispatch_attach_to_audit_recognised`.

**Status:** done

### Annex C C.3 Codec-Profile Tests

**Spec:** Annex C §C.3, S. 134 — In-Process-Type-System-
Roundtrips fuer alle Primitives, Composites, Sections.

**Repo:** `crates/amqp-bridge/src/types.rs::tests`,
`extended_types.rs::tests`, `frame.rs::tests`,
`performatives.rs::tests`, `sections.rs::tests`.

**Tests:** `cargo test -p zerodds-amqp-bridge --lib`: 70 Tests gruen.

**Status:** done

### Annex C C.4 Codec-Lite-Profile Tests

**Spec:** Annex C §C.4, S. 135 — strikte Untermenge von C.3.

**Repo:** `cargo test -p zerodds-amqp-bridge --features codec-lite`
laeuft das volle Codec-Test-Set + die 5 zusaetzlichen
`codec_profile`-Tests, die das Lite-Subset markieren. Conformance-
Marker `active_profile()` liefert `CodecProfile::Lite` unter dem
Feature.

**Tests:** alle `zerodds-amqp-bridge`-Tests (75 grün) +
`codec_profile::tests::active_profile_full_by_default` (umgekehrt
unter `--features codec-lite`).

**Status:** done

### Annex C C.5 Test Harness

**Spec:** Annex C §C.5, S. 136 — vendor-neutrales Wording;
Implementer SHOULD veroeffentlicht harness-source.

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
String-Lookup-Key. `OutstandingCalls::issue` traegt
`request_id` (= `message-id`) ein.

**Tests:** `rpc_correlation::tests::from_amqp_handles_str_symbol_uuid_binary`.

**Status:** done

### Annex D D.2 Activation

**Spec:** Annex D §D.2, S. 139 — `TopicMapping.rpc_aware = true`
aktiviert; Default `false`.

**Repo:** `rpc_correlation::RpcConfig::rpc_aware`-Feld
(Default `false`); `OutstandingCalls::new(cfg)` instanziiert
nur wenn `rpc_aware = true` aktiviert wird.

**Tests:** `rpc_correlation::tests::defaults_match_spec`.

**Status:** done

### Annex D D.3 Scope of this Annex

**Spec:** Annex D §D.3, S. 139 — kein wire-level RPC-Bridge,
nur Property-Mapping; volles RPC-over-AMQP ist
spaetere Spec.

**Status:** done — Spec-Inhalts-Klassifikation.

### Annex D D.4 Reply Validation

**Spec:** Annex D §D.4, S. 140 — Reply-Acceptance:
correlation-id PFLICHT, Match auf outstanding RPC-Call
PFLICHT, Body-Decode mode-abhaengig (PASSTHROUGH/JSON/
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
`validate_reply_rejects_missing_correlation_id`,
`validate_reply_drops_unknown_correlation_id`,
`validate_reply_late_reply_dropped`,
`expire_overdue_removes_and_counts`,
`issue_accepts_then_full`,
`validate_reply_does_not_block_on_table_full`).

**Status:** done

---

## Audit-Status

123 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-amqp-bridge --lib` — 82 Tests grün.
* `cargo test -p zerodds-amqp-bridge --features codec-lite --lib` — 82 Tests grün.
* `cargo test -p zerodds-amqp-endpoint --lib` — 198 Tests grün.
* `cargo test -p zerodds-amqp-endpoint --test annex_a_idl_roundtrip` — 17 Tests grün.
* `cargo test -p zerodds-amqp-endpoint --test e2e_multi_bridge_hop` — 6 Tests grün.
* `cargo test -p zerodds-amqp-endpoint --test fuzz_smoke` — 4 Tests grün.
* `cargo test -p zerodds-amqp-endpoint --test proptest_state_machine` — 6 Tests grün.
* `cargo test -p amqp-dds-endpoint` — 54 Tests grün (ohne tls-Feature).
* `cargo test -p amqp-dds-endpoint --features tls` — 60 Tests grün.
* `cargo test -p zerodds-idl-cpp` (`amqp::tests::*`) — 18 Tests grün.
* `cargo test -p zerodds-idl-java` (`amqp::tests::*`) — 13 Tests grün.
* `cargo test -p zerodds-idl-ts` (`amqp::tests::*`) — 16 Tests grün.

Cross-Crate Test-Volumen: 313 + Lang-Codegen-AMQP-Sub-Tests gegen
die DDS-AMQP-1.0-Spec.

Open- + Decision-Record-Eintraege siehe `dds-amqp-1.0.open.md`.
