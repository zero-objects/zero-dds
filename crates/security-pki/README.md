# `zerodds-security-pki`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-pki/badge.svg)](https://docs.rs/zerodds-security-pki)

PKI/X.509 backend for the DDS-Security
[ZeroDDS](https://zerodds.org) `AuthenticationPlugin` per OMG
DDS-Security 1.1 §8.3. Wrapper around `rustls-webpki` + `ring` — no
own raw-crypto code. Safety classification: **SAFE**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-Security 1.1 | §8.3, §9.3, §10.3 |
| OMG DDS-Security 1.2 | §10.7 + §10.8 (PSK profile) |
| RFC 5280 | X.509 cert chain |
| RFC 6960 | OCSP |
| ZeroDDS architecture §09 | delegation chain |

## What's inside

- `PkiAuthenticationPlugin`, `PskAuthenticationPlugin`.
- `IdentityConfig`, `IdentityHandle`, `IdentityToken`, `IdentityStatusToken`.
- `HandshakeToken`, `HandshakeError`, `HandshakeStepOutcome`, `AuthRequestMessage`.
- `ocsp` (RFC 6960 stapling validation).
- `crl` (RFC 5280 §5 + cache).
- `delegation::{DelegationLink, DelegationChain, SignatureAlgorithm}` — ECDSA-P256/P384, RSA-PSS-2048, Ed25519.

## Layer position

Layer 4. Consumes `zerodds-security` + `zerodds-security-keyexchange`. Consumers: `zerodds-security-permissions` (DelegationChain), `zerodds-security-runtime`, `dcps` (feature `security`).

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

## Stability

`1.0.0-rc.1`. Public API + wire format RC1-stable; cross-vendor with Cyclone/FastDDS.

## Tests

```bash
cargo test -p zerodds-security-pki
```

197 tests green.

## License

Apache-2.0.
