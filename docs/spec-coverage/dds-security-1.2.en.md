# DDS-Security 1.2 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-security-1.2.pdf` (351 pages, OMG formal/25-03-06) —
[OMG DDS-Security 1.2](https://www.omg.org/spec/DDS-SECURITY/1.2/)

Item-by-item audit against the PDF according to
`internal/spec-coverage/PROCESS.md`. Page numbers are PDF pages (PDF page =
printed page + 16). Every operation of the plugin interfaces with its return
values, every token, every property, every step of the handshake, every check
of the builtin plugins, every element and every rule of the governance and
permissions documents, every submessage, every parameter ID and every builtin
endpoint is an item of its own. `done` stands only where code and test cover
the requirement; a test citation names the test function as
`file::module::name`. A repository citation names file, symbol and line (l.);
the lines refer to the state with which this file was checked in. Every
statement about behaviour is read from the code; no test was run and nothing
was built for this audit.

**Context:** DDS-Security is spread over these crates in ZeroDDS:

- `crates/security/` — the plugin interfaces as Rust traits
  (`AuthenticationPlugin`, `AccessControlPlugin`, `CryptographicPlugin`,
  `LoggingPlugin`, `DataTaggingPlugin`), `DataHolder` and the tokens,
  `ParticipantGenericMessage`, `PropertyList`.
- `crates/security-pki/` — DDS:Auth:PKI-DH and DDS:Auth:PSK.
- `crates/security-permissions/` — DDS:Access:Permissions and
  DDS:Access:PSK: governance and permissions documents, CMS verification.
- `crates/security-crypto/` — DDS:Crypto:AES-GCM-GMAC and DDS:Crypto:PSK.
- `crates/security-keyexchange/` — key agreement ECDH P-256 and X25519.
- `crates/security-logging/` — DDS:Logging:DDS_LogTopic and further sinks.
- `crates/security-rtps/` — a second codec for SEC_PREFIX/SEC_BODY/SEC_POSTFIX
  and SRTPS_PREFIX/SRTPS_POSTFIX.
- `crates/security-runtime/` — connection of the plugins with the runtime:
  `SharedSecurityGate`, `SecurityProfile`, evaluation of the
  `PropertyQosPolicy`, builtin DataTagging.
- `crates/rtps/` — parameter IDs, EntityIds and wire types of the discovery
  extensions.
- `crates/discovery/` — the endpoints DCPSParticipantStatelessMessage and
  DCPSParticipantVolatileMessageSecure, secure SPDP/SEDP endpoints.
- `crates/dcps/` — integration into the runtime (`runtime.rs`,
  `participant.rs`).

The Rust API is not a mapping of the IDL of §9 and §11: operations return
`SecurityResult<T>` instead of a return value with `SecurityException`;
handles are newtypes over `u64`. Where an operation, a parameter or a return
value of the specification is missing from the Rust API, that is listed as an
item of its own.

**Open items:** none; every item is `done` or `n/a (informative)`.

---

## §1 Scope

### §1.1 General

**Spec:** §1.1, p. 17 — "This specification adds several new 'DDS Security
Support' compliance points ('profile') to the DDS Specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference to the conformance points of §2;
these are listed individually.

### §1.2 Overview of this Specification

**Spec:** §1.2, pp. 17–18 — "This specification defines five SPIs that when
combined together provide Information Assurance to DDS systems":
Authentication, AccessControl, Cryptographic, Logging, Data Tagging.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the five interfaces are listed in
§9.3 to §9.7 with every operation as an item of its own.

---

## §2 Conformance

### §2.1 Conformance points

**Spec:** §2.1, p. 19 — "(1) Builtin plugin interoperability (mandatory)
(2) Plugin framework (mandatory) (3) Plugin language APIs (optional)
(4) Logging and Tagging (optional)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment of clauses to conformance points;
whether a point is met follows from the status of the assigned items.

### §2.2 Builtin plugin interoperability (mandatory)

**Spec:** §2.2, p. 19 — "Conformance to this point requires conformance to:
Clause 7 [...] The configuration of the plugins and the observable
wire-protocol behavior specified in Clause 10 [...] except for sub clause
10.6."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment; clauses 7 and 10 are listed
item by item.

### §2.3 Plugin framework (mandatory) — assignment

**Spec:** §2.3, p. 19 — "Conformance to this point requires conformance to:
Clause 7 [...] Clause 9 (the plugin model) with the exception of 9.6 and 9.7
[...] The conformance to the plugin model is at the UML level; it does not
mandate a particular language mapping. [...] Clause 10, the builtin-plugins,
except for 10.6".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment; the clauses are listed item by
item.

### §2.3 Public API to insert the plugins

**Spec:** §2.3, p. 19 — "In addition it requires the conforming DDS
implementation to provide a public API to insert the plugins that conform to
the aforementioned sections."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`
(Authentication);
`crates/security-runtime/src/shared.rs::SharedSecurityGate::new` (Cryptographic)
and `SharedSecurityGate::set_access_control` (AccessControl);
`crates/security-runtime/src/bundle.rs::SecurityBundleBuilder::logging_plugin`
(Logging)

**Tests:**
`crates/dcps/src/runtime.rs::tests::enable_security_builtins_with_auth_activates_handshake_driver`,
`crates/dcps/tests/security_access_plugin_e2e.rs::create_topic_is_asked_of_the_plugin`,
`crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`,
`crates/dcps/tests/security_bundle_e2e.rs::bundle_logger_is_wired_into_runtime_config_and_fires`

**Status:** `done` — Authentication, Cryptographic, AccessControl and Logging
have a public insertion point. The insertion point for AccessControl
(`set_access_control`, sec-access) exists, and the runtime asks the plugin
inserted for its access decisions (test `security_access_plugin_e2e`). §2.3 asks
for clause 9 "with the exception of 9.6 and 9.7": Logging (§9.6) and Data
Tagging (§9.7) are exempt. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-082254.log`,
`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §2.3 No insertion point for AccessControl and DataTagging

**Spec:** §2.3, p. 19 — as above.

**Repo:**
`crates/security-runtime/src/shared.rs::SharedSecurityGate::set_access_control`,
`crates/security-runtime/src/shared.rs::SharedSecurityGate::application_access_control`

**Tests:**
`crates/dcps/tests/security_access_plugin_e2e.rs::instance_operations_are_asked_of_the_plugin`,
`crates/dcps/tests/security_access_plugin_e2e.rs::participant_set_qos_is_asked_of_the_plugin`

**Status:** `done` — a user inserts an AccessControl plugin of their own into a
participant (`SharedSecurityGate::set_access_control`); the gate and the runtime
decide through it. There is no insertion point for DataTagging, the spec does
not ask for one (§2.3 exempts §9.7); the tags of an endpoint come from
`DataWriterQos.data_tags`/`DataReaderQos.data_tags`. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-082254.log`).

### §2.4 Plugin Language APIs (optional)

**Spec:** §2.4, pp. 19–20 — "Conformance with the 'plugin language API' point
requires conformance with at least one of the 5 language APIs enumerated
below: C [...] 11.2 and 11.3, C++ classic [...] 11.4, Java classic [...]
11.5, C++11 [...] 11.6, Java5+ [...] 11.7".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment; §11 is listed item by item.

### §2.5 Logging and Tagging profile (optional)

**Spec:** §2.5, p. 20 — "Conformance to this point requires conformance to sub
clauses 9.6, 9.7, and 10.6."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — assignment; §9.6, §9.7 and §10.6 are listed
item by item.

---

## §3 Normative References

### §3 Normative References

**Spec:** §3, p. 21 — DDS 1.4, DDS-RTPS 2.5, DDS-XTYPES 1.3, OMG-IDL 4.2,
RFC 2104 (HMAC), RFC 6151, RFC 2315 (PKCS #7), RFC 8017 (PKCS #1), XSD 1.1
Part 2.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — list of references; where a referenced
standard carries a requirement, it stands with the item that uses it.

---

## §4 Terms and Definitions

### §4 Terms and Definitions

**Spec:** §4, pp. 22–25 — definitions from "Access Control" to "Public Key
Infrastructure".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary.

---

## §5 Symbols

### §5 Symbols

**Spec:** §5, p. 25 — "This specification does not define any symbols or
abbreviations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no symbols.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, p. 26 — "This specification does not modify any existing
adopted OMG specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — delimitation against DDS, DDS-RTPS,
DDS-XTYPES and IDL.

### §6.1 DDS-XTYPES: Builtin Secure TypeLookup Endpoints

**Spec:** §6.1, p. 26 — "Implementations of both this specification and
DDS-XTYPES (Basic Network Interoperability Profile) shall include the Builtin
Secure TypeLookup Endpoints (see section 7.5.11)."

**Repo:**
`crates/dcps/src/type_lookup_transport.rs::TypeLookupTransport::new_secure`;
`crates/dcps/src/runtime.rs` (`type_lookup_transport_secure`,
`type_lookup_secure_with`, `protect_type_lookup_outbound`);
`crates/rtps/src/wire_types.rs` (`EntityId::TL_SVC_*_SECURE_*`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_secure_type_lookup_service_is_protected_per_submessage`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_its_own_endpoints`,
`crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four secure TypeLookup endpoints exist (`a542c9f545d`,
`7cfdb4fd99a`, `37de123a72e`): own EntityIds, SPDP announcement, protection per
submessage, tokens. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §6.2 Acknowledgments

**Spec:** §6.2, pp. 26–27 — list of contributing persons and companies.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acknowledgments.

---

## §7 Support for DDS Security

### §7.1 Security Model

**Spec:** §7.1, p. 29 — "Securing DDS means providing: Confidentiality of the
data samples, Integrity of the data samples and the messages that contain
them, Authentication of DDS writers and readers, Authorization of DDS writers
and readers, Message-origin authentication, Data-origin authentication,
(Optional) Non-repudiation of data".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — statement of goals; the mechanisms stand in
§7.3 to §10 and are listed item by item there.

### §7.1.1 Threats

**Spec:** §7.1.1, pp. 29–30 — four threat classes (Unauthorized subscription,
Unauthorized publication, Tampering and replay, Unauthorized access to data)
and six actors (Alice, Bob, Eve, Trudy, Mallory, Trent).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — threat model without a requirement.

### §7.1.1.1 Unauthorized Subscription

**Spec:** §7.1.1.1, p. 30 — "All that is required is for Alice to encrypt the
data she writes using a secret key that is only shared with authorized
receivers".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale for encryption; the requirement
stands in §9.5 and §10.5.

### §7.1.1.2 Unauthorized Publication

**Spec:** §7.1.1.2, pp. 30–31 — "All that is required is for the protocol to
require that the messages include either a hash-based message authentication
code (HMAC) or digital signature."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale for authentication; the
requirement stands in §9.5 and §10.5.

### §7.1.1.3 Tampering and Replay

**Spec:** §7.1.1.3, pp. 31–32 — "if Alice wants to send an HMAC with a
different key for every receiver, the only solution is to append multiple
HMACs to the multicast message with some key-id that allows the recipient to
select the correct HMAC to verify."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — rationale for receiver-specific MACs; the
requirement stands in §9.5.3 and §10.5.

### §7.1.1.4 Unauthorized Access to Data by Infrastructure Services

**Spec:** §7.1.1.4, p. 32 — Trent may receive, verify and forward messages
without seeing their content.

**Repo:** `crates/dcps/src/runtime.rs` (`UserReaderConfig::relay`,
`write_user_sample_relayed`, `UserSample::Alive::crypto_content`);
`crates/security-permissions/src/governance.rs` (`check_relay_reader`,
`check_relay_writer`); `crates/security-runtime/src/shared.rs`
(`check_create_relay_datareader`, `remote_may_relay`);
`crates/routing-service/src/config.rs` (`Route::relay`)

**Tests:** `crates/dcps/tests/security_relay_e2e.rs::a_relay_forwards_what_it_cannot_read_and_the_reader_takes_it_as_the_originals`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`,
`crates/routing-service/tests/relay_route.rs::a_relay_route_forwards_on_behalf_of_the_writer`

**Status:** `done` — a relay reader is created on the grant's `<relay>`, gets the
submessage key only and hands the payload on unopened; a writer sends it
unchanged on behalf of the original writer (PID_ORIGINAL_WRITER_INFO), only
the submessage with its own key. The receiver takes the sample as the
original's only from a writer whose grant allows `<relay>`, and opens it with
the original's key; a writer without `<relay>` counts as itself. The routing
service has a relay mode for it. For creating a reader the spec names
`<subscribe>` only (Table 70); the section presupposes the relay reader.
Measured: green on the Linux test host, live test Alice → Trent → Carol three
times in a row.

### §7.2 Cryptographic Algorithm Classes

**Spec:** §7.2, pp. 32–33 — three classes: Digital Signature, Key Establishment
and KeyAgreement, Symmetric Cipher. "The common set of predefined
cryptographic algorithms available for use by the SPIs are defined in Clause
8."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — classification; the algorithms are listed in
§8 item by item.

### §7.3 Types used by DDS Security

**Spec:** §7.3, p. 33 — "The types described in this sub clause are used in
these extensions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §7.3.1 Use of IDL and XTYPES notation

**Spec:** §7.3.1, p. 33 — "the requirement is that the serialized data for
types defined/used in the DDS-Security specification the corresponding
DDS-XTYPES data representations for those same concrete data types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notational convention; the serialization of
each type is listed with the type as an item of its own.

### §7.3.1.1 Type Extensibility

**Spec:** §7.3.1.1, pp. 33–34 — types of discovery topics MUTABLE, types of a
QoS MUTABLE, a QoS policy APPENDABLE, other top-level types APPENDABLE, types
in sequences or in non-mutable types FINAL.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — convention by which the specification
declares its types; the consequence for the wire stands in §7.3.1.2.

### §7.3.1.2 Data Representation: Extended CDR version 1

**Spec:** §7.3.1.2, p. 34 — "DDS-Security only uses the Extended CDR
representation with encoded version 1. Specifically, this means that the
serialization of a type with extensibility kind APPENDABLE is the same as if
it had been declared to have extensibility kind FINAL."

**Repo:** `crates/security/src/token.rs::encode` (l. 357, `DataHolder` without
DHEADER); `crates/security/src/generic_message.rs::ParticipantGenericMessage::to_cdr`
(l. 138, APPENDABLE type without DHEADER)

**Tests:** `crates/security/src/token.rs::tests::cdr_le_layout_class_id_only`,
`crates/security/src/generic_message.rs::tests::message_data_dataholder_is_inline_not_length_prefixed`

**Status:** done — `DataHolder` and `ParticipantGenericMessage` are written as
XCDR1 without DHEADER; the discovery types as a ParameterList.

### §7.3.1.3 Vendor-specific extensions only in discovery types

**Spec:** §7.3.1.3, p. 34 — "Vendors may only create vendor-specific
extensions to the Types representing builtin Topics used for discovery [...]
The member IDs of these vendor-specific members shall be in the
Vendor-specific ParameterId space [...] The Ignore/Must Understand bit [...]
must also be set according with the meaning of table 9.6".

**Repo:** `crates/rtps/src/parameter_list.rs` (constants `pid::*`, own
parameters from `0x8000`); `crates/security-runtime/src/caps_wire.rs` (names
`org.zerodds.sec.*`)

**Tests:**
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`,
`crates/rtps/src/property_list.rs::tests::data_tags_wire_form`

**Status:** `done` — the own parameter ids of ZeroDDS are in the vendor range.
Own entries in `PID_PROPERTY_LIST` carry the vendor prefix `org.zerodds.`
(§7.3.2); the tags of an endpoint go as `PID_DATA_TAGS` (0x1003), no longer as
`dds.sec.data_tags.*` entries (`715589c2094`, `aef84a98a0b`). Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.2 Property_t: name, value, propagate

**Spec:** §7.3.2, pp. 34–35, Table 1 — "The DDS Security specification extends
the DDS-RTPS definition of Property_t to contain the additional boolean
attribute 'propagate'". Attributes `name String`, `value String`,
`propagate Boolean`.

**Repo:** `crates/security/src/properties.rs::Property` (l. 21, fields `name`,
`value`, `propagate`)

**Tests:** `crates/security/src/properties.rs::tests::propagatable_filter`,
`crates/security/src/properties.rs::tests::set_replaces_duplicate`

**Status:** done

### §7.3.2 Prefix "dds.sec." reserved

**Spec:** §7.3.2, p. 35 — "Property_t objects with names that start with the
prefix 'dds.sec.' are reserved by this specification, including future
versions of this specification."

**Repo:** `crates/security-runtime/src/qos_properties.rs`,
`crates/security-logging/src/from_properties.rs`,
`crates/security-permissions/src/plugin.rs`,
`crates/security-pki/src/x509v1_compat.rs`,
`crates/security-runtime/src/caps_wire.rs`

**Tests:**
`crates/security-runtime/tests/property_qos_names.rs::builtin_property_names_match_the_spec_strings`,
`crates/security-pki/src/x509v1_compat.rs::tests::the_property_name_reads_as_a_deviation`

**Status:** `done` — the names of the spec (`dds.sec.auth.*`,
`dds.sec.access.{permissions,permissions_ca,governance}`,
`dds.sec.crypto.{symmetric_cipher_algorithm,rtps_psk_*}`, `dds.sec.log.plugin`,
`dds.sec.log.builtin.DDS_LogTopic.{log_file,distribute}`) stay; every name the
spec does not define is under `org.zerodds.`:
`org.zerodds.sec.{access.psk_governance,access.psk_permissions,access.subject_name,auth.crl,auth.accept_x509v1_identity_certificate,crypto.max_blocks_per_session,crypto.rtps_psk_secret_passphrase_alt,crypto.rtps_psk_secret_passphrase_extra,log.level,log.jsonl.path,log.syslog.*,log.builtin.DDS_LogTopic.{logging_level,legacy_topic}}`,
the plugin classes in `caps_wire.rs` (`7ed5d2621ff`, `aef84a98a0b`). The name
`dds.sec.log.builtin.DDS_LogTopic.logging_level` stays in the bench app for Fast
DDS, which reads it itself. New on the wire and in configurations: the old names
are no longer read. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §7.3.2 Vendor prefix from the reversed ICANN domain name

**Spec:** §7.3.2, p. 35 — "implementers shall use property names that start
with a prefix to an ICANN domain name they own, in reverse order. For example,
the prefix would be 'com.acme.'".

**Repo:** `crates/security-runtime/src/caps_wire.rs`,
`crates/security-runtime/src/qos_properties.rs`, `crates/security/src/crypto.rs`
(`org.zerodds.crypto.*`), `crates/security-crypto/src/psk_rtps.rs`
(`org.zerodds.psk.*`)

**Tests:**
`crates/security-runtime/tests/spdp_caps_e2e.rs::secure_caps_roundtrip_through_spdp_beacon`,
`crates/security-runtime/tests/property_qos_names.rs::builtin_property_names_match_the_spec_strings`

**Status:** `done` — all names of ZeroDDS start with `org.zerodds.` (the domain
zerodds.org, reversed): `org.zerodds.sec.*`, `org.zerodds.crypto.*`,
`org.zerodds.psk.*`. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §7.3.2.1 IDL Representation for Property_t

**Spec:** §7.3.2.1, p. 35 — "@extensibility(FINAL) struct Property_t {
string name; string value; @non-serialized boolean propagate; }; typedef
sequence< Property_t > PropertySeq;"

**Repo:** `crates/rtps/src/property_list.rs::WireProperty` (l. 52, without
`propagate`), `WirePropertyList::encode` (l. 131);
`crates/security/src/token.rs::WireProperty` (l. 53)

**Tests:** `crates/rtps/src/property_list.rs::tests::single_property_roundtrip_le`,
`crates/rtps/src/property_list.rs::tests::encode_pads_strings_to_4_between_name_and_value`,
`crates/rtps/src/property_list.rs::tests::multi_property_roundtrip_be`

**Status:** done — `propagate` is not serialized; name and value are CDR
strings aligned to four.

### §7.3.3 BinaryProperty_t: name, value, propagate

**Spec:** §7.3.3, pp. 35–36, Table 2 — `name String`, `value OctetSeq`,
`propagate Boolean`; "BinaryProperty_t objects with a 'name' attribute that
start with the prefix 'dds.sec.' are reserved".

**Repo:** `crates/security/src/token.rs::BinaryProperty` (`propagate`, not
serialized); `crates/security/src/properties.rs::PropertyList` (binary
entries); `crates/qos/src/policies/property.rs::PropertyQosPolicy`
(`binary_value`, `propagate` per entry)

**Tests:**
`crates/security/src/properties.rs::tests::binary_properties_carry_propagate_and_it_is_not_serialized`,
`crates/qos/src/policies/property.rs::tests::only_propagate_true_entries_are_propagated`,
`crates/rtps/src/property_list.rs::tests::binary_properties_cross_the_wire`

**Status:** `done` — `BinaryProperty` has `propagate` (not serialized); the
local `PropertyList` and the `PropertyQosPolicy` hold binary properties
(`b4c91fb4ed1`, `928d39304cd`, `f6657d231dd`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.3 BinaryProperty_t: propagate and local binary properties

**Spec:** §7.3.3, p. 35 — "BinaryProperty_t also contains the boolean
attribute 'propagate'. Similar to Property_t this attribute is used to
indicate whether the corresponding binary property is intended for local use
only or shall be propagated by DDS discovery."

**Repo:** `crates/security/src/token.rs::BinaryProperty` (`propagate`, not
serialized); `crates/security/src/properties.rs::PropertyList` (binary
entries); `crates/qos/src/policies/property.rs::PropertyQosPolicy`
(`binary_value`, `propagate` per entry)

**Tests:**
`crates/security/src/properties.rs::tests::binary_properties_carry_propagate_and_it_is_not_serialized`,
`crates/qos/src/policies/property.rs::tests::only_propagate_true_entries_are_propagated`,
`crates/rtps/src/property_list.rs::tests::binary_properties_cross_the_wire`

**Status:** `done` — `BinaryProperty` has `propagate` (not serialized); the
local `PropertyList` and the `PropertyQosPolicy` hold binary properties
(`b4c91fb4ed1`, `928d39304cd`, `f6657d231dd`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.3.1 IDL Representation for BinaryProperty_t

**Spec:** §7.3.3.1, p. 36 — "@extensibility(FINAL) struct BinaryProperty_t {
string name; OctetSeq value; @non-serialized boolean propagate; };"

**Repo:** `crates/security/src/token.rs::encode` (l. 357, `encode_string` and
`encode_octet_seq`)

**Tests:** `crates/security/src/token.rs::tests::token_with_binary_property_roundtrip`

**Status:** done — name as CDR string, value as `sequence<octet>`.

### §7.3.3.1 ASCII value with NUL as octet sequence

**Spec:** §7.3.3.1, p. 36 — "When setting the BinaryProperty_t value octet
sequence from an ASCII string, the length of the sequence shall be set to the
number of characters in the string, counting the NUL terminating character
[...] the string 'ECDSA-SHA256' shall result in an octet sequence value with
length 13 where the first octet is 0x45 [...] and the last octet is 0x00."

**Repo:** `crates/security/src/vendor_forms.rs::AlgoNameNul`;
`crates/security-pki/src/handshake_token.rs`;
`crates/discovery/src/security/stack.rs` (`set_algo_name_nul`);
`crates/security-pki/src/plugin.rs` (`der_to_pem`, `c_perm_of`);
`crates/security-permissions/src/plugin.rs` (`validate_remote_permissions`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::the_algorithm_names_follow_the_setting`,
`crates/discovery/src/security/stack.rs::tests::the_reply_has_the_form_of_the_request`,
`crates/security/src/vendor_forms.rs::tests::the_spec_form_has_the_nul_for_every_peer`,
`crates/security-runtime/src/qos_properties.rs::tests::the_algo_name_form_is_the_spec_form_unless_named`,
`crates/security-pki/src/plugin.rs::tests::request_token_has_spec_class_id_and_properties`,
`crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`

**Status:** `done` — the default is the form of the spec: `c.dsign_algo` and
`c.kagree_algo` have 13 and 21 octets, the NUL included (§7.3.3.1), for every
peer. `c.id` (the certificate as PEM) and `c.perm` (the permissions document as
S/MIME text) end with the NUL as well; a binary document goes as it is. On
receipt a trailing NUL is cut off before the certificate and the document are
read. RTI Connext 7.7.0 reported "not properly null terminated" for a `c.id`
without it (OMG dds-security, Linux test host). The forms of other vendors are named settings:
`org.zerodds.sec.algo_name_nul` / `ZERODDS_SEC_ALGO_NAME_NUL` = `omit` (without
the NUL, Fast DDS), `mirror` (as initiator with the NUL, as replier in the form
of the request: a request without the NUL is answered without it, `97a1d7fc001`,
`89b70651dad`) or `by_vendor` (the NUL for OpenDDS only, the former form)
(`e87b0411306`). Measured (Linux test host, sec-diag10, before it became a setting, then
as a behaviour of the replier): `fastdds:zerodds` and `zerodds:fastdds` of
`common-subset` ok on `32f11c44270`; with the default `spec` ZeroDDS answers a
Fast DDS initiator with the NUL, which is not measured and, by the finding of
that time, NO_MATCH. Fast DDS cells need `mirror`; the column with Fast DDS in
`tests/perf/dds-roundtrip-bench/security-matrix/deep_matrix.sh` sets it in the
ZeroDDS process, the other columns run with the defaults. Source (Fast DDS
2.6.10, `src/cpp/security/authentication/PKIDH.cpp`, read; the source of 3.6.1
is not at hand, its behaviour is concluded from the matrix):
`begin_handshake_request` writes `c.dsign_algo` and `c.kagree_algo` with
`assign(sign_alg_.begin(), sign_alg_.end())` from a `std::string` without a NUL
(l. 1257-1270); `process_handshake_request`, the initiator processing the reply,
checks `s_dsign_algo.compare(ECDSA_SHA256)` on a `std::string` made from the
octets of the value (l. 1915-1925), a NUL in the value makes it unequal and the
reply is refused; `begin_handshake_reply` checks the request with `strcmp` (l.
1503) and tolerates a NUL. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.4 DataHolder

**Spec:** §7.3.4, p. 36, Table 3 — `class_id String`, `properties
PropertySeq`, `binary_properties BinaryPropertySeq`.

**Repo:** `crates/security/src/token.rs::DataHolder` (l. 96)

**Tests:** `crates/security/src/token.rs::tests::pki_dh_identity_token_roundtrip`,
`crates/security/src/token.rs::tests::token_with_binary_property_roundtrip`

**Status:** done

### §7.3.4.1 IDL representation for DataHolder

**Spec:** §7.3.4.1, p. 37 — "@extensibility(FINAL) struct DataHolder { string
class_id; PropertySeq properties; BinaryPropertySeq binary_properties; };
typedef sequence<DataHolder> DataHolderSeq;"

**Repo:** `crates/security/src/token.rs::encode` (l. 357),
`decode_consumed` (l. 381)

**Tests:** `crates/security/src/token.rs::tests::cdr_le_layout_class_id_only`,
`crates/security/src/token.rs::tests::empty_data_holder_roundtrip_be`,
`crates/security/src/token.rs::tests::missing_trailing_nul_rejected`,
`crates/security/src/token.rs::tests::truncated_buffer_is_error`

**Status:** done — `DataHolderSeq` appears in `ParticipantGenericMessage`
(§7.3.22).

### §7.3.5 Token

**Spec:** §7.3.5, p. 37 — "The Token class is structurally identical to the
DataHolder class [...] the contents and interpretation of the Token objects
shall be specified by each plugin implementation."

**Repo:** `crates/security/src/token.rs::IdentityToken` (l. 247,
`pub type IdentityToken = DataHolder`)

**Tests:** `crates/security/src/token.rs::tests::pki_dh_identity_token_roundtrip`,
`crates/security/src/token.rs::tests::permissions_token_roundtrip`

**Status:** done

### §7.3.5.1 Attribute: class_id — prefix "dds.sec." reserved

**Spec:** §7.3.5.1, p. 38 — "Strings with the prefix 'dds.sec.' are reserved
for this specification [...] the class_id they use shall start with a prefix
to an ICANN domain name they own".

**Repo:** `crates/security-pki/src/psk.rs::class_id`,
`crates/security-pki/src/psk.rs::SPEC_CLASS_ID`,
`crates/security-permissions/src/psk_access.rs::CLASS_ID_PSK_PERMISSIONS`

**Tests:**
`crates/security-pki/src/psk.rs::tests::the_handshake_classes_are_not_the_spec_class`

**Status:** `done` — the classes of `DDS:Auth:PSK` and `DDS:Access:PSK` are
those of the spec (`DDS:Auth:PSK:1.2`, `DDS:Access:PSK:1.2`, sec-auth and
sec-access); the handshake with PSK that ZeroDDS speaks in addition carries the
classes `org.zerodds.auth.psk-handshake:1.0[+AuthReq|+AuthReply|+AuthFinal]`, a
reversed domain name (`7ed5d2621ff`). The constant
`DDS:Auth:PKI-DH:1.0+IdentityStatus` in
`crates/security-pki/src/identity_status.rs` is unused; the IdentityStatusToken
of the plugin carries the class `DDS:Auth:PKI-DH:1.0` (Table 53). Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §7.3.5.2 Token typedefs

**Spec:** §7.3.5.2, p. 38 — "typedef DataHolder Token; typedef Token
MessageToken; typedef MessageToken AuthRequestMessageToken; typedef
MessageToken HandshakeMessageToken; typedef Token IdentityToken; typedef Token
IdentityStatusToken; typedef Token PermissionsToken; typedef Token
AuthenticatedPeerCredentialToken; typedef Token PermissionsCredentialToken;
typedef Token CryptoToken; typedef Token ParticipantCryptoToken; typedef
Token DatawriterCryptoToken; typedef Token DatareaderCryptoToken;"

**Repo:** `crates/security/src/token.rs` (l. 247–268: `IdentityToken`,
`PermissionsToken`, `IdentityStatusToken`, `PermissionsCredentialToken`,
`AuthRequestMessageToken`, `HandshakeMessageToken`, `CryptoToken` as
`DataHolder`)

**Tests:** `crates/security/src/token.rs::tests::pki_dh_identity_token_roundtrip`,
`crates/security/src/token.rs::tests::permissions_token_roundtrip`

**Status:** done — all tokens are `DataHolder` on the wire. Rust has no alias
for `Token`, `MessageToken`, `AuthenticatedPeerCredentialToken`,
`ParticipantCryptoToken`, `DatawriterCryptoToken` and
`DatareaderCryptoToken`; these values are `DataHolder` directly.

### §7.3.5.2 Token sequences

**Spec:** §7.3.5.2, p. 38 — "typedef sequence<HandshakeMessageToken>
HandshakeMessageTokenSeq; typedef sequence<CryptoToken> CryptoTokenSeq;
typedef CryptoTokenSeq ParticipantCryptoTokenSeq; typedef CryptoTokenSeq
DatawriterCryptoTokenSeq; typedef CryptoTokenSeq DatareaderCryptoTokenSeq;"

**Repo:** `crates/security/src/generic_message.rs::ParticipantGenericMessage`
(l. 97, `message_data: Vec<DataHolder>`)

**Tests:** `crates/security/src/generic_message.rs::tests::message_data_dataholder_is_inline_not_length_prefixed`

**Status:** done — on the wire the sequences appear only as `message_data`;
in Rust they are `Vec<DataHolder>` without aliases of their own.

### §7.3.5.3 TokenNIL

**Spec:** §7.3.5.3, p. 38 — "This name refers to the Token object having
class_id set to the empty string, and both properties and binary_properties
sequences set to the empty sequence. The TokenNIL object is used to indicate
the absence of a Token."

**Repo:** `crates/security/src/token.rs::DataHolder` (l. 96, `Default` yields
an empty `class_id` and empty sequences)

**Tests:** `crates/security/src/token.rs::tests::empty_data_holder_roundtrip_le`

**Status:** done — `DataHolder::default()` is TokenNIL; there is no named
constant. The use of TokenNIL in the plugin operations is listed with the
operations in §9.

### §7.3.6 CryptoAlgorithmName

**Spec:** §7.3.6, pp. 38–39 — "typedef string<64> CryptoAlgorithmName;" — "See
clause 8 for the values".

**Repo:** `crates/security-pki/src/handshake_token.rs` (`MAX_ALGO_NAME`, the
names of Table 25 and 26), `crates/security-pki/src/identity_token.rs`

**Tests:**
`crates/security-pki/src/handshake_token.rs::tests::an_algorithm_name_over_64_characters_is_refused`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_names_the_algorithms_as_table_25_and_26_unless_the_peer_reads_1_1`,
`crates/security-pki/src/identity_token.rs::tests::the_algorithms_are_the_names_of_table_25`

**Status:** `done` — algorithm names are strings of up to 64 characters; ZeroDDS
sends the names of version 1.2 (Tables 25 and 26: `ECDSA+P256+SHA256`, ...) and
those of 1.1 only when the peer reads them (sec-crypto/sec-auth, `8a3e542788d`).
Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §7.3.6 CryptoAlgorithmName: bound 64 not checked

**Spec:** §7.3.6, p. 38 — "typedef string<64> CryptoAlgorithmName;"

**Repo:** `crates/security-pki/src/handshake_token.rs::take_bin_string`
(`MAX_ALGO_NAME` = 64)

**Tests:**
`crates/security-pki/src/handshake_token.rs::tests::an_algorithm_name_over_64_characters_is_refused`

**Status:** `done` — an algorithm name over 64 characters is refused when read
(`a4c86d977df`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.7 CryptoAlgorithmId and CRYPTO_ALGORITHM_INVALID_ID

**Spec:** §7.3.7, p. 39 — "typedef octet CryptoAlgorithmId; const
CryptoAlgorithmId CRYPTO_ALGORITHM_INVALID ID=0x00;" — 0x01 to 0x7F for the
specification, 0x80 to 0xFF vendor-specific.

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformKind`
(l. 29, `None = 0` to `Aes256Gcm = 4`)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::kind_round_trip_all_variants`,
`crates/security-crypto/src/crypto_transform.rs::tests::unknown_kind_rejected`

**Status:** done — the value 0 means no transformation; ZeroDDS occupies no
value in the vendor range. There is no constant named
`CRYPTO_ALGORITHM_INVALID_ID`.

### §7.3.8 CryptoAlgorithmBit and CRYPTO_ALGORITHM_COMPATIBILITY_MODE

**Spec:** §7.3.8, pp. 39–40 — "typedef uint32 CryptoAlgorithmBit; const
CryptoAlgorithmBit CRYPTO_ALGORITHM_COMPATIBILITY_MODE=0x80000000;" — bits
`0x00000001` to `0x00008000` for the specification, `0x00010000` to
`0x40000000` for vendors.

**Repo:** `crates/security/src/algorithm.rs::CryptoAlgorithmBit`, `CRYPTO_ALGORITHM_COMPATIBILITY_MODE`; `crates/rtps/src/security_algo_info.rs::CRYPTO_ALGORITHM_COMPATIBILITY_MODE`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — type and constant `0x80000000`.

### §7.3.8 CRYPTO_ALGORITHM_COMPATIBILITY_MODE missing

**Spec:** §7.3.8, p. 39 — "The value 0x80000000 is reserved and has a special
meaning defined in 7.3.10.1."

**Repo:** `crates/security/src/algorithm.rs::CRYPTO_ALGORITHM_COMPATIBILITY_MODE`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — in `zerodds-security` and in `zerodds-rtps`.

### §7.3.9 CryptoAlgorithmSet, SET_ALL, SET_EMPTY

**Spec:** §7.3.9, p. 40 — "typedef uint32 CryptoAlgorithmSet; const
CryptoAlgorithmSet CRYPTO_ALGORITHM_SET_ALL = 0xffffffff; const
CryptoAlgorithmSet CRYPTO_ALGORITHM_SET_EMPTY = 0x00000000;"

**Repo:** `crates/security/src/algorithm.rs::CryptoAlgorithmSet`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — type and both constants.

### §7.3.9 Constants CRYPTO_ALGORITHM_SET_ALL and _EMPTY missing

**Spec:** §7.3.9, p. 40 — as above.

**Repo:** `crates/security/src/algorithm.rs::CRYPTO_ALGORITHM_SET_ALL`, `CRYPTO_ALGORITHM_SET_EMPTY`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — `0xffffffff` and `0`.

### §7.3.10 CryptoAlgorithmRequirements

**Spec:** §7.3.10, p. 40 — "@extensibility (FINAL) struct
CryptoAlgorithmRequirements { CryptoAlgorithmSet supported_mask;
CryptoAlgorithmSet required_mask; };"

**Repo:** `crates/rtps/src/security_algo_info.rs::AlgorithmRequirements`
(l. 89, `to_bytes` l. 104, `from_bytes` l. 120)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::algorithm_requirements_layout_be`,
`crates/rtps/src/security_algo_info.rs::tests::algorithm_requirements_roundtrip_be`

**Status:** done — eight octets, `supported` before `required`. The type is
called `AlgorithmRequirements`, the fields `supported` and `required`
(naming).

### §7.3.10.1 CheckCryptoAlgorithmCompatibility

**Spec:** §7.3.10.1, pp. 40–41 — "return ( (required_mask & supported_mask ) ==
required_mask ) OR ( ((required_mask & supported_mask ) != 0 ) AND (
(required_mask & CRYPTO_ALGORITHM_COMPATIBILITY_MODE) != 0 ))".

**Repo:** `crates/rtps/src/security_algo_info.rs::check_crypto_algorithm_compatibility`, `CRYPTO_ALGORITHM_COMPATIBILITY_MODE`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::the_compatibility_mode_bit_makes_one_algorithm_enough`

**Status:** `done` — `(required & supported) == required`, or a common algorithm bit and the bit `0x80000000` in `required`.

### §7.3.10.1 Compatibility of two participants

**Spec:** §7.3.10.1, p. 41 — "compatible if and only if [...]
CheckCryptoAlgorithmCompatibility (P2.supported_mask, P1.required_mask) AND
CheckCryptoAlgorithmCompatibility (P1.supported_mask, P2.required_mask)".

**Repo:** `crates/rtps/src/security_algo_info.rs::AlgorithmRequirements::compatible_with`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::two_requirements_are_compatible_both_ways`

**Status:** `done` — both directions.

### §7.3.11 ParticipantSecurityDigitalSignatureAlgorithmInfo

**Spec:** §7.3.11, p. 41 — "@extensibility (APPENDABLE) struct
ParticipantSecurityDigitalSignatureAlgorithmInfo {
CryptoAlgorithmRequirements trust_chain; CryptoAlgorithmRequirements
message_auth; };"

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecurityDigitalSignatureAlgorithmInfo`
(l. 157); field `crates/rtps/src/participant_data.rs::sig_algo_info` (l. 252)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::sig_info_roundtrip`,
`crates/rtps/src/participant_data.rs::tests::participant_data_sig_algo_info_roundtrip`

**Status:** done — 16 octets, `trust_chain` before `message_auth`.

### §7.3.11 trust_chain and message_auth: content of the masks

**Spec:** §7.3.11, pp. 41–42 — "The trust_chain.supported_mask shall contain
the algorithms the SPIs is able to use to validate the digital signature of
documents. The trust_chain.required_mask shall contain all the algorithms that
are contained in digitally-signed documents sent by the SPI [...] The
message_auth.required_mask shall contain all the algorithms the SPIs will use
to sign documents or messages".

**Repo:** `crates/rtps/src/security_algo_info.rs`;
`crates/security-pki/src/plugin.rs::set_participant_security_config`,
`set_local_permissions_signature_algorithm`;
`crates/security-permissions/src/cms.rs::CmsPkcs7Verifier::signature_algorithm_oid`;
call in `crates/security-runtime/src/profile.rs`;
`crates/dcps/src/runtime.rs::local_algorithm_infos`

**Tests:**
`crates/security-pki/src/plugin.rs::tests::the_algorithm_masks_follow_the_identity_and_the_key_agreement`,
`crates/security-pki/src/plugin.rs::tests::the_required_trust_chain_mask_holds_the_algorithm_of_the_permissions_document`,
`crates/security-permissions/tests/cms_verify.rs::the_signature_algorithm_of_a_signed_document_is_read`

**Status:** `done` — `trust_chain.required` holds the signature of the identity
certificate and the signature of the own permissions document (read from the
SignerInfo of the `.p7s`, set when the profile is loaded);
`message_auth.required` the signature of the identity key; `supported` the four
algorithms the plugin verifies (`aef84a98a0b`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195430.log`).

### §7.3.11.1 Compatibility (digital signature)

**Spec:** §7.3.11.1, p. 42 — "compatible if and only if both the trust_chain
and the message_auth are compatible [...] DomainParticipants with
incompatible ParticipantSecurityDigitalSignatureAlgorithmInfo are not able to
authenticate with each other."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`; `crates/rtps/src/security_algo_info.rs::ParticipantSecurityDigitalSignatureAlgorithmInfo::compatible_with`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`, `crates/rtps/src/security_algo_info.rs::tests::signature_and_key_establishment_compatibility`

**Status:** `done` — a remote participant with incompatible signature masks is not authenticated (`security.participant.incompatible`).

### §7.3.11.2 Default value (digital signature)

**Spec:** §7.3.11.2, p. 42 — "trust_chain.supported_mask =
CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256 | CBIT_RSASSA_PKCS1_V15_2048_SHA256 |
CBIT_ECDSA_P256_SHA256; trust_chain.required_mask = CBIT_ECDSA_P256_SHA256;
message_auth.supported_mask = CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256 |
CBIT_ECDSA_P256_SHA256; message_auth.required_mask = CBIT_ECDSA_P256_SHA256".

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::sig_info_spec_default`

**Status:** `done` — the values of §7.3.11.2 (before, the required masks equalled the supported ones, no PKCS1).

### §7.3.12 ParticipantSecurityKeyEstablishmentAlgorithmInfo

**Spec:** §7.3.12, p. 42 — "@extensibility (APPENDABLE) struct
ParticipantSecurityKeyEstablishmentAlgorithmInfo {
CryptoAlgorithmRequirements shared_secret; };"

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecurityKeyEstablishmentAlgorithmInfo`
(l. 217); field `crates/rtps/src/participant_data.rs::kx_algo_info` (l. 257)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::kx_info_roundtrip`,
`crates/rtps/src/participant_data.rs::tests::participant_data_kx_algo_info_roundtrip`

**Status:** done

### §7.3.12 shared_secret: content of the masks

**Spec:** §7.3.12, p. 43 — "The shared_secret.supported_mask shall contain all
the algorithms the SPIs is able to use to establish a shared key. The
shared_secret.required_mask shall contain all the algorithms the SPIs of
other participants must support in order to interoperate."

**Repo:** `crates/security-pki/src/plugin.rs::set_participant_security_config`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_algorithm_masks_follow_the_identity_and_the_key_agreement`

**Status:** `done` — `supported`: DHE+MODP-2048-256, ECDHE-CEUM P-256 and P-384; `required`: the preferred one (X25519 has no bit and gives 0).

### §7.3.12.1 Compatibility (key establishment)

**Spec:** §7.3.12.1, p. 43 — "compatible if and only if the shared_secret is
compatible according to the compatibility rules [...] of 7.3.10.1."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — as §7.3.11.1.

### §7.3.12.2 Default value (key establishment)

**Spec:** §7.3.12.2, p. 43 — "member.supported_mask = CBIT_DHE_MODP_2048_256 |
CBIT_ECDHE_CEUM_P256; member.required_mask = CBIT_ECDHE_CEUM_P256".

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::kx_info_spec_default`

**Status:** `done` — §7.3.12.2.

### §7.3.13 ParticipantSecuritySymmetricCipherAlgorithmInfo

**Spec:** §7.3.13, p. 43 — "@extensibility (APPENDABLE) struct
ParticipantSecuritySymmetricCipherAlgorithmInfo { CryptoAlgorithmSet
supported_mask; CryptoAlgorithmSet builtin_endpoints_required_mask;
CryptoAlgorithmSet builtin_kx_endpoints_required_mask; CryptoAlgorithmSet
user_endpoints_default_required_mask; };"

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecuritySymmetricCipherAlgorithmInfo`
(l. 262); field `crates/rtps/src/participant_data.rs::sym_cipher_algo_info`
(l. 262)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::sym_info_roundtrip`,
`crates/rtps/src/participant_data.rs::tests::participant_data_sym_cipher_algo_info_roundtrip`

**Status:** done

### §7.3.13 Content of the four masks

**Spec:** §7.3.13, pp. 43–44 — "The supported_mask shall contain all the
algorithms the SPIs is able to use to decrypt messages or validate
authentication tags. The builtin_endpoints_required_mask shall contain all
the algorithms [...] for all the builtin endpoints, except for the
DCPSParticipantVolatileMessageSecure [...] The
user_endpoints_default_required_mask shall contain all the default algorithms
that will be used by user-defined (non-builtin) endpoint."

**Repo:** `crates/dcps/src/runtime.rs::local_algorithm_infos`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — `supported`: AES128 and AES256; `builtin_endpoints_required_mask` and `user_endpoints_default_required_mask`: the width of the property `dds.sec.crypto.symmetric_cipher_algorithm`; `builtin_kx_endpoints_required_mask`: AES256 (the Kx key is AES256-GCM).

### §7.3.13.1 Compatibility (symmetric cipher)

**Spec:** §7.3.13.1, p. 44 — four calls of `CheckCryptoAlgorithmCompatibility`
over `builtin_endpoints_required_mask` and
`builtin_kx_endpoints_required_mask` of both participants; "the
user_endpoints_default_required_mask is not considered for compatibility".

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`; `crates/rtps/src/security_algo_info.rs::ParticipantSecuritySymmetricCipherAlgorithmInfo::compatible_with`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`, `crates/rtps/src/security_algo_info.rs::tests::symmetric_cipher_info_compatibility`

**Status:** `done` — builtin and Kx requirements of both sides; `user_endpoints_default_required_mask` does not count.

### §7.3.13.2 Default value (symmetric cipher)

**Spec:** §7.3.13.2, p. 44 — "supported_mask = CBIT_AES128_GCM |
CBIT_AES256_GCM; builtin_endpoints_required_mask = CBIT_AES256_GCM;
builtin_kx_endpoints_required_mask = CBIT_AES256_GCM;
user_endpoints_default_required_mask = CBIT_AES256_GCM".

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::sym_info_spec_default`

**Status:** `done` — §7.3.13.2 (all three requirements AES256).

### §7.3.14 ParticipantSecurityAlgorithmInfo

**Spec:** §7.3.14, p. 45 — "@extensibility (APPENDABLE) struct
ParticipantSecurityAlgorithmInfo {
ParticipantSecurityDigitalSignatureAlgorithmInfo digital_signature;
ParticipantSecurityKeyEstablishmentAlgorithmInfo key_establishment;
ParticipantSecuritySymmetricCipherAlgorithmInfo symmetric_cipher; };"

**Repo:**
`crates/rtps/src/security_algo_info.rs::ParticipantSecurityAlgorithmInfo`,
`EndpointSecurityAlgorithmInfo`

**Tests:**
`crates/rtps/src/security_algo_info.rs::tests::the_aggregates_hold_their_members_in_order`

**Status:** `done` — the aggregate type exists
(`ParticipantSecurityAlgorithmInfo`: 40 octets, the three parts in the order of
the spec; `EndpointSecurityAlgorithmInfo`) (`a5b73d27f19`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). Its use in the plugin
operations belongs to the items of §9.

### §7.3.15 EndpointSecuritySymmetricCipherAlgorithmInfo

**Spec:** §7.3.15, p. 45 — "@extensibility (APPENDABLE) struct
EndpointSecuritySymmetricCipherAlgorithmInfo { CryptoAlgorithmSet
required_mask; @non_serialized CryptoAlgorithmSet supported_mask; };"

**Repo:** `crates/rtps/src/publication_data.rs`, `subscription_data.rs` (`sym_cipher_algo_info`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::endpoint_symmetric_cipher_info_roundtrip`

**Status:** `done` — `required_mask` as `PID_ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO` 0x1013 in both endpoint data types.

### §7.3.15 EndpointSecuritySymmetricCipherAlgorithmInfo: supported_mask missing

**Spec:** §7.3.15, p. 45 — "The value of this member shall be set by the SPI
implementations to match the supported_mask in the
ParticipantSecuritySymmetricCipherAlgorithmInfo of the DomainParticipant that
contains the Endpoint."

**Repo:**
`crates/rtps/src/security_algo_info.rs::EndpointSecuritySymmetricCipherAlgorithmInfo`
(`supported_mask`, not serialized, `with_supported_mask`)

**Tests:**
`crates/rtps/src/publication_data.rs::tests::endpoint_symmetric_cipher_info_roundtrip`,
`crates/rtps/src/security_algo_info.rs::tests::the_aggregates_hold_their_members_in_order`

**Status:** `done` — the field `supported_mask` exists; it is not serialized
(`a5b73d27f19`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.15 required_mask of a DataWriter and a DataReader

**Spec:** §7.3.15, p. 45 — "If the Endpoint is a DataWriter then the
required_mask shall contain all the algorithms that are used for
encrypting/authenticating the data payload and submessages [...] If the
Endpoint is a DataReader then the required_mask shall contain the algorithms
that are used for encrypting/authenticating the protocol-level messages sent
to matched writers".

**Repo:** `crates/dcps/src/runtime.rs::register_endpoint_crypto`,
`local_endpoint_cipher`, `endpoint_incompatibility`;
`crates/security-runtime/src/plugin_attributes.rs::endpoint_cipher_required_mask`

**Tests:**
`crates/dcps/src/runtime.rs::tests::an_endpoint_requires_the_algorithm_its_own_property_names`,
`crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`,
`crates/security-runtime/src/plugin_attributes.rs::tests::an_endpoint_requires_the_algorithm_it_names`

**Status:** `done` — an endpoint requires the algorithm its own property
`dds.sec.crypto.symmetric_cipher_algorithm` names; it goes into discovery as
`PID_ENDPOINT_SECURITY_SYMMETRIC_CIPHER` (0x1013), and an endpoint with an
incompatible setting is not matched (`b7c1adc7281`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.15.1 Compatibility (endpoint)

**Spec:** §7.3.15.1, p. 46 — "CheckCryptoAlgorithmCompatibility (
P2.symmetric_cipher.supported_mask, E1.required_mask) AND
CheckCryptoAlgorithmCompatibility ( P1.symmetric_cipher.supported_mask,
E2.required_mask)".

**Repo:** `crates/dcps/src/runtime.rs::endpoint_incompatibility`, `wire_writer_to_remote_reader`, `wire_reader_to_remote_writer`

**Tests:** `crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`, `crates/rtps/src/security_algo_info.rs::tests::endpoint_symmetric_cipher_compatibility`

**Status:** `done` — endpoints with an incompatible requirement are not matched (`match.refused`).

### §7.3.15.2 Default value (endpoint)

**Spec:** §7.3.15.2, p. 46 — "If the EndpointSecuritySymmetricCipherAlgorithmInfo
is not present [...] the value shall be set to
symmetric_cipher.user_endpoints_default_required_mask of the
DomainParticipant that contains the Endpoint".

**Repo:** `crates/rtps/src/security_algo_info.rs::EndpointSecuritySymmetricCipherAlgorithmInfo::spec_default`

**Tests:** `crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`, `crates/rtps/src/security_algo_info.rs::tests::sym_info_spec_default`

**Status:** `done` — missing: the `user_endpoints_default_required_mask` of the participant of the endpoint.

### §7.3.16 EndpointSecurityAlgorithmInfo

**Spec:** §7.3.16, p. 46 — "@extensibility (APPENDABLE) struct
EndpointSecurityAlgorithmInfo { EndpointSecuritySymmetricCipherAlgorithmInfo
symmetric_cipher; };"

**Repo:**
`crates/rtps/src/security_algo_info.rs::ParticipantSecurityAlgorithmInfo`,
`EndpointSecurityAlgorithmInfo`

**Tests:**
`crates/rtps/src/security_algo_info.rs::tests::the_aggregates_hold_their_members_in_order`

**Status:** `done` — the aggregate type exists
(`ParticipantSecurityAlgorithmInfo`: 40 octets, the three parts in the order of
the spec; `EndpointSecurityAlgorithmInfo`) (`a5b73d27f19`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). Its use in the plugin
operations belongs to the items of §9.

### §7.3.17 CryptoTransformKeyRevision

**Spec:** §7.3.17, p. 46 — "typedef octet CryptoTransformKeyRevision[3];
#define CRYPTO_TRANSFORM_KEY_REVISION_NONE {0x00, 0x00, 0x00}".

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier::key_revision`; `crates/security-crypto/src/plugin.rs` (`KeyMaterial::key_revision`)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::the_key_revision_is_part_of_the_kind`

**Status:** `done` — three octets, in the `transformation_kind` of the CryptoHeader.

### §7.3.17 A revision other than NONE is not evaluated

**Spec:** §7.3.17, p. 46 — "The CryptoTransformKeyRevision provides a way to
represent changes to Key Material."

**Repo:** `crates/security-crypto/src/plugin.rs::check_header_kind`, `from_keymat_cyclone`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_key_revision_is_carried_in_the_keymat_and_the_header`

**Status:** `done` — the key material keeps the revision of its keymat, a CryptoHeader under another revision of the same key id is refused. Making a key revision (`revise_local_entity_keys`, §9.5.1.6) is missing (its own item).

### §7.3.17 CryptoTransformKeyRevisionIntHolder

**Spec:** §7.3.17, pp. 46–47 — "typedef int32
CryptoTransformKeyRevisionIntHolder; [...] int_holder_value =
256*256*transformation_key_revision_value[0] + 256*key_revision_value[1] +
key_revision_value[2]".

**Repo:** `crates/security/src/algorithm.rs::CryptoTransformKeyRevisionIntHolder`, `key_revision_to_int`, `key_revision_from_int`

**Tests:** `crates/security/src/algorithm.rs::tests::the_key_revision_int_holder_follows_the_formula`

**Status:** `done` — `256*256*r[0] + 256*r[1] + r[2]`.

### §7.3.18 CryptoTransformKind

**Spec:** §7.3.18, p. 47 — "@extensibility(FINAL) struct CryptoTransformKind {
CryptoTransformKeyRevision transformation_key_revision; CryptoAlgorithmId
transformation_algorithm_id; }; #define CRYPTO_TRANSFORM_KIND_INVALID
{{0x00, 0x00, 0x00}, 0x00}".

**Repo:** `crates/security-crypto/src/crypto_transform.rs`; `crates/security-crypto/src/plugin.rs`

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::the_key_revision_is_part_of_the_kind`

**Status:** `done` — revision and algorithm id, read the same in both codecs.

### §7.3.18.1 Attribute: transformation_key_revision

**Spec:** §7.3.18.1, p. 47 — "This attribute is used to support the change of
the key material used by a DDS Entity. It is meant to be used in combination
with a CryptoTransformKeyId."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::the_key_revision_is_part_of_the_kind`

**Status:** `done` — as §7.3.17.

### §7.3.18.2 Attribute: transformation_algorithm_id

**Spec:** §7.3.18.2, p. 47 — "The CryptoAlgorithmId values used for the
transformation_algorithm_id shall correspond to those assigned to Symmetric
Cipher and MAC algorithms, see clause 8."

**Repo:** `crates/security-crypto/src/suite.rs::Suite` (`Aes128Gmac` = 0x01,
`from_transform_kind_id`)

**Tests:**
`crates/security-crypto/src/suite.rs::tests::aes128_gmac_uses_spec_id_1`,
`crates/security-crypto/src/suite.rs::tests::from_id_roundtrip_for_all_supported`

**Status:** `done` — the ids are those of §8.1: 0x01 AES128-GMAC, 0x02
AES128-GCM, 0x03 AES256-GMAC, 0x04 AES256-GCM; id 0x01 no longer carries
HMAC-SHA256 (`702c1c51a54`, before this package; the entries were out of date).
Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.18.2 Id 0x01 carries HMAC-SHA256 instead of AES128-GMAC

**Spec:** §7.3.18.2, p. 47, with §8.1, p. 91 — "const CryptoAlgorithmId
CID_AES128_GMAC = 0x01;"

**Repo:** `crates/security-crypto/src/suite.rs::Suite` (`Aes128Gmac` = 0x01,
`from_transform_kind_id`)

**Tests:**
`crates/security-crypto/src/suite.rs::tests::aes128_gmac_uses_spec_id_1`,
`crates/security-crypto/src/suite.rs::tests::from_id_roundtrip_for_all_supported`

**Status:** `done` — the ids are those of §8.1: 0x01 AES128-GMAC, 0x02
AES128-GCM, 0x03 AES256-GMAC, 0x04 AES256-GCM; id 0x01 no longer carries
HMAC-SHA256 (`702c1c51a54`, before this package; the entries were out of date).
Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.19 CryptoTransformKeyId

**Spec:** §7.3.19, p. 47 — "typedef octet CryptoTransformKeyId[4];" — "an
opaque value that helps create a unique 'lookup' reference [...] The scope for
the CryptoTransformKeyId is the DomainParticipant that generated the
CryptoTransformKeyId."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`
(l. 99, `key_id: [u8; 4]`); `crates/security-crypto/src/plugin.rs::KeyMaterial`
(`transformation_key_id`, generated at random, l. 91–97)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::transform_identifier_round_trip`,
`crates/security-crypto/src/plugin.rs::tests::key_rotation_changes_the_wire_key_id_and_the_derived_session_key`

**Status:** done — four random octets per KeyMaterial.

### §7.3.20 CryptoTransformIdentifier

**Spec:** §7.3.20, p. 48 — "@extensibility(FINAL) struct
CryptoTransformIdentifier { CryptoTransformKind transformation_kind;
CryptoTransformKeyId transformation_key_id; };"

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`
(l. 99, `to_bytes`/`from_bytes`); `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(l. 278, writes `transform_kind` and `transformation_key_id` at the start of
the CryptoHeader)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::transform_identifier_round_trip`,
`crates/security-crypto/src/crypto_transform.rs::tests::header_round_trip_with_full_iv`

**Status:** done — eight octets, `transformation_kind` before
`transformation_key_id`.

### §7.3.20.1 Attribute: transformation_kind

**Spec:** §7.3.20.1, p. 48 — "The transformation_algorithm_id identifies the
Crytographic Algorithm used by the transformation. It shall contain one of
the CryptoAlgorithmId values defined in Section 8 [...] The
transformation_key_revision value (see 7.3.17) shall be combined with the
transformation_key_id attribute to identify the key material".

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::the_key_revision_is_part_of_the_kind`

**Status:** `done` — algorithm and revision.

### §7.3.20.2 Key lookup by (sender GUID, key_id, revision)

**Spec:** §7.3.20.2, p. 48 — "The 3-tuple (sender_participant_guid,
transformation_key_id, transformation_key_revision) uniquely identifies the Key
Material within the scope of all Domain Participants that are communicating in
a common DDS Domain."

**Repo:** `crates/security-crypto/src/plugin.rs` (`remote_by_key_id` with the
key `(Sender, transformation_key_id, Revision)`, `decode_data_by_key_id`)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_key_is_found_by_sender_key_id_and_revision`

**Status:** `done` — the remote key is stored and looked up by (handle of the
sender, `transformation_key_id`, revision); two peers with the same
`transformation_key_id` do not overwrite each other (`409b63fcd0d`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.21 PropertyQosPolicy

**Spec:** §7.3.21, pp. 48–49 — "@extensibility(APPENDABLE) struct
PropertyQosPolicy { PropertySeq value; BinaryPropertySeq binary_value; };"

**Repo:** `crates/qos/src/policies/property.rs::PropertyQosPolicy` (strings
and `binary_value`, `propagate` per entry)

**Tests:** `crates/qos/src/policies/property.rs::tests::insert_get_roundtrip`,
`crates/qos/src/policies/property.rs::tests::last_value_wins_and_preserves_order`,
`crates/qos/src/policies/property.rs::tests::only_propagate_true_entries_are_propagated`

**Status:** `done` — the policy carries `value` and `binary_value`, each entry
with `propagate`; `propagated()` and `propagated_binary()` give the entries
for discovery.

### §7.3.21 PropertyQosPolicy: binary_value and propagate missing

**Spec:** §7.3.21, p. 49 — as above; plus §7.3.2 and §7.3.3 (`propagate`).

**Repo:** `crates/qos/src/policies/property.rs::PropertyQosPolicy::set_propagated`,
`::set_binary`, `::propagated`, `::propagated_binary`

**Tests:** `crates/qos/src/policies/property.rs::tests::only_propagate_true_entries_are_propagated`

**Status:** `done` — the sequence of binary properties and `propagate` per
entry exist.

### §7.3.21 DomainParticipantQos.property

**Spec:** §7.3.21, p. 49 — "@extensibility(MUTABLE) struct
DomainParticipantQos { // Existing policies from the DDS specification
PropertyQosPolicy property; };"

**Repo:** `crates/dcps/src/qos.rs::DomainParticipantQos` (l. 306, field
`property`, l. 313)

**Tests:** `crates/dcps/tests/security_qos_properties.rs::a_complete_dds_sec_property_list_creates_a_secured_participant`

**Status:** done

### §7.3.21 DataWriterQos.property

**Spec:** §7.3.21, p. 49 — "struct DataWriterQos { [...] PropertyQosPolicy
property; DataTagQosPolicy data_tags; };"

**Repo:** `crates/dcps/src/qos.rs::DataWriterQos` (`property`,
`propagated_properties`); `crates/dcps/src/runtime.rs::register_endpoint_crypto`

**Tests:**
`crates/dcps/src/qos.rs::tests::property_is_a_member_and_immutable_after_enable`,
`crates/dcps/src/runtime.rs::tests::only_propagated_properties_are_announced`,
`crates/dcps/src/runtime.rs::tests::the_cipher_property_of_an_endpoint_sets_its_key_width`,
`crates/zerodds-c-api/src/qos_ffi.rs::tests::property_and_data_tags_cross_the_c_abi_both_ways`

**Status:** `done` — the field exists and cannot be changed after enabling;
`dds.sec.crypto.symmetric_cipher_algorithm` sets the key width of the writer
(§10.5.1.1), a value outside Table 22 refuses it with `SecurityConfiguration`;
the entries with `propagate = TRUE` go as `PID_PROPERTY_LIST` into the
`PublicationBuiltinTopicData` (`f6657d231dd`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). The C structs
`ZeroDdsDataWriterQos` and `ZeroDdsDataReaderQos`
(`crates/zerodds-c-api/src/qos_ffi.rs`) end in the members `property` and
`data_tags` (`e797d6d6433`, header `e08203d6784`); the layouts in
`crates/ts-node` (`dd8e8f32c24`) and `crates/cs` (`1bd5d2126cf`) follow, the C++
binding reads the header. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-220659.log`):
`crates/zerodds-c-api/src/qos_ffi.rs::tests::property_and_data_tags_cross_the_c_abi_both_ways`.
The bindings ts-node and csharp were not built or run.

### §7.3.21 DataReaderQos.property

**Spec:** §7.3.21, p. 49 — "struct DataReaderQos { [...] PropertyQosPolicy
property; DataTagQosPolicy data_tags; };"

**Repo:** `crates/dcps/src/qos.rs::DataReaderQos` (`property`)

**Tests:**
`crates/rtps/src/subscription_data.rs::tests::endpoint_property_list_roundtrip`,
`crates/dcps/src/runtime.rs::tests::the_cipher_property_of_an_endpoint_sets_its_key_width`,
`crates/zerodds-c-api/src/qos_ffi.rs::tests::property_and_data_tags_cross_the_c_abi_both_ways`

**Status:** `done` — as for `DataWriterQos.property`: the field, the key width
of the reader and the announcement of the `propagate = TRUE` entries in the
`SubscriptionBuiltinTopicData` exist. The C structs `ZeroDdsDataWriterQos` and
`ZeroDdsDataReaderQos` (`crates/zerodds-c-api/src/qos_ffi.rs`) end in the
members `property` and `data_tags` (`e797d6d6433`, header `e08203d6784`); the
layouts in `crates/ts-node` (`dd8e8f32c24`) and `crates/cs` (`1bd5d2126cf`)
follow, the C++ binding reads the header. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-220659.log`):
`crates/zerodds-c-api/src/qos_ffi.rs::tests::property_and_data_tags_cross_the_c_abi_both_ways`.
The bindings ts-node and csharp were not built or run.

### §7.3.21 Tag, TagSeq, DataTags, DataTagQosPolicy

**Spec:** §7.3.21, p. 49 — "@extensibility(FINAL) struct Tag { string name;
string value; }; typedef sequence<Tag> TagSeq; @extensibility(APPENDABLE)
struct DataTags { TagSeq tags; }; typedef DataTags DataTagQosPolicy;"

**Repo:** `crates/qos/src/policies/data_tag.rs` (`Tag`, `DataTagQosPolicy`);
`crates/dcps/src/qos.rs` (`DataWriterQos.data_tags`, `DataReaderQos.data_tags`,
`announced_data_tags`)

**Tests:** `crates/qos/src/policies/data_tag.rs::tests::tags_keep_their_order`,
`crates/dcps/src/runtime.rs::tests::data_tags_are_announced`

**Status:** `done` — `Tag`, `DataTagQosPolicy` and the fields `data_tags` of
`DataWriterQos` and `DataReaderQos` exist (not changeable after enabling) and go
into discovery as `PID_DATA_TAGS` (`bf4642c3c25`, `7b1de8f5255`). Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). The member in the C
struct is with the items on `property` (built, `e797d6d6433`).

### §7.3.21 DataWriterQos.data_tags and DataReaderQos.data_tags

**Spec:** §7.3.21, p. 49 — as above.

**Repo:** `crates/qos/src/policies/data_tag.rs` (`Tag`, `DataTagQosPolicy`);
`crates/dcps/src/qos.rs` (`DataWriterQos.data_tags`, `DataReaderQos.data_tags`,
`announced_data_tags`)

**Tests:** `crates/qos/src/policies/data_tag.rs::tests::tags_keep_their_order`,
`crates/dcps/src/runtime.rs::tests::data_tags_are_announced`

**Status:** `done` — `Tag`, `DataTagQosPolicy` and the fields `data_tags` of
`DataWriterQos` and `DataReaderQos` exist (not changeable after enabling) and go
into discovery as `PID_DATA_TAGS` (`bf4642c3c25`, `7b1de8f5255`). Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). The member in the C
struct is with the items on `property` (built, `e797d6d6433`).

### §7.3.21 Propagation of the PropertyQosPolicy through discovery

**Spec:** §7.3.21, p. 49 — "The PropertyQosPolicy shall be propagated via DDS
discovery so it appears in the ParticipantBuiltinTopicData,
PublicationBuiltinTopicData, and SubscriptionBuiltinTopicData [...] only the
ones with propagate=TRUE are propagated via DDS discovery".

**Repo:** `crates/dcps/src/runtime.rs::update_participant_properties`,
`RuntimeConfig.participant_properties`; `crates/rtps/src/participant_data.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_propagated_participant_properties_reach_the_peer`,
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** `done` — only entries of the participant's `PropertyQosPolicy` with
`propagate = TRUE` go as `PID_PROPERTY_LIST` into the SPDP beacon; a change goes
out at once; a list the wire cannot carry is refused (`a5b73d27f19`,
`e357ab7a5ac`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.3.22 MessageIdentity

**Spec:** §7.3.22, p. 50 — "@extensibility(FINAL) struct MessageIdentity {
GUID_t source_guid; long long sequence_number; };"

**Repo:** `crates/security/src/generic_message.rs::MessageIdentity` (l. 80)

**Tests:** `crates/security/src/generic_message.rs::tests::nil_message_identity`,
`crates/security/src/generic_message.rs::tests::roundtrip_le`

**Status:** done — `sequence_number` is aligned to eight.

### §7.3.22 GenericMessageClassId

**Spec:** §7.3.22, p. 50 — "typedef string<> GenericMessageClassId;"

**Repo:** `crates/security/src/generic_message.rs::ParticipantGenericMessage`

**Tests:** `crates/security/src/generic_message.rs::tests::a_message_class_id_is_not_bounded_to_256_octets`, `::class_id_constants_match_spec`, `::invalid_class_id_utf8_rejected`

**Status:** `done` — an unbounded string; the decoder bounds it by the bound of the whole message only.

### §7.3.22 GenericMessageClassId: bound 256 in the decoder

**Spec:** §7.3.22, p. 50 — as above.

**Repo:** `crates/security/src/generic_message.rs` (`MAX_CLASS_ID_LEN`)

**Tests:** `crates/security/src/generic_message.rs::tests::a_message_class_id_is_not_bounded_to_256_octets`

**Status:** `done` — the bound of 256 octets is removed.

### §7.3.22 ParticipantGenericMessage

**Spec:** §7.3.22, p. 50 — "@extensibility(APPENDABLE) struct
ParticipantGenericMessage { MessageIdentity message_identity; MessageIdentity
related_message_identity; GUID_t destination_participant_guid; GUID_t
destination_endpoint_guid; GUID_t source_endpoint_guid; GenericMessageClassId
message_class_id; DataHolderSeq message_data; };"

**Repo:** `crates/security/src/generic_message.rs::ParticipantGenericMessage`
(l. 97), `to_cdr` (l. 138), `from_cdr` (l. 176)

**Tests:** `crates/security/src/generic_message.rs::tests::roundtrip_le`,
`crates/security/src/generic_message.rs::tests::handshake_request_token_in_message_data`,
`crates/security/src/generic_message.rs::tests::truncated_buffer_rejected`

**Status:** done — the fields are called `destination_participant_key`,
`destination_endpoint_key` and `source_endpoint_key` instead of `*_guid`.

### §7.3.23 ParticipantSecurityProtectionInfo: three members

**Spec:** §7.3.23, p. 50 — "@extensibility (APPENDABLE) struct
ParticipantSecurityProtectionInfo { ParticipantSecurityAttributesMask
participant_security_attributes; PluginParticipantSecurityAttributesMask
plugin_participant_security_attributes; ParticipantSecurityAttributesMaskExt
participant_security_optional_attributes; };"

**Repo:** `crates/rtps/src/participant_security_info.rs` (three members,
`ParticipantSecurityAttributesMaskExt`)

**Tests:**
`crates/rtps/src/participant_security_info.rs::tests::the_optional_attributes_are_the_third_member`

**Status:** `done` — `ParticipantSecurityProtectionInfo` has the three members
of the spec (`470192aa99a`, before this package; the entries were out of date).
Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.23 ParticipantSecurityAttributesMaskExt missing

**Spec:** §7.3.23, pp. 50–51 — "struct ParticipantSecurityAttributesMaskExt {
unsigned short is_set; unsigned short value; }; [...] The is_set mask indicates
whether the corresponding bit in the value mask is set. The interpretation of
each bit is specified in clause 9.4.2.5."

**Repo:** `crates/rtps/src/participant_security_info.rs::ParticipantSecurityAttributesMaskExt`

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::the_optional_attributes_are_the_third_member`

**Status:** `done` — type, codec (read from 12 octets, written only when `is_set` is not 0).

### §7.3.23 PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_VALID

**Spec:** §7.3.23, p. 50 — "#define
PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_VALID (0x1 << 31)".

**Repo:** `crates/rtps/src/participant_security_info.rs::attrs` (l. 20,
`IS_VALID = 0x8000_0000`), `plugin_attrs` (l. 33)

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::round_trip_fully_protected`

**Status:** done

### §7.3.23 Default PARTICIPANT_SECURITY_ATTRIBUTES_INFO_DEFAULT

**Spec:** §7.3.23, p. 51 — "#define PARTICIPANT_SECURITY_ATTRIBUTES_INFO_DEFAULT
{0, 0}".

**Repo:** `crates/rtps/src/participant_security_info.rs::ParticipantSecurityInfo`
(l. 54, `Default` yields both masks 0)

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::empty_default_is_not_valid`,
`crates/rtps/src/participant_security_info.rs::tests::round_trip_default`

**Status:** done

### §7.3.23 Compatible configuration of the participants

**Spec:** §7.3.23, p. 51 — "A compatible configuration is defined as having
the same value for the participant_security_attributes and the
plugin_participant_security_attributes [...] If the is_valid is set to zero on
either of the masks, the comparison [...] shall ignore the attribute."

**Repo:** `crates/rtps/src/participant_security_info.rs::is_compatible_with`; `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`, `crates/rtps/src/participant_security_info.rs::tests::compatibility_is_equality_of_the_valid_masks`

**Status:** `done` — equal masks, a mask without `IS_VALID` on either side is not compared. Not measured against Cyclone, Fast DDS, OpenDDS; `zerodds.sec.compatibility_check=ignore` turns it off.

### §7.3.23 Incompatible participants count as unauthenticated

**Spec:** §7.3.23, p. 51 — "Two participants that don't have compatible
configurations shall not attempt authentication and each participant shall
consider the other participant as an 'unauthenticated participant.'"

**Repo:** `crates/dcps/src/runtime.rs::spdp_security_handshake`, `inproc_drive_security_handshake`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — the handshake does not begin; the participant stays unauthenticated.

### §7.3.23 participant_security_optional_attributes outside compatibility

**Spec:** §7.3.23, p. 51 — "The participant_security_optional_attributes
encode configuration information about the plugin that does not need to be
set consistently for two Participants to authenticate."

**Repo:** `crates/rtps/src/participant_security_info.rs::is_compatible_with`

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::the_optional_attributes_are_the_third_member`

**Status:** `done` — the member is not part of the compatibility.

### §7.3.24 EndpointSecurityProtectionInfo

**Spec:** §7.3.24, pp. 51–52 — "@extensibility(APPENDABLE) struct
EndpointSecurityProtectionInfo { EndpointSecurityAttributesMask
endpoint_security_attributes; PluginEndpointSecurityAttributesMask
plugin_endpoint_security_attributes; }; #define
ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_VALID (0x1 << 31)".

**Repo:** `crates/rtps/src/endpoint_security_info.rs::EndpointSecurityInfo`
(l. 80), `attrs::IS_VALID` (l. 30), `plugin_attrs::IS_VALID` (l. 58)

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::encoded_bytes_le_match_spec_layout`,
`crates/rtps/src/endpoint_security_info.rs::tests::roundtrip_be`

**Status:** done — the type is called `EndpointSecurityInfo` (naming).

### §7.3.24 Default ENDPOINT_SECURITY_ATTRIBUTES_INFO_DEFAULT

**Spec:** §7.3.24, p. 52 — "#define ENDPOINT_SECURITY_ATTRIBUTES_INFO_DEFAULT
{0, 0}".

**Repo:** `crates/rtps/src/endpoint_security_info.rs::EndpointSecurityInfo`
(l. 80, `Default`)

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::default_is_not_valid`

**Status:** done

### §7.3.24 Compatible configuration of the endpoints

**Spec:** §7.3.24, p. 52 — "A compatible configuration is defined as having
the same value for all of the attributes in the EndpointSecurityInfo, except
that [...] If the is_valid is set to zero on either of the masks, the
comparison [...] shall ignore the attribute."

**Repo:** `crates/rtps/src/endpoint_security_info.rs::is_compatible_with`; `crates/dcps/src/runtime.rs::endpoint_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`, `crates/rtps/src/endpoint_security_info.rs::tests::endpoint_compatibility_is_equality_of_the_valid_masks`

**Status:** `done` — as for participants; an endpoint without a mask (OpenDDS) is not compared.

### §7.3.24 plugin_endpoint_security_attributes opaque to the middleware

**Spec:** §7.3.24, p. 52 — "The value of the plugin_endpoint_security_attributes
shall be defined by the security plugin implementation and is opaque to the
DDS middleware (other than the is_valid bit)."

**Repo:** `crates/security-runtime/src/plugin_attributes.rs`;
`crates/dcps/src/runtime.rs` (passes the plugin's masks through)

**Tests:**
`crates/security-runtime/src/plugin_attributes.rs::tests::the_plugin_reads_back_what_it_announces`,
`crates/security-runtime/src/plugin_attributes.rs::tests::key_protected_follows_data_encrypt_per_spec_10_4_1_2_6`,
`crates/security-runtime/src/plugin_attributes.rs::tests::liveliness_protected_sets_0x40_per_spec_9_4_1_3`

**Status:** `done` — the masks `plugin_*_security_attributes` come from the
plugin (`plugin_attributes`) and the middleware only passes them on, it does not
evaluate them (`ee7183c45df`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.25 NOT_ALLOWED_BY_SECURITY

**Spec:** §7.3.25, p. 52 — "The DDS Security specification adds an additional
return code NOT_ALLOWED_BY_SECURITY, which shall be returned by any operation
on the DDS API that fails because the security plugins do not allow it."

**Repo:** `crates/dcps/src/error.rs::DdsError::NotAllowedBySecurity` (l. 67),
`return_code::NOT_ALLOWED_BY_SECURITY` (l. 211)

**Tests:** `crates/dcps/src/error.rs::tests::rc_not_allowed_by_security_maps`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`

**Status:** done — which operations return the code is stated with the
AccessControl operations in §9.4 and §9.8.

### §7.4 Securing DDS Messages on the Wire

**Spec:** §7.4, p. 52 — "OMG DDS uses the Real-Time Publish-Subscribe (RTPS)
on-the-wire protocol [2] for communicating data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §7.4.1 RTPS Background (Non-Normative)

**Spec:** §7.4.1, pp. 52–54 — structure of an RTPS message; division into
DataWriter, DataReader and Interpreter submessages.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the heading says "Non-Normative"; the
consequence for the minor version stands in §7.4.6.1.

### §7.4.2 Secure RTPS Messages

**Spec:** §7.4.2, pp. 54–55 — "this specification uses a 'message
transformation' mechanism that gives the Security Plugin Implementations
fine-grain control over which parts of the RTPS Message need to be encrypted
and/or signed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description of the mechanism; the
submessages stand in §7.4.6 and §7.4.7, the transformations in §9.5.3.

### §7.4.3 GUID tied to the identity: own participant

**Spec:** §7.4.3, p. 55 — "this specification constrains the GUID that can be
chosen by a DomainParticipant, so that it is tied to the Identity of the
DomainParticipant. This is enforced by the Authentication plugin."

**Repo:**
`crates/security-pki/src/guid_adjust.rs::adjust_participant_guid_prefix`;
`crates/security-runtime/src/profile.rs`; `crates/security-pki/src/plugin.rs`
(`check_pdata`)

**Tests:**
`crates/security-pki/src/guid_adjust.rs::tests::adjusted_prefix_sets_high_bit_and_is_deterministic_per_identity`,
`crates/dcps/tests/security_guid_wiring_e2e.rs::with_security_bundle_wires_the_identity_adjusted_prefix_not_random`,
`crates/security-pki/src/plugin.rs::tests::a_request_with_a_pdata_guid_not_bound_to_its_certificate_is_refused`

**Status:** `done` — the own GUID prefix is derived from the certificate and
that of a remote participant is checked against its certificate (the second item
below was already `done`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.3 The GUID of a remote participant is not checked

**Spec:** §7.4.3, p. 55 — as above.

**Repo:** `crates/security-pki/src/plugin.rs` (`check_pdata`,
`validate_remote_identity`: `remote_guids`)

**Tests:**
`crates/security-pki/src/plugin.rs::tests::a_request_with_a_pdata_guid_not_bound_to_its_certificate_is_refused`,
`crates/security-pki/src/plugin.rs::tests::a_request_from_a_participant_discovered_under_another_guid_is_refused`,
`crates/security-pki/src/plugin.rs::tests::a_reply_with_a_pdata_guid_not_bound_to_its_certificate_is_refused`

**Status:** `done` — the GUID in the `c.pdata` of request and reply must derive
from the certificate's SubjectName and equal the prefix the participant was
discovered with (`validate_remote_identity`).

### §7.4.4 KeyHash in the inline QoS with an encrypted payload

**Spec:** §7.4.4, p. 55 — "(1) The KeyHash shall be included in the Inline
Qos."

**Repo:** `crates/rtps/src/reliable_writer.rs::compact_data_prefix` (l. 1828,
`[PID_KEY_HASH, PID_SENTINEL]` for every keyed change);
`crates/dcps/src/runtime.rs::key_hash_of_keyless_write` (l. 14514)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds`,
`crates/dcps/src/runtime.rs::tests::alive_sample_without_key_hash_gets_the_types_computed_hash`

**Status:** done — every change of a keyed type carries `PID_KEY_HASH`,
independent of payload protection.

### §7.4.4 KeyHash as MD5 regardless of the key length

**Spec:** §7.4.4, pp. 55–56 — "(2) The KeyHash shall be computed as the 128
bit MD5 Digest (IETF RFC 1321) applied to the CDR Big- Endian encapsulation of
all the Key fields in sequence. Unlike the rule stated in sub clause 9.6.3.3
[...] the MD5 hash shall be used regardless of the maximum-size of the
serialized key."

**Repo:** `crates/dcps/src/runtime.rs::payload_key_hash`,
`write_user_sample_with_key`, `write_user_lifecycle_with`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_key_hash_of_an_encrypted_payload_is_the_md5_of_the_key`

**Status:** `done` — under `data_protection_kind` ENCRYPT, samples and
dispose/unregister carry the MD5 of the key holder, also for keys of up to 16
octets. A write with an unknown key holder is refused with
`NotAllowedBySecurity`. A keyless type gets no KeyHash. Measured: open
(security matrix against Cyclone, Fast DDS, OpenDDS).

### §7.4.5 PartitionQos of a Publisher immutable

**Spec:** §7.4.5, p. 56 — "The DDS implementation shall not allow a Publisher
to change PartitionQos policy after the Publisher has been enabled if it
contains any DataWriter that meets the following two criteria: (1) The
TopicSecurityAttributes for the Topic associated with the DataWriter have
is_read_protected set to TRUE. (2) The DataWriter has the DurabilityQos policy
kind set to something other than VOLATILE."

**Repo:** `crates/dcps/src/runtime.rs::check_user_writer_partition_change`;
`crates/dcps/src/publisher.rs` (`set_qos`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_publisher_partition_stays_with_a_durable_read_protected_writer`

**Status:** `done` — `Publisher::set_qos` refuses a change of the PARTITION
of an enabled publisher with `ImmutablePolicy` when it holds a DataWriter
with a durability other than VOLATILE on a topic with
`enable_read_access_control`.

### §7.4.6.1 RTPS version 2.5

**Spec:** §7.4.6.1, p. 56 — "The RTPS Major version number shall be set to 2
and the RTPS Minor version number shall be set to 5."

**Repo:** `crates/rtps/src/wire_types.rs::ProtocolVersion::V2_5` (l. 55)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_default_is_2_5`

**Status:** done

### §7.4.6.2 RTPS Secure Submessage Elements

**Spec:** §7.4.6.2, p. 56 — "This specification introduces new RTPS
SubmessageElements that may appear inside RTPS Submessages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction to §7.4.6.2.1 to .4.

### §7.4.6.2.1 CryptoTransformIdentifier as a submessage element

**Spec:** §7.4.6.2.1, pp. 56–57 — "all Cryptographic plugin implementations
shall be set in a way that allows the operations preprocess_secure_submsg,
decode_datareader_submessage, decode_datawriter_submessage, and
decode_serialized_payload to uniquely recognize the cryptographic material
they shall use to decode the message, or recognize that they do not have the
necessary key material."

**Repo:** `crates/security-crypto/src/plugin.rs` (`remote_by_key_id` with the
key `(Sender, transformation_key_id, Revision)`, `decode_data_by_key_id`)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_key_is_found_by_sender_key_id_and_revision`

**Status:** `done` — the remote key is stored and looked up by (handle of the
sender, `transformation_key_id`, revision); two peers with the same
`transformation_key_id` do not overwrite each other (`409b63fcd0d`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.2.2 CryptoContent

**Spec:** §7.4.6.2.2, p. 57 — "The CryptoContent submessage element is used to
wrap a SerializedPayload, an RTPS Submessage, or a complete RTPS Message.
[...] The specific format of this shall be defined by each Cryptographic
plugin implementation."

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (l. 1106,
`content.length` and ciphertext)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`

**Status:** done — format of the builtin plugin in §10.5.

### §7.4.6.2.3 CryptoHeader starts with the CryptoTransformIdentifier

**Spec:** §7.4.6.2.3, p. 57 — "The CryptoHeader submessage element shall
extend the CryptoTransformIdentifier element. Consequently, the leading bytes
in the CryptoHeader shall encode the CryptoTransformIdentifier".

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(l. 278, `transform_kind`, `transformation_key_id`, `session_id`,
`iv_suffix`)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.2.4 CryptoFooter

**Spec:** §7.4.6.2.4, p. 57 — "The CryptoFooter submessage element is used as
postfix to wrap a SerializedPayload, an RTPS Submessage, or a complete RTPS
Message."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116, `common_mac` and the number of receiver-specific MACs)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/security-crypto/src/crypto_transform.rs::tests::footer_round_trip_with_receivers`

**Status:** done — format in §10.5.

### §7.4.6.3 SecureBodySubMsg: content (Table 4)

**Spec:** §7.4.6.3.2, p. 58, Table 4 — `SEC_BODY SubmessageKind`,
`submessageLength ushort`, `EndianessFlag SubmessageFlag`, `crypto_content
CryptoContent`.

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (l. 1106)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`

**Status:** done

### §7.4.6.3.3 SecureBodySubMsg: Validity

**Spec:** §7.4.6.3.3, p. 59 — "The RTPS Submessage is invalid if the
submessageLength in the Submessage header is too small."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_secure_body_with_too_small_a_length_is_invalid`

**Status:** `done` — a SEC_BODY whose length is smaller than `content.length`
asks for is refused in the codec of the runtime (`66307936260`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.3.4 SecureBodySubMsg: Logical Interpretation

**Spec:** §7.4.6.3.4, p. 59 — "A SecureBodySubMsg may wrap a single RTPS
Submessage or a whole RTPS Message."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116), `assemble_srtps_message` (l. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done — with GMAC (ids 0x01, 0x03) the SEC_BODY is left out, the
submessages stand in clear text between prefix and postfix (as Cyclone does).

### §7.4.6.4 SecurePrefixSubMsg: content (Table 5)

**Spec:** §7.4.6.4.2, p. 59, Table 5 — `SEC_PREFIX SubmessageKind`,
`submessageLength`, `EndianessFlag`, `transformation_id
CryptoTransformIdentifier`, `plugin_crypto_header_extra octet[]`.

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116, 20-octet CryptoHeader after the submessage header)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.4.3 SecurePrefixSubMsg: Validity

**Spec:** §7.4.6.4.3, p. 59 — "The RTPS Submessage is invalid if the
submessageLength in the Submessage header is too small."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_secure_prefix_with_too_small_a_length_is_invalid`

**Status:** `done` — a SEC_PREFIX whose length does not hold the 20 octets of
the CryptoHeader is refused (`66307936260`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.4.4 SecurePrefixSubMsg: exactly one submessage, then SEC_POSTFIX

**Spec:** §7.4.6.4.4, p. 59 — "A SecurePrefixSubMsg shall be followed by a
single RTPS Submessage which itself shall be followed by a
SecurePostfixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`
(l. 1144, order prefix, body, postfix)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.5 SecurePostfixSubMsg: content (Table 6)

**Spec:** §7.4.6.5.2, p. 61, Table 6 — `SEC_POSTFIX SubmessageKind`,
`submessageLength`, `EndianessFlag`, `crypto_footer CryptoFooter`.

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.5.3 SecurePostfixSubMsg: Validity

**Spec:** §7.4.6.5.3, p. 61 — "The RTPS Submessage is invalid if there is no
SecurePrefixSubMsg. Immediately before the RTPS submessage that precedes the
SecurePostfixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_secure_postfix_without_a_prefix_is_invalid`

**Status:** `done` — a SEC_POSTFIX without a SEC_PREFIX is invalid and refused
(`66307936260`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.5.4 SecurePostfixSubMsg: Logical Interpretation

**Spec:** §7.4.6.5.4, p. 61 — "The SecurePostfixSubMsg provides a way to
authenticate the validity and origin of the RTPS SubMessage that precedes the
SecurePrefixSubMsg. The Cryptographic transformation applied is identified in
the related SecurePrefixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`
(l. 1144, `common_mac` from the postfix, transformation from the prefix)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.6 SecureRTPSPrefixSubMsg: content (Table 7)

**Spec:** §7.4.6.6.2, pp. 61–62, Table 7 — `SRTPS_PREFIX`, `submessageLength`,
`EndianessFlag (E)`, `AdditionalAuthenticatedDataFlag (A)`,
`PreSharedKeyFlag (P)`, `transformation_id`, `plugin_crypto_header_extra`.

**Repo:** `crates/security-crypto/src/plugin.rs` (`parse_srtps`, flags A and P
read, `assemble_srtps_message`);
`crates/security-rtps/src/srtps.rs::PRE_SHARED_KEY_FLAG` (0x04)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_message_with_the_flag_a_is_checked_with_its_header`,
`crates/security-crypto/src/plugin.rs::tests::the_header_extension_is_adjusted_for_the_aad_and_the_decoded_message`,
`crates/security-rtps/src/srtps.rs::tests::psk_encode_sets_pre_shared_key_flag`

**Status:** `done` — E and the CryptoHeader are written; A and P are read
(`9732105bf3e`, `a01b12a89e9`, before this package; the entries were out of
date). The sender does not set A: it does not bind the RTPS header into the AAD,
which the spec leaves open. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.6 SRTPS_PREFIX: flag A not evaluated

**Spec:** §7.4.6.6.4, p. 62 — "If the AdditionalAuthenticatedDataFlag is set
the authentication tag(s) present in the SecureRTPSPostfixSubMsg include also
the RTPS Header and RTPS HeaderExtension as 'Additional Authenticated Data'
(AAD)."

**Repo:** `crates/security-crypto/src/plugin.rs` (`parse_srtps`,
`open_rtps_cyclone` with the AAD)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_message_with_the_flag_a_is_checked_with_its_header`

**Status:** `done` — a message with A=1 is checked against the RTPS header and
HeaderExtension as AAD (`9732105bf3e`, before this package; the entry was out of
date). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.6 SRTPS_PREFIX: flag P

**Spec:** §7.4.6.6.2, Table 7, p. 62 — "PreSharedKeyFlag (P) [...] P =
SubmessageHeader.flags & 0x04 Indicates that the RTPS message is protected
using a Pre-Shared-Key".

**Repo:** `crates/security-rtps/src/srtps.rs::PRE_SHARED_KEY_FLAG` (0x04),
`srtps_psk_flag`; `crates/security-crypto/src/plugin.rs` (`parse_srtps`)

**Tests:**
`crates/security-rtps/src/srtps.rs::tests::psk_encode_sets_pre_shared_key_flag`

**Status:** `done` — P is bit 0x04; the PSK codec sets and reads it, the codec
of the runtime reads it (`a01b12a89e9`, `9732105bf3e`, before this package; the
entry was out of date). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.6.3 SecureRTPSPrefixSubMsg: Validity

**Spec:** §7.4.6.6.3, p. 62 — "The RTPS Submessage is invalid if the
submessageLength in the Submessage header is too small. The
SecureRTPSPrefixSubMsg shall immediately follow the RTPS Header."

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::transform_inbound`
(l. 1400, SRTPS_PREFIX only at octet 20); `crates/security-crypto/src/plugin.rs::parse_srtps_body`
(l. 995, "expected SRTPS_PREFIX(20)")

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.6.6.4 SecureRTPSPrefixSubMsg: Logical Interpretation

**Spec:** §7.4.6.6.4, p. 62 — "A SecureRTPSPrefixSubMsg shall be followed by a
list of RTPS Submessages which themselves shall be followed by a
SecureRTPSPostfixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_srtps_message`
(l. 956), `parse_srtps_body` (l. 995)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`,
`crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.6.7 SecureRTPSPostfixSubMsg: content (Table 8)

**Spec:** §7.4.6.7.2, p. 63, Table 8 — `SRTPS_POSTFIX`, `submessageLength`,
`EndianessFlag`, `crypto_footer CryptoFooter`.

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_srtps_message`
(l. 956, `common_mac` and count 0)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.6.7.3 SecureRTPSPostfixSubMsg: Validity

**Spec:** §7.4.6.7.3, p. 63 — "The RTPS SecureRTPSPostfixSubMsg is invalid if
there is no SecureRTPSPrefixSubMsg following the RTPS Header."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_srtps_body` (l. 995)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done — evaluation begins only when octet 20 is SRTPS_PREFIX.

### §7.4.6.7.4 SecureRTPSPostfixSubMsg: Logical Interpretation

**Spec:** §7.4.6.7.4, p. 63 — "The SecureRTPSPostfixSubMsg provides a way to
authenticate the validity and origin of the list of RTPS Submessages between
the related SecureRTPSPrefixSubMsg and the SecureRTPSPrefixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_srtps_body` (l. 995)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.7 Mapping to UDP/IP PSM

**Spec:** §7.4.7, p. 63 — "Sub clause 7.4.7 does the same thing for the new
RTPS submessage elements and submessages introduced by the DDS Security
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §7.4.7.1 Table 9 — SEDPbuiltinPublicationsSecureWriter {{ff,00,03},c2}

**Spec:** §7.4.7.1, Table 9, p. 63 — "ENTITYID_SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER
{{ff, 00, 03}, c2}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER`
(l. 534)

**Tests:** `crates/rtps/src/wire_types.rs::tests::secure_publications_writer_layout`

**Status:** done

### §7.4.7.1 Table 9 — SEDPbuiltinPublicationsSecureReader {{ff,00,03},c7}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 00, 03}, c7}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — SEDPbuiltinSubscriptionsSecureWriter {{ff,00,04},c2}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 00, 04}, c2}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — SEDPbuiltinSubscriptionsSecureReader {{ff,00,04},c7}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 00, 04}, c7}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — BuiltinParticipantMessageSecureWriter {{ff,02,00},c2}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 02, 00}, c2}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — BuiltinParticipantMessageSecureReader {{ff,02,00},c7}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 02, 00}, c7}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — BuiltinParticipantStatelessMessageWriter {{00,02,01},c3}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{00, 02, 01}, c3}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`
(l. 567)

**Tests:** `crates/rtps/src/wire_types.rs::tests::stateless_writer_is_no_key_kind`

**Status:** done

### §7.4.7.1 Table 9 — BuiltinParticipantStatelessMessageReader {{00,02,01},c4}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{00, 02, 01}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — BuiltinParticipantVolatileMessageSecureWriter {{ff,02,02},c3}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 02, 02}, c3}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER`
(l. 579)

**Tests:** `crates/rtps/src/wire_types.rs::tests::volatile_message_secure_entity_ids_match_spec_and_cyclone`

**Status:** done

### §7.4.7.1 Table 9 — BuiltinParticipantVolatileMessageSecureReader {{ff,02,02},c4}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 02, 02}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER`
(l. 587)

**Tests:** `crates/rtps/src/wire_types.rs::tests::volatile_message_secure_entity_ids_match_spec_and_cyclone`

**Status:** done

### §7.4.7.1 Table 9 — SPDPbuiltinParticipantsSecureWriter {{ff,01,01},c2}

**Spec:** §7.4.7.1, Table 9, p. 64 — "ENTITYID_SPDP_RELIABLE_BUILTIN_PARTICIPANT_SECURE_WRITER
{{ff, 01, 01}, c2}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — SPDPbuiltinParticipantsSecureReader {{ff,01,01},c7}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 01, 01}, c7}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER`
(l. 599)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_secure_reader_layout`

**Status:** done

### §7.4.7.1 Table 9 — octets without a test

**Spec:** §7.4.7.1, Table 9, pp. 63–64 — the seven rows listed as `partial`
above.

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — TypeLookupServiceRequestSecureWriter {{ff,03,00},c3}

**Spec:** §7.4.7.1, Table 9, p. 64 — "ENTITYID_TL_SVC_REQ_SECURE _WRITER
{{ff, 03, 00}, c3}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::TL_SVC_REQ_SECURE_WRITER`,
`TL_SVC_REQ_SECURE_READER`, `TL_SVC_REPLY_SECURE_WRITER`,
`TL_SVC_REPLY_SECURE_READER`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the constant exists with the octets of the table
(`7b1de8f5255`, test `a5b73d27f19`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — TypeLookupServiceRequestSecureReader {{ff,03,00},c4}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 03, 00}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::TL_SVC_REQ_SECURE_WRITER`,
`TL_SVC_REQ_SECURE_READER`, `TL_SVC_REPLY_SECURE_WRITER`,
`TL_SVC_REPLY_SECURE_READER`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the constant exists with the octets of the table
(`7b1de8f5255`, test `a5b73d27f19`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — TypeLookupServiceReplySecureWriter {{ff,03,01},c3}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 03, 01}, c3}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::TL_SVC_REQ_SECURE_WRITER`,
`TL_SVC_REQ_SECURE_READER`, `TL_SVC_REPLY_SECURE_WRITER`,
`TL_SVC_REPLY_SECURE_READER`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the constant exists with the octets of the table
(`7b1de8f5255`, test `a5b73d27f19`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — TypeLookupServiceReplySecureReader {{ff,03,01},c4}

**Spec:** §7.4.7.1, Table 9, p. 64 — "{{ff, 03, 01}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::TL_SVC_REQ_SECURE_WRITER`,
`TL_SVC_REQ_SECURE_READER`, `TL_SVC_REPLY_SECURE_WRITER`,
`TL_SVC_REPLY_SECURE_READER`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the constant exists with the octets of the table
(`7b1de8f5255`, test `a5b73d27f19`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.2 Mapping of the CryptoTransformIdentifier Type

**Spec:** §7.4.7.2, p. 64 — "The UDP/IP PSM maps the CryptoTransformIdentifier
to the IDL definition in 7.3.20."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`
(l. 99)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::transform_identifier_round_trip`

**Status:** done

### §7.4.7.3 CryptoHeader: Wire Representation

**Spec:** §7.4.7.3, pp. 64–65 — "The CryptoHeader shall start with the
CryptoTransformIdentifier and be followed by a plugin-specific
plugin_crypto_header_extra returned by the encoding transformation." Layout:
`transformation_kind` (4), `transformation_key_id` (4),
`plugin_crypto_header_extra`.

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(l. 278); `crates/security-crypto/src/crypto_transform.rs::CryptoHeader`
(l. 150)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::header_round_trip_with_full_iv`,
`crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.4 CryptoFooter: Wire Representation

**Spec:** §7.4.7.4, p. 65 — "The CryptoFooter contains a plugin-specific
plugin_crypto_footer returned by the encoding transformation."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoFooter`
(l. 207); `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::footer_round_trip_no_receivers`,
`crates/security-crypto/src/crypto_transform.rs::tests::footer_rejects_oversized_receiver_count_cleanly`

**Status:** done

### §7.4.7.5 SecureBodySubMsg: wire representation and id 0x30

**Spec:** §7.4.7.5, p. 65 — "| SEC_BODY |X|X|X|X|X|X|X|E| octetsToNextHeader |
[...] CryptoContent crypto_content" — "The SecureBodySubMsg shall have the
submessageId set to the value 0x30."

**Repo:** `crates/security-crypto/src/plugin.rs::SEC_BODY_ID` (l. 943),
`push_sec_body` (l. 1106)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`

**Status:** done

### §7.4.7.5.3 SecureBodySubMsg: only the EndiannessFlag

**Spec:** §7.4.7.5.3, p. 65 — "The SecureBodySubMsg only uses the
EndiannessFlag."

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (l. 1106,
flag `0x01`)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`

**Status:** done — ZeroDDS always writes E=1; on receipt the length follows
the flag (`parse_secure_submessages`).

### §7.4.7.6 SecurePrefixSubMsg: wire representation and id 0x31

**Spec:** §7.4.7.6, pp. 65–66 — "| SEC_PREFIX |X|X|X|X|X|X|X|E| [...]
CryptoHeader crypto_header" — "submessageId set to the value 0x31".

**Repo:** `crates/security-crypto/src/plugin.rs::SEC_PREFIX_ID` (l. 942),
`assemble_secure_submessages` (l. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.6.3 SecurePrefixSubMsg: only the EndiannessFlag

**Spec:** §7.4.7.6.3, p. 66 — "The SecurePrefixSubMsg only uses the
EndiannessFlag."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.7 SecurePostfixSubMsg: wire representation and id 0x32

**Spec:** §7.4.7.7, p. 66 — "| SEC_POSTFIX |X|X|X|X|X|X|X|E| [...]
CryptoFooter crypto_footer" — "submessageId set to the value 0x32".

**Repo:** `crates/security-crypto/src/plugin.rs::SEC_POSTFIX_ID` (l. 944),
`assemble_secure_submessages` (l. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.7.3 SecurePostfixSubMsg: only the EndiannessFlag

**Spec:** §7.4.7.7.3, p. 66 — "The SecurePostfixSubMsg only uses the
EndiannessFlag."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.8 SecureRTPSPrefixSubMsg: wire representation and id 0x33

**Spec:** §7.4.7.8, p. 66 — "| SRTPS_PREFIX |X|X|X|X|X|P|A|E| [...]
CryptoHeader crypto_header" — "submessageId set to the value 0x33".

**Repo:** `crates/security-crypto/src/plugin.rs::SRTPS_PREFIX_ID` (l. 951),
`assemble_srtps_message` (l. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.7.8.3 SecureRTPSPrefixSubMsg: flags E, A, P

**Spec:** §7.4.7.8.3, pp. 66–67 — "EndiannessFlag (E) [...]
AdditionalAuthenticatedDataFlag (A) [...] PreSharedKeyFlag (P): Indicates that
the RTPS message is protected using a Pre-Shared-Key."

**Repo:** `crates/security-crypto/src/plugin.rs` (`parse_srtps`, flags A and P
read, `assemble_srtps_message`);
`crates/security-rtps/src/srtps.rs::PRE_SHARED_KEY_FLAG` (0x04)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_message_with_the_flag_a_is_checked_with_its_header`,
`crates/security-crypto/src/plugin.rs::tests::the_header_extension_is_adjusted_for_the_aad_and_the_decoded_message`,
`crates/security-rtps/src/srtps.rs::tests::psk_encode_sets_pre_shared_key_flag`

**Status:** `done` — E and the CryptoHeader are written; A and P are read
(`9732105bf3e`, `a01b12a89e9`, before this package; the entries were out of
date). The sender does not set A: it does not bind the RTPS header into the AAD,
which the spec leaves open. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.9 SecureRTPSPostfixSubMsg: wire representation and id 0x34

**Spec:** §7.4.7.9, p. 67 — "| SRTPS_POSTFIX |X|X|X|X|X|X|X|E| [...]" —
"submessageId set to the value 0x34".

**Repo:** `crates/security-crypto/src/plugin.rs::SRTPS_POSTFIX_ID` (l. 952),
`assemble_srtps_message` (l. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.7.9.3 SecureRTPSPostfixSubMsg: only the EndiannessFlag

**Spec:** §7.4.7.9.3, p. 67 — "The SecureRTPSPostfixSubMsg only uses the
EndiannessFlag."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_srtps_message`
(l. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.5 DDS Support for Security Plugin Information Exchange

**Spec:** §7.5, p. 67 — "Where possible, this specification tries to reuse and
extend existing DDS concepts and facilities so that they can fulfill the needs
of the security plugins".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §7.5.1.1 Background (Non-Normative)

**Spec:** §7.5.1.1, pp. 67–68 — rationale for additional protected discovery
topics beside the existing ones.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the heading says "Non-Normative".

### §7.5.1.2 Extending the Data Types used by DDS Discovery

**Spec:** §7.5.1.2, pp. 68–69 — "All that is required is to serialize the
specific data types defined here according to the format described in the
DDS-XTYPES specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the individual members are listed in
§7.5.1.4 to §7.5.1.8.

### §7.5.1.3 Reserved RTPS parameter IDs

**Spec:** §7.5.1.3, p. 69 — "This specification reserves the RTPS Simple
Discovery Protocol ParameterIDs in the range: 0x1000 to 0x1FFF and 0x5000 to
0x5FFF."

**Repo:** `crates/rtps/src/parameter_list.rs` (in the two ranges only the ids
of the specification: `0x1001`, `0x1002`, `0x1004`, `0x1005`, `0x1006`,
`0x1010` to `0x1013`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::is_standard_pid_recognises_dds_security_pids`

**Status:** done

### §7.5.1.4 Table 10 — identity_token, PID_IDENTITY_TOKEN 0x1001

**Spec:** §7.5.1.4, Table 10, p. 69 — "identity_token IdentityToken
PID_IDENTITY_TOKEN 0x1001".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::IDENTITY_TOKEN` (l. 159);
`crates/rtps/src/participant_data.rs::identity_token` (l. 241)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_identity_token_pid_roundtrip`

**Status:** done

### §7.5.1.4 Table 10 — permissions_token, PID_PERMISSIONS_TOKEN 0x1002

**Spec:** §7.5.1.4, Table 10, p. 69 — "permissions_token PermissionsToken
PID_PERMISSIONS_TOKEN 0x1002".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::PERMISSIONS_TOKEN` (l. 164);
`crates/rtps/src/participant_data.rs::permissions_token` (l. 244)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_permissions_token_pid_roundtrip`

**Status:** done

### §7.5.1.4 Table 10 — property, PID_PROPERTY_LIST 0x0059

**Spec:** §7.5.1.4, Table 10, p. 69 — "property PropertyQosPolicy
PID_PROPERTY_LIST 0x0059".

**Repo:** `crates/rtps/src/property_list.rs::WirePropertyList` (binary
entries); `crates/rtps/src/participant_data.rs`

**Tests:**
`crates/rtps/src/property_list.rs::tests::binary_properties_cross_the_wire`,
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** `done` — the `BinaryPropertySeq` is written and read with its
entries (`928d39304cd`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 PID_PROPERTY_LIST: binary_value is lost

**Spec:** §7.5.1.4, p. 70 — "Only the Property_t and BinaryProperty_t elements
having the propagate member set to TRUE are serialized."

**Repo:** `crates/rtps/src/property_list.rs::WirePropertyList::binary_entries`

**Tests:** `crates/rtps/src/property_list.rs::tests::binary_properties_cross_the_wire`

**Status:** `done` — `binary_value` is written and read.

### §7.5.1.4 Table 10 — protection_info, PID_PARTICIPANT_SECURITY_PROTECTION_INFO 0x1005

**Spec:** §7.5.1.4, Table 10, p. 69 — "protection_info
ParticipantSecurityProtectionInfo PID_PARTICIPANT_SECURITY_PROTECTION_INFO
0x1005".

**Repo:** `crates/rtps/src/participant_security_info.rs` (three members,
`ParticipantSecurityAttributesMaskExt`)

**Tests:**
`crates/rtps/src/participant_security_info.rs::tests::the_optional_attributes_are_the_third_member`

**Status:** `done` — `ParticipantSecurityProtectionInfo` has the three members
of the spec (`470192aa99a`, before this package; the entries were out of date).
Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 10 — available_builtin_endpoints_ext, PID 0x1007

**Spec:** §7.5.1.4, Table 10, p. 70 — "available_builtin_endpoints_ext
AvailableBuiltinEndpointsExtSet_t PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT 0x1007".

**Repo:** `crates/rtps/src/participant_data.rs`
(`pid::AVAILABLE_BUILTIN_ENDPOINTS_EXT` 0x1007, field
`available_builtin_endpoints_ext`, missing = 0)

**Tests:**
`crates/rtps/src/participant_data.rs::tests::available_builtin_endpoints_ext_roundtrip`

**Status:** `done` — the parameter 0x1007 exists; when absent the value is 0
(`7b1de8f5255`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 10 — digital_signature, PID 0x1010

**Spec:** §7.5.1.4, Table 10, p. 70 — "digital_signature
ParticipantSecurityDigitalSignatureAlgorithmInfo
PID_PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO 0x1010".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO`
(l. 173)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_sig_algo_info_roundtrip`

**Status:** done — PID and codec; that the runtime never sets the parameter is
stated with §7.3.11.

### §7.5.1.4 Table 10 — key_establishment, PID 0x1011

**Spec:** §7.5.1.4, Table 10, p. 70 — "key_establishment
ParticipantSecurityKeyEstablishmentAlgorithmInfo
PID_PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO 0x1011".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO`
(l. 178)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_kx_algo_info_roundtrip`

**Status:** done

### §7.5.1.4 Table 10 — symmetric_cipher, PID 0x1012

**Spec:** §7.5.1.4, Table 10, p. 70 — "symmetric_cipher
ParticipantSecuritySymmetricCipherAlgorithmInfo
PID_PARTICIPANT_SECURITY_BUILTIN_EP_SYMMETRIC_CIPHER_ALGORITHM_INFO 0x1012".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO`
(l. 184)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_sym_cipher_algo_info_roundtrip`

**Status:** done

### §7.5.1.4 Missing available_builtin_endpoints_ext counts as 0

**Spec:** §7.5.1.4, p. 70 — "If the member available_builtin_endpoints_ext is
not present in the ParticipantBuiltinTopic, the receiver shall interpret the
value of the member to be 0x00000000."

**Repo:** `crates/rtps/src/participant_data.rs`
(`pid::AVAILABLE_BUILTIN_ENDPOINTS_EXT` 0x1007, field
`available_builtin_endpoints_ext`, missing = 0)

**Tests:**
`crates/rtps/src/participant_data.rs::tests::available_builtin_endpoints_ext_roundtrip`

**Status:** `done` — the parameter 0x1007 exists; when absent the value is 0
(`7b1de8f5255`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Missing digital_signature counts as the default

**Spec:** §7.5.1.4, p. 70 — "If the member digital_signature is not present
[...] the receiver shall interpret the value of the member to be the default
defined in clause 7.3.11.2."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — missing: the default of §7.3.11.2.

### §7.5.1.4 Missing key_establishment counts as the default

**Spec:** §7.5.1.4, p. 70 — "If the member key_establishment is not present in
the ParticipantBuiltinTopic, the receiver shall interpret the value of the
member to be the default defined in clause 7.3.13.2."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — missing: §7.3.12.2.

### §7.5.1.4 Missing symmetric_cipher counts as the default

**Spec:** §7.5.1.4, p. 70 — "If the member symmetric_cipher is not present
[...] the default defined in clause 7.3.13.2."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — missing: §7.3.13.2.

### §7.5.1.4 Only properties with propagate=TRUE are serialized

**Spec:** §7.5.1.4, p. 70 — "Only the Property_t and BinaryProperty_t elements
having the propagate member set to TRUE are serialized."

**Repo:** `crates/dcps/src/runtime.rs::update_participant_properties`,
`RuntimeConfig.participant_properties`; `crates/rtps/src/participant_data.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_propagated_participant_properties_reach_the_peer`,
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** `done` — only entries of the participant's `PropertyQosPolicy` with
`propagate = TRUE` go as `PID_PROPERTY_LIST` into the SPDP beacon; a change goes
out at once; a list the wire cannot carry is refused (`a5b73d27f19`,
`e357ab7a5ac`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.1.4 propagate is not serialized

**Spec:** §7.5.1.4, p. 70 — "the serialization of the Property_t and
BinaryProperty_t elements shall omit the serialization of the propagate
member."

**Repo:** `crates/rtps/src/property_list.rs::WireProperty` (l. 52, without
`propagate`)

**Tests:** `crates/rtps/src/property_list.rs::tests::single_property_roundtrip_le`

**Status:** done

### §7.5.1.4 Receiver sets propagate to TRUE

**Spec:** §7.5.1.4, p. 70 — "Even if it is not present in the serialized data,
the receiver will set the propagate member to TRUE."

**Repo:** `crates/rtps/src/participant_data.rs::properties` (l. 235,
`WirePropertyList`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** done — the received properties stand as `WireProperty` without a
`propagate` member in `ParticipantBuiltinTopicData`; they are not taken into a
local `PropertyList`.

### §7.5.1.4 PID_PROPERTY_LIST carries PropertySeq

**Spec:** §7.5.1.4, p. 70 — "Note that according to DDS-RTPS the
PID_PROPERTY_LIST is associated with a single PropertySeq rather than the
PropertyQosPolicy [...] This does not cause any interoperability problems".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — note without a requirement.

### §7.5.1.4 Table 11 — SEDPbuiltinPublicationsSecureWriter, bit 16

**Spec:** §7.5.1.4, Table 11, p. 71 — "SEDPbuiltinPublicationsSecureWriter
(0x00000001 << 16)" — "These bits shall be set on the
availableBuiltinEndpoints."

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PUBLICATIONS_SECURE_WRITER`
(l. 94); set in `crates/dcps/src/runtime.rs` (l. 8670)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — SEDPbuiltinPublicationsSecureReader, bit 17

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 17)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PUBLICATIONS_SECURE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — SEDPbuiltinSubscriptionsSecureWriter, bit 18

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 18)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::SUBSCRIPTIONS_SECURE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — SEDPbuiltinSubscriptionsSecureReader, bit 19

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 19)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::SUBSCRIPTIONS_SECURE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantMessageSecureWriter, bit 20

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 20)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_MESSAGE_SECURE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantMessageSecureReader, bit 21

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 21)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_MESSAGE_SECURE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantStatelessMessageWriter, bit 22

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 22)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantStatelessMessageReader, bit 23

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 23)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantVolatileMessageSecureWriter, bit 24

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 24)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantVolatileMessageSecureReader, bit 25

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 25)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — SPDPbuiltinParticipantSecureWriter, bit 26

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 26)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_SECURE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done — with `enable_secure_spdp = false`
(`crates/dcps/src/runtime.rs` l. 1589) bits 26 and 27 stay set although the
endpoints do not run (comment l. 1578–1579).

### §7.5.1.4 Table 11 — SPDPbuiltinParticipantSecureReader, bit 27

**Spec:** §7.5.1.4, Table 11, p. 71 — "(0x00000001 << 27)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_SECURE_READER`
(l. 127)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done — as bit 26.

### §7.5.1.4 Announce the secure TypeLookup endpoints in available_builtin_endpoints_ext

**Spec:** §7.5.1.4, p. 71 — "DDS-Security implementations that support
DDS-XTYPES shall advertise the availability of the Secure TypeLookup Built-In
Endpoints using the Parameter with ID PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT".

**Repo:** `crates/dcps/src/runtime.rs` (SPDP-Beacon:
`available_builtin_endpoints_ext = ALL_TYPE_LOOKUP_SECURE`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_secure_type_lookup_service_is_protected_per_submessage`,
`crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — a secured participant announces the four endpoints
(`a542c9f545d`). Measured (Linux test host, sec-diag10): the announcement changes
nothing in any cell of `common-subset`, `disc-meta-data`, `disc-data-enc`,
`data-sign` (`4912df99883` against `32f11c44270`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 12 — TypeLookupServiceRequestSecureWriter, bit 0

**Spec:** §7.5.1.4, Table 12, p. 71 — "TypeLookupServiceRequestSecureWriter
(0x00000001 << 0)".

**Repo:** `crates/rtps/src/participant_data.rs::builtin_endpoint_ext`

**Tests:** `crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — the constant exists with the bit of the table
(`7b1de8f5255`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 12 — TypeLookupServiceRequestSecureReader, bit 1

**Spec:** §7.5.1.4, Table 12, p. 71 — "(0x00000001 << 1)".

**Repo:** `crates/rtps/src/participant_data.rs::builtin_endpoint_ext`

**Tests:** `crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — the constant exists with the bit of the table
(`7b1de8f5255`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 12 — TypeLookupServiceReplySecureWriter, bit 2

**Spec:** §7.5.1.4, Table 12, p. 72 — "(0x00000001 << 2)".

**Repo:** `crates/rtps/src/participant_data.rs::builtin_endpoint_ext`

**Tests:** `crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — the constant exists with the bit of the table
(`7b1de8f5255`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 12 — TypeLookupServiceReplySecureReader, bit 3

**Spec:** §7.5.1.4, Table 12, p. 72 — "(0x00000001 << 3)".

**Repo:** `crates/rtps/src/participant_data.rs::builtin_endpoint_ext`

**Tests:** `crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — the constant exists with the bit of the table
(`7b1de8f5255`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.5 Table 13 — protection_info, PID_ENDPOINT_SECURITY_PROTECTION_INFO 0x1004

**Spec:** §7.5.1.5, Table 13, p. 72 — "protection_info EndpointSecurityInfo
PID_ENDPOINT_SECURITY_PROTECTION_INFO 0x1004".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::ENDPOINT_SECURITY_INFO`
(l. 146); `crates/rtps/src/publication_data.rs::security_info` (l. 279);
`crates/rtps/src/subscription_data.rs::security_info` (l. 90); set in
`crates/dcps/src/runtime.rs::user_endpoint_security_info` (l. 9664)

**Tests:** `crates/rtps/src/publication_data.rs::tests::security_info_roundtrip`,
`crates/rtps/src/subscription_data.rs::tests::security_info_roundtrip`

**Status:** done

### §7.5.1.5 Table 13 — symmetric_cipher, PID 0x1013

**Spec:** §7.5.1.5, Table 13, p. 72 — "symmetric_cipher
EndpointSecuritySymmetricCipherAlgorithmInfo
PID_ENDPOINT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO 0x1013".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO`; `publication_data.rs`, `subscription_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::endpoint_symmetric_cipher_info_roundtrip`

**Status:** `done` — codec and field; the runtime sends none (no endpoint differs from the default of its participant).

### §7.5.1.5 Missing symmetric_cipher counts as the default

**Spec:** §7.5.1.5, p. 73 — "If the member symmetric_cipher is not present [...]
the receiver shall interpret the value of the member to be the default defined
in clause 7.3.15.2."

**Repo:** `crates/dcps/src/runtime.rs::endpoint_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`

**Status:** `done` — see §7.3.15.2.

### §7.5.1.6 DCPSParticipants stays for the announcement

**Spec:** §7.5.1.6, p. 73 — "Implementations of the DDS Security shall use that
same DCPSParticipants builtin Topic to announce the DomainParticipant
information."

**Repo:** `crates/dcps/src/runtime.rs` (SPDP beacon over
`SPDP_BUILTIN_PARTICIPANT_WRITER`, also with security)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_beacon_announces_identity_token_and_discovery_triggers_handshake`

**Status:** done

### §7.5.1.6 DCPSParticipantsSecure: topic and endpoints

**Spec:** §7.5.1.6, p. 73 — "Implementations of the DDS Security specification
shall have an additional builtin Topic referred to as DCPSParticipantsSecure
and associated builtin DataReader and DataWriter entities".

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`TOPIC_NAME_DCPS_PARTICIPANTS_SECURE`, `ParticipantBuiltinTopicDataSecure`);
`crates/dcps/src/builtin_subscriber.rs`;
`crates/dcps/src/runtime.rs::push_sedp_events_to_secure_readers`,
`send_secure_spdp`; `crates/discovery/src/spdp.rs::serialize_secure`

**Tests:**
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`,
`crates/dcps/src/runtime.rs::tests::the_secure_discovery_data_reaches_the_secure_readers`,
`crates/discovery/src/spdp.rs::tests::secure_spdp_takes_a_new_number_for_a_change_only`

**Status:** `done` — the topic `DCPSParticipantsSecure` exists with reader, type
and the QoS of `DCPSPublications` (RELIABLE); the secure SPDP writer numbers
every change and does not go to multicast (`1bee936d138`, `054dd0f3a8b`). Tests
green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 Topic name "DCPSParticipantsSecure"

**Spec:** §7.5.1.6, p. 73 — "The Topic name for the DCPSParticipantsSecure
Topic shall be 'DCPSParticipantsSecure'."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`TOPIC_NAME_DCPS_PARTICIPANTS_SECURE`, `ParticipantBuiltinTopicDataSecure`);
`crates/dcps/src/builtin_subscriber.rs`;
`crates/dcps/src/runtime.rs::push_sedp_events_to_secure_readers`,
`send_secure_spdp`; `crates/discovery/src/spdp.rs::serialize_secure`

**Tests:**
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`,
`crates/dcps/src/runtime.rs::tests::the_secure_discovery_data_reaches_the_secure_readers`,
`crates/discovery/src/spdp.rs::tests::secure_spdp_takes_a_new_number_for_a_change_only`

**Status:** `done` — the topic `DCPSParticipantsSecure` exists with reader, type
and the QoS of `DCPSPublications` (RELIABLE); the secure SPDP writer numbers
every change and does not go to multicast (`1bee936d138`, `054dd0f3a8b`). Tests
green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 ParticipantBuiltinTopicDataSecure: identity_status_token 0x1006

**Spec:** §7.5.1.6, Table 14, p. 73 — "identity_status_token
IdentityStatusToken PID_IDENTITY_STATUS_TOKEN 0x1006" — "struct
ParticipantBuiltinTopicDataSecure: ParticipantBuiltinTopicData { @id(0x1006)
@optional IdentityStatusToken identity_status_token; };"

**Repo:** `crates/discovery/src/spdp.rs::serialize` (without), `serialize_secure`
(with `identity_status_token`); `crates/dcps/src/runtime.rs` (sets it from the
plugin); `crates/security-pki/src/plugin.rs::get_identity_status_token`

**Tests:**
`crates/discovery/src/spdp.rs::tests::the_identity_status_token_is_in_the_secure_participant_data_only`,
`crates/security-pki/src/plugin.rs::tests::the_identity_status_token_has_the_class_of_table_53`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`

**Status:** `done` — the `identity_status_token` (0x1006) is in the secure SPDP
and in the type `ParticipantBuiltinTopicDataSecure` only, not in the beacon of
`DCPSParticipants`; the runtime sets it from the plugin's `IdentityStatusToken`
(class `DDS:Auth:PKI-DH:1.0`, `ocsp_status` when an OCSP response exists, Table
53) (`aef84a98a0b`, `257d0f8d0e6`). New on the wire: PID 0x1006 in the secure
SPDP to authenticated peers; not measured against Cyclone, Fast DDS and OpenDDS.
Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 identity_status_token only in the secure type, and filled in

**Spec:** §7.5.1.6, p. 73 — as above.

**Repo:** `crates/discovery/src/spdp.rs::serialize` (without), `serialize_secure`
(with `identity_status_token`); `crates/dcps/src/runtime.rs` (sets it from the
plugin); `crates/security-pki/src/plugin.rs::get_identity_status_token`

**Tests:**
`crates/discovery/src/spdp.rs::tests::the_identity_status_token_is_in_the_secure_participant_data_only`,
`crates/security-pki/src/plugin.rs::tests::the_identity_status_token_has_the_class_of_table_53`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`

**Status:** `done` — the `identity_status_token` (0x1006) is in the secure SPDP
and in the type `ParticipantBuiltinTopicDataSecure` only, not in the beacon of
`DCPSParticipants`; the runtime sets it from the plugin's `IdentityStatusToken`
(class `DDS:Auth:PKI-DH:1.0`, `ocsp_status` when an OCSP response exists, Table
53) (`aef84a98a0b`, `257d0f8d0e6`). New on the wire: PID 0x1006 in the secure
SPDP to authenticated peers; not measured against Cyclone, Fast DDS and OpenDDS.
Tests green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 QoS as DCPSPublications (RELIABLE)

**Spec:** §7.5.1.6, p. 73 — "The QoS associated with the
DCPSParticipantsSecure builtin Topic shall be the same as for the
DCPSPublications and DCPSSubscriptions builtin Topic. [...] the
DCPSParticipantsSecure has ReliabilityQosPolicy kind set to RELIABLE."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`TOPIC_NAME_DCPS_PARTICIPANTS_SECURE`, `ParticipantBuiltinTopicDataSecure`);
`crates/dcps/src/builtin_subscriber.rs`;
`crates/dcps/src/runtime.rs::push_sedp_events_to_secure_readers`,
`send_secure_spdp`; `crates/discovery/src/spdp.rs::serialize_secure`

**Tests:**
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`,
`crates/dcps/src/runtime.rs::tests::the_secure_discovery_data_reaches_the_secure_readers`,
`crates/discovery/src/spdp.rs::tests::secure_spdp_takes_a_new_number_for_a_change_only`

**Status:** `done` — the topic `DCPSParticipantsSecure` exists with reader, type
and the QoS of `DCPSPublications` (RELIABLE); the secure SPDP writer numbers
every change and does not go to multicast (`1bee936d138`, `054dd0f3a8b`). Tests
green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 SPDPbuiltinParticipantsSecureWriter/-Reader and EntityIds

**Spec:** §7.5.1.6, p. 73 — "The builtin DataWriter for the
DCPSParticipantsSecure Topic shall be referred to as the
SPDPbuiltinParticipantsSecureWriter [...] The RTPS EntityId_t [...] shall be
as specified in 7.5.8."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER`
(l. 594), `SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER` (l. 599)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_secure_reader_layout`

**Status:** done

### §7.5.1.6 Send changes over DCPSParticipantsSecure

**Spec:** §7.5.1.6, p. 73 — "The ParticipantBuiltinTopicData contains
information, such as participant Locators, which may change at run-time. These
changes shall be sent using the DCPSParticipantsSecure builtin Topic."

**Repo:** `crates/discovery/src/spdp.rs::SpdpBeacon::secure_sequence_number`, `serialize_secure`

**Tests:** `crates/discovery/src/spdp.rs::tests::secure_spdp_takes_a_new_number_for_a_change_only`

**Status:** `done` — a change of the participant data takes the next sequence number of the secure writer; the same data sent again keeps its number. HEARTBEAT and repair name the current number. Measured: open (security matrix, Fast DDS).

### §7.5.1.6 Send the deletion over DCPSParticipantsSecure

**Spec:** §7.5.1.6, p. 73 — "The deletion of a DomainParticipant shall also be
sent using the DCPSParticipantsSecure builtin Topic."

**Repo:** `crates/discovery/src/spdp.rs::SpdpBeacon::serialize_secure_departure`; `crates/dcps/src/runtime.rs::announce_departure`, `spdp_departed`

**Tests:** `crates/discovery/src/spdp.rs::tests::secure_spdp_departure_is_a_key_only_change_on_the_secure_writer`

**Status:** `done` — a secured participant sends the departure (`D=0`, `DISPOSED|UNREGISTERED`) on the secure writer to the authenticated participants; a participant takes a departure only from the secure writer of an authenticated participant. Measured: open.

### §7.5.1.6 After authentication, ignore DCPSParticipants changes

**Spec:** §7.5.1.6, pp. 73–74 — "After authentication has completed
successfully a DomainParticipant shall ignore any changes to the
ParticipantBuiltinTopicData (including dispose messages) received on the
DCPSParticipants builtin Topic from the authenticated DomainParticipant."

**Repo:** `crates/discovery/src/spdp.rs::SpdpReader::samples_with_channel`; `crates/dcps/src/runtime.rs::handle_spdp_datagram`, `spdp_plain_is_liveliness_only`

**Tests:** `crates/discovery/src/spdp.rs::tests::samples_name_the_writer_they_came_from`, `crates/discovery/src/spdp.rs::tests::touch_renews_the_lease_of_a_known_participant_only`, `crates/dcps/src/runtime.rs::tests` (handshake dispatch test, last step)

**Status:** `done` — after the authentication a sample of the plain writer only renews the lease and liveliness of the participant; content and departure from the plain writer are ignored, changes come from the secure writer.

### §7.5.1.7 DCPSPublications for non-sensitive topics

**Spec:** §7.5.1.7, p. 74 — "Implementations of the DDS Security shall use that
same DCPSPublications Topic to communicate the DataWriter information for
Topic entities that are not considered sensitive."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — an endpoint of a topic without `enable_discovery_protection` goes over the plain writer (the dispose too, with the topic of the deleted endpoint).

### §7.5.1.7 DCPSPublicationsSecure: topic and endpoints

**Spec:** §7.5.1.7, p. 74 — "Implementations of the DDS Security specification
shall have an additional builtin Topic referred to as DCPSPublicationsSecure
and associated builtin DataReader and DataWriter entities".

**Repo:** `crates/discovery/src/sedp/writer.rs::SedpPublicationsWriter::new_secure`
(l. 91); `crates/discovery/src/sedp/reader.rs::SedpPublicationsReader::new_secure`
(l. 154)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst`

**Status:** done

### §7.5.1.7 The AccessControl plugin decides which topics are sensitive

**Spec:** §7.5.1.7, p. 74 — "The determination of which Topic entities are
considered sensitive shall be specified by the AccessControl plugin."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`, `SharedSecurityGate::topic_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — the selection follows `is_discovery_protected` of the rule of the topic.

### §7.5.1.7 Topic name "DCPSPublicationsSecure"

**Spec:** §7.5.1.7, p. 74 — "The Topic name for the DCPSPublicationsSecure
Topic shall be 'DCPSPublicationsSecure'."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicDataSecure`, `SubscriptionBuiltinTopicDataSecure`,
`data_tags`); `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_are_made_from_the_wire_data`,
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`

**Status:** `done` — the topics `DCPSPublicationsSecure` and
`DCPSSubscriptionsSecure` exist with their types and `data_tags` (0x1003)
(`054dd0f3a8b`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.1.7 PublicationBuiltinTopicDataSecure: data_tags 0x1003

**Spec:** §7.5.1.7, Table 15, p. 74 — "data_tags DataTags PID_DATA_TAGS 0x1003".

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicDataSecure`, `SubscriptionBuiltinTopicDataSecure`,
`data_tags`); `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_are_made_from_the_wire_data`,
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`

**Status:** `done` — the topics `DCPSPublicationsSecure` and
`DCPSSubscriptionsSecure` exist with their types and `data_tags` (0x1003)
(`054dd0f3a8b`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.1.7 QoS as DCPSPublications

**Spec:** §7.5.1.7, p. 74 — "The QoS associated with the DCPSPublicationsSecure
Topic shall be the same as for the DCPSPublications Topic."

**Repo:** `crates/discovery/src/sedp/writer.rs::SedpPublicationsWriter::new_secure`
(l. 91, the same writer type as the plain writer)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst`

**Status:** done

### §7.5.1.7 SEDPbuiltinPublicationsSecureWriter/-Reader and EntityIds

**Spec:** §7.5.1.7, p. 74 — "The builtin DataWriter for the
DCPSPublicationsSecure Topic shall be referred to as the
SEDPbuiltinPublicationsSecureWriter."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER`
(l. 534), `SEDP_BUILTIN_PUBLICATIONS_SECURE_READER` (l. 539)

**Tests:** `crates/rtps/src/wire_types.rs::tests::secure_publications_writer_layout`

**Status:** done

### §7.5.1.8 DCPSSubscriptions for non-sensitive topics

**Spec:** §7.5.1.8, p. 75 — "Implementations of the DDS Security specification
shall use that same DCPSSubscriptions Topic to send the DataReader information
for Topic entities that are not considered sensitive."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — as §7.5.1.7 for subscriptions.

### §7.5.1.8 DCPSSubscriptionsSecure: topic and endpoints

**Spec:** §7.5.1.8, p. 75 — "Implementations of the DDS Security specification
shall have an additional builtin Topic referred to as DCPSSubscriptionsSecure
and associated builtin DataReader and DataWriter entities".

**Repo:** `crates/discovery/src/sedp/writer.rs::SedpSubscriptionsWriter::new_secure`
(l. 273); `crates/discovery/src/sedp/reader.rs::SedpSubscriptionsReader::new_secure`
(l. 317)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst`

**Status:** done

### §7.5.1.8 The AccessControl plugin decides which topics are sensitive

**Spec:** §7.5.1.8, p. 75 — "The determination of which Topic entities are
considered sensitive shall be specified by the AccessControl plugin."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`, `SharedSecurityGate::topic_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — as §7.5.1.7.

### §7.5.1.8 SubscriptionBuiltinTopicDataSecure: data_tags 0x1003

**Spec:** §7.5.1.8, Table 16, p. 75 — "data_tags DataTags PID_DATA_TAGS
0x1003".

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicDataSecure`, `SubscriptionBuiltinTopicDataSecure`,
`data_tags`); `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_are_made_from_the_wire_data`,
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`

**Status:** `done` — the topics `DCPSPublicationsSecure` and
`DCPSSubscriptionsSecure` exist with their types and `data_tags` (0x1003)
(`054dd0f3a8b`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.1.8 QoS as DCPSSubscriptions

**Spec:** §7.5.1.8, p. 75 — "The QoS associated with the
DCPSSubscriptionsSecure Topic shall be the same as for the DCPSSubscriptions
Topic."

**Repo:** `crates/discovery/src/sedp/writer.rs::SedpSubscriptionsWriter::new_secure`
(l. 273)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst`

**Status:** done

### §7.5.1.8 SEDPbuiltinSubscriptionsSecureWriter/-Reader and EntityIds

**Spec:** §7.5.1.8, p. 75 — "The builtin DataWriter for the
DCPSSubscriptionsSecure Topic shall be referred to as the
SEDPbuiltinSubscriptionsSecureWriter."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — the four octets of the table are in the test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` now names the secure WLP writer
`0xff0200c2` as the table does. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.2 DCPSParticipantMessage for non-sensitive topics


**Spec:** §7.5.2, p. 75 — "implementations of the DDS Security specification
shall use the same standard BuiltinParticipantMessageWriter and
BuiltinParticipantMessageReader to communicate liveliness information on Topic
entities that are not considered sensitive."

**Repo:** `crates/dcps/src/wlp.rs::WlpEndpoint::set_channels`,
`crates/dcps/src/runtime.rs::wlp_channels`,
`crates/dcps/src/runtime.rs::protect_wlp_outbound`,
`crates/security-runtime/src/shared.rs::SharedSecurityGate::topic_liveliness_protected`

**Tests:**
`crates/dcps/src/wlp.rs::tests::the_plain_and_the_secure_writer_beat_with_sequence_numbers_of_their_own`,
`crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`,
`crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`

**Status:** `done` — the participant beats on the plain writer for the writers
of its topics that are not sensitive and on the secure writer for those of the
sensitive ones (`is_liveliness_protected` of the rule of the topic); with topics
of both kinds it beats on both, each writer with its own sequence numbers,
history and reader proxies. Without a writer, `liveliness_protection_kind` of
the domain decides. Restored: the choice of `62017f49303` was lost in the merge
`13cb1667912` when the WLP endpoint of wip/dcps-1.4 (a `ReliableWriter` with
reader proxies) was taken; it now sits on that endpoint (`WlpEndpoint` holds the
plain and the secure `ReliableWriter`). The plain WLP DATA is not protected with
the key of the secure writer. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-073220.log`).

### §7.5.2 DCPSParticipantMessageSecure: topic and endpoints

**Spec:** §7.5.2, p. 76 — "Implementations of the DDS Security specification
shall have an additional DCPSParticipantMessageSecure builtin Topic and
associated builtin DataReader and DataWriter entities".

**Repo:** `crates/dcps/src/wlp.rs::WlpEndpoint::set_secure` (l. 156)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`,
`crates/dcps/src/wlp.rs::tests::the_secure_writer_is_answered_by_the_secure_reader`

**Status:** done

### §7.5.2 Data type ParticipantMessageData

**Spec:** §7.5.2, p. 76 — "The data type associated with the
DCPSParticipantMessageSecure Topic shall be the same as the
ParticipantMessageData structure."

**Repo:** `crates/dcps/src/wlp.rs` (the same codec for both writers)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`

**Status:** done

### §7.5.2 QoS as DCPSParticipantMessage

**Spec:** §7.5.2, p. 76 — "The QoS associated with the
DCPSParticipantMessageSecure Topic shall be the same as for the
DCPSParticipantMessage Topic as defined in sub clause 8.4.13 of the DDS
Interoperability Wire Protocol".

**Repo:** `crates/dcps/src/wlp.rs` (reliable sending with a history of the
last 32 beats for both writers)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_secure_writer_is_answered_by_the_secure_reader`,
`crates/dcps/src/wlp.rs::tests::a_heartbeat_announcing_a_pulse_not_received_is_answered_with_a_request_for_it`

**Status:** done

### §7.5.2 BuiltinParticipantMessageSecureWriter/-Reader and EntityIds

**Spec:** §7.5.2, p. 76 — "The builtin DataWriter for the
DCPSParticipantMessageSecure Topic shall be referred to as the
BuiltinParticipantMessageSecureWriter."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER`
(l. 555), `BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER` (l. 560)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`

**Status:** done

### §7.5.2 MANUAL_BY_TOPIC: liveliness HEARTBEAT protected as the writer HEARTBEAT

**Spec:** §7.5.2, p. 76 — "If a DataWriter LivelinessQos policy is
MANUAL_BY_TOPIC_LIVELINESS_QOS, implementations compliant with DDS-Security
shall use the same RTPS DataWriter for the liveliness heartbeats. The
liveliness heartbeats shall be protected using the same means as the regular
DataWriter heartbeats."

**Repo:** `crates/dcps/src/runtime.rs` (protection of the HEARTBEAT submessage)

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_liveliness_heartbeat_of_a_manual_writer_is_protected_like_its_heartbeats`

**Status:** `done` — the HEARTBEAT of the liveliness of a MANUAL_BY_TOPIC writer
is protected like a HEARTBEAT of the writer (`054dd0f3a8b`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.2 Liveliness per topic over both channels

**Spec:** §7.5.2, p. 76 — "It shall use the DCPSParticipantMessage Topic if
is_liveliness_protected is set to false, otherwise it shall use the
DCPSParticipantMessageSecure Topic."

**Repo:** `crates/dcps/src/wlp.rs::WlpEndpoint::set_channels`,
`crates/dcps/src/runtime.rs::wlp_channels`,
`crates/dcps/src/runtime.rs::protect_wlp_outbound`,
`crates/security-runtime/src/shared.rs::SharedSecurityGate::topic_liveliness_protected`

**Tests:**
`crates/dcps/src/wlp.rs::tests::the_plain_and_the_secure_writer_beat_with_sequence_numbers_of_their_own`,
`crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`,
`crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`

**Status:** `done` — the participant beats on the plain writer for the writers
of its topics that are not sensitive and on the secure writer for those of the
sensitive ones (`is_liveliness_protected` of the rule of the topic); with topics
of both kinds it beats on both, each writer with its own sequence numbers,
history and reader proxies. Without a writer, `liveliness_protection_kind` of
the domain decides. Restored: the choice of `62017f49303` was lost in the merge
`13cb1667912` when the WLP endpoint of wip/dcps-1.4 (a `ReliableWriter` with
reader proxies) was taken; it now sits on that endpoint (`WlpEndpoint` holds the
plain and the secure `ReliableWriter`). The plain WLP DATA is not protected with
the key of the secure writer. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-073220.log`).

### §7.5.2 AUTOMATIC/MANUAL_BY_PARTICIPANT: selection by is_liveliness_protected


**Spec:** §7.5.2, p. 76 — "It shall use the DCPSParticipantMessage Topic if
is_liveliness_protected is set to false, otherwise it shall use the
DCPSParticipantMessageSecure Topic."

**Repo:** `crates/dcps/src/wlp.rs::WlpEndpoint::set_channels`,
`crates/dcps/src/runtime.rs::wlp_channels`,
`crates/dcps/src/runtime.rs::protect_wlp_outbound`,
`crates/security-runtime/src/shared.rs::SharedSecurityGate::topic_liveliness_protected`

**Tests:**
`crates/dcps/src/wlp.rs::tests::the_plain_and_the_secure_writer_beat_with_sequence_numbers_of_their_own`,
`crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`,
`crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`

**Status:** `done` — the participant beats on the plain writer for the writers
of its topics that are not sensitive and on the secure writer for those of the
sensitive ones (`is_liveliness_protected` of the rule of the topic); with topics
of both kinds it beats on both, each writer with its own sequence numbers,
history and reader proxies. Without a writer, `liveliness_protection_kind` of
the domain decides. Restored: the choice of `62017f49303` was lost in the merge
`13cb1667912` when the WLP endpoint of wip/dcps-1.4 (a `ReliableWriter` with
reader proxies) was taken; it now sits on that endpoint (`WlpEndpoint` holds the
plain and the secure `ReliableWriter`). The plain WLP DATA is not protected with
the key of the secure writer. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-073220.log`).

### §7.5.3 New DCPSParticipantStatelessMessage builtin Topic

**Spec:** §7.5.3, p. 76 — "this specification introduces a new
DCPSParticipantStatelessMessage builtin Topic and corresponding builtin
DataReader and DataWriter entities".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the requirements follow in
§7.5.3.2 to §7.5.3.6.

### §7.5.3.1 Background: Sequence Number Attacks (non normative)

**Spec:** §7.5.3.1, pp. 76–77 — rationale for stateless endpoints.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — "non normative".

### §7.5.3.2 Stateless endpoints present

**Spec:** §7.5.3.2, p. 77 — "These two endpoints shall be present in compliant
implementations of this specification."

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter`
(l. 52), `StatelessMessageReader` (l. 173)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::writer_has_expected_entity_id`,
`crates/discovery/src/security/stateless.rs::tests::reader_has_expected_entity_id`

**Status:** done

### §7.5.3.2 Writer: Best-Effort StatelessWriter

**Spec:** §7.5.3.2, p. 77 — "The BuiltinParticipantStatelessMessageWriter is an
RTPS Best-Effort StatelessWriter".

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter::write`
(l. 122, one DATA per reader proxy, no HEARTBEAT, no history)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::write_increments_sequence_number`,
`crates/discovery/src/security/stateless.rs::tests::write_to_two_proxies_produces_two_datagrams`

**Status:** done

### §7.5.3.2 Reader: Best-Effort StatelessReader

**Spec:** §7.5.3.2, p. 77 — "The BuiltinParticipantStatelessMessageReader is an
RTPS Best-Effort StatelessReader".

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageReader`
(l. 173, no ACKNACK, no history)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::reader_handle_data_decodes_generic_message`,
`crates/discovery/src/security/stateless.rs::tests::end_to_end_writer_to_reader_loopback`

**Status:** done

### §7.5.3.2 Data type ParticipantStatelessMessage

**Spec:** §7.5.3.2, p. 77 — "typedef ParticipantStatelessMessage
ParticipantGenericMessage;"

**Repo:** `crates/security/src/generic_message.rs::encode_generic_message`
(l. 385), `decode_generic_message` (l. 406)

**Tests:** `crates/discovery/src/security/codec.rs::tests::roundtrip_preserves_message`,
`crates/discovery/src/security/codec.rs::tests::encode_starts_with_cdr_le_encapsulation`

**Status:** done

### §7.5.3.2 EntityIds of the stateless endpoints

**Spec:** §7.5.3.2, p. 77 — "The RTPS EntityId_t associated with the
BuiltinParticipantStatelessMessageWriter and
BuiltinParticipantStatelessMessageReader shall be as specified in 7.5.8."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`
(l. 567), `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER` (l. 572)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::write_carries_writer_entity_id_on_wire`

**Status:** done

### §7.5.3.3 message_identity.source_guid = GUID of the stateless writer

**Spec:** §7.5.3.3, p. 78 — "The source_guid field within the message_identity
shall be set to match the GUID_t of the BuiltinParticipantStatelessMessageWriter
that writes the message."

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter::next_message_identity`; `crates/discovery/src/security/stack.rs::build_message`

**Tests:** `crates/discovery/src/security/stateless.rs::tests::message_identities_are_the_writers_and_rise_by_one`, `crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`

**Status:** `done` — `source_guid` is the GUID of the stateless writer (EntityId `0x000201c3`). Measured: open (security matrix).

### §7.5.3.3 sequence_number starts at 1 and counts per message of the writer

**Spec:** §7.5.3.3, p. 78 — "The sequence_number field within the
message_identity shall start with the value set to one and be incremented for
each different message sent by the BuiltinParticipantStatelessMessageWriter."

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter::next_message_identity`

**Tests:** `crates/discovery/src/security/stateless.rs::tests::message_identities_are_the_writers_and_rise_by_one`

**Status:** `done` — one counter of the writer across all peers, starting at 1; a repeated message keeps its identity.

### §7.5.3.3 related_message_identity

**Spec:** §7.5.3.3, p. 78 — "It shall be set to either the tuple
{GUID_UNKNOWN, 0} if the message is not related to any other message, or else
set to match the message_identity of the related
ParticipantStatelessMessage."

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`
(l. 418, `MessageIdentity::default()` for the request),
`on_stateless_message` (l. 551, `msg.message_identity` for reply and final)

**Tests:** `crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`,
`crates/security/src/generic_message.rs::tests::related_message_identity_links_reply_to_request`

**Status:** done — the receiver tells request and reply apart by
`related_message_identity`; the handshake in the test reaches the shared
secret only with the field set correctly.

### §7.5.3.3 destination_participant_guid

**Spec:** §7.5.3.3, p. 78 — "The destination_participant_guid shall contain
either the value GUID_UNKNOWN [...] or else the GUID_t of the destination
DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.3 destination_endpoint_guid

**Spec:** §7.5.3.3, p. 78 — "It can contain either GUID_UNKNOWN or else the GUID
of a specific endpoint within destination DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.3 message_data per message_class_id

**Spec:** §7.5.3.3, p. 78 — "The contents message_data depend on the value of
the message_class_id".

**Repo:** `crates/discovery/src/security/stack.rs::build_message` (l. 752)

**Tests:** `crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** done

### §7.5.3.4 Implied content filter on destination_participant_guid

**Spec:** §7.5.3.4, p. 78 — "the BuiltinParticipantStatelessMessageReader has
an implied content filter with the logical expression:
'destination_participant_guid == GUID_UNKNOWN || destination_participant_guid
== BuiltinParticipantStatelessMessageReader.participant.guid'".

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_handshake_message_is_taken_only_as_the_spec_addresses_it`

**Status:** `done` — a message whose `destination_participant_guid` names another
participant is not taken. The writer sends every message to all reader
proxies; the filter sits at the reader, where the spec places it. Measured: green on the Linux test host.

### §7.5.3.4 Target endpoint through destination_endpoint_guid

**Spec:** §7.5.3.4, p. 78 — "If the destination_endpoint_guid member is not set
to GUID_UNKNOWN, the message written targets the specific endpoint within the
destination DomainParticipant with a matching Endpoint Key."

**Repo:** `crates/discovery/src/security/stack.rs` (Stateless-Reader),
`crates/dcps/src/runtime.rs::install_crypto_token`, `local_endpoint_named`

**Tests:**
`crates/discovery/src/security/stack.rs::tests::a_stateless_message_for_another_endpoint_is_left_alone`,
`crates/dcps/src/runtime.rs::tests::a_token_for_an_endpoint_this_participant_lacks_is_not_installed`

**Status:** `done` — a message whose `destination_endpoint_guid` is not
GUID_UNKNOWN targets the named endpoint: the stateless reader leaves one for
another endpoint alone, a token for an endpoint this participant does not have
is not installed and recorded as `security.token.not_addressed` (`e493c610d8a`).
New on the wire (receiving): a message for another endpoint is ignored. Tests
green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.3.5 Prefix "dds.sec." reserved for GenericMessageClassId

**Spec:** §7.5.3.5, p. 78 — "This specification, including future versions of
this specification reserves GenericMessageClassId values that start with the
prefix 'dds.sec.'".

**Repo:** `crates/security/src/generic_message.rs::class_id` (l. 63)

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done — ZeroDDS uses only the five values of the specification.

### §7.5.3.5 GMCLASSID_SECURITY_AUTH_REQUEST

**Spec:** §7.5.3.5, p. 79 — "#define GMCLASSID_SECURITY_AUTH_REQUEST
'dds.sec.auth_request'".

**Repo:** `crates/security/src/generic_message.rs::class_id::AUTH_REQUEST`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.3.5 GMCLASSID_SECURITY_AUTH_HANDSHAKE

**Spec:** §7.5.3.5, p. 79 — "#define GMCLASSID_SECURITY_AUTH_HANDSHAKE
'dds.sec.auth'".

**Repo:** `crates/security/src/generic_message.rs::class_id::AUTH`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.3.6 Format of data within ParticipantStatelessMessage

**Spec:** §7.5.3.6, p. 79 — "Each value for the GenericMessageClassId uses
different schema to store data within the generic attributes in the
message_data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction to §7.5.3.6.1 and .2.

### §7.5.3.6.1 AUTH_HANDSHAKE: message_data with one HandshakeMessageToken

**Spec:** §7.5.3.6.1, p. 79 — "the message_data attribute shall contain the
HandshakeMessageTokenSeq containing one element."

**Repo:** `crates/discovery/src/security/stack.rs::build_message` (l. 752,
`message_data: vec![holder]`)

**Tests:** `crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** done

### §7.5.3.6.1 AUTH_HANDSHAKE: destination_participant_guid = destination participant

**Spec:** §7.5.3.6.1, p. 79 — "The destination_participant_guid shall be set to
the GUID_t of the destination DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.1 AUTH_HANDSHAKE: destination_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.3.6.1, p. 79 — "The destination_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.1 AUTH_HANDSHAKE: source_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.3.6.1, p. 79 — "The source_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.3/§7.5.3.6 — field values without a test

**Spec:** §7.5.3.3 and §7.5.3.6.1, pp. 78–79 — the four fields listed as
`partial` above.

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.2 AUTH_REQUEST: message_data with one AuthRequestMessageToken

**Spec:** §7.5.3.6.2, p. 79 — "If GenericMessageClassId is
GMCLASSID_SECURITY_AUTH_REQUEST the message_data attribute shall contain an
AuthRequestMessageTokenSeq containing one element."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.2 AUTH_REQUEST: destination_participant_guid

**Spec:** §7.5.3.6.2, p. 79 — "The destination_participant_guid shall be set to
the GUID_t of the destination DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.2 AUTH_REQUEST: destination_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.3.6.2, p. 79 — "The destination_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.2 AUTH_REQUEST: source_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.3.6.2, p. 79 — "The source_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
and `message_data` of AUTH_REQUEST and AUTH_HANDSHAKE have the values of the
spec; the test checks them on sent messages (`368e8ed28f0`). ZeroDDS sends the
AUTH_REQUEST message. Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.4.1 Background (Non-Normative)

**Spec:** §7.5.4.1, pp. 79–80 — rationale for a reliable, secure and volatile
channel for the key exchange.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the heading says "Non-Normative".

### §7.5.4.2 VolatileMessageSecure endpoints present

**Spec:** §7.5.4.2, p. 80 — "These two endpoints shall be present in compliant
implementations of this specification."

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`
(l. 56), `VolatileSecureMessageReader` (l. 177)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::writer_has_expected_entity_id`,
`crates/discovery/src/security/volatile_secure.rs::tests::reader_has_expected_entity_id`

**Status:** done

### §7.5.4.2 Table 17 — TopicSecurityConfig of ParticipantVolatileMessageSecure

**Spec:** §7.5.4.2, Table 17, p. 80 — `is_read_protected false`,
`is_write_protected false`, `is_discovery_protected false`,
`is_liveliness_protected false`.

**Repo:** `crates/security-runtime/src/builtin_attributes.rs`
(`builtin_endpoint_security_attributes`, `builtin_endpoint_kinds`)

**Tests:**
`crates/security-runtime/src/builtin_attributes.rs::tests::table_17_has_the_values_of_the_spec`

**Status:** `done` — the `TopicSecurityConfig` of the
`ParticipantVolatileMessageSecure` has the values of Table 17 (`054dd0f3a8b`).
Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.4.2 Table 18 — EndpointSecurityConfig: only is_submessage_protected

**Spec:** §7.5.4.2, Table 18, p. 80 — `is_submessage_protected true`, all
others `false`.

**Repo:** `crates/dcps/src/runtime.rs::protect_volatile_outbound` (l. 21887,
every DATA submessage as SEC_PREFIX/SEC_BODY/SEC_POSTFIX with the Kx key; the
payload itself stays unchanged)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/dcps/src/runtime.rs::tests::handshake_completes_through_runtime_dispatch_e2e`

**Status:** done

### §7.5.4.2 Writer: Reliable StatefulWriter

**Spec:** §7.5.4.2, p. 80 — "The BuiltinParticipantVolatileMessageSecureWriter is
an RTPS Reliable StatefulWriter".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`
(l. 56, `ReliableWriter`)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::write_with_heartbeat_piggybacks_heartbeat_submessage`

**Status:** done

### §7.5.4.2 Table 19 — writer RELIABILITY RELIABLE

**Spec:** §7.5.4.2, Table 19, p. 80 — "RELIABILITY kind= RELIABLE".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`
(l. 56)

**Tests:** `crates/discovery/src/security/stack.rs::tests::end_to_end_volatile_secure_handshake_via_reliable_loop`

**Status:** done

### §7.5.4.2 Table 19 — writer HISTORY KEEP_ALL

**Spec:** §7.5.4.2, Table 19, p. 80 — "HISTORY kind= KEEP_ALL".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::identity_is_the_writers_and_a_late_reader_gets_no_old_message`

**Status:** `done` — HISTORY KEEP_ALL (limit 1024; a write beyond it fails).

### §7.5.4.2 Table 19 — writer DURABILITY VOLATILE

**Spec:** §7.5.4.2, Table 19, p. 80 — "DURABILITY kind= VOLATILE".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`::add_reader_proxy

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::identity_is_the_writers_and_a_late_reader_gets_no_old_message`

**Status:** `done` — a reader matched later skips earlier messages, acknowledged ones are removed.

### §7.5.4.2 Reader: Reliable StatefulReader

**Spec:** §7.5.4.2, p. 81 — "The BuiltinParticipantVolatileMessageSecureReader is
an RTPS Reliable StatefulReader".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageReader`
(l. 177, `ReliableReader`)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::reader_decodes_data_with_known_writer`,
`crates/discovery/src/security/volatile_secure.rs::tests::reader_drops_data_from_unknown_writer`

**Status:** done

### §7.5.4.2 Table 20 — reader RELIABILITY RELIABLE

**Spec:** §7.5.4.2, Table 20, p. 81 — "RELIABILITY kind= RELIABLE".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageReader`
(l. 177)

**Tests:** `crates/discovery/src/security/stack.rs::tests::end_to_end_volatile_secure_handshake_via_reliable_loop`

**Status:** done

### §7.5.4.2 Table 20 — reader HISTORY KEEP_ALL

**Spec:** §7.5.4.2, Table 20, p. 81 — "HISTORY kind= KEEP_ALL".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VOLATILE_SECURE_READER_CAPACITY`
(l. 52, 64 samples per writer proxy until delivery)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::reader_decodes_data_with_known_writer`

**Status:** done — the reader hands every sample in order to the caller and
keeps no history; the bound of 64 applies to samples that cannot be delivered
yet.

### §7.5.4.2 Table 20 — reader DURABILITY VOLATILE

**Spec:** §7.5.4.2, Table 20, p. 81 — "DURABILITY kind= VOLATILE".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageReader`
(l. 177)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::reader_decodes_data_with_known_writer`

**Status:** done — the reader requests no samples before the first HEARTBEAT.

### §7.5.4.2 Data type ParticipantVolatileMessageSecure

**Spec:** §7.5.4.2, p. 81 — "typedef ParticipantVolatileMessageSecure
ParticipantGenericMessage;"

**Repo:** `crates/security/src/generic_message.rs::encode_generic_message`
(l. 385)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::reader_decodes_data_with_known_writer`

**Status:** done

### §7.5.4.2 EntityIds of the VolatileMessageSecure endpoints

**Spec:** §7.5.4.2, p. 81 — "The RTPS EntityId_t [...] shall be as specified in
7.5.8."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER`
(l. 579), `BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER` (l. 587)

**Tests:** `crates/rtps/src/wire_types.rs::tests::volatile_message_secure_entity_ids_match_spec_and_cyclone`

**Status:** done

### §7.5.4.3 destination_participant_guid

**Spec:** §7.5.4.3, p. 81 — "The destination_participant_guid shall contain
either the value GUID_UNKNOWN [...] or else the GUID_t of the destination
DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.3 message_identity.source_guid = GUID of the volatile writer

**Spec:** §7.5.4.3, p. 81 — "The source_guid field within the message_identity
shall be set to match the GUID_t of the
BuiltinParticipantVolatileMessageSecureWriter that writes the message."

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`::with_identity

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::identity_is_the_writers_and_a_late_reader_gets_no_old_message`

**Status:** `done` — the writer sets `source_guid` to its GUID (EntityId `0xff0202c3`), whatever the caller put there.

### §7.5.4.3 sequence_number starts at 1 and counts

**Spec:** §7.5.4.3, p. 81 — "The sequence_number field within the
message_identity shall start with the value set to one and be incremented for
each different message sent by the
BuiltinParticipantVolatileMessageSecureWriter."

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`::with_identity

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::identity_is_the_writers_and_a_late_reader_gets_no_old_message`

**Status:** `done` — the number starts at 1 and rises per message.

### §7.5.4.3 related_message_identity

**Spec:** §7.5.4.3, p. 81 — "It shall be set to either the tuple
{GUID_UNKNOWN, 0} if the message is not related to any other message".

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.3 message_data per message_class_id

**Spec:** §7.5.4.3, p. 81 — "The contents message_data depend on the value of
the message_class_id".

**Repo:** `crates/dcps/src/runtime.rs::build_crypto_token_message` (l. 21338)

**Tests:** `crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done

### §7.5.4.4 Implied content filter on destination_participant_guid

**Spec:** §7.5.4.4, pp. 81–82 — "the BuiltinParticipantVolatileMessageSecureReader
has an implied content filter with the logical expression:
'destination_participant_guid == GUID_UNKNOWN ||
destination_participant_guid==BuiltinParticipantVolatileMessageSecureReader.participant.guid'".

**Repo:** `crates/discovery/src/security/volatile_secure.rs` (`destination_of`,
`is_addressed_to`); `crates/rtps/src/history_cache.rs` (`CacheChange::destination`);
`crates/rtps/src/reliable_writer.rs` (`write_with_heartbeat_to`)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::a_change_for_one_participant_reaches_the_others_as_a_gap`,
`crates/discovery/src/security/volatile_secure.rs::tests::writer_sends_a_directed_message_to_its_destination_only`,
`crates/discovery/src/security/volatile_secure.rs::tests::the_implied_content_filter`,
`crates/discovery/src/security/volatile_secure.rs::tests::reader_does_not_take_a_message_for_another_participant`

**Status:** `done` — the reader takes only messages for its participant and that
participant's endpoints and counts the others. The writer gives every other
reader a GAP for the number of a directed message, on the first send and on
every repair; before, every reader got every token message. Measured: green on
the Linux test host; the security matrix against Cyclone, Fast DDS and
OpenDDS is unchanged.

### §7.5.4.4 Target endpoint through destination_endpoint_guid

**Spec:** §7.5.4.4, p. 82 — "If the destination_endpoint_guid member is not set
to GUID_UNKNOWN the message written targets a specific endpoint within the
destination DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (Stateless-Reader),
`crates/dcps/src/runtime.rs::install_crypto_token`, `local_endpoint_named`

**Tests:**
`crates/discovery/src/security/stack.rs::tests::a_stateless_message_for_another_endpoint_is_left_alone`,
`crates/dcps/src/runtime.rs::tests::a_token_for_an_endpoint_this_participant_lacks_is_not_installed`

**Status:** `done` — a message whose `destination_endpoint_guid` is not
GUID_UNKNOWN targets the named endpoint: the stateless reader leaves one for
another endpoint alone, a token for an endpoint this participant does not have
is not installed and recorded as `security.token.not_addressed` (`e493c610d8a`).
New on the wire (receiving): a message for another endpoint is ignored. Tests
green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.5 Prefix "dds.sec." reserved

**Spec:** §7.5.4.5, p. 82 — "This specification [...] reserves
GenericMessageClassId values that start with the prefix 'dds.sec.'".

**Repo:** `crates/security/src/generic_message.rs::class_id` (l. 63)

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.4.5 GMCLASSID_SECURITY_PARTICIPANT_CRYPTO_TOKENS

**Spec:** §7.5.4.5, p. 82 — "'dds.sec.participant_crypto_tokens'".

**Repo:** `crates/security/src/generic_message.rs::class_id::PARTICIPANT_CRYPTO_TOKENS`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.4.5 GMCLASSID_SECURITY_DATAWRITER_CRYPTO_TOKENS

**Spec:** §7.5.4.5, p. 82 — "'dds.sec.datawriter_crypto_tokens'".

**Repo:** `crates/security/src/generic_message.rs::class_id::DATAWRITER_CRYPTO_TOKENS`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.4.5 GMCLASSID_SECURITY_DATAREADER_CRYPTO_TOKENS

**Spec:** §7.5.4.5, p. 82 — "'dds.sec.datareader_crypto_tokens'".

**Repo:** `crates/security/src/generic_message.rs::class_id::DATAREADER_CRYPTO_TOKENS`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.4.6 Format of data within ParticipantVolatileMessageSecure

**Spec:** §7.5.4.6, p. 82 — "Each value for the GenericMessageClassId uses
different schema".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §7.5.4.6.1 PARTICIPANT_CRYPTO_TOKENS: message_data

**Spec:** §7.5.4.6.1, p. 82 — "the message_data attribute shall contain the
ParticipantCryptoTokenSeq."

**Repo:** `crates/dcps/src/runtime.rs::build_crypto_token_message` (l. 21338)

**Tests:** `crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done

### §7.5.4.6.1 PARTICIPANT_CRYPTO_TOKENS: destination_participant_guid

**Spec:** §7.5.4.6.1, p. 82 — "The destination_participant_guid shall be set to
the GUID_t of the destination DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.1 PARTICIPANT_CRYPTO_TOKENS: destination_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.4.6.1, p. 82 — "The destination_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.1 PARTICIPANT_CRYPTO_TOKENS: source_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.4.6.1, p. 82 — "The source_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.2 DATAWRITER_CRYPTO_TOKENS: message_data

**Spec:** §7.5.4.6.2, p. 83 — "the message_data shall contain the
DatawriterCryptoTokenSeq."

**Repo:** `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`
(l. 21431)

**Tests:** `crates/dcps/src/runtime.rs::tests::pending_endpoint_tokens_keeps_late_user_token_after_builtins_sent`

**Status:** done

### §7.5.4.6.2 DATAWRITER_CRYPTO_TOKENS: destination_endpoint_guid = DataReader

**Spec:** §7.5.4.6.2, p. 83 — "The destination_endpoint_guid shall be set to the
GUID_t of the DataReader that should receive the CryptoToken values in the
message."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.2 DATAWRITER_CRYPTO_TOKENS: source_endpoint_guid = DataWriter

**Spec:** §7.5.4.6.2, p. 83 — "The source_endpoint_guid shall be set to the
GUID_t of the DataWriter that will be using the CryptoToken values to encode
the data it sends to the DataReader."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.3 DATAREADER_CRYPTO_TOKENS: message_data

**Spec:** §7.5.4.6.3, p. 83 — "the message_data attribute shall contain the
DatareaderCryptoTokenSeq."

**Repo:** `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`
(l. 21431)

**Tests:** `crates/dcps/src/runtime.rs::tests::pending_endpoint_tokens_keeps_late_user_token_after_builtins_sent`

**Status:** done

### §7.5.4.6.3 DATAREADER_CRYPTO_TOKENS: destination_endpoint_guid = DataWriter

**Spec:** §7.5.4.6.3, p. 83 — "The destination_endpoint_guid shall be set to the
GUID_t of the DataWriter that should receive the CryptoToken values".

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.3 DATAREADER_CRYPTO_TOKENS: source_endpoint_guid = DataReader

**Spec:** §7.5.4.6.3, p. 83 — "The source_endpoint_guid shall be set to the
GUID_t of the DataReader that will be using the CryptoToken values".

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4 — field values without a test

**Spec:** §7.5.4.3, §7.5.4.4 and §7.5.4.6, pp. 81–83 — the fields listed as
`partial` above.

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — the fields of the volatile messages
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) have the values of the spec
for each token class, and the tokens are assigned by source and destination when
received; the test checks sending and assignment (`368e8ed28f0`). Test green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.5.1 Background (TypeLookup)

**Spec:** §7.5.5.1, pp. 83–84 — "Compliance with DDS-XTYPES requires any
DomainParticipant that implements the TypeLookup service, to respond to
requests for any TypeIdentifier that the DomainParticipant announces".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — restatement of DDS-XTYPES; its coverage
stands in `dds-xtypes-1.3.md`.

### §7.5.5.2 Four secure TypeLookup endpoints

**Spec:** §7.5.5.2, p. 84 — "These consist of four new endpoints:
TypeLookupServiceRequestSecureWriter, TypeLookupServiceRequestSecureReader,
TypeLookupServiceReplySecureWriter, and TypeLookupServiceReplySecureReader."

**Repo:**
`crates/dcps/src/type_lookup_transport.rs::TypeLookupTransport::new_secure`;
`crates/rtps/src/wire_types.rs`

**Tests:**
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_its_own_endpoints`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_the_qos_of_the_plain_one`

**Status:** `done` — four secure endpoints with the EntityIds of Table 9 and the
same data types and QoS as the plain ones (`a542c9f545d`, `14e1984914d`). Tests
green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.5.2 EntityIds of the secure TypeLookup endpoints

**Spec:** §7.5.5.2, p. 84 — "The EntityIds for the Secure Type Lookup builtin
endpoints are defined in Table 9."

**Repo:**
`crates/dcps/src/type_lookup_transport.rs::TypeLookupTransport::new_secure`;
`crates/rtps/src/wire_types.rs`

**Tests:**
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_its_own_endpoints`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_the_qos_of_the_plain_one`

**Status:** `done` — four secure endpoints with the EntityIds of Table 9 and the
same data types and QoS as the plain ones (`a542c9f545d`, `14e1984914d`). Tests
green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.5.2 Data types and QoS as the non-secure endpoints

**Spec:** §7.5.5.2, p. 84 — "The data types and Qos policies of the Secure Type
Lookup builtin shall be the same defined for the corresponding (non-secure)
endpoint in DDS-XTYPES."

**Repo:**
`crates/dcps/src/type_lookup_transport.rs::TypeLookupTransport::new_secure`;
`crates/rtps/src/wire_types.rs`

**Tests:**
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_its_own_endpoints`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_the_qos_of_the_plain_one`

**Status:** `done` — four secure endpoints with the EntityIds of Table 9 and the
same data types and QoS as the plain ones (`a542c9f545d`, `14e1984914d`). Tests
green on the Linux test host (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.6 Definition of the Types a DDS Endpoint depends on

**Spec:** §7.5.6, pp. 84–85 — "the collection consisting of the top-level type
as well as the transitive closure that includes the types that the top-level
type depends [...] shall be referred to as the types the endpoint depends on."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition for §7.5.5.1.

### §7.5.7 Definition of the "RTPS Bootstrapping Messages"

**Spec:** §7.5.7, p. 85 — a message is an "RTPS Bootstrapping Message" if it
contains submessages for DCPSParticipants, DCPSParticipantStatelessMessage or
DCPSParticipantVolatileMessageSecure, or is not intended for a
DomainParticipant.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition; the requirement stands in
§7.5.12.

### §7.5.8 Definition of the "Builtin Secure Endpoints"

**Spec:** §7.5.8, p. 85 — list of the fourteen Builtin Secure Endpoints.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition; the endpoints are listed
individually in §7.4.7.1 and §7.5.1 to §7.5.5.

### §7.5.9 Definition of the "Builtin Secure Discovery Endpoints"

**Spec:** §7.5.9, p. 85 — the six secure SPDP and SEDP endpoints.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition.

### §7.5.10 Definition of the "Builtin Secure Liveliness Endpoints"

**Spec:** §7.5.10, p. 86 — BuiltinParticipantMessageSecureWriter and -Reader.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition.

### §7.5.11 Definition of the "Builtin Secure TypeLookup Endpoints"

**Spec:** §7.5.11, p. 86 — the four secure TypeLookup endpoints.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — definition.

### §7.5.12 Bootstrapping messages without other topics

**Spec:** §7.5.12, p. 86 — "RTPS Bootstrapping messages (see 7.5.7) shall not
contain submessages for any other (non-bootstrapping) builtin topic or
application-defined topic."

**Repo:** `crates/discovery/src/spdp.rs`,
`crates/discovery/src/security/stateless.rs`,
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/discovery/src/spdp.rs::tests::an_spdp_datagram_holds_the_submessages_of_its_writer_only`,
`crates/discovery/src/security/stateless.rs::tests::a_stateless_datagram_holds_the_submessages_of_its_writer_only`,
`crates/discovery/src/security/volatile_secure.rs::tests::a_volatile_datagram_holds_the_submessages_of_its_writer_only`

**Status:** `done` — a datagram of the bootstrapping messages holds submessages
of its writer only (`054dd0f3a8b`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.13 get_datawriter_sec_attributes/get_datareader_sec_attributes for the Builtin Secure Endpoints

**Spec:** §7.5.13, p. 86 — "the middleware shall call the operations
get_datawriter_security_config and get_datareader_security_config on the
AccessControl interface to obtain the EndpointSecurityConfig associated with
DataReader and DataWriter entities on all the 'Builtin Secure Endpoints'."

**Repo:** `crates/security-runtime/src/builtin_attributes.rs`
(`builtin_endpoint_security_attributes`, `builtin_endpoint_kinds`);
`crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`

**Tests:**
`crates/security-runtime/src/builtin_attributes.rs::tests::table_21_has_the_values_of_the_spec`,
`crates/security-runtime/src/builtin_attributes.rs::tests::the_kinds_follow_table_21`,
`crates/dcps/src/runtime.rs::tests::the_secure_type_lookup_service_is_protected_per_submessage`

**Status:** `done` — the attributes of the secure builtin endpoints follow Table
21 and are determined in one place; the runtime protects accordingly, the
TypeLookup endpoints too (`054dd0f3a8b`, `ee7183c45df`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.13 Table 21 — DCPSParticipantSecure, DCPSPublicationsSecure, DCPSSubscriptionsSecure

**Spec:** §7.5.13, Table 21, pp. 86–87 — `is_read_protected false`,
`is_write_protected false`, `is_submessage_protected` = "Set to match
ParticipantSecurityConfig is_discovery_protected", `is_payload_protected
false`, `is_key_protected false`.

**Repo:** `crates/dcps/src/runtime.rs::protect_secure_spdp` (l. 17028,
protection only with `discovery_protection_kind != NONE`); secure SEDP
submessages likewise

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_discovery_protection`

**Status:** done

### §7.5.13 Table 21 — DCPSParticipantMessageSecure

**Spec:** §7.5.13, Table 21, pp. 86–87 — `is_submessage_protected` = "Set to
match ParticipantSecurityConfig is_liveliness_protected", otherwise `false`.

**Repo:** `crates/dcps/src/runtime.rs` (l. 18036, secure WLP writer with
`liveliness_protection_kind != NONE`)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_liveliness_protection`

**Status:** done

### §7.5.13 Table 21 — DCPSParticipantStatelessMessage

**Spec:** §7.5.13, Table 21, pp. 86–87 — all attributes `false`.

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter::write`
(l. 122, plain DATA)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::write_carries_writer_entity_id_on_wire`

**Status:** done

### §7.5.13 Table 21 — DCPSParticipantVolatileMessageSecure, TypeLookupServiceRequestSecure, TypeLookupServiceReplySecure

**Spec:** §7.5.13, Table 21, pp. 86–87 — `is_submessage_protected true`,
otherwise `false`.

**Repo:** `crates/security-runtime/src/builtin_attributes.rs`
(`builtin_endpoint_security_attributes`, `builtin_endpoint_kinds`);
`crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`

**Tests:**
`crates/security-runtime/src/builtin_attributes.rs::tests::table_21_has_the_values_of_the_spec`,
`crates/security-runtime/src/builtin_attributes.rs::tests::the_kinds_follow_table_21`,
`crates/dcps/src/runtime.rs::tests::the_secure_type_lookup_service_is_protected_per_submessage`

**Status:** `done` — the attributes of the secure builtin endpoints follow Table
21 and are determined in one place; the runtime protects accordingly, the
TypeLookup endpoints too (`054dd0f3a8b`, `ee7183c45df`). Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.13 Builtin Secure Endpoints under is_access_protected

**Spec:** §7.5.13, p. 87 — "they are still protected by the access control
mechanism imposed by the DomainParticipant. That is, if
ParticipantSecurityConfig member is_access_protected is true, then access to
the secure builtin topics is protected."

**Repo:** `crates/dcps/src/runtime.rs` (admission of an authenticated peer after
the join access control)

**Tests:**
`crates/dcps/src/runtime.rs::tests::a_peer_the_join_access_control_does_not_admit_gets_no_access`

**Status:** `done` — an authenticated peer that the join access control of the
domain does not admit gets no access to the secure builtin topics
(`054dd0f3a8b`). Test green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

---

## §8 Common Cryptographic Algorithms

### §8 Three representations of an algorithm

**Spec:** §8, p. 89 — "The specification uses three identifier
representations: A CryptoAlgorithmName (string) [...] A CryptoAlgorithmId
(binary) [...] A CryptoAlgorithmBit (bit)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the types stand in §7.3.6 to
§7.3.9, the values in Tables 22, 25 and 26.

### §8 Several names of one algorithm are equivalent

**Spec:** §8, p. 89 — "In the case where multiple string identifiers are
provided for the same algorithm, they shall all be treated as equivalent."

**Repo:** `crates/security-pki/src/plugin.rs::DsignScheme::from_name`, `kx_suite_for_algo`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_algorithm_names_of_1_2_and_1_1_are_taken`, `crates/security-pki/src/plugin.rs::tests::the_algorithm_names_follow_the_version_of_the_peer_and_the_setting`

**Status:** `done` — Names of 1.2 and the names of 1.1 marked "deprecated in v 1.2" are treated alike on receipt (Table 25, Table 26). The default sent is the name of the table; `auto` (names of 1.2 only to a peer whose IdentityToken names 1.2) and `1.1` are named settings (`org.zerodds.sec.algorithm_names`, `ZERODDS_SEC_ALGORITHM_NAMES`). The specification does not make the sent spelling depend on the peer version, so `auto` is not the default. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.1 Symmetric Cipher AEAD and MAC Algorithms

**Spec:** §8.1, p. 89 — "SPIs may use symmetric cipher algorithms for two
purposes: Authenticated Encryption with Additional Data (AEAD) [...] Message
Authentication Codes."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the algorithms follow in
Table 22.

### §8.1 Table 22 — AES128+GMAC, id 0x01, bit 0

**Spec:** §8.1, Table 22, p. 90 — "AES128+ GCM 0x01 0x0001 << 0 Message
authentication codes (MACs) computed using Galois MAC (AES-GMAC). [...]
specialized to 128-bit AES keys with 96-bit Initialization Vector."

**Repo:** `crates/security-crypto/src/suite.rs::Suite::Aes128Gmac`

**Tests:** `crates/security-crypto/src/suite.rs::tests::aes128_gmac_uses_spec_id_1`, `crates/security-crypto/src/plugin.rs::tests::gmac_only_suite_roundtrip_without_encryption`, `crates/security-crypto/src/plugin.rs::tests::an_aes128_gmac_key_with_a_short_salt_crosses_instances`

**Status:** `done` — Id 0x01 is AES128-GMAC with a 16-octet key (tag over the data as AAD, AES-GCM over an empty plaintext). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.1 Table 22 — AES128+GCM, id 0x02, bit 0

**Spec:** §8.1, Table 22, p. 90 — "AES128+GCM 0x02 0x0001 << 0 Authenticated
Encryption with Additional Data (AEAD) using Advanced Encryption Standard (AES)
in Galois Counter Mode".

**Repo:** `crates/security-crypto/src/suite.rs::Suite::Aes128Gcm` (l. 61,
16-octet key), `transform_kind::AES128_GCM` (l. 38)

**Tests:** `crates/security-crypto/src/suite.rs::tests::aes128_gcm_uses_spec_id_2`,
`crates/security-crypto/src/plugin.rs::tests::encrypt_decrypt_roundtrip`

**Status:** done

### §8.1 Table 22 — AES256+GMAC, id 0x03, bit 1

**Spec:** §8.1, Table 22, p. 90 — "AES256+ GCM 0x03 0x0001 << 1 Message
authentication codes (MACs) computed using Galois MAC (AES-GMAC)".

**Repo:** `crates/security-crypto/src/suite.rs::Suite::Aes256Gmac` (l. 61);
`crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone` (l. 278,
branch without encryption)

**Tests:** `crates/security-crypto/src/suite.rs::tests::from_id_roundtrip_for_all_supported`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** done

### §8.1 Table 22 — AES256+GCM, id 0x04, bit 1

**Spec:** §8.1, Table 22, p. 90 — "AES256+GCM 0x04 0x0001 << 1 Authenticated
Encryption with Additional Data (AEAD) using Advanced Encryption Standard (AES)
in Galois Counter Mode".

**Repo:** `crates/security-crypto/src/suite.rs::Suite::Aes256Gcm` (l. 61,
32-octet key)

**Tests:** `crates/security-crypto/src/suite.rs::tests::aes256_gcm_uses_spec_id_4`,
`crates/security-crypto/src/plugin.rs::tests::aes256_encrypt_decrypt_roundtrip`

**Status:** done

### §8.1 Same bit and same name for the AEAD and the MAC variant

**Spec:** §8.1, p. 90 — "the same value of the CryptoAlgorithmName and
CryptoAlgorithmBit is used for the message-authentication-only (MAC-only)
variants of the corresponding AEAD algorithms."

**Repo:** `crates/rtps/src/security_algo_info.rs::symmetric_bit` (l. 53, one
bit each for AES128 and AES256)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::spec_bit_constants_match`

**Status:** done

### §8.1 Constants CNAME_AES*

**Spec:** §8.1, p. 91 — "const string CNAME_AES128_GMAC = 'AES128+GCM'; const
string CNAME_AES128_GCM = 'AES128+GCM'; const string CNAME_AES256_GMAC =
'AES256+GCM'; const string CNAME_AES256_GCM = 'AES256+GCM';"

**Repo:** `crates/security/src/algorithm.rs::CNAME_AES128_GMAC`, `CNAME_AES128_GCM`, `CNAME_AES256_GMAC`, `CNAME_AES256_GCM`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — the strings of the spec (`AES128+GCM`, `AES256+GCM`; the GMAC constants have the same value as the GCM ones).

### §8.1 Constants CID_INVALID and CID_AES*

**Spec:** §8.1, p. 91 — "const CryptoAlgorithmId CID_INVALID = 0x00; [...]
CID_AES128_GMAC = 0x01; CID_AES128_GCM = 0x02; CID_AES256_GMAC = 0x03;
CID_AES256_GCM = 0x04;"

**Repo:** `crates/security-crypto/src/suite.rs::transform_kind` (l. 38,
`NONE`, `AES128_GMAC`, `AES128_GCM`, `AES256_GMAC`, `AES256_GCM`)

**Tests:** `crates/security-crypto/src/suite.rs::tests::spec_constants_match_table_79`

**Status:** done — the constants carry other names (`transform_kind::*`;
naming).

### §8.1 Constants CBIT_AES*

**Spec:** §8.1, p. 91 — "const CryptoAlgorithmBit CBIT_AES128_GMAC = 1 << 0;
CBIT_AES128_GCM = 1 << 0; CBIT_AES256_GMAC = 1 << 1; CBIT_AES256_GCM = 1 <<
1;"

**Repo:** `crates/rtps/src/security_algo_info.rs::symmetric_bit` (l. 53)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::spec_bit_constants_match`

**Status:** done — `AES128 = 1 << 0`, `AES256 = 1 << 1`.

### §8.1.1 Table 23 — AES-GCM inputs K, P, AAD, IV

**Spec:** §8.1.1, Table 23, p. 91 — "K The 128-bit key [...] or the 256-bit
key [...] P The plaintext [...] AAD Additional Authenticated Data [...] IV
Initialization Vector. This is a 96-bit NONCE that shall not be repeated for
the same key."

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(l. 278, nonce = `session_id` (4) and a running counter (8))

**Tests:** `crates/security-crypto/src/plugin.rs::tests::two_encrypts_produce_different_ciphertexts`,
`crates/security-crypto/src/plugin.rs::tests::encrypts_remaining_decrements_per_call`

**Status:** done — the counter is incremented per seal; the cipher itself
comes from `ring` or `aws-lc-rs`.

### §8.1.1 Table 24 — AES-GCM outputs C and T

**Spec:** §8.1.1, Table 24, pp. 91–92 — "C Ciphertext [...] T Authentication
Tag. This is a Message Authentication Code (MAC) that provides authentication
for the Ciphertext (C) and the Additional Authenticated Data (AAD)."

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(l. 278, ciphertext and 16-octet tag apart)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/security-crypto/src/plugin.rs::tests::decrypt_rejects_tampered_ciphertext`

**Status:** done

### §8.1.1 Counter blocks from the IV with inc32

**Spec:** §8.1.1, p. 92 — "The initial value of the 128-bit counter block is a
128-bit string containing the IV as the leading 96 bits and zeros the
remaining right-most 32 bits. Incremental values [...] are obtained using the
'inc32' function".

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(l. 278, `Nonce::assume_unique_for_key` with 96 bits)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::encrypt_decrypt_roundtrip`

**Status:** done — the counter blocks are formed by the AES-GCM implementation
of `ring`/`aws-lc-rs`; there is no test vector after NIST SP 800-38D in the
repository.

### §8.1.1 AES-GMAC as AES-GCM with an empty plaintext

**Spec:** §8.1.1, p. 92 — "T = AES-GMAC(K, AAD, IV) = AES-GCM(K, '', AAD, IV)".

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(l. 278, GMAC branch: payload as AAD, empty plaintext)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** done — for AES256-GMAC only; AES128-GMAC see Table 22.

### §8.2 Digital Signature Algorithms

**Spec:** §8.2, p. 92 — "SPIs may use digital signature algorithms for
signing/validating identity-type certificates and attestation documents."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the algorithms follow in
Table 25.

### §8.2 Table 25 — RSASSA-PSS-MGF1SHA256+2048+SHA256, id 0x10, bit 0

**Spec:** §8.2, Table 25, p. 92 — "The digital signature shall be computed
using the RSASSA-PSS algorithm specified in PKCS #1 [...] using SHA256 as hash
function, and MGF1 with SHA256 [...] The length of the salt is not specified.
[...] The validation of the signature shall detect the salt length from the
signature."

**Repo:** `crates/security-crypto/src/rsa_pss.rs::verify_rsassa_pss_sha256`; `crates/security-pki/src/plugin.rs::verify_signature_with_cert`; `crates/security-permissions/src/cms.rs`

**Tests:** `crates/security-crypto/src/rsa_pss.rs::tests::the_salt_length_is_read_from_the_signature`, `crates/security-pki/src/plugin.rs::tests::an_rsa_pss_handshake_signature_of_any_salt_length_verifies`, `crates/security-permissions/tests/cms_verify.rs::rsa_pss_signers_of_any_salt_length_are_accepted`

**Status:** `done` — Verification reads the salt length from the signature (OpenSSL 4.0.3 vectors with 0, 8, 20, 32, 64 and 222 octets); the handshake and the CMS verification use it. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.2 RSASSA-PSS: detect the salt length from the signature

**Spec:** §8.2, Table 25, p. 92 — "The validation of the signature shall detect
the salt length from the signature."

**Repo:** `crates/security-crypto/src/rsa_pss.rs`

**Tests:** `crates/security-crypto/src/rsa_pss.rs::tests::the_salt_length_is_read_from_the_signature`, `crates/security-crypto/src/rsa_pss.rs::tests::a_3072_bit_key_verifies_signatures_of_any_salt_length`, `crates/security-crypto/src/rsa_pss.rs::tests::what_does_not_verify_is_refused`

**Status:** `done` — `webpki` verifies 32 octets of salt only; the own verification per RFC 8017 §9.1.2 reads the length from `DB`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.2 Table 25 — RSASSA-PKCS1-V1_5+2048+SHA256, id 0x11, bit 1

**Spec:** §8.2, Table 25, pp. 92–93 — "The digital signature shall be computed
using the RSASSA-PKCS1-v1_5 algorithm specified in PKCS #1, using SHA256 as
hash function."

**Repo:** `crates/security-pki/src/plugin.rs::DsignScheme::RsaPkcs1`, `sign_with`, `verify_signature_with_cert`

**Tests:** `crates/security-pki/src/plugin.rs::tests::an_rsa_identity_signs_pkcs1_when_the_peer_verifies_only_that`, `crates/security-permissions/tests/cms_verify.rs::rsa_pkcs1_signer_is_accepted`

**Status:** `done` — certificate chain, documents and handshake; an RSA key signs PKCS#1 v1.5 when the peer verifies only that.

### §8.2 RSASSA-PKCS1-V1_5 and ECDSA-P384 for handshake signatures

**Spec:** §8.2, Table 25, pp. 92–93 — as above and the row
ECDSA+P384+SHA384.

**Repo:** `crates/security-pki/src/plugin.rs::DsignScheme`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_dsign_algo_must_fit_the_certificate`, `::a_p384_identity_authenticates_with_ecdsa_p384`, `::an_rsa_identity_signs_pkcs1_when_the_peer_verifies_only_that`

**Status:** `done` — `c.dsign_algo` names a scheme that has to fit the certificate.

### §8.2 Table 25 — ECDSA+P256+SHA256, id 0x12, bit 2

**Spec:** §8.2, Table 25, p. 93 — "256-bit Elliptic Curve Key for the secp256r1
curve [...] The digital signature shall be computed using the ECDSA-SHA256
algorithm".

**Repo:** `crates/security-pki/src/plugin.rs::sign_with` (l. 515,
`ECDSA_P256_SHA256_ASN1_SIGNING`), `verify_signature_with_cert` (l. 584)

**Tests:** `crates/security-pki/tests/e2e_handshake.rs::full_three_stage_handshake`,
`crates/security-pki/src/identity.rs::mutation_killers::detect_cert_algo_requires_both_ecdsa_oids`

**Status:** done

### §8.2 Table 25 — ECDSA+P384+SHA384, id 0x13, bit 3

**Spec:** §8.2, Table 25, p. 93 — "384-bit Elliptic Curve Key for the secp384r1
curve [...] The digital signature shall be computed using the ECDSA-SHA384
algorithm".

**Repo:** `crates/security-pki/src/identity.rs::CertKeyAlgo::EcdsaP384Sha384`; `crates/security-pki/src/plugin.rs::DsignScheme::EcdsaP384`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_p384_identity_authenticates_with_ecdsa_p384`

**Status:** `done` — key, handshake and masks; a chain with a P-384 CA is not tested (the chain in the test is P-256).

### §8.2 Constants CNAME_* of the signature algorithms

**Spec:** §8.2, p. 93 — "CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256 =
'RSASSA-PSS-MGF1SHA256+2048+SHA256'; CNAME_RSASSA_PKCS1_V15_2048_SHA256 =
'RSASSA-PKCS1-V1_5+2048+SHA256'; CNAME_ECDSA_P256_SHA256_NAME =
'ECDSA+P256+SHA256'; CNAME_ECDSA_P384_SHA384 = 'ECDSA+P384+SHA384';"

**Repo:** `crates/security/src/algorithm.rs::CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256`` u. a.

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — four constants.

### §8.2 Constants CID_* of the signature algorithms

**Spec:** §8.2, p. 93 — "CID_RSASSA_PSS_MGF1SHA256_2048_SHA256 = 0x10;
CID_RSASSA_PKCS1_V15_2048_SHA256 = 0x11; CID_ECDSA_P256_SHA256 = 0x12;
CID_ECDSA_P384_SHA384 = 0x13;"

**Repo:** `crates/security/src/algorithm.rs::CID_RSASSA_PSS_MGF1SHA256_2048_SHA256`` u. a.

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — 0x10 to 0x13.

### §8.2 Constants CBIT_* of the signature algorithms

**Spec:** §8.2, p. 93 — "CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256 = 1 << 0;
CBIT_RSASSA_PKCS1_V15_2048_SHA256 = 1 << 1; CBIT_ECDSA_P256_SHA256 = 1 << 2;
CBIT_ECDSA_P384_SHA384 = 1 << 3;"

**Repo:** `crates/rtps/src/security_algo_info.rs::digital_signature_bit`
(l. 61)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::spec_bit_constants_match`

**Status:** done

### §8.3 Key Establishment Algorithms

**Spec:** §8.3, p. 93 — "SPIs may use key establishment algorithms to establish
a shared secret key between two Endpoints".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the algorithms follow in
Table 26.

### §8.3 Table 26 — DHE+MODP-2048-256, id 0x20, bit 0

**Spec:** §8.3, Table 26, p. 94 — "The Diffie-Hellman Public Key shall be for
the 2048-bit MODP Group with 256-bit Prime Order Subgroup, see IETF RFC 5114
[...] section 2.3. [...] The Key Agreement Algorithm shall be the 'dhEphem,
C(2e, 0s, FFC DH) Scheme'".

**Repo:** `crates/security-keyexchange/src/modp.rs`, `crates/security-keyexchange/src/lib.rs::KxSuite::DhModp2048_256`; `crates/security-pki/src/plugin.rs::kx_suite_for_algo`

**Tests:** `crates/security-keyexchange/src/modp.rs::tests::the_constants_are_the_ones_printed_in_rfc_5114_2_3`, `crates/security-keyexchange/src/modp.rs::tests::the_group_has_the_structure_of_rfc_5114_2_3`, `crates/security-pki/src/plugin.rs::tests::the_handshake_runs_over_dhe_modp_2048_256`

**Status:** `done` — group of RFC 5114 §2.3 (the test compares the constants byte for byte with the text of the RFC, plus 2048/256 bits, q | p-1, g^q = 1); names `DH+MODP-2048-256` (sent) and `DHE+MODP-2048-256`. Not measured against another vendor; the exponentiation runs with `crypto-bigint` (Montgomery form, `pow` over the full width of the 256-bit exponent, the same time for every exponent), the exponent is zeroed on drop.

### §8.3 Table 26 — ECDHE-CEUM+P256, id 0x21, bit 1

**Spec:** §8.3, Table 26, p. 94 — "The Diffie-Hellman Public Key shall be for
the NIST's EC Curve P-256 [...] The Key Agreement Algorithm shall be the
'(Cofactor) Ephemeral Unified Model, C(2e, 0s, ECC CDH)'".

**Repo:** `crates/security-keyexchange/src/lib.rs::KxSuite::EcdhP256`
(l. 100)

**Tests:** `crates/security-keyexchange/src/lib.rs::tests::p256_two_parties_derive_identical_secret`,
`crates/security-keyexchange/src/lib.rs::tests::p256_rejects_off_curve_point`

**Status:** done

### §8.3 Table 26 — ECDHE-CEUM+P384, id 0x22, bit 2

**Spec:** §8.3, Table 26, p. 94 — "The Diffie-Hellman Public Key shall be for
the NIST's EC Curve P-384 [...] (Cofactor) Ephemeral Unified Model".

**Repo:** `crates/security-keyexchange/src/lib.rs::KxSuite::EcdhP384`
(l. 110)

**Tests:** `crates/security-keyexchange/src/lib.rs::tests::p384_matches_the_spec_table_and_derives_one_shared_secret`,
`crates/security-keyexchange/src/lib.rs::tests::p384_rejects_a_p256_public_key`

**Status:** done

### §8.3 Public MODP DH key as a big-endian number

**Spec:** §8.3, p. 94 — "If the public key corresponds to the
'DHE+MODP-2048-256' crypto algorithm, then: The OctetSeq's value shall contain
the big endian representation [...] of the DH public key".

**Repo:** `crates/security-keyexchange/src/modp.rs`

**Tests:** `crates/security-keyexchange/src/lib.rs::tests::modp_2048_256_agrees_and_has_id_0x20`

**Status:** `done` — big-endian number without leading zeros, up to 256 octets; the peer key is checked per SP 800-56A §5.6.2.3.1.

### §8.3 Public ECDHE key as an octet string after SEC 1 §2.3.3

**Spec:** §8.3, pp. 94–95 — "If the public key corresponds to the
'ECDHE-CEUM+P256' or 'ECDHE-CEUM+P384' crypto algorithms, then: The OctetSeq's
value shall contain the octet string representation of the ECDHE public key.
The octet string representation encoding must conform with Sec. 2.3.3
'Elliptic-Curve-Point-to-Octet-String Conversion' of the SECG SEC 1".

**Repo:** `crates/security-keyexchange/src/lib.rs::KxSuite` (l. 100,
uncompressed point `0x04 || X || Y`)

**Tests:** `crates/security-keyexchange/src/lib.rs::tests::p256_public_key_is_65_bytes`,
`crates/security-keyexchange/src/lib.rs::tests::p384_matches_the_spec_table_and_derives_one_shared_secret`

**Status:** done

### §8.3 Constants CNAME_* of key agreement

**Spec:** §8.3, p. 95 — "CNAME_DHE_MODP_2048_256 = 'DHE+MODP-2048-256';
CNAME_ECDHE_CEUM_P256 = 'ECDHE-CEUM+P256'; CNAME_ECDHE_CEUM_P384 =
'ECDHE-CEUM+P384';"

**Repo:** `crates/security/src/algorithm.rs`; `crates/security-pki/src/plugin.rs::kagree_algo_str`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`, `crates/security-pki/src/plugin.rs::tests::the_algorithm_names_follow_the_version_of_the_peer_and_the_setting`

**Status:** `done` — The constants of the specification are there; by default the initiator sends `ECDHE-CEUM+P256` and `DHE+MODP-2048-256`, and `ECDH+prime256v1-CEUM` and `DH+MODP-2048-256` with the setting `1.1`. Not measured against Cyclone, Fast DDS, OpenDDS (matrix); they send the names of 1.1. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.3 Constants CID_* of key agreement

**Spec:** §8.3, p. 95 — "CID_DHE_MODP_2048_256 = 0x20; CID_ECDHE_CEUM_P256 =
0x21; CID_ECDHE_CEUM_P384 = 0x22;"

**Repo:** `crates/security/src/algorithm.rs::CID_DHE_MODP_2048_256`` u. a.

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — 0x20 to 0x22.

### §8.3 Constants CBIT_* of key agreement

**Spec:** §8.3, p. 95 — "CBIT_DHE_MODP_2048_256 = 1 << 0; CBIT_ECDHE_CEUM_P256
= 1 << 1; CBIT_ECDHE_CEUM_P384 = 1 << 2;"

**Repo:** `crates/rtps/src/security_algo_info.rs::key_establishment_bit`
(l. 73)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::spec_bit_constants_match`

**Status:** done

### §8.3 X25519 only as an option

**Spec:** §8.3, Table 26, p. 94 — the table names only DHE+MODP-2048-256,
ECDHE-CEUM+P256 and ECDHE-CEUM+P384.

**Repo:** `crates/security-pki/src/plugin.rs::kx_suite_for_algo`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_algorithm_names_of_1_2_and_1_1_are_taken`

**Status:** `done` — As replier `X25519` is taken only where this side set it itself (`allow_x25519`). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

---

## §9 Plugin Architecture

### §9.1.1 Service Plugin Interface Overview

**Spec:** §9.1.1, p. 97–98 — "There are five plugin SPIs: Authentication,
Access-Control, Cryptographic, Logging, and Data Tagging." (Figure 7,
Table 27)

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; every operation of the five
interfaces is listed on its own in §9.3 to §9.7.

### §9.1.2 Plugins configurable separately per DomainParticipant

**Spec:** §9.1.2, p. 98 — "The Security Plugins shall be configurable
separately for each DomainParticipant even when multiple DomainParticipants
are constructed within the same Operating System Process and share the same
Address Space."

**Repo:** `crates/dcps/src/participant.rs::security_from_participant_properties`
(l. 203, per participant from its `DomainParticipantQos`);
`crates/dcps/src/runtime.rs::with_security_bundle` (l. 3788);
`crates/security-runtime/src/bundle.rs::SecurityBundle` (l. 40)

**Tests:** `crates/dcps/tests/security_guid_wiring_e2e.rs::secured_participants_on_public_rust_api_use_identity_adjusted_guids_and_discover`

**Status:** done — the test creates two participants with different
identities in the same process.

### §9.1.2 Plugin suite bound before the DomainParticipant is enabled

**Spec:** §9.1.2, p. 98 — "it is required for the Plugin Suite to be
initialized and bound by the time the DomainParticipant is enabled.
Therefore this process shall complete either during the
DomainParticipantFactory create_domain_participant or else during the
DomainParticipant enable operations".

**Repo:** `crates/dcps/src/participant.rs::security_from_participant_properties`
(l. 203, called while the participant is created, l. 676)

**Tests:** `crates/dcps/tests/security_qos_properties.rs::a_complete_dds_sec_property_list_creates_a_secured_participant`,
`crates/dcps/tests/security_qos_properties.rs::an_incomplete_property_list_fails_the_create_and_names_the_property`

**Status:** done

### §9.2.1 SecurityException: message, code, minor_code

**Spec:** §9.2.1, Table 28, p. 98–99 — "SecurityException objects are
potentially returned from many of the calls in the Security plugins. They
are used to return an error code and message." Attributes: `message String`,
`code long`, `minor_code long`.

**Repo:** `crates/security/src/error.rs::SecurityError` (`detail`, `code`, `minor_code`), `SecurityErrorKind::code`

**Tests:** `crates/security/src/error.rs::tests::a_security_exception_has_message_code_and_minor_code`

**Status:** `done` — `message` is `detail`; `code` the number of the category (1 to 7), `minor_code` 0 unless the plugin sets one. The spec assigns no values.

---

### §9.3.1 Background (Non-Normative)

**Spec:** §9.3.1, p. 99 — "The discovery mechanism that registers
participants with the DDS middleware is enhanced with an authentication
protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — background.

### §9.3.2 Authentication Plugin Model

**Spec:** §9.3.2, Figure 8, p. 99–100 — classes IdentityHandle,
IdentityToken, IdentityStatusToken, HandshakeHandle, HandshakeMessageToken,
AuthRequestMessageToken, SharedSecretHandle,
AuthenticatedPeerCredentialToken, interfaces Authentication and
AuthenticationListener, enumeration AuthStatusKind.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — diagram; the elements are listed on their
own below.

### §9.3.2.1 IdentityToken

**Spec:** §9.3.2.1, p. 100 — "An IdentityToken contains summary information
on the identity of a DomainParticipant in a manner that can be externalized
and propagated via DDS discovery. The specific content of the IdentityToken
shall be defined by each Authentication plugin specialization."

**Repo:** `crates/security/src/token.rs` (alias `IdentityToken` of
`DataHolder`, l. 247–268);
`crates/security/src/authentication.rs::get_identity_token` (l. 196)

**Tests:** `crates/security/src/token.rs::tests::pki_dh_identity_token_roundtrip`,
`crates/security-pki/src/plugin.rs::tests::get_identity_token_returns_decodable_spec_descriptor`

**Status:** done — the content of the builtin plugin is in §10.3.

### §9.3.2.2 IdentityStatusToken via DCPSParticipantSecure

**Spec:** §9.3.2.2, p. 100–101 — "An IdentityStatusToken contains
authentication information of a DomainParticipant in a manner that can be
externalized and propagated via the DCPSParticipantSecure builtin Topic.
[...] The information shall be retrieved from the Authentication plugin by
calling the operation get_identity_status_token. And included in the
DCPSParticipantSecure builtin Topic."

**Repo:** `crates/security-pki/src/plugin.rs::get_identity_status_token`; `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (fetches the token, sets it in the beacon), `process_authentication_events` (new token after a change); `crates/discovery/src/spdp.rs::SpdpBeacon::serialize_secure` (carries it), `serialize` (leaves it out); `crates/discovery/src/security/stack.rs::identity_status_token`

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_beacon_announces_identity_token_and_discovery_triggers_handshake`,
`crates/discovery/src/security/stack.rs::tests::a_status_change_is_passed_on_with_the_new_token`,
`crates/security-pki/src/plugin.rs::tests::the_identity_status_token_carries_the_ocsp_response`,
`crates/security-pki/src/plugin.rs::tests::a_changed_status_token_is_reported_as_identity_status`

**Status:** `done` — the runtime calls `get_identity_status_token` and puts the token into `identity_status_token` of the beacon. The secure SPDP (DCPSParticipantsSecure) carries it, the plain SPDP (DCPSParticipants) does not, nor does `c.pdata` of the handshake. When the plugin changes the status (`on_status_changed`, `IDENTITY_STATUS`) the runtime fetches the new token and sends the secure SPDP again; the changed beacon is a new change of the secure writer.

### §9.3.2.3 IdentityHandle

**Spec:** §9.3.2.3, p. 101 — "An IdentityHandle is an opaque local reference
to internal state within the Authentication plugin, which uniquely
identifies a DomainParticipant."

**Repo:** `crates/security/src/authentication.rs::IdentityHandle` (l. 34)

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_accepts_ca_signed_cert`

**Status:** done — newtype over `u64`.

### §9.3.2.4 HandshakeHandle

**Spec:** §9.3.2.4, p. 101 — "A HandshakeHandle is an opaque local reference
used to refer to the internal state of a possible mutual authentication or
handshake protocol."

**Repo:** `crates/security/src/authentication.rs::HandshakeHandle` (l. 38)

**Tests:** `crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`

**Status:** done

### §9.3.2.5 AuthRequestMessageToken

**Spec:** §9.3.2.5, p. 101 — "The AuthRequestMessageToken encodes
plugin-specific information that the Authentication plugins associated with
two DomainParticipant entities exchange to bootstrap the mutual
authentication handshake. [...] The AuthRequestMessageToken has class_id set
to GMCLASSID_SECURITY_AUTH_REQUEST".

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::validate_remote_identity` (`remote_auth_request_token` in, `local_auth_request_token` out); `crates/security-pki/src/plugin.rs::validate_remote_identity`; `crates/discovery/src/security/stack.rs::begin_handshake_with`, `on_auth_request`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/security-pki/src/plugin.rs::tests::the_auth_request_announces_the_challenge1_of_the_request`,
`crates/discovery/src/security/stack.rs::tests::an_auth_request_goes_out_on_discovery_and_binds_the_challenge`,
`crates/discovery/src/security/stack.rs::tests::an_auth_request_is_passed_to_the_plugin_whenever_it_arrives`

**Status:** `done` — `validate_remote_identity` takes the AuthRequestMessageToken of the peer and returns the own one (`local_auth_request_token`, `None` is TokenNIL). The stack sends it with `message_class_id` `dds.sec.auth_request`, keeps one received before the discovery for `validate_remote_identity` (at most 256), and passes every received one to the plugin (§9.3.2.9.1 rule 1).

### §9.3.2.6 HandshakeMessageToken

**Spec:** §9.3.2.6, p. 101 — "A HandshakeMessageToken encodes
plugin-specific information that the Authentication plugins associated with
two DomainParticipant entities exchange as part of the mutual
authentication handshake. [...] The HandshakeMessageToken has class_id set
to GMCLASSID_SECURITY_AUTH_HANDSHAKE".

**Repo:** `crates/security/src/token.rs` (alias `HandshakeMessageToken`,
l. 247–268); `crates/discovery/src/security/stack.rs::build_message`
(l. 752, `class_id::AUTH`)

**Tests:** `crates/security/src/generic_message.rs::tests::handshake_request_token_in_message_data`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** done

### §9.3.2.7 AuthenticatedPeerCredentialToken

**Spec:** §9.3.2.7, p. 101 — "An AuthenticatedPeerCredentialToken encodes
plugin-specific information that the Authentication plugin obtains from a
remote DomainParticipant during the authentication process that is of
interest to the AccessControlPlugin. This information is accessible via the
operation get_authenticated_peer_credential_token."

**Repo:** `crates/security-pki/src/plugin.rs::get_authenticated_peer_credential_token`; `crates/security-pki/src/psk.rs::get_authenticated_peer_credential_token`; `crates/discovery/src/security/stack.rs::collect_completion` (calls the operation after completion; `peer_permissions` and `peer_identity_certificate` come from it)

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`,
`crates/discovery/src/security/stack.rs::tests::the_peer_credential_token_gives_the_permissions_and_the_certificate`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — the plugin returns the token after a completed handshake (the initiator after VALIDATION_OK_FINAL_MESSAGE, the replier after VALIDATION_OK), an error before. The stack fetches it on every completion, reads `c.perm` and `c.id` from it for the access control, and gives it back. The own operation `peer_permissions` is gone.

### §9.3.2.8 SharedSecretHandle

**Spec:** §9.3.2.8, p. 102 — "A SharedSecretHandle is an opaque local
reference to internal state within the AuthenticationPlugin containing a
secret that is shared between the AuthenticationPlugin implementation and
the peer AuthenticationPlugin implementation".

**Repo:** `crates/security/src/authentication.rs::SharedSecretHandle`
(l. 43), `SharedSecretProvider` (l. 57)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_shared_secret_returns_stored_value`

**Status:** done — the crypto plugin reads the bytes of the secret through
the own interface `SharedSecretProvider`.

### §9.3.2.9 State machine of the authentication (Figure 9)

**Spec:** §9.3.2.9, Figure 9, p. 102–103 — states ValidateRemoteIdentity,
HandshakeInit, HandshakeMessageSend/-Wait/-Received, HandshakeCompleted with
get_shared_secret, wait states RetryWait1–5 for VALIDATION_PENDING_RETRY,
branch AuthRequestSend/AuthRequestWait and AuthenticationTimeoutWait.

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`, `continue_validation`, `apply_outcome`, `on_stateless_message`, `run_retries`, `expire_handshakes`, `poll_stateless`; `crates/security-runtime/src/profile.rs` (`LOCAL_VALIDATION_RETRY_DELAY`)

**Tests:** `crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`,
`crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`,
`crates/discovery/src/security/stack.rs::tests::a_pending_validation_is_retried_after_the_retry_timeout`,
`crates/discovery/src/security/stack.rs::tests::a_pending_request_is_retried_after_the_retry_timeout`,
`crates/discovery/src/security/stack.rs::tests::a_pending_reply_and_a_pending_process_are_retried_with_their_message`,
`crates/discovery/src/security/stack.rs::tests::a_handshake_that_does_not_complete_ends_with_the_authentication_timeout`,
`crates/discovery/src/security/stack.rs::tests::a_refused_reply_makes_the_stack_send_the_request_again`,
`crates/discovery/src/security/stack.rs::tests::an_auth_request_is_passed_to_the_plugin_whenever_it_arrives`,
`crates/security-pki/src/ocsp.rs::tests::a_responder_that_is_slow_gives_pending_retry_until_the_answer_is_in`

**Status:** `done` — all states of Figure 9: `validate_local_identity` (the profile calls it again after a fixed pause of 100 ms as long as it returns VALIDATION_PENDING_RETRY; only a key on a token goes through the blocking operation), `validate_remote_identity` with the branches PENDING_HANDSHAKE_REQUEST and _MESSAGE, `begin_handshake_request`, `begin_handshake_reply`, `process_handshake` with the wait states for VALIDATION_PENDING_RETRY (`set_retry_timeout`, 500 ms), sending and resending the message, the branch AuthRequestSend/-Wait, `get_shared_secret` after OK and OK_FINAL_MESSAGE, AuthenticationTimeoutWait (`set_authentication_timeout`, 60 s). The timeout is not fixed ("implementation-specific timeout").

### §9.3.2.9 Wait states for VALIDATION_PENDING_RETRY and timeout

**Spec:** §9.3.2.9, Figure 9, p. 103 — RetryWait1 to RetryWait5 with
`retry_timeout`, AuthenticationTimeoutWait with `authentication_timeout`.

**Repo:** `crates/discovery/src/security/stack.rs::run_retries`, `retry_step`, `expire_handshakes`, `set_retry_timeout`, `set_authentication_timeout`

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_pending_validation_is_retried_after_the_retry_timeout`,
`crates/discovery/src/security/stack.rs::tests::a_pending_request_is_retried_after_the_retry_timeout`,
`crates/discovery/src/security/stack.rs::tests::a_pending_reply_and_a_pending_process_are_retried_with_their_message`,
`crates/discovery/src/security/stack.rs::tests::a_handshake_that_does_not_complete_ends_with_the_authentication_timeout`

**Status:** `done` — a step that returned VALIDATION_PENDING_RETRY is called again after the retry timeout with the same inputs (token, message, `related_message_identity`); a handshake not complete after the timeout ends: the peer is unauthenticated (reason `Timeout`), the handles go back to the plugin, the failure is in `take_handshake_failures`, the next beacon starts again. A completed handshake does not expire.

### §9.3.2.9.1 Rule 1: every AUTH_REQUEST and AUTH_HANDSHAKE message to the plugin

**Spec:** §9.3.2.9.1, p. 104 — "1. The DDS security implementation shall
pass to the AuthenticationPlugin any message received by the
BuiltinParticipantStatelessMessageReader that has a GenericMessageClassId
set to GMCLASSID_SECURITY_AUTH_REQUEST or GMCLASSID_SECURITY_AUTH_HANDSHAKE."

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`, `on_auth_request`

**Tests:** `crates/discovery/src/security/stack.rs::tests::an_auth_request_is_passed_to_the_plugin_whenever_it_arrives`,
`crates/discovery/src/security/stack.rs::tests::an_auth_request_goes_out_on_discovery_and_binds_the_challenge`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** `done` — messages with `GMCLASSID_SECURITY_AUTH_HANDSHAKE` go to `begin_handshake_reply` or `process_handshake`. A message with `GMCLASSID_SECURITY_AUTH_REQUEST` goes to `validate_remote_identity` as `remote_auth_request_token`: from a participant not yet discovered it is kept until the discovery; for a participant whose handshake has not begun, the identity made with the token replaces the first one; with a handshake begun, the plugin checks the token and the identity this makes goes back at once.

### §9.3.2.9.1 Rule 2: cache the sent message and repeat it

**Spec:** §9.3.2.9.1, p. 104 — "2. [...] the DDS implementation shall cache
the message that was sent and set a timer. If a correct reply message is not
received when the timer expires, the state-machine shall send the same
message again. This process shall be repeated multiple times until a correct
message is received."

**Repo:** `crates/discovery/src/security/stack.rs::poll` (l. 335, repeat
every 500 ms, l. 347), `begin_handshake_with` (l. 418, repeat on every SPDP
beacon), `on_stateless_message` (l. 622, a duplicate request gets the cached
reply)

**Tests:** `crates/discovery/src/security/stack.rs::tests::lost_reply_recovered_by_resend_yields_matching_secret`,
`crates/discovery/src/security/stack.rs::tests::initiator_resends_auth_request_on_repeated_begin_while_incomplete`,
`crates/discovery/src/security/stack.rs::tests::a_duplicate_reply_after_completion_is_answered_with_the_final`

**Status:** done

### §9.3.2.9.1 Rule 3: after VALIDATION_NOT_OK resend the same message

**Spec:** §9.3.2.9.1, p. 104 — "3. [...] If the plugin operation returns
VALIDATION_NOT_OK, the implementation transitions back to the previous state
that caused the message to be sent and resends the same message. [...] Rule
#3 makes authentication robust to an attacker trying to disrupt an
authentication exchange by sending bad replies."

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message` (`resend_now`), `poll_stateless`; `crates/security-pki/src/plugin.rs::process_handshake` (the state stays until the check has passed)

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_refused_reply_makes_the_stack_send_the_request_again`,
`crates/discovery/src/security/stack.rs::tests::a_forged_final_does_not_take_the_handshake_down`,
`crates/security-pki/src/plugin.rs::tests::a_forged_reply_does_not_take_the_handshake_down`,
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`,
`crates/security-pki/src/psk.rs::tests::a_forged_message_leaves_the_handshake_in_place`

**Status:** `done` — a step the plugin refuses leaves the handshake as it was; the stack sends the cached message that caused the step again at the next `poll` (at most one per peer per tick). The error still goes to the caller. The name VALIDATION_NOT_OK is not in Table 30 (contradiction in the spec); a failure of the operation is meant.

### §9.3.2.10 Treat unauthenticated DomainParticipants the same

**Spec:** §9.3.2.10, p. 104–105 — "This can be either because they lack
support for the Authentication plugin being used, have incompatible plugins,
incompatible plugin configurations, or simply fail the authentication
protocol. All these cases shall be treated the same."

**Repo:** `crates/discovery/src/security/stack.rs::authentication_status`, `is_authenticated`, `mark_unauthenticated`, `PeerAuthentication`, `UnauthenticatedReason`; `crates/dcps/src/runtime.rs::builtin_match` (decides by `is_authenticated`), `spdp_security_handshake`

**Tests:** `crates/discovery/src/security/stack.rs::tests::every_reason_for_being_unauthenticated_is_one_case`,
`crates/dcps/src/runtime.rs::tests::spdp_beacon_announces_identity_token_and_discovery_triggers_handshake`

**Status:** `done` — a participant without an IdentityToken, with a refused identity, with a failed handshake, with an incompatible configuration (§7.3.23) or after the timeout is `Unauthenticated`; the reason is for the log. The matching of the builtin endpoints (`builtin_match`) and the acceptance of plaintext ask only whether the peer is authenticated, and `allow_unauthenticated_participants`. A failure of `begin_handshake_with` is reported as `security.handshake.failed`.

### §9.3.2.11 Table 29 — validate_local_identity

**Spec:** §9.3.2.11.2, p. 108–109 — "Validates the identity of the local
DomainParticipant. [...] this operation also returns the DomainParticipant
GUID_t that shall be used by the DDS implementation to uniquely identify the
DomainParticipant on the network." Parameters: out `local_identity_handle`,
out `adjusted_participant_guid`, `domain_id`, `participant_qos`,
`candidate_participant_guid`; return VALIDATION_OK, VALIDATION_FAILED,
VALIDATION_PENDING_RETRY.

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::validate_local_identity`; `crates/security-pki/src/plugin.rs::validate_local_identity`, `validate_identity`; `crates/security-pki/src/psk.rs::validate_local_identity`; `crates/security-runtime/src/profile.rs`

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_returns_the_adjusted_guid_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_with_a_foreign_ca_fails_without_a_handle`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_accepts_ca_signed_cert`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_rejects_wrong_ca`,
`crates/security-pki/src/ocsp.rs::tests::a_responder_that_is_slow_gives_pending_retry_until_the_answer_is_in`,
`crates/security-pki/src/ocsp.rs::tests::a_revoked_answer_ends_the_pending_validation_with_a_failure`

**Status:** `done` — the operation has the parameters `domain_id`, `participant_qos` (the PropertyQosPolicy) and `candidate_participant_guid` and returns result, handle and `adjusted_participant_guid` (`LocalIdentityValidation`); a failure is `Err` (VALIDATION_FAILED with SecurityException); VALIDATION_PENDING_RETRY is in the next item.

### §9.3.2.11.2 validate_local_identity: adjusted_participant_guid, domain_id, participant_qos, PENDING_RETRY

**Spec:** §9.3.2.11.2, p. 109 — "Parameter (out) adjusted_participant_guid:
The GUID_t that the DDS implementation shall use [...] Parameter domain_id
[...] Parameter participant_qos [...] VALIDATION_PENDING_RETRY if
verification has not completed and the operation should be retried later."

**Repo:** `crates/security-pki/src/plugin.rs::validate_local_identity_nonblocking`, `register_local_identity`; `crates/security-runtime/src/profile.rs` (retry)

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_returns_the_adjusted_guid_of_table_59`,
`crates/security-pki/src/ocsp.rs::tests::a_responder_that_is_slow_gives_pending_retry_until_the_answer_is_in`,
`crates/security-pki/src/ocsp.rs::tests::a_revoked_answer_ends_the_pending_validation_with_a_failure`

**Status:** `done` — `adjusted_participant_guid` as Table 59 says (bit 0 set, 47 bits of the SHA-256 of the SubjectName, 48 bits of the SHA-256 of the candidate GUID, the same EntityId); the profile takes the GUID from the result. `domain_id` stays with the identity. A certificate that names an OCSP responder is checked on a thread of its own: the first call returns VALIDATION_PENDING_RETRY without waiting, each further one the result or `PendingRetry` until the answer is in.

### §9.3.2.11 Table 29 — get_identity_token

**Spec:** §9.3.2.11.9, p. 115 — "Retrieves an IdentityToken used to
represent on the network the identity of the DomainParticipant identified by
the specified IdentityHandle. [...] The handle must have been returned by a
successful call to validate_local_identity, otherwise the operation shall
return false and fill the SecurityException."

**Repo:** `crates/security/src/authentication.rs::get_identity_token`
(l. 196); `crates/security-pki/src/plugin.rs::get_identity_token` (l. 791,
an unknown handle yields `BadArgument`)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_identity_token_returns_decodable_spec_descriptor`

**Status:** done

### §9.3.2.11 Table 29 — get_identity_status_token

**Spec:** §9.3.2.11.10, p. 115–116 — "Retrieves an AuthenticationToken used
to represent on the network the authentication state of the
DomainParticipant identified by the specified IdentityHandle."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::get_identity_status_token`; `crates/security-pki/src/plugin.rs::get_identity_status_token`; `crates/security-pki/src/psk.rs::get_identity_status_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_identity_status_token_carries_the_ocsp_response`,
`crates/security-pki/src/plugin.rs::tests::the_tokens_of_a_local_identity_need_a_local_identity_handle`,
`crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — PKI-DH returns the token of Table 53, `DDS:Auth:PSK` TokenNIL (Table 83); the runtime calls the operation (see §9.3.2.2); a handle `validate_local_identity` did not return is an error.

### §9.3.2.11 Table 29 — set_participant_security_config

**Spec:** §9.3.2.11.11, p. 116 — "Configures various aspects of the
Authentication algorithm used by the Authentication plugin and retrieves an
updated ParticipantSecurityAlgorithmInfo [...] If the Authentication plugin
is not able to restrict the algorithms used as specified in the
participant_security_config.algorithm_info the operation shall fail".

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::set_participant_security_config`, `ParticipantSecurityConfig`; `crates/security-pki/src/plugin.rs::set_participant_security_config`; `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`

**Tests:** `crates/security-pki/src/plugin.rs::tests::set_participant_security_config_adjusts_a_copy_of_the_given_info`,
`crates/security-pki/src/plugin.rs::tests::set_participant_security_config_fails_without_an_algorithm_of_the_plugin`,
`crates/security-pki/src/plugin.rs::tests::set_participant_security_config_fails_when_a_used_algorithm_is_not_supported`,
`crates/security-pki/src/plugin.rs::tests::the_configured_algorithms_bind_the_handshake`,
`crates/security-pki/src/plugin.rs::tests::the_algorithm_masks_follow_the_identity_and_the_key_agreement`,
`crates/security-pki/src/psk.rs::tests::table_83_set_participant_security_config_and_credential_do_nothing`

**Status:** `done` — the operation takes `ParticipantSecurityConfig` (the `algorithm_info` part, the digital signature and key establishment masks) and returns the adjusted info; it fails where the spec says so. The runtime calls it with `ParticipantSecurityConfig::unrestricted()` and announces the adjusted info (`algorithm_masks` is gone).

### §9.3.2.11 Table 29 — set_permissions_credential_and_token

**Spec:** §9.3.2.11.12, p. 117 — "Associates the PermissionsCredentialToken
[...] returned by the AccessControl plugin operation
get_permissions_credential_token with the local DomainParticipant
identified by the IdentityHandle. This operation shall be called by the
middleware after calling validate_local_identity and prior to any calls to
validate_remote_identity."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::set_permissions_credential_and_token`; `crates/security-pki/src/plugin.rs::set_permissions_credential_and_token`; `crates/security-runtime/src/profile.rs`; `crates/security-pki/src/psk.rs::set_permissions_credential_and_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_permissions_credential_is_what_the_handshake_sends_as_c_perm`,
`crates/security-pki/src/plugin.rs::tests::a_permissions_credential_that_is_none_is_refused`,
`crates/security-pki/src/psk.rs::tests::table_83_set_participant_security_config_and_credential_do_nothing`

**Status:** `done` — PKI-DH keeps the credential token and the PermissionsToken per IdentityHandle; `dds.perm.cert` (Table 64) is the `c.perm` of the handshakes of that identity, the token the answer of `get_permissions_token`. The profile calls the operation with the signed permissions document; a token of the wrong class, without the document, or no DataHolder is an error.

### §9.3.2.11 Table 29 — validate_remote_identity

**Spec:** §9.3.2.11.3, p. 109–110 — "Initiates the process of validating the
identity of the discovered remote DomainParticipant, represented as an
IdentityToken object." Parameters: out `remote_identity_handle`, out
`local_auth_request_token`, `remote_auth_request_token`,
`local_identity_handle`, `remote_identity_token`, `remote_participant_guid`;
return VALIDATION_OK, VALIDATION_FAILED,
VALIDATION_PENDING_HANDSHAKE_REQUEST, VALIDATION_PENDING_HANDSHAKE_MESSAGE,
VALIDATION_PENDING_RETRY.

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::validate_remote_identity`, `RemoteIdentityValidation`; `crates/security-pki/src/plugin.rs::validate_remote_identity`; `crates/security-pki/src/psk.rs::validate_remote_identity`

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_remote_identity_orders_the_guids_like_table_59`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_compares_the_plugin_name_and_the_major_version`,
`crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_takes_only_a_pki_dh_identity_token`,
`crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_orders_remote_identities_by_guid`

**Status:** `done` — the operation returns `RemoteIdentityValidation` (result, handle, `local_auth_request_token`); the results OK, PENDING_HANDSHAKE_REQUEST, PENDING_HANDSHAKE_MESSAGE and PENDING_RETRY can be represented, VALIDATION_FAILED is `Err`.

### §9.3.2.11.3 validate_remote_identity: AuthRequest token, return values, GUID

**Spec:** §9.3.2.11.3, p. 109–110 — "If the operation returns
VALIDATION_PENDING_HANDSHAKE_REQUEST, then the DDS implementation shall call
the operation begin_handshake_request [...] Parameter (out)
local_auth_request_token [...] Parameter remote_auth_request_token".

**Repo:** `crates/security-pki/src/plugin.rs::validate_remote_identity`; `crates/discovery/src/security/stack.rs::continue_validation`

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_remote_identity_orders_the_guids_like_table_59`,
`crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`,
`crates/discovery/src/security/stack.rs::tests::a_pending_validation_is_retried_after_the_retry_timeout`

**Status:** `done` — the plugin decides the role: a greater GUID of the peer is PENDING_HANDSHAKE_REQUEST, a smaller one PENDING_HANDSHAKE_MESSAGE, the own GUID an error. The stack follows the result. The GUID of the peer stays with its identity (the plugin checks `c.pdata` against it).

### §9.3.2.11 Table 29 — begin_handshake_request

**Spec:** §9.3.2.11.4, p. 110–111 — "This operation is used to initiate a
handshake. It shall be called by the DDS middleware solely as a result of
having a previous call to validate_remote_identity returning
VALIDATION_PENDING_HANDSHAKE_REQUEST." Parameters: out `handshake_handle`,
out `handshake_message`, `initiator_identity_handle`,
`replier_identity_handle`, `serialized_local_participant_data`.

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::begin_handshake_request`; `crates/security-pki/src/plugin.rs::begin_handshake_request`; `crates/security-pki/src/psk.rs::begin_handshake_request`; `crates/discovery/src/security/stack.rs::continue_validation`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_operations_need_the_result_of_validate_remote_identity`,
`crates/security-pki/src/plugin.rs::tests::request_token_has_spec_class_id_and_properties`,
`crates/discovery/src/security/stack.rs::tests::a_pending_request_is_retried_after_the_retry_timeout`

**Status:** `done` — `serialized_local_participant_data` is a parameter (the `c.pdata`); the call is allowed only for a handle `validate_remote_identity` returned with PENDING_HANDSHAKE_REQUEST (otherwise PreconditionError); the result is PENDING_HANDSHAKE_MESSAGE with the message, or PENDING_RETRY.

### §9.3.2.11 Table 29 — begin_handshake_reply

**Spec:** §9.3.2.11.5, p. 112–113 — "This operation shall be invoked by the
DDS implementation in reaction to the reception of the initial handshake
message that originated on a DomainParticipant that called the
begin_handshake_request operation." Parameters: out `handshake_handle`, out
`handshake_message_out`, `handshake_message_in`,
`initiator_identity_handle`, `replier_identity_handle`,
`serialized_local_participant_data`.

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::begin_handshake_reply`; `crates/security-pki/src/plugin.rs::begin_handshake_reply`; `crates/security-pki/src/psk.rs::begin_handshake_reply`; `crates/discovery/src/security/stack.rs::on_stateless_message`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_operations_need_the_result_of_validate_remote_identity`,
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`,
`crates/discovery/src/security/stack.rs::tests::a_pending_reply_and_a_pending_process_are_retried_with_their_message`

**Status:** `done` — as `begin_handshake_request`: `serialized_local_participant_data` as a parameter, only for a handle with PENDING_HANDSHAKE_MESSAGE (otherwise PreconditionError); the stack calls the operation with the identity `validate_remote_identity` returned for the initiator, not with the local one.

### §9.3.2.11 Table 29 — process_handshake

**Spec:** §9.3.2.11.6, p. 113–114 — "This operation is used to continue a
handshake. [...] This operation generates a handshake_message_out
HandshakeMessageToken in response to a received handshake_message_in".
Return VALIDATION_OK, VALIDATION_FAILED,
VALIDATION_PENDING_HANDSHAKE_MESSAGE, VALIDATION_OK_FINAL_MESSAGE,
VALIDATION_PENDING_RETRY.

**Repo:** `crates/security/src/authentication.rs::HandshakeStepOutcome`; `crates/security-pki/src/plugin.rs::process_handshake`; `crates/security-pki/src/psk.rs::process_handshake`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`,
`crates/security/src/authentication.rs::tests::a_handshake_outcome_is_the_validation_result_of_table_30`

**Status:** `done` — the initiator returns VALIDATION_OK_FINAL_MESSAGE after the reply (`SendFinalMessage`, distinguishable from VALIDATION_PENDING_HANDSHAKE_MESSAGE), the replier VALIDATION_OK after the final; PENDING_RETRY can be represented.

### §9.3.2.11 Return values and parameters of the handshake operations

**Spec:** §9.3.2.11.4–§9.3.2.11.6, p. 110–114 — the return values
VALIDATION_OK_FINAL_MESSAGE and VALIDATION_PENDING_RETRY and the parameter
`serialized_local_participant_data` ("CDR Big Endian Serialization for the
ParticipantBuiltInTopicDataSecure object associated with the local
DomainParticipant").

**Repo:** `crates/security/src/authentication.rs::HandshakeStepOutcome` (`SendMessage`, `SendFinalMessage`, `Complete`, `WaitingForPeer`, `PendingRetry`, `validation_result`, `message_out`); `crates/discovery/src/security/stack.rs::set_local_participant_data`

**Tests:** `crates/security/src/authentication.rs::tests::a_handshake_outcome_is_the_validation_result_of_table_30`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/discovery/src/security/stack.rs::tests::a_pending_request_is_retried_after_the_retry_timeout`

**Status:** `done` — all six values of Table 30 can be represented; the data of the participant go into the operations as the parameter `serialized_local_participant_data`. The runtime gives the ParticipantBuiltinTopicData of the beacon as PL_CDR in big endian (without `identity_status_token`) to the stack; they are not the ParticipantBuiltinTopicDataSecure, because Cyclone and Fast DDS read `c.pdata` as ParticipantBuiltinTopicData (Table 56).

### §9.3.2.11.4 Fields of the sent ParticipantStatelessMessage

**Spec:** §9.3.2.11.4, p. 111 — "The message_class_id shall be set to
GMCLASSID_SECURITY_AUTH_HANDSHAKE. The destination_participant_guid shall be
set to match the DDS GUID_t of the destination DomainParticipant. The
message_identity shall be set to have the source_guid matching the DDS GUID_t
of the DomainParticipant that is sending the message and the sequence_number
to the value in the previous message sent by the
BuiltinParticipantMessageWriter, incremented by one. The
related_message_identity shall be set with source_guid as GUID_UNKNOWN and
sequence_number to zero."

**Repo:** `crates/discovery/src/security/stack.rs::PeerHandshake::build_message`, `apply_outcome`

**Tests:** `crates/discovery/src/security/stack.rs::tests::the_sequence_number_counts_the_messages_of_the_writer`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`,
`crates/security/src/generic_message.rs::tests::nil_message_identity`

**Status:** `done` — `message_class_id`, `destination_participant_guid`, `source_guid`, the empty `related_message_identity` of the request and the sequence of the `sequence_number` (next item) are as the clause says.

### §9.3.2.11.4 sequence_number of the writer's previous message plus one

**Spec:** §9.3.2.11.4, §9.3.2.11.5, §9.3.2.11.6, p. 111, 113, 114 — "the
sequence_number to the value in the previous message sent by the
BuiltinParticipantMessageWriter, incremented by one."

**Repo:** `crates/discovery/src/security/stack.rs::SecurityBuiltinStack::stateless_writer` (`next_message_identity`), `apply_outcome`

**Tests:** `crates/discovery/src/security/stack.rs::tests::the_sequence_number_counts_the_messages_of_the_writer`

**Status:** `done` — the `sequence_number` counts the messages of the stateless writer (1, 2, 3 for three peers), not those to one peer.

### §9.3.2.11.5 Fields of the reply: related_message_identity

**Spec:** §9.3.2.11.5, p. 113 — "The related_message_identity shall be set to
match the message_identity of the ParticipantStatelessMessage received that
triggered the execution of the begin_handshake_reply operation."

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`
(l. 551, `msg.message_identity` as `related`)

**Tests:** `crates/security/src/generic_message.rs::tests::related_message_identity_links_reply_to_request`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** done — for `process_handshake` the code also sets the identity
of the received message; the text of §9.3.2.11.6 names
`begin_handshake_reply` at this point (contradiction in the specification).

### §9.3.2.11.5 Input check before begin_handshake_reply

**Spec:** §9.3.2.11.5, p. 112 — "having received a message on the
BuiltinParticipantMessageReader with attributes set as follows:
message_class_id GMCLASSID_SECURITY_AUTH_HANDSHAKE, message_identity
source_guid matching the GUID_t of the DomainParticipant associated with the
initiator_identity_handle, destination_participant_guid matching the GUID_t
of the receiving DomainParticipant".

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_handshake_message_is_taken_only_as_the_spec_addresses_it`,
`crates/discovery/src/security/stack.rs::tests::the_reply_to_an_undiscovered_initiator_is_addressed_to_its_participant`,
`crates/discovery/src/security/stack.rs::tests::an_undiscovered_request_under_another_prefix_is_refused`

**Status:** `done` — `message_class_id`, `message_identity.source_guid` (the prefix has to be the sender's) and `destination_participant_guid` are checked before `begin_handshake_reply`; a message for another participant is not taken, one with a foreign `source_guid` is an error. The plugin additionally requires a handle with PENDING_HANDSHAKE_MESSAGE.

### §9.3.2.11.6 Input check before process_handshake

**Spec:** §9.3.2.11.6, p. 113 — "related_message_identity matching the
message_identity of the last ParticipantStatelessMessage sent to the peer
DomainParticipant associated with the handshake_handle.
destination_participant_guid matching the GUID_t of the receiving
DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_handshake_message_is_taken_only_as_the_spec_addresses_it`,
`crates/discovery/src/security/stack.rs::tests::a_forged_final_does_not_take_the_handshake_down`

**Status:** `done` — `related_message_identity` has to name the last message sent to the peer and `destination_participant_guid` the receiver; otherwise the message is not given to `process_handshake` and the handshake stays.

### §9.3.2.11 Table 29 — get_shared_secret

**Spec:** §9.3.2.11.7, p. 114–115 — "Retrieves the SharedSecretHandle
resulting with a successfully completed handshake. [...] If an error occurs,
this method shall return the NILHandle and fill the SecurityException."

**Repo:** `crates/security/src/authentication.rs::shared_secret` (l. 175);
`crates/security-pki/src/plugin.rs::shared_secret` (l. 1059)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_shared_secret_returns_stored_value`,
`crates/security-pki/src/plugin.rs::tests::shared_secret_returns_bad_argument_for_unknown_handle`

**Status:** done — the operation is named `shared_secret` (naming).

### §9.3.2.11 Table 29 — get_authenticated_peer_credential_token

**Spec:** §9.3.2.11.8, p. 115 — "Retrieves the
AuthenticatedPeerCredentialToken resulting with a successfully completed
authentication of a discovered DomainParticipant. This operation shall be
called by the DDS middleware on each HandshakeHandle after the handshake
that uses that handle completes successfully".

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::get_authenticated_peer_credential_token`; `crates/security-pki/src/plugin.rs::get_authenticated_peer_credential_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`,
`crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/discovery/src/security/stack.rs::tests::the_peer_credential_token_gives_the_permissions_and_the_certificate`

**Status:** `done` — see §9.3.2.7; a call before the completion is an error.

### §9.3.2.11 Table 29 — set_listener

**Spec:** §9.3.2.11.13, p. 117 — "Sets the AuthenticationListener that the
Authentication plugin will use to notify the DDS middleware infrastructure
of events relevant to the Authentication of DDS Participants."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::set_listener`, `AuthenticationListener`; `crates/security-pki/src/plugin.rs::set_listener`; `crates/discovery/src/security/stack.rs::install_listener`

**Tests:** `crates/security-pki/src/plugin.rs::tests::revoking_an_identity_tells_the_listener_and_invalidates_the_handle`,
`crates/security-pki/src/plugin.rs::tests::an_expired_identity_certificate_is_revoked`,
`crates/security-pki/src/psk.rs::tests::table_83_set_listener_saves_the_listener`,
`crates/discovery/src/security/stack.rs::tests::a_revoked_identity_ends_the_authentication_of_its_participant`

**Status:** `done` — the operation sets the listener (`None` removes it); the discovery stack sets its own.

### §9.3.2.11 Table 29 — return_identity_token

**Spec:** §9.3.2.11.14, p. 117 — "Returns the IdentityToken object to the
plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_identity_token`, `IssuedTokens`; `crates/security-pki/src/plugin.rs::return_identity_token`; `crates/security-pki/src/psk.rs::return_identity_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — the plugin keeps the IdentityTokens it issued and takes back only those, each as often as it was issued; a foreign token or one already returned is an error.

### §9.3.2.11 Table 29 — return_identity_status_token

**Spec:** §9.3.2.11.15, p. 117 — "Returns the IdentityStatusToken object to
the plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_identity_status_token`, `IssuedTokens`; `crates/security-pki/src/plugin.rs::return_identity_status_token`; `crates/security-pki/src/psk.rs::return_identity_status_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — the plugin keeps the IdentityStatusTokens it issued and takes back only those, each as often as it was issued; a foreign token or one already returned is an error.

### §9.3.2.11 Table 29 — return_authenticated_peer_credential_token

**Spec:** §9.3.2.11.16, p. 118 — "Returns the
AuthenticatedPeerCredentialToken object to the plugin so it can be disposed
of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_authenticated_peer_credential_token`, `IssuedTokens`; `crates/security-pki/src/plugin.rs::return_authenticated_peer_credential_token`; `crates/security-pki/src/psk.rs::return_authenticated_peer_credential_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — the plugin keeps the AuthenticatedPeerCredentialTokens it issued and takes back only those, each as often as it was issued; a foreign token or one already returned is an error.

### §9.3.2.11 Table 29 — return_handshake_handle

**Spec:** §9.3.2.11.17, p. 118 — "Returns the HandshakeHandle object to the
plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_handshake_handle`; `crates/security-pki/src/plugin.rs::return_handshake_handle`; `crates/discovery/src/security/stack.rs::release_peer_handles`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/discovery/src/security/stack.rs::tests::a_handshake_that_does_not_complete_ends_with_the_authentication_timeout`

**Status:** `done` — the plugin takes an issued handle back once and releases the state of the handshake (request, reply, association with the secret, token of the peer); the stack gives the handles of an ended authentication and of a lost participant back.

### §9.3.2.11 Table 29 — return_identity_handle

**Spec:** §9.3.2.11.18, p. 118 — "Returns the IdentityHandle object to the
plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_identity_handle`; `crates/security-pki/src/plugin.rs::return_identity_handle`; `crates/security-pki/src/psk.rs::return_identity_handle`; `crates/discovery/src/security/stack.rs::release_peer_handles`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_returned_identity_handle_is_disposed_of`,
`crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`

**Status:** `done` — the plugin takes the handles of `validate_local_identity` and `validate_remote_identity` back once each and releases the state (version of the peer, GUID, challenges, role).

### §9.3.2.11 Table 29 — return_sharedsecret_handle

**Spec:** §9.3.2.11.19, p. 118 — "Returns the SharedSecretHandle object to
the plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_sharedsecret_handle`; `crates/security-pki/src/plugin.rs::return_sharedsecret_handle`; `crates/security-pki/src/psk.rs::return_sharedsecret_handle`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — a secret a completed handshake returned goes back once; the bytes and the challenges are gone afterwards. The stack does it only when the peer is lost (the crypto plugin reads the secret through the `SharedSecretProvider` as long as the peer is there).

### §9.3.2.11 Own operations outside Table 29

**Spec:** §9.3.2.11, Table 29, p. 105–107 — the table lists the operations
of the interface completely.

**Repo:** `crates/security/src/authentication.rs::AuthenticationExtensions`, `AuthenticationPlugin`

**Tests:** `crates/security/src/authentication.rs::tests::stub_can_be_boxed`

**Status:** `done` — `AuthenticationPlugin` has exactly the operations of Table 29. What is needed beyond them is in the supertrait `AuthenticationExtensions` and reasoned there: `plugin_class_id` (name for the log), `set_algo_nul_terminate`, `set_remote_message_auth_supported` and `set_peer_algorithm_names` (spelling per vendor of the peer), `get_permissions_token` (the token for `PID_PERMISSIONS_TOKEN`, until AccessControl supplies it) and `check_identities` (the clock: the interface of Table 29 has no operation that brings the time, the plugin needs it for expired certificates). `peer_permissions`, `peer_identity_certificate`, `local_auth_request_token`, `algorithm_masks` and `set_local_participant_data` are gone.

### §9.3.2.11.1 Table 30 — ValidationResult_t

**Spec:** §9.3.2.11.1, Table 30, p. 108 — VALIDATION_OK, VALIDATION_FAILED,
VALIDATION_PENDING_RETRY, VALIDATION_PENDING_HANDSHAKE_REQUEST,
VALIDATION_PENDING_HANDSHAKE_MESSAGE, VALIDATION_OK_FINAL_MESSAGE.

**Repo:** `crates/security/src/authentication.rs::ValidationResult`, `HandshakeStepOutcome::validation_result`

**Tests:** `crates/security/src/authentication.rs::tests::a_handshake_outcome_is_the_validation_result_of_table_30`,
`crates/security/src/authentication.rs::tests::only_ok_and_ok_final_message_succeed`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_orders_the_guids_like_table_59`

**Status:** `done` — all six values; VALIDATION_FAILED is the error case `Err`, `ValidationResult::Failed` names it.

### §9.3.2.12 Table 31 — AuthenticationListener::on_revoke_identity

**Spec:** §9.3.2.12.2, p. 119–120 — "Revokes the identity of the participant
identified by the IdentityHandle. The corresponding IdentityHandle becomes
invalid. As a result of this, the DDS middleware shall terminate any
communications with the DomainParticipant associated with that handle."

**Repo:** `crates/security/src/authentication.rs::AuthenticationListener`; `crates/security-pki/src/plugin.rs::revoke_identity`, `check_identities`; `crates/discovery/src/security/stack.rs::drain_auth_events`; `crates/dcps/src/runtime.rs::process_authentication_events`

**Tests:** `crates/security-pki/src/plugin.rs::tests::revoking_an_identity_tells_the_listener_and_invalidates_the_handle`,
`crates/security-pki/src/plugin.rs::tests::an_expired_identity_certificate_is_revoked`,
`crates/security-pki/src/plugin.rs::tests::a_crl_that_lists_the_peer_revokes_its_identity`,
`crates/discovery/src/security/stack.rs::tests::a_revoked_identity_ends_the_authentication_of_its_participant`,
`crates/dcps/src/runtime.rs::tests::a_revoked_identity_ends_the_authentication_through_the_runtime_tick`

**Status:** `done` — the plugin calls `on_revoke_identity` when an identity is revoked (`revoke_identity`, expired certificate, new CRL); the handle is invalid afterwards, the handshake and the secret go with it. The stack ends the authentication of the participant, does not authenticate it again, and reports it; the runtime ignores the participant (endpoints and matches go) and releases its keys.

### §9.3.2.12.2 New revision of the key material after on_revoke_identity

**Spec:** §9.3.2.12.2, p. 119 — "The DDS middleware shall create a new
revision of any Key Material that it had shared with the DomainParticipant
identified by the IdentityHandle and send the revised Key Material to the
remaining DomainParticipant entities [...] DataReaders with DURABILITY kind
different from VOLATILE shall retail at least the last 8 revisions of the Key
Material. Likewise, DataWriters shall not send messages that use KeyMaterial
that is more than 7 revisions earlier than the current."

**Repo:** `crates/dcps/src/runtime.rs::process_authentication_events`, `renew_key_material_after_revocation`; `crates/security-runtime/src/shared.rs::SharedSecurityGate::revise_local_entity_keys`, `activate_key_revision`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_revocation_makes_a_new_revision_of_the_keys_of_the_participant_and_its_endpoints`

**Status:** `done` — after `on_revoke_identity` the runtime calls `revise_local_entity_keys` (§9.5.1.8.7): the keys of the participant and of all local endpoints get a new revision. The participant token goes to the remaining authenticated participants at once, the endpoint tokens at the next tick; `activate_key_revision` (§9.5.1.8.8) follows. The revoked participant holds the earlier revision only and cannot decode the new one. The activation does not wait for the endpoint tokens.

### §9.3.2.12 Table 31 — AuthenticationListener::on_status_changed

**Spec:** §9.3.2.12.3, p. 120 — "Informs the DomainParticipant that a status
associated with the Authentication plugin, or an Identity managed by the
plugin has changed."

**Repo:** `crates/security/src/authentication.rs::AuthenticationListener::on_status_changed`; `crates/security-pki/src/plugin.rs::check_identities`; `crates/discovery/src/security/stack.rs::drain_auth_events`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_changed_status_token_is_reported_as_identity_status`,
`crates/discovery/src/security/stack.rs::tests::a_status_change_is_passed_on_with_the_new_token`

**Status:** `done` — the plugin calls the operation when the IdentityStatusToken changes; the stack notes the change, the runtime fetches the new token.

### §9.3.2.12.1 Table 32 — AuthStatusKind IDENTITY_STATUS

**Spec:** §9.3.2.12.1, Table 32, p. 119 — "IDENTITY_STATUS Indicates a
change to an identity status. [...] The changed IdentityStatusToken shall be
propagated by the DDS implementation to the other DomainParticipants using
the DCPSParticipantsSecure builtin Topic."

**Repo:** `crates/security/src/authentication.rs::AuthStatusKind`; `crates/discovery/src/security/stack.rs::take_identity_status_changed`; `crates/dcps/src/runtime.rs::process_authentication_events`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_changed_status_token_is_reported_as_identity_status`,
`crates/discovery/src/security/stack.rs::tests::a_status_change_is_passed_on_with_the_new_token`

**Status:** `done` — `AuthStatusKind::IdentityStatus`; a changed token goes into the beacon and is sent in the secure SPDP (DCPSParticipantsSecure) to the authenticated participants.

---

### §9.4.1 Background (Non-Normative)

**Spec:** §9.4.1, p. 120 — "Once a DomainParticipant is authenticated, its
permissions need to be validated and enforced."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — background.

### §9.4.2 AccessControl Plugin Model

**Spec:** §9.4.2, Figure 10, p. 120–121 — interfaces AccessControl and
AccessControlListener, types PermissionsToken, PermissionsHandle,
ParticipantSecurityConfig, TopicSecurityConfig, EndpointSecurityConfig,
ParticipantSecurityAlgorithmInfo, EndpointSecurityAlgorithmInfo.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — diagram; the elements are listed on their
own below.

### §9.4.2 The runtime decides without AccessControlPlugin

**Spec:** §9.4.2.9, Table 39, p. 129 — "AccessControl [...] Operations"; §2.3:
"Clause 9 (the plugin model) [...] The conformance to the plugin model is at
the UML level".

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate` (holds a `Box<dyn AccessControlPlugin>`, `application_access_control`), `crates/dcps/src/runtime.rs`, `crates/dcps/src/participant.rs`, `crates/dcps/src/publisher.rs`, `crates/dcps/src/access_request.rs`

**Tests:** `crates/security-runtime/src/shared.rs::tests::a_plugin_of_the_application_decides_every_check`,
`crates/security-runtime/src/shared.rs::tests::a_refusing_plugin_of_the_application_refuses_every_check`,
`crates/dcps/tests/security_access_plugin_e2e.rs::create_topic_is_asked_of_the_plugin`,
`crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`

**Status:** `done` — every decision of the runtime (participant, topic, endpoint, instance, match, remote participants and endpoints, configs, tokens) goes through `dyn AccessControlPlugin`. Without an application plugin the gate builds the builtin one from governance, permissions, CA and identity certificate. An application plugin replaces it (`set_access_control`).

### §9.4.2.1 PermissionsToken

**Spec:** §9.4.2.1, p. 121 — "A PermissionsToken contains summary
information on the permissions for a DomainParticipant in a manner that can
be externalized and propagated over DDS discovery."

**Repo:** `crates/security/src/token.rs` (alias `PermissionsToken`,
l. 247–268); `crates/security-pki/src/plugin.rs::get_permissions_token`
(l. 806)

**Tests:** `crates/security/src/token.rs::tests::permissions_token_roundtrip`,
`crates/security-pki/src/plugin.rs::tests::permissions_token_announces_spec_class_id_when_permissions_set`

**Status:** done — produced by the Authentication plugin, not by
AccessControl (see Table 39 — get_permissions_token); the content is in
§10.4.

### §9.4.2.2 PermissionsCredentialToken

**Spec:** §9.4.2.2, p. 122 — "A PermissionsCredentialToken encodes the
permissions and access information for a DomainParticipant in a manner that
can be externalized and sent over the network. [...] The
PermissionsCredentialToken is intended for dissemination during the
authentication handshake."

**Repo:** `crates/security-permissions/src/plugin.rs::get_permissions_credential_token` (Table 64: `dds.perm.cert`), `validate_remote_permissions` (liest `c.perm`)

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`

**Status:** `done` — the plugin creates the `DDS:Access:PermissionsCredential` token with the signed document and reads it of the remote participant from the `AuthenticatedPeerCredentialToken` (`c.perm`, §10.3.2.3). The runtime takes it through `stack.peer_credential_token`.

### §9.4.2.3 PermissionsHandle

**Spec:** §9.4.2.3, p. 122 — "A PermissionsHandle is an opaque local
reference to internal state within the AccessControl plugin."

**Repo:** `crates/security/src/access_control.rs::PermissionsHandle` (l. 30)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::permit_on_exact_match`

**Status:** done — the runtime uses no handle but keeps the documents in
`SharedSecurityGate` (see above).

### §9.4.2.4 ParticipantSecurityConfig: type

**Spec:** §9.4.2.4, p. 122 — "@extensibility (APPENDABLE) struct
ParticipantSecurityConfig { boolean allow_unauthenticated_participants;
boolean is_access_protected; boolean is_rtps_axk_protected; boolean
is_rtps_psk_protected; boolean is_discovery_protected; boolean
is_liveliness_protected; boolean is_key_revision_enabled;
PluginParticipantSecurityAttributesMask plugin_participant_attributes;
PropertySeq ac_endpoint_properties; ParticipantSecurityAlgorithmInfo
algorithm_info; };"

**Repo:** `crates/security/src/access_control.rs::ParticipantSecurityConfig` (+ `ParticipantSecurityAlgorithmInfo`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/security/src/access_control.rs::tests::the_participant_mask_has_the_bits_of_table_34`

**Status:** `done` — the type has every member of §9.4.2.4 including `is_rtps_psk_protected`, `is_key_revision_enabled`, `ac_participant_properties` and `algorithm_info`; both plugins fill it.

### §9.4.2.4 Table 33 — allow_unauthenticated_participants

**Spec:** §9.4.2.4, Table 33, p. 122–123 — "If
allow_unauthenticated_participants is TRUE, the DomainParticipant shall allow
matching other DomainParticipants—even if the remote DomainParticipant
cannot authenticate [...] If allow_unauthenticated_participants is FALSE,
the DomainParticipant shall enforce the authentication of remote
DomainParticipants and disallow matching those that cannot be successfully
authenticated."

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`; `crates/dcps/src/runtime.rs::builtin_match`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_sedp_of_a_peer_is_matched_once_it_authenticated`,
`crates/dcps/src/runtime.rs::tests::an_unauthenticated_peer_gets_the_regular_sedp_only`,
`crates/security-runtime/src/shared.rs::tests::allow_unauthenticated_reads_true_when_set`

**Status:** `done` — the value comes from the plugin, decides the matching of §9.8.3.1/§9.8.3.2 and the acceptance of plaintext. The kick-out on the same GUID is its own item below.

### §9.4.2.4 allow_unauthenticated_participants: same GUID displaces

**Spec:** §9.4.2.4, Table 33, p. 122 — "as long as there is not already a
valid authentication with the same DomainParticipant's GUID. Additionally, a
DomainParticipant that later authenticates would kick out the
unauthenticated DomainParticipant if it has the same GUID."

**Repo:** `crates/dcps/src/runtime.rs::displace_unauthenticated_endpoints`, `store_remote_permissions`, `spdp_plain_is_liveliness_only`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_authenticating_participant_displaces_the_unauthenticated_one_with_its_guid`

**Status:** `done` — with `allow_unauthenticated_participants` TRUE, `store_remote_permissions` drops at the end of the handshake everything learned of the GUID over the regular discovery before it authenticated (endpoints in the SEDP cache and their matches); the endpoints of the authenticated participant come over the secure channels. A plaintext announcement of an already authenticated GUID renews the lease only (`spdp_plain_is_liveliness_only`).

### §9.4.2.4 Plaintext over loopback also with allow_unauthenticated_participants=FALSE

**Spec:** §9.4.2.4, Table 33, p. 123 — "If allow_unauthenticated_participants
is FALSE, the DomainParticipant shall enforce the authentication of remote
DomainParticipants".

**Repo:** `crates/security-runtime/src/shared.rs::classify_inbound`,
`::set_plaintext_loopback`; setting `zerodds.sec.plaintext_loopback`

**Tests:** `crates/security-runtime/src/shared.rs::tests::classify_inbound_plain_on_loopback_needs_the_setting`,
`crates/dcps/src/runtime.rs::tests::inbound_loopback_plaintext_needs_the_setting`

**Status:** `done` — plaintext from loopback counts as plaintext from any other
interface. It is accepted only with `zerodds.sec.plaintext_loopback` (opt-in)
or without a security configuration. Measured: green on the Linux test host.

### §9.4.2.4 Table 33 — is_access_protected

**Spec:** §9.4.2.4, Table 33, p. 123 — "If is_access_protected is TRUE, then
the DDS middleware shall call and get_authenticated_peer_credential_token,
validate_remote_permissions, and check_remote_participant operations on the
matched and authenticated remote DomainParticipant. Any failure in these
calls will result in failing to authorize the remote participant, which shall
be removed by the local participant. If is_access_protected is FALSE [...] a
HandleNIL return from these operations will not prevent authorization."

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`; `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `crates/dcps/tests/security_join_access_e2e.rs::a_participant_the_permissions_refuse_is_ignored_when_the_domain_is_access_protected`,
`crates/dcps/tests/security_join_access_e2e.rs::a_participant_is_not_checked_when_the_domain_is_not_access_protected`,
`crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — the plugin returns `is_access_protected` from `enable_join_access_control`; with TRUE the runtime calls `validate_remote_permissions` and `check_remote_participant` after the handshake and ignores a participant whose check fails (`ignore_unauthorized_participant`): its writer is never matched. With FALSE nothing is checked. Measured with three running runtimes over UDP (log `sec-access-1018-112603.log`). The test found that the exclusion lapsed with the next SPDP on a runtime without a `DomainParticipant`; fixed.

### §9.4.2.4 Table 33 — is_rtps_axk_protected

**Spec:** §9.4.2.4, Table 33, p. 123 — "Indicates whether RTPS
Non-Bootstrapping Messages (7.5.7) should be protected with a Participant
Key created by the sending DomainParticipant and exchanged
post-authentication. [...] (4) All RTPS non-bootstrapping messages sent by
the DomainParticipant to matched DomainParticipants shall be transformed
using the CryptoTransform operation encode_rtps_message (5) All RTPS
non-bootstrapping messages received shall be transformed using the
CryptoTransform operation decode_rtps_message."

**Repo:** `crates/security-runtime/src/shared.rs::rtps_protection`, `transform_outbound_for`, `transform_inbound_from`; `crates/security-permissions/src/governance.rs::parse_domain_rule`

**Tests:** `crates/security-runtime/src/shared.rs::tests::transform_outbound_for_encrypt_produces_srtps_wire`,
`crates/security-runtime/src/shared.rs::tests::transform_inbound_from_looks_up_slot_by_guid`,
`crates/security-permissions/src/governance.rs::tests::rtps_protection_with_unauthenticated_participants_is_refused`

**Status:** `done` — whole messages are protected by `rtps_protection_kind`; condition (1) is checked when the governance is read (item below).

### §9.4.2.4 is_rtps_axk_protected requires allow_unauthenticated_participants=FALSE

**Spec:** §9.4.2.4, Table 33, p. 123 — "If is_rtps_axk_protected is TRUE
then: (1) allow_unauthenticated_participants must be FALSE."

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_rule`

**Tests:** `crates/security-permissions/src/governance.rs::tests::rtps_protection_with_unauthenticated_participants_is_refused`

**Status:** `done` — `rtps_protection_kind` other than NONE with `allow_unauthenticated_participants` TRUE makes the governance invalid.

### §9.4.2.4 Table 33 — is_rtps_psk_protected

**Spec:** §9.4.2.4, Table 33, p. 123–124 — "Indicates whether all RTPS
Messages (including RTPS Bootstrapping Messages) should be protected: RTPS
Messages that are not otherwise protected by an 'Authenticated Partcipant
Exchanged Key' will be protected with a Pre-Shared Key. [...] If
is_rtps_psk_protected is FALSE, then: (1) RTPS Bootstrapping Messages sent
shall NOT be transformed".

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`; `crates/security-runtime/src/qos_properties.rs::rtps_psk_from_properties`; `crates/security-runtime/src/psk_gate.rs`; `crates/dcps/src/runtime.rs::secure_outbound_bytes`, `secure_inbound_bytes`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_governance_decides_the_pre_shared_key_protection`, `crates/security-runtime/src/psk_gate.rs::tests::the_gate_protects_a_message_with_the_pre_shared_key`, `crates/dcps/src/runtime.rs::tests::a_pre_shared_key_protects_the_bootstrapping_messages`

**Status:** `done` — The `rtps_psk_protection_kind` of the governance (otherwise the property of Table 84) decides whether and how the gate transforms the RTPS messages with the pre-shared key (§10.9); with the key set, bootstrapping messages go out with flags A and P and unprotected ones are refused. The enclave path without a property list has no passphrase and does not protect. run `cargo test -p zerodds-security-runtime --lib` and `-p zerodds-dcps --features security --lib`: green, logs `/root/zerodds-rc7/agent-logs/sec-crypto-1018-085015.log`, `…085119.log`.

### §9.4.2.4 Table 33 — is_discovery_protected

**Spec:** §9.4.2.4, Table 33, p. 124 — "If is_discovery_protected is TRUE,
the submessages sent by the DCPSPublicationsSecure and
DCPSSubscriptionsSecure DataWriters shall be transformed using the
CryptoTransform operation encode_datawriter_submessage [...] Independent of
the setting of is_discovery_protected, the CryptoTransform operations
encode_serialized_payload and decode_serialized_payload shall never be called
for the DCPSPublicationsSecure and DCPSSubscriptionsSecure entities."

**Repo:** `crates/security-runtime/src/shared.rs::discovery_protection`
(l. 537); `crates/dcps/src/runtime.rs::protect_secure_spdp` (l. 17028)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_discovery_protection`

**Status:** done — the protection of the submessages of the secure SEDP
endpoints follows `discovery_protection_kind`; the runtime protects the
SerializedPayload only for application writers (comment in
`crates/dcps/src/runtime.rs`, l. 4704–4712).

### §9.4.2.4 Table 33 — is_liveliness_protected

**Spec:** §9.4.2.4, Table 33, p. 124–125 — "If is_liveliness_protected is
TRUE, the submessages sent by the BuiltinParticipantMessageSecure DataWriter
shall be transformed using the CryptoTransform operation
encode_datawriter_submessage [...] the CryptoTransform operations
encode_serialized_payload and decode_serialized_payload shall never be called
for the BuiltinParticipantMessageSecure entities."

**Repo:** `crates/security-runtime/src/shared.rs::liveliness_protection`
(l. 622)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_liveliness_protection`

**Status:** done

### §9.4.2.4 Table 33 — is_key_revision_enabled

**Spec:** §9.4.2.4, Table 33, p. 125 — "Indicates the DDS middleware will
revise Key Material for its entities when certain events are encountered
(e.g. the identity of a matched DomainParticipant is revoked)."

**Repo:** `crates/security-runtime/src/shared.rs::key_revision_enabled`; `crates/dcps/src/runtime.rs::renew_key_material_after_revocation`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_revocation_makes_a_new_revision_of_the_keys_of_the_participant_and_its_endpoints`, `crates/dcps/src/runtime.rs::tests::a_revocation_makes_no_revision_unless_the_governance_enables_it`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`

**Status:** `done` — With `<enable_key_revision>` the runtime makes a new revision (`revise_local_entity_keys`) when an identity is revoked, sends it to the remaining participants and activates it (`activate_key_revision`); without the element nothing happens. run `cargo test -p zerodds-security-runtime --lib` and `-p zerodds-dcps --features security --lib`: green, logs `/root/zerodds-rc7/agent-logs/sec-crypto-1018-085015.log`, `…085119.log`.

### §9.4.2.4 Table 33 — plugin_participant_attributes

**Spec:** §9.4.2.4, Table 33, p. 125 — "This field is a holder for
plugin-specific information that is propagated via discovery as part of the
ParticipantSecurityInfo".

**Repo:** `crates/rtps/src/participant_security_info.rs::plugin_attrs`
(l. 33); `crates/dcps/src/runtime.rs` (l. 8722, set in the beacon)

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::fully_protected_sets_all_encrypt_bits`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done — the runtime derives the bits from the governance, not a
plugin.

### §9.4.2.4 Table 33 — ac_participant_properties

**Spec:** §9.4.2.4, Table 33, p. 125 — "Additional properties to add to the
participant_properties parameter passed to the CryptoKeyFactory operation
register_local_participant."

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`; `crates/security-runtime/src/shared.rs::ensure_local` (passes the properties to `register_local_participant`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — the plugin returns the properties; the gate hands them to the CryptoKeyFactory in `participant_properties`. The IDL of the spec names the member `ac_endpoint_properties`, Table 33 `ac_participant_properties` (contradiction in the spec); the code uses `ac_participant_properties`.

### §9.4.2.4 Table 33 — algorithm_info

**Spec:** §9.4.2.4, Table 33, p. 125 — "Cryptographic algorithms used and
supported by the participant. See 7.3.14."

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`, `governance.rs::AllowedCryptoAlgorithms`; `crates/dcps/src/runtime.rs` (SPDP)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::without_allowed_algorithms_every_algorithm_is_supported`,
`crates/security-permissions/src/plugin.rs::tests::the_identity_certificates_algorithms_must_be_allowed`,
`crates/dcps/src/runtime.rs::tests::the_allowed_algorithms_cut_the_announced_ones`

**Status:** `done` — `supported_mask` and `required_mask` follow `<allowed_crypto_algorithms>`; without the element CRYPTO_ALGORITHM_SET_ALL applies (Table 70). The identity certificate must use the allowed algorithms. The runtime announces the masks in the SPDP.

### §9.4.2.5 Table 34 — mapping to ParticipantSecurityAttributesMask

**Spec:** §9.4.2.5, Table 34, p. 125–126 — is_rtps_axk_protected
(0x1 << 0), is_discovery_protected (0x1 << 1), is_liveliness_protected
(0x1 << 2), is_key_revision_enabled (0x1 << 3), is_rtps_psk_protected
(0x1 << 4), is_valid (0x1 << 31).

**Repo:** `crates/rtps/src/participant_security_info.rs::attrs`; `crates/dcps/src/runtime.rs` (SPDP from the plugin configuration)

**Tests:** `crates/security/src/access_control.rs::tests::the_participant_mask_has_the_bits_of_table_34`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — all bits of Table 34 (0, 1, 2, 3, 4, 31) come from `ParticipantSecurityConfig::attributes_mask`.

### §9.4.2.5 IS_KEY_REVISION_ENABLED and IS_RTPS_PSK_PROTECTED missing

**Spec:** §9.4.2.5, Table 34, p. 126 — "#define
PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_KEY_REVISION_ENABLED (0x00000001 <<
3) [...] #define PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_PROTECTED
(0x00000001 << 4)".

**Repo:** `crates/security/src/access_control.rs::participant_attributes_mask`; `crates/rtps/src/participant_security_info.rs::attrs`

**Tests:** `crates/security/src/access_control.rs::tests::the_participant_mask_has_the_bits_of_table_34`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — both bits are set from the governance (`enable_key_revision`, `rtps_psk_protection_kind`) and announced.

### §9.4.2.5 Table 35 — ParticipantSecurityOptionalAttributesMask

**Spec:** §9.4.2.5, Table 35, p. 126 —
PARTICIPANT_SECURITY_OPT_ATTRIBUTES_FLAG_ALLOW_UNAUTHENTICATED_PARTICIPANTS
(0x0001 << 0), PARTICIPANT_SECURITY_OPT_ATTRIBUTES_FLAG_IS_ACCESS_PROTECTED
(0x0001 << 1), "applies to both is_set and value fields".

**Repo:** `crates/security/src/access_control.rs::participant_optional_attributes_mask`; `crates/rtps/src/participant_security_info.rs::opt_attrs`; `crates/security-runtime/src/qos_properties.rs::PROP_OPTIONAL_ATTRIBUTES`

**Tests:** `crates/security/src/access_control.rs::tests::the_optional_mask_has_the_bits_of_table_35`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — the third member `participant_security_optional_attributes` is sent by default (value from `allow_unauthenticated_participants` and `is_access_protected`). With `org.zerodds.sec.optional_attributes=omit` it is left out. Not measured against Cyclone DDS, Fast DDS and OpenDDS; expected: peers that read two masks only ignore the member.

### §9.4.2.6 TopicSecurityConfig: type

**Spec:** §9.4.2.6, p. 126 — "@extensibility (APPENDABLE) struct
TopicSecurityConfig { boolean is_read_protected; boolean is_write_protected;
boolean is_discovery_protected; boolean is_liveliness_protected; };"

**Repo:** `crates/security/src/access_control.rs::TopicSecurityConfig`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`

**Status:** `done` — the type has the four members; the plugin fills them from the topic rule.

### §9.4.2.6 Table 36 — is_read_protected

**Spec:** §9.4.2.6, Table 36, p. 126 — "Indicates if read access to the
Topic is protected. If is_read_protected is FALSE, then local DataReader
creation and remote DataReader matching can proceed without further
access-control mechanisms imposed. Otherwise, they shall be checked using
the AccessControl operations."

**Repo:** `crates/security-permissions/src/governance.rs::read_access_controlled`
(l. 414); `crates/security-runtime/src/shared.rs::check_create_datareader`
(l. 441), `check_remote_datareader` (l. 362)

**Tests:** `crates/security-permissions/src/governance.rs::tests::access_control_flags_come_from_the_matching_rule`,
`crates/security-permissions/src/governance.rs::tests::an_uncontrolled_topic_needs_no_grant`

**Status:** done — the check of the remote reader is assessed under
Table 39 — check_remote_datareader.

### §9.4.2.6 Table 36 — is_write_protected

**Spec:** §9.4.2.6, Table 36, p. 127 — "Indicates if write access to the
Topic is protected. If is_write_protected is FALSE, then local DataWriter
creation and remote DataWriter matching can proceed without further
access-control mechanisms imposed."

**Repo:** `crates/security-permissions/src/governance.rs::write_access_controlled`
(l. 405); `crates/security-runtime/src/shared.rs::check_create_datawriter`
(l. 406), `check_remote_datawriter` (l. 326)

**Tests:** `crates/security-permissions/src/governance.rs::tests::access_control_flags_come_from_the_matching_rule`,
`crates/security-permissions/src/governance.rs::tests::the_granted_direction_still_creates`

**Status:** done

### §9.4.2.6 Table 36 — is_discovery_protected per topic

**Spec:** §9.4.2.6, Table 36, p. 127 — "If is_discovery_protected is TRUE,
then discovery information for that entity shall be sent using the
SEDPbuiltinPublicationsSecureWriter SEDPbuiltinSubscriptionsSecureWriter. If
is_discovery_protected is FALSE, then discovery information for that entity
shall be sent using the SEDPbuiltinPublicationsWriter or
SEDPbuiltinSubscriptionsWriter."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`, `SharedSecurityGate::topic_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — `is_discovery_protected` of the topic selects the SEDP endpoints and enters the mask. TypeLookup: see the item below.

### §9.4.2.6 Table 36 — is_discovery_protected and TypeLookup

**Spec:** §9.4.2.6, Table 36, p. 127 — "Also impacts which Types can be
looked-up using the regular (non-secure) Builtin TypeLookup Endpoints and
which require use of the Builtin Secure TypeLookup Endpoints".

**Repo:** `crates/dcps/src/runtime.rs::hidden_from_regular_type_lookup`, `without_hidden_types`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_regular_type_lookup_does_not_hand_out_the_types_of_protected_topics`

**Status:** `done` — the regular TypeLookup endpoints do not hand out the types of topics with `is_discovery_protected`; the secure endpoints do.

### §9.4.2.6 Table 36 — is_liveliness_protected per topic


**Spec:** §9.4.2.6, Table 36, p. 127 — "If is_liveliness_protected is TRUE
then the liveliness heartbeats are sent using the
BuiltinParticipantMessageSecure builtin Topic. Otherwise they are sent using
the BuiltinParticipantMessage builtin Topic."

**Repo:** `crates/dcps/src/wlp.rs::WlpEndpoint::set_channels`,
`crates/dcps/src/runtime.rs::wlp_channels`,
`crates/dcps/src/runtime.rs::protect_wlp_outbound`,
`crates/security-runtime/src/shared.rs::SharedSecurityGate::topic_liveliness_protected`

**Tests:**
`crates/dcps/src/wlp.rs::tests::the_plain_and_the_secure_writer_beat_with_sequence_numbers_of_their_own`,
`crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`,
`crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`

**Status:** `done` — the participant beats on the plain writer for the writers
of its topics that are not sensitive and on the secure writer for those of the
sensitive ones (`is_liveliness_protected` of the rule of the topic); with topics
of both kinds it beats on both, each writer with its own sequence numbers,
history and reader proxies. Without a writer, `liveliness_protection_kind` of
the domain decides. Restored: the choice of `62017f49303` was lost in the merge
`13cb1667912` when the WLP endpoint of wip/dcps-1.4 (a `ReliableWriter` with
reader proxies) was taken; it now sits on that endpoint (`WlpEndpoint` holds the
plain and the secure `ReliableWriter`). The plain WLP DATA is not protected with
the key of the secure writer. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-073220.log`).

### §9.4.2.7 EndpointSecurityConfig: type

**Spec:** §9.4.2.7, p. 127 — "@extensibility (APPENDABLE) struct
EndpointSecurityConfig : TopicSecurityConfig { boolean
is_submessage_protected; boolean is_payload_protected; boolean
is_key_protected; PluginEndpointSecurityAttributesMask
plugin_endpoint_attributes; PropertySeq ac_endpoint_properties;
EndpointSecurityAlgorithmInfo algorithm_info; };"

**Repo:** `crates/security/src/access_control.rs::EndpointSecurityConfig`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security/src/access_control.rs::tests::the_endpoint_mask_has_the_bits_of_table_38`

**Status:** `done` — the type has every member of §9.4.2.7; the endpoint mask is derived from it.

### §9.4.2.7 Table 37 — is_submessage_protected

**Spec:** §9.4.2.7, Table 37, p. 128 — "If is_submessage_protected is TRUE
and the entity is a DataWriter, the submessages sent by the DataWriter shall
be transformed using the CryptoTransform operation
encode_datawriter_submessage [...] and the entity is a DataReader, the
submessages sent by the DataReader shall be transformed using the
CryptoTransform operation encode_datareader_submessage".

**Repo:** `crates/security-runtime/src/shared.rs::metadata_protection`
(l. 515); `crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info`
(l. 2087)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** done — which submessages of a reader are protected is covered
in §7.4.7 and §10.5.

### §9.4.2.7 Table 37 — is_payload_protected

**Spec:** §9.4.2.7, Table 37, p. 128 — "If is_ payload_protected is TRUE and
the entity is a DataWriter, the serialized data sent by the DataWriter shall
be transformed by calling encode_serialized_payload. If is_
payload_protected is TRUE and the entity is a DataReader, the serialized
data received by the DataReader shall be transformed by calling
decode_serialized_payload".

**Repo:** `crates/security-runtime/src/shared.rs::data_protection` (l. 496),
`encode_serialized_payload` (l. 1024), `decode_serialized_payload`
(l. 1040)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** done

### §9.4.2.7 Table 37 — is_key_protected

**Spec:** §9.4.2.7, Table 37, p. 128 — "If is_key_protected is TRUE, then the
DDS middleware shall compute the KeyHash for the Instance Key as described
in section 7.4.4."

**Repo:**
`crates/security-permissions/src/plugin.rs::get_datawriter_security_config`,
`crates/security-runtime/src/plugin_attributes.rs`,
`crates/dcps/src/runtime.rs::payload_key_hash`

**Tests:**
`crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security-runtime/src/plugin_attributes.rs::tests::key_protected_follows_data_encrypt_per_spec_10_4_1_2_6`,
`crates/dcps/src/runtime.rs::tests::the_key_hash_of_an_encrypted_payload_is_the_md5_of_the_key`

**Status:** `done` — the member is set from `data_protection_kind` ENCRYPT
(§10.4.1.2.6), and under that protection every sample and every
dispose/unregister of a type with a key carries the KeyHash as the MD5 of the
key holder (§7.4.4, also for keys up to 16 octets; unknown key holder:
`NotAllowedBySecurity`); `payload_key_hash` applies to exactly the endpoints
whose `is_key_protected` is TRUE that way. Tests green on the Linux test host
(`/root/zerodds-rc7/agent-logs/security-1018-083929.log`,
`/root/zerodds-rc7/agent-logs/security-1018-082254.log`).

### §9.4.2.7 Table 37 — plugin_endpoint_attributes

**Spec:** §9.4.2.7, Table 37, p. 128 — "This field is a holder for
plugin-specific information that is propagated via discovery as part of the
EndpointSecurityInfo".

**Repo:** `crates/rtps/src/endpoint_security_info.rs::plugin_attrs` (l. 56);
`crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info` (l. 2087)

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::protection_bit_accessors_read_correctly`

**Status:** done — the runtime sets the bits of the standard mask (`user_endpoint_security_info`); `security_runtime::plugin_attributes` makes the mask of the plugin (§7.3.23, §7.3.24: the plugin's own), and the runtime passes it on.

### §9.4.2.7 Table 37 — ac_endpoint_properties

**Spec:** §9.4.2.7, Table 37, p. 128–129 — "Additional properties to add to
the datawriter_properties or datareader_properties passed to the
CryptoKeyFactory operations register_local_datawriter and
register_local_datareader."

**Repo:** `crates/security-permissions/src/plugin.rs::get_datawriter_security_config`; `crates/security-runtime/src/shared.rs::register_local_endpoint` (properties to the CryptoKeyFactory); `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`

**Status:** `done` — the plugin returns the properties; the runtime hands them to `register_local_datawriter` and `register_local_datareader`.

### §9.4.2.7 Table 37 — algorithm_info

**Spec:** §9.4.2.7, Table 37, p. 129 — "Cryptographic algorithms required to
interoperate with the endpoint. See 7.3.16."

**Repo:** `crates/security-runtime/src/gate_crypto.rs::endpoint_required_mask`; `crates/dcps/src/runtime.rs::local_endpoint_cipher`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_announced_cipher_is_the_algorithm_info_of_the_plugin`, `crates/security-runtime/src/shared.rs::tests::the_basic_protection_kind_of_an_endpoint_decides_gmac_or_gcm`

**Status:** `done` — The `required_mask` of the `adjusted_algorithm_info` the plugin returns from `register_local_datawriter`/`_datareader` is the announcement as PID 0x1013 (only when it differs from the default of the participant, §7.3.15.2). run `cargo test -p zerodds-security-runtime --lib` and `-p zerodds-dcps --features security --lib`: green, logs `/root/zerodds-rc7/agent-logs/sec-crypto-1018-085015.log`, `…085119.log`.

### §9.4.2.8 Table 38 — mapping to EndpointSecurityAttributesMask

**Spec:** §9.4.2.8, Table 38, p. 129 — is_read_protected (0x1 << 0),
is_write_protected (0x1 << 1), is_discovery_protected (0x1 << 2),
is_submessage_protected (0x1 << 3), is_payload_protected (0x1 << 4),
is_key_protected (0x1 << 5), is_liveliness_protected (0x1 << 6), is_valid
(0x1 << 31).

**Repo:** `crates/rtps/src/endpoint_security_info.rs::attrs` (l. 27);
`crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info` (l. 2087)

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::encoded_bytes_le_match_spec_layout`,
`crates/security-runtime/src/plugin_attributes.rs::tests::liveliness_protected_sets_0x40_per_spec_9_4_1_3`

**Status:** done

### §9.4.2.9 Table 39 — validate_local_permissions

**Spec:** §9.4.2.9.1, p. 134 — "Validates the permissions of the local
DomainParticipant. The operation returns a PermissionsHandle object, if
successful. [...] This operation shall be called before the
DomainParticipant is enabled." Parameters `auth_plugin`, `identity`,
`domain_id`, `participant_qos`.

**Repo:** `crates/security-permissions/src/plugin.rs::validate_local_permissions` (signature, subject, validity, algorithms); `crates/security-runtime/src/shared.rs`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_signed_local_documents_validate_and_decide`,
`tools/secure-permissions/tests/access_plugin_signed.rs::local_documents_that_do_not_validate_are_refused`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_profiles_gate_goes_through_the_builtin_plugin`

**Status:** `done` — the operation checks the CMS signature of both documents against the permissions CA and the SubjectName against the identity certificate, and returns the `PermissionsHandle`; the gate calls it before the participant is enabled.

### §9.4.2.9 Table 39 — validate_remote_permissions

**Spec:** §9.4.2.9.2, p. 134–135 — "Validates the permissions of the
previously authenticated remote DomainParticipant, given the
PermissionsToken object received via DDS discovery and the
PermissionsCredentialToken obtained as part of the authentication process."

**Repo:** `crates/security-permissions/src/plugin.rs::validate_remote_permissions`; `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_document_needs_the_permissions_ca`,
`crates/security-permissions/src/plugin.rs::tests::a_remote_validation_without_ca_or_credential_is_refused`

**Status:** `done` — signature and SubjectName of `c.perm` are checked; a failure returns the NIL handle with a minor code.

### §9.4.2.9 Table 39 — check_create_participant

**Spec:** §9.4.2.9.3, p. 135 — "When the local DomainParticipant is created,
its permissions must allow it to join the DDS Domain specified by the
domain_id. [...] This operation shall also be called when the application
calls the operation set_qos() [...] If the check_create_participant does not
succeed (return true), the set_qos operation shall fail with the
NOT_ALLOWED_BY_SECURITY error."

**Repo:** `crates/security-permissions/src/plugin.rs::check_create_participant`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::check_create_participant_honours_the_domain_range`,
`crates/dcps/tests/security_access_plugin_e2e.rs::participant_set_qos_is_asked_of_the_plugin`

**Status:** `done` — the check runs at creation and at `set_qos`; a refusal is NOT_ALLOWED_BY_SECURITY.

### §9.4.2.9.3 check_create_participant: error code and set_qos

**Spec:** §9.4.2.9.3, p. 135 — "If the check_create_participant does not
succeed (return true), the set_qos operation shall fail with the
NOT_ALLOWED_BY_SECURITY error."; §9.8.1, step 3, p. 171: "the
create_participant operation shall return NIL and set the return code to
NOT_ALLOWED_BY_SECURITY."

**Repo:** `crates/dcps/src/participant.rs` (JoinDenied and `set_qos` on NOT_ALLOWED_BY_SECURITY)

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::participant_set_qos_is_asked_of_the_plugin`

**Status:** `done` — `set_qos` of the participant asks the plugin with the new QoS; a refusal is NOT_ALLOWED_BY_SECURITY.

### §9.4.2.9 Table 39 — check_create_datawriter

**Spec:** §9.4.2.9.4, p. 135–136 — "When the local DomainParticipant creates
a DataWriter for topic_name with the specified DataWriterQos associated with
the data_tag, its permissions must allow this. [...] This operation shall
also be called when the application calls the operation set_qos() on a
DataWriter". Parameters `permissions_handle`, `domain_id`, `topic_name`,
`qos`, `partition`, `data_tag`.

**Repo:** `crates/dcps/src/runtime.rs::check_create_endpoint`, `crates/dcps/src/publisher.rs` (`set_qos`)

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::the_granted_direction_and_the_open_topic_still_create`

**Status:** `done` — creating and `set_qos` of the DataWriter ask the plugin with topic, partitions, QoS and data tags.

### §9.4.2.9 check_create_datawriter/datareader: qos and data_tag

**Spec:** §9.4.2.9.4, §9.4.2.9.5, p. 136–137 — "Parameter qos: The QoS
policies of the local DataWriter. [...] Parameter data_tag: The data tags
that the local DataWriter is requesting to be associated with its data. This
argument can be nil if it is not considered for access control."

**Repo:** `crates/dcps/src/access_request.rs`; `DataWriterQos.data_tags`, `DataReaderQos.data_tags`

**Tests:** `crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`,
`crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`

**Status:** `done` — `qos` and `data_tag` are part of the request; a tag the plugin refuses prevents the creation.

### §9.4.2.9 Table 39 — check_create_datareader

**Spec:** §9.4.2.9.5, p. 136–137 — "When the local DomainParticipant creates
a DataReader for a Topic for topic_name with the specified DataReaderQos qos
associated with the data_tag, its permissions must allow this."

**Repo:** `crates/dcps/src/runtime.rs::check_create_endpoint`, `crates/dcps/src/subscriber.rs`

**Tests:** `crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`,
`crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`

**Status:** `done` — as `check_create_datawriter`, for the DataReader.

### §9.4.2.9 Table 39 — check_create_topic

**Spec:** §9.4.2.9.6, p. 137 — "When an entity of the local
DomainParticipant creates a Topic with topic_name and TopicQos qos its
permissions must allow this. [...] If the check_create_topic does not
succeed (return true), the set_qos operation shall fail with the
NOT_ALLOWED_BY_SECURITY error."

**Repo:** `crates/dcps/src/participant.rs`, `topic.rs`

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::create_topic_is_asked_of_the_plugin`,
`crates/security-permissions/src/plugin.rs::tests::check_create_topic_follows_governance_and_grant`

**Status:** `done` — creating and `set_qos` of the Topic ask the plugin.

### §9.4.2.9 Table 39 — check_local_datawriter_register_instance

**Spec:** §9.4.2.9.7, p. 137–138 — "In case the access control requires a
finer granularity at the instance level, this operation enforces the
permissions of the local DataWriter. The key identifies the instance being
registered".

**Repo:** `crates/dcps/src/publisher.rs` (`register_on_write`)

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::instance_operations_are_asked_of_the_plugin`

**Status:** `done` — `register_instance` asks the plugin with the serialized key; a refused instance is not registered.

### §9.4.2.9 Table 39 — check_local_datawriter_dispose_instance

**Spec:** §9.4.2.9.8, p. 138 — "The key has to match the permissions for
disposing an instance."

**Repo:** `crates/dcps/src/publisher.rs` (`dispose_key`)

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::instance_operations_are_asked_of_the_plugin`

**Status:** `done` — `dispose` asks the plugin with the key.

### §9.4.2.9 Table 39 — check_remote_participant

**Spec:** §9.4.2.9.9, p. 138–139 — "When the remote DomainParticipant is
discovered, the domain_id and, optionally, the DomainParticipantQoS are
checked to verify that joining that DDS Domain and using that QoS is allowed
by its permissions. [...] If the check_remote_participant does not succeed
(return true), the remote participant shall be considered invalid."

**Repo:** `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`,
`crates/security-permissions/src/plugin.rs::tests::a_remote_token_of_another_class_or_major_version_is_refused`

**Status:** `done` — after the handshake the runtime checks domain, token class and grant; with `is_access_protected` a participant that is refused is ignored.

### §9.4.2.9 Table 39 — check_remote_datawriter

**Spec:** §9.4.2.9.10, p. 139 — "This operation shall be called by a
DomainParticipant prior to matching a local DataReader belonging to that
DomainParticipant with a DataWriter belonging to a different (peer)
DomainParticipant. This operation shall also be called whenever a
DomainParticipant detects a QoS change for a DataWriter belonging to a
different (peer) DomainParticipant".

**Repo:** `crates/dcps/src/runtime.rs` (assignment of remote writers)

**Tests:** `crates/security-runtime/src/shared.rs::tests::a_peer_without_a_grant_is_denied_on_an_access_controlled_topic`,
`crates/security-runtime/src/shared.rs::tests::a_plugin_of_the_application_decides_every_check`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::the_granted_direction_and_the_open_topic_still_create`

**Status:** `done` — before matching, with partitions, data tags and QoS of the remote writer; without a known grant the result on a protected topic is deny, and the match is made again once the grant arrived.

### §9.4.2.9 Table 39 — check_remote_datareader

**Spec:** §9.4.2.9.11, p. 139–140 — "This operation shall be called by a
DomainParticipant prior to matching a local DataWriter [...] with a
DataReader belonging to a different (peer) DomainParticipant. [...] If the
operation returns true, the relay_only parameter shall be remembered by the
DDS middleware and passed to the register_matched_remote_datareader
operation on the CryptoKeyFactory."

**Repo:** `crates/dcps/src/runtime.rs` (`relay_only_readers`)

**Tests:** `crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`,
`crates/security-runtime/src/shared.rs::tests::relaying_needs_the_relay_grant_on_an_access_controlled_topic`,
`crates/security-permissions/src/plugin.rs::tests::relaying_needs_the_relay_action_where_the_topic_is_controlled`

**Status:** `done` — as `check_remote_datawriter`; `relay_only` is remembered and gives the reader the submessage key material only (§9.5.1.9.5).

### §9.4.2.9 Table 39 — check_remote_topic

**Spec:** §9.4.2.9.12, p. 140 — "When the remote DomainParticipant creates a
certain topic, the topic_name and optionally the TopicQoS extracted from the
topic_data are verified to ensure the remote DomainParticipant permissions
allow it to create the DDS Topic with the specified QoS."

**Repo:** `crates/dcps/src/runtime.rs`, `crates/dcps/src/access_request.rs::remote_topic_qos`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_nil_handle_is_refused_on_a_controlled_topic_only`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`

**Status:** `done` — before matching, the runtime checks the topic of the remote endpoint with the topic QoS from `topic_data`.

### §9.4.2.9 Table 39 — check_local_datawriter_match

**Spec:** §9.4.2.9.13, p. 140–141 — "Provides the means for the
AccessControl plugin to enforce access control rules that are based on the
DataTag associated with DataWriter and a matching DataReader."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`,
`crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`

**Status:** `done` — the runtime asks the plugin before the match with the remote reader (data tags).

### §9.4.2.9 Table 39 — check_local_datareader_match

**Spec:** §9.4.2.9.14, p. 141 — "Provides the means for the AccessControl
plugin to enforce access control rules that are based on the DataTag
associated with a DataReader and a matching DataWriter."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`,
`crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`

**Status:** `done` — the runtime asks the plugin before the match with the remote writer.

### §9.4.2.9 Table 39 — check_remote_datawriter_register_instance

**Spec:** §9.4.2.9.15, p. 141–142 — "In case the access control requires a
finer granularity at the instance level, this operation enforces the
permissions of the remote DataWriter."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`,
`crates/security-runtime/src/shared.rs::tests::the_builtin_plugin_is_not_asked_for_what_it_always_permits`

**Status:** `done` — the runtime asks the plugin for instances of remote writers (key from the sample).

### §9.4.2.9 Table 39 — check_remote_datawriter_dispose_instance

**Spec:** §9.4.2.9.16, p. 142 — "The key has to match the permissions for
disposing an instance."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`,
`crates/security-runtime/src/shared.rs::tests::the_builtin_plugin_is_not_asked_for_what_it_always_permits`

**Status:** `done` — as `register_instance`, for `dispose`.

### §9.4.2.9 Table 39 — get_permissions_token

**Spec:** §9.4.2.9.17, p. 142 — "Retrieves a PermissionsToken object. The
PermissionsToken is propagated via DDS discovery to summarize the
permissions of the DomainParticipant identified by the specified
PermissionsHandle."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`,
`crates/security-permissions/src/plugin.rs::tests::the_permissions_token_class_is_1_2_unless_named`

**Status:** `done` — the token `DDS:Access:Permissions:1.2` with `dds.perm_ca.sn` and `dds.perm_ca.algo`; the runtime writes it into the SPDP.

### §9.4.2.9 Table 39 — get_permissions_credential_token

**Spec:** §9.4.2.9.18, p. 143 — "Retrieves a PermissionsCredentialToken
object that can be used to represent on the network the permissions of the
DomainParticipant identified by the specified PermissionsHandle."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`

**Status:** `done` — the token `DDS:Access:PermissionsCredential` with `dds.perm.cert`.

### §9.4.2.9 Table 39 — set_listener

**Spec:** §9.4.2.9.19, p. 143 — "Sets the listener for the AccessControl
plugin."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::revoking_permissions_tells_the_listener`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — `AccessControlListener` and `set_listener` are in the interface; the plugin calls `on_revoke_permissions`.

### §9.4.2.9 Table 39 — return_permissions_token

**Spec:** §9.4.2.9.20, p. 143 — "Returns the PermissionsToken to the plugin
for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — the plugin takes back only tokens it issued.

### §9.4.2.9 Table 39 — return_permissions_credential_token

**Spec:** §9.4.2.9.21, p. 143 — "Returns the PermissionsCredentialToken to
the plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — as `return_permissions_token`.

### §9.4.2.9 Table 39 — get_participant_security_config

**Spec:** §9.4.2.9.22, p. 143–144 — "This operation shall be called by the
DDS middleware as part of the creation or enabling of the DDS
DomainParticipant. [...] The returned ParticipantSecurityConfig shall be used
to call the operation set_participant_security_config on the Authentication
plugin and the operation register_local_participant on the Cryptographic
plugin."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — the gate takes the config when the participant is created and hands it to authentication and cryptography (`algorithm_info`, `ac_participant_properties`).

### §9.4.2.9 Table 39 — get_topic_security_config

**Spec:** §9.4.2.9.23, p. 144 — "Retrieves the TopicSecurityConfig, which
describes how the DDS middleware should enforce the security and integrity
of the information related to the DDS Topic. This operation shall be called
by the DDS middleware as part of the creation or enabling of a DDS Topic."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security-permissions/src/psk_access.rs::tests::the_topic_and_endpoint_configs_are_all_false`

**Status:** `done` — the plugin returns the four values from the topic rule.

### §9.4.2.9 Table 39 — get_datawriter_security_config

**Spec:** §9.4.2.9.24, p. 144–145 — "Retrieves the EndpointSecurityConfig,
which describes how the DDS middleware should enforce the security and
integrity of the information related to the DDS DataWriter endpoint. [...]
The operation shall be called after calling check_create_datawriter."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security-permissions/src/psk_access.rs::tests::the_topic_and_endpoint_configs_are_all_false`

**Status:** `done` — after `check_create_datawriter`; the endpoint mask is derived from it.

### §9.4.2.9 Table 39 — get_datareader_security_config

**Spec:** §9.4.2.9.25, p. 145 — "Retrieves the EndpointSecurityConfig, which
describes how the DDS middleware should enforce the security and integrity
of the information related to the DDS DataReader endpoint."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security-permissions/src/psk_access.rs::tests::the_topic_and_endpoint_configs_are_all_false`

**Status:** `done` — as `get_datawriter_security_config`.

### §9.4.2.9 Table 39 — return_participant_security_config

**Spec:** §9.4.2.9.26, p. 145–146 — "Returns the ParticipantSecurityConfig
to the plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — the plugin takes back only configs it issued.

### §9.4.2.9 Table 39 — return_topic_security_config

**Spec:** §9.4.2.9.27, p. 146 — "Returns the TopicSecurityConfig to the
plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — the plugin takes back only configs it issued.

### §9.4.2.9 Table 39 — return_datawriter_security_config

**Spec:** §9.4.2.9.28, p. 146 — "Returns the EndpointSecurityConfig to the
plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — the plugin takes back only configs it issued.

### §9.4.2.9 Table 39 — return_datareader_security_config

**Spec:** §9.4.2.9.29, p. 146 — "Returns the EndpointSecurityConfig to the
plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — the plugin takes back only configs it issued.

### §9.4.2.9 Own operations outside Table 39

**Spec:** §9.4.2.9, Table 39, p. 129–134 — the table lists the operations of
the interface.

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::plugin_class_id_matches_spec`,
`crates/security-runtime/src/shared.rs::tests::the_builtin_plugin_is_not_asked_for_what_it_always_permits`

**Status:** `done` — the interface has the 29 operations of Table 39 and, in addition, `check_relay` (§7.1.1.4, default: refuse) and `plugin_class_id`. The earlier own operations `check_*_in_partitions` and `check_remote_*_match` are replaced by the parameters of the spec.

### §9.4.2.10 Table 40 — AccessControlListener::on_revoke_permissions

**Spec:** §9.4.2.10.1, p. 146 — "DomainParticipants' Permissions can be
revoked/changed. This listener provides a callback for permission
revocation/changes."

**Repo:** `crates/security/src/access_control.rs::AccessControlListener`; `crates/security-runtime/src/shared.rs` (removes the handle and re-matches)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::revoking_permissions_tells_the_listener`,
`crates/security-runtime/src/shared.rs::tests::a_revoked_remote_handle_is_handed_to_the_runtime_once`

**Status:** `done` — the plugin reports revoked permissions; the gate hands the handle to the runtime once.

---

### §9.5 Cryptographic Plugin

**Spec:** §9.5, p. 147 — "The Cryptographic plugin defines the types and
operations necessary to support encryption, digest, message authentication
codes, and key exchange for DDS DomainParticipants, DataWriters and DDS
DataReaders."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §9.5.1 Cryptographic Plugin Model

**Spec:** §9.5.1, Figure 11, p. 147 — interfaces CryptoKeyFactory,
CryptoKeyExchange, CryptoTransform; types ParticipantCryptoHandle,
DatawriterCryptoHandle, DatareaderCryptoHandle, CryptoToken,
CryptoTransformIdentifier.

**Repo:** `crates/security/src/crypto.rs::CryptographicPlugin` (l. 59, one
interface for all three)

**Tests:** —

**Status:** `n/a (informative)` — diagram; the operations are listed on
their own below.

### §9.5.1.1 CryptoToken

**Spec:** §9.5.1.1, p. 148 — "This class represents a generic holder for key
material. [...] Each plugin implementation shall fully define itself, so that
applications are able to interoperate."

**Repo:** `crates/security/src/token.rs` (alias `CryptoToken`, l. 247–268);
`crates/dcps/src/runtime.rs::build_crypto_token_message` (l. 21338)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cross_plugin_interop_via_tokens`,
`crates/security-crypto/src/plugin.rs::tests::tokens_carry_suite_tag_so_cross_suite_interop_works`

**Status:** done — the content of the builtin plugin is in §10.5.

### §9.5.1.2 ParticipantCryptoHandle

**Spec:** §9.5.1.2, p. 148 — "The ParticipantCryptoHandle object is an
opaque local reference that represents the key material used to encrypt and
sign whole RTPS Messages."

**Repo:** `crates/security/src/crypto.rs::CryptoHandle` (l. 29);
`crates/security-runtime/src/shared.rs::ensure_local` (l. 637)

**Tests:** `crates/security-runtime/src/shared.rs::tests::e2e_alice_bob_with_shared_gate`

**Status:** done — one type `CryptoHandle` for participant, writer and
reader handles.

### §9.5.1.3 DatawriterCryptoHandle

**Spec:** §9.5.1.3, p. 148 — "The DatawriterCryptoHandle object is an opaque
local reference that represents the key material used to encrypt and sign
RTPS submessages sent from a DataWriter. This includes the RTPS submessages
Data, DataFrag, Gap, Heartbeat, and HeartbeatFrag, as well as, the
SerializedPayload submessage element".

**Repo:** `crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`
(l. 21403); `crates/security-runtime/src/shared.rs::register_local_endpoint`
(l. 904)

**Tests:** `crates/security-runtime/src/shared.rs::tests::per_endpoint_datawriter_token_roundtrips_by_key_id`

**Status:** done

### §9.5.1.4 DatareaderCryptoHandle

**Spec:** §9.5.1.4, p. 148 — "The DatareaderCryptoHandle object is an opaque
local reference that represents the key material used to encrypt and sign
RTPS Submessages sent from a DataReader. This includes the RTPS Submessages
AckNack and NackFrag."

**Repo:** `crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`
(l. 21403, `is_writer = false`); selection of the reader submessages
`SMID_ACKNACK` (l. 21716)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** done

### §9.5.1.5 CryptoTransformIdentifier

**Spec:** §9.5.1.5, p. 148 — "The CryptoTransformIdentifier object used to
uniquely identify the transformation applied on the sending side [...] The
definition and interpretation is provided in 7.3.20."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`
(l. 99)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::transform_identifier_round_trip`

**Status:** done — details in §7.3.20.

### §9.5.1.6 Key Revision: same key_id, same algorithm, higher revision

**Spec:** §9.5.1.6, p. 148–149 — "The CryptoTransformIdentifier for the
'revised' key material shall have: The same value of the
transformation_key_id. The same value for the transformation_kind's member
transformation_algorithm_id. An updated (incremented) value for the
transformation_kind's member transformation_key_revision."

**Repo:** `crates/security-crypto/src/plugin/spec.rs`::revise_local_entity_keys

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_revised_with_the_entities`

**Status:** `done` — A revision keeps the `transformation_key_id` and the algorithm id and increments the revision in the CryptoHeader; the receiver keeps the revision it replaces and drops the one before. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.7 Table 41 — SecureSubmessageCategory_t

**Spec:** §9.5.1.7, Table 41, p. 149 — INFO_SUBMESSAGE,
DATAWRITER_SUMBESSAGE, DATAREADER_SUMBESSAGE.

**Repo:** `crates/security/src/cryptographic.rs`::SecureSubmessageCategory

**Tests:** `crates/security/src/cryptographic.rs::tests::the_three_categories_of_table_41_are_distinct`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — The three categories; `preprocess_secure_submsg` reports writer and reader submessages. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_local_participant

**Spec:** §9.5.1.8.1, p. 151–152 — "Registers a local DomainParticipant with
the Cryptographic Plugin. [...] Configures the cryptographic algorithms that
may be used by the Cryptographic plugin and retrieves an updated
ParticipantSecurityAlgorithmInfo". Parameters: out `adjusted_algorithm_info`,
`participant_identity`, `participant_permissions`, `participant_properties`,
`participant_security_config`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::register_local_participant_fills_the_adjusted_algorithm_info`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_gcm_or_gmac_by_the_protection_kind_and_width`, `crates/security-runtime/src/shared.rs::tests::e2e_alice_bob_with_shared_gate`

**Status:** `done` — Handle, `adjusted_algorithm_info`, identity, permissions, properties and `ParticipantSecurityConfig` are parameters; the gate calls the operation (`SharedSecurityGate::ensure_local`). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` and `-p zerodds-dcps --features security --lib` (7 tests), log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §9.5.1.8.1 register_local_participant: parameters

**Spec:** §9.5.1.8.1, p. 152 — "Parameter participant_properties: This
parameter shall contain all the properties in the PropertyQosPolicy of the
local DomainParticipant whose name has the prefix 'dds.sec.crypto.' [...]
Parameter participant_security_config: The ParticipantSecurityConfig returned
by the AccessControl get_participant_security_config operation."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::register_local_participant_fills_the_adjusted_algorithm_info`, `crates/security-crypto/src/plugin/spec.rs::tests::the_plugin_accepts_only_the_supported_algorithms`

**Status:** `done` — A nil identity or permissions handle is an error; unknown properties are ignored; only the algorithms of `supported_mask` are accepted. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_matched_remote_participant

**Spec:** §9.5.1.8.2, p. 152–153 — "Registers a remote DomainParticipant with
the Cryptographic Plugin. [...] 1. It shall create any necessary key material
needed to decrypt and verify the signatures of messages received from that
remote DomainParticipant [...] 2. It shall create any necessary key material
that will be used by the local DomainParticipant when encrypting or signing
messages that are intended only for that remote DomainParticipant."
Parameters `local_participant_crypto_handle`, `remote_participant_identity`,
`remote_participant_permissions`, `shared_secret`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_participant2participant_material_matches_the_participant_key`, `crates/security-runtime/src/shared.rs::tests::kx_channel_round_trips_through_gate`

**Status:** `done` — `remote_participant_permissions` is a parameter; with origin authentication a key for each remote participant. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_local_datawriter

**Spec:** §9.5.1.8.3, p. 153–154 — "Registers a local DataWriter with the
Cryptographic Plugin. [...] This operation shall create the cryptographic
material necessary to encrypt and/or sign the data written by the
DataWriter". Parameters: out `adjusted_algorithm_info`,
`participant_crypto`, `datawriter_properties`, `datawriter_security_config`,
`endpoint_guid`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/dcps/src/runtime.rs::tests::key_material_exists_for_protected_endpoints_only_and_goes_with_them`

**Status:** `done` — `adjusted_algorithm_info`, config, `endpoint_guid` and properties are parameters. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` and `-p zerodds-dcps --features security --lib` (7 tests), log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §9.5.1.8 Table 42 — register_local_datareader

**Spec:** §9.5.1.8.5, p. 154–155 — "Registers a local DataReader with the
Cryptographic Plugin. [...] This operation shall create the cryptographic
material necessary to encrypt and/or sign the messages sent by the
DataReader when the encryption/signature is independent of the targeted
DataWriter."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — As register_local_datawriter, for readers. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Missing parameters of CryptoKeyFactory

**Spec:** §9.5.1.8.2, §9.5.1.8.3, §9.5.1.8.5, p. 152–155 — parameters
`remote_participant_permissions`, `adjusted_algorithm_info`,
`datawriter_security_config`, `datareader_security_config`,
`endpoint_guid`; "Parameter local_datawriter_properties: This parameter shall
contain all the properties in the PropertyQosPolicy of the local DataWriter
whose name has the prefix 'dds.sec.crypto.'"

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/security-crypto/src/plugin/spec.rs::tests::the_plugin_accepts_only_the_supported_algorithms`

**Status:** `done` — All parameters of Table 42 are in `CryptoKeyFactory`; `endpoint_guid` identifies the volatile endpoints (EntityId 0xff0202c3/c4). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_matched_remote_datareader

**Spec:** §9.5.1.8.4, p. 154 — "Registers a remote DataReader with the
Cryptographic Plugin. [...] The operation shall associate the value of the
relay_only parameter with the returned DatawriterCryptoHandle. This
information shall be used in the generation of the KeyToken objects to be
sent to the DataReader."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_relay_only_reader_gets_the_submessage_key_alone`, `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — `relay_only` is associated with the handle and decides the writer tokens. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_matched_remote_datawriter

**Spec:** §9.5.1.8.6, p. 155–156 — "Registers a remote DataWriter with the
Cryptographic Plugin. [...] This operation shall create the cryptographic
material necessary to decrypt and/or verify the signatures of the RTPS
submessages (Data, DataFrag, Heartbeat, HeartbeatFrag, Gap) sent from the
remote DataWriter to the DataReader."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_volatile_endpoints_use_the_key_of_the_shared_secret`, `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — Material of the remote writer; for the volatile endpoints the Kx key of the SharedSecret. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — revise_local_entity_keys

**Spec:** §9.5.1.8.7, p. 156 — "Creates a revision of the KeyMaterial used by
the local DomainParticipant and its contained DataReader and DataWriter
entities. [...] Returning a key_revision value of '0' indicates the Plugin
does not support Key Revisions".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::the_volatile_endpoints_are_not_revised`

**Status:** `done` — Creates the revision for the participant and endpoints, not for the volatile endpoints; gate: `SharedSecurityGate::revise_local_entity_keys`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — activate_key_revision

**Spec:** §9.5.1.8.8, p. 156–157 — "Configures the plugin to start using the
KeyMaterial that corresponds to a Key Revision created by a previous call to
the operation revise_local_entity_keys."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_revised_with_the_entities`

**Status:** `done` — Afterwards the plugin encodes with the revision; gate: `SharedSecurityGate::activate_key_revision`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — unregister_participant

**Spec:** §9.5.1.8.9, p. 157 — "Releases the resources, associated with a
DomainParticipant that the Cryptographic plugin maintains. [...]
Specifically, it shall be called when the application deletes a local
DomainParticipant and also when the DDS Discovery mechanism detects that a
matched DomainParticipant is no longer in the system."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::unregistering_releases_the_key_material_and_the_matches`, `crates/security-runtime/src/shared.rs::tests::forget_remote_removes_mapping`

**Status:** `done` — Releases local and remote participant with endpoints; gate: `forget_remote`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — unregister_datawriter

**Spec:** §9.5.1.8.10, p. 157–158 — "Releases the resources, associated with
a DataWriter that the Cryptographic plugin maintains."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::unregistering_releases_the_key_material_and_the_matches`, `crates/dcps/src/runtime.rs::tests::key_material_exists_for_protected_endpoints_only_and_goes_with_them`

**Status:** `done` — Releases the material of the writer and its matches; gate: `unregister_local_endpoint`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` and `-p zerodds-dcps --features security --lib` (7 tests), log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §9.5.1.8 Table 42 — unregister_datareader

**Spec:** §9.5.1.8.11, p. 158 — "Releases the resources, associated with a
DataReader, that the Cryptographic plugin maintains."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::unregistering_releases_the_key_material_and_the_matches`

**Status:** `done` — As unregister_datawriter, for readers. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — create_local_participant_crypto_tokens

**Spec:** §9.5.1.9.1, p. 160–161 — "This operation creates a sequence of
CryptoToken tokens containing the information needed to correctly interpret
cipher text encoded using the local_participant_crypto. [...] The returned
ParticipantCryptoTokenSeq shall be sent to the remote DomainParticipant using
the BuiltinParticipantVolatileMessageSecureWriter with kind set to
GMCLASSID_SECURITY_PARTICIPANT_CRYPTO_TOKENS". Parameter `key_revision`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_participant2participant_material_matches_the_participant_key`, `crates/security-runtime/src/shared.rs::tests::data_round_trips_via_token_after_kx_register`

**Status:** `done` — `key_revision` is a parameter; gate: `local_token_for`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — set_remote_participant_crypto_tokens

**Spec:** §9.5.1.9.2, p. 161–162 — "This operation shall be called by the DDS
implementation upon reception of a message on the
BuiltinParticipantVolatileMessageSecureReader with kind set to
GMCLASSID_SECURITY_PARTICIPANT_CRYPTO_TOKENS".

**Repo:** `crates/security/src/crypto.rs::set_remote_participant_crypto_tokens`
(l. 107); `crates/security-runtime/src/shared.rs::set_remote_data_token_by_guid`
(l. 762); `crates/dcps/src/runtime.rs::on_volatile_messages` (l. 22563)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cross_plugin_interop_via_tokens`,
`crates/security-crypto/src/plugin.rs::tests::data_token_exchange_does_not_clobber_kx_key`

**Status:** done

### §9.5.1.9 Table 43 — create_local_datawriter_crypto_tokens

**Spec:** §9.5.1.9.3, p. 162–163 — "This operation creates a
DatawriterCryptoTokenSeq [...] The returned CryptoToken sequence shall be
sent by the DDS middleware to the remote DataReader using the
BuiltinParticipantVolatileMessageSecureWriter with kind set to
GMCLASSID_SECURITY_DATAWRITER_CRYPTO_TOKENS [...] The source_endpoint_guid
attribute shall be set to the GUID_t of the local DataWriter and the
destination_endpoint_guid attribute shall be set to the GUID_t of the remote
DataReader."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_relay_only_reader_gets_the_submessage_key_alone`, `crates/security-runtime/src/shared.rs::tests::per_endpoint_datawriter_token_roundtrips_by_key_id`

**Status:** `done` — Own operation with `remote_datareader_crypto`; for relay-only the submessage material alone. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — set_remote_datawriter_crypto_tokens

**Spec:** §9.5.1.9.4, p. 163 — "This operation shall be called by the DDS
implementation upon reception of a message on the
BuiltinParticipantVolatileMessageSecureReader with kind set to
GMCLASSID_SECURITY_DATAWRITER_CRYPTO_TOKENS".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-runtime/src/shared.rs::tests::per_endpoint_datawriter_token_roundtrips_by_key_id`

**Status:** `done` — Own operation with `local_datareader_crypto` and `remote_datawriter_crypto`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — create_local_datareader_crypto_tokens

**Spec:** §9.5.1.9.5, p. 164 — "The returned DatareaderCryptoTokenSeq shall
be sent by the DDS middleware to the remote DataWriter using the
BuiltinParticipantVolatileMessageSecureWriter with kind set to
GMCLASSID_SECURITY_DATAREADER_CRYPTO_TOKENS".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Own operation for readers. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — set_remote_datareader_crypto_tokens

**Spec:** §9.5.1.9.6, p. 164–165 — "This operation shall be called by the DDS
implementation upon reception of a message on the
BuiltinParticipantVolatileMessageSecureReader with kind set to
GMCLASSID_SECURITY_DATAREADER_CRYPTO_TOKENS".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Own operation for readers. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Dedicated endpoint operations in CryptoKeyExchange

**Spec:** §9.5.1.9, Table 43, p. 159–160 — create_local_datawriter_crypto_tokens
(`local_datawriter_crypto`, `remote_datareader_crypto`),
set_remote_datawriter_crypto_tokens (`local_datareader_crypto`,
`remote_datawriter_crypto`), create_local_datareader_crypto_tokens,
set_remote_datareader_crypto_tokens.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-crypto/src/plugin/spec.rs::tests::the_volatile_endpoints_use_the_key_of_the_shared_secret`

**Status:** `done` — The four operations are in `CryptoKeyExchange`; the gate calls them (`set_endpoint_token`, `endpoint_token`). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 key_revision of the operations create_local_*_crypto_tokens

**Spec:** §9.5.1.9.1, §9.5.1.9.3, §9.5.1.9.5, p. 161–164 — "Parameter
key_revision: An integer that selects the revision of the Key Material that
is encoded into the returned CryptoTokenSeq."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`

**Status:** `done` — A revision that `revise_local_entity_keys` did not return is an error. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — return_crypto_tokens

**Spec:** §9.5.1.9.7, p. 165 — "Returns the tokens in the CryptoToken
sequence to the plugin so the plugin can release any information associated
with it."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::crypto_tokens_are_returned_once`

**Status:** `done` — Takes issued tokens back once. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 CryptoTransform: the middleware substitutes the bytes

**Spec:** §9.5.1.10, p. 165–166 — "The DDS implementation is only responsible
for calling the operations in the CryptoTransform plugin at the appropriate
times as it generates and processes the RTPS messages, substitutes the input
bytes with the transformed bytes produced by the CryptoTransform operations".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description of the division of work; the
operations are listed on their own.

### §9.5.1.10 Table 44 — encode_serialized_payload

**Spec:** §9.5.1.10.1, p. 168–169 — "The operation receives the data written
by the DataWriter in serialized form wrapped inside the RTPS
SerializedPayload submessage element and shall output an RTPS CryptoContent
submessage element and a extra_inline_qos containing InlineQos formatted as a
ParameterList [...] The DDS implementation shall call this operation for all
outgoing RTPS Submessages with submessage kind Data and DataFrag."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_serialized_payload_goes_through_the_interface`, `crates/security-runtime/src/shared.rs::tests::e2e_alice_bob_with_shared_gate`

**Status:** `done` — Output `EncodedPayload` with `extra_inline_qos` (empty: the identifier is in the CryptoHeader). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` and `-p zerodds-dcps --features security --lib` (7 tests), log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §9.5.1.10 Table 44 — encode_datawriter_submessage

**Spec:** §9.5.1.10.2, p. 169–171 — "This operation shall be called by the
DDS implementation whenever it has constructed an RTPS submessage of kind
Data, DataFrag, Gap, Heartbeat, or HeartbeatFrag. [...] In the case of
BuiltinParticipantVolatileMessageSecureWriter [...] the
DatareaderCryptoHandle list has ONE element containing KxKey material
derived from the SharedSecret".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_receiver_specific_macs_follow_the_list_index`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — List of receivers with index; MACs per receiver. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — encode_datareader_submessage

**Spec:** §9.5.1.10.3, p. 171–172 — "This operation shall be called by the
DDS implementation whenever it has constructed an RTPS submessage of kind
AckNack or NackFrag."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — List of writers as a parameter. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — encode_rtps_message

**Spec:** §9.5.1.10.4, p. 172–175 — "This operation shall be called by the
DDS implementation whenever it has constructed an RTPS message prior to
sending it on the wire. [...] This operation may optionally not perform any
transformation of the input RTPS message. In this case, the operation shall
return false but not set the exception object." Parameters
`receiving_participant_crypto_list`, `receiving_participant_crypto_list_index`,
`transform_with_psk`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::an_rtps_message_goes_through_the_interface_without_aad`, `crates/security-crypto/src/plugin/spec.rs::tests::transform_with_psk_uses_the_pre_shared_key`

**Status:** `done` — Receiver list, index, `transform_with_psk`; FALSE without an exception when nothing is transformed. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — decode_rtps_message

**Spec:** §9.5.1.10.5, p. 175–176 — "This operation shall be called by the
DDS implementation whenever it receives an RTPS message prior to parsing it.
This operation shall reverse the transformation performed by the
encode_rtps_message operation".

**Repo:** `crates/security/src/crypto.rs::decode_rtps_message_cyclone`
(l. 372); `crates/security-runtime/src/shared.rs::transform_inbound_from`
(l. 1131), `classify_inbound` (l. 1348)

**Tests:** `crates/security-runtime/src/shared.rs::tests::classify_inbound_srtps_from_known_peer_accepts`,
`crates/security-runtime/src/shared.rs::tests::classify_inbound_srtps_with_wrong_key_is_crypto_error`

**Status:** done — `receiving_participant_crypto` and
`sending_participant_crypto` are no parameters; the key is found by
`transformation_key_id` and the GuidPrefix.

### §9.5.1.10 Table 44 — preprocess_secure_submsg

**Spec:** §9.5.1.10.6, p. 176–178 — "The purpose of the operation is to
determine whether the secure submessage was produced as a result of a call to
encode_datawriter_submessage or a call to encode_datareader_submessage, and
to retrieve the appropriate DatawriterCryptoHandle and
DatareaderCryptoHandle needed to decode the submessage."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-crypto/src/plugin/spec.rs::tests::preprocess_refuses_unknown_kinds_and_unknown_keys`

**Status:** `done` — Category and handles; the gate decodes with them. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — decode_datawriter_submessage

**Spec:** §9.5.1.10.7, p. 178–179 — "This operation shall reverse the
transformation performed by the encode_datawriter_submessage operation,
decrypting the content if appropriate and verifying any MACs or digital
signatures that were produced by the encode_datawriter_submessage
operation."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_is_taken_only_from_the_entities_it_is_associated_with`

**Status:** `done` — With the handles of `preprocess_secure_submsg`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — decode_datareader_submessage

**Spec:** §9.5.1.10.8, p. 179–180 — "This operation shall reverse the
transformation performed by the encode_datareader_submessage operation".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — As for writers. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — decode_serialized_payload

**Spec:** §9.5.1.10.9, p. 180–181 — "The operation shall receive in the
inline_qos parameter the InlineQos RTPS SubmessageElement that appeared in
the RTPS Data submessage that carried the SerializedPayload. [...] it shall
use the CryptoTransformIdentifier present in the CryptoContent to verify that
the correct key is available".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_serialized_payload_goes_through_the_interface`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_is_taken_only_from_the_entities_it_is_associated_with`

**Status:** `done` — With `inline_qos` and the handles; key of the sender only. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Parameters of the CryptoTransform operations

**Spec:** §9.5.1.10, Table 44, p. 166–168 — `extra_inline_qos`,
`receiving_datareader_crypto_list`, `receiving_datareader_crypto_list_index`,
`receiving_datawriter_crypto_list`, `receiving_participant_crypto_list`,
`receiving_participant_crypto_list_index`, `transform_with_psk`,
`inline_qos`, the sender and receiver handles of the decode_* operations.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_receiver_specific_macs_follow_the_list_index`, `crates/security-crypto/src/plugin/spec.rs::tests::the_serialized_payload_goes_through_the_interface`

**Status:** `done` — All parameters of Table 44 are in `CryptoTransform`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Own operations outside Tables 42 to 44

**Spec:** §9.5.1.8 to §9.5.1.10, Tables 42–44, p. 149–168 — the tables list
the operations of the three interfaces.

**Repo:** `crates/security/src/cryptographic.rs`; `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`; `crates/security/src/cryptographic.rs::VendorFormCrypto`

**Tests:** `crates/security-runtime/src/shared.rs::tests::data_round_trips_via_token_after_kx_register`, `crates/security-runtime/src/shared.rs::tests::two_peers_with_the_same_key_id_keep_their_keys`, `crates/dcps/src/runtime.rs::tests::a_user_datagram_under_the_kx_key_is_not_opened`

**Status:** `done` — Gate and runtime call only the operations of Tables 42 to 44 (`gate_crypto.rs`). What a vendor needs without a spec basis (participant key for submessages and payload, Kx key for payload) is in `VendorFormCrypto` and is used only with `org.zerodds.sec.kx_key_fallback=accept` or for endpoints without a key of their own. The older trait `CryptographicPlugin` remains as an implementation detail of `security-rtps` and `PskDerivedCryptoPlugin`; the middleware does not call it. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` and `-p zerodds-dcps --features security --lib` (7 tests), log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

---

### §9.6 The Logging Plugin

**Spec:** §9.6, p. 181 — "The Logging Control Plugin API defines the types
and operations necessary to support logging of security events for a DDS
DomainParticipant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §9.6.1 Background (Non-Normative)

**Spec:** §9.6.1, p. 181 — "The Logging plugin will add an ID to the log
message that uniquely specifies the DomainParticipant. It will also add a
time-stamp to each log message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — background; the normative rules are in
§9.6.2.

### §9.6.2 Logging Plugin Model

**Spec:** §9.6.2, Figure 21, p. 182 — interfaces Logging and LoggerListener,
types LogOptions, NameValuePair, BuiltinLoggingType.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — diagram; BuiltinLoggingType is in §10.6.

### §9.6.2.1 LogOptions before logging starts, unchangeable afterwards

**Spec:** §9.6.2.1, p. 182 — "The options must be set before logging starts
and may not be changed at run-time after logging has commenced."

**Repo:** `crates/security-logging/src/log_topic.rs::set_log_options`
(l. 624, `AlreadyEnabled` after `enable_logging`)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::options_cannot_change_after_logging_is_enabled`

**Status:** done — for the builtin plugin `LogTopicLoggingPlugin`.

### §9.6.2.1 Table 45 — LogOptions: log_level, log_file, distribute

**Spec:** §9.6.2.1, Table 45, p. 182 — `log_level Long`, `log_file String`,
`distribute Boolean`.

**Repo:** `crates/security-logging/src/log_topic.rs::LogOptions` (l. 101)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::distribute_off_publishes_nothing`

**Status:** done — `log_file` is `Option<PathBuf>`, `log_level` a
`LogLevel`.

### §9.6.2.1.1 log_level: levels FATAL to TRACE

**Spec:** §9.6.2.1.1, p. 183 — "Messages at or below the log_level are
logged. The levels are as follows, from low to high: FATAL_LEVEL [...]
SEVERE_LEVEL [...] ERROR_LEVEL [...] WARNING_LEVEL [...] NOTICE_LEVEL [...]
INFO_LEVEL [...] DEBUG_LEVEL [...] TRACE_LEVEL".

**Repo:** `crates/security/src/logging.rs::LogLevel`, `to_spec_level`, `from_spec_level`; `crates/security-logging/src/control.rs::LogControl::admit`

**Tests:** `crates/security/src/logging.rs::tests::the_spec_level_names_are_numbered_low_to_high`,
`crates/security-logging/src/control.rs::tests::the_spec_levels_filter_from_fatal_to_trace`

**Status:** `done` — eight levels `FATAL_LEVEL` to `TRACE_LEVEL` numbered 0 to 7; a message at or below the saved level is logged.
### §9.6.2.1.1 Names FATAL_LEVEL to TRACE_LEVEL

**Spec:** §9.6.2.1.1, p. 183 — FATAL_LEVEL, SEVERE_LEVEL, ERROR_LEVEL,
WARNING_LEVEL, NOTICE_LEVEL, INFO_LEVEL, DEBUG_LEVEL, TRACE_LEVEL.

**Repo:** `crates/security/src/logging.rs` (`FATAL_LEVEL` bis `TRACE_LEVEL`, `LogLevel::spec_name`, `LogLevel::Trace`)

**Tests:** `crates/security/src/logging.rs::tests::the_spec_level_names_are_numbered_low_to_high`

**Status:** `done` — `LogLevel` has `Trace`; `Alert` and `Critical` are both `SEVERE_LEVEL`. The conversion to `LoggingLevel` (§10.6) maps `Trace` to `Debug`, the IDL enumeration having no value for it.
### §9.6.2.1.2 log_file: append, NULL writes no file

**Spec:** §9.6.2.1.2, p. 183 — "Specifies the full path to a local file for
logging events. If the file already exists, the logger will append log
messages to the file. If it is NULL, then the logger will not log messages
to a file."

**Repo:** `crates/security-logging/src/control.rs::open_log_file`

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::an_unopenable_log_file_is_refused`

**Status:** done

### §9.6.2.1.3 distribute: publication as a DDS Topic

**Spec:** §9.6.2.1.3, p. 183 — "Specifies whether the log events should be
distributed over DDS. If it is TRUE, each log message at or above the
log_level is published as a DDS Topic."

**Repo:** `crates/security-logging/src/log_topic.rs::LogTopicLoggingPlugin`
(l. 554)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::distribute_off_publishes_nothing`,
`crates/dcps/tests/security_log_topic_e2e.rs::a_security_event_reaches_the_v2_log_topic`

**Status:** done — published are messages at or below the level, as with the
filter in §9.6.2.1.1; the text of §9.6.2.1.3 says "at or above" (contradiction in the specification).

### §9.6.2.2 Table 46 — set_log_options

**Spec:** §9.6.2.2.1, p. 184 — "Sets the options for the logger. This must
be called before enable_logging; it is an error to set the options after
logging has been enabled."

**Repo:** `crates/security/src/logging.rs::LoggingPlugin::set_log_options`; `crates/security-logging/src/control.rs::LogControl::set_log_options`

**Tests:** `crates/security-logging/src/control.rs::tests::a_sink_without_a_writer_refuses_distribute`,
`crates/security-logging/src/control.rs::tests::a_sink_enabled_at_construction_refuses_new_options`,
`crates/security-logging/src/log_topic.rs::tests::options_cannot_change_after_logging_is_enabled`

**Status:** `done` — on the interface and on all sinks; after `enable_logging` a `SecurityException` error, `distribute` without a DataWriter is refused.
### §9.6.2.2 Table 46 — log

**Spec:** §9.6.2.2.2, p. 184 — "Log a message. The logger shall log the
message if its log_level is at or above the level set in the LogOptions. The
Logger shall add to the message the RTPS GUID of the DomainParticipant whose
operations are being logged. The Logger shall populate the facility,
severity, and timestamp, fields." Parameters `log_level`, `message`,
`category`.

**Repo:** `crates/security/src/logging.rs::log` (l. 56);
`crates/security-logging/src/log_topic.rs::from_log_event` (l. 407)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::an_event_fills_every_field_from_the_clause`,
`crates/security/src/mock.rs::tests::mock_logging_captures_events`

**Status:** done — the caller passes the GUID as parameter `participant`.

### §9.6.2.2.2 Table 47 — structured_data "DDS"

**Spec:** §9.6.2.2.2, Table 47, p. 184 — "The Logger shall add an entry to
the structured_data field with the key 'DDS'. This NameValuePair sequence
shall include the following name-value pairs: guid [...] domain_id [...]
plugin_class [...] plugin_method".

**Repo:** `crates/security-logging/src/log_topic.rs::dds_structured_data`
(l. 377, `guid`, `plugin_class`, `plugin_method`);
`crates/dcps/src/security_log_topic.rs::add_domain_id` (l. 270, `domain_id`
when writing through `LogTopicDataWriterPublisher`)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::an_event_fills_every_field_from_the_clause`,
`crates/dcps/src/security_log_topic.rs::tests::the_publisher_inserts_the_domain_id_pair`,
`crates/dcps/tests/security_log_topic_e2e.rs::a_security_event_reaches_the_v2_log_topic`

**Status:** done — the four pairs are in every published sample.
`LoggingPlugin::log` receives no domain id; `domain_id` is set by
`LogTopicDataWriterPublisher` in `zerodds-dcps`. The line in `log_file`
carries no structured_data.

### §9.6.2.2 Table 46 — enable_logging

**Spec:** §9.6.2.2.3, p. 184 — "Enables logging. After this method is
called, any call to log shall log the messages according to the options.
After this method is called, the options may not be modified."

**Repo:** `crates/security/src/logging.rs::LoggingPlugin::enable_logging`; `crates/security-logging/src/control.rs::LogControl::enable_logging`

**Tests:** `crates/security-logging/src/control.rs::tests::enable_logging_without_options_is_an_error`,
`crates/security-logging/src/log_topic.rs::tests::enable_without_options_is_refused`

**Status:** `done` — on the interface and on all sinks; an error without options.
### §9.6.2.2 Table 46 — set_listener

**Spec:** §9.6.2.2.4, p. 185 — "Sets the LoggerListener that the Logger
plugin will use to notify the application of log events."

**Repo:** `crates/security/src/logging.rs::{LoggerListener, LoggingPlugin::set_listener}`; `crates/security-logging/src/control.rs::LogControl::admit`

**Tests:** `crates/security-logging/src/control.rs::tests::the_listener_hears_each_logged_message`,
`crates/security-logging/src/log_topic.rs::tests::the_listener_is_told_of_each_message_the_sink_logs`

**Status:** `done` — the listener is told of each logged message; `None` removes it.
### §9.6.2.2 Operations of Table 46 missing in LoggingPlugin

**Spec:** §9.6.2.2, Table 46, p. 183 — set_log_options, log, enable_logging,
set_listener.

**Repo:** `crates/security/src/logging.rs::LoggingPlugin`; `LogControl` in all sinks of `crates/security-logging`

**Tests:** `crates/security-logging/src/control.rs::tests::a_sink_without_a_writer_refuses_distribute`,
`crates/security-logging/src/control.rs::tests::the_listener_hears_each_logged_message`,
`crates/security-logging/src/log_topic.rs::tests::the_listener_is_told_of_each_message_the_sink_logs`

**Status:** `done` — all four operations are on the interface; the stderr, JSON-lines, syslog, fan-out, `LogTopicLoggingPlugin` and `MockLoggingPlugin` sinks implement them.

---

### §9.7 Data Tagging

**Spec:** §9.7, p. 185 — "Data tagging is the ability to add a security label
or tag to data. [...] It can be used for access control [...] It could be
used for message prioritization."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §9.7.1 Background (Non-Normative)

**Spec:** §9.7.1, p. 185 — "This specification supports DataWriter tagging."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reason for the choice.

### §9.7.2 DataTag of the DataWriter as immutable DataWriterQos

**Spec:** §9.7.2, p. 185 — "The DataWriter tag will be associated with every
sample written by the DataWriter. The DataWriter DataTag is implemented as an
immutable DataWriterQos."

**Repo:** `crates/dcps/src/qos.rs::DataWriterQos::data_tags`, `immutable_writer_change`

**Tests:** `crates/dcps/src/qos.rs::data_representation_tests::property_is_a_member_and_immutable_after_enable`

**Status:** `done` — field `data_tags`, not changeable after enabling (`DATA_TAGS`).
### §9.7.2 DataTag of the DataWriter in PublicationBuiltinTopicData

**Spec:** §9.7.2, p. 185 — "The DataWriter DataTag shall be propagated in the
PublicationBuiltinTopicData as part of the DDS discovery protocol."

**Repo:** `crates/dcps/src/builtin_topics.rs::PublicationBuiltinTopicData::data_tags`; `crates/dcps/src/runtime.rs::build_publication_data`

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::the_data_tags_of_an_announcement_reach_the_builtin_topic_samples`,
`crates/dcps/src/runtime.rs::tests::data_tags_are_announced`

**Status:** `done` — `PID_DATA_TAGS` goes into the announcement and appears in the builtin topic sample.
### §9.7.2 DataTag of the DataReader as immutable DataReaderQos

**Spec:** §9.7.2, p. 185 — "The DataReader DataTag is implemented as an
immutable DataReaderQos."

**Repo:** `crates/dcps/src/qos.rs::DataReaderQos::data_tags`, `immutable_reader_change`

**Tests:** `crates/dcps/src/qos.rs::data_representation_tests::property_is_a_member_and_immutable_after_enable`

**Status:** `done` — field `data_tags`, not changeable after enabling (`DATA_TAGS`).
### §9.7.2 DataTag of the DataReader in SubscriptionBuiltinTopicData

**Spec:** §9.7.2, p. 185 — "The DataReader DataTag shall be propagated in the
SubscriptionBuiltinTopicData as part of the DDS discovery protocol."

**Repo:** `crates/dcps/src/builtin_topics.rs::SubscriptionBuiltinTopicData::data_tags`; `crates/dcps/src/runtime.rs::build_subscription_data`

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::the_data_tags_of_an_announcement_reach_the_builtin_topic_samples`,
`crates/dcps/src/runtime.rs::tests::data_tags_are_announced`

**Status:** `done` — as for the DataWriter.

---

### §9.8 Security Plugins Behavior

**Spec:** §9.8, p. 186 — "This sub clause provides additional information on
how the plugins are integrated with the middleware."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction.

### §9.8.1 Step 1: validate_local_identity before creation

**Spec:** §9.8.1, p. 186–187 — "1. The DDS middleware shall validate the
identity of the application attempting to create the DomainParticipant by
calling the Authentication::validate_local_identity operation, passing the
domain_id, the DomainParticipantQos, and a candidate_participant_guid. [...]
If the identity is not successfully validated, the DDS middleware shall not
create the DomainParticipant and the create_participant operation shall
return NIL and set the return code to NOT_ALLOWED_BY_SECURITY."

**Repo:** `crates/dcps/src/participant.rs::security_from_participant_properties`

**Tests:** `crates/dcps/tests/security_participant_sequence.rs::step_1_an_identity_that_is_not_validated_refuses_the_participant`,
`crates/dcps/tests/security_participant_sequence.rs::the_fixtures_create_a_participant_when_nothing_is_wrong`

**Status:** `done` — an identity that is not validated gives `NOT_ALLOWED_BY_SECURITY` and no participant; the control without a fault creates one.
### §9.8.1 Step 2: validate_local_permissions before creation

**Spec:** §9.8.1, p. 187 — "2. The DDS middleware shall validate that the DDS
application has the necessary permissions to join DDS domains by calling the
AccessControl::validate_local_permissions operation. [...] If the
permissions are not validated, the DomainParticipant shall not be created,
the create_participant operation shall return NIL and set the return code to
NOT_ALLOWED_BY_SECURITY."

**Repo:** `crates/dcps/src/participant.rs::security_from_participant_properties`

**Tests:** `crates/dcps/tests/security_participant_sequence.rs::step_2_permissions_that_are_not_validated_refuse_the_participant`

**Status:** `done` — permissions whose signature the permissions CA does not verify give `NOT_ALLOWED_BY_SECURITY`.
### §9.8.1 Step 3: check_create_participant before creation

**Spec:** §9.8.1, p. 187 — "3. The DDS middleware shall verify that the DDS
application has the necessary permissions to join the specific Domain
identified by the domainId by calling the operation
AccessControl::check_create_participant. If this operation returns FALSE,
the DomainParticipant shall not be created".

**Repo:** `crates/security-runtime/src/qos_properties.rs::security_bundle_from_properties`

**Tests:** `crates/dcps/tests/security_participant_sequence.rs::step_3_a_domain_the_permissions_do_not_grant_refuses_the_participant`

**Status:** `done` — a grant for another domain under `enable_join_access_control` gives `NOT_ALLOWED_BY_SECURITY`; the message names `check_create_participant`.
### §9.8.1 Steps 1 to 3: return code NOT_ALLOWED_BY_SECURITY

**Spec:** §9.8.1, p. 187 — "the create_participant operation shall return
NIL and set the return code to NOT_ALLOWED_BY_SECURITY."

**Repo:** `crates/dcps/src/error.rs::DdsError::as_return_code`

**Tests:** `crates/dcps/tests/security_participant_sequence.rs::step_1_an_identity_that_is_not_validated_refuses_the_participant`,
`crates/dcps/tests/security_participant_sequence.rs::step_2_permissions_that_are_not_validated_refuse_the_participant`,
`crates/dcps/tests/security_participant_sequence.rs::step_3_a_domain_the_permissions_do_not_grant_refuses_the_participant`

**Status:** `done` — all three steps give `NOT_ALLOWED_BY_SECURITY`; `lookup_participant` finds no participant afterwards.
### §9.8.1 Step 4: IdentityToken in ParticipantBuiltinTopicData

**Spec:** §9.8.1, p. 187 — "4. The DDS middleware shall call the
get_identity_token operation to obtain the IdentityToken object corresponding
to the received IdentityHandle. The IdentityToken object shall be placed in
the ParticipantBuiltinTopicData sent via discovery".

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`
(l. 8638)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_beacon_announces_identity_token_and_discovery_triggers_handshake`

**Status:** done

### §9.8.1 Step 5: IdentityStatusToken in ParticipantBuiltinTopicDataSecure

**Spec:** §9.8.1, p. 187 — "5. The DDS middleware shall call the
get_identity_status_token operation [...] If the returned
IdentityStatusToken object is different than TokenNIL, it shall be placed in
the ParticipantBuiltinTopicDataSecure sent via secure discovery".

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`; `crates/discovery/src/spdp.rs::SpdpBeacon::serialize`, `serialize_secure`

**Tests:** `crates/discovery/src/spdp.rs::tests::the_identity_status_token_is_in_the_secure_participant_data_only`,
`crates/dcps/src/runtime.rs::tests::the_tokens_of_the_plugins_are_announced_and_token_nil_is_not`

**Status:** `done` — the token goes out on the secure SPDP writer only, not on the plain one; TokenNIL is not announced.
### §9.8.1 Step 6: PermissionsToken in ParticipantBuiltinTopicData

**Spec:** §9.8.1, p. 187 — "6. The middleware shall call the
get_permissions_token operation on the AccessControl plugin to obtain the
PermissionsToken object corresponding to the received PermissionsHandle. The
PermissionsToken shall be placed in the ParticipantBuiltinTopicData sent via
discovery".

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (the token comes from `SharedSecurityGate::local_permissions_token`, which calls `get_permissions_token` of the AccessControl plugin and returns the token with `return_permissions_token`)

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`

**Status:** `done` — the plugin gives the PermissionsToken; it is in the participant data, and `return_permissions_token` follows.

### §9.8.1 Step 7: get_permissions_credential_token

**Spec:** §9.8.1, p. 187 — "7. The middleware calls the
get_permissions_credential_token operation on the AccessControl plugin,
which returns the PermissionsCredentialToken object corresponding to the
received PermissionsHandle."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`; `crates/security-runtime/src/shared.rs::SharedSecurityGate::local_permissions_credential_token`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`

**Status:** `done` — `get_permissions_credential_token` is called, and the token goes back with `return_permissions_credential_token`.

### §9.8.1 Step 8: permissions in the handshake

**Spec:** §9.8.1, p. 187 — "8. The middleware calls the
set_permissions_credential_and_token operation on the Authentication plugin
such that it can be sent during the authentication handshake."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (`set_permissions_credential_and_token` on the Authentication plugin)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`

**Status:** `done` — `set_permissions_credential_and_token` is called with the credential token and the PermissionsToken, before `set_participant_security_config`.

### §9.8.1 Steps 9 and 10: get_/set_participant_security_config

**Spec:** §9.8.1, p. 188 — "9. The middleware calls the
get_participant_security_config operation on the AccessControl plugin [...]
10. The middleware calls the set_participant_security_config operation on
the Authentication plugin passing the ParticipantSecurityConfig".

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::participant_security_config`; `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (`set_participant_security_config`)

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`

**Status:** `done` — `get_participant_security_config` on the AccessControl plugin, then `set_participant_security_config` on the Authentication plugin with the algorithms of the config.

### §9.8.1 Step 11: register_local_participant with ParticipantSecurityConfig

**Spec:** §9.8.1, p. 188 — "11. The middleware calls the
register_local_participant operation on the Cryptographic plugin passing the
ParticipantSecurityConfig returned by the call to
get_participant_security_config".

**Repo:** `crates/security-runtime/src/gate_crypto.rs::CryptoLayer::ensure_local` (`LocalParams::config`, `algorithm_info`); `crates/security-runtime/src/shared.rs::SharedSecurityGate::ensure_local`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_cryptographic_plugin_registers_the_participant_and_the_volatile_endpoints`

**Status:** `done` — `register_local_participant` gets the ParticipantSecurityConfig that `get_participant_security_config` returned (the test checks `is_rtps_axk_protected` on the recorded operation).

### §9.8.1 Step 12: participant data from tokens and configuration

**Spec:** §9.8.1, p. 188 — "IdentityToken is used to set the identity_token.
PermissionsToken is used to set the permissions_token. The
ParticipantSecurityConfig is used to set the security_info. The two
ParticipantSecurityAlgorithmInfo are combined and used to set the
digital_signature, key_establishment, and symmetric_cipher. [...] The
IdentityStatusToken, is used to set the identity_status_token."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (`participant_security_info`, `local_algorithm_infos`); `crates/security-runtime/src/plugin_attributes.rs::participant_security_info`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`,
`crates/dcps/src/runtime.rs::tests::the_tokens_of_the_plugins_are_announced_and_token_nil_is_not`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** `done` — `identity_token`, `permissions_token`, `participant_security_info`, the three algorithm infos (the answer of `set_participant_security_config`, cut by the `algorithm_info` of the AccessControl plugin) and `identity_status_token` (in the data of the secure topic only) are in the participant data.

### §9.8.2 Check of plugin compatibility

**Spec:** §9.8.2, p. 188–189 — "whenever a (local) DomainParticipant
discovers a (remote) DomainParticipant the first step is to check the
information present in the ParticipantBuiltinTopicData of the remote
DomainParticipant to determine security plugin compatibility. The check
examines the following members: identity_token, permissions_token,
security_protection_info, digital_signature, key_establishment,
symmetric_cipher."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — `identity_token` and `permissions_token` (PluginClassName and MajorVersion), protection masks and algorithm infos are compared; a difference means no authentication.
### §9.8.3 Authentication behavior with discovered DomainParticipant

**Spec:** §9.8.3, p. 189 — "Depending on the ParticipantSecurityConfig
returned by the AccessControl operation get_participant_security_config the
DomainParticipant may allow remote DomainParticipants that lack the ability
to authenticate [...] to match."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition to §9.8.3.1 and §9.8.3.2.

### §9.8.3.1 TRUE: participants without DDS Security are matched

**Spec:** §9.8.3.1, p. 189 — "Discovered DomainParticipant entities that do
not implement the DDS Security specification or do not contain compatible
Security Plugins shall be matched without the DomainParticipant attempting
to authenticate them and shall be treated as 'Unauthenticated'
DomainParticipant entities."

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`, `BuiltinMatch::Regular`

**Tests:** `crates/dcps/src/runtime.rs::tests::unauthenticated_participants_get_the_regular_builtin_endpoints_only`,
`crates/dcps/src/runtime.rs::tests::an_unauthenticated_peer_gets_the_regular_sedp_only`

**Status:** `done` — a participant without DDS Security gets the regular builtin endpoints, without a handshake.
### §9.8.3.1 TRUE: a failed authentication counts as unauthenticated

**Spec:** §9.8.3.1, p. 189 — "Discovered DomainParticipant entities that do
implement the DDS Security specification and declare compatible Security
Plugins but fail the Authentication protocol shall be matched and treated as
'Unauthenticated' DomainParticipants entities."

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`

**Tests:** `crates/dcps/src/runtime.rs::tests::unauthenticated_participants_get_the_regular_builtin_endpoints_only`

**Status:** `done` — a participant of another CA fails the handshake and stays matched with the regular endpoints.
### §9.8.3.1 Unauthenticated participants only with the regular builtin endpoints

**Spec:** §9.8.3.1, p. 189 — "For any matched 'Unauthenticated'
DomainParticipant entities, the DomainParticipant shall match only the
regular builtin Endpoints (ParticipantMessage, DCPSParticipants,
DCPSPublications, DCPSSubscriptions) and not the builtin secure Endpoints".

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`, `handle_spdp_datagram`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_unauthenticated_peer_gets_the_regular_sedp_only`,
`crates/dcps/src/runtime.rs::tests::unauthenticated_participants_get_the_regular_builtin_endpoints_only`

**Status:** `done` — the secure endpoints are matched only after the authentication.
### §9.8.3.1 Authenticated participants with all builtin endpoints

**Spec:** §9.8.3.1, p. 189 — "For any matched authenticated
DomainParticipant entities, the DomainParticipant shall match all the
builtin endpoints."

**Repo:** `crates/discovery/src/security/stack.rs::handle_remote_endpoints`
(l. 236); `crates/discovery/src/sedp/stack.rs::on_participant_discovered`
(l. 433)

**Tests:** `crates/dcps/src/runtime.rs::tests::c34c_security_builtin_wiring_end_to_end`,
`crates/discovery/src/security/stack.rs::tests::handle_remote_endpoints_with_all_bits_wires_all_four`

**Status:** done

### §9.8.3.2 FALSE: reject participants without compatible plugins

**Spec:** §9.8.3.2, p. 189 — "Discovered DomainParticipant entities that do
not implement the DDS Security specification or do not contain compatible
Security Plugins shall be rejected without the DomainParticipant attempting
to authenticate them."

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`, `BuiltinMatch::Stateless`

**Tests:** `crates/dcps/src/runtime.rs::tests::unauthenticated_participants_are_rejected_where_they_are_not_allowed`

**Status:** `done` — with `allow_unauthenticated_participants` FALSE nothing but the stateless message endpoints is matched.
### §9.8.3.2 FALSE: reject a failed authentication

**Spec:** §9.8.3.2, p. 189 — "Discovered DomainParticipant entities that do
implement the DDS Security specification, declare compatible Security
Plugins but fail the Authentication protocol shall be rejected."

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`

**Tests:** `crates/dcps/src/runtime.rs::tests::unauthenticated_participants_are_rejected_where_they_are_not_allowed`

**Status:** `done` — a participant of another CA is not matched.
### §9.8.3.2 FALSE: stateless message endpoints before the authentication

**Spec:** §9.8.3.2, p. 189 — "Discovered DomainParticipant entities that do
implement the DDS Security specification and declare compatible Security
Plugins automatically 'match' the ParticipantStatelessMessage builtin
endpoints to allow the authentication handshake to proceed."

**Repo:** `crates/discovery/src/security/stack.rs::handle_remote_endpoints`
(l. 236)

**Tests:** `crates/discovery/src/security/stack.rs::tests::handle_remote_endpoints_with_only_stateless_bits_skips_volatile`,
`crates/discovery/src/security/stack.rs::tests::end_to_end_stateless_message_loopback_between_stacks`

**Status:** done

### §9.8.3.2 FALSE: the other builtin endpoints only after the authentication

**Spec:** §9.8.3.2, p. 189–190 — "Discovered DomainParticipant entities that
do implement the DDS Security specification, declare compatible Security
Plugins, and pass the Authentication protocol successfully shall be matched
and the DomainParticipant shall also match all the builtin endpoints of the
discovered DomainParticipant, except for the ParticipantStatelessMessage
builtin endpoints, which were already matched prior to the Authentication
protocol."

**Repo:** `crates/dcps/src/runtime.rs::match_authenticated_builtins`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_sedp_of_a_peer_is_matched_once_it_authenticated`

**Status:** `done` — SEDP is not matched before the handshake and fully matched after it.
### §9.8.3 Figure 23 — authentication sequence with a discovered participant

**Spec:** §9.8.3, Figure 23, p. 190–192 — steps 1 to 18:
validate_remote_identity on both sides, begin_handshake_request,
begin_handshake_reply, process_handshake on both sides, get_shared_secret
and get_authenticated_peer_credential_token on both sides.

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`, `on_stateless_message`, `collect_completion` (`shared_secret`, `get_authenticated_peer_credential_token`, `return_authenticated_peer_credential_token`)

**Tests:** `crates/discovery/src/security/stack.rs::tests::figure_23_calls_the_operations_of_both_plugins_in_order`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`,
`crates/dcps/src/runtime.rs::tests::handshake_completes_through_runtime_dispatch_e2e`

**Status:** `done` — both sides call `validate_remote_identity`, the initiator `begin_handshake_request`, the replier `begin_handshake_reply`, both `process_handshake`, then `get_shared_secret` and `get_authenticated_peer_credential_token` (order checked on the recording plugin).

### §9.8.4 AccessControl does not affect the RTPS builtin endpoints and the stateless ones

**Spec:** §9.8.4, p. 193 — "The AccessControl plugin shall impact only the
Builtin Secure Entities and the application-defined Entities. It shall not
impact the builtin entities defined by the DDS-RTPS Protocol specification
nor the BuiltinParticipantStatelessMessageWriter or the
BuiltinParticipantStatelessMessageReader."

**Repo:** `crates/dcps/src/runtime.rs::check_create_endpoint` (l. 9685,
called only from the paths for application writers and readers, l. 9868 and
10318)

**Tests:** `crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`

**Status:** done

### §9.8.4 Table 48 — groups of operations

**Spec:** §9.8.4, Table 48, p. 193–194 — Group1 to Group5 and their effect on
DomainParticipant, RTPS builtin entities, Builtin Secure Entities, stateless
endpoints and application entities.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the rules are listed on their
own in the items of §9.8.4 to §9.8.8.

### §9.8.4 Create secure builtin endpoints unconditionally

**Spec:** §9.8.4, p. 194 — "This means that the Secure Builtin Entities
shall be created unconditionally when the DomainParticipant is created."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`
(l. 8638)

**Tests:** `crates/dcps/src/runtime.rs::tests::c34c_security_builtin_wiring_end_to_end`,
`crates/dcps/src/runtime.rs::tests::enable_security_builtins_with_auth_activates_handshake_driver`

**Status:** done — `security.secure_spdp: false` switches off the endpoints
of DCPSParticipantsSecure; the default is on (see §7.5.1.6).

### §9.8.4 get_*_security_config for the secure builtin endpoints

**Spec:** §9.8.4, p. 194 — "During the creation process of DataWriter
entities the get_datawriter_security_config shall be called and likewise
during the creation process of DataReader entities the
get_datareader_security_config shall be called. The purpose of calling these
get_xxx_security_config operations is to obtain the information necessary to
call the Cryptographic plugin operations on these endpoints."

**Repo:** `crates/security-runtime/src/builtin_attributes.rs::builtin_endpoint_kinds_asked`, `kinds_of_endpoint_config`, `builtin_topic_name`; `crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`, `register_endpoint_crypto`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_endpoint_config_of_the_plugin_gives_the_protection_of_an_endpoint`,
`crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/security-runtime/src/builtin_attributes.rs::tests::the_plugin_is_not_asked_for_the_endpoints_table_21_fixes`

**Status:** `done` — with an AccessControl plugin of the application the runtime calls `get_datawriter_security_config` / `get_datareader_security_config` for the endpoints of DCPSParticipantsSecure, DCPSPublicationsSecure, DCPSSubscriptionsSecure and DCPSParticipantMessageSecure and follows the answer; application endpoints likewise. For the endpoints of DCPSParticipantVolatileMessageSecure and the secure TypeLookup endpoints (request and reply) §7.5.13 Table 21 fixes the protection (`is_submessage_protected true`, all other attributes `false`, independent of the plugin and of the governance). The plugin does not decide there; the runtime does not ask it and protects as Table 21 says. The test installs a plugin that protects nothing and checks that it is not called for these six endpoints and that the protection kinds of Table 21 apply; for DCPSPublicationsSecure it is called and its answer is followed. The plugin test in C, C++ and Java checks that the Cryptographic plugin gets the volatile endpoints under `dds.sec.builtin_endpoint_name`. Green on the Linux test host (`/root/zerodds-rc7/agent-logs/sec-behavior-1018-105058.log`).

### §9.8.4 Stateless message endpoints for every participant

**Spec:** §9.8.4, p. 194 — "DDS Secure implementations shall create these
endpoints unconditionally for all created DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`
(l. 8638)

**Tests:** `crates/dcps/src/runtime.rs::tests::enable_security_builtins_with_auth_activates_handshake_driver`

**Status:** done — for every participant with security; a participant
without security does not have the endpoints.

### §9.8.4 Partition of Publisher and Subscriber

**Spec:** §9.8.4, p. 195 — "They are impacted indirectly by operations in
Group2 by the fact that the PartitionQos settings of the Publisher (or
Subscriber) may cause the AccessControl plugin to prevent the creation of
DataWriter (or DataReader) entities belonging to them. Likewise they are
impacted indirectly by operations in Group4 in that the PartitionQos settings
of the remote Publisher (or Subscriber) may cause the AccessControl plugin to
prevent matching".

**Repo:** `crates/dcps/src/runtime.rs::wire_writer_to_remote_reader`, `wire_reader_to_remote_writer`; `crates/security-runtime/src/shared.rs::SharedSecurityGate::check_remote_datawriter`, `check_remote_datareader`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::partition_scope_narrows_an_otherwise_permitted_topic`,
`crates/security-permissions/src/governance.rs::tests::partitions_scope_the_grant`,
`crates/dcps/src/runtime.rs::tests::the_plugin_is_asked_about_the_endpoints_of_authenticated_participants_only`

**Status:** `done` — the partitions of the remote endpoint enter the check; a grant with partitions permits only those; the plugin of the application sees them in the request.
### §9.8.5 AccessControl behavior with local participant creation

**Spec:** §9.8.5, p. 195 — "The functionality of the AccesControl plugin with
regards to the creation of local DDS DomainParticipant entities was
illustrated in Figure 22 and described in 9.8.1."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference to §9.8.1.

### §9.8.6 Steps 1 to 3: topic with get_topic_security_config and check_create_topic

**Spec:** §9.8.6, Figure 24, p. 196 — "2. The middleware calls
AccessControl::get_topic_security_config to obtain the TopicSecurityConfig
for the Topic being created. 3. [...] Operation
AccessControl::check_create_topic() is called for this verification. If the
verification fails, the Topic object is not created."

**Repo:** `crates/dcps/src/runtime.rs::check_create_topic_access` (`get_topic_security_config`, `check_create_topic`); `crates/security-runtime/src/shared.rs::SharedSecurityGate::topic_security_config`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`

**Status:** `done` — `get_topic_security_config`, `return_topic_security_config` and `check_create_topic` in this order; a topic the plugin refuses is not created.

### §9.8.6 Steps 4, 5, 8 and 9: writer and reader only with permission

**Spec:** §9.8.6, p. 196–197 — "5. The middleware verifies that the
DataWriter has the right permissions to publish on Topic topicName.
Operation AccessControl::check_create_datawriter() is called for this
verification. [...] If the verification doesn't succeed, the DataWriter is
not created. [...] 9. [...] If the verification doesn't succeed, the
DataReader is not created."

**Repo:** `crates/dcps/src/runtime.rs::check_create_endpoint` (l. 9685)

**Tests:** `crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::an_unarmed_gate_refuses_instead_of_creating`

**Status:** done — the optional checks of data tags and QoS are not
implemented (see §9.4.2.9).

### §9.8.6 Steps 6 and 10: get_datawriter_/get_datareader_security_config

**Spec:** §9.8.6, p. 196–197 — "6. The middleware calls
AccessControl::get_datawriter_security_config to obtain the
EndpointSecurityConfig for the created DataWriter. [...] 10. The middleware
calls the operation AccessControl::get_datareader_security_config".

**Repo:** `crates/dcps/src/runtime.rs::register_endpoint_crypto` (`SharedSecurityGate::endpoint_security_config`)

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/src/runtime.rs::tests::the_endpoint_config_of_the_plugin_gives_the_protection_of_an_endpoint`

**Status:** `done` — `get_datawriter_security_config` and `get_datareader_security_config` after `check_create_*`, with `return_*_security_config`; the answer decides whether the endpoint has key material and with which protection.

### §9.8.6 Steps 7 and 11: discovery depending on is_discovery_protected

**Spec:** §9.8.6, p. 197 — "7. [...] has the is_discovery_protected attribute
set to FALSE. In this situation the middleware configures Discovery to use
regular (not secure) publications discovery endpoint (DCPSPublications) [...]
11. [...] set to TRUE. In this situation the middleware configures Discovery
to use the secure subscriptions discovery endpoint".

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_topic_config_of_the_plugin_chooses_the_secure_sedp`,
`crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — `is_discovery_protected` of the plugin's TopicSecurityConfig chooses between the regular and the secure discovery endpoints, also against the governance; a plugin that fails protects.

### §9.8.6 Steps 12 to 15: registering and disposing instances

**Spec:** §9.8.6, p. 197 — "13. [...] The operation
AccessControl::check_local_datawriter_register_instance() is called for
this verification. If the verification doesn't succeed, the instance is not
registered. [...] 15. [...] If the verification doesn't succeed, the
instance is not disposed."

**Repo:** `crates/dcps/src/publisher.rs` (`register_instance`, `dispose`); `crates/security-runtime/src/shared.rs::SharedSecurityGate::check_local_datawriter_instance`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/tests/security_access_plugin_e2e.rs::instance_operations_are_asked_of_the_plugin`

**Status:** `done` — `check_local_datawriter_register_instance` and `check_local_datawriter_dispose_instance` before the operation; when refused, the instance is left as it was.

### §9.8.7 No permission check for unauthenticated participants

**Spec:** §9.8.7, p. 197 — "Local DomainParticipant will not perform any
further participant AccessControl validation with unauthenticated
participants (i.e., validate_remote_permissions and
check_remote_participant will not be called)."

**Repo:** `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_unauthenticated_participant_is_not_checked_for_permissions`

**Status:** `done` — neither `validate_remote_permissions` nor `check_remote_participant` is called for an unauthenticated participant; no `security.remote_permissions.refused` event.
### §9.8.7 is_access_protected=TRUE: check and exclusion of the peer

**Spec:** §9.8.7, p. 197–198 — "If the DomainParticipant discovers a
DomainParticipant entity that it can authenticate successfully, and
is_access_protected is TRUE, then it shall validate with the AccessControl
plugin that it has the permissions necessary to join the DDS domain. [...]
If the validation fails, the discovered DomainParticipant shall be
considered ignored and all the builtin Topics should not be matched."

**Repo:** `crates/dcps/src/runtime.rs::store_remote_permissions`, `ignore_unauthorized_participant`

**Tests:** `crates/dcps/tests/security_remote_participant_access_e2e.rs::an_authenticated_peer_without_a_domain_grant_is_ignored_only_where_the_domain_is_protected`

**Status:** `done` — an authenticated peer without a grant for the domain is ignored and not matched.
### §9.8.7 is_access_protected=FALSE: HandleNIL without consequences

**Spec:** §9.8.7, p. 198 — "If the DomainParticipant discovers a
DomainParticipant entity that it can authenticate successfully, and
is_access_protected is FALSE, then validation will succeed with no access
control checking. In this case, only get_authenticated_peer_credential_token
and validate_remote_permissions are called, and a HandleNIL return will not
impact the validation result."

**Repo:** `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `crates/dcps/tests/security_remote_participant_access_e2e.rs::an_authenticated_peer_without_a_domain_grant_is_ignored_only_where_the_domain_is_protected`

**Status:** `done` — without `enable_join_access_control` the same peer is kept.
### §9.8.7 Figure 25 steps 3 and 5: check_remote_participant and check_remote_topic

**Spec:** §9.8.7, Figure 25, p. 198–199 — "3. Participant1 calls the operation
check_remote_participant to verify the remote DomainParticipant
(Participant2) is allowed to join the DDS domain [...] 5. [...] The operation
check_remote_topic is called for this verification. If the verification
fails, the discovered Topic is ignored."

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::check_remote_participant`, `check_remote_topic`; `crates/dcps/src/runtime.rs` (`wire_*_to_remote_*_unguarded`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_plugin_is_asked_about_the_endpoints_of_authenticated_participants_only`,
`crates/dcps/tests/security_remote_participant_access_e2e.rs::an_authenticated_peer_without_a_domain_grant_is_ignored_only_where_the_domain_is_protected`

**Status:** `done` — `check_remote_participant` ignores the peer; `check_remote_topic` refuses endpoints of a topic that the plugin or the grant of the peer does not permit (the event `match.refused` names §9.4.2.9.12).
### §9.8.8 Endpoints of unauthenticated participants over regular discovery

**Spec:** §9.8.8, p. 199 — "If the ParticipantSecurityConfig object returned
by the AccessControl operation get_participant_security_config has the
is_access_protected attribute set to FALSE, the DomainParticipant may have
matched a remote 'Unauthenticated' DomainParticipant".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — transition; the sentence names
`is_access_protected` where, by §9.8.3, `allow_unauthenticated_participants`
is meant (contradiction in the specification).

### §9.8.8.1 Unauthenticated: do not match local endpoints of protected topics

**Spec:** §9.8.8.1, p. 199 — "Reject (do not try to match) local endpoints for
which the related TopicSecurityConfig have the attribute is_read_protected or
is_write_protected set to TRUE."

**Repo:** `crates/dcps/src/runtime.rs::refuses_unauthenticated_peer`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_that_is_not_authenticated_gets_no_endpoint_on_a_protected_topic`

**Status:** `done` — endpoints of an unauthenticated peer are not matched on read- or write-protected topics; the `match.refused` event names §9.8.8.1.
### §9.8.8.1 and §9.8.8.2 Match unprotected topics without AccessControl

**Spec:** §9.8.8.1, §9.8.8.2, p. 199–200 — "Proceed to try matching without
checking with the AccessControl plugin the local DataWriter endpoints for
which the related TopicSecurityConfig [...] have the attribute
is_read_protected set to FALSE. Proceed to try matching without checking
with the AccessControl plugin the local DataReader endpoints for which [...]
is_write_protected set to FALSE." (Figure 26)

**Repo:** `crates/dcps/src/runtime.rs::refuses_unauthenticated_peer`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_that_is_not_authenticated_gets_no_endpoint_on_a_protected_topic`

**Status:** `done` — the open topic (no read and no write protection) is matched with the peer over the plain SEDP, without a call to AccessControl.
### §9.8.8.2 Authenticated: check protected topics before matching

**Spec:** §9.8.8.2, p. 199–200 — "Perform the AccessControl checks for
discovered endpoints that would match local DataWriters for whom the
is_read_protected attribute is set to TRUE, and only proceed to try matching
the discovered endpoints for whom the access control checks succeed. Perform
the AccessControl checks for discovered endpoints that would match local
DataReader for whom the is_write_protected attribute is set to TRUE".

**Repo:** `crates/dcps/src/runtime.rs::wire_writer_to_remote_reader`, `wire_reader_to_remote_writer` (`check_remote_topic`, `check_remote_datareader`, `check_remote_datawriter`), `peer_is_unauthenticated`, `topic_is_access_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_plugin_is_asked_about_the_endpoints_of_authenticated_participants_only`

**Status:** `done` — for an authenticated participant the runtime asks the plugin `check_remote_topic` and `check_remote_datareader` / `check_remote_datawriter` with the topic and partitions of the remote endpoint and matches only where it permits (`RelayOnly` for check_remote_datareader); for an unauthenticated one it does not ask the plugin (§9.8.8.1), and a protected topic is not matched there.

### §9.8.8.2 Figure 27 steps 4 and 6: instances of remote writers

**Spec:** §9.8.8.2, Figure 27, p. 201–202 — "4. DataReader1 shall call the
operation check_remote_datawriter_register_instance [...] If the verification
doesn't succeed, the sample shall be ignored. [...] 6. DataReader1 shall call
the operation check_remote_datawriter_dispose_instance [...] If the
verification doesn't succeed, the instance disposal shall be ignored."

**Repo:** `crates/dcps/src/runtime.rs::remote_instance_refused`; `crates/security-runtime/src/shared.rs::SharedSecurityGate::check_remote_datawriter_instance`

**Tests:** `crates/dcps/tests/security_remote_instance_e2e.rs::a_sample_of_an_instance_the_plugin_refuses_to_register_is_ignored`,
`crates/dcps/tests/security_remote_instance_e2e.rs::a_disposal_the_plugin_refuses_is_ignored`,
`crates/dcps/tests/security_remote_instance_e2e.rs::a_remote_writer_that_the_plugin_permits_registers_and_disposes`

**Status:** `done` — the DataReader calls `check_remote_datawriter_register_instance` for the first sample of an instance and `check_remote_datawriter_dispose_instance` for a disposal; when refused, the sample, the disposal, is ignored. The control with a permitting plugin delivers both.

### §9.8.9 Cryptographic Plugin key generation behavior

**Spec:** §9.8.9, p. 202–203 — "Key Generation is potentially needed for:
The DomainParticipant as a whole, Each DomainParticipant match pair, Each
builtin secure endpoint (DataWriter or DataReader), Each builtin secure
endpoint match pair, Each application secure endpoint (DataWriter or
DataReader), Each application secure endpoint match pair".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — enumeration; the cases are listed on their
own below.

### §9.8.9.1 Keys for VolatileMessageSecure from the SharedSecret, without tokens

**Spec:** §9.8.9.1, p. 203 — "Effectively this means the key material used
for key-exchange is always derived from the SharedSecret. [...] the DDS
middleware shall not call the operation create_local_datawriter_crypto_tokens
or the operation set_remote_datareader_crypto_tokens on the
CryptoKeyExchange."

**Repo:** `crates/security-runtime/src/shared.rs::register_remote_by_guid_from_secret`
(l. 732); `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`
(l. 21431, without the VolatileMessageSecure endpoints)

**Tests:** `crates/security-runtime/src/shared.rs::tests::kx_channel_round_trips_through_gate`,
`crates/security-crypto/src/plugin.rs::tests::cyclone_kx_key_matches_both_directions`

**Status:** done

### §9.8.9.1 Property dds.sec.builtin_endpoint_name

**Spec:** §9.8.9.1, p. 203 — "The DDS implementation shall add a property with
name 'dds.sec.builtin_endpoint_name' and value
'BuiltinParticipantVolatileMessageSecureWriter' to the Property_t passed to
the operation register_local_datawriter when it registers the
BuiltinParticipantVolatileMessageSecureWriter with the CryptoKeyFactory.
[...] value 'BuiltinParticipantVolatileMessageSecureReader' [...]
register_local_datareader".

**Repo:** `crates/security-runtime/src/gate_crypto.rs::volatile_properties`, `CryptoLayer::ensure_local`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_cryptographic_plugin_registers_the_participant_and_the_volatile_endpoints`

**Status:** `done` — the registration of the volatile message writer and reader carries the property `dds.sec.builtin_endpoint_name` with the names of the specification; it runs once, after `register_local_participant`.
### §9.8.9.2 register_local_participant for every local participant

**Spec:** §9.8.9.2, p. 203 — "For each local DomainParticipant that is
successfully created the DDS implementation shall call the operation
register_local_participant on the KeyFactory."

**Repo:** `crates/security-runtime/src/shared.rs::ensure_local` (l. 637)

**Tests:** `crates/security-runtime/src/shared.rs::tests::e2e_alice_bob_with_shared_gate`

**Status:** done — on first use, not on creation.

### §9.8.9.2 register_matched_remote_participant for every authenticated peer

**Spec:** §9.8.9.2, p. 203 — "For each discovered DomainParticipant that has
successfully authenticated and has been matched to the local
DomainParticipant the DDS middleware shall call the operation
register_matched_remote_participant on the KeyFactory."

**Repo:** `crates/dcps/src/runtime.rs::prepare_crypto_token` (l. 21368)

**Tests:** `crates/dcps/src/runtime.rs::tests::handshake_completes_through_runtime_dispatch_e2e`,
`crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done

### §9.8.9.3 Keys of the secure builtin endpoints

**Spec:** §9.8.9.3, p. 204 — "For each DataWriter belonging to the list of
'Builtin Secure Endpoints', see 7.5.8, with the exception of the
BuiltinParticipantVolatileMessageSecureWriter, the DDS middleware shall call
the operation register_local_datawriter [...] 1. Call the operation
KeyFactory::register_matched_remote_datawriter for each local DataWriter
[...] 2. Call the operation KeyFactory::register_matched_remote_datareader".

**Repo:** `crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`, `prepare_endpoint_crypto_tokens`; `crates/security-runtime/src/gate_crypto.rs::CryptoLayer::endpoint_tokens` (`register_matched_remote_datareader` / `_datawriter`)

**Tests:** `crates/dcps/tests/security_crypto_sequence.rs::the_builtin_secure_endpoints_are_registered_and_matched`

**Status:** `done` — `register_local_datawriter` / `register_local_datareader` for the secure builtin endpoints (the VolatileMessageSecure writer apart), then `register_matched_remote_datareader` / `_datawriter` and `create_local_*_crypto_tokens`, with the kinds of §10.4.1.2.5.

### §9.8.9.4 Keys of the application endpoints

**Spec:** §9.8.9.4, p. 204 — "For each non-builtin DataWriter for whom the
associated EndpointSecurityConfig object has either the member
is_submessage_protected or the member is_payload_protected set to TRUE, the
DDS middleware shall: 1. Call the operation register_local_datawriter [...]
2. Call the operation register_matched_remote_datareader for each discovered
DataReader that matches the DataWriter."

**Repo:** `crates/dcps/src/runtime.rs::register_endpoint_crypto`, `prepare_endpoint_crypto_tokens`; `crates/security-runtime/src/gate_crypto.rs::CryptoLayer::endpoint_tokens`

**Tests:** `crates/dcps/tests/security_crypto_sequence.rs::the_key_material_of_two_endpoints_and_the_protected_data_follow_the_figures`,
`crates/security-runtime/src/shared.rs::tests::per_endpoint_datawriter_token_roundtrips_by_key_id`

**Status:** `done` — `register_local_datawriter` with the config of the endpoint, `register_matched_remote_datareader` for the discovered DataReader, then the CryptoTokens; the other side sets them with `set_remote_datawriter_crypto_tokens`.

### §9.8.9.5 Procedure of a key revision

**Spec:** §9.8.9.5, p. 205 — "To perform a Key Revision the DDS middlewre
shall: 1. Call the operation revise_local_entity_keys [...] 2. Call the
operation create_local_participant_crypto_tokens [...] 5. Send the
CryptoTokens to the matched, authenticated, DomainParticipants [...] 6. Wait
until all the matched Authenticated DomainParticipant have acknowledged
receiving the CryptoTokens or else until sufficient time has elapsed. 7.
Call activate_key_revision".

**Repo:** `crates/dcps/src/runtime.rs::renew_key_material_after_revocation`, `complete_key_revision`; `RuntimeConfig::key_revision_ack_timeout`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_key_revision_is_activated_after_the_acknowledgement_or_the_timeout`,
`crates/dcps/src/runtime.rs::tests::a_revocation_makes_a_new_revision_of_the_keys_of_the_participant_and_its_endpoints`

**Status:** `done` — `revise_local_entity_keys`, the new CryptoTokens to the authenticated participants, waiting for the acknowledgement of the VolatileSecure writer or `key_revision_ack_timeout` (the specification gives no value: "sufficient time"; default 5 s, a programmatic setting), then `activate_key_revision`. The revision is triggered by the revocation of an identity (§9.3.2.12.2).

### §9.8.9.5 Rotation of keys is up to the plugins

**Spec:** §9.8.9.5, p. 205 — "DDS- Security does not use key revisions to
simply rotate the KeyMaterial because it has been used too long, or used to
protect too much data. Key Rotation for those purposes should be implemented
internally by the Security Plugins."

**Repo:** `crates/security-crypto/src/plugin.rs` (method `rotate_key`)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::rotate_key_resets_counter_and_changes_key`,
`crates/security-crypto/src/plugin.rs::tests::encrypts_remaining_decrements_per_call`

**Status:** done — details of the rotation of the builtin plugin are in
§10.5.

### §9.8.9.6 Limit size through keys for single receivers

**Spec:** §9.8.9.6, p. 205–206 — "The selection between and configuration of
these choices is implementation specific, as it does not affect
interoperability."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — notes for implementations; ZeroDDS
produces no MACs for single receivers in the runtime (see §9.5.1.10).

### §9.8.10 Cryptographic Plugin key exchange behavior

**Spec:** §9.8.10, p. 206 — "Cryptographic key exchange is potentially needed
for: Each DomainParticipant match pair. Each builtin secure endpoint match
pair. Each application secure endpoint match pair."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — enumeration.

### §9.8.10.1 Key exchange with a discovered participant

**Spec:** §9.8.10.1, p. 206–207 — "Cryptographic key exchange shall occur
between each DomainParticipant and each discovered DomainParticipant that has
successfully authenticated. [...] 1. Call the operation
create_local_participant_crypto_tokens [...] 2. Send the
ParticipantCryptoTokenSeq [...] using
BuiltinParticipantVolatileMessageSecureWriter. The discovered
DomainParticipant shall call the operation
set_remote_participant_crypto_tokens".

**Repo:** `crates/dcps/src/runtime.rs::prepare_crypto_token` (l. 21368),
`on_volatile_messages` (l. 22563)

**Tests:** `crates/security-runtime/src/shared.rs::tests::data_round_trips_via_token_after_kx_register`,
`crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done — the runtime sends ParticipantCryptoTokens only for
`rtps_protection_kind` other than NONE (l. 21387); this follows Table 33
(is_rtps_axk_protected), not the unconditional sentence of §9.8.10.1 (contradiction in the specification).

### §9.8.10.2 and §9.8.10.3 Key exchange with remote endpoints

**Spec:** §9.8.10.2, §9.8.10.3, p. 208–209 — "Cryptographic key exchange shall
occur between each builtin secure DataWriter and the matched builtin secure
DataReader entities of authenticated matched DomainParticipant entities [...]
Cryptographic key exchange shall also occur between each application
DataWriter whose EndpointSecurityConfig object has either the
is_submessage_protected or the is_payload_protected members set to TRUE, and
each of its matched DataReader entities."

**Repo:** `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`, `endpoint_is_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::key_material_exists_for_protected_endpoints_only_and_goes_with_them`, `::pending_endpoint_tokens_keeps_late_user_token_after_builtins_sent`, `::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** `done` — tokens go for the secure builtin endpoints by the protection kinds of the governance and for application endpoints with protected submessages or payload, per matching remote endpoint.

### §9.8.10.2 No tokens for endpoints without protection

**Spec:** §9.4.2.7, Table 37, p. 128 — "If is_submessage_protected is FALSE,
then the CryptoKeyFactory, CryptoKeyExchange and CryptoTransform operations
are called only if is_payload_protected is TRUE."

**Repo:** `crates/dcps/src/runtime.rs::endpoint_is_protected`, `register_endpoint_crypto`

**Tests:** `crates/dcps/src/runtime.rs::tests::key_material_exists_for_protected_endpoints_only_and_goes_with_them`

**Status:** `done` — an endpoint of a topic with neither submessage nor payload protection has no key material and gets no tokens.

### §9.8.10.4 Exchange after a key revision

**Spec:** §9.8.10.4, p. 210–211 — "Following the call to
revise_local_entity_keys the DDS middleware shall call the operation
create_local_participant_crypto_tokens [...] The DDS middleware shall wait
until the above CryptoTokens have been received or else a 'sufficient' time
has elapsed. After this it shall call activate_key_revision".

**Repo:** `crates/dcps/src/runtime.rs::renew_key_material_after_revocation`, `complete_key_revision`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_key_revision_is_activated_after_the_acknowledgement_or_the_timeout`,
`crates/dcps/src/runtime.rs::tests::a_revocation_makes_a_new_revision_of_the_keys_of_the_participant_and_its_endpoints`

**Status:** `done` — as §9.8.9.5: after `revise_local_entity_keys` the tokens go out; `activate_key_revision` follows the acknowledgement or the end of the time.

### §9.8.11 Order of encoding and decoding

**Spec:** §9.8.11, p. 211 — "This specification only requires that by the
time the RTPS message appears on the wire the proper encoding operations
have been executed first on each SerializedPayload submessage element, then
on the enclosing RTPS Submessage, and finally on the RTPS Message. Similarly
by the time a received RTPS Message is interpreted the proper decoding
operations are executed on the reverse order."

**Repo:** `crates/dcps/src/runtime.rs` (l. 4590–4602: payload, submessage,
message)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_full_secure_profile`,
`crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done

### §9.8.11.1 to §9.8.11.4 Sequences in Figure 31 to 34

**Spec:** §9.8.11.1–§9.8.11.4, Figures 31–34, p. 211–219 — encoding per writer
or reader submessage, then encode_rtps_message; decoding with
decode_rtps_message, preprocess_secure_submessage,
decode_datawriter_submessage or decode_datareader_submessage and
decode_serialized_payload.

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::encode_serialized_payload`, `encode_data_datawriter_by_handle`, `transform_outbound_for`, `transform_inbound_from`, `decode_data_by_key_id`, `decode_serialized_payload`; `crates/security-runtime/src/gate_crypto.rs::CryptoLayer::decode_submessage` (`preprocess_secure_submsg`)

**Tests:** `crates/dcps/tests/security_crypto_sequence.rs::the_key_material_of_two_endpoints_and_the_protected_data_follow_the_figures`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** `done` — Figures 31 and 32: `encode_serialized_payload`, `encode_datawriter_submessage`, `encode_rtps_message`; Figures 33 and 34: `decode_rtps_message`, `preprocess_secure_submsg`, `decode_datawriter_submessage`, `decode_serialized_payload`; order checked on the recording plugin.

### §10.1 Introduction

**Spec:** §10.1, p. 221 — "This specification defines the behavior and
implementation of at least one builtin plugin for each kind of plugin. The
builtin plugins provide out-of-the-box interoperability between
implementations of this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the plugins of Table 49 are
listed on their own.

### §10.1 Table 49 — Authentication: DDS:Auth:PKI-DH

**Spec:** §10.1, Table 49, p. 221 — "Authentication DDS:Auth:PKI-DH Uses PKI
with a pre-configured shared Certificate Authority."

**Repo:** `crates/security-pki/src/plugin.rs::PkiAuthenticationPlugin`
(`plugin_class_id`, l. 1071)

**Tests:** `crates/security-pki/src/plugin.rs::tests::plugin_class_id_matches_spec`

**Status:** done

### §10.1 Table 49 — AccessControl: DDS:Access:Permissions

**Spec:** §10.1, Table 49, p. 221 — "AccessControl DDS:Access:Permissions
Permissions document signed by shared Certificate Authority".

**Repo:** `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`
(`plugin_class_id`, l. 275); `crates/security-runtime/src/shared.rs::SharedSecurityGate`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::plugin_class_id_matches_spec`

**Status:** done — the runtime uses the plugin's types through
`SharedSecurityGate` (see §9.4.2).

### §10.1 Table 49 — Cryptography: DDS:Crypto:AES-GCM-GMAC

**Spec:** §10.1, Table 49, p. 221 — "Cryptography DDS:Crypto:AES-GCM-GMAC
AES-GCM (AES using Galois Counter Mode) for encryption. AES-GMAC for message
authentication."

**Repo:** `crates/security-crypto/src/plugin.rs::AesGcmCryptoPlugin`
(`plugin_class_id`, l. 2006)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::plugin_class_id_matches_spec`

**Status:** done

### §10.1 Table 49 — DataTagging: DDS:Tagging:DDS_Discovery

**Spec:** §10.1, Table 49, p. 221 — "DataTagging DDS:Tagging:DDS_Discovery
Send Tags via Endpoint Discovery".

**Repo:** `crates/security-runtime/src/data_tagging.rs::BuiltinDataTaggingPlugin` (`DATA_TAGGING_PLUGIN_CLASS_ID`); `crates/dcps/src/runtime.rs::DcpsRuntime::register_endpoint_tags`, `RuntimeConfig::security_data_tagging`

**Tests:** `crates/security-runtime/src/data_tagging.rs::tests::plugin_class_id_matches_spec_format`,
`crates/dcps/src/runtime.rs::tests::the_data_tagging_plugin_holds_the_tags_of_local_and_discovered_endpoints`,
`crates/dcps/src/runtime.rs::tests::without_a_data_tagging_plugin_the_qos_tags_are_announced`

**Status:** `done` — class id `DDS:Tagging:DDS_Discovery`; the runtime hands the tags of every endpoint to the plugin, announces what it returns, and registers the tags of discovered endpoints.
### §10.1 Table 49 — Logging: DDS:Logging:DDS_LogTopic

**Spec:** §10.1, Table 49, p. 221 — "Logging DDS:Logging:DDS_LogTopic Logs
security events to a dedicated DDS Log Topic".

**Repo:** `crates/security-logging/src/log_topic.rs::LogTopicLoggingPlugin`
(`plugin_class_id`, l. 728)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::plugin_class_id_is_the_spec_name`

**Status:** done

### §10.2 Requirements and Priorities (Non-Normative)

**Spec:** §10.2, p. 221–222 — "The selection of the builtin plugins was
driven by several functional, as well as, non-functional requirements".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — marked as non-normative.

### §10.2.1 Performance and Scalability

**Spec:** §10.2.1, p. 222 — "The use of Asymmetric Key Cryptography shall be
limited to the discovery, authentication, session and shared-secret
establishment phase".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — part of the non-normative §10.2.

### §10.2.2 Robustness and Availability

**Spec:** §10.2.2, p. 222–223 — "Centralized policy decision points or
services should be avoided."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — part of the non-normative §10.2.

### §10.2.3 Fitness to the DDS Data-Centric Model

**Spec:** §10.2.3, p. 223 — "any encryption and/or digital signature applied
to a sample should be able to be processed in isolation".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — part of the non-normative §10.2.

### §10.2.4 Leverage and Reuse of Existing Security Infrastructure and Technologies

**Spec:** §10.2.4, p. 223 — "The builtin plugins leverage existing standards
and tools for PKI, ciphers, hashing and digital signing."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — part of the non-normative §10.2.

### §10.2.5 Ease-of-Use while Supporting Common Application Requirements

**Spec:** §10.2.5, p. 223 — "The builtin plugins balance rich functionality
and ease-of-use".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — part of the non-normative §10.2.

### §10.3 Configuration with CA, private key and certificate before enabling

**Spec:** §10.3, p. 224 — "Prior to a DomainParticipant being enabled the
DDS:Auth:PKI-DH plugin associated with the DomainParticipant must be
configured with three things: 1. The X.509 Certificate that defines the
Shared Identity CA. [...] 2. The Private Key of the DomainParticipant. 3. An
X.509 Certificate that chains up to the Shared Identity CA".

**Repo:** `crates/security-runtime/src/qos_properties.rs::profile_config_from_properties`
(l. 380); `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`
(l. 301)

**Tests:** `crates/dcps/tests/security_qos_properties.rs::a_complete_dds_sec_property_list_creates_a_secured_participant`,
`crates/security-runtime/src/qos_properties.rs::tests::each_missing_property_is_reported_by_its_own_name`

**Status:** done

### §10.3.1 Table 50 — configuration through PropertyQosPolicy with prefix dds.sec.auth

**Spec:** §10.3.1, Table 50, p. 224–225 — "The builtin authentication plugin
shall be configured using the PropertyQosPolicy of the
DomainParticipantQos. [...] (all properties have 'dds.sec.auth' prefix) [...]
(all these properties shall have propagate set to FALSE) URI syntax follows
IETF RFC 3986."

**Repo:** `crates/security-runtime/src/qos_properties.rs` (constants
`PROP_IDENTITY_CA` to `PROP_KEY_ESTABLISHMENT`, l. 72–88)

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_property_names_are_the_spec_strings`,
`crates/security-runtime/src/qos_properties.rs::tests::the_profile_config_follows_the_property_list`

**Status:** done — `PropertyQosPolicy` has no `propagate`; the properties are
not sent over discovery (see §7.3.21).

### §10.3.1 Table 50 — identity_ca

**Spec:** §10.3.1, Table 50, p. 225 — "identity_ca URI to the X509
certificate [39] of the Identity CA. Supported URI schemes: file, data,
pkcs11".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`
(l. 302), `resolve_file_uri` (l. 252)

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::every_file_uri_form_resolves_to_a_path`,
`crates/security-runtime/src/qos_properties.rs::tests::a_data_uri_is_the_content_and_not_a_path`,
`crates/security-runtime/src/pkcs11.rs::tests::every_path_attribute_parses`

**Status:** done — `pkcs11:` only with the feature `pkcs11` of
`zerodds-security-runtime`; without the feature the value is refused with
`Pkcs11FeatureDisabled`.

### §10.3.1 Table 50 — private_key

**Spec:** §10.3.1, Table 50, p. 225 — "private_key URI to access the private
Private Key for the DomainParticipant Supported URI schemes: file, data,
pkcs11 [...] file:identity_ca_private_key.pem?password=OpenSesame [...]
pkcs11:object=MyParticipantPrivateKey;type=private?pin-value=OpenSesame".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`, `resolve_private_key`; `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_uri_schemes_that_are_not_paths_are_named_not_opened_as_paths`, `::a_password_in_the_key_uri_is_not_part_of_the_path`, `crates/security-runtime/src/pkcs11.rs::tests::every_query_attribute_parses`

**Status:** `done` — `file:`, `data:` and `pkcs11:` with `pin-value` are read; `?password=` on `file:` is the password.

### §10.3.1 Table 50 — private_key: query ?password= on file:

**Spec:** §10.3.1, Table 50, p. 225 — "file:identity_ca_private_key.pem?password=OpenSesame".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_private_key`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::a_password_in_the_key_uri_is_not_part_of_the_path`

**Status:** `done` — the query is not part of the path; `password` is percent-decoded; the property `dds.sec.auth.password` takes precedence.

### §10.3.1 Table 50 — password

**Spec:** §10.3.1, Table 50, p. 225 — "The value of the password property
shall be interpreted as the Base64 encoding of the AES-128 key that shall be
used to decrypt the private_key using AES128-CBC. If the password property is
not present, then the value supplied in the private_key property must contain
the unencrypted private key."

**Repo:** `crates/security-pki/src/encrypted_key.rs::decrypt_private_key_pem_as`, `PasswordReading`; `crates/security-runtime/src/qos_properties.rs::profile_config_from_properties`

**Tests:** `crates/security-pki/src/encrypted_key.rs::tests::the_clause_reading_is_the_default_and_the_passphrase_is_named`, `crates/security-pki/src/encrypted_key.rs::tests::the_clause_key_opens_a_file_encrypted_with_it`, `crates/security-pki/src/encrypted_key.rs::tests::an_unencrypted_key_ignores_the_password`, `crates/security-runtime/src/qos_properties.rs::tests::a_key_password_reaches_the_profile_config`, `::the_password_reading_is_the_clause_unless_named`

**Status:** `done` — the default is the clause's reading (Base64 of the AES-128 key); the OpenSSL passphrase reading (Cyclone, Fast DDS) is the setting `zerodds.sec.auth.password_reading=openssl_passphrase`.

### §10.3.1 Table 50 — password does not apply to pkcs11:

**Spec:** §10.3.1, Table 50, p. 225 — "The password property is only used if
the private_key is provided with a 'file:' or a 'data:' URI. It does not
apply to private keys supplied with the 'pkcs11:' URI."

**Repo:** `crates/security-runtime/src/profile.rs::private_key_pem`

**Tests:** `crates/security-runtime/src/profile.rs::tests::a_password_does_not_apply_to_a_pkcs11_key`,
`crates/security-runtime/src/qos_properties.rs::tests::a_key_password_reaches_the_profile_config`

**Status:** `done` — a key on a token is never decrypted, a password next to it is not applied and is no error (the PIN is `pin-value` of the URI); for `file:` and `data:` it opens the key, a wrong one is an error. The test needs no token, because the decision is taken before the access.

### §10.3.1 Table 50 — identity_certificate

**Spec:** §10.3.1, Table 50, p. 226 — "identity_certificate URI to a X509
certificate signed by the IdentityCA in PEM format containing the signed
public key for the DomainParticipant Supported URI schemes: file, data,
pkcs11".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`
(l. 302)

**Tests:** `crates/dcps/tests/security_qos_properties.rs::every_file_uri_spelling_is_resolved_on_the_participant_path`,
`crates/security-runtime/src/qos_properties.rs::tests::a_data_uri_with_a_media_type_decodes_by_rfc_2397`

**Status:** done

### §10.3.1 Table 50 — key_establishment_algorithm

**Spec:** §10.3.1, Table 50, p. 226 — "key_establishment_algorithm The string
'AUTO' or one of the CryptoAlgorithmName strings shown in Table 26 that
identifies a Key Establishment Algorithm. (The presence of this property is
optional)"

**Repo:** `crates/security-runtime/src/qos_properties.rs::kx_suite_from_properties`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_key_establishment_property_selects_a_suite_and_refuses_the_unknown`

**Status:** `done` — `AUTO`, `ECDHE-CEUM+P256`, `ECDHE-CEUM+P384`, `DHE+MODP-2048-256` and absence; also `ECDH+prime256v1-CEUM`, `ECDHE-CEUM-P256`, `DH+MODP-2048-256` and `X25519`.

### §10.3.1 Table 50 — key_establishment_algorithm: AUTO and DHE+MODP-2048-256

**Spec:** §10.3.1.4, p. 226–227 — "The key_establishment_algorithm property
may be omitted or may have the value set to 'AUTO'. In both these cases, the
selection of the algorithm will be left to the Authentication plugin."

**Repo:** `crates/security-runtime/src/qos_properties.rs::kx_suite_from_properties`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_key_establishment_property_selects_a_suite_and_refuses_the_unknown`

**Status:** `done` — `AUTO` is ECDHE-CEUM+P256, which every vendor runs.

### §10.3.1.1 Identity CA Certificate

**Spec:** §10.3.1.1, p. 226 — "The certificate shall be the X.509 v3
Certificate [39] of the issuer of the Identity Certificates [...] The
certificate can be self-signed if it is a root CA or signed by some other CA
public key if it is a subordinate CA. Regardless of this the Public Key in
the Certificate shall be accepted as the one for the Identity CA trusted to
sign DomainParticipant Identity Certificates".

**Repo:** `crates/security-pki/src/plugin.rs::validate_with_config` (l. 368)

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_accepts_ca_signed_cert`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_rejects_wrong_ca`

**Status:** done — a test with a subordinate CA as trust anchor was not
found (not checked whether it behaves like a root).

### §10.3.1.2 Private Key: algorithm per §8.2

**Spec:** §10.3.1.2, p. 226 — "The Private Key associated with the
DomainParticipant. The algorithm of the private key of the CA shall be one of
the algorithms defined in 8.2."

**Repo:** `crates/security-pki/src/plugin.rs::sign_with`, `signing_scheme`; `crates/security-pki/src/identity.rs::private_key_pkcs8_der`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_p384_identity_authenticates_with_ecdsa_p384`,
`crates/security-pki/src/plugin.rs::tests::an_rsa_identity_signs_pkcs1_when_the_peer_verifies_only_that`,
`crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`,
`crates/security-pki/tests/private_key_forms.rs::a_pkcs1_rsa_key_reaches_the_backend_as_pkcs8`,
`crates/security-pki/tests/private_key_forms.rs::a_sec1_ec_key_reaches_the_backend_as_the_same_pkcs8_openssl_writes`

**Status:** `done` — the four algorithms of Table 25 run: ECDSA P-256 (default), ECDSA P-384, RSASSA-PSS and RSASSA-PKCS1-v1_5 with an RSA key; the forms PKCS#8, PKCS#1 and SEC1 are read.

### §10.3.1.3 Identity Certificate

**Spec:** §10.3.1.3, p. 226 — "An X.509 v3 Certificate [39] that chains up
to the Identity CA (see 10.3.1.1). The Identity Certificate binds the Public
Key of the DomainParticipant to the Distinguished Name (subject name) for the
DomainParticipant."

**Repo:** `crates/security-pki/src/plugin.rs::validate_with_config` (l. 368)

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_accepts_ca_signed_cert`,
`crates/security-runtime/src/qos_properties.rs::tests::the_x509v1_deviation_comes_out_of_the_property_list`

**Status:** done — an X.509 v1 certificate is accepted only with the own
property `PROP_ACCEPT_X509V1_IDENTITY` (default off).

### §10.3.1.4 Key establishment algorithm of the initiator

**Spec:** §10.3.1.4, p. 226–227 — "The Key Establishment Algorithm that the
DomainParticipant will use in the situations when the DomainParticipant
initiates the Authentication handshake. The algorithm shall be one of the
algorithms defined in 8.2."

**Repo:** `crates/security-runtime/src/qos_properties.rs::kx_suite_from_properties`; `crates/security-pki/src/plugin.rs::set_preferred_kx_suite`, `begin_handshake_request`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_key_establishment_property_selects_a_suite_and_refuses_the_unknown`,
`crates/security-pki/src/plugin.rs::tests::kagree_algo_string_round_trips_for_every_supported_suite`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_runs_over_dhe_modp_2048_256`,
`crates/security-pki/src/plugin.rs::tests::set_participant_security_config_fails_when_a_used_algorithm_is_not_supported`

**Status:** `done` — the property selects the key agreement of the initiator: `AUTO` and absence are ECDHE-CEUM+P256, plus ECDHE-CEUM+P384 and DHE+MODP-2048-256 (Table 26); `set_participant_security_config` checks that it is in the supported mask. The reference to §8.2 instead of §8.3 is a contradiction of the spec.

### §10.3.2 Unspecified attributes and unknown properties

**Spec:** §10.3.2, p. 227 — "Plugin implementations may place data in these
attributes as long as they also include a property attribute that allows the
implementation to unambiguously detect the presence and interpret these
attributes. Attributes that are not understood shall be ignored."

**Repo:** `crates/security-pki/src/handshake_token.rs::parse_reply_token`
(l. 604), `parse_request_token` (l. 479)

**Tests:** `crates/security-pki/src/plugin.rs::tests::extra_unknown_properties_in_reply_accepted_forward_compat`,
`crates/security-pki/src/identity_token.rs::tests::unknown_property_skipped_in_decode`

**Status:** done

### §10.3.2.1 Table 51 — IdentityToken: properties

**Spec:** §10.3.2.1, Table 51, p. 227 — "class_id 'DDS:Auth:PKI-DH:1.2'
properties [...] dds.cert.sn The subject name of the Identity Certificate.
dds.cert.algo One of the CryptoAlgorithmName string identifiers for digital
signature algorithms defined in Table 25. dds.ca.sn [...] dds.ca.algo".

**Repo:** `crates/security-pki/src/identity_token.rs::IdentityToken::encode`, `decode`, `build_identity_token_from_der_named`

**Tests:** `crates/security-pki/src/identity_token.rs::tests::a_token_without_the_optional_properties_decodes`,
`crates/security-pki/src/identity_token.rs::tests::an_empty_property_is_left_out_of_the_token`,
`crates/security-pki/src/identity_token.rs::tests::the_algorithms_are_the_names_of_table_25`,
`crates/security-pki/src/identity_token.rs::tests::round_trip_full_token`,
`crates/security-pki/src/plugin.rs::tests::the_identity_token_names_the_algorithms_of_table_25`

**Status:** `done` — `dds.cert.sn`, `dds.cert.algo`, `dds.ca.sn`, `dds.ca.algo`: each property is optional (without a value it is not sent, a missing field is empty on reading).

### §10.3.2.1 Table 51 — class_id DDS:Auth:PKI-DH:1.2 and names per Table 25

**Spec:** §10.3.2.1, Table 51, p. 227 — "class_id 'DDS:Auth:PKI-DH:1.2'
[...] dds.cert.algo One of the CryptoAlgorithmName string identifiers for
digital signature algorithms defined in Table 25."

**Repo:** `crates/security-pki/src/identity_token.rs::crypto_algorithm_name`, `legacy_cert_algo`; `crates/security-pki/src/plugin.rs::validate_remote_identity`

**Tests:** `crates/security-pki/src/identity_token.rs::tests::the_algorithms_are_the_names_of_table_25`,
`crates/security-pki/src/identity_token.rs::tests::the_names_of_1_1_are_the_opt_in`,
`crates/security-pki/src/plugin.rs::tests::the_identity_token_names_the_algorithms_of_table_25`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_compares_the_plugin_name_and_the_major_version`

**Status:** `done` — `dds.cert.algo` and `dds.ca.algo` carry the names of Table 25 (`ECDSA+P256+SHA256`, `ECDSA+P384+SHA384`, `RSASSA-PSS-MGF1SHA256+2048+SHA256`, `RSASSA-PKCS1-V1_5+2048+SHA256`); an algorithm without a name in Table 25 (ecdsa-with-SHA512) leaves the property out. Only `org.zerodds.sec.algorithm_names=1.1` sends the names of 1.1 (`EC-prime256v1`, `RSA-2048`). The comparison of the class concerns name and MajorVersion. `DDS:Auth:PKI-DH:1.0` is sent (the text of §10.3.2.1 sets MinorVersion 0, Table 51 says 1.2: contradiction of the spec; only name and MajorVersion are compared).

### §10.3.2.1 class_id: PluginClassName, MajorVersion, MinorVersion

**Spec:** §10.3.2.1, p. 228 — "The value of the class_id shall be interpreted
as composed of three parts: a PluginClassName, a MajorVersion and a
MinorVersion according to the following format:
<PluginClassName>:<MajorVersion>.<MinorVersion>. The PluginClassName is
separated from the MajorVersion by the last ':' character in the class_id."

**Repo:** `crates/security-pki/src/handshake_token.rs::of_class_id` (l. 88)

**Tests:** `crates/security-pki/src/handshake_token.rs::class_version_tests::the_peers_version_is_read_and_carried_back`

**Status:** done — the peer's version is read and returned in the
handshake tokens.

### §10.3.2.1 Table 52 — IdentityToken with pre-shared key only

**Spec:** §10.3.2.1, Table 52, p. 228 — "class_id 'DDS:Auth:PSK:1.2'
properties [...] dds.psk.algo One of the CryptoAlgorithmName string
identifiers for Symmetric Cipher AEAD and MAC Algorithms".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — reference; the token belongs to the plugin
DDS:Auth:PSK and is listed in §10.7.2.

### §10.3.2.2 Table 53 — IdentityStatusToken

**Spec:** §10.3.2.2, Table 53, p. 228 — "class_id 'DDS:Auth:PKI-DH:1.0'
binary_properties [...] ocsp_stat A DER-encoded OCSP response (using the ASN.1
type OCSPResponse defined in clause 4.2.1 of RFC 2560 [54]) that provides the
status of the identity certificate of the DomainParticipant."

**Repo:** `crates/security-pki/src/plugin.rs::get_identity_status_token`, `identity_status_bytes`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_identity_status_token_carries_the_ocsp_response`,
`crates/security-pki/src/plugin.rs::tests::a_changed_status_token_is_reported_as_identity_status`

**Status:** `done` — class `DDS:Auth:PKI-DH:1.0`, the binary property `ocsp_status` with the OCSP response for the own certificate when there is one (the property is optional). The own class `+IdentityStatus` of `identity_status.rs` stays without a caller.

### §10.3.2.3 Table 54 — AuthenticatedPeerCredentialToken

**Spec:** §10.3.2.3, Table 54, p. 228 — "class_id 'DDS:Auth:PKI-DH:1.0'
binary_properties [...] c.id Contents of the certificate signed by IdentityCA
that was received from the peer [...] c.perm Contents of the permissions
document signed by the PermissionCA".

**Repo:** `crates/security-pki/src/plugin.rs::get_authenticated_peer_credential_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`

**Status:** `done` — class `DDS:Auth:PKI-DH:1.0`, binary properties `c.id` (the certificate of the peer as it stood in the request or reply) and `c.perm` (its permissions document).

### §10.3.2.4 Table 55 — AuthRequestMessageToken

**Spec:** §10.3.2.4, Table 55, p. 229 — "class_id
'DDS:Auth:PKI-DH:1.0+AuthReq' binary_properties [...] future_challenge A
256-bit NONCE generated by the Participant [...] The value shall match what
will be sent on the challenge1 property of the HandshakeRequestMessageToken
or the challenge2 property of the HandshakeReplyMessageToken."

**Repo:** `crates/security-pki/src/handshake_token.rs::build_auth_request_token`, `parse_auth_request_token`; `crates/security-pki/src/plugin.rs::validate_remote_identity`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/security-pki/src/plugin.rs::tests::the_auth_request_announces_the_challenge1_of_the_request`,
`crates/security-pki/src/plugin.rs::tests::a_request_whose_challenge1_is_not_the_announced_one_is_refused`

**Status:** `done` — class `DDS:Auth:PKI-DH:1.0+AuthReq`, binary property `future_challenge` with 256 bits; it is `challenge1` of the request or `challenge2` of the reply. The TLV encoder `auth_request.rs` is superseded and stays without a caller.

### §10.3.2.5.1 Table 56 — HandshakeRequestMessageToken

**Spec:** §10.3.2.5.1, Table 56, p. 230 — "class_id
'DDS:Auth:PKI-DH:1.0+Req' binary_properties c.id [...] c.perm [...] c.pdata
The CDR Big Endian Serialization of the ParticipantBuiltinTopicData
c.dsign_algo [...] c.kagree_algo [...] hash_c1 [...] dh1 [...] challenge1 A
256-bit NONCE [...] ocsp_status Inclusion of this property is optional."

**Repo:** `crates/security-pki/src/handshake_token.rs::build_request_token`; `crates/security-pki/src/plugin.rs::begin_handshake_request`

**Tests:** `crates/security-pki/src/plugin.rs::tests::request_token_has_spec_class_id_and_properties`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_names_the_algorithms_as_table_25_and_26_unless_the_peer_reads_1_1`,
`crates/security-pki/src/plugin.rs::tests::the_algorithm_names_follow_the_version_of_the_peer_and_the_setting`,
`crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`

**Status:** `done` — `class_id` and all properties; `c.dsign_algo` and `c.kagree_algo` carry the names of Table 25 and 26 (the default). Cyclone, Fast DDS and OpenDDS read the names of 1.1: `org.zerodds.sec.algorithm_names=1.1` sends them to every peer, `auto` to a peer of version 1.0 and to one of these vendors (by `VendorId`). `ocsp_status` goes along when the response of the responder is there.

### §10.3.2.5.1 hash_c1 over the c.* properties

**Spec:** §10.3.2.5.1, Table 56, p. 230 — "hash_c1 SHA-256 hash of the CDR
Big Endian serialization of a BinaryPropertySeq object containing all the
properties above that start with 'c.' placed in the same order as they appear
above."

**Repo:** `crates/security-pki/src/handshake_token.rs::compute_hash_c`
(l. 206), `serialize_bin_prop_seq` (l. 259)

**Tests:** `crates/security-pki/src/handshake_token.rs::tests::hash_c_is_deterministic_and_input_separated`,
`crates/security-pki/src/handshake_token.rs::tests::parse_request_token_rejects_tampered_hash_c1`,
`crates/security-pki/src/plugin.rs::tests::hash_c1_mismatch_in_request_rejected`

**Status:** done

### §10.3.2.5.2 Table 57 — HandshakeReplyMessageToken

**Spec:** §10.3.2.5.2, Table 57, p. 231–232 — "class_id
'DDS:Auth:PKI-DH:1.0+Reply' binary_properties c.id [...] c.perm [...]
c.pdata [...] c.dsign_algo [...] c.kagree_algo [...] hash_c2 [...] dh2 [...]
hash_c1 [...] dh1 [...] challenge1 [...] challenge2 [...] ocsp_status [...]
signature".

**Repo:** `crates/security-pki/src/handshake_token.rs::build_reply_token`; `crates/security-pki/src/plugin.rs::begin_handshake_reply`

**Tests:** `crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_names_the_algorithms_as_table_25_and_26_unless_the_peer_reads_1_1`,
`crates/security-pki/src/plugin.rs::tests::a_replier_answers_in_the_names_of_the_request`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** `done` — complete; the replier answers in the names of the request (`c.dsign_algo`, `c.kagree_algo`), so that every vendor gets its own spelling back.

### §10.3.2.5.2 Signature of the reply

**Spec:** §10.3.2.5.2, Table 57, p. 232 — "signature The Digital Signature of
the CDR Big Endian serialization of a BinaryPropertySeq object containing the
properties: hash_c2, challenge2, dh2, challenge1, dh1, and hash_c1, placed in
that order. [...] The digital signature shall be computed using the Private
Key associated with the DomainParticipant".

**Repo:** `crates/security-pki/src/handshake_token.rs::reply_signing_bytes`
(l. 289); `crates/security-pki/src/plugin.rs::sign_with_identity` (l. 566)

**Tests:** `crates/security-pki/src/plugin.rs::tests::signature_tamper_rejected_by_initiator`,
`crates/security-pki/src/plugin.rs::tests::dh_tamper_in_reply_breaks_final_signature`,
`crates/security-pki/src/handshake_token.rs::tests::reply_and_final_content_differ_by_order`

**Status:** done

### §10.3.2.5.2 SharedSecret as SHA-256 of the agreement result

**Spec:** §10.3.2.5.2, p. 232 — "Regardless of the key agreement algorithm,
the SharedSecret [...] shall be computed as the SHA256 hash of the derived
shared secret computed by the key agreement algorithm."

**Repo:** `crates/security-pki/src/plugin.rs::derive_shared_secret` (l. 701)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_shared_secret_returns_stored_value`,
`crates/security-pki/tests/e2e_handshake.rs::full_three_stage_handshake`

**Status:** done

### §10.3.2.5.3 Table 58 — HandshakeFinalMessageToken

**Spec:** §10.3.2.5.3, Table 58, p. 232–233 — "class_id
'DDS:Auth:PKI-DH:1.0+Final'. binary_properties hash_c1 [...] hash_c2 [...]
dh1 [...] dh2 [...] challenge1 [...] challenge2 [...] signature The Digital
Signature of the CDR Big Endian serialization of a BinaryPropertySeq object
containing the properties: hash_c1, challenge1, dh1, challenge2, dh2, and
hash_c2, placed in that order."

**Repo:** `crates/security-pki/src/handshake_token.rs::build_final_token`
(l. 690), `final_signing_bytes` (l. 313)

**Tests:** `crates/security-pki/src/plugin.rs::tests::final_token_challenge1_tamper_rejected`,
`crates/security-pki/src/plugin.rs::tests::final_token_dh2_tamper_rejected`,
`crates/security-pki/src/plugin.rs::tests::final_token_hash_c2_tamper_rejected`

**Status:** done

### §10.3.2.5.3 The initiator decides the key agreement

**Spec:** §10.3.2.5.3, p. 233 — "The parameters and algorithm shall be
determined based on the value of the HandshakeRequestMessageToken parameter
with key c.kagree_algo. In other words, it is the Participant that creates
the HandshakeRequestMessageToken the one that controls the key agreement
algorithm used."

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_reply`
(l. 900, `kx_suite_for_algo(&req.kagree_algo)`)

**Tests:** `crates/security-pki/src/plugin.rs::tests::kagree_algo_string_round_trips_for_every_supported_suite`,
`crates/security-pki/tests/e2e_handshake.rs::handshake_with_different_cert_dh_combos`

**Status:** done

### §10.3.3 Table 59 — validate_local_identity: check of certificate, CRL and OCSP

**Spec:** §10.3.3, Table 59, p. 233 — "The operation shall verify the
validity of the X509 certificate associated with the property named
dds.sec.auth.identity_certificate using the CA configured by the
dds.sec.auth.identity_ca property. The operation shall check a CRL and/or an
OCSP (RFC 2560 [54]) responder. This includes checking the expiration date of
the certificate."

**Repo:** `crates/security-pki/src/ocsp.rs::query_responder`, `check_identity`, `verify_staple`; `crates/security-pki/src/identity.rs::verify_cert_chain_checked`; `crates/security-pki/src/plugin.rs::validate_local_identity_nonblocking`

**Tests:** `crates/security-pki/src/ocsp.rs::tests::the_responder_in_the_certificate_is_asked`,
`crates/security-pki/src/ocsp.rs::tests::an_https_responder_is_asked_over_tls`,
`crates/security-pki/src/ocsp.rs::tests::an_https_responder_with_a_foreign_certificate_is_not_believed`,
`crates/security-pki/src/ocsp.rs::tests::a_responder_url_of_another_scheme_is_refused`,
`crates/security-pki/src/ocsp.rs::tests::an_unreachable_responder_fails_unless_the_setting_says_accept`,
`crates/security-pki/src/ocsp.rs::tests::a_responder_that_is_slow_gives_pending_retry_until_the_answer_is_in`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_with_a_foreign_ca_fails_without_a_handle`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_rejects_wrong_ca`

**Status:** `done` — chain, validity, CRL (`dds.sec.auth.crl`) and OCSP: the responder of the Authority Information Access is asked over `http://` or `https://` (RFC 6960 Appendix A names no scheme; TLS with rustls, the server certificate against the identity CA, feature `ocsp-https`, on by default); `good` goes along as `ocsp_status`. When the status cannot be determined the certificate is refused, except with `zerodds.sec.auth.ocsp_unreachable=accept` (warning in the security log). The query does not block the call: VALIDATION_PENDING_RETRY (thread). A certificate without a responder and without a CRL is not checked for revocation (the spec asks for "CRL and/or OCSP"; there is nothing to ask). No cache of the answers. Measured against a foreign responder: no.

### §10.3.3 Table 59 — CRL and OCSP are not consulted

**Spec:** §10.3.3, Table 59, p. 233, 236–238 — "The operation shall check a
CRL and/or an OCSP (RFC 2560 [54]) responder. [...] If the property
ocsp_status is present, the operation shall verify that the OCSP response
included in the property corresponds to the identity in the c.id property.
[...] If the status is revoked, the operation shall fail".

**Repo:** `crates/security-pki/src/ocsp.rs::verify_staple`, `check_identity`, `query_responder`; `crates/security-pki/src/identity.rs::verify_cert_chain_checked`

**Tests:** `crates/security-pki/src/ocsp.rs::tests::a_signed_response_for_the_certificate_gives_its_status`, `::a_response_that_does_not_hold_says_nothing`, `::a_delegated_responder_signs_only_with_the_ocsp_signing_purpose`, `::a_stapled_status_decides_and_a_useless_one_is_none`, `::the_responder_in_the_certificate_is_asked`, `::an_unreachable_responder_fails_unless_the_setting_says_accept`, `crates/security-runtime/src/qos_properties.rs::tests::the_ocsp_policy_refuses_by_default_and_reads_its_settings`

**Status:** `done` — the `ocsp_status` of a request or reply token is evaluated (CertID, validity interval, signature of the CA or of a responder certificate issued for OCSP signing): `good` is the proof, `revoked` fails, a response that does not hold counts as absent. `crl.rs` has no caller; `rustls-webpki` checks the CRLs.

### §10.3.3 Table 59 — adjusted_participant_guid

**Spec:** §10.3.3, Table 59, p. 233–234 — "The first bit (bit 0) shall be
set to 1. The 47 bits following the first bit (bits 1 to 47) shall be set to
the 47 first bits of the SHA-256 hash of the ASN.1 DER encoding of the
SubjectName [40] appearing on the identity_credential. The following 48 bits
(bits 48 to 95) shall be set to the first 48 bits of the SHA-256 hash of the
candidate_participant_guid."

**Repo:** `crates/security-pki/src/guid_adjust.rs::adjust_participant_guid_prefix`
(l. 28)

**Tests:** `crates/security-pki/src/guid_adjust.rs::tests::adjusted_prefix_sets_high_bit_and_is_deterministic_per_identity`,
`crates/dcps/tests/security_guid_wiring_e2e.rs::with_security_bundle_wires_the_identity_adjusted_prefix_not_random`

**Status:** done — the computation runs in
`crates/security-runtime/src/profile.rs` (l. 319), not in
`validate_local_identity` (see §9.3.2.11.2).

### §10.3.3 Table 59 — get_identity_token

**Spec:** §10.3.3, Table 59, p. 234 — "If the above condition is not met, the
operation shall return the exception DDS_SecurityException_PreconditionError.
This operation shall return an IdentityToken object with the content
specified in 10.3.2.1."

**Repo:** `crates/security-pki/src/plugin.rs::get_identity_token` (l. 791)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_identity_token_returns_decodable_spec_descriptor`

**Status:** done — an unknown handle yields `BadArgument`; for the content
see Table 51.

### §10.3.3 Table 59 — get_identity_status_token

**Spec:** §10.3.3, Table 59, p. 234 — "This operation shall return an
IdentityToken object with the content specified in 10.3.2.2."

**Repo:** `crates/security-pki/src/plugin.rs::get_identity_status_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_identity_status_token_carries_the_ocsp_response`

**Status:** `done` — see Table 53.

### §10.3.3 Table 59 — set_participant_security_config

**Spec:** §10.3.3, Table 59, p. 234 — "Check that the
digital_signature.trust_chain.supported_mask includes at least one algorithm
that is supported by the AUTH plugin. Fail otherwise. [...] The operation
shall configure the AUTH plugin to only accept the resulting set of supported
algorithms in the adjusted_algorithm_info."

**Repo:** `crates/security-pki/src/plugin.rs::set_participant_security_config`

**Tests:** `crates/security-pki/src/plugin.rs::tests::set_participant_security_config_adjusts_a_copy_of_the_given_info`,
`crates/security-pki/src/plugin.rs::tests::set_participant_security_config_fails_without_an_algorithm_of_the_plugin`,
`crates/security-pki/src/plugin.rs::tests::set_participant_security_config_fails_when_a_used_algorithm_is_not_supported`,
`crates/security-pki/src/plugin.rs::tests::the_configured_algorithms_bind_the_handshake`

**Status:** `done` — the three checks for at least one supported algorithm, the adjustment as a copy (unsupported bits out of the supported masks, certificate, key and the initiator's method into the required masks), the two checks "present in supported_mask" and the limit of the handshake to the adjusted masks.

### §10.3.3 Table 59 — set_permissions_credential_and_token

**Spec:** §10.3.3, Table 59, p. 234 — "This operation shall store the
PermissionsCredentialToken and the PermissionsToken internally to the plugin
and associate them with the DomainParticipant represented by the
IdentityHandle."

**Repo:** `crates/security-pki/src/plugin.rs::set_permissions_credential_and_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_permissions_credential_is_what_the_handshake_sends_as_c_perm`,
`crates/security-pki/src/plugin.rs::tests::a_permissions_credential_that_is_none_is_refused`

**Status:** `done` — credential token and PermissionsToken are stored per identity; `set_local_permissions` stays as the way for callers without a token.

### §10.3.3 Table 59 — validate_remote_identity: comparison of the class_id

**Spec:** §10.3.3, Table 59, p. 235 — "The operation shall compare the
class_id of the local identity_token with that of the remote_identity_token.
If the PluginClassName or the MajorVersion are different, it shall return
VALIDATION_FAILED."

**Repo:** `crates/security-pki/src/plugin.rs::validate_remote_identity`

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_remote_identity_compares_the_plugin_name_and_the_major_version`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_takes_only_a_pki_dh_identity_token`

**Status:** `done` — name and MajorVersion of the IdentityToken are compared with those of the own (`DDS:Auth:PKI-DH`, 1); `DDS:Auth:PKI-DHX:1.0`, 2.0 and a token that is no DataHolder are VALIDATION_FAILED; every MinorVersion is the same.

### §10.3.3 Table 59 — validate_remote_identity: order of the GUIDs

**Spec:** §10.3.3, Table 59, p. 235 — "If the remote_participant_guid >
local_participant_guid, the operation shall return
VALIDATION_PENDING_HANDSHAKE_REQUEST. If the remote_participant_guid <
local_participant_guid, the operation shall return
VALIDATION_PENDING_HANDSHAKE_MESSAGE."

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`
(l. 447)

**Tests:** `crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`

**Status:** done — decided in the stack, not in the plugin (see
§9.3.2.11.3).

### §10.3.3 Table 59 — validate_remote_identity and begin_handshake_*: AuthRequestMessageToken

**Spec:** §10.3.3, Table 59, p. 235–236 — "If the remote_auth_request_token
is NIL, the operation shall generate a local_auth_request_token [...] the
operation shall use the value of property named 'future_challenge' found in
the local_auth_request_token to fill the property named 'challenge1' [...]
the operation shall verify that the property named 'future_challenge' found
in that token is the same value as the property named 'challenge1'".

**Repo:** `crates/security-pki/src/plugin.rs::validate_remote_identity`, `begin_handshake_request`, `begin_handshake_reply`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_auth_request_announces_the_challenge1_of_the_request`,
`crates/security-pki/src/plugin.rs::tests::a_request_whose_challenge1_is_not_the_announced_one_is_refused`,
`crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/discovery/src/security/stack.rs::tests::a_reply_whose_challenge2_is_not_the_announced_one_is_refused`

**Status:** `done` — without a `remote_auth_request_token`, `validate_remote_identity` makes the own one, whose `future_challenge` becomes `challenge1` (initiator) or `challenge2` (replier); with one, `begin_handshake_reply` (and `process_handshake` at the initiator) checks that it equals `challenge1` (`challenge2`).

### §10.3.3 Table 59 — begin_handshake_request

**Spec:** §10.3.3, Table 59, p. 235 — "The operation shall return the
handshake_message containing a HandshakeRequestMessageToken object with
contents as defined in 10.3.2.5.1 [...] The operation shall return
VALIDATION_PENDING_HANDSHAKE_MESSAGE."

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_request`
(l. 828)

**Tests:** `crates/security-pki/src/plugin.rs::tests::request_token_has_spec_class_id_and_properties`

**Status:** done — content see Table 56.

### §10.3.3 Table 59 — begin_handshake_reply: check of c.id

**Spec:** §10.3.3, Table 59, p. 236 — "The operation shall verify the
validity of the IdentityCredential contained in the property named 'c.id'
found in the handshake_message_in HandshakeRequestMessageToken. This
verification shall be done using the locally configured CA in the same
manner as the validate_local_identity operation."

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_reply`
(l. 900, `verify_remote_der_checked`); `crates/security-pki/src/identity.rs::verify_remote_der_checked`
(l. 217)

**Tests:** `crates/security-pki/src/plugin.rs::tests::wrong_ca_initiator_rejected_by_replier`,
`crates/security-pki/src/plugin.rs::tests::cross_algorithm_dsign_mismatch_rejected`

**Status:** done — CRL and OCSP see above (`open`).

### §10.3.3 Table 59 — begin_handshake_reply: GUID in c.pdata against SubjectName

**Spec:** §10.3.3, Table 59, p. 237 — "The operation shall verify that the
first bit of the participant_guid of the ParticipantBuiltinTopic data inside
the 'c.pdata' is set to 1 and that the following 47 bits match the first 47
bits of the SHA-256 hash of the SubjectName appearing in the
IdentityCredential. If this verification fails, the operation shall fail and
return ValidationResult_Fail."

**Repo:** `crates/security-pki/src/plugin.rs` (`begin_handshake_reply`,
`check_pdata`, `pdata_participant_guid`),
`crates/security-pki/src/guid_adjust.rs::identity_prefix_head`

**Tests:**
`crates/security-pki/src/plugin.rs::tests::a_request_with_a_pdata_guid_not_bound_to_its_certificate_is_refused`,
`crates/security-pki/src/plugin.rs::tests::the_pdata_guid_is_read_from_the_parameter_list`

**Status:** `done` — bit 0 and the following 47 bits are checked, as Cyclone
(`validate_pdata`) and Fast DDS do; `c.pdata` is read as a big-endian parameter
list from the first octet.

### §10.3.3 Table 59 — begin_handshake_reply: result and stored state

**Spec:** §10.3.3, Table 59, p. 237 — "The operation shall fill the
handshake_message_out with a HandshakeReplyMessageToken object [...] The
operation shall fill the handshake_handle with an implementation-dependent
reference [...] The operation shall return
VALIDATION_PENDING_HANDSHAKE_MESSAGE."

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_reply`, `process_final_on_replier`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`

**Status:** `done` — reply and state are right; the secret is available only after the verified final (VALIDATION_PENDING_HANDSHAKE_MESSAGE until then).

### §10.3.3 Table 59 — get_shared_secret only after VALIDATION_OK

**Spec:** §10.3.3, Table 59, p. 238 — "get_shared_secret This operation shall
be called with the handshake_handle that was previously used to call either
process_handshake and for which the aforementioned operation returned
VALIDATION_OK_FINAL_MESSAGE or VALIDATION_OK. If the above conditon is not
met, the operation shall return the exception
DDS_SecurityException_PreconditionError."

**Repo:** `crates/security-pki/src/plugin.rs` (`begin_handshake_reply`,
`process_final_on_replier`, `shared_secret`)

**Tests:**
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`

**Status:** `done` — the replier registers the secret for the handshake only
after the checks of the final; a failed final removes it.

### §10.3.3 Table 59 — process_handshake on the initiator

**Spec:** §10.3.3, Table 59, p. 237–238 — "The operation shall verify that the
contents of the handshake_message_in correspond to a
HandshakeReplyMessageToken [...] The operation shall check that the
challenge1 matches the one that was sent on the
HandshakeRequestMessageToken. The operation shall validate the digital
signature in the 'signature' property [...] On success the operation shall
return VALIDATION_OK_FINAL_MESSAGE."

**Repo:** `crates/security-pki/src/plugin.rs::process_reply_on_initiator`
(l. 1077)

**Tests:** `crates/security-pki/src/plugin.rs::tests::signature_tamper_rejected_by_initiator`,
`crates/security-pki/src/plugin.rs::tests::cert_bind_replier_modifies_initiator_cert_in_reply_rejected`

**Status:** done — OCSP see above (`open`).

### §10.3.3 Table 59 — process_handshake on the replier

**Spec:** §10.3.3, Table 59, p. 238 — "The operation shall check that the
challenge1 and challenge2 match the ones that were sent on the
HandshakeReplyMessageToken. The operation shall validate the digital
signature in the 'signature' property [...] On success the operation shall
return VALIDATION_OK."

**Repo:** `crates/security-pki/src/plugin.rs::process_final_on_replier`
(l. 1219)

**Tests:** `crates/security-pki/src/plugin.rs::tests::final_token_challenge2_tamper_rejected`,
`crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`

**Status:** done — the check itself is present; its result comes too late
(see "get_shared_secret only after VALIDATION_OK").

### §10.3.3 Table 59 — refuse a repeated request

**Spec:** §10.3.3, Table 59, p. 237 — "begin_handshake_reply [...] The
operation shall fill the handshake_handle with an implementation-dependent
reference that allows the implementation to retrieve at least the following
information [...] The value attribute of the challenge_message returned".

**Repo:** `crates/security-pki/src/plugin.rs::record_challenge` (l. 432)

**Tests:** `crates/security-pki/src/plugin.rs::tests::replay_initiator_request_rejected_second_time`,
`crates/security-pki/src/plugin.rs::tests::replay_cache_evicts_oldest_at_cap_plus_one`

**Status:** done — ZeroDDS refuses a `challenge1` already seen; the spec
does not require this explicitly.

### §10.3.3 Table 59 — get_authenticated_peer_credential_token

**Spec:** §10.3.3, Table 59, p. 239 — "The operation shall return the
AuthenticatedPeerCredentialToken of the peer DomainParticipant associated
with the handshake_handle."

**Repo:** `crates/security-pki/src/plugin.rs::get_authenticated_peer_credential_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`

**Status:** `done` — see Table 54.

### §10.3.3 Table 59 — set_listener and return_*

**Spec:** §10.3.3, Table 59, p. 239 — "set_listener This operation shall save
a reference to the listener object [...] return_identity_token This operation
shall behave as specified in 9.3.2.11.14." (likewise for the other
return_*)

**Repo:** `crates/security-pki/src/plugin.rs::set_listener`, `return_identity_token`, `return_identity_status_token`, `return_authenticated_peer_credential_token`, `return_handshake_handle`, `return_identity_handle`, `return_sharedsecret_handle`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/plugin.rs::tests::revoking_an_identity_tells_the_listener_and_invalidates_the_handle`,
`crates/security-pki/src/plugin.rs::tests::a_returned_identity_handle_is_disposed_of`

**Status:** `done` — all seven behave as §9.3.2.11.13 to §9.3.2.11.19 say.

### §10.3.4 The participant with the lower GUID begins

**Spec:** §10.3.4, p. 239 — "To determine which of the two DomainParticipant
entities shall initiate the protocol, each DomainParticipant compares its own
GUID with that of the other DomainParticipant. The DomainParticipant with the
lower GUID (using lexicographical order) initiates the protocol."

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`
(l. 447)

**Tests:** `crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`

**Status:** done

### §10.3.4.1 Table 60 and Table 61 — terms and notation

**Spec:** §10.3.4.1, Tables 60–61, p. 240–241 — Participant1, Participant2,
PubK, PrivK, Cert, Perm, Pdata, Dsign_algo, Kagree_algo, Challenge, DH,
DHSharedSecret, SharedSecret, C1, C2; Sign(), Hash(), concatenation.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — terms; the rules are in Table 56 to 59.

### §10.3.4.2 Table 62 — three-message protocol sequence

**Spec:** §10.3.4.2, Table 62, p. 241–242 — request (C1, Hash(C1),
Challenge1, DH1); reply with check of Cert1 and Hash(C1) and
Sign(Hash(C2) | Challenge2 | DH2 | Challenge1 | DH1 | Hash(C1)); final with
check of Cert2 and the signature and Sign(Hash(C1) | Challenge1 | DH1 |
Challenge2 | DH2 | Hash(C2)); both sides compute the shared secret.

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_request`,
`begin_handshake_reply`, `process_reply_on_initiator`,
`process_final_on_replier`

**Tests:**
`crates/security-pki/tests/e2e_handshake.rs::full_three_stage_handshake`,
`crates/security-pki/tests/e2e_handshake.rs::tampered_request_token_fails_hash_check`,
`crates/security-pki/tests/e2e_handshake.rs::replay_attack_with_swapped_challenges_detected`,
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`

**Status:** `done` — the three messages with their checks; the secret is
available to the replier only after the verified final.

---

### §10.4 One instance of DDS:Access:Permissions per DomainParticipant

**Spec:** §10.4, p. 242 — "The DDS:Access:Permissions implements the
AccessControl plugin API using a permissions document signed by a shared
Certificate Authority (CA). [...] Each DomainParticipant has an associated
instance of the DDS:Access:Permissions plugin."

**Repo:** `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`
(l. 301, governance and permissions per profile)

**Tests:** `crates/dcps/tests/security_guid_wiring_e2e.rs::secured_participants_on_public_rust_api_use_identity_adjusted_guids_and_discover`

**Status:** done — the plugin interface is not used (see §9.4.2).

### §10.4.1 Three documents through the PropertyQosPolicy

**Spec:** §10.4.1, p. 242 — "The DDS:Access:Permissions plugin is configured
with three documents: 1. The Permissions CA certificate 2. The Domain
governance signed by the Permissions CA 3. The DomainParticipant permissions
signed by the Permissions CA. The configuration of the builtin access control
plugin shall be done using the PropertyQosPolicy of the
DomainParticipantQos."

**Repo:** `crates/security-runtime/src/qos_properties.rs::profile_config_from_properties`
(l. 380), constants `PROP_PERMISSIONS_CA`, `PROP_GOVERNANCE`,
`PROP_PERMISSIONS` (l. 91–95)

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_property_names_are_the_spec_strings`,
`crates/security-runtime/src/qos_properties.rs::tests::the_auth_and_access_namespaces_request_security_and_the_log_one_does_not`

**Status:** done

### §10.4.1 Table 63 — permissions_ca

**Spec:** §10.4.1, Table 63, p. 243 — "permissions_ca URI to a X509
certificate for the PermissionsCA in PEM format. Supported URI schemes: file,
data, pkcs11".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`
(l. 302)

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::every_file_uri_form_resolves_to_a_path`,
`crates/security-runtime/src/qos_properties.rs::tests::a_data_uri_is_the_content_and_not_a_path`

**Status:** done — `pkcs11:` as for Table 50 only with the feature `pkcs11`.

### §10.4.1 Table 63 — governance

**Spec:** §10.4.1, Table 63, p. 243–244 — "governance URI to the shared
Governance Document signed by the Permissions CA in S/MIME format URI
schemes: file, data".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`
(l. 302); `crates/security-permissions/src/cms.rs::CmsPkcs7Verifier`
(l. 117)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::happy_path_multipart_signed_passes`,
`crates/dcps/tests/security_qos_properties.rs::a_complete_dds_sec_property_list_creates_a_secured_participant`

**Status:** done

### §10.4.1 Table 63 — permissions

**Spec:** §10.4.1, Table 63, p. 244 — "permissions URI to the
DomainParticipant permissions document signed by the Permissions CA in
S/MIME format URI schemes: file, data".

**Repo:** `crates/security-permissions/src/signature.rs::open_signed_permissions`
(l. 89)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::happy_path_then_open_signed_permissions_parses`

**Status:** done

### §10.4.1.1 Permissions CA Certificate

**Spec:** §10.4.1.1, p. 244 — "The certificate can be self-signed or signed by
some other CA. Regardless of this the Public Key in the Certificate shall be
trusted to sign the aforementioned Governance and Permissions documents".

**Repo:** `crates/security-permissions/src/cms.rs::CmsPkcs7Verifier`
(l. 117)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::intermediate_ca_chain_is_accepted`,
`crates/security-permissions/tests/cms_verify.rs::wrong_ca_signature_is_rejected`

**Status:** done

### §10.4.1.2 Governance signed in S/MIME 3.2 format

**Spec:** §10.4.1.2, p. 244 — "The domain governance document shall be signed
by the Permissions CA. The signed document shall use S/MIME version 3.2
format as defined in IETF RFC 5761 using SignedData Content Type [...]
formatted as multipart/signed [...] Additionally the signer certificate shall
be included within the signature."

**Repo:** `crates/security-permissions/src/cms.rs::CmsPkcs7Verifier`
(l. 117); `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`
(l. 301)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::happy_path_multipart_signed_passes`,
`crates/security-permissions/tests/cms_verify.rs::tampered_xml_is_rejected`,
`crates/security-permissions/tests/cms_verify.rs::expired_signer_is_rejected`

**Status:** done — in addition `application/pkcs7-mime; smime-type=signed-data`
and PKCS#7 in PEM are accepted. The module documentation of
`crates/security-permissions/src/governance.rs` (l. 24) lists the signature
check as not implemented; the check sits in
`crates/security-runtime/src/profile.rs`.

### §10.4.1.2 Content of the governance

**Spec:** §10.4.1.2, p. 245 — "The governance document specifies which DDS
domain IDs shall be protected and the details of the protection." (list of
the aspects per domain and per topic)

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — overview; the elements are listed on their
own below.

### §10.4.1.2.1 Basic Protection Kinds: NONE, SIGN, ENCRYPT

**Spec:** §10.4.1.2.1, p. 245–246 — "SIGN indicates the cryptographic
transformation shall be purely a message authentication code (MAC) [...]
transformation_algorithm_id set to the CryptoAlgorithmId that corresponds to
the selected MAC algorithm (e.g. CID_AES256_GMAC) [...] ENCRYPT indicates the
cryptographic transformation shall be an encryption followed by a message
authentication code (MAC) computed on the ciphertext".

**Repo:** `crates/security-runtime/src/gate_crypto.rs::register_endpoint`; `crates/security-crypto/src/plugin/spec.rs`

**Tests:** `crates/security-runtime/src/shared.rs::tests::the_basic_protection_kind_of_an_endpoint_decides_gmac_or_gcm`, `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** `done` — SIGN gives GMAC per endpoint (AES256_GMAC 0x03, AES128_GMAC 0x01), ENCRYPT GCM (0x04, 0x02); the width follows `symmetric_cipher_algorithm`. run `cargo test -p zerodds-security-runtime --lib` and `-p zerodds-dcps --features security --lib`: green, logs `/root/zerodds-rc7/agent-logs/sec-crypto-1018-085015.log`, `…085119.log`.

### §10.4.1.2.1 Algorithm of the endpoints per topic

**Spec:** §10.4.1.2.6.6, §10.4.1.2.6.7, p. 260–261 — "The setting of this
element shall specify the protection kind (see 10.4.1.2.2) used for the RTPS
SubMessages sent by any DataWriter and DataReader whose associated Topic name
matches the rule's topic expression."

**Repo:** `crates/security-runtime/src/shared.rs::register_local_endpoint_with_kinds`; `crates/security-crypto/src/plugin.rs::register_local_endpoint`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_key_material_of_an_endpoint_follows_its_protection_kinds`, `crates/security-runtime/src/shared.rs::tests::the_protection_kinds_of_an_endpoint_come_from_its_topic_rule`, `crates/dcps/src/runtime.rs::tests::a_writer_gets_the_key_material_of_its_topic_rule`

**Status:** `done` — an application endpoint gets its key material at creation for the kinds of the rule of its topic, a secure builtin one for the discovery or liveliness kind of the domain.

### §10.4.1.2.2 Protection Kinds with Origin Authentication

**Spec:** §10.4.1.2.2, p. 246 — "SIGN_WITH_ORIGIN_AUTHENTICATION [...] In
addition, the cryptographic transformation shall create additional
authentication codes, each produced with a different secret key. [...]
ENCRYPT_WITH_ORIGIN_AUTHENTICATION indicates the cryptographic transformation
shall be an encryption followed by a message authentication code (MAC)
computed on the ciphertext, followed by additional authentication codes."

**Repo:** `crates/security-permissions/src/governance.rs::ProtectionKind`; `crates/security-runtime/src/shared.rs::metadata_origin_authenticated`, `discovery_origin_authenticated`, `liveliness_origin_authenticated`, `rtps_origin_authenticated`; `crates/security-runtime/src/policy.rs::from_protection_kind` (for the level SIGN/ENCRYPT)

**Tests:** `crates/security-runtime/src/shared.rs::tests::origin_authentication_follows_the_governance_kinds`, `crates/security-crypto/src/plugin.rs::tests::a_reader_checks_the_origin_of_a_submessage_by_its_own_key`

**Status:** `done` — the four kinds are read; `ProtectionLevel` still folds them into SIGN and ENCRYPT, the origin authentication comes from the four queries of the gate and goes into the keys (`register_local_endpoint` with the kind as a property, `register_local_participant` with `zerodds.crypto.rtps_origin_authentication`).

### §10.4.1.2.2 Additional MACs for origin authentication

**Spec:** §10.4.1.2.2, p. 246 — "Each of these additional secret keys shall be
shared only with a subset of the receivers. In the limit case each secret key
is shared with only one receiver. The additional MACs prove to the receiver
that the sender originated the message".

**Repo:** `crates/security-crypto/src/plugin.rs::footer_tail`, `receiver_key_for`, `create_endpoint_crypto_token_for`, `create_local_participant_crypto_tokens`; `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`, `prepare_crypto_token`

**Tests:** `crates/security-runtime/src/shared.rs::tests::origin_authentication_follows_the_governance_kinds`, `crates/security-crypto/src/plugin.rs::tests::a_reader_checks_the_origin_of_a_submessage_by_its_own_key`, `crates/security-crypto/src/plugin.rs::tests::the_receiver_specific_mac_is_the_one_of_the_spec`

**Status:** `done` — each remote endpoint (participant) gets a key of its own, in the token to it; the footers of the submessages (RTPS messages) carry the MAC for each. The payload stays without MACs (Table 80). Measured: nothing; the matrix on the Linux host has no profile with `*_WITH_ORIGIN_AUTHENTICATION`. The functions `encrypt_submessage_multi`, `transform_outbound_group` and their relatives (HMAC-SHA256 over the ciphertext, no mechanism of the specification) are removed.

### §10.4.1.2.3 Governance XSD: structure dds, domain_access_rules, domain_rule

**Spec:** §10.4.1.2.3, §10.4.1.2.4, p. 246–252 — "<xs:element name='dds'
type='DomainAccessRulesNode' /> [...] domain_access_rules [...] domain_rule
[...] The XML domain governance document is delimited by the <dds> XML
element tag and contains a single domain access rules Section".

**Repo:** `crates/security-permissions/src/governance.rs::parse_governance_xml`, `parse_dds_root`, `parse_domain_rule`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_structure_of_the_document_is_the_xsds`,
`crates/security-permissions/src/governance.rs::tests::a_governance_in_the_omg_namespace_parses`,
`crates/security-permissions/src/governance.rs::tests::a_rule_without_a_domain_is_refused`,
`crates/security-permissions/src/governance.rs::tests::missing_domain_rule_flags_are_true`

**Status:** `done` — the root `<dds>` holds exactly one `<domain_access_rules>`; elements outside the XSD follow `must_interpret`; the namespace of the OMG sample files is accepted; a missing `allow_unauthenticated_participants` or `enable_join_access_control` counts as TRUE (Table 70).

### §10.4.1.2.3 XSD: enable_key_revision, rtps_psk_protection_kind, allowed_crypto_algorithms

**Spec:** §10.4.1.2.3, p. 247 — "<xs:element name='enable_key_revision'
type='xs:boolean' /> [...] <xs:element name='rtps_psk_protection_kind'
type='BasicProtectionKind' /> [...] <xs:element
name='allowed_crypto_algorithms' type='AllowedCryptoAlgorithms'
minOccurs='0'/>".

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_rule`, `AllowedCryptoAlgorithms`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_key_revision_psk_and_allowed_algorithm_elements_are_read`,
`crates/security-permissions/src/governance.rs::tests::the_allowed_algorithm_names_are_those_of_the_xsd`

**Status:** `done` — the three elements are read, checked and evaluated in the plugin (`is_key_revision_enabled`, `is_rtps_psk_protected`, `algorithm_info`).

### §10.4.1.2.3 XSD: values of ProtectionKind and BasicProtectionKind

**Spec:** §10.4.1.2.3, p. 249 — "ProtectionKind [...]
ENCRYPT_WITH_ORIGIN_AUTHENTICATION, SIGN_WITH_ORIGIN_AUTHENTICATION, ENCRYPT,
SIGN, NONE [...] BasicProtectionKind [...] ENCRYPT, SIGN, NONE".

**Repo:** `crates/security-permissions/src/governance.rs` (`ProtectionKind::parse_xsd`, `basic_of`)

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_protection_kind_enumerations_of_the_xsd_are_exact`,
`crates/security-permissions/src/governance.rs::tests::protection_kind_parses_case_insensitive`

**Status:** `done` — a value outside the five makes the document invalid; basic elements (`data_protection_kind`, `rtps_psk_protection_kind`) accept NONE, SIGN, ENCRYPT only; the values are exactly those of the XSD in their spelling.

### §10.4.1.2.3 XSD: values of xs:boolean

**Spec:** §10.4.1.2.3, p. 247, 249 — "type='xs:boolean'" for
`allow_unauthenticated_participants`, `enable_join_access_control`,
`enable_*_protection`, `enable_*_access_control`.

**Repo:** `crates/security-permissions/src/governance.rs::parse_bool`

**Tests:**
`crates/security-permissions/src/governance.rs::tests::a_governance_outside_the_xsd_is_refused`

**Status:** `done` — `true`, `false`, `1`, `0` (also upper-case); any other text
makes the document invalid.

### §10.4.1.2.5 Domain Rules: order, first matching rule

**Spec:** §10.4.1.2.5, p. 252 — "The domain rules shall be evaluated in the
same order as they appear in the document. A rule only applies to a
particular DomainParticipant if the domain Section matches the DDS domain_id
to which the DomainParticipant belongs. If multiple rules match, the first
rule that matches is the only one that applies."

**Repo:** `crates/security-permissions/src/governance.rs::find_domain_rule`
(l. 322)

**Tests:** `crates/security-permissions/src/governance.rs::tests::domain_filter_id_range_matches_inclusive`,
`crates/security-permissions/src/governance.rs::tests::an_uncovered_domain_creates_nothing`

**Status:** done

### §10.4.1.2.5.1 Domains element: id and id_range

**Spec:** §10.4.1.2.5.1, p. 252–253 — "DDS domains are identified by their
DomainId (an integer) and a DomainTag (a string). One or more DomainId values
(or ranges) must always be specified."

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_filter`

**Tests:**
`crates/security-permissions/src/governance.rs::tests::parses_domain_rule_with_ranges`,
`crates/security-permissions/src/governance.rs::tests::domain_filter_id_range_matches_inclusive`,
`crates/security-permissions/src/governance.rs::tests::a_rule_without_a_domain_is_refused`

**Status:** `done` — `<id>` and `<id_range>` as non-negative integers; an
unreadable value, a `<domains>` without a domain and a rule without `<domains>`
make the document invalid.

### §10.4.1.2.5.1 Domains element: an empty set applies to all domains

**Spec:** §10.4.1.2.5.1, p. 252 — "One or more DomainId values (or ranges)
must always be specified."

**Repo:** `crates/security-permissions/src/governance.rs`
(`parse_domain_filter`, `parse_domain_rule`)

**Tests:**
`crates/security-permissions/src/governance.rs::tests::a_rule_without_a_domain_is_refused`

**Status:** `done` — an empty or missing set is refused instead of read as every
domain.

### §10.4.1.2.5.1 Domains element: tag and tag_expression

**Spec:** §10.4.1.2.5.1, §10.4.1.2.7, p. 252, 261 — "In addition, one or more
DomainTag values or DomainTagExpression values may be optionally specified.
If no DomainTag and DomainTagExpression is specified, then it shall be
treated as if the empty string ('') was specified as the only DomainTag
value. [...] For the DomainParticipant Domain to be matched, both the
DomainParticipant's DomainId and the DomainParticipant's DomainTag must
match".

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_filter`,
`DomainFilter::matches`; `crates/security-runtime/src/shared.rs::set_domain_tag`

**Tests:** `crates/security-permissions/src/governance.rs::tests::domain_rules_match_the_domain_tag`

**Status:** `done` — `<tag>` and `<tag_expression>` apply to the DomainTag of
the participant (`fnmatch()` for the expression); a rule with neither
applies to the empty tag.

### §10.4.1.2.5.2 Allow Unauthenticated Participants element

**Spec:** §10.4.1.2.5.2, p. 253–254 — "If the value is set to FALSE, the
ParticipantSecurityConfig returned by the get_participant_security_config
operation on the AccessControl shall have the
allow_unauthenticated_participants member set to FALSE."

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_rule`
(l. 876); `crates/security-runtime/src/shared.rs::allow_unauthenticated`
(l. 1314)

**Tests:** `crates/security-runtime/src/shared.rs::tests::allow_unauthenticated_reads_true_when_set`,
`crates/security-runtime/src/shared.rs::tests::allow_unauthenticated_default_false_without_element`

**Status:** done — for the effect of the value see §9.4.2.4 and §9.8.3.

### §10.4.1.2.5.3 Enable Join Access Control element

**Spec:** §10.4.1.2.5.3, p. 254 — "If the value is set to TRUE, the
ParticipantSecurityConfig returned by the get_participant_security_config
operation on the AccessControl shall have the is_access_protected member set
to TRUE."

**Repo:** `crates/security-permissions/src/governance.rs::DomainRule`
(l. 154, `enable_join_access_control`)

**Tests:** `crates/security-permissions/src/governance.rs::tests::check_create_participant_consults_permissions`

**Status:** done — the effect for remote participants is missing (see
§9.4.2.4 Table 33 — is_access_protected, `open`).

### §10.4.1.2.5.4 Enable Key Revision element

**Spec:** §10.4.1.2.5.4, p. 254 — "If the value is set to TRUE, the
ParticipantSecurityConfig returned by the get_participant_security_config
operation on the AccessControl shall have the is_key_revision_enabled member
set to TRUE."

**Repo:** `crates/security-permissions/src/governance.rs::DomainRule::enable_key_revision`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_key_revision_psk_and_allowed_algorithm_elements_are_read`,
`crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — `is_key_revision_enabled` of the config follows the element.

### §10.4.1.2.5.5 Discovery Protection Kind element

**Spec:** §10.4.1.2.5.5, p. 254–255 — "The discovery protection element
specifies the protection kind (see 10.4.1.2.2) used for the secure builtin
DataWriter and DataReader entities used for discovery [...] The attribute
is_discovery_encrypted in the PluginParticipantSecurityAttributes [...] shall
be set to TRUE if the value specified in the <discovery_protection_kind> is
ENCRYPT or ENCRYPT_WITH_ORIGIN_AUTHENTICATION [...] is_discovery_origin_authenticated
[...] shall be set to TRUE if the value [...] is
SIGN_WITH_ORIGIN_AUTHENTICATION or ENCRYPT_WITH_ORIGIN_AUTHENTICATION".

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config` (`plugin_participant_attributes`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — the `*_encrypted` and `*_origin_authenticated` attributes of the config follow the value of the element; the runtime announces them.

### §10.4.1.2.5.6 Liveliness Protection Kind element

**Spec:** §10.4.1.2.5.6, p. 255 — "The liveliness protection element
specifies the protection kind (see 10.4.1.2.2) used for builtin DataWriter
and DataReader associated with the ParticipantMessageSecure builtin Topic".

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config` (`plugin_participant_attributes`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — the `*_encrypted` and `*_origin_authenticated` attributes of the config follow the value of the element; the runtime announces them.

### §10.4.1.2.5.7 RTPS Protection Kind element

**Spec:** §10.4.1.2.5.7, p. 255–256 — "The RTPS protection kind element
specifies the protection kind (see 10.4.1.2.2) used for the whole RTPS
message. [...] The attribute is_rtps_axk_protected [...] shall be set to
FALSE if the value specified in the <rtps_protection_kind> element is NONE
and to TRUE otherwise."

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config` (`plugin_participant_attributes`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — the `*_encrypted` and `*_origin_authenticated` attributes of the config follow the value of the element; the runtime announces them.

### §10.4.1.2.5.5 to §10.4.1.2.5.7 Bits for origin authentication

**Spec:** §10.4.1.2.5.5–§10.4.1.2.5.7, p. 255–256 — `is_discovery_origin_authenticated`,
`is_liveliness_origin_authenticated`, `is_rtps_origin_authenticated` "shall
be set to TRUE if the value specified [...] is SIGN_WITH_ORIGIN_AUTHENTICATION
or ENCRYPT_WITH_ORIGIN_AUTHENTICATION".

**Repo:** `crates/dcps/src/runtime.rs` (mask in `enable_security_builtins_with_auth`); `crates/security-runtime/src/shared.rs::metadata_origin_authenticated`, `discovery_origin_authenticated`, `liveliness_origin_authenticated`, `rtps_origin_authenticated`

**Tests:** `crates/security-runtime/src/shared.rs::tests::origin_authentication_follows_the_governance_kinds` (the queries); the bits themselves: no test

**Status:** `done` — `IS_RTPS_ORIGIN_AUTHENTICATED` (0x8), `IS_DISCOVERY_ORIGIN_AUTHENTICATED` (0x10), `IS_LIVELINESS_ORIGIN_AUTHENTICATED` (0x20) in the plugin attributes of the participant follow the kinds.

### §10.4.1.2.5.8 RTPS PSK Protection Kind element

**Spec:** §10.4.1.2.5.8, p. 256 — "The RTPS protection kind element specifies
the protection kind (see 10.4.1.2.2) used for RTPS Messages that are not
otherwise protected by an 'Authenticated Participant Exchanged Key'. This
includes the RTPS Bootstrapping Messages".

**Repo:** `crates/security-permissions/src/governance.rs::DomainRule::rtps_psk_protection_kind`; `crates/security-permissions/src/plugin.rs::get_participant_security_config`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_key_revision_psk_and_allowed_algorithm_elements_are_read`,
`crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — the element is read; `is_rtps_psk_protected` and `is_rtps_psk_encrypted` follow it.

### §10.4.1.2.5.9 Allowed Algorithms Section

**Spec:** §10.4.1.2.5.9, p. 256–258 — "The Allowed Algorithms section defines
rules that control the cryptographic algorithms that may be used in the
domain." Elements `digital_signature`, `digital_signature_trust_chain`,
`key_establishment`, `symmetric_cipher`; if an element is absent, all
algorithms of §8 are allowed.

**Repo:** `crates/security-permissions/src/governance.rs::AllowedCryptoAlgorithms`, `parse_allowed_crypto_algorithms`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_allowed_algorithm_names_are_those_of_the_xsd`,
`crates/security-permissions/src/plugin.rs::tests::without_allowed_algorithms_every_algorithm_is_supported`,
`crates/security-permissions/src/plugin.rs::tests::the_identity_certificates_algorithms_must_be_allowed`,
`crates/dcps/src/runtime.rs::tests::the_allowed_algorithms_cut_the_announced_ones`

**Status:** `done` — the section is read; a missing element allows all algorithms; `algorithm_info` and the check of the identity certificate follow it.

### §10.4.1.2.5.10 Topic Access Rules Section

**Spec:** §10.4.1.2.5.10, p. 258 — "This section is delimited by the XML
element <topic_access_rules> and contains a sequence of topic rule elements."

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_rule`
(l. 897)

**Tests:** `crates/security-permissions/src/governance.rs::tests::topic_rule_matches_exact_topic_first`

**Status:** done

### §10.4.1.2.6 Topic Rules: order, first matching rule

**Spec:** §10.4.1.2.6, p. 258 — "The topic access rules shall be evaluated in
the same order as they appear within the <topic_access_rules> Section. If
multiple rules match the first rule that matches is the only one that
applies."

**Repo:** `crates/security-permissions/src/governance.rs::find_topic_rule`
(l. 332)

**Tests:** `crates/security-permissions/src/governance.rs::tests::topic_rule_matches_exact_topic_first`,
`crates/security-permissions/src/governance.rs::tests::topic_rule_falls_through_to_wildcard`,
`crates/security-permissions/src/governance.rs::tests::access_control_flags_come_from_the_matching_rule`

**Status:** done

### §10.4.1.2.6.1 Topic expression: syntax of fnmatch()

**Spec:** §10.4.1.2.6.1, p. 258 — "The Topic name expression syntax and
matching shall use the syntax and rules of the POSIX fnmatch() function as
specified in POSIX 1003.2-1992, Section B.6 [38]."

**Repo:** `crates/security-permissions/src/topic_match.rs::fnmatch`, `::topic_match`

**Tests:** `crates/security-permissions/src/topic_match.rs::tests::star_matches_all`,
`crates/security-permissions/src/topic_match.rs::tests::question_mark_exact_one`,
`crates/security-permissions/src/topic_match.rs::tests::combined_wildcards`,
`crates/security-permissions/src/topic_match.rs::tests::sets_ranges_and_negation_match_one_character`,
`crates/security-permissions/src/topic_match.rs::tests::a_backslash_quotes_unless_noescape`

**Status:** `done` — POSIX `fnmatch()`: `*`, `?`, bracket expressions with sets, ranges
and negation, the backslash as escape. The same function checks topics,
partitions, data tags and domain tags in governance and permissions.

### §10.4.1.2.6.1 fnmatch(): bracket expressions and escape character

**Spec:** §10.4.1.2.6.1, §10.4.1.5.3.4.1.3, §10.4.1.5.3.4.1.4, p. 258, 258,
258 — "shall use the syntax and rules of the POSIX fnmatch() function".

**Repo:** `crates/security-permissions/src/topic_match.rs::fnmatch`

**Tests:** `crates/security-permissions/src/topic_match.rs::tests::sets_ranges_and_negation_match_one_character`,
`crates/security-permissions/src/topic_match.rs::tests::a_backslash_quotes_unless_noescape`,
`crates/security-permissions/src/topic_match.rs::tests::pathname_keeps_slashes_out_of_wildcards`

**Status:** `done` — bracket expressions (`[abc]`, `[a-z]`, `[!a-z]`) and the backslash
as escape, `FNM_NOESCAPE` and `FNM_PATHNAME` for the subject expressions.

### §10.4.1.2.6.2 Enable Discovery protection element

**Spec:** §10.4.1.2.6.2, p. 258–259 — "Specifically the is_discovery_protected
attribute in the TopicSecurityConfig shall be set to the boolean value
specified in the <enable_discovery_protection> element."

**Repo:** `crates/security-runtime/src/shared.rs::topic_discovery_protected`
(l. 559)

**Tests:** `crates/security-runtime/src/shared.rs::tests::topic_discovery_protected_reads_topic_rule_flag`

**Status:** done — the choice of the SEDP endpoints does not follow the value
(see §9.4.2.6 Table 36 — is_discovery_protected per topic, `open`).

### §10.4.1.2.6.3 Enable Liveliness Protection element

**Spec:** §10.4.1.2.6.3, p. 259 — "Specifically the is_liveliness_protected
attribute in the TopicSecurityConfig shall be set to the boolean value
specified in the <enable_liveliness_protection> element."

**Repo:** `crates/security-permissions/src/plugin.rs::get_topic_security_config`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`

**Status:** `done` — `is_liveliness_protected` of the `TopicSecurityConfig` follows the element.

### §10.4.1.2.6.4 Enable Read Access Control element

**Spec:** §10.4.1.2.6.4, p. 259 — "In addition, this element shall control
the AccessControl::check_create_datareader operation on any DataReader entity
whose associated Topic name matches the rule's topic expression."

**Repo:** `crates/security-permissions/src/governance.rs::read_access_controlled`
(l. 414), `check_create_datareader` (l. 460)

**Tests:** `crates/security-permissions/src/governance.rs::tests::an_uncontrolled_topic_needs_no_grant`,
`crates/security-permissions/src/governance.rs::tests::the_six_omg_suite_refusals_are_denied`

**Status:** done — the spec names `enable_write_access_control` in the two
bullets; the code takes `enable_read_access_control` (contradiction in the specification).

### §10.4.1.2.6.5 Enable Write Access Control element

**Spec:** §10.4.1.2.6.5, p. 259–260 — "If the value of
<enable_write_access_control> element is FALSE, the operation
check_create_datawriter shall return TRUE without further checking the
Permissions document."

**Repo:** `crates/security-permissions/src/governance.rs::write_access_controlled`
(l. 405), `check_create_datawriter` (l. 439)

**Tests:** `crates/security-permissions/src/governance.rs::tests::an_uncontrolled_topic_needs_no_grant`,
`crates/security-permissions/src/governance.rs::tests::the_granted_direction_still_creates`

**Status:** done

### §10.4.1.2.6.6 Metadata Protection Kind element

**Spec:** §10.4.1.2.6.6, p. 260 — "The attribute is_submessage_protected in
the EndpointSecurityConfig shall be set to FALSE if the value specified in
the <metadata_protection_kind> is NONE and shall be set to TRUE otherwise.
[...] is_submessage_encrypted [...] is_submessage_origin_authenticated".

**Repo:** `crates/security-runtime/src/shared.rs::metadata_protection`, `metadata_origin_authenticated`; `crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info`

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`, `crates/security-runtime/src/plugin_attributes.rs::tests::key_protected_follows_data_encrypt_per_spec_10_4_1_2_6` (the call with the new parameter `false`)

**Status:** `done` — `is_submessage_protected`, `is_submessage_encrypted` and `is_submessage_origin_authenticated` (plugin bit 0x4) follow the value. The bit 0x4 is not tested.

### §10.4.1.2.6.7 Data Protection Kind element

**Spec:** §10.4.1.2.6.7, p. 260–261 — "If the value specified in the
<data_protection_kind> element is NONE, then is_payload_protected,
is_key_protected and is_payload_encrypted shall be set to FALSE. [...] SIGN,
then is_payload_protected shall be set to TRUE. The attributes
is_key_protected and is_payload_encrypted shall be set to FALSE. [...]
ENCRYPT, then is_payload_protected, is_key_protected, and
is_payload_encrypted shall be set to TRUE."

**Repo:** `crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info` (l. 2087)

**Tests:** `crates/security-runtime/src/plugin_attributes.rs::tests::key_protected_follows_data_encrypt_per_spec_10_4_1_2_6`,
`crates/security-runtime/src/plugin_attributes.rs::tests::liveliness_protected_sets_0x40_per_spec_9_4_1_3`

**Status:** done

### §10.4.1.2.7 No matching domain rule: error

**Spec:** §10.4.1.2.7, p. 261 — "If no Domain Rule matches the
DomainParticipant domain_id the operation under consideration shall fail
with a suitable 'permissions error'."

**Repo:** `crates/security-permissions/src/governance.rs::check_create_participant`
(l. 364), `check_create_datawriter` (l. 439)

**Tests:** `crates/security-permissions/src/governance.rs::tests::an_uncovered_domain_creates_nothing`

**Status:** done

### §10.4.1.2.7 No matching topic rule: error

**Spec:** §10.4.1.2.7, p. 262 — "If no Topic Rule matches the Entity topic
name the operation under consideration shall fail with a suitable
'permissions error'."

**Repo:** `crates/security-permissions/src/governance.rs::has_topic_rule`;
`crates/security-runtime/src/shared.rs` (`check_create_datawriter`,
`check_create_datareader`, `check_remote_datawriter`, `check_remote_datareader`)

**Tests:** `crates/security-permissions/src/governance.rs::tests::a_topic_without_a_rule_is_refused`

**Status:** `done` — a topic without a matching rule gets no endpoint,
neither local nor remote; the builtin logging topic keeps its builtin
rule.

### §10.4.1.3 Governance Document Extensibility: must_interpret

**Spec:** §10.4.1.3, p. 262–263 — "If an XML element does not have the
must_interpret attribute it shall be treated as if it had
must_interpret='TRUE'. [...] If a governance extension element has the
must_interpret attribute set to 'true' or 'TRUE', then the governance document
shall be considered invalid, and an error shall be raised."

**Repo:** `crates/security-permissions/src/governance.rs::extension_element`, `crates/security-permissions/src/xml.rs::check_known_element`

**Tests:** `crates/security-permissions/src/governance.rs::tests::an_extension_attribute_follows_must_interpret`,
`crates/security-permissions/src/governance.rs::tests::zerodds_elements_without_the_namespace_follow_must_interpret`,
`crates/security-permissions/src/governance.rs::tests::a_governance_outside_the_xsd_is_refused`

**Status:** `done` — unknown elements and attributes follow `must_interpret`; an invalid attribute value makes the document invalid; the elements in the namespace `ZERODDS_NS` are treated by `must_interpret` as well.

### §10.4.1.4 Example Domain Governance document (non normative)

**Spec:** §10.4.1.4, p. 263–266 — example of a governance.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — marked as non-normative.

### §10.4.1.5 Permissions signed in S/MIME 3.2 format

**Spec:** §10.4.1.5, p. 266 — "The permissions document shall be signed by the
Permissions CA. The signed document shall use S/MIME version 3.2 format [...]
Additionally, the signer certificate shall be included within the signature."

**Repo:** `crates/security-permissions/src/signature.rs::open_signed_permissions`
(l. 89)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::happy_path_then_open_signed_permissions_parses`,
`crates/security-permissions/tests/cms_verify.rs::tampered_xml_is_rejected`

**Status:** done

### §10.4.1.5.1 Permissions XSD: dds, permissions, grant

**Spec:** §10.4.1.5.1, p. 266–270 — "<xs:element name='dds'
type='PermissionsNode' /> [...] permissions [...] grant [...]
<xs:attribute name='name' type='xs:string' use='required' />".

**Repo:** `crates/security-permissions/src/xml.rs::parse_permissions_xml`, `parse_grant`

**Tests:** `crates/security-permissions/src/xml.rs::tests::a_document_outside_the_xsd_is_refused`,
`crates/security-permissions/src/xml.rs::tests::rejects_invalid_xml`,
`crates/security-permissions/src/xml.rs::tests::missing_subject_name_is_malformed`

**Status:** `done` — documents follow the XSD: root `<dds>`, `<validity>`, at least one
rule, `<domains>` in each rule, `<topics>` in each action; an element outside
the XSD needs `must_interpret="false"` (§10.4.1.6).

### §10.4.1.5.3.1 Subject name: a grant applies only to a matching name

**Spec:** §10.4.1.5.3.1, p. 271 — "The contents of the <subject_name> element
shall be the x.509 subject name for the DomainParticipant as is given in its
Authorization Certificate. A permissions Section with a subject name that
does not match the subject name given in the corresponding Authorization
certificate shall be ignored."

**Repo:** `crates/security-permissions/src/xml.rs` (`Permissions::for_subject`),
`crates/security-runtime/src/profile.rs::from_files_with_cert_policy`,
`crates/security-runtime/src/shared.rs::set_remote_permissions`,
`crates/security-pki/src/subject.rs::subject_attributes`

**Tests:**
`crates/security-permissions/src/xml.rs::tests::only_the_grant_of_the_subject_counts`,
`crates/security-pki/src/subject.rs::tests::the_subject_is_read_in_certificate_order_with_short_names`

**Status:** `done` — locally and for a peer only the grant whose `subject_name`
is the certificate's SubjectName counts; without one the profile refuses, a peer
stays without a grant. A grant without `<subject_name>` still takes the `name`
attribute as subject.

### §10.4.1.5.3.1.1 Subject name matching per RFC 4514

**Spec:** §10.4.1.5.3.1, §10.4.1.5.3.1.1, p. 271–272 — "The format of x.509
subject name shall be the string representation of the X.509 certificate
Subject name as defined in IETF RFC 4514 [...] Two subject names match if an
only if the following conditions are met: They contain the same attribute
names. For each attribute name, the corresponding attribute values are
identical strings [...] the order of the atributes does not affect the
matching."

**Repo:** `crates/security-permissions/src/xml.rs` (`subject_name_attributes`,
`normalized`)

**Tests:**
`crates/security-permissions/src/xml.rs::tests::a_subject_name_reads_in_both_forms_and_any_order`,
`crates/security-permissions/src/xml.rs::tests::only_the_grant_of_the_subject_counts`

**Status:** `done` — the same attribute names (case-insensitive), equal values,
the order without meaning; read are the RFC 4514 form and the OpenSSL one-line
form (`/C=…/CN=…`).

### §10.4.1.5.3.2 Subject name expression

**Spec:** §10.4.1.5.3.2, §10.4.1.5.3.2.1, p. 272 — "The <subject_name_expression>
element shall contain the same type of attribute-value assertions as the
<subject_name > element [...] except that the attribute values may also
contain the special 'pattern' characters [...] match according to the POSIX
fnmatch() function [...] with flags: FNM_PATHNAME=TRUE, FNM_PERIOD=FALSE, and
FNM_NOESCAPE=TRUE."

**Repo:** `crates/security-permissions/src/xml.rs::parse_grant`, `Grant::is_for`,
`Permissions::for_subject`

**Tests:** `crates/security-permissions/src/xml.rs::tests::a_subject_name_expression_matches_by_fnmatch`

**Status:** `done` — `<subject_name_expression>` is read and compared with the
SubjectName by `fnmatch()` (`FNM_PATHNAME`, `FNM_NOESCAPE`).

### §10.4.1.5.3.3 Validity Section

**Spec:** §10.4.1.5.3.3, p. 272 — "It contains both the starting date and the
end date using the format defined by dateTime data type [...] Time zones that
aren't specified are considered UTC. A permissions Section with a validity
date that falls outside the current date at which the permissions are being
evaluated shall be ignored."

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_validity`, `Grant::is_valid_at`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::validity_window_enforced`,
`crates/security-permissions/src/governance.rs::tests::validity_and_domain_bound_the_grant`

**Status:** `done` — a date without a zone is UTC; a grant outside its period is
passed over.

### §10.4.1.5.3.3 Validity: time zones, absent and unreadable values

**Spec:** §10.4.1.5.3.3, §10.4.1.7, pp. 272, 283 — "Format is
CCYY-MM-DDThh:mm:ss[Z|(+|-)hh:mm] The time zone may be specified as Z (UTC)
or (+|-)hh:mm."; XSD: `<xs:element name='validity' type='Validity' />`
without `minOccurs='0'`.

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_validity`,
`parse_iso_seconds`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::iso_parser_applies_the_zone`,
`crates/security-permissions/src/xml.rs::tests::an_unreadable_validity_or_default_is_refused`,
`crates/security-permissions/src/xml.rs::tests::a_document_outside_the_xsd_is_refused`

**Status:** `done` — the time zone is applied; an unreadable `not_before` or
`not_after` and a grant without `<validity>` are errors, as the XSD requires.

### §10.4.1.5.3.4 Rules Section: order and default

**Spec:** §10.4.1.5.3.4, p. 272–273 — "The rules are applied in the same order
that appear in the document. If the criteria for the rule matches the
domain_id join and/or publish or subscribe operation that is being attempted,
then the allow or deny decision is applied. [...] If all rules have been
examined without a match, then the decision specified by the 'default' rule
is applied. [...] the absence of the default rule shall be treated as a
<default>DENY</default>."

**Repo:** `crates/security-permissions/src/xml.rs` (`Grant::decide`, `parse_grant`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::the_first_matching_rule_decides`,
`crates/security-permissions/src/xml.rs::tests::default_deny_without_explicit_tag`,
`crates/security-permissions/src/plugin.rs::tests::allow_rule_and_deny_rule_decide_the_same_xml_differently`

**Status:** `done` — the rules apply in document order, the first matching one decides;
without a match `default` applies, without `default` DENY.

### §10.4.1.5.3.4 Rules Section: the first matching rule decides

**Spec:** §10.4.1.5.3.4, p. 272 — "The rules are applied in the same order
that appear in the document."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::the_first_matching_rule_decides`,
`crates/security-permissions/src/plugin.rs::tests::allow_rule_and_deny_rule_decide_the_same_xml_differently`

**Status:** `done` — an `allow_rule` before a `deny_rule` for the same topic gives
"allow", in the opposite order "deny".

### §10.4.1.5.3.4 Criteria: topics AND partitions AND data tags

**Spec:** §10.4.1.5.3.4, p. 273 — "For the grant to match there shall be a
match of the topics, partitions, and data-tags criteria. This is interpreted
as an AND of each of the criteria. For a specific criterion to match (e.g.,
<topics>) it is enough that one of the topic expressions listed matches".

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`; `crates/security-permissions/src/governance.rs::granted`

**Tests:** `crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`,
`crates/security-permissions/src/xml.rs::tests::data_tags_of_allow_and_deny_rules`,
`crates/security-permissions/src/plugin.rs::tests::partition_scope_narrows_an_otherwise_permitted_topic`

**Status:** `done` — the topics, partitions and data tags of an action are combined, per
rule. Endpoints carry no data tags yet and enter with the empty set (item
§9.7.2).

### §10.4.1.5.3.4 Partitions, domains and data tags per action and rule

**Spec:** §10.4.1.5.3.4.1, §10.4.1.5.3.4.1.2, p. 273 — "Each rule contains a
Domains Section [...] followed by a set of allowed actions. [...] Each
allowed action logically contains three orthogonal conditions. These cover
the topic name, partitions, and data-tags."

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_grant`, `Grant::decide`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::a_deny_rule_holds_only_in_its_domains`,
`crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`,
`crates/security-permissions/src/xml.rs::tests::data_tags_of_allow_and_deny_rules`,
`crates/security-permissions/src/xml.rs::tests::domain_ranges_of_a_rule`

**Status:** `done` — domains, partitions and data tags count for the rule or action they
stand in; a deny rule holds only in its domains.

### §10.4.1.5.3.4.1.2 Allow Relay Action

**Spec:** §10.4.1.5.3.4.1.2, p. 273 — "There are three kinds of allowed
actions: publish, subscribe and relay. [...] The Allow Relay Action is
delimited by the <relay> XML element".

**Repo:** `crates/security-permissions/src/xml.rs` (`Action::Relay`);
`crates/security-permissions/src/governance.rs` (`check_datareader`,
`check_relay_reader`, `check_relay_writer`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::relay_is_an_action_of_its_own`,
`crates/security-permissions/src/governance.rs::tests::relaying_needs_the_relay_action_where_access_is_controlled`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`

**Status:** `done` — `<relay>` is an action of its own. A remote reader with `<relay>`
only is relay-only (§9.4.2.9.11); a local relay reader is created on
`<relay>`; a reader takes a forwarded sample as the original's only from a
writer with `<relay>` (§7.1.1.4).

### §10.4.1.5.3.4.1.3 Allowed Topic condition section

**Spec:** §10.4.1.5.3.4.1.3, p. 274 — "Topic names may be given explicitly or
by means of Topic name expressions. Each topic name or topic-name expression
appears separately in a <topic> sub-element within the <topics> element."

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_grant`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::a_document_outside_the_xsd_is_refused`,
`crates/security-permissions/src/plugin.rs::tests::permit_on_wildcard_match`

**Status:** `done` — `<topics>` with one `<topic>` for each name or expression; a
`<topic>` directly under the action is outside the XSD and refused.

### §10.4.1.5.3.4.1.4 Allowed Partitions condition: subset

**Spec:** §10.4.1.5.3.4.1.4, p. 274–275 — "the set of the Partitions
associated with the DDS entity [...] must be contained in the set of
partitions defined by the allowed partitions condition section. If there is
no <partitions> Section within an allow rule, then the default 'empty
string' partition is assumed."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`,
`crates/security-permissions/src/plugin.rs::tests::grant_without_partitions_covers_only_the_default_partition`,
`crates/security-permissions/src/plugin.rs::tests::partition_check_never_widens_a_denied_topic`

**Status:** `done` — per rule: the endpoint's partitions must lie within those of the
action; without `<partitions>` the partition `""` applies.

### §10.4.1.5.3.4.1.4 Selection of legacy matching for partitions

**Spec:** §10.4.1.5.3.4.1.4, p. 275 — "For legacy reasons DDS-Security
implementations shall provide a way to select an alternative 'legacy
matching' behavior. The 'legacy matching behavior' shall match the allowed
partitions condition condition as long as one or more of the Partitions
associated with DDS Entity attempting to perform the action [...] matches one
of the partitions in the allowed partitions condition."

**Repo:** `crates/security-permissions/src/xml.rs` (`PartitionMatching`);
setting `zerodds.sec.access.partition_matching`

**Tests:** `crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`

**Status:** `done` — selectable with `zerodds.sec.access.partition_matching`; the
default is the spec's comparison (subset).

### §10.4.1.5.3.4.1.5 Allowed Data tags condition section

**Spec:** §10.4.1.5.3.4.1.5, p. 275–276 — "the set of the Data Tags associated
with the DDS Entity performing the action [...] must be contained in the set
of data tags defined by the allowed data tags condition section. If there is
no <data_tags> section then the default empty set is assumed."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::data_tags_of_allow_and_deny_rules`,
`crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`,
`crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`

**Status:** `done` — the condition is evaluated per rule; endpoints carry data tags (`DataTagQosPolicy`) and the plugin sees them.

### §10.4.1.5.3.4.2 Deny rules

**Spec:** §10.4.1.5.3.4.2, p. 277 — "Deny rules have the same format as the
allow rules. The only difference is how they are interpreted. If the criteria
in the deny rule matches the operation being performed, then the decision is
to deny the operation."

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_grant`, `Grant::decide`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::the_first_matching_rule_decides`,
`crates/security-permissions/src/plugin.rs::tests::deny_rule_also_binds_the_reader_side`,
`crates/security-permissions/src/plugin.rs::tests::remote_match_inherits_the_deny_decision`

**Status:** `done` — a matching deny rule refuses the operation, in rule order, with its
domains and partitions.

### §10.4.1.5.3.4.2.4 Denied Partitions: intersection, "*" when absent

**Spec:** §10.4.1.5.3.4.2.4, p. 278 — "For this to happen one of more of the
partition names associated with the DDS Entity performing the action [...]
must match one the partitions or partition expressions explicitly listed in
the partitions condition section. If there is no <partitions> section then
the '*' partition expression is assumed."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`

**Status:** `done` — a deny rule applies when one partition of the endpoint matches one
of its partitions; without `<partitions>` `*` applies.

### §10.4.1.5.3.4.2.5 Denied Data tags condition section

**Spec:** §10.4.1.5.3.4.2.5, p. 279 — "If there is no <data_tags> section then
the 'set of all possible tags' set is assumed as default."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::data_tags_of_allow_and_deny_rules`,
`crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`

**Status:** `done` — without `<data_tags>` the set of all tags applies; endpoints carry data tags.

### §10.4.1.6 Permissions Document Extensibility

**Spec:** §10.4.1.6, p. 282 — "The approach is the same used for extending the
Governance document, see 10.4.1.3."

**Repo:** `crates/security-permissions/src/xml.rs::check_known_element_with`, `check_must_interpret_value`

**Tests:** `crates/security-permissions/src/xml.rs::tests::the_permissions_document_is_extended_by_must_interpret`

**Status:** `done` — unknown elements and attributes of the permissions document follow `must_interpret` as in the governance.

### §10.4.1.7 DomainParticipant example permissions document (non normative)

**Spec:** §10.4.1.7, p. 283–286 — example of a permissions document.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — marked as non-normative.

### §10.4.2.1 Table 64 — PermissionsCredentialToken

**Spec:** §10.4.2.1, Table 64, p. 286 — "class_id
'DDS:Access:PermissionsCredential' properties [...] dds.perm.cert Contents of
the permissions document signed by the PermissionCA".

**Repo:** `crates/security-permissions/src/plugin.rs::get_permissions_credential_token`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`

**Status:** `done` — the token has the class `DDS:Access:PermissionsCredential` and `dds.perm.cert`.

### §10.4.2.2 Table 65 — PermissionsToken

**Spec:** §10.4.2.2, Table 65, p. 286 — "class_id 'DDS:Access:Permissions:1.2'
properties (The presence of each of these properties is optional)
dds.perm_ca.sn [...] dds.perm_ca.algo".

**Repo:** `crates/security-permissions/src/plugin.rs::get_permissions_token`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`

**Status:** `done` — the token carries `dds.perm_ca.sn` and `dds.perm_ca.algo` of the permissions CA.

### §10.4.2.2 PermissionsToken: class_id 1.2

**Spec:** §10.4.2.2, Table 65, p. 286 — "class_id 'DDS:Access:Permissions:1.2'
[...] Accordingly this version of the specification has PluginClassName equal
to 'DDS:Access:Permissions', MajorVersion set to 1, and MinorVersion set to
0."

**Repo:** `crates/security-permissions/src/plugin.rs` (`PROP_TOKEN_CLASS_ID`, `plugin_class_id`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_permissions_token_class_is_1_2_unless_named`,
`crates/security-permissions/src/plugin.rs::tests::a_remote_token_of_another_class_or_major_version_is_refused`

**Status:** `done` — the default is `DDS:Access:Permissions:1.2` (Table 65). `DDS:Access:Permissions:1.0` stays as the named setting `org.zerodds.sec.access.permissions_token_class_id` for peers that send 1.0. Expected: Cyclone DDS and Fast DDS compare PluginClassName and MajorVersion (Table 70), so they accept 1.2; not measured.

### §10.4.2.3 Table 66 — PluginParticipantSecurityAttributes

**Spec:** §10.4.2.3, Table 66, p. 287–289 — `is_rtps_axk_encrypted`,
`is_rtps_psk_encrypted`, `is_discovery_encrypted`, `is_liveliness_encrypted`,
`is_rtps_origin_authenticated`, `is_discovery_origin_authenticated`,
`is_liveliness_origin_authenticated` and their effect on keys and
transformation.

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — all seven attributes (`*_encrypted`, `*_origin_authenticated`, `is_rtps_psk_encrypted`) follow the governance.

### §10.4.2.4 Table 67 — PluginParticipantSecurityAttributesMask

**Spec:** §10.4.2.4, Table 67, p. 290 — bits 0 to 6:
IS_RTPS_AXK_ENCRYPTED, IS_DISCOVERY_ENCRYPTED, IS_LIVELINESS_ENCRYPTED,
IS_RTPS_ORIGIN_AUTHENTICATED, IS_DISCOVERY_ORIGIN_AUTHENTICATED,
IS_LIVELINESS_ORIGIN_AUTHENTICATED, IS_RTPS_PSK_ENCRYPTED; default 0.

**Repo:** `crates/security/src/access_control.rs::plugin_participant_attributes_mask`; `crates/rtps/src/participant_security_info.rs::plugin_attrs`

**Tests:** `crates/security/src/access_control.rs::tests::the_plugin_masks_have_the_bits_of_tables_67_and_69`,
`crates/rtps/src/participant_security_info.rs::tests::fully_protected_sets_all_encrypt_bits`

**Status:** `done` — bits 0 to 6 are assigned and set from the config.

### §10.4.2.4 IS_RTPS_PSK_ENCRYPTED missing

**Spec:** §10.4.2.4, Table 67, p. 290 — "#define
PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_ENCRYPTED
(0x00000001 << 6)".

**Repo:** `crates/security/src/access_control.rs::plugin_participant_attributes_mask`

**Tests:** `crates/security/src/access_control.rs::tests::the_plugin_masks_have_the_bits_of_tables_67_and_69`,
`crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — bit 6 is set from `rtps_psk_protection_kind`.

### §10.4.2.5 Table 68 — PluginEndpointSecurityAttributes

**Spec:** §10.4.2.5, Table 68, p. 291–292 — `is_submessage_encrypted`,
`is_submessage_origin_authenticated`, `is_payload_encrypted` and their effect
on keys and transformation.

**Repo:** `crates/security-permissions/src/plugin.rs::get_datawriter_security_config`; `crates/security/src/access_control.rs::plugin_endpoint_attributes_mask`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security/src/access_control.rs::tests::the_plugin_masks_have_the_bits_of_tables_67_and_69`

**Status:** `done` — `is_submessage_encrypted`, `is_submessage_origin_authenticated` and `is_payload_encrypted` follow the topic rule.

### §10.4.2.6 Table 69 — PluginEndpointSecurityAttributesMask

**Spec:** §10.4.2.6, Table 69, p. 292 — IS_SUBMESSAGE_ENCRYPTED (0x1 << 0),
IS_PAYLOAD_ENCRYPTED (0x1 << 1), IS_SUBMESSAGE_ORIGIN_AUTHENTICATED
(0x1 << 2); default 0.

**Repo:** `crates/rtps/src/endpoint_security_info.rs::plugin_attrs` (l. 56)

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::protection_bit_accessors_read_correctly`

**Status:** done

### §10.4.2.4 and §10.4.2.6 Equal masks as a condition for communication

**Spec:** §10.4.2.4, §10.4.2.6, p. 290, 292 — "As described in section
7.3.23, in order to communicate, two DomainParticipants need to have the same
ParticipantSecurityInfo. As as consequence the
PluginParticipantSecurityAttributesMask must also be the same."

**Repo:** `crates/dcps/src/runtime.rs::endpoint_incompatibility` and the participant check during authentication

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — participants and endpoints with other masks than the own are not authenticated or not matched, respectively.

### §10.4.3 The plugin knows the public key of the Permissions CA

**Spec:** §10.4.3, p. 293 — "The DDS:Access:Permissions shall be initialized
to have access to the Permissions CA public key. As this is a builtin plugin
the mechanism for initialization is implementation dependent."

**Repo:** `crates/security-runtime/src/endpoint_access.rs::arm_endpoint_access_control`
(l. 26)

**Tests:** `crates/dcps/tests/security_endpoint_access_e2e.rs::an_unarmed_gate_refuses_instead_of_creating`

**Status:** done

### §10.4.3 Table 70 — check_create_participant

**Spec:** §10.4.3, Table 70, p. 293 — "If the ParticipantSecurityConfig has
is_access_protected set to FALSE, then the operation shall succeed and return
TRUE. If the Grant's first matching rule for the DomainParticipant's domain
is an allow rule, then the operation shall succeed and return TRUE. If the
Grant's first matching rule for the DomainParticipant's domain is a deny rule
with no publish or subscribe rules, then the operation shall fail and return
FALSE. If none of the previous conditions are true, then the operation shall
return TRUE if the default is ALLOW and return FALSE otherwise."

**Repo:** `crates/security-permissions/src/governance.rs::check_create_participant`;
`crates/security-permissions/src/xml.rs::Grant::may_join`

**Tests:** `crates/security-permissions/src/governance.rs::tests::check_create_participant_consults_permissions`,
`crates/security-permissions/src/xml.rs::tests::joining_follows_the_first_rule_for_the_domain`,
`crates/security-permissions/src/plugin.rs::tests::a_deny_rule_with_actions_first_leaves_joining_to_the_default`

**Status:** `done` — with FALSE the participant is admitted; otherwise the grant's first
rule for the domain decides: an allow rule admits, a deny rule without publish
and subscribe refuses, otherwise `default` applies.

### §10.4.3 Table 70 — check_create_participant: deny_rule and default

**Spec:** §10.4.3, Table 70, p. 293 — "If the Grant's first matching rule for
the DomainParticipant's domain is a deny rule with no publish or subscribe
rules, then the operation shall fail and return FALSE. If none of the
previous conditions are true, then the operation shall return TRUE if the
default is ALLOW and return FALSE otherwise."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::may_join`

**Tests:** `crates/security-permissions/src/xml.rs::tests::joining_follows_the_first_rule_for_the_domain`,
`crates/security-permissions/src/plugin.rs::tests::a_deny_rule_with_actions_first_leaves_joining_to_the_default`

**Status:** `done` — see the item above.

### §10.4.3 Table 70 — check_create_datawriter and check_create_datareader

**Spec:** §10.4.3, Table 70, p. 293 — "If the Governance specifies a topic or
topic-expression on the DomainParticipant domain_id matching the DataWriter
topic with enable_write_access_control set to FALSE, then the operation shall
succeed and return TRUE. If the Permissions document contains a Grant for the
DomainParticipant allowing it to publish the Topic with specified topic_name
on all the Publisher's PartitionQosPolicy names and with all the tags in the
DataWriter DataTagQosPolicy, then the operation shall succeed and return
TRUE. Otherwise the operation shall return FALSE."

**Repo:** `crates/security-permissions/src/plugin.rs::check_create_datawriter`, `check_create_datareader`; `governance.rs::check_datawriter`, `check_datareader`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_six_omg_suite_refusals_are_denied`,
`crates/security-permissions/src/governance.rs::tests::partitions_scope_the_grant`,
`crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`

**Status:** `done` — governance, topic, all partitions, data tags, grant of the subject and topic rule are checked.

### §10.4.3 Table 70 — check_create_topic

**Spec:** §10.4.3, Table 70, p. 293–294 — "If the Governance specifies a topic
or topic-expression on the DomainParticipant domain_id matching the Topic
name with enable_read_access_control set to FALSE or with
enable_write_access_control set to FALSE, then the operation shall succeed
and return TRUE. If the Permissions document contains a Grant for the
DomainParticipant allowing it to publish the Topic [...] subscribe the Topic
[...] return TRUE. Otherwise the operation shall return FALSE."

**Repo:** `crates/security-permissions/src/plugin.rs::check_create_topic`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::check_create_topic_follows_governance_and_grant`

**Status:** `done` — TRUE for an open topic or an allowed publish or subscribe; otherwise FALSE.

### §10.4.3 Table 70 — operations that return TRUE

**Spec:** §10.4.3, Table 70, p. 294–296 — "check_local_datawriter_register_instance
This operation shall return TRUE. check_local_datawriter_dispose_instance
[...] check_local_datawriter_match [...] check_local_datareader_match [...]
check_remote_datawriter_register_instance [...]
check_remote_datawriter_dispose_instance This operation shall return TRUE."

**Repo:** `crates/security-permissions/src/plugin.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`,
`crates/security-permissions/src/plugin.rs::tests::the_nil_handle_is_refused_on_a_controlled_topic_only`

**Status:** `done` — the six operations return TRUE; the handles are checked.

### §10.4.3 Table 70 — check_remote_participant

**Spec:** §10.4.3, Table 70, p. 294 — "If the PluginClassName or the
MajorVersion of the local permissions_token differ from those in the
remote_permissions_token, the operation shall return FALSE. If the Grant's
first matching rule for the remote DomainParticipant's domain is an allow
rule, then the operation shall succeed and return TRUE."

**Repo:** `crates/security-permissions/src/plugin.rs::check_remote_participant`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::a_remote_token_of_another_class_or_major_version_is_refused`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`

**Status:** `done` — PluginClassName and MajorVersion of the token are compared; otherwise the first matching rule of the grant for the domain decides.

### §10.4.3 Table 70 — check_remote_datawriter and check_remote_datareader

**Spec:** §10.4.3, Table 70, p. 294–295 — "If the PluginClassName or the
MajorVersion of the local permissions_token differ from those in the
remote_permissions_token, the operation shall return FALSE. If the remote
DomainParticipant Permissions document contains a Grant allowing it to
publish the DataWriter's topic_name on all the remote Publisher's
PartitionQosPolicy names [...] If the Permissions document contains a Grant
for the remote DomainParticipant allowing it to 'relay' the DataReader's
topic_name, the operation shall return TRUE and also set the
'allow_relay_only' output parameter to TRUE."

**Repo:** `crates/security-runtime/src/shared.rs::check_remote_datawriter`,
`::check_remote_datareader`, `::set_remote_permissions`;
`crates/security/src/token.rs::class_id::same_class_and_major_version`

**Tests:** `crates/security-runtime/src/shared.rs::tests::a_peer_without_a_grant_is_denied_on_an_access_controlled_topic`,
`crates/security/src/token.rs::tests::class_ids_compare_by_plugin_class_name_and_major_version`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`

**Status:** `done` — a peer whose PermissionsToken is missing or names another
PluginClassName or MajorVersion keeps no grant and is refused on
access-controlled topics; the peer's grant and partitions decide; a reader
with `<relay>` only is relay-only. Measured: green on the Linux test host.

### §10.4.3 Table 70 — check_remote_topic

**Spec:** §10.4.3, Table 70, p. 295 — "If the Permissions document contains a
Grant for the DomainParticipant allowing it to publish the Topic with
specified topic_name, then the operation shall succeed and return TRUE."

**Repo:** `crates/security-permissions/src/plugin.rs::check_remote_topic`; `crates/security-permissions/src/xml.rs::Grant::allows_topic`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_nil_handle_is_refused_on_a_controlled_topic_only`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`

**Status:** `done` — TRUE for an allowed publish, subscribe or relay of the topic in the grant of the remote participant.

### §10.4.3 Table 70 — get_permissions_token and get_permissions_credential_token

**Spec:** §10.4.3, Table 70, p. 296 — "get_permissions_token This operation
shall return the PermissionsToken formatted as described in 10.4.2.2.
get_permissions_credential_token This operation shall return the
PermissionsToken formatted as described in 10.4.2.1".

**Repo:** `crates/security-permissions/src/plugin.rs`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`

**Status:** `done` — both tokens are formed as in §10.4.2.1 and §10.4.2.2.

### §10.4.3 Table 70 — validate_local_permissions

**Spec:** §10.4.3, Table 70, p. 296 — "The operation shall check the subject
name in the Identity Certificate matches the one from the Signed Permissions
document. The operation shall verify the signature of the Signed Domain
Governance and Signed Permissions document by the configured Permissions CA."

**Repo:** `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`
(`NoGrantForSubject`)

**Tests:**
`crates/security-permissions/tests/cms_verify.rs::wrong_ca_signature_is_rejected`,
`crates/security-permissions/tests/cms_verify.rs::tampered_xml_is_rejected`,
`crates/security-permissions/src/xml.rs::tests::only_the_grant_of_the_subject_counts`

**Status:** `done` — both signatures are checked; a local document without a
grant for the own certificate's SubjectName is refused.

### §10.4.3 Table 70 — validate_remote_permissions

**Spec:** §10.4.3, Table 70, p. 296–297 — "The operation shall check the
subject name in the Signed Permissions Document matches the one in the
Identity Certificate. The operation shall verify the signature of the Signed
Permissions Document by the configured Permissions CA."

**Repo:** `crates/security-permissions/src/plugin.rs::validate_remote_permissions`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_document_needs_the_permissions_ca`

**Status:** `done` — the signature by the configured permissions CA and the match of the subject with the identity certificate are checked.

### §10.4.3 Table 70 — get_participant_security_config

**Spec:** §10.4.3, Table 70, p. 297–298 — "If the Governance document has the
element allow_unauthenticated_participants set to FALSE, the attributes field
allow_unauthenticated_participants shall be set to FALSE. Otherwise the field
shall be set to TRUE. [...] If the Governance document does not have the XML
element <allowed_crypto_algorithms> the algorithm_info shall have the
'supported_mask' field corresponding each of the algorithm types set to
CRYPTO_ALGORITHM_SET_ALL".

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/security-permissions/src/plugin.rs::tests::without_allowed_algorithms_every_algorithm_is_supported`,
`crates/security-permissions/src/governance.rs::tests::missing_domain_rule_flags_are_true`

**Status:** `done` — `allow_unauthenticated_participants` and all other members follow Table 70; without `<allowed_crypto_algorithms>` the `supported_mask` is CRYPTO_ALGORITHM_SET_ALL.

### §10.4.3 Table 70 — set_listener and return_*

**Spec:** §10.4.3, Table 70, p. 296, 298 — "set_listener This operation shall
save a reference to the listener object [...] return_permissions_token This
operation shall behave as specified in 9.4.2.9.20" (likewise for the other
return_*)

**Repo:** `crates/security-permissions/src/plugin.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::revoking_permissions_tells_the_listener`,
`crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`

**Status:** `done` — `set_listener` keeps the reference; the `return_*` behave as in §9.4.2.9.

---

### §10.5 DDS:Crypto:AES-GCM-GMAC: AEAD with AES-GCM and AES-GMAC

**Spec:** §10.5, p. 299 — "This plugin does Authenticated Encryption with
Associated Data (AEAD) using Advanced Encryption Standard with Galois Counter
Mode (AES-GCM/GMAC) [...] the DataWriter should create a separate
'reader-specific key' used only for authentication and append additional
reader-specific MACs".

**Repo:** `crates/security-crypto/src/plugin.rs::AesGcmCryptoPlugin`
(l. 724)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::encrypt_decrypt_roundtrip`,
`crates/security-crypto/src/plugin.rs::tests::gmac_only_suite_roundtrip_without_encryption`

**Status:** done — the MACs for single readers are assessed under Table 80.

### §10.5.1 Table 71 — configuration through dds.sec.crypto.*

**Spec:** §10.5.1, Table 71, p. 299–301 — "The DDS:Crypto:AES-GCM-GMAC plugin
shall be configured using the PropertyQosPolicy of the DomainParticipantQos,
DataWriterQos, or DataReaderQos." Properties `symmetric_cipher_algorithm`,
`rtps_psk_symmetric_cipher_algorithm`, `rtps_psk_secret_passphrase`,
`rtps_psk_secret_passphrase_extra`, each with prefix `dds.sec.crypto.`.

**Repo:** `crates/security-runtime/src/qos_properties.rs`; `crates/security-crypto/src/plugin/spec.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_cipher_property_of_an_endpoint_sets_its_key_width`, `crates/security-crypto/src/psk_rtps.rs::tests::the_uri_properties_of_the_pre_shared_key_are_read`, `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`

**Status:** `done` — `symmetric_cipher_algorithm` of participant, writer and reader (the runtime passes the endpoint property on), the PSK properties as `file:`/`data:` URI. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` and `-p zerodds-dcps --features security --lib` (7 tests), log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §10.5.1.1 Symmetric Cipher Algorithm: default AES256-GCM

**Spec:** §10.5.1, Table 71, §10.5.1.1, p. 299–301 — "If 'AUTO' is specified
it is treated as if it was specified to be 'AES256-GCM'. [...] If used in a
DataWriterQos or DataReaderQos it configures the symmetric cipher used by that
specific DataWriter or DataReader".

**Repo:** `crates/security-crypto/src/suite.rs::symmetric_cipher_key_bits`; `crates/security-crypto/src/plugin/spec.rs`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::default_plugin_uses_aes256`, `crates/security-crypto/src/plugin.rs::tests::the_key_width_follows_symmetric_cipher_algorithm`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_gcm_or_gmac_by_the_protection_kind_and_width`, `crates/dcps/src/runtime.rs::tests::the_cipher_property_of_an_endpoint_sets_its_key_width`

**Status:** `done` — Missing or `AUTO` is AES256-GCM, overridable per endpoint. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` and `-p zerodds-dcps --features security --lib` (7 tests), log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §10.5.1.2 to §10.5.1.4 PSK Symmetric Cipher Algorithm and Secret Passphrase

**Spec:** §10.5.1.2–§10.5.1.4, p. 301–302 — "Configures the Cipher Algorithm
Pair that the DomainParticipant will use to protect RTPS Messages with a
pre-shared Key. [...] Configures the KeyMaterial and CryptoTransformKeyId and
CryptoTransformKeyRevision used by the operation encode_rtps_message when the
parameter transform_with_psk is set to true. [...] Configures alternative
values for the KeyMaterial".

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskConfig``, `key_bits_from_cipher`; `crates/security-runtime/src/qos_properties.rs::psk_crypto_config_from_properties`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_passphrase_rules_of_table_86`, `crates/security-runtime/src/qos_properties.rs::tests::the_psk_crypto_properties_of_table_86_are_read`

**Status:** `done` — key width, passphrase with its id and the alternatives are read; the derivation follows §10.5.2.1.3.

### §10.5.2 Types of the plugin fully specified

**Spec:** §10.5.2, p. 302 — "All 'Handle' types are local opaque handles that
are only understood by the local plugin objects that create or use them. The
remaining types shall be fully specified so that independent implementations
of DDS:Crypto:AES-GCM-GMAC can interoperate."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — introduction; the types follow in Table 72
to 77.

### §10.5.2.1 Table 72 — CryptoToken

**Spec:** §10.5.2.1, Table 72, p. 302 — "class_id 'DDS:Crypto:AES_GCM_GMAC'
binary_properties [...] dds.cryp.keymat The Big Endian CDR Serialization of
the KeyMaterial_AES_GCM_GMAC structure defined below."

**Repo:** `crates/dcps/src/runtime.rs` (constants `CRYPTO_TOKEN_PROP` l. 21328
and `CRYPTO_TOKEN_CLASS_ID` l. 21333), `build_crypto_token_message`
(l. 21338)

**Tests:** `crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`,
`crates/security-crypto/src/plugin.rs::tests::cross_plugin_interop_via_tokens`

**Status:** done

### §10.5.2.1.1 KeyMaterial_AES_GCM_GMAC: structure and encoding

**Spec:** §10.5.2.1.1, p. 302–303 — "@extensibility(FINAL) struct
KeyMaterial_AES_GCM_GMAC { CryptoTransformKind transformation_kind;
sequence<octet, 32> master_salt; CryptoTransformKeyId sender_key_id;
sequence<octet, 32> master_sender_key; CryptoTransformKeyId
receiver_specific_key_id; sequence<octet, 32> master_receiver_specific_key;
};"

**Repo:** `crates/security-crypto/src/plugin.rs::serialize_keymat_for`, `serialize_keymat_cyclone`, `from_keymat_cyclone`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::a_receiver_specific_key_id_and_key_go_together`, `::tokens_carry_suite_tag_so_cross_suite_interop_works`, `::rejects_token_with_unknown_suite_id`

**Status:** `done` — the six members are written and read in this order in Big Endian, with `receiver_specific_key_id` and `master_receiver_specific_key`; the id is 0 if and only if the key is empty, and the key has the length of the sender key.

### §10.5.2.1.1 master_salt of 16 octets for AES128 and keys for single receivers

**Spec:** §10.5.2.1.1, §10.5.3.3.2, Table 81, pp. 303, 321 — "MasterSalt
octet[16] for AES128 [...] octet[32] for AES256"; "A zero value for
receiver_specific_key_id indicates there is no receiver-specific
authentication tags and shall occur if and only if the length of the
master_receiver_specific_key is also zero."

**Repo:** `crates/security-crypto/src/plugin.rs::from_keymat_cyclone`, `receiver_key_for`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::an_aes128_gmac_key_with_a_short_salt_crosses_instances`, `::a_receiver_specific_key_id_and_key_go_together`

**Status:** `done` — `master_salt` of up to 32 octets (earlier round); `receiver_specific_key_id` and `master_receiver_specific_key` are written under origin authentication (an id other than 0, never that of another key of this side) and read.

### §10.5.2.1.2 Table 73 — key material for VolatileMessageSecure from the SharedSecret

**Spec:** §10.5.2.1.2, Table 73, Table 74, p. 303–304 — "transformation_kind
Set transformation_algorithm_id to CRYPTO_ALGORITHM_ID_AES256_GCM master_salt
HMACsha256 ( sha256(Challenge1 | KxSaltCookie | Challenge2) , SharedSecret)
sender_key_id 0 master_sender_key HMACsha256 ( sha256(Challenge2 |
KxKeyCookie | Challenge1) , SharedSecret ) receiver_specific_key_id 0
master_receiver_specific_key Zero-length sequence"; KxKeyCookie "key exchange
key", KxSaltCookie "keyexchange salt".

**Repo:** `crates/security-crypto/src/plugin.rs::from_kx_shared_secret`
(l. 590), `register_matched_remote_participant` (l. 1352)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_kx_key_matches_both_directions`,
`crates/security-crypto/src/plugin.rs::tests::with_secret_provider_derives_same_kx_key_for_both_sides`

**Status:** done — without known challenges the plugin derives the key with
its own HKDF (`from_shared_secret_kx`, l. 573; comment at l. 1380–1383); this
case concerns PSK and mocks.

### §10.5.2.1.3 Table 75 and Table 76 — key material for RTPS PSK protection

**Spec:** §10.5.2.1.3, Table 75, Table 76, p. 304–307 — "master_salt
HMACsha256( HMACsha256('PSK-SALT' | SenderKeyId | 'RTPS' | ProtocolVersion |
VendorId | GuidPrefix, PreSharedSecret), 'master salt derivation' | 0x01 )
[...] master_sender_key HMACsha256( HMACsha256('PSK-SKEY' | SenderKeyId |
'RTPS' | ProtocolVersion | VendorId | GuidPrefix, PreSharedSecret), 'master
sender key derivation'| 0x01 )"; example with result values.

**Repo:** `crates/security-crypto/src/psk_rtps.rs::derive_key_material``, `domain_key_prefix`, `Passphrase`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_material_is_the_one_the_spec_prints`

**Status:** `done` — SenderKeyId, `master_salt`, `master_sender_key` and revision equal the example of the spec (domain 201, passphrase 5632); cut to 16 octets for AES-128. `receiver_specific_key_id` 0, no `master_receiver_specific_key`.

### §10.5.2.2 Table 77 — CryptoTransformIdentifier: transformation_kind

**Spec:** §10.5.2.2, Table 77, p. 308 — "CRYPTO_ALGORITHM_ID_NONE
CRYPTO_ALGORITHM_ID_AES128_GMAC CRYPTO_ALGORITHM_ID_AES128_GCM
CRYPTO_ALGORITHM_ID_AES256_GMAC CRYPTO_ALGORITHM_ID_AES256_GCM [...] These
variants shall have master_sender_key with 16 octets in length [...] 32
octets".

**Repo:** `crates/security-crypto/src/suite.rs`

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::kind_round_trip_all_variants`, `crates/security-crypto/src/suite.rs::tests::aes128_gmac_uses_spec_id_1`

**Status:** `done` — All four algorithm ids carry AES (0x01 AES128-GMAC). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.2.2 Table 77 — transformation_key_revision

**Spec:** §10.5.2.2, Table 77, p. 308 — "Set the transformation_key_revision
field to the value {0, 0, 0} the first time Key Material is produced for a
specific value of the transformation_key_id. Subsequent generation of the Key
Material for that same transformation_key_id should increment the
transformation_key_revision".

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_key_revision_is_carried_in_the_keymat_and_the_header`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`

**Status:** `done` — First revision {0,0,0}, then incremented. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.2.2 Table 77 — transformation_key_id

**Spec:** §10.5.2.2, Table 77, p. 308 — "This is set to a different value each
time new Key Material is produced by a DomainParticipant. The algorithm used
is implementation specific but it shall avoid repeating the values for the
same DomainParticipant."

**Repo:** `crates/security-crypto/src/plugin.rs::new_local_key`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_local_keys_have_distinct_nonzero_key_ids`

**Status:** `done` — Random, never 0, never repeated for a participant. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.2.2 transformation_key_id: no repetition per participant

**Spec:** §10.5.2.2, Table 77, p. 308 — "it shall avoid repeating the values
for the same DomainParticipant."

**Repo:** `crates/security-crypto/src/plugin.rs::new_local_key`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_local_keys_have_distinct_nonzero_key_ids`

**Status:** `done` — A drawn id is discarded when this side has already used it. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.2.3 CryptoHeader: transform_identifier, session_id, initialization_vector_suffix

**Spec:** §10.5.2.3, p. 309 — "struct CryptoHeader {
CryptoTransformIdentifier transform_identifier; octet session_id[4]; octet
initialization_vector_suffix[8]; }; [...] The CryptoHeader structure shall be
serialized using Big Endian serialization".

**Repo:** `crates/security-crypto/src/plugin.rs::seal_cyclone` (l. 278,
20-octet header)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::header_round_trip_with_full_iv`,
`crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §10.5.2.4 CryptoContent

**Spec:** §10.5.2.4, p. 309 — "struct CryptoContent { sequence<octet>
crypto_content; }; The CryptoContent structure shall be serialized using Big
Endian serialization".

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (l. 1106),
`encode_payload_from_mat` (l. 918)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`,
`crates/security-crypto/src/plugin.rs::tests::serialized_payload_cross_plugin_via_tokens`

**Status:** done

### §10.5.2.5 CryptoFooter and ReceiverSpecificMAC

**Spec:** §10.5.2.5, p. 309–310 — "struct ReceiverSpecificMAC {
CryptoTransformKeyId receiver_mac_key_id; octet receiver_mac[16]; }; [...]
struct CryptoFooter { octet common_mac[16]; sequence<ReceiverSpecificMAC>
receiver_specific_macs; };"

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoFooter`
(l. 207); `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::footer_round_trip_with_receivers`,
`crates/security-crypto/src/crypto_transform.rs::tests::footer_rejects_oversized_receiver_count_cleanly`

**Status:** done — the runtime codec always writes an empty list.

### §10.5.3 Three interfaces

**Spec:** §10.5.3, p. 310 — "This plugin implements three interfaces:
CryptoKeyFactory, CryptoKeyExchange, and CryptoTransform."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure; ZeroDDS bundles the three in one
interface (see §9.5.1).

### §10.5.3.1 Table 78 — register_local_participant: ParticipantKeyMaterial

**Spec:** §10.5.3.1, Table 78, p. 310–311 — "This operation shall create a new
KeyMaterial_AES_GCM_GMAC object [...] ParticipantKeyMaterial. The
transformation_kind member transformation_algorithm_id [...] determines
whether the transformation performs authentication only (GMAC) or
authenticated encryption (GCM). The selection between these two options
shall be done according to the setting of the RTPS Protection Kind".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_gcm_or_gmac_by_the_protection_kind_and_width`, `crates/security-crypto/src/plugin/spec.rs::tests::register_local_participant_fills_the_adjusted_algorithm_info`

**Status:** `done` — GCM or GMAC by the RTPS Protection Kind (`is_rtps_axk_encrypted`), width by the property; `adjusted_algorithm_info` filled. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — register_matched_remote_participant

**Spec:** §10.5.3.1, Table 78, p. 311 — "The Participant2ParticipantKeyMaterial
transformation_kind, master_salt, and master_sender_key, and sender_key_id
shall match those of the ParticipantKeyMaterial. [...] This operation also
creates a KeyMaterial_AES_GCM_GMAC object derived from the SharedSecret
passed as a parameter. [...] Participant2ParticipantKxKeyMaterial."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_participant2participant_material_matches_the_participant_key`, `crates/security-crypto/src/plugin/spec.rs::tests::the_plugin_accepts_only_the_supported_algorithms`

**Status:** `done` — Kind, salt, key and id as the ParticipantKeyMaterial; Kx material from the SharedSecret. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — register_local_datawriter: WriterKeyMaterialSeq

**Spec:** §10.5.3.1, Table 78, p. 311–312 — "If the Metadata Protection Kind
is different from NONE, then the operation shall create a
KeyMaterial_AES_GCM_GMAC to use for the encode_datawriter_submessage
operation. In addition, this key material shall appear as the first element
in the KeyMaterial_AES_GCM_GMAC_Seq. If the Data Protection Kind is different
from NONE, then the operation shall create a KeyMaterial_AES_GCM_GMAC to use
for the encode_serialized_payload operation. In the case where both meta-data
protection and data protection are the same, it is allowed for an
implementation to reuse the same key material for both."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`

**Status:** `done` — One or two elements by Metadata and Data Protection Kind, submessage key first; the kinds come from `EndpointSecurityConfig`. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — register_matched_remote_datareader and register_matched_remote_datawriter

**Spec:** §10.5.3.1, Table 78, p. 312–313 — "This operation shall create a new
KeyMaterial_AES_GCM_GMAC_Seq object [...] Writer2ReaderKeyMaterialSeq. The
first elements of the Writer2ReaderKeyMaterialSeq shall contain the elements
of the WriterKeyMaterialSeq. [...] Reader2WriterKeyMaterial. The
transformation_kind, master_salt, and master_sender_key, and sender_key_id
for the Reader2WriterKeyMaterial object shall match those in the DataReader
ReaderKeyMaterial."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — Writer2ReaderKeyMaterialSeq and Reader2WriterKeyMaterial as operations of the specification; a receiver key for each remote endpoint. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — register_local_datareader: ReaderKeyMaterial

**Spec:** §10.5.3.1, Table 78, p. 312–313 — "This operation shall create a new
KeyMaterial_AES_GCM_GMAC object [...] ReaderKeyMaterial. The
transformation_kind. member transformation_algorithm_id for the
ReaderKeyMaterial object determines whether the transformation performs
authentication only (GMAC) or encryption followed by authentication (GCM)."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — One material; the kind follows the submessage attribute (the specification names the Data Protection Kind here, a contradiction in the specification; readers protect only submessages). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — revise_local_entity_keys and activate_key_revision

**Spec:** §10.5.3.1, Table 78, p. 313–314 — "The transformation_kind shall
only change the key_revision member. The master_salt shall be entirely new.
The sender_key_id shall remain the same. [...] This operation shall not
create new Key Material for the BuiltinParticipantVolatileMessageSecureWriter
and BuiltinParticipantVolatileMessageSecureReader."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_revised_with_the_entities`

**Status:** `done` — New salt and key; id and receiver key stay. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — unregister_participant, unregister_datawriter, unregister_datareader

**Spec:** §10.5.3.1, Table 78, p. 314 — "Releases any resources allocated on
the corresponding call to register_local_participant, or
register_matched_remote_participant."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::unregistering_releases_the_key_material_and_the_matches`

**Status:** `done` — Release the resources of the registrations. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.2 Table 79 — create_local_participant_crypto_tokens and set_remote_participant_crypto_tokens

**Spec:** §10.5.3.2, Table 79, p. 314 — "Creates a DDS:Crypto:AES-GCM-GMAC
CryptoToken object and returns it in the output sequence. The CryptoToken
contains the Participant2ParticipantKeyMaterial created on the call to
register_matched_remote_participant [...] Shall receive the sequence
containing one CryptoToken object".

**Repo:** `crates/security-crypto/src/plugin.rs::create_local_participant_crypto_tokens`
(l. 1430), `set_remote_participant_crypto_tokens` (l. 1450)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cross_plugin_interop_via_tokens`,
`crates/security-runtime/src/shared.rs::tests::data_round_trips_via_token_after_kx_register`

**Status:** done

### §10.5.3.2 Table 79 — endpoint tokens and return_crypto_tokens

**Spec:** §10.5.3.2, Table 79, p. 314 — "create_local_datawriter_crypto_tokens
[...] The CryptoToken contains the Writer2ReaderKeyMaterial [...]
create_local_datareader_crypto_tokens [...] The CryptoToken contains the
Reader2WriterKeyMaterial [...] return_crypto_tokens Releases the resources".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::crypto_tokens_are_returned_once`, `crates/security-crypto/src/plugin/spec.rs::tests::a_relay_only_reader_gets_the_submessage_key_alone`

**Status:** `done` — Writer2Reader and Reader2Writer material in the tokens; `return_crypto_tokens` releases. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — encode_serialized_payload

**Spec:** §10.5.3.3.1, Table 80, p. 315 — "If the transformation_kind
indicates that encryption is performed, then the output shall be the three
RTPS Submessage elements: CryptoHeader, CryptoContent, and CryptoFooter [...]
If the transformation_kind indicates that only authentication is performed,
then the output shall be the three RTPS Submessage elements: CryptoHeader,
SerializedPayload, and CryptoFooter. [...] This operation shall always set
the receiver_specific_macs attribute in the CryptoFooter to the empty
sequence."

**Repo:** `crates/security-crypto/src/plugin.rs::encode_serialized_payload`
(l. 1745), `encode_payload_from_mat` (l. 918)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::serialized_payload_cross_plugin_via_tokens`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** done

### §10.5.3.3.1 Table 80 — encode_datawriter_submessage and encode_datareader_submessage

**Spec:** §10.5.3.3.1, Table 80, p. 315–316 — "If the transformation_kind
indicates that encryption is performed, then the output shall be the three
RTPS Submessages: SecurePrefixSubMsg, SecureBodySubMsg, and
SecurePostfixSubMsg. [...] only authentication [...] SecurePrefixSubMsg,
InputSubmessage, and SecurePostfixSubMsg. [...] Depending on the
configuration the operation may compute and set the common_mac and the
receiver_specific_macs".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_receiver_specific_macs_follow_the_list_index`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — `receiver_specific_macs` for each receiver of the list. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — encode_rtps_message without AAD

**Spec:** §10.5.3.3.1, Table 80, p. 316–317 — "1) Transformation when
'Additional Authenticated Data (AAD)' is disabled Let RTPSMessage{RTPSHdr->
InfoSourceSubMsg} represent the input RTPS Message transformed so that the
RTPS Header is replaced with an RTPS InfoSourceSubMsg [...] The
SecureRTPSPrefixSubMsg flag AuthenticatedDataFlag shall be unset. The
SecureRTPSPrefixSubMsg PreSharedKeyFlag shall be set if encode_rtps_message
was called with transform_with_psk=TRUE."

**Repo:** `crates/security-crypto/src/plugin.rs::encode_rtps_message_cyclone`
(l. 1655, INFO_SRC before the content), `assemble_srtps_message` (l. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`,
`crates/security-runtime/src/shared.rs::tests::transform_outbound_for_encrypt_produces_srtps_wire`

**Status:** done — with GMAC SEC_BODY is left out as in §10.5.3.3.4.6.2; the
flag P is never set because there is no protection with a pre-shared key.

### §10.5.3.3.1 Table 80 — encode_rtps_message with AAD and HeaderExtension

**Spec:** §10.5.3.3.1, Table 80, p. 317–318 — "2) Transformation when
'Additional Authenticated Data (AAD)' is enabled [...] The
SecureRTPSPrefixSubMsg flag AdditionlAuthenticatedDataFlag shall be set.
[...] 3) [...] the HeaderExtension, if present, shall be adjusted as follows:
[...] messageLength element, if present, set to zero [...] messageChecksum
element, if present, set to zero".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::an_rtps_message_goes_through_the_interface_with_aad_and_a_header_extension`, `crates/security-crypto/src/plugin.rs::tests::the_header_extension_is_adjusted_for_the_aad_and_the_decoded_message`

**Status:** `done` — Flag A, header and HeaderExtension (length and checksum zero) as AAD, then `messageLength` and `messageChecksum` (CRC-32C, CRC-64/XZ, MD5) of the encoded message; setting `org.zerodds.crypto.rtps_additional_authenticated_data` (default off: INFO_SRC form). run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — decode_rtps_message

**Spec:** §10.5.3.3.1, Table 80, p. 318–319 — "Examines the
SecureRTPSPrefixSubMsg to determine the transformation_kind matches the one
the receiving DomainParticipant is expecting [...] Uses source
DomainParticipant GUIDs in the RTPS Header to locate the
sending_participant_crypto and receiving_participant_crypto. Then looks
whether the transformation_key_id attribute in the CryptoTransformIdentifier
is associated with those ParticipantCryptoHandles. If the association is not
found, the operation shall fail with an exception."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::an_rtps_message_goes_through_the_interface_without_aad`, `crates/security-crypto/src/plugin/spec.rs::tests::a_kind_other_than_the_expected_one_is_refused`, `crates/security-runtime/src/shared.rs::tests::classify_inbound_srtps_from_known_peer_accepts`

**Status:** `done` — Expected kind, key only of the handles of sender and receiver. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 decode_rtps_message: only the sender's key, expected kind

**Spec:** §10.5.3.3.1, Table 80, p. 318 — "If the kind is not the expected
one, the operation shall fail with an exception. [...] If the association is
not found, the operation shall fail with an exception."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_kind_other_than_the_expected_one_is_refused`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_is_taken_only_from_the_entities_it_is_associated_with`, `crates/security-runtime/src/shared.rs::tests::a_key_of_another_peer_does_not_decode`

**Status:** `done` — The kind (GCM/GMAC, origin authentication, algorithm in `supported_mask`) is checked; a key of another sender does not open. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — preprocess_secure_submsg

**Spec:** §10.5.3.3.1, Table 80, p. 319 — "1. Determine whether the
CryptoTransformIdentifier the transformation_kind matches one of the
recognized kinds. 2. Classify the RTPS Submessage as a Writer or Reader
Submessage. 3. Retrieve the DatawriterCryptoHandle and DataReaderCryptoHandle
handles associated with the CryptoTransformIdentifier transformation_key_id."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::preprocess_refuses_unknown_kinds_and_unknown_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Recognized kind, classification, handles. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — decode_datawriter_submessage and decode_datareader_submessage

**Spec:** §10.5.3.3.1, Table 80, p. 319–320 — "If the
RemoteDatawriterKeyMaterial specified a transformation_kind different from
CRYPTO_ALGORITHM_ID_NONE, then the operation shall check that the received
SecurePostfixSubMsg contains a common_mac and use it to verify the RTPS
SubMessage [...] If the RemoteDatawriter2DatareaderKeyMaterial specified a
receiver_specific_key_id different from zero, then the operation shall check
that the received SecurePostfixSubMsg contains a non-zero length
receiver_specific_macs element".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-crypto/src/plugin/spec.rs::tests::a_kind_other_than_the_expected_one_is_refused`

**Status:** `done` — `common_mac` and the receiver MAC are required and checked. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — decode_serialized_payload

**Spec:** §10.5.3.3.1, Table 80, p. 320 — "Uses writerGUID and the readerGUID
in the RTPS SubMessage to locate the sending_datawriter_crypto and
receiving_datareader_crypto. Then looks whether the transformation_key_id
attribute in the CryptoTransformIdentifier in the CryptoHeader
SubmessageElement is associated with those CryptoHandles. If the association
is not found, the operation shall fail with an exception."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_serialized_payload_goes_through_the_interface`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_is_taken_only_from_the_entities_it_is_associated_with`

**Status:** `done` — Key by id and revision among the keys associated with the handles. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.2 Table 81 — state of the virtual machine

**Spec:** §10.5.3.3.2, Table 81, p. 320–322 — MasterKey, MasterSalt,
MasterKeyId, MasterReceiverSpecificKey, InitializationVectorSuffix,
SessionId, SessionKey, SessionReceiverSpecificKey, session_block_counter,
max_blocks_per_session.

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_session_id_changes_after_max_blocks_per_session`, `crates/security-crypto/src/plugin/spec.rs::tests::the_initialization_vector_suffix_starts_random_and_counts`, `crates/security-crypto/src/plugin/spec.rs::tests::the_receiver_specific_macs_follow_the_list_index`

**Status:** `done` — MasterReceiverSpecificKey, `session_block_counter`, `max_blocks_per_session`, random start of the IV suffix. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.2 and §10.5.3.3.4 New SessionId after max_blocks_per_session

**Spec:** §10.5.3.3.2, §10.5.3.3.4, p. 321, 322 — "If the session_block_counter
exceeds this value, a new SessionKey and SessionReceiverSpecificKey are
computed and the session_block_counter is reset to zero. [...] If the
operation detects that the counter would exceed the maximum then it should
modify the SessionId and derive new session keys prior to transforming any of
the input plain text."

**Repo:** `crates/security-crypto/src/plugin.rs::session_for`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_session_id_changes_after_max_blocks_per_session`

**Status:** `done` — The SessionId is incremented and the counter reset before plaintext is transformed. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.3 SessionKey from MasterKey, MasterSalt and SessionId

**Spec:** §10.5.3.3.3, p. 322 — "SessionKey := HMAC256(MasterKey,'SessionKey'
| MasterSalt | SessionId) [...] In case a 128 key is desired the 256 bit HMAC
is truncated to the first 128 bits."

**Repo:** `crates/security-crypto/src/session_key.rs::derive_session_key_cyclone`
(l. 95)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/security-crypto/src/session_key.rs::tests::session_key_aes128_gcm_truncated_to_16_byte`

**Status:** done — the function `derive_session_key` (l. 65) next to it puts
MasterSalt before the string; it is only called by tests, and its module
documentation gives this order as the spec's.

### §10.5.3.3.3 SessionReceiverSpecificKey

**Spec:** §10.5.3.3.3, p. 322 — "SessionReceiverSpecificKey :=
HMAC256(MasterReceiverSpecificKey, 'SessionReceiverKey' | MasterSalt |
SessionId)".

**Repo:** `crates/security-crypto/src/session_key.rs::derive_session_receiver_key_cyclone`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_receiver_specific_mac_is_the_one_of_the_spec` (computed with Python from the formula)

**Status:** `done` — `HMAC256(MasterReceiverSpecificKey, "SessionReceiverKey" | MasterSalt | SessionId)`, cut to the key length, the salt cut to the key length as in `derive_session_key_cyclone`. `derive_session_hmac_key` (salt before the label) is removed.

### §10.5.3.3.4 InitializationVector = SessionId | InitializationVectorSuffix

**Spec:** §10.5.3.3.4, p. 322 — "The encryption operation uses a 96-bit
initialization vector constructed as: InitializationVector = SessionId |
InitializationVectorSuffix [...] It shall be incremented each time any of
those keys are used to encrypt and/or create a MAC."

**Repo:** `crates/security-crypto/src/plugin.rs::seal_cyclone` (l. 278)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::two_encrypts_produce_different_ciphertexts`

**Status:** done — the suffix starts at 1, not at random as in Table 81.

### §10.5.3.3.4.1 to §10.5.3.3.4.3 Formats of CryptoHeader, CryptoContent and CryptoFooter

**Spec:** §10.5.3.3.4.1–§10.5.3.3.4.3, p. 323–324 — CryptoHeader with
transformation_kind, transformation_key_id, session_id, init_vector_suffix;
CryptoContent with `long crypto_content.length = N`; CryptoFooter with
`common_mac` and `receiver_specific_macs.length`, all in Big Endian.

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (l. 1106),
`assemble_secure_submessages` (l. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`,
`crates/security-crypto/src/crypto_transform.rs::tests::footer_round_trip_no_receivers`

**Status:** done — the sentence that N is always a multiple of 16 does not
hold for AES-GCM (contradiction in the specification); ZeroDDS pads SEC_BODY to 4 octets.

### §10.5.3.3.4.4 Result of encode_serialized_payload

**Spec:** §10.5.3.3.4.4, p. 324–325 — authentication only: CryptoHeader,
unchanged SerializedPayload, CryptoFooter; encryption: CryptoHeader,
CryptoContent, CryptoFooter; "The common_mac in the CryptoFooter is the
authentication tag generated by the same AES-GCM where the Additional
Authenticated Data is empty."

**Repo:** `crates/security-crypto/src/plugin.rs::encode_payload_from_mat`
(l. 918)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::serialized_payload_cross_plugin_via_tokens`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** done

### §10.5.3.3.4.5 Result of encode_datawriter_submessage and encode_datareader_submessage

**Spec:** §10.5.3.3.4.5, p. 325–326 — authentication only: SEC_PREFIX,
unchanged submessage, SEC_POSTFIX; encryption: SEC_PREFIX, SEC_BODY,
SEC_POSTFIX; AAD empty.

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(l. 1116), `seal_cyclone` (l. 278)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** done

### §10.5.3.3.4.6.1 and §10.5.3.3.4.6.3 encode_rtps_message with AAD

**Spec:** §10.5.3.3.4.6.1, §10.5.3.3.4.6.3, p. 326–328 — "SRTPS_PREFIX |
(flags) A E [...] The common_mac in the CryptoFooter is the authentication tag
generated by the AES-GMAC transformation [...] where the Additional
Authenticated Data is set to the RTPS Header and RTPS HeaderExtension."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; called by the gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::an_rtps_message_goes_through_the_interface_with_aad_and_a_header_extension`

**Status:** `done` — GMAC and GCM with header and HeaderExtension as AAD. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.4.6.2 and §10.5.3.3.4.6.4 encode_rtps_message without AAD

**Spec:** §10.5.3.3.4.6.2, §10.5.3.3.4.6.4, p. 327–329 — RTPS header
unchanged, SRTPS_PREFIX with CryptoHeader, for GMAC
RTPSMessage{RTPSHdr -> InfoSourceSubMsg} in plaintext, for GCM SEC_BODY with
the encrypted content, SRTPS_POSTFIX; "If Additional Authenticated Data
(AAD)' is not enabled the input RTPS Message cannot contain an RTPS Header
Extension."

**Repo:** `crates/security-crypto/src/plugin.rs::encode_rtps_message_cyclone`
(l. 1655), `assemble_srtps_message` (l. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`,
`crates/security-runtime/src/shared.rs::tests::transform_outbound_for_sign_also_uses_srtps_encoder`

**Status:** done

### §10.5.3.3.5 Computation of plaintext from ciphertext

**Spec:** §10.5.3.3.5, p. 329 — "The decrypt operation first checks that the
CryptoTransformIdentifier attribute in the CryptoHeader has the proper
transformation_kind and also uses the CryptoTransformIdentifier
transformation_key_id to locate the MasterKey, and MasterSalt. [...] If the
MasterKeyId is not found associated with the crypto handle the operation
shall fail. The session_id attribute within the CryptoHeader is used to
obtain the proper SessionReceiverSpecificKeys and SessionKey."

**Repo:** `crates/security-crypto/src/plugin.rs::open_cyclone`; `crates/security-crypto/src/plugin/spec.rs`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_kind_other_than_the_expected_one_is_refused`, `crates/security-crypto/src/plugin.rs::tests::a_header_kind_other_than_the_keys_is_refused`

**Status:** `done` — The kind in the header is compared with the expected one and with that of the material. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.6 Computation of the message authentication codes

**Spec:** §10.5.3.3.6, p. 329–330 — "There are two types of message
authentication codes (MACs) that may appear. The first stored in the
common_mac uses the SessionKey. [...] The second type, stored in the
receiver_specific_macs contains MACs that use different
SessionReceiverSpecificKey".

**Repo:** `crates/security-crypto/src/plugin.rs::seal_cyclone`, `footer_tail`, `receiver_mac`, `check_receiver_mac`, `open_submessage`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_receiver_specific_mac_is_the_one_of_the_spec`, `::a_reader_checks_the_origin_of_a_submessage_by_its_own_key`, `::decrypt_rejects_tampered_ciphertext`

**Status:** `done` — `common_mac` under the SessionKey; the MACs for single receivers are the AES-GMAC on the `common_mac` under the SessionReceiverSpecificKey with the IV of the header. A receiver with a key of its own requires its MAC and checks it.

---

### §10.6 Builtin Logging Plugin: DDS:Logging:DDS_LogTopic

**Spec:** §10.6, p. 330 — "The builtin Logging Plugin is known as the
DDS:Logging:DDS_LogTopic. The DDS:Logging:DDS_LogTopic implements logging by
publishing information to a DDS Topic BuiltinLoggingTopic defined below."

**Repo:** `crates/security-logging/src/log_topic.rs::LogTopicLoggingPlugin`
(l. 554), `plugin_class_id` (l. 728)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::plugin_class_id_is_the_spec_name`

**Status:** done

### §10.6 BuiltinLoggingTopic: name "DDS:Security:LogTopicV2", type BuiltinLoggingTypeV2

**Spec:** §10.6, p. 330 — "The BuiltinLoggingTopic shall have the Topic name
'DDS:Security:LogTopicV2'. The BuiltinLoggingTopic shall have the Type
BuiltinLoggingTypeV2 defined in the IDL below."

**Repo:** `crates/security/src/logging.rs::TOPIC_LOG_TOPIC_V2` (l. 42);
`crates/dcps/src/security_log_topic.rs::log_topic_publisher_for_participant`
(l. 344), `DdsType for BuiltinLoggingTypeV2` (l. 122, `TYPE_NAME`)

**Tests:** `crates/dcps/tests/security_log_topic_e2e.rs::a_security_event_reaches_the_v2_log_topic`,
`crates/dcps/tests/security_log_topic_e2e.rs::the_topic_names_are_the_clause_names`,
`crates/dcps/src/security_log_topic.rs::tests::the_type_names_are_the_idl_names`

**Status:** done

### §10.6 Topic "DDS:Security:LogTopic" of the earlier version optional, two DataWriters

**Spec:** §10.6, p. 330 — "Implementors of DDS-Security 1.2 may optionally
provide mechanisms that configure a DomainParticipant to publish the legacy
BuiltinLoggingTopic, that is, topic name 'DDS:Security:LogTopic' and type
'BuiltinLoggingType'. [...] Note that publishing both Topics requires two
separate DataWriters."

**Repo:** `crates/security/src/logging.rs::TOPIC_LOG_TOPIC` (l. 47);
`crates/dcps/src/security_log_topic.rs::log_topic_publisher_for_participant`
(l. 344, second DataWriter with `publish_legacy`);
`crates/security-logging/src/from_properties.rs::PROP_LOG_BUILTIN_LEGACY_TOPIC`
(l. 76)

**Tests:** `crates/dcps/tests/security_log_topic_e2e.rs::the_legacy_topic_is_published_when_configured`,
`crates/security-logging/src/log_topic.rs::tests::legacy_topic_is_published_when_asked`

**Status:** done — switched on through the property
`dds.sec.log.builtin.DDS_LogTopic.legacy_topic`, a name in the reserved
`dds.sec.` space (see §7.3.2).

### §10.6 Application DataReader for LogTopicV2

**Spec:** §10.6, p. 330 — "Users of DDS-Security 1.2 that want to receive the
secure log messages shall create a DataReader for topic
'DDS:Security:LogTopicV2' with type BuiltinLoggingTypeV2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — a requirement on applications, not on the
implementation.

### §10.6 enum LoggingLevel

**Spec:** §10.6, p. 330 — "enum LoggingLevel { EMERGENCY_LEVEL, ALERT_LEVEL,
CRITICAL_LEVEL, ERROR_LEVEL, WARNING_LEVEL, NOTICE_LEVEL, INFORMATIONAL_LEVEL,
DEBUG_LEVEL };"

**Repo:** `crates/security-logging/src/log_topic.rs::LoggingLevel` (l. 59);
`crates/dcps/src/security_log_topic.rs::severity_from_u32` (l. 50)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::an_unknown_severity_is_refused_not_guessed`,
`crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_writes_the_idl_octets`

**Status:** done — values 0 to 7 in IDL order, 32 bits on the wire.
§9.6.2.1.1 names other levels (FATAL_LEVEL to TRACE_LEVEL; contradiction in the specification).

### §10.6 NameValuePair, NameValuePairSeq and structured_data

**Spec:** §10.6, pp. 330–331 — "@extensibility(FINAL) struct NameValuePair {
string name; string value; }; typedef sequence<NameValuePair>
NameValuePairSeq;" and "map<string, NameValuePairSeq> structured_data;"

**Repo:** `crates/security-logging/src/log_topic.rs::NameValuePair` (l. 199),
`NameValuePairSeq` (l. 207);
`crates/dcps/src/security_log_topic.rs::write_structured_data` (l. 85)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_writes_the_idl_octets`,
`crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_round_trips`

**Status:** done — map and sequence each carry a DHEADER, the NameValuePair
elements carry none.

### §10.6 LegacyTime_t and Time_t

**Spec:** §10.6, pp. 330–331 — "struct LegacyTime_t { long sec; unsigned long
nanosec; }; struct Time_t { long long sec; unsigned long nanosec; };"

**Repo:** `crates/security-logging/src/log_topic.rs::LegacyTime` (l. 214),
`Time` (l. 226)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::the_legacy_codec_round_trips_with_a_32_bit_second`,
`crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_writes_the_idl_octets`

**Status:** done

### §10.6 BuiltinLoggingType (@appendable)

**Spec:** §10.6, p. 331 — "@extensibility(APPENDABLE) struct
BuiltinLoggingType { octet facility; LoggingLevel severity; LegacyTime_t
timestamp; string hostname; string hostip; string appname; string procid;
string msgid; string message; map<string, NameValuePairSeq> structured_data;
};"

**Repo:** `crates/security-logging/src/log_topic.rs::BuiltinLoggingType`
(l. 271); `crates/dcps/src/security_log_topic.rs::DdsType for BuiltinLoggingType`
(l. 179)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::the_legacy_codec_round_trips_with_a_32_bit_second`,
`crates/dcps/tests/security_log_topic_e2e.rs::the_legacy_topic_is_published_when_configured`

**Status:** done

### §10.6 BuiltinLoggingTypeV2 (@appendable)

**Spec:** §10.6, p. 331 — "@extensibility(APPENDABLE) struct
BuiltinLoggingTypeV2 { octet facility; LoggingLevel severity; Time_t
timestamp; string hostname; [...] map<string, NameValuePairSeq>
structured_data; };"

**Repo:** `crates/security-logging/src/log_topic.rs::BuiltinLoggingTypeV2`
(l. 297); `crates/dcps/src/security_log_topic.rs::DdsType for BuiltinLoggingTypeV2`
(l. 122, XCDR2 with DHEADER)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_writes_the_idl_octets`,
`crates/dcps/src/security_log_topic.rs::tests::a_reader_skips_unknown_trailing_member_octets`

**Status:** done — the reference value of the test is hand-built in the same
repository; no comparison with bytes from other vendors exists.

### §10.6 Filling the fields: facility 0x0A, timestamp, hostname to message

**Spec:** §10.6, p. 331 — "octet facility; // Set to 0x0A (10). Indicates
sec/auth msgs [...] Time_t timestamp; // Since epoch 1970-01-01 00:00:00
+0000 (UTC) string hostname; // IP host name of originator string hostip; //
IP address of originator string appname; // Identify the device or
application string procid; // Process name/ID for syslog system string msgid;
// Identify the type of message string message; // Free-form message".

**Repo:** `crates/security-logging/src/log_topic.rs::FACILITY_SEC_AUTH`
(l. 51), `BuiltinLoggingTypeV2::from_log_event` (l. 407),
`BuiltinLoggingType::from_log_event` (l. 433), `Time::now` (l. 236)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::an_event_fills_every_field_from_the_clause`,
`crates/security-logging/src/log_topic.rs::tests::legacy_topic_is_published_when_asked`

**Status:** done — `hostname` from `HOSTNAME`/`COMPUTERNAME`, `hostip` from
`ZERODDS_HOSTIP`, else empty; `msgid` is the `category` argument.

### §10.6 Builtin topic rule in the DDS:Access:Permissions plugin

**Spec:** §10.6, p. 331 — "Knowledge of the BuiltinLoggingTopic shall be
builtin into the DDS:Access:Permissions AccessControl plugin and it shall be
treated according to the following topic rule: <topic_rule>
<topic_expression>DDS:Security:LogTopic</topic_expression>
<enable_discovery_protection>FALSE</enable_discovery_protection>
<enable_read_access_control>TRUE</enable_read_access_control>
<enable_write_access_control>FALSE</enable_write_access_control>
<metadata_protection_kind>SIGN</metadata_protection_kind>
<data_protection_kind>ENCRYPT</data_protection_kind> </topic_rule>"

**Repo:** `crates/security-permissions/src/governance.rs::builtin_topic_rule`
(l. 115), `effective_topic_rule` (l. 392, the builtin rule takes precedence
over the governance document)

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_builtin_log_topic_rule_allows_any_write_and_gates_reads`

**Status:** done — the rule applies to both names. The text gives only the
name of the earlier version's topic as `topic_expression` (contradiction in the specification).

### §10.6 Writing without a grant, reading only with a grant

**Spec:** §10.6, p. 331 — "The above rule states that any DomainParticipant
with permission necessary to join the DDS Domain shall be allowed to write the
BuiltinLoggingTopic but in order to read the BuiltinLoggingTopic the
DomainParticipant needs to have a grant for the BuiltinLoggingTopic in its
permissions document."

**Repo:** `crates/security-permissions/src/governance.rs::check_create_datawriter`, `check_create_datareader`; `crates/security-runtime/src/shared.rs::check_remote_datawriter`, `check_remote_datareader`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_builtin_log_topic_rule_allows_any_write_and_gates_reads`,
`crates/security-runtime/src/shared.rs::tests::a_log_topic_is_written_without_a_grant_and_read_only_with_one`

**Status:** `done` — local and remote endpoints, both names of the log topic.
### §10.6 Protection of the DDS:Security:LogTopicV2 endpoints by the builtin rule

**Spec:** §10.6, p. 331 — "<metadata_protection_kind>SIGN</metadata_protection_kind>
<data_protection_kind>ENCRYPT</data_protection_kind>".

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::endpoint_protection_kinds`, `topic_*_protected` (via `effective_topic_rule`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_endpoints_of_the_log_topics_are_protected_by_the_builtin_rule`,
`crates/security-runtime/src/shared.rs::tests::the_builtin_log_topic_rule_carries_its_protection`

**Status:** `done` — key material, announced mask and read/write protection of the endpoints of both log topics follow the builtin rule (SIGN, ENCRYPT, read with a grant).
### §10.6.1 Table 82 — set_log_options: DataWriter only with distribute

**Spec:** §10.6.1, Table 82, p. 332 — "If the distribute parameter is set to
TRUE, the DDS:Logging:DDS_LogTopic shall create a DataWriter to send the
BuiltinLoggingTopic if it is FALSE, it shall not."

**Repo:** `crates/dcps/src/participant.rs` (l. 638–642, l. 758:
`log_topic_publisher_for_participant` only with `distribute`);
`crates/security-logging/src/from_properties.rs::logging_plugin_from_properties_with_publisher`
(l. 259)

**Tests:** `crates/dcps/tests/security_log_properties_e2e.rs::builtin_sink_with_distribute_off_is_file_only`,
`crates/security-logging/src/from_properties.rs::tests::distribute_off_needs_no_publisher`,
`crates/security-logging/src/log_topic.rs::tests::distribute_off_publishes_nothing`

**Status:** done — the participant creates the DataWriter when it is created,
not `LogTopicLoggingPlugin::set_log_options`; that method receives the
publisher when the plugin is constructed.

### §10.6.1 Table 82 — set_log_options: open log_file

**Spec:** §10.6.1, Table 82, p. 332 — "The plugin shall open a file with the
name indicated in the log_file parameter."

**Repo:** `crates/security-logging/src/log_topic.rs::set_log_options`
(l. 624), `open_log_file` (l. 183)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::an_unopenable_log_file_is_refused`

**Status:** done

### §10.6.1 Table 82 — set_log_options: store log_level

**Spec:** §10.6.1, Table 82, p. 332 — "The plugin shall remember the value of
the log_level so that it can be used during the log operation."

**Repo:** `crates/security-logging/src/log_topic.rs::set_log_options`
(l. 624)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::options_cannot_change_after_logging_is_enabled`,
`crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`

**Status:** done

### §10.6.1 Table 82 — log: no action without enable_logging

**Spec:** §10.6.1, Table 82, p. 332 — "This operation shall check if logging
was enabled by a prior call to enable_logging and if not it shall return
without performing any action."

**Repo:** `crates/security-logging/src/control.rs::LogControl::admit`; `LogTopicLoggingPlugin::log`

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_before_enable_logging_does_nothing`,
`crates/security-logging/src/control.rs::tests::log_before_enable_logging_does_nothing`

**Status:** `done` — before `enable_logging` there is no file line, no publication and no listener call.
### §10.6.1 Table 82 — log: comparison with the stored log_level

**Spec:** §10.6.1, Table 82, p. 332 — "If the log_level parameter value is
greater than the one saved by the set_log_options operation, the operation
shall return without performing any action. If the log_level parameter value
is less than or equal to the one saved, the log operation shall perform two
actions".

**Repo:** `crates/security-logging/src/log_topic.rs::log` (l. 712)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::events_above_the_level_are_dropped_and_at_or_below_pass`,
`crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`

**Status:** done

### §10.6.1 Table 82 — log: append a line to log_file

**Spec:** §10.6.1, Table 82, p. 332 — "It shall append a string
representation of the parameters passed to the log operation to the end of
the file opened by the set_log_options operation."

**Repo:** `crates/security-logging/src/control.rs::file_line`

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::a_field_with_a_newline_stays_on_one_line`

**Status:** done — one line per message; line breaks inside fields are
escaped.

### §10.6.1 Table 82 — log: publishing with distribute

**Spec:** §10.6.1, Table 82, p. 332 — "If the value of the distribute option
was set on the call to set_log_options, the plugin shall fill an object of
type BuiltinLoggingType with the values passed as arguments to the log
operation and publish it using the DataWriter associated with the
BuiltinLoggingTopic created by the set_log_options operation."

**Repo:** `crates/security-logging/src/log_topic.rs::log` (l. 723),
`publish` (l. 677)

**Tests:** `crates/dcps/tests/security_log_topic_e2e.rs::a_security_event_reaches_the_v2_log_topic`,
`crates/security-logging/src/log_topic.rs::tests::distribute_off_publishes_nothing`

**Status:** done — BuiltinLoggingTypeV2 is published on
`DDS:Security:LogTopicV2` as in §10.6, BuiltinLoggingType in addition on
request. Table 82 names BuiltinLoggingType (contradiction in the specification).

### §10.6.1 Table 82 — enable_logging

**Spec:** §10.6.1, Table 82, p. 332 — "This operation shall save the fact
that logging was enabled such that the information can be used by the log
operation."

**Repo:** `crates/security-logging/src/log_topic.rs::enable_logging`
(l. 645)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::enable_without_options_is_refused`

**Status:** done

### §10.6.1 Table 82 — set_listener

**Spec:** §10.6.1, Table 82, p. 332 — "This operation shall save a reference
to the LoggerListener such that the listener is be notified each time a log
message is produced."

**Repo:** `crates/security-logging/src/log_topic.rs::LogTopicLoggingPlugin` (`LoggingPlugin::set_listener`)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::the_listener_is_told_of_each_message_the_sink_logs`,
`crates/security-logging/src/control.rs::tests::the_listener_hears_each_logged_message`

**Status:** `done` — the listener is notified of each logged message.

---

### §10.7 DDS:Auth:PSK: no authentication, discovered participants count as unauthenticated

**Spec:** §10.7, p. 332 — "The DDS:Auth:PSK plugin is mostly a 'NOOP' plugin
that constructs an IdentityHandle with the information the 'DDS:Access:PSK'
and 'DDS:Crypto:PSK need. The DDS:Auth:PSK plugin does not do any
Authentication treating all participants it discovers as 'Unauthenticated'
Participants."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — `DDS:Auth:PSK` authenticates nobody; `validate_remote_identity` always fails. The handshake with keys that carried this name is `PskHandshakeAuthenticationPlugin` with classes of its own (`ZeroDDS:Auth:PSK-Handshake:1.0`).

### §10.7.1 Configuration: only the selection of the plugin

**Spec:** §10.7.1, p. 332 — "This plugin does not require any configuration
beyond selecting it to be used. The mechanism for selecting which plugins are
active is implementation specific."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — `validate_local_identity` needs no property.

### §10.7.2 Unspecified attributes of credentials and tokens

**Spec:** §10.7.2, p. 333 — "Plugin implementations may place data in these
attributes as long as they also include a property attribute that allows the
implementation to unambiguously detect the presence and interpret these
attributes. Attributes that are not understood shall be ignored. Property_t
and BinaryProperty_t names shall comply with the rules defined in 7.3.1 and
7.3.3, respectively."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — the IdentityToken carries the `class_id` only.

### §10.7.2.1 IdentityToken: class_id "DDS:Auth:PSK:1.2"

**Spec:** §10.7.2.1, p. 333 — "The DDS:Auth:PSK plugin shall set the class_id
attribute of the IdentityToken object to 'DDS:Auth:PSK:1.2' no other
attributes are specified or required."

**Repo:** `crates/security-pki/src/psk.rs::SPEC_CLASS_ID`, `PskAuthenticationPlugin::get_identity_token`

**Tests:** `crates/security-pki/src/psk.rs::tests::token_roundtrip_via_data_holder_codec`,
`crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** done — the token of the spec plugin `PskAuthenticationPlugin` carries the `class_id` only and no properties (`the_spec_plugin_authenticates_nobody`); the handshake plugin of ZeroDDS (`PskHandshakeAuthenticationPlugin`) has a class of its own, `ZeroDDS:Auth:PSK-Handshake:1.0`, and carries `dds.psk.identity_id` (see
§10.7.2). Table 52 (§10.3.2.1, p. 228) names the property `dds.psk.algo`
for this token, which §10.7.2.1 does not require (contradiction in the
specification); ZeroDDS does not set it.

### §10.7.2.1 class_id from PluginClassName, MajorVersion and MinorVersion

**Spec:** §10.7.2.1, p. 333 — "The value of the class_id shall be interpreted
as composed of three parts: a PluginClassName, a MajorVersion and a
MinorVersion [...] this version of the specification has PluginClassName
equal to 'DDS:Auth:PSK', MajorVersion set to 1, and MinorVersion set to 2."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`, `crates/security-pki/src/psk.rs::tests::the_handshake_classes_are_not_the_spec_class`

**Status:** `done` — the plugin does not evaluate the class of a remote token; `validate_remote_identity` fails for every token.

### §10.7.2.2 to §10.7.2.4 IdentityStatusToken, AuthenticatedPeerCredentialToken, AuthRequestMessageToken

**Spec:** §10.7.2.2 to §10.7.2.4, p. 333 — "The DDS:Auth:PSK plugin does not
use this Token. There is no value specified for it."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no value specified; the return value of
`get_identity_status_token` is covered under Table 83.

### §10.7.2.5 HandshakeMessageToken: not used

**Spec:** §10.7.2.5, §10.7.2.5.1 to §10.7.2.5.3, p. 333 — "There is no value
specified for it. [...] HandshakeRequestMessageToken objects The
DDS:Auth:PSK plugin does not use this Token. [...] HandshakeReplyMessageToken
The DDS:Auth:PSK plugin does not use this Token."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — the plugin knows no handshake tokens; the handshake operations fail.

### §10.7.3 Table 83 — validate_local_identity

**Spec:** §10.7.3, Table 83, p. 334 — "This operation shall receive the
participant_guid associated with the local DomainParticipant whose identity is
being validated. The operation shall always return VALIDATION_OK. The
operation shall set the output 16-byte adjusted_participant_guid GUID to the
same value as the input participant_guid."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — `validate_local_identity` always returns a handle (VALIDATION_OK); the GUID stays as it is.

### §10.7.3 Table 83 — get_identity_token

**Spec:** §10.7.3, Table 83, p. 334 — "This operation returns the Token
specified in 10.7.2.1".

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — token of the class `DDS:Auth:PSK:1.2` with no other attribute.

### §10.7.3 Table 83 — get_identity_status_token

**Spec:** §10.7.3, Table 83, p. 334 — "This operation shall return TokenNIL."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — TokenNIL (empty).

### §10.7.3 Table 83 — set_participant_security_config

**Spec:** §10.7.3, Table 83, p. 334 — "This operation shall do nothing and
return TRUE."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin::set_participant_security_config`

**Tests:** `crates/security-pki/src/psk.rs::tests::table_83_set_participant_security_config_and_credential_do_nothing`

**Status:** `done` — does nothing and returns the given info (TRUE).

### §10.7.3 Table 83 — set_permissions_credential_and_token

**Spec:** §10.7.3, Table 83, p. 334 — "This operation shall do nothing and
return TRUE."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin::set_permissions_credential_and_token`

**Tests:** `crates/security-pki/src/psk.rs::tests::table_83_set_participant_security_config_and_credential_do_nothing`

**Status:** `done` — does nothing and returns TRUE, for empty or arbitrary tokens too.

### §10.7.3 Table 83 — validate_remote_identity: always VALIDATION_FAILED

**Spec:** §10.7.3, Table 83, p. 334 — "The operation shall always return
VALIDATION_FAILED."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — always refused (`AuthenticationFailed`).

### §10.7.3 Table 83 — the handshake operations are not called

**Spec:** §10.7.3, Table 83, pp. 334–335 — begin_handshake_request,
begin_handshake_reply, process_handshake, get_shared_secret,
get_authenticated_peer_credential_token, return_authenticated_peer_credential_token,
return_handshake_handle, return_sharedsecret_handle: "This operation does not
need to be implemented. It will not be called on the plugin given that
validate_remote_identity always returns VALIDATION_FAILED."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — `begin_handshake_request`, `begin_handshake_reply`, `process_handshake` and `shared_secret` fail.

### §10.7.3 Table 83 — set_listener

**Spec:** §10.7.3, Table 83, p. 334 — "This operation shall save a reference
to the listener object and associate it with the specified IdentityHandle."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin::set_listener`

**Tests:** `crates/security-pki/src/psk.rs::tests::table_83_set_listener_saves_the_listener`

**Status:** `done` — saves the listener (`None` removes it); it is never called, because the plugin authenticates nothing and revokes nothing.

### §10.7.3 Table 83 — return_identity_token, return_identity_status_token, return_identity_handle

**Spec:** §10.7.3, Table 83, pp. 334–335 — "This operation shall behave as
specified in 9.3.2.11.14." (return_identity_token), "[...] 9.3.2.11.15."
(return_identity_status_token), "[...] 9.3.2.11.18." (return_identity_handle).

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin::return_identity_token`, `return_identity_status_token`, `return_identity_handle`

**Tests:** `crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`

**Status:** `done` — behave as §9.3.2.11.14, .15 and .18: only what was issued goes back, once; TokenNIL (the status) is taken back as an empty token.

---

### §10.8 DDS:Access:PSK: all permissions

**Spec:** §10.8, p. 335 — "The plugin implements the AccessControl plugin API
granting all permissions. Specifically: It allows the local application to
join any DDS domain, as well as publish and subscribe to any DDS Topic in that
domain. It allows a remote DomainParticipant to join any DDS domain, as well
as publish and subscribe to any DDS Topic in that domain."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — all checks of Table 85 permit; no document is needed.

### §10.8.1 Table 84 — dds.sec.access.rtps_psk_protection_kind

**Spec:** §10.8.1, Table 84, p. 335 — "rtps_psk_protection_kind One of the
following 3 string options: 'NONE', 'SIGN', or 'ENCRYPT'. [...] If not
specified it is treated as if it was specified to be 'ENCRYPT'. This property
must be configured consistently on all the DomainParticipants that join a DDS
Domain." Prefix `dds.sec.access.`, `propagate` FALSE.

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`::validate_local_permissions, `RtpsPskProtection`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — NONE, SIGN or ENCRYPT, ENCRYPT when absent, another value is refused.

### §10.8.2 and §10.8.2.1 Types, PermissionsCredentialToken

**Spec:** §10.8.2.1, p. 336 — "The DDS:Access:PSK plugin does not interpret
the value of this Token. For this reason the value is implementation
specific."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — no value specified.

### §10.8.2.2 PermissionsToken: class_id "DDS:Access:PSK:1.2"

**Spec:** §10.8.2.2, p. 336 — "The DDS:Access:PSK plugin shall set the
class_id attributes to 'DDS:Access:PSK:1.2'. No other attributes need to be
set."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — class `DDS:Access:PSK:1.2`.

### §10.8.2.3 to §10.8.2.6 PluginParticipantSecurityAttributes, PluginEndpointSecurityAttributes and their masks

**Spec:** §10.8.2.3 to §10.8.2.6, p. 336 — "The
PluginParticipantSecurityAttributes describe plugin-specific behavior of the
associated cryptographic plugin affecting the key material and transformations
for the RTPS messages [...] The PluginEndpointSecurityAttributesMask is used
to encode the value of the PluginEndpointSecurityAttributes".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — description; Table 85 sets the values
(get_participant_security_config, get_datawriter_security_config).

### §10.8.3 Table 85 — check_create_participant, check_remote_participant, check_create_topic

**Spec:** §10.8.3, Table 85, p. 336 — "This operation shall return TRUE."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::the_participant_topic_instance_and_match_checks_return_true`

**Status:** `done` — the three operations return TRUE.

### §10.8.3 Table 85 — check_create_datawriter, check_create_datareader, check_remote_datawriter, check_remote_datareader

**Spec:** §10.8.3, Table 85, p. 336 — "This operation shall return TRUE."

**Repo:** `crates/security-permissions/src/psk_access.rs` (`check_create_datawriter`,
`check_create_datareader`, `check_remote_datawriter_match`,
`check_remote_datareader_match`)

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::every_check_returns_true`

**Status:** `done` — every one of the checks returns TRUE, for topics a grant would
deny too; an unknown handle stays an error.

### §10.8.3 Table 85 — Checks for instances, check_remote_topic, check_local_datawriter_match, check_local_datareader_match

**Spec:** §10.8.3, Table 85, pp. 336–337 — check_local_datawriter_register_instance,
check_local_datawriter_dispose_instance, check_remote_topic,
check_local_datawriter_match, check_local_datareader_match,
check_remote_datawriter_register_instance,
check_remote_datawriter_dispose_instance: "This operation shall return TRUE."

**Repo:** `crates/security-permissions/src/psk_access.rs`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::the_participant_topic_instance_and_match_checks_return_true`

**Status:** `done` — the operations return TRUE.

### §10.8.3 Table 85 — get_permissions_token

**Spec:** §10.8.3, Table 85, p. 337 — "This operation shall return the
PermissionsToken formatted as described in 10.8.2.2."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`::get_permissions_token

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — token per §10.8.2.2.

### §10.8.3 Table 85 — get_permissions_credential_token

**Spec:** §10.8.3, Table 85, p. 337 — "This operation shall return the
PermissionsToken formatted as described in 10.8.2.1".

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`::get_permissions_credential_token

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — token of the same class (the value is implementation specific, §10.8.2.1).

### §10.8.3 Table 85 — set_listener, return_permissions_token, return_permissions_credential_token

**Spec:** §10.8.3, Table 85, p. 337 — "This operation shall save a reference
to the listener object and associate it with the specified
PermissionsHandle." (set_listener), "This operation shall behave as specified
in 9.4.2.9.20" (return_permissions_token), "[...] 9.4.2.9.21"
(return_permissions_credential_token).

**Repo:** `crates/security-permissions/src/psk_access.rs`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — `set_listener` keeps the reference; the returns take only tokens that were issued.

### §10.8.3 Table 85 — validate_local_permissions: always succeeds

**Spec:** §10.8.3, Table 85, p. 337 — "This operation shall succeedand return
an opaque handle that the plugin can use to refer to any
implementation-specific saved information."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — always succeeds, without a document.

### §10.8.3 Table 85 — validate_remote_permissions: always succeeds

**Spec:** §10.8.3, Table 85, p. 337 — "This operation shall succeedand return
an opaque handle that the plugin can use to refer to any
implementation-specific saved information."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — always succeeds, token and credential arbitrary.

### §10.8.3 Table 85 — get_participant_security_config

**Spec:** §10.8.3, Table 85, pp. 337–338 — "The field
allow_unauthenticated_participants shall be set to TRUE. The field
is_access_protected shall be set to FALSE. The field is_rtps_axk_protected
shall be set to FALSE. The field is_rtps_psk_protected shall be set to TRUE
if the DomanParticipant was configured with the PropertyQos property
dds.sec.access.rtps_psk_protection_kind set to 'ENCRYPT' or 'SIGN'. [...] The
field plugin_participant_mask shall have the
PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_ENCRYPTED set if and
only if [...] 'ENCRYPT' [...] All the 'supported_mask' nested fields [...]
CRYPTO_ALGORITHM_SET_ALL [...] All the 'required_mask' nested fields [...]
CRYPTO_ALGORITHM_SET_EMPTY".

**Repo:** `crates/security-permissions/src/psk_access.rs::get_participant_security_config`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::the_participant_config_is_the_one_of_table_85`

**Status:** `done` — the config is that of Table 85.

### §10.8.3 Table 85 — get_topic_security_config, get_datawriter_security_config, get_datareader_security_config

**Spec:** §10.8.3, Table 85, p. 338 — "The fields of the TopicSecurityConfig
attributes shall all be set to FALSE." and for writers and readers "The
boolean fields [...] shall all be set to FALSE. The plugin_endpoint_attributes
shall be set to the empty mask. [...] supported_mask [...]
CRYPTO_ALGORITHM_SET_ALL [...] required_mask [...] CRYPTO_ALGORITHM_SET_EMPTY".

**Repo:** `crates/security-permissions/src/psk_access.rs`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::the_topic_and_endpoint_configs_are_all_false`

**Status:** `done` — all values are FALSE.

### §10.8.3 Table 85 — return_participant_security_config to return_datareader_security_config

**Spec:** §10.8.3, Table 85, p. 338 — "This operation shall behave as
specified in 9.4.2.9.26" to "[...] 9.4.2.9.29."

**Repo:** `crates/security-permissions/src/psk_access.rs`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — the four returns take only configs that were issued.

---

### §10.9 DDS:Crypto:PSK: AEAD with AES-GCM/GMAC as in §10.5

**Spec:** §10.9, pp. 338–339 — "This plugin does Authenticated Encryption with
Associated Data (AEAD) using Advanced Encryption Standard with Galois Counter
Mode (AES-GCM/GMAC), see 8.1 for more details. The algorithms used are the
same described in 10.5."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin`; `crates/security-runtime/src/psk_gate.rs`; `crates/dcps/src/runtime.rs::secure_inbound_bytes`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_transform_of_table_89`, `crates/security-runtime/src/psk_gate.rs::tests::the_gate_protects_a_message_with_the_pre_shared_key`, `crates/dcps/src/runtime.rs::tests::a_pre_shared_key_protects_the_bootstrapping_messages`

**Status:** `done` — With a passphrase set the runtime protects whole messages (bootstrap with flags A and P) and refuses unprotected ones. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` and `-p zerodds-dcps --features security --lib` (7 tests), log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §10.9.1 Table 86 — rtps_psk_symmetric_cipher_algorithm

**Spec:** §10.9.1, Table 86, p. 339 — "The string 'AUTO' or one of the
CryptoAlgorithmName strings shown in Table 22 [...] If not specified it is
treated as if it was specified to be 'AUTO'. If 'AUTO' is specified it is
treated as if it was specified to be 'AES256-GCM'. This property must be
configured consistently on all the DomainParticipants that join the same DDS
Domain." Prefix `dds.sec.crypto.`.

**Repo:** `crates/security-crypto/src/psk_rtps.rs::key_bits_from_cipher`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_passphrase_rules_of_table_86`

**Status:** `done` — AUTO is AES256-GCM; the names of Table 22 select the width.

### §10.9.1 Table 86 — rtps_psk_secret_passphrase

**Spec:** §10.9.1, Table 86, pp. 339–340 — "Specifying this property enables
pre-shared-key (PSK) protection. [...] (the presence of this property is
mandatory) The property specifies the URI to access the passphrase_id and
passphrase [...] The passphrase_id shall be a number between 0 and 2^32-1
represented as a decimal string. [...] The range of passphrase_id that verify
passphrase_id && 0xFF== 0xFF is reserved and shall not be used. The
passphrase shall contain up to 512 ASCII printable characters [...] Supported
URI schemas are: 'file' and 'data'."

**Repo:** `crates/security-runtime/src/qos_properties.rs::psk_crypto_config_from_properties`, `crates/security-crypto/src/psk_rtps.rs::Passphrase`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_psk_crypto_properties_of_table_86_are_read`, `crates/security-crypto/src/psk_rtps.rs::tests::the_passphrase_rules_of_table_86`

**Status:** `done` — mandatory; `file:` and `data:`; id up to 2^32-1 without 0xFF as the low octet; passphrase of up to 512 printable characters without a space at either end.

### §10.9.1 Table 86 — rtps_psk_secret_passphrase_alt

**Spec:** §10.9.1, Table 86, p. 340 — "URI to access a list of additional
passphrase_id and passphase values that are also accepted during decoding.
[...] each secret passphrase_id, and passphrase tuple shall be separated from
the next using the LineFeed (\n, character 10), the CarryReturn (\r, character
13), or both."

**Repo:** `crates/security-runtime/src/qos_properties.rs::psk_crypto_config_from_properties`, `crates/security-crypto/src/psk_rtps.rs::parse_passphrase_list`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_psk_crypto_properties_of_table_86_are_read`, `crates/security-crypto/src/psk_rtps.rs::tests::an_alternative_passphrase_decodes`

**Status:** `done` — list separated by LF, CR or both; accepted in decoding.

### §10.9.2 and §10.9.2.1 CryptoToken as for DDS:Crypto:AES-GCM-GMAC

**Spec:** §10.9.2.1, p. 340 — "This type is defined the same way as the
DDS:Crypto:AES-GCM-GMAC CryptoToken, see 10.5.2.1. This type is not strictly
needed as the pre-shared Keys are not sent over the network."

**Repo:** `crates/security-crypto/src/psk_rtps.rs`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_exchange_of_table_88`, `crates/security-crypto/src/psk_rtps.rs::tests::the_key_material_is_the_one_the_spec_prints`

**Status:** `done` — The key material is derived, no token is sent (Table 88: FALSE); 16 octets of salt for AES128. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.2.1 Key material from the properties by §10.5.2.1.3

**Spec:** §10.9.2.1, p. 341 — "The Key Material used by the plugin shall be
derived from the DomainParticipant configuration properties defined in 10.9.1
using the same algorithm described in 10.5.2.1.3."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::derive_key_material`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_material_is_the_one_the_spec_prints`

**Status:** `done` — per §10.5.2.1.3.

### §10.9.2.2 to §10.9.2.5 CryptoTransformIdentifier, CryptoHeader, CryptoContent, CryptoFooter

**Spec:** §10.9.2.2 to §10.9.2.5, p. 341 — "This type is defined the same
wasy as the DDS:Crypto:AES-GCM-GMAC CryptoTransformIdentifier, see 10.5.2.2."
(likewise for CryptoHeader, CryptoContent, CryptoFooter).

**Repo:** `crates/security-crypto/src/psk_plugin.rs::encrypt_submessage`
(l. 344), `decrypt_submessage` (l. 355)

**Tests:** `crates/security-crypto/src/psk_plugin.rs::tests::transform_kind_id_aes128_matches_x509_path`,
`crates/security-crypto/src/psk_plugin.rs::tests::transform_kind_id_aes256_matches_x509_path`,
`crates/security-crypto/src/psk_plugin.rs::tests::psk_encrypt_decrypt_roundtrip_two_plugins_same_psk`

**Status:** done — the formats are those of the embedded
`AesGcmCryptoPlugin`; its deviations (§10.5) apply here as well.

### §10.9.3 Three interfaces

**Spec:** §10.9.3, p. 341 — "This plugin implements three interfaces:
CryptoKeyFactory, CryptoKeyExchange, and CryptoTransform. Each is described
separately."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — structure; the operations follow in
Table 87 to Table 89.

### §10.9.3.1 Table 87 — register_local_participant: ParticipantKeyMaterial

**Spec:** §10.9.3.1, Table 87, p. 341 — "This operation shall create a new
KeyMaterial_AES_GCM_GMAC object and return a handle that the plugin can use to
access the created object. We will refer to this object by the name:
ParticipantKeyMaterial."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::register_local_participant_for`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_message_goes_through_encode_and_decode`

**Status:** `done` — ParticipantKeyMaterial from the key material of the passphrase.

### §10.9.3.1 Table 87 — register_local_participant: GMAC or GCM by RTPS Protection Kind, adjusted_algorithm_info

**Spec:** §10.9.3.1, Table 87, pp. 341–342 — "The selection between these two
options shall be done according to the setting of the RTPS Protection Kind
(see 10.4.1.2.5.7). [...] The operation shall store in the internal state of
the plugin the value for
particiant_security_config.algorithm_info.symmetric_cipher.supported_mask.
This operation shall fill the adjusted_algorithm_info output parameter as
follows: [...] symmetric_cipher.supported_mask [...] the algorithm configured
using the property rtps_psk_symmetric_cipher_algorithm [...] required_mask
[...] the same value [...] builtin_kx_endpoints_required_mask [...]
CRYPTO_ALGORITHM_SET_EMPTY [...] builtin_endpoints_required_mask [...]
CRYPTO_ALGORITHM_SET_EMPTY. [...] The operation shall configure the Crypto
plugins to only accept the resulting set of supported algorithms".

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyFactory`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_factory_of_table_87`

**Status:** `done` — `adjusted_algorithm_info`: `supported_mask` of the property, builtin masks empty, rest zero; the `supported_mask` of the config is stored. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.1 Table 87 — Registration of remote participants and of endpoints, revise_local_entity_keys: empty handle

**Spec:** §10.9.3.1, Table 87, p. 342 — register_matched_remote_participant,
register_local_datawriter, register_matched_remote_datareader,
register_local_datareader, register_matched_remote_datawriter,
revise_local_entity_keys: "This operation shall do nothing and return dummy
Handle."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyFactory`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_factory_of_table_87`

**Status:** `done` — The registrations return a dummy handle (not HandleNIL), `revise_local_entity_keys` the revision 0. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.1 Table 87 — activate_key_revision: FALSE

**Spec:** §10.9.3.1, Table 87, p. 342 — "This operation shall do nothing and
return FALSE."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyFactory`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_factory_of_table_87`

**Status:** `done` — Does nothing and returns FALSE. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.1 Table 87 — unregister_participant, unregister_datawriter, unregister_datareader

**Spec:** §10.9.3.1, Table 87, p. 342 — "Releases any resources allocated on
the corresponding call to register_local_participant, or
register_matched_remote_participant." (likewise for writers and readers).

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyFactory`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_factory_of_table_87`

**Status:** `done` — As operations of the specification. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.2 Table 88 — create_local_participant_crypto_tokens to set_remote_datareader_crypto_tokens: FALSE

**Spec:** §10.9.3.2, Table 88, pp. 342–343 —
create_local_participant_crypto_tokens, set_remote_participant_crypto_tokens,
create_local_datawriter_crypto_tokens, set_remote_datawriter_crypto_tokens,
create_local_datareader_crypto_tokens, set_remote_datareader_crypto_tokens:
"This operation shall do nothing and return FALSE."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin` (CryptographicPlugin)

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_other_operations_do_nothing`

**Status:** `done` — `create_*` and `set_remote_*_crypto_tokens` do nothing and fail (`NotImplemented`, the return value FALSE in the interface).

### §10.9.3.2 Table 88 — return_crypto_tokens

**Spec:** §10.9.3.2, Table 88, p. 343 — "Releases the resources associated
with the CryptoToken objects in the sequence."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyExchange`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_exchange_of_table_88`

**Status:** `done` — Releases tokens; the plugin issues none. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.3.1 Table 89 — encode_serialized_payload, encode_datawriter_submessage, encode_datareader_submessage: FALSE

**Spec:** §10.9.3.3.1, Table 89, p. 343 — "This operation shall do nothing
and return FALSE."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin` (CryptographicPlugin)

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_other_operations_do_nothing`

**Status:** `done` — `encode_serialized_payload`, the submessage operations and `decode_serialized_payload` do nothing and fail.

### §10.9.3.3.1 Table 89 — encode_rtps_message: transform_with_psk and empty receiver_specific_macs

**Spec:** §10.9.3.3.1, Table 89, p. 343 — "The operation checks that the
parameter transform_with_psk=TRUE. If this is not the case the operation
shall fail and return FALSE. The operation checks that the parameter
receiver_specific_macs contains an empty list. If this is not the case the
operation shall fail and return FALSE. The transformation uses the
ParticipantKeyMaterial associated with the sending_participant_crypto."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::encode_rtps_message`, `crates/security-crypto/src/plugin.rs::PskRtpsKey`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_message_goes_through_encode_and_decode`

**Status:** `done` — `transform_with_psk` is the operation `encode_rtps_message_psk`; empty `receiver_specific_macs`: the plugin writes none.

### §10.9.3.3.1 Table 89 — encode_rtps_message with encryption

**Spec:** §10.9.3.3.1, Table 89, p. 343 — "If the transformation_kind
indicates that encryption is performed, then the output shall be the original
RTPS Header and (if present) the (adjusted) HeaderExtension [...] plus three
RTPS Submessages: SecureRTPSPrefixSubMsg, SecureBodySubMsg, and
SecureRTPSPostfixSubMsg. The SecureRTPSPrefixSubMsg flag
AdditionalAuthenticatedDataFlag shall be set. The SecureRTPSPrefixSubMsg flag
PreSharedKeyFlag shall be set. [...] The SecureRTPSPostfixSubMsg shall contain
the authentication tags computed on the SecureBodySubMsg with both the RTPS
Header and (if present) the (adjusted) HeaderExtension as AAD".

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::encode_rtps_message`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_message_goes_through_encode_and_decode`

**Status:** `done` — RTPS Header, SRTPS_PREFIX with A and P, SEC_BODY, SRTPS_POSTFIX; tag over the body with header and HeaderExtension as AAD.

### §10.9.3.3.1 Table 89 — encode_rtps_message with authentication only

**Spec:** §10.9.3.3.1, Table 89, pp. 343–344 — "If the transformation_kind
indicates that only authentication is performed then the output shall be: the
original RTPS Header and (if present) the (adjusted) Header Extension [...]
followed by the SecureRTPSPostfixSubMsg, RTPSMessage{Body}, and
SecureRTPSPostfixSubMsg. [...] The common_mac shall be computed using the
ParticipantKeyMaterial associated with the sending_participant_crypto."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::encode_rtps_message`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_message_goes_through_encode_and_decode`

**Status:** `done` — GMAC: the body stays, the tag covers AAD and body.

### §10.9.3.3.1 Table 89 — encode_rtps_message: adjusting the HeaderExtension

**Spec:** §10.9.3.3.1, Table 89, p. 344 — "3.1) The HeaderExtension used as
input to the AAD shall have the messageLength element, if present, set to
zero. 3.2) [...] messageChecksum element, if present, set to zero. 3.3) After
computing the SecureRTPSPrefixSubMsg, SecureBodySubMsg, and
SecureRTPSPostfixSubMsg. The HeaderExtension shall be adjusted setting the
appropriate values of the messageLength and messageChecksum elements".

**Repo:** `crates/security-crypto/src/plugin.rs::adjust_header_extension`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_header_extension_with_a_checksum_is_adjusted_both_ways`, `crates/security-crypto/src/psk_rtps.rs::tests::a_header_extension_is_kept_and_its_length_set`

**Status:** `done` — `messageLength` and `messageChecksum` zero in the AAD, then the values of the encoded message; the decoded message gets those of the decoded message. run `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: green, log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.3.1 Table 89 — decode_rtps_message: expected kind, transform_with_psk, material of the sender

**Spec:** §10.9.3.3.1, Table 89, p. 344 — "Examines the
SecureRTPSPrefixSubMsg to determine the transformation_kind matches the one
the receiving DomainParticipant is expecting [...] If the kind is not the
expected one, the operation shall fail with an exception. The operation checks
that the parameter transform_with_psk=TRUE. [...] Uses content of the RTPS
Header, the pre-shared secret and SenderKeyId to compute (or locate a
previously computed) PSK Key Material associated with the sending Participant
(see 10.5.2.1.3)."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::decode_rtps_message`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::what_does_not_hold_is_refused`, `::an_alternative_passphrase_decodes`

**Status:** `done` — expected kind, key of the sender from the RTPS Header and the passphrase (primary or alternative), domain.

### §10.9.3.3.1 Table 89 — decode_rtps_message: flag A, AAD and HeaderExtension

**Spec:** §10.9.3.3.1, Table 89, p. 344 — "Checks the
SecureRTPSPrefixSubMsg's AdditionalAuthenticatedDataFlag. If this flag is not
set, the decode operation shall fail. If the flag is set, the decode shall
validate the tag present in the SecureRTPSPostfixSubMsg passing the RTPS
Header and (if present) the (adjusted) HeaderExtension as AAD. [...] Upon
success the returned RTPS Message shall match the input to the
encode_rtps_message operation on the DomainParticipant that sent the
message."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::decode_rtps_message`, `crates/security-crypto/src/plugin.rs::parse_psk_wire`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::what_does_not_hold_is_refused`

**Status:** `done` — without the flag A or P the decode fails; header and HeaderExtension (length and checksum zero) are in the AAD.

### §10.9.3.3.1 Table 89 — preprocess_secure_submsg, decode_datawriter_submessage, decode_datareader_submessage, decode_serialized_payload: FALSE

**Spec:** §10.9.3.3.1, Table 89, pp. 344–345 — "This operation shall do
nothing and return FALSE."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin` (CryptographicPlugin)

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_other_operations_do_nothing`

**Status:** `done` — do nothing and fail.

### §11.1 Introduction: PIM and PSM

**Spec:** §11.1, p. 347 — "Clause 9 defines the plugin interfaces in a
programming-language independent manner using UML. [...] The mapping of the
plugin interfaces to specific programming languages is defined by first
defining the interfaces using OMG-IDL version 3.5 with the additional syntax
defined in the DDS-XTYPES specification and subsequently applying the IDL to
language mapping to the target language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — classification; the mappings follow in
§11.2 to §11.7.

### §11.1 IDL types without @extensibility count as APPENDABLE

**Spec:** §11.1, p. 347 — "IDL Types lacking the DDS-XTYPES @extensibility
annotation shall be interpreted as having the extensibility kind APPENDABLE.
This matches the DDS-XTYPES specification implied extensibility of
un-annotated types."

**Repo:** `crates/security/src/token.rs::encode` (l. 357);
`crates/security/src/generic_message.rs::ParticipantGenericMessage::to_cdr`
(l. 138)

**Tests:** `crates/security/src/token.rs::tests::cdr_le_layout_class_id_only`,
`crates/security/src/generic_message.rs::tests::message_data_dataholder_is_inline_not_length_prefixed`

**Status:** done — the specification's types are written as XCDR1, where
APPENDABLE and FINAL are encoded the same way (§7.3.1.2). The types of the
BuiltinLoggingTopic are annotated explicitly (§10.6).

### §11.1 Language bindings C, C++ classic, Java classic, C++ modern, Java modern

**Spec:** §11.1, p. 347 — "For consistency with the DDS specification, the
DDS security specification defines language bindings to each of the language
PSMs specified for DDS, namely: C as derived from the IDL to C mapping, C++
classic, as derived from the IDL to C++ mapping, Java classic, as derived from
the IDL to Java mapping, C++ modern, aligned with the DDS-STDC++
specification, this is derived from the IDL to C++11 mapping, Java modern with
the DDS-JAVA5+ specification".

**Repo:** `crates/security/idl/dds_security.idl`; `crates/security/tests/idl_bindings/`; `crates/security/examples/dds_security_bindings.rs`

**Tests:** `crates/security/tests/dds_security_bindings.rs::c_representation_compiles_and_the_logging_operations_link`,
`crates/security/tests/dds_security_bindings.rs::cpp_classic_representation_compiles_and_the_logging_interface_is_implementable`,
`crates/security/tests/dds_security_bindings.rs::cpp11_representation_compiles_without_a_cpp11_only_feature`,
`crates/security/tests/dds_security_bindings.rs::java_classic_representation_compiles_and_the_logging_interface_is_implementable`,
`crates/security/tests/dds_security_bindings.rs::java_modern_representation_maps_module_dds_to_org_omg_dds`

**Status:** `done` — all five representations are generated from `dds_security.idl` and compiled with the language's compiler; the Logging interface is implemented with the generated signatures. The base types of the ORB mappings and of the DDS-STD-C++ PSM are stand-ins in the tests. Placing a plugin of the application written in C, C++ or Java into a participant is listed in §11.3, §11.4 and §11.5 (Authentication, AccessControl, Cryptographic, Logging, DataTagging).
### §11.2 IDL representation: PIM to PSM rules and dds_security.idl

**Spec:** §11.2, p. 347 — "The PIM to PSM mapping maps the UML interfaces and
classes into IDL interfaces. Plain data types are mapped into structures.
'Out' parameters in the PIM are conventionally mapped to 'inout' parameters
in the PSM [...] The resulting IDL representation of the plugin interfaces
appears in the file dds_security.idl which shall be considered part of the DDS
Security specification."

**Repo:** `crates/security/idl/dds_security.idl`

**Tests:** `crates/security/tests/dds_security_idl.rs::the_file_parses_as_omg_idl`,
`crates/security/tests/dds_security_idl.rs::no_operation_has_an_out_parameter`,
`crates/security/tests/dds_security_idl.rs::authentication_has_the_operations_of_table_29`,
`crates/security/tests/dds_security_idl.rs::access_control_has_the_operations_of_table_39`,
`crates/security/tests/dds_security_idl.rs::crypto_key_factory_has_the_operations_of_table_42`

**Status:** `done` — the file is there, parses as IDL 4.2, has no `out` parameter and, per interface, the operations of its table.
### §11.3 C language representation

**Spec:** §11.3, p. 347 — "The C language representation of the plugin
interfaces shall be obtained applying the IDL to C mapping [5] to the
dds_security.idl file."

**Repo:** `crates/security/tests/idl_bindings/c.rs`

**Tests:** `crates/security/tests/dds_security_bindings.rs::c_representation_compiles_and_the_logging_operations_link`,
`crates/zerodds-c-api/tests/plugin_languages.rs::a_plugin_in_c_is_placed_into_a_participant`,
`crates/zerodds-c-api/src/plugin_ffi/security_plugins.rs::tests::the_handshake_goes_through_the_c_functions`,
`crates/zerodds-c-api/src/plugin_ffi/security_plugins.rs::tests::tokens_and_transforms_go_through_the_c_functions`

**Status:** `done` — header by the rules of IDL2C, compiled as C99. A plugin of the application in C (Authentication, AccessControl, Cryptographic, Logging, DataTagging) is placed into a participant through the structs of `zerodds_security_plugins.h` and called by the runtime at the points of §9.8; these structs are not an IDL2C mapping of `dds_security.idl`. Authentication and Cryptographic of the application: `zerodds_runtime_create_secure` takes them instead of the PKI and AES-GCM path from files; a Cryptographic plugin without an Authentication plugin is refused; the paths of the configuration (governance, permissions CA, identity files if set) go to `validate_local_identity` as the properties of Table 50, governance and permissions CA are required. Checked are the calls of §9.3.2.11 and §9.8.1 (tokens given back, credential before config, `register_local_participant` once, volatile endpoints named, protected writer registered, `destroy` once each) and the passing of the structures. Not checked: a handshake and encoding and decoding between two runtimes with plugins of the application; the passing of the structures of those operations is checked by the Rust tests `plugin_ffi::security_plugins::tests::the_handshake_goes_through_the_c_functions` and `::tokens_and_transforms_go_through_the_c_functions` and the size checks (`SIZE_IS`) in the C test. Green on the Linux test host: `/root/zerodds-rc7/agent-logs/sec-behavior-1018-105526.log` (C, C++, Rust), Java on the Linux test host `/root/agent-cargo/logs/sec-behavior-1018-104914.log`.
### §11.4 C++ classic representation

**Spec:** §11.4, p. 348 — "The C++ classic (without the use of the C++
standard library) language representation of the plugin interfaces shall be
obtained using the IDL2C++ mapping [7] to the dds_security.idl file."

**Repo:** `crates/security/tests/idl_bindings/cpp.rs`

**Tests:** `crates/security/tests/dds_security_bindings.rs::cpp_classic_representation_compiles_and_the_logging_interface_is_implementable`,
`crates/zerodds-c-api/tests/plugin_languages.rs::a_plugin_in_cpp_is_placed_into_a_participant`

**Status:** `done` — header by IDL2C++ without the standard library, compiled as C++98. A plugin of the application in C++ (Authentication, AccessControl, Cryptographic, Logging, DataTagging; abstract classes in `zerodds_security_plugins.hpp`, C++11) is placed into a participant.
### §11.5 Java classic

**Spec:** §11.5, p. 348 — "The Java classic language representation of the
plugin interfaces shall be obtained using the IDL2Java mapping [6] to the
dds_security.idl file."

**Repo:** `crates/security/tests/idl_bindings/java.rs`

**Tests:** `crates/security/tests/dds_security_bindings.rs::java_classic_representation_compiles_and_the_logging_interface_is_implementable`,
`crates/zerodds-c-api/tests/plugin_languages.rs::a_plugin_in_java_is_placed_into_a_participant`

**Status:** `done` — classes by IDL2Java (struct, holder, helper, operations interface), compiled with `javac`. A plugin of the application in Java (Authentication, AccessControl, Cryptographic, Logging, DataTagging; `org.zerodds.security`, JDK 21 with `--enable-preview`, foreign function interface) is placed into a participant.
### §11.6 C++11 representation with five exceptions

**Spec:** §11.6, p. 348 — "This representation is aligned with the
DDS-STDC++ PSM. [...] shall be obtained using the IDL2C++11 mapping [8] to the
dds_security.idl file with the following exceptions: 1. The IDL module DDS
shall be mapped to the C++ namespace dds [...] 2. The mapping shall not use
any C++11-only feature [...] 3. Arrays shall map to the dds::core::array
template [...] 4. The enumerations shall map to the dds::core::safe_enum
template [...] 5. The IDL DynamicData native type shall be mapped to the C++
type dds::code::xtypes::DynamicData".

**Repo:** `crates/security/tests/idl_bindings/cpp.rs` (`Dialect::Cxx11`)

**Tests:** `crates/security/tests/dds_security_bindings.rs::cpp11_representation_compiles_without_a_cpp11_only_feature`

**Status:** `done` — the five exceptions: namespace `dds`, no C++11-only feature (compiled as C++98), `dds::core::array`, `dds::core::safe_enum`, `dds::core::xtypes::DynamicData`.
### §11.7 Java modern with two exceptions

**Spec:** §11.7, p. 348 — "The Java classic language representation of the
plugin interfaces shall be obtained using the IDL2Java mapping [6] to the
dds_security.idl file with the following exceptions: 1. The IDL module DDS
shall be mapped to the Java namespace org.omg.dds [...] 2. The IDL
DynamicData native type shall be mapped to the type
org.omg.dds.type.dynamic.DynamicData".

**Repo:** `crates/security/tests/idl_bindings/java.rs` (`Dialect::Modern`)

**Tests:** `crates/security/tests/dds_security_bindings.rs::java_modern_representation_maps_module_dds_to_org_omg_dds`

**Status:** `done` — module `DDS` is the package `org.omg.dds`, DynamicData is `org.omg.dds.type.dynamic.DynamicData`.

---

## Annex A References

### Annex A Bibliography [1] to [59]

**Spec:** Annex A, pp. 349–351 — bibliography [1] to [59].

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — bibliography; normative references are in
§3.

---

## Audit-Status

803 done / 0 partial / 0 open / 88 n/a (informative) / 0 n/a (rejected).

Test run: to be added on the Linux test host.

