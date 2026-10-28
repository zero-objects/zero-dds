# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- The builtin and the PSK access-control plugin share `give_back` and the four
  `return_*_security_config` operations (`issued.rs`); the error names the plugin as before
  (`permissions:`, `psk-access:`).

### Changed

- `dds.perm_ca.sn` of the PermissionsToken follows `SubjectNameForm` (RFC 4514 by
  default); `PermissionsAccessControl::set_subject_name_form`.
- `dds.perm_ca.algo` of the PermissionsToken follows `AlgorithmNames`: the name of
  DDS-Security 1.2 Table 25 by default, `EC-prime256v1` / `RSA-2048` of 1.1 with
  `V1_1` (`org.zerodds.sec.algorithm_names=1.1`), which Fast DDS compares with its
  own value; `PermissionsAccessControl::set_algorithm_names`.

### Changed (DDS-Security 1.2 §7)

- BREAKING (property names): `dds.sec.access.{psk_governance,psk_permissions,subject_name}` are `org.zerodds.sec.access.*` (§7.3.2).


### Changed

- `PermissionsAccessControl` and `PskPermissionsAccessControl` implement every
  operation of Table 39 as Tables 70 and 85 describe it; the permissions token
  class is `DDS:Access:Permissions:1.2` (Table 65), `1.0` through the setting
  `org.zerodds.sec.access.permissions_token_class_id`.
- The governance parser follows the XSD of §10.4.1.2.3: root `<dds>` (also in
  the OMG namespace), one `<domain_access_rules>`, exact enumeration values,
  `enable_key_revision`, `rtps_psk_protection_kind`, `allowed_crypto_algorithms`,
  `must_interpret` for unknown elements and attributes, in the permissions
  document too. A document outside the XSD is invalid; the strict root requires
  `<dds>` around the governance of every test and configuration.
- `Grant::allows_topic` counts `<relay>` as well, so a relay-only reader's topic
  passes `check_remote_topic`.
- The CMS verification of an RSASSA-PSS signer reads the salt length from the signature (§8.2 Table 25).

### Added (DDS-Security 1.2, §7 package)

- `CmsPkcs7Verifier::signature_algorithm_oid` reads the signature algorithm
  of a signed document.


### Fixed

- `PskProfile::crypto` is a `PskDerivedCryptoPlugin`
  (`ZeroDDS:Crypto:PSK-Derived:1.0`), not `DDS:Crypto:PSK`.

- `PskPermissionsAccessControl` is `DDS:Access:PSK` as DDS-Security 1.2 §10.8
  and Table 85 describe it: `validate_local_permissions` and
  `validate_remote_permissions` always succeed and need no document, a token of
  any class is accepted, the PermissionsToken has the class
  `DDS:Access:PSK:1.2`, and `dds.sec.access.rtps_psk_protection_kind` (NONE,
  SIGN, ENCRYPT; ENCRYPT when absent) gives `is_rtps_psk_protected` and
  `is_rtps_psk_encrypted`. A token of the spec class was refused for want of a
  `dds.psk.permissions_id`, and a missing document was a configuration error.

- `PskProfile::auth` is a `PskHandshakeAuthenticationPlugin`
  (`zerodds-security-pki`): the handshake of ZeroDDS, not the NOOP plugin
  `DDS:Auth:PSK` of DDS-Security 1.2 §10.7.

- `parse_permissions_xml` and `parse_governance_xml` refuse a text that
  nests deeper than `zerodds_foundation::XML_NESTING_LIMIT` before the
  parser sees it; a long run of unclosed start tags overflowed the stack.
- `PermissionsAccessControl::validate_remote_permissions` verifies the
  S/MIME signature of the remote permissions document against the
  Permissions CA armed with the new `arm_remote_permissions_verifier`
  (DDS-Security 1.2 §9.4.1.2.1) and takes the grant of the remote identity
  certificate's subject (`remote_credential`, DER). It read the token as
  plain XML and took the first grant. Without an armed CA a remote token
  is refused.

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security-permissions` crate.

### Spec references

- **OMG DDS-Security 1.1** §9.4 + §10.4.1.
- **OMG DDS-Security 1.2** §10.4.1.1 + §10.8.
- RFC 5751 (S/MIME), RFC 5652 (CMS), RFC 5280 (X.509).

### Public API

- `PermissionsAccessControl` (`AccessControlPlugin` impl).
- `PskPermissionsAccessControl` (PSK profile, spec §10.8).
- `xml::*` — permissions XML parser.
- `governance::*` — governance XML parser incl. the ZeroDDS extension namespace (peer classes, edge identities, delegation profiles).
- `signature::{XmlSignatureVerifier, NoOpVerifier, EnvelopeCheckVerifier, open_signed_permissions, SignedPermissionsXml}`.
- `cms::{CmsVerifier, CmsError}` — production PKCS#7 verifier.
- `topic_match::topic_match`.
- `delegation_check::{validate_chain, ValidatedChain, DelegationProfile, TrustAnchor, TrustPolicy, DelegationCheckError}`.
- `psk_access::{PskPermissionsAccessControl, CLASS_ID_PSK_PERMISSIONS, PROP_PSK_*}`.

### Implementation

`PermissionsAccessControl::validate_*_permissions` parses the permissions/governance XML delivered via `<DomainParticipantQos><property>`, checks the S/MIME-CMS signature via `XmlSignatureVerifier` (production: `CmsVerifier` with `rustls-webpki`), and stores one slot per subject. `check_*_topic` calls evaluate wildcard patterns + validity period + default deny.

`CmsVerifier` covers both `multipart/signed` (detached) and `application/pkcs7-mime; smime-type=signed-data` (opaque). ASN.1-DER decoding via the `cms`/`x509-cert`/`der` crates; cert-chain validation via `rustls-webpki`; signature verify (ECDSA-P256, RSA-PKCS1-v1.5, RSA-PSS) via `ring`.

`delegation_check::validate_chain` implements the 7-point validation: chain continuity, origin match, trust-anchor match, signature chain, time window, max chain depth, scope intersection. Four trust-policy modes.

`PskPermissionsAccessControl` is the PSK profile variant (spec §10.8): the permissions XML is loaded **directly** as a string without an S/MIME wrap, because the PSK path has no permissions CA — authenticity stems from the pre-shared key.

ZeroDDS extensions in the governance namespace: `<zerodds:peer_class>`, `<zerodds:edge_identity>`, `<zerodds:delegation_profiles>` — additive vendor extensions for heterogeneous mesh setups (vehicle ↔ C4I backend).

`forbid(unsafe_code)`.

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security`, `zerodds-security-pki`, `zerodds-security-crypto`, `roxmltree`, `cms`, `x509-cert`, `der`, `const-oid`, `sha2`, `ring`, `rustls-pki-types`, `rustls-webpki`.
- **Dependents (out):** `zerodds-security-runtime` (plugin lifecycle), end-user builds, `dcps` (feature `security`).
- **Feature flags:** `std` (default).

### Stability

Public API + XML schema (permissions/governance) + CMS wire format + ZeroDDS extensions namespace RC1-stable.
