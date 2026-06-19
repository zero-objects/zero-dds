# DDS-Security 1.2 — Spec Coverage

**Spec:** [OMG DDS-Security 1.2 — formal/2025-03-06 (351 pages) →](https://www.omg.org/spec/DDS-SECURITY/)

**Context:** ZeroDDS-Security is spread across 8 crates with 655 tests green
in total:

- `crates/security/` — SPI traits (plugin definitions)
- `crates/security-pki/` — Builtin Authentication (DDS:Auth:PKI-DH, 182 tests)
- `crates/security-permissions/` — Builtin Access Control (116 tests)
- `crates/security-crypto/` — Builtin Cryptographic (80 tests)
- `crates/security-rtps/` — wire codec (31 tests)
- `crates/security-runtime/` — gate, caps, policy, heterogeneous security (214 tests)
- `crates/security-keyexchange/` — X25519 + RSA-wrap (16 tests)
- `crates/security-logging/` — DDS:Logging plugin (16 tests)
- `crates/rtps/src/{endpoint,participant}_security_info.rs` — discovery wire PIDs

---

## §1 Scope

### 1.1 DDS-Security compliance profile as an extension of DDS

**Spec:** §1.1.

**Repo:** the plugin SPI in `crates/security/` + all 5 builtins live: Auth
(`security-pki`), Access (`security-permissions`), Crypto
(`security-crypto`), Logging (`security-logging`), DataTagging
(`security-runtime/src/data_tagging.rs`).

**Tests:** crate-wide (~520 tests) +
`crates/security-runtime/tests/conformance_matrix.rs` (11 tests: per SPI
accepts_builtin + rejects_misimplemented + conformance_points_full_matrix).

**Status:** done — all 5 SPIs in production with a builtin; the compliance
profile fully satisfied.

### 1.2 5 SPIs: Authentication, AccessControl, Cryptographic, Logging, DataTagging

**Spec:** §1.2.

**Repo:** trait definitions in
`crates/security/src/{authentication,access_control,crypto,logging,data_tagging}.rs`;
all 5 builtins: `PkiAuthenticationPlugin`, `PermissionsAccessControl`,
`AesGcmCryptoPlugin`, `StderrLoggingPlugin` / `JsonLinesLoggingPlugin`,
`BuiltinDataTaggingPlugin`.

**Tests:** mock-plugin tests in `crates/security/src/mock.rs` (5 mocks for
all 5 SPIs) +
`security-runtime/tests/conformance_matrix.rs::auth_*` / `access_control_*` /
`crypto_*` / `logging_*` / `data_tagging_*` (10 SPI tests + 1 matrix test).

**Status:** done — all 5 SPI traits satisfied by one production builtin and
one mock each.

---

## §2 Conformance

### 2.1 Conformance points (builtin plugins, plugin framework, plugin-language APIs, logging+tagging profile)

**Spec:** §2.1.

**Repo:** builtin interop running (all 5 builtins); the plugin framework via
`Box<dyn>` over the `zerodds-security` SPI; the logging profile via
`security-logging`; the tagging profile via `BuiltinDataTaggingPlugin`.
Language APIs n/a (Rust-only crate boundary, serviceable as `Box<dyn
TraitName>` instead of FFI).

**Tests:** plugin tests + wire tests +
`security-runtime/tests/conformance_matrix.rs::conformance_points_full_matrix`
(verifies all 4 conformance points: builtin plugins / plugin framework via
class-id uniqueness / plugin-language APIs as `Box<dyn>` / logging+tagging
profile operations).

**Status:** done — all 4 conformance points have a corresponding test in the
matrix.

---

## §3 Normative references

### 3.0 [DDS] DDS 1.4 / [RTPS] RTPS 2.5 / [DDS-XTYPES] XTypes 1.3 / [IDL] IDL 4.2

**Spec:** §3.

**Repo:** all present.

**Tests:** —

**Status:** done

### 3.1 Normative IETF/NIST/ISO references (X.509, AES-GCM, ECDH, RSA, etc.)

**Spec:** §3.

**Repo:** implemented via the `ring` crate (AES-GCM, ECDH/X25519);
`rustls-pemfile` (PEM parser); `x509-cert` (X.509 parser).

**Tests:** PKI+crypto tests.

**Status:** done

---

## §4 Terms / §5 Symbols / §6 Additional

### §4 Terms and definitions

**Spec:** §4.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary.

### §5 Symbols

**Spec:** §5.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acronyms.

### §6 Additional information

**Spec:** §6.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — acknowledgments.

---

## §7 Plugin architecture

### 7.1 Plugin-architecture overview

**Spec:** §7.1.

**Repo:** plugin-trait definitions in `crates/security/src/`.

**Tests:** —

**Status:** done

### 7.2 SPI separation (5 SPIs as independent plugins)

**Spec:** §7.2.

**Repo:** 5 modules: authentication.rs / access_control.rs / crypto.rs /
logging.rs / data_tagging.rs.

**Tests:** mock-plugin tests.

**Status:** done

### 7.3 Plugin discovery via properties

**Spec:** §7.3.

**Repo:** `crates/security/src/properties.rs` with a property map + plugin
config.

**Tests:** property tests.

**Status:** done

---

## §8 Builtin plugins — Authentication (DDS:Auth:PKI-DH)

### 8.1 Authentication SPI

**Spec:** §8.1.

**Repo:** `crates/security/src/authentication.rs`.

**Tests:** auth-trait tests.

**Status:** done

### 8.2 Builtin Authentication: PKI-DH (3-way handshake)

**Spec:** §8.2 — validate_local_identity, validate_remote_identity,
begin/process_handshake_request/reply.

**Repo:** `crates/security-pki/src/{identity,handshake_token,plugin}.rs`.

**Tests:** PKI-DH tests.

**Status:** done

### 8.3 IdentityCertificate (X.509)

**Spec:** §8.3.

**Repo:** `security-pki/src/identity.rs` with X.509 validation.

**Tests:** identity tests.

**Status:** done

### 8.4 IdentityCA validation (cert-chain verification)

**Spec:** §8.4.

**Repo:** `security-pki/src/identity.rs` with a cert-chain walker.

**Tests:** —

**Status:** done

### 8.5 Handshake tokens (BinaryProperty with RSA/ECDSA sign)

**Spec:** §8.5.

**Repo:** `security-pki/src/handshake_token.rs`.

**Tests:** handshake-token tests.

**Status:** done

### 8.6 Shared secret (X25519/ECDH)

**Spec:** §8.6.

**Repo:** `security-keyexchange/src/lib.rs` (X25519) + `rsa_wrap.rs`
(RSA-OAEP).

**Tests:** keyexchange tests.

**Status:** done

### 8.7 PSK authentication

**Spec:** §8.7.

**Repo:** `security-pki/src/psk.rs` + `security-crypto/src/psk_plugin.rs`.

**Tests:** PSK tests.

**Status:** done

### 8.8 OCSP/CRL revocation checks

**Spec:** §8.8.

**Repo:** OCSP stapling in `security-pki/src/ocsp.rs` (`parse_ocsp_status` +
`require_good_status`); CRL validation in `security-pki/src/crl.rs`
(`parse_crl_serials` + `validate_crl` with a DER walker for the RFC-5280
`CertificateList`).

**Tests:** OCSP (`ocsp::tests::*`, 12 tests): `empty_input_is_malformed`,
`good_status_parses_to_good`, `good_tag_requires_zero_length`,
`prefix_bytes_before_sequence_are_skipped`, `require_good_accepts_good`,
`require_good_rejects_malformed`,
`require_good_rejects_revoked_with_auth_failed`, `require_good_rejects_unknown`,
`revoked_tag_81_parses_to_revoked`, `revoked_tag_a1_parses_to_revoked`,
`sequence_tag_recognized_via_equality`, `unknown_tag_82_parses_to_unknown`.
CRL (`crl::tests::*`, 24 tests):
`parse_error_messages_are_specific_per_variant`,
`parse_serials_empty_revocation_list`, `parse_serials_handles_long_form_length`,
`parse_serials_handles_long_serial`,
`parse_serials_keeps_leading_zero_byte_for_positive_serials`,
`parse_serials_rejects_empty_input`, `parse_serials_rejects_indefinite_length`,
`parse_serials_rejects_non_sequence_outer`, `parse_serials_returns_all_revoked`,
`read_length_0x80_is_long_form_marker_not_short`,
`read_length_buf_exactly_one_plus_n_accepted`,
`read_length_buf_one_plus_n_minus_one_truncated`,
`read_length_n_equals_four_accepted`, `read_length_rejects_n_greater_than_four`,
`read_length_three_byte_length_correct`,
`read_length_two_byte_length_high_byte_first`,
`try_parse_revoked_list_rejects_non_time_tag`,
`validate_crl_against_empty_list_passes`,
`validate_crl_empty_input_returns_bad_argument`,
`validate_crl_known_revoked_rejects`, `validate_crl_signature_invalid_rejects`,
`validate_crl_truncated_input_returns_bad_argument`,
`validate_crl_unknown_serial_passes`, `validate_crl_with_two_revoked_finds_second`.

**Status:** done — the OCSP-stapling path live; the CRL fallback live with a
positive (revoked-rejects) AND a negative (unknown-passes) test plus
malformed defense.

---

## §9 Builtin plugins — Access Control (DDS:Access:Permissions)

### 9.1 AccessControl SPI

**Spec:** §9.1.

**Repo:** `crates/security/src/access_control.rs`.

**Tests:** —

**Status:** done

### 9.2 Builtin Access Control via signed Permissions-XML + Governance-XML

**Spec:** §9.2.

**Repo:** `security-permissions/src/{governance,plugin,signature}.rs`.

**Tests:** permissions tests.

**Status:** done

### 9.3 Permissions-XML (Allow/Deny per Domain/Topic/Partition)

**Spec:** §9.3.

**Repo:** `security-permissions/src/xml.rs` + `topic_match.rs`.

**Tests:** Permissions-XML tests.

**Status:** done

### 9.4 Governance-XML (domain-wide policies: Discovery/Liveliness/RTPS protection kinds)

**Spec:** §9.4.

**Repo:** `security-permissions/src/governance.rs` with a ProtectionKind enum
(NONE/SIGN/ENCRYPT/SIGN_WITH_ORIGIN_AUTH/etc.).

**Tests:** governance tests.

**Status:** done

### 9.5 CMS/PKCS#7 signature verification

**Spec:** §9.5.

**Repo:** `security-permissions/src/cms.rs` + `signature.rs`.

**Tests:** CMS tests.

**Status:** done

### 9.6 Permission caching + check_create/check_remote

**Spec:** §9.6.

**Repo:** `security-permissions/src/plugin.rs` + `delegation_check.rs`.

**Tests:** permissions tests.

**Status:** done

### 9.7 PSK access

**Spec:** §9.7.

**Repo:** `security-permissions/src/psk_access.rs`.

**Tests:** PSK-access tests.

**Status:** done

---

## §10 Builtin plugins — Cryptographic (DDS:Crypto:AES-GCM-GMAC)

### 10.1 Cryptographic SPI

**Spec:** §10.1.

**Repo:** `crates/security/src/crypto.rs`.

**Tests:** —

**Status:** done

### 10.2 Builtin Crypto: AES128/AES256-GCM/GMAC

**Spec:** §10.2.

**Repo:** `security-crypto/src/{plugin,suite,session_key}.rs`.

**Tests:** crypto tests.

**Status:** done

### 10.3 KeyMaterial: master_key + master_salt + key_id (with version change)

**Spec:** §10.3.

**Repo:** `security-crypto/src/session_key.rs::SessionKey` + KeyMaterial wire.

**Tests:** session-key tests.

**Status:** done

### 10.4 Receiver-specific MAC (a per-reader MAC in addition to the common MAC)

**Spec:** §10.4.

**Repo:** `security-crypto/src/plugin.rs` with a receiver-specific-MAC path.

**Tests:** —

**Status:** done

### 10.5 PSK crypto plugin

**Spec:** §10.5.

**Repo:** `security-crypto/src/psk_plugin.rs`.

**Tests:** PSK-crypto tests.

**Status:** done

---

## §11 Builtin plugins — Logging (DDS:Logging:DDS_LogTopic)

### 11.1 Logging SPI

**Spec:** §11.1.

**Repo:** `crates/security/src/logging.rs`.

**Tests:** —

**Status:** done

### 11.2 Logging sinks (jsonl, syslog, stderr, fanout)

**Spec:** §11.2.

**Repo:** `crates/security-logging/src/{jsonl,syslog,stderr_sink,fanout}.rs`.

**Tests:** logging-sink tests.

**Status:** done

### 11.3 BuiltinLoggingType (Topic + Severity + Message)

**Spec:** §11.3.

**Repo:** `security-logging/src/lib.rs` with a BuiltinLoggingType struct.

**Tests:** —

**Status:** done

---

## §12 Builtin plugins — Data Tagging

### 12.0 DataTagging SPI

**Spec:** §12.

**Repo:** the SPI trait in `crates/security/src/data_tagging.rs`; the builtin
in `crates/security-runtime/src/data_tagging.rs` (`BuiltinDataTaggingPlugin`
+ a subset-match predicate + a `PID_PROPERTY_LIST` wire codec with the
namespace prefix `dds.sec.data_tags.`); a mock in `crates/security/src/mock.rs`
(`MockDataTaggingPlugin`).

**Tests:** `data_tagging::tests::*` (15 tests):
`decode_tags_skips_non_tag_properties`,
`empty_publisher_with_required_subscriber_rejects`,
`encode_tags_uses_namespace_prefix`, `match_empty_subscriber_is_wildcard`,
`match_full_set_passes`, `match_missing_required_tag_rejects`,
`match_subset_passes`, `match_unknown_subscriber_tag_rejects`,
`match_value_mismatch_rejects`, `plugin_class_id_matches_spec_format`,
`plugin_is_object_safe_via_dyn_trait`, `set_empty_clears_existing`,
`set_get_roundtrip`, `unknown_endpoint_returns_empty`,
`wire_roundtrip_via_property_list` +
`mock::tests::mock_data_tagging_set_get_roundtrip`.

**Status:** done — builtin in production, the wire path proven, the subset
match tested positively AND negatively.

---

## §13 RTPS wire protection

### 13.1 SecuredPayload (DATA with encryption + MAC)

**Spec:** §13.1.

**Repo:** `crates/security-rtps/src/{srtps,codec}.rs`.

**Tests:** SRTPS tests.

**Status:** done

### 13.2 SEC_PREFIX / SEC_BODY / SEC_POSTFIX (submessage wrapping)

**Spec:** §13.2.

**Repo:** `security-rtps/src/codec.rs` with submessage IDs 0x30/0x31/0x32.

**Tests:** submessage-wrapping tests.

**Status:** done

### 13.3 Receiver-specific MAC in the SEC_POSTFIX

**Spec:** §13.3.

**Repo:** `security-rtps/src/codec.rs::SecPostfix` with a ReceiverSpecificMacs
vec.

**Tests:** —

**Status:** done

### 13.4 ProtectionKind decision (NONE/SIGN/ENCRYPT/SIGN_WITH_ORIGIN_AUTH/ENCRYPT_WITH_ORIGIN_AUTH)

**Spec:** §13.4.

**Repo:** `security-permissions/src/governance.rs::ProtectionKind`.

**Tests:** —

**Status:** done

### 13.5 RTPS header protection (sign/encrypt the entire message)

**Spec:** §13.5.

**Repo:** `security-rtps/src/srtps.rs::rtps_header_protect/unprotect`.

**Tests:** —

**Status:** done

---

## §14 Discovery — builtin endpoints for the auth handshake

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

### 14.3 ParticipantStatelessMessage (auth-handshake topic)

**Spec:** §14.3.

**Repo:** `security-runtime/src/builtin_topics.rs::ParticipantStatelessMessage`.

**Tests:** auth-topic tests.

**Status:** done

### 14.4 ParticipantVolatileMessageSecure (crypto key distribution)

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

## §15 Plugin configuration via properties

### 15.1 PropertyQosPolicy with dds.sec.* properties

**Spec:** §15.1.

**Repo:** `crates/security/src/properties.rs` (PropertyKey constants).

**Tests:** property tests.

**Status:** done

### 15.2 Property-driven plugin selection

**Spec:** §15.2.

**Repo:** the properties→plugin mapping in `security-runtime/src/engine.rs`.

**Tests:** engine tests.

**Status:** done

---

## §16 Heterogeneous security (ZeroDDS-specific)

### 16.1 PolicyEngine: capability negotiation (Cyclone-compatible mode + strict mode)

**Spec:** §16 (ZeroDDS-specific extension; see DDS-Security-1.2 §9.2-9.4 as
the base).

**Repo:** `security-runtime/src/{engine,policy,caps,peer_class}.rs`.

**Tests:** heterogeneous tests.

**Status:** done — ZeroDDS-specific, covers the DDS-Security-1.2 spec.

### 16.2 Anti-squatter (identity-hijack prevention)

**Spec:** §16.

**Repo:** `security-runtime/src/anti_squatter.rs`.

**Tests:** anti-squatter tests.

**Status:** done

### 16.3 Gateway bridge (untrusted-trusted border)

**Spec:** §16.

**Repo:** `security-runtime/src/gateway_bridge.rs`.

**Tests:** —

**Status:** done

---

## §17 Logging + audit

### 17 Audit log (all plugin operations)

**Spec:** §17.

**Repo:** `security-logging/src/lib.rs` with audit records.

**Tests:** —

**Status:** done

---

## Annex: IDL definitions (builtin topic types + plugin SPIs)

### Annex-A IDL module dds::security (all builtin topic types)

**Spec:** Annex.

**Repo:** implemented via Rust structures in
`security-runtime/src/builtin_topics.rs`.

**Tests:** —

**Status:** done

### Annex-B plugin-trait IDLs

**Spec:** Annex.

**Repo:** Rust traits instead of IDL.

**Tests:** —

**Status:** done

---

## Audit status

50 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-security-runtime` — 214 tests green.
* `cargo test -p zerodds-security-pki` — 182 tests green.
* `cargo test -p zerodds-security-permissions` — 116 tests green.
* `cargo test -p zerodds-security-crypto` — 80 tests green.
* `cargo test -p zerodds-security-rtps` — 31 tests green.
* `cargo test -p zerodds-security-keyexchange` — 16 tests green.
* `cargo test -p zerodds-security-logging` — 16 tests green.

Cross-crate test volume: 655 tests against DDS-Security-1.2.
