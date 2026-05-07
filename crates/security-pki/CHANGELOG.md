# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-security-pki`-Crate.

### Spec-Referenzen

- **OMG DDS-Security 1.1** §8.3, §9.3, §10.3.
- **OMG DDS-Security 1.2** §10.7 + §10.8 (PSK-Profile).
- RFC 5280 (X.509), RFC 6960 (OCSP).
- ZeroDDS-Architektur §09 (Delegation-Chain).

### Public-API

- `PkiAuthenticationPlugin` (`AuthenticationPlugin`-Impl).
- `PskAuthenticationPlugin`.
- `IdentityConfig`, `IdentityHandle`, `IdentityToken`, `IdentityStatusToken`.
- `HandshakeToken`, `HandshakeError`, `HandshakeStepOutcome`.
- `AuthRequestMessage`.
- `ocsp::*` (OCSP-Stapling-Validation).
- `crl::*` (CRL-Cache).
- `delegation::{DelegationLink, DelegationChain, SignatureAlgorithm}`.

### Implementierung

`PkiAuthenticationPlugin::validate_with_config` parst PEM-Cert + PEM-CA + PKCS8-Private-Key und validiert die Signatur-Kette via `rustls-webpki`. Remote-Validation laeuft analog ueber DER-Tokens. `HandshakeToken`-State-Machine implementiert Spec §9.3.2.4 Challenge/Response mit X25519-Ephemeral-DH; SharedSecret-Output speist `zerodds-security-crypto` per `SharedSecretProvider`-Bruecke.

`ocsp.rs` parst DER-OCSP-Responses und liefert `OcspStatus::{Good, Revoked, Unknown, Malformed}` per Issuer-+Serial-Match. `crl.rs` haelt einen Cache + Online-Fetch.

`delegation.rs` implementiert vier Signatur-Algorithmen (ECDSA-P256, ECDSA-P384, RSA-PSS-2048, Ed25519) ueber `ring`; `DelegationChain::sign_link` produziert eine signierte Kette, die `delegation_check` in `security-permissions` als Berechtigung gegen Trust-Anchors validiert.

`forbid(unsafe_code)`. Cross-Vendor-Wire-Compat fuer `IdentityToken` + `HandshakeToken` per Spec §10.3 byte-genau zu Cyclone/FastDDS.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security`, `zerodds-security-keyexchange`, `rustls-pki-types`, `rustls-webpki`, `ring`, `x509-cert`.
- **Dependents (out):** `zerodds-security-permissions` (DelegationChain), `zerodds-security-runtime`, end-user-Builds, `dcps` (Feature `security`).
- **Feature-Flags:** `std` (default).

### Stabilitaet

Public-API + Wire-Format RC1-stabil.
