# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- `certificate_algorithm_name`: the name a token carries for the signature algorithm
  of a certificate, of Table 25 or of DDS-Security 1.1.

- `PkiAuthenticationPlugin::get_permissions_token` (the token without one from the
  AccessControl plugin) has the class `DDS:Access:Permissions:1.2` (DDS-Security 1.2
  Table 65); it was `1.0`.

### Changed

- `dds.cert.sn` and `dds.ca.sn` of the IdentityToken are RFC 4514 by default
  (DDS-Security 1.2 §9.4.1.2.3); they were written in the `XN_FLAG_ONELINE` form.
  `format_subject_name` and `build_identity_token_from_der_with` write the forms
  of `SubjectNameForm`; `PkiAuthenticationPlugin::set_subject_name_form`.

### Changed (DDS-Security 1.2 §7)

- BREAKING (names): `dds.sec.auth.crl` is `org.zerodds.sec.auth.crl`, `dds.sec.auth.zerodds.spec_deviation.accept_x509v1_identity_certificate` is `org.zerodds.sec.auth.accept_x509v1_identity_certificate`, the class ids `ZeroDDS:Auth:PSK-Handshake:1.0[+AuthReq|+AuthReply|+AuthFinal]` are `org.zerodds.auth.psk-handshake:1.0[...]` (§7.3.2, §7.3.5.1: a name outside the specification starts with a reversed domain name).


### Changed

- `PkiAuthenticationPlugin`, `PskAuthenticationPlugin` and
  `PskHandshakeAuthenticationPlugin` implement the operations of Table 29 of
  DDS-Security 1.2 (see `zerodds-security`): `validate_remote_identity` orders
  the GUIDs and returns `VALIDATION_PENDING_HANDSHAKE_REQUEST` or `..._MESSAGE`,
  compares the plugin name and the major version of the IdentityToken, and
  makes the AuthRequestMessageToken; the initiator's `process_handshake` of the
  reply returns `VALIDATION_OK_FINAL_MESSAGE`; the handshake operations refuse
  handles that `validate_remote_identity` did not return for them
  (PreconditionError); the AuthenticatedPeerCredentialToken (`c.id`, `c.perm`)
  exists once the handshake completed; the return operations take back what
  was issued.
- The IdentityToken carries the `CryptoAlgorithmName`s of Table 25 in
  `dds.cert.algo` and `dds.ca.algo`, all four properties optional;
  `c.dsign_algo` and `c.kagree_algo` carry those of Table 25 and 26. Peers that
  read the names of 1.1 (Cyclone, Fast DDS, OpenDDS by `VendorId`) are spoken to
  in those; `zerodds.sec.auth.algorithm_names` = `auto` | `spec` | `legacy`.
- `validate_local_identity` returns the `adjusted_participant_guid` of Table 59
  and `VALIDATION_PENDING_RETRY` for a certificate that names an OCSP responder
  (checked on a thread, called again until it has the answer).
  `PkiAuthenticationPlugin::validate_identity` is the blocking variant for a
  caller that holds the identity material.
- `DDS:Auth:PSK` follows Table 83 (`set_participant_security_config`,
  `set_permissions_credential_and_token`, `set_listener`, `return_identity_*`).

### Added

- `set_participant_security_config` (Table 59): checks and adjusts the
  algorithm masks and limits the handshake to the supported algorithms.
- `set_permissions_credential_and_token`: the `dds.perm.cert` of the
  PermissionsCredentialToken is the `c.perm` of the handshake.
- `AuthenticationListener` support: `revoke_identity` and the
  `check_identities` tick (expired certificates, new CRLs) call
  `on_revoke_identity`; a changed IdentityStatusToken calls
  `on_status_changed(IDENTITY_STATUS)`.
- `get_identity_status_token` returns the IdentityStatusToken of Table 53
  (`ocsp_status`).
