# `zerodds-security-permissions`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-permissions/badge.svg)](https://docs.rs/zerodds-security-permissions)

DDS-Security 1.1 §9.4 ("Builtin Access Control Plugin") fuer den
[ZeroDDS](https://zerodds.org)-Stack: Permissions/Governance-XML-Parser
+ S/MIME-CMS-Signatur-Verifier + Topic-Wildcard-Match + Delegation-
Chain + PSK-Profile. Safety classification: **SAFE**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG DDS-Security 1.1 | §9.4 (Builtin Access Control), §10.4.1 (XML-Format) |
| OMG DDS-Security 1.2 | §10.4.1.1 (S/MIME-CMS), §10.8 (PSK-Profile) |
| RFC 5751/5652/5280 | S/MIME / CMS / X.509 |

## Was ist drin

- **`PermissionsAccessControl`** — `AccessControlPlugin`-Implementation.
- **`xml`-Modul** — Permissions-XML-Parser.
- **`governance`-Modul** — Governance-XML inkl. ZeroDDS-Extension-Namespace.
- **`signature`-Modul** — `XmlSignatureVerifier`-Trait + `NoOpVerifier` (Dev) + `EnvelopeCheckVerifier` + `open_signed_permissions`.
- **`cms`-Modul** — produktiver CMS/PKCS#7-Verifier (RFC 5751/5652/5280) auf `rustls-webpki`.
- **`topic_match`-Modul** — Wildcard `*`/`?`.
- **`delegation_check`-Modul** — Permissions-Delegation-Chain (4 Trust-Policies).
- **`psk_access`-Modul** — Pre-Shared-Key-Access-Control (Spec §10.8).

## Schichten-Position

Layer 4. Konsumiert `zerodds-security`, `zerodds-security-pki`, `zerodds-security-crypto`.

## Quickstart

```rust,no_run
use zerodds_security_permissions::PermissionsAccessControl;
use zerodds_security_permissions::signature::NoOpVerifier;

let plugin = PermissionsAccessControl::new(NoOpVerifier);
```

## Stabilitaet

`1.0.0-rc.1`. Public-API + XML-Schema + CMS-Wire-Format RC1-stabil.

## Tests

```bash
cargo test -p zerodds-security-permissions
```

136+ Tests + 3 Integration-Suites grün.

## Lizenz

Apache-2.0.
