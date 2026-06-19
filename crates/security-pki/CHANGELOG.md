# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-security-pki` crate.

### Spec references

- **OMG DDS-Security 1.1** §8.3, §9.3, §10.3.
- **OMG DDS-Security 1.2** §10.7 + §10.8 (PSK profile).
- RFC 5280 (X.509), RFC 6960 (OCSP).
- ZeroDDS architecture §09 (delegation chain).

### Public API

- `PkiAuthenticationPlugin` (`AuthenticationPlugin` impl).
- `PskAuthenticationPlugin`.
- `IdentityConfig`, `IdentityHandle`, `IdentityToken`, `IdentityStatusToken`.
- `HandshakeToken`, `HandshakeError`, `HandshakeStepOutcome`.
- `AuthRequestMessage`.
- `ocsp::*` (OCSP stapling validation).
- `crl::*` (CRL cache).
- `delegation::{DelegationLink, DelegationChain, SignatureAlgorithm}`.

### Implementation

`PkiAuthenticationPlugin::validate_with_config` parses PEM cert + PEM CA + PKCS8 private key and validates the signature chain via `rustls-webpki`. Remote validation runs analogously over DER tokens. The `HandshakeToken` state machine implements spec §9.3.2.4 challenge/response with X25519 ephemeral DH; the SharedSecret output feeds `zerodds-security-crypto` via the `SharedSecretProvider` bridge.

`ocsp.rs` parses DER OCSP responses and returns `OcspStatus::{Good, Revoked, Unknown, Malformed}` by issuer + serial match. `crl.rs` holds a cache + online fetch.

`delegation.rs` implements four signature algorithms (ECDSA-P256, ECDSA-P384, RSA-PSS-2048, Ed25519) over `ring`; `DelegationChain::sign_link` produces a signed chain that `delegation_check` in `security-permissions` validates as a permission against trust anchors.

`forbid(unsafe_code)`. Cross-vendor wire compat for `IdentityToken` + `HandshakeToken` per spec §10.3 byte-exact with Cyclone/FastDDS.

### Architecture

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security`, `zerodds-security-keyexchange`, `rustls-pki-types`, `rustls-webpki`, `ring`, `x509-cert`.
- **Dependents (out):** `zerodds-security-permissions` (DelegationChain), `zerodds-security-runtime`, end-user builds, `dcps` (feature `security`).
- **Feature flags:** `std` (default).

### Stability

Public API + wire format RC1-stable.