- OCSP responders over `https://` (feature `ocsp-https`, default on; rustls,
  server certificate against the identity CA): `query_responder` takes the
  trust anchors.
- `PskHandshakeAuthenticationPlugin::validate_local_psk_identity_for`.
- RSASSA-PSS handshake signatures of any salt length verify (§8.2 Table 25). `AlgorithmNames` and `set_algorithm_names`: the initiator sends the names of Tables 25 and 26 by default; `auto` and `1.1` are settings.

### Added (DDS-Security 1.2, §7 package)

- A `CryptoAlgorithmName` over 64 characters is refused (§7.3.6); the
  algorithm names carry the NUL by default (§7.3.3.1); `AlgorithmMasks` hold the
  signature algorithm of the permissions document; `get_identity_status_token`
  (Table 53).


### Fixed

- The handshake runs over `DHE+MODP-2048-256` (`DH+MODP-2048-256` in the
  request, as the other vendors name it; both names are taken) and announces
  its bit in the key establishment info.
- The handshake signs and verifies with `ECDSA+P384+SHA384` (P-384
  identities, `CertKeyAlgo::EcdsaP384Sha384`) and `RSASSA-PKCS1-V1_5+2048+SHA256`
  (DDS-Security 1.2 §8.2 Table 25): an RSA identity signs RSASSA-PSS, or
  RSASSA-PKCS1-v1_5 when the peer announced that it verifies only that one
  (`message_auth.supported_mask`, via
  `AuthenticationPlugin::set_remote_message_auth_supported`); the replier
  follows the initiator; `c.dsign_algo` has to fit the certificate.
- `PkiAuthenticationPlugin::algorithm_masks`: trust chain (documents:
  RSASSA-PSS, RSASSA-PKCS1, ECDSA P-256 and P-384; required: the signature
  algorithm of the identity certificate), handshake signature (RSASSA-PSS and
  ECDSA P-256; required: that of the identity key), key agreement (ECDHE-CEUM
  P-256 and P-384; required: the preferred one).
- `PskAuthenticationPlugin` is `DDS:Auth:PSK` of DDS-Security 1.2 §10.7 and
  Table 83: no authentication, `validate_remote_identity` always fails, no
  handshake tokens, IdentityToken class `DDS:Auth:PSK:1.2`. The three-message
  PSK handshake that carried that name is
  `PskHandshakeAuthenticationPlugin`, with the classes
  `ZeroDDS:Auth:PSK-Handshake:1.0[+AuthReq|+AuthReply|+AuthFinal]`. Callers of
  the handshake use the new name; `PskProfile::auth` is of that type.

- OCSP (DDS-Security 1.2 §10.3.3 Table 59, §10.3.2.3): `ocsp_status` of a
  request or reply token is read as an `OCSPResponse` and checked (the
  `CertID` names the certificate and its issuer, the validity interval, the
  signature of the CA or of a responder certificate the CA issued for OCSP
  signing); `revoked` fails the handshake, a response that does not hold is
  as none. Without one, the responder named in the certificate's Authority
  Information Access is asked over HTTP, for remote certificates and for the
  local one; its `good` response is sent as our `ocsp_status`. A status that
  cannot be determined refuses the certificate unless
  `zerodds.sec.auth.ocsp_unreachable=accept`. The earlier module scanned the
  response for a tag and had no caller.

- `encrypted_key::PasswordReading` and `decrypt_private_key_pem_as`: the
  password is read as the clause's AES-128 key (default) or also as an
  OpenSSL passphrase (DDS-Security 1.2 §10.3.1 Table 50).

- `validate_remote_identity` takes a DataHolder of the class
  `DDS:Auth:PKI-DH:1.0` or `:1.2` as the remote IdentityToken and refuses
  anything else: a class that only starts like it (`DDS:Auth:PKI-DHX:1.0`),
  another version, another plugin's class, and a certificate in DER, which was
  checked as a certificate (DDS-Security 1.2 §10.3.2.1, Table 51).

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
