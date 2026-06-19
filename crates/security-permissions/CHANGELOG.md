# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

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
