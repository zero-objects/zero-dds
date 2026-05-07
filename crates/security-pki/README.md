# `zerodds-security-pki`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-pki/badge.svg)](https://docs.rs/zerodds-security-pki)

PKI/X.509-Backend fuer den DDS-Security
[ZeroDDS](https://zerodds.org)-`AuthenticationPlugin` nach OMG
DDS-Security 1.1 §8.3. Wrapper um `rustls-webpki` + `ring` — kein
eigener Raw-Crypto-Code. Safety classification: **SAFE**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS-Security 1.1 | §8.3, §9.3, §10.3 |
| OMG DDS-Security 1.2 | §10.7 + §10.8 (PSK-Profile) |
| RFC 5280 | X.509 Cert-Chain |
| RFC 6960 | OCSP |
| ZeroDDS-Architektur §09 | Delegation-Chain |

## Was ist drin

- `PkiAuthenticationPlugin`, `PskAuthenticationPlugin`.
- `IdentityConfig`, `IdentityHandle`, `IdentityToken`, `IdentityStatusToken`.
- `HandshakeToken`, `HandshakeError`, `HandshakeStepOutcome`, `AuthRequestMessage`.
- `ocsp` (RFC 6960 Stapling-Validation).
- `crl` (RFC 5280 §5 + Cache).
- `delegation::{DelegationLink, DelegationChain, SignatureAlgorithm}` — ECDSA-P256/P384, RSA-PSS-2048, Ed25519.

## Schichten-Position

Layer 4. Konsumiert `zerodds-security` + `zerodds-security-keyexchange`. Konsumenten: `zerodds-security-permissions` (DelegationChain), `zerodds-security-runtime`, `dcps` (Feature `security`).

## Quickstart

```rust,ignore
use zerodds_security_pki::{PkiAuthenticationPlugin, IdentityConfig};

let mut plugin = PkiAuthenticationPlugin::new();
let cfg = IdentityConfig {
    identity_cert_pem: alice_cert.into(),
    identity_ca_pem: ca_pem.into(),
    identity_key_pem: Some(alice_key_pkcs8_pem.into()),
};
let local = plugin.validate_with_config(cfg, [0xAA; 16])?;
```

## Stabilitaet

`1.0.0-rc.1`. Public-API + Wire-Format RC1-stabil; Cross-Vendor zu Cyclone/FastDDS.

## Tests

```bash
cargo test -p zerodds-security-pki
```

197 Tests grün.

## Lizenz

Apache-2.0.
