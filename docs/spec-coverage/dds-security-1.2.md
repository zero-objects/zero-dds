# DDS-Security 1.2 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-security-1.2.pdf` (351 Seiten, OMG formal/25-03-06) —
[OMG DDS-Security 1.2](https://www.omg.org/spec/DDS-SECURITY/1.2/)

Audit Item für Item gegen die PDF nach `internal/spec-coverage/PROCESS.md`.
Seitenangaben sind PDF-Seiten (PDF-Seite = gedruckte Seite + 16). Jede
Operation der Schnittstellen der Plugins mit ihren Rückgabewerten, jedes Token,
jede Property, jeder Schritt des Handshakes, jede Prüfung der eingebauten
Plugins, jedes Element und jede Regel von Governance- und
Permissions-Dokument, jede Submessage, jede Parameter-ID und jeder
eingebaute Endpunkt ist ein eigenes Item. `done` steht nur, wo Code und Test die
Anforderung abdecken; ein zitierter Test nennt die Testfunktion als
`datei::modul::name`. Eine Fundstelle im Repo nennt Datei, Symbol und Zeile
(Z.); die Zeilen beziehen sich auf den Stand, mit dem diese Datei eingecheckt
wurde. Jede Aussage über Verhalten ist aus dem Code gelesen; für dieses Audit
wurde kein Test ausgeführt und nichts gebaut.

**Kontext:** DDS-Security ist in ZeroDDS über diese Crates verteilt:

- `crates/security/` — die Schnittstellen der Plugins als Rust-Traits
  (`AuthenticationPlugin`, `AccessControlPlugin`, `CryptographicPlugin`,
  `LoggingPlugin`, `DataTaggingPlugin`), `DataHolder` und die Token,
  `ParticipantGenericMessage`, `PropertyList`.
- `crates/security-pki/` — DDS:Auth:PKI-DH und DDS:Auth:PSK.
- `crates/security-permissions/` — DDS:Access:Permissions und
  DDS:Access:PSK: Governance- und Permissions-Dokument, CMS-Prüfung.
- `crates/security-crypto/` — DDS:Crypto:AES-GCM-GMAC und DDS:Crypto:PSK.
- `crates/security-keyexchange/` — Schlüsselvereinbarung ECDH P-256 und X25519.
- `crates/security-logging/` — DDS:Logging:DDS_LogTopic und weitere Senken.
- `crates/security-rtps/` — ein zweiter Codec für SEC_PREFIX/SEC_BODY/SEC_POSTFIX
  und SRTPS_PREFIX/SRTPS_POSTFIX.
- `crates/security-runtime/` — Verbindung der Plugins mit der Laufzeit:
  `SharedSecurityGate`, `SecurityProfile`, Auswertung der `PropertyQosPolicy`,
  eingebautes DataTagging.
- `crates/rtps/` — Parameter-IDs, EntityIds und Leitungsformate der
  Erweiterungen der Discovery.
- `crates/discovery/` — die Endpunkte DCPSParticipantStatelessMessage und
  DCPSParticipantVolatileMessageSecure, sichere SPDP/Endpunkte von SEDP.
- `crates/dcps/` — Einbindung in die Laufzeit (`runtime.rs`, `participant.rs`).

Die Rust-API ist keine Abbildung der IDL von §9 und §11: Operationen geben
`SecurityResult<T>` statt eines Rückgabewerts mit `SecurityException` zurück,
Handles sind Newtypes über `u64`. Wo eine Operation, ein Parameter oder ein
Rückgabewert der Spec in der Rust-API fehlt, ist das als eigenes Item geführt.

**Offene Punkte:** keine; jedes Item ist `done` oder `n/a (informative)`.

---

## §1 Scope

### §1.1 General

**Spec:** §1.1, S. 17 — "This specification adds several new 'DDS Security
Support' compliance points ('profile') to the DDS Specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf die Konformitätspunkte von §2;
diese sind einzeln geführt.

### §1.2 Overview of this Specification

**Spec:** §1.2, S. 17–18 — "This specification defines five SPIs that when
combined together provide Information Assurance to DDS systems":
Authentication, AccessControl, Cryptographic, Logging, Data Tagging.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die fünf Schnittstellen sind in
§9.3 bis §9.7 mit jeder Operation einzeln geführt.

---

## §2 Conformance

### §2.1 Conformance points

**Spec:** §2.1, S. 19 — "(1) Builtin plugin interoperability (mandatory)
(2) Plugin framework (mandatory) (3) Plugin language APIs (optional)
(4) Logging and Tagging (optional)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung von Klauseln zu
Konformitätspunkten; ob ein Punkt erfüllt ist, folgt aus dem Status der
zugeordneten Items.

### §2.2 Builtin plugin interoperability (mandatory)

**Spec:** §2.2, S. 19 — "Conformance to this point requires conformance to:
Clause 7 [...] The configuration of the plugins and the observable
wire-protocol behavior specified in Clause 10 [...] except for sub clause
10.6."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; Klausel 7 und 10 sind einzeln
geführt.

### §2.3 Plugin framework (mandatory) — Zuordnung

**Spec:** §2.3, S. 19 — "Conformance to this point requires conformance to:
Clause 7 [...] Clause 9 (the plugin model) with the exception of 9.6 and 9.7
[...] The conformance to the plugin model is at the UML level; it does not
mandate a particular language mapping. [...] Clause 10, the builtin-plugins,
except for 10.6".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; die Klauseln sind einzeln
geführt.

### §2.3 Öffentliche API zum Einsetzen der Plugins

**Spec:** §2.3, S. 19 — "In addition it requires the conforming DDS
implementation to provide a public API to insert the plugins that conform to
the aforementioned sections."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`
(Authentication);
`crates/security-runtime/src/shared.rs::SharedSecurityGate::new` (Cryptographic)
und `SharedSecurityGate::set_access_control` (AccessControl);
`crates/security-runtime/src/bundle.rs::SecurityBundleBuilder::logging_plugin`
(Logging)

**Tests:**
`crates/dcps/src/runtime.rs::tests::enable_security_builtins_with_auth_activates_handshake_driver`,
`crates/dcps/tests/security_access_plugin_e2e.rs::create_topic_is_asked_of_the_plugin`,
`crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`,
`crates/dcps/tests/security_bundle_e2e.rs::bundle_logger_is_wired_into_runtime_config_and_fires`

**Status:** `done` — Authentication, Cryptographic, AccessControl und Logging
haben einen öffentlichen Einsetzpunkt. Der Einsetzpunkt für AccessControl
(`set_access_control`, sec-access) besteht, und die Laufzeit fragt das
eingesetzte Plugin für die Zugriffsentscheidungen (Test
`security_access_plugin_e2e`). §2.3 verlangt die Klausel 9 "with the exception
of 9.6 and 9.7": Logging (§9.6) und Data Tagging (§9.7) sind davon ausgenommen.
Tests grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1018-082254.log`,
`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §2.3 Kein Einsetzpunkt für AccessControl und DataTagging

**Spec:** §2.3, S. 19 — wie oben.

**Repo:**
`crates/security-runtime/src/shared.rs::SharedSecurityGate::set_access_control`,
`crates/security-runtime/src/shared.rs::SharedSecurityGate::application_access_control`

**Tests:**
`crates/dcps/tests/security_access_plugin_e2e.rs::instance_operations_are_asked_of_the_plugin`,
`crates/dcps/tests/security_access_plugin_e2e.rs::participant_set_qos_is_asked_of_the_plugin`

**Status:** `done` — ein Anwender setzt ein eigenes AccessControl-Plugin in
einen Participant ein (`SharedSecurityGate::set_access_control`); das Gate und
die Laufzeit entscheiden über es. Für DataTagging gibt es keinen Einsetzpunkt,
die Spec verlangt ihn nicht (§2.3 nimmt §9.7 aus); die Tags eines Endpunkts
kommen aus `DataWriterQos.data_tags`/`DataReaderQos.data_tags`. Tests grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1018-082254.log`).

### §2.4 Plugin Language APIs (optional)

**Spec:** §2.4, S. 19–20 — "Conformance with the 'plugin language API' point
requires conformance with at least one of the 5 language APIs enumerated
below: C [...] 11.2 and 11.3, C++ classic [...] 11.4, Java classic [...]
11.5, C++11 [...] 11.6, Java5+ [...] 11.7".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; §11 ist einzeln geführt.

### §2.5 Logging and Tagging profile (optional)

**Spec:** §2.5, S. 20 — "Conformance to this point requires conformance to sub
clauses 9.6, 9.7, and 10.6."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; §9.6, §9.7 und §10.6 sind
einzeln geführt.

---

## §3 Normative References

### §3 Normative References

**Spec:** §3, S. 21 — DDS 1.4, DDS-RTPS 2.5, DDS-XTYPES 1.3, OMG-IDL 4.2,
RFC 2104 (HMAC), RFC 6151, RFC 2315 (PKCS #7), RFC 8017 (PKCS #1), XSD 1.1
Part 2.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literaturliste; wo eine referenzierte Norm
eine Anforderung trägt, steht sie beim Item, das sie verwendet.

---

## §4 Terms and Definitions

### §4 Terms and Definitions

**Spec:** §4, S. 22–25 — Begriffsbestimmungen von „Access Control“ bis
„Public Key Infrastructure“.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar.

---

## §5 Symbols

### §5 Symbols

**Spec:** §5, S. 25 — "This specification does not define any symbols or
abbreviations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — keine Symbole.

---

## §6 Additional Information

### §6.1 Changes to Adopted OMG Specifications

**Spec:** §6.1, S. 26 — "This specification does not modify any existing
adopted OMG specifications."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abgrenzung zu DDS, DDS-RTPS, DDS-XTYPES und
IDL.

### §6.1 DDS-XTYPES: Builtin Secure TypeLookup Endpoints

**Spec:** §6.1, S. 26 — "Implementations of both this specification and
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

**Status:** `done` — die vier sicheren Endpunkte für TypeLookup bestehen
(`a542c9f545d`, `7cfdb4fd99a`, `37de123a72e`): eigene EntityIds,
SPDP-Ankündigung, Schutz je Submessage, Token. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §6.2 Acknowledgments

**Spec:** §6.2, S. 26–27 — Liste der beitragenden Personen und Firmen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Danksagung.

---

## §7 Support for DDS Security

### §7.1 Security Model

**Spec:** §7.1, S. 29 — "Securing DDS means providing: Confidentiality of the
data samples, Integrity of the data samples and the messages that contain
them, Authentication of DDS writers and readers, Authorization of DDS writers
and readers, Message-origin authentication, Data-origin authentication,
(Optional) Non-repudiation of data".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zielbeschreibung; die Mechanismen stehen
in §7.3 bis §10 und sind dort einzeln geführt.

### §7.1.1 Threats

**Spec:** §7.1.1, S. 29–30 — vier Bedrohungsklassen (Unauthorized
subscription, Unauthorized publication, Tampering and replay, Unauthorized
access to data) und sechs Akteure (Alice, Bob, Eve, Trudy, Mallory, Trent).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Bedrohungsmodell ohne Anforderung.

### §7.1.1.1 Unauthorized Subscription

**Spec:** §7.1.1.1, S. 30 — "All that is required is for Alice to encrypt the
data she writes using a secret key that is only shared with authorized
receivers".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung der Verschlüsselung; die
Anforderung steht in §9.5 und §10.5.

### §7.1.1.2 Unauthorized Publication

**Spec:** §7.1.1.2, S. 30–31 — "All that is required is for the protocol to
require that the messages include either a hash-based message authentication
code (HMAC) or digital signature."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung der Authentisierung; die
Anforderung steht in §9.5 und §10.5.

### §7.1.1.3 Tampering and Replay

**Spec:** §7.1.1.3, S. 31–32 — "if Alice wants to send an HMAC with a
different key for every receiver, the only solution is to append multiple
HMACs to the multicast message with some key-id that allows the recipient to
select the correct HMAC to verify."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung der empfängerspezifischen MACs;
die Anforderung steht in §9.5.3 und §10.5.

### §7.1.1.4 Unauthorized Access to Data by Infrastructure Services

**Spec:** §7.1.1.4, S. 32 — Trent darf Nachrichten empfangen, prüfen und
weiterleiten, ohne den Inhalt zu sehen.

**Repo:** `crates/dcps/src/runtime.rs` (`UserReaderConfig::relay`,
`write_user_sample_relayed`, `UserSample::Alive::crypto_content`);
`crates/security-permissions/src/governance.rs` (`check_relay_reader`,
`check_relay_writer`); `crates/security-runtime/src/shared.rs`
(`check_create_relay_datareader`, `remote_may_relay`);
`crates/routing-service/src/config.rs` (`Route::relay`)

**Tests:** `crates/dcps/tests/security_relay_e2e.rs::a_relay_forwards_what_it_cannot_read_and_the_reader_takes_it_as_the_originals`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`,
`crates/routing-service/tests/relay_route.rs::a_relay_route_forwards_on_behalf_of_the_writer`

**Status:** `done` — ein Relay-Reader wird auf `<relay>` des Grants angelegt, bekommt nur
den Schlüssel der Submessage und gibt die Payload ungeöffnet weiter; ein
Writer sendet sie unverändert im Namen des Original-Writers
(PID_ORIGINAL_WRITER_INFO), nur die Submessage mit seinem Schlüssel. Der
Empfänger nimmt die Probe als die des Originals nur von einem Writer, dessen
Grant `<relay>` erlaubt, und öffnet sie mit dem Schlüssel des Originals; ein
Writer ohne `<relay>` gilt als er selbst. Der Routing-Service hat dafür einen
Relay-Modus. Die Spec nennt für das Anlegen eines Readers nur `<subscribe>`
(Table 70); den Relay-Reader setzt der Abschnitt voraus. Gemessen: grün auf dem
Linux-Testhost, Live-Test Alice → Trent → Carol dreimal hintereinander.

### §7.2 Cryptographic Algorithm Classes

**Spec:** §7.2, S. 32–33 — drei Klassen: Digital Signature, Key Establishment
and KeyAgreement, Symmetric Cipher. "The common set of predefined
cryptographic algorithms available for use by the SPIs are defined in Clause
8."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einteilung; die Algorithmen sind in §8
einzeln geführt.

### §7.3 Types used by DDS Security

**Spec:** §7.3, S. 33 — "The types described in this sub clause are used in
these extensions."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.3.1 Use of IDL and XTYPES notation

**Spec:** §7.3.1, S. 33 — "the requirement is that the serialized data for
types defined/used in the DDS-Security specification the corresponding
DDS-XTYPES data representations for those same concrete data types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Konvention der Notation; die Serialisierung
jedes Typs ist beim Typ als eigenes Item geführt.

### §7.3.1.1 Type Extensibility

**Spec:** §7.3.1.1, S. 33–34 — Typen der Topics der Discovery MUTABLE, Typen der
QoS MUTABLE, eine QoS-Policy APPENDABLE, andere Typen oberster Ebene APPENDABLE,
Typen in Sequenzen oder in nicht-mutable Typen FINAL.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Konvention, nach der die Spec ihre Typen
deklariert; die Folge für die Leitung steht in §7.3.1.2.

### §7.3.1.2 Data Representation: Extended CDR Version 1

**Spec:** §7.3.1.2, S. 34 — "DDS-Security only uses the Extended CDR
representation with encoded version 1. Specifically, this means that the
serialization of a type with extensibility kind APPENDABLE is the same as if
it had been declared to have extensibility kind FINAL."

**Repo:** `crates/security/src/token.rs::encode` (Z. 357, `DataHolder` ohne
DHEADER); `crates/security/src/generic_message.rs::ParticipantGenericMessage::to_cdr`
(Z. 138, APPENDABLE-Typ ohne DHEADER)

**Tests:** `crates/security/src/token.rs::tests::cdr_le_layout_class_id_only`,
`crates/security/src/generic_message.rs::tests::message_data_dataholder_is_inline_not_length_prefixed`

**Status:** done — `DataHolder` und `ParticipantGenericMessage` werden als
XCDR1 ohne DHEADER geschrieben; die Typen der Discovery als ParameterList.

### §7.3.1.3 Herstellerspezifische Erweiterungen nur in Typen der Discovery

**Spec:** §7.3.1.3, S. 34 — "Vendors may only create vendor-specific
extensions to the Types representing builtin Topics used for discovery [...]
The member IDs of these vendor-specific members shall be in the
Vendor-specific ParameterId space [...] The Ignore/Must Understand bit [...]
must also be set according with the meaning of table 9.6".

**Repo:** `crates/rtps/src/parameter_list.rs` (Konstanten `pid::*`, eigene
Parameter ab `0x8000`); `crates/security-runtime/src/caps_wire.rs` (Namen
`org.zerodds.sec.*`)

**Tests:**
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`,
`crates/rtps/src/property_list.rs::tests::data_tags_wire_form`

**Status:** `done` — die eigenen Parameter-IDs von ZeroDDS liegen im
Herstellerbereich. Eigene Einträge in `PID_PROPERTY_LIST` tragen den
Herstellerpräfix `org.zerodds.` (§7.3.2); die Tags eines Endpunkts gehen als
`PID_DATA_TAGS` (0x1003), nicht mehr als Einträge `dds.sec.data_tags.*`
(`715589c2094`, `aef84a98a0b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.2 Property_t: name, value, propagate

**Spec:** §7.3.2, S. 34–35, Table 1 — "The DDS Security specification extends
the DDS-RTPS definition of Property_t to contain the additional boolean
attribute 'propagate'". Attribute `name String`, `value String`,
`propagate Boolean`.

**Repo:** `crates/security/src/properties.rs::Property` (Z. 21, Felder
`name`, `value`, `propagate`)

**Tests:** `crates/security/src/properties.rs::tests::propagatable_filter`,
`crates/security/src/properties.rs::tests::set_replaces_duplicate`

**Status:** done

### §7.3.2 Präfix „dds.sec.“ reserviert

**Spec:** §7.3.2, S. 35 — "Property_t objects with names that start with the
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

**Status:** `done` — die Namen der Spec (`dds.sec.auth.*`,
`dds.sec.access.{permissions,permissions_ca,governance}`,
`dds.sec.crypto.{symmetric_cipher_algorithm,rtps_psk_*}`, `dds.sec.log.plugin`,
`dds.sec.log.builtin.DDS_LogTopic.{log_file,distribute}`) bleiben; jeder Name,
den die Spec nicht definiert, steht unter `org.zerodds.`:
`org.zerodds.sec.{access.psk_governance,access.psk_permissions,access.subject_name,auth.crl,auth.accept_x509v1_identity_certificate,crypto.max_blocks_per_session,crypto.rtps_psk_secret_passphrase_alt,crypto.rtps_psk_secret_passphrase_extra,log.level,log.jsonl.path,log.syslog.*,log.builtin.DDS_LogTopic.{logging_level,legacy_topic}}`,
die Plugin-Klassen in `caps_wire.rs` (`7ed5d2621ff`, `aef84a98a0b`). Der Name
`dds.sec.log.builtin.DDS_LogTopic.logging_level` bleibt in der Bench-App für
Fast DDS, das ihn selbst liest. Neu auf der Leitung und in Konfigurationen: die
alten Namen werden nicht mehr gelesen. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §7.3.2 Herstellerpräfix aus umgekehrtem ICANN-Domainnamen

**Spec:** §7.3.2, S. 35 — "implementers shall use property names that start
with a prefix to an ICANN domain name they own, in reverse order. For example,
the prefix would be 'com.acme.'".

**Repo:** `crates/security-runtime/src/caps_wire.rs`,
`crates/security-runtime/src/qos_properties.rs`, `crates/security/src/crypto.rs`
(`org.zerodds.crypto.*`), `crates/security-crypto/src/psk_rtps.rs`
(`org.zerodds.psk.*`)

**Tests:**
`crates/security-runtime/tests/spdp_caps_e2e.rs::secure_caps_roundtrip_through_spdp_beacon`,
`crates/security-runtime/tests/property_qos_names.rs::builtin_property_names_match_the_spec_strings`

**Status:** `done` — alle eigenen Namen von ZeroDDS beginnen mit `org.zerodds.`
(die Domain zerodds.org, umgekehrt): `org.zerodds.sec.*`,
`org.zerodds.crypto.*`, `org.zerodds.psk.*`. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §7.3.2.1 IDL Representation for Property_t

**Spec:** §7.3.2.1, S. 35 — "@extensibility(FINAL) struct Property_t {
string name; string value; @non-serialized boolean propagate; }; typedef
sequence< Property_t > PropertySeq;"

**Repo:** `crates/rtps/src/property_list.rs::WireProperty` (Z. 52, ohne
`propagate`), `WirePropertyList::encode` (Z. 131);
`crates/security/src/token.rs::WireProperty` (Z. 53)

**Tests:** `crates/rtps/src/property_list.rs::tests::single_property_roundtrip_le`,
`crates/rtps/src/property_list.rs::tests::encode_pads_strings_to_4_between_name_and_value`,
`crates/rtps/src/property_list.rs::tests::multi_property_roundtrip_be`

**Status:** done — `propagate` wird nicht serialisiert; Name und Wert sind
CDR-Strings mit Ausrichtung auf vier.

### §7.3.3 BinaryProperty_t: name, value, propagate

**Spec:** §7.3.3, S. 35–36, Table 2 — `name String`, `value OctetSeq`,
`propagate Boolean`; "BinaryProperty_t objects with a 'name' attribute that
start with the prefix 'dds.sec.' are reserved".

**Repo:** `crates/security/src/token.rs::BinaryProperty` (`propagate`, nicht
serialisiert); `crates/security/src/properties.rs::PropertyList` (binäre
Einträge); `crates/qos/src/policies/property.rs::PropertyQosPolicy`
(`binary_value`, je Eintrag `propagate`)

**Tests:**
`crates/security/src/properties.rs::tests::binary_properties_carry_propagate_and_it_is_not_serialized`,
`crates/qos/src/policies/property.rs::tests::only_propagate_true_entries_are_propagated`,
`crates/rtps/src/property_list.rs::tests::binary_properties_cross_the_wire`

**Status:** `done` — `BinaryProperty` hat `propagate` (nicht serialisiert); die
lokale `PropertyList` und die `PropertyQosPolicy` führen binäre Properties
(`b4c91fb4ed1`, `928d39304cd`, `f6657d231dd`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.3 BinaryProperty_t: propagate und lokale binäre Properties

**Spec:** §7.3.3, S. 35 — "BinaryProperty_t also contains the boolean
attribute 'propagate'. Similar to Property_t this attribute is used to
indicate whether the corresponding binary property is intended for local use
only or shall be propagated by DDS discovery."

**Repo:** `crates/security/src/token.rs::BinaryProperty` (`propagate`, nicht
serialisiert); `crates/security/src/properties.rs::PropertyList` (binäre
Einträge); `crates/qos/src/policies/property.rs::PropertyQosPolicy`
(`binary_value`, je Eintrag `propagate`)

**Tests:**
`crates/security/src/properties.rs::tests::binary_properties_carry_propagate_and_it_is_not_serialized`,
`crates/qos/src/policies/property.rs::tests::only_propagate_true_entries_are_propagated`,
`crates/rtps/src/property_list.rs::tests::binary_properties_cross_the_wire`

**Status:** `done` — `BinaryProperty` hat `propagate` (nicht serialisiert); die
lokale `PropertyList` und die `PropertyQosPolicy` führen binäre Properties
(`b4c91fb4ed1`, `928d39304cd`, `f6657d231dd`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.3.1 IDL Representation for BinaryProperty_t

**Spec:** §7.3.3.1, S. 36 — "@extensibility(FINAL) struct BinaryProperty_t {
string name; OctetSeq value; @non-serialized boolean propagate; };"

**Repo:** `crates/security/src/token.rs::encode` (Z. 357, `encode_string` und
`encode_octet_seq`)

**Tests:** `crates/security/src/token.rs::tests::token_with_binary_property_roundtrip`

**Status:** done — Name als CDR-String, Wert als `sequence<octet>`.

### §7.3.3.1 ASCII-Wert mit NUL als Oktettfolge

**Spec:** §7.3.3.1, S. 36 — "When setting the BinaryProperty_t value octet
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

**Status:** `done` — Vorgabe ist die Form der Spec: `c.dsign_algo` und
`c.kagree_algo` haben 13 und 21 Oktette, das NUL eingeschlossen (§7.3.3.1), für
jeden Peer. `c.id` (das Zertifikat als PEM) und `c.perm` (das Berechtigungsdokument
als S/MIME-Text) enden ebenfalls mit dem NUL; ein binäres Dokument geht unverändert.
Beim Empfang wird ein NUL am Ende vor dem Lesen von Zertifikat und Dokument
abgeschnitten. RTI Connext 7.7.0 meldete zu einem `c.id` ohne NUL „not properly
null terminated“ (OMG dds-security, Linux-Testhost). Die Formen anderer Hersteller sind benannte Einstellungen:
`org.zerodds.sec.algo_name_nul` / `ZERODDS_SEC_ALGO_NAME_NUL` = `omit` (ohne
NUL, Fast DDS), `mirror` (als Initiator mit NUL, als Replier in der Form des
Requests: ein Request ohne NUL wird ohne NUL beantwortet, `97a1d7fc001`,
`89b70651dad`) oder `by_vendor` (NUL nur für OpenDDS, die frühere Form)
(`e87b0411306`). Gemessen (Linux-Testhost, sec-diag10, vor der Umstellung auf die
Einstellung, damals als Verhalten des Repliers): `fastdds:zerodds` und
`zerodds:fastdds` von `common-subset` ok auf `32f11c44270`; mit dem Vorgabewert
`spec` antwortet ZeroDDS einem Fast-DDS-Initiator mit NUL, das ist ungemessen
und nach dem damaligen Befund NO_MATCH. Fast-DDS-Zellen brauchen `mirror`; die
Spalte mit Fast DDS in
`tests/perf/dds-roundtrip-bench/security-matrix/deep_matrix.sh` setzt es im
ZeroDDS-Prozess, die anderen Spalten laufen mit den Vorgaben. Quelle (Fast DDS
2.6.10, `src/cpp/security/authentication/PKIDH.cpp`, gelesen; der Quelltext von
3.6.1 liegt nicht vor, dessen Verhalten ist aus der Matrix geschlossen):
`begin_handshake_request` schreibt `c.dsign_algo` und `c.kagree_algo` mit
`assign(sign_alg_.begin(), sign_alg_.end())` aus einem `std::string` ohne NUL
(Z. 1257-1270); `process_handshake_request`, die Verarbeitung der Antwort beim
Initiator, prüft `s_dsign_algo.compare(ECDSA_SHA256)` auf einem `std::string`
aus den Oktetten des Werts (Z. 1915-1925), ein NUL im Wert macht ihn ungleich
und die Antwort wird abgewiesen; `begin_handshake_reply` prüft die Anfrage mit
`strcmp` (Z. 1503) und nimmt ein NUL hin. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.4 DataHolder

**Spec:** §7.3.4, S. 36, Table 3 — `class_id String`, `properties
PropertySeq`, `binary_properties BinaryPropertySeq`.

**Repo:** `crates/security/src/token.rs::DataHolder` (Z. 96)

**Tests:** `crates/security/src/token.rs::tests::pki_dh_identity_token_roundtrip`,
`crates/security/src/token.rs::tests::token_with_binary_property_roundtrip`

**Status:** done

### §7.3.4.1 IDL representation for DataHolder

**Spec:** §7.3.4.1, S. 37 — "@extensibility(FINAL) struct DataHolder { string
class_id; PropertySeq properties; BinaryPropertySeq binary_properties; };
typedef sequence<DataHolder> DataHolderSeq;"

**Repo:** `crates/security/src/token.rs::encode` (Z. 357),
`decode_consumed` (Z. 381)

**Tests:** `crates/security/src/token.rs::tests::cdr_le_layout_class_id_only`,
`crates/security/src/token.rs::tests::empty_data_holder_roundtrip_be`,
`crates/security/src/token.rs::tests::missing_trailing_nul_rejected`,
`crates/security/src/token.rs::tests::truncated_buffer_is_error`

**Status:** done — `DataHolderSeq` steht in `ParticipantGenericMessage`
(§7.3.22).

### §7.3.5 Token

**Spec:** §7.3.5, S. 37 — "The Token class is structurally identical to the
DataHolder class [...] the contents and interpretation of the Token objects
shall be specified by each plugin implementation."

**Repo:** `crates/security/src/token.rs::IdentityToken` (Z. 247,
`pub type IdentityToken = DataHolder`)

**Tests:** `crates/security/src/token.rs::tests::pki_dh_identity_token_roundtrip`,
`crates/security/src/token.rs::tests::permissions_token_roundtrip`

**Status:** done

### §7.3.5.1 Attribute: class_id — Präfix „dds.sec.“ reserviert

**Spec:** §7.3.5.1, S. 38 — "Strings with the prefix 'dds.sec.' are reserved
for this specification [...] the class_id they use shall start with a prefix
to an ICANN domain name they own".

**Repo:** `crates/security-pki/src/psk.rs::class_id`,
`crates/security-pki/src/psk.rs::SPEC_CLASS_ID`,
`crates/security-permissions/src/psk_access.rs::CLASS_ID_PSK_PERMISSIONS`

**Tests:**
`crates/security-pki/src/psk.rs::tests::the_handshake_classes_are_not_the_spec_class`

**Status:** `done` — die Klassen von `DDS:Auth:PSK` und `DDS:Access:PSK` sind
die der Spec (`DDS:Auth:PSK:1.2`, `DDS:Access:PSK:1.2`, sec-auth und
sec-access); der Handshake mit PSK, den ZeroDDS zusätzlich spricht, trägt die
Klassen `org.zerodds.auth.psk-handshake:1.0[+AuthReq|+AuthReply|+AuthFinal]`,
also ein umgekehrter Domainname (`7ed5d2621ff`). Die Konstante
`DDS:Auth:PKI-DH:1.0+IdentityStatus` in
`crates/security-pki/src/identity_status.rs` ist ungenutzt; der
IdentityStatusToken des Plugins trägt die Klasse `DDS:Auth:PKI-DH:1.0` (Table
53). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §7.3.5.2 Typedefs der Token

**Spec:** §7.3.5.2, S. 38 — "typedef DataHolder Token; typedef Token
MessageToken; typedef MessageToken AuthRequestMessageToken; typedef
MessageToken HandshakeMessageToken; typedef Token IdentityToken; typedef Token
IdentityStatusToken; typedef Token PermissionsToken; typedef Token
AuthenticatedPeerCredentialToken; typedef Token PermissionsCredentialToken;
typedef Token CryptoToken; typedef Token ParticipantCryptoToken; typedef
Token DatawriterCryptoToken; typedef Token DatareaderCryptoToken;"

**Repo:** `crates/security/src/token.rs` (Z. 247–268: `IdentityToken`,
`PermissionsToken`, `IdentityStatusToken`, `PermissionsCredentialToken`,
`AuthRequestMessageToken`, `HandshakeMessageToken`, `CryptoToken` als
`DataHolder`)

**Tests:** `crates/security/src/token.rs::tests::pki_dh_identity_token_roundtrip`,
`crates/security/src/token.rs::tests::permissions_token_roundtrip`

**Status:** done — alle Token sind auf der Leitung `DataHolder`. In Rust gibt
es keinen Alias für `Token`, `MessageToken`,
`AuthenticatedPeerCredentialToken`, `ParticipantCryptoToken`,
`DatawriterCryptoToken` und `DatareaderCryptoToken`; diese Werte sind
unmittelbar `DataHolder`.

### §7.3.5.2 Sequenzen der Token

**Spec:** §7.3.5.2, S. 38 — "typedef sequence<HandshakeMessageToken>
HandshakeMessageTokenSeq; typedef sequence<CryptoToken> CryptoTokenSeq;
typedef CryptoTokenSeq ParticipantCryptoTokenSeq; typedef CryptoTokenSeq
DatawriterCryptoTokenSeq; typedef CryptoTokenSeq DatareaderCryptoTokenSeq;"

**Repo:** `crates/security/src/generic_message.rs::ParticipantGenericMessage`
(Z. 97, `message_data: Vec<DataHolder>`)

**Tests:** `crates/security/src/generic_message.rs::tests::message_data_dataholder_is_inline_not_length_prefixed`

**Status:** done — die Sequenzen erscheinen auf der Leitung nur als
`message_data`; in Rust sind sie `Vec<DataHolder>` ohne eigene Aliase.

### §7.3.5.3 TokenNIL

**Spec:** §7.3.5.3, S. 38 — "This name refers to the Token object having
class_id set to the empty string, and both properties and binary_properties
sequences set to the empty sequence. The TokenNIL object is used to indicate
the absence of a Token."

**Repo:** `crates/security/src/token.rs::DataHolder` (Z. 96, `Default`
ergibt leere `class_id` und leere Sequenzen)

**Tests:** `crates/security/src/token.rs::tests::empty_data_holder_roundtrip_le`

**Status:** done — `DataHolder::default()` ist TokenNIL; eine benannte
Konstante gibt es nicht. Die Verwendung von TokenNIL in den
Operationen der Plugins ist bei den Operationen in §9 geführt.

### §7.3.6 CryptoAlgorithmName

**Spec:** §7.3.6, S. 38–39 — "typedef string<64> CryptoAlgorithmName;" — "See
clause 8 for the values".

**Repo:** `crates/security-pki/src/handshake_token.rs` (`MAX_ALGO_NAME`, die
Namen von Table 25 und 26), `crates/security-pki/src/identity_token.rs`

**Tests:**
`crates/security-pki/src/handshake_token.rs::tests::an_algorithm_name_over_64_characters_is_refused`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_names_the_algorithms_as_table_25_and_26_unless_the_peer_reads_1_1`,
`crates/security-pki/src/identity_token.rs::tests::the_algorithms_are_the_names_of_table_25`

**Status:** `done` — Algorithmusnamen sind Zeichenketten bis 64 Zeichen; ZeroDDS
sendet die Namen von Version 1.2 (Table 25 und 26: `ECDSA+P256+SHA256`, ...) und
die von 1.1 nur, wenn der Peer sie liest (sec-crypto/sec-auth, `8a3e542788d`).
Tests grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1018-083929.log`).

### §7.3.6 CryptoAlgorithmName: Grenze 64 nicht geprüft

**Spec:** §7.3.6, S. 38 — "typedef string<64> CryptoAlgorithmName;"

**Repo:** `crates/security-pki/src/handshake_token.rs::take_bin_string`
(`MAX_ALGO_NAME` = 64)

**Tests:**
`crates/security-pki/src/handshake_token.rs::tests::an_algorithm_name_over_64_characters_is_refused`

**Status:** `done` — ein Algorithmusname über 64 Zeichen wird beim Lesen
abgewiesen (`a4c86d977df`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.7 CryptoAlgorithmId und CRYPTO_ALGORITHM_INVALID_ID

**Spec:** §7.3.7, S. 39 — "typedef octet CryptoAlgorithmId; const
CryptoAlgorithmId CRYPTO_ALGORITHM_INVALID ID=0x00;" — 0x01 bis 0x7F für die
Spec, 0x80 bis 0xFF herstellerspezifisch.

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformKind`
(Z. 29, `None = 0` bis `Aes256Gcm = 4`)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::kind_round_trip_all_variants`,
`crates/security-crypto/src/crypto_transform.rs::tests::unknown_kind_rejected`

**Status:** done — der Wert 0 bedeutet keine Transformation; ZeroDDS belegt
keinen Wert im Herstellerbereich. Eine Konstante mit dem Namen
`CRYPTO_ALGORITHM_INVALID_ID` gibt es nicht.

### §7.3.8 CryptoAlgorithmBit und CRYPTO_ALGORITHM_COMPATIBILITY_MODE

**Spec:** §7.3.8, S. 39–40 — "typedef uint32 CryptoAlgorithmBit; const
CryptoAlgorithmBit CRYPTO_ALGORITHM_COMPATIBILITY_MODE=0x80000000;" — Bits
`0x00000001` bis `0x00008000` für die Spec, `0x00010000` bis `0x40000000` für
Hersteller.

**Repo:** `crates/security/src/algorithm.rs::CryptoAlgorithmBit`, `CRYPTO_ALGORITHM_COMPATIBILITY_MODE`; `crates/rtps/src/security_algo_info.rs::CRYPTO_ALGORITHM_COMPATIBILITY_MODE`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — Typ und Konstante `0x80000000`.

### §7.3.8 CRYPTO_ALGORITHM_COMPATIBILITY_MODE fehlt

**Spec:** §7.3.8, S. 39 — "The value 0x80000000 is reserved and has a special
meaning defined in 7.3.10.1."

**Repo:** `crates/security/src/algorithm.rs::CRYPTO_ALGORITHM_COMPATIBILITY_MODE`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — in `zerodds-security` und in `zerodds-rtps`.

### §7.3.9 CryptoAlgorithmSet, SET_ALL, SET_EMPTY

**Spec:** §7.3.9, S. 40 — "typedef uint32 CryptoAlgorithmSet; const
CryptoAlgorithmSet CRYPTO_ALGORITHM_SET_ALL = 0xffffffff; const
CryptoAlgorithmSet CRYPTO_ALGORITHM_SET_EMPTY = 0x00000000;"

**Repo:** `crates/security/src/algorithm.rs::CryptoAlgorithmSet`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — Typ und beide Konstanten.

### §7.3.9 Konstanten CRYPTO_ALGORITHM_SET_ALL und _EMPTY fehlen

**Spec:** §7.3.9, S. 40 — wie oben.

**Repo:** `crates/security/src/algorithm.rs::CRYPTO_ALGORITHM_SET_ALL`, `CRYPTO_ALGORITHM_SET_EMPTY`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — `0xffffffff` und `0`.

### §7.3.10 CryptoAlgorithmRequirements

**Spec:** §7.3.10, S. 40 — "@extensibility (FINAL) struct
CryptoAlgorithmRequirements { CryptoAlgorithmSet supported_mask;
CryptoAlgorithmSet required_mask; };"

**Repo:** `crates/rtps/src/security_algo_info.rs::AlgorithmRequirements`
(Z. 89, `to_bytes` Z. 104, `from_bytes` Z. 120)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::algorithm_requirements_layout_be`,
`crates/rtps/src/security_algo_info.rs::tests::algorithm_requirements_roundtrip_be`

**Status:** done — acht Oktette, `supported` vor `required`. Der Typ heißt
`AlgorithmRequirements`, die Felder `supported` und `required` (Benennung).

### §7.3.10.1 CheckCryptoAlgorithmCompatibility

**Spec:** §7.3.10.1, S. 40–41 — "return ( (required_mask & supported_mask ) ==
required_mask ) OR ( ((required_mask & supported_mask ) != 0 ) AND (
(required_mask & CRYPTO_ALGORITHM_COMPATIBILITY_MODE) != 0 ))".

**Repo:** `crates/rtps/src/security_algo_info.rs::check_crypto_algorithm_compatibility`, `CRYPTO_ALGORITHM_COMPATIBILITY_MODE`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::the_compatibility_mode_bit_makes_one_algorithm_enough`

**Status:** `done` — `(required & supported) == required`, oder eine gemeinsame Algorithmus-Bit und das Bit `0x80000000` in `required`.

### §7.3.10.1 Kompatibilität zweier Participants

**Spec:** §7.3.10.1, S. 41 — "compatible if and only if [...]
CheckCryptoAlgorithmCompatibility (P2.supported_mask, P1.required_mask) AND
CheckCryptoAlgorithmCompatibility (P1.supported_mask, P2.required_mask)".

**Repo:** `crates/rtps/src/security_algo_info.rs::AlgorithmRequirements::compatible_with`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::two_requirements_are_compatible_both_ways`

**Status:** `done` — beide Richtungen.

### §7.3.11 ParticipantSecurityDigitalSignatureAlgorithmInfo

**Spec:** §7.3.11, S. 41 — "@extensibility (APPENDABLE) struct
ParticipantSecurityDigitalSignatureAlgorithmInfo {
CryptoAlgorithmRequirements trust_chain; CryptoAlgorithmRequirements
message_auth; };"

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecurityDigitalSignatureAlgorithmInfo`
(Z. 157); Feld `crates/rtps/src/participant_data.rs::sig_algo_info` (Z. 252)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::sig_info_roundtrip`,
`crates/rtps/src/participant_data.rs::tests::participant_data_sig_algo_info_roundtrip`

**Status:** done — 16 Oktette, `trust_chain` vor `message_auth`.

### §7.3.11 trust_chain und message_auth: Inhalt der Masken

**Spec:** §7.3.11, S. 41–42 — "The trust_chain.supported_mask shall contain
the algorithms the SPIs is able to use to validate the digital signature of
documents. The trust_chain.required_mask shall contain all the algorithms that
are contained in digitally-signed documents sent by the SPI [...] The
message_auth.required_mask shall contain all the algorithms the SPIs will use
to sign documents or messages".

**Repo:** `crates/rtps/src/security_algo_info.rs`;
`crates/security-pki/src/plugin.rs::set_participant_security_config`,
`set_local_permissions_signature_algorithm`;
`crates/security-permissions/src/cms.rs::CmsPkcs7Verifier::signature_algorithm_oid`;
Aufruf in `crates/security-runtime/src/profile.rs`;
`crates/dcps/src/runtime.rs::local_algorithm_infos`

**Tests:**
`crates/security-pki/src/plugin.rs::tests::the_algorithm_masks_follow_the_identity_and_the_key_agreement`,
`crates/security-pki/src/plugin.rs::tests::the_required_trust_chain_mask_holds_the_algorithm_of_the_permissions_document`,
`crates/security-permissions/tests/cms_verify.rs::the_signature_algorithm_of_a_signed_document_is_read`

**Status:** `done` — `trust_chain.required` enthält die Signatur des
Identity-Zertifikats und die Signatur des eigenen Permissions-Dokuments (aus dem
SignerInfo des `.p7s` gelesen, beim Laden des Profils gesetzt);
`message_auth.required` die Signatur des Identitätsschlüssels; `supported` je
die vier Algorithmen, die das Plugin prüft (`aef84a98a0b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195430.log`).

### §7.3.11.1 Compatibility (Digital Signature)

**Spec:** §7.3.11.1, S. 42 — "compatible if and only if both the trust_chain
and the message_auth are compatible [...] DomainParticipants with
incompatible ParticipantSecurityDigitalSignatureAlgorithmInfo are not able to
authenticate with each other."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`; `crates/rtps/src/security_algo_info.rs::ParticipantSecurityDigitalSignatureAlgorithmInfo::compatible_with`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`, `crates/rtps/src/security_algo_info.rs::tests::signature_and_key_establishment_compatibility`

**Status:** `done` — ein entfernter Participant mit unverträglichen Signaturmasken wird nicht authentisiert (`security.participant.incompatible`).

### §7.3.11.2 Default value (Digital Signature)

**Spec:** §7.3.11.2, S. 42 — "trust_chain.supported_mask =
CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256 | CBIT_RSASSA_PKCS1_V15_2048_SHA256 |
CBIT_ECDSA_P256_SHA256; trust_chain.required_mask = CBIT_ECDSA_P256_SHA256;
message_auth.supported_mask = CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256 |
CBIT_ECDSA_P256_SHA256; message_auth.required_mask = CBIT_ECDSA_P256_SHA256".

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecurityDigitalSignatureAlgorithmInfo::spec_default`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::sig_info_spec_default`

**Status:** `done` — die Werte von §7.3.11.2 (vorher gleiche `required`- wie `supported`-Masken, ohne PKCS1).

### §7.3.12 ParticipantSecurityKeyEstablishmentAlgorithmInfo

**Spec:** §7.3.12, S. 42 — "@extensibility (APPENDABLE) struct
ParticipantSecurityKeyEstablishmentAlgorithmInfo {
CryptoAlgorithmRequirements shared_secret; };"

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecurityKeyEstablishmentAlgorithmInfo`
(Z. 217); Feld `crates/rtps/src/participant_data.rs::kx_algo_info` (Z. 257)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::kx_info_roundtrip`,
`crates/rtps/src/participant_data.rs::tests::participant_data_kx_algo_info_roundtrip`

**Status:** done

### §7.3.12 shared_secret: Inhalt der Masken

**Spec:** §7.3.12, S. 43 — "The shared_secret.supported_mask shall contain all
the algorithms the SPIs is able to use to establish a shared key. The
shared_secret.required_mask shall contain all the algorithms the SPIs of
other participants must support in order to interoperate."

**Repo:** `crates/security-pki/src/plugin.rs::set_participant_security_config`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_algorithm_masks_follow_the_identity_and_the_key_agreement`

**Status:** `done` — `supported`: DHE+MODP-2048-256, ECDHE-CEUM P-256 und P-384; `required`: das bevorzugte (X25519 hat kein Bit und gibt 0).

### §7.3.12.1 Compatibility (Key Establishment)

**Spec:** §7.3.12.1, S. 43 — "compatible if and only if the shared_secret is
compatible according to the compatibility rules [...] of 7.3.10.1."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — wie §7.3.11.1.

### §7.3.12.2 Default value (Key Establishment)

**Spec:** §7.3.12.2, S. 43 — "member.supported_mask = CBIT_DHE_MODP_2048_256 |
CBIT_ECDHE_CEUM_P256; member.required_mask = CBIT_ECDHE_CEUM_P256".

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecurityKeyEstablishmentAlgorithmInfo::spec_default`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::kx_info_spec_default`

**Status:** `done` — §7.3.12.2.

### §7.3.13 ParticipantSecuritySymmetricCipherAlgorithmInfo

**Spec:** §7.3.13, S. 43 — "@extensibility (APPENDABLE) struct
ParticipantSecuritySymmetricCipherAlgorithmInfo { CryptoAlgorithmSet
supported_mask; CryptoAlgorithmSet builtin_endpoints_required_mask;
CryptoAlgorithmSet builtin_kx_endpoints_required_mask; CryptoAlgorithmSet
user_endpoints_default_required_mask; };"

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecuritySymmetricCipherAlgorithmInfo`
(Z. 262); Feld `crates/rtps/src/participant_data.rs::sym_cipher_algo_info`
(Z. 262)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::sym_info_roundtrip`,
`crates/rtps/src/participant_data.rs::tests::participant_data_sym_cipher_algo_info_roundtrip`

**Status:** done

### §7.3.13 Inhalt der vier Masken

**Spec:** §7.3.13, S. 43–44 — "The supported_mask shall contain all the
algorithms the SPIs is able to use to decrypt messages or validate
authentication tags. The builtin_endpoints_required_mask shall contain all
the algorithms [...] for all the builtin endpoints, except for the
DCPSParticipantVolatileMessageSecure [...] The
user_endpoints_default_required_mask shall contain all the default algorithms
that will be used by user-defined (non-builtin) endpoint."

**Repo:** `crates/dcps/src/runtime.rs::local_algorithm_infos`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — `supported`: AES128 und AES256; `builtin_endpoints_required_mask` und `user_endpoints_default_required_mask`: die Breite der Property `dds.sec.crypto.symmetric_cipher_algorithm`; `builtin_kx_endpoints_required_mask`: AES256 (der Kx-Schlüssel ist AES256-GCM).

### §7.3.13.1 Compatibility (Symmetric Cipher)

**Spec:** §7.3.13.1, S. 44 — vier Aufrufe von
`CheckCryptoAlgorithmCompatibility` über `builtin_endpoints_required_mask` und
`builtin_kx_endpoints_required_mask` beider Participants; "the
user_endpoints_default_required_mask is not considered for compatibility".

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`; `crates/rtps/src/security_algo_info.rs::ParticipantSecuritySymmetricCipherAlgorithmInfo::compatible_with`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`, `crates/rtps/src/security_algo_info.rs::tests::symmetric_cipher_info_compatibility`

**Status:** `done` — Builtin- und Kx-Anforderungen beider Seiten; `user_endpoints_default_required_mask` zählt nicht.

### §7.3.13.2 Default value (Symmetric Cipher)

**Spec:** §7.3.13.2, S. 44 — "supported_mask = CBIT_AES128_GCM |
CBIT_AES256_GCM; builtin_endpoints_required_mask = CBIT_AES256_GCM;
builtin_kx_endpoints_required_mask = CBIT_AES256_GCM;
user_endpoints_default_required_mask = CBIT_AES256_GCM".

**Repo:** `crates/rtps/src/security_algo_info.rs::ParticipantSecuritySymmetricCipherAlgorithmInfo::spec_default`

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::sym_info_spec_default`

**Status:** `done` — §7.3.13.2 (alle drei Anforderungen AES256).

### §7.3.14 ParticipantSecurityAlgorithmInfo

**Spec:** §7.3.14, S. 45 — "@extensibility (APPENDABLE) struct
ParticipantSecurityAlgorithmInfo {
ParticipantSecurityDigitalSignatureAlgorithmInfo digital_signature;
ParticipantSecurityKeyEstablishmentAlgorithmInfo key_establishment;
ParticipantSecuritySymmetricCipherAlgorithmInfo symmetric_cipher; };"

**Repo:**
`crates/rtps/src/security_algo_info.rs::ParticipantSecurityAlgorithmInfo`,
`EndpointSecurityAlgorithmInfo`

**Tests:**
`crates/rtps/src/security_algo_info.rs::tests::the_aggregates_hold_their_members_in_order`

**Status:** `done` — der zusammenfassende Typ besteht
(`ParticipantSecurityAlgorithmInfo`: 40 Oktette, die drei Teile in der
Reihenfolge der Spec; `EndpointSecurityAlgorithmInfo`) (`a5b73d27f19`). Test
grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). Die
Verwendung in den Operationen der Plugins ist Sache der Items von §9.

### §7.3.15 EndpointSecuritySymmetricCipherAlgorithmInfo

**Spec:** §7.3.15, S. 45 — "@extensibility (APPENDABLE) struct
EndpointSecuritySymmetricCipherAlgorithmInfo { CryptoAlgorithmSet
required_mask; @non_serialized CryptoAlgorithmSet supported_mask; };"

**Repo:** `crates/rtps/src/publication_data.rs`, `subscription_data.rs` (`sym_cipher_algo_info`)

**Tests:** `crates/rtps/src/publication_data.rs::tests::endpoint_symmetric_cipher_info_roundtrip`

**Status:** `done` — `required_mask` als `PID_ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO` 0x1013 in beiden Endpunkt-Daten.

### §7.3.15 EndpointSecuritySymmetricCipherAlgorithmInfo: supported_mask fehlt

**Spec:** §7.3.15, S. 45 — "The value of this member shall be set by the SPI
implementations to match the supported_mask in the
ParticipantSecuritySymmetricCipherAlgorithmInfo of the DomainParticipant that
contains the Endpoint."

**Repo:**
`crates/rtps/src/security_algo_info.rs::EndpointSecuritySymmetricCipherAlgorithmInfo`
(`supported_mask`, nicht serialisiert, `with_supported_mask`)

**Tests:**
`crates/rtps/src/publication_data.rs::tests::endpoint_symmetric_cipher_info_roundtrip`,
`crates/rtps/src/security_algo_info.rs::tests::the_aggregates_hold_their_members_in_order`

**Status:** `done` — das Feld `supported_mask` besteht; es wird nicht
serialisiert (`a5b73d27f19`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.15 required_mask eines DataWriters und DataReaders

**Spec:** §7.3.15, S. 45 — "If the Endpoint is a DataWriter then the
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

**Status:** `done` — ein Endpunkt verlangt den Algorithmus, den seine eigene
Property `dds.sec.crypto.symmetric_cipher_algorithm` nennt; er geht als
`PID_ENDPOINT_SECURITY_SYMMETRIC_CIPHER` (0x1013) in die Discovery, und ein
Endpunkt mit unverträglicher Einstellung wird nicht gepaart (`b7c1adc7281`).
Tests grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.15.1 Compatibility (Endpoint)

**Spec:** §7.3.15.1, S. 46 — "CheckCryptoAlgorithmCompatibility (
P2.symmetric_cipher.supported_mask, E1.required_mask) AND
CheckCryptoAlgorithmCompatibility ( P1.symmetric_cipher.supported_mask,
E2.required_mask)".

**Repo:** `crates/dcps/src/runtime.rs::endpoint_incompatibility`, `wire_writer_to_remote_reader`, `wire_reader_to_remote_writer`

**Tests:** `crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`, `crates/rtps/src/security_algo_info.rs::tests::endpoint_symmetric_cipher_compatibility`

**Status:** `done` — Endpunkte mit unverträglicher Anforderung werden nicht gematcht (`match.refused`).

### §7.3.15.2 Default value (Endpoint)

**Spec:** §7.3.15.2, S. 46 — "If the EndpointSecuritySymmetricCipherAlgorithmInfo
is not present [...] the value shall be set to
symmetric_cipher.user_endpoints_default_required_mask of the
DomainParticipant that contains the Endpoint".

**Repo:** `crates/rtps/src/security_algo_info.rs::EndpointSecuritySymmetricCipherAlgorithmInfo::spec_default`

**Tests:** `crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`, `crates/rtps/src/security_algo_info.rs::tests::sym_info_spec_default`

**Status:** `done` — fehlend: `user_endpoints_default_required_mask` des Participants des Endpunkts.

### §7.3.16 EndpointSecurityAlgorithmInfo

**Spec:** §7.3.16, S. 46 — "@extensibility (APPENDABLE) struct
EndpointSecurityAlgorithmInfo { EndpointSecuritySymmetricCipherAlgorithmInfo
symmetric_cipher; };"

**Repo:**
`crates/rtps/src/security_algo_info.rs::ParticipantSecurityAlgorithmInfo`,
`EndpointSecurityAlgorithmInfo`

**Tests:**
`crates/rtps/src/security_algo_info.rs::tests::the_aggregates_hold_their_members_in_order`

**Status:** `done` — der zusammenfassende Typ besteht
(`ParticipantSecurityAlgorithmInfo`: 40 Oktette, die drei Teile in der
Reihenfolge der Spec; `EndpointSecurityAlgorithmInfo`) (`a5b73d27f19`). Test
grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). Die
Verwendung in den Operationen der Plugins ist Sache der Items von §9.

### §7.3.17 CryptoTransformKeyRevision

**Spec:** §7.3.17, S. 46 — "typedef octet CryptoTransformKeyRevision[3];
#define CRYPTO_TRANSFORM_KEY_REVISION_NONE {0x00, 0x00, 0x00}".

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier::key_revision`; `crates/security-crypto/src/plugin.rs` (`KeyMaterial::key_revision`)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::the_key_revision_is_part_of_the_kind`

**Status:** `done` — drei Oktette, in der `transformation_kind` des CryptoHeader.

### §7.3.17 Revision ungleich NONE wird nicht ausgewertet

**Spec:** §7.3.17, S. 46 — "The CryptoTransformKeyRevision provides a way to
represent changes to Key Material."

**Repo:** `crates/security-crypto/src/plugin.rs::check_header_kind`, `from_keymat_cyclone`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_key_revision_is_carried_in_the_keymat_and_the_header`

**Status:** `done` — das Schlüsselmaterial behält die Revision seines Keymat, ein CryptoHeader unter einer anderen Revision derselben Kennung wird abgewiesen. Eine Schlüsselrevision zu erzeugen (`revise_local_entity_keys`, §9.5.1.6) fehlt (eigenes Item).

### §7.3.17 CryptoTransformKeyRevisionIntHolder

**Spec:** §7.3.17, S. 46–47 — "typedef int32
CryptoTransformKeyRevisionIntHolder; [...] int_holder_value =
256*256*transformation_key_revision_value[0] + 256*key_revision_value[1] +
key_revision_value[2]".

**Repo:** `crates/security/src/algorithm.rs::CryptoTransformKeyRevisionIntHolder`, `key_revision_to_int`, `key_revision_from_int`

**Tests:** `crates/security/src/algorithm.rs::tests::the_key_revision_int_holder_follows_the_formula`

**Status:** `done` — `256*256*r[0] + 256*r[1] + r[2]`.

### §7.3.18 CryptoTransformKind

**Spec:** §7.3.18, S. 47 — "@extensibility(FINAL) struct CryptoTransformKind {
CryptoTransformKeyRevision transformation_key_revision; CryptoAlgorithmId
transformation_algorithm_id; }; #define CRYPTO_TRANSFORM_KIND_INVALID
{{0x00, 0x00, 0x00}, 0x00}".

**Repo:** `crates/security-crypto/src/crypto_transform.rs`; `crates/security-crypto/src/plugin.rs`

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::the_key_revision_is_part_of_the_kind`

**Status:** `done` — Revision und Algorithmus-Kennung, in beiden Codecs gleich gelesen.

### §7.3.18.1 Attribute: transformation_key_revision

**Spec:** §7.3.18.1, S. 47 — "This attribute is used to support the change of
the key material used by a DDS Entity. It is meant to be used in combination
with a CryptoTransformKeyId."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::the_key_revision_is_part_of_the_kind`

**Status:** `done` — wie §7.3.17.

### §7.3.18.2 Attribute: transformation_algorithm_id

**Spec:** §7.3.18.2, S. 47 — "The CryptoAlgorithmId values used for the
transformation_algorithm_id shall correspond to those assigned to Symmetric
Cipher and MAC algorithms, see clause 8."

**Repo:** `crates/security-crypto/src/suite.rs::Suite` (`Aes128Gmac` = 0x01,
`from_transform_kind_id`)

**Tests:**
`crates/security-crypto/src/suite.rs::tests::aes128_gmac_uses_spec_id_1`,
`crates/security-crypto/src/suite.rs::tests::from_id_roundtrip_for_all_supported`

**Status:** `done` — die Ids sind die von §8.1: 0x01 AES128-GMAC, 0x02
AES128-GCM, 0x03 AES256-GMAC, 0x04 AES256-GCM; die Id 0x01 trägt nicht mehr
HMAC-SHA256 (`702c1c51a54`, vor diesem Paket; die Einträge waren veraltet).
Tests grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.18.2 Id 0x01 trägt HMAC-SHA256 statt AES128-GMAC

**Spec:** §7.3.18.2, S. 47, mit §8.1, S. 91 — "const CryptoAlgorithmId
CID_AES128_GMAC = 0x01;"

**Repo:** `crates/security-crypto/src/suite.rs::Suite` (`Aes128Gmac` = 0x01,
`from_transform_kind_id`)

**Tests:**
`crates/security-crypto/src/suite.rs::tests::aes128_gmac_uses_spec_id_1`,
`crates/security-crypto/src/suite.rs::tests::from_id_roundtrip_for_all_supported`

**Status:** `done` — die Ids sind die von §8.1: 0x01 AES128-GMAC, 0x02
AES128-GCM, 0x03 AES256-GMAC, 0x04 AES256-GCM; die Id 0x01 trägt nicht mehr
HMAC-SHA256 (`702c1c51a54`, vor diesem Paket; die Einträge waren veraltet).
Tests grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.19 CryptoTransformKeyId

**Spec:** §7.3.19, S. 47 — "typedef octet CryptoTransformKeyId[4];" — "an
opaque value that helps create a unique 'lookup' reference [...] The scope for
the CryptoTransformKeyId is the DomainParticipant that generated the
CryptoTransformKeyId."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`
(Z. 99, `key_id: [u8; 4]`); `crates/security-crypto/src/plugin.rs::KeyMaterial`
(`transformation_key_id`, zufällig erzeugt, Z. 91–97)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::transform_identifier_round_trip`,
`crates/security-crypto/src/plugin.rs::tests::key_rotation_changes_the_wire_key_id_and_the_derived_session_key`

**Status:** done — vier zufällige Oktette je KeyMaterial.

### §7.3.20 CryptoTransformIdentifier

**Spec:** §7.3.20, S. 48 — "@extensibility(FINAL) struct
CryptoTransformIdentifier { CryptoTransformKind transformation_kind;
CryptoTransformKeyId transformation_key_id; };"

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`
(Z. 99, `to_bytes`/`from_bytes`); `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(Z. 278, schreibt `transform_kind` und `transformation_key_id` an den Anfang
des CryptoHeader)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::transform_identifier_round_trip`,
`crates/security-crypto/src/crypto_transform.rs::tests::header_round_trip_with_full_iv`

**Status:** done — acht Oktette, `transformation_kind` vor
`transformation_key_id`.

### §7.3.20.1 Attribute: transformation_kind

**Spec:** §7.3.20.1, S. 48 — "The transformation_algorithm_id identifies the
Crytographic Algorithm used by the transformation. It shall contain one of
the CryptoAlgorithmId values defined in Section 8 [...] The
transformation_key_revision value (see 7.3.17) shall be combined with the
transformation_key_id attribute to identify the key material".

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::the_key_revision_is_part_of_the_kind`

**Status:** `done` — Algorithmus und Revision.

### §7.3.20.2 Schlüsselsuche über (GUID des Senders, key_id, Revision)

**Spec:** §7.3.20.2, S. 48 — "The 3-tuple (sender_participant_guid,
transformation_key_id, transformation_key_revision) uniquely identifies the Key
Material within the scope of all Domain Participants that are communicating in
a common DDS Domain."

**Repo:** `crates/security-crypto/src/plugin.rs` (`remote_by_key_id` mit dem
Schlüssel `(Sender, transformation_key_id, Revision)`, `decode_data_by_key_id`)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_key_is_found_by_sender_key_id_and_revision`

**Status:** `done` — der entfernte Schlüssel wird über (Handle des Senders,
`transformation_key_id`, Revision) abgelegt und gesucht; zwei Peers mit
derselben `transformation_key_id` überschreiben sich nicht (`409b63fcd0d`). Test
grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.21 PropertyQosPolicy

**Spec:** §7.3.21, S. 48–49 — "@extensibility(APPENDABLE) struct
PropertyQosPolicy { PropertySeq value; BinaryPropertySeq binary_value; };"

**Repo:** `crates/qos/src/policies/property.rs::PropertyQosPolicy` (Zeichenketten
und `binary_value`, je Eintrag `propagate`)

**Tests:** `crates/qos/src/policies/property.rs::tests::insert_get_roundtrip`,
`crates/qos/src/policies/property.rs::tests::last_value_wins_and_preserves_order`,
`crates/qos/src/policies/property.rs::tests::only_propagate_true_entries_are_propagated`

**Status:** `done` — die Policy führt `value` und `binary_value`, je Eintrag mit
`propagate`; `propagated()` und `propagated_binary()` liefern die Einträge für
die Discovery.

### §7.3.21 PropertyQosPolicy: binary_value und propagate fehlen

**Spec:** §7.3.21, S. 49 — wie oben; dazu §7.3.2 und §7.3.3 (`propagate`).

**Repo:** `crates/qos/src/policies/property.rs::PropertyQosPolicy::set_propagated`,
`::set_binary`, `::propagated`, `::propagated_binary`

**Tests:** `crates/qos/src/policies/property.rs::tests::only_propagate_true_entries_are_propagated`

**Status:** `done` — Sequenz binärer Properties und `propagate` je Eintrag sind
vorhanden.

### §7.3.21 DomainParticipantQos.property

**Spec:** §7.3.21, S. 49 — "@extensibility(MUTABLE) struct
DomainParticipantQos { // Existing policies from the DDS specification
PropertyQosPolicy property; };"

**Repo:** `crates/dcps/src/qos.rs::DomainParticipantQos` (Z. 306, Feld
`property`, Z. 313)

**Tests:** `crates/dcps/tests/security_qos_properties.rs::a_complete_dds_sec_property_list_creates_a_secured_participant`

**Status:** done

### §7.3.21 DataWriterQos.property

**Spec:** §7.3.21, S. 49 — "struct DataWriterQos { [...] PropertyQosPolicy
property; DataTagQosPolicy data_tags; };"

**Repo:** `crates/dcps/src/qos.rs::DataWriterQos` (`property`,
`propagated_properties`); `crates/dcps/src/runtime.rs::register_endpoint_crypto`

**Tests:**
`crates/dcps/src/qos.rs::tests::property_is_a_member_and_immutable_after_enable`,
`crates/dcps/src/runtime.rs::tests::only_propagated_properties_are_announced`,
`crates/dcps/src/runtime.rs::tests::the_cipher_property_of_an_endpoint_sets_its_key_width`,
`crates/zerodds-c-api/src/qos_ffi.rs::tests::property_and_data_tags_cross_the_c_abi_both_ways`

**Status:** `done` — das Feld ist vorhanden und nach dem Aktivieren nicht
änderbar; `dds.sec.crypto.symmetric_cipher_algorithm` setzt die Schlüsselbreite
des Writers (§10.5.1.1), ein Wert außerhalb von Table 22 weist ihn mit
`SecurityConfiguration` ab; die Einträge mit `propagate = TRUE` gehen als
`PID_PROPERTY_LIST` in die `PublicationBuiltinTopicData` (`f6657d231dd`). Tests
grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). Die C-Strukturen
`ZeroDdsDataWriterQos` und `ZeroDdsDataReaderQos`
(`crates/zerodds-c-api/src/qos_ffi.rs`) enden in den Members `property` und
`data_tags` (`e797d6d6433`, Header `e08203d6784`); die Layouts in
`crates/ts-node` (`dd8e8f32c24`) und `crates/cs` (`1bd5d2126cf`) folgen, das
C++-Binding liest den Header. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-220659.log`):
`crates/zerodds-c-api/src/qos_ffi.rs::tests::property_and_data_tags_cross_the_c_abi_both_ways`.
Die Bindings ts-node und csharp wurden nicht gebaut oder ausgeführt.

### §7.3.21 DataReaderQos.property

**Spec:** §7.3.21, S. 49 — "struct DataReaderQos { [...] PropertyQosPolicy
property; DataTagQosPolicy data_tags; };"

**Repo:** `crates/dcps/src/qos.rs::DataReaderQos` (`property`)

**Tests:**
`crates/rtps/src/subscription_data.rs::tests::endpoint_property_list_roundtrip`,
`crates/dcps/src/runtime.rs::tests::the_cipher_property_of_an_endpoint_sets_its_key_width`,
`crates/zerodds-c-api/src/qos_ffi.rs::tests::property_and_data_tags_cross_the_c_abi_both_ways`

**Status:** `done` — wie bei `DataWriterQos.property`: das Feld, die
Schlüsselbreite des Readers und die Ankündigung der `propagate = TRUE`-Einträge
in der `SubscriptionBuiltinTopicData` bestehen. Die C-Strukturen
`ZeroDdsDataWriterQos` und `ZeroDdsDataReaderQos`
(`crates/zerodds-c-api/src/qos_ffi.rs`) enden in den Members `property` und
`data_tags` (`e797d6d6433`, Header `e08203d6784`); die Layouts in
`crates/ts-node` (`dd8e8f32c24`) und `crates/cs` (`1bd5d2126cf`) folgen, das
C++-Binding liest den Header. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-220659.log`):
`crates/zerodds-c-api/src/qos_ffi.rs::tests::property_and_data_tags_cross_the_c_abi_both_ways`.
Die Bindings ts-node und csharp wurden nicht gebaut oder ausgeführt.

### §7.3.21 Tag, TagSeq, DataTags, DataTagQosPolicy

**Spec:** §7.3.21, S. 49 — "@extensibility(FINAL) struct Tag { string name;
string value; }; typedef sequence<Tag> TagSeq; @extensibility(APPENDABLE)
struct DataTags { TagSeq tags; }; typedef DataTags DataTagQosPolicy;"

**Repo:** `crates/qos/src/policies/data_tag.rs` (`Tag`, `DataTagQosPolicy`);
`crates/dcps/src/qos.rs` (`DataWriterQos.data_tags`, `DataReaderQos.data_tags`,
`announced_data_tags`)

**Tests:** `crates/qos/src/policies/data_tag.rs::tests::tags_keep_their_order`,
`crates/dcps/src/runtime.rs::tests::data_tags_are_announced`

**Status:** `done` — `Tag`, `DataTagQosPolicy` und die Felder `data_tags` von
`DataWriterQos` und `DataReaderQos` bestehen (nach dem Aktivieren nicht
änderbar) und gehen als `PID_DATA_TAGS` in die Discovery (`bf4642c3c25`,
`7b1de8f5255`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). Das Member in der
C-Struktur steht bei den Items zu `property` (gebaut, `e797d6d6433`).

### §7.3.21 DataWriterQos.data_tags und DataReaderQos.data_tags

**Spec:** §7.3.21, S. 49 — wie oben.

**Repo:** `crates/qos/src/policies/data_tag.rs` (`Tag`, `DataTagQosPolicy`);
`crates/dcps/src/qos.rs` (`DataWriterQos.data_tags`, `DataReaderQos.data_tags`,
`announced_data_tags`)

**Tests:** `crates/qos/src/policies/data_tag.rs::tests::tags_keep_their_order`,
`crates/dcps/src/runtime.rs::tests::data_tags_are_announced`

**Status:** `done` — `Tag`, `DataTagQosPolicy` und die Felder `data_tags` von
`DataWriterQos` und `DataReaderQos` bestehen (nach dem Aktivieren nicht
änderbar) und gehen als `PID_DATA_TAGS` in die Discovery (`bf4642c3c25`,
`7b1de8f5255`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`). Das Member in der
C-Struktur steht bei den Items zu `property` (gebaut, `e797d6d6433`).

### §7.3.21 Weitergabe der PropertyQosPolicy über Discovery

**Spec:** §7.3.21, S. 49 — "The PropertyQosPolicy shall be propagated via DDS
discovery so it appears in the ParticipantBuiltinTopicData,
PublicationBuiltinTopicData, and SubscriptionBuiltinTopicData [...] only the
ones with propagate=TRUE are propagated via DDS discovery".

**Repo:** `crates/dcps/src/runtime.rs::update_participant_properties`,
`RuntimeConfig.participant_properties`; `crates/rtps/src/participant_data.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_propagated_participant_properties_reach_the_peer`,
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** `done` — nur Einträge der `PropertyQosPolicy` des Participants mit
`propagate = TRUE` gehen als `PID_PROPERTY_LIST` in den SPDP-Beacon; eine
Änderung geht sofort hinaus; eine Liste, die die Leitung nicht tragen kann, wird
abgewiesen (`a5b73d27f19`, `e357ab7a5ac`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.3.22 MessageIdentity

**Spec:** §7.3.22, S. 50 — "@extensibility(FINAL) struct MessageIdentity {
GUID_t source_guid; long long sequence_number; };"

**Repo:** `crates/security/src/generic_message.rs::MessageIdentity` (Z. 80)

**Tests:** `crates/security/src/generic_message.rs::tests::nil_message_identity`,
`crates/security/src/generic_message.rs::tests::roundtrip_le`

**Status:** done — `sequence_number` wird auf acht ausgerichtet.

### §7.3.22 GenericMessageClassId

**Spec:** §7.3.22, S. 50 — "typedef string<> GenericMessageClassId;"

**Repo:** `crates/security/src/generic_message.rs::ParticipantGenericMessage`

**Tests:** `crates/security/src/generic_message.rs::tests::a_message_class_id_is_not_bounded_to_256_octets`, `::class_id_constants_match_spec`, `::invalid_class_id_utf8_rejected`

**Status:** `done` — unbegrenzter String; der Decoder begrenzt nur durch die Grenze der ganzen Nachricht.

### §7.3.22 GenericMessageClassId: Grenze 256 im Decoder

**Spec:** §7.3.22, S. 50 — wie oben.

**Repo:** `crates/security/src/generic_message.rs` (`MAX_CLASS_ID_LEN`)

**Tests:** `crates/security/src/generic_message.rs::tests::a_message_class_id_is_not_bounded_to_256_octets`

**Status:** `done` — die Grenze von 256 Oktetten ist entfernt.

### §7.3.22 ParticipantGenericMessage

**Spec:** §7.3.22, S. 50 — "@extensibility(APPENDABLE) struct
ParticipantGenericMessage { MessageIdentity message_identity; MessageIdentity
related_message_identity; GUID_t destination_participant_guid; GUID_t
destination_endpoint_guid; GUID_t source_endpoint_guid; GenericMessageClassId
message_class_id; DataHolderSeq message_data; };"

**Repo:** `crates/security/src/generic_message.rs::ParticipantGenericMessage`
(Z. 97), `to_cdr` (Z. 138), `from_cdr` (Z. 176)

**Tests:** `crates/security/src/generic_message.rs::tests::roundtrip_le`,
`crates/security/src/generic_message.rs::tests::handshake_request_token_in_message_data`,
`crates/security/src/generic_message.rs::tests::truncated_buffer_rejected`

**Status:** done — die Felder heißen `destination_participant_key`,
`destination_endpoint_key` und `source_endpoint_key` statt `*_guid`.

### §7.3.23 ParticipantSecurityProtectionInfo: drei Member

**Spec:** §7.3.23, S. 50 — "@extensibility (APPENDABLE) struct
ParticipantSecurityProtectionInfo { ParticipantSecurityAttributesMask
participant_security_attributes; PluginParticipantSecurityAttributesMask
plugin_participant_security_attributes; ParticipantSecurityAttributesMaskExt
participant_security_optional_attributes; };"

**Repo:** `crates/rtps/src/participant_security_info.rs` (drei Member,
`ParticipantSecurityAttributesMaskExt`)

**Tests:**
`crates/rtps/src/participant_security_info.rs::tests::the_optional_attributes_are_the_third_member`

**Status:** `done` — `ParticipantSecurityProtectionInfo` hat die drei Member der
Spec (`470192aa99a`, vor diesem Paket; die Einträge waren veraltet). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.23 ParticipantSecurityAttributesMaskExt fehlt

**Spec:** §7.3.23, S. 50–51 — "struct ParticipantSecurityAttributesMaskExt {
unsigned short is_set; unsigned short value; }; [...] The is_set mask indicates
whether the corresponding bit in the value mask is set. The interpretation of
each bit is specified in clause 9.4.2.5."

**Repo:** `crates/rtps/src/participant_security_info.rs::ParticipantSecurityAttributesMaskExt`

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::the_optional_attributes_are_the_third_member`

**Status:** `done` — Typ, Codec (gelesen ab 12 Oktetten, geschrieben nur bei `is_set` ungleich 0).

### §7.3.23 PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_VALID

**Spec:** §7.3.23, S. 50 — "#define
PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_VALID (0x1 << 31)".

**Repo:** `crates/rtps/src/participant_security_info.rs::attrs` (Z. 20,
`IS_VALID = 0x8000_0000`), `plugin_attrs` (Z. 33)

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::round_trip_fully_protected`

**Status:** done

### §7.3.23 Default PARTICIPANT_SECURITY_ATTRIBUTES_INFO_DEFAULT

**Spec:** §7.3.23, S. 51 — "#define PARTICIPANT_SECURITY_ATTRIBUTES_INFO_DEFAULT
{0, 0}".

**Repo:** `crates/rtps/src/participant_security_info.rs::ParticipantSecurityInfo`
(Z. 54, `Default` ergibt beide Masken 0)

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::empty_default_is_not_valid`,
`crates/rtps/src/participant_security_info.rs::tests::round_trip_default`

**Status:** done

### §7.3.23 Kompatible Konfiguration der Participants

**Spec:** §7.3.23, S. 51 — "A compatible configuration is defined as having
the same value for the participant_security_attributes and the
plugin_participant_security_attributes [...] If the is_valid is set to zero on
either of the masks, the comparison [...] shall ignore the attribute."

**Repo:** `crates/rtps/src/participant_security_info.rs::is_compatible_with`; `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`, `crates/rtps/src/participant_security_info.rs::tests::compatibility_is_equality_of_the_valid_masks`

**Status:** `done` — gleiche Masken, eine Maske ohne `IS_VALID` auf einer Seite wird nicht verglichen. Nicht gemessen gegen Cyclone, Fast DDS, OpenDDS; `zerodds.sec.compatibility_check=ignore` schaltet ab.

### §7.3.23 Inkompatible Participants gelten als nicht authentisiert

**Spec:** §7.3.23, S. 51 — "Two participants that don't have compatible
configurations shall not attempt authentication and each participant shall
consider the other participant as an 'unauthenticated participant.'"

**Repo:** `crates/dcps/src/runtime.rs::spdp_security_handshake`, `inproc_drive_security_handshake`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — der Handshake beginnt nicht; der Participant bleibt unauthentisiert.

### §7.3.23 participant_security_optional_attributes außerhalb der Kompatibilität

**Spec:** §7.3.23, S. 51 — "The participant_security_optional_attributes
encode configuration information about the plugin that does not need to be
set consistently for two Participants to authenticate."

**Repo:** `crates/rtps/src/participant_security_info.rs::is_compatible_with`

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::the_optional_attributes_are_the_third_member`

**Status:** `done` — der Member zählt nicht zur Kompatibilität.

### §7.3.24 EndpointSecurityProtectionInfo

**Spec:** §7.3.24, S. 51–52 — "@extensibility(APPENDABLE) struct
EndpointSecurityProtectionInfo { EndpointSecurityAttributesMask
endpoint_security_attributes; PluginEndpointSecurityAttributesMask
plugin_endpoint_security_attributes; }; #define
ENDPOINT_SECURITY_ATTRIBUTES_FLAG_IS_VALID (0x1 << 31)".

**Repo:** `crates/rtps/src/endpoint_security_info.rs::EndpointSecurityInfo`
(Z. 80), `attrs::IS_VALID` (Z. 30), `plugin_attrs::IS_VALID` (Z. 58)

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::encoded_bytes_le_match_spec_layout`,
`crates/rtps/src/endpoint_security_info.rs::tests::roundtrip_be`

**Status:** done — der Typ heißt `EndpointSecurityInfo` (Benennung).

### §7.3.24 Default ENDPOINT_SECURITY_ATTRIBUTES_INFO_DEFAULT

**Spec:** §7.3.24, S. 52 — "#define ENDPOINT_SECURITY_ATTRIBUTES_INFO_DEFAULT
{0, 0}".

**Repo:** `crates/rtps/src/endpoint_security_info.rs::EndpointSecurityInfo`
(Z. 80, `Default`)

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::default_is_not_valid`

**Status:** done

### §7.3.24 Kompatible Konfiguration der Endpunkte

**Spec:** §7.3.24, S. 52 — "A compatible configuration is defined as having
the same value for all of the attributes in the EndpointSecurityInfo, except
that [...] If the is_valid is set to zero on either of the masks, the
comparison [...] shall ignore the attribute."

**Repo:** `crates/rtps/src/endpoint_security_info.rs::is_compatible_with`; `crates/dcps/src/runtime.rs::endpoint_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`, `crates/rtps/src/endpoint_security_info.rs::tests::endpoint_compatibility_is_equality_of_the_valid_masks`

**Status:** `done` — wie bei den Participants; ein Endpunkt ohne Maske (OpenDDS) wird nicht verglichen.

### §7.3.24 plugin_endpoint_security_attributes für die Middleware opak

**Spec:** §7.3.24, S. 52 — "The value of the plugin_endpoint_security_attributes
shall be defined by the security plugin implementation and is opaque to the
DDS middleware (other than the is_valid bit)."

**Repo:** `crates/security-runtime/src/plugin_attributes.rs`;
`crates/dcps/src/runtime.rs` (reicht die Masken des Plugins durch)

**Tests:**
`crates/security-runtime/src/plugin_attributes.rs::tests::the_plugin_reads_back_what_it_announces`,
`crates/security-runtime/src/plugin_attributes.rs::tests::key_protected_follows_data_encrypt_per_spec_10_4_1_2_6`,
`crates/security-runtime/src/plugin_attributes.rs::tests::liveliness_protected_sets_0x40_per_spec_9_4_1_3`

**Status:** `done` — die Masken `plugin_*_security_attributes` stammen aus dem
Plugin (`plugin_attributes`) und werden von der Middleware nur weitergegeben,
nicht ausgewertet (`ee7183c45df`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.3.25 NOT_ALLOWED_BY_SECURITY

**Spec:** §7.3.25, S. 52 — "The DDS Security specification adds an additional
return code NOT_ALLOWED_BY_SECURITY, which shall be returned by any operation
on the DDS API that fails because the security plugins do not allow it."

**Repo:** `crates/dcps/src/error.rs::DdsError::NotAllowedBySecurity` (Z. 67),
`return_code::NOT_ALLOWED_BY_SECURITY` (Z. 211)

**Tests:** `crates/dcps/src/error.rs::tests::rc_not_allowed_by_security_maps`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`

**Status:** done — welche Operationen den Code liefern, steht bei den
Operationen von AccessControl in §9.4 und §9.8.

### §7.4 Securing DDS Messages on the Wire

**Spec:** §7.4, S. 52 — "OMG DDS uses the Real-Time Publish-Subscribe (RTPS)
on-the-wire protocol [2] for communicating data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.4.1 RTPS Background (Non-Normative)

**Spec:** §7.4.1, S. 52–54 — Aufbau einer RTPS-Nachricht; Einteilung in
DataWriter-, DataReader- und Interpreter-Submessages.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Überschrift sagt „Non-Normative“; die
Folgerung zur Minor-Version steht in §7.4.6.1.

### §7.4.2 Secure RTPS Messages

**Spec:** §7.4.2, S. 54–55 — "this specification uses a 'message
transformation' mechanism that gives the Security Plugin Implementations
fine-grain control over which parts of the RTPS Message need to be encrypted
and/or signed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung des Mechanismus; die
Submessages stehen in §7.4.6 und §7.4.7, die Transformationen in §9.5.3.

### §7.4.3 GUID an die Identität gebunden: eigener Participant

**Spec:** §7.4.3, S. 55 — "this specification constrains the GUID that can be
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

**Status:** `done` — der eigene GUID-Präfix wird aus dem Zertifikat abgeleitet
und der eines entfernten Participants gegen sein Zertifikat geprüft (das zweite
Item unten war bereits `done`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.3 GUID eines entfernten Participants wird nicht geprüft

**Spec:** §7.4.3, S. 55 — wie oben.

**Repo:** `crates/security-pki/src/plugin.rs` (`check_pdata`,
`validate_remote_identity`: `remote_guids`)

**Tests:**
`crates/security-pki/src/plugin.rs::tests::a_request_with_a_pdata_guid_not_bound_to_its_certificate_is_refused`,
`crates/security-pki/src/plugin.rs::tests::a_request_from_a_participant_discovered_under_another_guid_is_refused`,
`crates/security-pki/src/plugin.rs::tests::a_reply_with_a_pdata_guid_not_bound_to_its_certificate_is_refused`

**Status:** `done` — die GUID in `c.pdata` von Anfrage und Antwort muss aus dem
SubjectName des Zertifikats abgeleitet sein und dem Präfix gleichen, unter dem
der Participant entdeckt wurde (`validate_remote_identity`).

### §7.4.4 KeyHash in der Inline-QoS bei verschlüsselter Nutzlast

**Spec:** §7.4.4, S. 55 — "(1) The KeyHash shall be included in the Inline
Qos."

**Repo:** `crates/rtps/src/reliable_writer.rs::compact_data_prefix` (Z. 1828,
`[PID_KEY_HASH, PID_SENTINEL]` für jede Änderung mit Schlüssel);
`crates/dcps/src/runtime.rs::key_hash_of_keyless_write` (Z. 14514)

**Tests:** `crates/rtps/src/reliable_writer.rs::tests::keyed_alive_preserves_inline_hash_on_data_fragments_and_rebuilds`,
`crates/dcps/src/runtime.rs::tests::alive_sample_without_key_hash_gets_the_types_computed_hash`

**Status:** done — jede Änderung eines Typs mit Schlüssel trägt
`PID_KEY_HASH`, unabhängig vom Schutz der Nutzlast.

### §7.4.4 KeyHash als MD5 unabhängig von der Schlüssellänge

**Spec:** §7.4.4, S. 55–56 — "(2) The KeyHash shall be computed as the 128 bit
MD5 Digest (IETF RFC 1321) applied to the CDR Big- Endian encapsulation of all
the Key fields in sequence. Unlike the rule stated in sub clause 9.6.3.3 [...]
the MD5 hash shall be used regardless of the maximum-size of the serialized
key."

**Repo:** `crates/dcps/src/runtime.rs::payload_key_hash`,
`write_user_sample_with_key`, `write_user_lifecycle_with`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_key_hash_of_an_encrypted_payload_is_the_md5_of_the_key`

**Status:** `done` — unter `data_protection_kind` ENCRYPT tragen Samples und
Dispose/Unregister den MD5 des Key-Holders, auch bei Schlüsseln bis 16
Oktette. Ein Schreibvorgang mit unbekanntem Key-Holder wird mit
`NotAllowedBySecurity` abgewiesen. Ein Typ ohne Schlüssel bekommt keinen
KeyHash. Gemessen: offen (Security-Matrix gegen Cyclone, Fast DDS, OpenDDS).

### §7.4.5 PartitionQos eines Publishers unveränderlich

**Spec:** §7.4.5, S. 56 — "The DDS implementation shall not allow a Publisher
to change PartitionQos policy after the Publisher has been enabled if it
contains any DataWriter that meets the following two criteria: (1) The
TopicSecurityAttributes for the Topic associated with the DataWriter have
is_read_protected set to TRUE. (2) The DataWriter has the DurabilityQos policy
kind set to something other than VOLATILE."

**Repo:** `crates/dcps/src/runtime.rs::check_user_writer_partition_change`;
`crates/dcps/src/publisher.rs` (`set_qos`)

**Tests:** `crates/dcps/src/runtime.rs::tests::a_publisher_partition_stays_with_a_durable_read_protected_writer`

**Status:** `done` — `Publisher::set_qos` weist die Änderung der PARTITION
eines aktivierten Publishers mit `ImmutablePolicy` ab, wenn er einen
DataWriter mit Durability ungleich VOLATILE auf einem Topic mit
`enable_read_access_control` enthält.

### §7.4.6.1 Version 2.5 von RTPS

**Spec:** §7.4.6.1, S. 56 — "The RTPS Major version number shall be set to 2
and the RTPS Minor version number shall be set to 5."

**Repo:** `crates/rtps/src/wire_types.rs::ProtocolVersion::V2_5` (Z. 55)

**Tests:** `crates/rtps/src/wire_types.rs::tests::protocol_version_default_is_2_5`

**Status:** done

### §7.4.6.2 RTPS Secure Submessage Elements

**Spec:** §7.4.6.2, S. 56 — "This specification introduces new RTPS
SubmessageElements that may appear inside RTPS Submessages."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung zu §7.4.6.2.1 bis .4.

### §7.4.6.2.1 CryptoTransformIdentifier als Submessage-Element

**Spec:** §7.4.6.2.1, S. 56–57 — "all Cryptographic plugin implementations
shall be set in a way that allows the operations preprocess_secure_submsg,
decode_datareader_submessage, decode_datawriter_submessage, and
decode_serialized_payload to uniquely recognize the cryptographic material
they shall use to decode the message, or recognize that they do not have the
necessary key material."

**Repo:** `crates/security-crypto/src/plugin.rs` (`remote_by_key_id` mit dem
Schlüssel `(Sender, transformation_key_id, Revision)`, `decode_data_by_key_id`)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_key_is_found_by_sender_key_id_and_revision`

**Status:** `done` — der entfernte Schlüssel wird über (Handle des Senders,
`transformation_key_id`, Revision) abgelegt und gesucht; zwei Peers mit
derselben `transformation_key_id` überschreiben sich nicht (`409b63fcd0d`). Test
grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.2.2 CryptoContent

**Spec:** §7.4.6.2.2, S. 57 — "The CryptoContent submessage element is used to
wrap a SerializedPayload, an RTPS Submessage, or a complete RTPS Message.
[...] The specific format of this shall be defined by each Cryptographic
plugin implementation."

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (Z. 1106,
`content.length` und Chiffrat)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`

**Status:** done — Format des eingebauten Plugins in §10.5.

### §7.4.6.2.3 CryptoHeader beginnt mit dem CryptoTransformIdentifier

**Spec:** §7.4.6.2.3, S. 57 — "The CryptoHeader submessage element shall
extend the CryptoTransformIdentifier element. Consequently, the leading bytes
in the CryptoHeader shall encode the CryptoTransformIdentifier".

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(Z. 278, `transform_kind`, `transformation_key_id`, `session_id`,
`iv_suffix`)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.2.4 CryptoFooter

**Spec:** §7.4.6.2.4, S. 57 — "The CryptoFooter submessage element is used as
postfix to wrap a SerializedPayload, an RTPS Submessage, or a complete RTPS
Message."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116, `common_mac` und Anzahl der empfängerspezifischen MACs)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/security-crypto/src/crypto_transform.rs::tests::footer_round_trip_with_receivers`

**Status:** done — Format in §10.5.

### §7.4.6.3 SecureBodySubMsg: Inhalt (Table 4)

**Spec:** §7.4.6.3.2, S. 58, Table 4 — `SEC_BODY SubmessageKind`,
`submessageLength ushort`, `EndianessFlag SubmessageFlag`, `crypto_content
CryptoContent`.

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (Z. 1106)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`

**Status:** done

### §7.4.6.3.3 SecureBodySubMsg: Validity

**Spec:** §7.4.6.3.3, S. 59 — "The RTPS Submessage is invalid if the
submessageLength in the Submessage header is too small."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_secure_body_with_too_small_a_length_is_invalid`

**Status:** `done` — ein SEC_BODY, dessen Länge kleiner ist als `content.length`
verlangt, wird im Codec der Laufzeit abgewiesen (`66307936260`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.3.4 SecureBodySubMsg: Logical Interpretation

**Spec:** §7.4.6.3.4, S. 59 — "A SecureBodySubMsg may wrap a single RTPS
Submessage or a whole RTPS Message."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116), `assemble_srtps_message` (Z. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done — bei GMAC (Ids 0x01, 0x03) entfällt die SEC_BODY, die
Submessages stehen im Klartext zwischen Präfix und Postfix (wie Cyclone).

### §7.4.6.4 SecurePrefixSubMsg: Inhalt (Table 5)

**Spec:** §7.4.6.4.2, S. 59, Table 5 — `SEC_PREFIX SubmessageKind`,
`submessageLength`, `EndianessFlag`, `transformation_id
CryptoTransformIdentifier`, `plugin_crypto_header_extra octet[]`.

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116, 20-Oktett-CryptoHeader nach dem Submessage-Header)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.4.3 SecurePrefixSubMsg: Validity

**Spec:** §7.4.6.4.3, S. 59 — "The RTPS Submessage is invalid if the
submessageLength in the Submessage header is too small."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_secure_prefix_with_too_small_a_length_is_invalid`

**Status:** `done` — ein SEC_PREFIX, dessen Länge keine 20 Oktette CryptoHeader
trägt, wird abgewiesen (`66307936260`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.4.4 SecurePrefixSubMsg: genau eine Submessage, dann SEC_POSTFIX

**Spec:** §7.4.6.4.4, S. 59 — "A SecurePrefixSubMsg shall be followed by a
single RTPS Submessage which itself shall be followed by a
SecurePostfixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`
(Z. 1144, Reihenfolge Präfix, Body, Postfix)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.5 SecurePostfixSubMsg: Inhalt (Table 6)

**Spec:** §7.4.6.5.2, S. 61, Table 6 — `SEC_POSTFIX SubmessageKind`,
`submessageLength`, `EndianessFlag`, `crypto_footer CryptoFooter`.

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.5.3 SecurePostfixSubMsg: Validity

**Spec:** §7.4.6.5.3, S. 61 — "The RTPS Submessage is invalid if there is no
SecurePrefixSubMsg. Immediately before the RTPS submessage that precedes the
SecurePostfixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_secure_postfix_without_a_prefix_is_invalid`

**Status:** `done` — ein SEC_POSTFIX ohne SEC_PREFIX ist ungültig und wird
abgewiesen (`66307936260`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.5.4 SecurePostfixSubMsg: Logical Interpretation

**Spec:** §7.4.6.5.4, S. 61 — "The SecurePostfixSubMsg provides a way to
authenticate the validity and origin of the RTPS SubMessage that precedes the
SecurePrefixSubMsg. The Cryptographic transformation applied is identified in
the related SecurePrefixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_secure_submessages`
(Z. 1144, `common_mac` aus dem Postfix, Transformation aus dem Präfix)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.6.6 SecureRTPSPrefixSubMsg: Inhalt (Table 7)

**Spec:** §7.4.6.6.2, S. 61–62, Table 7 — `SRTPS_PREFIX`, `submessageLength`,
`EndianessFlag (E)`, `AdditionalAuthenticatedDataFlag (A)`,
`PreSharedKeyFlag (P)`, `transformation_id`, `plugin_crypto_header_extra`.

**Repo:** `crates/security-crypto/src/plugin.rs` (`parse_srtps`, Flags A und P
gelesen, `assemble_srtps_message`);
`crates/security-rtps/src/srtps.rs::PRE_SHARED_KEY_FLAG` (0x04)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_message_with_the_flag_a_is_checked_with_its_header`,
`crates/security-crypto/src/plugin.rs::tests::the_header_extension_is_adjusted_for_the_aad_and_the_decoded_message`,
`crates/security-rtps/src/srtps.rs::tests::psk_encode_sets_pre_shared_key_flag`

**Status:** `done` — E und der CryptoHeader werden geschrieben; A und P werden
gelesen (`9732105bf3e`, `a01b12a89e9`, vor diesem Paket; die Einträge waren
veraltet). Der Sender setzt A nicht: er bindet den RTPS-Header nicht in die AAD
ein, was die Spec freistellt. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.6 SRTPS_PREFIX: Flag A nicht ausgewertet

**Spec:** §7.4.6.6.4, S. 62 — "If the AdditionalAuthenticatedDataFlag is set
the authentication tag(s) present in the SecureRTPSPostfixSubMsg include also
the RTPS Header and RTPS HeaderExtension as 'Additional Authenticated Data'
(AAD)."

**Repo:** `crates/security-crypto/src/plugin.rs` (`parse_srtps`,
`open_rtps_cyclone` mit der AAD)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_message_with_the_flag_a_is_checked_with_its_header`

**Status:** `done` — eine Nachricht mit A=1 wird gegen RTPS-Header und
HeaderExtension als AAD geprüft (`9732105bf3e`, vor diesem Paket; der Eintrag
war veraltet). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.6 SRTPS_PREFIX: Flag P

**Spec:** §7.4.6.6.2, Table 7, S. 62 — "PreSharedKeyFlag (P) [...] P =
SubmessageHeader.flags & 0x04 Indicates that the RTPS message is protected
using a Pre-Shared-Key".

**Repo:** `crates/security-rtps/src/srtps.rs::PRE_SHARED_KEY_FLAG` (0x04),
`srtps_psk_flag`; `crates/security-crypto/src/plugin.rs` (`parse_srtps`)

**Tests:**
`crates/security-rtps/src/srtps.rs::tests::psk_encode_sets_pre_shared_key_flag`

**Status:** `done` — P ist das Bit 0x04; der PSK-Codec setzt und liest es, der
Codec der Laufzeit liest es (`a01b12a89e9`, `9732105bf3e`, vor diesem Paket; der
Eintrag war veraltet). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.6.6.3 SecureRTPSPrefixSubMsg: Validity

**Spec:** §7.4.6.6.3, S. 62 — "The RTPS Submessage is invalid if the
submessageLength in the Submessage header is too small. The
SecureRTPSPrefixSubMsg shall immediately follow the RTPS Header."

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::transform_inbound`
(Z. 1400, SRTPS_PREFIX nur an Oktett 20); `crates/security-crypto/src/plugin.rs::parse_srtps_body`
(Z. 995, „expected SRTPS_PREFIX(20)“)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.6.6.4 SecureRTPSPrefixSubMsg: Logical Interpretation

**Spec:** §7.4.6.6.4, S. 62 — "A SecureRTPSPrefixSubMsg shall be followed by a
list of RTPS Submessages which themselves shall be followed by a
SecureRTPSPostfixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_srtps_message`
(Z. 956), `parse_srtps_body` (Z. 995)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`,
`crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.6.7 SecureRTPSPostfixSubMsg: Inhalt (Table 8)

**Spec:** §7.4.6.7.2, S. 63, Table 8 — `SRTPS_POSTFIX`, `submessageLength`,
`EndianessFlag`, `crypto_footer CryptoFooter`.

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_srtps_message`
(Z. 956, `common_mac` und Anzahl 0)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.6.7.3 SecureRTPSPostfixSubMsg: Validity

**Spec:** §7.4.6.7.3, S. 63 — "The RTPS SecureRTPSPostfixSubMsg is invalid if
there is no SecureRTPSPrefixSubMsg following the RTPS Header."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_srtps_body` (Z. 995)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done — die Auswertung beginnt nur, wenn Oktett 20 SRTPS_PREFIX
ist.

### §7.4.6.7.4 SecureRTPSPostfixSubMsg: Logical Interpretation

**Spec:** §7.4.6.7.4, S. 63 — "The SecureRTPSPostfixSubMsg provides a way to
authenticate the validity and origin of the list of RTPS Submessages between
the related SecureRTPSPrefixSubMsg and the SecureRTPSPrefixSubMsg."

**Repo:** `crates/security-crypto/src/plugin.rs::parse_srtps_body` (Z. 995)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.7 Mapping to UDP/IP PSM

**Spec:** §7.4.7, S. 63 — "Sub clause 7.4.7 does the same thing for the new
RTPS submessage elements and submessages introduced by the DDS Security
specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.4.7.1 Table 9 — SEDPbuiltinPublicationsSecureWriter {{ff,00,03},c2}

**Spec:** §7.4.7.1, Table 9, S. 63 — "ENTITYID_SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER
{{ff, 00, 03}, c2}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER`
(Z. 534)

**Tests:** `crates/rtps/src/wire_types.rs::tests::secure_publications_writer_layout`

**Status:** done

### §7.4.7.1 Table 9 — SEDPbuiltinPublicationsSecureReader {{ff,00,03},c7}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 00, 03}, c7}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — SEDPbuiltinSubscriptionsSecureWriter {{ff,00,04},c2}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 00, 04}, c2}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — SEDPbuiltinSubscriptionsSecureReader {{ff,00,04},c7}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 00, 04}, c7}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — BuiltinParticipantMessageSecureWriter {{ff,02,00},c2}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 02, 00}, c2}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — BuiltinParticipantMessageSecureReader {{ff,02,00},c7}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 02, 00}, c7}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — BuiltinParticipantStatelessMessageWriter {{00,02,01},c3}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{00, 02, 01}, c3}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`
(Z. 567)

**Tests:** `crates/rtps/src/wire_types.rs::tests::stateless_writer_is_no_key_kind`

**Status:** done

### §7.4.7.1 Table 9 — BuiltinParticipantStatelessMessageReader {{00,02,01},c4}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{00, 02, 01}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — BuiltinParticipantVolatileMessageSecureWriter {{ff,02,02},c3}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 02, 02}, c3}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER`
(Z. 579)

**Tests:** `crates/rtps/src/wire_types.rs::tests::volatile_message_secure_entity_ids_match_spec_and_cyclone`

**Status:** done

### §7.4.7.1 Table 9 — BuiltinParticipantVolatileMessageSecureReader {{ff,02,02},c4}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 02, 02}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER`
(Z. 587)

**Tests:** `crates/rtps/src/wire_types.rs::tests::volatile_message_secure_entity_ids_match_spec_and_cyclone`

**Status:** done

### §7.4.7.1 Table 9 — SPDPbuiltinParticipantsSecureWriter {{ff,01,01},c2}

**Spec:** §7.4.7.1, Table 9, S. 64 — "ENTITYID_SPDP_RELIABLE_BUILTIN_PARTICIPANT_SECURE_WRITER
{{ff, 01, 01}, c2}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — SPDPbuiltinParticipantsSecureReader {{ff,01,01},c7}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 01, 01}, c7}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER`
(Z. 599)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_secure_reader_layout`

**Status:** done

### §7.4.7.1 Table 9 — Oktette ohne Test

**Spec:** §7.4.7.1, Table 9, S. 63–64 — die sieben oben als `partial`
geführten Zeilen.

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — TypeLookupServiceRequestSecureWriter {{ff,03,00},c3}

**Spec:** §7.4.7.1, Table 9, S. 64 — "ENTITYID_TL_SVC_REQ_SECURE _WRITER
{{ff, 03, 00}, c3}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::TL_SVC_REQ_SECURE_WRITER`,
`TL_SVC_REQ_SECURE_READER`, `TL_SVC_REPLY_SECURE_WRITER`,
`TL_SVC_REPLY_SECURE_READER`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die Konstante besteht mit den Oktetten der Tabelle
(`7b1de8f5255`, Test `a5b73d27f19`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — TypeLookupServiceRequestSecureReader {{ff,03,00},c4}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 03, 00}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::TL_SVC_REQ_SECURE_WRITER`,
`TL_SVC_REQ_SECURE_READER`, `TL_SVC_REPLY_SECURE_WRITER`,
`TL_SVC_REPLY_SECURE_READER`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die Konstante besteht mit den Oktetten der Tabelle
(`7b1de8f5255`, Test `a5b73d27f19`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — TypeLookupServiceReplySecureWriter {{ff,03,01},c3}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 03, 01}, c3}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::TL_SVC_REQ_SECURE_WRITER`,
`TL_SVC_REQ_SECURE_READER`, `TL_SVC_REPLY_SECURE_WRITER`,
`TL_SVC_REPLY_SECURE_READER`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die Konstante besteht mit den Oktetten der Tabelle
(`7b1de8f5255`, Test `a5b73d27f19`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.1 Table 9 — TypeLookupServiceReplySecureReader {{ff,03,01},c4}

**Spec:** §7.4.7.1, Table 9, S. 64 — "{{ff, 03, 01}, c4}".

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::TL_SVC_REQ_SECURE_WRITER`,
`TL_SVC_REQ_SECURE_READER`, `TL_SVC_REPLY_SECURE_WRITER`,
`TL_SVC_REPLY_SECURE_READER`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die Konstante besteht mit den Oktetten der Tabelle
(`7b1de8f5255`, Test `a5b73d27f19`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.2 Mapping of the CryptoTransformIdentifier Type

**Spec:** §7.4.7.2, S. 64 — "The UDP/IP PSM maps the CryptoTransformIdentifier
to the IDL definition in 7.3.20."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`
(Z. 99)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::transform_identifier_round_trip`

**Status:** done

### §7.4.7.3 CryptoHeader: Wire Representation

**Spec:** §7.4.7.3, S. 64–65 — "The CryptoHeader shall start with the
CryptoTransformIdentifier and be followed by a plugin-specific
plugin_crypto_header_extra returned by the encoding transformation." Layout:
`transformation_kind` (4), `transformation_key_id` (4),
`plugin_crypto_header_extra`.

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(Z. 278); `crates/security-crypto/src/crypto_transform.rs::CryptoHeader`
(Z. 150)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::header_round_trip_with_full_iv`,
`crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.4 CryptoFooter: Wire Representation

**Spec:** §7.4.7.4, S. 65 — "The CryptoFooter contains a plugin-specific
plugin_crypto_footer returned by the encoding transformation."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoFooter`
(Z. 207); `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::footer_round_trip_no_receivers`,
`crates/security-crypto/src/crypto_transform.rs::tests::footer_rejects_oversized_receiver_count_cleanly`

**Status:** done

### §7.4.7.5 SecureBodySubMsg: Wire Representation und Id 0x30

**Spec:** §7.4.7.5, S. 65 — "| SEC_BODY |X|X|X|X|X|X|X|E| octetsToNextHeader |
[...] CryptoContent crypto_content" — "The SecureBodySubMsg shall have the
submessageId set to the value 0x30."

**Repo:** `crates/security-crypto/src/plugin.rs::SEC_BODY_ID` (Z. 943),
`push_sec_body` (Z. 1106)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`

**Status:** done

### §7.4.7.5.3 SecureBodySubMsg: nur EndiannessFlag

**Spec:** §7.4.7.5.3, S. 65 — "The SecureBodySubMsg only uses the
EndiannessFlag."

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (Z. 1106,
Flag `0x01`)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`

**Status:** done — ZeroDDS schreibt stets E=1; beim Empfang folgt die Länge
dem Flag (`parse_secure_submessages`).

### §7.4.7.6 SecurePrefixSubMsg: Wire Representation und Id 0x31

**Spec:** §7.4.7.6, S. 65–66 — "| SEC_PREFIX |X|X|X|X|X|X|X|E| [...]
CryptoHeader crypto_header" — "submessageId set to the value 0x31".

**Repo:** `crates/security-crypto/src/plugin.rs::SEC_PREFIX_ID` (Z. 942),
`assemble_secure_submessages` (Z. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.6.3 SecurePrefixSubMsg: nur EndiannessFlag

**Spec:** §7.4.7.6.3, S. 66 — "The SecurePrefixSubMsg only uses the
EndiannessFlag."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.7 SecurePostfixSubMsg: Wire Representation und Id 0x32

**Spec:** §7.4.7.7, S. 66 — "| SEC_POSTFIX |X|X|X|X|X|X|X|E| [...]
CryptoFooter crypto_footer" — "submessageId set to the value 0x32".

**Repo:** `crates/security-crypto/src/plugin.rs::SEC_POSTFIX_ID` (Z. 944),
`assemble_secure_submessages` (Z. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.7.3 SecurePostfixSubMsg: nur EndiannessFlag

**Spec:** §7.4.7.7.3, S. 66 — "The SecurePostfixSubMsg only uses the
EndiannessFlag."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §7.4.7.8 SecureRTPSPrefixSubMsg: Wire Representation und Id 0x33

**Spec:** §7.4.7.8, S. 66 — "| SRTPS_PREFIX |X|X|X|X|X|P|A|E| [...]
CryptoHeader crypto_header" — "submessageId set to the value 0x33".

**Repo:** `crates/security-crypto/src/plugin.rs::SRTPS_PREFIX_ID` (Z. 951),
`assemble_srtps_message` (Z. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.7.8.3 SecureRTPSPrefixSubMsg: Flags E, A, P

**Spec:** §7.4.7.8.3, S. 66–67 — "EndiannessFlag (E) [...]
AdditionalAuthenticatedDataFlag (A) [...] PreSharedKeyFlag (P): Indicates that
the RTPS message is protected using a Pre-Shared-Key."

**Repo:** `crates/security-crypto/src/plugin.rs` (`parse_srtps`, Flags A und P
gelesen, `assemble_srtps_message`);
`crates/security-rtps/src/srtps.rs::PRE_SHARED_KEY_FLAG` (0x04)

**Tests:**
`crates/security-crypto/src/plugin.rs::tests::a_message_with_the_flag_a_is_checked_with_its_header`,
`crates/security-crypto/src/plugin.rs::tests::the_header_extension_is_adjusted_for_the_aad_and_the_decoded_message`,
`crates/security-rtps/src/srtps.rs::tests::psk_encode_sets_pre_shared_key_flag`

**Status:** `done` — E und der CryptoHeader werden geschrieben; A und P werden
gelesen (`9732105bf3e`, `a01b12a89e9`, vor diesem Paket; die Einträge waren
veraltet). Der Sender setzt A nicht: er bindet den RTPS-Header nicht in die AAD
ein, was die Spec freistellt. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.4.7.9 SecureRTPSPostfixSubMsg: Wire Representation und Id 0x34

**Spec:** §7.4.7.9, S. 67 — "| SRTPS_POSTFIX |X|X|X|X|X|X|X|E| [...]" —
"submessageId set to the value 0x34".

**Repo:** `crates/security-crypto/src/plugin.rs::SRTPS_POSTFIX_ID` (Z. 952),
`assemble_srtps_message` (Z. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.4.7.9.3 SecureRTPSPostfixSubMsg: nur EndiannessFlag

**Spec:** §7.4.7.9.3, S. 67 — "The SecureRTPSPostfixSubMsg only uses the
EndiannessFlag."

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_srtps_message`
(Z. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`

**Status:** done

### §7.5 DDS Support for Security Plugin Information Exchange

**Spec:** §7.5, S. 67 — "Where possible, this specification tries to reuse and
extend existing DDS concepts and facilities so that they can fulfill the needs
of the security plugins".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.5.1.1 Background (Non-Normative)

**Spec:** §7.5.1.1, S. 67–68 — Begründung zusätzlicher geschützter
Topics der Discovery neben den bestehenden.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Überschrift sagt „Non-Normative“.

### §7.5.1.2 Extending the Data Types used by DDS Discovery

**Spec:** §7.5.1.2, S. 68–69 — "All that is required is to serialize the
specific data types defined here according to the format described in the
DDS-XTYPES specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die einzelnen Member sind in §7.5.1.4 bis
§7.5.1.8 geführt.

### §7.5.1.3 Reserved RTPS parameter IDs

**Spec:** §7.5.1.3, S. 69 — "This specification reserves the RTPS Simple
Discovery Protocol ParameterIDs in the range: 0x1000 to 0x1FFF and 0x5000 to
0x5FFF."

**Repo:** `crates/rtps/src/parameter_list.rs` (in den beiden Bereichen nur die
Ids der Spec: `0x1001`, `0x1002`, `0x1004`, `0x1005`, `0x1006`, `0x1010` bis
`0x1013`)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::is_standard_pid_recognises_dds_security_pids`

**Status:** done

### §7.5.1.4 Table 10 — identity_token, PID_IDENTITY_TOKEN 0x1001

**Spec:** §7.5.1.4, Table 10, S. 69 — "identity_token IdentityToken
PID_IDENTITY_TOKEN 0x1001".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::IDENTITY_TOKEN` (Z. 159);
`crates/rtps/src/participant_data.rs::identity_token` (Z. 241)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_identity_token_pid_roundtrip`

**Status:** done

### §7.5.1.4 Table 10 — permissions_token, PID_PERMISSIONS_TOKEN 0x1002

**Spec:** §7.5.1.4, Table 10, S. 69 — "permissions_token PermissionsToken
PID_PERMISSIONS_TOKEN 0x1002".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::PERMISSIONS_TOKEN` (Z. 164);
`crates/rtps/src/participant_data.rs::permissions_token` (Z. 244)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_permissions_token_pid_roundtrip`

**Status:** done

### §7.5.1.4 Table 10 — property, PID_PROPERTY_LIST 0x0059

**Spec:** §7.5.1.4, Table 10, S. 69 — "property PropertyQosPolicy
PID_PROPERTY_LIST 0x0059".

**Repo:** `crates/rtps/src/property_list.rs::WirePropertyList` (binäre
Einträge); `crates/rtps/src/participant_data.rs`

**Tests:**
`crates/rtps/src/property_list.rs::tests::binary_properties_cross_the_wire`,
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** `done` — die `BinaryPropertySeq` wird mit den Einträgen geschrieben
und gelesen (`928d39304cd`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 PID_PROPERTY_LIST: binary_value geht verloren

**Spec:** §7.5.1.4, S. 70 — "Only the Property_t and BinaryProperty_t elements
having the propagate member set to TRUE are serialized."

**Repo:** `crates/rtps/src/property_list.rs::WirePropertyList::binary_entries`

**Tests:** `crates/rtps/src/property_list.rs::tests::binary_properties_cross_the_wire`

**Status:** `done` — `binary_value` wird geschrieben und gelesen.

### §7.5.1.4 Table 10 — protection_info, PID_PARTICIPANT_SECURITY_PROTECTION_INFO 0x1005

**Spec:** §7.5.1.4, Table 10, S. 69 — "protection_info
ParticipantSecurityProtectionInfo PID_PARTICIPANT_SECURITY_PROTECTION_INFO
0x1005".

**Repo:** `crates/rtps/src/participant_security_info.rs` (drei Member,
`ParticipantSecurityAttributesMaskExt`)

**Tests:**
`crates/rtps/src/participant_security_info.rs::tests::the_optional_attributes_are_the_third_member`

**Status:** `done` — `ParticipantSecurityProtectionInfo` hat die drei Member der
Spec (`470192aa99a`, vor diesem Paket; die Einträge waren veraltet). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 10 — available_builtin_endpoints_ext, PID 0x1007

**Spec:** §7.5.1.4, Table 10, S. 70 — "available_builtin_endpoints_ext
AvailableBuiltinEndpointsExtSet_t PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT 0x1007".

**Repo:** `crates/rtps/src/participant_data.rs`
(`pid::AVAILABLE_BUILTIN_ENDPOINTS_EXT` 0x1007, Feld
`available_builtin_endpoints_ext`, fehlend = 0)

**Tests:**
`crates/rtps/src/participant_data.rs::tests::available_builtin_endpoints_ext_roundtrip`

**Status:** `done` — der Parameter 0x1007 besteht; fehlt er, ist der Wert 0
(`7b1de8f5255`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 10 — digital_signature, PID 0x1010

**Spec:** §7.5.1.4, Table 10, S. 70 — "digital_signature
ParticipantSecurityDigitalSignatureAlgorithmInfo
PID_PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO 0x1010".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::PARTICIPANT_SECURITY_DIGITAL_SIGNATURE_ALGORITHM_INFO`
(Z. 173)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_sig_algo_info_roundtrip`

**Status:** done — PID und Codec; dass die Laufzeit den Parameter nie setzt,
steht bei §7.3.11.

### §7.5.1.4 Table 10 — key_establishment, PID 0x1011

**Spec:** §7.5.1.4, Table 10, S. 70 — "key_establishment
ParticipantSecurityKeyEstablishmentAlgorithmInfo
PID_PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO 0x1011".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::PARTICIPANT_SECURITY_KEY_ESTABLISHMENT_ALGORITHM_INFO`
(Z. 178)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_kx_algo_info_roundtrip`

**Status:** done

### §7.5.1.4 Table 10 — symmetric_cipher, PID 0x1012

**Spec:** §7.5.1.4, Table 10, S. 70 — "symmetric_cipher
ParticipantSecuritySymmetricCipherAlgorithmInfo
PID_PARTICIPANT_SECURITY_BUILTIN_EP_SYMMETRIC_CIPHER_ALGORITHM_INFO 0x1012".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::PARTICIPANT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO`
(Z. 184)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_sym_cipher_algo_info_roundtrip`

**Status:** done

### §7.5.1.4 Fehlendes available_builtin_endpoints_ext gilt als 0

**Spec:** §7.5.1.4, S. 70 — "If the member available_builtin_endpoints_ext is
not present in the ParticipantBuiltinTopic, the receiver shall interpret the
value of the member to be 0x00000000."

**Repo:** `crates/rtps/src/participant_data.rs`
(`pid::AVAILABLE_BUILTIN_ENDPOINTS_EXT` 0x1007, Feld
`available_builtin_endpoints_ext`, fehlend = 0)

**Tests:**
`crates/rtps/src/participant_data.rs::tests::available_builtin_endpoints_ext_roundtrip`

**Status:** `done` — der Parameter 0x1007 besteht; fehlt er, ist der Wert 0
(`7b1de8f5255`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Fehlendes digital_signature gilt als Default

**Spec:** §7.5.1.4, S. 70 — "If the member digital_signature is not present
[...] the receiver shall interpret the value of the member to be the default
defined in clause 7.3.11.2."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — fehlend: der Vorgabewert von §7.3.11.2.

### §7.5.1.4 Fehlendes key_establishment gilt als Default

**Spec:** §7.5.1.4, S. 70 — "If the member key_establishment is not present in
the ParticipantBuiltinTopic, the receiver shall interpret the value of the
member to be the default defined in clause 7.3.13.2."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — fehlend: §7.3.12.2.

### §7.5.1.4 Fehlendes symmetric_cipher gilt als Default

**Spec:** §7.5.1.4, S. 70 — "If the member symmetric_cipher is not present
[...] the default defined in clause 7.3.13.2."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — fehlend: §7.3.13.2.

### §7.5.1.4 Nur Properties mit propagate=TRUE werden serialisiert

**Spec:** §7.5.1.4, S. 70 — "Only the Property_t and BinaryProperty_t elements
having the propagate member set to TRUE are serialized."

**Repo:** `crates/dcps/src/runtime.rs::update_participant_properties`,
`RuntimeConfig.participant_properties`; `crates/rtps/src/participant_data.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_propagated_participant_properties_reach_the_peer`,
`crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** `done` — nur Einträge der `PropertyQosPolicy` des Participants mit
`propagate = TRUE` gehen als `PID_PROPERTY_LIST` in den SPDP-Beacon; eine
Änderung geht sofort hinaus; eine Liste, die die Leitung nicht tragen kann, wird
abgewiesen (`a5b73d27f19`, `e357ab7a5ac`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.1.4 propagate wird nicht serialisiert

**Spec:** §7.5.1.4, S. 70 — "the serialization of the Property_t and
BinaryProperty_t elements shall omit the serialization of the propagate
member."

**Repo:** `crates/rtps/src/property_list.rs::WireProperty` (Z. 52, ohne
`propagate`)

**Tests:** `crates/rtps/src/property_list.rs::tests::single_property_roundtrip_le`

**Status:** done

### §7.5.1.4 Empfänger setzt propagate auf TRUE

**Spec:** §7.5.1.4, S. 70 — "Even if it is not present in the serialized data,
the receiver will set the propagate member to TRUE."

**Repo:** `crates/rtps/src/participant_data.rs::properties` (Z. 235,
`WirePropertyList`)

**Tests:** `crates/rtps/src/participant_data.rs::tests::participant_data_properties_roundtrip`

**Status:** done — die empfangenen Properties stehen als `WireProperty` ohne
`propagate`-Member in `ParticipantBuiltinTopicData`; sie werden nicht in eine
lokale `PropertyList` übernommen.

### §7.5.1.4 PID_PROPERTY_LIST trägt PropertySeq

**Spec:** §7.5.1.4, S. 70 — "Note that according to DDS-RTPS the
PID_PROPERTY_LIST is associated with a single PropertySeq rather than the
PropertyQosPolicy [...] This does not cause any interoperability problems".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis ohne Anforderung.

### §7.5.1.4 Table 11 — SEDPbuiltinPublicationsSecureWriter, Bit 16

**Spec:** §7.5.1.4, Table 11, S. 71 — "SEDPbuiltinPublicationsSecureWriter
(0x00000001 << 16)" — "These bits shall be set on the
availableBuiltinEndpoints."

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PUBLICATIONS_SECURE_WRITER`
(Z. 94); gesetzt in `crates/dcps/src/runtime.rs` (Z. 8670)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — SEDPbuiltinPublicationsSecureReader, Bit 17

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 17)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PUBLICATIONS_SECURE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — SEDPbuiltinSubscriptionsSecureWriter, Bit 18

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 18)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::SUBSCRIPTIONS_SECURE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — SEDPbuiltinSubscriptionsSecureReader, Bit 19

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 19)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::SUBSCRIPTIONS_SECURE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantMessageSecureWriter, Bit 20

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 20)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_MESSAGE_SECURE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantMessageSecureReader, Bit 21

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 21)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_MESSAGE_SECURE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantStatelessMessageWriter, Bit 22

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 22)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantStatelessMessageReader, Bit 23

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 23)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_STATELESS_MESSAGE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantVolatileMessageSecureWriter, Bit 24

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 24)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — BuiltinParticipantVolatileMessageSecureReader, Bit 25

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 25)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done

### §7.5.1.4 Table 11 — SPDPbuiltinParticipantSecureWriter, Bit 26

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 26)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_SECURE_WRITER`

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done — mit `enable_secure_spdp = false`
(`crates/dcps/src/runtime.rs` Z. 1589) bleiben die Bits 26 und 27 gesetzt,
obwohl die Endpunkte nicht laufen (Kommentar Z. 1578–1579).

### §7.5.1.4 Table 11 — SPDPbuiltinParticipantSecureReader, Bit 27

**Spec:** §7.5.1.4, Table 11, S. 71 — "(0x00000001 << 27)".

**Repo:** `crates/rtps/src/participant_data.rs::endpoint_flag::PARTICIPANT_SECURE_READER`
(Z. 127)

**Tests:** `crates/rtps/src/participant_data.rs::tests::endpoint_flag_bit_positions_match_spec`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done — wie Bit 26.

### §7.5.1.4 Secure Endpunkte für TypeLookup in available_builtin_endpoints_ext ankündigen

**Spec:** §7.5.1.4, S. 71 — "DDS-Security implementations that support
DDS-XTYPES shall advertise the availability of the Secure TypeLookup Built-In
Endpoints using the Parameter with ID PID_AVAILABLE_BUILTIN_ENDPOINTS_EXT".

**Repo:** `crates/dcps/src/runtime.rs` (SPDP-Beacon:
`available_builtin_endpoints_ext = ALL_TYPE_LOOKUP_SECURE`)

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_secure_type_lookup_service_is_protected_per_submessage`,
`crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — ein gesicherter Participant kündigt die vier Endpunkte an
(`a542c9f545d`). Gemessen (Linux-Testhost, sec-diag10): die Ankündigung ändert in
keiner Zelle von `common-subset`, `disc-meta-data`, `disc-data-enc`, `data-sign`
etwas (`4912df99883` gegen `32f11c44270`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 12 — TypeLookupServiceRequestSecureWriter, Bit 0

**Spec:** §7.5.1.4, Table 12, S. 71 — "TypeLookupServiceRequestSecureWriter
(0x00000001 << 0)".

**Repo:** `crates/rtps/src/participant_data.rs::builtin_endpoint_ext`

**Tests:** `crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — die Konstante besteht mit dem Bit der Tabelle
(`7b1de8f5255`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 12 — TypeLookupServiceRequestSecureReader, Bit 1

**Spec:** §7.5.1.4, Table 12, S. 71 — "(0x00000001 << 1)".

**Repo:** `crates/rtps/src/participant_data.rs::builtin_endpoint_ext`

**Tests:** `crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — die Konstante besteht mit dem Bit der Tabelle
(`7b1de8f5255`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 12 — TypeLookupServiceReplySecureWriter, Bit 2

**Spec:** §7.5.1.4, Table 12, S. 72 — "(0x00000001 << 2)".

**Repo:** `crates/rtps/src/participant_data.rs::builtin_endpoint_ext`

**Tests:** `crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — die Konstante besteht mit dem Bit der Tabelle
(`7b1de8f5255`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.4 Table 12 — TypeLookupServiceReplySecureReader, Bit 3

**Spec:** §7.5.1.4, Table 12, S. 72 — "(0x00000001 << 3)".

**Repo:** `crates/rtps/src/participant_data.rs::builtin_endpoint_ext`

**Tests:** `crates/rtps/src/participant_data.rs::tests::table_12_bits`

**Status:** `done` — die Konstante besteht mit dem Bit der Tabelle
(`7b1de8f5255`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.5 Table 13 — protection_info, PID_ENDPOINT_SECURITY_PROTECTION_INFO 0x1004

**Spec:** §7.5.1.5, Table 13, S. 72 — "protection_info EndpointSecurityInfo
PID_ENDPOINT_SECURITY_PROTECTION_INFO 0x1004".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::ENDPOINT_SECURITY_INFO`
(Z. 146); `crates/rtps/src/publication_data.rs::security_info` (Z. 279);
`crates/rtps/src/subscription_data.rs::security_info` (Z. 90); gesetzt in
`crates/dcps/src/runtime.rs::user_endpoint_security_info` (Z. 9664)

**Tests:** `crates/rtps/src/publication_data.rs::tests::security_info_roundtrip`,
`crates/rtps/src/subscription_data.rs::tests::security_info_roundtrip`

**Status:** done

### §7.5.1.5 Table 13 — symmetric_cipher, PID 0x1013

**Spec:** §7.5.1.5, Table 13, S. 72 — "symmetric_cipher
EndpointSecuritySymmetricCipherAlgorithmInfo
PID_ENDPOINT_SECURITY_SYMMETRIC_CIPHER_ALGORITHM_INFO 0x1013".

**Repo:** `crates/rtps/src/parameter_list.rs::pid::ENDPOINT_SYMMETRIC_CIPHER_ALGORITHM_INFO`; `publication_data.rs`, `subscription_data.rs`

**Tests:** `crates/rtps/src/publication_data.rs::tests::endpoint_symmetric_cipher_info_roundtrip`

**Status:** `done` — Codec und Feld; die Laufzeit sendet keines (kein Endpunkt weicht vom Vorgabewert des Participants ab).

### §7.5.1.5 Fehlendes symmetric_cipher gilt als Default

**Spec:** §7.5.1.5, S. 73 — "If the member symmetric_cipher is not present [...]
the receiver shall interpret the value of the member to be the default defined
in clause 7.3.15.2."

**Repo:** `crates/dcps/src/runtime.rs::endpoint_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::endpoints_with_an_incompatible_configuration_do_not_match`

**Status:** `done` — siehe §7.3.15.2.

### §7.5.1.6 DCPSParticipants bleibt für die Ankündigung

**Spec:** §7.5.1.6, S. 73 — "Implementations of the DDS Security shall use that
same DCPSParticipants builtin Topic to announce the DomainParticipant
information."

**Repo:** `crates/dcps/src/runtime.rs` (Beacon von SPDP über
`SPDP_BUILTIN_PARTICIPANT_WRITER`, auch mit Security)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_beacon_announces_identity_token_and_discovery_triggers_handshake`

**Status:** done

### §7.5.1.6 DCPSParticipantsSecure: Topic und Endpunkte

**Spec:** §7.5.1.6, S. 73 — "Implementations of the DDS Security specification
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

**Status:** `done` — das Topic `DCPSParticipantsSecure` besteht mit Leser, Typ
und der QoS von `DCPSPublications` (RELIABLE); der sichere SPDP-Writer
nummeriert jede Änderung und geht nicht an Multicast (`1bee936d138`,
`054dd0f3a8b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 Name des Topics „DCPSParticipantsSecure“

**Spec:** §7.5.1.6, S. 73 — "The Topic name for the DCPSParticipantsSecure
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

**Status:** `done` — das Topic `DCPSParticipantsSecure` besteht mit Leser, Typ
und der QoS von `DCPSPublications` (RELIABLE); der sichere SPDP-Writer
nummeriert jede Änderung und geht nicht an Multicast (`1bee936d138`,
`054dd0f3a8b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 ParticipantBuiltinTopicDataSecure: identity_status_token 0x1006

**Spec:** §7.5.1.6, Table 14, S. 73 — "identity_status_token
IdentityStatusToken PID_IDENTITY_STATUS_TOKEN 0x1006" — "struct
ParticipantBuiltinTopicDataSecure: ParticipantBuiltinTopicData { @id(0x1006)
@optional IdentityStatusToken identity_status_token; };"

**Repo:** `crates/discovery/src/spdp.rs::serialize` (ohne), `serialize_secure`
(mit `identity_status_token`); `crates/dcps/src/runtime.rs` (setzt ihn aus dem
Plugin); `crates/security-pki/src/plugin.rs::get_identity_status_token`

**Tests:**
`crates/discovery/src/spdp.rs::tests::the_identity_status_token_is_in_the_secure_participant_data_only`,
`crates/security-pki/src/plugin.rs::tests::the_identity_status_token_has_the_class_of_table_53`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`

**Status:** `done` — der `identity_status_token` (0x1006) steht nur im sicheren
SPDP und im Typ `ParticipantBuiltinTopicDataSecure`, nicht im Beacon von
`DCPSParticipants`; die Laufzeit setzt ihn aus dem `IdentityStatusToken` des
Plugins (Klasse `DDS:Auth:PKI-DH:1.0`, `ocsp_status` wenn eine OCSP-Antwort
vorliegt, Table 53) (`aef84a98a0b`, `257d0f8d0e6`). Neu auf der Leitung: PID
0x1006 im sicheren SPDP an authentisierte Peers; ungemessen gegen Cyclone, Fast
DDS und OpenDDS. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 identity_status_token nur im sicheren Typ und befüllt

**Spec:** §7.5.1.6, S. 73 — wie oben.

**Repo:** `crates/discovery/src/spdp.rs::serialize` (ohne), `serialize_secure`
(mit `identity_status_token`); `crates/dcps/src/runtime.rs` (setzt ihn aus dem
Plugin); `crates/security-pki/src/plugin.rs::get_identity_status_token`

**Tests:**
`crates/discovery/src/spdp.rs::tests::the_identity_status_token_is_in_the_secure_participant_data_only`,
`crates/security-pki/src/plugin.rs::tests::the_identity_status_token_has_the_class_of_table_53`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`

**Status:** `done` — der `identity_status_token` (0x1006) steht nur im sicheren
SPDP und im Typ `ParticipantBuiltinTopicDataSecure`, nicht im Beacon von
`DCPSParticipants`; die Laufzeit setzt ihn aus dem `IdentityStatusToken` des
Plugins (Klasse `DDS:Auth:PKI-DH:1.0`, `ocsp_status` wenn eine OCSP-Antwort
vorliegt, Table 53) (`aef84a98a0b`, `257d0f8d0e6`). Neu auf der Leitung: PID
0x1006 im sicheren SPDP an authentisierte Peers; ungemessen gegen Cyclone, Fast
DDS und OpenDDS. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 QoS wie DCPSPublications (RELIABLE)

**Spec:** §7.5.1.6, S. 73 — "The QoS associated with the
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

**Status:** `done` — das Topic `DCPSParticipantsSecure` besteht mit Leser, Typ
und der QoS von `DCPSPublications` (RELIABLE); der sichere SPDP-Writer
nummeriert jede Änderung und geht nicht an Multicast (`1bee936d138`,
`054dd0f3a8b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.1.6 SPDPbuiltinParticipantsSecureWriter/-Reader und EntityIds

**Spec:** §7.5.1.6, S. 73 — "The builtin DataWriter for the
DCPSParticipantsSecure Topic shall be referred to as the
SPDPbuiltinParticipantsSecureWriter [...] The RTPS EntityId_t [...] shall be
as specified in 7.5.8."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER`
(Z. 594), `SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER` (Z. 599)

**Tests:** `crates/rtps/src/wire_types.rs::tests::spdp_secure_reader_layout`

**Status:** done

### §7.5.1.6 Änderungen über DCPSParticipantsSecure senden

**Spec:** §7.5.1.6, S. 73 — "The ParticipantBuiltinTopicData contains
information, such as participant Locators, which may change at run-time. These
changes shall be sent using the DCPSParticipantsSecure builtin Topic."

**Repo:** `crates/discovery/src/spdp.rs::SpdpBeacon::secure_sequence_number`, `serialize_secure`

**Tests:** `crates/discovery/src/spdp.rs::tests::secure_spdp_takes_a_new_number_for_a_change_only`

**Status:** `done` — eine Änderung der Daten des Participants bekommt die nächste Sequenznummer des sicheren Writers; dieselben Daten erneut gesendet behalten die Nummer. HEARTBEAT und Reparatur nennen die aktuelle Nummer. Gemessen: offen (Security-Matrix, Fast DDS).

### §7.5.1.6 Löschen über DCPSParticipantsSecure senden

**Spec:** §7.5.1.6, S. 73 — "The deletion of a DomainParticipant shall also be
sent using the DCPSParticipantsSecure builtin Topic."

**Repo:** `crates/discovery/src/spdp.rs::SpdpBeacon::serialize_secure_departure`; `crates/dcps/src/runtime.rs::announce_departure`, `spdp_departed`

**Tests:** `crates/discovery/src/spdp.rs::tests::secure_spdp_departure_is_a_key_only_change_on_the_secure_writer`

**Status:** `done` — ein gesicherter Participant sendet die Abmeldung (`D=0`, `DISPOSED|UNREGISTERED`) über den sicheren Writer an die authentisierten Participants; ein Participant nimmt eine Abmeldung nur vom sicheren Writer eines authentisierten Participants an. Gemessen: offen.

### §7.5.1.6 Nach der Authentisierung DCPSParticipants-Änderungen übergehen

**Spec:** §7.5.1.6, S. 73–74 — "After authentication has completed
successfully a DomainParticipant shall ignore any changes to the
ParticipantBuiltinTopicData (including dispose messages) received on the
DCPSParticipants builtin Topic from the authenticated DomainParticipant."

**Repo:** `crates/discovery/src/spdp.rs::SpdpReader::samples_with_channel`; `crates/dcps/src/runtime.rs::handle_spdp_datagram`, `spdp_plain_is_liveliness_only`

**Tests:** `crates/discovery/src/spdp.rs::tests::samples_name_the_writer_they_came_from`, `crates/discovery/src/spdp.rs::tests::touch_renews_the_lease_of_a_known_participant_only`, `crates/dcps/src/runtime.rs::tests` (Handshake-Dispatch-Test, letzter Schritt)

**Status:** `done` — nach der Authentisierung erneuert eine Probe des plain Writers nur Lease und Liveliness des Participants; Inhalt und Abmeldung vom plain Writer werden übergangen, Änderungen kommen vom sicheren Writer.

### §7.5.1.7 DCPSPublications für nicht sensible Topics

**Spec:** §7.5.1.7, S. 74 — "Implementations of the DDS Security shall use that
same DCPSPublications Topic to communicate the DataWriter information for
Topic entities that are not considered sensitive."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — ein Endpunkt eines Topics ohne `enable_discovery_protection` geht über den plain Writer (auch das Dispose, mit dem Topic des gelöschten Endpunkts).

### §7.5.1.7 DCPSPublicationsSecure: Topic und Endpunkte

**Spec:** §7.5.1.7, S. 74 — "Implementations of the DDS Security specification
shall have an additional builtin Topic referred to as DCPSPublicationsSecure
and associated builtin DataReader and DataWriter entities".

**Repo:** `crates/discovery/src/sedp/writer.rs::SedpPublicationsWriter::new_secure`
(Z. 91); `crates/discovery/src/sedp/reader.rs::SedpPublicationsReader::new_secure`
(Z. 154)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst`

**Status:** done

### §7.5.1.7 Sensible Topics bestimmt das AccessControl-Plugin

**Spec:** §7.5.1.7, S. 74 — "The determination of which Topic entities are
considered sensitive shall be specified by the AccessControl plugin."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`, `SharedSecurityGate::topic_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — die Auswahl folgt `is_discovery_protected` der Regel des Topics.

### §7.5.1.7 Name des Topics „DCPSPublicationsSecure“

**Spec:** §7.5.1.7, S. 74 — "The Topic name for the DCPSPublicationsSecure
Topic shall be 'DCPSPublicationsSecure'."

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicDataSecure`, `SubscriptionBuiltinTopicDataSecure`,
`data_tags`); `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_are_made_from_the_wire_data`,
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`

**Status:** `done` — die Topics `DCPSPublicationsSecure` und
`DCPSSubscriptionsSecure` bestehen mit ihren Typen und `data_tags` (0x1003)
(`054dd0f3a8b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.1.7 PublicationBuiltinTopicDataSecure: data_tags 0x1003

**Spec:** §7.5.1.7, Table 15, S. 74 — "data_tags DataTags PID_DATA_TAGS 0x1003".

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicDataSecure`, `SubscriptionBuiltinTopicDataSecure`,
`data_tags`); `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_are_made_from_the_wire_data`,
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`

**Status:** `done` — die Topics `DCPSPublicationsSecure` und
`DCPSSubscriptionsSecure` bestehen mit ihren Typen und `data_tags` (0x1003)
(`054dd0f3a8b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.1.7 QoS wie DCPSPublications

**Spec:** §7.5.1.7, S. 74 — "The QoS associated with the DCPSPublicationsSecure
Topic shall be the same as for the DCPSPublications Topic."

**Repo:** `crates/discovery/src/sedp/writer.rs::SedpPublicationsWriter::new_secure`
(Z. 91, derselbe Typ des Writers wie der plain Writer)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst`

**Status:** done

### §7.5.1.7 SEDPbuiltinPublicationsSecureWriter/-Reader und EntityIds

**Spec:** §7.5.1.7, S. 74 — "The builtin DataWriter for the
DCPSPublicationsSecure Topic shall be referred to as the
SEDPbuiltinPublicationsSecureWriter."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER`
(Z. 534), `SEDP_BUILTIN_PUBLICATIONS_SECURE_READER` (Z. 539)

**Tests:** `crates/rtps/src/wire_types.rs::tests::secure_publications_writer_layout`

**Status:** done

### §7.5.1.8 DCPSSubscriptions für nicht sensible Topics

**Spec:** §7.5.1.8, S. 75 — "Implementations of the DDS Security specification
shall use that same DCPSSubscriptions Topic to send the DataReader information
for Topic entities that are not considered sensitive."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — wie §7.5.1.7 für Subscriptions.

### §7.5.1.8 DCPSSubscriptionsSecure: Topic und Endpunkte

**Spec:** §7.5.1.8, S. 75 — "Implementations of the DDS Security specification
shall have an additional builtin Topic referred to as DCPSSubscriptionsSecure
and associated builtin DataReader and DataWriter entities".

**Repo:** `crates/discovery/src/sedp/writer.rs::SedpSubscriptionsWriter::new_secure`
(Z. 273); `crates/discovery/src/sedp/reader.rs::SedpSubscriptionsReader::new_secure`
(Z. 317)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst`

**Status:** done

### §7.5.1.8 Sensible Topics bestimmt das AccessControl-Plugin

**Spec:** §7.5.1.8, S. 75 — "The determination of which Topic entities are
considered sensitive shall be specified by the AccessControl plugin."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`, `SharedSecurityGate::topic_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — wie §7.5.1.7.

### §7.5.1.8 SubscriptionBuiltinTopicDataSecure: data_tags 0x1003

**Spec:** §7.5.1.8, Table 16, S. 75 — "data_tags DataTags PID_DATA_TAGS
0x1003".

**Repo:** `crates/dcps/src/builtin_topics.rs`
(`PublicationBuiltinTopicDataSecure`, `SubscriptionBuiltinTopicDataSecure`,
`data_tags`); `crates/dcps/src/builtin_subscriber.rs`

**Tests:**
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_carry_their_added_member`,
`crates/dcps/src/builtin_topics.rs::tests::the_secure_types_are_made_from_the_wire_data`,
`crates/dcps/src/builtin_subscriber.rs::tests::the_secure_topics_have_their_names_and_the_reliable_qos`

**Status:** `done` — die Topics `DCPSPublicationsSecure` und
`DCPSSubscriptionsSecure` bestehen mit ihren Typen und `data_tags` (0x1003)
(`054dd0f3a8b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.1.8 QoS wie DCPSSubscriptions

**Spec:** §7.5.1.8, S. 75 — "The QoS associated with the
DCPSSubscriptionsSecure Topic shall be the same as for the DCPSSubscriptions
Topic."

**Repo:** `crates/discovery/src/sedp/writer.rs::SedpSubscriptionsWriter::new_secure`
(Z. 273)

**Tests:** `crates/discovery/src/sedp/stack.rs::tests::handle_datagram_secure_sedp_heartbeat_is_gated_by_info_dst`

**Status:** done

### §7.5.1.8 SEDPbuiltinSubscriptionsSecureWriter/-Reader und EntityIds

**Spec:** §7.5.1.8, S. 75 — "The builtin DataWriter for the
DCPSSubscriptionsSecure Topic shall be referred to as the
SEDPbuiltinSubscriptionsSecureWriter."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId`

**Tests:** `crates/rtps/src/wire_types.rs::tests::table_9_entity_id_octets`

**Status:** `done` — die vier Oktette der Tabelle stehen im Test
(`a5b73d27f19`); `crates/dcps/src/wlp.rs` nennt den sicheren WLP-Writer nun
`0xff0200c2` wie die Tabelle. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.2 DCPSParticipantMessage für nicht sensible Topics


**Spec:** §7.5.2, S. 75 — "implementations of the DDS Security specification
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

**Status:** `done` — der Participant schlägt auf dem plain Writer für die Writer
seiner nicht sensiblen Topics und auf dem sicheren Writer für die der sensiblen
(`is_liveliness_protected` der Regel des Topics); mit Topics beider Arten
schlägt er auf beiden, jeder Writer mit eigenen Sequenznummern, eigener Historie
und eigenen Reader-Proxies. Ohne Writer entscheidet `liveliness_protection_kind`
der Domain. Wiederhergestellt: die Auswahl aus `62017f49303` ging im Merge
`13cb1667912` verloren, als der WLP-Endpunkt von wip/dcps-1.4 (ein
`ReliableWriter` mit Reader-Proxies) übernommen wurde; sie sitzt jetzt auf
diesem WLP-Endpunkt (`WlpEndpoint` hält den plain und den sicheren
`ReliableWriter`). Der plain WLP-DATA wird nicht mit dem Schlüssel des sicheren
Writers geschützt. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1018-073220.log`).

### §7.5.2 DCPSParticipantMessageSecure: Topic und Endpunkte

**Spec:** §7.5.2, S. 76 — "Implementations of the DDS Security specification
shall have an additional DCPSParticipantMessageSecure builtin Topic and
associated builtin DataReader and DataWriter entities".

**Repo:** `crates/dcps/src/wlp.rs::WlpEndpoint::set_secure` (Z. 156)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`,
`crates/dcps/src/wlp.rs::tests::the_secure_writer_is_answered_by_the_secure_reader`

**Status:** done

### §7.5.2 Datentyp ParticipantMessageData

**Spec:** §7.5.2, S. 76 — "The data type associated with the
DCPSParticipantMessageSecure Topic shall be the same as the
ParticipantMessageData structure."

**Repo:** `crates/dcps/src/wlp.rs` (derselbe Codec für beide Writer)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`

**Status:** done

### §7.5.2 QoS wie DCPSParticipantMessage

**Spec:** §7.5.2, S. 76 — "The QoS associated with the
DCPSParticipantMessageSecure Topic shall be the same as for the
DCPSParticipantMessage Topic as defined in sub clause 8.4.13 of the DDS
Interoperability Wire Protocol".

**Repo:** `crates/dcps/src/wlp.rs` (zuverlässiger Versand mit Historie der
letzten 32 Beats für beide Writer)

**Tests:** `crates/dcps/src/wlp.rs::tests::the_secure_writer_is_answered_by_the_secure_reader`,
`crates/dcps/src/wlp.rs::tests::a_heartbeat_announcing_a_pulse_not_received_is_answered_with_a_request_for_it`

**Status:** done

### §7.5.2 BuiltinParticipantMessageSecureWriter/-Reader und EntityIds

**Spec:** §7.5.2, S. 76 — "The builtin DataWriter for the
DCPSParticipantMessageSecure Topic shall be referred to as the
BuiltinParticipantMessageSecureWriter."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER`
(Z. 555), `BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER` (Z. 560)

**Tests:** `crates/dcps/src/wlp.rs::tests::wlp_secure_sender_uses_secure_entity_and_is_received`

**Status:** done

### §7.5.2 MANUAL_BY_TOPIC: HEARTBEAT der Liveliness wie HEARTBEAT des Writers geschützt

**Spec:** §7.5.2, S. 76 — "If a DataWriter LivelinessQos policy is
MANUAL_BY_TOPIC_LIVELINESS_QOS, implementations compliant with DDS-Security
shall use the same RTPS DataWriter for the liveliness heartbeats. The
liveliness heartbeats shall be protected using the same means as the regular
DataWriter heartbeats."

**Repo:** `crates/dcps/src/runtime.rs` (Schutz der HEARTBEAT-Submessage)

**Tests:**
`crates/dcps/src/runtime.rs::tests::the_liveliness_heartbeat_of_a_manual_writer_is_protected_like_its_heartbeats`

**Status:** `done` — der HEARTBEAT der Liveliness eines MANUAL_BY_TOPIC-Writers
wird wie ein HEARTBEAT des Writers geschützt (`054dd0f3a8b`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.2 Liveliness je Topic über beide Kanäle

**Spec:** §7.5.2, S. 76 — "It shall use the DCPSParticipantMessage Topic if
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

**Status:** `done` — der Participant schlägt auf dem plain Writer für die Writer
seiner nicht sensiblen Topics und auf dem sicheren Writer für die der sensiblen
(`is_liveliness_protected` der Regel des Topics); mit Topics beider Arten
schlägt er auf beiden, jeder Writer mit eigenen Sequenznummern, eigener Historie
und eigenen Reader-Proxies. Ohne Writer entscheidet `liveliness_protection_kind`
der Domain. Wiederhergestellt: die Auswahl aus `62017f49303` ging im Merge
`13cb1667912` verloren, als der WLP-Endpunkt von wip/dcps-1.4 (ein
`ReliableWriter` mit Reader-Proxies) übernommen wurde; sie sitzt jetzt auf
diesem WLP-Endpunkt (`WlpEndpoint` hält den plain und den sicheren
`ReliableWriter`). Der plain WLP-DATA wird nicht mit dem Schlüssel des sicheren
Writers geschützt. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1018-073220.log`).

### §7.5.2 AUTOMATIC/MANUAL_BY_PARTICIPANT: Auswahl nach is_liveliness_protected


**Spec:** §7.5.2, S. 76 — "It shall use the DCPSParticipantMessage Topic if
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

**Status:** `done` — der Participant schlägt auf dem plain Writer für die Writer
seiner nicht sensiblen Topics und auf dem sicheren Writer für die der sensiblen
(`is_liveliness_protected` der Regel des Topics); mit Topics beider Arten
schlägt er auf beiden, jeder Writer mit eigenen Sequenznummern, eigener Historie
und eigenen Reader-Proxies. Ohne Writer entscheidet `liveliness_protection_kind`
der Domain. Wiederhergestellt: die Auswahl aus `62017f49303` ging im Merge
`13cb1667912` verloren, als der WLP-Endpunkt von wip/dcps-1.4 (ein
`ReliableWriter` mit Reader-Proxies) übernommen wurde; sie sitzt jetzt auf
diesem WLP-Endpunkt (`WlpEndpoint` hält den plain und den sicheren
`ReliableWriter`). Der plain WLP-DATA wird nicht mit dem Schlüssel des sicheren
Writers geschützt. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1018-073220.log`).

### §7.5.3 New DCPSParticipantStatelessMessage builtin Topic

**Spec:** §7.5.3, S. 76 — "this specification introduces a new
DCPSParticipantStatelessMessage builtin Topic and corresponding builtin
DataReader and DataWriter entities".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Anforderungen folgen in
§7.5.3.2 bis §7.5.3.6.

### §7.5.3.1 Background: Sequence Number Attacks (non normative)

**Spec:** §7.5.3.1, S. 76–77 — Begründung zustandsloser Endpunkte.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — „non normative“.

### §7.5.3.2 Endpunkte für Stateless Messages vorhanden

**Spec:** §7.5.3.2, S. 77 — "These two endpoints shall be present in compliant
implementations of this specification."

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter`
(Z. 52), `StatelessMessageReader` (Z. 173)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::writer_has_expected_entity_id`,
`crates/discovery/src/security/stateless.rs::tests::reader_has_expected_entity_id`

**Status:** done

### §7.5.3.2 Writer: Best-Effort StatelessWriter

**Spec:** §7.5.3.2, S. 77 — "The BuiltinParticipantStatelessMessageWriter is an
RTPS Best-Effort StatelessWriter".

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter::write`
(Z. 122, ein DATA je Reader-Proxy, kein HEARTBEAT, keine Historie)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::write_increments_sequence_number`,
`crates/discovery/src/security/stateless.rs::tests::write_to_two_proxies_produces_two_datagrams`

**Status:** done

### §7.5.3.2 Reader: Best-Effort StatelessReader

**Spec:** §7.5.3.2, S. 77 — "The BuiltinParticipantStatelessMessageReader is an
RTPS Best-Effort StatelessReader".

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageReader`
(Z. 173, kein ACKNACK, keine Historie)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::reader_handle_data_decodes_generic_message`,
`crates/discovery/src/security/stateless.rs::tests::end_to_end_writer_to_reader_loopback`

**Status:** done

### §7.5.3.2 Datentyp ParticipantStatelessMessage

**Spec:** §7.5.3.2, S. 77 — "typedef ParticipantStatelessMessage
ParticipantGenericMessage;"

**Repo:** `crates/security/src/generic_message.rs::encode_generic_message`
(Z. 385), `decode_generic_message` (Z. 406)

**Tests:** `crates/discovery/src/security/codec.rs::tests::roundtrip_preserves_message`,
`crates/discovery/src/security/codec.rs::tests::encode_starts_with_cdr_le_encapsulation`

**Status:** done

### §7.5.3.2 EntityIds der Endpunkte für Stateless Messages

**Spec:** §7.5.3.2, S. 77 — "The RTPS EntityId_t associated with the
BuiltinParticipantStatelessMessageWriter and
BuiltinParticipantStatelessMessageReader shall be as specified in 7.5.8."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`
(Z. 567), `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER` (Z. 572)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::write_carries_writer_entity_id_on_wire`

**Status:** done

### §7.5.3.3 message_identity.source_guid = GUID des Writers von DCPSParticipantStatelessMessage

**Spec:** §7.5.3.3, S. 78 — "The source_guid field within the message_identity
shall be set to match the GUID_t of the BuiltinParticipantStatelessMessageWriter
that writes the message."

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter::next_message_identity`; `crates/discovery/src/security/stack.rs::build_message`

**Tests:** `crates/discovery/src/security/stateless.rs::tests::message_identities_are_the_writers_and_rise_by_one`, `crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`

**Status:** `done` — `source_guid` ist der GUID des Stateless-Writers (EntityId `0x000201c3`). Gemessen: offen (Security-Matrix).

### §7.5.3.3 sequence_number beginnt bei 1 und zählt je Nachricht des Writers

**Spec:** §7.5.3.3, S. 78 — "The sequence_number field within the
message_identity shall start with the value set to one and be incremented for
each different message sent by the BuiltinParticipantStatelessMessageWriter."

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter::next_message_identity`

**Tests:** `crates/discovery/src/security/stateless.rs::tests::message_identities_are_the_writers_and_rise_by_one`

**Status:** `done` — ein Zähler des Writers über alle Peers, beginnt bei 1; eine wiederholte Nachricht behält ihre Identität.

### §7.5.3.3 related_message_identity

**Spec:** §7.5.3.3, S. 78 — "It shall be set to either the tuple
{GUID_UNKNOWN, 0} if the message is not related to any other message, or else
set to match the message_identity of the related
ParticipantStatelessMessage."

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`
(Z. 418, `MessageIdentity::default()` für die Anfrage),
`on_stateless_message` (Z. 551, `msg.message_identity` für Antwort und
Abschluss)

**Tests:** `crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`,
`crates/security/src/generic_message.rs::tests::related_message_identity_links_reply_to_request`

**Status:** done — der Empfänger unterscheidet Anfrage und Antwort an
`related_message_identity`; der Handshake im Test kommt nur mit korrekt
gesetztem Feld zum gemeinsamen Geheimnis.

### §7.5.3.3 destination_participant_guid

**Spec:** §7.5.3.3, S. 78 — "The destination_participant_guid shall contain
either the value GUID_UNKNOWN [...] or else the GUID_t of the destination
DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.3 destination_endpoint_guid

**Spec:** §7.5.3.3, S. 78 — "It can contain either GUID_UNKNOWN or else the GUID
of a specific endpoint within destination DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.3 message_data je message_class_id

**Spec:** §7.5.3.3, S. 78 — "The contents message_data depend on the value of
the message_class_id".

**Repo:** `crates/discovery/src/security/stack.rs::build_message` (Z. 752)

**Tests:** `crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** done

### §7.5.3.4 Implizierter Inhaltsfilter auf destination_participant_guid

**Spec:** §7.5.3.4, S. 78 — "the BuiltinParticipantStatelessMessageReader has
an implied content filter with the logical expression:
'destination_participant_guid == GUID_UNKNOWN || destination_participant_guid
== BuiltinParticipantStatelessMessageReader.participant.guid'".

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_handshake_message_is_taken_only_as_the_spec_addresses_it`

**Status:** `done` — eine Nachricht, deren `destination_participant_guid` einen anderen
Participant nennt, wird nicht genommen. Der Writer schickt jede Nachricht an
alle Reader-Proxies; der Filter liegt beim Reader, wo die Spec ihn beschreibt. Gemessen: grün auf dem Linux-Testhost.

### §7.5.3.4 Zielendpunkt über destination_endpoint_guid

**Spec:** §7.5.3.4, S. 78 — "If the destination_endpoint_guid member is not set
to GUID_UNKNOWN, the message written targets the specific endpoint within the
destination DomainParticipant with a matching Endpoint Key."

**Repo:** `crates/discovery/src/security/stack.rs` (Stateless-Reader),
`crates/dcps/src/runtime.rs::install_crypto_token`, `local_endpoint_named`

**Tests:**
`crates/discovery/src/security/stack.rs::tests::a_stateless_message_for_another_endpoint_is_left_alone`,
`crates/dcps/src/runtime.rs::tests::a_token_for_an_endpoint_this_participant_lacks_is_not_installed`

**Status:** `done` — eine Nachricht, deren `destination_endpoint_guid` nicht
GUID_UNKNOWN ist, gilt dem benannten Endpunkt: der Stateless-Reader lässt eine
für einen anderen Endpunkt liegen, ein Token für einen Endpunkt, den dieser
Participant nicht hat, wird nicht installiert und als
`security.token.not_addressed` vermerkt (`e493c610d8a`). Neu auf der Leitung
(Empfang): eine Nachricht an einen anderen Endpunkt wird ignoriert. Tests grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.3.5 Präfix „dds.sec.“ für GenericMessageClassId reserviert

**Spec:** §7.5.3.5, S. 78 — "This specification, including future versions of
this specification reserves GenericMessageClassId values that start with the
prefix 'dds.sec.'".

**Repo:** `crates/security/src/generic_message.rs::class_id` (Z. 63)

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done — ZeroDDS verwendet nur die fünf Werte der Spec.

### §7.5.3.5 GMCLASSID_SECURITY_AUTH_REQUEST

**Spec:** §7.5.3.5, S. 79 — "#define GMCLASSID_SECURITY_AUTH_REQUEST
'dds.sec.auth_request'".

**Repo:** `crates/security/src/generic_message.rs::class_id::AUTH_REQUEST`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.3.5 GMCLASSID_SECURITY_AUTH_HANDSHAKE

**Spec:** §7.5.3.5, S. 79 — "#define GMCLASSID_SECURITY_AUTH_HANDSHAKE
'dds.sec.auth'".

**Repo:** `crates/security/src/generic_message.rs::class_id::AUTH`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.3.6 Format of data within ParticipantStatelessMessage

**Spec:** §7.5.3.6, S. 79 — "Each value for the GenericMessageClassId uses
different schema to store data within the generic attributes in the
message_data."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung zu §7.5.3.6.1 und .2.

### §7.5.3.6.1 AUTH_HANDSHAKE: message_data mit einem HandshakeMessageToken

**Spec:** §7.5.3.6.1, S. 79 — "the message_data attribute shall contain the
HandshakeMessageTokenSeq containing one element."

**Repo:** `crates/discovery/src/security/stack.rs::build_message` (Z. 752,
`message_data: vec![holder]`)

**Tests:** `crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** done

### §7.5.3.6.1 AUTH_HANDSHAKE: destination_participant_guid = Participant am Ziel

**Spec:** §7.5.3.6.1, S. 79 — "The destination_participant_guid shall be set to
the GUID_t of the destination DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.1 AUTH_HANDSHAKE: destination_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.3.6.1, S. 79 — "The destination_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.1 AUTH_HANDSHAKE: source_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.3.6.1, S. 79 — "The source_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.3/§7.5.3.6 — Feldwerte ohne Test

**Spec:** §7.5.3.3 und §7.5.3.6.1, S. 78–79 — die vier oben als `partial`
geführten Felder.

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.2 AUTH_REQUEST: message_data mit einem AuthRequestMessageToken

**Spec:** §7.5.3.6.2, S. 79 — "If GenericMessageClassId is
GMCLASSID_SECURITY_AUTH_REQUEST the message_data attribute shall contain an
AuthRequestMessageTokenSeq containing one element."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.2 AUTH_REQUEST: destination_participant_guid

**Spec:** §7.5.3.6.2, S. 79 — "The destination_participant_guid shall be set to
the GUID_t of the destination DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.2 AUTH_REQUEST: destination_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.3.6.2, S. 79 — "The destination_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.3.6.2 AUTH_REQUEST: source_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.3.6.2, S. 79 — "The source_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/discovery/src/security/stack.rs` (`begin_handshake_with`,
`on_stateless_message`)

**Tests:**
`crates/discovery/src/security/stack.rs::tests::handshake_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder `destination_participant_key`,
`destination_endpoint_key`, `source_endpoint_key`, `related_message_identity`
und `message_data` von AUTH_REQUEST und AUTH_HANDSHAKE haben die Werte der Spec;
der Test prüft sie an gesendeten Nachrichten (`368e8ed28f0`). ZeroDDS sendet die
Nachricht AUTH_REQUEST. Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.4.1 Background (Non-Normative)

**Spec:** §7.5.4.1, S. 79–80 — Begründung eines zuverlässigen, sicheren und
flüchtigen Kanals für den Schlüsselaustausch.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — die Überschrift sagt „Non-Normative“.

### §7.5.4.2 Endpunkte für VolatileMessageSecure vorhanden

**Spec:** §7.5.4.2, S. 80 — "These two endpoints shall be present in compliant
implementations of this specification."

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`
(Z. 56), `VolatileSecureMessageReader` (Z. 177)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::writer_has_expected_entity_id`,
`crates/discovery/src/security/volatile_secure.rs::tests::reader_has_expected_entity_id`

**Status:** done

### §7.5.4.2 Table 17 — TopicSecurityConfig der ParticipantVolatileMessageSecure

**Spec:** §7.5.4.2, Table 17, S. 80 — `is_read_protected false`,
`is_write_protected false`, `is_discovery_protected false`,
`is_liveliness_protected false`.

**Repo:** `crates/security-runtime/src/builtin_attributes.rs`
(`builtin_endpoint_security_attributes`, `builtin_endpoint_kinds`)

**Tests:**
`crates/security-runtime/src/builtin_attributes.rs::tests::table_17_has_the_values_of_the_spec`

**Status:** `done` — die `TopicSecurityConfig` der
`ParticipantVolatileMessageSecure` hat die Werte von Table 17 (`054dd0f3a8b`).
Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.4.2 Table 18 — EndpointSecurityConfig: nur is_submessage_protected

**Spec:** §7.5.4.2, Table 18, S. 80 — `is_submessage_protected true`, alle
anderen `false`.

**Repo:** `crates/dcps/src/runtime.rs::protect_volatile_outbound` (Z. 21887,
jede DATA-Submessage als SEC_PREFIX/SEC_BODY/SEC_POSTFIX mit dem Schlüssel Kx;
die Nutzlast selbst bleibt unverändert)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/dcps/src/runtime.rs::tests::handshake_completes_through_runtime_dispatch_e2e`

**Status:** done

### §7.5.4.2 Writer: Reliable StatefulWriter

**Spec:** §7.5.4.2, S. 80 — "The BuiltinParticipantVolatileMessageSecureWriter is
an RTPS Reliable StatefulWriter".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`
(Z. 56, `ReliableWriter`)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::write_with_heartbeat_piggybacks_heartbeat_submessage`

**Status:** done

### §7.5.4.2 Table 19 — Writer RELIABILITY RELIABLE

**Spec:** §7.5.4.2, Table 19, S. 80 — "RELIABILITY kind= RELIABLE".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`
(Z. 56)

**Tests:** `crates/discovery/src/security/stack.rs::tests::end_to_end_volatile_secure_handshake_via_reliable_loop`

**Status:** done

### §7.5.4.2 Table 19 — Writer HISTORY KEEP_ALL

**Spec:** §7.5.4.2, Table 19, S. 80 — "HISTORY kind= KEEP_ALL".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::identity_is_the_writers_and_a_late_reader_gets_no_old_message`

**Status:** `done` — HISTORY KEEP_ALL (Grenze 1024; ein Schreibvorgang über der Grenze schlägt fehl).

### §7.5.4.2 Table 19 — Writer DURABILITY VOLATILE

**Spec:** §7.5.4.2, Table 19, S. 80 — "DURABILITY kind= VOLATILE".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`::add_reader_proxy

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::identity_is_the_writers_and_a_late_reader_gets_no_old_message`

**Status:** `done` — ein später verbundener Reader überspringt frühere Nachrichten, bestätigte werden entfernt.

### §7.5.4.2 Reader: Reliable StatefulReader

**Spec:** §7.5.4.2, S. 81 — "The BuiltinParticipantVolatileMessageSecureReader is
an RTPS Reliable StatefulReader".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageReader`
(Z. 177, `ReliableReader`)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::reader_decodes_data_with_known_writer`,
`crates/discovery/src/security/volatile_secure.rs::tests::reader_drops_data_from_unknown_writer`

**Status:** done

### §7.5.4.2 Table 20 — Reader RELIABILITY RELIABLE

**Spec:** §7.5.4.2, Table 20, S. 81 — "RELIABILITY kind= RELIABLE".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageReader`
(Z. 177)

**Tests:** `crates/discovery/src/security/stack.rs::tests::end_to_end_volatile_secure_handshake_via_reliable_loop`

**Status:** done

### §7.5.4.2 Table 20 — Reader HISTORY KEEP_ALL

**Spec:** §7.5.4.2, Table 20, S. 81 — "HISTORY kind= KEEP_ALL".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VOLATILE_SECURE_READER_CAPACITY`
(Z. 52, 64 Proben je Writer-Proxy bis zur Auslieferung)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::reader_decodes_data_with_known_writer`

**Status:** done — der Reader gibt jede Probe in Reihenfolge an den Aufrufer
weiter und hält keine Historie; die Grenze von 64 gilt für noch nicht
auslieferbare Proben.

### §7.5.4.2 Table 20 — Reader DURABILITY VOLATILE

**Spec:** §7.5.4.2, Table 20, S. 81 — "DURABILITY kind= VOLATILE".

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageReader`
(Z. 177)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::reader_decodes_data_with_known_writer`

**Status:** done — der Reader fordert keine Proben vor dem ersten HEARTBEAT an.

### §7.5.4.2 Datentyp ParticipantVolatileMessageSecure

**Spec:** §7.5.4.2, S. 81 — "typedef ParticipantVolatileMessageSecure
ParticipantGenericMessage;"

**Repo:** `crates/security/src/generic_message.rs::encode_generic_message`
(Z. 385)

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::reader_decodes_data_with_known_writer`

**Status:** done

### §7.5.4.2 EntityIds der Endpunkte für VolatileMessageSecure

**Spec:** §7.5.4.2, S. 81 — "The RTPS EntityId_t [...] shall be as specified in
7.5.8."

**Repo:** `crates/rtps/src/wire_types.rs::EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER`
(Z. 579), `BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER` (Z. 587)

**Tests:** `crates/rtps/src/wire_types.rs::tests::volatile_message_secure_entity_ids_match_spec_and_cyclone`

**Status:** done

### §7.5.4.3 destination_participant_guid

**Spec:** §7.5.4.3, S. 81 — "The destination_participant_guid shall contain
either the value GUID_UNKNOWN [...] or else the GUID_t of the destination
DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.3 message_identity.source_guid = GUID des Writers von DCPSParticipantVolatileMessageSecure

**Spec:** §7.5.4.3, S. 81 — "The source_guid field within the message_identity
shall be set to match the GUID_t of the
BuiltinParticipantVolatileMessageSecureWriter that writes the message."

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`::with_identity

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::identity_is_the_writers_and_a_late_reader_gets_no_old_message`

**Status:** `done` — der Writer setzt `source_guid` auf seinen GUID (EntityId `0xff0202c3`), unabhängig vom Aufrufer.

### §7.5.4.3 sequence_number beginnt bei 1 und zählt

**Spec:** §7.5.4.3, S. 81 — "The sequence_number field within the
message_identity shall start with the value set to one and be incremented for
each different message sent by the
BuiltinParticipantVolatileMessageSecureWriter."

**Repo:** `crates/discovery/src/security/volatile_secure.rs::VolatileSecureMessageWriter`::with_identity

**Tests:** `crates/discovery/src/security/volatile_secure.rs::tests::identity_is_the_writers_and_a_late_reader_gets_no_old_message`

**Status:** `done` — die Nummer beginnt bei 1 und steigt je Nachricht.

### §7.5.4.3 related_message_identity

**Spec:** §7.5.4.3, S. 81 — "It shall be set to either the tuple
{GUID_UNKNOWN, 0} if the message is not related to any other message".

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.3 message_data je message_class_id

**Spec:** §7.5.4.3, S. 81 — "The contents message_data depend on the value of
the message_class_id".

**Repo:** `crates/dcps/src/runtime.rs::build_crypto_token_message` (Z. 21338)

**Tests:** `crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done

### §7.5.4.4 Implizierter Inhaltsfilter auf destination_participant_guid

**Spec:** §7.5.4.4, S. 81–82 — "the BuiltinParticipantVolatileMessageSecureReader
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

**Status:** `done` — der Reader nimmt nur Nachrichten an seinen Participant und an dessen
Endpunkte und zählt die übrigen. Der Writer gibt jedem anderen Reader für die
Nummer einer gerichteten Nachricht einen GAP, beim ersten Senden und bei jeder
Reparatur; vorher bekam jeder Reader jede Token-Nachricht. Gemessen: grün auf
dem Linux-Testhost; die Security-Matrix gegen Cyclone, Fast DDS und OpenDDS ist
unverändert.

### §7.5.4.4 Zielendpunkt über destination_endpoint_guid

**Spec:** §7.5.4.4, S. 82 — "If the destination_endpoint_guid member is not set
to GUID_UNKNOWN the message written targets a specific endpoint within the
destination DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs` (Stateless-Reader),
`crates/dcps/src/runtime.rs::install_crypto_token`, `local_endpoint_named`

**Tests:**
`crates/discovery/src/security/stack.rs::tests::a_stateless_message_for_another_endpoint_is_left_alone`,
`crates/dcps/src/runtime.rs::tests::a_token_for_an_endpoint_this_participant_lacks_is_not_installed`

**Status:** `done` — eine Nachricht, deren `destination_endpoint_guid` nicht
GUID_UNKNOWN ist, gilt dem benannten Endpunkt: der Stateless-Reader lässt eine
für einen anderen Endpunkt liegen, ein Token für einen Endpunkt, den dieser
Participant nicht hat, wird nicht installiert und als
`security.token.not_addressed` vermerkt (`e493c610d8a`). Neu auf der Leitung
(Empfang): eine Nachricht an einen anderen Endpunkt wird ignoriert. Tests grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.5 Präfix „dds.sec.“ reserviert

**Spec:** §7.5.4.5, S. 82 — "This specification [...] reserves
GenericMessageClassId values that start with the prefix 'dds.sec.'".

**Repo:** `crates/security/src/generic_message.rs::class_id` (Z. 63)

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.4.5 GMCLASSID_SECURITY_PARTICIPANT_CRYPTO_TOKENS

**Spec:** §7.5.4.5, S. 82 — "'dds.sec.participant_crypto_tokens'".

**Repo:** `crates/security/src/generic_message.rs::class_id::PARTICIPANT_CRYPTO_TOKENS`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.4.5 GMCLASSID_SECURITY_DATAWRITER_CRYPTO_TOKENS

**Spec:** §7.5.4.5, S. 82 — "'dds.sec.datawriter_crypto_tokens'".

**Repo:** `crates/security/src/generic_message.rs::class_id::DATAWRITER_CRYPTO_TOKENS`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.4.5 GMCLASSID_SECURITY_DATAREADER_CRYPTO_TOKENS

**Spec:** §7.5.4.5, S. 82 — "'dds.sec.datareader_crypto_tokens'".

**Repo:** `crates/security/src/generic_message.rs::class_id::DATAREADER_CRYPTO_TOKENS`

**Tests:** `crates/security/src/generic_message.rs::tests::class_id_constants_match_spec`

**Status:** done

### §7.5.4.6 Format of data within ParticipantVolatileMessageSecure

**Spec:** §7.5.4.6, S. 82 — "Each value for the GenericMessageClassId uses
different schema".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.5.4.6.1 PARTICIPANT_CRYPTO_TOKENS: message_data

**Spec:** §7.5.4.6.1, S. 82 — "the message_data attribute shall contain the
ParticipantCryptoTokenSeq."

**Repo:** `crates/dcps/src/runtime.rs::build_crypto_token_message` (Z. 21338)

**Tests:** `crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done

### §7.5.4.6.1 PARTICIPANT_CRYPTO_TOKENS: destination_participant_guid

**Spec:** §7.5.4.6.1, S. 82 — "The destination_participant_guid shall be set to
the GUID_t of the destination DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.1 PARTICIPANT_CRYPTO_TOKENS: destination_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.4.6.1, S. 82 — "The destination_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.1 PARTICIPANT_CRYPTO_TOKENS: source_endpoint_guid = GUID_UNKNOWN

**Spec:** §7.5.4.6.1, S. 82 — "The source_endpoint_guid shall be set to
GUID_UNKNOWN."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.2 DATAWRITER_CRYPTO_TOKENS: message_data

**Spec:** §7.5.4.6.2, S. 83 — "the message_data shall contain the
DatawriterCryptoTokenSeq."

**Repo:** `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`
(Z. 21431)

**Tests:** `crates/dcps/src/runtime.rs::tests::pending_endpoint_tokens_keeps_late_user_token_after_builtins_sent`

**Status:** done

### §7.5.4.6.2 DATAWRITER_CRYPTO_TOKENS: destination_endpoint_guid = DataReader

**Spec:** §7.5.4.6.2, S. 83 — "The destination_endpoint_guid shall be set to the
GUID_t of the DataReader that should receive the CryptoToken values in the
message."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.2 DATAWRITER_CRYPTO_TOKENS: source_endpoint_guid = DataWriter

**Spec:** §7.5.4.6.2, S. 83 — "The source_endpoint_guid shall be set to the
GUID_t of the DataWriter that will be using the CryptoToken values to encode
the data it sends to the DataReader."

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.3 DATAREADER_CRYPTO_TOKENS: message_data

**Spec:** §7.5.4.6.3, S. 83 — "the message_data attribute shall contain the
DatareaderCryptoTokenSeq."

**Repo:** `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`
(Z. 21431)

**Tests:** `crates/dcps/src/runtime.rs::tests::pending_endpoint_tokens_keeps_late_user_token_after_builtins_sent`

**Status:** done

### §7.5.4.6.3 DATAREADER_CRYPTO_TOKENS: destination_endpoint_guid = DataWriter

**Spec:** §7.5.4.6.3, S. 83 — "The destination_endpoint_guid shall be set to the
GUID_t of the DataWriter that should receive the CryptoToken values".

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4.6.3 DATAREADER_CRYPTO_TOKENS: source_endpoint_guid = DataReader

**Spec:** §7.5.4.6.3, S. 83 — "The source_endpoint_guid shall be set to the
GUID_t of the DataReader that will be using the CryptoToken values".

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.4 — Feldwerte ohne Test

**Spec:** §7.5.4.3, §7.5.4.4 und §7.5.4.6, S. 81–83 — die oben als `partial`
geführten Felder.

**Repo:** `crates/dcps/src/runtime.rs` (`volatile_write_endpoint_token`,
`prepare_endpoint_crypto_tokens`);
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/dcps/src/runtime.rs::tests::volatile_messages_carry_the_fields_of_the_spec`

**Status:** `done` — die Felder der Volatile-Nachrichten
(`destination_participant_key`, `destination_endpoint_key`,
`source_endpoint_key`, `related_message_identity`) haben die Werte der Spec je
Klasse der Token, und die Token werden beim Empfang nach Quelle und Ziel
zugeordnet; der Test prüft Senden und Zuordnung (`368e8ed28f0`). Test grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.5.1 Background (TypeLookup)

**Spec:** §7.5.5.1, S. 83–84 — "Compliance with DDS-XTYPES requires any
DomainParticipant that implements the TypeLookup service, to respond to
requests for any TypeIdentifier that the DomainParticipant announces".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Wiedergabe von DDS-XTYPES; dessen Coverage
steht in `dds-xtypes-1.3.md`.

### §7.5.5.2 Vier sichere Endpunkte für TypeLookup

**Spec:** §7.5.5.2, S. 84 — "These consist of four new endpoints:
TypeLookupServiceRequestSecureWriter, TypeLookupServiceRequestSecureReader,
TypeLookupServiceReplySecureWriter, and TypeLookupServiceReplySecureReader."

**Repo:**
`crates/dcps/src/type_lookup_transport.rs::TypeLookupTransport::new_secure`;
`crates/rtps/src/wire_types.rs`

**Tests:**
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_its_own_endpoints`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_the_qos_of_the_plain_one`

**Status:** `done` — vier sichere Endpunkte mit den EntityIds von Table 9 und
denselben Datentypen und derselben QoS wie die nicht sicheren (`a542c9f545d`,
`14e1984914d`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.5.2 EntityIds der sicheren Endpunkte für TypeLookup

**Spec:** §7.5.5.2, S. 84 — "The EntityIds for the Secure Type Lookup builtin
endpoints are defined in Table 9."

**Repo:**
`crates/dcps/src/type_lookup_transport.rs::TypeLookupTransport::new_secure`;
`crates/rtps/src/wire_types.rs`

**Tests:**
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_its_own_endpoints`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_the_qos_of_the_plain_one`

**Status:** `done` — vier sichere Endpunkte mit den EntityIds von Table 9 und
denselben Datentypen und derselben QoS wie die nicht sicheren (`a542c9f545d`,
`14e1984914d`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.5.2 Datentypen und QoS wie die nicht sicheren Endpunkte

**Spec:** §7.5.5.2, S. 84 — "The data types and Qos policies of the Secure Type
Lookup builtin shall be the same defined for the corresponding (non-secure)
endpoint in DDS-XTYPES."

**Repo:**
`crates/dcps/src/type_lookup_transport.rs::TypeLookupTransport::new_secure`;
`crates/rtps/src/wire_types.rs`

**Tests:**
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_its_own_endpoints`,
`crates/dcps/src/type_lookup_transport.rs::tests::the_secure_service_has_the_qos_of_the_plain_one`

**Status:** `done` — vier sichere Endpunkte mit den EntityIds von Table 9 und
denselben Datentypen und derselben QoS wie die nicht sicheren (`a542c9f545d`,
`14e1984914d`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.6 Definition of the Types a DDS Endpoint depends on

**Spec:** §7.5.6, S. 84–85 — "the collection consisting of the top-level type
as well as the transitive closure that includes the types that the top-level
type depends [...] shall be referred to as the types the endpoint depends on."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung für §7.5.5.1.

### §7.5.7 Definition of the "RTPS Bootstrapping Messages"

**Spec:** §7.5.7, S. 85 — eine Nachricht ist „RTPS Bootstrapping Message“,
wenn sie Submessages für DCPSParticipants, DCPSParticipantStatelessMessage
oder DCPSParticipantVolatileMessageSecure enthält oder nicht für einen
DomainParticipant bestimmt ist.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung; die Anforderung steht
in §7.5.12.

### §7.5.8 Definition of the "Builtin Secure Endpoints"

**Spec:** §7.5.8, S. 85 — Liste der vierzehn Builtin Secure Endpoints.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung; die Endpunkte sind
einzeln in §7.4.7.1 und §7.5.1 bis §7.5.5 geführt.

### §7.5.9 Definition of the "Builtin Secure Discovery Endpoints"

**Spec:** §7.5.9, S. 85 — die sechs sicheren SPDP- und Endpunkte von SEDP.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung.

### §7.5.10 Definition of the "Builtin Secure Liveliness Endpoints"

**Spec:** §7.5.10, S. 86 — BuiltinParticipantMessageSecureWriter und -Reader.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung.

### §7.5.11 Definition of the "Builtin Secure TypeLookup Endpoints"

**Spec:** §7.5.11, S. 86 — die vier sicheren Endpunkte für TypeLookup.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsbestimmung.

### §7.5.12 Bootstrapping-Nachrichten ohne andere Topics

**Spec:** §7.5.12, S. 86 — "RTPS Bootstrapping messages (see 7.5.7) shall not
contain submessages for any other (non-bootstrapping) builtin topic or
application-defined topic."

**Repo:** `crates/discovery/src/spdp.rs`,
`crates/discovery/src/security/stateless.rs`,
`crates/discovery/src/security/volatile_secure.rs`

**Tests:**
`crates/discovery/src/spdp.rs::tests::an_spdp_datagram_holds_the_submessages_of_its_writer_only`,
`crates/discovery/src/security/stateless.rs::tests::a_stateless_datagram_holds_the_submessages_of_its_writer_only`,
`crates/discovery/src/security/volatile_secure.rs::tests::a_volatile_datagram_holds_the_submessages_of_its_writer_only`

**Status:** `done` — ein Datagramm der Bootstrapping-Nachrichten enthält nur
Submessages seines Writers (`054dd0f3a8b`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`).

### §7.5.13 get_datawriter_sec_attributes/get_datareader_sec_attributes für Builtin Secure Endpoints

**Spec:** §7.5.13, S. 86 — "the middleware shall call the operations
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

**Status:** `done` — die Attribute der sicheren eingebauten Endpunkte folgen
Table 21 und werden an einer Stelle bestimmt; die Laufzeit schützt danach, auch
die Endpunkte für TypeLookup (`054dd0f3a8b`, `ee7183c45df`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.13 Table 21 — DCPSParticipantSecure, DCPSPublicationsSecure, DCPSSubscriptionsSecure

**Spec:** §7.5.13, Table 21, S. 86–87 — `is_read_protected false`,
`is_write_protected false`, `is_submessage_protected` = „Set to match
ParticipantSecurityConfig is_discovery_protected“, `is_payload_protected
false`, `is_key_protected false`.

**Repo:** `crates/dcps/src/runtime.rs::protect_secure_spdp` (Z. 17028,
Schutz nur bei `discovery_protection_kind != NONE`); sichere Submessages von SEDP
ebenso

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_discovery_protection`

**Status:** done

### §7.5.13 Table 21 — DCPSParticipantMessageSecure

**Spec:** §7.5.13, Table 21, S. 86–87 — `is_submessage_protected` = „Set to
match ParticipantSecurityConfig is_liveliness_protected“, sonst `false`.

**Repo:** `crates/dcps/src/runtime.rs` (Z. 18036, sicherer Writer von WLP bei
`liveliness_protection_kind != NONE`)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_liveliness_protection`

**Status:** done

### §7.5.13 Table 21 — DCPSParticipantStatelessMessage

**Spec:** §7.5.13, Table 21, S. 86–87 — alle Attribute `false`.

**Repo:** `crates/discovery/src/security/stateless.rs::StatelessMessageWriter::write`
(Z. 122, plain DATA)

**Tests:** `crates/discovery/src/security/stateless.rs::tests::write_carries_writer_entity_id_on_wire`

**Status:** done

### §7.5.13 Table 21 — DCPSParticipantVolatileMessageSecure, TypeLookupServiceRequestSecure, TypeLookupServiceReplySecure

**Spec:** §7.5.13, Table 21, S. 86–87 — `is_submessage_protected true`, sonst
`false`.

**Repo:** `crates/security-runtime/src/builtin_attributes.rs`
(`builtin_endpoint_security_attributes`, `builtin_endpoint_kinds`);
`crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`

**Tests:**
`crates/security-runtime/src/builtin_attributes.rs::tests::table_21_has_the_values_of_the_spec`,
`crates/security-runtime/src/builtin_attributes.rs::tests::the_kinds_follow_table_21`,
`crates/dcps/src/runtime.rs::tests::the_secure_type_lookup_service_is_protected_per_submessage`

**Status:** `done` — die Attribute der sicheren eingebauten Endpunkte folgen
Table 21 und werden an einer Stelle bestimmt; die Laufzeit schützt danach, auch
die Endpunkte für TypeLookup (`054dd0f3a8b`, `ee7183c45df`). Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195038.log`,
`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

### §7.5.13 Builtin Secure Endpoints unter is_access_protected

**Spec:** §7.5.13, S. 87 — "they are still protected by the access control
mechanism imposed by the DomainParticipant. That is, if
ParticipantSecurityConfig member is_access_protected is true, then access to
the secure builtin topics is protected."

**Repo:** `crates/dcps/src/runtime.rs` (Zugang eines authentisierten Peers nach
der Join-Zugriffskontrolle)

**Tests:**
`crates/dcps/src/runtime.rs::tests::a_peer_the_join_access_control_does_not_admit_gets_no_access`

**Status:** `done` — ein authentisierter Peer, den die Zugriffskontrolle der
Domain nicht zulässt, erhält keinen Zugang zu den sicheren eingebauten Topics
(`054dd0f3a8b`). Test grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1017-195553.log`).

---

## §8 Common Cryptographic Algorithms

### §8 Drei Darstellungen eines Algorithmus

**Spec:** §8, S. 89 — "The specification uses three identifier
representations: A CryptoAlgorithmName (string) [...] A CryptoAlgorithmId
(binary) [...] A CryptoAlgorithmBit (bit)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Typen stehen in §7.3.6 bis
§7.3.9, die Werte in den Tabellen 22, 25 und 26.

### §8 Mehrere Namen eines Algorithmus sind gleichwertig

**Spec:** §8, S. 89 — "In the case where multiple string identifiers are
provided for the same algorithm, they shall all be treated as equivalent."

**Repo:** `crates/security-pki/src/plugin.rs::DsignScheme::from_name`, `kx_suite_for_algo`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_algorithm_names_of_1_2_and_1_1_are_taken`, `crates/security-pki/src/plugin.rs::tests::the_algorithm_names_follow_the_version_of_the_peer_and_the_setting`

**Status:** `done` — Die Namen von 1.2 und die als „deprecated in v 1.2“ markierten von 1.1 werden beim Empfang gleich behandelt (Table 25, Table 26). Gesendet wird als Vorgabe der Name der Tabelle; `auto` (Namen 1.2 nur an einen Peer, dessen IdentityToken 1.2 nennt) und `1.1` sind benannte Einstellungen (`org.zerodds.sec.algorithm_names`, `ZERODDS_SEC_ALGORITHM_NAMES`). Die Spec macht die gesendete Schreibweise nicht von der Version des Peers abhängig, deshalb ist `auto` nicht die Vorgabe. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.1 Symmetric Cipher AEAD and MAC Algorithms

**Spec:** §8.1, S. 89 — "SPIs may use symmetric cipher algorithms for two
purposes: Authenticated Encryption with Additional Data (AEAD) [...] Message
Authentication Codes."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Algorithmen folgen in
Table 22.

### §8.1 Table 22 — AES128+GMAC, Id 0x01, Bit 0

**Spec:** §8.1, Table 22, S. 90 — "AES128+ GCM 0x01 0x0001 << 0 Message
authentication codes (MACs) computed using Galois MAC (AES-GMAC). [...]
specialized to 128-bit AES keys with 96-bit Initialization Vector."

**Repo:** `crates/security-crypto/src/suite.rs::Suite::Aes128Gmac`

**Tests:** `crates/security-crypto/src/suite.rs::tests::aes128_gmac_uses_spec_id_1`, `crates/security-crypto/src/plugin.rs::tests::gmac_only_suite_roundtrip_without_encryption`, `crates/security-crypto/src/plugin.rs::tests::an_aes128_gmac_key_with_a_short_salt_crosses_instances`

**Status:** `done` — Id 0x01 ist AES128-GMAC mit 16-Oktett-Schlüssel (Tag über die Daten als AAD, AES-GCM mit leerem Klartext). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.1 Table 22 — AES128+GCM, Id 0x02, Bit 0

**Spec:** §8.1, Table 22, S. 90 — "AES128+GCM 0x02 0x0001 << 0 Authenticated
Encryption with Additional Data (AEAD) using Advanced Encryption Standard (AES)
in Galois Counter Mode".

**Repo:** `crates/security-crypto/src/suite.rs::Suite::Aes128Gcm` (Z. 61,
16-Oktett-Schlüssel), `transform_kind::AES128_GCM` (Z. 38)

**Tests:** `crates/security-crypto/src/suite.rs::tests::aes128_gcm_uses_spec_id_2`,
`crates/security-crypto/src/plugin.rs::tests::encrypt_decrypt_roundtrip`

**Status:** done

### §8.1 Table 22 — AES256+GMAC, Id 0x03, Bit 1

**Spec:** §8.1, Table 22, S. 90 — "AES256+ GCM 0x03 0x0001 << 1 Message
authentication codes (MACs) computed using Galois MAC (AES-GMAC)".

**Repo:** `crates/security-crypto/src/suite.rs::Suite::Aes256Gmac` (Z. 61);
`crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone` (Z. 278,
Zweig ohne Verschlüsselung)

**Tests:** `crates/security-crypto/src/suite.rs::tests::from_id_roundtrip_for_all_supported`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** done

### §8.1 Table 22 — AES256+GCM, Id 0x04, Bit 1

**Spec:** §8.1, Table 22, S. 90 — "AES256+GCM 0x04 0x0001 << 1 Authenticated
Encryption with Additional Data (AEAD) using Advanced Encryption Standard (AES)
in Galois Counter Mode".

**Repo:** `crates/security-crypto/src/suite.rs::Suite::Aes256Gcm` (Z. 61,
32-Oktett-Schlüssel)

**Tests:** `crates/security-crypto/src/suite.rs::tests::aes256_gcm_uses_spec_id_4`,
`crates/security-crypto/src/plugin.rs::tests::aes256_encrypt_decrypt_roundtrip`

**Status:** done

### §8.1 Gleiches Bit und gleicher Name für AEAD- und MAC-Variante

**Spec:** §8.1, S. 90 — "the same value of the CryptoAlgorithmName and
CryptoAlgorithmBit is used for the message-authentication-only (MAC-only)
variants of the corresponding AEAD algorithms."

**Repo:** `crates/rtps/src/security_algo_info.rs::symmetric_bit` (Z. 53, je
ein Bit für AES128 und AES256)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::spec_bit_constants_match`

**Status:** done

### §8.1 Konstanten CNAME_AES*

**Spec:** §8.1, S. 91 — "const string CNAME_AES128_GMAC = 'AES128+GCM'; const
string CNAME_AES128_GCM = 'AES128+GCM'; const string CNAME_AES256_GMAC =
'AES256+GCM'; const string CNAME_AES256_GCM = 'AES256+GCM';"

**Repo:** `crates/security/src/algorithm.rs::CNAME_AES128_GMAC`, `CNAME_AES128_GCM`, `CNAME_AES256_GMAC`, `CNAME_AES256_GCM`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — die Zeichenketten der Spec (`AES128+GCM`, `AES256+GCM`; die GMAC-Konstanten haben denselben Wert wie die GCM-Konstanten).

### §8.1 Konstanten CID_INVALID und CID_AES*

**Spec:** §8.1, S. 91 — "const CryptoAlgorithmId CID_INVALID = 0x00; [...]
CID_AES128_GMAC = 0x01; CID_AES128_GCM = 0x02; CID_AES256_GMAC = 0x03;
CID_AES256_GCM = 0x04;"

**Repo:** `crates/security-crypto/src/suite.rs::transform_kind` (Z. 38,
`NONE`, `AES128_GMAC`, `AES128_GCM`, `AES256_GMAC`, `AES256_GCM`)

**Tests:** `crates/security-crypto/src/suite.rs::tests::spec_constants_match_table_79`

**Status:** done — die Konstanten heißen anders (`transform_kind::*`;
Benennung).

### §8.1 Konstanten CBIT_AES*

**Spec:** §8.1, S. 91 — "const CryptoAlgorithmBit CBIT_AES128_GMAC = 1 << 0;
CBIT_AES128_GCM = 1 << 0; CBIT_AES256_GMAC = 1 << 1; CBIT_AES256_GCM = 1 <<
1;"

**Repo:** `crates/rtps/src/security_algo_info.rs::symmetric_bit` (Z. 53)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::spec_bit_constants_match`

**Status:** done — `AES128 = 1 << 0`, `AES256 = 1 << 1`.

### §8.1.1 Table 23 — AES-GCM-Eingaben K, P, AAD, IV

**Spec:** §8.1.1, Table 23, S. 91 — "K The 128-bit key [...] or the 256-bit
key [...] P The plaintext [...] AAD Additional Authenticated Data [...] IV
Initialization Vector. This is a 96-bit NONCE that shall not be repeated for
the same key."

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(Z. 278, Nonce = `session_id` (4) und laufender Zähler (8))

**Tests:** `crates/security-crypto/src/plugin.rs::tests::two_encrypts_produce_different_ciphertexts`,
`crates/security-crypto/src/plugin.rs::tests::encrypts_remaining_decrements_per_call`

**Status:** done — der Zähler wird je Versiegelung erhöht; die eigentliche
Chiffre stammt aus `ring` oder `aws-lc-rs`.

### §8.1.1 Table 24 — AES-GCM-Ausgaben C und T

**Spec:** §8.1.1, Table 24, S. 91–92 — "C Ciphertext [...] T Authentication
Tag. This is a Message Authentication Code (MAC) that provides authentication
for the Ciphertext (C) and the Additional Authenticated Data (AAD)."

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(Z. 278, Chiffrat und 16-Oktett-Tag getrennt)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/security-crypto/src/plugin.rs::tests::decrypt_rejects_tampered_ciphertext`

**Status:** done

### §8.1.1 Zählerblöcke aus dem IV mit inc32

**Spec:** §8.1.1, S. 92 — "The initial value of the 128-bit counter block is a
128-bit string containing the IV as the leading 96 bits and zeros the
remaining right-most 32 bits. Incremental values [...] are obtained using the
'inc32' function".

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(Z. 278, `Nonce::assume_unique_for_key` mit 96 Bit)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::encrypt_decrypt_roundtrip`

**Status:** done — die Zählerblöcke bildet die AES-GCM-Implementierung von
`ring`/`aws-lc-rs`; ein Testvektor nach NIST SP 800-38D steht nicht im Repo.

### §8.1.1 AES-GMAC als AES-GCM mit leerem Klartext

**Spec:** §8.1.1, S. 92 — "T = AES-GMAC(K, AAD, IV) = AES-GCM(K, '', AAD, IV)".

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial::seal_cyclone`
(Z. 278, GMAC-Zweig: Nutzdaten als AAD, leerer Klartext)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** done — nur für AES256-GMAC; AES128-GMAC siehe Table 22.

### §8.2 Digital Signature Algorithms

**Spec:** §8.2, S. 92 — "SPIs may use digital signature algorithms for
signing/validating identity-type certificates and attestation documents."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Algorithmen folgen in
Table 25.

### §8.2 Table 25 — RSASSA-PSS-MGF1SHA256+2048+SHA256, Id 0x10, Bit 0

**Spec:** §8.2, Table 25, S. 92 — "The digital signature shall be computed
using the RSASSA-PSS algorithm specified in PKCS #1 [...] using SHA256 as hash
function, and MGF1 with SHA256 [...] The length of the salt is not specified.
[...] The validation of the signature shall detect the salt length from the
signature."

**Repo:** `crates/security-crypto/src/rsa_pss.rs::verify_rsassa_pss_sha256`; `crates/security-pki/src/plugin.rs::verify_signature_with_cert`; `crates/security-permissions/src/cms.rs`

**Tests:** `crates/security-crypto/src/rsa_pss.rs::tests::the_salt_length_is_read_from_the_signature`, `crates/security-pki/src/plugin.rs::tests::an_rsa_pss_handshake_signature_of_any_salt_length_verifies`, `crates/security-permissions/tests/cms_verify.rs::rsa_pss_signers_of_any_salt_length_are_accepted`

**Status:** `done` — Das Prüfen liest die Salzlänge aus der Signatur (Vektoren von OpenSSL 4.0.3 mit 0, 8, 20, 32, 64 und 222 Oktetten); Handshake und CMS-Prüfung nutzen es. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.2 RSASSA-PSS: Salzlänge aus der Signatur bestimmen

**Spec:** §8.2, Table 25, S. 92 — "The validation of the signature shall detect
the salt length from the signature."

**Repo:** `crates/security-crypto/src/rsa_pss.rs`

**Tests:** `crates/security-crypto/src/rsa_pss.rs::tests::the_salt_length_is_read_from_the_signature`, `crates/security-crypto/src/rsa_pss.rs::tests::a_3072_bit_key_verifies_signatures_of_any_salt_length`, `crates/security-crypto/src/rsa_pss.rs::tests::what_does_not_verify_is_refused`

**Status:** `done` — `webpki` prüft nur 32 Oktette Salz; die eigene Prüfung nach RFC 8017 §9.1.2 liest die Länge aus `DB`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.2 Table 25 — RSASSA-PKCS1-V1_5+2048+SHA256, Id 0x11, Bit 1

**Spec:** §8.2, Table 25, S. 92–93 — "The digital signature shall be computed
using the RSASSA-PKCS1-v1_5 algorithm specified in PKCS #1, using SHA256 as
hash function."

**Repo:** `crates/security-pki/src/plugin.rs::DsignScheme::RsaPkcs1`, `sign_with`, `verify_signature_with_cert`

**Tests:** `crates/security-pki/src/plugin.rs::tests::an_rsa_identity_signs_pkcs1_when_the_peer_verifies_only_that`, `crates/security-permissions/tests/cms_verify.rs::rsa_pkcs1_signer_is_accepted`

**Status:** `done` — Zertifikatskette, Dokumente und Handshake; ein RSA-Schlüssel signiert PKCS#1 v1.5, wenn der Peer nur das prüft.

### §8.2 RSASSA-PKCS1-V1_5 und ECDSA-P384 für Handshake-Signaturen

**Spec:** §8.2, Table 25, S. 92–93 — wie oben und Zeile ECDSA+P384+SHA384.

**Repo:** `crates/security-pki/src/plugin.rs::DsignScheme`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_dsign_algo_must_fit_the_certificate`, `::a_p384_identity_authenticates_with_ecdsa_p384`, `::an_rsa_identity_signs_pkcs1_when_the_peer_verifies_only_that`

**Status:** `done` — `c.dsign_algo` benennt ein Verfahren, das zum Zertifikat passen muss.

### §8.2 Table 25 — ECDSA+P256+SHA256, Id 0x12, Bit 2

**Spec:** §8.2, Table 25, S. 93 — "256-bit Elliptic Curve Key for the secp256r1
curve [...] The digital signature shall be computed using the ECDSA-SHA256
algorithm".

**Repo:** `crates/security-pki/src/plugin.rs::sign_with` (Z. 515,
`ECDSA_P256_SHA256_ASN1_SIGNING`), `verify_signature_with_cert` (Z. 584)

**Tests:** `crates/security-pki/tests/e2e_handshake.rs::full_three_stage_handshake`,
`crates/security-pki/src/identity.rs::mutation_killers::detect_cert_algo_requires_both_ecdsa_oids`

**Status:** done

### §8.2 Table 25 — ECDSA+P384+SHA384, Id 0x13, Bit 3

**Spec:** §8.2, Table 25, S. 93 — "384-bit Elliptic Curve Key for the secp384r1
curve [...] The digital signature shall be computed using the ECDSA-SHA384
algorithm".

**Repo:** `crates/security-pki/src/identity.rs::CertKeyAlgo::EcdsaP384Sha384`; `crates/security-pki/src/plugin.rs::DsignScheme::EcdsaP384`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_p384_identity_authenticates_with_ecdsa_p384`

**Status:** `done` — Schlüssel, Handshake und Masken; eine Kette mit P-384-CA ist nicht getestet (die Kette im Test ist P-256).

### §8.2 Konstanten CNAME_* der Signaturalgorithmen

**Spec:** §8.2, S. 93 — "CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256 =
'RSASSA-PSS-MGF1SHA256+2048+SHA256'; CNAME_RSASSA_PKCS1_V15_2048_SHA256 =
'RSASSA-PKCS1-V1_5+2048+SHA256'; CNAME_ECDSA_P256_SHA256_NAME =
'ECDSA+P256+SHA256'; CNAME_ECDSA_P384_SHA384 = 'ECDSA+P384+SHA384';"

**Repo:** `crates/security/src/algorithm.rs::CNAME_RSASSA_PSS_MGF1SHA256_2048_SHA256`` u. a.

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — vier Konstanten.

### §8.2 Konstanten CID_* der Signaturalgorithmen

**Spec:** §8.2, S. 93 — "CID_RSASSA_PSS_MGF1SHA256_2048_SHA256 = 0x10;
CID_RSASSA_PKCS1_V15_2048_SHA256 = 0x11; CID_ECDSA_P256_SHA256 = 0x12;
CID_ECDSA_P384_SHA384 = 0x13;"

**Repo:** `crates/security/src/algorithm.rs::CID_RSASSA_PSS_MGF1SHA256_2048_SHA256`` u. a.

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — 0x10 bis 0x13.

### §8.2 Konstanten CBIT_* der Signaturalgorithmen

**Spec:** §8.2, S. 93 — "CBIT_RSASSA_PSS_MGF1SHA256_2048_SHA256 = 1 << 0;
CBIT_RSASSA_PKCS1_V15_2048_SHA256 = 1 << 1; CBIT_ECDSA_P256_SHA256 = 1 << 2;
CBIT_ECDSA_P384_SHA384 = 1 << 3;"

**Repo:** `crates/rtps/src/security_algo_info.rs::digital_signature_bit`
(Z. 61)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::spec_bit_constants_match`

**Status:** done

### §8.3 Key Establishment Algorithms

**Spec:** §8.3, S. 93 — "SPIs may use key establishment algorithms to establish
a shared secret key between two Endpoints".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Algorithmen folgen in
Table 26.

### §8.3 Table 26 — DHE+MODP-2048-256, Id 0x20, Bit 0

**Spec:** §8.3, Table 26, S. 94 — "The Diffie-Hellman Public Key shall be for
the 2048-bit MODP Group with 256-bit Prime Order Subgroup, see IETF RFC 5114
[...] section 2.3. [...] The Key Agreement Algorithm shall be the 'dhEphem,
C(2e, 0s, FFC DH) Scheme'".

**Repo:** `crates/security-keyexchange/src/modp.rs`, `crates/security-keyexchange/src/lib.rs::KxSuite::DhModp2048_256`; `crates/security-pki/src/plugin.rs::kx_suite_for_algo`

**Tests:** `crates/security-keyexchange/src/modp.rs::tests::the_constants_are_the_ones_printed_in_rfc_5114_2_3`, `crates/security-keyexchange/src/modp.rs::tests::the_group_has_the_structure_of_rfc_5114_2_3`, `crates/security-pki/src/plugin.rs::tests::the_handshake_runs_over_dhe_modp_2048_256`

**Status:** `done` — Gruppe von RFC 5114 §2.3 (die Konstanten werden im Test Byte für Byte mit dem Text des RFC verglichen, dazu 2048/256 Bit, q | p-1, g^q = 1); Namen `DH+MODP-2048-256` (gesendet) und `DHE+MODP-2048-256`. Nicht gegen einen anderen Hersteller gemessen; die Potenzierung läuft mit `crypto-bigint` (Montgomery-Form, `pow` über die volle Breite des 256-Bit-Exponenten, gleiche Zeit für jeden Exponenten), der Exponent wird beim Löschen genullt.

### §8.3 Table 26 — ECDHE-CEUM+P256, Id 0x21, Bit 1

**Spec:** §8.3, Table 26, S. 94 — "The Diffie-Hellman Public Key shall be for
the NIST's EC Curve P-256 [...] The Key Agreement Algorithm shall be the
'(Cofactor) Ephemeral Unified Model, C(2e, 0s, ECC CDH)'".

**Repo:** `crates/security-keyexchange/src/lib.rs::KxSuite::EcdhP256`
(Z. 100)

**Tests:** `crates/security-keyexchange/src/lib.rs::tests::p256_two_parties_derive_identical_secret`,
`crates/security-keyexchange/src/lib.rs::tests::p256_rejects_off_curve_point`

**Status:** done

### §8.3 Table 26 — ECDHE-CEUM+P384, Id 0x22, Bit 2

**Spec:** §8.3, Table 26, S. 94 — "The Diffie-Hellman Public Key shall be for
the NIST's EC Curve P-384 [...] (Cofactor) Ephemeral Unified Model".

**Repo:** `crates/security-keyexchange/src/lib.rs::KxSuite::EcdhP384`
(Z. 110)

**Tests:** `crates/security-keyexchange/src/lib.rs::tests::p384_matches_the_spec_table_and_derives_one_shared_secret`,
`crates/security-keyexchange/src/lib.rs::tests::p384_rejects_a_p256_public_key`

**Status:** done

### §8.3 Öffentlicher DH-Schlüssel MODP als Big-Endian-Zahl

**Spec:** §8.3, S. 94 — "If the public key corresponds to the
'DHE+MODP-2048-256' crypto algorithm, then: The OctetSeq's value shall contain
the big endian representation [...] of the DH public key".

**Repo:** `crates/security-keyexchange/src/modp.rs`

**Tests:** `crates/security-keyexchange/src/lib.rs::tests::modp_2048_256_agrees_and_has_id_0x20`

**Status:** `done` — Big-Endian-Zahl ohne führende Nullen, bis 256 Oktette; der Schlüssel des Peers wird nach SP 800-56A §5.6.2.3.1 geprüft.

### §8.3 Öffentlicher ECDHE-Schlüssel als Oktettfolge nach SEC 1 §2.3.3

**Spec:** §8.3, S. 94–95 — "If the public key corresponds to the
'ECDHE-CEUM+P256' or 'ECDHE-CEUM+P384' crypto algorithms, then: The OctetSeq's
value shall contain the octet string representation of the ECDHE public key.
The octet string representation encoding must conform with Sec. 2.3.3
'Elliptic-Curve-Point-to-Octet-String Conversion' of the SECG SEC 1".

**Repo:** `crates/security-keyexchange/src/lib.rs::KxSuite` (Z. 100,
unkomprimierter Punkt `0x04 || X || Y`)

**Tests:** `crates/security-keyexchange/src/lib.rs::tests::p256_public_key_is_65_bytes`,
`crates/security-keyexchange/src/lib.rs::tests::p384_matches_the_spec_table_and_derives_one_shared_secret`

**Status:** done

### §8.3 Konstanten CNAME_* der Schlüsselvereinbarung

**Spec:** §8.3, S. 95 — "CNAME_DHE_MODP_2048_256 = 'DHE+MODP-2048-256';
CNAME_ECDHE_CEUM_P256 = 'ECDHE-CEUM+P256'; CNAME_ECDHE_CEUM_P384 =
'ECDHE-CEUM+P384';"

**Repo:** `crates/security/src/algorithm.rs`; `crates/security-pki/src/plugin.rs::kagree_algo_str`

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`, `crates/security-pki/src/plugin.rs::tests::the_algorithm_names_follow_the_version_of_the_peer_and_the_setting`

**Status:** `done` — Die Konstanten der Spec stehen vor; als Vorgabe sendet der Initiator `ECDHE-CEUM+P256` und `DHE+MODP-2048-256`, `ECDH+prime256v1-CEUM` und `DH+MODP-2048-256` mit der Einstellung `1.1`. Gegen Cyclone, Fast DDS, OpenDDS nicht gemessen (Matrix); sie senden die Namen von 1.1. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §8.3 Konstanten CID_* der Schlüsselvereinbarung

**Spec:** §8.3, S. 95 — "CID_DHE_MODP_2048_256 = 0x20; CID_ECDHE_CEUM_P256 =
0x21; CID_ECDHE_CEUM_P384 = 0x22;"

**Repo:** `crates/security/src/algorithm.rs::CID_DHE_MODP_2048_256`` u. a.

**Tests:** `crates/security/src/algorithm.rs::tests::the_constants_are_the_ones_of_the_specification`

**Status:** `done` — 0x20 bis 0x22.

### §8.3 Konstanten CBIT_* der Schlüsselvereinbarung

**Spec:** §8.3, S. 95 — "CBIT_DHE_MODP_2048_256 = 1 << 0; CBIT_ECDHE_CEUM_P256
= 1 << 1; CBIT_ECDHE_CEUM_P384 = 1 << 2;"

**Repo:** `crates/rtps/src/security_algo_info.rs::key_establishment_bit`
(Z. 73)

**Tests:** `crates/rtps/src/security_algo_info.rs::tests::spec_bit_constants_match`

**Status:** done

### §8.3 X25519 nur als Option

**Spec:** §8.3, Table 26, S. 94 — die Tabelle nennt nur DHE+MODP-2048-256,
ECDHE-CEUM+P256 und ECDHE-CEUM+P384.

**Repo:** `crates/security-pki/src/plugin.rs::kx_suite_for_algo`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_algorithm_names_of_1_2_and_1_1_are_taken`

**Status:** `done` — Als Replier wird `X25519` nur angenommen, wenn diese Seite es selbst eingestellt hat (`allow_x25519`). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

---

## §9 Plugin Architecture

### §9.1.1 Service Plugin Interface Overview

**Spec:** §9.1.1, S. 97–98 — "There are five plugin SPIs: Authentication,
Access-Control, Cryptographic, Logging, and Data Tagging." (Figure 7,
Table 27)

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; jede Operation der fünf
Schnittstellen ist in §9.3 bis §9.7 einzeln geführt.

### §9.1.2 Plugins je DomainParticipant getrennt konfigurierbar

**Spec:** §9.1.2, S. 98 — "The Security Plugins shall be configurable
separately for each DomainParticipant even when multiple DomainParticipants
are constructed within the same Operating System Process and share the same
Address Space."

**Repo:** `crates/dcps/src/participant.rs::security_from_participant_properties`
(Z. 203, je Participant aus dessen `DomainParticipantQos`);
`crates/dcps/src/runtime.rs::with_security_bundle` (Z. 3788);
`crates/security-runtime/src/bundle.rs::SecurityBundle` (Z. 40)

**Tests:** `crates/dcps/tests/security_guid_wiring_e2e.rs::secured_participants_on_public_rust_api_use_identity_adjusted_guids_and_discover`

**Status:** done — der Test legt zwei Participants mit verschiedenen
Identitäten im selben Prozess an.

### §9.1.2 Plugin-Suite gebunden, bevor der DomainParticipant aktiviert ist

**Spec:** §9.1.2, S. 98 — "it is required for the Plugin Suite to be
initialized and bound by the time the DomainParticipant is enabled.
Therefore this process shall complete either during the
DomainParticipantFactory create_domain_participant or else during the
DomainParticipant enable operations".

**Repo:** `crates/dcps/src/participant.rs::security_from_participant_properties`
(Z. 203, aufgerufen in der Erzeugung des Participants, Z. 676)

**Tests:** `crates/dcps/tests/security_qos_properties.rs::a_complete_dds_sec_property_list_creates_a_secured_participant`,
`crates/dcps/tests/security_qos_properties.rs::an_incomplete_property_list_fails_the_create_and_names_the_property`

**Status:** done

### §9.2.1 SecurityException: message, code, minor_code

**Spec:** §9.2.1, Table 28, S. 98–99 — "SecurityException objects are
potentially returned from many of the calls in the Security plugins. They
are used to return an error code and message." Attribute: `message String`,
`code long`, `minor_code long`.

**Repo:** `crates/security/src/error.rs::SecurityError` (`detail`, `code`, `minor_code`), `SecurityErrorKind::code`

**Tests:** `crates/security/src/error.rs::tests::a_security_exception_has_message_code_and_minor_code`

**Status:** `done` — `message` ist `detail`; `code` die Nummer der Kategorie (1 bis 7), `minor_code` 0, wenn das Plugin keine setzt. Die Spec vergibt keine Werte.

---

### §9.3.1 Background (Non-Normative)

**Spec:** §9.3.1, S. 99 — "The discovery mechanism that registers
participants with the DDS middleware is enhanced with an authentication
protocol."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hintergrund.

### §9.3.2 Authentication Plugin Model

**Spec:** §9.3.2, Figure 8, S. 99–100 — Klassen IdentityHandle,
IdentityToken, IdentityStatusToken, HandshakeHandle, HandshakeMessageToken,
AuthRequestMessageToken, SharedSecretHandle,
AuthenticatedPeerCredentialToken, Schnittstellen Authentication und
AuthenticationListener, Enumeration AuthStatusKind.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Diagramm; die Elemente sind unten einzeln
geführt.

### §9.3.2.1 IdentityToken

**Spec:** §9.3.2.1, S. 100 — "An IdentityToken contains summary information
on the identity of a DomainParticipant in a manner that can be externalized
and propagated via DDS discovery. The specific content of the IdentityToken
shall be defined by each Authentication plugin specialization."

**Repo:** `crates/security/src/token.rs` (Alias `IdentityToken` auf
`DataHolder`, Z. 247–268);
`crates/security/src/authentication.rs::get_identity_token` (Z. 196)

**Tests:** `crates/security/src/token.rs::tests::pki_dh_identity_token_roundtrip`,
`crates/security-pki/src/plugin.rs::tests::get_identity_token_returns_decodable_spec_descriptor`

**Status:** done — der Inhalt des eingebauten Plugins steht in §10.3.

### §9.3.2.2 IdentityStatusToken über DCPSParticipantSecure

**Spec:** §9.3.2.2, S. 100–101 — "An IdentityStatusToken contains
authentication information of a DomainParticipant in a manner that can be
externalized and propagated via the DCPSParticipantSecure builtin Topic.
[...] The information shall be retrieved from the Authentication plugin by
calling the operation get_identity_status_token. And included in the
DCPSParticipantSecure builtin Topic."

**Repo:** `crates/security-pki/src/plugin.rs::get_identity_status_token`; `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (holt den Token, setzt ihn in den Beacon), `process_authentication_events` (neuer Token nach einer Änderung); `crates/discovery/src/spdp.rs::SpdpBeacon::serialize_secure` (trägt ihn), `serialize` (lässt ihn weg); `crates/discovery/src/security/stack.rs::identity_status_token`

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_beacon_announces_identity_token_and_discovery_triggers_handshake`,
`crates/discovery/src/security/stack.rs::tests::a_status_change_is_passed_on_with_the_new_token`,
`crates/security-pki/src/plugin.rs::tests::the_identity_status_token_carries_the_ocsp_response`,
`crates/security-pki/src/plugin.rs::tests::a_changed_status_token_is_reported_as_identity_status`

**Status:** `done` — die Laufzeit ruft `get_identity_status_token` und setzt den Token in `identity_status_token` des Beacons. Das sichere SPDP (DCPSParticipantsSecure) trägt ihn, das einfache SPDP (DCPSParticipants) trägt ihn nicht, und `c.pdata` des Handshakes auch nicht. Ändert das Plugin den Status (`on_status_changed`, `IDENTITY_STATUS`), holt die Laufzeit den neuen Token und sendet das sichere SPDP erneut; der geänderte Beacon ist eine neue Änderung des sicheren Writers.

### §9.3.2.3 IdentityHandle

**Spec:** §9.3.2.3, S. 101 — "An IdentityHandle is an opaque local reference
to internal state within the Authentication plugin, which uniquely
identifies a DomainParticipant."

**Repo:** `crates/security/src/authentication.rs::IdentityHandle` (Z. 34)

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_accepts_ca_signed_cert`

**Status:** done — Newtype über `u64`.

### §9.3.2.4 HandshakeHandle

**Spec:** §9.3.2.4, S. 101 — "A HandshakeHandle is an opaque local reference
used to refer to the internal state of a possible mutual authentication or
handshake protocol."

**Repo:** `crates/security/src/authentication.rs::HandshakeHandle` (Z. 38)

**Tests:** `crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`

**Status:** done

### §9.3.2.5 AuthRequestMessageToken

**Spec:** §9.3.2.5, S. 101 — "The AuthRequestMessageToken encodes
plugin-specific information that the Authentication plugins associated with
two DomainParticipant entities exchange to bootstrap the mutual
authentication handshake. [...] The AuthRequestMessageToken has class_id set
to GMCLASSID_SECURITY_AUTH_REQUEST".

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::validate_remote_identity` (`remote_auth_request_token` in, `local_auth_request_token` out); `crates/security-pki/src/plugin.rs::validate_remote_identity`; `crates/discovery/src/security/stack.rs::begin_handshake_with`, `on_auth_request`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/security-pki/src/plugin.rs::tests::the_auth_request_announces_the_challenge1_of_the_request`,
`crates/discovery/src/security/stack.rs::tests::an_auth_request_goes_out_on_discovery_and_binds_the_challenge`,
`crates/discovery/src/security/stack.rs::tests::an_auth_request_is_passed_to_the_plugin_whenever_it_arrives`

**Status:** `done` — `validate_remote_identity` nimmt den AuthRequestMessageToken des Peers und liefert den eigenen (`local_auth_request_token`, `None` ist TokenNIL). Der Stack sendet ihn mit `message_class_id` `dds.sec.auth_request`, hält einen vor der Discovery empfangenen für `validate_remote_identity` (höchstens 256) und gibt jeden empfangenen an das Plugin (§9.3.2.9.1 Regel 1).

### §9.3.2.6 HandshakeMessageToken

**Spec:** §9.3.2.6, S. 101 — "A HandshakeMessageToken encodes
plugin-specific information that the Authentication plugins associated with
two DomainParticipant entities exchange as part of the mutual
authentication handshake. [...] The HandshakeMessageToken has class_id set
to GMCLASSID_SECURITY_AUTH_HANDSHAKE".

**Repo:** `crates/security/src/token.rs` (Alias `HandshakeMessageToken`,
Z. 247–268); `crates/discovery/src/security/stack.rs::build_message`
(Z. 752, `class_id::AUTH`)

**Tests:** `crates/security/src/generic_message.rs::tests::handshake_request_token_in_message_data`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** done

### §9.3.2.7 AuthenticatedPeerCredentialToken

**Spec:** §9.3.2.7, S. 101 — "An AuthenticatedPeerCredentialToken encodes
plugin-specific information that the Authentication plugin obtains from a
remote DomainParticipant during the authentication process that is of
interest to the AccessControlPlugin. This information is accessible via the
operation get_authenticated_peer_credential_token."

**Repo:** `crates/security-pki/src/plugin.rs::get_authenticated_peer_credential_token`; `crates/security-pki/src/psk.rs::get_authenticated_peer_credential_token`; `crates/discovery/src/security/stack.rs::collect_completion` (ruft die Operation nach dem Abschluss, `peer_permissions` und `peer_identity_certificate` kommen aus ihm)

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`,
`crates/discovery/src/security/stack.rs::tests::the_peer_credential_token_gives_the_permissions_and_the_certificate`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — das Plugin liefert den Token nach einem abgeschlossenen Handshake (Initiator nach VALIDATION_OK_FINAL_MESSAGE, Replier nach VALIDATION_OK), vorher einen Fehler. Der Stack holt ihn bei jedem Abschluss, liest `c.perm` und `c.id` daraus für die Zugriffskontrolle und gibt ihn zurück. Die eigene Operation `peer_permissions` gibt es nicht mehr.

### §9.3.2.8 SharedSecretHandle

**Spec:** §9.3.2.8, S. 102 — "A SharedSecretHandle is an opaque local
reference to internal state within the AuthenticationPlugin containing a
secret that is shared between the AuthenticationPlugin implementation and
the peer AuthenticationPlugin implementation".

**Repo:** `crates/security/src/authentication.rs::SharedSecretHandle`
(Z. 43), `SharedSecretProvider` (Z. 57)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_shared_secret_returns_stored_value`

**Status:** done — die Bytes des Geheimnisses liest das Crypto-Plugin über
die eigene Schnittstelle `SharedSecretProvider`.

### §9.3.2.9 Zustandsautomat der Authentisierung (Figure 9)

**Spec:** §9.3.2.9, Figure 9, S. 102–103 — Zustände ValidateRemoteIdentity,
HandshakeInit, HandshakeMessageSend/-Wait/-Received, HandshakeCompleted mit
get_shared_secret, Wartezustände RetryWait1–5 für
VALIDATION_PENDING_RETRY, Zweig AuthRequestSend/AuthRequestWait und
AuthenticationTimeoutWait.

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

**Status:** `done` — alle Zustände von Figure 9: `validate_local_identity` (das Profil ruft sie nach einer festen Pause von 100 ms wieder, solange sie VALIDATION_PENDING_RETRY liefert; nur ein Schlüssel auf einem Token geht über die blockierende Operation), `validate_remote_identity` mit den Zweigen PENDING_HANDSHAKE_REQUEST und _MESSAGE, `begin_handshake_request`, `begin_handshake_reply`, `process_handshake` mit den Wartezuständen für VALIDATION_PENDING_RETRY (`set_retry_timeout`, 500 ms), das Senden und Wiederholen der Nachricht, der Zweig AuthRequestSend/-Wait, `get_shared_secret` nach OK und OK_FINAL_MESSAGE, AuthenticationTimeoutWait (`set_authentication_timeout`, 60 s). Die Frist ist nicht festgelegt („implementation-specific timeout“).

### §9.3.2.9 Wartezustände für VALIDATION_PENDING_RETRY und Zeitüberschreitung

**Spec:** §9.3.2.9, Figure 9, S. 103 — RetryWait1 bis RetryWait5 mit
`retry_timeout`, AuthenticationTimeoutWait mit `authentication_timeout`.

**Repo:** `crates/discovery/src/security/stack.rs::run_retries`, `retry_step`, `expire_handshakes`, `set_retry_timeout`, `set_authentication_timeout`

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_pending_validation_is_retried_after_the_retry_timeout`,
`crates/discovery/src/security/stack.rs::tests::a_pending_request_is_retried_after_the_retry_timeout`,
`crates/discovery/src/security/stack.rs::tests::a_pending_reply_and_a_pending_process_are_retried_with_their_message`,
`crates/discovery/src/security/stack.rs::tests::a_handshake_that_does_not_complete_ends_with_the_authentication_timeout`

**Status:** `done` — ein Schritt mit VALIDATION_PENDING_RETRY wird nach dem Wiederholungszeitraum mit denselben Eingaben (Token, Nachricht, `related_message_identity`) wieder aufgerufen; ein Handshake, der nach der Frist nicht abgeschlossen ist, endet: der Peer ist nicht authentisiert (Grund `Timeout`), die Handles gehen an das Plugin zurück, der Fehler steht in `take_handshake_failures`, der nächste Beacon beginnt neu. Ein abgeschlossener Handshake läuft nicht ab.

### §9.3.2.9.1 Regel 1: jede Nachricht AUTH_REQUEST und AUTH_HANDSHAKE an das Plugin

**Spec:** §9.3.2.9.1, S. 104 — "1. The DDS security implementation shall
pass to the AuthenticationPlugin any message received by the
BuiltinParticipantStatelessMessageReader that has a GenericMessageClassId
set to GMCLASSID_SECURITY_AUTH_REQUEST or GMCLASSID_SECURITY_AUTH_HANDSHAKE."

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`, `on_auth_request`

**Tests:** `crates/discovery/src/security/stack.rs::tests::an_auth_request_is_passed_to_the_plugin_whenever_it_arrives`,
`crates/discovery/src/security/stack.rs::tests::an_auth_request_goes_out_on_discovery_and_binds_the_challenge`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** `done` — Nachrichten mit `GMCLASSID_SECURITY_AUTH_HANDSHAKE` gehen an `begin_handshake_reply` oder `process_handshake`. Eine Nachricht mit `GMCLASSID_SECURITY_AUTH_REQUEST` geht an `validate_remote_identity` als `remote_auth_request_token`: von einem noch nicht entdeckten Participant wird sie bis zur Discovery gehalten, bei einem Participant ohne begonnenen Handshake ersetzt die Identität mit dem Token die erste, bei einem begonnenen Handshake prüft das Plugin den Token, und die dabei gemachte Identität geht sofort zurück.

### §9.3.2.9.1 Regel 2: gesendete Nachricht zwischenspeichern und wiederholen

**Spec:** §9.3.2.9.1, S. 104 — "2. [...] the DDS implementation shall cache
the message that was sent and set a timer. If a correct reply message is not
received when the timer expires, the state-machine shall send the same
message again. This process shall be repeated multiple times until a correct
message is received."

**Repo:** `crates/discovery/src/security/stack.rs::poll` (Z. 335, Wiederholung
alle 500 ms, Z. 347), `begin_handshake_with` (Z. 418, Wiederholung je Beacon
von SPDP), `on_stateless_message` (Z. 622, doppelte Anfrage erhält die
gespeicherte Antwort)

**Tests:** `crates/discovery/src/security/stack.rs::tests::lost_reply_recovered_by_resend_yields_matching_secret`,
`crates/discovery/src/security/stack.rs::tests::initiator_resends_auth_request_on_repeated_begin_while_incomplete`,
`crates/discovery/src/security/stack.rs::tests::a_duplicate_reply_after_completion_is_answered_with_the_final`

**Status:** done

### §9.3.2.9.1 Regel 3: nach VALIDATION_NOT_OK dieselbe Nachricht erneut senden

**Spec:** §9.3.2.9.1, S. 104 — "3. [...] If the plugin operation returns
VALIDATION_NOT_OK, the implementation transitions back to the previous state
that caused the message to be sent and resends the same message. [...] Rule
#3 makes authentication robust to an attacker trying to disrupt an
authentication exchange by sending bad replies."

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message` (`resend_now`), `poll_stateless`; `crates/security-pki/src/plugin.rs::process_handshake` (Zustand bleibt bis zur bestandenen Prüfung)

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_refused_reply_makes_the_stack_send_the_request_again`,
`crates/discovery/src/security/stack.rs::tests::a_forged_final_does_not_take_the_handshake_down`,
`crates/security-pki/src/plugin.rs::tests::a_forged_reply_does_not_take_the_handshake_down`,
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`,
`crates/security-pki/src/psk.rs::tests::a_forged_message_leaves_the_handshake_in_place`

**Status:** `done` — ein Schritt, den das Plugin ablehnt, lässt den Handshake, wie er war; der Stack sendet die zwischengespeicherte Nachricht, die den Schritt auslöste, beim nächsten `poll` erneut (je Peer höchstens eine je Tick). Die Fehlermeldung geht weiter an den Aufrufer. Der Name VALIDATION_NOT_OK steht in Table 30 nicht (Widerspruch in der Spec); gemeint ist ein Fehlschlag der Operation.

### §9.3.2.10 Nicht authentisierte DomainParticipants gleich behandeln

**Spec:** §9.3.2.10, S. 104–105 — "This can be either because they lack
support for the Authentication plugin being used, have incompatible plugins,
incompatible plugin configurations, or simply fail the authentication
protocol. All these cases shall be treated the same."

**Repo:** `crates/discovery/src/security/stack.rs::authentication_status`, `is_authenticated`, `mark_unauthenticated`, `PeerAuthentication`, `UnauthenticatedReason`; `crates/dcps/src/runtime.rs::builtin_match` (entscheidet nach `is_authenticated`), `spdp_security_handshake`

**Tests:** `crates/discovery/src/security/stack.rs::tests::every_reason_for_being_unauthenticated_is_one_case`,
`crates/dcps/src/runtime.rs::tests::spdp_beacon_announces_identity_token_and_discovery_triggers_handshake`

**Status:** `done` — ein Participant ohne IdentityToken, mit abgewiesener Identität, mit gescheitertem Handshake, mit unverträglicher Konfiguration (§7.3.23) oder nach der Frist ist `Unauthenticated`, der Grund dient dem Log. Das Matching der eingebauten Endpunkte (`builtin_match`) und die Annahme von Klartext fragen nur, ob der Peer authentisiert ist, und nach `allow_unauthenticated_participants`. Ein Fehlschlag von `begin_handshake_with` wird als `security.handshake.failed` gemeldet.

### §9.3.2.11 Table 29 — validate_local_identity

**Spec:** §9.3.2.11.2, S. 108–109 — "Validates the identity of the local
DomainParticipant. [...] this operation also returns the DomainParticipant
GUID_t that shall be used by the DDS implementation to uniquely identify the
DomainParticipant on the network." Parameter: out `local_identity_handle`,
out `adjusted_participant_guid`, `domain_id`, `participant_qos`,
`candidate_participant_guid`; Rückgabe VALIDATION_OK, VALIDATION_FAILED,
VALIDATION_PENDING_RETRY.

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::validate_local_identity`; `crates/security-pki/src/plugin.rs::validate_local_identity`, `validate_identity`; `crates/security-pki/src/psk.rs::validate_local_identity`; `crates/security-runtime/src/profile.rs`

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_returns_the_adjusted_guid_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_with_a_foreign_ca_fails_without_a_handle`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_accepts_ca_signed_cert`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_rejects_wrong_ca`,
`crates/security-pki/src/ocsp.rs::tests::a_responder_that_is_slow_gives_pending_retry_until_the_answer_is_in`,
`crates/security-pki/src/ocsp.rs::tests::a_revoked_answer_ends_the_pending_validation_with_a_failure`

**Status:** `done` — die Operation hat die Parameter `domain_id`, `participant_qos` (die PropertyQosPolicy) und `candidate_participant_guid` und liefert Ergebnis, Handle und `adjusted_participant_guid` (`LocalIdentityValidation`); Fehlschlag ist `Err` (VALIDATION_FAILED mit SecurityException); VALIDATION_PENDING_RETRY steht im nächsten Item.

### §9.3.2.11.2 validate_local_identity: adjusted_participant_guid, domain_id, participant_qos, PENDING_RETRY

**Spec:** §9.3.2.11.2, S. 109 — "Parameter (out) adjusted_participant_guid:
The GUID_t that the DDS implementation shall use [...] Parameter domain_id
[...] Parameter participant_qos [...] VALIDATION_PENDING_RETRY if
verification has not completed and the operation should be retried later."

**Repo:** `crates/security-pki/src/plugin.rs::validate_local_identity_nonblocking`, `register_local_identity`; `crates/security-runtime/src/profile.rs` (Wiederholung)

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_returns_the_adjusted_guid_of_table_59`,
`crates/security-pki/src/ocsp.rs::tests::a_responder_that_is_slow_gives_pending_retry_until_the_answer_is_in`,
`crates/security-pki/src/ocsp.rs::tests::a_revoked_answer_ends_the_pending_validation_with_a_failure`

**Status:** `done` — `adjusted_participant_guid` nach Table 59 (Bit 0 gesetzt, 47 Bit des SHA-256 des SubjectName, 48 Bit des SHA-256 der Kandidaten-GUID, dieselbe EntityId); das Profil nimmt die GUID aus dem Ergebnis. `domain_id` bleibt bei der Identität. Ein Zertifikat, das einen OCSP-Responder nennt, wird auf einem eigenen Thread geprüft: der erste Aufruf liefert VALIDATION_PENDING_RETRY ohne zu warten, jeder weitere das Ergebnis oder `PendingRetry`, bis die Antwort da ist.

### §9.3.2.11 Table 29 — get_identity_token

**Spec:** §9.3.2.11.9, S. 115 — "Retrieves an IdentityToken used to
represent on the network the identity of the DomainParticipant identified by
the specified IdentityHandle. [...] The handle must have been returned by a
successful call to validate_local_identity, otherwise the operation shall
return false and fill the SecurityException."

**Repo:** `crates/security/src/authentication.rs::get_identity_token`
(Z. 196); `crates/security-pki/src/plugin.rs::get_identity_token` (Z. 791,
unbekanntes Handle ergibt `BadArgument`)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_identity_token_returns_decodable_spec_descriptor`

**Status:** done

### §9.3.2.11 Table 29 — get_identity_status_token

**Spec:** §9.3.2.11.10, S. 115–116 — "Retrieves an AuthenticationToken used
to represent on the network the authentication state of the
DomainParticipant identified by the specified IdentityHandle."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::get_identity_status_token`; `crates/security-pki/src/plugin.rs::get_identity_status_token`; `crates/security-pki/src/psk.rs::get_identity_status_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_identity_status_token_carries_the_ocsp_response`,
`crates/security-pki/src/plugin.rs::tests::the_tokens_of_a_local_identity_need_a_local_identity_handle`,
`crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — PKI-DH liefert den Token von Table 53, `DDS:Auth:PSK` TokenNIL (Table 83); die Laufzeit ruft die Operation (siehe §9.3.2.2); ein Handle, das `validate_local_identity` nicht lieferte, ist ein Fehler.

### §9.3.2.11 Table 29 — set_participant_security_config

**Spec:** §9.3.2.11.11, S. 116 — "Configures various aspects of the
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

**Status:** `done` — die Operation nimmt `ParticipantSecurityConfig` (der Teil `algorithm_info` der Digital-Signature- und Key-Establishment-Masken) und liefert die angepasste Info; sie schlägt fehl, wo die Spec es verlangt. Die Laufzeit ruft sie mit `ParticipantSecurityConfig::unrestricted()` und kündigt die angepasste Info an (`algorithm_masks` gibt es nicht mehr).

### §9.3.2.11 Table 29 — set_permissions_credential_and_token

**Spec:** §9.3.2.11.12, S. 117 — "Associates the PermissionsCredentialToken
[...] returned by the AccessControl plugin operation
get_permissions_credential_token with the local DomainParticipant
identified by the IdentityHandle. This operation shall be called by the
middleware after calling validate_local_identity and prior to any calls to
validate_remote_identity."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::set_permissions_credential_and_token`; `crates/security-pki/src/plugin.rs::set_permissions_credential_and_token`; `crates/security-runtime/src/profile.rs`; `crates/security-pki/src/psk.rs::set_permissions_credential_and_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_permissions_credential_is_what_the_handshake_sends_as_c_perm`,
`crates/security-pki/src/plugin.rs::tests::a_permissions_credential_that_is_none_is_refused`,
`crates/security-pki/src/psk.rs::tests::table_83_set_participant_security_config_and_credential_do_nothing`

**Status:** `done` — PKI-DH legt Credential-Token und PermissionsToken je IdentityHandle ab; `dds.perm.cert` (Table 64) ist das `c.perm` des Handshakes dieser Identität, der Token die Antwort von `get_permissions_token`. Das Profil ruft die Operation mit dem signierten Berechtigungsdokument; ein Token der falschen Klasse, ohne Dokument oder kein DataHolder ist ein Fehler.

### §9.3.2.11 Table 29 — validate_remote_identity

**Spec:** §9.3.2.11.3, S. 109–110 — "Initiates the process of validating the
identity of the discovered remote DomainParticipant, represented as an
IdentityToken object." Parameter: out `remote_identity_handle`, out
`local_auth_request_token`, `remote_auth_request_token`,
`local_identity_handle`, `remote_identity_token`, `remote_participant_guid`;
Rückgabe VALIDATION_OK, VALIDATION_FAILED,
VALIDATION_PENDING_HANDSHAKE_REQUEST, VALIDATION_PENDING_HANDSHAKE_MESSAGE,
VALIDATION_PENDING_RETRY.

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::validate_remote_identity`, `RemoteIdentityValidation`; `crates/security-pki/src/plugin.rs::validate_remote_identity`; `crates/security-pki/src/psk.rs::validate_remote_identity`

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_remote_identity_orders_the_guids_like_table_59`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_compares_the_plugin_name_and_the_major_version`,
`crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_takes_only_a_pki_dh_identity_token`,
`crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_orders_remote_identities_by_guid`

**Status:** `done` — die Operation liefert `RemoteIdentityValidation` (Ergebnis, Handle, `local_auth_request_token`); die Ergebnisse OK, PENDING_HANDSHAKE_REQUEST, PENDING_HANDSHAKE_MESSAGE und PENDING_RETRY sind darstellbar, VALIDATION_FAILED ist `Err`.

### §9.3.2.11.3 validate_remote_identity: AuthRequest-Token, Rückgabewerte, GUID

**Spec:** §9.3.2.11.3, S. 109–110 — "If the operation returns
VALIDATION_PENDING_HANDSHAKE_REQUEST, then the DDS implementation shall call
the operation begin_handshake_request [...] Parameter (out)
local_auth_request_token [...] Parameter remote_auth_request_token".

**Repo:** `crates/security-pki/src/plugin.rs::validate_remote_identity`; `crates/discovery/src/security/stack.rs::continue_validation`

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_remote_identity_orders_the_guids_like_table_59`,
`crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`,
`crates/discovery/src/security/stack.rs::tests::a_pending_validation_is_retried_after_the_retry_timeout`

**Status:** `done` — das Plugin bestimmt die Rolle: eine größere GUID des Peers ist PENDING_HANDSHAKE_REQUEST, eine kleinere PENDING_HANDSHAKE_MESSAGE, die eigene GUID ein Fehler. Der Stack folgt dem Ergebnis. Die GUID des Peers bleibt bei seiner Identität (das Plugin prüft `c.pdata` gegen sie).

### §9.3.2.11 Table 29 — begin_handshake_request

**Spec:** §9.3.2.11.4, S. 110–111 — "This operation is used to initiate a
handshake. It shall be called by the DDS middleware solely as a result of
having a previous call to validate_remote_identity returning
VALIDATION_PENDING_HANDSHAKE_REQUEST." Parameter: out `handshake_handle`,
out `handshake_message`, `initiator_identity_handle`,
`replier_identity_handle`, `serialized_local_participant_data`.

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::begin_handshake_request`; `crates/security-pki/src/plugin.rs::begin_handshake_request`; `crates/security-pki/src/psk.rs::begin_handshake_request`; `crates/discovery/src/security/stack.rs::continue_validation`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_operations_need_the_result_of_validate_remote_identity`,
`crates/security-pki/src/plugin.rs::tests::request_token_has_spec_class_id_and_properties`,
`crates/discovery/src/security/stack.rs::tests::a_pending_request_is_retried_after_the_retry_timeout`

**Status:** `done` — `serialized_local_participant_data` ist ein Parameter (das `c.pdata`); der Aufruf ist nur für ein Handle erlaubt, das `validate_remote_identity` mit PENDING_HANDSHAKE_REQUEST lieferte (sonst PreconditionError); das Ergebnis ist PENDING_HANDSHAKE_MESSAGE mit der Nachricht oder PENDING_RETRY.

### §9.3.2.11 Table 29 — begin_handshake_reply

**Spec:** §9.3.2.11.5, S. 112–113 — "This operation shall be invoked by the
DDS implementation in reaction to the reception of the initial handshake
message that originated on a DomainParticipant that called the
begin_handshake_request operation." Parameter: out `handshake_handle`, out
`handshake_message_out`, `handshake_message_in`,
`initiator_identity_handle`, `replier_identity_handle`,
`serialized_local_participant_data`.

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::begin_handshake_reply`; `crates/security-pki/src/plugin.rs::begin_handshake_reply`; `crates/security-pki/src/psk.rs::begin_handshake_reply`; `crates/discovery/src/security/stack.rs::on_stateless_message`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_operations_need_the_result_of_validate_remote_identity`,
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`,
`crates/discovery/src/security/stack.rs::tests::a_pending_reply_and_a_pending_process_are_retried_with_their_message`

**Status:** `done` — wie `begin_handshake_request`: `serialized_local_participant_data` als Parameter, nur für ein Handle mit PENDING_HANDSHAKE_MESSAGE (sonst PreconditionError); der Stack ruft die Operation mit der Identität, die `validate_remote_identity` für den Initiator lieferte, nicht mit der lokalen.

### §9.3.2.11 Table 29 — process_handshake

**Spec:** §9.3.2.11.6, S. 113–114 — "This operation is used to continue a
handshake. [...] This operation generates a handshake_message_out
HandshakeMessageToken in response to a received handshake_message_in".
Rückgabe VALIDATION_OK, VALIDATION_FAILED,
VALIDATION_PENDING_HANDSHAKE_MESSAGE, VALIDATION_OK_FINAL_MESSAGE,
VALIDATION_PENDING_RETRY.

**Repo:** `crates/security/src/authentication.rs::HandshakeStepOutcome`; `crates/security-pki/src/plugin.rs::process_handshake`; `crates/security-pki/src/psk.rs::process_handshake`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`,
`crates/security/src/authentication.rs::tests::a_handshake_outcome_is_the_validation_result_of_table_30`

**Status:** `done` — der Initiator liefert nach der Antwort VALIDATION_OK_FINAL_MESSAGE (`SendFinalMessage`, unterscheidbar von VALIDATION_PENDING_HANDSHAKE_MESSAGE), der Replier nach dem Final VALIDATION_OK; PENDING_RETRY ist darstellbar.

### §9.3.2.11 Rückgabewerte und Parameter der Operationen des Handshakes

**Spec:** §9.3.2.11.4–§9.3.2.11.6, S. 110–114 — die Rückgabewerte
VALIDATION_OK_FINAL_MESSAGE und VALIDATION_PENDING_RETRY und der Parameter
`serialized_local_participant_data` ("CDR Big Endian Serialization for the
ParticipantBuiltInTopicDataSecure object associated with the local
DomainParticipant").

**Repo:** `crates/security/src/authentication.rs::HandshakeStepOutcome` (`SendMessage`, `SendFinalMessage`, `Complete`, `WaitingForPeer`, `PendingRetry`, `validation_result`, `message_out`); `crates/discovery/src/security/stack.rs::set_local_participant_data`

**Tests:** `crates/security/src/authentication.rs::tests::a_handshake_outcome_is_the_validation_result_of_table_30`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/discovery/src/security/stack.rs::tests::a_pending_request_is_retried_after_the_retry_timeout`

**Status:** `done` — alle sechs Werte von Table 30 sind darstellbar; die Daten des Participants gehen als Parameter `serialized_local_participant_data` in die Operationen. Die Laufzeit gibt die ParticipantBuiltinTopicData des Beacons als PL_CDR in Big Endian (ohne `identity_status_token`) an den Stack; sie sind nicht die ParticipantBuiltinTopicDataSecure, weil Cyclone und Fast DDS `c.pdata` als ParticipantBuiltinTopicData lesen (Table 56).

### §9.3.2.11.4 Felder der gesendeten ParticipantStatelessMessage

**Spec:** §9.3.2.11.4, S. 111 — "The message_class_id shall be set to
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

**Status:** `done` — `message_class_id`, `destination_participant_guid`, `source_guid`, das leere `related_message_identity` der Anfrage und die Folge der `sequence_number` (nächstes Item) stimmen.

### §9.3.2.11.4 sequence_number der vorigen Nachricht des Writers plus eins

**Spec:** §9.3.2.11.4, §9.3.2.11.5, §9.3.2.11.6, S. 111, 113, 114 — "the
sequence_number to the value in the previous message sent by the
BuiltinParticipantMessageWriter, incremented by one."

**Repo:** `crates/discovery/src/security/stack.rs::SecurityBuiltinStack::stateless_writer` (`next_message_identity`), `apply_outcome`

**Tests:** `crates/discovery/src/security/stack.rs::tests::the_sequence_number_counts_the_messages_of_the_writer`

**Status:** `done` — die `sequence_number` zählt die Nachrichten des Stateless-Writers (1, 2, 3 für drei Peers), nicht die an einen Peer.

### §9.3.2.11.5 Felder der Antwort: related_message_identity

**Spec:** §9.3.2.11.5, S. 113 — "The related_message_identity shall be set to
match the message_identity of the ParticipantStatelessMessage received that
triggered the execution of the begin_handshake_reply operation."

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`
(Z. 551, `msg.message_identity` als `related`)

**Tests:** `crates/security/src/generic_message.rs::tests::related_message_identity_links_reply_to_request`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** done — für `process_handshake` setzt der Code ebenfalls die
Identität der empfangenen Nachricht; der Text von §9.3.2.11.6 nennt an
dieser Stelle `begin_handshake_reply` (Widerspruch in der Spec).

### §9.3.2.11.5 Eingangsprüfung vor begin_handshake_reply

**Spec:** §9.3.2.11.5, S. 112 — "having received a message on the
BuiltinParticipantMessageReader with attributes set as follows:
message_class_id GMCLASSID_SECURITY_AUTH_HANDSHAKE, message_identity
source_guid matching the GUID_t of the DomainParticipant associated with the
initiator_identity_handle, destination_participant_guid matching the GUID_t
of the receiving DomainParticipant".

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_handshake_message_is_taken_only_as_the_spec_addresses_it`,
`crates/discovery/src/security/stack.rs::tests::the_reply_to_an_undiscovered_initiator_is_addressed_to_its_participant`,
`crates/discovery/src/security/stack.rs::tests::an_undiscovered_request_under_another_prefix_is_refused`

**Status:** `done` — `message_class_id`, `message_identity.source_guid` (das Präfix muss das des Absenders sein) und `destination_participant_guid` werden vor `begin_handshake_reply` geprüft; eine Nachricht an einen anderen Participant wird nicht angenommen, eine mit fremder `source_guid` ist ein Fehler. Das Plugin verlangt zusätzlich ein Handle mit PENDING_HANDSHAKE_MESSAGE.

### §9.3.2.11.6 Eingangsprüfung vor process_handshake

**Spec:** §9.3.2.11.6, S. 113 — "related_message_identity matching the
message_identity of the last ParticipantStatelessMessage sent to the peer
DomainParticipant associated with the handshake_handle.
destination_participant_guid matching the GUID_t of the receiving
DomainParticipant."

**Repo:** `crates/discovery/src/security/stack.rs::on_stateless_message`

**Tests:** `crates/discovery/src/security/stack.rs::tests::a_handshake_message_is_taken_only_as_the_spec_addresses_it`,
`crates/discovery/src/security/stack.rs::tests::a_forged_final_does_not_take_the_handshake_down`

**Status:** `done` — `related_message_identity` muss die letzte an den Peer gesendete Nachricht benennen und `destination_participant_guid` den Empfänger; sonst wird die Nachricht nicht an `process_handshake` gegeben und der Handshake bleibt.

### §9.3.2.11 Table 29 — get_shared_secret

**Spec:** §9.3.2.11.7, S. 114–115 — "Retrieves the SharedSecretHandle
resulting with a successfully completed handshake. [...] If an error occurs,
this method shall return the NILHandle and fill the SecurityException."

**Repo:** `crates/security/src/authentication.rs::shared_secret` (Z. 175);
`crates/security-pki/src/plugin.rs::shared_secret` (Z. 1059)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_shared_secret_returns_stored_value`,
`crates/security-pki/src/plugin.rs::tests::shared_secret_returns_bad_argument_for_unknown_handle`

**Status:** done — die Operation heißt `shared_secret` (Benennung).

### §9.3.2.11 Table 29 — get_authenticated_peer_credential_token

**Spec:** §9.3.2.11.8, S. 115 — "Retrieves the
AuthenticatedPeerCredentialToken resulting with a successfully completed
authentication of a discovered DomainParticipant. This operation shall be
called by the DDS middleware on each HandshakeHandle after the handshake
that uses that handle completes successfully".

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::get_authenticated_peer_credential_token`; `crates/security-pki/src/plugin.rs::get_authenticated_peer_credential_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`,
`crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/discovery/src/security/stack.rs::tests::the_peer_credential_token_gives_the_permissions_and_the_certificate`

**Status:** `done` — siehe §9.3.2.7; der Aufruf vor dem Abschluss ist ein Fehler.

### §9.3.2.11 Table 29 — set_listener

**Spec:** §9.3.2.11.13, S. 117 — "Sets the AuthenticationListener that the
Authentication plugin will use to notify the DDS middleware infrastructure
of events relevant to the Authentication of DDS Participants."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::set_listener`, `AuthenticationListener`; `crates/security-pki/src/plugin.rs::set_listener`; `crates/discovery/src/security/stack.rs::install_listener`

**Tests:** `crates/security-pki/src/plugin.rs::tests::revoking_an_identity_tells_the_listener_and_invalidates_the_handle`,
`crates/security-pki/src/plugin.rs::tests::an_expired_identity_certificate_is_revoked`,
`crates/security-pki/src/psk.rs::tests::table_83_set_listener_saves_the_listener`,
`crates/discovery/src/security/stack.rs::tests::a_revoked_identity_ends_the_authentication_of_its_participant`

**Status:** `done` — die Operation setzt den Listener (`None` nimmt ihn weg); der Discovery-Stack setzt seinen eigenen.

### §9.3.2.11 Table 29 — return_identity_token

**Spec:** §9.3.2.11.14, S. 117 — "Returns the IdentityToken object to the
plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_identity_token`, `IssuedTokens`; `crates/security-pki/src/plugin.rs::return_identity_token`; `crates/security-pki/src/psk.rs::return_identity_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — das Plugin merkt sich die ausgegebenen IdentityTokens und nimmt nur diese zurück, jeden so oft, wie er ausgegeben wurde; ein fremder oder schon zurückgegebener Token ist ein Fehler.

### §9.3.2.11 Table 29 — return_identity_status_token

**Spec:** §9.3.2.11.15, S. 117 — "Returns the IdentityStatusToken object to
the plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_identity_status_token`, `IssuedTokens`; `crates/security-pki/src/plugin.rs::return_identity_status_token`; `crates/security-pki/src/psk.rs::return_identity_status_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — das Plugin merkt sich die ausgegebenen IdentityStatusTokens und nimmt nur diese zurück, jeden so oft, wie er ausgegeben wurde; ein fremder oder schon zurückgegebener Token ist ein Fehler.

### §9.3.2.11 Table 29 — return_authenticated_peer_credential_token

**Spec:** §9.3.2.11.16, S. 118 — "Returns the
AuthenticatedPeerCredentialToken object to the plugin so it can be disposed
of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_authenticated_peer_credential_token`, `IssuedTokens`; `crates/security-pki/src/plugin.rs::return_authenticated_peer_credential_token`; `crates/security-pki/src/psk.rs::return_authenticated_peer_credential_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — das Plugin merkt sich die ausgegebenen AuthenticatedPeerCredentialTokens und nimmt nur diese zurück, jeden so oft, wie er ausgegeben wurde; ein fremder oder schon zurückgegebener Token ist ein Fehler.

### §9.3.2.11 Table 29 — return_handshake_handle

**Spec:** §9.3.2.11.17, S. 118 — "Returns the HandshakeHandle object to the
plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_handshake_handle`; `crates/security-pki/src/plugin.rs::return_handshake_handle`; `crates/discovery/src/security/stack.rs::release_peer_handles`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/discovery/src/security/stack.rs::tests::a_handshake_that_does_not_complete_ends_with_the_authentication_timeout`

**Status:** `done` — das Plugin nimmt ein ausgegebenes Handle einmal zurück und gibt den Zustand des Handshakes frei (Anfrage, Antwort, Zuordnung zum Geheimnis, Token des Peers); der Stack gibt die Handles einer beendeten Authentisierung und eines verlorenen Participant zurück.

### §9.3.2.11 Table 29 — return_identity_handle

**Spec:** §9.3.2.11.18, S. 118 — "Returns the IdentityHandle object to the
plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_identity_handle`; `crates/security-pki/src/plugin.rs::return_identity_handle`; `crates/security-pki/src/psk.rs::return_identity_handle`; `crates/discovery/src/security/stack.rs::release_peer_handles`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_returned_identity_handle_is_disposed_of`,
`crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`

**Status:** `done` — das Plugin nimmt die Handles von `validate_local_identity` und `validate_remote_identity` je einmal zurück und gibt den Zustand frei (Version des Peers, GUID, Challenges, Rolle).

### §9.3.2.11 Table 29 — return_sharedsecret_handle

**Spec:** §9.3.2.11.19, S. 118 — "Returns the SharedSecretHandle object to
the plugin so it can be disposed of."

**Repo:** `crates/security/src/authentication.rs::AuthenticationPlugin::return_sharedsecret_handle`; `crates/security-pki/src/plugin.rs::return_sharedsecret_handle`; `crates/security-pki/src/psk.rs::return_sharedsecret_handle`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/psk.rs::tests::the_handshake_plugin_gives_the_peer_credential_and_takes_back_what_it_issued`

**Status:** `done` — ein Geheimnis, das ein abgeschlossener Handshake lieferte, geht einmal zurück; die Bytes und die Challenges sind danach weg. Der Stack tut es erst, wenn der Peer verloren ist (das Crypto-Plugin liest das Geheimnis über den `SharedSecretProvider`, solange der Peer da ist).

### §9.3.2.11 Eigene Operationen außerhalb von Table 29

**Spec:** §9.3.2.11, Table 29, S. 105–107 — die Tabelle zählt die
Operationen der Schnittstelle abschließend auf.

**Repo:** `crates/security/src/authentication.rs::AuthenticationExtensions`, `AuthenticationPlugin`

**Tests:** `crates/security/src/authentication.rs::tests::stub_can_be_boxed`

**Status:** `done` — `AuthenticationPlugin` hat genau die Operationen von Table 29. Was darüber hinaus gebraucht wird, steht im Supertrait `AuthenticationExtensions` und ist dort begründet: `plugin_class_id` (Name für das Log), `set_algo_nul_terminate`, `set_remote_message_auth_supported` und `set_peer_algorithm_names` (Schreibweise je Hersteller des Peers), `get_permissions_token` (das Token für `PID_PERMISSIONS_TOKEN`, bis AccessControl es liefert) und `check_identities` (die Uhr: die Schnittstelle von Table 29 hat keine Operation, die die Zeit bringt, das Plugin braucht sie für abgelaufene Zertifikate). `peer_permissions`, `peer_identity_certificate`, `local_auth_request_token`, `algorithm_masks` und `set_local_participant_data` sind entfallen.

### §9.3.2.11.1 Table 30 — ValidationResult_t

**Spec:** §9.3.2.11.1, Table 30, S. 108 — VALIDATION_OK, VALIDATION_FAILED,
VALIDATION_PENDING_RETRY, VALIDATION_PENDING_HANDSHAKE_REQUEST,
VALIDATION_PENDING_HANDSHAKE_MESSAGE, VALIDATION_OK_FINAL_MESSAGE.

**Repo:** `crates/security/src/authentication.rs::ValidationResult`, `HandshakeStepOutcome::validation_result`

**Tests:** `crates/security/src/authentication.rs::tests::a_handshake_outcome_is_the_validation_result_of_table_30`,
`crates/security/src/authentication.rs::tests::only_ok_and_ok_final_message_succeed`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_orders_the_guids_like_table_59`

**Status:** `done` — alle sechs Werte; VALIDATION_FAILED ist der Fehlerfall `Err`, `ValidationResult::Failed` benennt ihn.

### §9.3.2.12 Table 31 — AuthenticationListener::on_revoke_identity

**Spec:** §9.3.2.12.2, S. 119–120 — "Revokes the identity of the participant
identified by the IdentityHandle. The corresponding IdentityHandle becomes
invalid. As a result of this, the DDS middleware shall terminate any
communications with the DomainParticipant associated with that handle."

**Repo:** `crates/security/src/authentication.rs::AuthenticationListener`; `crates/security-pki/src/plugin.rs::revoke_identity`, `check_identities`; `crates/discovery/src/security/stack.rs::drain_auth_events`; `crates/dcps/src/runtime.rs::process_authentication_events`

**Tests:** `crates/security-pki/src/plugin.rs::tests::revoking_an_identity_tells_the_listener_and_invalidates_the_handle`,
`crates/security-pki/src/plugin.rs::tests::an_expired_identity_certificate_is_revoked`,
`crates/security-pki/src/plugin.rs::tests::a_crl_that_lists_the_peer_revokes_its_identity`,
`crates/discovery/src/security/stack.rs::tests::a_revoked_identity_ends_the_authentication_of_its_participant`,
`crates/dcps/src/runtime.rs::tests::a_revoked_identity_ends_the_authentication_through_the_runtime_tick`

**Status:** `done` — das Plugin ruft `on_revoke_identity`, wenn eine Identität widerrufen wird (`revoke_identity`, abgelaufenes Zertifikat, neue CRL); das Handle ist danach ungültig, Handshake und Geheimnis gehen mit. Der Stack beendet die Authentisierung des Participant, authentisiert ihn nicht erneut und meldet ihn; die Laufzeit ignoriert den Participant (Endpunkte und Matches gehen) und gibt seine Schlüssel frei.

### §9.3.2.12.2 Neue Revision des Schlüsselmaterials nach on_revoke_identity

**Spec:** §9.3.2.12.2, S. 119 — "The DDS middleware shall create a new
revision of any Key Material that it had shared with the DomainParticipant
identified by the IdentityHandle and send the revised Key Material to the
remaining DomainParticipant entities [...] DataReaders with DURABILITY kind
different from VOLATILE shall retail at least the last 8 revisions of the Key
Material. Likewise, DataWriters shall not send messages that use KeyMaterial
that is more than 7 revisions earlier than the current."

**Repo:** `crates/dcps/src/runtime.rs::process_authentication_events`, `renew_key_material_after_revocation`; `crates/security-runtime/src/shared.rs::SharedSecurityGate::revise_local_entity_keys`, `activate_key_revision`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_revocation_makes_a_new_revision_of_the_keys_of_the_participant_and_its_endpoints`

**Status:** `done` — nach `on_revoke_identity` ruft die Laufzeit `revise_local_entity_keys` (§9.5.1.8.7): Schlüssel des Participant und aller lokalen Endpunkte erhalten eine neue Revision. Der Participant-Token geht sofort an die verbleibenden authentisierten Participants, die Endpunkt-Tokens beim nächsten Tick; danach folgt `activate_key_revision` (§9.5.1.8.8). Der widerrufene Participant hält nur die frühere Revision und kann die neue nicht entschlüsseln. Die Aktivierung wartet nicht auf die Endpunkt-Tokens.

### §9.3.2.12 Table 31 — AuthenticationListener::on_status_changed

**Spec:** §9.3.2.12.3, S. 120 — "Informs the DomainParticipant that a status
associated with the Authentication plugin, or an Identity managed by the
plugin has changed."

**Repo:** `crates/security/src/authentication.rs::AuthenticationListener::on_status_changed`; `crates/security-pki/src/plugin.rs::check_identities`; `crates/discovery/src/security/stack.rs::drain_auth_events`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_changed_status_token_is_reported_as_identity_status`,
`crates/discovery/src/security/stack.rs::tests::a_status_change_is_passed_on_with_the_new_token`

**Status:** `done` — das Plugin ruft die Operation, wenn sich der IdentityStatusToken ändert; der Stack merkt die Änderung, die Laufzeit holt den neuen Token.

### §9.3.2.12.1 Table 32 — AuthStatusKind IDENTITY_STATUS

**Spec:** §9.3.2.12.1, Table 32, S. 119 — "IDENTITY_STATUS Indicates a
change to an identity status. [...] The changed IdentityStatusToken shall be
propagated by the DDS implementation to the other DomainParticipants using
the DCPSParticipantsSecure builtin Topic."

**Repo:** `crates/security/src/authentication.rs::AuthStatusKind`; `crates/discovery/src/security/stack.rs::take_identity_status_changed`; `crates/dcps/src/runtime.rs::process_authentication_events`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_changed_status_token_is_reported_as_identity_status`,
`crates/discovery/src/security/stack.rs::tests::a_status_change_is_passed_on_with_the_new_token`

**Status:** `done` — `AuthStatusKind::IdentityStatus`; ein geänderter Token geht in den Beacon und wird im sicheren SPDP (DCPSParticipantsSecure) an die authentisierten Participants gesendet.

---

### §9.4.1 Background (Non-Normative)

**Spec:** §9.4.1, S. 120 — "Once a DomainParticipant is authenticated, its
permissions need to be validated and enforced."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hintergrund.

### §9.4.2 AccessControl Plugin Model

**Spec:** §9.4.2, Figure 10, S. 120–121 — Schnittstellen AccessControl und
AccessControlListener, Typen PermissionsToken, PermissionsHandle,
ParticipantSecurityConfig, TopicSecurityConfig, EndpointSecurityConfig,
ParticipantSecurityAlgorithmInfo, EndpointSecurityAlgorithmInfo.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Diagramm; die Elemente sind unten einzeln
geführt.

### §9.4.2 Laufzeit entscheidet ohne AccessControlPlugin

**Spec:** §9.4.2.9, Table 39, S. 129 — "AccessControl [...] Operations"; §2.3:
"Clause 9 (the plugin model) [...] The conformance to the plugin model is at
the UML level".

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate` (hält ein `Box<dyn AccessControlPlugin>`, `application_access_control`), `crates/dcps/src/runtime.rs`, `crates/dcps/src/participant.rs`, `crates/dcps/src/publisher.rs`, `crates/dcps/src/access_request.rs`

**Tests:** `crates/security-runtime/src/shared.rs::tests::a_plugin_of_the_application_decides_every_check`,
`crates/security-runtime/src/shared.rs::tests::a_refusing_plugin_of_the_application_refuses_every_check`,
`crates/dcps/tests/security_access_plugin_e2e.rs::create_topic_is_asked_of_the_plugin`,
`crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`

**Status:** `done` — jede Entscheidung der Laufzeit (Teilnehmer, Topic, Endpunkt, Instanz, Match, entfernte Teilnehmer und Endpunkte, Konfigurationen, Tokens) geht durch `dyn AccessControlPlugin`. Ohne Anwendungs-Plugin baut das Gate das eingebaute aus Governance, Permissions, CA und Identitätszertifikat. Ein Plugin der Anwendung ersetzt es (`set_access_control`).

### §9.4.2.1 PermissionsToken

**Spec:** §9.4.2.1, S. 121 — "A PermissionsToken contains summary
information on the permissions for a DomainParticipant in a manner that can
be externalized and propagated over DDS discovery."

**Repo:** `crates/security/src/token.rs` (Alias `PermissionsToken`,
Z. 247–268); `crates/security-pki/src/plugin.rs::get_permissions_token`
(Z. 806)

**Tests:** `crates/security/src/token.rs::tests::permissions_token_roundtrip`,
`crates/security-pki/src/plugin.rs::tests::permissions_token_announces_spec_class_id_when_permissions_set`

**Status:** done — erzeugt vom Authentication-Plugin, nicht von
AccessControl (siehe Table 39 — get_permissions_token); der Inhalt steht in
§10.4.

### §9.4.2.2 PermissionsCredentialToken

**Spec:** §9.4.2.2, S. 122 — "A PermissionsCredentialToken encodes the
permissions and access information for a DomainParticipant in a manner that
can be externalized and sent over the network. [...] The
PermissionsCredentialToken is intended for dissemination during the
authentication handshake."

**Repo:** `crates/security-permissions/src/plugin.rs::get_permissions_credential_token` (Table 64: `dds.perm.cert`), `validate_remote_permissions` (liest `c.perm`)

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`

**Status:** `done` — das Plugin erzeugt das Token `DDS:Access:PermissionsCredential` mit dem signierten Dokument und liest es beim entfernten Teilnehmer aus dem `AuthenticatedPeerCredentialToken` (`c.perm`, §10.3.2.3). Die Laufzeit holt es über `stack.peer_credential_token`.

### §9.4.2.3 PermissionsHandle

**Spec:** §9.4.2.3, S. 122 — "A PermissionsHandle is an opaque local
reference to internal state within the AccessControl plugin."

**Repo:** `crates/security/src/access_control.rs::PermissionsHandle` (Z. 30)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::permit_on_exact_match`

**Status:** done — die Laufzeit verwendet kein Handle, sondern hält die
Dokumente in `SharedSecurityGate` (siehe oben).

### §9.4.2.4 ParticipantSecurityConfig: Typ

**Spec:** §9.4.2.4, S. 122 — "@extensibility (APPENDABLE) struct
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

**Status:** `done` — der Typ hat alle Member von §9.4.2.4 einschließlich `is_rtps_psk_protected`, `is_key_revision_enabled`, `ac_participant_properties` und `algorithm_info`; beide Plugins füllen ihn.

### §9.4.2.4 Table 33 — allow_unauthenticated_participants

**Spec:** §9.4.2.4, Table 33, S. 122–123 — "If
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

**Status:** `done` — der Wert kommt aus dem Plugin, entscheidet das Matching nach §9.8.3.1/§9.8.3.2 und die Annahme von Klartext. Das Verdrängen bei gleicher GUID steht im eigenen Item darunter.

### §9.4.2.4 allow_unauthenticated_participants: gleiche GUID verdrängt

**Spec:** §9.4.2.4, Table 33, S. 122 — "as long as there is not already a
valid authentication with the same DomainParticipant's GUID. Additionally, a
DomainParticipant that later authenticates would kick out the
unauthenticated DomainParticipant if it has the same GUID."

**Repo:** `crates/dcps/src/runtime.rs::displace_unauthenticated_endpoints`, `store_remote_permissions`, `spdp_plain_is_liveliness_only`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_authenticating_participant_displaces_the_unauthenticated_one_with_its_guid`

**Status:** `done` — mit `allow_unauthenticated_participants` TRUE verwirft `store_remote_permissions` beim Abschluss des Handshakes alles, was von der GUID vor der Authentisierung über die reguläre Discovery gelernt wurde (Endpunkte im SEDP-Cache und ihre Zuordnungen); die Endpunkte des authentisierten Teilnehmers kommen über die sicheren Kanäle. Eine Klartext-Ankündigung einer bereits authentisierten GUID erneuert nur die Lease (`spdp_plain_is_liveliness_only`).

### §9.4.2.4 Klartext über Loopback auch bei allow_unauthenticated_participants=FALSE

**Spec:** §9.4.2.4, Table 33, S. 123 — "If allow_unauthenticated_participants
is FALSE, the DomainParticipant shall enforce the authentication of remote
DomainParticipants".

**Repo:** `crates/security-runtime/src/shared.rs::classify_inbound`,
`::set_plaintext_loopback`; Einstellung `zerodds.sec.plaintext_loopback`

**Tests:** `crates/security-runtime/src/shared.rs::tests::classify_inbound_plain_on_loopback_needs_the_setting`,
`crates/dcps/src/runtime.rs::tests::inbound_loopback_plaintext_needs_the_setting`

**Status:** `done` — Klartext von Loopback gilt wie Klartext von jeder anderen
Schnittstelle. Angenommen wird er nur mit `zerodds.sec.plaintext_loopback`
(Opt-in) oder ohne Security-Konfiguration. Gemessen: grün auf dem Linux-Testhost.

### §9.4.2.4 Table 33 — is_access_protected

**Spec:** §9.4.2.4, Table 33, S. 123 — "If is_access_protected is TRUE, then
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

**Status:** `done` — das Plugin liefert `is_access_protected` aus `enable_join_access_control`; bei TRUE ruft die Laufzeit nach dem Handshake `validate_remote_permissions` und `check_remote_participant` auf und ignoriert einen Teilnehmer, dessen Prüfung scheitert (`ignore_unauthorized_participant`): sein Writer wird nie zugeordnet. Bei FALSE wird nicht geprüft. Gemessen mit drei laufenden Laufzeiten über UDP (Log `sec-access-1018-112603.log`). Der Test fand, dass der Ausschluss auf einer Laufzeit ohne `DomainParticipant` mit dem nächsten SPDP verfiel; behoben.

### §9.4.2.4 Table 33 — is_rtps_axk_protected

**Spec:** §9.4.2.4, Table 33, S. 123 — "Indicates whether RTPS
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

**Status:** `done` — ganze Nachrichten werden nach `rtps_protection_kind` geschützt; die Bedingung (1) wird beim Lesen der Governance geprüft (Item darunter).

### §9.4.2.4 is_rtps_axk_protected verlangt allow_unauthenticated_participants=FALSE

**Spec:** §9.4.2.4, Table 33, S. 123 — "If is_rtps_axk_protected is TRUE
then: (1) allow_unauthenticated_participants must be FALSE."

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_rule`

**Tests:** `crates/security-permissions/src/governance.rs::tests::rtps_protection_with_unauthenticated_participants_is_refused`

**Status:** `done` — `rtps_protection_kind` ungleich NONE mit `allow_unauthenticated_participants` TRUE macht die Governance ungültig.

### §9.4.2.4 Table 33 — is_rtps_psk_protected

**Spec:** §9.4.2.4, Table 33, S. 123–124 — "Indicates whether all RTPS
Messages (including RTPS Bootstrapping Messages) should be protected: RTPS
Messages that are not otherwise protected by an 'Authenticated Partcipant
Exchanged Key' will be protected with a Pre-Shared Key. [...] If
is_rtps_psk_protected is FALSE, then: (1) RTPS Bootstrapping Messages sent
shall NOT be transformed".

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`; `crates/security-runtime/src/qos_properties.rs::rtps_psk_from_properties`; `crates/security-runtime/src/psk_gate.rs`; `crates/dcps/src/runtime.rs::secure_outbound_bytes`, `secure_inbound_bytes`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_governance_decides_the_pre_shared_key_protection`, `crates/security-runtime/src/psk_gate.rs::tests::the_gate_protects_a_message_with_the_pre_shared_key`, `crates/dcps/src/runtime.rs::tests::a_pre_shared_key_protects_the_bootstrapping_messages`

**Status:** `done` — Das `rtps_psk_protection_kind` der Governance (sonst die Property von Table 84) bestimmt, ob und wie das Gate die RTPS-Nachrichten mit dem Pre-Shared Key transformiert (§10.9); mit gesetztem Schlüssel gehen Bootstrap-Nachrichten mit den Flags A und P hinaus, ungeschützte werden verworfen. Der Enclave-Pfad ohne Property-Liste hat keine Passphrase und schützt nicht. Lauf `cargo test -p zerodds-security-runtime --lib` und `-p zerodds-dcps --features security --lib`: grün, Logs `/root/zerodds-rc7/agent-logs/sec-crypto-1018-085015.log`, `…085119.log`.

### §9.4.2.4 Table 33 — is_discovery_protected

**Spec:** §9.4.2.4, Table 33, S. 124 — "If is_discovery_protected is TRUE,
the submessages sent by the DCPSPublicationsSecure and
DCPSSubscriptionsSecure DataWriters shall be transformed using the
CryptoTransform operation encode_datawriter_submessage [...] Independent of
the setting of is_discovery_protected, the CryptoTransform operations
encode_serialized_payload and decode_serialized_payload shall never be called
for the DCPSPublicationsSecure and DCPSSubscriptionsSecure entities."

**Repo:** `crates/security-runtime/src/shared.rs::discovery_protection`
(Z. 537); `crates/dcps/src/runtime.rs::protect_secure_spdp` (Z. 17028)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_discovery_protection`

**Status:** done — der Schutz der Submessages der sicheren Endpunkte von
SEDP richtet sich nach `discovery_protection_kind`; die Laufzeit schützt die
SerializedPayload nur für Writer der Anwendung (Kommentar in
`crates/dcps/src/runtime.rs`, Z. 4704–4712).

### §9.4.2.4 Table 33 — is_liveliness_protected

**Spec:** §9.4.2.4, Table 33, S. 124–125 — "If is_liveliness_protected is
TRUE, the submessages sent by the BuiltinParticipantMessageSecure DataWriter
shall be transformed using the CryptoTransform operation
encode_datawriter_submessage [...] the CryptoTransform operations
encode_serialized_payload and decode_serialized_payload shall never be called
for the BuiltinParticipantMessageSecure entities."

**Repo:** `crates/security-runtime/src/shared.rs::liveliness_protection`
(Z. 622)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_liveliness_protection`

**Status:** done

### §9.4.2.4 Table 33 — is_key_revision_enabled

**Spec:** §9.4.2.4, Table 33, S. 125 — "Indicates the DDS middleware will
revise Key Material for its entities when certain events are encountered
(e.g. the identity of a matched DomainParticipant is revoked)."

**Repo:** `crates/security-runtime/src/shared.rs::key_revision_enabled`; `crates/dcps/src/runtime.rs::renew_key_material_after_revocation`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_revocation_makes_a_new_revision_of_the_keys_of_the_participant_and_its_endpoints`, `crates/dcps/src/runtime.rs::tests::a_revocation_makes_no_revision_unless_the_governance_enables_it`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`

**Status:** `done` — Mit `<enable_key_revision>` erzeugt die Laufzeit bei einer widerrufenen Identität eine neue Revision (`revise_local_entity_keys`), sendet sie an die übrigen Participants und aktiviert sie (`activate_key_revision`); ohne das Element geschieht nichts. Lauf `cargo test -p zerodds-security-runtime --lib` und `-p zerodds-dcps --features security --lib`: grün, Logs `/root/zerodds-rc7/agent-logs/sec-crypto-1018-085015.log`, `…085119.log`.

### §9.4.2.4 Table 33 — plugin_participant_attributes

**Spec:** §9.4.2.4, Table 33, S. 125 — "This field is a holder for
plugin-specific information that is propagated via discovery as part of the
ParticipantSecurityInfo".

**Repo:** `crates/rtps/src/participant_security_info.rs::plugin_attrs`
(Z. 33); `crates/dcps/src/runtime.rs` (Z. 8722, Belegung im Beacon)

**Tests:** `crates/rtps/src/participant_security_info.rs::tests::fully_protected_sets_all_encrypt_bits`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** done — die Bits leitet die Laufzeit aus der Governance ab, nicht
ein Plugin.

### §9.4.2.4 Table 33 — ac_participant_properties

**Spec:** §9.4.2.4, Table 33, S. 125 — "Additional properties to add to the
participant_properties parameter passed to the CryptoKeyFactory operation
register_local_participant."

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`; `crates/security-runtime/src/shared.rs::ensure_local` (übergibt die Properties an `register_local_participant`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — das Plugin liefert die Properties; das Gate reicht sie in `participant_properties` an den CryptoKeyFactory. Die IDL der Spec nennt das Member `ac_endpoint_properties`, Table 33 `ac_participant_properties` (Widerspruch in der Spec); der Code heißt `ac_participant_properties`.

### §9.4.2.4 Table 33 — algorithm_info

**Spec:** §9.4.2.4, Table 33, S. 125 — "Cryptographic algorithms used and
supported by the participant. See 7.3.14."

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`, `governance.rs::AllowedCryptoAlgorithms`; `crates/dcps/src/runtime.rs` (SPDP)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::without_allowed_algorithms_every_algorithm_is_supported`,
`crates/security-permissions/src/plugin.rs::tests::the_identity_certificates_algorithms_must_be_allowed`,
`crates/dcps/src/runtime.rs::tests::the_allowed_algorithms_cut_the_announced_ones`

**Status:** `done` — `supported_mask` und `required_mask` folgen `<allowed_crypto_algorithms>`; ohne das Element gilt CRYPTO_ALGORITHM_SET_ALL (Table 70). Das Identitätszertifikat muss die erlaubten Algorithmen verwenden. Die Laufzeit kündigt die Masken im SPDP an.

### §9.4.2.5 Table 34 — Abbildung auf ParticipantSecurityAttributesMask

**Spec:** §9.4.2.5, Table 34, S. 125–126 — is_rtps_axk_protected
(0x1 << 0), is_discovery_protected (0x1 << 1), is_liveliness_protected
(0x1 << 2), is_key_revision_enabled (0x1 << 3), is_rtps_psk_protected
(0x1 << 4), is_valid (0x1 << 31).

**Repo:** `crates/rtps/src/participant_security_info.rs::attrs`; `crates/dcps/src/runtime.rs` (SPDP aus der Plugin-Konfiguration)

**Tests:** `crates/security/src/access_control.rs::tests::the_participant_mask_has_the_bits_of_table_34`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — alle Bits der Table 34 (0, 1, 2, 3, 4, 31) kommen aus `ParticipantSecurityConfig::attributes_mask`.

### §9.4.2.5 IS_KEY_REVISION_ENABLED und IS_RTPS_PSK_PROTECTED fehlen

**Spec:** §9.4.2.5, Table 34, S. 126 — "#define
PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_KEY_REVISION_ENABLED (0x00000001 <<
3) [...] #define PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_PROTECTED
(0x00000001 << 4)".

**Repo:** `crates/security/src/access_control.rs::participant_attributes_mask`; `crates/rtps/src/participant_security_info.rs::attrs`

**Tests:** `crates/security/src/access_control.rs::tests::the_participant_mask_has_the_bits_of_table_34`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — beide Bits werden aus der Governance gesetzt (`enable_key_revision`, `rtps_psk_protection_kind`) und angekündigt.

### §9.4.2.5 Table 35 — ParticipantSecurityOptionalAttributesMask

**Spec:** §9.4.2.5, Table 35, S. 126 —
PARTICIPANT_SECURITY_OPT_ATTRIBUTES_FLAG_ALLOW_UNAUTHENTICATED_PARTICIPANTS
(0x0001 << 0), PARTICIPANT_SECURITY_OPT_ATTRIBUTES_FLAG_IS_ACCESS_PROTECTED
(0x0001 << 1), "applies to both is_set and value fields".

**Repo:** `crates/security/src/access_control.rs::participant_optional_attributes_mask`; `crates/rtps/src/participant_security_info.rs::opt_attrs`; `crates/security-runtime/src/qos_properties.rs::PROP_OPTIONAL_ATTRIBUTES`

**Tests:** `crates/security/src/access_control.rs::tests::the_optional_mask_has_the_bits_of_table_35`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — das dritte Member `participant_security_optional_attributes` wird standardmäßig gesendet (Wert aus `allow_unauthenticated_participants` und `is_access_protected`). Mit `org.zerodds.sec.optional_attributes=omit` bleibt es weg. Gegen Cyclone DDS, Fast DDS und OpenDDS nicht gemessen; erwartet: Peers, die nur zwei Masken lesen, ignorieren das Member.

### §9.4.2.6 TopicSecurityConfig: Typ

**Spec:** §9.4.2.6, S. 126 — "@extensibility (APPENDABLE) struct
TopicSecurityConfig { boolean is_read_protected; boolean is_write_protected;
boolean is_discovery_protected; boolean is_liveliness_protected; };"

**Repo:** `crates/security/src/access_control.rs::TopicSecurityConfig`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`

**Status:** `done` — der Typ hat die vier Member; das Plugin füllt sie aus der Topic Rule.

### §9.4.2.6 Table 36 — is_read_protected

**Spec:** §9.4.2.6, Table 36, S. 126 — "Indicates if read access to the
Topic is protected. If is_read_protected is FALSE, then local DataReader
creation and remote DataReader matching can proceed without further
access-control mechanisms imposed. Otherwise, they shall be checked using
the AccessControl operations."

**Repo:** `crates/security-permissions/src/governance.rs::read_access_controlled`
(Z. 414); `crates/security-runtime/src/shared.rs::check_create_datareader`
(Z. 441), `check_remote_datareader` (Z. 362)

**Tests:** `crates/security-permissions/src/governance.rs::tests::access_control_flags_come_from_the_matching_rule`,
`crates/security-permissions/src/governance.rs::tests::an_uncontrolled_topic_needs_no_grant`

**Status:** done — die Prüfung des entfernten Readers ist unter Table 39 —
check_remote_datareader bewertet.

### §9.4.2.6 Table 36 — is_write_protected

**Spec:** §9.4.2.6, Table 36, S. 127 — "Indicates if write access to the
Topic is protected. If is_write_protected is FALSE, then local DataWriter
creation and remote DataWriter matching can proceed without further
access-control mechanisms imposed."

**Repo:** `crates/security-permissions/src/governance.rs::write_access_controlled`
(Z. 405); `crates/security-runtime/src/shared.rs::check_create_datawriter`
(Z. 406), `check_remote_datawriter` (Z. 326)

**Tests:** `crates/security-permissions/src/governance.rs::tests::access_control_flags_come_from_the_matching_rule`,
`crates/security-permissions/src/governance.rs::tests::the_granted_direction_still_creates`

**Status:** done

### §9.4.2.6 Table 36 — is_discovery_protected je Topic

**Spec:** §9.4.2.6, Table 36, S. 127 — "If is_discovery_protected is TRUE,
then discovery information for that entity shall be sent using the
SEDPbuiltinPublicationsSecureWriter SEDPbuiltinSubscriptionsSecureWriter. If
is_discovery_protected is FALSE, then discovery information for that entity
shall be sent using the SEDPbuiltinPublicationsWriter or
SEDPbuiltinSubscriptionsWriter."

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`, `SharedSecurityGate::topic_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — `is_discovery_protected` des Topics wählt die Endpunkte von SEDP und geht in die Maske ein. TypeLookup: siehe das Item unten.

### §9.4.2.6 Table 36 — is_discovery_protected und TypeLookup

**Spec:** §9.4.2.6, Table 36, S. 127 — "Also impacts which Types can be
looked-up using the regular (non-secure) Builtin TypeLookup Endpoints and
which require use of the Builtin Secure TypeLookup Endpoints".

**Repo:** `crates/dcps/src/runtime.rs::hidden_from_regular_type_lookup`, `without_hidden_types`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_regular_type_lookup_does_not_hand_out_the_types_of_protected_topics`

**Status:** `done` — die regulären TypeLookup-Endpunkte geben die Typen von Topics mit `is_discovery_protected` nicht heraus; die sicheren Endpunkte tun es.

### §9.4.2.6 Table 36 — is_liveliness_protected je Topic


**Spec:** §9.4.2.6, Table 36, S. 127 — "If is_liveliness_protected is TRUE
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

**Status:** `done` — der Participant schlägt auf dem plain Writer für die Writer
seiner nicht sensiblen Topics und auf dem sicheren Writer für die der sensiblen
(`is_liveliness_protected` der Regel des Topics); mit Topics beider Arten
schlägt er auf beiden, jeder Writer mit eigenen Sequenznummern, eigener Historie
und eigenen Reader-Proxies. Ohne Writer entscheidet `liveliness_protection_kind`
der Domain. Wiederhergestellt: die Auswahl aus `62017f49303` ging im Merge
`13cb1667912` verloren, als der WLP-Endpunkt von wip/dcps-1.4 (ein
`ReliableWriter` mit Reader-Proxies) übernommen wurde; sie sitzt jetzt auf
diesem WLP-Endpunkt (`WlpEndpoint` hält den plain und den sicheren
`ReliableWriter`). Der plain WLP-DATA wird nicht mit dem Schlüssel des sicheren
Writers geschützt. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1018-073220.log`).

### §9.4.2.7 EndpointSecurityConfig: Typ

**Spec:** §9.4.2.7, S. 127 — "@extensibility (APPENDABLE) struct
EndpointSecurityConfig : TopicSecurityConfig { boolean
is_submessage_protected; boolean is_payload_protected; boolean
is_key_protected; PluginEndpointSecurityAttributesMask
plugin_endpoint_attributes; PropertySeq ac_endpoint_properties;
EndpointSecurityAlgorithmInfo algorithm_info; };"

**Repo:** `crates/security/src/access_control.rs::EndpointSecurityConfig`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security/src/access_control.rs::tests::the_endpoint_mask_has_the_bits_of_table_38`

**Status:** `done` — der Typ hat alle Member von §9.4.2.7; die Endpunkt-Maske entsteht aus ihm.

### §9.4.2.7 Table 37 — is_submessage_protected

**Spec:** §9.4.2.7, Table 37, S. 128 — "If is_submessage_protected is TRUE
and the entity is a DataWriter, the submessages sent by the DataWriter shall
be transformed using the CryptoTransform operation
encode_datawriter_submessage [...] and the entity is a DataReader, the
submessages sent by the DataReader shall be transformed using the
CryptoTransform operation encode_datareader_submessage".

**Repo:** `crates/security-runtime/src/shared.rs::metadata_protection`
(Z. 515); `crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info`

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** done — welche Submessages eines Readers geschützt werden, steht
bei §7.4.7 und §10.5.

### §9.4.2.7 Table 37 — is_payload_protected

**Spec:** §9.4.2.7, Table 37, S. 128 — "If is_ payload_protected is TRUE and
the entity is a DataWriter, the serialized data sent by the DataWriter shall
be transformed by calling encode_serialized_payload. If is_
payload_protected is TRUE and the entity is a DataReader, the serialized
data received by the DataReader shall be transformed by calling
decode_serialized_payload".

**Repo:** `crates/security-runtime/src/shared.rs::data_protection` (Z. 496),
`encode_serialized_payload` (Z. 1024), `decode_serialized_payload`
(Z. 1040)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** done

### §9.4.2.7 Table 37 — is_key_protected

**Spec:** §9.4.2.7, Table 37, S. 128 — "If is_key_protected is TRUE, then the
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

**Status:** `done` — das Member wird aus `data_protection_kind` ENCRYPT gesetzt
(§10.4.1.2.6), und bei diesem Schutz trägt jedes Sample und jedes
Dispose/Unregister eines Typs mit Schlüssel den KeyHash als MD5 des Key-Holders
(§7.4.4, auch für Schlüssel bis 16 Oktette; unbekannter Key-Holder:
`NotAllowedBySecurity`); `payload_key_hash` gilt für genau die Endpunkte, deren
`is_key_protected` damit TRUE ist. Tests grün auf dem Linux-Testhost
(`/root/zerodds-rc7/agent-logs/security-1018-083929.log`,
`/root/zerodds-rc7/agent-logs/security-1018-082254.log`).

### §9.4.2.7 Table 37 — plugin_endpoint_attributes

**Spec:** §9.4.2.7, Table 37, S. 128 — "This field is a holder for
plugin-specific information that is propagated via discovery as part of the
EndpointSecurityInfo".

**Repo:** `crates/rtps/src/endpoint_security_info.rs::plugin_attrs` (Z. 56);
`crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info`

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::protection_bit_accessors_read_correctly`

**Status:** done — die Bits der Standardmaske setzt die Laufzeit (`user_endpoint_security_info`); die Maske des Plugins bildet `security_runtime::plugin_attributes` (§7.3.23, §7.3.24: Sache des Plugins), die Laufzeit reicht sie weiter.

### §9.4.2.7 Table 37 — ac_endpoint_properties

**Spec:** §9.4.2.7, Table 37, S. 128–129 — "Additional properties to add to
the datawriter_properties or datareader_properties passed to the
CryptoKeyFactory operations register_local_datawriter and
register_local_datareader."

**Repo:** `crates/security-permissions/src/plugin.rs::get_datawriter_security_config`; `crates/security-runtime/src/shared.rs::register_local_endpoint` (Properties an den CryptoKeyFactory); `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`

**Status:** `done` — das Plugin liefert die Properties; die Laufzeit reicht sie an `register_local_datawriter` und `register_local_datareader`.

### §9.4.2.7 Table 37 — algorithm_info

**Spec:** §9.4.2.7, Table 37, S. 129 — "Cryptographic algorithms required to
interoperate with the endpoint. See 7.3.16."

**Repo:** `crates/security-runtime/src/gate_crypto.rs::endpoint_required_mask`; `crates/dcps/src/runtime.rs::local_endpoint_cipher`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_announced_cipher_is_the_algorithm_info_of_the_plugin`, `crates/security-runtime/src/shared.rs::tests::the_basic_protection_kind_of_an_endpoint_decides_gmac_or_gcm`

**Status:** `done` — Das `required_mask` des `adjusted_algorithm_info`, das das Plugin bei `register_local_datawriter`/`_datareader` liefert, ist die Ankündigung als PID 0x1013 (nur wenn es vom Vorgabewert des Teilnehmers abweicht, §7.3.15.2). Lauf `cargo test -p zerodds-security-runtime --lib` und `-p zerodds-dcps --features security --lib`: grün, Logs `/root/zerodds-rc7/agent-logs/sec-crypto-1018-085015.log`, `…085119.log`.

### §9.4.2.8 Table 38 — Abbildung auf EndpointSecurityAttributesMask

**Spec:** §9.4.2.8, Table 38, S. 129 — is_read_protected (0x1 << 0),
is_write_protected (0x1 << 1), is_discovery_protected (0x1 << 2),
is_submessage_protected (0x1 << 3), is_payload_protected (0x1 << 4),
is_key_protected (0x1 << 5), is_liveliness_protected (0x1 << 6), is_valid
(0x1 << 31).

**Repo:** `crates/rtps/src/endpoint_security_info.rs::attrs` (Z. 27);
`crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info`

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::encoded_bytes_le_match_spec_layout`,
`crates/security-runtime/src/plugin_attributes.rs::tests::liveliness_protected_sets_0x40_per_spec_9_4_1_3`

**Status:** done

### §9.4.2.9 Table 39 — validate_local_permissions

**Spec:** §9.4.2.9.1, S. 134 — "Validates the permissions of the local
DomainParticipant. The operation returns a PermissionsHandle object, if
successful. [...] This operation shall be called before the
DomainParticipant is enabled." Parameter `auth_plugin`, `identity`,
`domain_id`, `participant_qos`.

**Repo:** `crates/security-permissions/src/plugin.rs::validate_local_permissions` (Signatur, Subjekt, Gültigkeit, Algorithmen); `crates/security-runtime/src/shared.rs`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_signed_local_documents_validate_and_decide`,
`tools/secure-permissions/tests/access_plugin_signed.rs::local_documents_that_do_not_validate_are_refused`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_profiles_gate_goes_through_the_builtin_plugin`

**Status:** `done` — die Operation prüft die CMS-Signatur beider Dokumente gegen die Permissions CA, den SubjectName gegen das Identitätszertifikat und liefert das `PermissionsHandle`; das Gate ruft sie vor dem Aktivieren des Teilnehmers.

### §9.4.2.9 Table 39 — validate_remote_permissions

**Spec:** §9.4.2.9.2, S. 134–135 — "Validates the permissions of the
previously authenticated remote DomainParticipant, given the
PermissionsToken object received via DDS discovery and the
PermissionsCredentialToken obtained as part of the authentication process."

**Repo:** `crates/security-permissions/src/plugin.rs::validate_remote_permissions`; `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_document_needs_the_permissions_ca`,
`crates/security-permissions/src/plugin.rs::tests::a_remote_validation_without_ca_or_credential_is_refused`

**Status:** `done` — Signatur und SubjectName des `c.perm` werden geprüft; ein Fehler liefert den Handle NIL mit Mindestcode.

### §9.4.2.9 Table 39 — check_create_participant

**Spec:** §9.4.2.9.3, S. 135 — "When the local DomainParticipant is created,
its permissions must allow it to join the DDS Domain specified by the
domain_id. [...] This operation shall also be called when the application
calls the operation set_qos() [...] If the check_create_participant does not
succeed (return true), the set_qos operation shall fail with the
NOT_ALLOWED_BY_SECURITY error."

**Repo:** `crates/security-permissions/src/plugin.rs::check_create_participant`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::check_create_participant_honours_the_domain_range`,
`crates/dcps/tests/security_access_plugin_e2e.rs::participant_set_qos_is_asked_of_the_plugin`

**Status:** `done` — die Prüfung läuft beim Anlegen und bei `set_qos`; die Ablehnung ist NOT_ALLOWED_BY_SECURITY.

### §9.4.2.9.3 check_create_participant: Fehlercode und set_qos

**Spec:** §9.4.2.9.3, S. 135 — "If the check_create_participant does not
succeed (return true), the set_qos operation shall fail with the
NOT_ALLOWED_BY_SECURITY error."; §9.8.1, Schritt 3, S. 171: "the
create_participant operation shall return NIL and set the return code to
NOT_ALLOWED_BY_SECURITY."

**Repo:** `crates/dcps/src/participant.rs` (JoinDenied und `set_qos` auf NOT_ALLOWED_BY_SECURITY)

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::participant_set_qos_is_asked_of_the_plugin`

**Status:** `done` — `set_qos` des Teilnehmers fragt das Plugin mit der neuen QoS; eine Ablehnung ergibt NOT_ALLOWED_BY_SECURITY.

### §9.4.2.9 Table 39 — check_create_datawriter

**Spec:** §9.4.2.9.4, S. 135–136 — "When the local DomainParticipant creates
a DataWriter for topic_name with the specified DataWriterQos associated with
the data_tag, its permissions must allow this. [...] This operation shall
also be called when the application calls the operation set_qos() on a
DataWriter". Parameter `permissions_handle`, `domain_id`, `topic_name`,
`qos`, `partition`, `data_tag`.

**Repo:** `crates/dcps/src/runtime.rs::check_create_endpoint`, `crates/dcps/src/publisher.rs` (`set_qos`)

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::the_granted_direction_and_the_open_topic_still_create`

**Status:** `done` — Anlegen und `set_qos` des DataWriters fragen das Plugin mit Topic, Partitionen, QoS und Data Tags.

### §9.4.2.9 check_create_datawriter/datareader: qos und data_tag

**Spec:** §9.4.2.9.4, §9.4.2.9.5, S. 136–137 — "Parameter qos: The QoS
policies of the local DataWriter. [...] Parameter data_tag: The data tags
that the local DataWriter is requesting to be associated with its data. This
argument can be nil if it is not considered for access control."

**Repo:** `crates/dcps/src/access_request.rs`; `DataWriterQos.data_tags`, `DataReaderQos.data_tags`

**Tests:** `crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`,
`crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`

**Status:** `done` — `qos` und `data_tag` gehen in die Anfrage; ein vom Plugin verweigertes Tag verhindert das Anlegen.

### §9.4.2.9 Table 39 — check_create_datareader

**Spec:** §9.4.2.9.5, S. 136–137 — "When the local DomainParticipant creates
a DataReader for a Topic for topic_name with the specified DataReaderQos qos
associated with the data_tag, its permissions must allow this."

**Repo:** `crates/dcps/src/runtime.rs::check_create_endpoint`, `crates/dcps/src/subscriber.rs`

**Tests:** `crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`,
`crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`

**Status:** `done` — wie `check_create_datawriter`, für DataReader.

### §9.4.2.9 Table 39 — check_create_topic

**Spec:** §9.4.2.9.6, S. 137 — "When an entity of the local
DomainParticipant creates a Topic with topic_name and TopicQos qos its
permissions must allow this. [...] If the check_create_topic does not
succeed (return true), the set_qos operation shall fail with the
NOT_ALLOWED_BY_SECURITY error."

**Repo:** `crates/dcps/src/participant.rs`, `topic.rs`

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::create_topic_is_asked_of_the_plugin`,
`crates/security-permissions/src/plugin.rs::tests::check_create_topic_follows_governance_and_grant`

**Status:** `done` — Anlegen und `set_qos` des Topics fragen das Plugin.

### §9.4.2.9 Table 39 — check_local_datawriter_register_instance

**Spec:** §9.4.2.9.7, S. 137–138 — "In case the access control requires a
finer granularity at the instance level, this operation enforces the
permissions of the local DataWriter. The key identifies the instance being
registered".

**Repo:** `crates/dcps/src/publisher.rs` (`register_on_write`)

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::instance_operations_are_asked_of_the_plugin`

**Status:** `done` — `register_instance` fragt das Plugin mit dem serialisierten Schlüssel; eine verweigerte Instanz wird nicht registriert.

### §9.4.2.9 Table 39 — check_local_datawriter_dispose_instance

**Spec:** §9.4.2.9.8, S. 138 — "The key has to match the permissions for
disposing an instance."

**Repo:** `crates/dcps/src/publisher.rs` (`dispose_key`)

**Tests:** `crates/dcps/tests/security_access_plugin_e2e.rs::instance_operations_are_asked_of_the_plugin`

**Status:** `done` — `dispose` fragt das Plugin mit dem Schlüssel.

### §9.4.2.9 Table 39 — check_remote_participant

**Spec:** §9.4.2.9.9, S. 138–139 — "When the remote DomainParticipant is
discovered, the domain_id and, optionally, the DomainParticipantQoS are
checked to verify that joining that DDS Domain and using that QoS is allowed
by its permissions. [...] If the check_remote_participant does not succeed
(return true), the remote participant shall be considered invalid."

**Repo:** `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`,
`crates/security-permissions/src/plugin.rs::tests::a_remote_token_of_another_class_or_major_version_is_refused`

**Status:** `done` — nach dem Handshake prüft die Laufzeit Domain, Token-Klasse und Grant; bei `is_access_protected` wird der Teilnehmer bei Ablehnung ignoriert.

### §9.4.2.9 Table 39 — check_remote_datawriter

**Spec:** §9.4.2.9.10, S. 139 — "This operation shall be called by a
DomainParticipant prior to matching a local DataReader belonging to that
DomainParticipant with a DataWriter belonging to a different (peer)
DomainParticipant. This operation shall also be called whenever a
DomainParticipant detects a QoS change for a DataWriter belonging to a
different (peer) DomainParticipant".

**Repo:** `crates/dcps/src/runtime.rs` (Zuordnung entfernter Writer)

**Tests:** `crates/security-runtime/src/shared.rs::tests::a_peer_without_a_grant_is_denied_on_an_access_controlled_topic`,
`crates/security-runtime/src/shared.rs::tests::a_plugin_of_the_application_decides_every_check`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::the_granted_direction_and_the_open_topic_still_create`

**Status:** `done` — vor dem Matching mit Partitionen, Data Tags und QoS des entfernten Writers; ohne bekannten Grant ist das Ergebnis auf einem geschützten Topic „deny“, mit Eingang des Grants wird neu zugeordnet.

### §9.4.2.9 Table 39 — check_remote_datareader

**Spec:** §9.4.2.9.11, S. 139–140 — "This operation shall be called by a
DomainParticipant prior to matching a local DataWriter [...] with a
DataReader belonging to a different (peer) DomainParticipant. [...] If the
operation returns true, the relay_only parameter shall be remembered by the
DDS middleware and passed to the register_matched_remote_datareader
operation on the CryptoKeyFactory."

**Repo:** `crates/dcps/src/runtime.rs` (`relay_only_readers`)

**Tests:** `crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`,
`crates/security-runtime/src/shared.rs::tests::relaying_needs_the_relay_grant_on_an_access_controlled_topic`,
`crates/security-permissions/src/plugin.rs::tests::relaying_needs_the_relay_action_where_the_topic_is_controlled`

**Status:** `done` — wie `check_remote_datawriter`; `relay_only` wird gemerkt und gibt dem Reader nur das Material der Submessage (§9.5.1.9.5).

### §9.4.2.9 Table 39 — check_remote_topic

**Spec:** §9.4.2.9.12, S. 140 — "When the remote DomainParticipant creates a
certain topic, the topic_name and optionally the TopicQoS extracted from the
topic_data are verified to ensure the remote DomainParticipant permissions
allow it to create the DDS Topic with the specified QoS."

**Repo:** `crates/dcps/src/runtime.rs`, `crates/dcps/src/access_request.rs::remote_topic_qos`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_nil_handle_is_refused_on_a_controlled_topic_only`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`

**Status:** `done` — die Laufzeit prüft das Topic des entfernten Endpunkts mit dem Topic-QoS aus `topic_data` vor dem Matching.

### §9.4.2.9 Table 39 — check_local_datawriter_match

**Spec:** §9.4.2.9.13, S. 140–141 — "Provides the means for the
AccessControl plugin to enforce access control rules that are based on the
DataTag associated with DataWriter and a matching DataReader."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`,
`crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`

**Status:** `done` — die Laufzeit fragt das Plugin vor dem Match mit dem entfernten Reader (Data Tags).

### §9.4.2.9 Table 39 — check_local_datareader_match

**Spec:** §9.4.2.9.14, S. 141 — "Provides the means for the AccessControl
plugin to enforce access control rules that are based on the DataTag
associated with a DataReader and a matching DataWriter."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`,
`crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`

**Status:** `done` — die Laufzeit fragt das Plugin vor dem Match mit dem entfernten Writer.

### §9.4.2.9 Table 39 — check_remote_datawriter_register_instance

**Spec:** §9.4.2.9.15, S. 141–142 — "In case the access control requires a
finer granularity at the instance level, this operation enforces the
permissions of the remote DataWriter."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`,
`crates/security-runtime/src/shared.rs::tests::the_builtin_plugin_is_not_asked_for_what_it_always_permits`

**Status:** `done` — die Laufzeit fragt das Plugin für Instanzen entfernter Writer (Schlüssel aus dem Sample).

### §9.4.2.9 Table 39 — check_remote_datawriter_dispose_instance

**Spec:** §9.4.2.9.16, S. 142 — "The key has to match the permissions for
disposing an instance."

**Repo:** `crates/dcps/src/runtime.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`,
`crates/security-runtime/src/shared.rs::tests::the_builtin_plugin_is_not_asked_for_what_it_always_permits`

**Status:** `done` — wie `register_instance`, für `dispose`.

### §9.4.2.9 Table 39 — get_permissions_token

**Spec:** §9.4.2.9.17, S. 142 — "Retrieves a PermissionsToken object. The
PermissionsToken is propagated via DDS discovery to summarize the
permissions of the DomainParticipant identified by the specified
PermissionsHandle."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`,
`crates/security-permissions/src/plugin.rs::tests::the_permissions_token_class_is_1_2_unless_named`

**Status:** `done` — das Token `DDS:Access:Permissions:1.2` mit `dds.perm_ca.sn` und `dds.perm_ca.algo`; die Laufzeit schreibt es in den SPDP.

### §9.4.2.9 Table 39 — get_permissions_credential_token

**Spec:** §9.4.2.9.18, S. 143 — "Retrieves a PermissionsCredentialToken
object that can be used to represent on the network the permissions of the
DomainParticipant identified by the specified PermissionsHandle."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`

**Status:** `done` — das Token `DDS:Access:PermissionsCredential` mit `dds.perm.cert`.

### §9.4.2.9 Table 39 — set_listener

**Spec:** §9.4.2.9.19, S. 143 — "Sets the listener for the AccessControl
plugin."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::revoking_permissions_tells_the_listener`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — `AccessControlListener` und `set_listener` stehen in der Schnittstelle; das Plugin ruft `on_revoke_permissions`.

### §9.4.2.9 Table 39 — return_permissions_token

**Spec:** §9.4.2.9.20, S. 143 — "Returns the PermissionsToken to the plugin
for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — das Plugin nimmt nur ausgegebene Tokens zurück.

### §9.4.2.9 Table 39 — return_permissions_credential_token

**Spec:** §9.4.2.9.21, S. 143 — "Returns the PermissionsCredentialToken to
the plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — wie `return_permissions_token`.

### §9.4.2.9 Table 39 — get_participant_security_config

**Spec:** §9.4.2.9.22, S. 143–144 — "This operation shall be called by the
DDS middleware as part of the creation or enabling of the DDS
DomainParticipant. [...] The returned ParticipantSecurityConfig shall be used
to call the operation set_participant_security_config on the Authentication
plugin and the operation register_local_participant on the Cryptographic
plugin."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — das Gate holt die Konfiguration beim Anlegen und reicht sie an Authentication und Krypto (`algorithm_info`, `ac_participant_properties`).

### §9.4.2.9 Table 39 — get_topic_security_config

**Spec:** §9.4.2.9.23, S. 144 — "Retrieves the TopicSecurityConfig, which
describes how the DDS middleware should enforce the security and integrity
of the information related to the DDS Topic. This operation shall be called
by the DDS middleware as part of the creation or enabling of a DDS Topic."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security-permissions/src/psk_access.rs::tests::the_topic_and_endpoint_configs_are_all_false`

**Status:** `done` — das Plugin liefert die vier Werte aus der Topic Rule.

### §9.4.2.9 Table 39 — get_datawriter_security_config

**Spec:** §9.4.2.9.24, S. 144–145 — "Retrieves the EndpointSecurityConfig,
which describes how the DDS middleware should enforce the security and
integrity of the information related to the DDS DataWriter endpoint. [...]
The operation shall be called after calling check_create_datawriter."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security-permissions/src/psk_access.rs::tests::the_topic_and_endpoint_configs_are_all_false`

**Status:** `done` — nach `check_create_datawriter`; die Maske des Endpunkts entsteht aus ihr.

### §9.4.2.9 Table 39 — get_datareader_security_config

**Spec:** §9.4.2.9.25, S. 145 — "Retrieves the EndpointSecurityConfig, which
describes how the DDS middleware should enforce the security and integrity
of the information related to the DDS DataReader endpoint."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security-permissions/src/psk_access.rs::tests::the_topic_and_endpoint_configs_are_all_false`

**Status:** `done` — wie `get_datawriter_security_config`.

### §9.4.2.9 Table 39 — return_participant_security_config

**Spec:** §9.4.2.9.26, S. 145–146 — "Returns the ParticipantSecurityConfig
to the plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — das Plugin nimmt nur ausgegebene Konfigurationen zurück.

### §9.4.2.9 Table 39 — return_topic_security_config

**Spec:** §9.4.2.9.27, S. 146 — "Returns the TopicSecurityConfig to the
plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — das Plugin nimmt nur ausgegebene Konfigurationen zurück.

### §9.4.2.9 Table 39 — return_datawriter_security_config

**Spec:** §9.4.2.9.28, S. 146 — "Returns the EndpointSecurityConfig to the
plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — das Plugin nimmt nur ausgegebene Konfigurationen zurück.

### §9.4.2.9 Table 39 — return_datareader_security_config

**Spec:** §9.4.2.9.29, S. 146 — "Returns the EndpointSecurityConfig to the
plugin for disposal."

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`, `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`, `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`,
`crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — das Plugin nimmt nur ausgegebene Konfigurationen zurück.

### §9.4.2.9 Eigene Operationen außerhalb von Table 39

**Spec:** §9.4.2.9, Table 39, S. 129–134 — die Tabelle zählt die
Operationen der Schnittstelle auf.

**Repo:** `crates/security/src/access_control.rs::AccessControlPlugin`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::plugin_class_id_matches_spec`,
`crates/security-runtime/src/shared.rs::tests::the_builtin_plugin_is_not_asked_for_what_it_always_permits`

**Status:** `done` — die Schnittstelle hat die 29 Operationen der Table 39 und zusätzlich `check_relay` (§7.1.1.4, Vorgabe: verweigern) und `plugin_class_id`. Die früheren Eigenbauten `check_*_in_partitions` und `check_remote_*_match` sind durch die Parameter der Spec ersetzt.

### §9.4.2.10 Table 40 — AccessControlListener::on_revoke_permissions

**Spec:** §9.4.2.10.1, S. 146 — "DomainParticipants' Permissions can be
revoked/changed. This listener provides a callback for permission
revocation/changes."

**Repo:** `crates/security/src/access_control.rs::AccessControlListener`; `crates/security-runtime/src/shared.rs` (entfernt den Handle und gleicht neu ab)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::revoking_permissions_tells_the_listener`,
`crates/security-runtime/src/shared.rs::tests::a_revoked_remote_handle_is_handed_to_the_runtime_once`

**Status:** `done` — das Plugin meldet widerrufene Berechtigungen; das Gate übergibt den Handle einmal an die Laufzeit.

---

### §9.5 Cryptographic Plugin

**Spec:** §9.5, S. 147 — "The Cryptographic plugin defines the types and
operations necessary to support encryption, digest, message authentication
codes, and key exchange for DDS DomainParticipants, DataWriters and DDS
DataReaders."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §9.5.1 Cryptographic Plugin Model

**Spec:** §9.5.1, Figure 11, S. 147 — Schnittstellen CryptoKeyFactory,
CryptoKeyExchange, CryptoTransform; Typen ParticipantCryptoHandle,
DatawriterCryptoHandle, DatareaderCryptoHandle, CryptoToken,
CryptoTransformIdentifier.

**Repo:** `crates/security/src/crypto.rs::CryptographicPlugin` (Z. 59, eine
Schnittstelle für alle drei)

**Tests:** —

**Status:** `n/a (informative)` — Diagramm; die Operationen sind unten
einzeln geführt.

### §9.5.1.1 CryptoToken

**Spec:** §9.5.1.1, S. 148 — "This class represents a generic holder for key
material. [...] Each plugin implementation shall fully define itself, so that
applications are able to interoperate."

**Repo:** `crates/security/src/token.rs` (Alias `CryptoToken`, Z. 247–268);
`crates/dcps/src/runtime.rs::build_crypto_token_message` (Z. 21338)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cross_plugin_interop_via_tokens`,
`crates/security-crypto/src/plugin.rs::tests::tokens_carry_suite_tag_so_cross_suite_interop_works`

**Status:** done — der Inhalt des eingebauten Plugins steht in §10.5.

### §9.5.1.2 ParticipantCryptoHandle

**Spec:** §9.5.1.2, S. 148 — "The ParticipantCryptoHandle object is an
opaque local reference that represents the key material used to encrypt and
sign whole RTPS Messages."

**Repo:** `crates/security/src/crypto.rs::CryptoHandle` (Z. 29);
`crates/security-runtime/src/shared.rs::ensure_local` (Z. 637)

**Tests:** `crates/security-runtime/src/shared.rs::tests::e2e_alice_bob_with_shared_gate`

**Status:** done — ein Typ `CryptoHandle` für Participant-, Writer- und
Reader-Handles.

### §9.5.1.3 DatawriterCryptoHandle

**Spec:** §9.5.1.3, S. 148 — "The DatawriterCryptoHandle object is an opaque
local reference that represents the key material used to encrypt and sign
RTPS submessages sent from a DataWriter. This includes the RTPS submessages
Data, DataFrag, Gap, Heartbeat, and HeartbeatFrag, as well as, the
SerializedPayload submessage element".

**Repo:** `crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`
(Z. 21403); `crates/security-runtime/src/shared.rs::register_local_endpoint`
(Z. 904)

**Tests:** `crates/security-runtime/src/shared.rs::tests::per_endpoint_datawriter_token_roundtrips_by_key_id`

**Status:** done

### §9.5.1.4 DatareaderCryptoHandle

**Spec:** §9.5.1.4, S. 148 — "The DatareaderCryptoHandle object is an opaque
local reference that represents the key material used to encrypt and sign
RTPS Submessages sent from a DataReader. This includes the RTPS Submessages
AckNack and NackFrag."

**Repo:** `crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`
(Z. 21403, `is_writer = false`); Auswahl der Submessages des Readers
`SMID_ACKNACK` (Z. 21716)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** done

### §9.5.1.5 CryptoTransformIdentifier

**Spec:** §9.5.1.5, S. 148 — "The CryptoTransformIdentifier object used to
uniquely identify the transformation applied on the sending side [...] The
definition and interpretation is provided in 7.3.20."

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoTransformIdentifier`
(Z. 99)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::transform_identifier_round_trip`

**Status:** done — Einzelheiten in §7.3.20.

### §9.5.1.6 Key Revision: gleiche key_id, gleicher Algorithmus, höhere Revision

**Spec:** §9.5.1.6, S. 148–149 — "The CryptoTransformIdentifier for the
'revised' key material shall have: The same value of the
transformation_key_id. The same value for the transformation_kind's member
transformation_algorithm_id. An updated (incremented) value for the
transformation_kind's member transformation_key_revision."

**Repo:** `crates/security-crypto/src/plugin/spec.rs`::revise_local_entity_keys

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_revised_with_the_entities`

**Status:** `done` — Eine Revision behält `transformation_key_id` und Algorithmus-Id, erhöht die Revision im CryptoHeader; der Empfänger hält die ersetzte und die davor ersetzte Revision. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.7 Table 41 — SecureSubmessageCategory_t

**Spec:** §9.5.1.7, Table 41, S. 149 — INFO_SUBMESSAGE,
DATAWRITER_SUMBESSAGE, DATAREADER_SUMBESSAGE.

**Repo:** `crates/security/src/cryptographic.rs`::SecureSubmessageCategory

**Tests:** `crates/security/src/cryptographic.rs::tests::the_three_categories_of_table_41_are_distinct`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Die drei Kategorien; `preprocess_secure_submsg` meldet Writer- und Reader-Submessage. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_local_participant

**Spec:** §9.5.1.8.1, S. 151–152 — "Registers a local DomainParticipant with
the Cryptographic Plugin. [...] Configures the cryptographic algorithms that
may be used by the Cryptographic plugin and retrieves an updated
ParticipantSecurityAlgorithmInfo". Parameter: out `adjusted_algorithm_info`,
`participant_identity`, `participant_permissions`, `participant_properties`,
`participant_security_config`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::register_local_participant_fills_the_adjusted_algorithm_info`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_gcm_or_gmac_by_the_protection_kind_and_width`, `crates/security-runtime/src/shared.rs::tests::e2e_alice_bob_with_shared_gate`

**Status:** `done` — Handle, `adjusted_algorithm_info`, Identität, Berechtigung, Properties und `ParticipantSecurityConfig` sind Parameter; das Gate ruft die Operation (`SharedSecurityGate::ensure_local`). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` und `-p zerodds-dcps --features security --lib` (7 Tests), Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §9.5.1.8.1 register_local_participant: Parameter

**Spec:** §9.5.1.8.1, S. 152 — "Parameter participant_properties: This
parameter shall contain all the properties in the PropertyQosPolicy of the
local DomainParticipant whose name has the prefix 'dds.sec.crypto.' [...]
Parameter participant_security_config: The ParticipantSecurityConfig returned
by the AccessControl get_participant_security_config operation."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::register_local_participant_fills_the_adjusted_algorithm_info`, `crates/security-crypto/src/plugin/spec.rs::tests::the_plugin_accepts_only_the_supported_algorithms`

**Status:** `done` — Nil-Identität oder -Berechtigung ergibt einen Fehler; unbekannte Properties werden ignoriert; nur die Algorithmen aus `supported_mask` werden angenommen. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_matched_remote_participant

**Spec:** §9.5.1.8.2, S. 152–153 — "Registers a remote DomainParticipant with
the Cryptographic Plugin. [...] 1. It shall create any necessary key material
needed to decrypt and verify the signatures of messages received from that
remote DomainParticipant [...] 2. It shall create any necessary key material
that will be used by the local DomainParticipant when encrypting or signing
messages that are intended only for that remote DomainParticipant."
Parameter `local_participant_crypto_handle`, `remote_participant_identity`,
`remote_participant_permissions`, `shared_secret`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_participant2participant_material_matches_the_participant_key`, `crates/security-runtime/src/shared.rs::tests::kx_channel_round_trips_through_gate`

**Status:** `done` — `remote_participant_permissions` ist Parameter; bei Origin Authentication ein Schlüssel je entferntem Participant. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_local_datawriter

**Spec:** §9.5.1.8.3, S. 153–154 — "Registers a local DataWriter with the
Cryptographic Plugin. [...] This operation shall create the cryptographic
material necessary to encrypt and/or sign the data written by the
DataWriter". Parameter: out `adjusted_algorithm_info`,
`participant_crypto`, `datawriter_properties`, `datawriter_security_config`,
`endpoint_guid`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/dcps/src/runtime.rs::tests::key_material_exists_for_protected_endpoints_only_and_goes_with_them`

**Status:** `done` — `adjusted_algorithm_info`, Config, `endpoint_guid` und Properties sind Parameter. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` und `-p zerodds-dcps --features security --lib` (7 Tests), Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §9.5.1.8 Table 42 — register_local_datareader

**Spec:** §9.5.1.8.5, S. 154–155 — "Registers a local DataReader with the
Cryptographic Plugin. [...] This operation shall create the cryptographic
material necessary to encrypt and/or sign the messages sent by the
DataReader when the encryption/signature is independent of the targeted
DataWriter."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — Wie register_local_datawriter, für Reader. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Fehlende Parameter von CryptoKeyFactory

**Spec:** §9.5.1.8.2, §9.5.1.8.3, §9.5.1.8.5, S. 152–155 — Parameter
`remote_participant_permissions`, `adjusted_algorithm_info`,
`datawriter_security_config`, `datareader_security_config`,
`endpoint_guid`; "Parameter local_datawriter_properties: This parameter shall
contain all the properties in the PropertyQosPolicy of the local DataWriter
whose name has the prefix 'dds.sec.crypto.'"

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/security-crypto/src/plugin/spec.rs::tests::the_plugin_accepts_only_the_supported_algorithms`

**Status:** `done` — Alle Parameter der Table 42 sind in `CryptoKeyFactory`; `endpoint_guid` bestimmt die Volatile-Endpunkte (EntityId 0xff0202c3/c4). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_matched_remote_datareader

**Spec:** §9.5.1.8.4, S. 154 — "Registers a remote DataReader with the
Cryptographic Plugin. [...] The operation shall associate the value of the
relay_only parameter with the returned DatawriterCryptoHandle. This
information shall be used in the generation of the KeyToken objects to be
sent to the DataReader."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_relay_only_reader_gets_the_submessage_key_alone`, `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — `relay_only` wird mit dem Handle verbunden und bestimmt die Token des Writers. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — register_matched_remote_datawriter

**Spec:** §9.5.1.8.6, S. 155–156 — "Registers a remote DataWriter with the
Cryptographic Plugin. [...] This operation shall create the cryptographic
material necessary to decrypt and/or verify the signatures of the RTPS
submessages (Data, DataFrag, Heartbeat, HeartbeatFrag, Gap) sent from the
remote DataWriter to the DataReader."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_volatile_endpoints_use_the_key_of_the_shared_secret`, `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — Material des entfernten Writers; für die Volatile-Endpunkte der Kx-Schlüssel aus dem SharedSecret. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — revise_local_entity_keys

**Spec:** §9.5.1.8.7, S. 156 — "Creates a revision of the KeyMaterial used by
the local DomainParticipant and its contained DataReader and DataWriter
entities. [...] Returning a key_revision value of '0' indicates the Plugin
does not support Key Revisions".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::the_volatile_endpoints_are_not_revised`

**Status:** `done` — Erzeugt die Revision für Participant und Endpunkte, nicht für die Volatile-Endpunkte; Gate: `SharedSecurityGate::revise_local_entity_keys`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — activate_key_revision

**Spec:** §9.5.1.8.8, S. 156–157 — "Configures the plugin to start using the
KeyMaterial that corresponds to a Key Revision created by a previous call to
the operation revise_local_entity_keys."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_revised_with_the_entities`

**Status:** `done` — Danach kodiert das Plugin mit der Revision; Gate: `SharedSecurityGate::activate_key_revision`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — unregister_participant

**Spec:** §9.5.1.8.9, S. 157 — "Releases the resources, associated with a
DomainParticipant that the Cryptographic plugin maintains. [...]
Specifically, it shall be called when the application deletes a local
DomainParticipant and also when the DDS Discovery mechanism detects that a
matched DomainParticipant is no longer in the system."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::unregistering_releases_the_key_material_and_the_matches`, `crates/security-runtime/src/shared.rs::tests::forget_remote_removes_mapping`

**Status:** `done` — Gibt lokalen und entfernten Participant mit Endpunkten frei; Gate: `forget_remote`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.8 Table 42 — unregister_datawriter

**Spec:** §9.5.1.8.10, S. 157–158 — "Releases the resources, associated with
a DataWriter that the Cryptographic plugin maintains."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::unregistering_releases_the_key_material_and_the_matches`, `crates/dcps/src/runtime.rs::tests::key_material_exists_for_protected_endpoints_only_and_goes_with_them`

**Status:** `done` — Gibt Material des Writers und der Matches frei; Gate: `unregister_local_endpoint`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` und `-p zerodds-dcps --features security --lib` (7 Tests), Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §9.5.1.8 Table 42 — unregister_datareader

**Spec:** §9.5.1.8.11, S. 158 — "Releases the resources, associated with a
DataReader, that the Cryptographic plugin maintains."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::unregistering_releases_the_key_material_and_the_matches`

**Status:** `done` — Wie unregister_datawriter, für Reader. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — create_local_participant_crypto_tokens

**Spec:** §9.5.1.9.1, S. 160–161 — "This operation creates a sequence of
CryptoToken tokens containing the information needed to correctly interpret
cipher text encoded using the local_participant_crypto. [...] The returned
ParticipantCryptoTokenSeq shall be sent to the remote DomainParticipant using
the BuiltinParticipantVolatileMessageSecureWriter with kind set to
GMCLASSID_SECURITY_PARTICIPANT_CRYPTO_TOKENS". Parameter `key_revision`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_participant2participant_material_matches_the_participant_key`, `crates/security-runtime/src/shared.rs::tests::data_round_trips_via_token_after_kx_register`

**Status:** `done` — `key_revision` ist Parameter; Gate: `local_token_for`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — set_remote_participant_crypto_tokens

**Spec:** §9.5.1.9.2, S. 161–162 — "This operation shall be called by the DDS
implementation upon reception of a message on the
BuiltinParticipantVolatileMessageSecureReader with kind set to
GMCLASSID_SECURITY_PARTICIPANT_CRYPTO_TOKENS".

**Repo:** `crates/security/src/crypto.rs::set_remote_participant_crypto_tokens`
(Z. 107); `crates/security-runtime/src/shared.rs::set_remote_data_token_by_guid`
(Z. 762); `crates/dcps/src/runtime.rs::on_volatile_messages` (Z. 22563)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cross_plugin_interop_via_tokens`,
`crates/security-crypto/src/plugin.rs::tests::data_token_exchange_does_not_clobber_kx_key`

**Status:** done

### §9.5.1.9 Table 43 — create_local_datawriter_crypto_tokens

**Spec:** §9.5.1.9.3, S. 162–163 — "This operation creates a
DatawriterCryptoTokenSeq [...] The returned CryptoToken sequence shall be
sent by the DDS middleware to the remote DataReader using the
BuiltinParticipantVolatileMessageSecureWriter with kind set to
GMCLASSID_SECURITY_DATAWRITER_CRYPTO_TOKENS [...] The source_endpoint_guid
attribute shall be set to the GUID_t of the local DataWriter and the
destination_endpoint_guid attribute shall be set to the GUID_t of the remote
DataReader."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_relay_only_reader_gets_the_submessage_key_alone`, `crates/security-runtime/src/shared.rs::tests::per_endpoint_datawriter_token_roundtrips_by_key_id`

**Status:** `done` — Eigene Operation mit `remote_datareader_crypto`; für relay-only nur das Submessage-Material. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — set_remote_datawriter_crypto_tokens

**Spec:** §9.5.1.9.4, S. 163 — "This operation shall be called by the DDS
implementation upon reception of a message on the
BuiltinParticipantVolatileMessageSecureReader with kind set to
GMCLASSID_SECURITY_DATAWRITER_CRYPTO_TOKENS".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-runtime/src/shared.rs::tests::per_endpoint_datawriter_token_roundtrips_by_key_id`

**Status:** `done` — Eigene Operation mit `local_datareader_crypto` und `remote_datawriter_crypto`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — create_local_datareader_crypto_tokens

**Spec:** §9.5.1.9.5, S. 164 — "The returned DatareaderCryptoTokenSeq shall
be sent by the DDS middleware to the remote DataWriter using the
BuiltinParticipantVolatileMessageSecureWriter with kind set to
GMCLASSID_SECURITY_DATAREADER_CRYPTO_TOKENS".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Eigene Operation für Reader. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — set_remote_datareader_crypto_tokens

**Spec:** §9.5.1.9.6, S. 164–165 — "This operation shall be called by the DDS
implementation upon reception of a message on the
BuiltinParticipantVolatileMessageSecureReader with kind set to
GMCLASSID_SECURITY_DATAREADER_CRYPTO_TOKENS".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Eigene Operation für Reader. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Eigene Operationen der Endpunkte in CryptoKeyExchange

**Spec:** §9.5.1.9, Table 43, S. 159–160 — create_local_datawriter_crypto_tokens
(`local_datawriter_crypto`, `remote_datareader_crypto`),
set_remote_datawriter_crypto_tokens (`local_datareader_crypto`,
`remote_datawriter_crypto`), create_local_datareader_crypto_tokens,
set_remote_datareader_crypto_tokens.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-crypto/src/plugin/spec.rs::tests::the_volatile_endpoints_use_the_key_of_the_shared_secret`

**Status:** `done` — Die vier Operationen stehen in `CryptoKeyExchange`; das Gate ruft sie (`set_endpoint_token`, `endpoint_token`). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 key_revision der Operationen create_local_*_crypto_tokens

**Spec:** §9.5.1.9.1, §9.5.1.9.3, §9.5.1.9.5, S. 161–164 — "Parameter
key_revision: An integer that selects the revision of the Key Material that
is encoded into the returned CryptoTokenSeq."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`

**Status:** `done` — Eine Revision, die `revise_local_entity_keys` nicht lieferte, ergibt einen Fehler. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.9 Table 43 — return_crypto_tokens

**Spec:** §9.5.1.9.7, S. 165 — "Returns the tokens in the CryptoToken
sequence to the plugin so the plugin can release any information associated
with it."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::crypto_tokens_are_returned_once`

**Status:** `done` — Nimmt ausgegebene Token einmal zurück. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 CryptoTransform: Ersetzen der Bytes durch die Middleware

**Spec:** §9.5.1.10, S. 165–166 — "The DDS implementation is only responsible
for calling the operations in the CryptoTransform plugin at the appropriate
times as it generates and processes the RTPS messages, substitutes the input
bytes with the transformed bytes produced by the CryptoTransform operations".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung der Arbeitsteilung; die
Operationen sind einzeln geführt.

### §9.5.1.10 Table 44 — encode_serialized_payload

**Spec:** §9.5.1.10.1, S. 168–169 — "The operation receives the data written
by the DataWriter in serialized form wrapped inside the RTPS
SerializedPayload submessage element and shall output an RTPS CryptoContent
submessage element and a extra_inline_qos containing InlineQos formatted as a
ParameterList [...] The DDS implementation shall call this operation for all
outgoing RTPS Submessages with submessage kind Data and DataFrag."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_serialized_payload_goes_through_the_interface`, `crates/security-runtime/src/shared.rs::tests::e2e_alice_bob_with_shared_gate`

**Status:** `done` — Ausgabe `EncodedPayload` mit `extra_inline_qos` (leer: die Kennung steht im CryptoHeader). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` und `-p zerodds-dcps --features security --lib` (7 Tests), Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §9.5.1.10 Table 44 — encode_datawriter_submessage

**Spec:** §9.5.1.10.2, S. 169–171 — "This operation shall be called by the
DDS implementation whenever it has constructed an RTPS submessage of kind
Data, DataFrag, Gap, Heartbeat, or HeartbeatFrag. [...] In the case of
BuiltinParticipantVolatileMessageSecureWriter [...] the
DatareaderCryptoHandle list has ONE element containing KxKey material
derived from the SharedSecret".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_receiver_specific_macs_follow_the_list_index`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Liste der Empfänger mit Index; MACs je Empfänger. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — encode_datareader_submessage

**Spec:** §9.5.1.10.3, S. 171–172 — "This operation shall be called by the
DDS implementation whenever it has constructed an RTPS submessage of kind
AckNack or NackFrag."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Liste der Writer als Parameter. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — encode_rtps_message

**Spec:** §9.5.1.10.4, S. 172–175 — "This operation shall be called by the
DDS implementation whenever it has constructed an RTPS message prior to
sending it on the wire. [...] This operation may optionally not perform any
transformation of the input RTPS message. In this case, the operation shall
return false but not set the exception object." Parameter
`receiving_participant_crypto_list`, `receiving_participant_crypto_list_index`,
`transform_with_psk`.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::an_rtps_message_goes_through_the_interface_without_aad`, `crates/security-crypto/src/plugin/spec.rs::tests::transform_with_psk_uses_the_pre_shared_key`

**Status:** `done` — Empfängerliste, Index, `transform_with_psk`; FALSE ohne Ausnahme, wenn nicht transformiert wird. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — decode_rtps_message

**Spec:** §9.5.1.10.5, S. 175–176 — "This operation shall be called by the
DDS implementation whenever it receives an RTPS message prior to parsing it.
This operation shall reverse the transformation performed by the
encode_rtps_message operation".

**Repo:** `crates/security/src/crypto.rs::decode_rtps_message_cyclone`
(Z. 372); `crates/security-runtime/src/shared.rs::transform_inbound_from`
(Z. 1131), `classify_inbound` (Z. 1348)

**Tests:** `crates/security-runtime/src/shared.rs::tests::classify_inbound_srtps_from_known_peer_accepts`,
`crates/security-runtime/src/shared.rs::tests::classify_inbound_srtps_with_wrong_key_is_crypto_error`

**Status:** done — `receiving_participant_crypto` und
`sending_participant_crypto` sind keine Parameter; der Schlüssel wird über
`transformation_key_id` und den GuidPrefix gefunden.

### §9.5.1.10 Table 44 — preprocess_secure_submsg

**Spec:** §9.5.1.10.6, S. 176–178 — "The purpose of the operation is to
determine whether the secure submessage was produced as a result of a call to
encode_datawriter_submessage or a call to encode_datareader_submessage, and
to retrieve the appropriate DatawriterCryptoHandle and
DatareaderCryptoHandle needed to decode the submessage."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-crypto/src/plugin/spec.rs::tests::preprocess_refuses_unknown_kinds_and_unknown_keys`

**Status:** `done` — Kategorie und Handles; das Gate dekodiert damit. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — decode_datawriter_submessage

**Spec:** §9.5.1.10.7, S. 178–179 — "This operation shall reverse the
transformation performed by the encode_datawriter_submessage operation,
decrypting the content if appropriate and verifying any MACs or digital
signatures that were produced by the encode_datawriter_submessage
operation."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_is_taken_only_from_the_entities_it_is_associated_with`

**Status:** `done` — Mit den Handles aus `preprocess_secure_submsg`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — decode_datareader_submessage

**Spec:** §9.5.1.10.8, S. 179–180 — "This operation shall reverse the
transformation performed by the encode_datareader_submessage operation".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Wie für Writer. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Table 44 — decode_serialized_payload

**Spec:** §9.5.1.10.9, S. 180–181 — "The operation shall receive in the
inline_qos parameter the InlineQos RTPS SubmessageElement that appeared in
the RTPS Data submessage that carried the SerializedPayload. [...] it shall
use the CryptoTransformIdentifier present in the CryptoContent to verify that
the correct key is available".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_serialized_payload_goes_through_the_interface`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_is_taken_only_from_the_entities_it_is_associated_with`

**Status:** `done` — Mit `inline_qos` und den Handles; Schlüssel nur des Senders. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Parameter der Operationen von CryptoTransform

**Spec:** §9.5.1.10, Table 44, S. 166–168 — `extra_inline_qos`,
`receiving_datareader_crypto_list`, `receiving_datareader_crypto_list_index`,
`receiving_datawriter_crypto_list`, `receiving_participant_crypto_list`,
`receiving_participant_crypto_list_index`, `transform_with_psk`,
`inline_qos`, die Handles von Sender und Empfänger bei den Operationen
decode_*.

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_receiver_specific_macs_follow_the_list_index`, `crates/security-crypto/src/plugin/spec.rs::tests::the_serialized_payload_goes_through_the_interface`

**Status:** `done` — Alle Parameter der Table 44 sind in `CryptoTransform`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §9.5.1.10 Eigene Operationen außerhalb von Tables 42 bis 44

**Spec:** §9.5.1.8 bis §9.5.1.10, Tables 42–44, S. 149–168 — die Tabellen
zählen die Operationen der drei Schnittstellen auf.

**Repo:** `crates/security/src/cryptographic.rs`; `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`; `crates/security/src/cryptographic.rs::VendorFormCrypto`

**Tests:** `crates/security-runtime/src/shared.rs::tests::data_round_trips_via_token_after_kx_register`, `crates/security-runtime/src/shared.rs::tests::two_peers_with_the_same_key_id_keep_their_keys`, `crates/dcps/src/runtime.rs::tests::a_user_datagram_under_the_kx_key_is_not_opened`

**Status:** `done` — Gate und Laufzeit rufen nur noch die Operationen der Tables 42 bis 44 (`gate_crypto.rs`). Was ein Hersteller ohne Spec-Grundlage braucht (Teilnehmer-Schlüssel für Submessages und Nutzlast, Kx-Schlüssel für Nutzlast), steht in `VendorFormCrypto` und wird nur mit `org.zerodds.sec.kx_key_fallback=accept` oder für Endpunkte ohne eigenen Schlüssel benutzt. Der ältere Trait `CryptographicPlugin` bleibt als Implementierungsdetail von `security-rtps` und `PskDerivedCryptoPlugin` bestehen; die Middleware ruft ihn nicht. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` und `-p zerodds-dcps --features security --lib` (7 Tests), Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

---

### §9.6 The Logging Plugin

**Spec:** §9.6, S. 181 — "The Logging Control Plugin API defines the types
and operations necessary to support logging of security events for a DDS
DomainParticipant."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §9.6.1 Background (Non-Normative)

**Spec:** §9.6.1, S. 181 — "The Logging plugin will add an ID to the log
message that uniquely specifies the DomainParticipant. It will also add a
time-stamp to each log message."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hintergrund; die normativen Regeln stehen
in §9.6.2.

### §9.6.2 Logging Plugin Model

**Spec:** §9.6.2, Figure 21, S. 182 — Schnittstellen Logging und
LoggerListener, Typen LogOptions, NameValuePair, BuiltinLoggingType.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Diagramm; BuiltinLoggingType steht in
§10.6.

### §9.6.2.1 LogOptions vor Beginn des Loggens, danach unveränderlich

**Spec:** §9.6.2.1, S. 182 — "The options must be set before logging starts
and may not be changed at run-time after logging has commenced."

**Repo:** `crates/security-logging/src/log_topic.rs::set_log_options`
(Z. 624, `AlreadyEnabled` nach `enable_logging`)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::options_cannot_change_after_logging_is_enabled`

**Status:** done — für das eingebaute Plugin `LogTopicLoggingPlugin`.

### §9.6.2.1 Table 45 — LogOptions: log_level, log_file, distribute

**Spec:** §9.6.2.1, Table 45, S. 182 — `log_level Long`, `log_file String`,
`distribute Boolean`.

**Repo:** `crates/security-logging/src/log_topic.rs::LogOptions` (Z. 101)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::distribute_off_publishes_nothing`

**Status:** done — `log_file` ist `Option<PathBuf>`, `log_level` ein
`LogLevel`.

### §9.6.2.1.1 log_level: Stufen FATAL bis TRACE

**Spec:** §9.6.2.1.1, S. 183 — "Messages at or below the log_level are
logged. The levels are as follows, from low to high: FATAL_LEVEL [...]
SEVERE_LEVEL [...] ERROR_LEVEL [...] WARNING_LEVEL [...] NOTICE_LEVEL [...]
INFO_LEVEL [...] DEBUG_LEVEL [...] TRACE_LEVEL".

**Repo:** `crates/security/src/logging.rs::LogLevel`, `to_spec_level`, `from_spec_level`; `crates/security-logging/src/control.rs::LogControl::admit`

**Tests:** `crates/security/src/logging.rs::tests::the_spec_level_names_are_numbered_low_to_high`,
`crates/security-logging/src/control.rs::tests::the_spec_levels_filter_from_fatal_to_trace`

**Status:** `done` — acht Stufen `FATAL_LEVEL` bis `TRACE_LEVEL` mit den Nummern 0 bis 7; eine Meldung auf oder unter der gespeicherten Stufe wird geloggt.
### §9.6.2.1.1 Namen FATAL_LEVEL bis TRACE_LEVEL

**Spec:** §9.6.2.1.1, S. 183 — FATAL_LEVEL, SEVERE_LEVEL, ERROR_LEVEL,
WARNING_LEVEL, NOTICE_LEVEL, INFO_LEVEL, DEBUG_LEVEL, TRACE_LEVEL.

**Repo:** `crates/security/src/logging.rs` (`FATAL_LEVEL` bis `TRACE_LEVEL`, `LogLevel::spec_name`, `LogLevel::Trace`)

**Tests:** `crates/security/src/logging.rs::tests::the_spec_level_names_are_numbered_low_to_high`

**Status:** `done` — `LogLevel` trägt `Trace`; `Alert` und `Critical` sind beide `SEVERE_LEVEL`. Die Umwandlung in `LoggingLevel` (§10.6) bildet `Trace` auf `Debug` ab, weil die IDL-Aufzählung keinen Wert dafür hat.
### §9.6.2.1.2 log_file: anhängen, NULL schreibt keine Datei

**Spec:** §9.6.2.1.2, S. 183 — "Specifies the full path to a local file for
logging events. If the file already exists, the logger will append log
messages to the file. If it is NULL, then the logger will not log messages
to a file."

**Repo:** `crates/security-logging/src/control.rs::open_log_file`

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::an_unopenable_log_file_is_refused`

**Status:** done

### §9.6.2.1.3 distribute: Veröffentlichung als DDS Topic

**Spec:** §9.6.2.1.3, S. 183 — "Specifies whether the log events should be
distributed over DDS. If it is TRUE, each log message at or above the
log_level is published as a DDS Topic."

**Repo:** `crates/security-logging/src/log_topic.rs::LogTopicLoggingPlugin`
(Z. 554)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::distribute_off_publishes_nothing`,
`crates/dcps/tests/security_log_topic_e2e.rs::a_security_event_reaches_the_v2_log_topic`

**Status:** done — veröffentlicht werden Meldungen auf oder unter der Stufe
wie beim Filter in §9.6.2.1.1; der Text von §9.6.2.1.3 sagt „at or above“
(Widerspruch in der Spec).

### §9.6.2.2 Table 46 — set_log_options

**Spec:** §9.6.2.2.1, S. 184 — "Sets the options for the logger. This must
be called before enable_logging; it is an error to set the options after
logging has been enabled."

**Repo:** `crates/security/src/logging.rs::LoggingPlugin::set_log_options`; `crates/security-logging/src/control.rs::LogControl::set_log_options`

**Tests:** `crates/security-logging/src/control.rs::tests::a_sink_without_a_writer_refuses_distribute`,
`crates/security-logging/src/control.rs::tests::a_sink_enabled_at_construction_refuses_new_options`,
`crates/security-logging/src/log_topic.rs::tests::options_cannot_change_after_logging_is_enabled`

**Status:** `done` — an der Schnittstelle und an allen Senken; nach `enable_logging` ein `SecurityException`-Fehler, ohne DataWriter wird `distribute` abgelehnt.
### §9.6.2.2 Table 46 — log

**Spec:** §9.6.2.2.2, S. 184 — "Log a message. The logger shall log the
message if its log_level is at or above the level set in the LogOptions. The
Logger shall add to the message the RTPS GUID of the DomainParticipant whose
operations are being logged. The Logger shall populate the facility,
severity, and timestamp, fields." Parameter `log_level`, `message`,
`category`.

**Repo:** `crates/security/src/logging.rs::log` (Z. 56);
`crates/security-logging/src/log_topic.rs::from_log_event` (Z. 407)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::an_event_fills_every_field_from_the_clause`,
`crates/security/src/mock.rs::tests::mock_logging_captures_events`

**Status:** done — die GUID übergibt der Aufrufer als Parameter
`participant`.

### §9.6.2.2.2 Table 47 — structured_data „DDS“

**Spec:** §9.6.2.2.2, Table 47, S. 184 — "The Logger shall add an entry to
the structured_data field with the key 'DDS'. This NameValuePair sequence
shall include the following name-value pairs: guid [...] domain_id [...]
plugin_class [...] plugin_method".

**Repo:** `crates/security-logging/src/log_topic.rs::dds_structured_data`
(Z. 377, `guid`, `plugin_class`, `plugin_method`);
`crates/dcps/src/security_log_topic.rs::add_domain_id` (Z. 270, `domain_id`
beim Schreiben über `LogTopicDataWriterPublisher`)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::an_event_fills_every_field_from_the_clause`,
`crates/dcps/src/security_log_topic.rs::tests::the_publisher_inserts_the_domain_id_pair`,
`crates/dcps/tests/security_log_topic_e2e.rs::a_security_event_reaches_the_v2_log_topic`

**Status:** done — die vier Paare stehen in jedem veröffentlichten Sample.
`LoggingPlugin::log` erhält keine Domain-ID; `domain_id` setzt erst
`LogTopicDataWriterPublisher` in `zerodds-dcps`. Die Zeile in `log_file`
enthält keine structured_data.

### §9.6.2.2 Table 46 — enable_logging

**Spec:** §9.6.2.2.3, S. 184 — "Enables logging. After this method is
called, any call to log shall log the messages according to the options.
After this method is called, the options may not be modified."

**Repo:** `crates/security/src/logging.rs::LoggingPlugin::enable_logging`; `crates/security-logging/src/control.rs::LogControl::enable_logging`

**Tests:** `crates/security-logging/src/control.rs::tests::enable_logging_without_options_is_an_error`,
`crates/security-logging/src/log_topic.rs::tests::enable_without_options_is_refused`

**Status:** `done` — an der Schnittstelle und an allen Senken; ohne Optionen ein Fehler.
### §9.6.2.2 Table 46 — set_listener

**Spec:** §9.6.2.2.4, S. 185 — "Sets the LoggerListener that the Logger
plugin will use to notify the application of log events."

**Repo:** `crates/security/src/logging.rs::{LoggerListener, LoggingPlugin::set_listener}`; `crates/security-logging/src/control.rs::LogControl::admit`

**Tests:** `crates/security-logging/src/control.rs::tests::the_listener_hears_each_logged_message`,
`crates/security-logging/src/log_topic.rs::tests::the_listener_is_told_of_each_message_the_sink_logs`

**Status:** `done` — der Listener erfährt jede geloggte Meldung; `None` entfernt ihn.
### §9.6.2.2 Operationen von Table 46 fehlen in LoggingPlugin

**Spec:** §9.6.2.2, Table 46, S. 183 — set_log_options, log, enable_logging,
set_listener.

**Repo:** `crates/security/src/logging.rs::LoggingPlugin`; `LogControl` in allen Senken von `crates/security-logging`

**Tests:** `crates/security-logging/src/control.rs::tests::a_sink_without_a_writer_refuses_distribute`,
`crates/security-logging/src/control.rs::tests::the_listener_hears_each_logged_message`,
`crates/security-logging/src/log_topic.rs::tests::the_listener_is_told_of_each_message_the_sink_logs`

**Status:** `done` — alle vier Operationen stehen an der Schnittstelle; Stderr, JSON-Lines, Syslog, FanOut, `LogTopicLoggingPlugin` und `MockLoggingPlugin` setzen sie um.

---

### §9.7 Data Tagging

**Spec:** §9.7, S. 185 — "Data tagging is the ability to add a security label
or tag to data. [...] It can be used for access control [...] It could be
used for message prioritization."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §9.7.1 Background (Non-Normative)

**Spec:** §9.7.1, S. 185 — "This specification supports DataWriter tagging."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begründung der Wahl.

### §9.7.2 DataTag des DataWriters als unveränderliche DataWriterQos

**Spec:** §9.7.2, S. 185 — "The DataWriter tag will be associated with every
sample written by the DataWriter. The DataWriter DataTag is implemented as an
immutable DataWriterQos."

**Repo:** `crates/dcps/src/qos.rs::DataWriterQos::data_tags`, `immutable_writer_change`

**Tests:** `crates/dcps/src/qos.rs::data_representation_tests::property_is_a_member_and_immutable_after_enable`

**Status:** `done` — Feld `data_tags`, nach dem Aktivieren nicht änderbar (`DATA_TAGS`).
### §9.7.2 DataTag des DataWriters in PublicationBuiltinTopicData

**Spec:** §9.7.2, S. 185 — "The DataWriter DataTag shall be propagated in the
PublicationBuiltinTopicData as part of the DDS discovery protocol."

**Repo:** `crates/dcps/src/builtin_topics.rs::PublicationBuiltinTopicData::data_tags`; `crates/dcps/src/runtime.rs::build_publication_data`

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::the_data_tags_of_an_announcement_reach_the_builtin_topic_samples`,
`crates/dcps/src/runtime.rs::tests::data_tags_are_announced`

**Status:** `done` — `PID_DATA_TAGS` geht in die Ankündigung und erscheint im Builtin-Topic-Sample.
### §9.7.2 DataTag des DataReaders als unveränderliche DataReaderQos

**Spec:** §9.7.2, S. 185 — "The DataReader DataTag is implemented as an
immutable DataReaderQos."

**Repo:** `crates/dcps/src/qos.rs::DataReaderQos::data_tags`, `immutable_reader_change`

**Tests:** `crates/dcps/src/qos.rs::data_representation_tests::property_is_a_member_and_immutable_after_enable`

**Status:** `done` — Feld `data_tags`, nach dem Aktivieren nicht änderbar (`DATA_TAGS`).
### §9.7.2 DataTag des DataReaders in SubscriptionBuiltinTopicData

**Spec:** §9.7.2, S. 185 — "The DataReader DataTag shall be propagated in the
SubscriptionBuiltinTopicData as part of the DDS discovery protocol."

**Repo:** `crates/dcps/src/builtin_topics.rs::SubscriptionBuiltinTopicData::data_tags`; `crates/dcps/src/runtime.rs::build_subscription_data`

**Tests:** `crates/dcps/src/builtin_topics.rs::tests::the_data_tags_of_an_announcement_reach_the_builtin_topic_samples`,
`crates/dcps/src/runtime.rs::tests::data_tags_are_announced`

**Status:** `done` — wie beim DataWriter.

---

### §9.8 Security Plugins Behavior

**Spec:** §9.8, S. 186 — "This sub clause provides additional information on
how the plugins are integrated with the middleware."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §9.8.1 Schritt 1: validate_local_identity vor der Erzeugung

**Spec:** §9.8.1, S. 186–187 — "1. The DDS middleware shall validate the
identity of the application attempting to create the DomainParticipant by
calling the Authentication::validate_local_identity operation, passing the
domain_id, the DomainParticipantQos, and a candidate_participant_guid. [...]
If the identity is not successfully validated, the DDS middleware shall not
create the DomainParticipant and the create_participant operation shall
return NIL and set the return code to NOT_ALLOWED_BY_SECURITY."

**Repo:** `crates/dcps/src/participant.rs::security_from_participant_properties`

**Tests:** `crates/dcps/tests/security_participant_sequence.rs::step_1_an_identity_that_is_not_validated_refuses_the_participant`,
`crates/dcps/tests/security_participant_sequence.rs::the_fixtures_create_a_participant_when_nothing_is_wrong`

**Status:** `done` — eine nicht validierte Identität gibt `NOT_ALLOWED_BY_SECURITY`, es entsteht kein Participant; die Kontrolle ohne Fehler erzeugt einen.
### §9.8.1 Schritt 2: validate_local_permissions vor der Erzeugung

**Spec:** §9.8.1, S. 187 — "2. The DDS middleware shall validate that the DDS
application has the necessary permissions to join DDS domains by calling the
AccessControl::validate_local_permissions operation. [...] If the
permissions are not validated, the DomainParticipant shall not be created,
the create_participant operation shall return NIL and set the return code to
NOT_ALLOWED_BY_SECURITY."

**Repo:** `crates/dcps/src/participant.rs::security_from_participant_properties`

**Tests:** `crates/dcps/tests/security_participant_sequence.rs::step_2_permissions_that_are_not_validated_refuse_the_participant`

**Status:** `done` — Permissions, deren Signatur die Permissions-CA nicht bestätigt, geben `NOT_ALLOWED_BY_SECURITY`.
### §9.8.1 Schritt 3: check_create_participant vor der Erzeugung

**Spec:** §9.8.1, S. 187 — "3. The DDS middleware shall verify that the DDS
application has the necessary permissions to join the specific Domain
identified by the domainId by calling the operation
AccessControl::check_create_participant. If this operation returns FALSE,
the DomainParticipant shall not be created".

**Repo:** `crates/security-runtime/src/qos_properties.rs::security_bundle_from_properties`

**Tests:** `crates/dcps/tests/security_participant_sequence.rs::step_3_a_domain_the_permissions_do_not_grant_refuses_the_participant`

**Status:** `done` — ein Grant für eine andere Domain bei `enable_join_access_control` gibt `NOT_ALLOWED_BY_SECURITY`, die Meldung nennt `check_create_participant`.
### §9.8.1 Schritte 1 bis 3: Rückgabecode NOT_ALLOWED_BY_SECURITY

**Spec:** §9.8.1, S. 187 — "the create_participant operation shall return
NIL and set the return code to NOT_ALLOWED_BY_SECURITY."

**Repo:** `crates/dcps/src/error.rs::DdsError::as_return_code`

**Tests:** `crates/dcps/tests/security_participant_sequence.rs::step_1_an_identity_that_is_not_validated_refuses_the_participant`,
`crates/dcps/tests/security_participant_sequence.rs::step_2_permissions_that_are_not_validated_refuse_the_participant`,
`crates/dcps/tests/security_participant_sequence.rs::step_3_a_domain_the_permissions_do_not_grant_refuses_the_participant`

**Status:** `done` — alle drei Schritte geben `NOT_ALLOWED_BY_SECURITY`; `lookup_participant` findet danach keinen Participant.
### §9.8.1 Schritt 4: IdentityToken in ParticipantBuiltinTopicData

**Spec:** §9.8.1, S. 187 — "4. The DDS middleware shall call the
get_identity_token operation to obtain the IdentityToken object corresponding
to the received IdentityHandle. The IdentityToken object shall be placed in
the ParticipantBuiltinTopicData sent via discovery".

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`
(Z. 8638)

**Tests:** `crates/dcps/src/runtime.rs::tests::spdp_beacon_announces_identity_token_and_discovery_triggers_handshake`

**Status:** done

### §9.8.1 Schritt 5: IdentityStatusToken in ParticipantBuiltinTopicDataSecure

**Spec:** §9.8.1, S. 187 — "5. The DDS middleware shall call the
get_identity_status_token operation [...] If the returned
IdentityStatusToken object is different than TokenNIL, it shall be placed in
the ParticipantBuiltinTopicDataSecure sent via secure discovery".

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`; `crates/discovery/src/spdp.rs::SpdpBeacon::serialize`, `serialize_secure`

**Tests:** `crates/discovery/src/spdp.rs::tests::the_identity_status_token_is_in_the_secure_participant_data_only`,
`crates/dcps/src/runtime.rs::tests::the_tokens_of_the_plugins_are_announced_and_token_nil_is_not`

**Status:** `done` — das Token geht nur auf dem sicheren SPDP-Writer hinaus, nicht auf dem einfachen; TokenNIL wird nicht angekündigt.
### §9.8.1 Schritt 6: PermissionsToken in ParticipantBuiltinTopicData

**Spec:** §9.8.1, S. 187 — "6. The middleware shall call the
get_permissions_token operation on the AccessControl plugin to obtain the
PermissionsToken object corresponding to the received PermissionsHandle. The
PermissionsToken shall be placed in the ParticipantBuiltinTopicData sent via
discovery".

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (der Token kommt aus `SharedSecurityGate::local_permissions_token`, das `get_permissions_token` des AccessControl-Plugins ruft und den Token mit `return_permissions_token` zurückgibt)

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`

**Status:** `done` — das Plugin liefert den PermissionsToken; er steht in den Daten des Participants, und `return_permissions_token` folgt.

### §9.8.1 Schritt 7: get_permissions_credential_token

**Spec:** §9.8.1, S. 187 — "7. The middleware calls the
get_permissions_credential_token operation on the AccessControl plugin,
which returns the PermissionsCredentialToken object corresponding to the
received PermissionsHandle."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`; `crates/security-runtime/src/shared.rs::SharedSecurityGate::local_permissions_credential_token`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`

**Status:** `done` — `get_permissions_credential_token` wird gerufen, der Token danach mit `return_permissions_credential_token` zurückgegeben.

### §9.8.1 Schritt 8: Berechtigung im Handshake

**Spec:** §9.8.1, S. 187 — "8. The middleware calls the
set_permissions_credential_and_token operation on the Authentication plugin
such that it can be sent during the authentication handshake."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (`set_permissions_credential_and_token` auf dem Authentication-Plugin)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`

**Status:** `done` — `set_permissions_credential_and_token` wird mit dem Credential-Token und dem PermissionsToken gerufen, vor `set_participant_security_config`.

### §9.8.1 Schritte 9 und 10: get_/set_participant_security_config

**Spec:** §9.8.1, S. 188 — "9. The middleware calls the
get_participant_security_config operation on the AccessControl plugin [...]
10. The middleware calls the set_participant_security_config operation on
the Authentication plugin passing the ParticipantSecurityConfig".

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::participant_security_config`; `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (`set_participant_security_config`)

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`

**Status:** `done` — `get_participant_security_config` auf dem AccessControl-Plugin, danach `set_participant_security_config` auf dem Authentication-Plugin mit den Algorithmen aus der Konfiguration.

### §9.8.1 Schritt 11: register_local_participant mit ParticipantSecurityConfig

**Spec:** §9.8.1, S. 188 — "11. The middleware calls the
register_local_participant operation on the Cryptographic plugin passing the
ParticipantSecurityConfig returned by the call to
get_participant_security_config".

**Repo:** `crates/security-runtime/src/gate_crypto.rs::CryptoLayer::ensure_local` (`LocalParams::config`, `algorithm_info`); `crates/security-runtime/src/shared.rs::SharedSecurityGate::ensure_local`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_cryptographic_plugin_registers_the_participant_and_the_volatile_endpoints`

**Status:** `done` — `register_local_participant` bekommt die ParticipantSecurityConfig, die `get_participant_security_config` lieferte (der Test prüft `is_rtps_axk_protected` an der aufgezeichneten Operation).

### §9.8.1 Schritt 12: Daten des Participants aus Token und Konfiguration

**Spec:** §9.8.1, S. 188 — "IdentityToken is used to set the identity_token.
PermissionsToken is used to set the permissions_token. The
ParticipantSecurityConfig is used to set the security_info. The two
ParticipantSecurityAlgorithmInfo are combined and used to set the
digital_signature, key_establishment, and symmetric_cipher. [...] The
IdentityStatusToken, is used to set the identity_status_token."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth` (`participant_security_info`, `local_algorithm_infos`); `crates/security-runtime/src/plugin_attributes.rs::participant_security_info`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_participant_data_follows_steps_6_to_12_of_the_creation`,
`crates/dcps/src/runtime.rs::tests::the_tokens_of_the_plugins_are_announced_and_token_nil_is_not`,
`crates/dcps/src/runtime.rs::tests::spdp_announces_secure_bits_when_configured`

**Status:** `done` — `identity_token`, `permissions_token`, `participant_security_info`, die drei Algorithmen (die Antwort von `set_participant_security_config`, eingeschränkt durch `algorithm_info` des AccessControl-Plugins) und `identity_status_token` (nur in den Daten des sicheren Topics) stehen in den Daten des Participants.

### §9.8.2 Prüfung der Kompatibilität der Plugins

**Spec:** §9.8.2, S. 188–189 — "whenever a (local) DomainParticipant
discovers a (remote) DomainParticipant the first step is to check the
information present in the ParticipantBuiltinTopicData of the remote
DomainParticipant to determine security plugin compatibility. The check
examines the following members: identity_token, permissions_token,
security_protection_info, digital_signature, key_establishment,
symmetric_cipher."

**Repo:** `crates/dcps/src/runtime.rs::participant_incompatibility`

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`

**Status:** `done` — `identity_token` und `permissions_token` (PluginClassName und MajorVersion), Schutzmasken und Algorithmus-Infos werden verglichen; bei einer Abweichung wird nicht authentisiert.
### §9.8.3 Authentication behavior with discovered DomainParticipant

**Spec:** §9.8.3, S. 189 — "Depending on the ParticipantSecurityConfig
returned by the AccessControl operation get_participant_security_config the
DomainParticipant may allow remote DomainParticipants that lack the ability
to authenticate [...] to match."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Überleitung zu §9.8.3.1 und §9.8.3.2.

### §9.8.3.1 TRUE: Participants ohne DDS Security werden gematcht

**Spec:** §9.8.3.1, S. 189 — "Discovered DomainParticipant entities that do
not implement the DDS Security specification or do not contain compatible
Security Plugins shall be matched without the DomainParticipant attempting
to authenticate them and shall be treated as 'Unauthenticated'
DomainParticipant entities."

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`, `BuiltinMatch::Regular`

**Tests:** `crates/dcps/src/runtime.rs::tests::unauthenticated_participants_get_the_regular_builtin_endpoints_only`,
`crates/dcps/src/runtime.rs::tests::an_unauthenticated_peer_gets_the_regular_sedp_only`

**Status:** `done` — ein Participant ohne DDS Security erhält die regulären eingebauten Endpunkte, ohne Handshake.
### §9.8.3.1 TRUE: fehlgeschlagene Authentisierung gilt als nicht authentisiert

**Spec:** §9.8.3.1, S. 189 — "Discovered DomainParticipant entities that do
implement the DDS Security specification and declare compatible Security
Plugins but fail the Authentication protocol shall be matched and treated as
'Unauthenticated' DomainParticipants entities."

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`

**Tests:** `crates/dcps/src/runtime.rs::tests::unauthenticated_participants_get_the_regular_builtin_endpoints_only`

**Status:** `done` — ein Participant einer fremden CA besteht den Handshake nicht und bleibt mit den regulären Endpunkten gematcht.
### §9.8.3.1 Nicht authentisierte Participants nur mit regulären eingebauten Endpunkten

**Spec:** §9.8.3.1, S. 189 — "For any matched 'Unauthenticated'
DomainParticipant entities, the DomainParticipant shall match only the
regular builtin Endpoints (ParticipantMessage, DCPSParticipants,
DCPSPublications, DCPSSubscriptions) and not the builtin secure Endpoints".

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`, `handle_spdp_datagram`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_unauthenticated_peer_gets_the_regular_sedp_only`,
`crates/dcps/src/runtime.rs::tests::unauthenticated_participants_get_the_regular_builtin_endpoints_only`

**Status:** `done` — die sicheren Endpunkte werden erst nach der Authentisierung gematcht.
### §9.8.3.1 Authentisierte Participants mit allen eingebauten Endpunkten

**Spec:** §9.8.3.1, S. 189 — "For any matched authenticated
DomainParticipant entities, the DomainParticipant shall match all the
builtin endpoints."

**Repo:** `crates/discovery/src/security/stack.rs::handle_remote_endpoints`
(Z. 236); `crates/discovery/src/sedp/stack.rs::on_participant_discovered`
(Z. 433)

**Tests:** `crates/dcps/src/runtime.rs::tests::c34c_security_builtin_wiring_end_to_end`,
`crates/discovery/src/security/stack.rs::tests::handle_remote_endpoints_with_all_bits_wires_all_four`

**Status:** done

### §9.8.3.2 FALSE: Participants ohne kompatible Plugins abweisen

**Spec:** §9.8.3.2, S. 189 — "Discovered DomainParticipant entities that do
not implement the DDS Security specification or do not contain compatible
Security Plugins shall be rejected without the DomainParticipant attempting
to authenticate them."

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`, `BuiltinMatch::Stateless`

**Tests:** `crates/dcps/src/runtime.rs::tests::unauthenticated_participants_are_rejected_where_they_are_not_allowed`

**Status:** `done` — bei `allow_unauthenticated_participants` FALSE wird nichts außer den Endpunkten für Stateless Messages gematcht.
### §9.8.3.2 FALSE: fehlgeschlagene Authentisierung abweisen

**Spec:** §9.8.3.2, S. 189 — "Discovered DomainParticipant entities that do
implement the DDS Security specification, declare compatible Security
Plugins but fail the Authentication protocol shall be rejected."

**Repo:** `crates/dcps/src/runtime.rs::builtin_match`

**Tests:** `crates/dcps/src/runtime.rs::tests::unauthenticated_participants_are_rejected_where_they_are_not_allowed`

**Status:** `done` — ein Participant einer fremden CA wird nicht gematcht.
### §9.8.3.2 FALSE: Endpunkte für Stateless Messages vor der Authentisierung

**Spec:** §9.8.3.2, S. 189 — "Discovered DomainParticipant entities that do
implement the DDS Security specification and declare compatible Security
Plugins automatically 'match' the ParticipantStatelessMessage builtin
endpoints to allow the authentication handshake to proceed."

**Repo:** `crates/discovery/src/security/stack.rs::handle_remote_endpoints`
(Z. 236)

**Tests:** `crates/discovery/src/security/stack.rs::tests::handle_remote_endpoints_with_only_stateless_bits_skips_volatile`,
`crates/discovery/src/security/stack.rs::tests::end_to_end_stateless_message_loopback_between_stacks`

**Status:** done

### §9.8.3.2 FALSE: übrige eingebaute Endpunkte erst nach der Authentisierung

**Spec:** §9.8.3.2, S. 189–190 — "Discovered DomainParticipant entities that
do implement the DDS Security specification, declare compatible Security
Plugins, and pass the Authentication protocol successfully shall be matched
and the DomainParticipant shall also match all the builtin endpoints of the
discovered DomainParticipant, except for the ParticipantStatelessMessage
builtin endpoints, which were already matched prior to the Authentication
protocol."

**Repo:** `crates/dcps/src/runtime.rs::match_authenticated_builtins`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_sedp_of_a_peer_is_matched_once_it_authenticated`

**Status:** `done` — SEDP ist vor dem Handshake nicht, danach vollständig gematcht.
### §9.8.3 Figure 23 — Ablauf der Authentisierung mit einem entdeckten Participant

**Spec:** §9.8.3, Figure 23, S. 190–192 — Schritte 1 bis 18:
validate_remote_identity auf beiden Seiten, begin_handshake_request,
begin_handshake_reply, process_handshake auf beiden Seiten,
get_shared_secret und get_authenticated_peer_credential_token auf beiden
Seiten.

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`, `on_stateless_message`, `collect_completion` (`shared_secret`, `get_authenticated_peer_credential_token`, `return_authenticated_peer_credential_token`)

**Tests:** `crates/discovery/src/security/stack.rs::tests::figure_23_calls_the_operations_of_both_plugins_in_order`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`,
`crates/dcps/src/runtime.rs::tests::handshake_completes_through_runtime_dispatch_e2e`

**Status:** `done` — beide Seiten rufen `validate_remote_identity`, der Initiator `begin_handshake_request`, der Replier `begin_handshake_reply`, beide `process_handshake`, danach `get_shared_secret` und `get_authenticated_peer_credential_token` (Reihenfolge am aufgezeichneten Plugin geprüft).

### §9.8.4 AccessControl wirkt nicht auf eingebaute Endpunkte von RTPS und Stateless

**Spec:** §9.8.4, S. 193 — "The AccessControl plugin shall impact only the
Builtin Secure Entities and the application-defined Entities. It shall not
impact the builtin entities defined by the DDS-RTPS Protocol specification
nor the BuiltinParticipantStatelessMessageWriter or the
BuiltinParticipantStatelessMessageReader."

**Repo:** `crates/dcps/src/runtime.rs::check_create_endpoint` (Z. 9685, nur
aus den Pfaden für Writer und Reader der Anwendung aufgerufen, Z. 9868 und
10318)

**Tests:** `crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`

**Status:** done

### §9.8.4 Table 48 — Gruppen der Operationen

**Spec:** §9.8.4, Table 48, S. 193–194 — Group1 bis Group5 und ihre Wirkung
auf DomainParticipant, eingebaute Entities von RTPS, Builtin Secure Entities,
Endpunkte für Stateless Messages und Entities der Anwendung.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Regeln stehen einzeln in
den Items von §9.8.4 bis §9.8.8.

### §9.8.4 Sichere eingebaute Endpunkte unbedingt anlegen

**Spec:** §9.8.4, S. 194 — "This means that the Secure Builtin Entities
shall be created unconditionally when the DomainParticipant is created."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`
(Z. 8638)

**Tests:** `crates/dcps/src/runtime.rs::tests::c34c_security_builtin_wiring_end_to_end`,
`crates/dcps/src/runtime.rs::tests::enable_security_builtins_with_auth_activates_handshake_driver`

**Status:** done — `security.secure_spdp: false` schaltet die Endpunkte von
DCPSParticipantsSecure ab; die Vorgabe ist an (siehe §7.5.1.6).

### §9.8.4 get_*_security_config für die sicheren eingebauten Endpunkte

**Spec:** §9.8.4, S. 194 — "During the creation process of DataWriter
entities the get_datawriter_security_config shall be called and likewise
during the creation process of DataReader entities the
get_datareader_security_config shall be called. The purpose of calling these
get_xxx_security_config operations is to obtain the information necessary to
call the Cryptographic plugin operations on these endpoints."

**Repo:** `crates/security-runtime/src/builtin_attributes.rs::builtin_endpoint_kinds_asked`, `kinds_of_endpoint_config`, `builtin_topic_name`; `crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`, `register_endpoint_crypto`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_endpoint_config_of_the_plugin_gives_the_protection_of_an_endpoint`,
`crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/security-runtime/src/builtin_attributes.rs::tests::the_plugin_is_not_asked_for_the_endpoints_table_21_fixes`

**Status:** `done` — bei einem AccessControl-Plugin der Anwendung ruft die Laufzeit `get_datawriter_security_config` / `get_datareader_security_config` für die Endpunkte von DCPSParticipantsSecure, DCPSPublicationsSecure, DCPSSubscriptionsSecure und DCPSParticipantMessageSecure und folgt der Antwort; Anwendungsendpunkte ebenso. Für die Endpunkte von DCPSParticipantVolatileMessageSecure und die sicheren TypeLookup-Endpunkte (Request und Reply) legt §7.5.13 Table 21 den Schutz fest (`is_submessage_protected true`, alle anderen Attribute `false`, unabhängig vom Plugin und von der Governance). Dort entscheidet nicht das Plugin; die Laufzeit fragt es nicht und schützt nach Table 21. Der Test legt ein Plugin an, das nichts schützt, und prüft, dass es für diese sechs Endpunkte nicht gerufen wird und die Schutzarten von Table 21 gelten; für DCPSPublicationsSecure wird es gerufen und seine Antwort befolgt. Der Plugin-Test in C, C++ und Java prüft, dass der Cryptographic-Plugin die Volatile-Endpunkte unter `dds.sec.builtin_endpoint_name` erhält. Grün auf dem Linux-Testhost (`/root/zerodds-rc7/agent-logs/sec-behavior-1018-105058.log`).

### §9.8.4 Endpunkte für Stateless Messages für jeden Participant

**Spec:** §9.8.4, S. 194 — "DDS Secure implementations shall create these
endpoints unconditionally for all created DomainParticipant."

**Repo:** `crates/dcps/src/runtime.rs::enable_security_builtins_with_auth`
(Z. 8638)

**Tests:** `crates/dcps/src/runtime.rs::tests::enable_security_builtins_with_auth_activates_handshake_driver`

**Status:** done — für jeden Participant mit Security; ein Participant ohne
Security hat die Endpunkte nicht.

### §9.8.4 Partition von Publisher und Subscriber

**Spec:** §9.8.4, S. 195 — "They are impacted indirectly by operations in
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

**Status:** `done` — die Partitionen des entfernten Endpunkts gehen in die Prüfung; ein Grant mit Partitionen erlaubt nur diese; das Plugin der Anwendung sieht sie in der Anfrage.
### §9.8.5 AccessControl behavior with local participant creation

**Spec:** §9.8.5, S. 195 — "The functionality of the AccesControl plugin with
regards to the creation of local DDS DomainParticipant entities was
illustrated in Figure 22 and described in 9.8.1."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf §9.8.1.

### §9.8.6 Schritte 1 bis 3: Topic mit get_topic_security_config und check_create_topic

**Spec:** §9.8.6, Figure 24, S. 196 — "2. The middleware calls
AccessControl::get_topic_security_config to obtain the TopicSecurityConfig
for the Topic being created. 3. [...] Operation
AccessControl::check_create_topic() is called for this verification. If the
verification fails, the Topic object is not created."

**Repo:** `crates/dcps/src/runtime.rs::check_create_topic_access` (`get_topic_security_config`, `check_create_topic`); `crates/security-runtime/src/shared.rs::SharedSecurityGate::topic_security_config`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`

**Status:** `done` — `get_topic_security_config`, `return_topic_security_config` und `check_create_topic` in dieser Reihenfolge; ein Topic, das das Plugin ablehnt, wird nicht angelegt.

### §9.8.6 Schritte 4, 5, 8 und 9: Writer und Reader nur mit Berechtigung

**Spec:** §9.8.6, S. 196–197 — "5. The middleware verifies that the
DataWriter has the right permissions to publish on Topic topicName.
Operation AccessControl::check_create_datawriter() is called for this
verification. [...] If the verification doesn't succeed, the DataWriter is
not created. [...] 9. [...] If the verification doesn't succeed, the
DataReader is not created."

**Repo:** `crates/dcps/src/runtime.rs::check_create_endpoint` (Z. 9685)

**Tests:** `crates/dcps/tests/security_endpoint_access_e2e.rs::the_six_omg_suite_refusals_reach_the_runtime`,
`crates/dcps/tests/security_endpoint_access_e2e.rs::an_unarmed_gate_refuses_instead_of_creating`

**Status:** done — die optionalen Prüfungen von Data Tags und QoS sind
nicht umgesetzt (siehe §9.4.2.9).

### §9.8.6 Schritte 6 und 10: get_datawriter_/get_datareader_security_config

**Spec:** §9.8.6, S. 196–197 — "6. The middleware calls
AccessControl::get_datawriter_security_config to obtain the
EndpointSecurityConfig for the created DataWriter. [...] 10. The middleware
calls the operation AccessControl::get_datareader_security_config".

**Repo:** `crates/dcps/src/runtime.rs::register_endpoint_crypto` (`SharedSecurityGate::endpoint_security_config`)

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/src/runtime.rs::tests::the_endpoint_config_of_the_plugin_gives_the_protection_of_an_endpoint`

**Status:** `done` — `get_datawriter_security_config` und `get_datareader_security_config` nach `check_create_*`, mit `return_*_security_config`; die Antwort entscheidet, ob der Endpunkt Schlüsselmaterial hat und mit welchem Schutz.

### §9.8.6 Schritte 7 und 11: Discovery je nach is_discovery_protected

**Spec:** §9.8.6, S. 197 — "7. [...] has the is_discovery_protected attribute
set to FALSE. In this situation the middleware configures Discovery to use
regular (not secure) publications discovery endpoint (DCPSPublications) [...]
11. [...] set to TRUE. In this situation the middleware configures Discovery
to use the secure subscriptions discovery endpoint".

**Repo:** `crates/dcps/src/runtime.rs::sync_sedp_discovery_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_topic_config_of_the_plugin_chooses_the_secure_sedp`,
`crates/dcps/src/runtime.rs::tests::discovery_and_liveliness_follow_the_topic_rule`

**Status:** `done` — `is_discovery_protected` der TopicSecurityConfig des Plugins wählt zwischen den regulären und den sicheren Discovery-Endpunkten, auch gegen die Governance; ein Plugin, das fehlschlägt, schützt.

### §9.8.6 Schritte 12 bis 15: Registrieren und Löschen von Instanzen

**Spec:** §9.8.6, S. 197 — "13. [...] The operation
AccessControl::check_local_datawriter_register_instance() is called for
this verification. If the verification doesn't succeed, the instance is not
registered. [...] 15. [...] If the verification doesn't succeed, the
instance is not disposed."

**Repo:** `crates/dcps/src/publisher.rs` (`register_instance`, `dispose`); `crates/security-runtime/src/shared.rs::SharedSecurityGate::check_local_datawriter_instance`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_access_control_operations_are_called_in_the_order_of_figures_22_and_24`,
`crates/dcps/tests/security_access_plugin_e2e.rs::instance_operations_are_asked_of_the_plugin`

**Status:** `done` — `check_local_datawriter_register_instance` und `check_local_datawriter_dispose_instance` vor der Operation; bei einer Ablehnung bleibt die Instanz unverändert.

### §9.8.7 Keine Prüfung der Berechtigung bei nicht authentisierten Participants

**Spec:** §9.8.7, S. 197 — "Local DomainParticipant will not perform any
further participant AccessControl validation with unauthenticated
participants (i.e., validate_remote_permissions and
check_remote_participant will not be called)."

**Repo:** `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `crates/dcps/src/runtime.rs::tests::an_unauthenticated_participant_is_not_checked_for_permissions`

**Status:** `done` — für einen nicht authentisierten Participant wird weder `validate_remote_permissions` noch `check_remote_participant` aufgerufen; das Ereignis `security.remote_permissions.refused` bleibt aus.
### §9.8.7 is_access_protected=TRUE: Prüfung und Ausschluss des Peers

**Spec:** §9.8.7, S. 197–198 — "If the DomainParticipant discovers a
DomainParticipant entity that it can authenticate successfully, and
is_access_protected is TRUE, then it shall validate with the AccessControl
plugin that it has the permissions necessary to join the DDS domain. [...]
If the validation fails, the discovered DomainParticipant shall be
considered ignored and all the builtin Topics should not be matched."

**Repo:** `crates/dcps/src/runtime.rs::store_remote_permissions`, `ignore_unauthorized_participant`

**Tests:** `crates/dcps/tests/security_remote_participant_access_e2e.rs::an_authenticated_peer_without_a_domain_grant_is_ignored_only_where_the_domain_is_protected`

**Status:** `done` — ein authentisierter Peer ohne Grant für die Domain wird ignoriert und nicht gematcht.
### §9.8.7 is_access_protected=FALSE: HandleNIL ohne Folgen

**Spec:** §9.8.7, S. 198 — "If the DomainParticipant discovers a
DomainParticipant entity that it can authenticate successfully, and
is_access_protected is FALSE, then validation will succeed with no access
control checking. In this case, only get_authenticated_peer_credential_token
and validate_remote_permissions are called, and a HandleNIL return will not
impact the validation result."

**Repo:** `crates/dcps/src/runtime.rs::store_remote_permissions`

**Tests:** `crates/dcps/tests/security_remote_participant_access_e2e.rs::an_authenticated_peer_without_a_domain_grant_is_ignored_only_where_the_domain_is_protected`

**Status:** `done` — ohne `enable_join_access_control` bleibt derselbe Peer erhalten.
### §9.8.7 Figure 25 Schritte 3 und 5: check_remote_participant und check_remote_topic

**Spec:** §9.8.7, Figure 25, S. 198–199 — "3. Participant1 calls the operation
check_remote_participant to verify the remote DomainParticipant
(Participant2) is allowed to join the DDS domain [...] 5. [...] The operation
check_remote_topic is called for this verification. If the verification
fails, the discovered Topic is ignored."

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::check_remote_participant`, `check_remote_topic`; `crates/dcps/src/runtime.rs` (`wire_*_to_remote_*_unguarded`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_plugin_is_asked_about_the_endpoints_of_authenticated_participants_only`,
`crates/dcps/tests/security_remote_participant_access_e2e.rs::an_authenticated_peer_without_a_domain_grant_is_ignored_only_where_the_domain_is_protected`

**Status:** `done` — `check_remote_participant` ignoriert den Peer; `check_remote_topic` weist Endpunkte eines Topics ab, das das Plugin oder der Grant des Peers nicht erlaubt (Ereignis `match.refused` nennt §9.4.2.9.12).
### §9.8.8 Endpunkte nicht authentisierter Participants über reguläre Discovery

**Spec:** §9.8.8, S. 199 — "If the ParticipantSecurityConfig object returned
by the AccessControl operation get_participant_security_config has the
is_access_protected attribute set to FALSE, the DomainParticipant may have
matched a remote 'Unauthenticated' DomainParticipant".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Überleitung; der Satz nennt
`is_access_protected`, wo nach §9.8.3 `allow_unauthenticated_participants`
gemeint ist (Widerspruch in der Spec).

### §9.8.8.1 Nicht authentisiert: lokale Endpunkte geschützter Topics nicht matchen

**Spec:** §9.8.8.1, S. 199 — "Reject (do not try to match) local endpoints for
which the related TopicSecurityConfig have the attribute is_read_protected or
is_write_protected set to TRUE."

**Repo:** `crates/dcps/src/runtime.rs::refuses_unauthenticated_peer`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_that_is_not_authenticated_gets_no_endpoint_on_a_protected_topic`

**Status:** `done` — Endpunkte eines nicht authentisierten Peers werden auf Topics mit Read- oder Write-Schutz nicht gematcht; das Ereignis `match.refused` nennt §9.8.8.1.
### §9.8.8.1 und §9.8.8.2 Nicht geschützte Topics ohne AccessControl matchen

**Spec:** §9.8.8.1, §9.8.8.2, S. 199–200 — "Proceed to try matching without
checking with the AccessControl plugin the local DataWriter endpoints for
which the related TopicSecurityConfig [...] have the attribute
is_read_protected set to FALSE. Proceed to try matching without checking
with the AccessControl plugin the local DataReader endpoints for which [...]
is_write_protected set to FALSE." (Figure 26)

**Repo:** `crates/dcps/src/runtime.rs::refuses_unauthenticated_peer`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_participant_that_is_not_authenticated_gets_no_endpoint_on_a_protected_topic`

**Status:** `done` — die offene Topic (kein Read- und kein Write-Schutz) wird mit dem Peer über das einfache SEDP gematcht, ohne Aufruf an AccessControl.
### §9.8.8.2 Authentisiert: Prüfung für geschützte Topics vor dem Matching

**Spec:** §9.8.8.2, S. 199–200 — "Perform the AccessControl checks for
discovered endpoints that would match local DataWriters for whom the
is_read_protected attribute is set to TRUE, and only proceed to try matching
the discovered endpoints for whom the access control checks succeed. Perform
the AccessControl checks for discovered endpoints that would match local
DataReader for whom the is_write_protected attribute is set to TRUE".

**Repo:** `crates/dcps/src/runtime.rs::wire_writer_to_remote_reader`, `wire_reader_to_remote_writer` (`check_remote_topic`, `check_remote_datareader`, `check_remote_datawriter`), `peer_is_unauthenticated`, `topic_is_access_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_plugin_is_asked_about_the_endpoints_of_authenticated_participants_only`

**Status:** `done` — für einen authentisierten Participant fragt die Laufzeit das Plugin nach `check_remote_topic` und `check_remote_datareader` / `check_remote_datawriter` mit Topic und Partitionen des entfernten Endpunkts und matcht nur bei Erlaubnis (`RelayOnly` bei check_remote_datareader); für einen nicht authentisierten fragt sie das Plugin nicht (§9.8.8.1), ein geschütztes Topic matcht dort nicht.

### §9.8.8.2 Figure 27 Schritte 4 und 6: Instanzen entfernter Writer

**Spec:** §9.8.8.2, Figure 27, S. 201–202 — "4. DataReader1 shall call the
operation check_remote_datawriter_register_instance [...] If the verification
doesn't succeed, the sample shall be ignored. [...] 6. DataReader1 shall call
the operation check_remote_datawriter_dispose_instance [...] If the
verification doesn't succeed, the instance disposal shall be ignored."

**Repo:** `crates/dcps/src/runtime.rs::remote_instance_refused`; `crates/security-runtime/src/shared.rs::SharedSecurityGate::check_remote_datawriter_instance`

**Tests:** `crates/dcps/tests/security_remote_instance_e2e.rs::a_sample_of_an_instance_the_plugin_refuses_to_register_is_ignored`,
`crates/dcps/tests/security_remote_instance_e2e.rs::a_disposal_the_plugin_refuses_is_ignored`,
`crates/dcps/tests/security_remote_instance_e2e.rs::a_remote_writer_that_the_plugin_permits_registers_and_disposes`

**Status:** `done` — der DataReader ruft `check_remote_datawriter_register_instance` beim ersten Sample einer Instanz und `check_remote_datawriter_dispose_instance` bei einer Löschung; bei einer Ablehnung wird das Sample, die Löschung, ignoriert. Die Gegenprobe mit erlaubendem Plugin liefert beides.

### §9.8.9 Cryptographic Plugin key generation behavior

**Spec:** §9.8.9, S. 202–203 — "Key Generation is potentially needed for:
The DomainParticipant as a whole, Each DomainParticipant match pair, Each
builtin secure endpoint (DataWriter or DataReader), Each builtin secure
endpoint match pair, Each application secure endpoint (DataWriter or
DataReader), Each application secure endpoint match pair".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung; die Fälle sind unten einzeln
geführt.

### §9.8.9.1 Schlüssel für VolatileMessageSecure aus dem SharedSecret, ohne Token

**Spec:** §9.8.9.1, S. 203 — "Effectively this means the key material used
for key-exchange is always derived from the SharedSecret. [...] the DDS
middleware shall not call the operation create_local_datawriter_crypto_tokens
or the operation set_remote_datareader_crypto_tokens on the
CryptoKeyExchange."

**Repo:** `crates/security-runtime/src/shared.rs::register_remote_by_guid_from_secret`
(Z. 732); `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`
(Z. 21431, ohne die Endpunkte für VolatileMessageSecure)

**Tests:** `crates/security-runtime/src/shared.rs::tests::kx_channel_round_trips_through_gate`,
`crates/security-crypto/src/plugin.rs::tests::cyclone_kx_key_matches_both_directions`

**Status:** done

### §9.8.9.1 Property dds.sec.builtin_endpoint_name

**Spec:** §9.8.9.1, S. 203 — "The DDS implementation shall add a property with
name 'dds.sec.builtin_endpoint_name' and value
'BuiltinParticipantVolatileMessageSecureWriter' to the Property_t passed to
the operation register_local_datawriter when it registers the
BuiltinParticipantVolatileMessageSecureWriter with the CryptoKeyFactory.
[...] value 'BuiltinParticipantVolatileMessageSecureReader' [...]
register_local_datareader".

**Repo:** `crates/security-runtime/src/gate_crypto.rs::volatile_properties`, `CryptoLayer::ensure_local`

**Tests:** `crates/dcps/tests/security_access_sequence.rs::the_cryptographic_plugin_registers_the_participant_and_the_volatile_endpoints`

**Status:** `done` — die Registrierung des Writers und des Readers von VolatileMessageSecure trägt die Property `dds.sec.builtin_endpoint_name` mit den Namen der Spec; die Registrierung läuft einmal, nach `register_local_participant`.
### §9.8.9.2 register_local_participant für jeden lokalen Participant

**Spec:** §9.8.9.2, S. 203 — "For each local DomainParticipant that is
successfully created the DDS implementation shall call the operation
register_local_participant on the KeyFactory."

**Repo:** `crates/security-runtime/src/shared.rs::ensure_local` (Z. 637)

**Tests:** `crates/security-runtime/src/shared.rs::tests::e2e_alice_bob_with_shared_gate`

**Status:** done — beim ersten Bedarf, nicht beim Anlegen.

### §9.8.9.2 register_matched_remote_participant für jeden authentisierten Peer

**Spec:** §9.8.9.2, S. 203 — "For each discovered DomainParticipant that has
successfully authenticated and has been matched to the local
DomainParticipant the DDS middleware shall call the operation
register_matched_remote_participant on the KeyFactory."

**Repo:** `crates/dcps/src/runtime.rs::prepare_crypto_token` (Z. 21368)

**Tests:** `crates/dcps/src/runtime.rs::tests::handshake_completes_through_runtime_dispatch_e2e`,
`crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done

### §9.8.9.3 Schlüssel der sicheren eingebauten Endpunkte

**Spec:** §9.8.9.3, S. 204 — "For each DataWriter belonging to the list of
'Builtin Secure Endpoints', see 7.5.8, with the exception of the
BuiltinParticipantVolatileMessageSecureWriter, the DDS middleware shall call
the operation register_local_datawriter [...] 1. Call the operation
KeyFactory::register_matched_remote_datawriter for each local DataWriter
[...] 2. Call the operation KeyFactory::register_matched_remote_datareader".

**Repo:** `crates/dcps/src/runtime.rs::local_endpoint_crypto_handle`, `prepare_endpoint_crypto_tokens`; `crates/security-runtime/src/gate_crypto.rs::CryptoLayer::endpoint_tokens` (`register_matched_remote_datareader` / `_datawriter`)

**Tests:** `crates/dcps/tests/security_crypto_sequence.rs::the_builtin_secure_endpoints_are_registered_and_matched`

**Status:** `done` — `register_local_datawriter` / `register_local_datareader` für die sicheren eingebauten Endpunkte (ohne den Writer von VolatileMessageSecure), dann `register_matched_remote_datareader` / `_datawriter` und `create_local_*_crypto_tokens`, mit den Kinds aus §10.4.1.2.5.

### §9.8.9.4 Schlüssel der Endpunkte der Anwendung

**Spec:** §9.8.9.4, S. 204 — "For each non-builtin DataWriter for whom the
associated EndpointSecurityConfig object has either the member
is_submessage_protected or the member is_payload_protected set to TRUE, the
DDS middleware shall: 1. Call the operation register_local_datawriter [...]
2. Call the operation register_matched_remote_datareader for each discovered
DataReader that matches the DataWriter."

**Repo:** `crates/dcps/src/runtime.rs::register_endpoint_crypto`, `prepare_endpoint_crypto_tokens`; `crates/security-runtime/src/gate_crypto.rs::CryptoLayer::endpoint_tokens`

**Tests:** `crates/dcps/tests/security_crypto_sequence.rs::the_key_material_of_two_endpoints_and_the_protected_data_follow_the_figures`,
`crates/security-runtime/src/shared.rs::tests::per_endpoint_datawriter_token_roundtrips_by_key_id`

**Status:** `done` — `register_local_datawriter` mit der Konfiguration des Endpunkts, `register_matched_remote_datareader` für den entdeckten DataReader, dann die CryptoTokens; die Gegenseite setzt sie mit `set_remote_datawriter_crypto_tokens`.

### §9.8.9.5 Ablauf einer Key Revision

**Spec:** §9.8.9.5, S. 205 — "To perform a Key Revision the DDS middlewre
shall: 1. Call the operation revise_local_entity_keys [...] 2. Call the
operation create_local_participant_crypto_tokens [...] 5. Send the
CryptoTokens to the matched, authenticated, DomainParticipants [...] 6. Wait
until all the matched Authenticated DomainParticipant have acknowledged
receiving the CryptoTokens or else until sufficient time has elapsed. 7.
Call activate_key_revision".

**Repo:** `crates/dcps/src/runtime.rs::renew_key_material_after_revocation`, `complete_key_revision`; `RuntimeConfig::key_revision_ack_timeout`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_key_revision_is_activated_after_the_acknowledgement_or_the_timeout`,
`crates/dcps/src/runtime.rs::tests::a_revocation_makes_a_new_revision_of_the_keys_of_the_participant_and_its_endpoints`

**Status:** `done` — `revise_local_entity_keys`, die neuen CryptoTokens an die authentisierten Participants, Warten auf die Bestätigung des VolatileSecure-Writers oder `key_revision_ack_timeout` (die Spec nennt keinen Wert: „sufficient time“; Voreinstellung 5 s, programmatische Einstellung), dann `activate_key_revision`. Ausgelöst wird die Revision durch den Widerruf einer Identität (§9.3.2.12.2).

### §9.8.9.5 Rotation von Schlüsseln ist Sache der Plugins

**Spec:** §9.8.9.5, S. 205 — "DDS- Security does not use key revisions to
simply rotate the KeyMaterial because it has been used too long, or used to
protect too much data. Key Rotation for those purposes should be implemented
internally by the Security Plugins."

**Repo:** `crates/security-crypto/src/plugin.rs` (Methode `rotate_key`)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::rotate_key_resets_counter_and_changes_key`,
`crates/security-crypto/src/plugin.rs::tests::encrypts_remaining_decrements_per_call`

**Status:** done — Einzelheiten der Rotation des eingebauten Plugins stehen
in §10.5.

### §9.8.9.6 Größe durch Schlüssel einzelner Empfänger begrenzen

**Spec:** §9.8.9.6, S. 205–206 — "The selection between and configuration of
these choices is implementation specific, as it does not affect
interoperability."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweise für Implementierungen; ZeroDDS
erzeugt in der Laufzeit keine MACs für einzelne Empfänger (siehe §9.5.1.10).

### §9.8.10 Cryptographic Plugin key exchange behavior

**Spec:** §9.8.10, S. 206 — "Cryptographic key exchange is potentially needed
for: Each DomainParticipant match pair. Each builtin secure endpoint match
pair. Each application secure endpoint match pair."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Aufzählung.

### §9.8.10.1 Austausch der Schlüssel mit einem entdeckten Participant

**Spec:** §9.8.10.1, S. 206–207 — "Cryptographic key exchange shall occur
between each DomainParticipant and each discovered DomainParticipant that has
successfully authenticated. [...] 1. Call the operation
create_local_participant_crypto_tokens [...] 2. Send the
ParticipantCryptoTokenSeq [...] using
BuiltinParticipantVolatileMessageSecureWriter. The discovered
DomainParticipant shall call the operation
set_remote_participant_crypto_tokens".

**Repo:** `crates/dcps/src/runtime.rs::prepare_crypto_token` (Z. 21368),
`on_volatile_messages` (Z. 22563)

**Tests:** `crates/security-runtime/src/shared.rs::tests::data_round_trips_via_token_after_kx_register`,
`crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done — die Laufzeit sendet ParticipantCryptoTokens nur bei
`rtps_protection_kind` ungleich NONE (Z. 21387); das folgt Table 33
(is_rtps_axk_protected), nicht dem unbedingten Satz von §9.8.10.1 (Widerspruch in der Spec).

### §9.8.10.2 und §9.8.10.3 Austausch der Schlüssel mit entfernten Endpunkten

**Spec:** §9.8.10.2, §9.8.10.3, S. 208–209 — "Cryptographic key exchange shall
occur between each builtin secure DataWriter and the matched builtin secure
DataReader entities of authenticated matched DomainParticipant entities [...]
Cryptographic key exchange shall also occur between each application
DataWriter whose EndpointSecurityConfig object has either the
is_submessage_protected or the is_payload_protected members set to TRUE, and
each of its matched DataReader entities."

**Repo:** `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`, `endpoint_is_protected`

**Tests:** `crates/dcps/src/runtime.rs::tests::key_material_exists_for_protected_endpoints_only_and_goes_with_them`, `::pending_endpoint_tokens_keeps_late_user_token_after_builtins_sent`, `::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** `done` — Token gehen für die sicheren eingebauten Endpunkte nach den Schutzarten der Governance und für Endpunkte der Anwendung mit geschützten Submessages oder Nutzlast je passendem entferntem Endpunkt.

### §9.8.10.2 Keine Token für Endpunkte ohne Schutz

**Spec:** §9.4.2.7, Table 37, S. 128 — "If is_submessage_protected is FALSE,
then the CryptoKeyFactory, CryptoKeyExchange and CryptoTransform operations
are called only if is_payload_protected is TRUE."

**Repo:** `crates/dcps/src/runtime.rs::endpoint_is_protected`, `register_endpoint_crypto`

**Tests:** `crates/dcps/src/runtime.rs::tests::key_material_exists_for_protected_endpoints_only_and_goes_with_them`

**Status:** `done` — ein Endpunkt eines Topics ohne Schutz von Submessages und Nutzlast hat kein Schlüsselmaterial und bekommt keine Token.

### §9.8.10.4 Austausch nach einer Key Revision

**Spec:** §9.8.10.4, S. 210–211 — "Following the call to
revise_local_entity_keys the DDS middleware shall call the operation
create_local_participant_crypto_tokens [...] The DDS middleware shall wait
until the above CryptoTokens have been received or else a 'sufficient' time
has elapsed. After this it shall call activate_key_revision".

**Repo:** `crates/dcps/src/runtime.rs::renew_key_material_after_revocation`, `complete_key_revision`

**Tests:** `crates/dcps/src/runtime.rs::tests::a_key_revision_is_activated_after_the_acknowledgement_or_the_timeout`,
`crates/dcps/src/runtime.rs::tests::a_revocation_makes_a_new_revision_of_the_keys_of_the_participant_and_its_endpoints`

**Status:** `done` — wie §9.8.9.5: nach `revise_local_entity_keys` gehen die Tokens hinaus; `activate_key_revision` folgt der Bestätigung oder dem Ablauf der Zeit.

### §9.8.11 Reihenfolge von Kodierung und Dekodierung

**Spec:** §9.8.11, S. 211 — "This specification only requires that by the
time the RTPS message appears on the wire the proper encoding operations
have been executed first on each SerializedPayload submessage element, then
on the enclosing RTPS Submessage, and finally on the RTPS Message. Similarly
by the time a received RTPS Message is interpreted the proper decoding
operations are executed on the reverse order."

**Repo:** `crates/dcps/src/runtime.rs` (Z. 4590–4602: Nutzlast, Submessage,
Nachricht)

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_full_secure_profile`,
`crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`

**Status:** done

### §9.8.11.1 bis §9.8.11.4 Abläufe in Figure 31 bis 34

**Spec:** §9.8.11.1–§9.8.11.4, Figures 31–34, S. 211–219 — Kodierung je
Submessage eines Writers oder Readers, danach encode_rtps_message;
Dekodierung mit decode_rtps_message, preprocess_secure_submessage,
decode_datawriter_submessage oder decode_datareader_submessage und
decode_serialized_payload.

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::encode_serialized_payload`, `encode_data_datawriter_by_handle`, `transform_outbound_for`, `transform_inbound_from`, `decode_data_by_key_id`, `decode_serialized_payload`; `crates/security-runtime/src/gate_crypto.rs::CryptoLayer::decode_submessage` (`preprocess_secure_submsg`)

**Tests:** `crates/dcps/tests/security_crypto_sequence.rs::the_key_material_of_two_endpoints_and_the_protected_data_follow_the_figures`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** `done` — Figure 31 und 32: `encode_serialized_payload`, `encode_datawriter_submessage`, `encode_rtps_message`; Figure 33 und 34: `decode_rtps_message`, `preprocess_secure_submsg`, `decode_datawriter_submessage`, `decode_serialized_payload`; Reihenfolge am aufgezeichneten Plugin geprüft.

### §10.1 Introduction

**Spec:** §10.1, S. 221 — "This specification defines the behavior and
implementation of at least one builtin plugin for each kind of plugin. The
builtin plugins provide out-of-the-box interoperability between
implementations of this specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Plugins aus Table 49 sind
einzeln geführt.

### §10.1 Table 49 — Authentication: DDS:Auth:PKI-DH

**Spec:** §10.1, Table 49, S. 221 — "Authentication DDS:Auth:PKI-DH Uses PKI
with a pre-configured shared Certificate Authority."

**Repo:** `crates/security-pki/src/plugin.rs::PkiAuthenticationPlugin`
(`plugin_class_id`, Z. 1071)

**Tests:** `crates/security-pki/src/plugin.rs::tests::plugin_class_id_matches_spec`

**Status:** done

### §10.1 Table 49 — AccessControl: DDS:Access:Permissions

**Spec:** §10.1, Table 49, S. 221 — "AccessControl DDS:Access:Permissions
Permissions document signed by shared Certificate Authority".

**Repo:** `crates/security-permissions/src/plugin.rs::PermissionsAccessControl`
(`plugin_class_id`, Z. 275); `crates/security-runtime/src/shared.rs::SharedSecurityGate`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::plugin_class_id_matches_spec`

**Status:** done — die Laufzeit nutzt die Typen des Plugins über
`SharedSecurityGate` (siehe §9.4.2).

### §10.1 Table 49 — Cryptography: DDS:Crypto:AES-GCM-GMAC

**Spec:** §10.1, Table 49, S. 221 — "Cryptography DDS:Crypto:AES-GCM-GMAC
AES-GCM (AES using Galois Counter Mode) for encryption. AES-GMAC for message
authentication."

**Repo:** `crates/security-crypto/src/plugin.rs::AesGcmCryptoPlugin`
(`plugin_class_id`, Z. 2006)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::plugin_class_id_matches_spec`

**Status:** done

### §10.1 Table 49 — DataTagging: DDS:Tagging:DDS_Discovery

**Spec:** §10.1, Table 49, S. 221 — "DataTagging DDS:Tagging:DDS_Discovery
Send Tags via Endpoint Discovery".

**Repo:** `crates/security-runtime/src/data_tagging.rs::BuiltinDataTaggingPlugin` (`DATA_TAGGING_PLUGIN_CLASS_ID`); `crates/dcps/src/runtime.rs::DcpsRuntime::register_endpoint_tags`, `RuntimeConfig::security_data_tagging`

**Tests:** `crates/security-runtime/src/data_tagging.rs::tests::plugin_class_id_matches_spec_format`,
`crates/dcps/src/runtime.rs::tests::the_data_tagging_plugin_holds_the_tags_of_local_and_discovered_endpoints`,
`crates/dcps/src/runtime.rs::tests::without_a_data_tagging_plugin_the_qos_tags_are_announced`

**Status:** `done` — Klassen-Id `DDS:Tagging:DDS_Discovery`; die Laufzeit gibt die Tags jedes Endpunkts an das Plugin, kündigt an, was es liefert, und trägt die Tags entdeckter Endpunkte ein.
### §10.1 Table 49 — Logging: DDS:Logging:DDS_LogTopic

**Spec:** §10.1, Table 49, S. 221 — "Logging DDS:Logging:DDS_LogTopic Logs
security events to a dedicated DDS Log Topic".

**Repo:** `crates/security-logging/src/log_topic.rs::LogTopicLoggingPlugin`
(`plugin_class_id`, Z. 728)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::plugin_class_id_is_the_spec_name`

**Status:** done

### §10.2 Requirements and Priorities (Non-Normative)

**Spec:** §10.2, S. 221–222 — "The selection of the builtin plugins was
driven by several functional, as well as, non-functional requirements".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — als nicht normativ gekennzeichnet.

### §10.2.1 Performance and Scalability

**Spec:** §10.2.1, S. 222 — "The use of Asymmetric Key Cryptography shall be
limited to the discovery, authentication, session and shared-secret
establishment phase".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Teil des nicht normativen §10.2.

### §10.2.2 Robustness and Availability

**Spec:** §10.2.2, S. 222–223 — "Centralized policy decision points or
services should be avoided."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Teil des nicht normativen §10.2.

### §10.2.3 Fitness to the DDS Data-Centric Model

**Spec:** §10.2.3, S. 223 — "any encryption and/or digital signature applied
to a sample should be able to be processed in isolation".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Teil des nicht normativen §10.2.

### §10.2.4 Leverage and Reuse of Existing Security Infrastructure and Technologies

**Spec:** §10.2.4, S. 223 — "The builtin plugins leverage existing standards
and tools for PKI, ciphers, hashing and digital signing."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Teil des nicht normativen §10.2.

### §10.2.5 Ease-of-Use while Supporting Common Application Requirements

**Spec:** §10.2.5, S. 223 — "The builtin plugins balance rich functionality
and ease-of-use".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Teil des nicht normativen §10.2.

### §10.3 Konfiguration mit CA, privatem Schlüssel und Zertifikat vor der Aktivierung

**Spec:** §10.3, S. 224 — "Prior to a DomainParticipant being enabled the
DDS:Auth:PKI-DH plugin associated with the DomainParticipant must be
configured with three things: 1. The X.509 Certificate that defines the
Shared Identity CA. [...] 2. The Private Key of the DomainParticipant. 3. An
X.509 Certificate that chains up to the Shared Identity CA".

**Repo:** `crates/security-runtime/src/qos_properties.rs::profile_config_from_properties`
(Z. 380); `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`
(Z. 301)

**Tests:** `crates/dcps/tests/security_qos_properties.rs::a_complete_dds_sec_property_list_creates_a_secured_participant`,
`crates/security-runtime/src/qos_properties.rs::tests::each_missing_property_is_reported_by_its_own_name`

**Status:** done

### §10.3.1 Table 50 — Konfiguration über PropertyQosPolicy mit Präfix dds.sec.auth

**Spec:** §10.3.1, Table 50, S. 224–225 — "The builtin authentication plugin
shall be configured using the PropertyQosPolicy of the
DomainParticipantQos. [...] (all properties have 'dds.sec.auth' prefix) [...]
(all these properties shall have propagate set to FALSE) URI syntax follows
IETF RFC 3986."

**Repo:** `crates/security-runtime/src/qos_properties.rs` (Konstanten
`PROP_IDENTITY_CA` bis `PROP_KEY_ESTABLISHMENT`, Z. 72–88)

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_property_names_are_the_spec_strings`,
`crates/security-runtime/src/qos_properties.rs::tests::the_profile_config_follows_the_property_list`

**Status:** done — `PropertyQosPolicy` kennt kein `propagate`; die Properties
werden nicht über Discovery gesendet (siehe §7.3.21).

### §10.3.1 Table 50 — identity_ca

**Spec:** §10.3.1, Table 50, S. 225 — "identity_ca URI to the X509
certificate [39] of the Identity CA. Supported URI schemes: file, data,
pkcs11".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`
(Z. 302), `resolve_file_uri` (Z. 252)

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::every_file_uri_form_resolves_to_a_path`,
`crates/security-runtime/src/qos_properties.rs::tests::a_data_uri_is_the_content_and_not_a_path`,
`crates/security-runtime/src/pkcs11.rs::tests::every_path_attribute_parses`

**Status:** done — `pkcs11:` nur mit dem Feature `pkcs11` von
`zerodds-security-runtime`, ohne das Feature wird der Wert mit
`Pkcs11FeatureDisabled` abgewiesen.

### §10.3.1 Table 50 — private_key

**Spec:** §10.3.1, Table 50, S. 225 — "private_key URI to access the private
Private Key for the DomainParticipant Supported URI schemes: file, data,
pkcs11 [...] file:identity_ca_private_key.pem?password=OpenSesame [...]
pkcs11:object=MyParticipantPrivateKey;type=private?pin-value=OpenSesame".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`, `resolve_private_key`; `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_uri_schemes_that_are_not_paths_are_named_not_opened_as_paths`, `::a_password_in_the_key_uri_is_not_part_of_the_path`, `crates/security-runtime/src/pkcs11.rs::tests::every_query_attribute_parses`

**Status:** `done` — `file:`, `data:` und `pkcs11:` mit `pin-value` werden gelesen; `?password=` an `file:` ist das Passwort.

### §10.3.1 Table 50 — private_key: Query ?password= an file:

**Spec:** §10.3.1, Table 50, S. 225 — "file:identity_ca_private_key.pem?password=OpenSesame".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_private_key`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::a_password_in_the_key_uri_is_not_part_of_the_path`

**Status:** `done` — die Query ist nicht Teil des Pfads; `password` wird prozentdekodiert; die Property `dds.sec.auth.password` hat Vorrang.

### §10.3.1 Table 50 — password

**Spec:** §10.3.1, Table 50, S. 225 — "The value of the password property
shall be interpreted as the Base64 encoding of the AES-128 key that shall be
used to decrypt the private_key using AES128-CBC. If the password property is
not present, then the value supplied in the private_key property must contain
the unencrypted private key."

**Repo:** `crates/security-pki/src/encrypted_key.rs::decrypt_private_key_pem_as`, `PasswordReading`; `crates/security-runtime/src/qos_properties.rs::profile_config_from_properties`

**Tests:** `crates/security-pki/src/encrypted_key.rs::tests::the_clause_reading_is_the_default_and_the_passphrase_is_named`, `crates/security-pki/src/encrypted_key.rs::tests::the_clause_key_opens_a_file_encrypted_with_it`, `crates/security-pki/src/encrypted_key.rs::tests::an_unencrypted_key_ignores_the_password`, `crates/security-runtime/src/qos_properties.rs::tests::a_key_password_reaches_the_profile_config`, `::the_password_reading_is_the_clause_unless_named`

**Status:** `done` — Vorgabe ist die Lesart der Spec (Base64 des AES-128-Schlüssels); die Lesart als OpenSSL-Passphrase (Cyclone, Fast DDS) ist die Einstellung `zerodds.sec.auth.password_reading=openssl_passphrase`.

### §10.3.1 Table 50 — password gilt nicht für pkcs11:

**Spec:** §10.3.1, Table 50, S. 225 — "The password property is only used if
the private_key is provided with a 'file:' or a 'data:' URI. It does not
apply to private keys supplied with the 'pkcs11:' URI."

**Repo:** `crates/security-runtime/src/profile.rs::private_key_pem`

**Tests:** `crates/security-runtime/src/profile.rs::tests::a_password_does_not_apply_to_a_pkcs11_key`,
`crates/security-runtime/src/qos_properties.rs::tests::a_key_password_reaches_the_profile_config`

**Status:** `done` — ein Schlüssel auf einem Token wird nie entschlüsselt, das Passwort daneben wird nicht angewandt und ist kein Fehler (die PIN steht in `pin-value` der URI); für `file:` und `data:` öffnet es den Schlüssel, ein falsches ist ein Fehler. Der Test braucht kein Token, weil die Entscheidung vor dem Zugriff fällt.

### §10.3.1 Table 50 — identity_certificate

**Spec:** §10.3.1, Table 50, S. 226 — "identity_certificate URI to a X509
certificate signed by the IdentityCA in PEM format containing the signed
public key for the DomainParticipant Supported URI schemes: file, data,
pkcs11".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`
(Z. 302)

**Tests:** `crates/dcps/tests/security_qos_properties.rs::every_file_uri_spelling_is_resolved_on_the_participant_path`,
`crates/security-runtime/src/qos_properties.rs::tests::a_data_uri_with_a_media_type_decodes_by_rfc_2397`

**Status:** done

### §10.3.1 Table 50 — key_establishment_algorithm

**Spec:** §10.3.1, Table 50, S. 226 — "key_establishment_algorithm The string
'AUTO' or one of the CryptoAlgorithmName strings shown in Table 26 that
identifies a Key Establishment Algorithm. (The presence of this property is
optional)"

**Repo:** `crates/security-runtime/src/qos_properties.rs::kx_suite_from_properties`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_key_establishment_property_selects_a_suite_and_refuses_the_unknown`

**Status:** `done` — `AUTO`, `ECDHE-CEUM+P256`, `ECDHE-CEUM+P384`, `DHE+MODP-2048-256` und die Abwesenheit; zusätzlich `ECDH+prime256v1-CEUM`, `ECDHE-CEUM-P256`, `DH+MODP-2048-256` und `X25519`.

### §10.3.1 Table 50 — key_establishment_algorithm: AUTO und DHE+MODP-2048-256

**Spec:** §10.3.1.4, S. 226–227 — "The key_establishment_algorithm property
may be omitted or may have the value set to 'AUTO'. In both these cases, the
selection of the algorithm will be left to the Authentication plugin."

**Repo:** `crates/security-runtime/src/qos_properties.rs::kx_suite_from_properties`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_key_establishment_property_selects_a_suite_and_refuses_the_unknown`

**Status:** `done` — `AUTO` ist ECDHE-CEUM+P256, was jeder Hersteller spricht.

### §10.3.1.1 Identity CA Certificate

**Spec:** §10.3.1.1, S. 226 — "The certificate shall be the X.509 v3
Certificate [39] of the issuer of the Identity Certificates [...] The
certificate can be self-signed if it is a root CA or signed by some other CA
public key if it is a subordinate CA. Regardless of this the Public Key in
the Certificate shall be accepted as the one for the Identity CA trusted to
sign DomainParticipant Identity Certificates".

**Repo:** `crates/security-pki/src/plugin.rs::validate_with_config` (Z. 368)

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_accepts_ca_signed_cert`,
`crates/security-pki/src/plugin.rs::tests::validate_local_identity_rejects_wrong_ca`

**Status:** done — ein Test mit einer untergeordneten CA als Vertrauensanker
wurde nicht gefunden (nicht geprüft, ob er sich wie eine Wurzel verhält).

### §10.3.1.2 Private Key: Algorithmus nach §8.2

**Spec:** §10.3.1.2, S. 226 — "The Private Key associated with the
DomainParticipant. The algorithm of the private key of the CA shall be one of
the algorithms defined in 8.2."

**Repo:** `crates/security-pki/src/plugin.rs::sign_with`, `signing_scheme`; `crates/security-pki/src/identity.rs::private_key_pkcs8_der`

**Tests:** `crates/security-pki/src/plugin.rs::tests::a_p384_identity_authenticates_with_ecdsa_p384`,
`crates/security-pki/src/plugin.rs::tests::an_rsa_identity_signs_pkcs1_when_the_peer_verifies_only_that`,
`crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`,
`crates/security-pki/tests/private_key_forms.rs::a_pkcs1_rsa_key_reaches_the_backend_as_pkcs8`,
`crates/security-pki/tests/private_key_forms.rs::a_sec1_ec_key_reaches_the_backend_as_the_same_pkcs8_openssl_writes`

**Status:** `done` — die vier Algorithmen von Table 25 laufen: ECDSA P-256 (Vorgabe), ECDSA P-384, RSASSA-PSS und RSASSA-PKCS1-v1_5 mit einem RSA-Schlüssel; die Formen PKCS#8, PKCS#1 und SEC1 werden gelesen.

### §10.3.1.3 Identity Certificate

**Spec:** §10.3.1.3, S. 226 — "An X.509 v3 Certificate [39] that chains up
to the Identity CA (see 10.3.1.1). The Identity Certificate binds the Public
Key of the DomainParticipant to the Distinguished Name (subject name) for the
DomainParticipant."

**Repo:** `crates/security-pki/src/plugin.rs::validate_with_config` (Z. 368)

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_local_identity_accepts_ca_signed_cert`,
`crates/security-runtime/src/qos_properties.rs::tests::the_x509v1_deviation_comes_out_of_the_property_list`

**Status:** done — ein Zertifikat X.509 v1 wird nur mit der eigenen Property
`PROP_ACCEPT_X509V1_IDENTITY` angenommen (Vorgabe aus).

### §10.3.1.4 Key Establishment Algorithm des Initiators

**Spec:** §10.3.1.4, S. 226–227 — "The Key Establishment Algorithm that the
DomainParticipant will use in the situations when the DomainParticipant
initiates the Authentication handshake. The algorithm shall be one of the
algorithms defined in 8.2."

**Repo:** `crates/security-runtime/src/qos_properties.rs::kx_suite_from_properties`; `crates/security-pki/src/plugin.rs::set_preferred_kx_suite`, `begin_handshake_request`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_key_establishment_property_selects_a_suite_and_refuses_the_unknown`,
`crates/security-pki/src/plugin.rs::tests::kagree_algo_string_round_trips_for_every_supported_suite`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_runs_over_dhe_modp_2048_256`,
`crates/security-pki/src/plugin.rs::tests::set_participant_security_config_fails_when_a_used_algorithm_is_not_supported`

**Status:** `done` — die Property wählt das Verfahren des Initiators: `AUTO` und das Fehlen sind ECDHE-CEUM+P256, dazu ECDHE-CEUM+P384 und DHE+MODP-2048-256 (Table 26); `set_participant_security_config` prüft, dass es in der unterstützten Maske steht. Die Verweisung auf §8.2 statt §8.3 ist ein Widerspruch der Spec.

### §10.3.2 Nicht spezifizierte Attribute und unbekannte Properties

**Spec:** §10.3.2, S. 227 — "Plugin implementations may place data in these
attributes as long as they also include a property attribute that allows the
implementation to unambiguously detect the presence and interpret these
attributes. Attributes that are not understood shall be ignored."

**Repo:** `crates/security-pki/src/handshake_token.rs::parse_reply_token`
(Z. 604), `parse_request_token` (Z. 479)

**Tests:** `crates/security-pki/src/plugin.rs::tests::extra_unknown_properties_in_reply_accepted_forward_compat`,
`crates/security-pki/src/identity_token.rs::tests::unknown_property_skipped_in_decode`

**Status:** done

### §10.3.2.1 Table 51 — IdentityToken: Properties

**Spec:** §10.3.2.1, Table 51, S. 227 — "class_id 'DDS:Auth:PKI-DH:1.2'
properties [...] dds.cert.sn The subject name of the Identity Certificate.
dds.cert.algo One of the CryptoAlgorithmName string identifiers for digital
signature algorithms defined in Table 25. dds.ca.sn [...] dds.ca.algo".

**Repo:** `crates/security-pki/src/identity_token.rs::IdentityToken::encode`, `decode`, `build_identity_token_from_der_named`

**Tests:** `crates/security-pki/src/identity_token.rs::tests::a_token_without_the_optional_properties_decodes`,
`crates/security-pki/src/identity_token.rs::tests::an_empty_property_is_left_out_of_the_token`,
`crates/security-pki/src/identity_token.rs::tests::the_algorithms_are_the_names_of_table_25`,
`crates/security-pki/src/identity_token.rs::tests::round_trip_full_token`,
`crates/security-pki/src/plugin.rs::tests::the_identity_token_names_the_algorithms_of_table_25`

**Status:** `done` — `dds.cert.sn`, `dds.cert.algo`, `dds.ca.sn`, `dds.ca.algo`: jede Property ist optional (ohne Wert wird sie nicht gesendet, ein fehlendes Feld ist beim Lesen leer).

### §10.3.2.1 Table 51 — class_id DDS:Auth:PKI-DH:1.2 und Namen nach Table 25

**Spec:** §10.3.2.1, Table 51, S. 227 — "class_id 'DDS:Auth:PKI-DH:1.2'
[...] dds.cert.algo One of the CryptoAlgorithmName string identifiers for
digital signature algorithms defined in Table 25."

**Repo:** `crates/security-pki/src/identity_token.rs::crypto_algorithm_name`, `legacy_cert_algo`; `crates/security-pki/src/plugin.rs::validate_remote_identity`

**Tests:** `crates/security-pki/src/identity_token.rs::tests::the_algorithms_are_the_names_of_table_25`,
`crates/security-pki/src/identity_token.rs::tests::the_names_of_1_1_are_the_opt_in`,
`crates/security-pki/src/plugin.rs::tests::the_identity_token_names_the_algorithms_of_table_25`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_compares_the_plugin_name_and_the_major_version`

**Status:** `done` — `dds.cert.algo` und `dds.ca.algo` tragen die Namen von Table 25 (`ECDSA+P256+SHA256`, `ECDSA+P384+SHA384`, `RSASSA-PSS-MGF1SHA256+2048+SHA256`, `RSASSA-PKCS1-V1_5+2048+SHA256`); ein Algorithmus ohne Namen in Table 25 (ecdsa-with-SHA512) lässt die Property weg. Die Namen von 1.1 (`EC-prime256v1`, `RSA-2048`) sendet nur `org.zerodds.sec.algorithm_names=1.1`. Der Vergleich der Klasse betrifft Name und MajorVersion. Gesendet wird `DDS:Auth:PKI-DH:1.0` (der Text von §10.3.2.1 setzt MinorVersion 0, Table 51 nennt 1.2: Widerspruch der Spec; verglichen werden nur Name und MajorVersion).

### §10.3.2.1 class_id: PluginClassName, MajorVersion, MinorVersion

**Spec:** §10.3.2.1, S. 228 — "The value of the class_id shall be interpreted
as composed of three parts: a PluginClassName, a MajorVersion and a
MinorVersion according to the following format:
<PluginClassName>:<MajorVersion>.<MinorVersion>. The PluginClassName is
separated from the MajorVersion by the last ':' character in the class_id."

**Repo:** `crates/security-pki/src/handshake_token.rs::of_class_id` (Z. 88)

**Tests:** `crates/security-pki/src/handshake_token.rs::class_version_tests::the_peers_version_is_read_and_carried_back`

**Status:** done — die Version des Peers wird gelesen und in den Token des
Handshakes zurückgegeben.

### §10.3.2.1 Table 52 — IdentityToken nur mit Pre-Shared Key

**Spec:** §10.3.2.1, Table 52, S. 228 — "class_id 'DDS:Auth:PSK:1.2'
properties [...] dds.psk.algo One of the CryptoAlgorithmName string
identifiers for Symmetric Cipher AEAD and MAC Algorithms".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis; das Token gehört zum Plugin
DDS:Auth:PSK und ist in §10.7.2 geführt.

### §10.3.2.2 Table 53 — IdentityStatusToken

**Spec:** §10.3.2.2, Table 53, S. 228 — "class_id 'DDS:Auth:PKI-DH:1.0'
binary_properties [...] ocsp_stat A DER-encoded OCSP response (using the ASN.1
type OCSPResponse defined in clause 4.2.1 of RFC 2560 [54]) that provides the
status of the identity certificate of the DomainParticipant."

**Repo:** `crates/security-pki/src/plugin.rs::get_identity_status_token`, `identity_status_bytes`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_identity_status_token_carries_the_ocsp_response`,
`crates/security-pki/src/plugin.rs::tests::a_changed_status_token_is_reported_as_identity_status`

**Status:** `done` — Klasse `DDS:Auth:PKI-DH:1.0`, die Binärproperty `ocsp_status` mit der OCSP-Antwort des eigenen Zertifikats, wenn es eine gibt (die Property ist optional). Die eigene Klasse `+IdentityStatus` von `identity_status.rs` bleibt ohne Aufrufer.

### §10.3.2.3 Table 54 — AuthenticatedPeerCredentialToken

**Spec:** §10.3.2.3, Table 54, S. 228 — "class_id 'DDS:Auth:PKI-DH:1.0'
binary_properties [...] c.id Contents of the certificate signed by IdentityCA
that was received from the peer [...] c.perm Contents of the permissions
document signed by the PermissionCA".

**Repo:** `crates/security-pki/src/plugin.rs::get_authenticated_peer_credential_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`

**Status:** `done` — Klasse `DDS:Auth:PKI-DH:1.0`, Binärproperties `c.id` (das Zertifikat des Peers, wie es im Request oder Reply stand) und `c.perm` (dessen Berechtigungsdokument).

### §10.3.2.4 Table 55 — AuthRequestMessageToken

**Spec:** §10.3.2.4, Table 55, S. 229 — "class_id
'DDS:Auth:PKI-DH:1.0+AuthReq' binary_properties [...] future_challenge A
256-bit NONCE generated by the Participant [...] The value shall match what
will be sent on the challenge1 property of the HandshakeRequestMessageToken
or the challenge2 property of the HandshakeReplyMessageToken."

**Repo:** `crates/security-pki/src/handshake_token.rs::build_auth_request_token`, `parse_auth_request_token`; `crates/security-pki/src/plugin.rs::validate_remote_identity`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_local_auth_request_token_is_made_for_a_nil_remote_one_only`,
`crates/security-pki/src/plugin.rs::tests::the_auth_request_announces_the_challenge1_of_the_request`,
`crates/security-pki/src/plugin.rs::tests::a_request_whose_challenge1_is_not_the_announced_one_is_refused`

**Status:** `done` — Klasse `DDS:Auth:PKI-DH:1.0+AuthReq`, Binärproperty `future_challenge` mit 256 Bit; sie ist `challenge1` der Anfrage oder `challenge2` der Antwort. Der TLV-Kodierer `auth_request.rs` ist ersetzt und bleibt ohne Aufrufer.

### §10.3.2.5.1 Table 56 — HandshakeRequestMessageToken

**Spec:** §10.3.2.5.1, Table 56, S. 230 — "class_id
'DDS:Auth:PKI-DH:1.0+Req' binary_properties c.id [...] c.perm [...] c.pdata
The CDR Big Endian Serialization of the ParticipantBuiltinTopicData
c.dsign_algo [...] c.kagree_algo [...] hash_c1 [...] dh1 [...] challenge1 A
256-bit NONCE [...] ocsp_status Inclusion of this property is optional."

**Repo:** `crates/security-pki/src/handshake_token.rs::build_request_token`; `crates/security-pki/src/plugin.rs::begin_handshake_request`

**Tests:** `crates/security-pki/src/plugin.rs::tests::request_token_has_spec_class_id_and_properties`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_names_the_algorithms_as_table_25_and_26_unless_the_peer_reads_1_1`,
`crates/security-pki/src/plugin.rs::tests::the_algorithm_names_follow_the_version_of_the_peer_and_the_setting`,
`crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`

**Status:** `done` — `class_id` und alle Properties; `c.dsign_algo` und `c.kagree_algo` tragen die Namen von Table 25 und 26 (Voreinstellung). Cyclone, Fast DDS und OpenDDS lesen die Namen von 1.1: `org.zerodds.sec.algorithm_names=1.1` sendet sie jedem Peer, `auto` einem Peer der Version 1.0 und einem dieser Hersteller (nach `VendorId`). `ocsp_status` geht mit, wenn die Antwort des Responders vorliegt.

### §10.3.2.5.1 hash_c1 über die Properties c.*

**Spec:** §10.3.2.5.1, Table 56, S. 230 — "hash_c1 SHA-256 hash of the CDR
Big Endian serialization of a BinaryPropertySeq object containing all the
properties above that start with 'c.' placed in the same order as they appear
above."

**Repo:** `crates/security-pki/src/handshake_token.rs::compute_hash_c`
(Z. 206), `serialize_bin_prop_seq` (Z. 259)

**Tests:** `crates/security-pki/src/handshake_token.rs::tests::hash_c_is_deterministic_and_input_separated`,
`crates/security-pki/src/handshake_token.rs::tests::parse_request_token_rejects_tampered_hash_c1`,
`crates/security-pki/src/plugin.rs::tests::hash_c1_mismatch_in_request_rejected`

**Status:** done

### §10.3.2.5.2 Table 57 — HandshakeReplyMessageToken

**Spec:** §10.3.2.5.2, Table 57, S. 231–232 — "class_id
'DDS:Auth:PKI-DH:1.0+Reply' binary_properties c.id [...] c.perm [...]
c.pdata [...] c.dsign_algo [...] c.kagree_algo [...] hash_c2 [...] dh2 [...]
hash_c1 [...] dh1 [...] challenge1 [...] challenge2 [...] ocsp_status [...]
signature".

**Repo:** `crates/security-pki/src/handshake_token.rs::build_reply_token`; `crates/security-pki/src/plugin.rs::begin_handshake_reply`

**Tests:** `crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_names_the_algorithms_as_table_25_and_26_unless_the_peer_reads_1_1`,
`crates/security-pki/src/plugin.rs::tests::a_replier_answers_in_the_names_of_the_request`,
`crates/discovery/src/security/stack.rs::tests::full_pki_handshake_between_two_stacks_yields_shared_secret`

**Status:** `done` — vollständig; der Replier antwortet in den Namen der Anfrage (`c.dsign_algo`, `c.kagree_algo`), sodass jeder Hersteller seine Schreibweise zurückbekommt.

### §10.3.2.5.2 Signatur der Antwort

**Spec:** §10.3.2.5.2, Table 57, S. 232 — "signature The Digital Signature of
the CDR Big Endian serialization of a BinaryPropertySeq object containing the
properties: hash_c2, challenge2, dh2, challenge1, dh1, and hash_c1, placed in
that order. [...] The digital signature shall be computed using the Private
Key associated with the DomainParticipant".

**Repo:** `crates/security-pki/src/handshake_token.rs::reply_signing_bytes`
(Z. 289); `crates/security-pki/src/plugin.rs::sign_with_identity` (Z. 566)

**Tests:** `crates/security-pki/src/plugin.rs::tests::signature_tamper_rejected_by_initiator`,
`crates/security-pki/src/plugin.rs::tests::dh_tamper_in_reply_breaks_final_signature`,
`crates/security-pki/src/handshake_token.rs::tests::reply_and_final_content_differ_by_order`

**Status:** done

### §10.3.2.5.2 SharedSecret als SHA-256 des Ergebnisses der Vereinbarung

**Spec:** §10.3.2.5.2, S. 232 — "Regardless of the key agreement algorithm,
the SharedSecret [...] shall be computed as the SHA256 hash of the derived
shared secret computed by the key agreement algorithm."

**Repo:** `crates/security-pki/src/plugin.rs::derive_shared_secret` (Z. 701)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_shared_secret_returns_stored_value`,
`crates/security-pki/tests/e2e_handshake.rs::full_three_stage_handshake`

**Status:** done

### §10.3.2.5.3 Table 58 — HandshakeFinalMessageToken

**Spec:** §10.3.2.5.3, Table 58, S. 232–233 — "class_id
'DDS:Auth:PKI-DH:1.0+Final'. binary_properties hash_c1 [...] hash_c2 [...]
dh1 [...] dh2 [...] challenge1 [...] challenge2 [...] signature The Digital
Signature of the CDR Big Endian serialization of a BinaryPropertySeq object
containing the properties: hash_c1, challenge1, dh1, challenge2, dh2, and
hash_c2, placed in that order."

**Repo:** `crates/security-pki/src/handshake_token.rs::build_final_token`
(Z. 690), `final_signing_bytes` (Z. 313)

**Tests:** `crates/security-pki/src/plugin.rs::tests::final_token_challenge1_tamper_rejected`,
`crates/security-pki/src/plugin.rs::tests::final_token_dh2_tamper_rejected`,
`crates/security-pki/src/plugin.rs::tests::final_token_hash_c2_tamper_rejected`

**Status:** done

### §10.3.2.5.3 Der Initiator bestimmt die Schlüsselvereinbarung

**Spec:** §10.3.2.5.3, S. 233 — "The parameters and algorithm shall be
determined based on the value of the HandshakeRequestMessageToken parameter
with key c.kagree_algo. In other words, it is the Participant that creates
the HandshakeRequestMessageToken the one that controls the key agreement
algorithm used."

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_reply`
(Z. 900, `kx_suite_for_algo(&req.kagree_algo)`)

**Tests:** `crates/security-pki/src/plugin.rs::tests::kagree_algo_string_round_trips_for_every_supported_suite`,
`crates/security-pki/tests/e2e_handshake.rs::handshake_with_different_cert_dh_combos`

**Status:** done

### §10.3.3 Table 59 — validate_local_identity: Prüfung von Zertifikat, CRL und OCSP

**Spec:** §10.3.3, Table 59, S. 233 — "The operation shall verify the
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

**Status:** `done` — Kette, Gültigkeit, CRL (`dds.sec.auth.crl`) und OCSP: der Responder aus der Authority Information Access wird über `http://` oder `https://` befragt (RFC 6960 Anhang A nennt kein Schema; TLS mit rustls, das Zertifikat des Servers gegen die Identity-CA, Feature `ocsp-https`, an in der Vorgabe); `good` geht als `ocsp_status` mit. Kann der Status nicht bestimmt werden, wird das Zertifikat abgewiesen, außer mit `zerodds.sec.auth.ocsp_unreachable=accept` (Warnung im Security-Log). Die Abfrage blockiert den Aufruf nicht: VALIDATION_PENDING_RETRY (Thread). Ein Zertifikat ohne Responder und ohne CRL wird nicht auf Widerruf geprüft (die Spec verlangt „CRL and/or OCSP“; es gibt nichts zu befragen). Kein Zwischenspeicher der Antworten. Gemessen gegen einen fremden Responder: nicht.

### §10.3.3 Table 59 — CRL und OCSP werden nicht befragt

**Spec:** §10.3.3, Table 59, S. 233, 236–238 — "The operation shall check a
CRL and/or an OCSP (RFC 2560 [54]) responder. [...] If the property
ocsp_status is present, the operation shall verify that the OCSP response
included in the property corresponds to the identity in the c.id property.
[...] If the status is revoked, the operation shall fail".

**Repo:** `crates/security-pki/src/ocsp.rs::verify_staple`, `check_identity`, `query_responder`; `crates/security-pki/src/identity.rs::verify_cert_chain_checked`

**Tests:** `crates/security-pki/src/ocsp.rs::tests::a_signed_response_for_the_certificate_gives_its_status`, `::a_response_that_does_not_hold_says_nothing`, `::a_delegated_responder_signs_only_with_the_ocsp_signing_purpose`, `::a_stapled_status_decides_and_a_useless_one_is_none`, `::the_responder_in_the_certificate_is_asked`, `::an_unreachable_responder_fails_unless_the_setting_says_accept`, `crates/security-runtime/src/qos_properties.rs::tests::the_ocsp_policy_refuses_by_default_and_reads_its_settings`

**Status:** `done` — `ocsp_status` eines Request- oder Reply-Tokens wird ausgewertet (CertID, Gültigkeitsintervall, Signatur der CA oder eines für OCSP-Signatur ausgestellten Responder-Zertifikats): `good` ist der Nachweis, `revoked` bricht ab, eine Antwort, die nicht gilt, zählt als nicht vorhanden. `crl.rs` bleibt ohne Aufrufer; die CRLs prüft `rustls-webpki`.

### §10.3.3 Table 59 — adjusted_participant_guid

**Spec:** §10.3.3, Table 59, S. 233–234 — "The first bit (bit 0) shall be
set to 1. The 47 bits following the first bit (bits 1 to 47) shall be set to
the 47 first bits of the SHA-256 hash of the ASN.1 DER encoding of the
SubjectName [40] appearing on the identity_credential. The following 48 bits
(bits 48 to 95) shall be set to the first 48 bits of the SHA-256 hash of the
candidate_participant_guid."

**Repo:** `crates/security-pki/src/guid_adjust.rs::adjust_participant_guid_prefix`
(Z. 28)

**Tests:** `crates/security-pki/src/guid_adjust.rs::tests::adjusted_prefix_sets_high_bit_and_is_deterministic_per_identity`,
`crates/dcps/tests/security_guid_wiring_e2e.rs::with_security_bundle_wires_the_identity_adjusted_prefix_not_random`

**Status:** done — die Berechnung läuft in
`crates/security-runtime/src/profile.rs` (Z. 319), nicht in
`validate_local_identity` (siehe §9.3.2.11.2).

### §10.3.3 Table 59 — get_identity_token

**Spec:** §10.3.3, Table 59, S. 234 — "If the above condition is not met, the
operation shall return the exception DDS_SecurityException_PreconditionError.
This operation shall return an IdentityToken object with the content
specified in 10.3.2.1."

**Repo:** `crates/security-pki/src/plugin.rs::get_identity_token` (Z. 791)

**Tests:** `crates/security-pki/src/plugin.rs::tests::get_identity_token_returns_decodable_spec_descriptor`

**Status:** done — ein unbekanntes Handle ergibt `BadArgument`; den Inhalt
siehe Table 51.

### §10.3.3 Table 59 — get_identity_status_token

**Spec:** §10.3.3, Table 59, S. 234 — "This operation shall return an
IdentityToken object with the content specified in 10.3.2.2."

**Repo:** `crates/security-pki/src/plugin.rs::get_identity_status_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_identity_status_token_carries_the_ocsp_response`

**Status:** `done` — siehe Table 53.

### §10.3.3 Table 59 — set_participant_security_config

**Spec:** §10.3.3, Table 59, S. 234 — "Check that the
digital_signature.trust_chain.supported_mask includes at least one algorithm
that is supported by the AUTH plugin. Fail otherwise. [...] The operation
shall configure the AUTH plugin to only accept the resulting set of supported
algorithms in the adjusted_algorithm_info."

**Repo:** `crates/security-pki/src/plugin.rs::set_participant_security_config`

**Tests:** `crates/security-pki/src/plugin.rs::tests::set_participant_security_config_adjusts_a_copy_of_the_given_info`,
`crates/security-pki/src/plugin.rs::tests::set_participant_security_config_fails_without_an_algorithm_of_the_plugin`,
`crates/security-pki/src/plugin.rs::tests::set_participant_security_config_fails_when_a_used_algorithm_is_not_supported`,
`crates/security-pki/src/plugin.rs::tests::the_configured_algorithms_bind_the_handshake`

**Status:** `done` — die drei Prüfungen auf mindestens einen unterstützten Algorithmus, die Anpassung als Kopie (nicht unterstützte Bits aus den supported-Masken, Zertifikat, Schlüssel und Verfahren des Initiators in die required-Masken), die beiden Prüfungen „present in supported_mask“ und die Beschränkung des Handshakes auf die angepassten Masken.

### §10.3.3 Table 59 — set_permissions_credential_and_token

**Spec:** §10.3.3, Table 59, S. 234 — "This operation shall store the
PermissionsCredentialToken and the PermissionsToken internally to the plugin
and associate them with the DomainParticipant represented by the
IdentityHandle."

**Repo:** `crates/security-pki/src/plugin.rs::set_permissions_credential_and_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_permissions_credential_is_what_the_handshake_sends_as_c_perm`,
`crates/security-pki/src/plugin.rs::tests::a_permissions_credential_that_is_none_is_refused`

**Status:** `done` — Credential-Token und PermissionsToken werden je Identität gespeichert; `set_local_permissions` bleibt als Weg für Aufrufer ohne Token.

### §10.3.3 Table 59 — validate_remote_identity: Vergleich des class_id

**Spec:** §10.3.3, Table 59, S. 235 — "The operation shall compare the
class_id of the local identity_token with that of the remote_identity_token.
If the PluginClassName or the MajorVersion are different, it shall return
VALIDATION_FAILED."

**Repo:** `crates/security-pki/src/plugin.rs::validate_remote_identity`

**Tests:** `crates/security-pki/src/plugin.rs::tests::validate_remote_identity_compares_the_plugin_name_and_the_major_version`,
`crates/security-pki/src/plugin.rs::tests::validate_remote_identity_takes_only_a_pki_dh_identity_token`

**Status:** `done` — Name und MajorVersion des IdentityToken werden mit denen des eigenen verglichen (`DDS:Auth:PKI-DH`, 1); `DDS:Auth:PKI-DHX:1.0`, 2.0 und ein Token, das kein DataHolder ist, sind VALIDATION_FAILED; jede MinorVersion ist gleich.

### §10.3.3 Table 59 — validate_remote_identity: Reihenfolge der GUIDs

**Spec:** §10.3.3, Table 59, S. 235 — "If the remote_participant_guid >
local_participant_guid, the operation shall return
VALIDATION_PENDING_HANDSHAKE_REQUEST. If the remote_participant_guid <
local_participant_guid, the operation shall return
VALIDATION_PENDING_HANDSHAKE_MESSAGE."

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`
(Z. 447)

**Tests:** `crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`

**Status:** done — entschieden im Stack, nicht im Plugin (siehe §9.3.2.11.3).

### §10.3.3 Table 59 — validate_remote_identity und begin_handshake_*: AuthRequestMessageToken

**Spec:** §10.3.3, Table 59, S. 235–236 — "If the remote_auth_request_token
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

**Status:** `done` — ohne `remote_auth_request_token` erzeugt `validate_remote_identity` den eigenen, dessen `future_challenge` `challenge1` (Initiator) oder `challenge2` (Replier) wird; mit einem prüft `begin_handshake_reply` (und `process_handshake` beim Initiator), dass er gleich `challenge1` (`challenge2`) ist.

### §10.3.3 Table 59 — begin_handshake_request

**Spec:** §10.3.3, Table 59, S. 235 — "The operation shall return the
handshake_message containing a HandshakeRequestMessageToken object with
contents as defined in 10.3.2.5.1 [...] The operation shall return
VALIDATION_PENDING_HANDSHAKE_MESSAGE."

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_request`
(Z. 828)

**Tests:** `crates/security-pki/src/plugin.rs::tests::request_token_has_spec_class_id_and_properties`

**Status:** done — Inhalt siehe Table 56.

### §10.3.3 Table 59 — begin_handshake_reply: Prüfung von c.id

**Spec:** §10.3.3, Table 59, S. 236 — "The operation shall verify the
validity of the IdentityCredential contained in the property named 'c.id'
found in the handshake_message_in HandshakeRequestMessageToken. This
verification shall be done using the locally configured CA in the same
manner as the validate_local_identity operation."

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_reply`
(Z. 900, `verify_remote_der_checked`); `crates/security-pki/src/identity.rs::verify_remote_der_checked`
(Z. 217)

**Tests:** `crates/security-pki/src/plugin.rs::tests::wrong_ca_initiator_rejected_by_replier`,
`crates/security-pki/src/plugin.rs::tests::cross_algorithm_dsign_mismatch_rejected`

**Status:** done — CRL und OCSP siehe oben (`open`).

### §10.3.3 Table 59 — begin_handshake_reply: GUID in c.pdata gegen SubjectName

**Spec:** §10.3.3, Table 59, S. 237 — "The operation shall verify that the
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

**Status:** `done` — Bit 0 und die folgenden 47 Bits werden geprüft, wie Cyclone
(`validate_pdata`) und Fast DDS; `c.pdata` wird als big-endian Parameterliste ab
dem ersten Oktett gelesen.

### §10.3.3 Table 59 — begin_handshake_reply: Ergebnis und gespeicherter Zustand

**Spec:** §10.3.3, Table 59, S. 237 — "The operation shall fill the
handshake_message_out with a HandshakeReplyMessageToken object [...] The
operation shall fill the handshake_handle with an implementation-dependent
reference [...] The operation shall return
VALIDATION_PENDING_HANDSHAKE_MESSAGE."

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_reply`, `process_final_on_replier`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`,
`crates/security-pki/src/plugin.rs::tests::the_handshake_returns_the_validation_results_of_table_59`,
`crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`

**Status:** `done` — Antwort und Zustand stimmen; das Geheimnis ist erst nach dem geprüften Final abrufbar (VALIDATION_PENDING_HANDSHAKE_MESSAGE bis dahin).

### §10.3.3 Table 59 — get_shared_secret erst nach VALIDATION_OK

**Spec:** §10.3.3, Table 59, S. 238 — "get_shared_secret This operation shall
be called with the handshake_handle that was previously used to call either
process_handshake and for which the aforementioned operation returned
VALIDATION_OK_FINAL_MESSAGE or VALIDATION_OK. If the above conditon is not
met, the operation shall return the exception
DDS_SecurityException_PreconditionError."

**Repo:** `crates/security-pki/src/plugin.rs` (`begin_handshake_reply`,
`process_final_on_replier`, `shared_secret`)

**Tests:**
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`

**Status:** `done` — der Replier trägt das Geheimnis erst nach den Prüfungen des
Final für den Handshake ein; ein fehlgeschlagener Final entfernt es.

### §10.3.3 Table 59 — process_handshake auf dem Initiator

**Spec:** §10.3.3, Table 59, S. 237–238 — "The operation shall verify that the
contents of the handshake_message_in correspond to a
HandshakeReplyMessageToken [...] The operation shall check that the
challenge1 matches the one that was sent on the
HandshakeRequestMessageToken. The operation shall validate the digital
signature in the 'signature' property [...] On success the operation shall
return VALIDATION_OK_FINAL_MESSAGE."

**Repo:** `crates/security-pki/src/plugin.rs::process_reply_on_initiator`
(Z. 1077)

**Tests:** `crates/security-pki/src/plugin.rs::tests::signature_tamper_rejected_by_initiator`,
`crates/security-pki/src/plugin.rs::tests::cert_bind_replier_modifies_initiator_cert_in_reply_rejected`

**Status:** done — OCSP siehe oben (`open`).

### §10.3.3 Table 59 — process_handshake auf dem Replier

**Spec:** §10.3.3, Table 59, S. 238 — "The operation shall check that the
challenge1 and challenge2 match the ones that were sent on the
HandshakeReplyMessageToken. The operation shall validate the digital
signature in the 'signature' property [...] On success the operation shall
return VALIDATION_OK."

**Repo:** `crates/security-pki/src/plugin.rs::process_final_on_replier`
(Z. 1219)

**Tests:** `crates/security-pki/src/plugin.rs::tests::final_token_challenge2_tamper_rejected`,
`crates/security-pki/src/plugin.rs::tests::full_three_round_handshake_alice_bob`

**Status:** done — die Prüfung selbst ist vorhanden; ihr Ergebnis kommt zu
spät (siehe „get_shared_secret erst nach VALIDATION_OK“).

### §10.3.3 Table 59 — Wiederholte Anfrage abweisen

**Spec:** §10.3.3, Table 59, S. 237 — "begin_handshake_reply [...] The
operation shall fill the handshake_handle with an implementation-dependent
reference that allows the implementation to retrieve at least the following
information [...] The value attribute of the challenge_message returned".

**Repo:** `crates/security-pki/src/plugin.rs::record_challenge` (Z. 432)

**Tests:** `crates/security-pki/src/plugin.rs::tests::replay_initiator_request_rejected_second_time`,
`crates/security-pki/src/plugin.rs::tests::replay_cache_evicts_oldest_at_cap_plus_one`

**Status:** done — ZeroDDS weist ein bereits gesehenes `challenge1` ab; die
Spec fordert das nicht ausdrücklich.

### §10.3.3 Table 59 — get_authenticated_peer_credential_token

**Spec:** §10.3.3, Table 59, S. 239 — "The operation shall return the
AuthenticatedPeerCredentialToken of the peer DomainParticipant associated
with the handshake_handle."

**Repo:** `crates/security-pki/src/plugin.rs::get_authenticated_peer_credential_token`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_authenticated_peer_credential_token_holds_c_id_and_c_perm`

**Status:** `done` — siehe Table 54.

### §10.3.3 Table 59 — set_listener und return_*

**Spec:** §10.3.3, Table 59, S. 239 — "set_listener This operation shall save
a reference to the listener object [...] return_identity_token This operation
shall behave as specified in 9.3.2.11.14." (entsprechend für die übrigen
return_*)

**Repo:** `crates/security-pki/src/plugin.rs::set_listener`, `return_identity_token`, `return_identity_status_token`, `return_authenticated_peer_credential_token`, `return_handshake_handle`, `return_identity_handle`, `return_sharedsecret_handle`

**Tests:** `crates/security-pki/src/plugin.rs::tests::the_return_operations_take_back_what_was_issued`,
`crates/security-pki/src/plugin.rs::tests::revoking_an_identity_tells_the_listener_and_invalidates_the_handle`,
`crates/security-pki/src/plugin.rs::tests::a_returned_identity_handle_is_disposed_of`

**Status:** `done` — alle sieben verhalten sich wie in §9.3.2.11.13 bis §9.3.2.11.19.

### §10.3.4 Der Participant mit der kleineren GUID beginnt

**Spec:** §10.3.4, S. 239 — "To determine which of the two DomainParticipant
entities shall initiate the protocol, each DomainParticipant compares its own
GUID with that of the other DomainParticipant. The DomainParticipant with the
lower GUID (using lexicographical order) initiates the protocol."

**Repo:** `crates/discovery/src/security/stack.rs::begin_handshake_with`
(Z. 447)

**Tests:** `crates/discovery/src/security/stack.rs::tests::smaller_local_guid_initiates_handshake_cyclone_compatible`

**Status:** done

### §10.3.4.1 Table 60 und Table 61 — Begriffe und Notation

**Spec:** §10.3.4.1, Tables 60–61, S. 240–241 — Participant1, Participant2,
PubK, PrivK, Cert, Perm, Pdata, Dsign_algo, Kagree_algo, Challenge, DH,
DHSharedSecret, SharedSecret, C1, C2; Sign(), Hash(), Verkettung.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffe; die Regeln stehen in Table 56
bis 59.

### §10.3.4.2 Table 62 — Ablauf des Protokolls in drei Nachrichten

**Spec:** §10.3.4.2, Table 62, S. 241–242 — Anfrage (C1, Hash(C1),
Challenge1, DH1); Antwort mit Prüfung von Cert1 und Hash(C1) und
Sign(Hash(C2) | Challenge2 | DH2 | Challenge1 | DH1 | Hash(C1)); Final mit
Prüfung von Cert2 und der Signatur und Sign(Hash(C1) | Challenge1 | DH1 |
Challenge2 | DH2 | Hash(C2)); beide Seiten berechnen das gemeinsame
Geheimnis.

**Repo:** `crates/security-pki/src/plugin.rs::begin_handshake_request`,
`begin_handshake_reply`, `process_reply_on_initiator`,
`process_final_on_replier`

**Tests:**
`crates/security-pki/tests/e2e_handshake.rs::full_three_stage_handshake`,
`crates/security-pki/tests/e2e_handshake.rs::tampered_request_token_fails_hash_check`,
`crates/security-pki/tests/e2e_handshake.rs::replay_attack_with_swapped_challenges_detected`,
`crates/security-pki/src/plugin.rs::tests::the_replier_has_no_shared_secret_before_a_verified_final`

**Status:** `done` — die drei Nachrichten mit ihren Prüfungen; das Geheimnis
steht dem Replier erst nach dem geprüften Final zur Verfügung.

---

### §10.4 Eine Instanz von DDS:Access:Permissions je DomainParticipant

**Spec:** §10.4, S. 242 — "The DDS:Access:Permissions implements the
AccessControl plugin API using a permissions document signed by a shared
Certificate Authority (CA). [...] Each DomainParticipant has an associated
instance of the DDS:Access:Permissions plugin."

**Repo:** `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`
(Z. 301, Governance und Permissions je Profil)

**Tests:** `crates/dcps/tests/security_guid_wiring_e2e.rs::secured_participants_on_public_rust_api_use_identity_adjusted_guids_and_discover`

**Status:** done — die Schnittstelle des Plugins wird nicht verwendet (siehe
§9.4.2).

### §10.4.1 Drei Dokumente über die PropertyQosPolicy

**Spec:** §10.4.1, S. 242 — "The DDS:Access:Permissions plugin is configured
with three documents: 1. The Permissions CA certificate 2. The Domain
governance signed by the Permissions CA 3. The DomainParticipant permissions
signed by the Permissions CA. The configuration of the builtin access control
plugin shall be done using the PropertyQosPolicy of the
DomainParticipantQos."

**Repo:** `crates/security-runtime/src/qos_properties.rs::profile_config_from_properties`
(Z. 380), Konstanten `PROP_PERMISSIONS_CA`, `PROP_GOVERNANCE`,
`PROP_PERMISSIONS` (Z. 91–95)

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_property_names_are_the_spec_strings`,
`crates/security-runtime/src/qos_properties.rs::tests::the_auth_and_access_namespaces_request_security_and_the_log_one_does_not`

**Status:** done

### §10.4.1 Table 63 — permissions_ca

**Spec:** §10.4.1, Table 63, S. 243 — "permissions_ca URI to a X509
certificate for the PermissionsCA in PEM format. Supported URI schemes: file,
data, pkcs11".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`
(Z. 302)

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::every_file_uri_form_resolves_to_a_path`,
`crates/security-runtime/src/qos_properties.rs::tests::a_data_uri_is_the_content_and_not_a_path`

**Status:** done — `pkcs11:` wie bei Table 50 nur mit dem Feature `pkcs11`.

### §10.4.1 Table 63 — governance

**Spec:** §10.4.1, Table 63, S. 243–244 — "governance URI to the shared
Governance Document signed by the Permissions CA in S/MIME format URI
schemes: file, data".

**Repo:** `crates/security-runtime/src/qos_properties.rs::resolve_material`
(Z. 302); `crates/security-permissions/src/cms.rs::CmsPkcs7Verifier`
(Z. 117)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::happy_path_multipart_signed_passes`,
`crates/dcps/tests/security_qos_properties.rs::a_complete_dds_sec_property_list_creates_a_secured_participant`

**Status:** done

### §10.4.1 Table 63 — permissions

**Spec:** §10.4.1, Table 63, S. 244 — "permissions URI to the
DomainParticipant permissions document signed by the Permissions CA in
S/MIME format URI schemes: file, data".

**Repo:** `crates/security-permissions/src/signature.rs::open_signed_permissions`
(Z. 89)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::happy_path_then_open_signed_permissions_parses`

**Status:** done

### §10.4.1.1 Permissions CA Certificate

**Spec:** §10.4.1.1, S. 244 — "The certificate can be self-signed or signed by
some other CA. Regardless of this the Public Key in the Certificate shall be
trusted to sign the aforementioned Governance and Permissions documents".

**Repo:** `crates/security-permissions/src/cms.rs::CmsPkcs7Verifier`
(Z. 117)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::intermediate_ca_chain_is_accepted`,
`crates/security-permissions/tests/cms_verify.rs::wrong_ca_signature_is_rejected`

**Status:** done

### §10.4.1.2 Governance signiert im Format S/MIME 3.2

**Spec:** §10.4.1.2, S. 244 — "The domain governance document shall be signed
by the Permissions CA. The signed document shall use S/MIME version 3.2
format as defined in IETF RFC 5761 using SignedData Content Type [...]
formatted as multipart/signed [...] Additionally the signer certificate shall
be included within the signature."

**Repo:** `crates/security-permissions/src/cms.rs::CmsPkcs7Verifier`
(Z. 117); `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`
(Z. 301)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::happy_path_multipart_signed_passes`,
`crates/security-permissions/tests/cms_verify.rs::tampered_xml_is_rejected`,
`crates/security-permissions/tests/cms_verify.rs::expired_signer_is_rejected`

**Status:** done — zusätzlich angenommen werden
`application/pkcs7-mime; smime-type=signed-data` und PKCS#7 in PEM. Die
Moduldoku von `crates/security-permissions/src/governance.rs` (Z. 24) führt
die Prüfung der Signatur als nicht umgesetzt; die Prüfung liegt in
`crates/security-runtime/src/profile.rs`.

### §10.4.1.2 Inhalt der Governance

**Spec:** §10.4.1.2, S. 245 — "The governance document specifies which DDS
domain IDs shall be protected and the details of the protection."
(Aufzählung der Aspekte je Domain und je Topic)

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Elemente sind unten einzeln
geführt.

### §10.4.1.2.1 Basic Protection Kinds: NONE, SIGN, ENCRYPT

**Spec:** §10.4.1.2.1, S. 245–246 — "SIGN indicates the cryptographic
transformation shall be purely a message authentication code (MAC) [...]
transformation_algorithm_id set to the CryptoAlgorithmId that corresponds to
the selected MAC algorithm (e.g. CID_AES256_GMAC) [...] ENCRYPT indicates the
cryptographic transformation shall be an encryption followed by a message
authentication code (MAC) computed on the ciphertext".

**Repo:** `crates/security-runtime/src/gate_crypto.rs::register_endpoint`; `crates/security-crypto/src/plugin/spec.rs`

**Tests:** `crates/security-runtime/src/shared.rs::tests::the_basic_protection_kind_of_an_endpoint_decides_gmac_or_gcm`, `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** `done` — SIGN gibt je Endpunkt GMAC (AES256_GMAC 0x03, AES128_GMAC 0x01), ENCRYPT GCM (0x04, 0x02), die Breite folgt `symmetric_cipher_algorithm`. Lauf `cargo test -p zerodds-security-runtime --lib` und `-p zerodds-dcps --features security --lib`: grün, Logs `/root/zerodds-rc7/agent-logs/sec-crypto-1018-085015.log`, `…085119.log`.

### §10.4.1.2.1 Algorithmus der Endpunkte je Topic

**Spec:** §10.4.1.2.6.6, §10.4.1.2.6.7, S. 260–261 — "The setting of this
element shall specify the protection kind (see 10.4.1.2.2) used for the RTPS
SubMessages sent by any DataWriter and DataReader whose associated Topic name
matches the rule's topic expression."

**Repo:** `crates/security-runtime/src/shared.rs::register_local_endpoint_with_kinds`; `crates/security-crypto/src/plugin.rs::register_local_endpoint`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_key_material_of_an_endpoint_follows_its_protection_kinds`, `crates/security-runtime/src/shared.rs::tests::the_protection_kinds_of_an_endpoint_come_from_its_topic_rule`, `crates/dcps/src/runtime.rs::tests::a_writer_gets_the_key_material_of_its_topic_rule`

**Status:** `done` — ein Endpunkt der Anwendung bekommt sein Schlüsselmaterial beim Anlegen für die Arten der Regel seines Topics, ein sicherer eingebauter für die Discovery- bzw. Liveliness-Art der Domain.

### §10.4.1.2.2 Protection Kinds mit Origin Authentication

**Spec:** §10.4.1.2.2, S. 246 — "SIGN_WITH_ORIGIN_AUTHENTICATION [...] In
addition, the cryptographic transformation shall create additional
authentication codes, each produced with a different secret key. [...]
ENCRYPT_WITH_ORIGIN_AUTHENTICATION indicates the cryptographic transformation
shall be an encryption followed by a message authentication code (MAC)
computed on the ciphertext, followed by additional authentication codes."

**Repo:** `crates/security-permissions/src/governance.rs::ProtectionKind`; `crates/security-runtime/src/shared.rs::metadata_origin_authenticated`, `discovery_origin_authenticated`, `liveliness_origin_authenticated`, `rtps_origin_authenticated`; `crates/security-runtime/src/policy.rs::from_protection_kind` (für die Stufe SIGN/ENCRYPT)

**Tests:** `crates/security-runtime/src/shared.rs::tests::origin_authentication_follows_the_governance_kinds`, `crates/security-crypto/src/plugin.rs::tests::a_reader_checks_the_origin_of_a_submessage_by_its_own_key`

**Status:** `done` — die vier Arten werden gelesen; `ProtectionLevel` fasst sie weiter auf SIGN und ENCRYPT zusammen, die Origin Authentication kommt aus den vier Abfragen der Schleuse und geht in die Schlüssel (`register_local_endpoint` mit der Art als Property, `register_local_participant` mit `zerodds.crypto.rtps_origin_authentication`).

### §10.4.1.2.2 Zusätzliche MACs für Origin Authentication

**Spec:** §10.4.1.2.2, S. 246 — "Each of these additional secret keys shall be
shared only with a subset of the receivers. In the limit case each secret key
is shared with only one receiver. The additional MACs prove to the receiver
that the sender originated the message".

**Repo:** `crates/security-crypto/src/plugin.rs::footer_tail`, `receiver_key_for`, `create_endpoint_crypto_token_for`, `create_local_participant_crypto_tokens`; `crates/dcps/src/runtime.rs::prepare_endpoint_crypto_tokens`, `prepare_crypto_token`

**Tests:** `crates/security-runtime/src/shared.rs::tests::origin_authentication_follows_the_governance_kinds`, `crates/security-crypto/src/plugin.rs::tests::a_reader_checks_the_origin_of_a_submessage_by_its_own_key`, `crates/security-crypto/src/plugin.rs::tests::the_receiver_specific_mac_is_the_one_of_the_spec`

**Status:** `done` — jeder entfernte Endpunkt (Participant) bekommt einen eigenen Schlüssel, im Token an ihn; die Footer der Submessages (RTPS-Nachrichten) tragen das MAC für jeden. Die Nutzlast bleibt ohne MACs (Table 80). Gemessen: nichts; die Matrix des Linux-Hosts hat kein Profil mit `*_WITH_ORIGIN_AUTHENTICATION`. Die Funktionen `encrypt_submessage_multi`, `transform_outbound_group` und ihre Verwandten (HMAC-SHA256 über den Chiffretext, kein Mechanismus der Spec) sind entfernt.

### §10.4.1.2.3 XSD der Governance: Aufbau dds, domain_access_rules, domain_rule

**Spec:** §10.4.1.2.3, §10.4.1.2.4, S. 246–252 — "<xs:element name='dds'
type='DomainAccessRulesNode' /> [...] domain_access_rules [...] domain_rule
[...] The XML domain governance document is delimited by the <dds> XML
element tag and contains a single domain access rules Section".

**Repo:** `crates/security-permissions/src/governance.rs::parse_governance_xml`, `parse_dds_root`, `parse_domain_rule`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_structure_of_the_document_is_the_xsds`,
`crates/security-permissions/src/governance.rs::tests::a_governance_in_the_omg_namespace_parses`,
`crates/security-permissions/src/governance.rs::tests::a_rule_without_a_domain_is_refused`,
`crates/security-permissions/src/governance.rs::tests::missing_domain_rule_flags_are_true`

**Status:** `done` — die Wurzel `<dds>` hält genau eine `<domain_access_rules>`; Elemente außerhalb der XSD folgen `must_interpret`; der Namensraum der OMG-Beispieldateien wird angenommen; fehlende `allow_unauthenticated_participants` und `enable_join_access_control` gelten als TRUE (Table 70).

### §10.4.1.2.3 XSD: enable_key_revision, rtps_psk_protection_kind, allowed_crypto_algorithms

**Spec:** §10.4.1.2.3, S. 247 — "<xs:element name='enable_key_revision'
type='xs:boolean' /> [...] <xs:element name='rtps_psk_protection_kind'
type='BasicProtectionKind' /> [...] <xs:element
name='allowed_crypto_algorithms' type='AllowedCryptoAlgorithms'
minOccurs='0'/>".

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_rule`, `AllowedCryptoAlgorithms`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_key_revision_psk_and_allowed_algorithm_elements_are_read`,
`crates/security-permissions/src/governance.rs::tests::the_allowed_algorithm_names_are_those_of_the_xsd`

**Status:** `done` — die drei Elemente werden gelesen, geprüft und im Plugin ausgewertet (`is_key_revision_enabled`, `is_rtps_psk_protected`, `algorithm_info`).

### §10.4.1.2.3 XSD: Werte von ProtectionKind und BasicProtectionKind

**Spec:** §10.4.1.2.3, S. 249 — "ProtectionKind [...]
ENCRYPT_WITH_ORIGIN_AUTHENTICATION, SIGN_WITH_ORIGIN_AUTHENTICATION, ENCRYPT,
SIGN, NONE [...] BasicProtectionKind [...] ENCRYPT, SIGN, NONE".

**Repo:** `crates/security-permissions/src/governance.rs` (`ProtectionKind::parse_xsd`, `basic_of`)

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_protection_kind_enumerations_of_the_xsd_are_exact`,
`crates/security-permissions/src/governance.rs::tests::protection_kind_parses_case_insensitive`

**Status:** `done` — ein Wert außerhalb der fünf macht das Dokument ungültig; Basic-Elemente (`data_protection_kind`, `rtps_psk_protection_kind`) nehmen nur NONE, SIGN, ENCRYPT; die Werte sind genau die der XSD in ihrer Schreibweise.

### §10.4.1.2.3 XSD: Werte von xs:boolean

**Spec:** §10.4.1.2.3, S. 247, 249 — "type='xs:boolean'" für
`allow_unauthenticated_participants`, `enable_join_access_control`,
`enable_*_protection`, `enable_*_access_control`.

**Repo:** `crates/security-permissions/src/governance.rs::parse_bool`

**Tests:**
`crates/security-permissions/src/governance.rs::tests::a_governance_outside_the_xsd_is_refused`

**Status:** `done` — `true`, `false`, `1`, `0` (auch groß geschrieben); jeder
andere Text macht das Dokument ungültig.

### §10.4.1.2.5 Domain Rules: Reihenfolge, erste passende Regel

**Spec:** §10.4.1.2.5, S. 252 — "The domain rules shall be evaluated in the
same order as they appear in the document. A rule only applies to a
particular DomainParticipant if the domain Section matches the DDS domain_id
to which the DomainParticipant belongs. If multiple rules match, the first
rule that matches is the only one that applies."

**Repo:** `crates/security-permissions/src/governance.rs::find_domain_rule`
(Z. 322)

**Tests:** `crates/security-permissions/src/governance.rs::tests::domain_filter_id_range_matches_inclusive`,
`crates/security-permissions/src/governance.rs::tests::an_uncovered_domain_creates_nothing`

**Status:** done

### §10.4.1.2.5.1 Domains element: id und id_range

**Spec:** §10.4.1.2.5.1, S. 252–253 — "DDS domains are identified by their
DomainId (an integer) and a DomainTag (a string). One or more DomainId values
(or ranges) must always be specified."

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_filter`

**Tests:**
`crates/security-permissions/src/governance.rs::tests::parses_domain_rule_with_ranges`,
`crates/security-permissions/src/governance.rs::tests::domain_filter_id_range_matches_inclusive`,
`crates/security-permissions/src/governance.rs::tests::a_rule_without_a_domain_is_refused`

**Status:** `done` — `<id>` und `<id_range>` als nicht-negative ganze Zahlen;
ein unlesbarer Wert, ein `<domains>` ohne Domain und eine Regel ohne `<domains>`
machen das Dokument ungültig.

### §10.4.1.2.5.1 Domains element: leere Menge gilt für alle Domains

**Spec:** §10.4.1.2.5.1, S. 252 — "One or more DomainId values (or ranges)
must always be specified."

**Repo:** `crates/security-permissions/src/governance.rs`
(`parse_domain_filter`, `parse_domain_rule`)

**Tests:**
`crates/security-permissions/src/governance.rs::tests::a_rule_without_a_domain_is_refused`

**Status:** `done` — eine leere oder fehlende Menge wird abgewiesen statt für
alle Domains gelesen.

### §10.4.1.2.5.1 Domains element: tag und tag_expression

**Spec:** §10.4.1.2.5.1, §10.4.1.2.7, S. 252, 261 — "In addition, one or more
DomainTag values or DomainTagExpression values may be optionally specified.
If no DomainTag and DomainTagExpression is specified, then it shall be
treated as if the empty string ('') was specified as the only DomainTag
value. [...] For the DomainParticipant Domain to be matched, both the
DomainParticipant's DomainId and the DomainParticipant's DomainTag must
match".

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_filter`,
`DomainFilter::matches`; `crates/security-runtime/src/shared.rs::set_domain_tag`

**Tests:** `crates/security-permissions/src/governance.rs::tests::domain_rules_match_the_domain_tag`

**Status:** `done` — `<tag>` und `<tag_expression>` gelten für das DomainTag
des Participants (`fnmatch()` für den Ausdruck); ohne beide gilt eine Regel
für das leere Tag.

### §10.4.1.2.5.2 Allow Unauthenticated Participants element

**Spec:** §10.4.1.2.5.2, S. 253–254 — "If the value is set to FALSE, the
ParticipantSecurityConfig returned by the get_participant_security_config
operation on the AccessControl shall have the
allow_unauthenticated_participants member set to FALSE."

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_rule`
(Z. 876); `crates/security-runtime/src/shared.rs::allow_unauthenticated`
(Z. 1314)

**Tests:** `crates/security-runtime/src/shared.rs::tests::allow_unauthenticated_reads_true_when_set`,
`crates/security-runtime/src/shared.rs::tests::allow_unauthenticated_default_false_without_element`

**Status:** done — die Wirkung des Werts siehe §9.4.2.4 und §9.8.3.

### §10.4.1.2.5.3 Enable Join Access Control element

**Spec:** §10.4.1.2.5.3, S. 254 — "If the value is set to TRUE, the
ParticipantSecurityConfig returned by the get_participant_security_config
operation on the AccessControl shall have the is_access_protected member set
to TRUE."

**Repo:** `crates/security-permissions/src/governance.rs::DomainRule`
(Z. 154, `enable_join_access_control`)

**Tests:** `crates/security-permissions/src/governance.rs::tests::check_create_participant_consults_permissions`

**Status:** done — die Wirkung für entfernte Participants fehlt (siehe
§9.4.2.4 Table 33 — is_access_protected, `open`).

### §10.4.1.2.5.4 Enable Key Revision element

**Spec:** §10.4.1.2.5.4, S. 254 — "If the value is set to TRUE, the
ParticipantSecurityConfig returned by the get_participant_security_config
operation on the AccessControl shall have the is_key_revision_enabled member
set to TRUE."

**Repo:** `crates/security-permissions/src/governance.rs::DomainRule::enable_key_revision`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_key_revision_psk_and_allowed_algorithm_elements_are_read`,
`crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — `is_key_revision_enabled` der Konfiguration folgt dem Element.

### §10.4.1.2.5.5 Discovery Protection Kind element

**Spec:** §10.4.1.2.5.5, S. 254–255 — "The discovery protection element
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

**Status:** `done` — die Attribute `*_encrypted` und `*_origin_authenticated` der Konfiguration folgen dem Wert des Elements; die Laufzeit kündigt sie an.

### §10.4.1.2.5.6 Liveliness Protection Kind element

**Spec:** §10.4.1.2.5.6, S. 255 — "The liveliness protection element
specifies the protection kind (see 10.4.1.2.2) used for builtin DataWriter
and DataReader associated with the ParticipantMessageSecure builtin Topic".

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config` (`plugin_participant_attributes`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — die Attribute `*_encrypted` und `*_origin_authenticated` der Konfiguration folgen dem Wert des Elements; die Laufzeit kündigt sie an.

### §10.4.1.2.5.7 RTPS Protection Kind element

**Spec:** §10.4.1.2.5.7, S. 255–256 — "The RTPS protection kind element
specifies the protection kind (see 10.4.1.2.2) used for the whole RTPS
message. [...] The attribute is_rtps_axk_protected [...] shall be set to
FALSE if the value specified in the <rtps_protection_kind> element is NONE
and to TRUE otherwise."

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config` (`plugin_participant_attributes`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — die Attribute `*_encrypted` und `*_origin_authenticated` der Konfiguration folgen dem Wert des Elements; die Laufzeit kündigt sie an.

### §10.4.1.2.5.5 bis §10.4.1.2.5.7 Bits für Origin Authentication

**Spec:** §10.4.1.2.5.5–§10.4.1.2.5.7, S. 255–256 — `is_discovery_origin_authenticated`,
`is_liveliness_origin_authenticated`, `is_rtps_origin_authenticated` "shall
be set to TRUE if the value specified [...] is SIGN_WITH_ORIGIN_AUTHENTICATION
or ENCRYPT_WITH_ORIGIN_AUTHENTICATION".

**Repo:** `crates/dcps/src/runtime.rs` (Maske in `enable_security_builtins_with_auth`); `crates/security-runtime/src/shared.rs::metadata_origin_authenticated`, `discovery_origin_authenticated`, `liveliness_origin_authenticated`, `rtps_origin_authenticated`

**Tests:** `crates/security-runtime/src/shared.rs::tests::origin_authentication_follows_the_governance_kinds` (die Abfragen); die Bits selbst: kein Test

**Status:** `done` — `IS_RTPS_ORIGIN_AUTHENTICATED` (0x8), `IS_DISCOVERY_ORIGIN_AUTHENTICATED` (0x10), `IS_LIVELINESS_ORIGIN_AUTHENTICATED` (0x20) in den Plugin-Attributen des Participants folgen den Arten.

### §10.4.1.2.5.8 RTPS PSK Protection Kind element

**Spec:** §10.4.1.2.5.8, S. 256 — "The RTPS protection kind element specifies
the protection kind (see 10.4.1.2.2) used for RTPS Messages that are not
otherwise protected by an 'Authenticated Participant Exchanged Key'. This
includes the RTPS Bootstrapping Messages".

**Repo:** `crates/security-permissions/src/governance.rs::DomainRule::rtps_psk_protection_kind`; `crates/security-permissions/src/plugin.rs::get_participant_security_config`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_key_revision_psk_and_allowed_algorithm_elements_are_read`,
`crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — das Element wird gelesen; `is_rtps_psk_protected` und `is_rtps_psk_encrypted` folgen ihm.

### §10.4.1.2.5.9 Allowed Algorithms Section

**Spec:** §10.4.1.2.5.9, S. 256–258 — "The Allowed Algorithms section defines
rules that control the cryptographic algorithms that may be used in the
domain." Elemente `digital_signature`, `digital_signature_trust_chain`,
`key_establishment`, `symmetric_cipher`; fehlt ein Element, sind alle
Algorithmen von §8 erlaubt.

**Repo:** `crates/security-permissions/src/governance.rs::AllowedCryptoAlgorithms`, `parse_allowed_crypto_algorithms`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_allowed_algorithm_names_are_those_of_the_xsd`,
`crates/security-permissions/src/plugin.rs::tests::without_allowed_algorithms_every_algorithm_is_supported`,
`crates/security-permissions/src/plugin.rs::tests::the_identity_certificates_algorithms_must_be_allowed`,
`crates/dcps/src/runtime.rs::tests::the_allowed_algorithms_cut_the_announced_ones`

**Status:** `done` — der Abschnitt wird gelesen; fehlt ein Element, sind alle Algorithmen erlaubt; `algorithm_info` und die Prüfung des Identitätszertifikats folgen ihm.

### §10.4.1.2.5.10 Topic Access Rules Section

**Spec:** §10.4.1.2.5.10, S. 258 — "This section is delimited by the XML
element <topic_access_rules> and contains a sequence of topic rule elements."

**Repo:** `crates/security-permissions/src/governance.rs::parse_domain_rule`
(Z. 897)

**Tests:** `crates/security-permissions/src/governance.rs::tests::topic_rule_matches_exact_topic_first`

**Status:** done

### §10.4.1.2.6 Topic Rules: Reihenfolge, erste passende Regel

**Spec:** §10.4.1.2.6, S. 258 — "The topic access rules shall be evaluated in
the same order as they appear within the <topic_access_rules> Section. If
multiple rules match the first rule that matches is the only one that
applies."

**Repo:** `crates/security-permissions/src/governance.rs::find_topic_rule`
(Z. 332)

**Tests:** `crates/security-permissions/src/governance.rs::tests::topic_rule_matches_exact_topic_first`,
`crates/security-permissions/src/governance.rs::tests::topic_rule_falls_through_to_wildcard`,
`crates/security-permissions/src/governance.rs::tests::access_control_flags_come_from_the_matching_rule`

**Status:** done

### §10.4.1.2.6.1 Topic expression: Syntax von fnmatch()

**Spec:** §10.4.1.2.6.1, S. 258 — "The Topic name expression syntax and
matching shall use the syntax and rules of the POSIX fnmatch() function as
specified in POSIX 1003.2-1992, Section B.6 [38]."

**Repo:** `crates/security-permissions/src/topic_match.rs::fnmatch`, `::topic_match`

**Tests:** `crates/security-permissions/src/topic_match.rs::tests::star_matches_all`,
`crates/security-permissions/src/topic_match.rs::tests::question_mark_exact_one`,
`crates/security-permissions/src/topic_match.rs::tests::combined_wildcards`,
`crates/security-permissions/src/topic_match.rs::tests::sets_ranges_and_negation_match_one_character`,
`crates/security-permissions/src/topic_match.rs::tests::a_backslash_quotes_unless_noescape`

**Status:** `done` — POSIX `fnmatch()`: `*`, `?`, Klammerausdrücke mit Mengen, Bereichen
und Negation, der Backslash als Fluchtzeichen. Dieselbe Funktion prüft Topics,
Partitionen, Data Tags und Domain Tags in Governance und Permissions.

### §10.4.1.2.6.1 fnmatch(): Klammerausdrücke und Fluchtzeichen

**Spec:** §10.4.1.2.6.1, §10.4.1.5.3.4.1.3, §10.4.1.5.3.4.1.4, S. 258, 258,
258 — "shall use the syntax and rules of the POSIX fnmatch() function".

**Repo:** `crates/security-permissions/src/topic_match.rs::fnmatch`

**Tests:** `crates/security-permissions/src/topic_match.rs::tests::sets_ranges_and_negation_match_one_character`,
`crates/security-permissions/src/topic_match.rs::tests::a_backslash_quotes_unless_noescape`,
`crates/security-permissions/src/topic_match.rs::tests::pathname_keeps_slashes_out_of_wildcards`

**Status:** `done` — Klammerausdrücke (`[abc]`, `[a-z]`, `[!a-z]`) und der Backslash als
Fluchtzeichen, `FNM_NOESCAPE` und `FNM_PATHNAME` für die Subjekt-Ausdrücke.

### §10.4.1.2.6.2 Enable Discovery protection element

**Spec:** §10.4.1.2.6.2, S. 258–259 — "Specifically the is_discovery_protected
attribute in the TopicSecurityConfig shall be set to the boolean value
specified in the <enable_discovery_protection> element."

**Repo:** `crates/security-runtime/src/shared.rs::topic_discovery_protected`
(Z. 559)

**Tests:** `crates/security-runtime/src/shared.rs::tests::topic_discovery_protected_reads_topic_rule_flag`

**Status:** done — die Wahl der Endpunkte von SEDP folgt dem Wert nicht
(siehe §9.4.2.6 Table 36 — is_discovery_protected je Topic, `open`).

### §10.4.1.2.6.3 Enable Liveliness Protection element

**Spec:** §10.4.1.2.6.3, S. 259 — "Specifically the is_liveliness_protected
attribute in the TopicSecurityConfig shall be set to the boolean value
specified in the <enable_liveliness_protection> element."

**Repo:** `crates/security-permissions/src/plugin.rs::get_topic_security_config`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`

**Status:** `done` — `is_liveliness_protected` der `TopicSecurityConfig` folgt dem Element.

### §10.4.1.2.6.4 Enable Read Access Control element

**Spec:** §10.4.1.2.6.4, S. 259 — "In addition, this element shall control
the AccessControl::check_create_datareader operation on any DataReader entity
whose associated Topic name matches the rule's topic expression."

**Repo:** `crates/security-permissions/src/governance.rs::read_access_controlled`
(Z. 414), `check_create_datareader` (Z. 460)

**Tests:** `crates/security-permissions/src/governance.rs::tests::an_uncontrolled_topic_needs_no_grant`,
`crates/security-permissions/src/governance.rs::tests::the_six_omg_suite_refusals_are_denied`

**Status:** done — die Spec nennt in den beiden Punkten
`enable_write_access_control`; der Code nimmt `enable_read_access_control`
(Widerspruch in der Spec).

### §10.4.1.2.6.5 Enable Write Access Control element

**Spec:** §10.4.1.2.6.5, S. 259–260 — "If the value of
<enable_write_access_control> element is FALSE, the operation
check_create_datawriter shall return TRUE without further checking the
Permissions document."

**Repo:** `crates/security-permissions/src/governance.rs::write_access_controlled`
(Z. 405), `check_create_datawriter` (Z. 439)

**Tests:** `crates/security-permissions/src/governance.rs::tests::an_uncontrolled_topic_needs_no_grant`,
`crates/security-permissions/src/governance.rs::tests::the_granted_direction_still_creates`

**Status:** done

### §10.4.1.2.6.6 Metadata Protection Kind element

**Spec:** §10.4.1.2.6.6, S. 260 — "The attribute is_submessage_protected in
the EndpointSecurityConfig shall be set to FALSE if the value specified in
the <metadata_protection_kind> is NONE and shall be set to TRUE otherwise.
[...] is_submessage_encrypted [...] is_submessage_origin_authenticated".

**Repo:** `crates/security-runtime/src/shared.rs::metadata_protection`, `metadata_origin_authenticated`; `crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info`

**Tests:** `crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`, `crates/security-runtime/src/plugin_attributes.rs::tests::key_protected_follows_data_encrypt_per_spec_10_4_1_2_6` (der Aufruf mit dem neuen Parameter `false`)

**Status:** `done` — `is_submessage_protected`, `is_submessage_encrypted` und `is_submessage_origin_authenticated` (Plugin-Bit 0x4) folgen dem Wert. Das Bit 0x4 ist nicht getestet.

### §10.4.1.2.6.7 Data Protection Kind element

**Spec:** §10.4.1.2.6.7, S. 260–261 — "If the value specified in the
<data_protection_kind> element is NONE, then is_payload_protected,
is_key_protected and is_payload_encrypted shall be set to FALSE. [...] SIGN,
then is_payload_protected shall be set to TRUE. The attributes
is_key_protected and is_payload_encrypted shall be set to FALSE. [...]
ENCRYPT, then is_payload_protected, is_key_protected, and
is_payload_encrypted shall be set to TRUE."

**Repo:** `crates/dcps/src/runtime.rs::user_endpoint_security_info`, `crates/security-runtime/src/plugin_attributes.rs::endpoint_security_info`

**Tests:** `crates/security-runtime/src/plugin_attributes.rs::tests::key_protected_follows_data_encrypt_per_spec_10_4_1_2_6`,
`crates/security-runtime/src/plugin_attributes.rs::tests::liveliness_protected_sets_0x40_per_spec_9_4_1_3`

**Status:** done

### §10.4.1.2.7 Keine passende Domain Rule: Fehler

**Spec:** §10.4.1.2.7, S. 261 — "If no Domain Rule matches the
DomainParticipant domain_id the operation under consideration shall fail
with a suitable 'permissions error'."

**Repo:** `crates/security-permissions/src/governance.rs::check_create_participant`
(Z. 364), `check_create_datawriter` (Z. 439)

**Tests:** `crates/security-permissions/src/governance.rs::tests::an_uncovered_domain_creates_nothing`

**Status:** done

### §10.4.1.2.7 Keine passende Topic Rule: Fehler

**Spec:** §10.4.1.2.7, S. 262 — "If no Topic Rule matches the Entity topic
name the operation under consideration shall fail with a suitable
'permissions error'."

**Repo:** `crates/security-permissions/src/governance.rs::has_topic_rule`;
`crates/security-runtime/src/shared.rs` (`check_create_datawriter`,
`check_create_datareader`, `check_remote_datawriter`, `check_remote_datareader`)

**Tests:** `crates/security-permissions/src/governance.rs::tests::a_topic_without_a_rule_is_refused`

**Status:** `done` — ein Topic ohne passende Regel bekommt weder lokal noch
entfernt einen Endpunkt; das eingebaute Logging-Topic behält seine
eingebaute Regel.

### §10.4.1.3 Governance Document Extensibility: must_interpret

**Spec:** §10.4.1.3, S. 262–263 — "If an XML element does not have the
must_interpret attribute it shall be treated as if it had
must_interpret='TRUE'. [...] If a governance extension element has the
must_interpret attribute set to 'true' or 'TRUE', then the governance document
shall be considered invalid, and an error shall be raised."

**Repo:** `crates/security-permissions/src/governance.rs::extension_element`, `crates/security-permissions/src/xml.rs::check_known_element`

**Tests:** `crates/security-permissions/src/governance.rs::tests::an_extension_attribute_follows_must_interpret`,
`crates/security-permissions/src/governance.rs::tests::zerodds_elements_without_the_namespace_follow_must_interpret`,
`crates/security-permissions/src/governance.rs::tests::a_governance_outside_the_xsd_is_refused`

**Status:** `done` — unbekannte Elemente und Attribute folgen `must_interpret`; ein ungültiger Attributwert macht das Dokument ungültig; die Elemente im Namensraum `ZERODDS_NS` werden ebenfalls nach `must_interpret` behandelt.

### §10.4.1.4 Example Domain Governance document (non normative)

**Spec:** §10.4.1.4, S. 263–266 — Beispiel einer Governance.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — als nicht normativ gekennzeichnet.

### §10.4.1.5 Permissions signiert im Format S/MIME 3.2

**Spec:** §10.4.1.5, S. 266 — "The permissions document shall be signed by the
Permissions CA. The signed document shall use S/MIME version 3.2 format [...]
Additionally, the signer certificate shall be included within the signature."

**Repo:** `crates/security-permissions/src/signature.rs::open_signed_permissions`
(Z. 89)

**Tests:** `crates/security-permissions/tests/cms_verify.rs::happy_path_then_open_signed_permissions_parses`,
`crates/security-permissions/tests/cms_verify.rs::tampered_xml_is_rejected`

**Status:** done

### §10.4.1.5.1 XSD der Permissions: dds, permissions, grant

**Spec:** §10.4.1.5.1, S. 266–270 — "<xs:element name='dds'
type='PermissionsNode' /> [...] permissions [...] grant [...]
<xs:attribute name='name' type='xs:string' use='required' />".

**Repo:** `crates/security-permissions/src/xml.rs::parse_permissions_xml`, `parse_grant`

**Tests:** `crates/security-permissions/src/xml.rs::tests::a_document_outside_the_xsd_is_refused`,
`crates/security-permissions/src/xml.rs::tests::rejects_invalid_xml`,
`crates/security-permissions/src/xml.rs::tests::missing_subject_name_is_malformed`

**Status:** `done` — Dokumente folgen der XSD: Wurzel `<dds>`, `<validity>`, mindestens
eine Regel, `<domains>` in jeder Regel, `<topics>` in jeder Aktion; ein Element
außerhalb der XSD verlangt `must_interpret="false"` (§10.4.1.6).

### §10.4.1.5.3.1 Subject name: Grant gilt nur für passenden Namen

**Spec:** §10.4.1.5.3.1, S. 271 — "The contents of the <subject_name> element
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

**Status:** `done` — lokal und für einen Peer gilt nur der Grant, dessen
`subject_name` der SubjectName des Zertifikats ist; ohne einen lehnt das Profil
ab, ein Peer bleibt ohne Grant. Ein Grant ohne `<subject_name>` nimmt weiter das
Attribut `name` als Subjekt.

### §10.4.1.5.3.1.1 Subject name matching nach RFC 4514

**Spec:** §10.4.1.5.3.1, §10.4.1.5.3.1.1, S. 271–272 — "The format of x.509
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

**Status:** `done` — dieselben Attributnamen (ohne Groß- und Kleinschreibung),
gleiche Werte, Reihenfolge ohne Bedeutung; gelesen werden die Form nach RFC 4514
und die einzeilige Form von OpenSSL (`/C=…/CN=…`).

### §10.4.1.5.3.2 Subject name expression

**Spec:** §10.4.1.5.3.2, §10.4.1.5.3.2.1, S. 272 — "The <subject_name_expression>
element shall contain the same type of attribute-value assertions as the
<subject_name > element [...] except that the attribute values may also
contain the special 'pattern' characters [...] match according to the POSIX
fnmatch() function [...] with flags: FNM_PATHNAME=TRUE, FNM_PERIOD=FALSE, and
FNM_NOESCAPE=TRUE."

**Repo:** `crates/security-permissions/src/xml.rs::parse_grant`, `Grant::is_for`,
`Permissions::for_subject`

**Tests:** `crates/security-permissions/src/xml.rs::tests::a_subject_name_expression_matches_by_fnmatch`

**Status:** `done` — `<subject_name_expression>` wird gelesen und mit `fnmatch()`
(`FNM_PATHNAME`, `FNM_NOESCAPE`) gegen den SubjectName verglichen.

### §10.4.1.5.3.3 Validity Section

**Spec:** §10.4.1.5.3.3, S. 272 — "It contains both the starting date and the
end date using the format defined by dateTime data type [...] Time zones that
aren't specified are considered UTC. A permissions Section with a validity
date that falls outside the current date at which the permissions are being
evaluated shall be ignored."

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_validity`, `Grant::is_valid_at`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::validity_window_enforced`,
`crates/security-permissions/src/governance.rs::tests::validity_and_domain_bound_the_grant`

**Status:** `done` — Datum ohne Zone gilt als UTC; ein Grant außerhalb seines Zeitraums
wird übergangen.

### §10.4.1.5.3.3 Validity: Zeitzonen, fehlende und unlesbare Werte

**Spec:** §10.4.1.5.3.3, §10.4.1.7, S. 272, 283 — "Format is
CCYY-MM-DDThh:mm:ss[Z|(+|-)hh:mm] The time zone may be specified as Z (UTC)
or (+|-)hh:mm."; XSD: `<xs:element name='validity' type='Validity' />` ohne
`minOccurs='0'`.

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_validity`,
`parse_iso_seconds`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::iso_parser_applies_the_zone`,
`crates/security-permissions/src/xml.rs::tests::an_unreadable_validity_or_default_is_refused`,
`crates/security-permissions/src/xml.rs::tests::a_document_outside_the_xsd_is_refused`

**Status:** `done` — die Zeitzone wird angewandt; ein unlesbares `not_before` oder
`not_after` und ein Grant ohne `<validity>` sind Fehler, wie die XSD es
verlangt.

### §10.4.1.5.3.4 Rules Section: Reihenfolge und default

**Spec:** §10.4.1.5.3.4, S. 272–273 — "The rules are applied in the same order
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

**Status:** `done` — die Regeln gelten in der Reihenfolge des Dokuments, die erste
passende entscheidet; ohne Treffer gilt `default`, ohne `default` DENY.

### §10.4.1.5.3.4 Rules Section: erste passende Regel entscheidet

**Spec:** §10.4.1.5.3.4, S. 272 — "The rules are applied in the same order
that appear in the document."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::the_first_matching_rule_decides`,
`crates/security-permissions/src/plugin.rs::tests::allow_rule_and_deny_rule_decide_the_same_xml_differently`

**Status:** `done` — ein `allow_rule` vor einem `deny_rule` für dasselbe Topic ergibt
„allow“, in umgekehrter Reihenfolge „deny“.

### §10.4.1.5.3.4 Kriterien: Topics UND Partitionen UND Data Tags

**Spec:** §10.4.1.5.3.4, S. 273 — "For the grant to match there shall be a
match of the topics, partitions, and data-tags criteria. This is interpreted
as an AND of each of the criteria. For a specific criterion to match (e.g.,
<topics>) it is enough that one of the topic expressions listed matches".

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`; `crates/security-permissions/src/governance.rs::granted`

**Tests:** `crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`,
`crates/security-permissions/src/xml.rs::tests::data_tags_of_allow_and_deny_rules`,
`crates/security-permissions/src/plugin.rs::tests::partition_scope_narrows_an_otherwise_permitted_topic`

**Status:** `done` — Topics, Partitionen und Data Tags einer Aktion werden verknüpft,
je Regel. Endpunkte tragen noch keine Data Tags und gehen mit der leeren Menge
ein (Item §9.7.2).

### §10.4.1.5.3.4 Partitionen, Domains und Data Tags je Aktion und Regel

**Spec:** §10.4.1.5.3.4.1, §10.4.1.5.3.4.1.2, S. 273 — "Each rule contains a
Domains Section [...] followed by a set of allowed actions. [...] Each
allowed action logically contains three orthogonal conditions. These cover
the topic name, partitions, and data-tags."

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_grant`, `Grant::decide`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::a_deny_rule_holds_only_in_its_domains`,
`crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`,
`crates/security-permissions/src/xml.rs::tests::data_tags_of_allow_and_deny_rules`,
`crates/security-permissions/src/xml.rs::tests::domain_ranges_of_a_rule`

**Status:** `done` — Domains, Partitionen und Data Tags gelten für die Regel bzw. die
Aktion, in der sie stehen; ein Verbot gilt nur in seinen Domains.

### §10.4.1.5.3.4.1.2 Allow Relay Action

**Spec:** §10.4.1.5.3.4.1.2, S. 273 — "There are three kinds of allowed
actions: publish, subscribe and relay. [...] The Allow Relay Action is
delimited by the <relay> XML element".

**Repo:** `crates/security-permissions/src/xml.rs` (`Action::Relay`);
`crates/security-permissions/src/governance.rs` (`check_datareader`,
`check_relay_reader`, `check_relay_writer`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::relay_is_an_action_of_its_own`,
`crates/security-permissions/src/governance.rs::tests::relaying_needs_the_relay_action_where_access_is_controlled`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`

**Status:** `done` — `<relay>` ist eine eigene Aktion. Ein entfernter Reader mit nur
`<relay>` ist relay-only (§9.4.2.9.11); ein lokaler Relay-Reader wird auf
`<relay>` angelegt; ein Reader nimmt eine weitergeleitete Probe nur von einem
Writer mit `<relay>` als die des Originals (§7.1.1.4).

### §10.4.1.5.3.4.1.3 Allowed Topic condition section

**Spec:** §10.4.1.5.3.4.1.3, S. 274 — "Topic names may be given explicitly or
by means of Topic name expressions. Each topic name or topic-name expression
appears separately in a <topic> sub-element within the <topics> element."

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_grant`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::a_document_outside_the_xsd_is_refused`,
`crates/security-permissions/src/plugin.rs::tests::permit_on_wildcard_match`

**Status:** `done` — `<topics>` mit je einem `<topic>` für jeden Namen oder Ausdruck; ein
`<topic>` direkt unter der Aktion liegt außerhalb der XSD und wird abgewiesen.

### §10.4.1.5.3.4.1.4 Allowed Partitions condition: Teilmenge

**Spec:** §10.4.1.5.3.4.1.4, S. 274–275 — "the set of the Partitions
associated with the DDS entity [...] must be contained in the set of
partitions defined by the allowed partitions condition section. If there is
no <partitions> Section within an allow rule, then the default 'empty
string' partition is assumed."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`,
`crates/security-permissions/src/plugin.rs::tests::grant_without_partitions_covers_only_the_default_partition`,
`crates/security-permissions/src/plugin.rs::tests::partition_check_never_widens_a_denied_topic`

**Status:** `done` — je Regel: die Partitionen des Endpunkts müssen in denen der Aktion
liegen; ohne `<partitions>` gilt die Partition `""`.

### §10.4.1.5.3.4.1.4 Wahl des Legacy Matching für Partitionen

**Spec:** §10.4.1.5.3.4.1.4, S. 275 — "For legacy reasons DDS-Security
implementations shall provide a way to select an alternative 'legacy
matching' behavior. The 'legacy matching behavior' shall match the allowed
partitions condition condition as long as one or more of the Partitions
associated with DDS Entity attempting to perform the action [...] matches one
of the partitions in the allowed partitions condition."

**Repo:** `crates/security-permissions/src/xml.rs` (`PartitionMatching`);
Einstellung `zerodds.sec.access.partition_matching`

**Tests:** `crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`

**Status:** `done` — wählbar über `zerodds.sec.access.partition_matching`; die Vorgabe
ist der Vergleich der Spec (Teilmenge).

### §10.4.1.5.3.4.1.5 Allowed Data tags condition section

**Spec:** §10.4.1.5.3.4.1.5, S. 275–276 — "the set of the Data Tags associated
with the DDS Entity performing the action [...] must be contained in the set
of data tags defined by the allowed data tags condition section. If there is
no <data_tags> section then the default empty set is assumed."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::data_tags_of_allow_and_deny_rules`,
`crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`,
`crates/dcps/tests/security_access_plugin_e2e.rs::a_datawriter_is_created_and_changed_with_its_request_in_the_plugins_hands`

**Status:** `done` — die Bedingung wird je Regel ausgewertet; Endpunkte tragen Data Tags (`DataTagQosPolicy`) und das Plugin sieht sie.

### §10.4.1.5.3.4.2 Deny rules

**Spec:** §10.4.1.5.3.4.2, S. 277 — "Deny rules have the same format as the
allow rules. The only difference is how they are interpreted. If the criteria
in the deny rule matches the operation being performed, then the decision is
to deny the operation."

**Repo:** `crates/security-permissions/src/xml.rs` (`parse_grant`, `Grant::decide`)

**Tests:** `crates/security-permissions/src/xml.rs::tests::the_first_matching_rule_decides`,
`crates/security-permissions/src/plugin.rs::tests::deny_rule_also_binds_the_reader_side`,
`crates/security-permissions/src/plugin.rs::tests::remote_match_inherits_the_deny_decision`

**Status:** `done` — ein passendes Verbot verweigert die Operation, in der Reihenfolge
der Regeln, mit seinen Domains und Partitionen.

### §10.4.1.5.3.4.2.4 Denied Partitions: Schnittmenge, ohne Angabe „*“

**Spec:** §10.4.1.5.3.4.2.4, S. 278 — "For this to happen one of more of the
partition names associated with the DDS Entity performing the action [...]
must match one the partitions or partition expressions explicitly listed in
the partitions condition section. If there is no <partitions> section then
the '*' partition expression is assumed."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::partitions_of_allow_and_deny_rules`

**Status:** `done` — ein Verbot greift, wenn eine Partition des Endpunkts zu einer seiner
Partitionen passt; ohne `<partitions>` gilt `*`.

### §10.4.1.5.3.4.2.5 Denied Data tags condition section

**Spec:** §10.4.1.5.3.4.2.5, S. 279 — "If there is no <data_tags> section then
the 'set of all possible tags' set is assumed as default."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::decide`

**Tests:** `crates/security-permissions/src/xml.rs::tests::data_tags_of_allow_and_deny_rules`,
`crates/security-runtime/src/shared.rs::tests::the_plugin_gets_partitions_data_tags_and_qos`

**Status:** `done` — ohne `<data_tags>` gilt die Menge aller Tags; Endpunkte tragen Data Tags.

### §10.4.1.6 Permissions Document Extensibility

**Spec:** §10.4.1.6, S. 282 — "The approach is the same used for extending the
Governance document, see 10.4.1.3."

**Repo:** `crates/security-permissions/src/xml.rs::check_known_element_with`, `check_must_interpret_value`

**Tests:** `crates/security-permissions/src/xml.rs::tests::the_permissions_document_is_extended_by_must_interpret`

**Status:** `done` — unbekannte Elemente und Attribute des Permissions-Dokuments folgen `must_interpret` wie in der Governance.

### §10.4.1.7 DomainParticipant example permissions document (non normative)

**Spec:** §10.4.1.7, S. 283–286 — Beispiel eines Permissions-Dokuments.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — als nicht normativ gekennzeichnet.

### §10.4.2.1 Table 64 — PermissionsCredentialToken

**Spec:** §10.4.2.1, Table 64, S. 286 — "class_id
'DDS:Access:PermissionsCredential' properties [...] dds.perm.cert Contents of
the permissions document signed by the PermissionCA".

**Repo:** `crates/security-permissions/src/plugin.rs::get_permissions_credential_token`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`

**Status:** `done` — das Token hat die Klasse `DDS:Access:PermissionsCredential` und `dds.perm.cert`.

### §10.4.2.2 Table 65 — PermissionsToken

**Spec:** §10.4.2.2, Table 65, S. 286 — "class_id 'DDS:Access:Permissions:1.2'
properties (The presence of each of these properties is optional)
dds.perm_ca.sn [...] dds.perm_ca.algo".

**Repo:** `crates/security-permissions/src/plugin.rs::get_permissions_token`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`

**Status:** `done` — das Token trägt `dds.perm_ca.sn` und `dds.perm_ca.algo` aus der Permissions CA.

### §10.4.2.2 PermissionsToken: class_id 1.2

**Spec:** §10.4.2.2, Table 65, S. 286 — "class_id 'DDS:Access:Permissions:1.2'
[...] Accordingly this version of the specification has PluginClassName equal
to 'DDS:Access:Permissions', MajorVersion set to 1, and MinorVersion set to
0."

**Repo:** `crates/security-permissions/src/plugin.rs` (`PROP_TOKEN_CLASS_ID`, `plugin_class_id`)

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_permissions_token_class_is_1_2_unless_named`,
`crates/security-permissions/src/plugin.rs::tests::a_remote_token_of_another_class_or_major_version_is_refused`

**Status:** `done` — Vorgabe ist `DDS:Access:Permissions:1.2` (Table 65). `DDS:Access:Permissions:1.0` bleibt als benannte Einstellung `org.zerodds.sec.access.permissions_token_class_id` für Peers, die 1.0 senden. Erwartet: Cyclone DDS und Fast DDS vergleichen PluginClassName und MajorVersion (Table 70), also nehmen sie 1.2 an; nicht gemessen.

### §10.4.2.3 Table 66 — PluginParticipantSecurityAttributes

**Spec:** §10.4.2.3, Table 66, S. 287–289 — `is_rtps_axk_encrypted`,
`is_rtps_psk_encrypted`, `is_discovery_encrypted`, `is_liveliness_encrypted`,
`is_rtps_origin_authenticated`, `is_discovery_origin_authenticated`,
`is_liveliness_origin_authenticated` und ihre Wirkung auf Schlüssel und
Transformation.

**Repo:** `crates/security-permissions/src/plugin.rs::get_participant_security_config`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — alle sieben Attribute (`*_encrypted`, `*_origin_authenticated`, `is_rtps_psk_encrypted`) folgen der Governance.

### §10.4.2.4 Table 67 — PluginParticipantSecurityAttributesMask

**Spec:** §10.4.2.4, Table 67, S. 290 — Bits 0 bis 6:
IS_RTPS_AXK_ENCRYPTED, IS_DISCOVERY_ENCRYPTED, IS_LIVELINESS_ENCRYPTED,
IS_RTPS_ORIGIN_AUTHENTICATED, IS_DISCOVERY_ORIGIN_AUTHENTICATED,
IS_LIVELINESS_ORIGIN_AUTHENTICATED, IS_RTPS_PSK_ENCRYPTED; Vorgabe 0.

**Repo:** `crates/security/src/access_control.rs::plugin_participant_attributes_mask`; `crates/rtps/src/participant_security_info.rs::plugin_attrs`

**Tests:** `crates/security/src/access_control.rs::tests::the_plugin_masks_have_the_bits_of_tables_67_and_69`,
`crates/rtps/src/participant_security_info.rs::tests::fully_protected_sets_all_encrypt_bits`

**Status:** `done` — Bits 0 bis 6 sind belegt und werden aus der Konfiguration gesetzt.

### §10.4.2.4 IS_RTPS_PSK_ENCRYPTED fehlt

**Spec:** §10.4.2.4, Table 67, S. 290 — "#define
PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_ENCRYPTED
(0x00000001 << 6)".

**Repo:** `crates/security/src/access_control.rs::plugin_participant_attributes_mask`

**Tests:** `crates/security/src/access_control.rs::tests::the_plugin_masks_have_the_bits_of_tables_67_and_69`,
`crates/security-permissions/src/plugin.rs::tests::the_participant_config_follows_the_domain_rule`

**Status:** `done` — Bit 6 wird aus `rtps_psk_protection_kind` gesetzt.

### §10.4.2.5 Table 68 — PluginEndpointSecurityAttributes

**Spec:** §10.4.2.5, Table 68, S. 291–292 — `is_submessage_encrypted`,
`is_submessage_origin_authenticated`, `is_payload_encrypted` und ihre
Wirkung auf Schlüssel und Transformation.

**Repo:** `crates/security-permissions/src/plugin.rs::get_datawriter_security_config`; `crates/security/src/access_control.rs::plugin_endpoint_attributes_mask`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_topic_and_endpoint_configs_follow_the_topic_rule`,
`crates/security/src/access_control.rs::tests::the_plugin_masks_have_the_bits_of_tables_67_and_69`

**Status:** `done` — `is_submessage_encrypted`, `is_submessage_origin_authenticated` und `is_payload_encrypted` folgen der Topic Rule.

### §10.4.2.6 Table 69 — PluginEndpointSecurityAttributesMask

**Spec:** §10.4.2.6, Table 69, S. 292 — IS_SUBMESSAGE_ENCRYPTED (0x1 << 0),
IS_PAYLOAD_ENCRYPTED (0x1 << 1), IS_SUBMESSAGE_ORIGIN_AUTHENTICATED
(0x1 << 2); Vorgabe 0.

**Repo:** `crates/rtps/src/endpoint_security_info.rs::plugin_attrs` (Z. 56)

**Tests:** `crates/rtps/src/endpoint_security_info.rs::tests::protection_bit_accessors_read_correctly`

**Status:** done

### §10.4.2.4 und §10.4.2.6 Gleiche Masken als Bedingung der Kommunikation

**Spec:** §10.4.2.4, §10.4.2.6, S. 290, 292 — "As described in section
7.3.23, in order to communicate, two DomainParticipants need to have the same
ParticipantSecurityInfo. As as consequence the
PluginParticipantSecurityAttributesMask must also be the same."

**Repo:** `crates/dcps/src/runtime.rs::endpoint_incompatibility` und Teilnehmer-Prüfung beim Authentisieren

**Tests:** `crates/dcps/src/runtime.rs::tests::participants_with_an_incompatible_configuration_are_not_authenticated`,
`crates/dcps/src/runtime.rs::tests::the_participant_security_info_is_the_config_of_the_access_control_plugin`

**Status:** `done` — Teilnehmer und Endpunkte mit anderen Masken als die eigenen werden nicht authentisiert beziehungsweise nicht zugeordnet.

### §10.4.3 Plugin kennt den öffentlichen Schlüssel der Permissions CA

**Spec:** §10.4.3, S. 293 — "The DDS:Access:Permissions shall be initialized
to have access to the Permissions CA public key. As this is a builtin plugin
the mechanism for initialization is implementation dependent."

**Repo:** `crates/security-runtime/src/endpoint_access.rs::arm_endpoint_access_control`
(Z. 26)

**Tests:** `crates/dcps/tests/security_endpoint_access_e2e.rs::an_unarmed_gate_refuses_instead_of_creating`

**Status:** done

### §10.4.3 Table 70 — check_create_participant

**Spec:** §10.4.3, Table 70, S. 293 — "If the ParticipantSecurityConfig has
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

**Status:** `done` — bei FALSE wird zugelassen; sonst entscheidet die erste Regel des
Grants für die Domain: eine Erlaubnis lässt zu, ein Verbot ohne publish und
subscribe verweigert, sonst gilt `default`.

### §10.4.3 Table 70 — check_create_participant: deny_rule und default

**Spec:** §10.4.3, Table 70, S. 293 — "If the Grant's first matching rule for
the DomainParticipant's domain is a deny rule with no publish or subscribe
rules, then the operation shall fail and return FALSE. If none of the
previous conditions are true, then the operation shall return TRUE if the
default is ALLOW and return FALSE otherwise."

**Repo:** `crates/security-permissions/src/xml.rs::Grant::may_join`

**Tests:** `crates/security-permissions/src/xml.rs::tests::joining_follows_the_first_rule_for_the_domain`,
`crates/security-permissions/src/plugin.rs::tests::a_deny_rule_with_actions_first_leaves_joining_to_the_default`

**Status:** `done` — siehe das Item darüber.

### §10.4.3 Table 70 — check_create_datawriter und check_create_datareader

**Spec:** §10.4.3, Table 70, S. 293 — "If the Governance specifies a topic or
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

**Status:** `done` — Governance, Topic, alle Partitionen, Data Tags, Grant des Subjekts und Topic Rule werden geprüft.

### §10.4.3 Table 70 — check_create_topic

**Spec:** §10.4.3, Table 70, S. 293–294 — "If the Governance specifies a topic
or topic-expression on the DomainParticipant domain_id matching the Topic
name with enable_read_access_control set to FALSE or with
enable_write_access_control set to FALSE, then the operation shall succeed
and return TRUE. If the Permissions document contains a Grant for the
DomainParticipant allowing it to publish the Topic [...] subscribe the Topic
[...] return TRUE. Otherwise the operation shall return FALSE."

**Repo:** `crates/security-permissions/src/plugin.rs::check_create_topic`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::check_create_topic_follows_governance_and_grant`

**Status:** `done` — TRUE bei offenem Topic oder erlaubtem Publish oder Subscribe; sonst FALSE.

### §10.4.3 Table 70 — Operationen, die TRUE liefern

**Spec:** §10.4.3, Table 70, S. 294–296 — "check_local_datawriter_register_instance
This operation shall return TRUE. check_local_datawriter_dispose_instance
[...] check_local_datawriter_match [...] check_local_datareader_match [...]
check_remote_datawriter_register_instance [...]
check_remote_datawriter_dispose_instance This operation shall return TRUE."

**Repo:** `crates/security-permissions/src/plugin.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::instance_and_match_checks_return_true`,
`crates/security-permissions/src/plugin.rs::tests::the_nil_handle_is_refused_on_a_controlled_topic_only`

**Status:** `done` — die sechs Operationen liefern TRUE; die Handles werden geprüft.

### §10.4.3 Table 70 — check_remote_participant

**Spec:** §10.4.3, Table 70, S. 294 — "If the PluginClassName or the
MajorVersion of the local permissions_token differ from those in the
remote_permissions_token, the operation shall return FALSE. If the Grant's
first matching rule for the remote DomainParticipant's domain is an allow
rule, then the operation shall succeed and return TRUE."

**Repo:** `crates/security-permissions/src/plugin.rs::check_remote_participant`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::a_remote_token_of_another_class_or_major_version_is_refused`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`

**Status:** `done` — PluginClassName und MajorVersion des Tokens werden verglichen; sonst entscheidet die erste passende Regel des Grants für die Domain.

### §10.4.3 Table 70 — check_remote_datawriter und check_remote_datareader

**Spec:** §10.4.3, Table 70, S. 294–295 — "If the PluginClassName or the
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

**Status:** `done` — ein Peer, dessen PermissionsToken fehlt oder eine andere
PluginClassName oder MajorVersion nennt, behält keinen Grant und wird auf
zugriffsgeschützten Topics abgelehnt; Grant und Partitionen des Peers
entscheiden; ein Reader mit nur `<relay>` ist relay-only. Gemessen: grün auf
dem Linux-Testhost.

### §10.4.3 Table 70 — check_remote_topic

**Spec:** §10.4.3, Table 70, S. 295 — "If the Permissions document contains a
Grant for the DomainParticipant allowing it to publish the Topic with
specified topic_name, then the operation shall succeed and return TRUE."

**Repo:** `crates/security-permissions/src/plugin.rs::check_remote_topic`; `crates/security-permissions/src/xml.rs::Grant::allows_topic`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::the_nil_handle_is_refused_on_a_controlled_topic_only`,
`crates/dcps/tests/security_relay_e2e.rs::a_relay_only_reader_is_matched_and_cannot_read_the_payload`

**Status:** `done` — TRUE bei erlaubtem Publish, Subscribe oder Relay des Topics im Grant des entfernten Teilnehmers.

### §10.4.3 Table 70 — get_permissions_token und get_permissions_credential_token

**Spec:** §10.4.3, Table 70, S. 296 — "get_permissions_token This operation
shall return the PermissionsToken formatted as described in 10.4.2.2.
get_permissions_credential_token This operation shall return the
PermissionsToken formatted as described in 10.4.2.1".

**Repo:** `crates/security-permissions/src/plugin.rs`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::the_tokens_are_the_ones_of_tables_64_and_65`

**Status:** `done` — beide Tokens sind wie in §10.4.2.1 und §10.4.2.2 geformt.

### §10.4.3 Table 70 — validate_local_permissions

**Spec:** §10.4.3, Table 70, S. 296 — "The operation shall check the subject
name in the Identity Certificate matches the one from the Signed Permissions
document. The operation shall verify the signature of the Signed Domain
Governance and Signed Permissions document by the configured Permissions CA."

**Repo:** `crates/security-runtime/src/profile.rs::from_files_with_cert_policy`
(`NoGrantForSubject`)

**Tests:**
`crates/security-permissions/tests/cms_verify.rs::wrong_ca_signature_is_rejected`,
`crates/security-permissions/tests/cms_verify.rs::tampered_xml_is_rejected`,
`crates/security-permissions/src/xml.rs::tests::only_the_grant_of_the_subject_counts`

**Status:** `done` — beide Signaturen werden geprüft; ein lokales Dokument ohne
Grant für den SubjectName des eigenen Zertifikats wird abgewiesen.

### §10.4.3 Table 70 — validate_remote_permissions

**Spec:** §10.4.3, Table 70, S. 296–297 — "The operation shall check the
subject name in the Signed Permissions Document matches the one in the
Identity Certificate. The operation shall verify the signature of the Signed
Permissions Document by the configured Permissions CA."

**Repo:** `crates/security-permissions/src/plugin.rs::validate_remote_permissions`

**Tests:** `tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_participant_is_validated_and_checked`,
`tools/secure-permissions/tests/access_plugin_signed.rs::a_remote_document_needs_the_permissions_ca`

**Status:** `done` — Signatur durch die konfigurierte Permissions CA und Übereinstimmung des Subjekts mit dem Identitätszertifikat werden geprüft.

### §10.4.3 Table 70 — get_participant_security_config

**Spec:** §10.4.3, Table 70, S. 297–298 — "If the Governance document has the
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

**Status:** `done` — `allow_unauthenticated_participants` und alle anderen Member folgen Table 70; ohne `<allowed_crypto_algorithms>` ist die `supported_mask` CRYPTO_ALGORITHM_SET_ALL.

### §10.4.3 Table 70 — set_listener und return_*

**Spec:** §10.4.3, Table 70, S. 296, 298 — "set_listener This operation shall
save a reference to the listener object [...] return_permissions_token This
operation shall behave as specified in 9.4.2.9.20" (entsprechend für die
übrigen return_*)

**Repo:** `crates/security-permissions/src/plugin.rs`

**Tests:** `crates/security-permissions/src/plugin.rs::tests::revoking_permissions_tells_the_listener`,
`crates/security-permissions/src/plugin.rs::tests::returned_tokens_and_configs_must_have_been_issued`

**Status:** `done` — `set_listener` speichert die Referenz; die `return_*` verhalten sich wie in §9.4.2.9.

---

### §10.5 DDS:Crypto:AES-GCM-GMAC: AEAD mit AES-GCM und AES-GMAC

**Spec:** §10.5, S. 299 — "This plugin does Authenticated Encryption with
Associated Data (AEAD) using Advanced Encryption Standard with Galois Counter
Mode (AES-GCM/GMAC) [...] the DataWriter should create a separate
'reader-specific key' used only for authentication and append additional
reader-specific MACs".

**Repo:** `crates/security-crypto/src/plugin.rs::AesGcmCryptoPlugin`
(Z. 724)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::encrypt_decrypt_roundtrip`,
`crates/security-crypto/src/plugin.rs::tests::gmac_only_suite_roundtrip_without_encryption`

**Status:** done — die MACs für einzelne Reader sind unter Table 80
bewertet.

### §10.5.1 Table 71 — Konfiguration über dds.sec.crypto.*

**Spec:** §10.5.1, Table 71, S. 299–301 — "The DDS:Crypto:AES-GCM-GMAC plugin
shall be configured using the PropertyQosPolicy of the DomainParticipantQos,
DataWriterQos, or DataReaderQos." Properties `symmetric_cipher_algorithm`,
`rtps_psk_symmetric_cipher_algorithm`, `rtps_psk_secret_passphrase`,
`rtps_psk_secret_passphrase_extra`, jeweils mit Präfix `dds.sec.crypto.`.

**Repo:** `crates/security-runtime/src/qos_properties.rs`; `crates/security-crypto/src/plugin/spec.rs`

**Tests:** `crates/dcps/src/runtime.rs::tests::the_cipher_property_of_an_endpoint_sets_its_key_width`, `crates/security-crypto/src/psk_rtps.rs::tests::the_uri_properties_of_the_pre_shared_key_are_read`, `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`

**Status:** `done` — `symmetric_cipher_algorithm` von Participant, Writer und Reader (Laufzeit gibt die Property des Endpunkts weiter), die PSK-Properties als `file:`/`data:`-URI. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` und `-p zerodds-dcps --features security --lib` (7 Tests), Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §10.5.1.1 Symmetric Cipher Algorithm: Vorgabe AES256-GCM

**Spec:** §10.5.1, Table 71, §10.5.1.1, S. 299–301 — "If 'AUTO' is specified
it is treated as if it was specified to be 'AES256-GCM'. [...] If used in a
DataWriterQos or DataReaderQos it configures the symmetric cipher used by that
specific DataWriter or DataReader".

**Repo:** `crates/security-crypto/src/suite.rs::symmetric_cipher_key_bits`; `crates/security-crypto/src/plugin/spec.rs`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::default_plugin_uses_aes256`, `crates/security-crypto/src/plugin.rs::tests::the_key_width_follows_symmetric_cipher_algorithm`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_gcm_or_gmac_by_the_protection_kind_and_width`, `crates/dcps/src/runtime.rs::tests::the_cipher_property_of_an_endpoint_sets_its_key_width`

**Status:** `done` — Fehlend oder `AUTO` ist AES256-GCM, je Endpunkt überschreibbar. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` und `-p zerodds-dcps --features security --lib` (7 Tests), Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §10.5.1.2 bis §10.5.1.4 PSK Symmetric Cipher Algorithm und Secret Passphrase

**Spec:** §10.5.1.2–§10.5.1.4, S. 301–302 — "Configures the Cipher Algorithm
Pair that the DomainParticipant will use to protect RTPS Messages with a
pre-shared Key. [...] Configures the KeyMaterial and CryptoTransformKeyId and
CryptoTransformKeyRevision used by the operation encode_rtps_message when the
parameter transform_with_psk is set to true. [...] Configures alternative
values for the KeyMaterial".

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskConfig``, `key_bits_from_cipher`; `crates/security-runtime/src/qos_properties.rs::psk_crypto_config_from_properties`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_passphrase_rules_of_table_86`, `crates/security-runtime/src/qos_properties.rs::tests::the_psk_crypto_properties_of_table_86_are_read`

**Status:** `done` — Schlüsselbreite, Passphrase mit Id und Alternativen werden gelesen; die Ableitung folgt §10.5.2.1.3.

### §10.5.2 Typen des Plugins vollständig festgelegt

**Spec:** §10.5.2, S. 302 — "All 'Handle' types are local opaque handles that
are only understood by the local plugin objects that create or use them. The
remaining types shall be fully specified so that independent implementations
of DDS:Crypto:AES-GCM-GMAC can interoperate."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Typen folgen in Table 72
bis 77.

### §10.5.2.1 Table 72 — CryptoToken

**Spec:** §10.5.2.1, Table 72, S. 302 — "class_id 'DDS:Crypto:AES_GCM_GMAC'
binary_properties [...] dds.cryp.keymat The Big Endian CDR Serialization of
the KeyMaterial_AES_GCM_GMAC structure defined below."

**Repo:** `crates/dcps/src/runtime.rs` (Konstanten `CRYPTO_TOKEN_PROP`
Z. 21328 und `CRYPTO_TOKEN_CLASS_ID` Z. 21333), `build_crypto_token_message`
(Z. 21338)

**Tests:** `crates/dcps/src/runtime.rs::tests::secured_data_round_trips_through_runtime_dispatch_e2e`,
`crates/security-crypto/src/plugin.rs::tests::cross_plugin_interop_via_tokens`

**Status:** done

### §10.5.2.1.1 KeyMaterial_AES_GCM_GMAC: Aufbau und Kodierung

**Spec:** §10.5.2.1.1, S. 302–303 — "@extensibility(FINAL) struct
KeyMaterial_AES_GCM_GMAC { CryptoTransformKind transformation_kind;
sequence<octet, 32> master_salt; CryptoTransformKeyId sender_key_id;
sequence<octet, 32> master_sender_key; CryptoTransformKeyId
receiver_specific_key_id; sequence<octet, 32> master_receiver_specific_key;
};"

**Repo:** `crates/security-crypto/src/plugin.rs::serialize_keymat_for`, `serialize_keymat_cyclone`, `from_keymat_cyclone`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::a_receiver_specific_key_id_and_key_go_together`, `::tokens_carry_suite_tag_so_cross_suite_interop_works`, `::rejects_token_with_unknown_suite_id`

**Status:** `done` — die sechs Member werden in dieser Reihenfolge in Big Endian geschrieben und gelesen, mit `receiver_specific_key_id` und `master_receiver_specific_key`; die Id ist 0 genau dann, wenn der Schlüssel leer ist, und der Schlüssel hat die Länge des Senderschlüssels.

### §10.5.2.1.1 master_salt mit 16 Oktetten bei AES128 und Schlüssel für einzelne Empfänger

**Spec:** §10.5.2.1.1, §10.5.3.3.2, Table 81, S. 303, 321 — "MasterSalt
octet[16] for AES128 [...] octet[32] for AES256"; "A zero value for
receiver_specific_key_id indicates there is no receiver-specific
authentication tags and shall occur if and only if the length of the
master_receiver_specific_key is also zero."

**Repo:** `crates/security-crypto/src/plugin.rs::from_keymat_cyclone`, `receiver_key_for`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::an_aes128_gmac_key_with_a_short_salt_crosses_instances`, `::a_receiver_specific_key_id_and_key_go_together`

**Status:** `done` — `master_salt` bis 32 Oktette (Vorrunde); `receiver_specific_key_id` und `master_receiver_specific_key` werden bei Origin Authentication geschrieben (eine Id ungleich 0, nie die eines anderen Schlüssels dieser Seite) und gelesen.

### §10.5.2.1.2 Table 73 — Key Material für VolatileMessageSecure aus dem SharedSecret

**Spec:** §10.5.2.1.2, Table 73, Table 74, S. 303–304 — "transformation_kind
Set transformation_algorithm_id to CRYPTO_ALGORITHM_ID_AES256_GCM master_salt
HMACsha256 ( sha256(Challenge1 | KxSaltCookie | Challenge2) , SharedSecret)
sender_key_id 0 master_sender_key HMACsha256 ( sha256(Challenge2 |
KxKeyCookie | Challenge1) , SharedSecret ) receiver_specific_key_id 0
master_receiver_specific_key Zero-length sequence"; KxKeyCookie "key exchange
key", KxSaltCookie "keyexchange salt".

**Repo:** `crates/security-crypto/src/plugin.rs::from_kx_shared_secret`
(Z. 590), `register_matched_remote_participant` (Z. 1352)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_kx_key_matches_both_directions`,
`crates/security-crypto/src/plugin.rs::tests::with_secret_provider_derives_same_kx_key_for_both_sides`

**Status:** done — ohne bekannte Challenges leitet das Plugin den Schlüssel
mit eigenem HKDF ab (`from_shared_secret_kx`, Z. 573; Kommentar in
Z. 1380–1383); dieser Fall betrifft PSK und Attrappen.

### §10.5.2.1.3 Table 75 und Table 76 — Key Material für RTPS PSK Protection

**Spec:** §10.5.2.1.3, Table 75, Table 76, S. 304–307 — "master_salt
HMACsha256( HMACsha256('PSK-SALT' | SenderKeyId | 'RTPS' | ProtocolVersion |
VendorId | GuidPrefix, PreSharedSecret), 'master salt derivation' | 0x01 )
[...] master_sender_key HMACsha256( HMACsha256('PSK-SKEY' | SenderKeyId |
'RTPS' | ProtocolVersion | VendorId | GuidPrefix, PreSharedSecret), 'master
sender key derivation'| 0x01 )"; Beispiel mit Ergebniswerten.

**Repo:** `crates/security-crypto/src/psk_rtps.rs::derive_key_material``, `domain_key_prefix`, `Passphrase`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_material_is_the_one_the_spec_prints`

**Status:** `done` — SenderKeyId, `master_salt`, `master_sender_key` und Revision stimmen mit dem Beispiel der Spec überein (Domain 201, Passphrase 5632); bei AES-128 auf 16 Oktette gekürzt. `receiver_specific_key_id` 0, kein `master_receiver_specific_key`.

### §10.5.2.2 Table 77 — CryptoTransformIdentifier: transformation_kind

**Spec:** §10.5.2.2, Table 77, S. 308 — "CRYPTO_ALGORITHM_ID_NONE
CRYPTO_ALGORITHM_ID_AES128_GMAC CRYPTO_ALGORITHM_ID_AES128_GCM
CRYPTO_ALGORITHM_ID_AES256_GMAC CRYPTO_ALGORITHM_ID_AES256_GCM [...] These
variants shall have master_sender_key with 16 octets in length [...] 32
octets".

**Repo:** `crates/security-crypto/src/suite.rs`

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::kind_round_trip_all_variants`, `crates/security-crypto/src/suite.rs::tests::aes128_gmac_uses_spec_id_1`

**Status:** `done` — Alle vier Algorithmus-Ids tragen AES (0x01 AES128-GMAC). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.2.2 Table 77 — transformation_key_revision

**Spec:** §10.5.2.2, Table 77, S. 308 — "Set the transformation_key_revision
field to the value {0, 0, 0} the first time Key Material is produced for a
specific value of the transformation_key_id. Subsequent generation of the Key
Material for that same transformation_key_id should increment the
transformation_key_revision".

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_key_revision_is_carried_in_the_keymat_and_the_header`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`

**Status:** `done` — Erste Revision {0,0,0}, danach erhöht. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.2.2 Table 77 — transformation_key_id

**Spec:** §10.5.2.2, Table 77, S. 308 — "This is set to a different value each
time new Key Material is produced by a DomainParticipant. The algorithm used
is implementation specific but it shall avoid repeating the values for the
same DomainParticipant."

**Repo:** `crates/security-crypto/src/plugin.rs::new_local_key`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_local_keys_have_distinct_nonzero_key_ids`

**Status:** `done` — Zufällig, nie 0, nie doppelt je Participant. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.2.2 transformation_key_id: keine Wiederholung je Participant

**Spec:** §10.5.2.2, Table 77, S. 308 — "it shall avoid repeating the values
for the same DomainParticipant."

**Repo:** `crates/security-crypto/src/plugin.rs::new_local_key`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_local_keys_have_distinct_nonzero_key_ids`

**Status:** `done` — Eine gezogene Kennung wird verworfen, wenn diese Seite sie schon vergeben hat. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.2.3 CryptoHeader: transform_identifier, session_id, initialization_vector_suffix

**Spec:** §10.5.2.3, S. 309 — "struct CryptoHeader {
CryptoTransformIdentifier transform_identifier; octet session_id[4]; octet
initialization_vector_suffix[8]; }; [...] The CryptoHeader structure shall be
serialized using Big Endian serialization".

**Repo:** `crates/security-crypto/src/plugin.rs::seal_cyclone` (Z. 278,
20 Oktette Kopf)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::header_round_trip_with_full_iv`,
`crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`

**Status:** done

### §10.5.2.4 CryptoContent

**Spec:** §10.5.2.4, S. 309 — "struct CryptoContent { sequence<octet>
crypto_content; }; The CryptoContent structure shall be serialized using Big
Endian serialization".

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (Z. 1106),
`encode_payload_from_mat` (Z. 918)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`,
`crates/security-crypto/src/plugin.rs::tests::serialized_payload_cross_plugin_via_tokens`

**Status:** done

### §10.5.2.5 CryptoFooter und ReceiverSpecificMAC

**Spec:** §10.5.2.5, S. 309–310 — "struct ReceiverSpecificMAC {
CryptoTransformKeyId receiver_mac_key_id; octet receiver_mac[16]; }; [...]
struct CryptoFooter { octet common_mac[16]; sequence<ReceiverSpecificMAC>
receiver_specific_macs; };"

**Repo:** `crates/security-crypto/src/crypto_transform.rs::CryptoFooter`
(Z. 207); `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116)

**Tests:** `crates/security-crypto/src/crypto_transform.rs::tests::footer_round_trip_with_receivers`,
`crates/security-crypto/src/crypto_transform.rs::tests::footer_rejects_oversized_receiver_count_cleanly`

**Status:** done — der Codec der Laufzeit schreibt immer eine leere Liste.

### §10.5.3 Drei Schnittstellen

**Spec:** §10.5.3, S. 310 — "This plugin implements three interfaces:
CryptoKeyFactory, CryptoKeyExchange, and CryptoTransform."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung; ZeroDDS bündelt die drei in
einer Schnittstelle (siehe §9.5.1).

### §10.5.3.1 Table 78 — register_local_participant: ParticipantKeyMaterial

**Spec:** §10.5.3.1, Table 78, S. 310–311 — "This operation shall create a new
KeyMaterial_AES_GCM_GMAC object [...] ParticipantKeyMaterial. The
transformation_kind member transformation_algorithm_id [...] determines
whether the transformation performs authentication only (GMAC) or
authenticated encryption (GCM). The selection between these two options
shall be done according to the setting of the RTPS Protection Kind".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_gcm_or_gmac_by_the_protection_kind_and_width`, `crates/security-crypto/src/plugin/spec.rs::tests::register_local_participant_fills_the_adjusted_algorithm_info`

**Status:** `done` — GCM oder GMAC nach der RTPS Protection Kind (`is_rtps_axk_encrypted`), Breite nach der Property; `adjusted_algorithm_info` gefüllt. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — register_matched_remote_participant

**Spec:** §10.5.3.1, Table 78, S. 311 — "The Participant2ParticipantKeyMaterial
transformation_kind, master_salt, and master_sender_key, and sender_key_id
shall match those of the ParticipantKeyMaterial. [...] This operation also
creates a KeyMaterial_AES_GCM_GMAC object derived from the SharedSecret
passed as a parameter. [...] Participant2ParticipantKxKeyMaterial."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_participant2participant_material_matches_the_participant_key`, `crates/security-crypto/src/plugin/spec.rs::tests::the_plugin_accepts_only_the_supported_algorithms`

**Status:** `done` — Kind, Salz, Schlüssel und Kennung wie das ParticipantKeyMaterial; Kx-Material aus dem SharedSecret. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — register_local_datawriter: WriterKeyMaterialSeq

**Spec:** §10.5.3.1, Table 78, S. 311–312 — "If the Metadata Protection Kind
is different from NONE, then the operation shall create a
KeyMaterial_AES_GCM_GMAC to use for the encode_datawriter_submessage
operation. In addition, this key material shall appear as the first element
in the KeyMaterial_AES_GCM_GMAC_Seq. If the Data Protection Kind is different
from NONE, then the operation shall create a KeyMaterial_AES_GCM_GMAC to use
for the encode_serialized_payload operation. In the case where both meta-data
protection and data protection are the same, it is allowed for an
implementation to reuse the same key material for both."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`

**Status:** `done` — Ein oder zwei Elemente nach Metadata und Data Protection Kind, Submessage-Schlüssel zuerst; die Arten kommen aus `EndpointSecurityConfig`. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — register_matched_remote_datareader und register_matched_remote_datawriter

**Spec:** §10.5.3.1, Table 78, S. 312–313 — "This operation shall create a new
KeyMaterial_AES_GCM_GMAC_Seq object [...] Writer2ReaderKeyMaterialSeq. The
first elements of the Writer2ReaderKeyMaterialSeq shall contain the elements
of the WriterKeyMaterialSeq. [...] Reader2WriterKeyMaterial. The
transformation_kind, master_salt, and master_sender_key, and sender_key_id
for the Reader2WriterKeyMaterial object shall match those in the DataReader
ReaderKeyMaterial."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — Writer2ReaderKeyMaterialSeq und Reader2WriterKeyMaterial als Operationen der Spec; Empfängerschlüssel je Gegenstelle. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — register_local_datareader: ReaderKeyMaterial

**Spec:** §10.5.3.1, Table 78, S. 312–313 — "This operation shall create a new
KeyMaterial_AES_GCM_GMAC object [...] ReaderKeyMaterial. The
transformation_kind. member transformation_algorithm_id for the
ReaderKeyMaterial object determines whether the transformation performs
authentication only (GMAC) or encryption followed by authentication (GCM)."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_datawriter_has_one_or_two_key_materials`, `crates/security-crypto/src/plugin/spec.rs::tests::the_endpoints_get_receiver_specific_keys_under_origin_authentication`

**Status:** `done` — Ein Material; die Art folgt dem Submessage-Attribut (die Spec nennt hier die Data Protection Kind, ein Widerspruch in der Spec; Reader schützen nur Submessages). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — revise_local_entity_keys und activate_key_revision

**Spec:** §10.5.3.1, Table 78, S. 313–314 — "The transformation_kind shall
only change the key_revision member. The master_salt shall be entirely new.
The sender_key_id shall remain the same. [...] This operation shall not
create new Key Material for the BuiltinParticipantVolatileMessageSecureWriter
and BuiltinParticipantVolatileMessageSecureReader."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_key_revision_keeps_the_key_id_and_changes_the_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::the_participant_key_is_revised_with_the_entities`

**Status:** `done` — Neues Salz und neuer Schlüssel, Kennung und Empfängerschlüssel bleiben. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.1 Table 78 — unregister_participant, unregister_datawriter, unregister_datareader

**Spec:** §10.5.3.1, Table 78, S. 314 — "Releases any resources allocated on
the corresponding call to register_local_participant, or
register_matched_remote_participant."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyFactory; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::unregistering_releases_the_key_material_and_the_matches`

**Status:** `done` — Geben die Ressourcen der Registrierungen frei. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.2 Table 79 — create_local_participant_crypto_tokens und set_remote_participant_crypto_tokens

**Spec:** §10.5.3.2, Table 79, S. 314 — "Creates a DDS:Crypto:AES-GCM-GMAC
CryptoToken object and returns it in the output sequence. The CryptoToken
contains the Participant2ParticipantKeyMaterial created on the call to
register_matched_remote_participant [...] Shall receive the sequence
containing one CryptoToken object".

**Repo:** `crates/security-crypto/src/plugin.rs::create_local_participant_crypto_tokens`
(Z. 1430), `set_remote_participant_crypto_tokens` (Z. 1450)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cross_plugin_interop_via_tokens`,
`crates/security-runtime/src/shared.rs::tests::data_round_trips_via_token_after_kx_register`

**Status:** done

### §10.5.3.2 Table 79 — Token der Endpunkte und return_crypto_tokens

**Spec:** §10.5.3.2, Table 79, S. 314 — "create_local_datawriter_crypto_tokens
[...] The CryptoToken contains the Writer2ReaderKeyMaterial [...]
create_local_datareader_crypto_tokens [...] The CryptoToken contains the
Reader2WriterKeyMaterial [...] return_crypto_tokens Releases the resources".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoKeyExchange; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::crypto_tokens_are_returned_once`, `crates/security-crypto/src/plugin/spec.rs::tests::a_relay_only_reader_gets_the_submessage_key_alone`

**Status:** `done` — Writer2Reader- und Reader2Writer-Material in den Token; `return_crypto_tokens` gibt frei. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — encode_serialized_payload

**Spec:** §10.5.3.3.1, Table 80, S. 315 — "If the transformation_kind
indicates that encryption is performed, then the output shall be the three
RTPS Submessage elements: CryptoHeader, CryptoContent, and CryptoFooter [...]
If the transformation_kind indicates that only authentication is performed,
then the output shall be the three RTPS Submessage elements: CryptoHeader,
SerializedPayload, and CryptoFooter. [...] This operation shall always set
the receiver_specific_macs attribute in the CryptoFooter to the empty
sequence."

**Repo:** `crates/security-crypto/src/plugin.rs::encode_serialized_payload`
(Z. 1745), `encode_payload_from_mat` (Z. 918)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::serialized_payload_cross_plugin_via_tokens`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** done

### §10.5.3.3.1 Table 80 — encode_datawriter_submessage und encode_datareader_submessage

**Spec:** §10.5.3.3.1, Table 80, S. 315–316 — "If the transformation_kind
indicates that encryption is performed, then the output shall be the three
RTPS Submessages: SecurePrefixSubMsg, SecureBodySubMsg, and
SecurePostfixSubMsg. [...] only authentication [...] SecurePrefixSubMsg,
InputSubmessage, and SecurePostfixSubMsg. [...] Depending on the
configuration the operation may compute and set the common_mac and the
receiver_specific_macs".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_receiver_specific_macs_follow_the_list_index`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — `receiver_specific_macs` je Empfänger der Liste. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — encode_rtps_message ohne AAD

**Spec:** §10.5.3.3.1, Table 80, S. 316–317 — "1) Transformation when
'Additional Authenticated Data (AAD)' is disabled Let RTPSMessage{RTPSHdr->
InfoSourceSubMsg} represent the input RTPS Message transformed so that the
RTPS Header is replaced with an RTPS InfoSourceSubMsg [...] The
SecureRTPSPrefixSubMsg flag AuthenticatedDataFlag shall be unset. The
SecureRTPSPrefixSubMsg PreSharedKeyFlag shall be set if encode_rtps_message
was called with transform_with_psk=TRUE."

**Repo:** `crates/security-crypto/src/plugin.rs::encode_rtps_message_cyclone`
(Z. 1655, INFO_SRC vor dem Inhalt), `assemble_srtps_message` (Z. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`,
`crates/security-runtime/src/shared.rs::tests::transform_outbound_for_encrypt_produces_srtps_wire`

**Status:** done — bei GMAC entfällt SEC_BODY wie in §10.5.3.3.4.6.2; das
Flag P wird nie gesetzt, da kein Schutz mit Pre-Shared Key besteht.

### §10.5.3.3.1 Table 80 — encode_rtps_message mit AAD und HeaderExtension

**Spec:** §10.5.3.3.1, Table 80, S. 317–318 — "2) Transformation when
'Additional Authenticated Data (AAD)' is enabled [...] The
SecureRTPSPrefixSubMsg flag AdditionlAuthenticatedDataFlag shall be set.
[...] 3) [...] the HeaderExtension, if present, shall be adjusted as follows:
[...] messageLength element, if present, set to zero [...] messageChecksum
element, if present, set to zero".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::an_rtps_message_goes_through_the_interface_with_aad_and_a_header_extension`, `crates/security-crypto/src/plugin.rs::tests::the_header_extension_is_adjusted_for_the_aad_and_the_decoded_message`

**Status:** `done` — Flag A, Header und HeaderExtension (Länge und Prüfsumme null) als AAD, danach `messageLength` und `messageChecksum` (CRC-32C, CRC-64/XZ, MD5) der kodierten Nachricht; Einstellung `org.zerodds.crypto.rtps_additional_authenticated_data` (Vorgabe aus: INFO_SRC-Form). Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — decode_rtps_message

**Spec:** §10.5.3.3.1, Table 80, S. 318–319 — "Examines the
SecureRTPSPrefixSubMsg to determine the transformation_kind matches the one
the receiving DomainParticipant is expecting [...] Uses source
DomainParticipant GUIDs in the RTPS Header to locate the
sending_participant_crypto and receiving_participant_crypto. Then looks
whether the transformation_key_id attribute in the CryptoTransformIdentifier
is associated with those ParticipantCryptoHandles. If the association is not
found, the operation shall fail with an exception."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::an_rtps_message_goes_through_the_interface_without_aad`, `crates/security-crypto/src/plugin/spec.rs::tests::a_kind_other_than_the_expected_one_is_refused`, `crates/security-runtime/src/shared.rs::tests::classify_inbound_srtps_from_known_peer_accepts`

**Status:** `done` — Erwartete Art, Schlüssel nur der Handles von Sender und Empfänger. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 decode_rtps_message: Schlüssel nur des Senders, erwartete Art

**Spec:** §10.5.3.3.1, Table 80, S. 318 — "If the kind is not the expected
one, the operation shall fail with an exception. [...] If the association is
not found, the operation shall fail with an exception."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_kind_other_than_the_expected_one_is_refused`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_is_taken_only_from_the_entities_it_is_associated_with`, `crates/security-runtime/src/shared.rs::tests::a_key_of_another_peer_does_not_decode`

**Status:** `done` — Die Art (GCM/GMAC, Origin Authentication, Algorithmus in `supported_mask`) wird geprüft; ein Schlüssel eines anderen Senders öffnet nicht. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — preprocess_secure_submsg

**Spec:** §10.5.3.3.1, Table 80, S. 319 — "1. Determine whether the
CryptoTransformIdentifier the transformation_kind matches one of the
recognized kinds. 2. Classify the RTPS Submessage as a Writer or Reader
Submessage. 3. Retrieve the DatawriterCryptoHandle and DataReaderCryptoHandle
handles associated with the CryptoTransformIdentifier transformation_key_id."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::preprocess_refuses_unknown_kinds_and_unknown_keys`, `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`

**Status:** `done` — Erkannte Art, Klassifizierung, Handles. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — decode_datawriter_submessage und decode_datareader_submessage

**Spec:** §10.5.3.3.1, Table 80, S. 319–320 — "If the
RemoteDatawriterKeyMaterial specified a transformation_kind different from
CRYPTO_ALGORITHM_ID_NONE, then the operation shall check that the received
SecurePostfixSubMsg contains a common_mac and use it to verify the RTPS
SubMessage [...] If the RemoteDatawriter2DatareaderKeyMaterial specified a
receiver_specific_key_id different from zero, then the operation shall check
that the received SecurePostfixSubMsg contains a non-zero length
receiver_specific_macs element".

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::submessages_go_through_preprocess_and_decode`, `crates/security-crypto/src/plugin/spec.rs::tests::a_kind_other_than_the_expected_one_is_refused`

**Status:** `done` — `common_mac` und MAC des Empfängers werden verlangt und geprüft. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.1 Table 80 — decode_serialized_payload

**Spec:** §10.5.3.3.1, Table 80, S. 320 — "Uses writerGUID and the readerGUID
in the RTPS SubMessage to locate the sending_datawriter_crypto and
receiving_datareader_crypto. Then looks whether the transformation_key_id
attribute in the CryptoTransformIdentifier in the CryptoHeader
SubmessageElement is associated with those CryptoHandles. If the association
is not found, the operation shall fail with an exception."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::the_serialized_payload_goes_through_the_interface`, `crates/security-crypto/src/plugin/spec.rs::tests::a_key_is_taken_only_from_the_entities_it_is_associated_with`

**Status:** `done` — Schlüssel über Kennung und Revision unter den mit den Handles verbundenen Schlüsseln. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.2 Table 81 — Zustand der virtuellen Maschine

**Spec:** §10.5.3.3.2, Table 81, S. 320–322 — MasterKey, MasterSalt,
MasterKeyId, MasterReceiverSpecificKey, InitializationVectorSuffix,
SessionId, SessionKey, SessionReceiverSpecificKey, session_block_counter,
max_blocks_per_session.

**Repo:** `crates/security-crypto/src/plugin.rs::KeyMaterial`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_session_id_changes_after_max_blocks_per_session`, `crates/security-crypto/src/plugin/spec.rs::tests::the_initialization_vector_suffix_starts_random_and_counts`, `crates/security-crypto/src/plugin/spec.rs::tests::the_receiver_specific_macs_follow_the_list_index`

**Status:** `done` — MasterReceiverSpecificKey, `session_block_counter`, `max_blocks_per_session`, zufälliger Start des IV-Suffix. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.2 und §10.5.3.3.4 Neue SessionId nach max_blocks_per_session

**Spec:** §10.5.3.3.2, §10.5.3.3.4, S. 321, 322 — "If the session_block_counter
exceeds this value, a new SessionKey and SessionReceiverSpecificKey are
computed and the session_block_counter is reset to zero. [...] If the
operation detects that the counter would exceed the maximum then it should
modify the SessionId and derive new session keys prior to transforming any of
the input plain text."

**Repo:** `crates/security-crypto/src/plugin.rs::session_for`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_session_id_changes_after_max_blocks_per_session`

**Status:** `done` — Die SessionId wird erhöht und der Zähler zurückgesetzt, bevor Klartext transformiert wird. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.3 SessionKey aus MasterKey, MasterSalt und SessionId

**Spec:** §10.5.3.3.3, S. 322 — "SessionKey := HMAC256(MasterKey,'SessionKey'
| MasterSalt | SessionId) [...] In case a 128 key is desired the 256 bit HMAC
is truncated to the first 128 bits."

**Repo:** `crates/security-crypto/src/session_key.rs::derive_session_key_cyclone`
(Z. 95)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/security-crypto/src/session_key.rs::tests::session_key_aes128_gcm_truncated_to_16_byte`

**Status:** done — die Funktion `derive_session_key` (Z. 65) daneben setzt
MasterSalt vor die Zeichenkette; sie wird nur von Tests aufgerufen, ihre
Doku am Modulkopf gibt diese Reihenfolge als die der Spec an.

### §10.5.3.3.3 SessionReceiverSpecificKey

**Spec:** §10.5.3.3.3, S. 322 — "SessionReceiverSpecificKey :=
HMAC256(MasterReceiverSpecificKey, 'SessionReceiverKey' | MasterSalt |
SessionId)".

**Repo:** `crates/security-crypto/src/session_key.rs::derive_session_receiver_key_cyclone`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_receiver_specific_mac_is_the_one_of_the_spec` (mit Python aus der Formel gerechnet)

**Status:** `done` — `HMAC256(MasterReceiverSpecificKey, "SessionReceiverKey" | MasterSalt | SessionId)`, auf die Schlüssellänge gekürzt, das Salz auf die Schlüssellänge geschnitten wie bei `derive_session_key_cyclone`. `derive_session_hmac_key` (Salz vor der Kennung) ist entfernt.

### §10.5.3.3.4 InitializationVector = SessionId | InitializationVectorSuffix

**Spec:** §10.5.3.3.4, S. 322 — "The encryption operation uses a 96-bit
initialization vector constructed as: InitializationVector = SessionId |
InitializationVectorSuffix [...] It shall be incremented each time any of
those keys are used to encrypt and/or create a MAC."

**Repo:** `crates/security-crypto/src/plugin.rs::seal_cyclone` (Z. 278)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::two_encrypts_produce_different_ciphertexts`

**Status:** done — der Suffix beginnt bei 1, nicht zufällig wie in Table 81.

### §10.5.3.3.4.1 bis §10.5.3.3.4.3 Formate von CryptoHeader, CryptoContent und CryptoFooter

**Spec:** §10.5.3.3.4.1–§10.5.3.3.4.3, S. 323–324 — CryptoHeader mit
transformation_kind, transformation_key_id, session_id, init_vector_suffix;
CryptoContent mit `long crypto_content.length = N`; CryptoFooter mit
`common_mac` und `receiver_specific_macs.length`, alles in Big Endian.

**Repo:** `crates/security-crypto/src/plugin.rs::push_sec_body` (Z. 1106),
`assemble_secure_submessages` (Z. 1116)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::sec_body_is_padded_to_four_for_any_ciphertext_length`,
`crates/security-crypto/src/crypto_transform.rs::tests::footer_round_trip_no_receivers`

**Status:** done — der Satz, N sei immer ein Vielfaches von 16, trifft auf
AES-GCM nicht zu (Widerspruch in der Spec); ZeroDDS füllt SEC_BODY auf 4 Oktette auf.

### §10.5.3.3.4.4 Ergebnis von encode_serialized_payload

**Spec:** §10.5.3.3.4.4, S. 324–325 — Nur Authentisierung: CryptoHeader,
unveränderte SerializedPayload, CryptoFooter; Verschlüsselung: CryptoHeader,
CryptoContent, CryptoFooter; "The common_mac in the CryptoFooter is the
authentication tag generated by the same AES-GCM where the Additional
Authenticated Data is empty."

**Repo:** `crates/security-crypto/src/plugin.rs::encode_payload_from_mat`
(Z. 918)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::serialized_payload_cross_plugin_via_tokens`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_data_protection_all_kinds`

**Status:** done

### §10.5.3.3.4.5 Ergebnis von encode_datawriter_submessage und encode_datareader_submessage

**Spec:** §10.5.3.3.4.5, S. 325–326 — Nur Authentisierung: SEC_PREFIX,
unveränderte Submessage, SEC_POSTFIX; Verschlüsselung: SEC_PREFIX, SEC_BODY,
SEC_POSTFIX; AAD leer.

**Repo:** `crates/security-crypto/src/plugin.rs::assemble_secure_submessages`
(Z. 1116), `seal_cyclone` (Z. 278)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::cyclone_volatile_submessage_roundtrips`,
`crates/dcps/tests/security_matrix_e2e.rs::matrix_metadata_protection_all_kinds`

**Status:** done

### §10.5.3.3.4.6.1 und §10.5.3.3.4.6.3 encode_rtps_message mit AAD

**Spec:** §10.5.3.3.4.6.1, §10.5.3.3.4.6.3, S. 326–328 — "SRTPS_PREFIX |
(flags) A E [...] The common_mac in the CryptoFooter is the authentication tag
generated by the AES-GMAC transformation [...] where the Additional
Authenticated Data is set to the RTPS Header and RTPS HeaderExtension."

**Repo:** `crates/security/src/cryptographic.rs`::CryptoTransform; `crates/security-crypto/src/plugin/spec.rs`; Aufruf im Gate: `crates/security-runtime/src/gate_crypto.rs::CryptoLayer`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::an_rtps_message_goes_through_the_interface_with_aad_and_a_header_extension`

**Status:** `done` — GMAC und GCM mit Header und HeaderExtension als AAD. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.4.6.2 und §10.5.3.3.4.6.4 encode_rtps_message ohne AAD

**Spec:** §10.5.3.3.4.6.2, §10.5.3.3.4.6.4, S. 327–329 — RTPS-Header
unverändert, SRTPS_PREFIX mit CryptoHeader, bei GMAC
RTPSMessage{RTPSHdr -> InfoSourceSubMsg} im Klartext, bei GCM SEC_BODY mit
dem verschlüsselten Inhalt, SRTPS_POSTFIX; "If Additional Authenticated Data
(AAD)' is not enabled the input RTPS Message cannot contain an RTPS Header
Extension."

**Repo:** `crates/security-crypto/src/plugin.rs::encode_rtps_message_cyclone`
(Z. 1655), `assemble_srtps_message` (Z. 956)

**Tests:** `crates/security-crypto/src/plugin.rs::tests::srtps_cyclone_cross_instance_roundtrip`,
`crates/security-runtime/src/shared.rs::tests::transform_outbound_for_sign_also_uses_srtps_encoder`

**Status:** done

### §10.5.3.3.5 Computation of plaintext from ciphertext

**Spec:** §10.5.3.3.5, S. 329 — "The decrypt operation first checks that the
CryptoTransformIdentifier attribute in the CryptoHeader has the proper
transformation_kind and also uses the CryptoTransformIdentifier
transformation_key_id to locate the MasterKey, and MasterSalt. [...] If the
MasterKeyId is not found associated with the crypto handle the operation
shall fail. The session_id attribute within the CryptoHeader is used to
obtain the proper SessionReceiverSpecificKeys and SessionKey."

**Repo:** `crates/security-crypto/src/plugin.rs::open_cyclone`; `crates/security-crypto/src/plugin/spec.rs`

**Tests:** `crates/security-crypto/src/plugin/spec.rs::tests::a_kind_other_than_the_expected_one_is_refused`, `crates/security-crypto/src/plugin.rs::tests::a_header_kind_other_than_the_keys_is_refused`

**Status:** `done` — Die Art im Kopf wird mit der erwarteten und der des Materials verglichen. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.5.3.3.6 Computation of the message authentication codes

**Spec:** §10.5.3.3.6, S. 329–330 — "There are two types of message
authentication codes (MACs) that may appear. The first stored in the
common_mac uses the SessionKey. [...] The second type, stored in the
receiver_specific_macs contains MACs that use different
SessionReceiverSpecificKey".

**Repo:** `crates/security-crypto/src/plugin.rs::seal_cyclone`, `footer_tail`, `receiver_mac`, `check_receiver_mac`, `open_submessage`

**Tests:** `crates/security-crypto/src/plugin.rs::tests::the_receiver_specific_mac_is_the_one_of_the_spec`, `::a_reader_checks_the_origin_of_a_submessage_by_its_own_key`, `::decrypt_rejects_tampered_ciphertext`

**Status:** `done` — `common_mac` unter dem SessionKey; die MACs für einzelne Empfänger sind der AES-GMAC auf dem `common_mac` unter dem SessionReceiverSpecificKey mit dem IV des Headers. Ein Empfänger mit einem Schlüssel für sich verlangt sein MAC und prüft es.

---

### §10.6 Builtin Logging Plugin: DDS:Logging:DDS_LogTopic

**Spec:** §10.6, S. 330 — "The builtin Logging Plugin is known as the
DDS:Logging:DDS_LogTopic. The DDS:Logging:DDS_LogTopic implements logging by
publishing information to a DDS Topic BuiltinLoggingTopic defined below."

**Repo:** `crates/security-logging/src/log_topic.rs::LogTopicLoggingPlugin`
(Z. 554), `plugin_class_id` (Z. 728)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::plugin_class_id_is_the_spec_name`

**Status:** done

### §10.6 BuiltinLoggingTopic: Name „DDS:Security:LogTopicV2“, Typ BuiltinLoggingTypeV2

**Spec:** §10.6, S. 330 — "The BuiltinLoggingTopic shall have the Topic name
'DDS:Security:LogTopicV2'. The BuiltinLoggingTopic shall have the Type
BuiltinLoggingTypeV2 defined in the IDL below."

**Repo:** `crates/security/src/logging.rs::TOPIC_LOG_TOPIC_V2` (Z. 42);
`crates/dcps/src/security_log_topic.rs::log_topic_publisher_for_participant`
(Z. 344), `DdsType for BuiltinLoggingTypeV2` (Z. 122, `TYPE_NAME`)

**Tests:** `crates/dcps/tests/security_log_topic_e2e.rs::a_security_event_reaches_the_v2_log_topic`,
`crates/dcps/tests/security_log_topic_e2e.rs::the_topic_names_are_the_clause_names`,
`crates/dcps/src/security_log_topic.rs::tests::the_type_names_are_the_idl_names`

**Status:** done

### §10.6 Topic „DDS:Security:LogTopic“ der Vorversion wahlweise, zwei DataWriter

**Spec:** §10.6, S. 330 — "Implementors of DDS-Security 1.2 may optionally
provide mechanisms that configure a DomainParticipant to publish the legacy
BuiltinLoggingTopic, that is, topic name 'DDS:Security:LogTopic' and type
'BuiltinLoggingType'. [...] Note that publishing both Topics requires two
separate DataWriters."

**Repo:** `crates/security/src/logging.rs::TOPIC_LOG_TOPIC` (Z. 47);
`crates/dcps/src/security_log_topic.rs::log_topic_publisher_for_participant`
(Z. 344, zweiter DataWriter bei `publish_legacy`);
`crates/security-logging/src/from_properties.rs::PROP_LOG_BUILTIN_LEGACY_TOPIC`
(Z. 76)

**Tests:** `crates/dcps/tests/security_log_topic_e2e.rs::the_legacy_topic_is_published_when_configured`,
`crates/security-logging/src/log_topic.rs::tests::legacy_topic_is_published_when_asked`

**Status:** done — eingeschaltet über die Property
`dds.sec.log.builtin.DDS_LogTopic.legacy_topic`, einen Namen im reservierten
Raum `dds.sec.` (siehe §7.3.2).

### §10.6 DataReader der Anwendung für LogTopicV2

**Spec:** §10.6, S. 330 — "Users of DDS-Security 1.2 that want to receive the
secure log messages shall create a DataReader for topic
'DDS:Security:LogTopicV2' with type BuiltinLoggingTypeV2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Vorgabe an Anwendungen, nicht an die
Implementierung.

### §10.6 enum LoggingLevel

**Spec:** §10.6, S. 330 — "enum LoggingLevel { EMERGENCY_LEVEL, ALERT_LEVEL,
CRITICAL_LEVEL, ERROR_LEVEL, WARNING_LEVEL, NOTICE_LEVEL, INFORMATIONAL_LEVEL,
DEBUG_LEVEL };"

**Repo:** `crates/security-logging/src/log_topic.rs::LoggingLevel` (Z. 59);
`crates/dcps/src/security_log_topic.rs::severity_from_u32` (Z. 50)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::an_unknown_severity_is_refused_not_guessed`,
`crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_writes_the_idl_octets`

**Status:** done — Werte 0 bis 7 in der Reihenfolge der IDL, auf der Leitung
32 Bit. §9.6.2.1.1 nennt andere Stufen (FATAL_LEVEL bis TRACE_LEVEL; Widerspruch in der Spec).

### §10.6 NameValuePair, NameValuePairSeq und structured_data

**Spec:** §10.6, S. 330–331 — "@extensibility(FINAL) struct NameValuePair {
string name; string value; }; typedef sequence<NameValuePair>
NameValuePairSeq;" und "map<string, NameValuePairSeq> structured_data;"

**Repo:** `crates/security-logging/src/log_topic.rs::NameValuePair` (Z. 199),
`NameValuePairSeq` (Z. 207);
`crates/dcps/src/security_log_topic.rs::write_structured_data` (Z. 85)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_writes_the_idl_octets`,
`crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_round_trips`

**Status:** done — Map und Sequenz tragen je einen DHEADER, die Elemente
von NameValuePair keinen.

### §10.6 LegacyTime_t und Time_t

**Spec:** §10.6, S. 330–331 — "struct LegacyTime_t { long sec; unsigned long
nanosec; }; struct Time_t { long long sec; unsigned long nanosec; };"

**Repo:** `crates/security-logging/src/log_topic.rs::LegacyTime` (Z. 214),
`Time` (Z. 226)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::the_legacy_codec_round_trips_with_a_32_bit_second`,
`crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_writes_the_idl_octets`

**Status:** done

### §10.6 BuiltinLoggingType (@appendable)

**Spec:** §10.6, S. 331 — "@extensibility(APPENDABLE) struct
BuiltinLoggingType { octet facility; LoggingLevel severity; LegacyTime_t
timestamp; string hostname; string hostip; string appname; string procid;
string msgid; string message; map<string, NameValuePairSeq> structured_data;
};"

**Repo:** `crates/security-logging/src/log_topic.rs::BuiltinLoggingType`
(Z. 271); `crates/dcps/src/security_log_topic.rs::DdsType for BuiltinLoggingType`
(Z. 179)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::the_legacy_codec_round_trips_with_a_32_bit_second`,
`crates/dcps/tests/security_log_topic_e2e.rs::the_legacy_topic_is_published_when_configured`

**Status:** done

### §10.6 BuiltinLoggingTypeV2 (@appendable)

**Spec:** §10.6, S. 331 — "@extensibility(APPENDABLE) struct
BuiltinLoggingTypeV2 { octet facility; LoggingLevel severity; Time_t
timestamp; string hostname; [...] map<string, NameValuePairSeq>
structured_data; };"

**Repo:** `crates/security-logging/src/log_topic.rs::BuiltinLoggingTypeV2`
(Z. 297); `crates/dcps/src/security_log_topic.rs::DdsType for BuiltinLoggingTypeV2`
(Z. 122, XCDR2 mit DHEADER)

**Tests:** `crates/dcps/src/security_log_topic.rs::tests::the_v2_codec_writes_the_idl_octets`,
`crates/dcps/src/security_log_topic.rs::tests::a_reader_skips_unknown_trailing_member_octets`

**Status:** done — der Vergleichswert des Tests ist im selben Repository von
Hand gebaut; ein Abgleich mit Bytes anderer Hersteller liegt nicht vor.

### §10.6 Belegung der Felder: facility 0x0A, timestamp, hostname bis message

**Spec:** §10.6, S. 331 — "octet facility; // Set to 0x0A (10). Indicates
sec/auth msgs [...] Time_t timestamp; // Since epoch 1970-01-01 00:00:00
+0000 (UTC) string hostname; // IP host name of originator string hostip; //
IP address of originator string appname; // Identify the device or
application string procid; // Process name/ID for syslog system string msgid;
// Identify the type of message string message; // Free-form message".

**Repo:** `crates/security-logging/src/log_topic.rs::FACILITY_SEC_AUTH`
(Z. 51), `BuiltinLoggingTypeV2::from_log_event` (Z. 407),
`BuiltinLoggingType::from_log_event` (Z. 433), `Time::now` (Z. 236)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::an_event_fills_every_field_from_the_clause`,
`crates/security-logging/src/log_topic.rs::tests::legacy_topic_is_published_when_asked`

**Status:** done — `hostname` aus `HOSTNAME`/`COMPUTERNAME`, `hostip` aus
`ZERODDS_HOSTIP`, sonst leer; `msgid` ist das Argument `category`.

### §10.6 Eingebaute Regel des Topics im Plugin DDS:Access:Permissions

**Spec:** §10.6, S. 331 — "Knowledge of the BuiltinLoggingTopic shall be
builtin into the DDS:Access:Permissions AccessControl plugin and it shall be
treated according to the following topic rule: <topic_rule>
<topic_expression>DDS:Security:LogTopic</topic_expression>
<enable_discovery_protection>FALSE</enable_discovery_protection>
<enable_read_access_control>TRUE</enable_read_access_control>
<enable_write_access_control>FALSE</enable_write_access_control>
<metadata_protection_kind>SIGN</metadata_protection_kind>
<data_protection_kind>ENCRYPT</data_protection_kind> </topic_rule>"

**Repo:** `crates/security-permissions/src/governance.rs::builtin_topic_rule`
(Z. 115), `effective_topic_rule` (Z. 392, die eingebaute Regel geht dem
Governance-Dokument vor)

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_builtin_log_topic_rule_allows_any_write_and_gates_reads`

**Status:** done — die Regel gilt für beide Namen. Der Text nennt als
`topic_expression` nur den Namen des Topics der Vorversion (Widerspruch in der Spec).

### §10.6 Schreiben ohne Grant, Lesen nur mit Grant

**Spec:** §10.6, S. 331 — "The above rule states that any DomainParticipant
with permission necessary to join the DDS Domain shall be allowed to write the
BuiltinLoggingTopic but in order to read the BuiltinLoggingTopic the
DomainParticipant needs to have a grant for the BuiltinLoggingTopic in its
permissions document."

**Repo:** `crates/security-permissions/src/governance.rs::check_create_datawriter`, `check_create_datareader`; `crates/security-runtime/src/shared.rs::check_remote_datawriter`, `check_remote_datareader`

**Tests:** `crates/security-permissions/src/governance.rs::tests::the_builtin_log_topic_rule_allows_any_write_and_gates_reads`,
`crates/security-runtime/src/shared.rs::tests::a_log_topic_is_written_without_a_grant_and_read_only_with_one`

**Status:** `done` — lokal und für entfernte Endpunkte, beide Namen des Log-Topics.
### §10.6 Schutz der Endpunkte von DDS:Security:LogTopicV2 nach der eingebauten Regel

**Spec:** §10.6, S. 331 — "<metadata_protection_kind>SIGN</metadata_protection_kind>
<data_protection_kind>ENCRYPT</data_protection_kind>".

**Repo:** `crates/security-runtime/src/shared.rs::SharedSecurityGate::endpoint_protection_kinds`, `topic_*_protected` (über `effective_topic_rule`)

**Tests:** `crates/dcps/src/runtime.rs::tests::the_endpoints_of_the_log_topics_are_protected_by_the_builtin_rule`,
`crates/security-runtime/src/shared.rs::tests::the_builtin_log_topic_rule_carries_its_protection`

**Status:** `done` — Schlüsselmaterial, angekündigte Maske und Read-/Write-Schutz der Endpunkte beider Log-Topics folgen der eingebauten Regel (SIGN, ENCRYPT, Lesen mit Grant).
### §10.6.1 Table 82 — set_log_options: DataWriter nur bei distribute

**Spec:** §10.6.1, Table 82, S. 332 — "If the distribute parameter is set to
TRUE, the DDS:Logging:DDS_LogTopic shall create a DataWriter to send the
BuiltinLoggingTopic if it is FALSE, it shall not."

**Repo:** `crates/dcps/src/participant.rs` (Z. 638–642, Z. 758:
`log_topic_publisher_for_participant` nur bei `distribute`);
`crates/security-logging/src/from_properties.rs::logging_plugin_from_properties_with_publisher`
(Z. 259)

**Tests:** `crates/dcps/tests/security_log_properties_e2e.rs::builtin_sink_with_distribute_off_is_file_only`,
`crates/security-logging/src/from_properties.rs::tests::distribute_off_needs_no_publisher`,
`crates/security-logging/src/log_topic.rs::tests::distribute_off_publishes_nothing`

**Status:** done — den DataWriter legt der Participant bei seiner Erzeugung
an, nicht `LogTopicLoggingPlugin::set_log_options`; diese Methode erhält den
Publisher bei der Konstruktion des Plugins.

### §10.6.1 Table 82 — set_log_options: log_file öffnen

**Spec:** §10.6.1, Table 82, S. 332 — "The plugin shall open a file with the
name indicated in the log_file parameter."

**Repo:** `crates/security-logging/src/log_topic.rs::set_log_options`
(Z. 624), `open_log_file` (Z. 183)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::an_unopenable_log_file_is_refused`

**Status:** done

### §10.6.1 Table 82 — set_log_options: log_level speichern

**Spec:** §10.6.1, Table 82, S. 332 — "The plugin shall remember the value of
the log_level so that it can be used during the log operation."

**Repo:** `crates/security-logging/src/log_topic.rs::set_log_options`
(Z. 624)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::options_cannot_change_after_logging_is_enabled`,
`crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`

**Status:** done

### §10.6.1 Table 82 — log: ohne enable_logging keine Aktion

**Spec:** §10.6.1, Table 82, S. 332 — "This operation shall check if logging
was enabled by a prior call to enable_logging and if not it shall return
without performing any action."

**Repo:** `crates/security-logging/src/control.rs::LogControl::admit`; `LogTopicLoggingPlugin::log`

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_before_enable_logging_does_nothing`,
`crates/security-logging/src/control.rs::tests::log_before_enable_logging_does_nothing`

**Status:** `done` — vor `enable_logging` entsteht weder eine Dateizeile noch eine Veröffentlichung noch ein Listener-Aufruf.
### §10.6.1 Table 82 — log: Vergleich mit dem gespeicherten log_level

**Spec:** §10.6.1, Table 82, S. 332 — "If the log_level parameter value is
greater than the one saved by the set_log_options operation, the operation
shall return without performing any action. If the log_level parameter value
is less than or equal to the one saved, the log operation shall perform two
actions".

**Repo:** `crates/security-logging/src/log_topic.rs::log` (Z. 712)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::events_above_the_level_are_dropped_and_at_or_below_pass`,
`crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`

**Status:** done

### §10.6.1 Table 82 — log: Zeile an log_file anhängen

**Spec:** §10.6.1, Table 82, S. 332 — "It shall append a string
representation of the parameters passed to the log operation to the end of
the file opened by the set_log_options operation."

**Repo:** `crates/security-logging/src/control.rs::file_line`

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::a_field_with_a_newline_stays_on_one_line`

**Status:** done — eine Zeile je Meldung; Zeilenumbrüche in Feldern werden
maskiert.

### §10.6.1 Table 82 — log: Veröffentlichung bei distribute

**Spec:** §10.6.1, Table 82, S. 332 — "If the value of the distribute option
was set on the call to set_log_options, the plugin shall fill an object of
type BuiltinLoggingType with the values passed as arguments to the log
operation and publish it using the DataWriter associated with the
BuiltinLoggingTopic created by the set_log_options operation."

**Repo:** `crates/security-logging/src/log_topic.rs::log` (Z. 723),
`publish` (Z. 677)

**Tests:** `crates/dcps/tests/security_log_topic_e2e.rs::a_security_event_reaches_the_v2_log_topic`,
`crates/security-logging/src/log_topic.rs::tests::distribute_off_publishes_nothing`

**Status:** done — veröffentlicht wird BuiltinLoggingTypeV2 auf
`DDS:Security:LogTopicV2` wie in §10.6, auf Wunsch zusätzlich
BuiltinLoggingType. Table 82 nennt BuiltinLoggingType (Widerspruch in der Spec).

### §10.6.1 Table 82 — enable_logging

**Spec:** §10.6.1, Table 82, S. 332 — "This operation shall save the fact
that logging was enabled such that the information can be used by the log
operation."

**Repo:** `crates/security-logging/src/log_topic.rs::enable_logging`
(Z. 645)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::log_options_write_one_line_per_event_at_or_below_the_level`,
`crates/security-logging/src/log_topic.rs::tests::enable_without_options_is_refused`

**Status:** done

### §10.6.1 Table 82 — set_listener

**Spec:** §10.6.1, Table 82, S. 332 — "This operation shall save a reference
to the LoggerListener such that the listener is be notified each time a log
message is produced."

**Repo:** `crates/security-logging/src/log_topic.rs::LogTopicLoggingPlugin` (`LoggingPlugin::set_listener`)

**Tests:** `crates/security-logging/src/log_topic.rs::tests::the_listener_is_told_of_each_message_the_sink_logs`,
`crates/security-logging/src/control.rs::tests::the_listener_hears_each_logged_message`

**Status:** `done` — der Listener wird bei jeder geloggten Meldung benachrichtigt.

---

### §10.7 DDS:Auth:PSK: keine Authentisierung, entdeckte Participants gelten als nicht authentisiert

**Spec:** §10.7, S. 332 — "The DDS:Auth:PSK plugin is mostly a 'NOOP' plugin
that constructs an IdentityHandle with the information the 'DDS:Access:PSK'
and 'DDS:Crypto:PSK need. The DDS:Auth:PSK plugin does not do any
Authentication treating all participants it discovers as 'Unauthenticated'
Participants."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — `DDS:Auth:PSK` authentisiert niemanden; `validate_remote_identity` schlägt immer fehl. Der Handshake mit Schlüsseln, der diesen Namen trug, ist `PskHandshakeAuthenticationPlugin` mit eigenen Klassen (`ZeroDDS:Auth:PSK-Handshake:1.0`).

### §10.7.1 Configuration: nur die Auswahl des Plugins

**Spec:** §10.7.1, S. 332 — "This plugin does not require any configuration
beyond selecting it to be used. The mechanism for selecting which plugins are
active is implementation specific."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — `validate_local_identity` braucht keine Property.

### §10.7.2 Nicht festgelegte Attribute von Credential und Token

**Spec:** §10.7.2, S. 333 — "Plugin implementations may place data in these
attributes as long as they also include a property attribute that allows the
implementation to unambiguously detect the presence and interpret these
attributes. Attributes that are not understood shall be ignored. Property_t
and BinaryProperty_t names shall comply with the rules defined in 7.3.1 and
7.3.3, respectively."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — das IdentityToken trägt nur `class_id`.

### §10.7.2.1 IdentityToken: class_id „DDS:Auth:PSK:1.2“

**Spec:** §10.7.2.1, S. 333 — "The DDS:Auth:PSK plugin shall set the class_id
attribute of the IdentityToken object to 'DDS:Auth:PSK:1.2' no other
attributes are specified or required."

**Repo:** `crates/security-pki/src/psk.rs::SPEC_CLASS_ID`, `PskAuthenticationPlugin::get_identity_token`

**Tests:** `crates/security-pki/src/psk.rs::tests::token_roundtrip_via_data_holder_codec`,
`crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** done — das Token des Spec-Plugins `PskAuthenticationPlugin` trägt nur die `class_id` und keine Properties (`the_spec_plugin_authenticates_nobody`); das Handshake-Plugin von ZeroDDS (`PskHandshakeAuthenticationPlugin`) hat eine eigene Klasse `ZeroDDS:Auth:PSK-Handshake:1.0` und trägt `dds.psk.identity_id` (siehe
§10.7.2). Table 52 (§10.3.2.1, S. 228) nennt für dieses Token die Property
`dds.psk.algo`, die §10.7.2.1 nicht verlangt (Widerspruch in der Spec);
ZeroDDS setzt sie nicht.

### §10.7.2.1 class_id aus PluginClassName, MajorVersion und MinorVersion

**Spec:** §10.7.2.1, S. 333 — "The value of the class_id shall be interpreted
as composed of three parts: a PluginClassName, a MajorVersion and a
MinorVersion [...] this version of the specification has PluginClassName
equal to 'DDS:Auth:PSK', MajorVersion set to 1, and MinorVersion set to 2."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`, `crates/security-pki/src/psk.rs::tests::the_handshake_classes_are_not_the_spec_class`

**Status:** `done` — das Plugin wertet die Klasse eines entfernten Tokens nicht aus; `validate_remote_identity` schlägt für jedes Token fehl.

### §10.7.2.2 bis §10.7.2.4 IdentityStatusToken, AuthenticatedPeerCredentialToken, AuthRequestMessageToken

**Spec:** §10.7.2.2 bis §10.7.2.4, S. 333 — "The DDS:Auth:PSK plugin does not
use this Token. There is no value specified for it."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — kein Wert festgelegt; die Rückgabe von
`get_identity_status_token` steht unter Table 83.

### §10.7.2.5 HandshakeMessageToken: nicht verwendet

**Spec:** §10.7.2.5, §10.7.2.5.1 bis §10.7.2.5.3, S. 333 — "There is no value
specified for it. [...] HandshakeRequestMessageToken objects The
DDS:Auth:PSK plugin does not use this Token. [...] HandshakeReplyMessageToken
The DDS:Auth:PSK plugin does not use this Token."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — das Plugin kennt keine Handshake-Tokens; die Operationen des Handshakes schlagen fehl.

### §10.7.3 Table 83 — validate_local_identity

**Spec:** §10.7.3, Table 83, S. 334 — "This operation shall receive the
participant_guid associated with the local DomainParticipant whose identity is
being validated. The operation shall always return VALIDATION_OK. The
operation shall set the output 16-byte adjusted_participant_guid GUID to the
same value as the input participant_guid."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — `validate_local_identity` liefert immer ein Handle (VALIDATION_OK); die GUID bleibt, wie sie ist.

### §10.7.3 Table 83 — get_identity_token

**Spec:** §10.7.3, Table 83, S. 334 — "This operation returns the Token
specified in 10.7.2.1".

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — Token der Klasse `DDS:Auth:PSK:1.2` ohne weitere Attribute.

### §10.7.3 Table 83 — get_identity_status_token

**Spec:** §10.7.3, Table 83, S. 334 — "This operation shall return TokenNIL."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — TokenNIL (leer).

### §10.7.3 Table 83 — set_participant_security_config

**Spec:** §10.7.3, Table 83, S. 334 — "This operation shall do nothing and
return TRUE."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin::set_participant_security_config`

**Tests:** `crates/security-pki/src/psk.rs::tests::table_83_set_participant_security_config_and_credential_do_nothing`

**Status:** `done` — tut nichts und liefert die gegebene Info zurück (TRUE).

### §10.7.3 Table 83 — set_permissions_credential_and_token

**Spec:** §10.7.3, Table 83, S. 334 — "This operation shall do nothing and
return TRUE."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin::set_permissions_credential_and_token`

**Tests:** `crates/security-pki/src/psk.rs::tests::table_83_set_participant_security_config_and_credential_do_nothing`

**Status:** `done` — tut nichts und liefert TRUE, auch für leere oder beliebige Token.

### §10.7.3 Table 83 — validate_remote_identity: immer VALIDATION_FAILED

**Spec:** §10.7.3, Table 83, S. 334 — "The operation shall always return
VALIDATION_FAILED."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — immer abgewiesen (`AuthenticationFailed`).

### §10.7.3 Table 83 — Operationen des Handshakes werden nicht aufgerufen

**Spec:** §10.7.3, Table 83, S. 334–335 — begin_handshake_request,
begin_handshake_reply, process_handshake, get_shared_secret,
get_authenticated_peer_credential_token, return_authenticated_peer_credential_token,
return_handshake_handle, return_sharedsecret_handle: "This operation does not
need to be implemented. It will not be called on the plugin given that
validate_remote_identity always returns VALIDATION_FAILED."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin`

**Tests:** `crates/security-pki/src/psk.rs::tests::the_spec_plugin_authenticates_nobody`

**Status:** `done` — `begin_handshake_request`, `begin_handshake_reply`, `process_handshake` und `shared_secret` schlagen fehl.

### §10.7.3 Table 83 — set_listener

**Spec:** §10.7.3, Table 83, S. 334 — "This operation shall save a reference
to the listener object and associate it with the specified IdentityHandle."

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin::set_listener`

**Tests:** `crates/security-pki/src/psk.rs::tests::table_83_set_listener_saves_the_listener`

**Status:** `done` — speichert den Listener (`None` löscht ihn); gerufen wird er nie, weil das Plugin nichts authentisiert und nichts widerruft.

### §10.7.3 Table 83 — return_identity_token, return_identity_status_token, return_identity_handle

**Spec:** §10.7.3, Table 83, S. 334–335 — "This operation shall behave as
specified in 9.3.2.11.14." (return_identity_token), "[...] 9.3.2.11.15."
(return_identity_status_token), "[...] 9.3.2.11.18." (return_identity_handle).

**Repo:** `crates/security-pki/src/psk.rs::PskAuthenticationPlugin::return_identity_token`, `return_identity_status_token`, `return_identity_handle`

**Tests:** `crates/security-pki/src/psk.rs::tests::table_83_the_return_operations_take_back_what_was_issued`

**Status:** `done` — verhalten sich wie in §9.3.2.11.14, .15 und .18: nur Ausgegebenes geht zurück, einmal; TokenNIL (der Status) wird als leerer Token zurückgenommen.

---

### §10.8 DDS:Access:PSK: alle Berechtigungen

**Spec:** §10.8, S. 335 — "The plugin implements the AccessControl plugin API
granting all permissions. Specifically: It allows the local application to
join any DDS domain, as well as publish and subscribe to any DDS Topic in that
domain. It allows a remote DomainParticipant to join any DDS domain, as well
as publish and subscribe to any DDS Topic in that domain."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — alle Prüfungen der Tabelle 85 erlauben; kein Dokument ist nötig.

### §10.8.1 Table 84 — dds.sec.access.rtps_psk_protection_kind

**Spec:** §10.8.1, Table 84, S. 335 — "rtps_psk_protection_kind One of the
following 3 string options: 'NONE', 'SIGN', or 'ENCRYPT'. [...] If not
specified it is treated as if it was specified to be 'ENCRYPT'. This property
must be configured consistently on all the DomainParticipants that join a DDS
Domain." Präfix `dds.sec.access.`, `propagate` FALSE.

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`::validate_local_permissions, `RtpsPskProtection`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — NONE, SIGN oder ENCRYPT, ENCRYPT ohne Angabe, anderer Wert wird abgewiesen.

### §10.8.2 und §10.8.2.1 Typen, PermissionsCredentialToken

**Spec:** §10.8.2.1, S. 336 — "The DDS:Access:PSK plugin does not interpret
the value of this Token. For this reason the value is implementation
specific."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — kein Wert festgelegt.

### §10.8.2.2 PermissionsToken: class_id „DDS:Access:PSK:1.2“

**Spec:** §10.8.2.2, S. 336 — "The DDS:Access:PSK plugin shall set the
class_id attributes to 'DDS:Access:PSK:1.2'. No other attributes need to be
set."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — Klasse `DDS:Access:PSK:1.2`.

### §10.8.2.3 bis §10.8.2.6 PluginParticipantSecurityAttributes, PluginEndpointSecurityAttributes und ihre Masken

**Spec:** §10.8.2.3 bis §10.8.2.6, S. 336 — "The
PluginParticipantSecurityAttributes describe plugin-specific behavior of the
associated cryptographic plugin affecting the key material and transformations
for the RTPS messages [...] The PluginEndpointSecurityAttributesMask is used
to encode the value of the PluginEndpointSecurityAttributes".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung; die Werte legt Table 85 fest
(get_participant_security_config, get_datawriter_security_config).

### §10.8.3 Table 85 — check_create_participant, check_remote_participant, check_create_topic

**Spec:** §10.8.3, Table 85, S. 336 — "This operation shall return TRUE."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::the_participant_topic_instance_and_match_checks_return_true`

**Status:** `done` — die drei Operationen liefern TRUE.

### §10.8.3 Table 85 — check_create_datawriter, check_create_datareader, check_remote_datawriter, check_remote_datareader

**Spec:** §10.8.3, Table 85, S. 336 — "This operation shall return TRUE."

**Repo:** `crates/security-permissions/src/psk_access.rs` (`check_create_datawriter`,
`check_create_datareader`, `check_remote_datawriter_match`,
`check_remote_datareader_match`)

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::every_check_returns_true`

**Status:** `done` — jede der Prüfungen liefert TRUE, auch für Topics, die ein Grant
verbieten würde; ein unbekanntes Handle bleibt ein Fehler.

### §10.8.3 Table 85 — Prüfungen für Instanzen, check_remote_topic, check_local_datawriter_match, check_local_datareader_match

**Spec:** §10.8.3, Table 85, S. 336–337 — check_local_datawriter_register_instance,
check_local_datawriter_dispose_instance, check_remote_topic,
check_local_datawriter_match, check_local_datareader_match,
check_remote_datawriter_register_instance,
check_remote_datawriter_dispose_instance: "This operation shall return TRUE."

**Repo:** `crates/security-permissions/src/psk_access.rs`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::the_participant_topic_instance_and_match_checks_return_true`

**Status:** `done` — die Operationen liefern TRUE.

### §10.8.3 Table 85 — get_permissions_token

**Spec:** §10.8.3, Table 85, S. 337 — "This operation shall return the
PermissionsToken formatted as described in 10.8.2.2."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`::get_permissions_token

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — Token nach §10.8.2.2.

### §10.8.3 Table 85 — get_permissions_credential_token

**Spec:** §10.8.3, Table 85, S. 337 — "This operation shall return the
PermissionsToken formatted as described in 10.8.2.1".

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`::get_permissions_credential_token

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — Token der gleichen Klasse (der Wert ist Sache der Implementierung, §10.8.2.1).

### §10.8.3 Table 85 — set_listener, return_permissions_token, return_permissions_credential_token

**Spec:** §10.8.3, Table 85, S. 337 — "This operation shall save a reference
to the listener object and associate it with the specified
PermissionsHandle." (set_listener), "This operation shall behave as specified
in 9.4.2.9.20" (return_permissions_token), "[...] 9.4.2.9.21"
(return_permissions_credential_token).

**Repo:** `crates/security-permissions/src/psk_access.rs`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — `set_listener` speichert die Referenz; die Rückgaben nehmen nur ausgegebene Tokens.

### §10.8.3 Table 85 — validate_local_permissions: immer erfolgreich

**Spec:** §10.8.3, Table 85, S. 337 — "This operation shall succeedand return
an opaque handle that the plugin can use to refer to any
implementation-specific saved information."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — immer erfolgreich, ohne Dokument.

### §10.8.3 Table 85 — validate_remote_permissions: immer erfolgreich

**Spec:** §10.8.3, Table 85, S. 337 — "This operation shall succeedand return
an opaque handle that the plugin can use to refer to any
implementation-specific saved information."

**Repo:** `crates/security-permissions/src/psk_access.rs::PskPermissionsAccessControl`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::nothing_is_needed_and_the_rtps_psk_protection_is_read`, `::permissions_token_class_id_matches_spec`, `::a_remote_is_accepted_whatever_it_presents`, `::every_check_returns_true`

**Status:** `done` — immer erfolgreich, Token und Credential beliebig.

### §10.8.3 Table 85 — get_participant_security_config

**Spec:** §10.8.3, Table 85, S. 337–338 — "The field
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

**Status:** `done` — die Konfiguration entspricht Table 85.

### §10.8.3 Table 85 — get_topic_security_config, get_datawriter_security_config, get_datareader_security_config

**Spec:** §10.8.3, Table 85, S. 338 — "The fields of the TopicSecurityConfig
attributes shall all be set to FALSE." und für Writer und Reader "The boolean
fields [...] shall all be set to FALSE. The plugin_endpoint_attributes shall
be set to the empty mask. [...] supported_mask [...] CRYPTO_ALGORITHM_SET_ALL
[...] required_mask [...] CRYPTO_ALGORITHM_SET_EMPTY".

**Repo:** `crates/security-permissions/src/psk_access.rs`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::the_topic_and_endpoint_configs_are_all_false`

**Status:** `done` — alle Werte sind FALSE.

### §10.8.3 Table 85 — return_participant_security_config bis return_datareader_security_config

**Spec:** §10.8.3, Table 85, S. 338 — "This operation shall behave as
specified in 9.4.2.9.26" bis "[...] 9.4.2.9.29."

**Repo:** `crates/security-permissions/src/psk_access.rs`

**Tests:** `crates/security-permissions/src/psk_access.rs::tests::listener_and_return_operations`

**Status:** `done` — die vier Rückgaben nehmen nur ausgegebene Konfigurationen.

---

### §10.9 DDS:Crypto:PSK: AEAD mit AES-GCM/GMAC wie §10.5

**Spec:** §10.9, S. 338–339 — "This plugin does Authenticated Encryption with
Associated Data (AEAD) using Advanced Encryption Standard with Galois Counter
Mode (AES-GCM/GMAC), see 8.1 for more details. The algorithms used are the
same described in 10.5."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin`; `crates/security-runtime/src/psk_gate.rs`; `crates/dcps/src/runtime.rs::secure_inbound_bytes`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_transform_of_table_89`, `crates/security-runtime/src/psk_gate.rs::tests::the_gate_protects_a_message_with_the_pre_shared_key`, `crates/dcps/src/runtime.rs::tests::a_pre_shared_key_protects_the_bootstrapping_messages`

**Status:** `done` — Die Laufzeit schützt mit gesetzter Passphrase ganze Nachrichten (Bootstrap mit Flags A und P) und verwirft ungeschützte. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log` und `-p zerodds-dcps --features security --lib` (7 Tests), Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051258.log`.

### §10.9.1 Table 86 — rtps_psk_symmetric_cipher_algorithm

**Spec:** §10.9.1, Table 86, S. 339 — "The string 'AUTO' or one of the
CryptoAlgorithmName strings shown in Table 22 [...] If not specified it is
treated as if it was specified to be 'AUTO'. If 'AUTO' is specified it is
treated as if it was specified to be 'AES256-GCM'. This property must be
configured consistently on all the DomainParticipants that join the same DDS
Domain." Präfix `dds.sec.crypto.`.

**Repo:** `crates/security-crypto/src/psk_rtps.rs::key_bits_from_cipher`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_passphrase_rules_of_table_86`

**Status:** `done` — AUTO ist AES256-GCM; die Namen der Tabelle 22 wählen die Breite.

### §10.9.1 Table 86 — rtps_psk_secret_passphrase

**Spec:** §10.9.1, Table 86, S. 339–340 — "Specifying this property enables
pre-shared-key (PSK) protection. [...] (the presence of this property is
mandatory) The property specifies the URI to access the passphrase_id and
passphrase [...] The passphrase_id shall be a number between 0 and 2^32-1
represented as a decimal string. [...] The range of passphrase_id that verify
passphrase_id && 0xFF== 0xFF is reserved and shall not be used. The
passphrase shall contain up to 512 ASCII printable characters [...] Supported
URI schemas are: 'file' and 'data'."

**Repo:** `crates/security-runtime/src/qos_properties.rs::psk_crypto_config_from_properties`, `crates/security-crypto/src/psk_rtps.rs::Passphrase`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_psk_crypto_properties_of_table_86_are_read`, `crates/security-crypto/src/psk_rtps.rs::tests::the_passphrase_rules_of_table_86`

**Status:** `done` — Pflicht; `file:` und `data:`; Id bis 2^32-1 ohne 0xFF als niedriges Oktett; Passphrase bis 512 druckbare Zeichen ohne Leerzeichen am Rand.

### §10.9.1 Table 86 — rtps_psk_secret_passphrase_alt

**Spec:** §10.9.1, Table 86, S. 340 — "URI to access a list of additional
passphrase_id and passphase values that are also accepted during decoding.
[...] each secret passphrase_id, and passphrase tuple shall be separated from
the next using the LineFeed (\n, character 10), the CarryReturn (\r, character
13), or both."

**Repo:** `crates/security-runtime/src/qos_properties.rs::psk_crypto_config_from_properties`, `crates/security-crypto/src/psk_rtps.rs::parse_passphrase_list`

**Tests:** `crates/security-runtime/src/qos_properties.rs::tests::the_psk_crypto_properties_of_table_86_are_read`, `crates/security-crypto/src/psk_rtps.rs::tests::an_alternative_passphrase_decodes`

**Status:** `done` — Liste durch LF, CR oder beide getrennt; beim Dekodieren angenommen.

### §10.9.2 und §10.9.2.1 CryptoToken wie bei DDS:Crypto:AES-GCM-GMAC

**Spec:** §10.9.2.1, S. 340 — "This type is defined the same way as the
DDS:Crypto:AES-GCM-GMAC CryptoToken, see 10.5.2.1. This type is not strictly
needed as the pre-shared Keys are not sent over the network."

**Repo:** `crates/security-crypto/src/psk_rtps.rs`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_exchange_of_table_88`, `crates/security-crypto/src/psk_rtps.rs::tests::the_key_material_is_the_one_the_spec_prints`

**Status:** `done` — Das Schlüsselmaterial wird abgeleitet, kein Token gesendet (Table 88: FALSE); 16 Oktette Salz bei AES128. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.2.1 Key Material aus den Properties nach §10.5.2.1.3

**Spec:** §10.9.2.1, S. 341 — "The Key Material used by the plugin shall be
derived from the DomainParticipant configuration properties defined in 10.9.1
using the same algorithm described in 10.5.2.1.3."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::derive_key_material`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_material_is_the_one_the_spec_prints`

**Status:** `done` — nach §10.5.2.1.3.

### §10.9.2.2 bis §10.9.2.5 CryptoTransformIdentifier, CryptoHeader, CryptoContent, CryptoFooter

**Spec:** §10.9.2.2 bis §10.9.2.5, S. 341 — "This type is defined the same
wasy as the DDS:Crypto:AES-GCM-GMAC CryptoTransformIdentifier, see 10.5.2.2."
(entsprechend für CryptoHeader, CryptoContent, CryptoFooter).

**Repo:** `crates/security-crypto/src/psk_plugin.rs::encrypt_submessage`
(Z. 344), `decrypt_submessage` (Z. 355)

**Tests:** `crates/security-crypto/src/psk_plugin.rs::tests::transform_kind_id_aes128_matches_x509_path`,
`crates/security-crypto/src/psk_plugin.rs::tests::transform_kind_id_aes256_matches_x509_path`,
`crates/security-crypto/src/psk_plugin.rs::tests::psk_encrypt_decrypt_roundtrip_two_plugins_same_psk`

**Status:** done — die Formate sind die des eingebetteten
`AesGcmCryptoPlugin`; dessen Abweichungen (§10.5) gelten auch hier.

### §10.9.3 Drei Schnittstellen

**Spec:** §10.9.3, S. 341 — "This plugin implements three interfaces:
CryptoKeyFactory, CryptoKeyExchange, and CryptoTransform. Each is described
separately."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Gliederung; die Operationen folgen in
Table 87 bis Table 89.

### §10.9.3.1 Table 87 — register_local_participant: ParticipantKeyMaterial

**Spec:** §10.9.3.1, Table 87, S. 341 — "This operation shall create a new
KeyMaterial_AES_GCM_GMAC object and return a handle that the plugin can use to
access the created object. We will refer to this object by the name:
ParticipantKeyMaterial."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::register_local_participant_for`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_message_goes_through_encode_and_decode`

**Status:** `done` — ParticipantKeyMaterial aus dem Schlüsselmaterial der Passphrase.

### §10.9.3.1 Table 87 — register_local_participant: GMAC oder GCM nach RTPS Protection Kind, adjusted_algorithm_info

**Spec:** §10.9.3.1, Table 87, S. 341–342 — "The selection between these two
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

**Status:** `done` — `adjusted_algorithm_info`: `supported_mask` der Property, Builtin-Masken leer, Rest null; `supported_mask` der Config wird gespeichert. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.1 Table 87 — Registrierung entfernter Participants und der Endpunkte, revise_local_entity_keys: Handle ohne Inhalt

**Spec:** §10.9.3.1, Table 87, S. 342 — register_matched_remote_participant,
register_local_datawriter, register_matched_remote_datareader,
register_local_datareader, register_matched_remote_datawriter,
revise_local_entity_keys: "This operation shall do nothing and return dummy
Handle."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyFactory`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_factory_of_table_87`

**Status:** `done` — Die Registrierungen geben ein Dummy-Handle (nicht HandleNIL), `revise_local_entity_keys` die Revision 0. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.1 Table 87 — activate_key_revision: FALSE

**Spec:** §10.9.3.1, Table 87, S. 342 — "This operation shall do nothing and
return FALSE."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyFactory`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_factory_of_table_87`

**Status:** `done` — Tut nichts und gibt FALSE zurück. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.1 Table 87 — unregister_participant, unregister_datawriter, unregister_datareader

**Spec:** §10.9.3.1, Table 87, S. 342 — "Releases any resources allocated on
the corresponding call to register_local_participant, or
register_matched_remote_participant." (entsprechend für Writer und Reader).

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyFactory`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_factory_of_table_87`

**Status:** `done` — Als Operationen der Spec. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.2 Table 88 — create_local_participant_crypto_tokens bis set_remote_datareader_crypto_tokens: FALSE

**Spec:** §10.9.3.2, Table 88, S. 342–343 —
create_local_participant_crypto_tokens, set_remote_participant_crypto_tokens,
create_local_datawriter_crypto_tokens, set_remote_datawriter_crypto_tokens,
create_local_datareader_crypto_tokens, set_remote_datareader_crypto_tokens:
"This operation shall do nothing and return FALSE."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin` (CryptographicPlugin)

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_other_operations_do_nothing`

**Status:** `done` — `create_*` und `set_remote_*_crypto_tokens` tun nichts und scheitern (`NotImplemented`, in der Schnittstelle der Rückgabewert FALSE).

### §10.9.3.2 Table 88 — return_crypto_tokens

**Spec:** §10.9.3.2, Table 88, S. 343 — "Releases the resources associated
with the CryptoToken objects in the sequence."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::CryptoKeyExchange`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_key_exchange_of_table_88`

**Status:** `done` — Gibt Token frei; das Plugin gibt keine aus. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.3.1 Table 89 — encode_serialized_payload, encode_datawriter_submessage, encode_datareader_submessage: FALSE

**Spec:** §10.9.3.3.1, Table 89, S. 343 — "This operation shall do nothing
and return FALSE."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin` (CryptographicPlugin)

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_other_operations_do_nothing`

**Status:** `done` — `encode_serialized_payload`, die Submessage-Operationen und `decode_serialized_payload` tun nichts und scheitern.

### §10.9.3.3.1 Table 89 — encode_rtps_message: transform_with_psk und leere receiver_specific_macs

**Spec:** §10.9.3.3.1, Table 89, S. 343 — "The operation checks that the
parameter transform_with_psk=TRUE. If this is not the case the operation
shall fail and return FALSE. The operation checks that the parameter
receiver_specific_macs contains an empty list. If this is not the case the
operation shall fail and return FALSE. The transformation uses the
ParticipantKeyMaterial associated with the sending_participant_crypto."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::encode_rtps_message`, `crates/security-crypto/src/plugin.rs::PskRtpsKey`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_message_goes_through_encode_and_decode`

**Status:** `done` — `transform_with_psk` ist die Operation `encode_rtps_message_psk`; leere `receiver_specific_macs`: das Plugin schreibt keine.

### §10.9.3.3.1 Table 89 — encode_rtps_message mit Verschlüsselung

**Spec:** §10.9.3.3.1, Table 89, S. 343 — "If the transformation_kind
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

**Status:** `done` — RTPS Header, SRTPS_PREFIX mit A und P, SEC_BODY, SRTPS_POSTFIX; Tag über den Body mit Header und HeaderExtension als AAD.

### §10.9.3.3.1 Table 89 — encode_rtps_message nur mit Authentisierung

**Spec:** §10.9.3.3.1, Table 89, S. 343–344 — "If the transformation_kind
indicates that only authentication is performed then the output shall be: the
original RTPS Header and (if present) the (adjusted) Header Extension [...]
followed by the SecureRTPSPostfixSubMsg, RTPSMessage{Body}, and
SecureRTPSPostfixSubMsg. [...] The common_mac shall be computed using the
ParticipantKeyMaterial associated with the sending_participant_crypto."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::encode_rtps_message`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_message_goes_through_encode_and_decode`

**Status:** `done` — GMAC: der Body bleibt, der Tag deckt AAD und Body.

### §10.9.3.3.1 Table 89 — encode_rtps_message: HeaderExtension anpassen

**Spec:** §10.9.3.3.1, Table 89, S. 344 — "3.1) The HeaderExtension used as
input to the AAD shall have the messageLength element, if present, set to
zero. 3.2) [...] messageChecksum element, if present, set to zero. 3.3) After
computing the SecureRTPSPrefixSubMsg, SecureBodySubMsg, and
SecureRTPSPostfixSubMsg. The HeaderExtension shall be adjusted setting the
appropriate values of the messageLength and messageChecksum elements".

**Repo:** `crates/security-crypto/src/plugin.rs::adjust_header_extension`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::a_header_extension_with_a_checksum_is_adjusted_both_ways`, `crates/security-crypto/src/psk_rtps.rs::tests::a_header_extension_is_kept_and_its_length_set`

**Status:** `done` — `messageLength` und `messageChecksum` im AAD null, danach die Werte der kodierten Nachricht; beim Dekodieren die der dekodierten. Lauf `cargo test -p zerodds-security -p zerodds-security-crypto -p zerodds-security-pki -p zerodds-security-permissions -p zerodds-security-runtime`: grün, Log `/root/zerodds-rc7/agent-logs/sec-crypto-1018-051206.log`.

### §10.9.3.3.1 Table 89 — decode_rtps_message: erwartete Art, transform_with_psk, Material des Senders

**Spec:** §10.9.3.3.1, Table 89, S. 344 — "Examines the
SecureRTPSPrefixSubMsg to determine the transformation_kind matches the one
the receiving DomainParticipant is expecting [...] If the kind is not the
expected one, the operation shall fail with an exception. The operation checks
that the parameter transform_with_psk=TRUE. [...] Uses content of the RTPS
Header, the pre-shared secret and SenderKeyId to compute (or locate a
previously computed) PSK Key Material associated with the sending Participant
(see 10.5.2.1.3)."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::decode_rtps_message`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::what_does_not_hold_is_refused`, `::an_alternative_passphrase_decodes`

**Status:** `done` — erwartete Art, Schlüssel des Senders aus RTPS Header und Passphrase (primär oder alternativ), Domain.

### §10.9.3.3.1 Table 89 — decode_rtps_message: Flag A, AAD und HeaderExtension

**Spec:** §10.9.3.3.1, Table 89, S. 344 — "Checks the
SecureRTPSPrefixSubMsg's AdditionalAuthenticatedDataFlag. If this flag is not
set, the decode operation shall fail. If the flag is set, the decode shall
validate the tag present in the SecureRTPSPostfixSubMsg passing the RTPS
Header and (if present) the (adjusted) HeaderExtension as AAD. [...] Upon
success the returned RTPS Message shall match the input to the
encode_rtps_message operation on the DomainParticipant that sent the
message."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin::decode_rtps_message`, `crates/security-crypto/src/plugin.rs::parse_psk_wire`

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::what_does_not_hold_is_refused`

**Status:** `done` — ohne Flag A oder P scheitert das Dekodieren; Header und HeaderExtension (Länge und Prüfsumme null) stehen im AAD.

### §10.9.3.3.1 Table 89 — preprocess_secure_submsg, decode_datawriter_submessage, decode_datareader_submessage, decode_serialized_payload: FALSE

**Spec:** §10.9.3.3.1, Table 89, S. 344–345 — "This operation shall do
nothing and return FALSE."

**Repo:** `crates/security-crypto/src/psk_rtps.rs::PskCryptoPlugin` (CryptographicPlugin)

**Tests:** `crates/security-crypto/src/psk_rtps.rs::tests::the_other_operations_do_nothing`

**Status:** `done` — tun nichts und scheitern.

### §11.1 Introduction: PIM und PSM

**Spec:** §11.1, S. 347 — "Clause 9 defines the plugin interfaces in a
programming-language independent manner using UML. [...] The mapping of the
plugin interfaces to specific programming languages is defined by first
defining the interfaces using OMG-IDL version 3.5 with the additional syntax
defined in the DDS-XTYPES specification and subsequently applying the IDL to
language mapping to the target language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einordnung; die Abbildungen folgen in
§11.2 bis §11.7.

### §11.1 Typen in IDL ohne @extensibility gelten als APPENDABLE

**Spec:** §11.1, S. 347 — "IDL Types lacking the DDS-XTYPES @extensibility
annotation shall be interpreted as having the extensibility kind APPENDABLE.
This matches the DDS-XTYPES specification implied extensibility of
un-annotated types."

**Repo:** `crates/security/src/token.rs::encode` (Z. 357);
`crates/security/src/generic_message.rs::ParticipantGenericMessage::to_cdr`
(Z. 138)

**Tests:** `crates/security/src/token.rs::tests::cdr_le_layout_class_id_only`,
`crates/security/src/generic_message.rs::tests::message_data_dataholder_is_inline_not_length_prefixed`

**Status:** done — die Typen der Spec werden als XCDR1 geschrieben, wo
APPENDABLE und FINAL gleich kodiert sind (§7.3.1.2). Die Typen des
BuiltinLoggingTopic sind ausdrücklich annotiert (§10.6).

### §11.1 Sprachbindungen C, C++ classic, Java classic, C++ modern, Java modern

**Spec:** §11.1, S. 347 — "For consistency with the DDS specification, the
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

**Status:** `done` — alle fünf Darstellungen werden aus `dds_security.idl` erzeugt und mit dem Compiler der Sprache übersetzt; das Logging-Interface wird mit den erzeugten Signaturen umgesetzt. Die Basistypen der ORB-Abbildungen und der DDS-STD-C++-PSM stehen als Platzhalter in den Tests. Ein Plugin der Anwendung in C, C++ oder Java in einen Participant einsetzen ist in §11.3, §11.4 und §11.5 geführt (Authentication, AccessControl, Cryptographic, Logging, DataTagging).
### §11.2 IDL representation: Regeln von PIM zu PSM und dds_security.idl

**Spec:** §11.2, S. 347 — "The PIM to PSM mapping maps the UML interfaces and
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

**Status:** `done` — die Datei liegt vor, parst als IDL 4.2, hat keinen `out`-Parameter und je Interface die Operationen der Tabelle.
### §11.3 C language representation

**Spec:** §11.3, S. 347 — "The C language representation of the plugin
interfaces shall be obtained applying the IDL to C mapping [5] to the
dds_security.idl file."

**Repo:** `crates/security/tests/idl_bindings/c.rs`

**Tests:** `crates/security/tests/dds_security_bindings.rs::c_representation_compiles_and_the_logging_operations_link`,
`crates/zerodds-c-api/tests/plugin_languages.rs::a_plugin_in_c_is_placed_into_a_participant`,
`crates/zerodds-c-api/src/plugin_ffi/security_plugins.rs::tests::the_handshake_goes_through_the_c_functions`,
`crates/zerodds-c-api/src/plugin_ffi/security_plugins.rs::tests::tokens_and_transforms_go_through_the_c_functions`

**Status:** `done` — Header nach den Regeln von IDL2C, übersetzt als C99. Ein Plugin der Anwendung in C (Authentication, AccessControl, Cryptographic, Logging, DataTagging) wird über die Structs von `zerodds_security_plugins.h` in einen Participant eingesetzt und von der Laufzeit an den Stellen von §9.8 gerufen; diese Structs sind keine IDL2C-Abbildung von `dds_security.idl`. Authentication und Cryptographic der Anwendung: `zerodds_runtime_create_secure` nimmt sie anstelle des PKI- und AES-GCM-Pfades aus Dateien; das Cryptographic-Plugin ohne Authentication-Plugin wird abgelehnt; die Pfade der Konfiguration (Governance, Permissions-CA, falls gesetzt Identitätsdateien) gehen als Properties nach Table 50 an `validate_local_identity`, Governance und Permissions-CA sind Pflicht. Geprüft sind die Aufrufe von §9.3.2.11 und §9.8.1 (Tokens werden zurückgegeben, Credential vor Config, `register_local_participant` einmal, Volatile-Endpunkte benannt, geschützter Writer registriert, `destroy` je einmal) und die Übergabe der Strukturen. Nicht geprüft: ein Handshake und das Ver- und Entschlüsseln zwischen zwei Laufzeiten mit Plugins der Anwendung; die Übergabe der Strukturen der Operationen dafür prüfen die Rust-Tests `plugin_ffi::security_plugins::tests::the_handshake_goes_through_the_c_functions` und `::tokens_and_transforms_go_through_the_c_functions` und die Größenprüfungen (`SIZE_IS`) im C-Test. Grün auf dem Linux-Testhost: `/root/zerodds-rc7/agent-logs/sec-behavior-1018-105526.log` (C, C++, Rust), Java auf dem Linux-Testhost `/root/agent-cargo/logs/sec-behavior-1018-104914.log`.
### §11.4 C++ classic representation

**Spec:** §11.4, S. 348 — "The C++ classic (without the use of the C++
standard library) language representation of the plugin interfaces shall be
obtained using the IDL2C++ mapping [7] to the dds_security.idl file."

**Repo:** `crates/security/tests/idl_bindings/cpp.rs`

**Tests:** `crates/security/tests/dds_security_bindings.rs::cpp_classic_representation_compiles_and_the_logging_interface_is_implementable`,
`crates/zerodds-c-api/tests/plugin_languages.rs::a_plugin_in_cpp_is_placed_into_a_participant`

**Status:** `done` — Header nach IDL2C++ ohne Standardbibliothek, übersetzt als C++98. Ein Plugin der Anwendung in C++ (Authentication, AccessControl, Cryptographic, Logging, DataTagging; abstrakte Klassen in `zerodds_security_plugins.hpp`, C++11) wird in einen Participant eingesetzt.
### §11.5 Java classic

**Spec:** §11.5, S. 348 — "The Java classic language representation of the
plugin interfaces shall be obtained using the IDL2Java mapping [6] to the
dds_security.idl file."

**Repo:** `crates/security/tests/idl_bindings/java.rs`

**Tests:** `crates/security/tests/dds_security_bindings.rs::java_classic_representation_compiles_and_the_logging_interface_is_implementable`,
`crates/zerodds-c-api/tests/plugin_languages.rs::a_plugin_in_java_is_placed_into_a_participant`

**Status:** `done` — Klassen nach IDL2Java (Struct, Holder, Helper, Operations-Interface), mit `javac` übersetzt. Ein Plugin der Anwendung in Java (Authentication, AccessControl, Cryptographic, Logging, DataTagging; `org.zerodds.security`, JDK 21 mit `--enable-preview`, Foreign Function Interface) wird in einen Participant eingesetzt.
### §11.6 C++11 representation mit fünf Ausnahmen

**Spec:** §11.6, S. 348 — "This representation is aligned with the
DDS-STDC++ PSM. [...] shall be obtained using the IDL2C++11 mapping [8] to the
dds_security.idl file with the following exceptions: 1. The IDL module DDS
shall be mapped to the C++ namespace dds [...] 2. The mapping shall not use
any C++11-only feature [...] 3. Arrays shall map to the dds::core::array
template [...] 4. The enumerations shall map to the dds::core::safe_enum
template [...] 5. The IDL DynamicData native type shall be mapped to the C++
type dds::code::xtypes::DynamicData".

**Repo:** `crates/security/tests/idl_bindings/cpp.rs` (`Dialect::Cxx11`)

**Tests:** `crates/security/tests/dds_security_bindings.rs::cpp11_representation_compiles_without_a_cpp11_only_feature`

**Status:** `done` — die fünf Ausnahmen: Namespace `dds`, kein C++11-Merkmal (als C++98 übersetzt), `dds::core::array`, `dds::core::safe_enum`, `dds::core::xtypes::DynamicData`.
### §11.7 Java modern mit zwei Ausnahmen

**Spec:** §11.7, S. 348 — "The Java classic language representation of the
plugin interfaces shall be obtained using the IDL2Java mapping [6] to the
dds_security.idl file with the following exceptions: 1. The IDL module DDS
shall be mapped to the Java namespace org.omg.dds [...] 2. The IDL
DynamicData native type shall be mapped to the type
org.omg.dds.type.dynamic.DynamicData".

**Repo:** `crates/security/tests/idl_bindings/java.rs` (`Dialect::Modern`)

**Tests:** `crates/security/tests/dds_security_bindings.rs::java_modern_representation_maps_module_dds_to_org_omg_dds`

**Status:** `done` — Modul `DDS` ist das Paket `org.omg.dds`, DynamicData ist `org.omg.dds.type.dynamic.DynamicData`.

---

## Annex A References

### Annex A Literaturverzeichnis [1] bis [59]

**Spec:** Annex A, S. 349–351 — Literaturverzeichnis [1] bis [59].

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literaturverzeichnis; normative Verweise
stehen in §3.

---

## Audit-Status

803 done / 0 partial / 0 open / 88 n/a (informative) / 0 n/a (rejected).

Test-Lauf: wird auf dem Linux-Testhost ergänzt.

