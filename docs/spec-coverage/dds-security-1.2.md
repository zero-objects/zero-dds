# DDS-Security 1.2 — Spec-Coverage

**Spec:** [OMG DDS-Security 1.2 — formal/2025-03-06 (351 Seiten) →](https://www.omg.org/spec/DDS-SECURITY/)

**Kontext:** ZeroDDS-Security ist über 8 Crates verteilt mit
zusammen 655 Tests grün:

- `crates/security/` — SPI-Traits (Plugin-Definitionen)
- `crates/security-pki/` — Builtin Authentication (DDS:Auth:PKI-DH, 182 Tests)
- `crates/security-permissions/` — Builtin Access Control (116 Tests)
- `crates/security-crypto/` — Builtin Cryptographic (80 Tests)
- `crates/security-rtps/` — Wire-Codec (31 Tests)
- `crates/security-runtime/` — Gate, Caps, Policy, Heterogeneous-Security (214 Tests)
- `crates/security-keyexchange/` — X25519 + RSA-Wrap (16 Tests)
- `crates/security-logging/` — DDS:Logging-Plugin (16 Tests)
- `crates/rtps/src/{endpoint,participant}_security_info.rs` — Discovery-Wire-PIDs

---

## §1 Scope

### 1.1 DDS-Security-Compliance-Profile als Erweiterung von DDS

**Spec:** §1.1.

**Repo:** Plugin-SPI in `crates/security/` + alle 5 Builtins live:
Auth (`security-pki`), Access (`security-permissions`),
Crypto (`security-crypto`), Logging (`security-logging`), DataTagging
(`security-runtime/src/data_tagging.rs`).

**Tests:** Crate-weit (~520 Tests) +
`crates/security-runtime/tests/conformance_matrix.rs` (11 Tests:
pro SPI accepts_builtin + rejects_misimplemented +
conformance_points_full_matrix).

**Status:** done — alle 5 SPIs produktiv mit Builtin; Compliance-
Profile vollständig erfüllt.

### 1.2 5 SPIs: Authentication, AccessControl, Cryptographic, Logging, DataTagging

**Spec:** §1.2.

**Repo:** Trait-Definitionen in `crates/security/src/{authentication,
access_control,crypto,logging,data_tagging}.rs`; alle 5 Builtins:
`PkiAuthenticationPlugin`, `PermissionsAccessControl`,
`AesGcmCryptoPlugin`, `StderrLoggingPlugin` /
`JsonLinesLoggingPlugin`, `BuiltinDataTaggingPlugin`.

**Tests:** Mock-Plugin-Tests in `crates/security/src/mock.rs`
(5 Mocks für alle 5 SPIs) +
`security-runtime/tests/conformance_matrix.rs::auth_*` /
`access_control_*` / `crypto_*` / `logging_*` / `data_tagging_*` (10
SPI-Tests + 1 Matrix-Test).

**Status:** done — alle 5 SPI-Traits sind erfüllt durch je einen
produktiven Builtin und einen Mock.

---

## §2 Conformance

### 2.1 Conformance-Points (Builtin Plugins, Plugin-Framework, Plugin-Language-APIs, Logging+Tagging-Profil)

**Spec:** §2.1.

**Repo:** Builtin-Interop laufend (alle 5 Builtins);
Plugin-Framework via `Box<dyn>` über das `zerodds-security`-SPI;
Logging-Profil via `security-logging`; Tagging-Profil via
`BuiltinDataTaggingPlugin`. Language-APIs n/a (Rust-only Crate-
Boundary, statt FFI bedienbar als `Box<dyn TraitName>`).

**Tests:** Plugin-Tests + Wire-Tests +
`security-runtime/tests/conformance_matrix.rs::conformance_points_full_matrix`
(verifiziert alle 4 Conformance-Points: Builtin Plugins / Plugin-
Framework via Class-Id-Eindeutigkeit / Plugin-Language-APIs als
`Box<dyn>` / Logging+Tagging-Profil-Operationen).

**Status:** done — alle 4 Conformance-Points haben einen
korrespondierenden Test in der Matrix.

---

## §3 Normative References

### 3.0 [DDS] DDS 1.4 / [RTPS] RTPS 2.5 / [DDS-XTYPES] XTypes 1.3 / [IDL] IDL 4.2

**Spec:** §3.

**Repo:** alle vorhanden.

**Tests:** —

**Status:** done

### 3.1 Normative IETF/NIST/ISO References (X.509, AES-GCM, ECDH, RSA, etc.)

**Spec:** §3.

**Repo:** Implementiert via `ring`-Crate (AES-GCM, ECDH/X25519);
`rustls-pemfile` (PEM-Parser); `x509-cert` (X.509-Parser).

**Tests:** PKI+Crypto-Tests.

**Status:** done

---

## §4 Terms / §5 Symbols / §6 Additional

### §4 Terms and Definitions

**Spec:** §4.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar.

### §5 Symbols

**Spec:** §5.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acronyms.

### §6 Additional Information

**Spec:** §6.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acknowledgments.

---

## §7 Plugin Architecture

### 7.1 Plugin-Architektur Uebersicht

**Spec:** §7.1.

**Repo:** Plugin-Trait-Definitionen in `crates/security/src/`.

**Tests:** —

**Status:** done

### 7.2 SPI-Trennung (5 SPIs als unabhängige Plugins)

**Spec:** §7.2.

**Repo:** 5 Module: authentication.rs / access_control.rs / crypto.rs
/ logging.rs / data_tagging.rs.

**Tests:** Mock-Plugin-Tests.

**Status:** done

### 7.3 Plugin-Discovery via Properties

**Spec:** §7.3.

**Repo:** `crates/security/src/properties.rs` mit Property-Map +
Plugin-Konfig.

**Tests:** Property-Tests.

**Status:** done

---

## §8 Builtin Plugins — Authentication (DDS:Auth:PKI-DH)

### 8.1 Authentication-SPI

**Spec:** §8.1.

**Repo:** `crates/security/src/authentication.rs`.

**Tests:** Auth-Trait-Tests.

**Status:** done

### 8.2 Builtin Authentication: PKI-DH (3-Way Handshake)

**Spec:** §8.2 — validate_local_identity, validate_remote_identity,
begin/process_handshake_request/reply.

**Repo:** `crates/security-pki/src/{identity,handshake_token,plugin}.rs`.

**Tests:** PKI-DH-Tests.

**Status:** done

### 8.3 IdentityCertificate (X.509)

**Spec:** §8.3.

**Repo:** `security-pki/src/identity.rs` mit X.509-Validation.

**Tests:** Identity-Tests.

**Status:** done

### 8.4 IdentityCA-Validation (Cert-Chain-Verification)

**Spec:** §8.4.

**Repo:** `security-pki/src/identity.rs` mit Cert-Chain-Walker.

**Tests:** —

**Status:** done

### 8.5 Handshake-Tokens (BinaryProperty mit RSA/ECDSA-Sign)

**Spec:** §8.5.

**Repo:** `security-pki/src/handshake_token.rs`.

**Tests:** Handshake-Token-Tests.

**Status:** done

### 8.6 Shared Secret (X25519/ECDH)

**Spec:** §8.6.

**Repo:** `security-keyexchange/src/lib.rs` (X25519) +
`rsa_wrap.rs` (RSA-OAEP).

**Tests:** Keyexchange-Tests.

**Status:** done

### 8.7 PSK-Authentication

**Spec:** §8.7.

**Repo:** `security-pki/src/psk.rs` + `security-crypto/src/psk_plugin.rs`.

**Tests:** PSK-Tests.

**Status:** done

### 8.8 OCSP/CRL-Revocation-Checks

**Spec:** §8.8.

**Repo:** OCSP-Stapling in `security-pki/src/ocsp.rs`
(`parse_ocsp_status` + `require_good_status`); CRL-Validation in
`security-pki/src/crl.rs` (`parse_crl_serials` + `validate_crl` mit
DER-Walker für RFC-5280-`CertificateList`).

**Tests:** OCSP (`ocsp::tests::*`, 12 Tests):
`empty_input_is_malformed`, `good_status_parses_to_good`,
`good_tag_requires_zero_length`,
`prefix_bytes_before_sequence_are_skipped`,
`require_good_accepts_good`, `require_good_rejects_malformed`,
`require_good_rejects_revoked_with_auth_failed`,
`require_good_rejects_unknown`, `revoked_tag_81_parses_to_revoked`,
`revoked_tag_a1_parses_to_revoked`,
`sequence_tag_recognized_via_equality`,
`unknown_tag_82_parses_to_unknown`.
CRL (`crl::tests::*`, 24 Tests):
`parse_error_messages_are_specific_per_variant`,
`parse_serials_empty_revocation_list`,
`parse_serials_handles_long_form_length`,
`parse_serials_handles_long_serial`,
`parse_serials_keeps_leading_zero_byte_for_positive_serials`,
`parse_serials_rejects_empty_input`,
`parse_serials_rejects_indefinite_length`,
`parse_serials_rejects_non_sequence_outer`,
`parse_serials_returns_all_revoked`,
`read_length_0x80_is_long_form_marker_not_short`,
`read_length_buf_exactly_one_plus_n_accepted`,
`read_length_buf_one_plus_n_minus_one_truncated`,
`read_length_n_equals_four_accepted`,
`read_length_rejects_n_greater_than_four`,
`read_length_three_byte_length_correct`,
`read_length_two_byte_length_high_byte_first`,
`try_parse_revoked_list_rejects_non_time_tag`,
`validate_crl_against_empty_list_passes`,
`validate_crl_empty_input_returns_bad_argument`,
`validate_crl_known_revoked_rejects`,
`validate_crl_signature_invalid_rejects`,
`validate_crl_truncated_input_returns_bad_argument`,
`validate_crl_unknown_serial_passes`,
`validate_crl_with_two_revoked_finds_second`.

**Status:** done — OCSP-Stapling-Pfad live; CRL-Fallback live mit
positivem (revoked-rejects) UND negativem (unknown-passes) Test sowie
Malformed-Defense.

---

## §9 Builtin Plugins — Access Control (DDS:Access:Permissions)

### 9.1 AccessControl-SPI

**Spec:** §9.1.

**Repo:** `crates/security/src/access_control.rs`.

**Tests:** —

**Status:** done

### 9.2 Builtin Access Control via signed Permissions-XML + Governance-XML

**Spec:** §9.2.

**Repo:** `security-permissions/src/{governance,plugin,signature}.rs`.

**Tests:** Permissions-Tests.

**Status:** done

### 9.3 Permissions-XML (Allow/Deny pro Domain/Topic/Partition)

**Spec:** §9.3.

**Repo:** `security-permissions/src/xml.rs` + `topic_match.rs`.

**Tests:** Permissions-XML-Tests.

**Status:** done

### 9.4 Governance-XML (Domain-weite Policies: Discovery/Liveliness/RTPS-Protection-Kinds)

**Spec:** §9.4.

**Repo:** `security-permissions/src/governance.rs` mit
ProtectionKind-Enum (NONE/SIGN/ENCRYPT/SIGN_WITH_ORIGIN_AUTH/etc.).

**Tests:** Governance-Tests.

**Status:** done

### 9.5 CMS/PKCS#7-Signature-Verification

**Spec:** §9.5.

**Repo:** `security-permissions/src/cms.rs` + `signature.rs`.

**Tests:** CMS-Tests.

**Status:** done

### 9.6 Permission-Caching + check_create/check_remote

**Spec:** §9.6.

**Repo:** `security-permissions/src/plugin.rs` + `delegation_check.rs`.

**Tests:** Permissions-Tests.

**Status:** done

### 9.7 PSK-Access

**Spec:** §9.7.

**Repo:** `security-permissions/src/psk_access.rs`.

**Tests:** PSK-Access-Tests.

**Status:** done

---

## §10 Builtin Plugins — Cryptographic (DDS:Crypto:AES-GCM-GMAC)

### 10.1 Cryptographic-SPI

**Spec:** §10.1.

**Repo:** `crates/security/src/crypto.rs`.

**Tests:** —

**Status:** done

### 10.2 Builtin Crypto: AES128/AES256-GCM/GMAC

**Spec:** §10.2.

**Repo:** `security-crypto/src/{plugin,suite,session_key}.rs`.

**Tests:** Crypto-Tests.

**Status:** done

### 10.3 KeyMaterial: master_key + master_salt + key_id (mit Versions-Wechsel)

**Spec:** §10.3.

**Repo:** `security-crypto/src/session_key.rs::SessionKey` +
KeyMaterial-Wire.

**Tests:** Session-Key-Tests.

**Status:** done

### 10.4 Receiver-Specific MAC (pro Reader-MAC zusätzlich zum Common-MAC)

**Spec:** §10.4.

**Repo:** `security-crypto/src/plugin.rs` mit Receiver-Specific-MAC-
Pfad.

**Tests:** —

**Status:** done

### 10.5 PSK-Crypto-Plugin

**Spec:** §10.5.

**Repo:** `security-crypto/src/psk_plugin.rs`.

**Tests:** PSK-Crypto-Tests.

**Status:** done

---

## §11 Builtin Plugins — Logging (DDS:Logging:DDS_LogTopic)

### 11.1 Logging-SPI

**Spec:** §11.1.

**Repo:** `crates/security/src/logging.rs`.

**Tests:** —

**Status:** done

### 11.2 Logging-Sinks (jsonl, syslog, stderr, fanout)

**Spec:** §11.2.

**Repo:** `crates/security-logging/src/{jsonl,syslog,stderr_sink,fanout}.rs`.

**Tests:** Logging-Sink-Tests.

**Status:** done

### 11.3 BuiltinLoggingType (Topic + Severity + Message)

**Spec:** §11.3.

**Repo:** `security-logging/src/lib.rs` mit BuiltinLoggingType-Struct.

**Tests:** —

**Status:** done

---

## §12 Builtin Plugins — Data Tagging

### 12.0 DataTagging-SPI

**Spec:** §12.

**Repo:** SPI-Trait in `crates/security/src/data_tagging.rs`;
Builtin in `crates/security-runtime/src/data_tagging.rs`
(`BuiltinDataTaggingPlugin` + Subset-Match-Predicate +
`PID_PROPERTY_LIST`-Wire-Codec mit Namespace-Prefix
`dds.sec.data_tags.`); Mock in `crates/security/src/mock.rs`
(`MockDataTaggingPlugin`).

**Tests:** `data_tagging::tests::*` (15 Tests):
`decode_tags_skips_non_tag_properties`,
`empty_publisher_with_required_subscriber_rejects`,
`encode_tags_uses_namespace_prefix`,
`match_empty_subscriber_is_wildcard`, `match_full_set_passes`,
`match_missing_required_tag_rejects`, `match_subset_passes`,
`match_unknown_subscriber_tag_rejects`,
`match_value_mismatch_rejects`, `plugin_class_id_matches_spec_format`,
`plugin_is_object_safe_via_dyn_trait`, `set_empty_clears_existing`,
`set_get_roundtrip`, `unknown_endpoint_returns_empty`,
`wire_roundtrip_via_property_list` +
`mock::tests::mock_data_tagging_set_get_roundtrip`.

**Status:** done — Builtin produktiv, Wire-Pfad belegt, Subset-Match
positiv UND negativ getestet.

---

## §13 RTPS Wire-Protection

### 13.1 SecuredPayload (DATA mit Encryption + MAC)

**Spec:** §13.1.

**Repo:** `crates/security-rtps/src/{srtps,codec}.rs`.

**Tests:** SRTPS-Tests.

**Status:** done

### 13.2 SEC_PREFIX / SEC_BODY / SEC_POSTFIX (Submessage-Wrapping)

**Spec:** §13.2.

**Repo:** `security-rtps/src/codec.rs` mit Submessage-IDs 0x30/0x31/0x32.

**Tests:** Submessage-Wrapping-Tests.

**Status:** done

### 13.3 Receiver-Specific MAC im SEC_POSTFIX

**Spec:** §13.3.

**Repo:** `security-rtps/src/codec.rs::SecPostfix` mit
ReceiverSpecificMacs-Vec.

**Tests:** —

**Status:** done

### 13.4 ProtectionKind-Decision (NONE/SIGN/ENCRYPT/SIGN_WITH_ORIGIN_AUTH/ENCRYPT_WITH_ORIGIN_AUTH)

**Spec:** §13.4.

**Repo:** `security-permissions/src/governance.rs::ProtectionKind`.

**Tests:** —

**Status:** done

### 13.5 RTPS Header Protection (gesamte Message Sign/Encrypt)

**Spec:** §13.5.

**Repo:** `security-rtps/src/srtps.rs::rtps_header_protect/unprotect`.

**Tests:** —

**Status:** done

---

## §14 Discovery — Builtin Endpoints für Auth-Handshake

### 14.1 ParticipantSecurityInfoBuiltinTopicData

**Spec:** §14.1.

**Repo:** `crates/rtps/src/participant_security_info.rs` (PID 0x1005).

**Tests:** —

**Status:** done

### 14.2 EndpointSecurityInfoBuiltinTopicData

**Spec:** §14.2.

**Repo:** `crates/rtps/src/endpoint_security_info.rs` (PID 0x1004).

**Tests:** —

**Status:** done

### 14.3 ParticipantStatelessMessage (Auth-Handshake-Topic)

**Spec:** §14.3.

**Repo:** `security-runtime/src/builtin_topics.rs::ParticipantStatelessMessage`.

**Tests:** Auth-Topic-Tests.

**Status:** done

### 14.4 ParticipantVolatileMessageSecure (Crypto-Key-Distribution)

**Spec:** §14.4.

**Repo:** `security-runtime/src/builtin_topics.rs`.

**Tests:** —

**Status:** done

### 14.5 PublicationsSecure / SubscriptionsSecure (signed SEDP)

**Spec:** §14.5.

**Repo:** `security-runtime/src/builtin_topics.rs`.

**Tests:** —

**Status:** done

---

## §15 Plugin Configuration via Properties

### 15.1 PropertyQosPolicy mit dds.sec.* Properties

**Spec:** §15.1.

**Repo:** `crates/security/src/properties.rs` (PropertyKey-Konstanten).

**Tests:** Property-Tests.

**Status:** done

### 15.2 Property-Driven Plugin-Selection

**Spec:** §15.2.

**Repo:** Properties->Plugin-Mapping in `security-runtime/src/engine.rs`.

**Tests:** Engine-Tests.

**Status:** done

---

## §16 Heterogeneous Security (zerodds-spezifisch)

### 16.1 PolicyEngine: Capability-Negotiation (Cyclone-Compatible-Mode + Strict-Mode)

**Spec:** §16 (zerodds-spezifische Erweiterung; siehe DDS-Security-1.2-§9.2-9.4 als Basis).

**Repo:** `security-runtime/src/{engine,policy,caps,peer_class}.rs`.

**Tests:** Heterogeneous-Tests.

**Status:** done — zerodds-spezifisch, deckt DDS-Security-1.2-Spec ab.

### 16.2 Anti-Squatter (Identity-Hijack-Prevention)

**Spec:** §16.

**Repo:** `security-runtime/src/anti_squatter.rs`.

**Tests:** Anti-Squatter-Tests.

**Status:** done

### 16.3 Gateway-Bridge (Untrusted-Trusted-Border)

**Spec:** §16.

**Repo:** `security-runtime/src/gateway_bridge.rs`.

**Tests:** —

**Status:** done

---

## §17 Logging + Audit

### 17 Audit-Log (alle Plugin-Operationen)

**Spec:** §17.

**Repo:** `security-logging/src/lib.rs` mit Audit-Records.

**Tests:** —

**Status:** done

---

## Annex: IDL-Definitionen (Builtin Topic Types + Plugin SPIs)

### Annex-A IDL-Module dds::security (alle Builtin-Topic-Types)

**Spec:** Annex.

**Repo:** Implementiert via Rust-Strukturen in `security-runtime/src/builtin_topics.rs`.

**Tests:** —

**Status:** done

### Annex-B Plugin-Trait-IDLs

**Spec:** Annex.

**Repo:** Rust-Traits statt IDL.

**Tests:** —

**Status:** done

---

## Audit-Status

50 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-security-runtime` — 214 Tests grün.
* `cargo test -p zerodds-security-pki` — 182 Tests grün.
* `cargo test -p zerodds-security-permissions` — 116 Tests grün.
* `cargo test -p zerodds-security-crypto` — 80 Tests grün.
* `cargo test -p zerodds-security-rtps` — 31 Tests grün.
* `cargo test -p zerodds-security-keyexchange` — 16 Tests grün.
* `cargo test -p zerodds-security-logging` — 16 Tests grün.

Cross-Crate Test-Volumen: 655 Tests gegen DDS-Security-1.2.
