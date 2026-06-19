# `zerodds-security-permissions`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security-permissions/badge.svg)](https://docs.rs/zerodds-security-permissions)

DDS-Security 1.1 §9.4 ("Builtin Access Control Plugin") for the
[ZeroDDS](https://zerodds.org) stack: permissions/governance XML parser
+ S/MIME-CMS signature verifier + topic wildcard match + delegation
chain + PSK profile. Safety classification: **SAFE**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG DDS-Security 1.1 | §9.4 (builtin access control), §10.4.1 (XML format) |
| OMG DDS-Security 1.2 | §10.4.1.1 (S/MIME-CMS), §10.8 (PSK profile) |
| RFC 5751/5652/5280 | S/MIME / CMS / X.509 |

## What's inside

- **`PermissionsAccessControl`** — `AccessControlPlugin` implementation.
- **`xml` module** — permissions XML parser.
- **`governance` module** — governance XML incl. the ZeroDDS extension namespace.
- **`signature` module** — `XmlSignatureVerifier` trait + `NoOpVerifier` (dev) + `EnvelopeCheckVerifier` + `open_signed_permissions`.
- **`cms` module** — production CMS/PKCS#7 verifier (RFC 5751/5652/5280) on `rustls-webpki`.
- **`topic_match` module** — wildcard `*`/`?`.
- **`delegation_check` module** — permissions delegation chain (4 trust policies).
- **`psk_access` module** — pre-shared-key access control (spec §10.8).

## Layer position

Layer 4. Consumes `zerodds-security`, `zerodds-security-pki`, `zerodds-security-crypto`.

## Quickstart

```rust,no_run
use zerodds_security_permissions::PermissionsAccessControl;
use zerodds_security_permissions::signature::NoOpVerifier;

let plugin = PermissionsAccessControl::new(NoOpVerifier);
```

## Stability

`1.0.0-rc.1`. Public API + XML schema + CMS wire format RC1-stable.

## Tests

```bash
cargo test -p zerodds-security-permissions
```

136+ tests + 3 integration suites green.

## Lizenz

Apache-2.0.
