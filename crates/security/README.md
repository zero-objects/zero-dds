# `zerodds-security`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-security/badge.svg)](https://docs.rs/zerodds-security)

DDS-Security 1.1 (formal/2018-04-01) plugin SPI for the
[ZeroDDS](https://zerodds.org) stack: trait definitions, token
data model, generic-message topics. Pure Rust + `alloc`. Safety
classification: **SAFE** (trust-neutral SPI layer).

## Spec mapping

| Spec | Trait / module | Concrete impl |
|------|---------------|---------------|
| §8.3 Authentication | `AuthenticationPlugin` | `zerodds-security-pki` |
| §8.4 Access Control | `AccessControlPlugin` | `zerodds-security-permissions` |
| §8.5 Cryptographic | `CryptographicPlugin` | `zerodds-security-crypto` |
| §8.6 Logging | `LoggingPlugin` | `zerodds-security-logging` |
| §8.7 Data Tagging | `DataTaggingPlugin` | `zerodds-security-runtime` |

Coverage doc: `docs/spec-coverage/dds-security-1.2.md` (50 done / 0 partial / 0 open / 1 n/a, K6 audit).

## What's inside

**Plugin traits** (object-safe, `Box<dyn Plugin>`-erasable):
- `AuthenticationPlugin` — identity validation + handshake.
- `AccessControlPlugin` — permissions check, topic allow/deny.
- `CryptographicPlugin` — encrypt/decrypt submessage + key material + receiver-specific MACs.
- `LoggingPlugin` — audit events.
- `DataTaggingPlugin` — built-in DataTagging (DDS-Security 1.2 §8.7).

**Token data model:**
- `IdentityToken`, `PermissionsToken`, `CryptoToken`, `IdentityStatusToken`.
- `DataHolder`, `BinaryProperty`, `WireProperty`.

**Generic messages** (DCPSParticipantStatelessMessage + DCPSParticipantVolatileMessageSecure):
- `ParticipantGenericMessage`, `MessageIdentity`.
- Topic constants: `TOPIC_STATELESS_MESSAGE`, `TOPIC_VOLATILE_MESSAGE_SECURE`, `TYPE_NAME_GENERIC_MESSAGE`.

**Cross-cutting:**
- `Property`, `PropertyList` — plugin configuration via `<participant_qos><property>`.
- `security_topic_qos` — built-in security-topic QoS profiles (§7.4.5).
- `SecurityError` — all plugin errors.
- `mock` (feature `std`) — test mock plugins.

## Layer position

Layer 4 — Core Services (SPI crate). Pure Rust + `alloc`, **no** ZeroDDS crate deps. Consumed by 7 further security crates (`security-pki`, `-crypto`, `-keyexchange`, `-permissions`, `-logging`, `-rtps`, `-runtime`) plus by `zerodds-discovery` (built-in endpoint slots) and `zerodds-dcps` (feature `security`).

## Quickstart

```rust,ignore
use zerodds_security::{AuthenticationPlugin, AccessControlPlugin};
use zerodds_security::mock::MockAuthenticationPlugin;

let auth: Box<dyn AuthenticationPlugin> = Box::new(MockAuthenticationPlugin::new());
// Use auth.validate_local_identity(...), auth.begin_handshake_request(...) etc.
```

Production use cases build the real plugins (`security-pki`, etc.) and plug them into the DCPS participant via `Box<dyn Plugin>`.

## Feature flags

| Feature | Default | Purpose |
|---------|---------|-------|
| `std` | ✅ | Mutex + thread-safe mock |
| `alloc` | ✅ via std | `Vec`/`String` |
| `safety` | ❌ | reserved hook |

## Stability

`1.0.0-rc.1` is **API-frozen** — breaking changes require a v2.0 major bump. Semver patch + minor may only add new methods with a default body or non-breaking enum variants. This frozen pledge is binding, because 7 sister crates + dcps + discovery depend on this SPI.

## Tests

```bash
cargo test -p zerodds-security
```

39 unit tests + 1 doc test green.

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- `docs/spec-coverage/dds-security-1.2.md` — spec coverage doc.
- [`zerodds-security-pki`](../security-pki) — X.509 + RSA-PSS + ECDSA + OCSP/CRL authentication.
- [`zerodds-security-crypto`](../security-crypto) — AES-GCM/HMAC cryptographic plugin.
- [`zerodds-security-permissions`](../security-permissions) — Governance + Permissions XML.
- [`zerodds-security-rtps`](../security-rtps) — RTPS header AAD wrapper.
- [`zerodds-security-runtime`](../security-runtime) — plugin runtime + built-in DataTagging.
