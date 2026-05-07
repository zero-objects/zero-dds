# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-security-permissions`-Crate.

### Spec-Referenzen

- **OMG DDS-Security 1.1** §9.4 + §10.4.1.
- **OMG DDS-Security 1.2** §10.4.1.1 + §10.8.
- RFC 5751 (S/MIME), RFC 5652 (CMS), RFC 5280 (X.509).

### Public-API

- `PermissionsAccessControl` (`AccessControlPlugin`-Impl).
- `PskPermissionsAccessControl` (PSK-Profile, Spec §10.8).
- `xml::*` — Permissions-XML-Parser.
- `governance::*` — Governance-XML-Parser inkl. ZeroDDS-Extension-Namespace (Peer-Classes, Edge-Identities, Delegation-Profiles).
- `signature::{XmlSignatureVerifier, NoOpVerifier, EnvelopeCheckVerifier, open_signed_permissions, SignedPermissionsXml}`.
- `cms::{CmsVerifier, CmsError}` — produktiver PKCS#7-Verifier.
- `topic_match::topic_match`.
- `delegation_check::{validate_chain, ValidatedChain, DelegationProfile, TrustAnchor, TrustPolicy, DelegationCheckError}`.
- `psk_access::{PskPermissionsAccessControl, CLASS_ID_PSK_PERMISSIONS, PROP_PSK_*}`.

### Implementierung

`PermissionsAccessControl::validate_*_permissions` parst das per `<DomainParticipantQos><property>` gelieferte Permissions/Governance-XML, prueft die S/MIME-CMS-Signatur via `XmlSignatureVerifier` (Production: `CmsVerifier` mit `rustls-webpki`), und legt einen Slot pro Subject ab. `check_*_topic`-Calls evaluieren Wildcard-Pattern + Validity-Period + Default-Deny.

`CmsVerifier` deckt sowohl `multipart/signed` (detached) als auch `application/pkcs7-mime; smime-type=signed-data` (opaque) ab. ASN.1-DER-Decoding via `cms`/`x509-cert`/`der`-Crates; Cert-Chain-Validation via `rustls-webpki`; Signatur-Verify (ECDSA-P256, RSA-PKCS1-v1.5, RSA-PSS) via `ring`.

`delegation_check::validate_chain` implementiert die 7-Punkte-Validation: Chain-Kontinuitaet, Origin-Match, Trust-Anchor-Match, Signatur-Kette, Zeitfenster, Max-Chain-Depth, Scope-Intersection. Vier Trust-Policy-Modi.

`PskPermissionsAccessControl` ist die PSK-Profile-Variante (Spec §10.8): Permissions-XML wird **direkt** als String ohne S/MIME-Wrap geladen, weil der PSK-Pfad keine Permissions-CA hat — Authentizitaet stammt aus dem Pre-Shared-Key.

ZeroDDS-Extensions im Governance-Namespace: `<zerodds:peer_class>`, `<zerodds:edge_identity>`, `<zerodds:delegation_profiles>` — additive Vendor-Extensions fuer Heterogeneous-Mesh-Setups (Vehicle ↔ C4I-Backend).

`forbid(unsafe_code)`. 

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security`, `zerodds-security-pki`, `zerodds-security-crypto`, `roxmltree`, `cms`, `x509-cert`, `der`, `const-oid`, `sha2`, `ring`, `rustls-pki-types`, `rustls-webpki`.
- **Dependents (out):** `zerodds-security-runtime` (Plugin-Lifecycle), end-user-Builds, `dcps` (Feature `security`).
- **Feature-Flags:** `std` (default).

### Stabilitaet

Public-API + XML-Schema (Permissions/Governance) + CMS-Wire-Format + ZeroDDS-Extensions-Namespace RC1-stabil.
